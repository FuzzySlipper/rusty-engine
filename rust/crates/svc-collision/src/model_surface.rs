//! A model's own triangles, for exact surface queries at a placement: the
//! nearest surface point to each of a set of points, and the first surface a
//! ray meets. The surface is built once in the model's own space; a placement
//! with a uniform scale is queried through it, and any other builds a placed
//! copy. Nothing here is collision residency.

use parry3d_f64::math::Vector;
use parry3d_f64::query::{PointQueryWithLocation, Ray as ParryRay, RayCast};
use parry3d_f64::shape::{FeatureId, TriMesh, TriMeshFlags};

use crate::static_mesh::{transform_position, validate_transform};
use crate::{StaticMeshCollisionError, StaticMeshTransform};

/// The nearest point of a model's surface to a query point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModelSurfacePoint {
    pub point: [f64; 3],
    /// The outward normal (by winding) of the triangle the point lies on.
    pub normal: [f64; 3],
    pub distance: f64,
    /// The query point lies behind the surface there. On a closed mesh that
    /// is inside it; on an open one, behind the nearest face.
    pub inside: bool,
}

/// Where a ray first meets a model's surface.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModelSurfaceHit {
    pub point: [f64; 3],
    /// The outward normal (by winding) of the triangle hit.
    pub normal: [f64; 3],
    pub distance: f64,
    /// The ray met the triangle from behind: from inside a closed mesh.
    pub back_face: bool,
}

/// A model's triangles in its own space.
pub struct ModelSurface {
    mesh: TriMesh,
}

/// A [`ModelSurface`] placed in the caller's space, answering there.
pub struct PlacedModelSurface<'a> {
    placed: Placed<'a>,
}

enum Placed<'a> {
    /// Queries map into the model's space and answers back out.
    Through(&'a ModelSurface, Similarity),
    /// The model's triangles placed by a non-uniform scale.
    Built(Box<ModelSurface>),
}

/// A placement with one scale on every axis: rotation, scale and translation
/// keep shapes, so a query can be answered in the model's space.
#[derive(Clone, Copy)]
struct Similarity {
    translation: Vector,
    /// Normalized, `[x, y, z, w]`.
    rotation: [f64; 4],
    scale: f64,
}

impl ModelSurface {
    /// `positions` and `triangles` in the model's own space, as an imported
    /// model gives them: finite positions, and every triangle's indices
    /// within `positions` (an index past them panics).
    pub fn new(
        positions: &[[f64; 3]],
        triangles: Vec<[u32; 3]>,
    ) -> Result<Self, StaticMeshCollisionError> {
        let vertices = positions
            .iter()
            .map(|position| Vector::from_array(*position))
            .collect();
        // Shared corners give the edge and vertex pseudo-normals that tell
        // inside from outside across seams.
        Self::build(
            vertices,
            triangles,
            TriMeshFlags::ORIENTED | TriMeshFlags::MERGE_DUPLICATE_VERTICES,
        )
    }

    fn build(
        vertices: Vec<Vector>,
        triangles: Vec<[u32; 3]>,
        flags: TriMeshFlags,
    ) -> Result<Self, StaticMeshCollisionError> {
        let mesh = TriMesh::with_flags(vertices, triangles, flags)
            .map_err(|_| StaticMeshCollisionError::InvalidTriangleMesh)?;
        Ok(Self { mesh })
    }

