//! A model's own triangles at one placement, for exact surface queries: the
//! nearest surface point to each of a set of points, and the first surface a
//! ray meets. Nothing here is collision residency; the caller builds one for
//! its query and drops it.

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

/// A model's triangles placed in the caller's space, ready for queries.
pub struct ModelSurface {
    mesh: TriMesh,
}

impl ModelSurface {
    /// `positions` and `triangles` in the model's own space, placed by
    /// `placement`. A mirroring placement keeps the faces' outward sides.
    pub fn new(
        positions: &[[f64; 3]],
        mut triangles: Vec<[u32; 3]>,
        placement: StaticMeshTransform,
    ) -> Result<Self, StaticMeshCollisionError> {
        if positions.is_empty() || triangles.is_empty() {
            return Err(StaticMeshCollisionError::EmptyGeometry);
        }
        if positions.iter().flatten().any(|value| !value.is_finite()) {
            return Err(StaticMeshCollisionError::NonFiniteVertex);
        }
        if let Some(index) = triangles
            .iter()
            .flatten()
            .copied()
            .find(|index| *index as usize >= positions.len())
        {
            return Err(StaticMeshCollisionError::InvalidTriangleIndex {
                index,
                vertex_count: positions.len(),
            });
        }
        validate_transform(placement)?;
        if placement.scale.iter().filter(|scale| **scale < 0.0).count() % 2 == 1 {
            for triangle in &mut triangles {
                triangle.swap(1, 2);
            }
        }
        let vertices = positions
            .iter()
            .map(|position| transform_position(*position, placement))
            .map(|position| Vector::new(position[0], position[1], position[2]))
            .collect();
        // Shared corners give the edge and vertex pseudo-normals that tell
        // inside from outside across seams.
        let mesh = TriMesh::with_flags(
            vertices,
            triangles,
            TriMeshFlags::ORIENTED | TriMeshFlags::MERGE_DUPLICATE_VERTICES,
        )
        .map_err(|_| StaticMeshCollisionError::InvalidTriangleMesh)?;
        Ok(Self { mesh })
    }

    /// The nearest surface point to `point`, if one lies within
    /// `max_distance`.
    pub fn nearest(&self, point: [f64; 3], max_distance: f64) -> Option<ModelSurfacePoint> {
        if !max_distance.is_finite() || max_distance < 0.0 || !point.iter().all(|v| v.is_finite()) {
            return None;
        }
        let query = Vector::new(point[0], point[1], point[2]);
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

    /// Where the ray from `origin` along `direction` first meets the
    /// surface, within `max_distance`.
    pub fn raycast(
        &self,
        origin: [f64; 3],
        direction: [f64; 3],
        max_distance: f64,
    ) -> Option<ModelSurfaceHit> {
        let direction = Vector::new(direction[0], direction[1], direction[2]).try_normalize()?;
        let origin = Vector::new(origin[0], origin[1], origin[2]);
        if !origin.is_finite() || !max_distance.is_finite() || max_distance < 0.0 {
            return None;
        }
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
        let surface = ModelSurface::new(&positions, triangles, placement).unwrap();

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
        let surface =
            ModelSurface::new(&positions, triangles, StaticMeshTransform::IDENTITY).unwrap();
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
        let surface = ModelSurface::new(&positions, triangles, mirrored).unwrap();
        // Spans -1..0 on x now.
        let outside = surface.nearest([-2.0, 0.5, 0.5], 5.0).unwrap();
        assert!(
            close(outside.normal, [-1.0, 0.0, 0.0]) && !outside.inside,
            "{outside:?}"
        );
        assert!(surface.nearest([-0.5, 0.5, 0.5], 5.0).unwrap().inside);
    }
}
