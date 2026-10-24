//! Residency changes for canonical voxel chunks. Callers decide which chunks
//! to load, replace or unload; this module validates dense chunk payloads,
//! writes them into the scene and rebuilds only the changed chunks' meshes and
//! colliders.

use std::collections::{BTreeMap, BTreeSet};

use core_space::{ChunkCoord, ChunkDims};
use core_voxel::VoxelValue;
use serde::{Deserialize, Serialize};
use svc_volume::VoxelChunk;

use crate::{
    ChunkRebuild, CollisionSceneError, SurfaceMode, VoxelCollisionScene, VoxelProjectionRevisions,
    VoxelSourceRevision, MAX_VOXEL_COORDINATE_ABS,
};

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
/// Slot zero is empty. Every other slot names a semantic-neutral material entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelChunkPayload {
    pub dimensions: [u32; 3],
    pub material_slots: Vec<u16>,
    /// Empty means default state for every cell; otherwise exactly one entry per slot.
    pub states: Vec<u16>,
    /// Empty means the default density for every cell; otherwise one density
    /// per slot, in voxel units. Its magnitude places the surface; its sign
    /// follows whether the slot is solid. Reconstructed surfaces cross between
    /// samples where the densities say.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub densities: Vec<f32>,
}

impl VoxelChunkPayload {
    pub fn new(dimensions: [u32; 3], material_slots: Vec<u16>) -> Self {
        Self {
            dimensions,
            material_slots,
            states: Vec::new(),
            densities: Vec::new(),
        }
    }

    pub fn solid_voxel_count(&self) -> usize {
        self.material_slots
            .iter()
            .filter(|slot| **slot != 0)
            .count()
    }
}

/// One resident-set operation. Admit and Replace both make the chunk hold the
/// payload; Evict of a non-resident chunk changes nothing.
#[derive(Debug, Clone, PartialEq)]
pub enum VoxelChunkResidencyOperation {
    Admit {
        chunk: VoxelChunkIdentity,
        payload: VoxelChunkPayload,
    },
    Replace {
        chunk: VoxelChunkIdentity,
        payload: VoxelChunkPayload,
    },
    Evict {
        chunk: VoxelChunkIdentity,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoxelChunkResidencyRejection {
    InvalidCellStates {
        operation_index: usize,
    },
    /// Densities must be finite and one per slot.
    InvalidDensities {
        operation_index: usize,
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
    /// Every operation already matched the resident set.
    NoChanges {
        retained: Vec<VoxelChunkIdentity>,
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
    /// Rebuilding a changed chunk failed; the change was reverted.
    ProjectionBuild(CollisionSceneError),
}

impl std::fmt::Display for VoxelChunkResidencyApplyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rejected(rejection) => rejection.fmt(formatter),
            Self::ProjectionBuild(error) => write!(formatter, "projection rebuild failed: {error}"),
        }
    }
}

impl std::error::Error for VoxelChunkResidencyApplyError {}

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
    pub authority_hash: u64,
    pub projections: VoxelProjectionRevisions,
    pub rebuilt_mesh_chunks: usize,
    pub reused_mesh_chunks: usize,
    pub removed_mesh_chunks: usize,
    /// Crossing normals (#9504) the replaced and evicted chunks carried. A
    /// residency payload carries voxels and densities, not normals: a chunk
    /// admitted again draws its sharp cuts from estimated normals until a
    /// brush or stamp cuts them again.
    pub dropped_hermite_normals: usize,
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

    /// Apply operations in order. Invalid payloads reject the batch before
    /// anything changes.
    pub fn apply(
        scene: &mut VoxelCollisionScene,
        operations: &[VoxelChunkResidencyOperation],
    ) -> Result<VoxelChunkResidencyReceipt, VoxelChunkResidencyApplyError> {
        let written = write(scene, operations)?;
        let rebuild =
            match scene.rebuild_chunks(&written.changed, &written.dirty, &written.field_dirty) {
                Ok(rebuild) => rebuild,
                Err(error) => {
                    put_back(scene, written.previous);
                    return Err(VoxelChunkResidencyApplyError::ProjectionBuild(error));
                }
            };
        let dropped_hermite_normals = account(scene, &written.previous);
        scene.publish_local_change(&written.dirty, rebuild);
        Ok(receipt(scene, written, dropped_hermite_normals))
    }

