use std::collections::{BTreeMap, BTreeSet};

use core_ids::EntityId;
use core_math::Vec3;
use core_space::{GlobalPosition, GlobalPositionError, WorldOrigin};
use entity_state::{
    CharacterMotionComponent, ComponentReplacement, EntityAuthoringError, EntityAuthoringService,
    EntityState, EntityTransform, TransformComponent, MAX_COMPONENT_REPLACEMENTS,
};
use serde::{Deserialize, Serialize};

use crate::{character_controller::character_collision_world_hash, VoxelCollisionScene};

pub const DEFAULT_LOCAL_COORDINATE_ENVELOPE: f32 = 16_384.0;
pub const MAX_WORLD_ORIGIN_ENTITIES: usize = 1_024;
pub const WORLD_ORIGIN_SNAPSHOT_SCHEMA_VERSION: u32 = 1;
const MAX_WORLD_ORIGIN_CELL_ABS: u64 = 9_000_000_000_000_000;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldOriginState {
    origin: WorldOrigin,
    revision: u64,
    local_envelope: f32,
}

impl WorldOriginState {
    pub fn new(local_envelope: f32) -> Result<Self, WorldOriginRebaseError> {
        validate_envelope(local_envelope)?;
        Ok(Self {
            origin: WorldOrigin::ZERO,
            revision: 0,
            local_envelope,
        })
    }

    pub const fn origin(&self) -> WorldOrigin {
        self.origin
    }

    pub const fn revision(&self) -> u64 {
        self.revision
    }

    pub const fn local_envelope(&self) -> f32 {
        self.local_envelope
    }

    pub const fn readout(&self) -> WorldOriginReadout {
        WorldOriginReadout {
            origin: self.origin,
            revision: self.revision,
            local_envelope: self.local_envelope,
        }
    }

    pub fn global_from_local(
        &self,
        local: [f32; 3],
    ) -> Result<GlobalPosition, GlobalPositionError> {
        GlobalPosition::from_local(self.origin, local)
    }

    pub fn local_from_global(
        &self,
        global: GlobalPosition,
    ) -> Result<[f32; 3], GlobalPositionError> {
        global.local(self.origin, self.local_envelope)
    }
}

