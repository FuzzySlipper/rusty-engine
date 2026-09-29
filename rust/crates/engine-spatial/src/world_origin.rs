use core_ids::EntityId;
use core_math::Vec3;
use core_space::{GlobalPosition, GlobalPositionError, WorldOrigin};
use entity_state::EntityTransform;
use serde::{Deserialize, Serialize};

use crate::VoxelCollisionScene;

pub const DEFAULT_LOCAL_COORDINATE_ENVELOPE: f32 = 16_384.0;
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

/// One product root to move into the target origin's local frame. Only the
/// translation of `transform` changes; rotation and scale pass through.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldOriginEntity {
    pub entity: EntityId,
    pub transform: EntityTransform,
    pub global_position: GlobalPosition,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorldOriginRebaseRequest {
    pub expected_origin_revision: u64,
    pub expected_voxel_source_revision: u64,
    pub expected_static_mesh_revision: u64,
    pub target_origin: WorldOrigin,
    pub entities: Vec<WorldOriginEntity>,
}

/// A prepared origin and collision-scene candidate plus the rebased local
/// transforms the product publishes through its own state.
pub struct PreparedWorldOriginRebase {
    expected_origin_revision: u64,
    expected_voxel_source_revision: u64,
    expected_static_mesh_revision: u64,
    candidate_origin: WorldOriginState,
    candidate_scene: VoxelCollisionScene,
    affected_transforms: Vec<WorldOriginAffectedTransform>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldOriginAffectedTransform {
    pub entity: EntityId,
    pub transform: EntityTransform,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldOriginRebaseReceipt {
    pub revision_before: u64,
    pub revision_after: u64,
    pub origin_before: WorldOrigin,
    pub origin_after: WorldOrigin,
    pub voxel_source_revision: u64,
    pub static_mesh_revision: u64,
    pub entity_count: usize,
    pub local_envelope: f32,
}

impl PreparedWorldOriginRebase {
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
    StaleVoxelScene {
        expected: u64,
        actual: u64,
    },
    StaleStaticMeshes {
        expected: u64,
        actual: u64,
    },
    SceneOriginMismatch,
    Position {
        entity: EntityId,
        reason: GlobalPositionError,
    },
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
        scene: &VoxelCollisionScene,
        request: WorldOriginRebaseRequest,
    ) -> Result<PreparedWorldOriginRebase, WorldOriginRebaseError> {
        validate_guards(
            origin,
            scene,
            request.expected_origin_revision,
            request.expected_voxel_source_revision,
            request.expected_static_mesh_revision,
        )?;
        validate_origin(request.target_origin)?;
        let revision_after = origin
            .revision
            .checked_add(1)
            .ok_or(WorldOriginRebaseError::OriginRevisionExhausted)?;
        let candidate_scene = scene
            .rebased_candidate(request.target_origin, revision_after)
            .map_err(WorldOriginRebaseError::SpatialCandidate)?;
        let affected_transforms = request
            .entities
            .into_iter()
            .map(|value| {
                let translation = value
                    .global_position
                    .local(request.target_origin, origin.local_envelope)
                    .map_err(|reason| WorldOriginRebaseError::Position {
                        entity: value.entity,
                        reason,
                    })?;
                Ok(WorldOriginAffectedTransform {
                    entity: value.entity,
                    transform: EntityTransform {
                        translation: Vec3::new(translation[0], translation[1], translation[2]),
                        ..value.transform
                    },
                })
            })
            .collect::<Result<Vec<_>, WorldOriginRebaseError>>()?;
        Ok(PreparedWorldOriginRebase {
            expected_origin_revision: request.expected_origin_revision,
            expected_voxel_source_revision: request.expected_voxel_source_revision,
            expected_static_mesh_revision: request.expected_static_mesh_revision,
            candidate_origin: WorldOriginState {
                origin: request.target_origin,
                revision: revision_after,
                local_envelope: origin.local_envelope,
            },
            candidate_scene,
            affected_transforms,
        })
    }

    /// Publishes a prepared origin and collision scene. The product applies
    /// the prepared local transforms itself. The scene checks stop a candidate
    /// built before a voxel or static-mesh change from overwriting it.
    pub fn commit(
        self,
        origin: &mut WorldOriginState,
        scene: &mut VoxelCollisionScene,
        prepared: &PreparedWorldOriginRebase,
    ) -> Result<WorldOriginRebaseReceipt, WorldOriginRebaseError> {
        validate_guards(
            origin,
            scene,
            prepared.expected_origin_revision,
            prepared.expected_voxel_source_revision,
            prepared.expected_static_mesh_revision,
        )?;
        let revision_before = origin.revision;
        let origin_before = origin.origin;
        *origin = prepared.candidate_origin;
        *scene = prepared.candidate_scene.clone();
        Ok(WorldOriginRebaseReceipt {
            revision_before,
            revision_after: origin.revision,
            origin_before,
            origin_after: origin.origin,
            voxel_source_revision: scene.source_revision().raw(),
            static_mesh_revision: scene.static_mesh_collision_revision(),
            entity_count: prepared.affected_transforms.len(),
            local_envelope: origin.local_envelope,
        })
    }
}

fn validate_guards(
    origin: &WorldOriginState,
    scene: &VoxelCollisionScene,
    expected_origin: u64,
    expected_voxels: u64,
    expected_static_meshes: u64,
) -> Result<(), WorldOriginRebaseError> {
    if expected_origin != origin.revision {
        return Err(WorldOriginRebaseError::StaleOrigin {
            expected: expected_origin,
            actual: origin.revision,
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
    if scene.world_origin() != origin.origin || scene.rebase_revision() != origin.revision {
        return Err(WorldOriginRebaseError::SceneOriginMismatch);
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
