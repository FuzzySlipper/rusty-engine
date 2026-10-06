//! Local voxel edits. A batch is validated, written into the scene's chunks,
//! and then only the touched chunks' meshes and colliders are rebuilt. The
//! scene is never copied or rebuilt whole.
//!
//! The product owns undo: an edit batch reports what changed, and the product
//! can apply the inverse edits itself.

use std::collections::{BTreeMap, BTreeSet};

use crate::{CollisionSceneError, MaterialVoxel, SurfaceMode, VoxelCollisionScene};
use core_space::{ChunkCoord, VoxelCoord};
use core_voxel::{VoxelMaterialId, VoxelState, VoxelValue};
use serde::{Deserialize, Serialize};
use svc_volume::VoxelChunk;
/// Keeps chunk addressing and projection work in a reviewable world-space span.
pub const MAX_VOXEL_COORDINATE_ABS: i64 = 1_000_000;
/// Slot zero is empty and the bounded positive range is authored material data.
pub const MAX_VOXEL_MATERIAL_SLOT: u16 = 4_095;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoxelAuthorityValidationError {
    CoordinateOutOfBounds {
        address: [i64; 3],
        axis: usize,
        limit: i64,
    },
    InvalidState {
        state: u16,
    },
    InvalidMaterialSlot {
        material_slot: u16,
        maximum: u16,
    },
}

impl std::fmt::Display for VoxelAuthorityValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for VoxelAuthorityValidationError {}

/// Apply the same practical world bound to authored, imported, restored, and
/// live-edited voxel authority.
pub fn validate_voxel_address(address: [i64; 3]) -> Result<(), VoxelAuthorityValidationError> {
    for (axis, coordinate) in address.into_iter().enumerate() {
        if coordinate.unsigned_abs() > MAX_VOXEL_COORDINATE_ABS as u64 {
            return Err(VoxelAuthorityValidationError::CoordinateOutOfBounds {
                address,
                axis,
                limit: MAX_VOXEL_COORDINATE_ABS,
            });
        }
    }
    Ok(())
}

pub fn validate_voxel_material_slot(
    material_slot: u16,
) -> Result<(), VoxelAuthorityValidationError> {
    if !(1..=MAX_VOXEL_MATERIAL_SLOT).contains(&material_slot) {
        return Err(VoxelAuthorityValidationError::InvalidMaterialSlot {
            material_slot,
            maximum: MAX_VOXEL_MATERIAL_SLOT,
        });
    }
    Ok(())
}

pub fn validate_material_voxel(voxel: MaterialVoxel) -> Result<(), VoxelAuthorityValidationError> {
    validate_voxel_address(voxel.address)?;
    validate_voxel_material_slot(voxel.material_slot)?;
    if core_voxel::VoxelState::from_raw(voxel.state).is_none() {
        return Err(VoxelAuthorityValidationError::InvalidState { state: voxel.state });
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct VoxelSourceRevision(u64);

impl VoxelSourceRevision {
    pub const INITIAL: Self = Self(0);

    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }

    pub fn checked_next(self) -> Option<Self> {
        self.0.checked_add(1).map(Self)
    }

    pub(crate) const fn next(self) -> Self {
        Self(self.0.wrapping_add(1))
    }
}

/// The deliberately small operation family required by the first product proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum VoxelEdit {
    SetState {
        address: [i64; 3],
        material_slot: u16,
        state: u16,
    },
    Set {
        address: [i64; 3],
        material_slot: u16,
    },
    Clear {
        address: [i64; 3],
    },
}

impl VoxelEdit {
    pub const fn address(self) -> [i64; 3] {
        match self {
            Self::Set { address, .. }
            | Self::SetState { address, .. }
            | Self::Clear { address } => address,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoxelEditRejection {
    InvalidState {
        edit_index: usize,
        state: u16,
    },
    CoordinateOutOfBounds {
        edit_index: usize,
        address: [i64; 3],
        axis: usize,
        limit: i64,
    },
    InvalidMaterialSlot {
        edit_index: usize,
        material_slot: u16,
        maximum: u16,
    },
    /// Every edit already matched the scene.
    NoChanges,
}

impl std::fmt::Display for VoxelEditRejection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for VoxelEditRejection {}

#[derive(Debug)]
pub enum VoxelEditApplyError {
    Rejected(VoxelEditRejection),
    /// Rebuilding a touched chunk failed; the edit was reverted.
    ProjectionBuild(CollisionSceneError),
}

impl std::fmt::Display for VoxelEditApplyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rejected(rejection) => rejection.fmt(formatter),
            Self::ProjectionBuild(error) => write!(formatter, "projection rebuild failed: {error}"),
        }
    }
}

