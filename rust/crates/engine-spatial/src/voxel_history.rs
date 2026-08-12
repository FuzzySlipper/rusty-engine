use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{
    CollisionSceneError, MaterialVoxel, SurfaceMeshOptions, VoxelCollisionScene, VoxelEdit,
    VoxelEditApplyError, VoxelEditDelta, VoxelEditReceipt, VoxelEditService, VoxelEditTransaction,
    VoxelSourceRevision,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoxelEditHistoryLimits {
    pub max_entries: usize,
    pub max_retained_deltas: usize,
    pub max_reconstruction_deltas: usize,
    pub max_diff_samples: usize,
}

impl Default for VoxelEditHistoryLimits {
    fn default() -> Self {
        Self {
            max_entries: 10_000,
            max_retained_deltas: 100_000,
            max_reconstruction_deltas: 100_000,
            max_diff_samples: 4_096,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelEditHistoryEntry {
    pub transaction_id: u64,
    pub parent_transaction_id: Option<u64>,
    pub before_hash: u64,
    pub after_hash: u64,
    pub deltas: Vec<VoxelEditDelta>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoxelEditHistoryCursor {
    pub index: usize,
    pub applied_transaction_id: Option<u64>,
    pub undo_depth: usize,
    pub redo_depth: usize,
    pub authority_hash: u64,
    pub history_hash: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoxelEditHistoryAppendReceipt {
    pub entry: VoxelEditHistoryEntry,
    pub edit: VoxelEditReceipt,
    pub cursor_before: VoxelEditHistoryCursor,
    pub cursor_after: VoxelEditHistoryCursor,
    pub invalidated_redo_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoxelEditHistoryBounds {
    pub min: [i64; 3],
    pub max: [i64; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct VoxelEditHistoryMaterialDelta {
    pub before_material: Option<u16>,
    pub after_material: Option<u16>,
    pub changed_voxels: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoxelEditHistoryDiffOptions {
    pub max_samples: usize,
}

impl Default for VoxelEditHistoryDiffOptions {
    fn default() -> Self {
        Self {
            max_samples: VoxelEditHistoryLimits::default().max_diff_samples,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoxelEditHistoryDiffSummary {
    pub changed_voxels: usize,
    pub bounds: Option<VoxelEditHistoryBounds>,
    pub material_deltas: Vec<VoxelEditHistoryMaterialDelta>,
    pub samples: Vec<VoxelEditDelta>,
    pub samples_truncated: bool,
    pub included_transaction_ids: Vec<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoxelEditHistoryRevertReceipt {
    pub applied: bool,
    pub cursor_before: VoxelEditHistoryCursor,
    pub cursor_after: VoxelEditHistoryCursor,
    pub diff: VoxelEditHistoryDiffSummary,
    pub revision_before: VoxelSourceRevision,
    pub revision_after: VoxelSourceRevision,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoxelEditHistoryResetReceipt {
    pub cursor_before: VoxelEditHistoryCursor,
    pub invalidated_entries: usize,
    pub invalidated_redo_entries: usize,
    pub authority_hash_after: u64,
    pub source_revision_after: VoxelSourceRevision,
}

#[derive(Debug)]
pub struct PreparedVoxelHistoryRevert {
    expected_revision: VoxelSourceRevision,
    expected_hash: u64,
    expected_static_collision_revision: u64,
    target_cursor: usize,
    candidate: VoxelCollisionScene,
    receipt: VoxelEditHistoryRevertReceipt,
}

impl PreparedVoxelHistoryRevert {
    pub fn receipt(&self) -> &VoxelEditHistoryRevertReceipt {
        &self.receipt
    }
}

#[derive(Debug)]
pub enum VoxelEditHistoryError {
    SceneShapeMismatch,
    StaleAuthority {
        expected: u64,
        actual: u64,
    },
    StaleRevision {
        expected: VoxelSourceRevision,
        actual: VoxelSourceRevision,
    },
    StaleStaticCollision {
        expected_revision: u64,
        actual_revision: u64,
    },
    InvalidCursor {
        requested: usize,
        entry_count: usize,
    },
    UnknownTransaction(u64),
    EmptyUndoStack,
    EmptyRedoStack,
    EntryQuotaExceeded {
        limit: usize,
        actual: usize,
    },
    DeltaQuotaExceeded {
        limit: usize,
        actual: usize,
    },
    ReconstructionQuotaExceeded {
        limit: usize,
        actual: usize,
    },
    RevisionExhausted,
    TransactionIdExhausted,
    CorruptEntry {
        transaction_id: u64,
        address: [i64; 3],
        expected_material: Option<u16>,
        actual_material: Option<u16>,
    },
    Edit(VoxelEditApplyError),
    Rebuild(CollisionSceneError),
}

impl std::fmt::Display for VoxelEditHistoryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for VoxelEditHistoryError {}

#[derive(Debug)]
pub struct VoxelEditHistory {
    pub(crate) base_voxel_size: f64,
    pub(crate) base_chunk_size: u32,
    pub(crate) base_voxels: Vec<MaterialVoxel>,
    pub(crate) base_resident_chunks: Vec<[i64; 3]>,
    pub(crate) base_mesh_options: SurfaceMeshOptions,
    pub(crate) base_hash: u64,
    pub(crate) entries: Vec<VoxelEditHistoryEntry>,
    pub(crate) cursor_index: usize,
    pub(crate) next_transaction_id: u64,
    pub(crate) source_revision: VoxelSourceRevision,
    pub(crate) limits: VoxelEditHistoryLimits,
}

#[derive(Debug)]
pub(crate) struct VoxelEditHistoryParts {
    pub base_voxel_size: f64,
    pub base_chunk_size: u32,
    pub base_voxels: Vec<MaterialVoxel>,
    pub base_resident_chunks: Vec<[i64; 3]>,
    pub base_mesh_options: SurfaceMeshOptions,
    pub base_hash: u64,
    pub entries: Vec<VoxelEditHistoryEntry>,
    pub cursor_index: usize,
    pub next_transaction_id: u64,
    pub source_revision: VoxelSourceRevision,
}

impl VoxelEditHistory {
    pub fn new(scene: &VoxelCollisionScene) -> Self {
        Self::with_limits(scene, VoxelEditHistoryLimits::default())
    }

    pub fn with_limits(scene: &VoxelCollisionScene, limits: VoxelEditHistoryLimits) -> Self {
        Self {
            base_voxel_size: scene.voxel_size(),
            base_chunk_size: scene.chunk_size(),
            base_voxels: scene.material_voxels().to_vec(),
            base_resident_chunks: scene.resident_chunk_coordinates(),
            base_mesh_options: scene.mesh_options(),
            base_hash: scene.authority_hash(),
            entries: Vec::new(),
            cursor_index: 0,
            next_transaction_id: 1,
            source_revision: scene.source_revision(),
            limits,
        }
    }

    pub fn entries(&self) -> &[VoxelEditHistoryEntry] {
        &self.entries
    }

    pub const fn limits(&self) -> VoxelEditHistoryLimits {
        self.limits
    }

    pub fn cursor(&self) -> VoxelEditHistoryCursor {
        self.cursor_at(self.cursor_index)
    }

    pub fn reset_to_scene(&mut self, scene: &VoxelCollisionScene) -> VoxelEditHistoryResetReceipt {
        let cursor_before = self.cursor();
        let invalidated_entries = self.entries.len();
        let invalidated_redo_entries = self.entries.len().saturating_sub(self.cursor_index);
        let limits = self.limits;
        *self = Self::with_limits(scene, limits);
        VoxelEditHistoryResetReceipt {
            cursor_before,
            invalidated_entries,
            invalidated_redo_entries,
            authority_hash_after: scene.authority_hash(),
            source_revision_after: scene.source_revision(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn apply(
        &mut self,
        scene: &mut VoxelCollisionScene,
        edits: &[VoxelEdit],
    ) -> Result<VoxelEditHistoryAppendReceipt, VoxelEditHistoryError> {
        self.ensure_scene_at_cursor(scene)?;
        let next_transaction_id = self
            .next_transaction_id
            .checked_add(1)
            .ok_or(VoxelEditHistoryError::TransactionIdExhausted)?;
        let actual_entries = self.cursor_index.saturating_add(1);
        if actual_entries > self.limits.max_entries {
            return Err(VoxelEditHistoryError::EntryQuotaExceeded {
                limit: self.limits.max_entries,
                actual: actual_entries,
            });
        }
        let retained_prefix = self.entries[..self.cursor_index]
            .iter()
            .map(|entry| entry.deltas.len())
            .sum::<usize>();
        if retained_prefix.saturating_add(edits.len()) > self.limits.max_retained_deltas {
            return Err(VoxelEditHistoryError::DeltaQuotaExceeded {
                limit: self.limits.max_retained_deltas,
                actual: retained_prefix.saturating_add(edits.len()),
            });
        }

        let prepared = VoxelEditService::preview(
            scene,
            VoxelEditTransaction {
                expected_revision: scene.source_revision(),
                edits,
            },
        )
        .map_err(VoxelEditHistoryError::Edit)?;
        let prospective_deltas = retained_prefix.saturating_add(prepared.deltas().len());
        if prospective_deltas > self.limits.max_retained_deltas {
            return Err(VoxelEditHistoryError::DeltaQuotaExceeded {
                limit: self.limits.max_retained_deltas,
                actual: prospective_deltas,
            });
        }
        let cursor_before = self.cursor();
        let invalidated_redo_count = self.entries.len().saturating_sub(self.cursor_index);
        let parent_transaction_id = self
            .cursor_index
            .checked_sub(1)
            .and_then(|index| self.entries.get(index))
            .map(|entry| entry.transaction_id);
        let entry = VoxelEditHistoryEntry {
            transaction_id: self.next_transaction_id,
            parent_transaction_id,
            before_hash: cursor_before.authority_hash,
            after_hash: prepared.receipt().authority_hash,
            deltas: prepared.deltas().to_vec(),
        };
        let edit =
            VoxelEditService::commit(scene, prepared).map_err(VoxelEditHistoryError::Edit)?;
        debug_assert_eq!(entry.after_hash, edit.authority_hash);

        self.entries.truncate(self.cursor_index);
        self.entries.push(entry.clone());
        self.cursor_index += 1;
        self.next_transaction_id = next_transaction_id;
        self.source_revision = scene.source_revision();
        Ok(VoxelEditHistoryAppendReceipt {
            entry,
            edit,
            cursor_before,
            cursor_after: self.cursor(),
            invalidated_redo_count,
        })
    }

    pub fn preview_revert_to_cursor(
        &self,
        scene: &VoxelCollisionScene,
        target_cursor: usize,
        options: VoxelEditHistoryDiffOptions,
    ) -> Result<PreparedVoxelHistoryRevert, VoxelEditHistoryError> {
        self.ensure_scene_at_cursor(scene)?;
        if target_cursor > self.entries.len() {
            return Err(VoxelEditHistoryError::InvalidCursor {
                requested: target_cursor,
                entry_count: self.entries.len(),
            });
        }
        let reconstruction_deltas = self.entries[..target_cursor]
            .iter()
            .map(|entry| entry.deltas.len())
            .sum::<usize>();
        if reconstruction_deltas > self.limits.max_reconstruction_deltas {
            return Err(VoxelEditHistoryError::ReconstructionQuotaExceeded {
                limit: self.limits.max_reconstruction_deltas,
                actual: reconstruction_deltas,
            });
        }
        let target_map = self.materials_at_cursor(target_cursor)?;
        let target_revision = scene
            .source_revision()
            .checked_next()
            .ok_or(VoxelEditHistoryError::RevisionExhausted)?;
        let mut candidate = VoxelCollisionScene::from_material_voxels_at_revision_with_residents(
            self.base_voxel_size,
            self.base_chunk_size,
            material_voxels(&target_map),
            self.base_resident_chunks.iter().copied(),
            target_revision,
            self.base_mesh_options,
            None,
        )
        .map_err(VoxelEditHistoryError::Rebuild)?;
        candidate.preserve_static_mesh_projection_from(scene);
        let current_map = material_map(scene.material_voxels());
        let diff = summarize_diff(
            &current_map,
            &target_map,
            transaction_ids_between(&self.entries, self.cursor_index, target_cursor),
            options.max_samples.min(self.limits.max_diff_samples),
        );
        let receipt = VoxelEditHistoryRevertReceipt {
            applied: false,
            cursor_before: self.cursor(),
            cursor_after: self.cursor_at(target_cursor),
            diff,
            revision_before: scene.source_revision(),
            revision_after: target_revision,
        };
        Ok(PreparedVoxelHistoryRevert {
            expected_revision: scene.source_revision(),
            expected_hash: scene.authority_hash(),
            expected_static_collision_revision: scene.static_mesh_collision_revision(),
            target_cursor,
            candidate,
            receipt,
        })
    }

    pub fn commit_revert(
        &mut self,
        scene: &mut VoxelCollisionScene,
        mut prepared: PreparedVoxelHistoryRevert,
    ) -> Result<VoxelEditHistoryRevertReceipt, VoxelEditHistoryError> {
        self.ensure_scene_at_cursor(scene)?;
        if scene.source_revision() != prepared.expected_revision
            || scene.authority_hash() != prepared.expected_hash
        {
            return Err(VoxelEditHistoryError::StaleAuthority {
                expected: prepared.expected_hash,
                actual: scene.authority_hash(),
            });
        }
        let actual_static_collision_revision = scene.static_mesh_collision_revision();
        if actual_static_collision_revision != prepared.expected_static_collision_revision {
            return Err(VoxelEditHistoryError::StaleStaticCollision {
                expected_revision: prepared.expected_static_collision_revision,
                actual_revision: actual_static_collision_revision,
            });
        }
        *scene = prepared.candidate;
        self.cursor_index = prepared.target_cursor;
        self.source_revision = scene.source_revision();
        prepared.receipt.applied = true;
        prepared.receipt.cursor_after = self.cursor();
        Ok(prepared.receipt)
    }

    pub fn apply_revert_to_cursor(
        &mut self,
        scene: &mut VoxelCollisionScene,
        target_cursor: usize,
        options: VoxelEditHistoryDiffOptions,
    ) -> Result<VoxelEditHistoryRevertReceipt, VoxelEditHistoryError> {
        let prepared = self.preview_revert_to_cursor(scene, target_cursor, options)?;
        self.commit_revert(scene, prepared)
    }

    pub fn apply_revert_to_transaction(
        &mut self,
        scene: &mut VoxelCollisionScene,
        transaction_id: u64,
        options: VoxelEditHistoryDiffOptions,
    ) -> Result<VoxelEditHistoryRevertReceipt, VoxelEditHistoryError> {
        let target = self
            .entries
            .iter()
            .position(|entry| entry.transaction_id == transaction_id)
            .map(|index| index + 1)
            .ok_or(VoxelEditHistoryError::UnknownTransaction(transaction_id))?;
        self.apply_revert_to_cursor(scene, target, options)
    }

    pub fn undo_one(
        &mut self,
        scene: &mut VoxelCollisionScene,
    ) -> Result<VoxelEditHistoryRevertReceipt, VoxelEditHistoryError> {
        let target = self
            .cursor_index
            .checked_sub(1)
            .ok_or(VoxelEditHistoryError::EmptyUndoStack)?;
        self.apply_revert_to_cursor(scene, target, VoxelEditHistoryDiffOptions::default())
    }

    pub fn redo_one(
        &mut self,
        scene: &mut VoxelCollisionScene,
    ) -> Result<VoxelEditHistoryRevertReceipt, VoxelEditHistoryError> {
        if self.cursor_index >= self.entries.len() {
            return Err(VoxelEditHistoryError::EmptyRedoStack);
        }
        self.apply_revert_to_cursor(
            scene,
            self.cursor_index + 1,
            VoxelEditHistoryDiffOptions::default(),
        )
    }

    pub(crate) fn from_parts(parts: VoxelEditHistoryParts, limits: VoxelEditHistoryLimits) -> Self {
        Self {
            base_voxel_size: parts.base_voxel_size,
            base_chunk_size: parts.base_chunk_size,
            base_voxels: parts.base_voxels,
            base_resident_chunks: parts.base_resident_chunks,
            base_mesh_options: parts.base_mesh_options,
            base_hash: parts.base_hash,
            entries: parts.entries,
            cursor_index: parts.cursor_index,
            next_transaction_id: parts.next_transaction_id,
            source_revision: parts.source_revision,
            limits,
        }
    }

    pub(crate) fn materials_at_cursor(
        &self,
        cursor: usize,
    ) -> Result<BTreeMap<[i64; 3], u16>, VoxelEditHistoryError> {
        let mut materials = material_map(&self.base_voxels);
        for entry in &self.entries[..cursor] {
            let before_hash = crate::hash_material_voxels(&material_voxels(&materials));
            if before_hash != entry.before_hash {
                return Err(VoxelEditHistoryError::StaleAuthority {
                    expected: entry.before_hash,
                    actual: before_hash,
                });
            }
            apply_deltas(&mut materials, entry)?;
            let after_hash = crate::hash_material_voxels(&material_voxels(&materials));
            if after_hash != entry.after_hash {
                return Err(VoxelEditHistoryError::StaleAuthority {
                    expected: entry.after_hash,
                    actual: after_hash,
                });
            }
        }
        Ok(materials)
    }

    pub(crate) fn ensure_scene_at_cursor(
        &self,
        scene: &VoxelCollisionScene,
    ) -> Result<(), VoxelEditHistoryError> {
        if scene.voxel_size() != self.base_voxel_size || scene.chunk_size() != self.base_chunk_size
        {
            return Err(VoxelEditHistoryError::SceneShapeMismatch);
        }
        let expected = self.cursor_at(self.cursor_index).authority_hash;
        if scene.authority_hash() != expected {
            return Err(VoxelEditHistoryError::StaleAuthority {
                expected,
                actual: scene.authority_hash(),
            });
        }
        if scene.source_revision() != self.source_revision {
            return Err(VoxelEditHistoryError::StaleRevision {
                expected: self.source_revision,
                actual: scene.source_revision(),
            });
        }
        Ok(())
    }

    fn cursor_at(&self, index: usize) -> VoxelEditHistoryCursor {
        let authority_hash = index
            .checked_sub(1)
            .and_then(|entry| self.entries.get(entry))
            .map_or(self.base_hash, |entry| entry.after_hash);
        VoxelEditHistoryCursor {
            index,
            applied_transaction_id: index
                .checked_sub(1)
                .and_then(|entry| self.entries.get(entry))
                .map(|entry| entry.transaction_id),
            undo_depth: index,
            redo_depth: self.entries.len().saturating_sub(index),
            authority_hash,
            history_hash: history_hash(&self.entries, index, authority_hash),
        }
    }
}

fn apply_deltas(
    materials: &mut BTreeMap<[i64; 3], u16>,
    entry: &VoxelEditHistoryEntry,
) -> Result<(), VoxelEditHistoryError> {
    for delta in &entry.deltas {
        let actual = materials.get(&delta.address).copied();
        if actual != delta.before_material {
            return Err(VoxelEditHistoryError::CorruptEntry {
                transaction_id: entry.transaction_id,
                address: delta.address,
                expected_material: delta.before_material,
                actual_material: actual,
            });
        }
        match delta.after_material {
            Some(material) => {
                materials.insert(delta.address, material);
            }
            None => {
                materials.remove(&delta.address);
            }
        }
    }
    Ok(())
}

fn summarize_diff(
    before: &BTreeMap<[i64; 3], u16>,
    after: &BTreeMap<[i64; 3], u16>,
    included_transaction_ids: Vec<u64>,
    max_samples: usize,
) -> VoxelEditHistoryDiffSummary {
    let addresses: BTreeSet<_> = before.keys().chain(after.keys()).copied().collect();
    let mut deltas = Vec::new();
    let mut material_counts = BTreeMap::<(Option<u16>, Option<u16>), usize>::new();
    for address in addresses {
        let before_material = before.get(&address).copied();
        let after_material = after.get(&address).copied();
        if before_material != after_material {
            deltas.push(VoxelEditDelta {
                address,
                before_material,
                after_material,
            });
            *material_counts
                .entry((before_material, after_material))
                .or_default() += 1;
        }
    }
    let bounds = (!deltas.is_empty()).then(|| VoxelEditHistoryBounds {
        min: [0, 1, 2].map(|axis| {
            deltas
                .iter()
                .map(|delta| delta.address[axis])
                .min()
                .expect("nonempty diff")
        }),
        max: [0, 1, 2].map(|axis| {
            deltas
                .iter()
                .map(|delta| delta.address[axis])
                .max()
                .expect("nonempty diff")
        }),
    });
    let changed_voxels = deltas.len();
    let samples_truncated = deltas.len() > max_samples;
    deltas.truncate(max_samples);
    VoxelEditHistoryDiffSummary {
        changed_voxels,
        bounds,
        material_deltas: material_counts
            .into_iter()
            .map(|((before_material, after_material), changed_voxels)| {
                VoxelEditHistoryMaterialDelta {
                    before_material,
                    after_material,
                    changed_voxels,
                }
            })
            .collect(),
        samples: deltas,
        samples_truncated,
        included_transaction_ids,
    }
}

fn transaction_ids_between(entries: &[VoxelEditHistoryEntry], from: usize, to: usize) -> Vec<u64> {
    let (start, end) = if from <= to { (from, to) } else { (to, from) };
    entries[start..end]
        .iter()
        .map(|entry| entry.transaction_id)
        .collect()
}

pub(crate) fn material_map(voxels: &[MaterialVoxel]) -> BTreeMap<[i64; 3], u16> {
    voxels
        .iter()
        .map(|voxel| (voxel.address, voxel.material_slot))
        .collect()
}

pub(crate) fn material_voxels(materials: &BTreeMap<[i64; 3], u16>) -> Vec<MaterialVoxel> {
    materials
        .iter()
        .map(|(&address, &material_slot)| MaterialVoxel {
            address,
            material_slot,
        })
        .collect()
}

fn history_hash(entries: &[VoxelEditHistoryEntry], cursor: usize, authority_hash: u64) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    feed(&mut hash, &(cursor as u64).to_le_bytes());
    feed(&mut hash, &authority_hash.to_le_bytes());
    for entry in entries {
        feed(&mut hash, &entry.transaction_id.to_le_bytes());
        feed(
            &mut hash,
            &entry.parent_transaction_id.unwrap_or(0).to_le_bytes(),
        );
        feed(&mut hash, &entry.before_hash.to_le_bytes());
        feed(&mut hash, &entry.after_hash.to_le_bytes());
        feed(&mut hash, &(entry.deltas.len() as u64).to_le_bytes());
    }
    hash
}

fn feed(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
}
