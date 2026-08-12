//! Bounded edit vocabulary owned by the successor's spatial service.
//!
//! An edit transaction names the source revision it observed. The service
//! validates the complete batch without mutation, rejects duplicate addresses,
//! and canonicalizes accepted edits by coordinate. A later commit can therefore
//! rebuild authority and every projection off to the side and swap one complete
//! [`crate::VoxelCollisionScene`] only after collision, navigation, and mesh all
//! exist at the accepted revision.
//!
//! Runtime snapshots store the accepted revision and concrete material voxels.
//! Explicit authored-project save materializes the same concrete authority; an
//! event stream, edit history, or generator recipe is never treated as the saved
//! state.

use std::collections::{BTreeMap, BTreeSet};

use crate::{CollisionSceneError, MaterialVoxel, SurfaceMode, VoxelCollisionScene};
use core_space::{ChunkCoord, VoxelCoord, VoxelGridSpec};
use serde::{Deserialize, Serialize};

/// One UI or tool transaction cannot silently expand into unbounded work.
pub const MAX_VOXEL_EDITS_PER_TRANSACTION: usize = 4_096;
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
    validate_voxel_material_slot(voxel.material_slot)
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
}

/// The deliberately small operation family required by the first product proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum VoxelEdit {
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
            Self::Set { address, .. } | Self::Clear { address } => address,
        }
    }
}

/// A caller must state which canonical authority revision it observed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoxelEditTransaction<'a> {
    pub expected_revision: VoxelSourceRevision,
    pub edits: &'a [VoxelEdit],
}

/// Fully validated and coordinate-ordered input. Callers can inspect this value,
/// but only [`VoxelEditService`] can create it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedVoxelEditTransaction {
    revision_before: VoxelSourceRevision,
    revision_after: VoxelSourceRevision,
    canonical_edits: Vec<VoxelEdit>,
}

impl ValidatedVoxelEditTransaction {
    pub const fn revision_before(&self) -> VoxelSourceRevision {
        self.revision_before
    }

    pub const fn revision_after(&self) -> VoxelSourceRevision {
        self.revision_after
    }

    pub fn canonical_edits(&self) -> &[VoxelEdit] {
        &self.canonical_edits
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoxelEditRejection {
    StaleRevision {
        expected: VoxelSourceRevision,
        actual: VoxelSourceRevision,
    },
    RevisionExhausted,
    EmptyTransaction,
    TooManyEdits {
        limit: usize,
        actual: usize,
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
    DuplicateAddress {
        first_index: usize,
        duplicate_index: usize,
        address: [i64; 3],
    },
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
    ProjectionBuild(CollisionSceneError),
    PreparedStateChanged {
        expected_revision: VoxelSourceRevision,
        actual_revision: VoxelSourceRevision,
        expected_hash: u64,
        actual_hash: u64,
    },
    PreparedStaticCollisionChanged {
        expected_revision: u64,
        actual_revision: u64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelEditDelta {
    pub address: [i64; 3],
    pub before_material: Option<u16>,
    pub after_material: Option<u16>,
}

/// Fully rebuilt candidate authority. Callers may inspect the exact projected
/// result before explicitly committing it; no projection or authority mutates
/// during preparation.
#[derive(Debug)]
pub struct PreparedVoxelEdit {
    expected_revision: VoxelSourceRevision,
    expected_hash: u64,
    expected_static_collision_revision: u64,
    candidate: VoxelCollisionScene,
    receipt: VoxelEditReceipt,
    deltas: Vec<VoxelEditDelta>,
    canonical_edits: Vec<VoxelEdit>,
}

impl PreparedVoxelEdit {
    pub const fn receipt(&self) -> &VoxelEditReceipt {
        &self.receipt
    }

    pub fn deltas(&self) -> &[VoxelEditDelta] {
        &self.deltas
    }

    pub fn canonical_edits(&self) -> &[VoxelEdit] {
        &self.canonical_edits
    }

    /// Exact signed chunk coordinates whose retained mesh may create, replace,
    /// or disappear when this candidate commits.
    pub fn dirty_mesh_chunks(&self) -> &[[i64; 3]] {
        &self.receipt.dirty_mesh_chunks
    }
}

impl std::fmt::Display for VoxelEditApplyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rejected(rejection) => rejection.fmt(formatter),
            Self::ProjectionBuild(error) => write!(formatter, "projection rebuild failed: {error}"),
            Self::PreparedStateChanged { .. } => {
                write!(formatter, "voxel authority changed after preview")
            }
            Self::PreparedStaticCollisionChanged { .. } => {
                write!(
                    formatter,
                    "static collision projection changed after preview"
                )
            }
        }
    }
}

impl std::error::Error for VoxelEditApplyError {}

/// Evidence that every derived consumer was built from one accepted authority.
/// The only constructor fills every projection from the same revision, so this
/// value cannot represent a mixed state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoxelProjectionRevisions {
    collision: VoxelSourceRevision,
    navigation: VoxelSourceRevision,
    mesh: VoxelSourceRevision,
}

impl VoxelProjectionRevisions {
    pub const fn coherent(revision: VoxelSourceRevision) -> Self {
        Self {
            collision: revision,
            navigation: revision,
            mesh: revision,
        }
    }

