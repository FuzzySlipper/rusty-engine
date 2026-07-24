use std::collections::{BTreeMap, BTreeSet};

use engine_spatial::{VoxelCollisionScene, VoxelMeshChunk};
use render_model::{
    Geometry, Material, MaterialDescriptorError, MeshAttribute, MeshAttributeKind,
    MeshAttributeName, MeshBoundsDescriptor, MeshBufferLayout, MeshDescriptorError,
    MeshGroupDescriptor, MeshIndexWidth, MeshPayloadDescriptor, MeshPayloadSource, MeshProvenance,
    RenderDiff, RenderFrameDiff, RenderFrameError, RenderHandle, RenderLayer,
    RenderMaterialDescriptor, RenderMetadata, RenderNode, Transform, TransformError,
};

use crate::{HandleAllocationError, RenderHandleNamespace, StableHandleRegistry};

#[derive(Debug)]
pub struct VoxelProjectionInstance<'a> {
    pub instance_id: String,
    pub asset_id: String,
    pub transform: Transform,
    pub scene: &'a VoxelCollisionScene,
}

#[derive(Debug, Clone, PartialEq)]
struct ChunkSnapshot {
    content_hash: u64,
    translation: [f32; 3],
}

#[derive(Debug, Clone, PartialEq)]
struct InstanceSnapshot {
    asset_id: String,
    transform: Transform,
    source_revision: u64,
    chunks: BTreeMap<[i64; 3], ChunkSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum VoxelRenderKey {
    Root(String),
    Chunk { instance: String, chunk: [i64; 3] },
}

#[derive(Debug, Clone)]
pub struct VoxelRenderProjector {
    registry: StableHandleRegistry<VoxelRenderKey>,
    last_instances: BTreeMap<String, InstanceSnapshot>,
    last_materials: BTreeMap<u16, RenderMaterialDescriptor>,
}

impl Default for VoxelRenderProjector {
    fn default() -> Self {
        Self::new()
    }
}

impl VoxelRenderProjector {
    pub fn new() -> Self {
        Self {
            registry: StableHandleRegistry::new(RenderHandleNamespace::VOXEL),
            last_instances: BTreeMap::new(),
            last_materials: BTreeMap::new(),
        }
    }