    /// Apply operations as [`Self::apply`] does, writing the voxels now and
    /// rebuilding the changed chunks' meshes and colliders on a worker
    /// thread. Until the returned rebuild is installed, the scene's meshes
    /// and colliders stay as they were and its projection revisions name
    /// the revision before. Further deferred changes may follow before it
    /// installs; install the rebuilds in the order they were made, and
    /// before any other change to the scene.
    pub fn apply_deferred(
        scene: &mut VoxelCollisionScene,
        operations: &[VoxelChunkResidencyOperation],
    ) -> Result<(VoxelChunkResidencyReceipt, DeferredChunkRebuild), VoxelChunkResidencyApplyError>
    {
        let written = write(scene, operations)?;
        let dropped_hermite_normals = account(scene, &written.previous);
        scene
            .projected_revision
            .get_or_insert(written.revision_before);
        scene.source_revision = scene.source_revision.next();
        let snapshot = scene.rebuild_snapshot(&written.dirty, &written.field_dirty);
        let (changed, dirty, field_dirty) = (
            written.changed.clone(),
            written.dirty.clone(),
            written.field_dirty.clone(),
        );
        let build = move || snapshot.source().rebuild(&changed, &dirty, &field_dirty);
        let rebuild = DeferredChunkRebuild {
            accepted: scene.source_revision,
            synchronous_state: scene.synchronous_state,
            changed: written.changed.clone(),
            dirty: written.dirty.clone(),
            field_dirty: written.field_dirty.clone(),
            previous: first_previous(&written.previous),
            built: match std::thread::Builder::new()
                .name("rusty-voxel-residency".to_owned())
                .spawn(build)
            {
                Ok(worker) => Built::Building(worker),
                // No thread to spare: build here, as a synchronous change does.
                Err(_) => Built::Ready(scene.rebuild_chunks(
                    &written.changed,
                    &written.dirty,
                    &written.field_dirty,
                )),
            },
        };
        Ok((receipt(scene, written, dropped_hermite_normals), rebuild))
    }
}

/// A deferred residency change's meshes and colliders, building on a worker
/// thread ([`VoxelChunkResidencyService::apply_deferred`]).
pub struct DeferredChunkRebuild {
    /// The source revision the change wrote.
    accepted: VoxelSourceRevision,
    /// The scene's last synchronous rebuild when the change was made.
    synchronous_state: u64,
    changed: BTreeSet<ChunkCoord>,
    dirty: BTreeSet<ChunkCoord>,
    field_dirty: BTreeSet<ChunkCoord>,
    /// Each changed chunk as it was before, to put back if a mesh fails.
    previous: BTreeMap<ChunkCoord, Option<VoxelChunk>>,
    built: Built,
}

enum Built {
    Building(std::thread::JoinHandle<Result<ChunkRebuild, CollisionSceneError>>),
    Ready(Result<ChunkRebuild, CollisionSceneError>),
}

impl DeferredChunkRebuild {
    /// The chunks the change admitted, replaced or evicted.
    pub fn changed_chunks(&self) -> impl Iterator<Item = VoxelChunkIdentity> + '_ {
        self.changed
            .iter()
            .map(|coordinate| VoxelChunkIdentity::from(*coordinate))
    }

    /// Whether the meshes and colliders are built, so [`Self::install`]
    /// will not wait.
    pub fn is_built(&self) -> bool {
        match &self.built {
            Built::Building(worker) => worker.is_finished(),
            Built::Ready(_) => true,
        }
    }

    /// Wait for the meshes and colliders and install them, so the scene's
    /// projections show its voxels up to this change. Deferred changes made
    /// after it install after it, and theirs replace its chunks they share.
    /// A scene a synchronous change rebuilt since rebuilds this change's
    /// chunks here instead. A chunk whose mesh fails to build puts the
    /// change's chunks back as they were, as
    /// [`VoxelChunkResidencyService::apply`] does, unless a later change
    /// wrote over them, and returns the error.
    pub fn install(self, scene: &mut VoxelCollisionScene) -> Result<(), CollisionSceneError> {
        let built = match self.built {
            Built::Building(worker) => worker.join().expect("chunk meshing does not panic"),
            Built::Ready(built) => built,
        };
        // Changes after this one are still building.
        scene.projected_revision =
            (scene.source_revision != self.accepted).then_some(self.accepted);
        let built = if scene.synchronous_state == self.synchronous_state {
            built
        } else {
            scene.rebuild_chunks(&self.changed, &self.dirty, &self.field_dirty)
        };
        match built {
            Ok(rebuild) => {
                scene.publish_rebuild(&self.dirty, rebuild);
                Ok(())
            }
            Err(error) if scene.source_revision != self.accepted => Err(error),
            Err(error) => {
                for (coordinate, before) in self.previous {
                    scene.account_chunk(coordinate, None, false);
                    match before {
                        Some(chunk) => {
                            scene.account_chunk(coordinate, Some(&chunk), true);
                            scene.voxel_world.insert(coordinate, chunk);
                        }
                        None => {
                            scene.voxel_world.remove(coordinate);
                        }
                    }
                }
                scene.source_revision = scene.source_revision.next();
                Err(error)
            }
        }
    }
}

