use std::collections::{BTreeMap, BTreeSet};

use render_model::{
    MaterialDescriptorError, MeshAttribute, MeshAttributeKind, MeshAttributeName,
    MeshBoundsDescriptor, MeshBufferLayout, MeshDescriptorError, MeshGroupDescriptor,
    MeshIndexWidth, MeshMaterialSlot, MeshPayloadDescriptor, MeshPayloadSource, MeshProvenance,
    RenderDiff, RenderFrameDiff, RenderFrameError, RenderHandle, RenderMaterialDescriptor,
    RenderMetadata, Transform, TransformError, VoxelObjectInstanceDescriptor,
    VoxelObjectRenderAsset, VoxelObjectRenderAssetError, VoxelObjectRenderFrame,
    VoxelObjectRenderMesh,
};
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
    asset: String,
    content_hash: String,
    frame: u32,
    transform: Transform,
    visible: bool,
    material_overrides: Vec<MeshMaterialSlot>,
    metadata: RenderMetadata,
}

#[derive(Debug, Clone)]
pub struct VoxelObjectRenderProjector {
    registry: StableHandleRegistry<String>,
    last_instances: BTreeMap<String, InstanceSnapshot>,
    last_resource_hashes: BTreeMap<String, String>,
    last_materials: BTreeMap<String, RenderMaterialDescriptor>,
}

impl Default for VoxelObjectRenderProjector {
    fn default() -> Self {
        Self::new()
    }
}

impl VoxelObjectRenderProjector {
    pub fn new() -> Self {
        Self {
            registry: StableHandleRegistry::new(RenderHandleNamespace::VOXEL_OBJECT),
            last_instances: BTreeMap::new(),
            last_resource_hashes: BTreeMap::new(),
            last_materials: BTreeMap::new(),
        }
    }

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
        let mut pending_resources = BTreeMap::new();
        for (asset_id, request) in &requested_resources {
            let is_cached = self
                .last_resource_hashes
                .get(asset_id)
                .is_some_and(|content_hash| content_hash == request.object.content_hash());
            if is_cached {
                continue;
            }
            let resource = voxel_object_render_asset(request.object);
            resource
                .validate()
                .map_err(VoxelObjectProjectionError::InvalidResource)?;
            pending_resources.insert(asset_id.clone(), resource);
        }
        let materialized_resources = pending_resources.keys().cloned().collect::<Vec<_>>();
        let mut registry = self.registry.clone();
        let mut operations = Vec::new();

        for (id, material) in &used_materials {
            if self.last_materials.get(id) != Some(material) {
                operations.push(RenderDiff::DefineMaterial {
                    material: material.clone(),
                });
            }
        }

        for (instance_id, previous) in &self.last_instances {
            let rebound = current_instances
                .get(instance_id)
                .is_some_and(|current| current.asset != previous.asset);
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

        for instance in sorted_instances(instances) {
            let snapshot = &current_instances[&instance.instance_id];
            let previous = self.last_instances.get(&instance.instance_id);
            let rebound = previous.is_some_and(|value| value.asset != snapshot.asset);
            if previous.is_none() || rebound {
                let handle = registry
                    .allocate(instance.instance_id.clone())
                    .map_err(VoxelObjectProjectionError::Handle)?;
                operations.push(RenderDiff::CreateVoxelObjectInstance {
                    handle,
                    parent: None,
                    instance: VoxelObjectInstanceDescriptor {
                        asset: snapshot.asset.clone(),
                        frame: snapshot.frame,
                        transform: snapshot.transform,
                        visible: snapshot.visible,
                        material_overrides: snapshot.material_overrides.clone(),
                        metadata: snapshot.metadata.clone(),
                    },
                });
                continue;
            }

            let previous = previous.expect("existing voxel-object snapshot");
            let handle = registry
                .handle_of(&instance.instance_id)
                .expect("retained voxel-object instance has a render handle");
            if previous.material_overrides != snapshot.material_overrides {
                return Err(VoxelObjectProjectionError::ChangedMaterialOverrides {
                    instance: instance.instance_id.clone(),
                });
            }
            if previous.transform != snapshot.transform
                || previous.visible != snapshot.visible
                || previous.metadata != snapshot.metadata
            {
                operations.push(RenderDiff::Update {
                    handle,
                    transform: (previous.transform != snapshot.transform)
                        .then_some(snapshot.transform),
                    material: None,
                    visible: (previous.visible != snapshot.visible).then_some(snapshot.visible),
                    metadata: (previous.metadata != snapshot.metadata)
                        .then(|| snapshot.metadata.clone()),
                });
            }
            if previous.frame != snapshot.frame || previous.content_hash != snapshot.content_hash {
                operations.push(RenderDiff::SetVoxelObjectFrame {
                    handle,
                    frame: snapshot.frame,
                });
            }
        }

        for asset in self.last_resource_hashes.keys() {
            if !requested_resources.contains_key(asset) {
                operations.push(RenderDiff::ReleaseVoxelObject {
                    asset: asset.clone(),
                });
            }
        }

        let frame =
            RenderFrameDiff::try_from_ops(operations).map_err(VoxelObjectProjectionError::Frame)?;
        self.registry = registry;
        self.last_instances = current_instances;
        self.last_resource_hashes = current_resource_hashes;
        self.last_materials = used_materials;

        Ok(VoxelObjectProjectionResult {
            frame,
            readout: VoxelObjectProjectionReadout {
                instance_frames: self
                    .last_instances
                    .iter()
                    .map(|(id, snapshot)| (id.clone(), snapshot.frame))
                    .collect(),
                resource_hashes: self.last_resource_hashes.clone(),
                materialized_resources,
            },
        })
    }