    pub fn project(
        &mut self,
        instances: &[VoxelProjectionInstance<'_>],
        materials: &BTreeMap<u16, RenderMaterialDescriptor>,
    ) -> Result<VoxelProjectionResult, VoxelProjectionError> {
        let current = validate_and_snapshot(instances, materials)?;
        let mut registry = self.registry.clone();
        let mut operations = Vec::new();

        for (slot, material) in materials {
            if self.last_materials.get(slot) != Some(material) {
                operations.push(RenderDiff::DefineMaterial {
                    material: material.clone(),
                });
            }
        }

        for (instance_id, previous) in &self.last_instances {
            let rebound = current
                .get(instance_id)
                .is_some_and(|next| next.asset_id != previous.asset_id);
            if !current.contains_key(instance_id) || rebound {
                retire_instance(&mut registry, instance_id, previous);
                let root = VoxelRenderKey::Root(instance_id.clone());
                let handle = registry
                    .remove(&root)
                    .expect("retained voxel root has a render handle");
                operations.push(RenderDiff::Destroy { handle });
            }
        }

        for instance in instances_by_id(instances) {
            let snapshot = &current[&instance.instance_id];
            let previous = self.last_instances.get(&instance.instance_id);
            let was_rebound = previous.is_some_and(|value| value.asset_id != snapshot.asset_id);
            let root_key = VoxelRenderKey::Root(instance.instance_id.clone());
            let root_handle = if previous.is_none() || was_rebound {
                let handle = registry
                    .allocate(root_key)
                    .map_err(VoxelProjectionError::Handle)?;
                operations.push(RenderDiff::Create {
                    handle,
                    parent: None,
                    node: root_node(instance),
                });
                handle
            } else {
                let handle = registry
                    .handle_of(&root_key)
                    .expect("retained voxel root has a render handle");
                if previous.is_some_and(|value| value.transform != snapshot.transform) {
                    operations.push(RenderDiff::Update {
                        handle,
                        transform: Some(snapshot.transform),
                        material: None,
                        visible: None,
                        metadata: None,
                    });
                }
                handle
            };

            let previous_chunks = if was_rebound {
                None
            } else {
                previous.map(|value| &value.chunks)
            };
            if let Some(previous_chunks) = previous_chunks {
                for coord in previous_chunks.keys() {
                    if !snapshot.chunks.contains_key(coord) {
                        let key = VoxelRenderKey::Chunk {
                            instance: instance.instance_id.clone(),
                            chunk: *coord,
                        };
                        let handle = registry
                            .remove(&key)
                            .expect("retained voxel chunk has a render handle");
                        operations.push(RenderDiff::Destroy { handle });
                    }
                }
            }

            let chunks: BTreeMap<[i64; 3], &VoxelMeshChunk> = instance
                .scene
                .mesh_chunks()
                .iter()
                .map(|chunk| (chunk.chunk, chunk))
                .collect();
            for (coord, chunk) in chunks {
                let key = VoxelRenderKey::Chunk {
                    instance: instance.instance_id.clone(),
                    chunk: coord,
                };
                let previous_chunk = previous_chunks.and_then(|values| values.get(&coord));
                let handle = if let Some(handle) = registry.handle_of(&key) {
                    handle
                } else {
                    let handle = registry
                        .allocate(key)
                        .map_err(VoxelProjectionError::Handle)?;
                    operations.push(RenderDiff::Create {
                        handle,
                        parent: Some(root_handle),
                        node: chunk_node(&instance.instance_id, chunk),
                    });
                    handle
                };
                if previous_chunk.is_none_or(|value| value.content_hash != chunk.content_hash) {
                    operations.push(RenderDiff::ReplaceMeshPayload {
                        handle,
                        payload: voxel_mesh_payload(chunk),
                    });
                } else if previous_chunk.is_some_and(|value| value.translation != chunk.translation)
                {
                    operations.push(RenderDiff::Update {
                        handle,
                        transform: Some(Transform {
                            translation: chunk.translation,
                            ..Transform::IDENTITY
                        }),
                        material: None,
                        visible: None,
                        metadata: None,
                    });
                }
            }
        }

        let frame =
            RenderFrameDiff::try_from_ops(operations).map_err(VoxelProjectionError::Frame)?;
        self.registry = registry;
        self.last_instances = current;
        self.last_materials = materials.clone();
        let source_revisions = self
            .last_instances
            .iter()
            .map(|(id, value)| (id.clone(), value.source_revision))
            .collect();
        Ok(VoxelProjectionResult {
            readout: VoxelProjectionReadout {
                instance_count: self.last_instances.len(),
                chunk_count: self
                    .last_instances
                    .values()
                    .map(|value| value.chunks.len())
                    .sum(),
                source_revisions,
            },
            frame,
        })
    }

    pub fn root_handle(&self, instance_id: &str) -> Option<RenderHandle> {
        self.registry
            .handle_of(&VoxelRenderKey::Root(instance_id.to_string()))
    }

    pub fn chunk_handle(&self, instance_id: &str, chunk: [i64; 3]) -> Option<RenderHandle> {
        self.registry.handle_of(&VoxelRenderKey::Chunk {
            instance: instance_id.to_string(),
            chunk,
        })
    }
}

fn validate_and_snapshot(
    instances: &[VoxelProjectionInstance<'_>],
    materials: &BTreeMap<u16, RenderMaterialDescriptor>,
) -> Result<BTreeMap<String, InstanceSnapshot>, VoxelProjectionError> {
    for (slot, material) in materials {
        material
            .validate()
            .map_err(|source| VoxelProjectionError::InvalidMaterial {
                slot: *slot,
                source,
            })?;
        let expected = voxel_material_id(*slot);
        if material.id != expected {
            return Err(VoxelProjectionError::MaterialIdMismatch {
                slot: *slot,
                expected,
                actual: material.id.clone(),
            });
        }
    }

    let mut current = BTreeMap::new();
    for instance in instances {
        if instance.instance_id.trim().is_empty() {
            return Err(VoxelProjectionError::EmptyInstanceId);
        }
        if instance.asset_id.trim().is_empty() {
            return Err(VoxelProjectionError::EmptyAssetId {
                instance: instance.instance_id.clone(),
            });
        }
        instance
            .transform
            .validate()
            .map_err(|source| VoxelProjectionError::InvalidTransform {
                instance: instance.instance_id.clone(),
                source,
            })?;
        if current.contains_key(&instance.instance_id) {
            return Err(VoxelProjectionError::DuplicateInstanceId {
                instance: instance.instance_id.clone(),
            });
        }
        let mut chunks = BTreeMap::new();
        let mut used_slots = BTreeSet::new();
        for chunk in instance.scene.mesh_chunks() {
            let payload = voxel_mesh_payload(chunk);
            payload
                .validate()
                .map_err(|source| VoxelProjectionError::InvalidMesh {
                    instance: instance.instance_id.clone(),
                    chunk: chunk.chunk,
                    source,
                })?;
            used_slots.extend(chunk.groups.iter().map(|group| group.material_slot));
            chunks.insert(
                chunk.chunk,
                ChunkSnapshot {
                    content_hash: chunk.content_hash,
                    translation: chunk.translation,
                },
            );
        }
        if let Some(slot) = used_slots
            .into_iter()
            .find(|slot| !materials.contains_key(slot))
        {
            return Err(VoxelProjectionError::MissingMaterial {
                instance: instance.instance_id.clone(),
                slot,
            });
        }
        current.insert(
            instance.instance_id.clone(),
            InstanceSnapshot {
                asset_id: instance.asset_id.clone(),
                transform: instance.transform,
                source_revision: instance.scene.source_revision().raw(),
                chunks,
            },
        );
    }
    Ok(current)
}

fn instances_by_id<'a>(
    instances: &'a [VoxelProjectionInstance<'a>],
) -> Vec<&'a VoxelProjectionInstance<'a>> {
    let mut values: Vec<_> = instances.iter().collect();
    values.sort_by(|left, right| left.instance_id.cmp(&right.instance_id));
    values
}

fn retire_instance(
    registry: &mut StableHandleRegistry<VoxelRenderKey>,
    instance_id: &str,
    snapshot: &InstanceSnapshot,
) {
    for chunk in snapshot.chunks.keys() {
        registry.remove(&VoxelRenderKey::Chunk {
            instance: instance_id.to_string(),
            chunk: *chunk,
        });
    }
}

fn root_node(instance: &VoxelProjectionInstance<'_>) -> RenderNode {
    RenderNode {
        geometry: Geometry::Group,
        material: Material::DEFAULT,
        transform: instance.transform,
        visible: true,
        layer: RenderLayer::Scene,
        metadata: RenderMetadata {
            source_entity: None,
            source_scene_node: None,
            tags: vec!["voxel-instance".to_string()],
            label: Some(instance.instance_id.clone()),
        },
    }
}

fn chunk_node(instance_id: &str, chunk: &VoxelMeshChunk) -> RenderNode {
    RenderNode {
        geometry: Geometry::Cube,
        material: Material::DEFAULT,
        transform: Transform {
            translation: chunk.translation,
            ..Transform::IDENTITY
        },
        visible: true,
        layer: RenderLayer::Scene,
        metadata: RenderMetadata {
            source_entity: None,
            source_scene_node: None,
            tags: vec!["voxel-chunk".to_string()],
            label: Some(format!(
                "{instance_id} [{}, {}, {}]",
                chunk.chunk[0], chunk.chunk[1], chunk.chunk[2]
            )),
        },
    }
}

pub fn voxel_mesh_payload(chunk: &VoxelMeshChunk) -> MeshPayloadDescriptor {
    MeshPayloadDescriptor {
        layout: MeshBufferLayout {
            vertex_count: chunk.vertices,
            index_count: chunk.indices.len() as u32,
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
        groups: chunk
            .groups
            .iter()
            .map(|group| MeshGroupDescriptor {
                material_slot: group.material_slot,
                start: group.start,
                count: group.count,
            })
            .collect(),
        bounds: MeshBoundsDescriptor {
            min: chunk.bounds_min,
            max: chunk.bounds_max,
        },
        source: MeshPayloadSource::Inline {
            positions: chunk.positions.clone(),
            normals: chunk.normals.clone(),
            indices: chunk.indices.clone(),
        },
        provenance: MeshProvenance::VoxelChunk,
    }
}

pub fn voxel_material_id(slot: u16) -> String {
    format!("voxel-material/{slot}")
}

#[derive(Debug, Clone, PartialEq)]
pub struct VoxelProjectionResult {
    pub frame: RenderFrameDiff,
    pub readout: VoxelProjectionReadout,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoxelProjectionReadout {
    pub instance_count: usize,
    pub chunk_count: usize,
    pub source_revisions: BTreeMap<String, u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VoxelProjectionError {
    EmptyInstanceId,
    EmptyAssetId {
        instance: String,
    },
    DuplicateInstanceId {
        instance: String,
    },
    InvalidTransform {
        instance: String,
        source: TransformError,
    },
    InvalidMaterial {
        slot: u16,
        source: MaterialDescriptorError,
    },
    MaterialIdMismatch {
        slot: u16,
        expected: String,
        actual: String,
    },
    MissingMaterial {
        instance: String,
        slot: u16,
    },
    InvalidMesh {
        instance: String,
        chunk: [i64; 3],
        source: MeshDescriptorError,
    },
    Handle(HandleAllocationError),
    Frame(RenderFrameError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_spatial::MaterialVoxel;
    use render_model::MaterialUvStrategy;

    fn material(slot: u16) -> RenderMaterialDescriptor {
        RenderMaterialDescriptor {
            schema_version: 2,
            id: voxel_material_id(slot),
            color: [0.4, 0.5, 0.6, 1.0],
            texture: None,
            roughness: 1.0,
            texture_tint: [1.0; 4],
            emission_color: [0.0; 3],
            emission_intensity: 0.0,
            uv_strategy: MaterialUvStrategy::Flat,
        }
    }

    #[test]
    fn projects_engine_spatial_chunks_with_stable_handles() {
        let scene = VoxelCollisionScene::from_material_voxels(
            1.0,
            16,
            [MaterialVoxel {
                address: [0, 0, 0],
                material_slot: 1,
            }],
        )
        .unwrap();
        let instances = [VoxelProjectionInstance {
            instance_id: "room".to_string(),
            asset_id: "voxel-object/room".to_string(),
            transform: Transform::IDENTITY,
            scene: &scene,
        }];
        let materials = BTreeMap::from([(1, material(1))]);
        let mut projector = VoxelRenderProjector::new();
        let first = projector.project(&instances, &materials).unwrap();
        let handle = projector.chunk_handle("room", [0, 0, 0]).unwrap();
        assert!(first.frame.ops.iter().any(|operation| matches!(
            operation,
            RenderDiff::ReplaceMeshPayload { handle: actual, .. } if *actual == handle
        )));

        let second = projector.project(&instances, &materials).unwrap();
        assert!(second.frame.is_empty());
        assert_eq!(projector.chunk_handle("room", [0, 0, 0]), Some(handle));
    }

    #[test]
    fn missing_material_rejects_without_projector_mutation() {
        let scene = VoxelCollisionScene::from_material_voxels(
            1.0,
            16,
            [MaterialVoxel {
                address: [0, 0, 0],
                material_slot: 7,
            }],
        )
        .unwrap();
        let instances = [VoxelProjectionInstance {
            instance_id: "room".to_string(),
            asset_id: "voxel-object/room".to_string(),
            transform: Transform::IDENTITY,
            scene: &scene,
        }];
        let mut projector = VoxelRenderProjector::new();
        assert!(matches!(
            projector.project(&instances, &BTreeMap::new()),
            Err(VoxelProjectionError::MissingMaterial { slot: 7, .. })
        ));
        assert_eq!(projector.root_handle("room"), None);
    }
}