impl std::error::Error for VoxelEditApplyError {}

/// The source revision each projection reflects. Local changes update every
/// projection together, so they always match the scene's revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoxelProjectionRevisions {
    collision: VoxelSourceRevision,
    mesh: VoxelSourceRevision,
}

impl VoxelProjectionRevisions {
    pub const fn coherent(revision: VoxelSourceRevision) -> Self {
        Self {
            collision: revision,
            mesh: revision,
        }
    }

    pub const fn collision(self) -> VoxelSourceRevision {
        self.collision
    }

    pub const fn mesh(self) -> VoxelSourceRevision {
        self.mesh
    }

    pub const fn is_coherent_with(self, authority: VoxelSourceRevision) -> bool {
        self.collision.0 == authority.0 && self.mesh.0 == authority.0
    }
}

/// Typed gameplay/tooling consequence of one accepted transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoxelEditFact {
    pub revision: VoxelSourceRevision,
    pub changed_voxels: usize,
    pub changed_min: [i64; 3],
    pub changed_max_inclusive: [i64; 3],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoxelEditReceipt {
    pub revision_before: VoxelSourceRevision,
    pub accepted_revision: VoxelSourceRevision,
    pub solid_voxel_count: usize,
    pub authority_hash: u64,
    pub projections: VoxelProjectionRevisions,
    pub fact: VoxelEditFact,
    pub dirty_mesh_chunks: Vec<[i64; 3]>,
    pub rebuilt_mesh_chunks: usize,
    pub reused_mesh_chunks: usize,
    pub removed_mesh_chunks: usize,
}

type Cell = Option<(u16, u16)>;

