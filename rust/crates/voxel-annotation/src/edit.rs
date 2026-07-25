use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{
    codec::normalize_and_rehash, validate_annotation_layer, VoxelAnnotationBounds,
    VoxelAnnotationError, VoxelAnnotationKind, VoxelAnnotationLayer, VoxelAnnotationLimits,
    VoxelAnnotationRegion, VoxelAnnotationSelection, VoxelAnnotationSparseRun,
};

pub const MAX_ANNOTATION_COMMANDS_PER_TRANSACTION: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum VoxelAnnotationEditCommand {
    UpsertRegion {
        region: VoxelAnnotationRegion,
    },
    RemoveRegion {
        region_id: String,
    },
    AddRuns {
        region_id: String,
        sparse_runs: Vec<VoxelAnnotationSparseRun>,
    },
    RemoveRuns {
        region_id: String,
        sparse_runs: Vec<VoxelAnnotationSparseRun>,
    },
    ReplaceSelection {
        region_id: String,
        selection: VoxelAnnotationSelection,
    },
    SetParent {
        region_id: String,
        parent_region_id: Option<String>,
    },
    SetTags {
        region_id: String,
        tags: Vec<String>,
    },
    SetLabel {
        region_id: String,
        label: String,
    },
    SetKind {
        region_id: String,
        annotation_kind: VoxelAnnotationKind,
    },
    SetBounds {
        region_id: String,
        bounds: VoxelAnnotationBounds,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelAnnotationEditTransaction {
    pub expected_layer_hash: String,
    pub commands: Vec<VoxelAnnotationEditCommand>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoxelAnnotationEditReceipt {
    pub layer_hash_before: String,
    pub layer_hash_after: String,
    pub membership_hash_before: String,
    pub membership_hash_after: String,
    pub affected_region_ids: Vec<String>,
    pub command_count: usize,
    pub region_count: usize,
    pub assigned_cell_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoxelAnnotationEditError {
    InvalidCurrent(VoxelAnnotationError),
    StaleLayerHash { expected: String, actual: String },
    EmptyTransaction,
    TooManyCommands { limit: usize, actual: usize },
    UnknownRegion(String),
    InvalidRemovalRun(VoxelAnnotationSparseRun),
    InvalidCandidate(VoxelAnnotationError),
    NoChanges,
}

impl std::fmt::Display for VoxelAnnotationEditError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for VoxelAnnotationEditError {}

#[derive(Debug, Default, Clone, Copy)]
pub struct VoxelAnnotationEditService;

impl VoxelAnnotationEditService {
    /// Apply a complete edit batch to a validated clone, then swap. Rejections
    /// never partially mutate the caller's layer.
    pub fn apply(
        layer: &mut VoxelAnnotationLayer,
        transaction: VoxelAnnotationEditTransaction,
    ) -> Result<VoxelAnnotationEditReceipt, VoxelAnnotationEditError> {
        validate_annotation_layer(layer, None, VoxelAnnotationLimits::default())
            .map_err(VoxelAnnotationEditError::InvalidCurrent)?;
        if transaction.expected_layer_hash != layer.content_hashes.canonical_layer {
            return Err(VoxelAnnotationEditError::StaleLayerHash {
                expected: transaction.expected_layer_hash,
                actual: layer.content_hashes.canonical_layer.clone(),
            });
        }
        if transaction.commands.is_empty() {
            return Err(VoxelAnnotationEditError::EmptyTransaction);
        }
        if transaction.commands.len() > MAX_ANNOTATION_COMMANDS_PER_TRANSACTION {
            return Err(VoxelAnnotationEditError::TooManyCommands {
                limit: MAX_ANNOTATION_COMMANDS_PER_TRANSACTION,
                actual: transaction.commands.len(),
            });
        }

        let before_layer = layer.content_hashes.canonical_layer.clone();
        let before_membership = layer.content_hashes.membership_data.clone();
        let mut candidate = layer.clone();
        let mut affected = BTreeSet::new();
        for command in transaction.commands.iter().cloned() {
            apply_command(&mut candidate, command, &mut affected)?;
        }
        normalize_and_rehash(&mut candidate, VoxelAnnotationLimits::default())
            .map_err(VoxelAnnotationEditError::InvalidCandidate)?;
        if candidate.content_hashes.canonical_layer == before_layer {
            return Err(VoxelAnnotationEditError::NoChanges);
        }
        let receipt = VoxelAnnotationEditReceipt {
            layer_hash_before: before_layer,
            layer_hash_after: candidate.content_hashes.canonical_layer.clone(),
            membership_hash_before: before_membership,
            membership_hash_after: candidate.content_hashes.membership_data.clone(),
            affected_region_ids: affected.into_iter().collect(),
            command_count: transaction.commands.len(),
            region_count: candidate.regions.len(),
            assigned_cell_count: candidate
                .regions
                .iter()
                .flat_map(|region| &region.selection.sparse_runs)
                .map(|run| u64::from(run.length))
                .sum(),
        };
        *layer = candidate;
        Ok(receipt)
    }
}

fn apply_command(
    layer: &mut VoxelAnnotationLayer,
    command: VoxelAnnotationEditCommand,
    affected: &mut BTreeSet<String>,
) -> Result<(), VoxelAnnotationEditError> {
    match command {
        VoxelAnnotationEditCommand::UpsertRegion { region } => {
            affected.insert(region.region_id.clone());
            if let Some(existing) = layer
                .regions
                .iter_mut()
                .find(|existing| existing.region_id == region.region_id)
            {
                *existing = region;
            } else {
                layer.regions.push(region);
            }
        }
        VoxelAnnotationEditCommand::RemoveRegion { region_id } => {
            let index = region_index(layer, &region_id)?;
            layer.regions.remove(index);
            affected.insert(region_id);
        }
        VoxelAnnotationEditCommand::AddRuns {
            region_id,
            sparse_runs,
        } => {
            let index = region_index(layer, &region_id)?;
            layer.regions[index]
                .selection
                .sparse_runs
                .extend(sparse_runs);
            affected.insert(region_id);
        }
        VoxelAnnotationEditCommand::RemoveRuns {
            region_id,
            sparse_runs,
        } => {
            let index = region_index(layer, &region_id)?;
            layer.regions[index].selection.sparse_runs =
                subtract_runs(&layer.regions[index].selection.sparse_runs, &sparse_runs)?;
            affected.insert(region_id);
        }
        VoxelAnnotationEditCommand::ReplaceSelection {
            region_id,
            selection,
        } => {
            let index = region_index(layer, &region_id)?;
            layer.regions[index].selection = selection;
            affected.insert(region_id);
        }
        VoxelAnnotationEditCommand::SetParent {
            region_id,
            parent_region_id,
        } => {
            let index = region_index(layer, &region_id)?;
            layer.regions[index].parent_region_id = parent_region_id;
            affected.insert(region_id);
        }
        VoxelAnnotationEditCommand::SetTags { region_id, tags } => {
            let index = region_index(layer, &region_id)?;
            layer.regions[index].tags = tags;
            affected.insert(region_id);
        }
        VoxelAnnotationEditCommand::SetLabel { region_id, label } => {
            let index = region_index(layer, &region_id)?;
            layer.regions[index].label = label;
            affected.insert(region_id);
        }
        VoxelAnnotationEditCommand::SetKind {
            region_id,
            annotation_kind,
        } => {
            let index = region_index(layer, &region_id)?;
            layer.regions[index].kind = annotation_kind;
            affected.insert(region_id);
        }
        VoxelAnnotationEditCommand::SetBounds { region_id, bounds } => {
            let index = region_index(layer, &region_id)?;
            layer.regions[index].bounds = bounds;
            affected.insert(region_id);
        }
    }
    Ok(())
}

fn region_index(
    layer: &VoxelAnnotationLayer,
    region_id: &str,
) -> Result<usize, VoxelAnnotationEditError> {
    layer
        .regions
        .iter()
        .position(|region| region.region_id == region_id)
        .ok_or_else(|| VoxelAnnotationEditError::UnknownRegion(region_id.to_string()))
}

fn subtract_runs(
    source: &[VoxelAnnotationSparseRun],
    removals: &[VoxelAnnotationSparseRun],
) -> Result<Vec<VoxelAnnotationSparseRun>, VoxelAnnotationEditError> {
    let mut by_row = BTreeMap::<(i64, i64), Vec<(i64, i64)>>::new();
    for removal in removals.iter().copied() {
        let Some(end) = removal.end_x() else {
            return Err(VoxelAnnotationEditError::InvalidRemovalRun(removal));
        };
        by_row
            .entry((removal.start[2], removal.start[1]))
            .or_default()
            .push((removal.start[0], end));
    }
    for intervals in by_row.values_mut() {
        intervals.sort_unstable();
    }

    let mut output = Vec::new();
    for run in source.iter().copied() {
        let Some(end) = run.end_x() else {
            return Err(VoxelAnnotationEditError::InvalidRemovalRun(run));
        };
        let mut fragments = vec![(run.start[0], end)];
        if let Some(removals) = by_row.get(&(run.start[2], run.start[1])) {
            for &(remove_start, remove_end) in removals {
                let mut next = Vec::new();
                for (start, end) in fragments {
                    if remove_end < start || remove_start > end {
                        next.push((start, end));
                        continue;
                    }
                    if remove_start > start {
                        next.push((start, remove_start - 1));
                    }
                    if remove_end < end {
                        next.push((remove_end + 1, end));
                    }
                }
                fragments = next;
            }
        }
        for (start, end) in fragments {
            let length = u32::try_from(end - start + 1)
                .map_err(|_| VoxelAnnotationEditError::InvalidRemovalRun(run))?;
            output.push(VoxelAnnotationSparseRun {
                start: [start, run.start[1], run.start[2]],
                length,
            });
        }
    }
    Ok(output)
}