/// Operations written into a scene's voxels, before their chunks rebuild.
struct Written {
    revision_before: VoxelSourceRevision,
    /// The chunks each operation replaced, in order, for accounting and for
    /// putting back if a mesh fails to build.
    previous: Vec<(ChunkCoord, Option<VoxelChunk>)>,
    admitted: Vec<VoxelChunkIdentity>,
    replaced: Vec<VoxelChunkIdentity>,
    evicted: Vec<VoxelChunkIdentity>,
    retained: Vec<VoxelChunkIdentity>,
    changed: BTreeSet<ChunkCoord>,
    dirty: BTreeSet<ChunkCoord>,
    field_dirty: BTreeSet<ChunkCoord>,
}

/// Validate every operation, then write them in order. Invalid payloads
/// reject the batch before anything changes.
fn write(
    scene: &mut VoxelCollisionScene,
    operations: &[VoxelChunkResidencyOperation],
) -> Result<Written, VoxelChunkResidencyApplyError> {
    let chunk_size = scene.chunk_size;
    let grid_id = scene.voxel_world.grid().id();
    let options = scene.mesh_options.clone();
    let reconstructed = |slot: u16| options.surface(slot).mode != SurfaceMode::GreedyCubes;
    let mut validated = Vec::with_capacity(operations.len());
    for (operation_index, operation) in operations.iter().enumerate() {
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
        let contents = match operation {
            VoxelChunkResidencyOperation::Admit { payload, .. }
            | VoxelChunkResidencyOperation::Replace { payload, .. } => Some(validate_payload(
                operation_index,
                identity,
                chunk_size,
                grid_id,
                &reconstructed,
                payload,
            )?),
            VoxelChunkResidencyOperation::Evict { .. } => None,
        };
        validated.push((identity, contents));
    }

    let revision_before = scene.source_revision;
    let mut previous: Vec<(ChunkCoord, Option<VoxelChunk>)> = Vec::new();
    let (mut admitted, mut replaced, mut evicted, mut retained) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for (identity, contents) in validated {
        let coordinate = identity.to_chunk_coord();
        let current = scene.voxel_world.get(coordinate);
        match contents {
            Some(chunk) => {
                if current.is_some_and(|current| current.same_content(&chunk)) {
                    retained.push(identity);
                    continue;
                }
                if current.is_some() {
                    replaced.push(identity);
                } else {
                    admitted.push(identity);
                }
                previous.push((coordinate, scene.voxel_world.insert(coordinate, chunk)));
            }
            None => match scene.voxel_world.remove(coordinate) {
                Some(chunk) => {
                    evicted.push(identity);
                    previous.push((coordinate, Some(chunk)));
                }
                None => retained.push(identity),
            },
        }
    }
    if previous.is_empty() {
        return Err(VoxelChunkResidencyApplyError::Rejected(
            VoxelChunkResidencyRejection::NoChanges { retained },
        ));
    }

    let changed: BTreeSet<ChunkCoord> =
        previous.iter().map(|(coordinate, _)| *coordinate).collect();
    let dirty = changed
        .iter()
        .flat_map(|coordinate| scene.mesh_neighbourhood(*coordinate))
        .collect();
    let field_dirty = changed
        .iter()
        .flat_map(|coordinate| scene.field_neighbourhood(*coordinate))
        .collect();
    Ok(Written {
        revision_before,
        previous,
        admitted,
        replaced,
        evicted,
        retained,
        changed,
        dirty,
        field_dirty,
    })
}

/// Undo a write whose chunks did not rebuild.
fn put_back(scene: &mut VoxelCollisionScene, previous: Vec<(ChunkCoord, Option<VoxelChunk>)>) {
    for (coordinate, chunk) in previous.into_iter().rev() {
        match chunk {
            Some(chunk) => {
                scene.voxel_world.insert(coordinate, chunk);
            }
            None => {
                scene.voxel_world.remove(coordinate);
            }
        }
    }
}