impl Default for WorldOriginState {
    fn default() -> Self {
        Self::new(DEFAULT_LOCAL_COORDINATE_ENVELOPE).expect("default origin envelope is valid")
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldOriginReadout {
    pub origin: WorldOrigin,
    pub revision: u64,
    pub local_envelope: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldOriginEntity {
    pub entity: EntityId,
    pub global_position: GlobalPosition,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorldOriginRebaseRequest {
    pub expected_origin_revision: u64,
    pub expected_entity_revision: u64,
    pub expected_voxel_source_revision: u64,
    pub expected_static_mesh_revision: u64,
    pub target_origin: WorldOrigin,
    pub entities: Vec<WorldOriginEntity>,
}

pub struct PreparedWorldOriginRebase {
    expected_origin_revision: u64,
    expected_entity_revision: u64,
    expected_voxel_source_revision: u64,
    expected_static_mesh_revision: u64,
    target_origin: WorldOrigin,
    candidate_entities: EntityState,
    candidate_scene: VoxelCollisionScene,
    affected_entities: Vec<EntityId>,
    entity_count: usize,
}

/// A validated world-origin candidate with only the spatial facts an external
/// owner needs to retain. It deliberately contains no [`EntityState`]: callers
/// publish the copied local transforms through their own product state model.
pub struct PreparedWorldOriginSpatialRebase {
    expected_origin_revision: u64,
    expected_voxel_source_revision: u64,
    expected_static_mesh_revision: u64,
    candidate_origin: WorldOriginState,
    candidate_scene: VoxelCollisionScene,
    affected_transforms: Vec<WorldOriginAffectedTransform>,
    entity_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldOriginAffectedTransform {
    pub entity: EntityId,
    pub transform: EntityTransform,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldOriginSpatialRebaseReceipt {
    pub revision_before: u64,
    pub revision_after: u64,
    pub origin_before: WorldOrigin,
    pub origin_after: WorldOrigin,
    pub voxel_source_revision: u64,
    pub static_mesh_revision: u64,
    pub entity_count: usize,
    pub local_envelope: f32,
}

impl std::fmt::Debug for PreparedWorldOriginRebase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PreparedWorldOriginRebase")
            .field("expected_origin_revision", &self.expected_origin_revision)
            .field("expected_entity_revision", &self.expected_entity_revision)
            .field("target_origin", &self.target_origin)
            .field("entity_count", &self.entity_count)
            .finish_non_exhaustive()
    }
}

impl PreparedWorldOriginRebase {
    /// Drops the call-local entity candidate after extracting its transformed
    /// roots into a reusable spatial candidate. This supports product-owned
    /// entity stores without introducing a second Engine entity world.
    pub fn into_spatial_candidate(
        self,
        origin: WorldOriginState,
    ) -> PreparedWorldOriginSpatialRebase {
        let affected_transforms = self
            .affected_entities
            .iter()
            .map(|entity| {
                self.candidate_entities
                    .transform(*entity)
                    .copied()
                    .map(|component| component.transform())
                    .map(|transform| WorldOriginAffectedTransform {
                        entity: *entity,
                        transform,
                    })
                    .expect("validated rebase roots retain their transform")
            })
            .collect();
        let candidate_origin = WorldOriginState {
            origin: self.target_origin,
            revision: origin.revision + 1,
            local_envelope: origin.local_envelope,
        };
        PreparedWorldOriginSpatialRebase {
            expected_origin_revision: self.expected_origin_revision,
            expected_voxel_source_revision: self.expected_voxel_source_revision,
            expected_static_mesh_revision: self.expected_static_mesh_revision,
            candidate_origin,
            candidate_scene: self.candidate_scene,
            affected_transforms,
            entity_count: self.entity_count,
        }
    }
}

impl PreparedWorldOriginSpatialRebase {
    pub fn affected_transforms(&self) -> &[WorldOriginAffectedTransform] {
        &self.affected_transforms
    }

    pub const fn origin(&self) -> WorldOriginReadout {
        self.candidate_origin.readout()
    }

    pub const fn scene_source_revision(&self) -> u64 {
        self.candidate_scene.source_revision().raw()
    }

    pub fn scene_static_mesh_revision(&self) -> u64 {
        self.candidate_scene.static_mesh_collision_revision()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldOriginRebaseReceipt {
    pub revision_before: u64,
    pub revision_after: u64,
    pub origin_before: WorldOrigin,
    pub origin_after: WorldOrigin,
    pub entity_revision_before: u64,
    pub entity_revision_after: u64,
    pub voxel_source_revision: u64,
    pub static_mesh_revision: u64,
    pub entity_count: usize,
    pub local_envelope: f32,
}

#[derive(Debug)]
pub enum WorldOriginRebaseError {
    InvalidEnvelope,
    OriginOutsideExactF64Range {
        axis: usize,
    },
    OriginRevisionExhausted,
    StaleOrigin {
        expected: u64,
        actual: u64,
    },
    StaleEntityState {
        expected: u64,
        actual: u64,
    },
    StaleVoxelScene {
        expected: u64,
        actual: u64,
    },
    StaleStaticMeshes {
        expected: u64,
        actual: u64,
    },
    SceneOriginMismatch,
    TooManyEntities {
        actual: usize,
        maximum: usize,
    },
    DuplicateEntity {
        entity: EntityId,
    },
    MissingRootEntity {
        entity: EntityId,
    },
    UnexpectedEntity {
        entity: EntityId,
    },
    MissingTransform {
        entity: EntityId,
    },
    ParentedEntity {
        entity: EntityId,
    },
    Position {
        entity: EntityId,
        reason: GlobalPositionError,
    },
    InvalidContinuationHeight {
        entity: EntityId,
    },
    EntityPublication(EntityAuthoringError),
    SpatialCandidate(crate::CollisionSceneError),
    SnapshotEncode,
    SnapshotDecode,
    UnsupportedSnapshotSchema {
        actual: u32,
    },
}

impl std::fmt::Display for WorldOriginRebaseError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "world-origin rebase rejected: {self:?}")
    }
}

impl std::error::Error for WorldOriginRebaseError {}

#[derive(Debug, Default, Clone, Copy)]
pub struct WorldOriginRebaseService;

impl WorldOriginRebaseService {
    pub fn prepare(
        self,
        origin: &WorldOriginState,
        entities: &EntityState,
        scene: &VoxelCollisionScene,
        request: WorldOriginRebaseRequest,
    ) -> Result<PreparedWorldOriginRebase, WorldOriginRebaseError> {
        validate_guards(origin, entities, scene, &request)?;
        validate_origin(request.target_origin)?;
        if request.entities.len() > MAX_WORLD_ORIGIN_ENTITIES {
            return Err(WorldOriginRebaseError::TooManyEntities {
                actual: request.entities.len(),
                maximum: MAX_WORLD_ORIGIN_ENTITIES,
            });
        }
        let mut supplied = BTreeMap::new();
        for binding in request.entities {
            if supplied
                .insert(binding.entity, binding.global_position)
                .is_some()
            {
                return Err(WorldOriginRebaseError::DuplicateEntity {
                    entity: binding.entity,
                });
            }
        }
        let required = entities
            .entities()
            .filter(|entity| {
                entities.transform(entity.id).is_some()
                    && entities.transform_parent(entity.id).is_none()
            })
            .map(|entity| entity.id)
            .collect::<BTreeSet<_>>();
        for entity in &required {
            if !supplied.contains_key(entity) {
                return Err(WorldOriginRebaseError::MissingRootEntity { entity: *entity });
            }
        }
        for entity in supplied.keys() {
            if !required.contains(entity) {
                return Err(if entities.transform_parent(*entity).is_some() {
                    WorldOriginRebaseError::ParentedEntity { entity: *entity }
                } else if entities.transform(*entity).is_none() {
                    WorldOriginRebaseError::MissingTransform { entity: *entity }
                } else {
                    WorldOriginRebaseError::UnexpectedEntity { entity: *entity }
                });
            }
        }

        let revision_after = origin
            .revision
            .checked_add(1)
            .ok_or(WorldOriginRebaseError::OriginRevisionExhausted)?;
        let candidate_scene = scene
            .rebased_candidate(request.target_origin, revision_after)
            .map_err(WorldOriginRebaseError::SpatialCandidate)?;
        let mut candidate_entities = entities.clone();
        replace_transforms(
            &mut candidate_entities,
            &supplied,
            request.target_origin,
            origin.local_envelope,
        )?;
        replace_character_continuations(
            &mut candidate_entities,
            &candidate_scene,
            origin.origin,
            request.target_origin,
            origin.local_envelope,
            supplied.keys().copied(),
        )?;

        Ok(PreparedWorldOriginRebase {
            expected_origin_revision: request.expected_origin_revision,
            expected_entity_revision: request.expected_entity_revision,
            expected_voxel_source_revision: request.expected_voxel_source_revision,
            expected_static_mesh_revision: request.expected_static_mesh_revision,
            target_origin: request.target_origin,
            candidate_entities,
            candidate_scene,
            affected_entities: supplied.keys().copied().collect(),
            entity_count: supplied.len(),
        })
    }

    pub fn commit(
        self,
        origin: &mut WorldOriginState,
        entities: &mut EntityState,
        scene: &mut VoxelCollisionScene,
        prepared: PreparedWorldOriginRebase,
    ) -> Result<WorldOriginRebaseReceipt, WorldOriginRebaseError> {
        validate_live_guards(
            origin,
            entities,
            scene,
            prepared.expected_origin_revision,
            prepared.expected_entity_revision,
            prepared.expected_voxel_source_revision,
            prepared.expected_static_mesh_revision,
        )?;
        let revision_before = origin.revision;
        let origin_before = origin.origin;
        let entity_revision_before = entities.revision();
        let entity_revision_after = prepared.candidate_entities.revision();
        origin.origin = prepared.target_origin;
        origin.revision = revision_before + 1;
        *entities = prepared.candidate_entities;
        *scene = prepared.candidate_scene;
        Ok(WorldOriginRebaseReceipt {
            revision_before,
            revision_after: origin.revision,
            origin_before,
            origin_after: origin.origin,
            entity_revision_before,
            entity_revision_after,
            voxel_source_revision: scene.source_revision().raw(),
            static_mesh_revision: scene.static_mesh_collision_revision(),
            entity_count: prepared.entity_count,
            local_envelope: origin.local_envelope,
        })
    }

    pub fn apply(
        self,
        origin: &mut WorldOriginState,
        entities: &mut EntityState,
        scene: &mut VoxelCollisionScene,
        request: WorldOriginRebaseRequest,
    ) -> Result<WorldOriginRebaseReceipt, WorldOriginRebaseError> {
        let prepared = self.prepare(origin, entities, scene, request)?;
        self.commit(origin, entities, scene, prepared)
    }

    /// Commits a candidate that was prepared using a temporary entity state.
    /// The product remains responsible for applying the returned local
    /// transforms and guarding its own entity revision; Engine rechecks only
    /// the origin and collision-scene facts it owns.
    pub fn commit_spatial(
        self,
        origin: &mut WorldOriginState,
        scene: &mut VoxelCollisionScene,
        prepared: &PreparedWorldOriginSpatialRebase,
    ) -> Result<WorldOriginSpatialRebaseReceipt, WorldOriginRebaseError> {
        if prepared.expected_origin_revision != origin.revision {
            return Err(WorldOriginRebaseError::StaleOrigin {
                expected: prepared.expected_origin_revision,
                actual: origin.revision,
            });
        }
        if prepared.expected_voxel_source_revision != scene.source_revision().raw() {
            return Err(WorldOriginRebaseError::StaleVoxelScene {
                expected: prepared.expected_voxel_source_revision,
                actual: scene.source_revision().raw(),
            });
        }
        if prepared.expected_static_mesh_revision != scene.static_mesh_collision_revision() {
            return Err(WorldOriginRebaseError::StaleStaticMeshes {
                expected: prepared.expected_static_mesh_revision,
                actual: scene.static_mesh_collision_revision(),
            });
        }
        if scene.world_origin() != origin.origin || scene.rebase_revision() != origin.revision {
            return Err(WorldOriginRebaseError::SceneOriginMismatch);
        }

        let revision_before = origin.revision;
        let origin_before = origin.origin;
        *origin = prepared.candidate_origin;
        *scene = prepared.candidate_scene.clone();
        Ok(WorldOriginSpatialRebaseReceipt {
            revision_before,
            revision_after: origin.revision,
            origin_before,
            origin_after: origin.origin,
            voxel_source_revision: scene.source_revision().raw(),
            static_mesh_revision: scene.static_mesh_collision_revision(),
            entity_count: prepared.entity_count,
            local_envelope: origin.local_envelope,
        })
    }
}

fn replace_transforms(
    entities: &mut EntityState,
    supplied: &BTreeMap<EntityId, GlobalPosition>,
    target: WorldOrigin,
    envelope: f32,
) -> Result<(), WorldOriginRebaseError> {
    let replacements = supplied
        .iter()
        .map(|(entity, global)| {
            let before = entities
                .transform(*entity)
                .copied()
                .ok_or(WorldOriginRebaseError::MissingTransform { entity: *entity })?;
            let translation = global.local(target, envelope).map_err(|reason| {
                WorldOriginRebaseError::Position {
                    entity: *entity,
                    reason,
                }
            })?;
            Ok(ComponentReplacement {
                expected_revision: entities
                    .component_revision::<TransformComponent>(*entity)
                    .expect("built-in transform registration"),
                entity: *entity,
                component: TransformComponent::from_transform(EntityTransform {
                    translation: vec3(translation),
                    rotation: before.rotation,
                    scale: before.scale,
                }),
            })
        })
        .collect::<Result<Vec<_>, WorldOriginRebaseError>>()?;
    for chunk in replacements.chunks(MAX_COMPONENT_REPLACEMENTS) {
        EntityAuthoringService
            .replace_root_transforms_for_local_frame(entities, chunk.to_vec())
            .map_err(WorldOriginRebaseError::EntityPublication)?;
    }
    Ok(())
}

fn replace_character_continuations(
    entities: &mut EntityState,
    scene: &VoxelCollisionScene,
    previous: WorldOrigin,
    target: WorldOrigin,
    envelope: f32,
    participants: impl IntoIterator<Item = EntityId>,
) -> Result<(), WorldOriginRebaseError> {
    let delta_y = (i128::from(previous.cell()[1]) - i128::from(target.cell()[1])) as f64;
    let mut replacements = Vec::new();
    for entity in participants {
        let Some(mut motion) = entities.character_motion(entity).copied() else {
            continue;
        };
        if let Some(support) = motion.support_entity {
            motion.support_previous_translation = entities
                .transform(support)
                .ok_or(WorldOriginRebaseError::MissingTransform { entity: support })?
                .translation;
        }
        motion.fall_origin_y = shifted_height(motion.fall_origin_y, delta_y, envelope)
            .ok_or(WorldOriginRebaseError::InvalidContinuationHeight { entity })?;
        motion.peak_y = shifted_height(motion.peak_y, delta_y, envelope)
            .ok_or(WorldOriginRebaseError::InvalidContinuationHeight { entity })?;
        motion.collision_world_hash = character_collision_world_hash(entities, scene, entity);
        replacements.push(ComponentReplacement {
            expected_revision: entities
                .component_revision::<CharacterMotionComponent>(entity)
                .expect("built-in character-motion registration"),
            entity,
            component: motion,
        });
    }
    for chunk in replacements.chunks(MAX_COMPONENT_REPLACEMENTS) {
        EntityAuthoringService
            .replace_components(entities, chunk.to_vec())
            .map_err(WorldOriginRebaseError::EntityPublication)?;
    }
    Ok(())
}

fn shifted_height(value: f32, delta: f64, envelope: f32) -> Option<f32> {
    let shifted = f64::from(value) + delta;
    (shifted.is_finite() && shifted.abs() <= f64::from(envelope)).then_some(shifted as f32)
}

fn validate_guards(
    origin: &WorldOriginState,
    entities: &EntityState,
    scene: &VoxelCollisionScene,
    request: &WorldOriginRebaseRequest,
) -> Result<(), WorldOriginRebaseError> {
    validate_live_guards(
        origin,
        entities,
        scene,
        request.expected_origin_revision,
        request.expected_entity_revision,
        request.expected_voxel_source_revision,
        request.expected_static_mesh_revision,
    )?;
    if scene.world_origin() != origin.origin || scene.rebase_revision() != origin.revision {
        return Err(WorldOriginRebaseError::SceneOriginMismatch);
    }
    Ok(())
}

fn validate_live_guards(
    origin: &WorldOriginState,
    entities: &EntityState,
    scene: &VoxelCollisionScene,
    expected_origin: u64,
    expected_entities: u64,
    expected_voxels: u64,
    expected_static_meshes: u64,
) -> Result<(), WorldOriginRebaseError> {
    if expected_origin != origin.revision {
        return Err(WorldOriginRebaseError::StaleOrigin {
            expected: expected_origin,
            actual: origin.revision,
        });
    }
    if expected_entities != entities.revision() {
        return Err(WorldOriginRebaseError::StaleEntityState {
            expected: expected_entities,
            actual: entities.revision(),
        });
    }
    if expected_voxels != scene.source_revision().raw() {
        return Err(WorldOriginRebaseError::StaleVoxelScene {
            expected: expected_voxels,
            actual: scene.source_revision().raw(),
        });
    }
    if expected_static_meshes != scene.static_mesh_collision_revision() {
        return Err(WorldOriginRebaseError::StaleStaticMeshes {
            expected: expected_static_meshes,
            actual: scene.static_mesh_collision_revision(),
        });
    }
    Ok(())
}

fn validate_origin(origin: WorldOrigin) -> Result<(), WorldOriginRebaseError> {
    if let Some(axis) = origin
        .cell()
        .iter()
        .position(|value| value.unsigned_abs() > MAX_WORLD_ORIGIN_CELL_ABS)
    {
        return Err(WorldOriginRebaseError::OriginOutsideExactF64Range { axis });
    }
    Ok(())
}

fn validate_envelope(envelope: f32) -> Result<(), WorldOriginRebaseError> {
    if !envelope.is_finite() || !(1.0..=1_000_000.0).contains(&envelope) {
        return Err(WorldOriginRebaseError::InvalidEnvelope);
    }
    Ok(())
}

fn vec3(value: [f32; 3]) -> Vec3 {
    Vec3::new(value[0], value[1], value[2])
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct WorldOriginSnapshotV1 {
    schema_version: u32,
    origin: WorldOrigin,
    revision: u64,
    local_envelope: f32,
}

pub fn encode_world_origin_state(
    state: WorldOriginState,
) -> Result<Vec<u8>, WorldOriginRebaseError> {
    serde_json::to_vec_pretty(&WorldOriginSnapshotV1 {
        schema_version: WORLD_ORIGIN_SNAPSHOT_SCHEMA_VERSION,
        origin: state.origin,
        revision: state.revision,
        local_envelope: state.local_envelope,
    })
    .map_err(|_| WorldOriginRebaseError::SnapshotEncode)
}

pub fn decode_world_origin_state(bytes: &[u8]) -> Result<WorldOriginState, WorldOriginRebaseError> {
    let snapshot: WorldOriginSnapshotV1 =
        serde_json::from_slice(bytes).map_err(|_| WorldOriginRebaseError::SnapshotDecode)?;
    if snapshot.schema_version != WORLD_ORIGIN_SNAPSHOT_SCHEMA_VERSION {
        return Err(WorldOriginRebaseError::UnsupportedSnapshotSchema {
            actual: snapshot.schema_version,
        });
    }
    validate_origin(snapshot.origin)?;
    validate_envelope(snapshot.local_envelope)?;
    Ok(WorldOriginState {
        origin: snapshot.origin,
        revision: snapshot.revision,
        local_envelope: snapshot.local_envelope,
    })
}