    pub fn handle(&self, instance_id: &str) -> Option<RenderHandle> {
        self.registry.handle_of(&instance_id.to_string())
    }
}

struct RequestedResource<'a> {
    object: &'a AdmittedVoxelObject,
}

struct ValidatedProjection<'a> {
    instances: BTreeMap<String, InstanceSnapshot>,
    resources: BTreeMap<String, RequestedResource<'a>>,
    materials: BTreeMap<String, RenderMaterialDescriptor>,
}

fn validate_and_snapshot<'a>(
    instances: &[VoxelObjectProjectionInstance<'a>],
    materials: &BTreeMap<String, RenderMaterialDescriptor>,
) -> Result<ValidatedProjection<'a>, VoxelObjectProjectionError> {
    let mut snapshots = BTreeMap::new();
    let mut resources = BTreeMap::<String, RequestedResource<'a>>::new();
    let mut used_materials = BTreeMap::new();
    for instance in instances {
        if instance.instance_id.trim().is_empty() {
            return Err(VoxelObjectProjectionError::EmptyInstanceId);
        }
        instance.transform.validate().map_err(|source| {
            VoxelObjectProjectionError::InvalidTransform {
                instance: instance.instance_id.clone(),
                source,
            }
        })?;
        instance.metadata.validate().map_err(|source| {
            VoxelObjectProjectionError::InvalidMetadata {
                instance: instance.instance_id.clone(),
                source,
            }
        })?;
        if instance.object.frame(instance.frame).is_none() {
            return Err(VoxelObjectProjectionError::FrameOutOfRange {
                instance: instance.instance_id.clone(),
                frame: instance.frame,
                frame_count: instance.object.frames().len() as u32,
            });
        }
        let asset_id = instance.object.asset_id();
        if let Some(existing) = resources.get(asset_id) {
            if existing.object.content_hash() != instance.object.content_hash() {
                return Err(VoxelObjectProjectionError::ConflictingResource {
                    asset: asset_id.to_string(),
                });
            }
        } else {
            resources.insert(
                asset_id.to_string(),
                RequestedResource {
                    object: instance.object,
                },
            );
        }

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
        if let Some(slot) = instance
            .material_overrides
            .iter()
            .map(|binding| binding.slot)
            .find(|slot| !bound_slots.contains(slot))
        {
            return Err(VoxelObjectProjectionError::UnboundMaterialOverride {
                instance: instance.instance_id.clone(),
                slot,
            });
        }
        for binding in &instance.material_overrides {
            collect_material(&binding.material, materials, &mut used_materials)?;
        }
        if snapshots
            .insert(
                instance.instance_id.clone(),
                InstanceSnapshot {
                    asset: instance.object.asset_id().to_string(),
                    content_hash: instance.object.content_hash().to_string(),
                    frame: instance.frame,
                    transform: instance.transform,
                    visible: instance.visible,
                    material_overrides: instance.material_overrides.clone(),
                    metadata: instance.metadata.clone(),
                },
            )
            .is_some()
        {
            return Err(VoxelObjectProjectionError::DuplicateInstanceId {
                instance: instance.instance_id.clone(),
            });
        }
    }
    Ok(ValidatedProjection {
        instances: snapshots,
        resources,
        materials: used_materials,
    })
}

