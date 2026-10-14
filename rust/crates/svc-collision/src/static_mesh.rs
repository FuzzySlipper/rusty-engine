use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use core_space::{WorldPos, WorldVec};
use parry3d_f64::math::{Pose, Vector};
use parry3d_f64::query::{cast_shapes, intersection_test, Ray as ParryRay, ShapeCastOptions};
use parry3d_f64::shape::{Cuboid, Shape, SharedShape};

use crate::{identity, world_to_point, Ray, WorldAabb};

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
    pub transform: StaticMeshTransform,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StaticMeshCollisionReceipt {
    pub revision_before: u64,
    pub revision_after: u64,
    pub asset_count: usize,
    pub instance_count: usize,
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
    RevisionExhausted,
    MissingAsset { id: StaticMeshAssetId },
    EmptyGeometry,
    NonFiniteVertex,
    InvalidTriangleIndex { index: u32, vertex_count: usize },
    InvalidTransform,
    InvalidTriangleMesh,
}

#[derive(Clone)]
struct PreparedInstance {
    asset: StaticMeshAssetId,
    geometry_hash: u64,
    transform: StaticMeshTransform,
    shape: SharedShape,
    /// Cached at admission so character queries never rebuild geometry
    /// bounds. `None` is an intentional fail-open sentinel.
    bounds: Option<WorldAabb>,
    /// Topology contributions computed once at admission: with and without
    /// the translation/rotation that an admitted moving instance may change.
    posed_topology: u64,
    moving_topology: u64,
}

impl PreparedInstance {
    fn bake(
        id: StaticMeshInstanceId,
        asset: &StaticMeshColliderAsset,
        transform: StaticMeshTransform,
    ) -> Result<Self, StaticMeshCollisionError> {
        validate_transform(transform)?;
        let vertices = asset
            .positions
            .iter()
            .copied()
            .map(|position| transform_position(position, transform))
            .map(|position| Vector::new(position[0], position[1], position[2]))
            .collect();
        let shape = SharedShape::trimesh(vertices, asset.triangles.clone())
            .map_err(|_| StaticMeshCollisionError::InvalidTriangleMesh)?;
        let bounds = shape_world_aabb(shape.as_ref());
        Ok(Self {
            asset: asset.id,
            geometry_hash: asset.geometry_hash,
            transform,
            shape,
            bounds,
            posed_topology: instance_topology(id, asset, transform, true),
            moving_topology: instance_topology(id, asset, transform, false),
        })
    }
}

#[derive(Clone)]
struct ResidentAsset {
    asset: Arc<StaticMeshColliderAsset>,
    /// The instances baked from this asset.
    users: BTreeSet<StaticMeshInstanceId>,
}

/// Query-optimized projection of immutable static-mesh assets and caller-owned
/// instances, changed by id; callers retain authored identity, transforms,
/// persistence, and mutation policy.
#[derive(Clone, Default)]
pub struct StaticMeshCollisionProjection {
    assets: BTreeMap<StaticMeshAssetId, ResidentAsset>,
    instances: BTreeMap<StaticMeshInstanceId, PreparedInstance>,
    revision: u64,
    /// Order-independent sum of every asset and posed instance contribution,
    /// maintained by each change so identity reads stay O(moving).
    topology: u64,
}

impl StaticMeshCollisionProjection {
    pub(crate) fn character_shapes(
        &self,
    ) -> impl Iterator<
        Item = (
            StaticMeshInstanceId,
            StaticMeshAssetId,
            u64,
            &SharedShape,
            Option<WorldAabb>,
        ),
    > {
        self.instances.iter().map(|(id, instance)| {
            (
                *id,
                instance.asset,
                instance.geometry_hash,
                &instance.shape,
                instance.bounds,
            )
        })
    }

    /// Return the retained authored identity and pose for one collision
    /// instance. The pose is a copied fact for a caller-owned bridge; the
    /// projection remains the collision authority and retains its own shape.
    pub(crate) fn instance_descriptor(
        &self,
        id: StaticMeshInstanceId,
    ) -> Option<(StaticMeshAssetId, u64, StaticMeshTransform)> {
        self.instances
            .get(&id)
            .map(|instance| (instance.asset, instance.geometry_hash, instance.transform))
    }

