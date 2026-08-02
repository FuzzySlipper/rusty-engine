use std::collections::BTreeMap;

use core_space::{WorldPos, WorldVec};
use parry3d_f64::math::{Pose, Vector};
use parry3d_f64::query::{cast_shapes, intersection_test, Ray as ParryRay, ShapeCastOptions};
use parry3d_f64::shape::{Cuboid, SharedShape};

use crate::{identity, world_to_point, Ray};

pub const MAX_STATIC_MESH_ASSETS: usize = 256;
pub const MAX_STATIC_MESH_INSTANCES: usize = 4_096;
pub const MAX_STATIC_MESH_VERTICES_PER_ASSET: usize = 1_000_000;
pub const MAX_STATIC_MESH_TRIANGLES_PER_ASSET: usize = 2_000_000;
pub const MAX_STATIC_MESH_VERTICES: usize = 2_000_000;
pub const MAX_STATIC_MESH_TRIANGLES: usize = 4_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StaticMeshAssetId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StaticMeshInstanceId(pub u64);

#[derive(Debug, Clone, PartialEq)]
pub struct StaticMeshColliderAsset {
    pub id: StaticMeshAssetId,
    pub geometry_hash: u64,
    pub positions: Vec<[f64; 3]>,
    pub triangles: Vec<[u32; 3]>,
}

