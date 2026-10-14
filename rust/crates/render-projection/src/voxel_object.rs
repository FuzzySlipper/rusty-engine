use std::collections::{BTreeMap, BTreeSet};

use render_model::{
    pack_mesh_resources, MeshAttribute, MeshAttributeKind, MeshAttributeName, MeshBoundsDescriptor,
    MeshBufferLayout, MeshDescriptorError, MeshGroupDescriptor, MeshIndexWidth, MeshMaterialSlot,
    MeshPayloadDescriptor, MeshPayloadSource, MeshProvenance, MeshResourceError, MeshTextureSpace,
    PackedMeshResource, RenderDiff, RenderFrameDiff, RenderHandle, RenderMaterialDescriptor,
    RenderMetadata, Transform, VoxelObjectInstanceDescriptor, VoxelObjectRenderAsset,
    VoxelObjectRenderFrame, VoxelObjectRenderMesh, MAX_MESH_RESOURCE_BYTES,
};
use svc_mesh::SurfaceMode;
use voxel_object_runtime::{AdmittedVoxelObject, VoxelObjectFrameSource, VoxelObjectRuntimeFrame};

use crate::{HandleAllocationError, RenderHandleNamespace, StableHandleRegistry};

#[derive(Debug)]
pub struct VoxelObjectProjectionInstance<'a> {
    pub instance_id: String,
    pub object: &'a AdmittedVoxelObject,
    pub frame: u32,
    pub transform: Transform,
    pub visible: bool,
    pub material_overrides: Vec<MeshMaterialSlot>,
    pub metadata: RenderMetadata,
}

#[derive(Debug, Clone, PartialEq)]
struct InstanceSnapshot {
    /// The render resource the instance draws.
    asset: String,
    /// The admitted object's own asset id, which `asset` adds a surface mode
    /// to when instances of one object draw more than one mode.
    object_asset: String,
    content_hash: String,
    frame: u32,
    transform: Transform,
    visible: bool,
    material_overrides: Vec<MeshMaterialSlot>,
    metadata: RenderMetadata,
    surface_mode: SurfaceMode,
    /// Every material the instance uses, from its palette and overrides.
    materials: BTreeSet<String>,
}

impl InstanceSnapshot {
    /// An instance whose resource or material overrides changed is destroyed
    /// and created again: the renderer has no update for either.
    fn rebinds(&self, previous: &InstanceSnapshot) -> bool {
        self.asset != previous.asset || self.material_overrides != previous.material_overrides
    }

    fn create(&self, handle: RenderHandle) -> RenderDiff {
        RenderDiff::CreateVoxelObjectInstance {
            handle,
            parent: None,
            instance: VoxelObjectInstanceDescriptor {
                asset: self.asset.clone(),
                frame: self.frame,
                transform: self.transform,
                visible: self.visible,
                material_overrides: self.material_overrides.clone(),
                metadata: self.metadata.clone(),
            },
        }
    }

    /// The updates that take a retained instance from `previous` to this.
    fn update(
        &self,
        handle: RenderHandle,
        previous: &InstanceSnapshot,
        operations: &mut Vec<RenderDiff>,
    ) {
        if previous.transform != self.transform
            || previous.visible != self.visible
            || previous.metadata != self.metadata
        {
            operations.push(RenderDiff::Update {
                handle,
                transform: (previous.transform != self.transform).then_some(self.transform),
                material: None,
                visible: (previous.visible != self.visible).then_some(self.visible),
                metadata: (previous.metadata != self.metadata).then(|| self.metadata.clone()),
            });
        }
        if previous.frame != self.frame
            || previous.content_hash != self.content_hash
            || previous.surface_mode != self.surface_mode
        {
            operations.push(RenderDiff::SetVoxelObjectFrame {
                handle,
                frame: self.frame,
            });
        }
    }
}

/// How many retained instances use each resource, object surface mode and
/// material, so one instance's change finds what it adds or releases
/// without visiting the others.
#[derive(Debug, Clone, Default)]
struct InstanceUses {
    resources: BTreeMap<String, usize>,
    surface_modes: BTreeMap<String, BTreeMap<SurfaceMode, usize>>,
    materials: BTreeMap<String, usize>,
}