    /// Stable collision identity for retained mesh topology while allowing
    /// explicitly admitted moving instances to change pose. Unadmitted
    /// instances retain their complete pose in this identity so unrelated
    /// static collision changes still invalidate controller continuity.
    pub(crate) fn topology_hash_excluding_pose(
        &self,
        moving_instances: &BTreeSet<StaticMeshInstanceId>,
    ) -> u64 {
        moving_instances
            .iter()
            .filter_map(|id| self.instances.get(id))
            .fold(self.topology, |hash, instance| {
                hash.wrapping_sub(instance.posed_topology)
                    .wrapping_add(instance.moving_topology)
            })
    }

    pub(crate) fn dynamics_shapes(&self) -> impl Iterator<Item = SharedShape> + '_ {
        self.instances
            .values()
            .map(|instance| instance.shape.clone())
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Every instance moved by `delta`; their triangles are rebaked.
    pub(crate) fn translated(&self, delta: WorldVec) -> Result<Self, StaticMeshCollisionError> {
        if ![delta.x, delta.y, delta.z].into_iter().all(f64::is_finite) {
            return Err(StaticMeshCollisionError::InvalidTransform);
        }
        let mut candidate = Self {
            assets: self.assets.clone(),
            instances: BTreeMap::new(),
            revision: self.revision,
            topology: self.topology,
        };
        for (id, instance) in &self.instances {
            let mut transform = instance.transform;
            transform.translation[0] += delta.x;
            transform.translation[1] += delta.y;
            transform.translation[2] += delta.z;
            let moved =
                PreparedInstance::bake(*id, &self.assets[&instance.asset].asset, transform)?;
            candidate.topology = candidate
                .topology
                .wrapping_sub(instance.posed_topology)
                .wrapping_add(moved.posed_topology);
            candidate.instances.insert(*id, moved);
        }
        Ok(candidate)
    }

    pub fn asset_count(&self) -> usize {
        self.assets.len()
    }

