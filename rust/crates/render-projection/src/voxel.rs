use std::collections::{BTreeMap, BTreeSet};

use core_space::Direction6;
use engine_spatial::{SurfaceMode, VoxelCollisionScene, VoxelMeshChunk};
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

/// Internal renderer realization for a canonical voxel scene. Base mappings
/// apply to every group, including directionless reconstructed groups; sparse
/// directional entries refine only greedy cube face groups.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VoxelMaterialSlotMapping {
    pub base: BTreeMap<u16, u16>,
    pub directional: BTreeMap<(u16, Direction6), u16>,
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
    rebase_revision: u64,
    material_slots: VoxelMaterialSlotMapping,
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
    publication_stream: Option<String>,
    publication_revision: u64,
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
            publication_stream: None,
            publication_revision: 0,
        }
    }

    pub fn with_publication_stream(stream: impl Into<String>) -> Self {
        Self {
            publication_stream: Some(stream.into()),
            ..Self::new()
        }
    }

    pub fn project(
        &mut self,
        instances: &[VoxelProjectionInstance<'_>],
        materials: &BTreeMap<u16, RenderMaterialDescriptor>,
    ) -> Result<VoxelProjectionResult, VoxelProjectionError> {
        self.project_mapped(instances, materials, &BTreeMap::new())
    }

    /// Projects voxel instances whose source material slots are translated to
    /// retained renderer slots before mesh payloads are published. The map is
    /// internal renderer realization: callers still own source scene slots,
    /// while `materials` is keyed by the effective renderer slot.
    pub fn project_mapped(
        &mut self,
        instances: &[VoxelProjectionInstance<'_>],
        materials: &BTreeMap<u16, RenderMaterialDescriptor>,
        material_slots: &BTreeMap<String, BTreeMap<u16, u16>>,
    ) -> Result<VoxelProjectionResult, VoxelProjectionError> {
        let mapped = material_slots
            .iter()
            .map(|(instance, base)| {
                (
                    instance.clone(),
                    VoxelMaterialSlotMapping {
                        base: base.clone(),
                        directional: BTreeMap::new(),
                    },
                )
            })
            .collect();
        self.project_mapped_directional(instances, materials, &mapped)
    }

    /// Directional material realization for canonical greedy cube groups.
    /// The public base-slot method remains compatible by retaining a separate
    /// base map and supplying no directional overrides.
    pub fn project_mapped_directional(
        &mut self,
        instances: &[VoxelProjectionInstance<'_>],
        materials: &BTreeMap<u16, RenderMaterialDescriptor>,
        material_slots: &BTreeMap<String, VoxelMaterialSlotMapping>,
    ) -> Result<VoxelProjectionResult, VoxelProjectionError> {
        let current = validate_and_snapshot(instances, materials, material_slots)?;
        for (instance, next) in &current {
            let Some(previous) = self.last_instances.get(instance) else {
                continue;
            };
            if next.asset_id != previous.asset_id {
                continue;
            }
            if next.rebase_revision < previous.rebase_revision {
                return Err(VoxelProjectionError::StaleRebaseRevision {
                    instance: instance.clone(),
                    previous: previous.rebase_revision,
                    candidate: next.rebase_revision,
                });
            }
            if next.source_revision < previous.source_revision
                || (next.source_revision == previous.source_revision
                    && next.rebase_revision == previous.rebase_revision
                    && (next.asset_id != previous.asset_id || next.chunks != previous.chunks))
            {
                return Err(VoxelProjectionError::StaleSourceRevision {
                    instance: instance.clone(),
                    previous: previous.source_revision,
                    candidate: next.source_revision,
                });
            }
        }
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
                if previous_chunk.is_none_or(|value| value.content_hash != chunk.content_hash)
                    || previous.is_some_and(|value| value.material_slots != snapshot.material_slots)
                {
                    operations.push(RenderDiff::ReplaceMeshPayload {
                        handle,
                        payload: voxel_mesh_payload_with_material_slots(
                            chunk,
                            &snapshot.material_slots,
                        ),
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

        let stream = self
            .publication_stream
            .clone()
            .unwrap_or_else(|| voxel_publication_stream(current.keys()));
        let publication_revision = self
            .publication_revision
            .checked_add(1)
            .ok_or(VoxelProjectionError::PublicationRevisionExhausted)?;
        let frame = RenderFrameDiff::try_from_published_ops(
            stream.clone(),
            self.publication_revision,
            publication_revision,
            operations,
        )
        .map_err(VoxelProjectionError::Frame)?;
        self.registry = registry;
        self.last_instances = current;
        self.last_materials = materials.clone();
        self.publication_stream = Some(stream);
        self.publication_revision = publication_revision;
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

    /// The active consumer's continuation point. Attachment projections use a
    /// detached projector with a fresh frame revision, so callers recovering a
    /// renderer must obtain this from the committed projector instead.
    pub fn publication_frontier(&self) -> Option<(&str, u64)> {
        self.publication_stream
            .as_deref()
            .map(|stream| (stream, self.publication_revision))
    }
}

fn voxel_publication_stream<'a>(instances: impl Iterator<Item = &'a String>) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let mut empty = true;
    for instance in instances {
        empty = false;
        for byte in instance.as_bytes().iter().copied().chain([0xff]) {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    if empty {
        "voxel:default".to_string()
    } else {
        format!("voxel:{hash:016x}")
    }
}

fn validate_and_snapshot(
    instances: &[VoxelProjectionInstance<'_>],
    materials: &BTreeMap<u16, RenderMaterialDescriptor>,
    material_slots: &BTreeMap<String, VoxelMaterialSlotMapping>,
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
        let empty_mapping = VoxelMaterialSlotMapping::default();
        let slots = material_slots
            .get(&instance.instance_id)
            .unwrap_or(&empty_mapping);
        let map_slot = |slot, direction: Option<Direction6>| {
            direction
                .and_then(|direction| slots.directional.get(&(slot, direction)).copied())
                .or_else(|| slots.base.get(&slot).copied())
                .unwrap_or(slot)
        };
        let mut chunks = BTreeMap::new();
        let mut used_slots = BTreeSet::new();
        for chunk in instance.scene.mesh_chunks() {
            let payload = voxel_mesh_payload_with_material_slots(chunk, slots);
            payload
                .validate()
                .map_err(|source| VoxelProjectionError::InvalidMesh {
                    instance: instance.instance_id.clone(),
                    chunk: chunk.chunk,
                    source,
                })?;
            used_slots.extend(
                chunk
                    .groups
                    .iter()
                    .map(|group| map_slot(group.material_slot, group.direction)),
            );
            chunks.insert(
                chunk.chunk,
                ChunkSnapshot {
                    content_hash: chunk.content_hash,
                    translation: chunk.translation,
                },
            );
        }
        if let Some(slot) = used_slots.iter().find(|slot| !materials.contains_key(slot)) {
            return Err(VoxelProjectionError::MissingMaterial {
                instance: instance.instance_id.clone(),
                slot: *slot,
            });
        }
        if let Some((chunk, slot)) = instance.scene.mesh_chunks().iter().find_map(|chunk| {
            if chunk.surface_mode == SurfaceMode::GreedyCubes {
                return None;
            }
            chunk.groups.iter().find_map(|group| {
                let effective_slot = map_slot(group.material_slot, group.direction);
                materials
                    .get(&effective_slot)
                    .filter(|material| {
                        material.texture.is_some() || material.voxel_surface.is_some()
                    })
                    .map(|_| (chunk, effective_slot))
            })
        }) {
            return Err(VoxelProjectionError::TexturedReconstructedSurface {
                instance: instance.instance_id.clone(),
                chunk: chunk.chunk,
                slot,
                mode: chunk.surface_mode,
            });
        }
        current.insert(
            instance.instance_id.clone(),
            InstanceSnapshot {
                asset_id: instance.asset_id.clone(),
                transform: instance.transform,
                source_revision: instance.scene.source_revision().raw(),
                rebase_revision: instance.scene.rebase_revision(),
                material_slots: slots.clone(),
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
            tags: vec![
                format!("voxel-asset:{}", instance.asset_id),
                "voxel-instance".to_string(),
                format!("voxel-instance:{}", instance.instance_id),
            ],
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
            tags: vec![
                "voxel-chunk".to_string(),
                format!("voxel-instance:{instance_id}"),
            ],
            label: Some(format!(
                "{instance_id} [{}, {}, {}]",
                chunk.chunk[0], chunk.chunk[1], chunk.chunk[2]
            )),
        },
    }
}

pub fn voxel_mesh_payload(chunk: &VoxelMeshChunk) -> MeshPayloadDescriptor {
    voxel_mesh_payload_with_material_slots(chunk, &VoxelMaterialSlotMapping::default())
}

fn voxel_mesh_payload_with_material_slots(
    chunk: &VoxelMeshChunk,
    material_slots: &VoxelMaterialSlotMapping,
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
    if chunk.surface_mode.supports_voxel_tile_coordinates() {
        attributes.push(MeshAttribute {
            name: MeshAttributeName::Uv,
            components: 2,
            kind: MeshAttributeKind::F32,
        });
    }
    MeshPayloadDescriptor {
        layout: MeshBufferLayout {
            vertex_count: chunk.vertices,
            index_count: chunk.indices.len() as u32,
            index_width: MeshIndexWidth::U32,
            attributes,
        },
        groups: chunk
            .groups
            .iter()
            .map(|group| MeshGroupDescriptor {
                material_slot: group
                    .direction
                    .and_then(|direction| {
                        material_slots
                            .directional
                            .get(&(group.material_slot, direction))
                            .copied()
                    })
                    .or_else(|| material_slots.base.get(&group.material_slot).copied())
                    .unwrap_or(group.material_slot),
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
            uvs: chunk
                .surface_mode
                .supports_voxel_tile_coordinates()
                .then(|| chunk.tile_coordinates.clone()),
            colors: None,
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
    TexturedReconstructedSurface {
        instance: String,
        chunk: [i64; 3],
        slot: u16,
        mode: SurfaceMode,
    },
    InvalidMesh {
        instance: String,
        chunk: [i64; 3],
        source: MeshDescriptorError,
    },
    StaleSourceRevision {
        instance: String,
        previous: u64,
        candidate: u64,
    },
    StaleRebaseRevision {
        instance: String,
        previous: u64,
        candidate: u64,
    },
    PublicationRevisionExhausted,
    Handle(HandleAllocationError),
    Frame(RenderFrameError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_spatial::{
        MaterialVoxel, SurfaceMeshOptions, VoxelChunkIdentity, VoxelChunkLeaseRegistry,
        VoxelChunkPayload, VoxelChunkResidencyOperation, VoxelChunkResidencyService,
        VoxelChunkResidencyTransaction, VoxelEdit, VoxelEditService, VoxelEditTransaction,
        VoxelSourceRevision, WorldOrigin, WorldOriginRebaseRequest, WorldOriginRebaseService,
        WorldOriginState,
    };
    use entity_state::EntityState;
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
            alpha_mode: Default::default(),
            double_sided: false,
            voxel_surface: None,
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
        let root = first
            .frame
            .ops
            .iter()
            .find_map(|operation| match operation {
                RenderDiff::Create { node, .. }
                    if node.metadata.tags.contains(&"voxel-instance".to_string()) =>
                {
                    Some(node)
                }
                _ => None,
            })
            .expect("voxel projection creates an instance root");
        assert!(root
            .metadata
            .tags
            .contains(&"voxel-instance:room".to_string()));
        assert!(root
            .metadata
            .tags
            .contains(&"voxel-asset:voxel-object/room".to_string()));
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
    fn active_publication_frontier_is_not_a_detached_baseline_revision() {
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
        let mut active = VoxelRenderProjector::new();
        active.project(&instances, &materials).unwrap();
        active.project(&instances, &materials).unwrap();
        let mut detached = VoxelRenderProjector::new();
        let baseline = detached.project(&instances, &materials).unwrap();

        assert_eq!(
            active.publication_frontier().map(|(_, revision)| revision),
            Some(2)
        );
        assert_eq!(
            baseline
                .frame
                .publication
                .as_ref()
                .map(|value| value.revision),
            Some(1)
        );
    }

    #[test]
    fn mapped_slots_rewrite_mesh_groups_and_refresh_payloads_when_the_mapping_changes() {
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
        let mut projector = VoxelRenderProjector::new();
        let first = projector
            .project_mapped(
                &instances,
                &BTreeMap::from([(4, material(4))]),
                &BTreeMap::from([("room".to_string(), BTreeMap::from([(1, 4)]))]),
            )
            .unwrap();
        assert!(first.frame.ops.iter().any(|operation| matches!(
            operation,
            RenderDiff::ReplaceMeshPayload { payload, .. }
                if payload.groups.iter().all(|group| group.material_slot == 4)
        )));

        let remapped = projector
            .project_mapped(
                &instances,
                &BTreeMap::from([(5, material(5))]),
                &BTreeMap::from([("room".to_string(), BTreeMap::from([(1, 5)]))]),
            )
            .unwrap();
        assert!(remapped.frame.ops.iter().any(|operation| matches!(
            operation,
            RenderDiff::ReplaceMeshPayload { payload, .. }
                if payload.groups.iter().all(|group| group.material_slot == 5)
        )));
    }

    #[test]
    fn world_origin_rebase_updates_chunk_transforms_without_replacing_stable_handles() {
        let mut scene = VoxelCollisionScene::from_material_voxels(
            1.0,
            16,
            [MaterialVoxel {
                address: [100_000, 0, 0],
                material_slot: 1,
            }],
        )
        .unwrap();
        let mut origin = WorldOriginState::default();
        let mut entities = EntityState::default();
        let materials = BTreeMap::from([(1, material(1))]);
        let mut projector = VoxelRenderProjector::new();
        let project = |projector: &mut VoxelRenderProjector, scene: &VoxelCollisionScene| {
            projector
                .project(
                    &[VoxelProjectionInstance {
                        instance_id: "world".to_string(),
                        asset_id: "voxel-object/world".to_string(),
                        transform: Transform::IDENTITY,
                        scene,
                    }],
                    &materials,
                )
                .unwrap()
        };
        project(&mut projector, &scene);
        let root = projector.root_handle("world").unwrap();
        let chunk = projector.chunk_handle("world", [6_250, 0, 0]).unwrap();

        let request = WorldOriginRebaseRequest {
            expected_origin_revision: 0,
            expected_entity_revision: entities.revision(),
            expected_voxel_source_revision: scene.source_revision().raw(),
            expected_static_mesh_revision: scene.static_mesh_collision_revision(),
            target_origin: WorldOrigin::new([100_000, 0, 0]),
            entities: Vec::new(),
        };
        WorldOriginRebaseService
            .apply(&mut origin, &mut entities, &mut scene, request)
            .unwrap();
        let update = project(&mut projector, &scene);

        assert_eq!(projector.root_handle("world"), Some(root));
        assert_eq!(projector.chunk_handle("world", [6_250, 0, 0]), Some(chunk));
        assert!(update.frame.ops.iter().any(|operation| matches!(
            operation,
            RenderDiff::Update { handle, transform: Some(transform), .. }
                if *handle == chunk && transform.translation[0].abs() < 0.001
        )));
        assert!(!update.frame.ops.iter().any(|operation| matches!(
            operation,
            RenderDiff::Destroy { .. } | RenderDiff::ReplaceMeshPayload { .. }
        )));
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

    #[test]
    fn textured_reconstructed_surface_rejects_without_projector_mutation() {
        let scene = VoxelCollisionScene::from_material_voxels_with_mesh_options(
            1.0,
            16,
            [MaterialVoxel {
                address: [0, 0, 0],
                material_slot: 1,
            }],
            SurfaceMeshOptions {
                mode: SurfaceMode::MarchingCubes,
                ..SurfaceMeshOptions::default()
            },
        )
        .unwrap();
        let mut textured = material(1);
        textured.texture = Some("texture/voxel-atlas".to_string());
        let mut projector = VoxelRenderProjector::new();
        assert!(matches!(
            projector.project(
                &[VoxelProjectionInstance {
                    instance_id: "room".to_string(),
                    asset_id: "voxel-object/room".to_string(),
                    transform: Transform::IDENTITY,
                    scene: &scene,
                }],
                &BTreeMap::from([(1, textured)]),
            ),
            Err(VoxelProjectionError::TexturedReconstructedSurface {
                chunk: [0, 0, 0],
                slot: 1,
                mode: SurfaceMode::MarchingCubes,
                ..
            })
        ));
        assert_eq!(projector.root_handle("room"), None);
    }

    #[test]
    fn boundary_edit_replaces_neighbor_once_and_destroys_emptied_chunk() {
        let mut scene = VoxelCollisionScene::from_material_voxels(
            1.0,
            4,
            [
                MaterialVoxel {
                    address: [-1, 0, 0],
                    material_slot: 1,
                },
                MaterialVoxel {
                    address: [0, 0, 0],
                    material_slot: 1,
                },
                MaterialVoxel {
                    address: [8, 0, 0],
                    material_slot: 1,
                },
            ],
        )
        .unwrap();
        let materials = BTreeMap::from([(1, material(1))]);
        let mut projector = VoxelRenderProjector::new();
        let project = |projector: &mut VoxelRenderProjector, scene: &VoxelCollisionScene| {
            projector
                .project(
                    &[VoxelProjectionInstance {
                        instance_id: "room".to_string(),
                        asset_id: "voxel-object/room".to_string(),
                        transform: Transform::IDENTITY,
                        scene,
                    }],
                    &materials,
                )
                .unwrap()
        };
        project(&mut projector, &scene);
        let left = projector.chunk_handle("room", [-1, 0, 0]).unwrap();
        let removed = projector.chunk_handle("room", [0, 0, 0]).unwrap();
        let unchanged = projector.chunk_handle("room", [2, 0, 0]).unwrap();
        VoxelEditService::apply(
            &mut scene,
            VoxelEditTransaction {
                expected_revision: VoxelSourceRevision::INITIAL,
                edits: &[VoxelEdit::Clear { address: [0, 0, 0] }],
            },
        )
        .unwrap();
        let update = project(&mut projector, &scene);
        assert_eq!(
            update
                .frame
                .ops
                .iter()
                .filter(|operation| matches!(operation, RenderDiff::ReplaceMeshPayload { .. }))
                .count(),
            1
        );
        assert!(update.frame.ops.iter().any(|operation| matches!(
            operation,
            RenderDiff::ReplaceMeshPayload { handle, .. } if *handle == left
        )));
        assert!(update.frame.ops.iter().any(|operation| matches!(
            operation,
            RenderDiff::Destroy { handle } if *handle == removed
        )));
        assert!(!update.frame.ops.iter().any(|operation| matches!(
            operation,
            RenderDiff::ReplaceMeshPayload { handle, .. } if *handle == unchanged
        )));
        assert_eq!(projector.chunk_handle("room", [-1, 0, 0]), Some(left));
        assert_eq!(projector.chunk_handle("room", [2, 0, 0]), Some(unchanged));
    }

    #[test]
    fn stale_or_same_revision_changed_scene_rejects_without_projector_mutation() {
        let first = VoxelCollisionScene::from_solid_voxels(1.0, 4, [[0, 0, 0]]).unwrap();
        let conflicting = VoxelCollisionScene::from_solid_voxels(1.0, 4, [[1, 0, 0]]).unwrap();
        let materials = BTreeMap::from([(1, material(1))]);
        let mut projector = VoxelRenderProjector::new();
        let instance = |scene| VoxelProjectionInstance {
            instance_id: "room".to_string(),
            asset_id: "voxel-object/room".to_string(),
            transform: Transform::IDENTITY,
            scene,
        };
        projector.project(&[instance(&first)], &materials).unwrap();
        let handle = projector.chunk_handle("room", [0, 0, 0]);
        assert!(matches!(
            projector.project(&[instance(&conflicting)], &materials),
            Err(VoxelProjectionError::StaleSourceRevision {
                previous: 0,
                candidate: 0,
                ..
            })
        ));
        assert_eq!(projector.chunk_handle("room", [0, 0, 0]), handle);
    }

    #[test]
    fn transform_only_update_does_not_require_a_voxel_revision() {
        let scene = VoxelCollisionScene::from_solid_voxels(1.0, 4, [[0, 0, 0]]).unwrap();
        let materials = BTreeMap::from([(1, material(1))]);
        let mut projector = VoxelRenderProjector::new();
        let instance = |translation| VoxelProjectionInstance {
            instance_id: "room".to_string(),
            asset_id: "voxel-object/room".to_string(),
            transform: Transform {
                translation,
                ..Transform::IDENTITY
            },
            scene: &scene,
        };
        projector
            .project(&[instance([0.0, 0.0, 0.0])], &materials)
            .unwrap();
        let root = projector.root_handle("room").unwrap();
        let update = projector
            .project(&[instance([2.0, 0.0, -1.0])], &materials)
            .unwrap();
        assert!(update.frame.ops.iter().any(|operation| matches!(
            operation,
            RenderDiff::Update { handle, transform: Some(value), .. }
                if *handle == root && value.translation == [2.0, 0.0, -1.0]
        )));
        assert_eq!(projector.root_handle("room"), Some(root));
    }

    #[test]
    fn residency_admit_replace_and_evict_keep_exact_retained_handles() {
        let mut scene = VoxelCollisionScene::from_material_voxels(1.0, 2, []).unwrap();
        let materials = BTreeMap::from([(1, material(1))]);
        let leases = VoxelChunkLeaseRegistry::default();
        let mut projector = VoxelRenderProjector::new();
        let project = |projector: &mut VoxelRenderProjector, scene: &VoxelCollisionScene| {
            projector
                .project(
                    &[VoxelProjectionInstance {
                        instance_id: "terrain".to_string(),
                        asset_id: "voxel-object/terrain".to_string(),
                        transform: Transform::IDENTITY,
                        scene,
                    }],
                    &materials,
                )
                .unwrap()
        };
        project(&mut projector, &scene);
        let chunk = VoxelChunkIdentity::new(0, 0, 0);
        let untouched = VoxelChunkIdentity::new(2, 0, 0);
        let payload = |filled_index: usize| {
            let mut slots = vec![0; 8];
            slots[filled_index] = 1;
            VoxelChunkPayload::new([2; 3], slots)
        };
        VoxelChunkResidencyService::apply(
            &mut scene,
            &leases,
            VoxelChunkResidencyTransaction {
                expected_scene_source_revision: VoxelSourceRevision::INITIAL,
                operations: &[
                    VoxelChunkResidencyOperation::Admit {
                        chunk,
                        payload: payload(0),
                    },
                    VoxelChunkResidencyOperation::Admit {
                        chunk: untouched,
                        payload: payload(0),
                    },
                ],
            },
        )
        .unwrap();
        project(&mut projector, &scene);
        let handle = projector.chunk_handle("terrain", chunk.to_array()).unwrap();
        let untouched_handle = projector
            .chunk_handle("terrain", untouched.to_array())
            .unwrap();
        let expected_content_hash = VoxelChunkResidencyService::resident_chunk(&scene, chunk)
            .unwrap()
            .content_hash;
        let expected_scene_source_revision = scene.source_revision();
        VoxelChunkResidencyService::apply(
            &mut scene,
            &leases,
            VoxelChunkResidencyTransaction {
                expected_scene_source_revision,
                operations: &[VoxelChunkResidencyOperation::Replace {
                    chunk,
                    expected_content_hash,
                    payload: payload(7),
                }],
            },
        )
        .unwrap();
        let replaced = project(&mut projector, &scene);
        assert_eq!(
            projector.chunk_handle("terrain", chunk.to_array()),
            Some(handle)
        );
        assert_eq!(
            projector.chunk_handle("terrain", untouched.to_array()),
            Some(untouched_handle)
        );
        assert_eq!(
            replaced
                .frame
                .ops
                .iter()
                .filter(|operation| matches!(operation, RenderDiff::ReplaceMeshPayload { .. }))
                .count(),
            1
        );

        let expected_content_hash = VoxelChunkResidencyService::resident_chunk(&scene, chunk)
            .unwrap()
            .content_hash;
        let expected_scene_source_revision = scene.source_revision();
        VoxelChunkResidencyService::apply(
            &mut scene,
            &leases,
            VoxelChunkResidencyTransaction {
                expected_scene_source_revision,
                operations: &[VoxelChunkResidencyOperation::Evict {
                    chunk,
                    expected_content_hash,
                }],
            },
        )
        .unwrap();
        let evicted = project(&mut projector, &scene);
        assert!(evicted.frame.ops.iter().any(|operation| matches!(
            operation,
            RenderDiff::Destroy { handle: actual } if *actual == handle
        )));
        assert_eq!(projector.chunk_handle("terrain", chunk.to_array()), None);
        assert_eq!(
            projector.chunk_handle("terrain", untouched.to_array()),
            Some(untouched_handle)
        );
    }
}