    pub const fn collision(self) -> VoxelSourceRevision {
        self.collision
    }

    pub const fn navigation(self) -> VoxelSourceRevision {
        self.navigation
    }

    pub const fn mesh(self) -> VoxelSourceRevision {
        self.mesh
    }

    pub const fn is_coherent_with(self, authority: VoxelSourceRevision) -> bool {
        self.collision.0 == authority.0
            && self.navigation.0 == authority.0
            && self.mesh.0 == authority.0
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

/// Compact success evidence; concrete voxel authority remains on the scene.
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

/// The sole owner of live voxel transaction validation and, in the next slice,
/// authoritative apply/rebuild/swap.
#[derive(Debug, Default, Clone, Copy)]
pub struct VoxelEditService;

impl VoxelEditService {
    pub fn validate_transaction(
        current_revision: VoxelSourceRevision,
        transaction: VoxelEditTransaction<'_>,
    ) -> Result<ValidatedVoxelEditTransaction, VoxelEditRejection> {
        if transaction.expected_revision != current_revision {
            return Err(VoxelEditRejection::StaleRevision {
                expected: transaction.expected_revision,
                actual: current_revision,
            });
        }
        let revision_after = current_revision
            .checked_next()
            .ok_or(VoxelEditRejection::RevisionExhausted)?;
        if transaction.edits.is_empty() {
            return Err(VoxelEditRejection::EmptyTransaction);
        }
        if transaction.edits.len() > MAX_VOXEL_EDITS_PER_TRANSACTION {
            return Err(VoxelEditRejection::TooManyEdits {
                limit: MAX_VOXEL_EDITS_PER_TRANSACTION,
                actual: transaction.edits.len(),
            });
        }

        let mut by_address = BTreeMap::new();
        for (edit_index, edit) in transaction.edits.iter().copied().enumerate() {
            let address = edit.address();
            if let Err(VoxelAuthorityValidationError::CoordinateOutOfBounds {
                axis, limit, ..
            }) = validate_voxel_address(address)
            {
                return Err(VoxelEditRejection::CoordinateOutOfBounds {
                    edit_index,
                    address,
                    axis,
                    limit,
                });
            }
            if let VoxelEdit::Set { material_slot, .. } = edit {
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
            if let Some((first_index, _)) = by_address.insert(address, (edit_index, edit)) {
                return Err(VoxelEditRejection::DuplicateAddress {
                    first_index,
                    duplicate_index: edit_index,
                    address,
                });
            }
        }

        Ok(ValidatedVoxelEditTransaction {
            revision_before: current_revision,
            revision_after,
            canonical_edits: by_address.into_values().map(|(_, edit)| edit).collect(),
        })
    }

    /// Validate, derive, and rebuild one complete coherent scene without
    /// mutating the source authority.
    pub fn preview(
        scene: &VoxelCollisionScene,
        transaction: VoxelEditTransaction<'_>,
    ) -> Result<PreparedVoxelEdit, VoxelEditApplyError> {
        let accepted = Self::validate_transaction(scene.source_revision, transaction)
            .map_err(VoxelEditApplyError::Rejected)?;
        let mut materials: BTreeMap<[i64; 3], u16> = scene
            .material_voxels
            .iter()
            .map(|voxel| (voxel.address, voxel.material_slot))
            .collect();
        let mut deltas = Vec::new();
        for edit in accepted.canonical_edits.iter().copied() {
            match edit {
                VoxelEdit::Set {
                    address,
                    material_slot,
                } => {
                    let before_material = materials.get(&address).copied();
                    if before_material != Some(material_slot) {
                        materials.insert(address, material_slot);
                        deltas.push(VoxelEditDelta {
                            address,
                            before_material,
                            after_material: Some(material_slot),
                        });
                    }
                }
                VoxelEdit::Clear { address } => {
                    if let Some(before_material) = materials.remove(&address) {
                        deltas.push(VoxelEditDelta {
                            address,
                            before_material: Some(before_material),
                            after_material: None,
                        });
                    }
                }
            }
        }
        if deltas.is_empty() {
            return Err(VoxelEditApplyError::Rejected(VoxelEditRejection::NoChanges));
        }

        let grid = scene.voxel_world.grid();
        let old_resident: BTreeSet<_> = scene
            .voxel_world
            .resident_chunks()
            .map(|(coordinate, _)| coordinate)
            .collect();
        let new_resident: BTreeSet<_> = materials
            .keys()
            .map(|address| grid.voxel_to_chunk(VoxelCoord::new(address[0], address[1], address[2])))
            .collect();
        let dirty_mesh_chunks = derive_dirty_mesh_chunks(
            grid,
            scene.mesh_options.mode,
            deltas.iter().map(|delta| delta.address),
            &old_resident,
            &new_resident,
        );
        let material_voxels = materials
            .into_iter()
            .map(|(address, material_slot)| MaterialVoxel {
                address,
                material_slot,
            });
        let mut rebuilt = VoxelCollisionScene::build_at_revision(
            scene.voxel_size,
            scene.chunk_size,
            material_voxels,
            None,
            accepted.revision_after,
            scene.mesh_options,
            Some((&scene.mesh_chunks, &dirty_mesh_chunks)),
        )
        .map_err(VoxelEditApplyError::ProjectionBuild)?;
        rebuilt.preserve_static_mesh_projection_from(scene);
        let changed_min = [0, 1, 2].map(|axis| {
            deltas
                .iter()
                .map(|delta| delta.address[axis])
                .min()
                .expect("at least one changed voxel")
        });
        let changed_max_inclusive = [0, 1, 2].map(|axis| {
            deltas
                .iter()
                .map(|delta| delta.address[axis])
                .max()
                .expect("at least one changed voxel")
        });
        let fact = VoxelEditFact {
            revision: accepted.revision_after,
            changed_voxels: deltas.len(),
            changed_min,
            changed_max_inclusive,
        };
        let receipt = VoxelEditReceipt {
            revision_before: accepted.revision_before,
            accepted_revision: accepted.revision_after,
            solid_voxel_count: rebuilt.solid_voxel_count(),
            authority_hash: rebuilt.authority_hash(),
            projections: rebuilt.projection_revisions(),
            fact,
            dirty_mesh_chunks: rebuilt.mesh_update.dirty_chunks.clone(),
            rebuilt_mesh_chunks: rebuilt.mesh_update.rebuilt_chunks,
            reused_mesh_chunks: rebuilt.mesh_update.reused_chunks,
            removed_mesh_chunks: rebuilt.mesh_update.removed_chunks,
        };
        debug_assert!(receipt
            .projections
            .is_coherent_with(receipt.accepted_revision));
        Ok(PreparedVoxelEdit {
            expected_revision: scene.source_revision,
            expected_hash: scene.authority_hash,
            expected_static_collision_revision: scene.static_mesh_collision_revision(),
            candidate: rebuilt,
            receipt,
            deltas,
            canonical_edits: accepted.canonical_edits,
        })
    }

    /// Commit a previously prepared complete authority only if its source scene
    /// is still exactly the one that was previewed.
    pub fn commit(
        scene: &mut VoxelCollisionScene,
        prepared: PreparedVoxelEdit,
    ) -> Result<VoxelEditReceipt, VoxelEditApplyError> {
        if scene.source_revision != prepared.expected_revision
            || scene.authority_hash != prepared.expected_hash
        {
            return Err(VoxelEditApplyError::PreparedStateChanged {
                expected_revision: prepared.expected_revision,
                actual_revision: scene.source_revision,
                expected_hash: prepared.expected_hash,
                actual_hash: scene.authority_hash,
            });
        }
        let actual_static_collision_revision = scene.static_mesh_collision_revision();
        if actual_static_collision_revision != prepared.expected_static_collision_revision {
            return Err(VoxelEditApplyError::PreparedStaticCollisionChanged {
                expected_revision: prepared.expected_static_collision_revision,
                actual_revision: actual_static_collision_revision,
            });
        }
        let receipt = prepared.receipt.clone();
        *scene = prepared.candidate;
        Ok(receipt)
    }

    /// Validate, derive, rebuild, then swap one complete coherent scene. The
    /// input scene remains unchanged if validation or any projection build fails.
    pub fn apply(
        scene: &mut VoxelCollisionScene,
        transaction: VoxelEditTransaction<'_>,
    ) -> Result<VoxelEditReceipt, VoxelEditApplyError> {
        let prepared = Self::preview(scene, transaction)?;
        Self::commit(scene, prepared)
    }
}

fn derive_dirty_mesh_chunks(
    grid: VoxelGridSpec,
    surface_mode: SurfaceMode,
    addresses: impl IntoIterator<Item = [i64; 3]>,
    old_resident: &BTreeSet<ChunkCoord>,
    new_resident: &BTreeSet<ChunkCoord>,
) -> BTreeSet<ChunkCoord> {
    let [width, height, depth] = grid.chunk_dims().to_array();
    let mut dirty = BTreeSet::new();
    for address in addresses {
        let (owner, local) =
            grid.voxel_to_chunk_local(VoxelCoord::new(address[0], address[1], address[2]));
        let mut x_offsets = vec![0];
        let mut y_offsets = vec![0];
        let mut z_offsets = vec![0];
        if local.x == 0 {
            x_offsets.push(-1);
        }
        if local.x + 1 == width {
            x_offsets.push(1);
        }
        if local.y == 0 {
            y_offsets.push(-1);
        }
        if local.y + 1 == height {
            y_offsets.push(1);
        }
        if local.z == 0 {
            z_offsets.push(-1);
        }
        if local.z + 1 == depth {
            z_offsets.push(1);
        }
        if surface_mode == SurfaceMode::GreedyCubes {
            dirty.insert(owner);
            for (x, y, z) in x_offsets
                .iter()
                .skip(1)
                .map(|x| (*x, 0, 0))
                .chain(y_offsets.iter().skip(1).map(|y| (0, *y, 0)))
                .chain(z_offsets.iter().skip(1).map(|z| (0, 0, *z)))
            {
                let candidate = ChunkCoord::new(owner.x + x, owner.y + y, owner.z + z);
                if old_resident.contains(&candidate) || new_resident.contains(&candidate) {
                    dirty.insert(candidate);
                }
            }
        } else {
            // Reconstructed samples cross face, edge, and corner boundaries,
            // so every occupied combination in the one-chunk halo is exact.
            for x in &x_offsets {
                for y in &y_offsets {
                    for z in &z_offsets {
                        let candidate = ChunkCoord::new(owner.x + x, owner.y + y, owner.z + z);
                        if candidate == owner
                            || old_resident.contains(&candidate)
                            || new_resident.contains(&candidate)
                        {
                            dirty.insert(candidate);
                        }
                    }
                }
            }
        }
    }
    dirty
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepted_edits_are_coordinate_ordered_and_advance_once() {
        let edits = [
            VoxelEdit::Clear { address: [2, 0, 0] },
            VoxelEdit::Set {
                address: [-1, 4, 3],
                material_slot: 9,
            },
        ];

        let accepted = VoxelEditService::validate_transaction(
            VoxelSourceRevision::new(7),
            VoxelEditTransaction {
                expected_revision: VoxelSourceRevision::new(7),
                edits: &edits,
            },
        )
        .unwrap();

        assert_eq!(accepted.revision_before().raw(), 7);
        assert_eq!(accepted.revision_after().raw(), 8);
        assert_eq!(accepted.canonical_edits(), &[edits[1], edits[0]]);
    }

    #[test]
    fn stale_duplicate_and_invalid_batches_fail_as_one_validation_unit() {
        let duplicate = [
            VoxelEdit::Clear { address: [1, 2, 3] },
            VoxelEdit::Set {
                address: [1, 2, 3],
                material_slot: 1,
            },
        ];
        assert!(matches!(
            validate(VoxelSourceRevision::INITIAL, &duplicate),
            Err(VoxelEditRejection::DuplicateAddress {
                first_index: 0,
                duplicate_index: 1,
                ..
            })
        ));
        assert!(matches!(
            VoxelEditService::validate_transaction(
                VoxelSourceRevision::new(2),
                VoxelEditTransaction {
                    expected_revision: VoxelSourceRevision::new(1),
                    edits: &duplicate,
                }
            ),
            Err(VoxelEditRejection::StaleRevision { .. })
        ));
        assert!(matches!(
            validate(
                VoxelSourceRevision::INITIAL,
                &[VoxelEdit::Set {
                    address: [0, 0, 0],
                    material_slot: 0,
                }]
            ),
            Err(VoxelEditRejection::InvalidMaterialSlot { edit_index: 0, .. })
        ));
        assert!(matches!(
            validate(
                VoxelSourceRevision::INITIAL,
                &[VoxelEdit::Clear {
                    address: [i64::MIN, 0, 0],
                }]
            ),
            Err(VoxelEditRejection::CoordinateOutOfBounds {
                edit_index: 0,
                axis: 0,
                ..
            })
        ));
    }

    #[test]
    fn empty_oversized_and_exhausted_requests_are_rejected() {
        assert_eq!(
            validate(VoxelSourceRevision::INITIAL, &[]),
            Err(VoxelEditRejection::EmptyTransaction)
        );
        let oversized =
            vec![VoxelEdit::Clear { address: [0, 0, 0] }; MAX_VOXEL_EDITS_PER_TRANSACTION + 1];
        assert!(matches!(
            validate(VoxelSourceRevision::INITIAL, &oversized),
            Err(VoxelEditRejection::TooManyEdits { .. })
        ));
        assert_eq!(
            validate(
                VoxelSourceRevision::new(u64::MAX),
                &[VoxelEdit::Clear { address: [0, 0, 0] }]
            ),
            Err(VoxelEditRejection::RevisionExhausted)
        );
    }

    fn validate(
        revision: VoxelSourceRevision,
        edits: &[VoxelEdit],
    ) -> Result<ValidatedVoxelEditTransaction, VoxelEditRejection> {
        VoxelEditService::validate_transaction(
            revision,
            VoxelEditTransaction {
                expected_revision: revision,
                edits,
            },
        )
    }
}