    /// The instances baked from one resident asset.
    pub(crate) fn asset_users(
        &self,
        id: StaticMeshAssetId,
    ) -> impl Iterator<Item = StaticMeshInstanceId> + '_ {
        self.assets
            .get(&id)
            .into_iter()
            .flat_map(|asset| asset.users.iter().copied())
    }

    pub fn instance_count(&self) -> usize {
        self.instances.len()
    }

    /// Replace the complete asset and instance set: everything not listed
    /// is removed, and listed entries are applied as by
    /// [`Self::apply_residency`].
    pub fn replace_all(
        &mut self,
        assets: impl IntoIterator<Item = StaticMeshColliderAsset>,
        instances: impl IntoIterator<Item = StaticMeshColliderInstance>,
    ) -> Result<StaticMeshCollisionReceipt, StaticMeshCollisionError> {
        let assets: Vec<_> = assets.into_iter().collect();
        let instances: Vec<_> = instances.into_iter().collect();
        let listed_assets: BTreeSet<_> = assets.iter().map(|asset| asset.id).collect();
        let listed_instances: BTreeSet<_> = instances.iter().map(|instance| instance.id).collect();
        let removed_assets: Vec<_> = self
            .assets
            .keys()
            .filter(|id| !listed_assets.contains(id))
            .copied()
            .collect();
        let removed_instances: Vec<_> = self
            .instances
            .keys()
            .filter(|id| !listed_instances.contains(id))
            .copied()
            .collect();
        self.apply_residency(assets, instances, removed_assets, removed_instances)
    }

    /// Applies a residency delta by id, touching only the named assets and
    /// instances and the instances baked from a replaced asset. Removals
    /// precede upserts; a later entry for an id replaces an earlier one.
    /// Removing an absent identity is idempotent. Assets still used by a
    /// retained instance cannot be removed. A refusal changes nothing.
    pub fn apply_residency(
        &mut self,
        assets: impl IntoIterator<Item = StaticMeshColliderAsset>,
        instances: impl IntoIterator<Item = StaticMeshColliderInstance>,
        removed_assets: impl IntoIterator<Item = StaticMeshAssetId>,
        removed_instances: impl IntoIterator<Item = StaticMeshInstanceId>,
    ) -> Result<StaticMeshCollisionReceipt, StaticMeshCollisionError> {
        let revision_after = self
            .revision
            .checked_add(1)
            .ok_or(StaticMeshCollisionError::RevisionExhausted)?;
        // Assets were validated and hashed when constructed.
        let mut admitted: BTreeMap<_, _> = assets
            .into_iter()
            .map(|asset| (asset.id, Arc::new(asset)))
            .collect();
        let removed_assets: BTreeSet<_> = removed_assets
            .into_iter()
            .filter(|id| self.assets.contains_key(id) && !admitted.contains_key(id))
            .collect();
        // Re-admitting an asset's geometry keeps it and its instances.
        admitted.retain(|id, asset| {
            self.assets
                .get(id)
                .is_none_or(|resident| resident.asset.geometry_hash != asset.geometry_hash)
        });
        let upserts: BTreeMap<_, _> = instances
            .into_iter()
            .map(|instance| (instance.id, instance))
            .collect();
        let removed_instances: BTreeSet<_> = removed_instances
            .into_iter()
            .filter(|id| self.instances.contains_key(id) && !upserts.contains_key(id))
            .collect();
        let asset_after = |id: StaticMeshAssetId| {
            admitted.get(&id).or_else(|| {
                (!removed_assets.contains(&id))
                    .then(|| self.assets.get(&id).map(|resident| &resident.asset))
                    .flatten()
            })
        };

        // Build everything that can fail before changing anything.
        let mut baked = Vec::new();
        for (id, instance) in &upserts {
            let asset = asset_after(instance.asset)
                .ok_or(StaticMeshCollisionError::MissingAsset { id: instance.asset })?;
            let unchanged = self.instances.get(id).is_some_and(|previous| {
                previous.asset == asset.id
                    && previous.geometry_hash == asset.geometry_hash
                    && previous.transform == instance.transform
            });
            if !unchanged {
                baked.push((*id, PreparedInstance::bake(*id, asset, instance.transform)?));
            }
        }
        for (asset_id, asset) in &admitted {
            let Some(resident) = self.assets.get(asset_id) else {
                continue;
            };
            for id in &resident.users {
                if !removed_instances.contains(id) && !upserts.contains_key(id) {
                    let transform = self.instances[id].transform;
                    baked.push((*id, PreparedInstance::bake(*id, asset, transform)?));
                }
            }
        }
        for asset_id in &removed_assets {
            let retained = self.assets[asset_id]
                .users
                .iter()
                .any(|id| !removed_instances.contains(id) && !upserts.contains_key(id));
            if retained {
                return Err(StaticMeshCollisionError::MissingAsset { id: *asset_id });
            }
        }

        let revision_before = self.revision;
        for id in removed_instances
            .iter()
            .chain(baked.iter().map(|(id, _)| id))
        {
            self.detach(*id);
        }
        for id in removed_assets {
            let resident = self.assets.remove(&id).expect("removed asset is resident");
            self.topology = self.topology.wrapping_sub(asset_topology(&resident.asset));
        }
        for (id, asset) in admitted {
            self.topology = self.topology.wrapping_add(asset_topology(&asset));
            let users = match self.assets.remove(&id) {
                Some(previous) => {
                    self.topology = self.topology.wrapping_sub(asset_topology(&previous.asset));
                    previous.users
                }
                None => BTreeSet::new(),
            };
            self.assets.insert(id, ResidentAsset { asset, users });
        }
        for (id, instance) in baked {
            self.topology = self.topology.wrapping_add(instance.posed_topology);
            self.assets
                .get_mut(&instance.asset)
                .expect("an instance's asset is resident")
                .users
                .insert(id);
            self.instances.insert(id, instance);
        }
        self.revision = revision_after;
        Ok(StaticMeshCollisionReceipt {
            revision_before,
            revision_after,
            asset_count: self.assets.len(),
            instance_count: self.instances.len(),
            projection_hash: self.topology,
        })
    }

    /// Take one instance out of the projection and its asset's users.
    fn detach(&mut self, id: StaticMeshInstanceId) {
        let Some(instance) = self.instances.remove(&id) else {
            return;
        };
        self.topology = self.topology.wrapping_sub(instance.posed_topology);
        if let Some(asset) = self.assets.get_mut(&instance.asset) {
            asset.users.remove(&id);
        }
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

fn shape_world_aabb(shape: &dyn Shape) -> Option<WorldAabb> {
    let bounds = shape.compute_aabb(&identity());
    let min = WorldPos::new(bounds.mins.x, bounds.mins.y, bounds.mins.z);
    let max = WorldPos::new(bounds.maxs.x, bounds.maxs.y, bounds.maxs.z);
    if [min.x, min.y, min.z, max.x, max.y, max.z]
        .into_iter()
        .all(f64::is_finite)
        && min.x <= max.x
        && min.y <= max.y
        && min.z <= max.z
    {
        Some(WorldAabb { min, max })
    } else {
        None
    }
}

fn validate_geometry(
    positions: &[[f64; 3]],
    triangles: &[[u32; 3]],
) -> Result<(), StaticMeshCollisionError> {
    if positions.is_empty() || triangles.is_empty() {
        return Err(StaticMeshCollisionError::EmptyGeometry);
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

pub(crate) fn validate_transform(
    transform: StaticMeshTransform,
) -> Result<(), StaticMeshCollisionError> {
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

pub(crate) fn transform_position(position: [f64; 3], transform: StaticMeshTransform) -> [f64; 3] {
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

const ASSET_TOPOLOGY_TAG: u8 = 1;
const INSTANCE_TOPOLOGY_TAG: u8 = 2;

fn asset_topology(asset: &StaticMeshColliderAsset) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    write_hash(&mut hash, &[ASSET_TOPOLOGY_TAG]);
    write_hash(&mut hash, &asset.id.0.to_le_bytes());
    write_hash(&mut hash, &asset.geometry_hash.to_le_bytes());
    hash
}

/// One instance's topology contribution. Without `posed`, translation and
/// rotation are omitted so an admitted moving instance keeps its identity.
fn instance_topology(
    id: StaticMeshInstanceId,
    asset: &StaticMeshColliderAsset,
    transform: StaticMeshTransform,
    posed: bool,
) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    write_hash(&mut hash, &[INSTANCE_TOPOLOGY_TAG]);
    write_hash(&mut hash, &id.0.to_le_bytes());
    write_hash(&mut hash, &asset.id.0.to_le_bytes());
    write_hash(&mut hash, &asset.geometry_hash.to_le_bytes());
    if posed {
        for value in transform.translation.iter().chain(&transform.rotation) {
            write_hash(&mut hash, &value.to_bits().to_le_bytes());
        }
    }
    for value in transform.scale {
        write_hash(&mut hash, &value.to_bits().to_le_bytes());
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
    fn residency_reuses_untouched_geometry_and_rejects_orphaning_atomically() {
        let first = ramp();
        let mut projection = StaticMeshCollisionProjection::default();
        let first_instance = StaticMeshColliderInstance {
            id: StaticMeshInstanceId(11),
            asset: first.id,
            transform: StaticMeshTransform::IDENTITY,
        };
        projection
            .apply_residency([first], [first_instance], [], [])
            .unwrap();
        let retained_asset = Arc::clone(&projection.assets[&StaticMeshAssetId(7)].asset);
        let retained_shape = projection.instances[&StaticMeshInstanceId(11)]
            .shape
            .clone();
        let mut second = ramp();
        second.id = StaticMeshAssetId(8);
        let mut second_instance = first_instance;
        second_instance.id = StaticMeshInstanceId(12);
        second_instance.asset = second.id;
        second_instance.transform.translation[0] = 10.0;
        projection
            .apply_residency([second], [second_instance], [], [])
            .unwrap();
        assert!(Arc::ptr_eq(
            &retained_asset,
            &projection.assets[&StaticMeshAssetId(7)].asset
        ));
        assert!(std::ptr::eq(
            retained_shape.as_ref(),
            projection.instances[&StaticMeshInstanceId(11)]
                .shape
                .as_ref()
        ));
        let none = BTreeSet::new();
        let identity = projection.topology_hash_excluding_pose(&none);
        assert!(matches!(
            projection.apply_residency([], [], [StaticMeshAssetId(7)], []),
            Err(StaticMeshCollisionError::MissingAsset { .. })
        ));
        assert_eq!(projection.revision(), 2);
        assert_eq!(projection.topology_hash_excluding_pose(&none), identity);
        projection
            .apply_residency([], [], [StaticMeshAssetId(7)], [StaticMeshInstanceId(11)])
            .unwrap();
        assert_eq!(projection.asset_count(), 1);
        let shifted = projection
            .translated(WorldVec::new(-10.0, 0.0, 0.0))
            .unwrap();
        let hit = shifted
            .raycast(
                Ray::new(WorldPos::new(1.0, 3.0, 0.0), WorldVec::new(0.0, -1.0, 0.0)),
                10.0,
            )
            .unwrap();
        assert_eq!(hit.instance, StaticMeshInstanceId(12));
    }

    #[test]
    fn maintained_topology_identity_depends_only_on_the_resulting_set() {
        let asset = ramp();
        let instance = |id, x| StaticMeshColliderInstance {
            id: StaticMeshInstanceId(id),
            asset: StaticMeshAssetId(7),
            transform: StaticMeshTransform {
                translation: [x, 0.0, 0.0],
                ..StaticMeshTransform::IDENTITY
            },
        };
        let none = std::collections::BTreeSet::new();
        let mut empty = StaticMeshCollisionProjection::default();
        let default_identity = empty.topology_hash_excluding_pose(&none);
        empty.replace_all([], []).unwrap();
        assert_eq!(empty.topology_hash_excluding_pose(&none), default_identity);

        let mut whole = StaticMeshCollisionProjection::default();
        whole
            .replace_all([asset.clone()], [instance(11, 0.0), instance(12, 5.0)])
            .unwrap();
        let mut incremental = StaticMeshCollisionProjection::default();
        incremental
            .replace_all([asset], [instance(12, 1.0)])
            .unwrap();
        incremental
            .apply_residency([], [instance(11, 0.0), instance(12, 5.0)], [], [])
            .unwrap();
        assert_eq!(
            incremental.topology_hash_excluding_pose(&none),
            whole.topology_hash_excluding_pose(&none)
        );
        let moving = std::collections::BTreeSet::from([StaticMeshInstanceId(12)]);
        assert_ne!(
            whole.topology_hash_excluding_pose(&moving),
            whole.topology_hash_excluding_pose(&none)
        );
    }

    #[test]
    fn topology_identity_excludes_only_admitted_mesh_pose() {
        let asset = ramp();
        let mut projection = StaticMeshCollisionProjection::default();
        projection
            .replace_all(
                [asset],
                [
                    StaticMeshColliderInstance {
                        id: StaticMeshInstanceId(11),
                        asset: StaticMeshAssetId(7),
                        transform: StaticMeshTransform::IDENTITY,
                    },
                    StaticMeshColliderInstance {
                        id: StaticMeshInstanceId(12),
                        asset: StaticMeshAssetId(7),
                        transform: StaticMeshTransform {
                            translation: [5.0, 0.0, 0.0],
                            ..StaticMeshTransform::IDENTITY
                        },
                    },
                ],
            )
            .unwrap();
        let moving = std::collections::BTreeSet::from([StaticMeshInstanceId(11)]);
        let before = projection.topology_hash_excluding_pose(&moving);
        let none = BTreeSet::new();
        let full_before = projection.topology_hash_excluding_pose(&none);
        projection
            .apply_residency(
                [],
                [StaticMeshColliderInstance {
                    id: StaticMeshInstanceId(11),
                    asset: StaticMeshAssetId(7),
                    transform: StaticMeshTransform {
                        translation: [2.0, 0.0, 0.0],
                        ..StaticMeshTransform::IDENTITY
                    },
                }],
                [],
                [],
            )
            .unwrap();
        assert_eq!(projection.topology_hash_excluding_pose(&moving), before);
        assert_ne!(projection.topology_hash_excluding_pose(&none), full_before);

        projection
            .apply_residency(
                [],
                [StaticMeshColliderInstance {
                    id: StaticMeshInstanceId(12),
                    asset: StaticMeshAssetId(7),
                    transform: StaticMeshTransform {
                        translation: [6.0, 0.0, 0.0],
                        ..StaticMeshTransform::IDENTITY
                    },
                }],
                [],
                [],
            )
            .unwrap();
        assert_ne!(projection.topology_hash_excluding_pose(&moving), before);
    }

    #[test]
    fn admits_more_assets_and_instances_than_the_former_caps() {
        // 300 assets and 5,000 instances: past the former 256 and 4,096 caps.
        let assets: Vec<_> = (0..300_u64)
            .map(|id| {
                StaticMeshColliderAsset::new(
                    StaticMeshAssetId(id),
                    vec![
                        [0.0, 0.0, 0.0],
                        [1.0, 0.0, 0.0],
                        [0.0, 0.0, 1.0 + id as f64],
                    ],
                    vec![[0, 1, 2]],
                )
                .unwrap()
            })
            .collect();
        let instances: Vec<_> = (0..5_000_u64)
            .map(|id| {
                let asset = &assets[(id % 300) as usize];
                StaticMeshColliderInstance {
                    id: StaticMeshInstanceId(id),
                    asset: asset.id,
                    transform: StaticMeshTransform {
                        translation: [(id % 100) as f64 * 2.0, 0.0, (id / 100) as f64 * 2.0],
                        ..StaticMeshTransform::IDENTITY
                    },
                }
            })
            .collect();
        let mut projection = StaticMeshCollisionProjection::default();
        let receipt = projection.replace_all(assets, instances).unwrap();
        assert_eq!(receipt.asset_count, 300);
        assert_eq!(receipt.instance_count, 5_000);
        assert!(projection
            .raycast(
                Ray::new(
                    WorldPos::new(0.25, 1.0, 0.25),
                    WorldVec::new(0.0, -1.0, 0.0)
                ),
                2.0,
            )
            .is_some());
    }

    #[test]
    fn ramp_raycast_and_swept_aabb_use_exact_transformed_triangles() {
        let asset = ramp();
        let mut projection = StaticMeshCollisionProjection::default();
        projection
            .replace_all(
                [asset],
                [StaticMeshColliderInstance {
                    id: StaticMeshInstanceId(11),
                    asset: StaticMeshAssetId(7),
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
    fn repeated_ids_replace_and_a_replaced_asset_rebakes_only_its_instances() {
        let instance = |id, asset, x| StaticMeshColliderInstance {
            id: StaticMeshInstanceId(id),
            asset: StaticMeshAssetId(asset),
            transform: StaticMeshTransform {
                translation: [x, 0.0, 0.0],
                ..StaticMeshTransform::IDENTITY
            },
        };
        let mut other = ramp();
        other.id = StaticMeshAssetId(8);
        let mut projection = StaticMeshCollisionProjection::default();
        // The later asset and instance of a repeated id win.
        let receipt = projection
            .replace_all(
                [ramp(), ramp(), other.clone()],
                [
                    instance(11, 7, 0.0),
                    instance(11, 7, 10.0),
                    instance(12, 8, 20.0),
                ],
            )
            .unwrap();
        assert_eq!((receipt.asset_count, receipt.instance_count), (2, 2));
        assert_eq!(
            projection.instances[&StaticMeshInstanceId(11)]
                .transform
                .translation[0],
            10.0
        );
        let untouched = projection.instances[&StaticMeshInstanceId(12)]
            .shape
            .clone();

        // New geometry under asset 7 rebakes instance 11 and nothing else.
        let taller = StaticMeshColliderAsset::new(
            StaticMeshAssetId(7),
            vec![[0.0, 5.0, -1.0], [2.0, 5.0, -1.0], [0.0, 5.0, 1.0]],
            vec![[0, 2, 1]],
        )
        .unwrap();
        let receipt = projection
            .apply_residency([taller.clone()], [], [], [])
            .unwrap();
        assert!(std::ptr::eq(
            untouched.as_ref(),
            projection.instances[&StaticMeshInstanceId(12)]
                .shape
                .as_ref()
        ));
        let hit = projection
            .raycast(
                Ray::new(WorldPos::new(10.5, 9.0, 0.0), WorldVec::new(0.0, -1.0, 0.0)),
                10.0,
            )
            .unwrap();
        assert_eq!(hit.instance, StaticMeshInstanceId(11));
        assert!((hit.point.y - 5.0).abs() < 1.0e-9);

        // The maintained identity is the one a fresh build of the same set has.
        let mut fresh = StaticMeshCollisionProjection::default();
        let rebuilt = fresh
            .replace_all(
                [taller, other],
                [instance(11, 7, 10.0), instance(12, 8, 20.0)],
            )
            .unwrap();
        assert_eq!(receipt.projection_hash, rebuilt.projection_hash);
    }

    #[test]
    fn invalid_replacement_is_fail_atomic() {
        let asset = ramp();
        let mut projection = StaticMeshCollisionProjection::default();
        projection
            .replace_all(
                [asset.clone()],
                [StaticMeshColliderInstance {
                    id: StaticMeshInstanceId(11),
                    asset: asset.id,
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
            [asset.clone()],
            [StaticMeshColliderInstance {
                id: StaticMeshInstanceId(11),
                asset: StaticMeshAssetId(asset.id.0 + 1),
                transform: StaticMeshTransform::IDENTITY,
            }],
        );
        assert!(matches!(
            rejected,
            Err(StaticMeshCollisionError::MissingAsset { .. })
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
        let half_sqrt = std::f64::consts::FRAC_1_SQRT_2;
        let mut projection = StaticMeshCollisionProjection::default();
        projection
            .replace_all(
                [asset],
                [StaticMeshColliderInstance {
                    id: StaticMeshInstanceId(41),
                    asset: StaticMeshAssetId(31),
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