impl InstanceUses {
    fn add(&mut self, snapshot: &InstanceSnapshot) {
        count_up(&mut self.resources, &snapshot.asset);
        if !self.surface_modes.contains_key(&snapshot.object_asset) {
            self.surface_modes
                .insert(snapshot.object_asset.clone(), BTreeMap::new());
        }
        let modes = self
            .surface_modes
            .get_mut(&snapshot.object_asset)
            .expect("surface modes were just inserted");
        *modes.entry(snapshot.surface_mode).or_default() += 1;
        for material in &snapshot.materials {
            count_up(&mut self.materials, material);
        }
    }

    /// Returns whether the instance's resource lost its last user, and calls
    /// `unused_material` for each material that did.
    fn remove(
        &mut self,
        snapshot: &InstanceSnapshot,
        mut unused_material: impl FnMut(&str),
    ) -> bool {
        if let Some(modes) = self.surface_modes.get_mut(&snapshot.object_asset) {
            if count_down(modes, &snapshot.surface_mode) && modes.is_empty() {
                self.surface_modes.remove(&snapshot.object_asset);
            }
        }
        for material in &snapshot.materials {
            if count_down(&mut self.materials, material.as_str()) {
                unused_material(material);
            }
        }
        count_down(&mut self.resources, snapshot.asset.as_str())
    }
}

fn count_up(counts: &mut BTreeMap<String, usize>, key: &str) {
    match counts.get_mut(key) {
        Some(count) => *count += 1,
        None => {
            counts.insert(key.to_owned(), 1);
        }
    }
}

/// Returns whether the key's count reached zero, which removes it.
fn count_down<K, Q>(counts: &mut BTreeMap<K, usize>, key: &Q) -> bool
where
    K: std::borrow::Borrow<Q> + Ord,
    Q: Ord + ?Sized,
{
    let Some(count) = counts.get_mut(key) else {
        return false;
    };
    *count -= 1;
    if *count == 0 {
        counts.remove(key);
        return true;
    }
    false
}

#[derive(Debug, Clone)]
pub struct VoxelObjectRenderProjector {
    registry: StableHandleRegistry<String>,
    last_instances: BTreeMap<String, InstanceSnapshot>,
    last_resource_hashes: BTreeMap<String, String>,
    last_resource_surface_modes: BTreeMap<String, SurfaceMode>,
    last_materials: BTreeMap<String, RenderMaterialDescriptor>,
    uses: InstanceUses,
    mesh_payloads: VoxelObjectMeshPayloads,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VoxelObjectMeshPayloads {
    Inline,
    PackedResources { maximum_resource_bytes: u32 },
}

impl Default for VoxelObjectRenderProjector {
    fn default() -> Self {
        Self::new()
    }
}

/// One instance's accepted replacement, ready to publish.
struct NextInstance {
    snapshot: InstanceSnapshot,
    defined_materials: Vec<RenderMaterialDescriptor>,
    resource: Option<VoxelObjectRenderAsset>,
}

impl VoxelObjectRenderProjector {
    pub fn new() -> Self {
        Self {
            registry: StableHandleRegistry::new(RenderHandleNamespace::VOXEL_OBJECT),
            last_instances: BTreeMap::new(),
            last_resource_hashes: BTreeMap::new(),
            last_resource_surface_modes: BTreeMap::new(),
            last_materials: BTreeMap::new(),
            uses: InstanceUses::default(),
            mesh_payloads: VoxelObjectMeshPayloads::Inline,
        }
    }

    /// Builds content-addressed packed mesh resources instead of embedding
    /// expanded number arrays in the retained control frame.
    pub fn with_packed_mesh_resources() -> Self {
        Self {
            mesh_payloads: VoxelObjectMeshPayloads::PackedResources {
                maximum_resource_bytes: MAX_MESH_RESOURCE_BYTES,
            },
            ..Self::new()
        }
    }