struct Change {
    address: [i64; 3],
    before: Cell,
    after: Cell,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct VoxelEditService;

impl VoxelEditService {
    /// Apply a batch of edits. Invalid edits reject the batch before anything
    /// is written; a later edit to the same address wins.
    pub fn apply(
        scene: &mut VoxelCollisionScene,
        edits: &[VoxelEdit],
    ) -> Result<VoxelEditReceipt, VoxelEditApplyError> {
        let options = scene.mesh_options.clone();
        let reconstructed = |slot: u16| options.surface(slot).mode != SurfaceMode::GreedyCubes;
        let mut canonical = BTreeMap::new();
        for (edit_index, edit) in edits.iter().copied().enumerate() {
            validate_edit(edit_index, edit, &reconstructed)
                .map_err(VoxelEditApplyError::Rejected)?;
            canonical.insert(edit.address(), edit);
        }
        let changes: Vec<_> = canonical
            .into_iter()
            .filter_map(|(address, edit)| {
                let after = match edit {
                    VoxelEdit::Set { material_slot, .. } => Some((material_slot, 0)),
                    VoxelEdit::SetState {
                        material_slot,
                        state,
                        ..
                    } => Some((material_slot, state)),
                    VoxelEdit::Clear { .. } => None,
                };
                let before = scene
                    .material_voxel(address)
                    .map(|voxel| (voxel.material_slot, voxel.state));
                (before != after).then_some(Change {
                    address,
                    before,
                    after,
                })
            })
            .collect();
        if changes.is_empty() {
            return Err(VoxelEditApplyError::Rejected(VoxelEditRejection::NoChanges));
        }

        let revision_before = scene.source_revision;
        let grid = scene.voxel_world.grid();
        let mut created = Vec::new();
        for change in &changes {
            write_cell(scene, change.address, change.after, &mut created);
        }
        let mut changed_chunks = BTreeSet::new();
        let mut dirty = BTreeSet::new();
        for change in &changes {
            let voxel = VoxelCoord::new(change.address[0], change.address[1], change.address[2]);
            changed_chunks.insert(grid.voxel_to_chunk(voxel));
            dirty.extend(scene.mesh_neighbourhood_of_voxel(voxel));
        }
        let rebuild = match scene.rebuild_chunks(&changed_chunks, &dirty) {
            Ok(rebuild) => rebuild,
            Err(error) => {
                for change in changes.iter().rev() {
                    write_cell(scene, change.address, change.before, &mut Vec::new());
                }
                for coordinate in created {
                    scene.voxel_world.remove(coordinate);
                }
                return Err(VoxelEditApplyError::ProjectionBuild(error));
            }
        };
        for change in &changes {
            if let Some((material_slot, state)) = change.before {
                scene.account_voxel(material_voxel(change.address, material_slot, state), false);
            }
            if let Some((material_slot, state)) = change.after {
                scene.account_voxel(material_voxel(change.address, material_slot, state), true);
            }
        }
        scene.publish_local_change(&dirty, rebuild);

        let bound = |pick: fn(i64, i64) -> i64| {
            [0, 1, 2].map(|axis| {
                changes
                    .iter()
                    .map(|change| change.address[axis])
                    .reduce(pick)
                    .expect("at least one change")
            })
        };
        let update = &scene.mesh_update;
        Ok(VoxelEditReceipt {
            revision_before,
            accepted_revision: scene.source_revision,
            solid_voxel_count: scene.solid_voxel_count,
            authority_hash: scene.authority_hash,
            projections: scene.projection_revisions(),
            fact: VoxelEditFact {
                revision: scene.source_revision,
                changed_voxels: changes.len(),
                changed_min: bound(i64::min),
                changed_max_inclusive: bound(i64::max),
            },
            dirty_mesh_chunks: update.dirty_chunks.clone(),
            rebuilt_mesh_chunks: update.rebuilt_chunks,
            reused_mesh_chunks: update.reused_chunks,
            removed_mesh_chunks: update.removed_chunks,
        })
    }
}

fn validate_edit(
    edit_index: usize,
    edit: VoxelEdit,
    reconstructed: &dyn Fn(u16) -> bool,
) -> Result<(), VoxelEditRejection> {
    let address = edit.address();
    if let Err(VoxelAuthorityValidationError::CoordinateOutOfBounds { axis, limit, .. }) =
        validate_voxel_address(address)
    {
        return Err(VoxelEditRejection::CoordinateOutOfBounds {
            edit_index,
            address,
            axis,
            limit,
        });
    }
    if let VoxelEdit::Set { material_slot, .. } | VoxelEdit::SetState { material_slot, .. } = edit {
        if let Err(VoxelAuthorityValidationError::InvalidMaterialSlot { maximum, .. }) =
            validate_voxel_material_slot(material_slot)
        {
            return Err(VoxelEditRejection::InvalidMaterialSlot {
                edit_index,
                material_slot,
                maximum,
            });
        }
    }
    if let VoxelEdit::SetState {
        state,
        material_slot,
        ..
    } = edit
    {
        // Only greedy cube surfaces can mesh voxel states.
        if VoxelState::from_raw(state).is_none() || (reconstructed(material_slot) && state != 0) {
            return Err(VoxelEditRejection::InvalidState { edit_index, state });
        }
    }
    Ok(())
}

fn material_voxel(address: [i64; 3], material_slot: u16, state: u16) -> MaterialVoxel {
    MaterialVoxel {
        state,
        address,
        material_slot,
    }
}

/// Write one cell, creating its chunk when a solid voxel lands outside the
/// resident set.
fn write_cell(
    scene: &mut VoxelCollisionScene,
    address: [i64; 3],
    cell: Cell,
    created: &mut Vec<ChunkCoord>,
) {
    let grid = scene.voxel_world.grid();
    let (coordinate, local) =
        grid.voxel_to_chunk_local(VoxelCoord::new(address[0], address[1], address[2]));
    let value = match cell {
        Some((material_slot, state)) => VoxelValue::solid(VoxelMaterialId::new(material_slot))
            .with_state(VoxelState::from_raw(state).expect("validated state")),
        None => VoxelValue::EMPTY,
    };
    if scene.voxel_world.get(coordinate).is_none() {
        if cell.is_none() {
            return;
        }
        scene
            .voxel_world
            .insert(coordinate, VoxelChunk::from_spec(&grid));
        created.push(coordinate);
    }
    scene
        .voxel_world
        .get_mut(coordinate)
        .expect("resident chunk")
        .set(local, value)
        .expect("local coordinate from the grid");
}
