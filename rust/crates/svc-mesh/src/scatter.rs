//! Where things may grow on a chunk's surface (#9546).
//!
//! The scatter service places grass and clutter on points of a meshed chunk.
//! The points come from a jittered grid over the ground in absolute
//! coordinates: each grid cell holds one candidate spot, chosen by hashing the
//! cell, and a spot is kept on every upward triangle whose footprint holds it.
//! So the same ground gives the same points whichever chunk meshed it, after
//! a remesh that did not change it, and across a world-origin rebase; ground
//! is covered at the same density whatever the triangles' sizes, and a ledge
//! over the ground has points of its own.

/// A spot on a chunk's surface.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfacePoint {
    /// Chunk-local, as the mesh's positions.
    pub position: [f32; 3],
    /// The triangle's unit normal.
    pub normal: [f32; 3],
    /// The material slot of the triangle's group.
    pub slot: u16,
    /// A hash of the spot's absolute grid cell and height: what a placement
    /// draws its random choices from.
    pub key: u64,
}

/// How densely and on which slopes to sample.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceSampling {
    /// Spots per square metre of ground (horizontal area).
    pub density: f64,
    /// Keeps one placement's grid apart from another's.
    pub seed: u64,
    /// The least normal y a triangle may have: the cosine of the steepest
    /// slope sampled.
    pub minimum_normal_y: f64,
}

/// The most spots a chunk gives one sampling; denser requests are refused
/// by the caller before they get here.
pub const MAX_SURFACE_POINTS: usize = 1 << 20;

/// The chunk's mesh, as `VoxelMeshChunk` carries it.
#[derive(Debug, Clone, Copy)]
pub struct ChunkSurface<'a> {
    /// Chunk-local positions, 3 per vertex.
    pub positions: &'a [f32],
    pub indices: &'a [u32],
    /// (material slot, first index, index count) per group.
    pub groups: &'a [(u16, u32, u32)],
    /// The absolute position of the chunk's local origin, in metres:
    /// origin voxel times voxel size.
    pub origin: [f64; 3],
    /// Height bands for the key: the voxel size.
    pub band: f64,
}

/// The spots `sampling` finds on `surface`, group by group.
pub fn surface_points(surface: &ChunkSurface<'_>, sampling: &SurfaceSampling) -> Vec<SurfacePoint> {
    let mut points = Vec::new();
    if !(sampling.density > 0.0 && sampling.density.is_finite()) {
        return points;
    }
    let spacing = 1.0 / sampling.density.sqrt();
    let vertex = |index: u32| -> [f64; 3] {
        let at = index as usize * 3;
        std::array::from_fn(|axis| surface.origin[axis] + f64::from(surface.positions[at + axis]))
    };
    for &(slot, start, count) in surface.groups {
        let end = (start + count) as usize;
        for triangle in surface.indices[start as usize..end].chunks_exact(3) {
            let [a, b, c] = [
                vertex(triangle[0]),
                vertex(triangle[1]),
                vertex(triangle[2]),
            ];
            let normal = cross(sub(b, a), sub(c, a));
            let length = dot(normal, normal).sqrt();
            if length <= f64::EPSILON {
                continue;
            }
            let normal = normal.map(|value| value / length);
            if normal[1] < sampling.minimum_normal_y || normal[1] <= 0.0 {
                continue;
            }
            // The triangle's footprint over the ground, in grid cells.
            let low = |axis: usize| (a[axis].min(b[axis]).min(c[axis]) / spacing).floor() as i64;
            let high = |axis: usize| (a[axis].max(b[axis]).max(c[axis]) / spacing).floor() as i64;
            for cell_x in low(0)..=high(0) {
                for cell_z in low(2)..=high(2) {
                    let cell = hash(sampling.seed, cell_x, cell_z, 0);
                    let x = (cell_x as f64 + unit(cell)) * spacing;
                    let z = (cell_z as f64 + unit(cell >> 32)) * spacing;
                    let Some(y) = height_within(a, b, c, x, z) else {
                        continue;
                    };
                    let band = (y / surface.band).floor() as i64;
                    points.push(SurfacePoint {
                        position: [
                            (x - surface.origin[0]) as f32,
                            (y - surface.origin[1]) as f32,
                            (z - surface.origin[2]) as f32,
                        ],
                        normal: normal.map(|value| value as f32),
                        slot,
                        key: hash(sampling.seed ^ 0x5ca7_7e7a, cell_x, cell_z, band),
                    });
                    if points.len() >= MAX_SURFACE_POINTS {
                        return points;
                    }
                }
            }
        }
    }
    points
}