    /// Projects the whole instance set, replacing whatever was projected.
    pub fn project(
        &mut self,
        instances: &[VoxelObjectProjectionInstance<'_>],
        materials: &BTreeMap<String, RenderMaterialDescriptor>,
    ) -> Result<VoxelObjectProjectionResult, VoxelObjectProjectionError> {
        let validated = validate_and_snapshot(instances, materials)?;
        let current_instances = validated.instances;
        let requested_resources = validated.resources;
        let used_materials = validated.materials;
        let current_resource_hashes = requested_resources
            .iter()
            .map(|(asset, request)| (asset.clone(), request.object.content_hash().to_string()))
            .collect::<BTreeMap<_, _>>();
        let current_resource_surface_modes = requested_resources
            .iter()
            .map(|(asset, request)| (asset.clone(), request.object.surface_mode()))
            .collect::<BTreeMap<_, _>>();
        let mut pending_resources = BTreeMap::new();
        let mut packed_mesh_resources = BTreeMap::new();
        for (asset_id, request) in &requested_resources {
            if self.resource_is_cached(asset_id, request.object) {
                continue;
            }
            let (resource, packed) = self.build_resource(request.object, asset_id)?;
            for packed_resource in packed {
                packed_mesh_resources
                    .entry(packed_resource.resource.clone())
                    .or_insert(packed_resource);
            }
            pending_resources.insert(asset_id.clone(), resource);
        }
        let materialized_resources = pending_resources.keys().cloned().collect::<Vec<_>>();
        let mut registry = self.registry.clone();
        let mut operations = Vec::new();

        let mut defined_materials = Vec::new();
        for (id, material) in &used_materials {
            if self.last_materials.get(*id) != Some(*material) {
                operations.push(RenderDiff::DefineMaterial {
                    material: (*material).clone(),
                });
                defined_materials.push((*material).clone());
            }
        }

        for (instance_id, previous) in &self.last_instances {
            let rebound = current_instances
                .get(instance_id)
                .is_some_and(|current| current.rebinds(previous));
            if !current_instances.contains_key(instance_id) || rebound {
                let handle = registry
                    .remove(instance_id)
                    .expect("retained voxel-object instance has a render handle");
                operations.push(RenderDiff::Destroy { handle });
            }
        }

        for resource in pending_resources.into_values() {
            operations.push(RenderDiff::DefineVoxelObject { asset: resource });
        }

        for (instance_id, snapshot) in &current_instances {
            let previous = self.last_instances.get(instance_id);
            let rebound = previous.is_some_and(|value| snapshot.rebinds(value));
            if previous.is_none() || rebound {
                let handle = registry
                    .allocate(instance_id.clone())
                    .map_err(VoxelObjectProjectionError::Handle)?;
                operations.push(snapshot.create(handle));
                continue;
            }

            let previous = previous.expect("existing voxel-object snapshot");
            let handle = registry
                .handle_of(instance_id)
                .expect("retained voxel-object instance has a render handle");
            snapshot.update(handle, previous, &mut operations);
        }

        for asset in self.last_resource_hashes.keys() {
            if !requested_resources.contains_key(asset) {
                operations.push(RenderDiff::ReleaseVoxelObject {
                    asset: asset.clone(),
                });
            }
        }

        let frame = RenderFrameDiff::from_ops(operations);
        self.registry = registry;
        self.uses = InstanceUses::default();
        for snapshot in current_instances.values() {
            self.uses.add(snapshot);
        }
        self.last_instances = current_instances;
        self.last_resource_hashes = current_resource_hashes;
        self.last_resource_surface_modes = current_resource_surface_modes;
        self.last_materials
            .retain(|id, _| used_materials.contains_key(id.as_str()));
        for material in defined_materials {
            self.last_materials.insert(material.id.clone(), material);
        }

        Ok(VoxelObjectProjectionResult {
            frame,
            mesh_resources: packed_mesh_resources.into_values().collect(),
            readout: VoxelObjectProjectionReadout {
                instance_frames: self
                    .last_instances
                    .iter()
                    .map(|(id, snapshot)| (id.clone(), snapshot.frame))
                    .collect(),
                resource_hashes: self.last_resource_hashes.clone(),
                surface_modes: self.last_resource_surface_modes.clone(),
                materialized_resources,
            },
        })
    }

    /// Puts one instance in place (adding it, or replacing the instance with
    /// its id) and publishes only what that changes: the frame is the one
    /// [`Self::project`] would publish for the whole set with this entry
    /// replaced. `materials` holds the materials this instance uses; the
    /// other instances keep the materials they were projected with. A
    /// refused change leaves the projector as it was.
    pub fn put_instance(
        &mut self,
        instance: &VoxelObjectProjectionInstance<'_>,
        materials: &BTreeMap<String, RenderMaterialDescriptor>,
    ) -> Result<VoxelObjectInstanceProjection, VoxelObjectProjectionError> {
        if instance.instance_id.trim().is_empty() {
            return Err(VoxelObjectProjectionError::EmptyInstanceId);
        }
        let object = instance.object;
        if object.frame(instance.frame).is_none() {
            return Err(VoxelObjectProjectionError::FrameOutOfRange {
                instance: instance.instance_id.clone(),
                frame: instance.frame,
                frame_count: object.frames().len() as u32,
            });
        }
        let previous = self.last_instances.get(&instance.instance_id);
        let next = (object.asset_id(), object.surface_mode());
        if let Some(previous) = previous.filter(|previous| previous.object_asset != next.0) {
            self.one_resource_per_object(&previous.object_asset, Some(previous), Some(next))?;
        }
        let asset = if self.one_resource_per_object(next.0, previous, Some(next))? {
            object.asset_id().to_string()
        } else {
            surface_mode_asset_id(object)
        };
        let cached = self.resource_is_cached(&asset, object);
        let other_users = self.uses.resources.get(&asset).copied().unwrap_or(0)
            - usize::from(previous.is_some_and(|previous| previous.asset == asset));
        if other_users > 0 && !cached {
            return Err(VoxelObjectProjectionError::ConflictingResource { asset });
        }
        let (snapshot, used_materials) = snapshot_instance(instance, asset, materials)?;
        let creates = previous.is_none_or(|previous| snapshot.rebinds(previous));
        if creates && !self.registry.can_allocate(1) {
            return Err(VoxelObjectProjectionError::Handle(
                HandleAllocationError::NamespaceExhausted {
                    namespace: RenderHandleNamespace::VOXEL_OBJECT.raw(),
                },
            ));
        }
        let (resource, mesh_resources) = if cached {
            (None, Vec::new())
        } else {
            let (resource, packed) = self.build_resource(object, &snapshot.asset)?;
            // By resource id, once each, as a whole projection lists them.
            let mut by_id = BTreeMap::new();
            for packed_resource in packed {
                by_id
                    .entry(packed_resource.resource.clone())
                    .or_insert(packed_resource);
            }
            (Some(resource), by_id.into_values().collect())
        };
        let defined_materials = used_materials
            .into_iter()
            .filter(|(id, material)| self.last_materials.get(*id) != Some(*material))
            .map(|(_, material)| material.clone())
            .collect();
        let frame = self.publish_change(
            &instance.instance_id,
            Some(NextInstance {
                snapshot,
                defined_materials,
                resource,
            }),
        );
        Ok(VoxelObjectInstanceProjection {
            frame,
            mesh_resources,
        })
    }

    /// Removes one instance and publishes only what that changes, as
    /// [`Self::put_instance`] does. An id that is not retained publishes
    /// nothing.
    pub fn remove_instance(
        &mut self,
        instance_id: &str,
    ) -> Result<RenderFrameDiff, VoxelObjectProjectionError> {
        let Some(previous) = self.last_instances.get(instance_id) else {
            return Ok(RenderFrameDiff::default());
        };
        self.one_resource_per_object(&previous.object_asset, Some(previous), None)?;
        Ok(self.publish_change(instance_id, None))
    }

    /// A material retained instances use: its published descriptor and how
    /// many instances use it.
    pub fn used_material(&self, id: &str) -> Option<(&RenderMaterialDescriptor, usize)> {
        self.last_materials
            .get(id)
            .zip(self.uses.materials.get(id).copied())
    }

    pub fn handle(&self, instance_id: &str) -> Option<RenderHandle> {
        self.registry.handle_of(&instance_id.to_string())
    }

    /// Whether the instances of `object_asset` share one resource once
    /// `previous` is replaced by `next`; with several surface modes each
    /// mode has its own. A change that would move the asset's other
    /// instances between the two is refused, since only the changed
    /// instance's object is at hand to rebuild resources.
    fn one_resource_per_object(
        &self,
        object_asset: &str,
        previous: Option<&InstanceSnapshot>,
        next: Option<(&str, SurfaceMode)>,
    ) -> Result<bool, VoxelObjectProjectionError> {
        let before = self.uses.surface_modes.get(object_asset);
        let mut after = before.cloned().unwrap_or_default();
        if let Some(previous) = previous.filter(|previous| previous.object_asset == object_asset) {
            count_down(&mut after, &previous.surface_mode);
        }
        let next_mode = next
            .filter(|(asset, _)| *asset == object_asset)
            .map(|(_, mode)| mode);
        if let Some(mode) = next_mode {
            *after.entry(mode).or_default() += 1;
        }
        let others = after.values().sum::<usize>() - usize::from(next_mode.is_some());
        let one_before = before.is_none_or(|modes| modes.len() <= 1);
        let one_after = after.len() <= 1;
        if others > 0 && one_before != one_after {
            return Err(VoxelObjectProjectionError::SurfaceModeRegrouping {
                asset: object_asset.to_string(),
            });
        }
        Ok(one_after)
    }

    fn resource_is_cached(&self, asset_id: &str, object: &AdmittedVoxelObject) -> bool {
        self.last_resource_hashes
            .get(asset_id)
            .is_some_and(|content_hash| content_hash == object.content_hash())
            && self.last_resource_surface_modes.get(asset_id) == Some(&object.surface_mode())
    }

    fn build_resource(
        &self,
        object: &AdmittedVoxelObject,
        asset_id: &str,
    ) -> Result<(VoxelObjectRenderAsset, Vec<PackedMeshResource>), VoxelObjectProjectionError> {
        match self.mesh_payloads {
            VoxelObjectMeshPayloads::Inline => {
                Ok((voxel_object_render_asset_as(object, asset_id), Vec::new()))
            }
            VoxelObjectMeshPayloads::PackedResources {
                maximum_resource_bytes,
            } => voxel_object_packed_render_asset_as(object, asset_id, maximum_resource_bytes)
                .map_err(VoxelObjectProjectionError::MeshResource),
        }
    }

    /// Publishes an accepted change of one instance, in the operation order
    /// [`Self::project`] uses, and retains it.
    fn publish_change(&mut self, instance_id: &str, next: Option<NextInstance>) -> RenderFrameDiff {
        let instance_id = instance_id.to_string();
        let previous = self.last_instances.remove(&instance_id);
        let mut operations = Vec::new();
        let Some(next) = next else {
            let previous = previous.expect("a removed voxel-object instance is retained");
            let handle = self
                .registry
                .remove(&instance_id)
                .expect("retained voxel-object instance has a render handle");
            operations.push(RenderDiff::Destroy { handle });
            self.release_uses(&previous, &mut operations);
            return RenderFrameDiff::from_ops(operations);
        };
        for material in &next.defined_materials {
            operations.push(RenderDiff::DefineMaterial {
                material: material.clone(),
            });
        }
        let snapshot = next.snapshot;
        let rebound = previous
            .as_ref()
            .is_some_and(|previous| snapshot.rebinds(previous));
        if rebound {
            let handle = self
                .registry
                .remove(&instance_id)
                .expect("retained voxel-object instance has a render handle");
            operations.push(RenderDiff::Destroy { handle });
        }
        if let Some(resource) = next.resource {
            operations.push(RenderDiff::DefineVoxelObject { asset: resource });
        }
        match previous.as_ref().filter(|_| !rebound) {
            Some(previous) => {
                let handle = self
                    .registry
                    .handle_of(&instance_id)
                    .expect("retained voxel-object instance has a render handle");
                snapshot.update(handle, previous, &mut operations);
            }
            None => {
                let handle = self
                    .registry
                    .allocate(instance_id.clone())
                    .expect("the voxel-object handle namespace was checked");
                operations.push(snapshot.create(handle));
            }
        }
        self.uses.add(&snapshot);
        self.last_resource_hashes
            .insert(snapshot.asset.clone(), snapshot.content_hash.clone());
        self.last_resource_surface_modes
            .insert(snapshot.asset.clone(), snapshot.surface_mode);
        if let Some(previous) = previous {
            self.release_uses(&previous, &mut operations);
        }
        for material in next.defined_materials {
            self.last_materials.insert(material.id.clone(), material);
        }
        self.last_instances.insert(instance_id, snapshot);
        RenderFrameDiff::from_ops(operations)
    }

    /// Drops a replaced or removed instance's uses, releasing a resource
    /// and forgetting materials nothing uses any more.
    fn release_uses(&mut self, previous: &InstanceSnapshot, operations: &mut Vec<RenderDiff>) {
        let last_materials = &mut self.last_materials;
        let released = self.uses.remove(previous, |material| {
            last_materials.remove(material);
        });
        if released {
            self.last_resource_hashes.remove(&previous.asset);
            self.last_resource_surface_modes.remove(&previous.asset);
            operations.push(RenderDiff::ReleaseVoxelObject {
                asset: previous.asset.clone(),
            });
        }
    }
}

struct RequestedResource<'a> {
    object: &'a AdmittedVoxelObject,
}

struct ValidatedProjection<'a, 'm> {
    instances: BTreeMap<String, InstanceSnapshot>,
    resources: BTreeMap<String, RequestedResource<'a>>,
    materials: BTreeMap<&'m str, &'m RenderMaterialDescriptor>,
}

/// Snapshots the instances and resolves the resources and materials they
/// use. A repeated instance id takes its last entry before anything is
/// resolved, so an overwritten entry adds no resource or material demand.
fn validate_and_snapshot<'a, 'm>(
    instances: &[VoxelObjectProjectionInstance<'a>],
    materials: &'m BTreeMap<String, RenderMaterialDescriptor>,
) -> Result<ValidatedProjection<'a, 'm>, VoxelObjectProjectionError> {
    let mut surviving = BTreeMap::<&str, &VoxelObjectProjectionInstance<'a>>::new();
    for instance in instances {
        surviving.insert(&instance.instance_id, instance);
    }
    let mut modes_by_asset = BTreeMap::<&str, BTreeSet<SurfaceMode>>::new();
    for instance in surviving.values() {
        modes_by_asset
            .entry(instance.object.asset_id())
            .or_default()
            .insert(instance.object.surface_mode());
    }
    let mut snapshots = BTreeMap::new();
    let mut resources = BTreeMap::<String, RequestedResource<'a>>::new();
    let mut used_materials = BTreeMap::new();
    for instance in surviving.into_values() {
        if instance.instance_id.trim().is_empty() {
            return Err(VoxelObjectProjectionError::EmptyInstanceId);
        }
        if instance.object.frame(instance.frame).is_none() {
            return Err(VoxelObjectProjectionError::FrameOutOfRange {
                instance: instance.instance_id.clone(),
                frame: instance.frame,
                frame_count: instance.object.frames().len() as u32,
            });
        }
        let asset_id = projection_asset_id(instance.object, &modes_by_asset);
        if let Some(existing) = resources.get(&asset_id) {
            if existing.object.content_hash() != instance.object.content_hash()
                || existing.object.surface_mode() != instance.object.surface_mode()
            {
                return Err(VoxelObjectProjectionError::ConflictingResource {
                    asset: asset_id.clone(),
                });
            }
        } else {
            resources.insert(
                asset_id.clone(),
                RequestedResource {
                    object: instance.object,
                },
            );
        }
        let (snapshot, instance_materials) = snapshot_instance(instance, asset_id, materials)?;
        used_materials.extend(instance_materials);
        snapshots.insert(instance.instance_id.clone(), snapshot);
    }
    Ok(ValidatedProjection {
        instances: snapshots,
        resources,
        materials: used_materials,
    })
}

/// Snapshots one instance drawing the `asset` resource and resolves the
/// materials it uses, by id.
fn snapshot_instance<'m>(
    instance: &VoxelObjectProjectionInstance<'_>,
    asset: String,
    materials: &'m BTreeMap<String, RenderMaterialDescriptor>,
) -> Result<
    (
        InstanceSnapshot,
        BTreeMap<&'m str, &'m RenderMaterialDescriptor>,
    ),
    VoxelObjectProjectionError,
> {
    let mut used_materials = BTreeMap::new();
    let bound_slots = instance
        .object
        .source()
        .material_palette
        .iter()
        .map(|binding| binding.material_slot)
        .collect::<BTreeSet<_>>();
    for binding in &instance.object.source().material_palette {
        collect_material(&binding.material_asset_id, materials, &mut used_materials)?;
    }
    // A repeated override slot takes its last material, and only that
    // one is resolved and published.
    let overrides = instance
        .material_overrides
        .iter()
        .map(|binding| (binding.slot, binding))
        .collect::<BTreeMap<_, _>>();
    for (slot, binding) in &overrides {
        if !bound_slots.contains(slot) {
            return Err(VoxelObjectProjectionError::UnboundMaterialOverride {
                instance: instance.instance_id.clone(),
                slot: *slot,
            });
        }
        collect_material(&binding.material, materials, &mut used_materials)?;
    }
    let snapshot = InstanceSnapshot {
        asset,
        object_asset: instance.object.asset_id().to_string(),
        content_hash: instance.object.content_hash().to_string(),
        frame: instance.frame,
        transform: instance.transform,
        visible: instance.visible,
        material_overrides: overrides.into_values().cloned().collect(),
        metadata: instance.metadata.clone(),
        surface_mode: instance.object.surface_mode(),
        materials: used_materials.keys().map(|id| id.to_string()).collect(),
    };
    Ok((snapshot, used_materials))
}

/// Resolves a material an instance uses. The map is keyed by material id.
fn collect_material<'m>(
    asset_id: &str,
    materials: &'m BTreeMap<String, RenderMaterialDescriptor>,
    used_materials: &mut BTreeMap<&'m str, &'m RenderMaterialDescriptor>,
) -> Result<(), VoxelObjectProjectionError> {
    let (id, material) = materials.get_key_value(asset_id).ok_or_else(|| {
        VoxelObjectProjectionError::MissingMaterial {
            asset: asset_id.to_string(),
        }
    })?;
    used_materials.insert(id, material);
    Ok(())
}

