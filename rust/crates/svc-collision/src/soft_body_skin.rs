//! A mesh drawn over a soft surface in place of its own triangles: a thick
//! or detailed garment riding a coarse simulated sheet. Each skin vertex
//! keeps the point of the nearest sheet triangle it was over at rest and its
//! offset from there in a frame that turns with the sheet: the sheet's smooth
//! normal and the triangle's first edge.

use std::collections::HashMap;

use rapier3d_f64::prelude::Vector;

use crate::DynamicsSoftBodySurface;

/// A particle: its piece and its index there, as the soft body numbers it.
pub(crate) type Particle = (usize, u32);

/// The mesh drawn over a surface soft body, in the world at rest.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DynamicsSoftBodySkin {
    pub positions: Vec<[f64; 3]>,
    /// One per position, or none.
    pub uvs: Vec<[f32; 2]>,
    pub triangles: Vec<[u32; 3]>,
}

impl DynamicsSoftBodySkin {
    pub(crate) fn is_valid(&self) -> bool {
        !self.positions.is_empty()
            && self
                .positions
                .iter()
                .all(|position| position.iter().copied().all(f64::is_finite))
            && (self.uvs.is_empty() || self.uvs.len() == self.positions.len())
            && self
                .triangles
                .iter()
                .flatten()
                .all(|vertex| (*vertex as usize) < self.positions.len())
    }
}

/// Where one skin vertex rides.
#[derive(Debug, Clone, Copy)]
struct Ride {
    particles: [Particle; 3],
    weights: [f64; 3],
    /// Along the triangle's first edge, across it, and along the normal.
    offset: [f64; 3],
}

#[derive(Debug, Clone)]
pub(crate) struct SkinBinding {
    uvs: Vec<[f32; 2]>,
    indices: Vec<u32>,
    rides: Vec<Ride>,
    /// Per skin vertex, the first vertex at its rest position: vertices split
    /// only for their UVs share one normal.
    welded: Vec<u32>,
}

/// The frame at a point of a triangle: its first edge, across, the normal.
fn frame(corners: [Vector; 3], normals: [Vector; 3], weights: [f64; 3]) -> [Vector; 3] {
    let face = (corners[1] - corners[0]).cross(corners[2] - corners[0]);
    let normal = (normals[0] * weights[0] + normals[1] * weights[1] + normals[2] * weights[2])
        .try_normalize()
        .or_else(|| face.try_normalize())
        .unwrap_or(Vector::Y);
    let edge = corners[1] - corners[0];
    let along = (edge - normal * edge.dot(normal))
        .try_normalize()
        .unwrap_or_else(|| normal.any_orthonormal_vector());
    [along, normal.cross(along), normal]
}

/// Area-weighted particle normals of a sheet's triangles.
pub(crate) fn particle_normals(positions: &[Vector], triangles: &[[u32; 3]]) -> Vec<Vector> {
    let mut normals = vec![Vector::ZERO; positions.len()];
    for triangle in triangles {
        let [a, b, c] = triangle.map(|particle| positions[particle as usize]);
        let weighted = (b - a).cross(c - a);
        for particle in triangle {
            normals[*particle as usize] += weighted;
        }
    }
    normals
        .into_iter()
        .map(|normal| normal.try_normalize().unwrap_or(Vector::Y))
        .collect()
}