/// The height of the triangle above (x, z) if its footprint holds it. An
/// edge belongs to one side: the half-open rule keeps a spot on a shared
/// edge from landing on both triangles.
fn height_within(a: [f64; 3], b: [f64; 3], c: [f64; 3], x: f64, z: f64) -> Option<f64> {
    let area = (b[0] - a[0]) * (c[2] - a[2]) - (c[0] - a[0]) * (b[2] - a[2]);
    if area.abs() <= f64::EPSILON {
        return None;
    }
    let edge =
        |p: [f64; 3], q: [f64; 3]| ((q[0] - p[0]) * (z - p[2]) - (x - p[0]) * (q[2] - p[2])) / area;
    let (wa, wb, wc) = (edge(b, c), edge(c, a), edge(a, b));
    let inside = |w: f64, p: [f64; 3], q: [f64; 3]| {
        w > 0.0 || (w == 0.0 && (q[2] > p[2] || (q[2] == p[2] && q[0] < p[0])))
    };
    (inside(wa, b, c) && inside(wb, c, a) && inside(wc, a, b))
        .then(|| wa * a[1] + wb * b[1] + wc * c[1])
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// SplitMix64 over the seed and a cell.
pub fn hash(seed: u64, x: i64, z: i64, band: i64) -> u64 {
    let mut value = seed
        ^ (x as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)
        ^ (z as u64).wrapping_mul(0xc2b2_ae3d_27d4_eb4f)
        ^ (band as u64).wrapping_mul(0x1656_67b1_9e37_79f9);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

/// The low 32 bits of `bits` as a number in [0, 1).
pub fn unit(bits: u64) -> f64 {
    f64::from(bits as u32) / 4_294_967_296.0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A flat square of ground `size` metres across at height `y`, as two
    /// triangles wound upward, in a chunk whose origin is `origin`.
    fn square(size: f32, y: f32) -> (Vec<f32>, Vec<u32>) {
        (
            vec![0.0, y, 0.0, 0.0, y, size, size, y, size, size, y, 0.0],
            vec![0, 1, 2, 0, 2, 3],
        )
    }

    fn sampling(density: f64) -> SurfaceSampling {
        SurfaceSampling {
            density,
            seed: 7,
            minimum_normal_y: 0.5,
        }
    }

    #[test]
    fn flat_ground_gets_its_density_and_each_spot_once() {
        let (positions, indices) = square(16.0, 2.0);
        let groups = [(3, 0, 6)];
        let surface = ChunkSurface {
            positions: &positions,
            indices: &indices,
            groups: &groups,
            origin: [32.0, 0.0, -16.0],
            band: 1.0,
        };
        let points = surface_points(&surface, &sampling(4.0));
        // 256 m² at 4 per m²: one spot per 0.5 m cell, none twice.
        assert_eq!(points.len(), 1024);
        let mut keys: Vec<u64> = points.iter().map(|point| point.key).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), 1024);
        assert!(points.iter().all(|point| point.slot == 3
            && point.position[1] == 2.0
            && point.normal == [0.0, 1.0, 0.0]
            && (0.0..16.0).contains(&point.position[0])
            && (0.0..16.0).contains(&point.position[2])));
    }

    #[test]
    fn spots_follow_absolute_ground_not_the_chunk_or_its_triangles() {
        // The same 8 m of ground meshed as one chunk, or as two chunks of
        // 4 m each, gives the same spots.
        let whole = {
            let (positions, indices) = square(8.0, 0.0);
            surface_points(
                &ChunkSurface {
                    positions: &positions,
                    indices: &indices,
                    groups: &[(0, 0, 6)],
                    origin: [100.0, 5.0, 100.0],
                    band: 1.0,
                },
                &sampling(2.0),
            )
        };
        let mut halves = Vec::new();
        for (offset_x, offset_z) in [(0.0, 0.0), (4.0, 0.0), (0.0, 4.0), (4.0, 4.0)] {
            let (positions, indices) = square(4.0, 0.0);
            let origin = [100.0 + offset_x, 5.0, 100.0 + offset_z];
            for point in surface_points(
                &ChunkSurface {
                    positions: &positions,
                    indices: &indices,
                    groups: &[(0, 0, 6)],
                    origin,
                    band: 1.0,
                },
                &sampling(2.0),
            ) {
                halves.push((
                    point.key,
                    [
                        f64::from(point.position[0]) + origin[0] - 100.0,
                        f64::from(point.position[2]) + origin[2] - 100.0,
                    ],
                ));
            }
        }
        let mut whole: Vec<(u64, [f64; 2])> = whole
            .iter()
            .map(|point| {
                (
                    point.key,
                    [f64::from(point.position[0]), f64::from(point.position[2])],
                )
            })
            .collect();
        whole.sort_by_key(|(key, _)| *key);
        halves.sort_by_key(|(key, _)| *key);
        assert_eq!(whole.len(), halves.len());
        for ((key, at), (other, there)) in whole.iter().zip(&halves) {
            assert_eq!(key, other);
            assert!((at[0] - there[0]).abs() < 1e-4 && (at[1] - there[1]).abs() < 1e-4);
        }
    }

    #[test]
    fn steep_and_downward_faces_grow_nothing() {
        // A wall (normal along x) and a ceiling (normal down).
        let positions = vec![
            0.0, 0.0, 0.0, 0.0, 4.0, 0.0, 0.0, 4.0, 4.0, //
            0.0, 3.0, 0.0, 4.0, 3.0, 0.0, 4.0, 3.0, 4.0,
        ];
        let indices = vec![0, 1, 2, 3, 4, 5];
        let points = surface_points(
            &ChunkSurface {
                positions: &positions,
                indices: &indices,
                groups: &[(0, 0, 6)],
                origin: [0.0; 3],
                band: 1.0,
            },
            &sampling(16.0),
        );
        assert!(points.is_empty(), "{points:?}");
    }
}
