//! A coarse signed distance field per chunk, built from occupancy when the
//! chunk is meshed (#5911). The renderer atlases the fields and cone-traces
//! them for ambient occlusion that does not stop at the screen's edge.
//!
//! The field is [`FIELD_CELLS`]³ cells over the chunk's box, each holding
//! the signed Euclidean distance (in cells, negative inside) to the nearest
//! occupied cell, exact within [`REACH`] cells and clamped beyond. A cell is
//! occupied when any voxel in it is solid; occupancy comes from the chunk
//! and its resident neighbours, so distances near a chunk's border see past
//! it. Session densities are not Euclidean away from brush margins, so the
//! field never reads them.

/// Cells per chunk axis.
pub const FIELD_CELLS: usize = 8;
/// The farthest distance a field holds, in cells; the encoding clamps there
/// on both sides.
pub const REACH: f32 = 4.0;
/// Cells of occupancy around the chunk the transform sees, so border
/// distances are right out to the reach.
const PAD: usize = REACH as usize;
const GRID: usize = FIELD_CELLS + 2 * PAD;

/// A chunk's field: `FIELD_CELLS`³ bytes, x fastest, each
/// `(distance / REACH + 1) / 2` in 0..=255.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DistanceField {
    pub data: Vec<u8>,
}

impl DistanceField {
    /// The signed distance in cells a byte encodes.
    pub fn decode(byte: u8) -> f32 {
        (f32::from(byte) / 255.0 * 2.0 - 1.0) * REACH
    }

    fn encode(distance: f32) -> u8 {
        ((distance / REACH + 1.0) * 0.5 * 255.0)
            .round()
            .clamp(0.0, 255.0) as u8
    }

    /// The decoded distance at a cell.
    pub fn at(&self, cell: [usize; 3]) -> f32 {
        Self::decode(self.data[(cell[2] * FIELD_CELLS + cell[1]) * FIELD_CELLS + cell[0]])
    }
}

/// Build a chunk's field. `solid` answers whether an absolute voxel is
/// solid (absent neighbours answer false); `origin` and `size` are the
/// chunk's first voxel and voxel extent per axis, a multiple of
/// [`FIELD_CELLS`] so the cells tile the chunk.
pub fn build(solid: impl Fn([i64; 3]) -> bool, origin: [i64; 3], size: [i64; 3]) -> DistanceField {
    // Voxels per cell.
    let block: [i64; 3] = size.map(|extent| extent / FIELD_CELLS as i64);
    let mut occupied = vec![false; GRID * GRID * GRID];
    for z in 0..GRID {
        for y in 0..GRID {
            for x in 0..GRID {
                let cell = [x, y, z];
                let first: [i64; 3] = std::array::from_fn(|axis| {
                    origin[axis] + (cell[axis] as i64 - PAD as i64) * block[axis]
                });
                let mut any = false;
                'block: for dz in 0..block[2] {
                    for dy in 0..block[1] {
                        for dx in 0..block[0] {
                            if solid([first[0] + dx, first[1] + dy, first[2] + dz]) {
                                any = true;
                                break 'block;
                            }
                        }
                    }
                }
                occupied[(z * GRID + y) * GRID + x] = any;
            }
        }
    }
    let outside = squared_distances(&occupied, true);
    let inside = squared_distances(&occupied, false);
    let mut data = Vec::with_capacity(FIELD_CELLS * FIELD_CELLS * FIELD_CELLS);
    for z in 0..FIELD_CELLS {
        for y in 0..FIELD_CELLS {
            for x in 0..FIELD_CELLS {
                let index = ((z + PAD) * GRID + (y + PAD)) * GRID + (x + PAD);
                let distance = if occupied[index] {
                    -inside[index].sqrt()
                } else {
                    outside[index].sqrt()
                };
                data.push(DistanceField::encode(distance));
            }
        }
    }
    DistanceField { data }
}

