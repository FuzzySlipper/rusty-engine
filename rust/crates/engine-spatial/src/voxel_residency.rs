//! Explicit, bounded residency transactions for canonical voxel chunks.
//!
//! Residency is a mechanism boundary, not a streaming policy. Callers decide
//! which chunks to source or retain; this module validates complete dense chunk
//! payloads, guards pinned chunks, prepares every derived projection off to the
//! side, and publishes one coherent scene revision only after a guarded commit.

use std::collections::{BTreeMap, BTreeSet};

use core_space::{ChunkCoord, ChunkDims};
use core_voxel::VoxelValue;
use serde::{Deserialize, Serialize};
use svc_volume::VoxelChunk;

use crate::{
    CollisionSceneError, SurfaceMode, VoxelCollisionScene, VoxelEditHistory, VoxelEditHistoryError,
    VoxelEditHistoryResetReceipt, VoxelProjectionRevisions, VoxelSourceRevision, MAX_SOLID_VOXELS,
    MAX_VOXEL_COORDINATE_ABS, MAX_VOXEL_MATERIAL_SLOT,
};

/// One transaction cannot perform unbounded resident-set churn.
pub const MAX_VOXEL_CHUNKS_PER_RESIDENCY_TRANSACTION: usize = 64;
/// Bounds validation and candidate materialization work for dense payloads.
pub const MAX_VOXEL_CHUNK_PAYLOAD_SLOTS_PER_TRANSACTION: usize =
    MAX_VOXEL_CHUNKS_PER_RESIDENCY_TRANSACTION * 64 * 64 * 64;
/// Bounds the canonical resident index rebuilt by this mechanism.
pub const MAX_RESIDENT_VOXEL_CHUNKS: usize = 4_096;
/// Bounds explicit in-flight ownership evidence retained by one registry.
pub const MAX_VOXEL_CHUNK_LEASES: usize = 4_096;

/// Stable signed identity of one canonical world chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelChunkIdentity {
    pub x: i64,
    pub y: i64,
    pub z: i64,
}

impl VoxelChunkIdentity {
    pub const ORIGIN: Self = Self { x: 0, y: 0, z: 0 };

    pub const fn new(x: i64, y: i64, z: i64) -> Self {
        Self { x, y, z }
    }

    pub const fn from_array(coordinate: [i64; 3]) -> Self {
        Self::new(coordinate[0], coordinate[1], coordinate[2])
    }

    pub const fn to_array(self) -> [i64; 3] {
        [self.x, self.y, self.z]
    }

    const fn to_chunk_coord(self) -> ChunkCoord {
        ChunkCoord::new(self.x, self.y, self.z)
    }
}

impl From<ChunkCoord> for VoxelChunkIdentity {
    fn from(coordinate: ChunkCoord) -> Self {
        Self::new(coordinate.x, coordinate.y, coordinate.z)
    }
}

impl From<VoxelChunkIdentity> for ChunkCoord {
    fn from(identity: VoxelChunkIdentity) -> Self {
        identity.to_chunk_coord()
    }
}

/// Deterministic hash of one chunk's dimensions and complete local contents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct VoxelChunkContentHash(u64);

impl VoxelChunkContentHash {
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}

/// Complete dense local authority in X-fastest, then Y, then Z order.
///
/// Slot zero is empty. Positive slots name semantic-neutral material entries and
/// are validated against [`MAX_VOXEL_MATERIAL_SLOT`] before candidate creation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelChunkPayload {
    pub dimensions: [u32; 3],
    pub material_slots: Vec<u16>,
}

impl VoxelChunkPayload {
    pub fn new(dimensions: [u32; 3], material_slots: Vec<u16>) -> Self {
        Self {
            dimensions,
            material_slots,
        }
    }

    pub fn solid_voxel_count(&self) -> usize {
        self.material_slots
            .iter()
            .filter(|slot| **slot != 0)
            .count()
    }
}

/// One explicit resident-set operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoxelChunkResidencyOperation {
    Admit {
        chunk: VoxelChunkIdentity,
        payload: VoxelChunkPayload,
    },
    Replace {
        chunk: VoxelChunkIdentity,
        expected_content_hash: VoxelChunkContentHash,
        payload: VoxelChunkPayload,
    },
    Evict {
        chunk: VoxelChunkIdentity,
        expected_content_hash: VoxelChunkContentHash,
    },
}

impl VoxelChunkResidencyOperation {
    pub const fn chunk(&self) -> VoxelChunkIdentity {
        match self {
            Self::Admit { chunk, .. } | Self::Replace { chunk, .. } | Self::Evict { chunk, .. } => {
                *chunk
            }
        }
    }
}