pub fn voxel_object_render_asset(object: &AdmittedVoxelObject) -> VoxelObjectRenderAsset {
    voxel_object_render_asset_as(object, object.asset_id())
}

fn voxel_object_render_asset_as(
    object: &AdmittedVoxelObject,
    render_asset_id: &str,
) -> VoxelObjectRenderAsset {
    let grid = &object.source().grid;
    let texture_space = MeshTextureSpace {
        cell_size: grid.cell_size as f32,
        origin: grid.pivot.map(|cell| cell as f32),
    };
    VoxelObjectRenderAsset {
        asset: render_asset_id.to_string(),
        content_hash: object.content_hash().to_string(),
        meshes: object
            .meshes()
            .iter()
            .map(|mesh| VoxelObjectRenderMesh {
                payload: voxel_object_mesh_payload(mesh, texture_space),
            })
            .collect(),
        frames: object
            .frames()
            .iter()
            .map(|frame| VoxelObjectRenderFrame {
                id: frame_id(frame),
                mesh: frame.mesh_index,
            })
            .collect(),
        material_slots: object
            .source()
            .material_palette
            .iter()
            .map(|binding| MeshMaterialSlot {
                slot: binding.material_slot,
                material: binding.material_asset_id.clone(),
            })
            .collect(),
    }
}