/// Barycentric weights of the point of triangle `abc` nearest `p`.
fn nearest_on_triangle(p: Vector, a: Vector, b: Vector, c: Vector) -> [f64; 3] {
    // Ericson, Real-Time Collision Detection, 5.1.5.
    let (ab, ac, ap) = (b - a, c - a, p - a);
    let (d1, d2) = (ab.dot(ap), ac.dot(ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return [1.0, 0.0, 0.0];
    }
    let bp = p - b;
    let (d3, d4) = (ab.dot(bp), ac.dot(bp));
    if d3 >= 0.0 && d4 <= d3 {
        return [0.0, 1.0, 0.0];
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        return [1.0 - v, v, 0.0];
    }
    let cp = p - c;
    let (d5, d6) = (ab.dot(cp), ac.dot(cp));
    if d6 >= 0.0 && d5 <= d6 {
        return [0.0, 0.0, 1.0];
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return [1.0 - w, 0.0, w];
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return [0.0, 1.0 - w, w];
    }
    let denominator = 1.0 / (va + vb + vc);
    let (v, w) = (vb * denominator, vc * denominator);
    [1.0 - v - w, v, w]
}

fn at(weights: [f64; 3], corners: [Vector; 3]) -> Vector {
    corners[0] * weights[0] + corners[1] * weights[1] + corners[2] * weights[2]
}

impl SkinBinding {
    /// Ride `skin` on the sheet of `positions` and `triangles` at rest.
    pub(crate) fn bind(
        skin: &DynamicsSoftBodySkin,
        positions: &[Vector],
        triangles: &[[u32; 3]],
    ) -> Option<Self> {
        if triangles.is_empty() {
            return None;
        }
        let normals = particle_normals(positions, triangles);
        let rides = skin
            .positions
            .iter()
            .map(|vertex| {
                let vertex = Vector::from_array(*vertex);
                let (triangle, weights, _) = triangles
                    .iter()
                    .map(|triangle| {
                        let corners = triangle.map(|particle| positions[particle as usize]);
                        let weights =
                            nearest_on_triangle(vertex, corners[0], corners[1], corners[2]);
                        let distance = (at(weights, corners) - vertex).length_squared();
                        (triangle, weights, distance)
                    })
                    .min_by(|a, b| a.2.total_cmp(&b.2))
                    .expect("a triangle");
                let corners = triangle.map(|particle| positions[particle as usize]);
                let frame = frame(
                    corners,
                    triangle.map(|particle| normals[particle as usize]),
                    weights,
                );
                let away = vertex - at(weights, corners);
                Ride {
                    particles: triangle.map(|particle| (0, particle)),
                    weights,
                    offset: frame.map(|axis| away.dot(axis)),
                }
            })
            .collect();
        let mut first = HashMap::new();
        let welded = skin
            .positions
            .iter()
            .enumerate()
            .map(|(vertex, position)| {
                *first
                    .entry(position.map(f64::to_bits))
                    .or_insert(vertex as u32)
            })
            .collect();
        Some(Self {
            uvs: if skin.uvs.is_empty() {
                vec![[0.0; 2]; skin.positions.len()]
            } else {
                skin.uvs.clone()
            },
            indices: skin.triangles.iter().flatten().copied().collect(),
            rides,
            welded,
        })
    }

    /// Follow a tear's renumbering.
    pub(crate) fn remap(&mut self, remap: impl Fn(Particle) -> Particle) {
        for ride in &mut self.rides {
            ride.particles = ride.particles.map(&remap);
        }
    }

    /// The skin where the sheet is now, from the `position` and `normal`
    /// of each particle, without bounds. Normals are area-weighted over the
    /// skin's own triangles.
    pub(crate) fn draw(
        &self,
        position: impl Fn(Particle) -> Option<Vector>,
        normal: impl Fn(Particle) -> Option<Vector>,
    ) -> DynamicsSoftBodySurface {
        let positions: Vec<[f32; 3]> = self
            .rides
            .iter()
            .map(|ride| {
                let corners = ride.particles.map(&position);
                let normals = ride.particles.map(&normal);
                let ([Some(a), Some(b), Some(c)], [Some(na), Some(nb), Some(nc)]) =
                    (corners, normals)
                else {
                    return [0.0; 3];
                };
                let corners = [a, b, c];
                let frame = frame(corners, [na, nb, nc], ride.weights);
                let point = at(ride.weights, corners)
                    + frame[0] * ride.offset[0]
                    + frame[1] * ride.offset[1]
                    + frame[2] * ride.offset[2];
                point.as_vec3().to_array()
            })
            .collect();
        let mut summed = vec![rapier3d_f64::glamx::Vec3::ZERO; positions.len()];
        for triangle in self.indices.as_chunks::<3>().0 {
            let [a, b, c] = triangle
                .map(|vertex| rapier3d_f64::glamx::Vec3::from_array(positions[vertex as usize]));
            let weighted = (b - a).cross(c - a);
            for vertex in triangle {
                summed[self.welded[*vertex as usize] as usize] += weighted;
            }
        }
        let normals = self
            .welded
            .iter()
            .map(|first| {
                summed[*first as usize]
                    .try_normalize()
                    .unwrap_or(rapier3d_f64::glamx::Vec3::Y)
                    .to_array()
            })
            .collect();
        DynamicsSoftBodySurface {
            positions,
            normals,
            uvs: self.uvs.clone(),
            indices: self.indices.clone(),
            ..DynamicsSoftBodySurface::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A unit square sheet in the XY plane, two triangles.
    fn sheet() -> (Vec<Vector>, Vec<[u32; 3]>) {
        (
            vec![
                Vector::new(0.0, 0.0, 0.0),
                Vector::new(1.0, 0.0, 0.0),
                Vector::new(0.0, 1.0, 0.0),
                Vector::new(1.0, 1.0, 0.0),
            ],
            vec![[0, 1, 2], [1, 3, 2]],
        )
    }

    #[test]
    fn a_skin_rides_its_sheet_through_a_turn() {
        let (rest, triangles) = sheet();
        // A slab 2 cm thick over the sheet, and a vertex past its edge.
        let skin = DynamicsSoftBodySkin {
            positions: vec![[0.25, 0.25, 0.01], [0.75, 0.6, -0.01], [1.1, 0.5, 0.0]],
            uvs: Vec::new(),
            triangles: vec![[0, 1, 2]],
        };
        let binding = SkinBinding::bind(&skin, &rest, &triangles).unwrap();
        let draw = |positions: &[Vector]| {
            let normals = particle_normals(positions, &triangles);
            binding
                .draw(
                    |(_, particle)| positions.get(particle as usize).copied(),
                    |(_, particle)| normals.get(particle as usize).copied(),
                )
                .positions
        };
        // At rest it is where it was given.
        for (drawn, given) in draw(&rest).iter().zip(&skin.positions) {
            for axis in 0..3 {
                assert!(
                    (f64::from(drawn[axis]) - given[axis]).abs() < 1e-6,
                    "{drawn:?} {given:?}"
                );
            }
        }
        // The sheet turned a quarter about Y and moved: the skin turns with it.
        let turn = rapier3d_f64::prelude::Rotation::from_rotation_y(std::f64::consts::FRAC_PI_2);
        let moved: Vec<Vector> = rest
            .iter()
            .map(|p| turn * *p + Vector::new(2.0, 0.0, 0.0))
            .collect();
        for (drawn, given) in draw(&moved).iter().zip(&skin.positions) {
            let want = turn * Vector::from_array(*given) + Vector::new(2.0, 0.0, 0.0);
            let got = Vector::new(
                f64::from(drawn[0]),
                f64::from(drawn[1]),
                f64::from(drawn[2]),
            );
            assert!((got - want).length() < 1e-6, "{got:?} {want:?}");
        }
    }

    #[test]
    fn vertices_split_for_uvs_share_a_normal() {
        let (rest, triangles) = sheet();
        // Two triangles folded at x = 0.5, the fold's vertices doubled.
        let skin = DynamicsSoftBodySkin {
            positions: vec![
                [0.0, 0.0, 0.0],
                [0.5, 0.0, 0.1],
                [0.0, 1.0, 0.0],
                [0.5, 0.0, 0.1],
                [1.0, 0.0, 0.0],
                [0.5, 1.0, 0.1],
                [0.5, 1.0, 0.1],
            ],
            uvs: Vec::new(),
            triangles: vec![[0, 1, 2], [3, 4, 5], [1, 6, 2]],
        };
        let binding = SkinBinding::bind(&skin, &rest, &triangles).unwrap();
        let normals = particle_normals(&rest, &triangles);
        let drawn = binding.draw(
            |(_, particle)| rest.get(particle as usize).copied(),
            |(_, particle)| normals.get(particle as usize).copied(),
        );
        assert_eq!(drawn.normals[1], drawn.normals[3]);
        assert_eq!(drawn.normals[5], drawn.normals[6]);
    }
}