/// Squared distance from every cell to the nearest cell whose occupancy is
/// `target` (exact Euclidean, Felzenszwalb and Huttenlocher's separable
/// transform), capped past the grid.
fn squared_distances(occupied: &[bool], target: bool) -> Vec<f32> {
    const FAR: f32 = (GRID * GRID * 4) as f32;
    let mut field: Vec<f32> = occupied
        .iter()
        .map(|&cell| if cell == target { 0.0 } else { FAR })
        .collect();
    let mut line = vec![0.0f32; GRID];
    let mut out = vec![0.0f32; GRID];
    for axis in 0..3 {
        let stride = GRID.pow(axis as u32);
        let others = [(axis + 1) % 3, (axis + 2) % 3];
        for a in 0..GRID {
            for b in 0..GRID {
                let mut base = [0usize; 3];
                base[others[0]] = a;
                base[others[1]] = b;
                let start = (base[2] * GRID + base[1]) * GRID + base[0];
                for (index, value) in line.iter_mut().enumerate() {
                    *value = field[start + index * stride];
                }
                transform_1d(&line, &mut out);
                for (index, value) in out.iter().enumerate() {
                    field[start + index * stride] = *value;
                }
            }
        }
    }
    field
}

/// One line of the squared distance transform: the lower envelope of the
/// parabolas each input raises.
fn transform_1d(input: &[f32], output: &mut [f32]) {
    let n = input.len();
    let mut vertices = vec![0usize; n];
    let mut boundaries = vec![0.0f32; n + 1];
    let mut k = 0;
    vertices[0] = 0;
    boundaries[0] = f32::NEG_INFINITY;
    boundaries[1] = f32::INFINITY;
    for q in 1..n {
        loop {
            let v = vertices[k];
            let s = ((input[q] + (q * q) as f32) - (input[v] + (v * v) as f32))
                / (2.0 * q as f32 - 2.0 * v as f32);
            if s <= boundaries[k] && k > 0 {
                k -= 1;
            } else {
                k += 1;
                vertices[k] = q;
                boundaries[k] = s;
                boundaries[k + 1] = f32::INFINITY;
                break;
            }
        }
    }
    k = 0;
    for (q, value) in output.iter_mut().enumerate() {
        while boundaries[k + 1] < q as f32 {
            k += 1;
        }
        let v = vertices[k];
        let d = q as f32 - v as f32;
        *value = d * d + input[v];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_floor_slab_measures_height_above_it_and_depth_into_it() {
        // A 16³ chunk at the origin whose bottom four voxels are solid: two
        // cells of floor, six of air.
        let field = build(|voxel| voxel[1] < 4, [0; 3], [16; 3]);
        assert!(
            field.at([3, 2, 3]) > 0.9 && field.at([3, 2, 3]) < 1.1,
            "{}",
            field.at([3, 2, 3])
        );
        assert!(
            (field.at([3, 5, 3]) - 4.0).abs() < 0.1,
            "{}",
            field.at([3, 5, 3])
        );
        assert!(field.at([3, 1, 3]) < 0.0, "inside is negative");
        assert!(
            field.at([3, 0, 3]) < field.at([3, 1, 3]),
            "deeper is more negative"
        );
        // Air out to the reach reads the reach.
        assert!((field.at([3, 7, 3]) - 4.0).abs() < 0.01);
    }

    #[test]
    fn a_pillar_in_a_neighbour_darkens_the_border() {
        // A solid pillar just past +x of the chunk: the chunk's own cells
        // next to it are close, the far side is not.
        let field = build(
            |voxel| (16..18).contains(&voxel[0]) && voxel[2] < 2,
            [0; 3],
            [16; 3],
        );
        assert!(field.at([7, 0, 0]) < 1.5, "{}", field.at([7, 0, 0]));
        assert!(field.at([0, 0, 0]) > 3.9, "{}", field.at([0, 0, 0]));
    }

    #[test]
    fn the_encoding_round_trips_the_reach() {
        for distance in [-4.0, -1.0, 0.0, 0.5, 2.0, 4.0] {
            let back = DistanceField::decode(DistanceField::encode(distance));
            assert!((back - distance).abs() < 0.02, "{distance} -> {back}");
        }
        assert_eq!(DistanceField::encode(9.0), 255);
        assert_eq!(DistanceField::encode(-9.0), 0);
    }

    #[test]
    fn the_line_transform_is_exact() {
        let input = [0.0, 1e9, 1e9, 1e9, 0.0, 1e9];
        let mut output = [0.0; 6];
        transform_1d(&input, &mut output);
        assert_eq!(output, [0.0, 1.0, 4.0, 1.0, 0.0, 1.0]);
    }
}