    /// This surface placed by `placement`. A mirroring placement keeps the
    /// faces' outward sides.
    pub fn placed(
        &self,
        placement: StaticMeshTransform,
    ) -> Result<PlacedModelSurface<'_>, StaticMeshCollisionError> {
        validate_transform(placement)?;
        let [x, y, z] = placement.scale;
        if x == y && y == z {
            let [qx, qy, qz, qw] = placement.rotation;
            let length = (qx * qx + qy * qy + qz * qz + qw * qw).sqrt();
            let similarity = Similarity {
                translation: Vector::from_array(placement.translation),
                rotation: placement.rotation.map(|value| value / length),
                scale: x,
            };
            return Ok(PlacedModelSurface {
                placed: Placed::Through(self, similarity),
            });
        }
        let mut triangles = self.mesh.indices().to_vec();
        if placement.scale.iter().filter(|scale| **scale < 0.0).count() % 2 == 1 {
            for triangle in &mut triangles {
                triangle.swap(1, 2);
            }
        }
        let vertices = self
            .mesh
            .vertices()
            .iter()
            .map(|vertex| Vector::from_array(transform_position(vertex.to_array(), placement)))
            .collect();
        // The corners are already shared.
        let built = Self::build(vertices, triangles, TriMeshFlags::ORIENTED)?;
        Ok(PlacedModelSurface {
            placed: Placed::Built(Box::new(built)),
        })
    }

    fn nearest(&self, query: Vector, max_distance: f64) -> Option<ModelSurfacePoint> {
        let (projection, (triangle, _)) = self
            .mesh
            .project_local_point_and_get_location_with_max_dist(query, false, max_distance)?;
        let distance = (query - projection.point).length();
        (distance <= max_distance).then(|| ModelSurfacePoint {
            point: projection.point.to_array(),
            normal: self.normal(triangle),
            distance,
            inside: projection.is_inside,
        })
    }

    /// `direction` is normalized.
    fn raycast(
        &self,
        origin: Vector,
        direction: Vector,
        max_distance: f64,
    ) -> Option<ModelSurfaceHit> {
        let hit = self.mesh.cast_local_ray_and_get_normal(
            &ParryRay::new(origin, direction),
            max_distance,
            false,
        )?;
        let count = self.mesh.indices().len() as u32;
        let FeatureId::Face(face) = hit.feature else {
            return None;
        };
        Some(ModelSurfaceHit {
            point: (origin + direction * hit.time_of_impact).to_array(),
            normal: self.normal(face % count),
            distance: hit.time_of_impact,
            back_face: face >= count,
        })
    }

    fn normal(&self, triangle: u32) -> [f64; 3] {
        self.mesh
            .triangle(triangle)
            .normal()
            .map_or([0.0; 3], |normal| normal.to_array())
    }
}

impl PlacedModelSurface<'_> {
    /// The nearest surface point to `point`, if one lies within
    /// `max_distance`.
    pub fn nearest(&self, point: [f64; 3], max_distance: f64) -> Option<ModelSurfacePoint> {
        if !max_distance.is_finite() || max_distance < 0.0 || !point.iter().all(|v| v.is_finite()) {
            return None;
        }
        let point = Vector::from_array(point);
        match &self.placed {
            Placed::Built(surface) => surface.nearest(point, max_distance),
            Placed::Through(surface, placement) => {
                let found = surface.nearest(
                    placement.point_to_model(point),
                    max_distance / placement.scale.abs(),
                )?;
                Some(ModelSurfacePoint {
                    point: placement.point_from_model(found.point),
                    normal: placement.normal_from_model(found.normal),
                    distance: found.distance * placement.scale.abs(),
                    inside: found.inside,
                })
            }
        }
    }

    /// Where the ray from `origin` along `direction` first meets the
    /// surface, within `max_distance`.
    pub fn raycast(
        &self,
        origin: [f64; 3],
        direction: [f64; 3],
        max_distance: f64,
    ) -> Option<ModelSurfaceHit> {
        let direction = Vector::from_array(direction).try_normalize()?;
        let origin = Vector::from_array(origin);
        if !origin.is_finite() || !max_distance.is_finite() || max_distance < 0.0 {
            return None;
        }
        match &self.placed {
            Placed::Built(surface) => surface.raycast(origin, direction, max_distance),
            Placed::Through(surface, placement) => {
                let hit = surface.raycast(
                    placement.point_to_model(origin),
                    placement.direction_to_model(direction).try_normalize()?,
                    max_distance / placement.scale.abs(),
                )?;
                Some(ModelSurfaceHit {
                    point: placement.point_from_model(hit.point),
                    normal: placement.normal_from_model(hit.normal),
                    distance: hit.distance * placement.scale.abs(),
                    back_face: hit.back_face,
                })
            }
        }
    }
}