/// Each changed chunk as it was before the first operation on it.
fn first_previous(
    previous: &[(ChunkCoord, Option<VoxelChunk>)],
) -> BTreeMap<ChunkCoord, Option<VoxelChunk>> {
    let mut first = BTreeMap::new();
    for (coordinate, chunk) in previous {
        first.entry(*coordinate).or_insert_with(|| chunk.clone());
    }
    first
}

/// Account each changed chunk once: out with what was there first, in with
/// what is there now. Returns the crossing normals the old chunks carried.
fn account(
    scene: &mut VoxelCollisionScene,
    previous: &[(ChunkCoord, Option<VoxelChunk>)],
) -> usize {
    let mut first = BTreeMap::new();
    for (coordinate, chunk) in previous {
        first.entry(*coordinate).or_insert(chunk);
    }
    let dropped_hermite_normals = first
        .values()
        .copied()
        .flatten()
        .map(VoxelChunk::edge_crossing_count)
        .sum();
    for (coordinate, before) in first {
        if let Some(before) = before {
            scene.account_chunk(coordinate, Some(before), false);
        }
        scene.account_chunk(coordinate, None, true);
    }
    dropped_hermite_normals
}

fn receipt(
    scene: &VoxelCollisionScene,
    written: Written,
    dropped_hermite_normals: usize,
) -> VoxelChunkResidencyReceipt {
    let update = &scene.mesh_update;
    // A deferred change has rebuilt nothing yet.
    let rebuilt = update.source_revision == scene.source_revision;
    VoxelChunkResidencyReceipt {
        revision_before: written.revision_before,
        accepted_revision: scene.source_revision,
        admitted: written.admitted,
        replaced: written.replaced,
        evicted: written.evicted,
        retained: written.retained,
        dirty_chunks: written
            .dirty
            .into_iter()
            .map(VoxelChunkIdentity::from)
            .collect(),
        resident_chunk_count: scene.resident_chunk_count(),
        resident_solid_voxel_count: scene.solid_voxel_count,
        authority_hash: scene.authority_hash,
        projections: scene.projection_revisions(),
        rebuilt_mesh_chunks: if rebuilt { update.rebuilt_chunks } else { 0 },
        reused_mesh_chunks: if rebuilt {
            update.reused_chunks
        } else {
            scene.mesh_chunks.len()
        },
        removed_mesh_chunks: if rebuilt { update.removed_chunks } else { 0 },
        dropped_hermite_normals,
    }
}

fn validate_payload(
    operation_index: usize,
    identity: VoxelChunkIdentity,
    chunk_size: u32,
    grid_id: core_space::GridId,
    reconstructed: &dyn Fn(u16) -> bool,
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
    if (!payload.states.is_empty() && payload.states.len() != expected_slot_count)
        || payload
            .states
            .iter()
            .zip(&payload.material_slots)
            .any(|(state, material)| {
                // Only greedy cube surfaces can mesh voxel states.
                core_voxel::VoxelState::from_raw(*state).is_none()
                    || (*state != 0 && (*material == 0 || reconstructed(*material)))
            })
    {
        return Err(VoxelChunkResidencyApplyError::Rejected(
            VoxelChunkResidencyRejection::InvalidCellStates { operation_index },
        ));
    }
    let values: Vec<_> = payload
        .material_slots
        .iter()
        .copied()
        .enumerate()
        .map(|(slot_index, material_slot)| {
            if material_slot == 0 {
                VoxelValue::EMPTY
            } else {
                VoxelValue::solid_raw(material_slot).with_state(
                    core_voxel::VoxelState::from_raw(
                        payload.states.get(slot_index).copied().unwrap_or(0),
                    )
                    .expect("validated state"),
                )
            }
        })
        .collect();
    if !payload.densities.is_empty()
        && (payload.densities.len() != expected_slot_count
            || payload.densities.iter().any(|density| !density.is_finite()))
    {
        return Err(VoxelChunkResidencyApplyError::Rejected(
            VoxelChunkResidencyRejection::InvalidDensities { operation_index },
        ));
    }
    let mut chunk = VoxelChunk::from_values(grid_id, dimensions, &values)
        .expect("validated payload length exactly matches dimensions");
    if !payload.densities.is_empty() {
        chunk
            .set_densities(Some(&payload.densities))
            .expect("validated densities match the chunk");
    }
    Ok(chunk)
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
