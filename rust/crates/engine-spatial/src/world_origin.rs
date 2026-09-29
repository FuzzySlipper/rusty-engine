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
    pub target_origin: WorldOrigin,
    pub entities: Vec<WorldOriginEntity>,
}

/// A prepared target origin plus the rebased local transforms the product
/// publishes through its own state. It holds no scene: commit rebases the
/// live collision scene, so edits made after prepare are kept.
#[derive(Debug, Clone, PartialEq)]
pub struct PreparedWorldOriginRebase {
    target_origin: WorldOrigin,
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

    pub const fn target_origin(&self) -> WorldOrigin {
        self.target_origin
    }
}

#[derive(Debug)]
pub enum WorldOriginRebaseError {
    InvalidEnvelope,
    OriginOutsideExactF64Range {
        axis: usize,
    },
    OriginRevisionExhausted,
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
    /// Computes each root's local transform in the target frame. The result
    /// depends only on global positions, the target and the session envelope,
    /// so it stays valid whatever changes before commit.
    pub fn prepare(
        self,
        origin: &WorldOriginState,
        request: WorldOriginRebaseRequest,
    ) -> Result<PreparedWorldOriginRebase, WorldOriginRebaseError> {
        validate_origin(request.target_origin)?;
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
            target_origin: request.target_origin,
            affected_transforms,
        })
    }

    /// Moves the origin and returns the live collision scene rebased into it,
    /// for the caller to install in place of `scene`. The product applies the
    /// prepared local transforms itself.
    pub fn commit(
        self,
        origin: &mut WorldOriginState,
        scene: &VoxelCollisionScene,
        prepared: &PreparedWorldOriginRebase,
    ) -> Result<(VoxelCollisionScene, WorldOriginRebaseReceipt), WorldOriginRebaseError> {
        let revision_before = origin.revision;
        let revision_after = revision_before
            .checked_add(1)
            .ok_or(WorldOriginRebaseError::OriginRevisionExhausted)?;
        let rebased = scene
            .rebased_candidate(prepared.target_origin, revision_after)
            .map_err(WorldOriginRebaseError::SpatialCandidate)?;
        let origin_before = origin.origin;
        origin.origin = prepared.target_origin;
        origin.revision = revision_after;
        let receipt = WorldOriginRebaseReceipt {
            revision_before,
            revision_after,
            origin_before,
            origin_after: origin.origin,
            voxel_source_revision: rebased.source_revision().raw(),
            static_mesh_revision: rebased.static_mesh_collision_revision(),
            entity_count: prepared.affected_transforms.len(),
            local_envelope: origin.local_envelope,
        };
        Ok((rebased, receipt))
    }
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