impl Similarity {
    fn rotate(rotation: [f64; 4], value: Vector) -> Vector {
        let [x, y, z, w] = rotation;
        let axis = Vector::new(x, y, z);
        value + 2.0 * axis.cross(axis.cross(value) + w * value)
    }

    fn inverse_rotation(&self) -> [f64; 4] {
        let [x, y, z, w] = self.rotation;
        [-x, -y, -z, w]
    }

    fn point_to_model(&self, point: Vector) -> Vector {
        Self::rotate(self.inverse_rotation(), point - self.translation) / self.scale
    }

    fn direction_to_model(&self, direction: Vector) -> Vector {
        Self::rotate(self.inverse_rotation(), direction) / self.scale
    }

    fn point_from_model(&self, point: [f64; 3]) -> [f64; 3] {
        (self.translation + Self::rotate(self.rotation, Vector::from_array(point) * self.scale))
            .to_array()
    }

    /// A negative scale mirrors the model, and its outward side with it.
    fn normal_from_model(&self, normal: [f64; 3]) -> [f64; 3] {
        (Self::rotate(self.rotation, Vector::from_array(normal)) * self.scale.signum()).to_array()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A unit cube from the origin, faces wound outward, with its corners
    /// split per face as a GLB's are.
    fn cube() -> (Vec<[f64; 3]>, Vec<[u32; 3]>) {
        let faces: [([f64; 3], [f64; 3], [f64; 3]); 6] = [
            ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
            ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
            ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]),
            ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
            ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
            ([0.0, 0.0, -1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]),
        ];
        let (mut positions, mut triangles) = (Vec::new(), Vec::new());
        for (normal, u, v) in faces {
            let base = positions.len() as u32;
            for (a, b) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                positions.push(std::array::from_fn(|axis| {
                    0.5 + 0.5 * (normal[axis] + u[axis] * a + v[axis] * b)
                }));
            }
            triangles.extend([[base, base + 1, base + 2], [base, base + 2, base + 3]]);
        }
        (positions, triangles)
    }

    fn close(a: [f64; 3], b: [f64; 3]) -> bool {
        a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-9)
    }

    #[test]
    fn points_near_on_and_inside_a_placed_closed_mesh() {
        let (positions, triangles) = cube();
        // Twice the size, moved 10 along x: it spans 10..12 on x, 0..2 on y
        // and z.
        let placement = StaticMeshTransform {
            translation: [10.0, 0.0, 0.0],
            scale: [2.0; 3],
            ..StaticMeshTransform::IDENTITY
        };
        let model = ModelSurface::new(&positions, triangles).unwrap();
        let surface = model.placed(placement).unwrap();

        let near = surface.nearest([13.0, 1.0, 1.0], 5.0).unwrap();
        assert!(close(near.point, [12.0, 1.0, 1.0]), "{near:?}");
        assert!(close(near.normal, [1.0, 0.0, 0.0]));
        assert!((near.distance - 1.0).abs() < 1e-9 && !near.inside);

        let on = surface.nearest([11.0, 2.0, 0.5], 5.0).unwrap();
        assert!(on.distance < 1e-9, "{on:?}");

        let inside = surface.nearest([11.0, 1.0, 0.25], 5.0).unwrap();
        assert!(close(inside.point, [11.0, 1.0, 0.0]), "{inside:?}");
        assert!(close(inside.normal, [0.0, 0.0, -1.0]));
        assert!((inside.distance - 0.25).abs() < 1e-9 && inside.inside);

        // Toward a corner, where the nearest feature is a vertex or edge
        // shared by split corners: still outside.
        let corner = surface.nearest([12.5, 2.5, 2.5], 5.0).unwrap();
        assert!(
            close(corner.point, [12.0, 2.0, 2.0]) && !corner.inside,
            "{corner:?}"
        );

        assert_eq!(surface.nearest([13.0, 1.0, 1.0], 0.5), None, "beyond reach");
    }

    #[test]
    fn rays_meet_the_first_face_and_tell_a_back_face() {
        let (positions, triangles) = cube();
        let model = ModelSurface::new(&positions, triangles).unwrap();
        let surface = model.placed(StaticMeshTransform::IDENTITY).unwrap();
        let hit = surface
            .raycast([-1.0, 0.5, 0.5], [2.0, 0.0, 0.0], 5.0)
            .unwrap();
        assert!(close(hit.point, [0.0, 0.5, 0.5]), "{hit:?}");
        assert!(close(hit.normal, [-1.0, 0.0, 0.0]) && !hit.back_face);
        assert!((hit.distance - 1.0).abs() < 1e-9);

        let from_inside = surface
            .raycast([0.5, 0.5, 0.5], [0.0, 1.0, 0.0], 5.0)
            .unwrap();
        assert!(close(from_inside.point, [0.5, 1.0, 0.5]), "{from_inside:?}");
        assert!(close(from_inside.normal, [0.0, 1.0, 0.0]) && from_inside.back_face);

        assert_eq!(
            surface.raycast([-1.0, 0.5, 0.5], [1.0, 0.0, 0.0], 0.5),
            None
        );
        assert_eq!(
            surface.raycast([-1.0, 0.5, 0.5], [0.0, 0.0, 0.0], 5.0),
            None
        );
    }

    #[test]
    fn a_mirroring_placement_keeps_the_outside_out() {
        let (positions, triangles) = cube();
        let mirrored = StaticMeshTransform {
            scale: [-1.0, 1.0, 1.0],
            ..StaticMeshTransform::IDENTITY
        };
        let model = ModelSurface::new(&positions, triangles).unwrap();
        let surface = model.placed(mirrored).unwrap();
        // Spans -1..0 on x now.
        let outside = surface.nearest([-2.0, 0.5, 0.5], 5.0).unwrap();
        assert!(
            close(outside.normal, [-1.0, 0.0, 0.0]) && !outside.inside,
            "{outside:?}"
        );
        assert!(surface.nearest([-0.5, 0.5, 0.5], 5.0).unwrap().inside);
    }

    /// A closed, irregular solid: the cube with one corner pulled out, so
    /// that no two faces share a plane or a size.
    fn lopsided() -> (Vec<[f64; 3]>, Vec<[u32; 3]>) {
        let (mut positions, triangles) = cube();
        for position in &mut positions {
            if *position == [1.0, 1.0, 1.0] {
                *position = [1.4, 1.7, 1.2];
            }
        }
        (positions, triangles)
    }

    /// A deterministic stream of values in [-1, 1).
    fn noise(seed: &mut u64) -> f64 {
        *seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (*seed >> 11) as f64 / (1u64 << 52) as f64 - 1.0
    }

    /// A query against a uniformly scaled placement goes through the
    /// model-space surface; its answers equal those of a surface built from
    /// the placed triangles, for translation, rotation, uniform scale and a
    /// uniform mirror.
    #[test]
    fn a_uniform_placement_answers_as_a_placed_build() {
        let (positions, triangles) = lopsided();
        let model = ModelSurface::new(&positions, triangles.clone()).unwrap();
        let rotation = {
            let [x, y, z, w]: [f64; 4] = [0.3, -0.5, 0.2, 0.8];
            // Left unnormalized, as a caller may give it.
            [x * 2.0, y * 2.0, z * 2.0, w * 2.0]
        };
        for scale in [1.0, 2.5, 0.25, -1.5] {
            let placement = StaticMeshTransform {
                translation: [10.0, -3.0, 4.5],
                rotation,
                scale: [scale; 3],
            };
            let mut placed_triangles = triangles.clone();
            if scale < 0.0 {
                placed_triangles.iter_mut().for_each(|t| t.swap(1, 2));
            }
            let placed_positions: Vec<[f64; 3]> = positions
                .iter()
                .map(|position| transform_position(*position, placement))
                .collect();
            let built = ModelSurface::new(&placed_positions, placed_triangles).unwrap();
            let built = built.placed(StaticMeshTransform::IDENTITY).unwrap();
            let through = model.placed(placement).unwrap();
            assert!(matches!(through.placed, Placed::Through(..)));

            let tolerance = 1e-9 * scale.abs().max(1.0);
            let near =
                |a: [f64; 3], b: [f64; 3]| a.iter().zip(b).all(|(a, b)| (a - b).abs() < tolerance);
            let world = |model: [f64; 3]| transform_position(model, placement);
            let mut seed = 7u64;
            let (mut inside, mut outside, mut hits, mut back_faces) = (0, 0, 0, 0);
            for step in 0..400 {
                // Within the model and well beyond it.
                let spread = if step % 2 == 0 { 0.45 } else { 1.5 };
                let point = world(std::array::from_fn(|_| 0.5 + spread * noise(&mut seed)));
                let reach = 2.0 * scale.abs() * (1.0 + noise(&mut seed));
                let (a, b) = (through.nearest(point, reach), built.nearest(point, reach));
                assert_eq!(a.is_some(), b.is_some(), "{point:?} {a:?} {b:?}");
                if let (Some(a), Some(b)) = (a, b) {
                    assert!(near(a.point, b.point), "{a:?} {b:?}");
                    assert!((a.distance - b.distance).abs() < tolerance, "{a:?} {b:?}");
                    assert_eq!(a.inside, b.inside, "{a:?} {b:?}");
                    if a.inside {
                        inside += 1;
                    } else {
                        outside += 1;
                    }
                }

                let target = world(std::array::from_fn(|_| 0.5 + 0.7 * noise(&mut seed)));
                let direction: [f64; 3] = std::array::from_fn(|axis| target[axis] - point[axis]);
                let reach = 3.0 * scale.abs();
                let (a, b) = (
                    through.raycast(point, direction, reach),
                    built.raycast(point, direction, reach),
                );
                assert_eq!(a.is_some(), b.is_some(), "{point:?} {a:?} {b:?}");
                if let (Some(a), Some(b)) = (a, b) {
                    assert!(near(a.point, b.point), "{a:?} {b:?}");
                    assert!(near(a.normal, b.normal), "{a:?} {b:?}");
                    assert!((a.distance - b.distance).abs() < tolerance, "{a:?} {b:?}");
                    assert_eq!(a.back_face, b.back_face, "{a:?} {b:?}");
                    hits += 1;
                    back_faces += usize::from(a.back_face);
                }
            }
            assert!(inside > 100 && outside > 100 && hits > 200 && back_faces > 100);

            // Points just off a face, away from its edges, find that face:
            // its normal turns and mirrors with the placement.
            for (corners, sign) in [([4u32, 5, 6], 1.0), ([12, 13, 14], -1.0)] {
                let [a, b, c] = corners.map(|index| positions[index as usize]);
                let on_face: [f64; 3] =
                    std::array::from_fn(|axis| 0.5 * a[axis] + 0.3 * b[axis] + 0.2 * c[axis]);
                let normal = model
                    .nearest(Vector::from_array(on_face), 1.0)
                    .unwrap()
                    .normal;
                let off = world(std::array::from_fn(|axis| {
                    on_face[axis] + sign * 0.05 * normal[axis]
                }));
                let (a, b) = (
                    through.nearest(off, 1.0).unwrap(),
                    built.nearest(off, 1.0).unwrap(),
                );
                assert!(near(a.normal, b.normal), "{a:?} {b:?}");
                assert_eq!(a.inside, sign < 0.0);
            }
        }
    }
}