impl StaticMeshColliderAsset {
    pub fn new(
        id: StaticMeshAssetId,
        positions: Vec<[f64; 3]>,
        triangles: Vec<[u32; 3]>,
    ) -> Result<Self, StaticMeshCollisionError> {
        validate_geometry(&positions, &triangles)?;
        let geometry_hash = geometry_hash(&positions, &triangles);
        Ok(Self {
            id,
            geometry_hash,
            positions,
            triangles,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StaticMeshTransform {
    pub translation: [f64; 3],
    /// Quaternion in `[x, y, z, w]` order.
    pub rotation: [f64; 4],
    pub scale: [f64; 3],
}

impl StaticMeshTransform {
    pub const IDENTITY: Self = Self {
        translation: [0.0; 3],
        rotation: [0.0, 0.0, 0.0, 1.0],
        scale: [1.0; 3],
    };
}

impl Default for StaticMeshTransform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StaticMeshColliderInstance {
    pub id: StaticMeshInstanceId,
    pub asset: StaticMeshAssetId,
    pub expected_geometry_hash: u64,
    pub transform: StaticMeshTransform,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StaticMeshCollisionReceipt {
    pub revision_before: u64,
    pub revision_after: u64,
    pub asset_count: usize,
    pub instance_count: usize,
    pub vertex_count: usize,
    pub triangle_count: usize,
    pub projected_vertex_count: usize,
    pub projected_triangle_count: usize,
    pub projection_hash: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StaticMeshHit {
    pub instance: StaticMeshInstanceId,
    pub asset: StaticMeshAssetId,
    pub geometry_hash: u64,
    pub point: WorldPos,
    pub normal: WorldVec,
    pub distance: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StaticMeshCollisionError {
    RevisionMismatch {
        expected: u64,
        actual: u64,
    },
    RevisionExhausted,
    TooManyAssets {
        limit: usize,
    },
    TooManyInstances {
        limit: usize,
    },
    TooManyVertices {
        limit: usize,
    },
    TooManyTriangles {
        limit: usize,
    },
    DuplicateAsset {
        id: StaticMeshAssetId,
    },
    DuplicateInstance {
        id: StaticMeshInstanceId,
    },
    MissingAsset {
        id: StaticMeshAssetId,
    },
    StaleAsset {
        id: StaticMeshAssetId,
        expected: u64,
        actual: u64,
    },
    EmptyGeometry,
    NonFiniteVertex,
    InvalidTriangleIndex {
        index: u32,
        vertex_count: usize,
    },
    InvalidTransform,
    InvalidTriangleMesh,
}

#[derive(Clone)]
struct PreparedInstance {
    asset: StaticMeshAssetId,
    geometry_hash: u64,
    transform: StaticMeshTransform,
    shape: SharedShape,
}

/// Query-optimized projection of immutable static-mesh assets and caller-owned
/// instances. The complete asset/instance set is revision-replaced atomically;
/// callers retain authored identity, transforms, persistence, and mutation policy.
#[derive(Clone, Default)]
pub struct StaticMeshCollisionProjection {
    assets: BTreeMap<StaticMeshAssetId, StaticMeshColliderAsset>,
    instances: BTreeMap<StaticMeshInstanceId, PreparedInstance>,
    revision: u64,
}

impl StaticMeshCollisionProjection {
    pub(crate) fn dynamics_shapes(&self) -> impl Iterator<Item = SharedShape> + '_ {
        self.instances
            .values()
            .map(|instance| instance.shape.clone())
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn asset_count(&self) -> usize {
        self.assets.len()
    }

    pub fn instance_count(&self) -> usize {
        self.instances.len()
    }

    pub(crate) fn identity_hash(&self) -> u64 {
        let mut hash = 0xcbf29ce484222325u64;
        write_hash(&mut hash, &(self.assets.len() as u64).to_le_bytes());
        write_hash(&mut hash, &(self.instances.len() as u64).to_le_bytes());
        for (id, asset) in &self.assets {
            write_hash(&mut hash, &id.0.to_le_bytes());
            write_hash(&mut hash, &asset.geometry_hash.to_le_bytes());
        }
        for (id, instance) in &self.instances {
            write_hash(&mut hash, &id.0.to_le_bytes());
            write_hash(&mut hash, &instance.asset.0.to_le_bytes());
            write_hash(&mut hash, &instance.geometry_hash.to_le_bytes());
            for value in instance
                .transform
                .translation
                .iter()
                .chain(&instance.transform.rotation)
                .chain(&instance.transform.scale)
            {
                write_hash(&mut hash, &value.to_bits().to_le_bytes());
            }
        }
        hash
    }

    pub fn replace_all(
        &mut self,
        expected_revision: u64,
        assets: impl IntoIterator<Item = StaticMeshColliderAsset>,
        instances: impl IntoIterator<Item = StaticMeshColliderInstance>,
    ) -> Result<StaticMeshCollisionReceipt, StaticMeshCollisionError> {
        if expected_revision != self.revision {
            return Err(StaticMeshCollisionError::RevisionMismatch {
                expected: expected_revision,
                actual: self.revision,
            });
        }
        let revision_after = self
            .revision
            .checked_add(1)
            .ok_or(StaticMeshCollisionError::RevisionExhausted)?;
        let mut next_assets = BTreeMap::new();
        let mut vertex_count = 0usize;
        let mut triangle_count = 0usize;
        for asset in assets {
            validate_geometry(&asset.positions, &asset.triangles)?;
            if asset.geometry_hash != geometry_hash(&asset.positions, &asset.triangles) {
                return Err(StaticMeshCollisionError::InvalidTriangleMesh);
            }
            vertex_count = vertex_count.checked_add(asset.positions.len()).ok_or(
                StaticMeshCollisionError::TooManyVertices {
                    limit: MAX_STATIC_MESH_VERTICES,
                },
            )?;
            triangle_count = triangle_count.checked_add(asset.triangles.len()).ok_or(
                StaticMeshCollisionError::TooManyTriangles {
                    limit: MAX_STATIC_MESH_TRIANGLES,
                },
            )?;
            if vertex_count > MAX_STATIC_MESH_VERTICES {
                return Err(StaticMeshCollisionError::TooManyVertices {
                    limit: MAX_STATIC_MESH_VERTICES,
                });
            }
            if triangle_count > MAX_STATIC_MESH_TRIANGLES {
                return Err(StaticMeshCollisionError::TooManyTriangles {
                    limit: MAX_STATIC_MESH_TRIANGLES,
                });
            }
            let id = asset.id;
            if next_assets.insert(id, asset).is_some() {
                return Err(StaticMeshCollisionError::DuplicateAsset { id });
            }
            if next_assets.len() > MAX_STATIC_MESH_ASSETS {
                return Err(StaticMeshCollisionError::TooManyAssets {
                    limit: MAX_STATIC_MESH_ASSETS,
                });
            }
        }

        let mut next_instances = BTreeMap::new();
        let mut projected_vertex_count = 0usize;
        let mut projected_triangle_count = 0usize;
        for instance in instances {
            if next_instances.len() >= MAX_STATIC_MESH_INSTANCES {
                return Err(StaticMeshCollisionError::TooManyInstances {
                    limit: MAX_STATIC_MESH_INSTANCES,
                });
            }
            if next_instances.contains_key(&instance.id) {
                return Err(StaticMeshCollisionError::DuplicateInstance { id: instance.id });
            }
            let Some(asset) = next_assets.get(&instance.asset) else {
                return Err(StaticMeshCollisionError::MissingAsset { id: instance.asset });
            };
            if instance.expected_geometry_hash != asset.geometry_hash {
                return Err(StaticMeshCollisionError::StaleAsset {
                    id: asset.id,
                    expected: instance.expected_geometry_hash,
                    actual: asset.geometry_hash,
                });
            }
            validate_transform(instance.transform)?;
            projected_vertex_count = projected_vertex_count
                .checked_add(asset.positions.len())
                .ok_or(StaticMeshCollisionError::TooManyVertices {
                    limit: MAX_STATIC_MESH_VERTICES,
                })?;
            projected_triangle_count = projected_triangle_count
                .checked_add(asset.triangles.len())
                .ok_or(StaticMeshCollisionError::TooManyTriangles {
                    limit: MAX_STATIC_MESH_TRIANGLES,
                })?;
            if projected_vertex_count > MAX_STATIC_MESH_VERTICES {
                return Err(StaticMeshCollisionError::TooManyVertices {
                    limit: MAX_STATIC_MESH_VERTICES,
                });
            }
            if projected_triangle_count > MAX_STATIC_MESH_TRIANGLES {
                return Err(StaticMeshCollisionError::TooManyTriangles {
                    limit: MAX_STATIC_MESH_TRIANGLES,
                });
            }
            let vertices = asset
                .positions
                .iter()
                .copied()
                .map(|position| transform_position(position, instance.transform))
                .map(|position| Vector::new(position[0], position[1], position[2]))
                .collect();
            let shape = SharedShape::trimesh(vertices, asset.triangles.clone())
                .map_err(|_| StaticMeshCollisionError::InvalidTriangleMesh)?;
            let id = instance.id;
            next_instances.insert(
                id,
                PreparedInstance {
                    asset: asset.id,
                    geometry_hash: asset.geometry_hash,
                    transform: instance.transform,
                    shape,
                },
            );
        }

        let mut candidate = Self {
            assets: next_assets,
            instances: next_instances,
            revision: revision_after,
        };
        let receipt = StaticMeshCollisionReceipt {
            revision_before: self.revision,
            revision_after,
            asset_count: candidate.assets.len(),
            instance_count: candidate.instances.len(),
            vertex_count,
            triangle_count,
            projected_vertex_count,
            projected_triangle_count,
            projection_hash: candidate.identity_hash(),
        };
        std::mem::swap(self, &mut candidate);
        Ok(receipt)
    }

    pub fn raycast(&self, ray: Ray, max_distance: f64) -> Option<StaticMeshHit> {
        let len = ray.dir.length();
        if !len.is_finite() || len <= 0.0 || !max_distance.is_finite() || max_distance <= 0.0 {
            return None;
        }
        let inv = 1.0 / len;
        let direction = WorldVec::new(ray.dir.x * inv, ray.dir.y * inv, ray.dir.z * inv);
        let parry_ray = ParryRay::new(
            world_to_point(ray.origin),
            Vector::new(direction.x, direction.y, direction.z),
        );
        let pose = identity();
        let mut best = None;
        for (instance_id, instance) in &self.instances {
            if let Some(hit) =
                instance
                    .shape
                    .cast_ray_and_get_normal(&pose, &parry_ray, max_distance, false)
            {
                if best
                    .as_ref()
                    .is_none_or(|current: &StaticMeshHit| hit.time_of_impact < current.distance)
                {
                    best = Some(StaticMeshHit {
                        instance: *instance_id,
                        asset: instance.asset,
                        geometry_hash: instance.geometry_hash,
                        point: WorldPos::new(
                            ray.origin.x + direction.x * hit.time_of_impact,
                            ray.origin.y + direction.y * hit.time_of_impact,
                            ray.origin.z + direction.z * hit.time_of_impact,
                        ),
                        normal: WorldVec::new(hit.normal.x, hit.normal.y, hit.normal.z),
                        distance: hit.time_of_impact,
                    });
                }
            }
        }
        best
    }

    pub fn aabb_overlaps(&self, min: WorldPos, max: WorldPos) -> bool {
        let (pose, cuboid) = query_cuboid(min, max);
        let collider_pose = identity();
        self.instances.values().any(|instance| {
            intersection_test(&pose, &cuboid, &collider_pose, instance.shape.as_ref()) == Ok(true)
        })
    }

    pub fn swept_aabb_overlaps(&self, min: WorldPos, max: WorldPos, translation: WorldVec) -> bool {
        if ![translation.x, translation.y, translation.z]
            .into_iter()
            .all(f64::is_finite)
        {
            return true;
        }
        let (pose, cuboid) = query_cuboid(min, max);
        let velocity = Vector::new(translation.x, translation.y, translation.z);
        let collider_pose = identity();
        self.instances.values().any(|instance| {
            cast_shapes(
                &pose,
                velocity,
                &cuboid,
                &collider_pose,
                Vector::ZERO,
                instance.shape.as_ref(),
                ShapeCastOptions::with_max_time_of_impact(1.0),
            )
            .map_or(true, |hit| hit.is_some())
        })
    }
}

fn validate_geometry(
    positions: &[[f64; 3]],
    triangles: &[[u32; 3]],
) -> Result<(), StaticMeshCollisionError> {
    if positions.is_empty() || triangles.is_empty() {
        return Err(StaticMeshCollisionError::EmptyGeometry);
    }
    if positions.len() > MAX_STATIC_MESH_VERTICES_PER_ASSET {
        return Err(StaticMeshCollisionError::TooManyVertices {
            limit: MAX_STATIC_MESH_VERTICES_PER_ASSET,
        });
    }
    if triangles.len() > MAX_STATIC_MESH_TRIANGLES_PER_ASSET {
        return Err(StaticMeshCollisionError::TooManyTriangles {
            limit: MAX_STATIC_MESH_TRIANGLES_PER_ASSET,
        });
    }
    if positions.iter().flatten().any(|value| !value.is_finite()) {
        return Err(StaticMeshCollisionError::NonFiniteVertex);
    }
    for triangle in triangles {
        if let Some(index) = triangle
            .iter()
            .copied()
            .find(|index| *index as usize >= positions.len())
        {
            return Err(StaticMeshCollisionError::InvalidTriangleIndex {
                index,
                vertex_count: positions.len(),
            });
        }
    }
    SharedShape::trimesh(
        positions
            .iter()
            .map(|point| Vector::new(point[0], point[1], point[2]))
            .collect(),
        triangles.to_vec(),
    )
    .map_err(|_| StaticMeshCollisionError::InvalidTriangleMesh)?;
    Ok(())
}

fn validate_transform(transform: StaticMeshTransform) -> Result<(), StaticMeshCollisionError> {
    if !transform
        .translation
        .iter()
        .chain(&transform.rotation)
        .chain(&transform.scale)
        .all(|value| value.is_finite())
        || transform
            .scale
            .iter()
            .any(|value| value.abs() <= f64::EPSILON)
    {
        return Err(StaticMeshCollisionError::InvalidTransform);
    }
    let length_squared = transform
        .rotation
        .iter()
        .map(|value| value * value)
        .sum::<f64>();
    if length_squared <= f64::EPSILON {
        return Err(StaticMeshCollisionError::InvalidTransform);
    }
    Ok(())
}

fn transform_position(position: [f64; 3], transform: StaticMeshTransform) -> [f64; 3] {
    let scaled = [
        position[0] * transform.scale[0],
        position[1] * transform.scale[1],
        position[2] * transform.scale[2],
    ];
    let [x, y, z, w] = transform.rotation;
    let inverse_length = 1.0 / (x * x + y * y + z * z + w * w).sqrt();
    let (x, y, z, w) = (
        x * inverse_length,
        y * inverse_length,
        z * inverse_length,
        w * inverse_length,
    );
    let q = Vector::new(x, y, z);
    let v = Vector::new(scaled[0], scaled[1], scaled[2]);
    let rotated = v + 2.0 * q.cross(q.cross(v) + w * v);
    [
        rotated.x + transform.translation[0],
        rotated.y + transform.translation[1],
        rotated.z + transform.translation[2],
    ]
}

fn query_cuboid(min: WorldPos, max: WorldPos) -> (Pose, Cuboid) {
    let lo = WorldPos::new(min.x.min(max.x), min.y.min(max.y), min.z.min(max.z));
    let hi = WorldPos::new(min.x.max(max.x), min.y.max(max.y), min.z.max(max.z));
    (
        Pose::translation(
            (lo.x + hi.x) * 0.5,
            (lo.y + hi.y) * 0.5,
            (lo.z + hi.z) * 0.5,
        ),
        Cuboid::new(Vector::new(
            (hi.x - lo.x) * 0.5,
            (hi.y - lo.y) * 0.5,
            (hi.z - lo.z) * 0.5,
        )),
    )
}

fn geometry_hash(positions: &[[f64; 3]], triangles: &[[u32; 3]]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    write_hash(&mut hash, &(positions.len() as u64).to_le_bytes());
    write_hash(&mut hash, &(triangles.len() as u64).to_le_bytes());
    for value in positions.iter().flatten() {
        write_hash(&mut hash, &value.to_bits().to_le_bytes());
    }
    for index in triangles.iter().flatten() {
        write_hash(&mut hash, &index.to_le_bytes());
    }
    hash
}

fn write_hash(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(0x100000001b3);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ramp() -> StaticMeshColliderAsset {
        StaticMeshColliderAsset::new(
            StaticMeshAssetId(7),
            vec![
                [0.0, 0.0, -1.0],
                [2.0, 2.0, -1.0],
                [2.0, 2.0, 1.0],
                [0.0, 0.0, 1.0],
                [3.0, 0.0, -1.0],
                [3.0, 3.0, -1.0],
                [3.0, 3.0, 1.0],
                [3.0, 0.0, 1.0],
            ],
            vec![[0, 1, 2], [0, 2, 3], [4, 5, 6], [4, 6, 7]],
        )
        .unwrap()
    }

    #[test]
    fn ramp_raycast_and_swept_aabb_use_exact_transformed_triangles() {
        let asset = ramp();
        let hash = asset.geometry_hash;
        let mut projection = StaticMeshCollisionProjection::default();
        projection
            .replace_all(
                0,
                [asset],
                [StaticMeshColliderInstance {
                    id: StaticMeshInstanceId(11),
                    asset: StaticMeshAssetId(7),
                    expected_geometry_hash: hash,
                    transform: StaticMeshTransform::IDENTITY,
                }],
            )
            .unwrap();

        let hit = projection
            .raycast(
                Ray::new(WorldPos::new(1.0, 3.0, 0.0), WorldVec::new(0.0, -1.0, 0.0)),
                10.0,
            )
            .unwrap();
        assert_eq!(hit.instance, StaticMeshInstanceId(11));
        assert!((hit.point.y - 1.0).abs() < 1.0e-9);
        assert!(projection.swept_aabb_overlaps(
            WorldPos::new(2.0, 0.5, -0.25),
            WorldPos::new(2.5, 1.5, 0.25),
            WorldVec::new(1.0, 0.0, 0.0),
        ));
        assert!(!projection.swept_aabb_overlaps(
            WorldPos::new(2.0, 4.0, -0.25),
            WorldPos::new(2.5, 5.0, 0.25),
            WorldVec::new(1.0, 0.0, 0.0),
        ));
    }

    #[test]
    fn stale_or_invalid_replacement_is_fail_atomic() {
        let asset = ramp();
        let hash = asset.geometry_hash;
        let mut projection = StaticMeshCollisionProjection::default();
        projection
            .replace_all(
                0,
                [asset.clone()],
                [StaticMeshColliderInstance {
                    id: StaticMeshInstanceId(11),
                    asset: asset.id,
                    expected_geometry_hash: hash,
                    transform: StaticMeshTransform::IDENTITY,
                }],
            )
            .unwrap();
        let before = projection
            .raycast(
                Ray::new(WorldPos::new(1.0, 3.0, 0.0), WorldVec::new(0.0, -1.0, 0.0)),
                10.0,
            )
            .unwrap();

        let rejected = projection.replace_all(
            1,
            [asset.clone()],
            [StaticMeshColliderInstance {
                id: StaticMeshInstanceId(11),
                asset: asset.id,
                expected_geometry_hash: hash.wrapping_add(1),
                transform: StaticMeshTransform::IDENTITY,
            }],
        );
        assert!(matches!(
            rejected,
            Err(StaticMeshCollisionError::StaleAsset { .. })
        ));
        assert_eq!(projection.revision(), 1);
        assert_eq!(
            projection
                .raycast(
                    Ray::new(WorldPos::new(1.0, 3.0, 0.0), WorldVec::new(0.0, -1.0, 0.0),),
                    10.0,
                )
                .unwrap(),
            before
        );
    }

    #[test]
    fn instance_translation_rotation_and_nonuniform_scale_are_baked_before_query() {
        let asset = StaticMeshColliderAsset::new(
            StaticMeshAssetId(31),
            vec![[-1.0, -1.0, 0.0], [1.0, -1.0, 0.0], [0.0, 1.0, 0.0]],
            vec![[0, 1, 2]],
        )
        .unwrap();
        let hash = asset.geometry_hash;
        let half_sqrt = std::f64::consts::FRAC_1_SQRT_2;
        let mut projection = StaticMeshCollisionProjection::default();
        projection
            .replace_all(
                0,
                [asset],
                [StaticMeshColliderInstance {
                    id: StaticMeshInstanceId(41),
                    asset: StaticMeshAssetId(31),
                    expected_geometry_hash: hash,
                    transform: StaticMeshTransform {
                        translation: [5.0, 0.0, 0.0],
                        rotation: [0.0, half_sqrt, 0.0, half_sqrt],
                        scale: [2.0, 1.0, 0.5],
                    },
                }],
            )
            .unwrap();

        let hit = projection
            .raycast(
                Ray::new(WorldPos::new(0.0, 0.0, 0.0), WorldVec::new(1.0, 0.0, 0.0)),
                10.0,
            )
            .unwrap();
        assert_eq!(hit.instance, StaticMeshInstanceId(41));
        assert!((hit.distance - 5.0).abs() < 1.0e-9);
    }
}