/// A caller must name the exact scene revision it observed.
#[derive(Debug, Clone, Copy)]
pub struct VoxelChunkResidencyTransaction<'a> {
    pub expected_scene_source_revision: VoxelSourceRevision,
    pub operations: &'a [VoxelChunkResidencyOperation],
}

/// Stable readout for one resident chunk without exposing mutable authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResidentVoxelChunk {
    pub chunk: VoxelChunkIdentity,
    pub content_hash: VoxelChunkContentHash,
    pub solid_voxel_count: usize,
}

impl ResidentVoxelChunk {
    pub const fn is_empty(self) -> bool {
        self.solid_voxel_count == 0
    }
}

/// Stable identity of one explicit lease. IDs are never reused by a registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VoxelChunkLeaseId(u64);

impl VoxelChunkLeaseId {
    pub const fn raw(self) -> u64 {
        self.0
    }
}

/// Evidence retained for each active pin on a resident chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoxelChunkLeaseEvidence {
    pub lease_id: VoxelChunkLeaseId,
    pub chunk: VoxelChunkIdentity,
    pub acquired_content_hash: VoxelChunkContentHash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoxelChunkLeaseError {
    ChunkNotResident { chunk: VoxelChunkIdentity },
    TooManyLeases { limit: usize },
    LeaseIdentityExhausted,
    UnknownLease { lease_id: VoxelChunkLeaseId },
}

impl std::fmt::Display for VoxelChunkLeaseError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for VoxelChunkLeaseError {}

/// Instance-owned registry for bounded, explicit chunk pins.
///
/// Releasing is deliberate; lease handles do not acquire hidden drop behavior.
#[derive(Debug, Default, Clone)]
pub struct VoxelChunkLeaseRegistry {
    next_lease_id: u64,
    generation: u64,
    leases: BTreeMap<VoxelChunkLeaseId, VoxelChunkLeaseEvidence>,
    by_chunk: BTreeMap<VoxelChunkIdentity, BTreeSet<VoxelChunkLeaseId>>,
}

impl VoxelChunkLeaseRegistry {
    pub fn acquire(
        &mut self,
        scene: &VoxelCollisionScene,
        chunk: VoxelChunkIdentity,
    ) -> Result<VoxelChunkLeaseEvidence, VoxelChunkLeaseError> {
        let resident = VoxelChunkResidencyService::resident_chunk(scene, chunk)
            .ok_or(VoxelChunkLeaseError::ChunkNotResident { chunk })?;
        if self.leases.len() >= MAX_VOXEL_CHUNK_LEASES {
            return Err(VoxelChunkLeaseError::TooManyLeases {
                limit: MAX_VOXEL_CHUNK_LEASES,
            });
        }
        let next = self
            .next_lease_id
            .checked_add(1)
            .ok_or(VoxelChunkLeaseError::LeaseIdentityExhausted)?;
        let evidence = VoxelChunkLeaseEvidence {
            lease_id: VoxelChunkLeaseId(next),
            chunk,
            acquired_content_hash: resident.content_hash,
        };
        self.next_lease_id = next;
        self.generation = self.generation.wrapping_add(1);
        self.leases.insert(evidence.lease_id, evidence);
        self.by_chunk
            .entry(chunk)
            .or_default()
            .insert(evidence.lease_id);
        Ok(evidence)
    }

    pub fn release(
        &mut self,
        lease_id: VoxelChunkLeaseId,
    ) -> Result<VoxelChunkLeaseEvidence, VoxelChunkLeaseError> {
        let evidence = self
            .leases
            .remove(&lease_id)
            .ok_or(VoxelChunkLeaseError::UnknownLease { lease_id })?;
        let leases = self
            .by_chunk
            .get_mut(&evidence.chunk)
            .expect("active lease has a chunk index entry");
        leases.remove(&lease_id);
        if leases.is_empty() {
            self.by_chunk.remove(&evidence.chunk);
        }
        self.generation = self.generation.wrapping_add(1);
        Ok(evidence)
    }

    pub fn active_lease_count(&self) -> usize {
        self.leases.len()
    }

    pub fn is_pinned(&self, chunk: VoxelChunkIdentity) -> bool {
        self.by_chunk
            .get(&chunk)
            .is_some_and(|leases| !leases.is_empty())
    }

    pub fn evidence_for(&self, chunk: VoxelChunkIdentity) -> Vec<VoxelChunkLeaseEvidence> {
        self.by_chunk.get(&chunk).map_or_else(Vec::new, |ids| {
            ids.iter()
                .map(|id| self.leases[id])
                .collect::<Vec<VoxelChunkLeaseEvidence>>()
        })
    }

