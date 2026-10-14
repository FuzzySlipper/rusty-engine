use std::collections::{BTreeMap, BTreeSet};

use core_space::MAX_VOXEL_COORDINATE_ABS;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    VoxelAsset, VoxelAssetBounds, VoxelRepresentation, VoxelRepresentationKind, VoxelSparseRun,
    MAX_REPRESENTED_VOXELS,
};

/// One complete local-space arrangement of material voxels.
///
/// This value is shared by durable voxel objects and the existing volume
/// format's read-only frame view. Schema-1 object frames are intentionally
/// complete rather than delta encoded so frame resolution stays local and
/// bounded while the workflow is being measured.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelFrame {
    pub bounds: VoxelAssetBounds,
    pub representation: VoxelRepresentation,
    pub voxel_data_hash: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoxelFrameCell {
    pub coordinate: [i64; 3],
    pub material_slot: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VoxelFrameDiagnostic {
    pub(crate) code: &'static str,
    pub(crate) path: String,
    pub(crate) message: String,
}

impl From<&VoxelAsset> for VoxelFrame {
    fn from(asset: &VoxelAsset) -> Self {
        Self {
            bounds: asset.bounds,
            representation: asset.representation.clone(),
            voxel_data_hash: asset.voxel_data_hash.clone(),
        }
    }
}

/// Expand a frame's sparse runs to local cells.
///
/// The frame must belong to an object that came from `decode_voxel_object`
/// or `with_computed_voxel_object_hashes`; those checked its runs, so this
/// only expands them.
pub fn resolve_voxel_frame(frame: &VoxelFrame) -> Vec<VoxelFrameCell> {
    expand_sparse_runs(&frame.representation.sparse_runs)
}

/// Resolve an existing volume asset to its canonical engine-cell addresses.
///
/// This is the ordinary provider admission seam for stored volume occupancy;
/// callers remain responsible for deciding whether those cells feed collision,
/// navigation, rendering, or another named consumer. The asset must come from
/// `decode_voxel_asset` or `with_computed_content_hash`, which checked that
/// every mapped run stays inside the engine address range.
pub fn resolve_voxel_asset(asset: &VoxelAsset) -> Vec<VoxelFrameCell> {
    let origin = asset.grid.origin;
    let mut cells = expand_sparse_runs(&asset.representation.sparse_runs);
    for cell in &mut cells {
        cell.coordinate = std::array::from_fn(|axis| origin[axis] + cell.coordinate[axis]);
    }
    cells
}

pub fn represented_voxel_count(frame: &VoxelFrame) -> usize {
    frame
        .representation
        .sparse_runs
        .iter()
        .fold(0usize, |total, run| {
            total.saturating_add(run.length as usize)
        })
}

pub(crate) fn canonicalize_frame(frame: &mut VoxelFrame) {
    canonicalize_sparse_runs(&mut frame.representation.sparse_runs);
}

pub(crate) fn canonicalize_sparse_runs(runs: &mut Vec<VoxelSparseRun>) {
    runs.sort_by_key(|run| {
        (
            run.start[1],
            run.start[2],
            run.start[0],
            run.material_slot,
            run.length,
        )
    });
    let mut merged: Vec<VoxelSparseRun> = Vec::with_capacity(runs.len());
    for run in runs.iter().copied() {
        if let Some(previous) = merged.last_mut() {
            let adjacent = previous.start[1] == run.start[1]
                && previous.start[2] == run.start[2]
                && previous.material_slot == run.material_slot
                && previous.start[0].checked_add(i64::from(previous.length)) == Some(run.start[0]);
            if adjacent {
                previous.length = previous.length.saturating_add(run.length);
                continue;
            }
        }
        merged.push(run);
    }
    merged.sort_by_key(|run| (run.start, run.material_slot, run.length));
    *runs = merged;
}

/// SHA-256 of sparse runs that are already in canonical order.
pub(crate) fn canonical_runs_hash(runs: &[VoxelSparseRun]) -> String {
    let mut hasher = Sha256::new();
    for run in runs {
        for coordinate in run.start {
            hasher.update(coordinate.to_le_bytes());
        }
        hasher.update(run.length.to_le_bytes());
        hasher.update(run.material_slot.to_le_bytes());
    }
    format!("sha256:{:x}", hasher.finalize())
}

pub(crate) fn semantic_diagnostics(
    frame: &VoxelFrame,
    materials: &BTreeSet<u16>,
) -> Vec<VoxelFrameDiagnostic> {
    let mut diagnostics = Vec::new();
    if frame.representation.kind != VoxelRepresentationKind::SparseRuns {
        diagnostics.push(diagnostic(
            "voxelFrame.unsupportedRepresentation",
            "representation.kind",
            "schema 1 supports only sparseRuns",
        ));
    }
    if frame.representation.sparse_runs.is_empty() {
        diagnostics.push(diagnostic(
            "voxelFrame.invalidSparseRun",
            "representation.sparseRuns",
            "at least one solid sparse run is required",
        ));
        return diagnostics;
    }

    let mut represented = 0usize;
    let mut actual_min = [i64::MAX; 3];
    let mut actual_max = [i64::MIN; 3];
    let mut rows = BTreeMap::<(i64, i64), Vec<(i64, i64, usize)>>::new();
    for (index, run) in frame.representation.sparse_runs.iter().enumerate() {
        if run.length == 0 {
            diagnostics.push(diagnostic(
                "voxelFrame.invalidSparseRun",
                format!("representation.sparseRuns[{index}].length"),
                "run length must be greater than zero",
            ));
            continue;
        }
        represented = represented.saturating_add(run.length as usize);
        let Some(end_x) = run.start[0].checked_add(i64::from(run.length) - 1) else {
            diagnostics.push(diagnostic(
                "voxelFrame.invalidSparseRun",
                format!("representation.sparseRuns[{index}]"),
                "run end coordinate overflowed",
            ));
            continue;
        };
        if !materials.contains(&run.material_slot) {
            diagnostics.push(diagnostic(
                "voxelFrame.unknownMaterial",
                format!("representation.sparseRuns[{index}].materialSlot"),
                format!(
                    "voxel material {} is not in the object palette",
                    run.material_slot
                ),
            ));
        }
        actual_min[0] = actual_min[0].min(run.start[0]);
        actual_min[1] = actual_min[1].min(run.start[1]);
        actual_min[2] = actual_min[2].min(run.start[2]);
        actual_max[0] = actual_max[0].max(end_x);
        actual_max[1] = actual_max[1].max(run.start[1]);
        actual_max[2] = actual_max[2].max(run.start[2]);
        rows.entry((run.start[1], run.start[2]))
            .or_default()
            .push((run.start[0], end_x, index));
        if run
            .start
            .iter()
            .chain(std::iter::once(&end_x))
            .any(|coordinate| coordinate.unsigned_abs() > MAX_VOXEL_COORDINATE_ABS as u64)
        {
            diagnostics.push(diagnostic(
                "voxelFrame.resourceLimit",
                format!("representation.sparseRuns[{index}]"),
                format!("local frame coordinates must stay within +/-{MAX_VOXEL_COORDINATE_ABS}"),
            ));
        }
    }
    if represented > MAX_REPRESENTED_VOXELS {
        diagnostics.push(diagnostic(
            "voxelFrame.resourceLimit",
            "representation.sparseRuns",
            format!("runs represent {represented} voxels; limit is {MAX_REPRESENTED_VOXELS}"),
        ));
    }
    for ((y, z), runs) in &mut rows {
        runs.sort_unstable();
        let mut prior_end = None;
        for (start, end, index) in runs {
            if prior_end.is_some_and(|prior| *start <= prior) {
                diagnostics.push(diagnostic(
                    "voxelFrame.duplicateVoxel",
                    format!("representation.sparseRuns[{index}]"),
                    format!("run overlaps an earlier run on row y={y}, z={z}"),
                ));
            }
            prior_end = Some(prior_end.map_or(*end, |prior: i64| prior.max(*end)));
        }
    }
    let actual_bounds = VoxelAssetBounds {
        min: actual_min,
        max: actual_max,
    };
    if frame.bounds != actual_bounds {
        diagnostics.push(diagnostic(
            "voxelFrame.invalidBounds",
            "bounds",
            format!(
                "declared bounds {:?} do not equal represented bounds {:?}",
                frame.bounds, actual_bounds
            ),
        ));
    }
    diagnostics
}

fn expand_sparse_runs(runs: &[VoxelSparseRun]) -> Vec<VoxelFrameCell> {
    let represented = runs.iter().map(|run| run.length as usize).sum::<usize>();
    let mut cells = Vec::with_capacity(represented);
    for run in runs {
        for offset in 0..run.length {
            cells.push(VoxelFrameCell {
                coordinate: [run.start[0] + i64::from(offset), run.start[1], run.start[2]],
                material_slot: run.material_slot,
            });
        }
    }
    cells
}

fn diagnostic(
    code: &'static str,
    path: impl Into<String>,
    message: impl Into<String>,
) -> VoxelFrameDiagnostic {
    VoxelFrameDiagnostic {
        code,
        path: path.into(),
        message: message.into(),
    }
}
