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
    last_resources: BTreeMap<String, VoxelObjectRenderAsset>,
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
            last_resources: BTreeMap::new(),
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
        let current_resources = validated.resources;
        let used_materials = validated.materials;
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

        for (asset_id, resource) in &current_resources {
            if self.last_resources.get(asset_id) != Some(resource) {
                operations.push(RenderDiff::DefineVoxelObject {
                    asset: resource.clone(),
                });
            }
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

        for asset in self.last_resources.keys() {
            if !current_resources.contains_key(asset) {
                operations.push(RenderDiff::ReleaseVoxelObject {
                    asset: asset.clone(),
                });
            }
        }

        let frame =
            RenderFrameDiff::try_from_ops(operations).map_err(VoxelObjectProjectionError::Frame)?;
        self.registry = registry;
        self.last_instances = current_instances;
        self.last_resources = current_resources;
        self.last_materials = used_materials;

        Ok(VoxelObjectProjectionResult {
            frame,
            readout: VoxelObjectProjectionReadout {
                instance_frames: self
                    .last_instances
                    .iter()
                    .map(|(id, snapshot)| (id.clone(), snapshot.frame))
                    .collect(),
                resource_hashes: self
                    .last_resources
                    .iter()
                    .map(|(id, resource)| (id.clone(), resource.content_hash.clone()))
                    .collect(),
            },
        })
    }

    pub fn handle(&self, instance_id: &str) -> Option<RenderHandle> {
        self.registry.handle_of(&instance_id.to_string())
    }
}

struct ValidatedProjection {
    instances: BTreeMap<String, InstanceSnapshot>,
    resources: BTreeMap<String, VoxelObjectRenderAsset>,
    materials: BTreeMap<String, RenderMaterialDescriptor>,
}

fn validate_and_snapshot(
    instances: &[VoxelObjectProjectionInstance<'_>],
    materials: &BTreeMap<String, RenderMaterialDescriptor>,
) -> Result<ValidatedProjection, VoxelObjectProjectionError> {
    let mut snapshots = BTreeMap::new();
    let mut resources = BTreeMap::new();
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
        let resource = voxel_object_render_asset(instance.object);
        resource
            .validate()
            .map_err(VoxelObjectProjectionError::InvalidResource)?;
        if let Some(existing) = resources.get(resource.asset.as_str()) {
            if existing != &resource {
                return Err(VoxelObjectProjectionError::ConflictingResource {
                    asset: resource.asset,
                });
            }
        } else {
            resources.insert(resource.asset.clone(), resource);
        }

        let bound_slots = instance
            .object
            .source()
            .material_palette
            .iter()
            .map(|binding| binding.material_slot)
            .collect::<BTreeSet<_>>();
        for binding in &instance.object.source().material_palette {
            let material = materials.get(&binding.material_asset_id).ok_or_else(|| {
                VoxelObjectProjectionError::MissingMaterial {
                    asset: binding.material_asset_id.clone(),
                }
            })?;
            material
                .validate()
                .map_err(|source| VoxelObjectProjectionError::InvalidMaterial {
                    asset: binding.material_asset_id.clone(),
                    source,
                })?;
            if material.id != binding.material_asset_id {
                return Err(VoxelObjectProjectionError::MaterialIdMismatch {
                    expected: binding.material_asset_id.clone(),
                    actual: material.id.clone(),
                });
            }
            used_materials.insert(material.id.clone(), material.clone());
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
