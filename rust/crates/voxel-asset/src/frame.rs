use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    validate_voxel_asset, VoxelAsset, VoxelAssetBounds, VoxelRepresentation,
    VoxelRepresentationKind, VoxelSparseRun, MAX_REPRESENTED_VOXELS,
};

pub const MAX_VOXEL_FRAME_COORDINATE_ABS: u64 = 1_000_000;

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
pub struct VoxelFrameDiagnostic {
    pub code: &'static str,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoxelFrameError {
    diagnostics: Vec<VoxelFrameDiagnostic>,
}

impl VoxelFrameError {
    pub fn diagnostics(&self) -> &[VoxelFrameDiagnostic] {
        &self.diagnostics
    }

    pub(crate) fn from_diagnostics(diagnostics: Vec<VoxelFrameDiagnostic>) -> Self {
        Self { diagnostics }
    }
}

impl std::fmt::Display for VoxelFrameError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let first = self
            .diagnostics
            .first()
            .expect("voxel frame error always has a diagnostic");
        write!(
            formatter,
            "{} at {}: {}",
            first.code, first.path, first.message
        )
    }
}

impl std::error::Error for VoxelFrameError {}

impl From<&VoxelAsset> for VoxelFrame {
    fn from(asset: &VoxelAsset) -> Self {
        Self {
            bounds: asset.bounds,
            representation: asset.representation.clone(),
            voxel_data_hash: asset.voxel_data_hash.clone(),
        }
    }
}

pub fn validate_voxel_frame(
    frame: &VoxelFrame,
    material_slots: impl IntoIterator<Item = u16>,
) -> Result<(), VoxelFrameError> {
    let materials = material_slots.into_iter().collect::<BTreeSet<_>>();
    let mut diagnostics = semantic_diagnostics(frame, &materials);
    if !crate::codec::valid_sha256(&frame.voxel_data_hash) {
        diagnostics.push(diagnostic(
            "voxelFrame.voxelDataHashMismatch",
            "voxelDataHash",
            "voxelDataHash must be `sha256:` followed by 64 lowercase hexadecimal digits",
        ));
    } else if frame.voxel_data_hash != computed_voxel_data_hash(frame) {
        diagnostics.push(diagnostic(
            "voxelFrame.voxelDataHashMismatch",
            "voxelDataHash",
            "voxelDataHash does not match canonical sparse occupancy",
        ));
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(VoxelFrameError::from_diagnostics(diagnostics))
    }
}

pub fn canonicalize_voxel_frame(
    frame: &VoxelFrame,
    material_slots: impl IntoIterator<Item = u16>,
) -> Result<VoxelFrame, VoxelFrameError> {
    validate_voxel_frame(frame, material_slots)?;
    let mut canonical = frame.clone();
    canonicalize_frame(&mut canonical);
    Ok(canonical)
}

pub fn with_computed_voxel_frame_hash(
    mut frame: VoxelFrame,
    material_slots: impl IntoIterator<Item = u16>,
) -> Result<VoxelFrame, VoxelFrameError> {
    let materials = material_slots.into_iter().collect::<BTreeSet<_>>();
    frame.voxel_data_hash.clear();
    let diagnostics = semantic_diagnostics(&frame, &materials);
    if !diagnostics.is_empty() {
        return Err(VoxelFrameError::from_diagnostics(diagnostics));
    }
    canonicalize_frame(&mut frame);
    frame.voxel_data_hash = computed_voxel_data_hash(&frame);
    validate_voxel_frame(&frame, materials)?;
    Ok(frame)
}

pub fn resolve_voxel_frame(
    frame: &VoxelFrame,
    material_slots: impl IntoIterator<Item = u16>,
) -> Result<Vec<VoxelFrameCell>, VoxelFrameError> {
    validate_voxel_frame(frame, material_slots)?;
    Ok(expand_sparse_runs(&frame.representation.sparse_runs))
}

/// Resolve an existing volume asset to its canonical engine-cell addresses.
///
/// This is the ordinary provider admission seam for stored volume occupancy;
/// callers remain responsible for deciding whether those cells feed collision,
/// navigation, rendering, or another named consumer.
pub fn resolve_voxel_asset(
    asset: &VoxelAsset,
) -> Result<Vec<VoxelFrameCell>, crate::VoxelAssetError> {
    validate_voxel_asset(asset)?;
    let mut cells = Vec::new();
    for run in &asset.representation.sparse_runs {
        for offset in 0..run.length {
            let local_x = run.start[0]
                .checked_add(i64::from(offset))
                .expect("validated sparse run cannot overflow");
            let local = [local_x, run.start[1], run.start[2]];
            let coordinate = std::array::from_fn(|axis| {
                asset.grid.origin[axis]
                    .checked_add(local[axis])
                    .expect("validated mapped coordinate cannot overflow")
            });
            cells.push(VoxelFrameCell {
                coordinate,
                material_slot: run.material_slot,
            });
        }
    }
    Ok(cells)
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

pub(crate) fn computed_voxel_data_hash(frame: &VoxelFrame) -> String {
    let mut runs = frame.representation.sparse_runs.clone();
    canonicalize_sparse_runs(&mut runs);
    let mut bytes = Vec::with_capacity(runs.len().saturating_mul(30));
    for run in runs {
        for coordinate in run.start {
            bytes.extend_from_slice(&coordinate.to_le_bytes());
        }
        bytes.extend_from_slice(&run.length.to_le_bytes());
        bytes.extend_from_slice(&run.material_slot.to_le_bytes());
    }
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn semantic_diagnostics(
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
        if !(1..=4_095).contains(&run.material_slot) || !materials.contains(&run.material_slot) {
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
            .any(|coordinate| coordinate.unsigned_abs() > MAX_VOXEL_FRAME_COORDINATE_ABS)
        {
            diagnostics.push(diagnostic(
                "voxelFrame.resourceLimit",
                format!("representation.sparseRuns[{index}]"),
                format!(
                    "local frame coordinates must stay within +/-{MAX_VOXEL_FRAME_COORDINATE_ABS}"
                ),
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
