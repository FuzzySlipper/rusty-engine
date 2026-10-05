use std::collections::{btree_map::Entry, BTreeMap};

use core_space::Direction6;
use engine_spatial::{VoxelCollisionScene, VoxelMeshChunk};
use render_model::{
    Geometry, Material, MeshAttribute, MeshAttributeKind, MeshAttributeName, MeshBoundsDescriptor,
    MeshBufferLayout, MeshGroupDescriptor, MeshIndexWidth, MeshPayloadDescriptor,
    MeshPayloadSource, MeshProvenance, MeshTextureSpace, RenderDiff, RenderFrameDiff,
    RenderFramePublication, RenderHandle, RenderLayer, RenderMaterialDescriptor, RenderMetadata,
    RenderNode, Transform,
};

use crate::{HandleAllocationError, RenderHandleNamespace, StableHandleRegistry};

/// One voxel scene to project. `instance_id` is the retained key and must be
/// unique within a projection.
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
    pub directional: BTreeMap<(u16, u16, Direction6), u16>,
}

impl VoxelMaterialSlotMapping {
    fn renderer_slot(&self, slot: u16, state: u16, direction: Option<Direction6>) -> u16 {
        direction
            .and_then(|direction| {
                self.directional
                    .get(&(slot, state >> 2, direction))
                    .or_else(|| self.directional.get(&(slot, 0, direction)))
                    .copied()
            })
            .or_else(|| self.base.get(&slot).copied())
            .unwrap_or(slot)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ChunkSnapshot {
    content_hash: u64,
    translation: [f32; 3],
}

/// What the renderer holds for one instance.
#[derive(Debug, Clone)]
struct InstanceSnapshot {
    asset_id: String,
    transform: Transform,
    mesh_state: u64,
    source_revision: u64,
    material_slots: VoxelMaterialSlotMapping,
    chunks: BTreeMap<[i64; 3], ChunkSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum VoxelRenderKey {
    Root(String),
    Chunk { instance: String, chunk: [i64; 3] },
}

/// Which of an instance's chunks a projection visits.
enum ChunkVisit<'a> {
    /// The scene has the mesh state last projected.
    None,
    /// The scene's last change was applied to the mesh state last projected,
    /// so its dirty list names every changed chunk.
    Dirty(&'a [[i64; 3]]),
    /// Compare every chunk: a new or rebound instance, a rebuilt or
    /// diverged scene, a skipped change, or a changed slot mapping.
    All,
}

struct InstancePlan<'a> {
    instance: &'a VoxelProjectionInstance<'a>,
    slots: &'a VoxelMaterialSlotMapping,
    /// The slot mapping changed, so every payload is rewritten.
    replace_all: bool,
    visit: ChunkVisit<'a>,
}

/// Retained voxel projection. Each call visits only the chunks whose meshes
/// changed since the last call, as named by the scene's mesh update, and
/// checks them before touching retained state.
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
        let empty_mapping = VoxelMaterialSlotMapping::default();
        let materials_changed = *materials != self.last_materials;

        // Plan and check every instance before any retained state changes.
        let mut plans = Vec::with_capacity(instances.len());
        for instance in instances_by_id(instances) {
            let slots = material_slots
                .get(&instance.instance_id)
                .unwrap_or(&empty_mapping);
            let scene = instance.scene;
            let previous = self
                .last_instances
                .get(&instance.instance_id)
                .filter(|previous| previous.asset_id == instance.asset_id);
            let replace_all = previous.is_some_and(|previous| previous.material_slots != *slots);
            let visit = match previous {
                Some(previous) if !replace_all && previous.mesh_state == scene.mesh_state() => {
                    ChunkVisit::None
                }
                Some(previous)
                    if !replace_all
                        && scene.mesh_update().previous_mesh_state == Some(previous.mesh_state) =>
                {
                    ChunkVisit::Dirty(&scene.mesh_update().dirty_chunks)
                }
                _ => ChunkVisit::All,
            };
            // A material change can invalidate chunks that did not change.
            match (&visit, materials_changed) {
                (ChunkVisit::All, _) | (_, true) => {
                    check_chunks(instance, slots, materials, scene.mesh_chunks())?
                }
                (ChunkVisit::Dirty(coords), false) => check_chunks(
                    instance,
                    slots,
                    materials,
                    coords.iter().filter_map(|coord| scene.mesh_chunk(*coord)),
                )?,
                (ChunkVisit::None, false) => {}
            }
            plans.push(InstancePlan {
                instance,
                slots,
                replace_all,
                visit,
            });
        }

        let mut operations = Vec::new();
        if materials_changed {
            for (slot, material) in materials {
                if self.last_materials.get(slot) != Some(material) {
                    operations.push(RenderDiff::DefineMaterial {
                        material: material.clone(),
                    });
                }
            }
            self.last_materials = materials.clone();
        }

        // Retire instances that left or were rebound to another asset.
        let current: BTreeMap<&str, &str> = plans
            .iter()
            .map(|plan| {
                (
                    plan.instance.instance_id.as_str(),
                    plan.instance.asset_id.as_str(),
                )
            })
            .collect();
        let retired: Vec<String> = self
            .last_instances
            .iter()
            .filter(|(id, previous)| current.get(id.as_str()) != Some(&previous.asset_id.as_str()))
            .map(|(id, _)| id.clone())
            .collect();
        for instance_id in retired {
            let previous = self
                .last_instances
                .remove(&instance_id)
                .expect("retired voxel instance is retained");
            for chunk in previous.chunks.keys() {
                self.registry.remove(&VoxelRenderKey::Chunk {
                    instance: instance_id.clone(),
                    chunk: *chunk,
                });
            }
            let handle = self
                .registry
                .remove(&VoxelRenderKey::Root(instance_id))
                .expect("retained voxel root has a render handle");
            operations.push(RenderDiff::Destroy { handle });
        }

        for plan in &plans {
            let instance = plan.instance;
            let scene = instance.scene;
            let root_key = VoxelRenderKey::Root(instance.instance_id.clone());
            let (snapshot, root) = match self.last_instances.entry(instance.instance_id.clone()) {
                Entry::Occupied(entry) => {
                    let snapshot = entry.into_mut();
                    let handle = self
                        .registry
                        .handle_of(&root_key)
                        .expect("retained voxel root has a render handle");
                    if snapshot.transform != instance.transform {
                        snapshot.transform = instance.transform;
                        operations.push(RenderDiff::Update {
                            handle,
                            transform: Some(instance.transform),
                            material: None,
                            visible: None,
                            metadata: None,
                        });
                    }
                    if plan.replace_all {
                        snapshot.material_slots = plan.slots.clone();
                    }
                    (snapshot, handle)
                }
                Entry::Vacant(entry) => {
                    let handle = self
                        .registry
                        .allocate(root_key)
                        .map_err(VoxelProjectionError::Handle)?;
                    operations.push(RenderDiff::Create {
                        handle,
                        parent: None,
                        node: root_node(instance),
                    });
                    let snapshot = entry.insert(InstanceSnapshot {
                        asset_id: instance.asset_id.clone(),
                        transform: instance.transform,
                        mesh_state: scene.mesh_state(),
                        source_revision: scene.source_revision().raw(),
                        material_slots: plan.slots.clone(),
                        chunks: BTreeMap::new(),
                    });
                    (snapshot, handle)
                }
            };
            let mut chunks = ChunkProjection {
                registry: &mut self.registry,
                operations: &mut operations,
                instance_id: &instance.instance_id,
                root,
                retained: &mut snapshot.chunks,
                slots: plan.slots,
                replace_all: plan.replace_all,
            };
            match plan.visit {
                ChunkVisit::None => {}
                ChunkVisit::Dirty(coords) => {
                    for coord in coords {
                        chunks.project(*coord, scene.mesh_chunk(*coord))?;
                    }
                }
                ChunkVisit::All => {
                    let gone: Vec<_> = chunks
                        .retained
                        .keys()
                        .filter(|coord| scene.mesh_chunk(**coord).is_none())
                        .copied()
                        .collect();
                    for coord in gone {
                        chunks.project(coord, None)?;
                    }
                    for chunk in scene.mesh_chunks() {
                        chunks.project(chunk.chunk, Some(chunk))?;
                    }
                }
            }
            snapshot.mesh_state = scene.mesh_state();
            snapshot.source_revision = scene.source_revision().raw();
        }

        let stream = self
            .publication_stream
            .get_or_insert_with(|| {
                voxel_publication_stream(plans.iter().map(|plan| &plan.instance.instance_id))
            })
            .clone();
        let base_revision = self.publication_revision;
        self.publication_revision += 1;
        let frame = RenderFrameDiff {
            publication: Some(RenderFramePublication {
                stream,
                base_revision,
                revision: self.publication_revision,
                operation_count: u32::try_from(operations.len())
                    .expect("a voxel frame holds fewer than 2^32 operations"),
            }),
            ops: operations,
        };
        Ok(VoxelProjectionResult {
            readout: VoxelProjectionReadout {
                instance_count: self.last_instances.len(),
                chunk_count: self
                    .last_instances
                    .values()
                    .map(|value| value.chunks.len())
                    .sum(),
                source_revisions: self
                    .last_instances
                    .iter()
                    .map(|(id, value)| (id.clone(), value.source_revision))
                    .collect(),
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

/// Renderer work for one instance's chunks.
struct ChunkProjection<'a> {
    registry: &'a mut StableHandleRegistry<VoxelRenderKey>,
    operations: &'a mut Vec<RenderDiff>,
    instance_id: &'a str,
    root: RenderHandle,
    retained: &'a mut BTreeMap<[i64; 3], ChunkSnapshot>,
    slots: &'a VoxelMaterialSlotMapping,
    replace_all: bool,
}

impl ChunkProjection<'_> {
    /// Bring one chunk to the scene's mesh: create, replace, move or destroy.
    fn project(
        &mut self,
        coord: [i64; 3],
        chunk: Option<&VoxelMeshChunk>,
    ) -> Result<(), VoxelProjectionError> {
        let key = VoxelRenderKey::Chunk {
            instance: self.instance_id.to_string(),
            chunk: coord,
        };
        let Some(chunk) = chunk else {
            if self.retained.remove(&coord).is_some() {
                let handle = self
                    .registry
                    .remove(&key)
                    .expect("retained voxel chunk has a render handle");
                self.operations.push(RenderDiff::Destroy { handle });
            }
            return Ok(());
        };
        let previous = self.retained.insert(
            coord,
            ChunkSnapshot {
                content_hash: chunk.content_hash,
                translation: chunk.translation,
            },
        );
        let handle = match previous {
            Some(_) => self
                .registry
                .handle_of(&key)
                .expect("retained voxel chunk has a render handle"),
            None => {
                let handle = self
                    .registry
                    .allocate(key)
                    .map_err(VoxelProjectionError::Handle)?;
                self.operations.push(RenderDiff::Create {
                    handle,
                    parent: Some(self.root),
                    node: chunk_node(self.instance_id, chunk),
                });
                handle
            }
        };
        if previous
            .is_none_or(|previous| previous.content_hash != chunk.content_hash || self.replace_all)
        {
            self.operations.push(RenderDiff::ReplaceMeshPayload {
                handle,
                payload: voxel_mesh_payload_with_material_slots(chunk, self.slots),
            });
        }
        // A rebuilt chunk can also move (a world-origin rebase does both).
        if previous.is_some_and(|previous| previous.translation != chunk.translation) {
            self.operations.push(RenderDiff::Update {
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
        Ok(())
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

/// Every group of these chunks must resolve to a defined material.
fn check_chunks<'a>(
    instance: &VoxelProjectionInstance<'_>,
    slots: &VoxelMaterialSlotMapping,
    materials: &BTreeMap<u16, RenderMaterialDescriptor>,
    chunks: impl Iterator<Item = &'a VoxelMeshChunk>,
) -> Result<(), VoxelProjectionError> {
    for chunk in chunks {
        for group in &chunk.groups {
            let slot = slots.renderer_slot(group.material_slot, group.state, group.direction);
            if !materials.contains_key(&slot) {
                return Err(VoxelProjectionError::MissingMaterial {
                    instance: instance.instance_id.clone(),
                    slot,
                });
            }
        }
    }
    Ok(())
}

fn instances_by_id<'a>(
    instances: &'a [VoxelProjectionInstance<'a>],
) -> Vec<&'a VoxelProjectionInstance<'a>> {
    let mut values: Vec<_> = instances.iter().collect();
    values.sort_by(|left, right| left.instance_id.cmp(&right.instance_id));
    values
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
    // Cube faces carry face tile coordinates and reconstructed surfaces
    // box-projected ones, so every chunk carries uvs.
    attributes.push(MeshAttribute {
        name: MeshAttributeName::Uv,
        components: 2,
        kind: MeshAttributeKind::F32,
    });
    // Terrain layer weights travel as the chunk's vertex colours.
    let layer_weights = (!chunk.layer_weights.is_empty()).then(|| chunk.layer_weights.clone());
    if layer_weights.is_some() {
        attributes.push(MeshAttribute {
            name: MeshAttributeName::Color,
            components: 4,
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
                material_slot: material_slots.renderer_slot(
                    group.material_slot,
                    group.state,
                    group.direction,
                ),
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
            uvs: Some(chunk.tile_coordinates.clone()),
            colors: layer_weights,
            indices: chunk.indices.clone(),
        },
        provenance: MeshProvenance::VoxelChunk,
        texture_space: Some(MeshTextureSpace {
            cell_size: chunk.voxel_size,
            origin: chunk.origin_voxel.map(|cell| cell as f32),
        }),
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

/// Material errors are reported before any retained state changes. Handle
/// exhaustion (2^40 handles) is not recoverable.
#[derive(Debug, Clone, PartialEq)]
pub enum VoxelProjectionError {
    MissingMaterial { instance: String, slot: u16 },
    Handle(HandleAllocationError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_spatial::SurfaceMode;
    use engine_spatial::{
        MaterialVoxel, SurfaceMeshOptions, VoxelChunkIdentity, VoxelChunkPayload,
        VoxelChunkResidencyOperation, VoxelChunkResidencyService, VoxelEdit, VoxelEditService,
        WorldOrigin, WorldOriginRebaseRequest, WorldOriginRebaseService, WorldOriginState,
    };
    use render_model::MaterialUvStrategy;

    #[test]
    fn state_variant_and_rotation_resolve_face_materials_with_default_fallback() {
        let scene = VoxelCollisionScene::from_material_voxels(
            1.0,
            8,
            [MaterialVoxel {
                address: [0, 0, 0],
                material_slot: 1,
                state: (7 << 2) | 1,
            }],
        )
        .unwrap();
        let mapping = VoxelMaterialSlotMapping {
            base: BTreeMap::from([(1, 10)]),
            directional: BTreeMap::from([
                ((1, 0, Direction6::PosY), 11),
                ((1, 7, Direction6::PosZ), 12),
            ]),
        };
        let chunk = &scene.mesh_chunks().cloned().collect::<Vec<_>>()[0];
        let payload = voxel_mesh_payload_with_material_slots(chunk, &mapping);
        for (source, rendered) in chunk.groups.iter().zip(&payload.groups) {
            let expected = match source.direction.unwrap() {
                Direction6::PosZ => 12,
                Direction6::PosY => 11,
                _ => 10,
            };
            assert_eq!(rendered.material_slot, expected);
        }
        let rotated = chunk
            .groups
            .iter()
            .position(|g| g.direction == Some(Direction6::PosZ))
            .unwrap();
        let vertex = chunk.indices[chunk.groups[rotated].start as usize] as usize;
        assert_eq!(&chunk.normals[vertex * 3..vertex * 3 + 3], &[1.0, 0.0, 0.0]);
    }

    fn material(slot: u16) -> RenderMaterialDescriptor {
        RenderMaterialDescriptor {
            texture_transform: None,
            stochastic_tiling: None,
            terrain_layers: None,
            shader: None,
            id: voxel_material_id(slot),
            color: [0.4, 0.5, 0.6, 1.0],
            texture: None,
            roughness: 1.0,
            metalness: 0.0,
            texture_tint: [1.0; 4],
            emission_color: [0.0; 3],
            emission_intensity: 0.0,
            uv_strategy: MaterialUvStrategy::Flat,
            alpha_mode: Default::default(),
            double_sided: false,
            voxel_surface: None,
            normal_map: None,
            triplanar: None,
        }
    }

    #[test]
    fn projects_engine_spatial_chunks_with_stable_handles() {
        let scene = VoxelCollisionScene::from_material_voxels(
            1.0,
            16,
            [MaterialVoxel {
                state: 0,
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
                state: 0,
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
                state: 0,
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
                state: 0,
                address: [100_000, 0, 0],
                material_slot: 1,
            }],
        )
        .unwrap();
        let mut origin = WorldOriginState::default();
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
            target_origin: WorldOrigin::new([100_000, 0, 0]),
            entities: Vec::new(),
        };
        let prepared = WorldOriginRebaseService.prepare(&origin, request).unwrap();
        (scene, _) = WorldOriginRebaseService
            .commit(&mut origin, &scene, &prepared)
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
                state: 0,
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
    fn textured_reconstructed_surface_projects_with_box_projected_uvs() {
        let scene = VoxelCollisionScene::from_material_voxels_with_mesh_options(
            1.0,
            16,
            [MaterialVoxel {
                state: 0,
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
        let result = projector
            .project(
                &[VoxelProjectionInstance {
                    instance_id: "room".to_string(),
                    asset_id: "voxel-object/room".to_string(),
                    transform: Transform::IDENTITY,
                    scene: &scene,
                }],
                &BTreeMap::from([(1, textured)]),
            )
            .unwrap();
        let uvs = result
            .frame
            .ops
            .iter()
            .find_map(|operation| match operation {
                RenderDiff::ReplaceMeshPayload { payload, .. } => match &payload.source {
                    MeshPayloadSource::Inline { uvs, .. } => uvs.clone(),
                    _ => None,
                },
                _ => None,
            })
            .expect("the chunk payload carries uvs");
        let vertices = scene.mesh_chunk([0, 0, 0]).unwrap().vertices as usize;
        assert_eq!(uvs.len(), vertices * 2);
    }

    #[test]
    fn boundary_edit_replaces_neighbor_once_and_destroys_emptied_chunk() {
        let mut scene = VoxelCollisionScene::from_material_voxels(
            1.0,
            4,
            [
                MaterialVoxel {
                    state: 0,
                    address: [-1, 0, 0],
                    material_slot: 1,
                },
                MaterialVoxel {
                    state: 0,
                    address: [0, 0, 0],
                    material_slot: 1,
                },
                MaterialVoxel {
                    state: 0,
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
        VoxelEditService::apply(&mut scene, &[VoxelEdit::Clear { address: [0, 0, 0] }]).unwrap();
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
    fn a_replaced_scene_at_the_same_revision_is_projected_in_full() {
        let empty = VoxelCollisionScene::from_solid_voxels(1.0, 4, []).unwrap();
        let first = VoxelCollisionScene::from_solid_voxels(1.0, 4, [[0, 0, 0]]).unwrap();
        let second = VoxelCollisionScene::from_solid_voxels(1.0, 4, [[5, 0, 0]]).unwrap();
        let materials = BTreeMap::from([(1, material(1))]);
        let mut projector = VoxelRenderProjector::new();
        let instance = |scene| VoxelProjectionInstance {
            instance_id: "room".to_string(),
            asset_id: "voxel-object/room".to_string(),
            transform: Transform::IDENTITY,
            scene,
        };
        projector.project(&[instance(&empty)], &materials).unwrap();
        projector.project(&[instance(&first)], &materials).unwrap();
        let replaced = projector.chunk_handle("room", [0, 0, 0]).unwrap();
        let update = projector.project(&[instance(&second)], &materials).unwrap();
        assert!(update.frame.ops.iter().any(|operation| matches!(
            operation,
            RenderDiff::Destroy { handle } if *handle == replaced
        )));
        assert!(projector.chunk_handle("room", [1, 0, 0]).is_some());
        assert_eq!(projector.chunk_handle("room", [0, 0, 0]), None);
    }

    #[test]
    fn independently_edited_clones_at_the_same_revision_are_projected_in_full() {
        let mut first = VoxelCollisionScene::from_solid_voxels(1.0, 4, [[0, 0, 0]]).unwrap();
        let mut second = first.clone();
        VoxelEditService::apply(
            &mut first,
            &[VoxelEdit::Set {
                address: [5, 0, 0],
                material_slot: 1,
            }],
        )
        .unwrap();
        VoxelEditService::apply(
            &mut second,
            &[VoxelEdit::Set {
                address: [9, 0, 0],
                material_slot: 1,
            }],
        )
        .unwrap();
        assert_eq!(first.source_revision(), second.source_revision());
        let materials = BTreeMap::from([(1, material(1))]);
        let mut projector = VoxelRenderProjector::new();
        let instance = |scene| VoxelProjectionInstance {
            instance_id: "room".to_string(),
            asset_id: "voxel-object/room".to_string(),
            transform: Transform::IDENTITY,
            scene,
        };
        projector.project(&[instance(&first)], &materials).unwrap();
        projector.project(&[instance(&second)], &materials).unwrap();
        assert!(projector.chunk_handle("room", [2, 0, 0]).is_some());
        assert_eq!(projector.chunk_handle("room", [1, 0, 0]), None);
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
            &[
                VoxelChunkResidencyOperation::Admit {
                    chunk,
                    payload: payload(0),
                },
                VoxelChunkResidencyOperation::Admit {
                    chunk: untouched,
                    payload: payload(0),
                },
            ],
        )
        .unwrap();
        project(&mut projector, &scene);
        let handle = projector.chunk_handle("terrain", chunk.to_array()).unwrap();
        let untouched_handle = projector
            .chunk_handle("terrain", untouched.to_array())
            .unwrap();
        VoxelChunkResidencyService::apply(
            &mut scene,
            &[VoxelChunkResidencyOperation::Replace {
                chunk,
                payload: payload(7),
            }],
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
        VoxelChunkResidencyService::apply(
            &mut scene,
            &[VoxelChunkResidencyOperation::Evict { chunk }],
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

    /// A renderer that keeps only what a viewer can see, and refuses any
    /// operation on a handle it does not hold.
    #[derive(Default)]
    struct Renderer {
        nodes: BTreeMap<RenderHandle, Realized>,
        materials: BTreeMap<String, RenderMaterialDescriptor>,
    }

    #[derive(Clone, Debug, PartialEq)]
    struct Realized {
        parent: Option<RenderHandle>,
        label: String,
        transform: Transform,
        payload: Option<MeshPayloadDescriptor>,
    }

    impl Renderer {
        fn apply(&mut self, frame: &RenderFrameDiff) {
            for operation in &frame.ops {
                match operation {
                    RenderDiff::DefineMaterial { material } => {
                        self.materials.insert(material.id.clone(), material.clone());
                    }
                    RenderDiff::Create {
                        handle,
                        parent,
                        node,
                    } => {
                        assert!(!self.nodes.contains_key(handle), "{handle:?} created twice");
                        if let Some(parent) = parent {
                            assert!(
                                self.nodes.contains_key(parent),
                                "parent {parent:?} not live"
                            );
                        }
                        self.nodes.insert(
                            *handle,
                            Realized {
                                parent: *parent,
                                label: node.metadata.label.clone().unwrap(),
                                transform: node.transform,
                                payload: None,
                            },
                        );
                    }
                    RenderDiff::Destroy { handle } => {
                        assert!(
                            self.nodes.contains_key(handle),
                            "destroy of stale {handle:?}"
                        );
                        let mut doomed = vec![*handle];
                        while let Some(next) = doomed.pop() {
                            self.nodes.remove(&next);
                            doomed.extend(
                                self.nodes
                                    .iter()
                                    .filter(|(_, node)| node.parent == Some(next))
                                    .map(|(child, _)| *child),
                            );
                        }
                    }
                    RenderDiff::Update {
                        handle, transform, ..
                    } => {
                        let node = self.nodes.get_mut(handle).expect("update of a live handle");
                        node.transform = transform.unwrap_or(node.transform);
                    }
                    RenderDiff::ReplaceMeshPayload { handle, payload } => {
                        let node = self
                            .nodes
                            .get_mut(handle)
                            .expect("payload of a live handle");
                        node.payload = Some(payload.clone());
                    }
                    other => panic!("unexpected operation {other:?}"),
                }
            }
        }

        /// The realized scene by label, with each parent's label.
        fn by_label(&self) -> BTreeMap<String, (Option<String>, Realized)> {
            self.nodes
                .values()
                .map(|node| {
                    let parent = node.parent.map(|parent| self.nodes[&parent].label.clone());
                    let mut node = node.clone();
                    node.parent = None;
                    (node.label.clone(), (parent, node))
                })
                .collect()
        }
    }

    struct Rng(u64);

    impl Rng {
        fn below(&mut self, bound: u64) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0 % bound
        }

        fn address(&mut self) -> [i64; 3] {
            [
                self.below(12) as i64,
                self.below(6) as i64,
                self.below(12) as i64,
            ]
        }
    }

    struct Placed {
        id: &'static str,
        scene: VoxelCollisionScene,
        transform: Transform,
        present: bool,
    }

    #[test]
    fn incremental_projection_realizes_the_same_scene_as_a_fresh_attachment() {
        let mut rng = Rng(0x8797_5eed);
        let build = |mode, voxels: Vec<MaterialVoxel>| {
            VoxelCollisionScene::from_material_voxels_with_mesh_options(
                1.0,
                4,
                voxels,
                SurfaceMeshOptions {
                    mode,
                    ..SurfaceMeshOptions::default()
                },
            )
            .unwrap()
        };
        let terrain = |rng: &mut Rng| {
            (0..80)
                .map(|_| (rng.address(), 1 + rng.below(2) as u16))
                .collect::<BTreeMap<_, _>>()
                .into_iter()
                .map(|(address, material_slot)| MaterialVoxel {
                    address,
                    material_slot,
                    state: 0,
                })
                .collect::<Vec<_>>()
        };
        let modes = [
            ("cubes", SurfaceMode::GreedyCubes),
            ("contoured", SurfaceMode::DualContouring),
            ("marched", SurfaceMode::MarchingCubes),
        ];
        let mut placed: Vec<Placed> = modes
            .iter()
            .map(|(id, mode)| Placed {
                id,
                scene: build(*mode, terrain(&mut rng)),
                transform: Transform::IDENTITY,
                present: true,
            })
            .collect();
        let materials: BTreeMap<_, _> = (1..=3).map(|slot| (slot, material(slot))).collect();
        let remapped = VoxelMaterialSlotMapping {
            base: BTreeMap::from([(2, 3)]),
            directional: BTreeMap::new(),
        };
        let mut mapped = false;
        // A clone of the third scene, edited on its own and swapped in.
        let mut fork: Option<VoxelCollisionScene> = None;
        let mut origin = WorldOriginState::default();
        let mut projector = VoxelRenderProjector::new();
        let mut renderer = Renderer::default();
        let mut partial = 0;

        for step in 0..300 {
            let target = rng.below(3) as usize;
            let edits: Vec<_> = (0..=rng.below(4))
                .map(|_| {
                    let address = rng.address();
                    if rng.below(2) == 0 {
                        VoxelEdit::Clear { address }
                    } else {
                        VoxelEdit::Set {
                            address,
                            material_slot: 1 + rng.below(2) as u16,
                        }
                    }
                })
                .collect();
            let _ = VoxelEditService::apply(&mut placed[target].scene, &edits);
            if let Some(fork) = &mut fork {
                let address = rng.address();
                let _ = VoxelEditService::apply(
                    fork,
                    &[VoxelEdit::Set {
                        address,
                        material_slot: 1,
                    }],
                );
            }
            match rng.below(40) {
                0..=3 => {
                    placed[target].transform.translation = [rng.below(5) as f32, 0.0, 0.0];
                }
                4..=6 => mapped = !mapped,
                7..=9 => placed[2].present = !placed[2].present,
                // A replacement scene starts a new mesh state at revision zero.
                10 | 11 => {
                    let scene = &placed[1].scene;
                    let mut voxels = scene.material_voxels();
                    voxels.retain(|_| rng.below(4) != 0);
                    placed[1].scene = build(SurfaceMode::DualContouring, voxels);
                }
                12 => {
                    let scene = &mut placed[0].scene;
                    let target_origin = WorldOrigin::new([4 * rng.below(3) as i64, 0, 0]);
                    let request = WorldOriginRebaseRequest {
                        target_origin,
                        entities: Vec::new(),
                    };
                    let prepared = WorldOriginRebaseService.prepare(&origin, request).unwrap();
                    (*scene, _) = WorldOriginRebaseService
                        .commit(&mut origin, scene, &prepared)
                        .unwrap();
                }
                13 | 14 => match &mut fork {
                    None => fork = Some(placed[2].scene.clone()),
                    Some(fork) => std::mem::swap(fork, &mut placed[2].scene),
                },
                _ => {}
            }
            // Some steps let several revisions pass before projecting.
            if rng.below(5) == 0 {
                continue;
            }
            let instances: Vec<_> = placed
                .iter()
                .filter(|value| value.present)
                .map(|value| VoxelProjectionInstance {
                    instance_id: value.id.to_string(),
                    asset_id: format!("voxel-object/{}", value.id),
                    transform: value.transform,
                    scene: &value.scene,
                })
                .collect();
            let slots = BTreeMap::from([(
                "cubes".to_string(),
                if mapped {
                    remapped.clone()
                } else {
                    VoxelMaterialSlotMapping::default()
                },
            )]);
            let update = projector
                .project_mapped_directional(&instances, &materials, &slots)
                .unwrap();
            let replaced = update
                .frame
                .ops
                .iter()
                .filter(|operation| matches!(operation, RenderDiff::ReplaceMeshPayload { .. }))
                .count();
            if replaced < update.readout.chunk_count {
                partial += 1;
            }
            renderer.apply(&update.frame);

            let mut attached = Renderer::default();
            attached.apply(
                &VoxelRenderProjector::new()
                    .project_mapped_directional(&instances, &materials, &slots)
                    .unwrap()
                    .frame,
            );
            let (incremental, fresh) = (renderer.by_label(), attached.by_label());
            let differing: Vec<_> = incremental
                .keys()
                .chain(fresh.keys())
                .filter(|label| incremental.get(*label) != fresh.get(*label))
                .collect();
            assert!(differing.is_empty(), "step {step}: {differing:?}");
            assert_eq!(renderer.materials, attached.materials, "step {step}");
        }
        // Most frames touch only some chunks.
        assert!(partial > 200, "{partial} partial frames");
    }
}