/// Produces the same renderer-neutral voxel-object descriptor as the inline
/// path while moving its mesh streams into deterministic resource bytes.
pub fn voxel_object_packed_render_asset(
    object: &AdmittedVoxelObject,
    maximum_resource_bytes: u32,
) -> Result<(VoxelObjectRenderAsset, Vec<PackedMeshResource>), MeshResourceError> {
    voxel_object_packed_render_asset_as(object, object.asset_id(), maximum_resource_bytes)
}

fn voxel_object_packed_render_asset_as(
    object: &AdmittedVoxelObject,
    render_asset_id: &str,
    maximum_resource_bytes: u32,
) -> Result<(VoxelObjectRenderAsset, Vec<PackedMeshResource>), MeshResourceError> {
    let mut asset = voxel_object_render_asset_as(object, render_asset_id);
    let packed = pack_mesh_resources(
        &asset
            .meshes
            .iter()
            .map(|mesh| mesh.payload.clone())
            .collect::<Vec<_>>(),
        maximum_resource_bytes,
    )?;
    for (mesh, payload) in asset.meshes.iter_mut().zip(packed.payloads) {
        mesh.payload = payload;
    }
    Ok((asset, packed.resources))
}

fn projection_asset_id(
    object: &AdmittedVoxelObject,
    modes_by_asset: &BTreeMap<&str, BTreeSet<SurfaceMode>>,
) -> String {
    if modes_by_asset
        .get(object.asset_id())
        .is_none_or(|modes| modes.len() == 1)
    {
        object.asset_id().to_string()
    } else {
        surface_mode_asset_id(object)
    }
}