fn collect_material(
    asset_id: &str,
    materials: &BTreeMap<String, RenderMaterialDescriptor>,
    used_materials: &mut BTreeMap<String, RenderMaterialDescriptor>,
) -> Result<(), VoxelObjectProjectionError> {
    let material =
        materials
            .get(asset_id)
            .ok_or_else(|| VoxelObjectProjectionError::MissingMaterial {
                asset: asset_id.to_string(),
            })?;
    material
        .validate()
        .map_err(|source| VoxelObjectProjectionError::InvalidMaterial {
            asset: asset_id.to_string(),
            source,
        })?;
    if material.id != asset_id {
        return Err(VoxelObjectProjectionError::MaterialIdMismatch {
            expected: asset_id.to_string(),
            actual: material.id.clone(),
        });
    }
    used_materials.insert(material.id.clone(), material.clone());
    Ok(())
}

fn sorted_instances<'a>(
    instances: &'a [VoxelObjectProjectionInstance<'a>],
) -> Vec<&'a VoxelObjectProjectionInstance<'a>> {
    let mut sorted = instances.iter().collect::<Vec<_>>();
    sorted.sort_by(|left, right| left.instance_id.cmp(&right.instance_id));
    sorted
}

pub fn voxel_object_render_asset(object: &AdmittedVoxelObject) -> VoxelObjectRenderAsset {
    VoxelObjectRenderAsset {
        asset: object.asset_id().to_string(),
        content_hash: object.content_hash().to_string(),
        meshes: object
            .meshes()
            .iter()
            .map(|mesh| VoxelObjectRenderMesh {
                payload: voxel_object_mesh_payload(mesh),
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

pub fn voxel_object_mesh_payload(mesh: &svc_mesh::MeshPayload) -> MeshPayloadDescriptor {
    MeshPayloadDescriptor {
        layout: MeshBufferLayout {
            vertex_count: mesh.stats.vertices,
            index_count: mesh.stats.indices,
            index_width: MeshIndexWidth::U32,
            attributes: vec![
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
            ],
        },
        groups: mesh
            .groups
            .iter()
            .map(|group| MeshGroupDescriptor {
                material_slot: group.material_slot,
                start: group.start,
                count: group.count,
            })
            .collect(),
        bounds: MeshBoundsDescriptor {
            min: mesh.bounds.min,
            max: mesh.bounds.max,
        },
        source: MeshPayloadSource::Inline {
            positions: mesh.positions.clone(),
            normals: mesh.normals.clone(),
            indices: mesh.indices.clone(),
        },
        provenance: MeshProvenance::VoxelObject,
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
    pub readout: VoxelObjectProjectionReadout,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoxelObjectProjectionReadout {
    pub instance_frames: BTreeMap<String, u32>,
    pub resource_hashes: BTreeMap<String, String>,
    /// Sorted asset identities whose complete geometry was built for this frame.
    pub materialized_resources: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VoxelObjectProjectionError {
    EmptyInstanceId,
    DuplicateInstanceId {
        instance: String,
    },
    InvalidTransform {
        instance: String,
        source: TransformError,
    },
    InvalidMetadata {
        instance: String,
        source: render_model::NodeError,
    },
    FrameOutOfRange {
        instance: String,
        frame: u32,
        frame_count: u32,
    },
    InvalidResource(VoxelObjectRenderAssetError),
    ConflictingResource {
        asset: String,
    },
    MissingMaterial {
        asset: String,
    },
    InvalidMaterial {
        asset: String,
        source: MaterialDescriptorError,
    },
    MaterialIdMismatch {
        expected: String,
        actual: String,
    },
    UnboundMaterialOverride {
        instance: String,
        slot: u16,
    },
    ChangedMaterialOverrides {
        instance: String,
    },
    InvalidMesh(MeshDescriptorError),
    Handle(HandleAllocationError),
    Frame(RenderFrameError),
}
