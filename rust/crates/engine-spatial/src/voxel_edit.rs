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

use std::collections::BTreeMap;

/// One UI or tool transaction cannot silently expand into unbounded work.
pub const MAX_VOXEL_EDITS_PER_TRANSACTION: usize = 4_096;
/// Keeps chunk addressing and projection work in a reviewable world-space span.
pub const MAX_VOXEL_COORDINATE_ABS: i64 = 1_000_000;
/// Slot zero is empty and the bounded positive range is authored material data.
pub const MAX_VOXEL_MATERIAL_SLOT: u16 = 4_095;

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

    fn next(self) -> Option<Self> {
        self.0.checked_add(1).map(Self)
    }
}

/// The deliberately small operation family required by the first product proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
}

impl std::fmt::Display for VoxelEditRejection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for VoxelEditRejection {}

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
}

/// Compact success evidence; concrete voxel authority remains on the scene.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoxelEditReceipt {
    pub revision_before: VoxelSourceRevision,
    pub accepted_revision: VoxelSourceRevision,
    pub solid_voxel_count: usize,
    pub authority_hash: u64,
    pub projections: VoxelProjectionRevisions,
    pub fact: VoxelEditFact,
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
            .next()
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
            for (axis, coordinate) in address.into_iter().enumerate() {
                if coordinate.unsigned_abs() > MAX_VOXEL_COORDINATE_ABS as u64 {
                    return Err(VoxelEditRejection::CoordinateOutOfBounds {
                        edit_index,
                        address,
                        axis,
                        limit: MAX_VOXEL_COORDINATE_ABS,
                    });
                }
            }
            if let VoxelEdit::Set { material_slot, .. } = edit {
                if !(1..=MAX_VOXEL_MATERIAL_SLOT).contains(&material_slot) {
                    return Err(VoxelEditRejection::InvalidMaterialSlot {
                        edit_index,
                        material_slot,
                        maximum: MAX_VOXEL_MATERIAL_SLOT,
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