/// The resource id of an object whose instances draw several surface modes.
fn surface_mode_asset_id(object: &AdmittedVoxelObject) -> String {
    format!(
        "{}#surface={}",
        object.asset_id(),
        object.surface_mode().as_str()
    )
}

/// `texture_space` is the object's local cells, which its tile coordinates
/// are in: its grid's cell size and pivot.
pub fn voxel_object_mesh_payload(
    mesh: &svc_mesh::MeshPayload,
    texture_space: MeshTextureSpace,
) -> MeshPayloadDescriptor {
    let mut attributes = vec![
        MeshAttribute {
            name: MeshAttributeName::Position,
            components: 3,
            kind: MeshAttributeKind::F32,
        },
        MeshAttribute {
            name: MeshAttributeName::Normal,
            components: 3,
            kind: MeshAttributeKind::F32,
        },
    ];
    let uvs = (!mesh.tile_coordinates.is_empty()).then(|| {
        attributes.push(MeshAttribute {
            name: MeshAttributeName::Uv,
            components: 2,
            kind: MeshAttributeKind::F32,
        });
        mesh.tile_coordinates.clone()
    });
    MeshPayloadDescriptor {
        layout: MeshBufferLayout {
            vertex_count: mesh.stats.vertices,
            index_count: mesh.stats.indices,
            index_width: MeshIndexWidth::U32,
            attributes,
        },
        groups: crate::voxel::draw_groups(mesh.groups.iter().map(|group| MeshGroupDescriptor {
            material_slot: group.material_slot,
            start: group.start,
            count: group.count,
        })),
        bounds: MeshBoundsDescriptor {
            min: mesh.bounds.min,
            max: mesh.bounds.max,
        },
        source: MeshPayloadSource::Inline {
            positions: mesh.positions.clone(),
            normals: mesh.normals.clone(),
            uvs,
            colors: None,
            indices: mesh.indices.clone(),
        },
        provenance: MeshProvenance::VoxelObject,
        layer_weights: false,
        layer_palette: Vec::new(),
        vertex_occlusion: false,
        texture_space: Some(texture_space),
        distance_field: None,
        morph: None,
    }
}