    pub const fn generation(&self) -> u64 {
        self.generation
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoxelChunkResidencyRejection {
    StaleSceneSourceRevision {
        expected: VoxelSourceRevision,
        actual: VoxelSourceRevision,
    },
    SceneSourceRevisionExhausted,
    EmptyTransaction,
    TooManyOperations {
        limit: usize,
        actual: usize,
    },
    DuplicateChunk {
        first_operation_index: usize,
        duplicate_operation_index: usize,
        chunk: VoxelChunkIdentity,
    },
    ChunkCoordinateOutOfBounds {
        operation_index: usize,
        chunk: VoxelChunkIdentity,
        axis: usize,
        voxel_min: i64,
        voxel_max_inclusive: i64,
        limit: i64,
    },
    PayloadDimensionsMismatch {
        operation_index: usize,
        chunk: VoxelChunkIdentity,
        expected: [u32; 3],
        actual: [u32; 3],
    },
    PayloadSlotCountMismatch {
        operation_index: usize,
        chunk: VoxelChunkIdentity,
        expected: usize,
        actual: usize,
    },
    InvalidMaterialSlot {
        operation_index: usize,
        chunk: VoxelChunkIdentity,
        slot_index: usize,
        material_slot: u16,
        maximum: u16,
    },
    AggregatePayloadSlotsExceeded {
        limit: usize,
        actual: usize,
    },
    ResidentChunkLimitExceeded {
        limit: usize,
        actual: usize,
    },
    ResidentSolidVoxelLimitExceeded {
        limit: usize,
        actual: usize,
    },
    ChunkAlreadyResident {
        operation_index: usize,
        chunk: VoxelChunkIdentity,
        actual_content_hash: VoxelChunkContentHash,
    },
    ChunkNotResident {
        operation_index: usize,
        chunk: VoxelChunkIdentity,
    },
    StaleChunkContentHash {
        operation_index: usize,
        chunk: VoxelChunkIdentity,
        expected: VoxelChunkContentHash,
        actual: VoxelChunkContentHash,
    },
    ChunkPinned {
        operation_index: usize,
        chunk: VoxelChunkIdentity,
        leases: Vec<VoxelChunkLeaseEvidence>,
    },
    NoChanges {
        retained: Vec<VoxelChunkIdentity>,
    },
    HistoryNotEmpty {
        entry_count: usize,
        cursor: usize,
    },
}

impl std::fmt::Display for VoxelChunkResidencyRejection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for VoxelChunkResidencyRejection {}

#[derive(Debug)]
pub enum VoxelChunkResidencyApplyError {
    Rejected(VoxelChunkResidencyRejection),
    ProjectionBuild(CollisionSceneError),
    History(VoxelEditHistoryError),
    PreparedSceneChanged {
        expected_revision: VoxelSourceRevision,
        actual_revision: VoxelSourceRevision,
        expected_residency_hash: u64,
        actual_residency_hash: u64,
    },
    PreparedStaticCollisionChanged {
        expected_revision: u64,
        actual_revision: u64,
    },
    PreparedLeaseRegistryChanged {
        expected_generation: u64,
        actual_generation: u64,
    },
}

impl std::fmt::Display for VoxelChunkResidencyApplyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rejected(rejection) => rejection.fmt(formatter),
            Self::ProjectionBuild(error) => write!(formatter, "projection rebuild failed: {error}"),
            Self::History(error) => write!(formatter, "edit history rejected residency: {error}"),
            Self::PreparedSceneChanged { .. } => {
                write!(
                    formatter,
                    "voxel residency changed after candidate preparation"
                )
            }
            Self::PreparedStaticCollisionChanged { .. } => {
                write!(
                    formatter,
                    "static collision changed after candidate preparation"
                )
            }
            Self::PreparedLeaseRegistryChanged { .. } => {
                write!(
                    formatter,
                    "voxel chunk leases changed after candidate preparation"
                )
            }
        }
    }
}

impl std::error::Error for VoxelChunkResidencyApplyError {}

/// Typed publication evidence for one accepted coherent revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoxelChunkResidencyReceipt {
    pub revision_before: VoxelSourceRevision,
    pub accepted_revision: VoxelSourceRevision,
    pub admitted: Vec<VoxelChunkIdentity>,
    pub replaced: Vec<VoxelChunkIdentity>,
    pub evicted: Vec<VoxelChunkIdentity>,
    pub retained: Vec<VoxelChunkIdentity>,
    pub dirty_chunks: Vec<VoxelChunkIdentity>,
    pub resident_chunk_count: usize,
    pub resident_solid_voxel_count: usize,
    pub residency_hash: u64,
    pub authority_hash: u64,
    pub projections: VoxelProjectionRevisions,
    pub rebuilt_mesh_chunks: usize,
    pub reused_mesh_chunks: usize,
    pub removed_mesh_chunks: usize,
    pub history_reset: Option<VoxelEditHistoryResetReceipt>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoxelResidencyHistoryPolicy {
    RejectIfNonEmpty,
    ResetToPublishedAuthority,
}