fn frame_id(frame: &VoxelObjectRuntimeFrame) -> String {
    match &frame.source {
        VoxelObjectFrameSource::Default => "default".to_string(),
        VoxelObjectFrameSource::Clip { clip, frame } => format!("{clip}/{frame}"),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct VoxelObjectProjectionResult {
    pub frame: RenderFrameDiff,
    /// Owner-neutral bytes that the caller may publish through its chosen
    /// resource policy. Empty for inline projections and cached resources.
    pub mesh_resources: Vec<PackedMeshResource>,
    pub readout: VoxelObjectProjectionReadout,
}

/// What one instance's change publishes.
#[derive(Debug, Clone, PartialEq)]
pub struct VoxelObjectInstanceProjection {
    pub frame: RenderFrameDiff,
    /// As in [`VoxelObjectProjectionResult::mesh_resources`].
    pub mesh_resources: Vec<PackedMeshResource>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoxelObjectProjectionReadout {
    pub instance_frames: BTreeMap<String, u32>,
    pub resource_hashes: BTreeMap<String, String>,
    pub surface_modes: BTreeMap<String, SurfaceMode>,
    /// Sorted asset identities whose complete geometry was built for this frame.
    pub materialized_resources: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VoxelObjectProjectionError {
    EmptyInstanceId,
    FrameOutOfRange {
        instance: String,
        frame: u32,
        frame_count: u32,
    },
    MeshResource(MeshResourceError),
    ConflictingResource {
        asset: String,
    },
    /// One instance's change would move the other instances of an object
    /// between one shared resource and one resource per surface mode.
    SurfaceModeRegrouping {
        asset: String,
    },
    MissingMaterial {
        asset: String,
    },
    UnboundMaterialOverride {
        instance: String,
        slot: u16,
    },
    InvalidMesh(MeshDescriptorError),
    Handle(HandleAllocationError),
}