/// Complete candidate scene plus the guards observed during preparation.
#[derive(Debug)]
pub struct PreparedVoxelChunkResidency {
    expected_scene_source_revision: VoxelSourceRevision,
    expected_residency_hash: u64,
    expected_static_collision_revision: u64,
    expected_lease_registry_generation: u64,
    candidate: VoxelCollisionScene,
    receipt: VoxelChunkResidencyReceipt,
}

impl PreparedVoxelChunkResidency {
    pub const fn receipt(&self) -> &VoxelChunkResidencyReceipt {
        &self.receipt
    }

    pub const fn candidate_scene(&self) -> &VoxelCollisionScene {
        &self.candidate
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct VoxelChunkResidencyService;

impl VoxelChunkResidencyService {
    pub fn resident_chunk(
        scene: &VoxelCollisionScene,
        identity: VoxelChunkIdentity,
    ) -> Option<ResidentVoxelChunk> {
        scene
            .voxel_world
            .get(identity.to_chunk_coord())
            .map(|chunk| resident_chunk_readout(identity, chunk))
    }

    pub fn resident_chunks(scene: &VoxelCollisionScene) -> Vec<ResidentVoxelChunk> {
        scene
            .voxel_world
            .resident_chunks()
            .map(|(coordinate, chunk)| resident_chunk_readout(coordinate.into(), chunk))
            .collect()
    }

    /// Validate and build a complete candidate without mutating the scene.
    pub fn prepare(
        scene: &VoxelCollisionScene,
        leases: &VoxelChunkLeaseRegistry,
        transaction: VoxelChunkResidencyTransaction<'_>,
    ) -> Result<PreparedVoxelChunkResidency, VoxelChunkResidencyApplyError> {
        if transaction.expected_scene_source_revision != scene.source_revision {
            return Err(VoxelChunkResidencyApplyError::Rejected(
                VoxelChunkResidencyRejection::StaleSceneSourceRevision {
                    expected: transaction.expected_scene_source_revision,
                    actual: scene.source_revision,
                },
            ));
        }
        let accepted_revision =
            scene
                .source_revision
                .checked_next()
                .ok_or(VoxelChunkResidencyApplyError::Rejected(
                    VoxelChunkResidencyRejection::SceneSourceRevisionExhausted,
                ))?;
        if transaction.operations.is_empty() {
            return Err(VoxelChunkResidencyApplyError::Rejected(
                VoxelChunkResidencyRejection::EmptyTransaction,
            ));
        }
        if transaction.operations.len() > MAX_VOXEL_CHUNKS_PER_RESIDENCY_TRANSACTION {
            return Err(VoxelChunkResidencyApplyError::Rejected(
                VoxelChunkResidencyRejection::TooManyOperations {
                    limit: MAX_VOXEL_CHUNKS_PER_RESIDENCY_TRANSACTION,
                    actual: transaction.operations.len(),
                },
            ));
        }

        let chunk_size = scene.chunk_size;
        let grid_id = scene.voxel_world.grid().id();
        let mut aggregate_payload_slots = 0usize;
        let mut operations = BTreeMap::new();
        for (operation_index, operation) in transaction.operations.iter().enumerate() {
            let identity = operation.chunk();
            validate_chunk_identity(identity, chunk_size).map_err(
                |(axis, voxel_min, voxel_max_inclusive)| {
                    VoxelChunkResidencyApplyError::Rejected(
                        VoxelChunkResidencyRejection::ChunkCoordinateOutOfBounds {
                            operation_index,
                            chunk: identity,
                            axis,
                            voxel_min,
                            voxel_max_inclusive,
                            limit: MAX_VOXEL_COORDINATE_ABS,
                        },
                    )
                },
            )?;
            if let Some((first_operation_index, _)) =
                operations.get(&identity).map(|(index, op)| (*index, op))
            {
                return Err(VoxelChunkResidencyApplyError::Rejected(
                    VoxelChunkResidencyRejection::DuplicateChunk {
                        first_operation_index,
                        duplicate_operation_index: operation_index,
                        chunk: identity,
                    },
                ));
            }

            let validated = match operation {
                VoxelChunkResidencyOperation::Admit { payload, .. } => {
                    aggregate_payload_slots = checked_payload_aggregate(
                        aggregate_payload_slots,
                        payload.material_slots.len(),
                    )?;
                    ValidatedResidencyOperation::Admit(validate_payload(
                        operation_index,
                        identity,
                        chunk_size,
                        grid_id,
                        payload,
                    )?)
                }
                VoxelChunkResidencyOperation::Replace {
                    expected_content_hash,
                    payload,
                    ..
                } => {
                    aggregate_payload_slots = checked_payload_aggregate(
                        aggregate_payload_slots,
                        payload.material_slots.len(),
                    )?;
                    ValidatedResidencyOperation::Replace {
                        expected_content_hash: *expected_content_hash,
                        chunk: validate_payload(
                            operation_index,
                            identity,
                            chunk_size,
                            grid_id,
                            payload,
                        )?,
                    }
                }
                VoxelChunkResidencyOperation::Evict {
                    expected_content_hash,
                    ..
                } => ValidatedResidencyOperation::Evict {
                    expected_content_hash: *expected_content_hash,
                },
            };
            operations.insert(identity, (operation_index, validated));
        }

        let old_resident: BTreeSet<_> = scene
            .voxel_world
            .resident_chunks()
            .map(|(coordinate, _)| VoxelChunkIdentity::from(coordinate))
            .collect();
        let mut candidate_world = scene.voxel_world.clone();
        let mut admitted = Vec::new();
        let mut replaced = Vec::new();
        let mut evicted = Vec::new();
        let mut retained = Vec::new();

        for (identity, (operation_index, operation)) in operations {
            let coordinate = identity.to_chunk_coord();
            match operation {
                ValidatedResidencyOperation::Admit(chunk) => {
                    if let Some(current) = candidate_world.get(coordinate) {
                        let actual_content_hash = chunk_content_hash(current);
                        if actual_content_hash == chunk_content_hash(&chunk) {
                            retained.push(identity);
                        } else {
                            return Err(VoxelChunkResidencyApplyError::Rejected(
                                VoxelChunkResidencyRejection::ChunkAlreadyResident {
                                    operation_index,
                                    chunk: identity,
                                    actual_content_hash,
                                },
                            ));
                        }
                    } else {
                        candidate_world.insert(coordinate, chunk);
                        admitted.push(identity);
                    }
                }
                ValidatedResidencyOperation::Replace {
                    expected_content_hash,
                    chunk,
                } => {
                    let current = candidate_world.get(coordinate).ok_or({
                        VoxelChunkResidencyApplyError::Rejected(
                            VoxelChunkResidencyRejection::ChunkNotResident {
                                operation_index,
                                chunk: identity,
                            },
                        )
                    })?;
                    let actual = chunk_content_hash(current);
                    if actual != expected_content_hash {
                        return Err(VoxelChunkResidencyApplyError::Rejected(
                            VoxelChunkResidencyRejection::StaleChunkContentHash {
                                operation_index,
                                chunk: identity,
                                expected: expected_content_hash,
                                actual,
                            },
                        ));
                    }
                    reject_if_pinned(leases, operation_index, identity)?;
                    if actual == chunk_content_hash(&chunk) {
                        retained.push(identity);
                    } else {
                        candidate_world.insert(coordinate, chunk);
                        replaced.push(identity);
                    }
                }
                ValidatedResidencyOperation::Evict {
                    expected_content_hash,
                } => {
                    let current = candidate_world.get(coordinate).ok_or({
                        VoxelChunkResidencyApplyError::Rejected(
                            VoxelChunkResidencyRejection::ChunkNotResident {
                                operation_index,
                                chunk: identity,
                            },
                        )
                    })?;
                    let actual = chunk_content_hash(current);
                    if actual != expected_content_hash {
                        return Err(VoxelChunkResidencyApplyError::Rejected(
                            VoxelChunkResidencyRejection::StaleChunkContentHash {
                                operation_index,
                                chunk: identity,
                                expected: expected_content_hash,
                                actual,
                            },
                        ));
                    }
                    reject_if_pinned(leases, operation_index, identity)?;
                    candidate_world.remove(coordinate);
                    evicted.push(identity);
                }
            }
        }

        if admitted.is_empty() && replaced.is_empty() && evicted.is_empty() {
            return Err(VoxelChunkResidencyApplyError::Rejected(
                VoxelChunkResidencyRejection::NoChanges { retained },
            ));
        }

        let resident_chunk_count = candidate_world.resident_chunks().count();
        if resident_chunk_count > MAX_RESIDENT_VOXEL_CHUNKS {
            return Err(VoxelChunkResidencyApplyError::Rejected(
                VoxelChunkResidencyRejection::ResidentChunkLimitExceeded {
                    limit: MAX_RESIDENT_VOXEL_CHUNKS,
                    actual: resident_chunk_count,
                },
            ));
        }
        let resident_solid_voxel_count = candidate_world
            .resident_chunks()
            .try_fold(0usize, |aggregate, (_, chunk)| {
                aggregate.checked_add(chunk.iter().filter(|(_, value)| value.is_solid()).count())
            })
            .unwrap_or(usize::MAX);
        if resident_solid_voxel_count > MAX_SOLID_VOXELS {
            return Err(VoxelChunkResidencyApplyError::Rejected(
                VoxelChunkResidencyRejection::ResidentSolidVoxelLimitExceeded {
                    limit: MAX_SOLID_VOXELS,
                    actual: resident_solid_voxel_count,
                },
            ));
        }

        let new_resident: BTreeSet<_> = candidate_world
            .resident_chunks()
            .map(|(coordinate, _)| VoxelChunkIdentity::from(coordinate))
            .collect();
        let changed = admitted.iter().chain(&replaced).chain(&evicted).copied();
        let dirty = derive_dirty_chunks(
            chunk_size,
            scene.mesh_options.mode,
            changed,
            &old_resident,
            &new_resident,
        );
        let dirty_coordinates: BTreeSet<ChunkCoord> =
            dirty.iter().copied().map(ChunkCoord::from).collect();
        let mut candidate = VoxelCollisionScene::build_from_voxel_world_at_revision(
            scene.voxel_size,
            scene.chunk_size,
            candidate_world,
            None,
            crate::SceneBuildRevision {
                source: accepted_revision,
                world_origin: scene.world_origin,
                rebase: scene.rebase_revision,
            },
            scene.mesh_options,
            Some((&scene.mesh_chunks, &dirty_coordinates)),
        )
        .map_err(VoxelChunkResidencyApplyError::ProjectionBuild)?;
        candidate.preserve_static_mesh_projection_from(scene);
        let candidate_residency_hash = residency_hash(&candidate);
        let receipt = VoxelChunkResidencyReceipt {
            revision_before: scene.source_revision,
            accepted_revision,
            admitted,
            replaced,
            evicted,
            retained,
            dirty_chunks: dirty,
            resident_chunk_count,
            resident_solid_voxel_count,
            residency_hash: candidate_residency_hash,
            authority_hash: candidate.authority_hash,
            projections: candidate.projection_revisions,
            rebuilt_mesh_chunks: candidate.mesh_update.rebuilt_chunks,
            reused_mesh_chunks: candidate.mesh_update.reused_chunks,
            removed_mesh_chunks: candidate.mesh_update.removed_chunks,
            history_reset: None,
        };
        debug_assert!(receipt
            .projections
            .is_coherent_with(receipt.accepted_revision));
        Ok(PreparedVoxelChunkResidency {
            expected_scene_source_revision: scene.source_revision,
            expected_residency_hash: residency_hash(scene),
            expected_static_collision_revision: scene.static_mesh_collision_revision(),
            expected_lease_registry_generation: leases.generation,
            candidate,
            receipt,
        })
    }

    /// Publish a prepared candidate only if all observed guards still match.
    pub fn commit(
        scene: &mut VoxelCollisionScene,
        leases: &VoxelChunkLeaseRegistry,
        prepared: PreparedVoxelChunkResidency,
    ) -> Result<VoxelChunkResidencyReceipt, VoxelChunkResidencyApplyError> {
        let actual_residency_hash = residency_hash(scene);
        if scene.source_revision != prepared.expected_scene_source_revision
            || actual_residency_hash != prepared.expected_residency_hash
        {
            return Err(VoxelChunkResidencyApplyError::PreparedSceneChanged {
                expected_revision: prepared.expected_scene_source_revision,
                actual_revision: scene.source_revision,
                expected_residency_hash: prepared.expected_residency_hash,
                actual_residency_hash,
            });
        }
        let actual_static_collision_revision = scene.static_mesh_collision_revision();
        if actual_static_collision_revision != prepared.expected_static_collision_revision {
            return Err(
                VoxelChunkResidencyApplyError::PreparedStaticCollisionChanged {
                    expected_revision: prepared.expected_static_collision_revision,
                    actual_revision: actual_static_collision_revision,
                },
            );
        }
        if leases.generation != prepared.expected_lease_registry_generation {
            return Err(
                VoxelChunkResidencyApplyError::PreparedLeaseRegistryChanged {
                    expected_generation: prepared.expected_lease_registry_generation,
                    actual_generation: leases.generation,
                },
            );
        }
        let receipt = prepared.receipt.clone();
        *scene = prepared.candidate;
        Ok(receipt)
    }

    /// Prepare and guarded-commit one complete transaction.
    pub fn apply(
        scene: &mut VoxelCollisionScene,
        leases: &VoxelChunkLeaseRegistry,
        transaction: VoxelChunkResidencyTransaction<'_>,
    ) -> Result<VoxelChunkResidencyReceipt, VoxelChunkResidencyApplyError> {
        let prepared = Self::prepare(scene, leases, transaction)?;
        Self::commit(scene, leases, prepared)
    }

    /// Apply residency and update caller-owned global edit history at the same
    /// no-failure publication boundary. History entries cannot be pruned by
    /// chunk because they are one global hash chain.
    pub fn apply_with_history(
        scene: &mut VoxelCollisionScene,
        leases: &VoxelChunkLeaseRegistry,
        history: &mut VoxelEditHistory,
        history_policy: VoxelResidencyHistoryPolicy,
        transaction: VoxelChunkResidencyTransaction<'_>,
    ) -> Result<VoxelChunkResidencyReceipt, VoxelChunkResidencyApplyError> {
        history
            .ensure_scene_at_cursor(scene)
            .map_err(VoxelChunkResidencyApplyError::History)?;
        if history_policy == VoxelResidencyHistoryPolicy::RejectIfNonEmpty && !history.is_empty() {
            return Err(VoxelChunkResidencyApplyError::Rejected(
                VoxelChunkResidencyRejection::HistoryNotEmpty {
                    entry_count: history.entries().len(),
                    cursor: history.cursor().index,
                },
            ));
        }
        let prepared = Self::prepare(scene, leases, transaction)?;
        let mut receipt = Self::commit(scene, leases, prepared)?;
        receipt.history_reset = Some(history.reset_to_scene(scene));
        Ok(receipt)
    }
}

#[derive(Debug)]
enum ValidatedResidencyOperation {
    Admit(VoxelChunk),
    Replace {
        expected_content_hash: VoxelChunkContentHash,
        chunk: VoxelChunk,
    },
    Evict {
        expected_content_hash: VoxelChunkContentHash,
    },
}

fn checked_payload_aggregate(
    current: usize,
    additional: usize,
) -> Result<usize, VoxelChunkResidencyApplyError> {
    let actual = current.saturating_add(additional);
    if actual > MAX_VOXEL_CHUNK_PAYLOAD_SLOTS_PER_TRANSACTION {
        return Err(VoxelChunkResidencyApplyError::Rejected(
            VoxelChunkResidencyRejection::AggregatePayloadSlotsExceeded {
                limit: MAX_VOXEL_CHUNK_PAYLOAD_SLOTS_PER_TRANSACTION,
                actual,
            },
        ));
    }
    Ok(actual)
}

fn validate_payload(
    operation_index: usize,
    identity: VoxelChunkIdentity,
    chunk_size: u32,
    grid_id: core_space::GridId,
    payload: &VoxelChunkPayload,
) -> Result<VoxelChunk, VoxelChunkResidencyApplyError> {
    let expected_dimensions = [chunk_size; 3];
    if payload.dimensions != expected_dimensions {
        return Err(VoxelChunkResidencyApplyError::Rejected(
            VoxelChunkResidencyRejection::PayloadDimensionsMismatch {
                operation_index,
                chunk: identity,
                expected: expected_dimensions,
                actual: payload.dimensions,
            },
        ));
    }
    let dimensions = ChunkDims::cubic(chunk_size).expect("scene has validated chunk dimensions");
    let expected_slot_count = dimensions.volume() as usize;
    if payload.material_slots.len() != expected_slot_count {
        return Err(VoxelChunkResidencyApplyError::Rejected(
            VoxelChunkResidencyRejection::PayloadSlotCountMismatch {
                operation_index,
                chunk: identity,
                expected: expected_slot_count,
                actual: payload.material_slots.len(),
            },
        ));
    }
    let values: Vec<_> = payload
        .material_slots
        .iter()
        .copied()
        .enumerate()
        .map(|(slot_index, material_slot)| {
            if material_slot > MAX_VOXEL_MATERIAL_SLOT {
                Err(VoxelChunkResidencyApplyError::Rejected(
                    VoxelChunkResidencyRejection::InvalidMaterialSlot {
                        operation_index,
                        chunk: identity,
                        slot_index,
                        material_slot,
                        maximum: MAX_VOXEL_MATERIAL_SLOT,
                    },
                ))
            } else if material_slot == 0 {
                Ok(VoxelValue::EMPTY)
            } else {
                Ok(VoxelValue::solid_raw(material_slot))
            }
        })
        .collect::<Result<_, _>>()?;
    Ok(VoxelChunk::from_values(grid_id, dimensions, &values)
        .expect("validated payload length exactly matches dimensions"))
}

fn validate_chunk_identity(
    identity: VoxelChunkIdentity,
    chunk_size: u32,
) -> Result<(), (usize, i64, i64)> {
    let extent = i64::from(chunk_size);
    for (axis, coordinate) in identity.to_array().into_iter().enumerate() {
        let voxel_min = coordinate.checked_mul(extent).unwrap_or_else(|| {
            if coordinate.is_negative() {
                i64::MIN
            } else {
                i64::MAX
            }
        });
        let voxel_max_inclusive = voxel_min.checked_add(extent - 1).unwrap_or(i64::MAX);
        if voxel_min < -MAX_VOXEL_COORDINATE_ABS || voxel_max_inclusive > MAX_VOXEL_COORDINATE_ABS {
            return Err((axis, voxel_min, voxel_max_inclusive));
        }
    }
    Ok(())
}

fn reject_if_pinned(
    leases: &VoxelChunkLeaseRegistry,
    operation_index: usize,
    chunk: VoxelChunkIdentity,
) -> Result<(), VoxelChunkResidencyApplyError> {
    let evidence = leases.evidence_for(chunk);
    if evidence.is_empty() {
        Ok(())
    } else {
        Err(VoxelChunkResidencyApplyError::Rejected(
            VoxelChunkResidencyRejection::ChunkPinned {
                operation_index,
                chunk,
                leases: evidence,
            },
        ))
    }
}

fn resident_chunk_readout(identity: VoxelChunkIdentity, chunk: &VoxelChunk) -> ResidentVoxelChunk {
    ResidentVoxelChunk {
        chunk: identity,
        content_hash: chunk_content_hash(chunk),
        solid_voxel_count: chunk.iter().filter(|(_, value)| value.is_solid()).count(),
    }
}

fn chunk_content_hash(chunk: &VoxelChunk) -> VoxelChunkContentHash {
    VoxelChunkContentHash::new(chunk.content_hash().0)
}

fn derive_dirty_chunks(
    chunk_size: u32,
    surface_mode: SurfaceMode,
    changed: impl IntoIterator<Item = VoxelChunkIdentity>,
    old_resident: &BTreeSet<VoxelChunkIdentity>,
    new_resident: &BTreeSet<VoxelChunkIdentity>,
) -> Vec<VoxelChunkIdentity> {
    let mut dirty = BTreeSet::new();
    for owner in changed {
        dirty.insert(owner);
        let offsets: &[(i64, i64, i64)] = if surface_mode == SurfaceMode::GreedyCubes {
            &[
                (-1, 0, 0),
                (1, 0, 0),
                (0, -1, 0),
                (0, 1, 0),
                (0, 0, -1),
                (0, 0, 1),
            ]
        } else {
            &SMOOTH_SURFACE_NEIGHBOR_OFFSETS
        };
        for &(x, y, z) in offsets {
            let Some(candidate) = checked_neighbor(owner, x, y, z) else {
                continue;
            };
            if validate_chunk_identity(candidate, chunk_size).is_ok()
                && (old_resident.contains(&candidate) || new_resident.contains(&candidate))
            {
                dirty.insert(candidate);
            }
        }
    }
    dirty.into_iter().collect()
}

fn checked_neighbor(
    owner: VoxelChunkIdentity,
    x: i64,
    y: i64,
    z: i64,
) -> Option<VoxelChunkIdentity> {
    Some(VoxelChunkIdentity::new(
        owner.x.checked_add(x)?,
        owner.y.checked_add(y)?,
        owner.z.checked_add(z)?,
    ))
}

const SMOOTH_SURFACE_NEIGHBOR_OFFSETS: [(i64, i64, i64); 26] = [
    (-1, -1, -1),
    (-1, -1, 0),
    (-1, -1, 1),
    (-1, 0, -1),
    (-1, 0, 0),
    (-1, 0, 1),
    (-1, 1, -1),
    (-1, 1, 0),
    (-1, 1, 1),
    (0, -1, -1),
    (0, -1, 0),
    (0, -1, 1),
    (0, 0, -1),
    (0, 0, 1),
    (0, 1, -1),
    (0, 1, 0),
    (0, 1, 1),
    (1, -1, -1),
    (1, -1, 0),
    (1, -1, 1),
    (1, 0, -1),
    (1, 0, 0),
    (1, 0, 1),
    (1, 1, -1),
    (1, 1, 0),
    (1, 1, 1),
];

fn residency_hash(scene: &VoxelCollisionScene) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET;
    let mut feed = |bytes: &[u8]| {
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(PRIME);
        }
    };
    for (coordinate, chunk) in scene.voxel_world.resident_chunks() {
        for axis in coordinate.to_array() {
            feed(&axis.to_le_bytes());
        }
        feed(&chunk.content_hash().0.to_le_bytes());
    }
    hash
}
