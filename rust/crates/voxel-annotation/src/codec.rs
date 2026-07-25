use std::collections::{BTreeMap, BTreeSet};

use core_assets::{AssetId, AssetKind};
use sha2::{Digest, Sha256};
use voxel_asset::VoxelAsset;

use crate::{
    VoxelAnnotationContentHashes, VoxelAnnotationDiagnostic, VoxelAnnotationDiagnosticCode,
    VoxelAnnotationLayer, VoxelAnnotationLayerDraft, VoxelAnnotationLimits, VoxelAnnotationRegion,
    VoxelAnnotationSparseRun, VOXEL_ANNOTATION_SCHEMA_VERSION,
};

pub const MAX_ANNOTATION_BYTES: usize = 32 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoxelAnnotationError {
    Decode { path: String, message: String },
    ResourceLimit { limit: usize, actual: usize },
    Invalid(Vec<VoxelAnnotationDiagnostic>),
}

impl VoxelAnnotationError {
    pub fn diagnostics(&self) -> &[VoxelAnnotationDiagnostic] {
        match self {
            Self::Invalid(diagnostics) => diagnostics,
            _ => &[],
        }
    }
}

impl std::fmt::Display for VoxelAnnotationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Decode { path, message } => {
                write!(formatter, "decode error at {path}: {message}")
            }
            Self::ResourceLimit { limit, actual } => {
                write!(formatter, "annotation has {actual} bytes; limit is {limit}")
            }
            Self::Invalid(diagnostics) => write!(
                formatter,
                "annotation failed validation with {} diagnostic(s)",
                diagnostics.len()
            ),
        }
    }
}

impl std::error::Error for VoxelAnnotationError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoxelAnnotationExport {
    pub layer: VoxelAnnotationLayer,
    pub canonical_json: String,
    pub canonical_layer_hash: String,
    pub membership_data_hash: String,
}

pub fn finalize_annotation_draft(
    draft: VoxelAnnotationLayerDraft,
    target: &VoxelAsset,
    limits: VoxelAnnotationLimits,
) -> Result<VoxelAnnotationLayer, VoxelAnnotationError> {
    let mut layer = VoxelAnnotationLayer {
        schema_version: VOXEL_ANNOTATION_SCHEMA_VERSION,
        layer_id: draft.layer_id,
        target_voxel_asset_id: draft.target_voxel_asset_id,
        target_voxel_data_hash: draft.target_voxel_data_hash,
        target_bounds: draft.target_bounds,
        regions: draft.regions,
        provenance: draft.provenance,
        content_hashes: VoxelAnnotationContentHashes {
            canonical_layer: String::new(),
            membership_data: String::new(),
        },
    };
    normalize_layer(&mut layer);
    let diagnostics = semantic_diagnostics(&layer, Some(target), limits);
    if !diagnostics.is_empty() {
        return Err(VoxelAnnotationError::Invalid(diagnostics));
    }
    set_hashes(&mut layer);
    validate_annotation_layer(&layer, Some(target), limits)?;
    Ok(layer)
}

pub fn validate_annotation_layer(
    layer: &VoxelAnnotationLayer,
    target: Option<&VoxelAsset>,
    limits: VoxelAnnotationLimits,
) -> Result<(), VoxelAnnotationError> {
    let mut diagnostics = semantic_diagnostics(layer, target, limits);
    let canonical_hash = canonical_layer_hash(layer);
    let membership_hash = membership_data_hash(layer);
    if !valid_sha256(&layer.content_hashes.canonical_layer)
        || layer.content_hashes.canonical_layer != canonical_hash
    {
        diagnostics.push(diagnostic(
            VoxelAnnotationDiagnosticCode::ContentHashMismatch,
            "contentHashes.canonicalLayer",
            "canonical layer hash is missing, malformed, or stale",
        ));
    }
    if !valid_sha256(&layer.content_hashes.membership_data)
        || layer.content_hashes.membership_data != membership_hash
    {
        diagnostics.push(diagnostic(
            VoxelAnnotationDiagnosticCode::ContentHashMismatch,
            "contentHashes.membershipData",
            "membership data hash is missing, malformed, or stale",
        ));
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(VoxelAnnotationError::Invalid(diagnostics))
    }
}

pub fn encode_annotation_layer(
    layer: &VoxelAnnotationLayer,
) -> Result<String, VoxelAnnotationError> {
    validate_annotation_layer(layer, None, VoxelAnnotationLimits::default())?;
    let mut canonical = layer.clone();
    normalize_layer(&mut canonical);
    let mut encoded =
        serde_json::to_string_pretty(&canonical).map_err(|error| VoxelAnnotationError::Decode {
            path: "$".to_string(),
            message: error.to_string(),
        })?;
    encoded.push('\n');
    if encoded.len() > MAX_ANNOTATION_BYTES {
        return Err(VoxelAnnotationError::ResourceLimit {
            limit: MAX_ANNOTATION_BYTES,
            actual: encoded.len(),
        });
    }
    Ok(encoded)
}

pub fn decode_annotation_layer(input: &str) -> Result<VoxelAnnotationLayer, VoxelAnnotationError> {
    if input.len() > MAX_ANNOTATION_BYTES {
        return Err(VoxelAnnotationError::ResourceLimit {
            limit: MAX_ANNOTATION_BYTES,
            actual: input.len(),
        });
    }
    let mut deserializer = serde_json::Deserializer::from_str(input);
    let mut layer: VoxelAnnotationLayer = serde_path_to_error::deserialize(&mut deserializer)
        .map_err(|error| VoxelAnnotationError::Decode {
            path: json_path(&error.path().to_string()),
            message: error.inner().to_string(),
        })?;
    deserializer
        .end()
        .map_err(|error| VoxelAnnotationError::Decode {
            path: "$".to_string(),
            message: error.to_string(),
        })?;
    validate_annotation_layer(&layer, None, VoxelAnnotationLimits::default())?;
    normalize_layer(&mut layer);
    Ok(layer)
}

pub fn export_annotation_layer(
    layer: &VoxelAnnotationLayer,
    expected_layer_hash: &str,
) -> Result<VoxelAnnotationExport, VoxelAnnotationError> {
    if expected_layer_hash != layer.content_hashes.canonical_layer {
        return Err(VoxelAnnotationError::Invalid(vec![diagnostic(
            VoxelAnnotationDiagnosticCode::ContentHashMismatch,
            "expectedLayerHash",
            "export expected a different annotation layer hash",
        )]));
    }
    let canonical_json = encode_annotation_layer(layer)?;
    Ok(VoxelAnnotationExport {
        layer: layer.clone(),
        canonical_json,
        canonical_layer_hash: layer.content_hashes.canonical_layer.clone(),
        membership_data_hash: layer.content_hashes.membership_data.clone(),
    })
}

pub(crate) fn normalize_and_rehash(
    layer: &mut VoxelAnnotationLayer,
    limits: VoxelAnnotationLimits,
) -> Result<(), VoxelAnnotationError> {
    normalize_layer(layer);
    layer.content_hashes.canonical_layer.clear();
    layer.content_hashes.membership_data.clear();
    let diagnostics = semantic_diagnostics(layer, None, limits);
    if !diagnostics.is_empty() {
        return Err(VoxelAnnotationError::Invalid(diagnostics));
    }
    set_hashes(layer);
    validate_annotation_layer(layer, None, limits)
}

fn semantic_diagnostics(
    layer: &VoxelAnnotationLayer,
    target: Option<&VoxelAsset>,
    limits: VoxelAnnotationLimits,
) -> Vec<VoxelAnnotationDiagnostic> {
    let mut diagnostics = Vec::new();
    if layer.schema_version != VOXEL_ANNOTATION_SCHEMA_VERSION {
        diagnostics.push(diagnostic(
            VoxelAnnotationDiagnosticCode::UnsupportedSchema,
            "schemaVersion",
            format!(
                "expected schema {VOXEL_ANNOTATION_SCHEMA_VERSION}, found {}",
                layer.schema_version
            ),
        ));
    }
    if !valid_scoped_id(
        &layer.layer_id,
        "voxel-annotation/",
        limits.max_string_bytes,
    ) {
        diagnostics.push(diagnostic(
            VoxelAnnotationDiagnosticCode::InvalidLayerId,
            "layerId",
            "layer id must use voxel-annotation/ and lowercase path segments",
        ));
    }
    match AssetId::parse(&layer.target_voxel_asset_id) {
        Ok(id) if id.kind() == AssetKind::VoxelVolume => {}
        Ok(id) => diagnostics.push(diagnostic(
            VoxelAnnotationDiagnosticCode::InvalidTarget,
            "targetVoxelAssetId",
            format!("expected voxel-volume identity, found {}", id.kind()),
        )),
        Err(error) => diagnostics.push(diagnostic(
            VoxelAnnotationDiagnosticCode::InvalidTarget,
            "targetVoxelAssetId",
            error.to_string(),
        )),
    }
    if !layer.target_bounds.is_valid() {
        diagnostics.push(diagnostic(
            VoxelAnnotationDiagnosticCode::InvalidBounds,
            "targetBounds",
            "inclusive bounds require min <= max on every axis",
        ));
    }
    if !valid_sha256(&layer.target_voxel_data_hash) {
        diagnostics.push(diagnostic(
            VoxelAnnotationDiagnosticCode::TargetHashMismatch,
            "targetVoxelDataHash",
            "target voxel data hash must be a lowercase SHA-256 identity",
        ));
    }
    if let Some(target) = target {
        if layer.target_voxel_asset_id != target.asset_id {
            diagnostics.push(diagnostic(
                VoxelAnnotationDiagnosticCode::InvalidTarget,
                "targetVoxelAssetId",
                "annotation target does not match the supplied voxel asset",
            ));
        }
        if layer.target_voxel_data_hash != target.voxel_data_hash {
            diagnostics.push(diagnostic(
                VoxelAnnotationDiagnosticCode::TargetHashMismatch,
                "targetVoxelDataHash",
                "annotation target hash is stale",
            ));
        }
        if layer.target_bounds.min != target.bounds.min
            || layer.target_bounds.max != target.bounds.max
        {
            diagnostics.push(diagnostic(
                VoxelAnnotationDiagnosticCode::InvalidBounds,
                "targetBounds",
                "annotation target bounds do not match the voxel asset",
            ));
        }
    }
    validate_provenance(layer, limits, &mut diagnostics);
    validate_regions(layer, limits, &mut diagnostics);
    diagnostics
}

fn validate_provenance(
    layer: &VoxelAnnotationLayer,
    limits: VoxelAnnotationLimits,
    diagnostics: &mut Vec<VoxelAnnotationDiagnostic>,
) {
    if layer.provenance.len() > limits.max_provenance_refs {
        diagnostics.push(diagnostic(
            VoxelAnnotationDiagnosticCode::QuotaExceeded,
            "provenance",
            "provenance reference quota exceeded",
        ));
    }
    for (index, provenance) in layer.provenance.iter().enumerate() {
        if !valid_string(&provenance.uri, limits.max_string_bytes)
            || !valid_sha256(&provenance.content_hash)
        {
            diagnostics.push(diagnostic(
                VoxelAnnotationDiagnosticCode::InvalidProvenance,
                format!("provenance[{index}]"),
                "provenance requires a bounded URI and lowercase SHA-256 hash",
            ));
        }
    }
}

fn validate_regions(
    layer: &VoxelAnnotationLayer,
    limits: VoxelAnnotationLimits,
    diagnostics: &mut Vec<VoxelAnnotationDiagnostic>,
) {
    if layer.regions.len() > limits.max_regions {
        diagnostics.push(diagnostic(
            VoxelAnnotationDiagnosticCode::QuotaExceeded,
            "regions",
            "region quota exceeded",
        ));
    }
    let mut ids = BTreeSet::new();
    let mut parents = BTreeMap::new();
    let mut assigned = 0u64;
    for (index, region) in layer.regions.iter().enumerate() {
        if !valid_scoped_id(&region.region_id, "region/", limits.max_string_bytes) {
            diagnostics.push(diagnostic(
                VoxelAnnotationDiagnosticCode::InvalidRegionId,
                format!("regions[{index}].regionId"),
                "region id must use region/ and lowercase path segments",
            ));
        } else if !ids.insert(region.region_id.as_str()) {
            diagnostics.push(diagnostic(
                VoxelAnnotationDiagnosticCode::DuplicateRegionId,
                format!("regions[{index}].regionId"),
                "region id appears more than once",
            ));
        }
        parents.insert(
            region.region_id.as_str(),
            region.parent_region_id.as_deref(),
        );
        validate_region(layer, region, index, limits, &mut assigned, diagnostics);
    }
    if assigned > limits.max_total_assigned_cells {
        diagnostics.push(diagnostic(
            VoxelAnnotationDiagnosticCode::QuotaExceeded,
            "regions.selection",
            format!(
                "assigned cell count {assigned} exceeds {}",
                limits.max_total_assigned_cells
            ),
        ));
    }
    validate_parent_tree(&parents, diagnostics);
}

fn validate_region(
    layer: &VoxelAnnotationLayer,
    region: &VoxelAnnotationRegion,
    index: usize,
    limits: VoxelAnnotationLimits,
    assigned: &mut u64,
    diagnostics: &mut Vec<VoxelAnnotationDiagnostic>,
) {
    if !valid_string(&region.label, limits.max_string_bytes) {
        diagnostics.push(diagnostic(
            VoxelAnnotationDiagnosticCode::QuotaExceeded,
            format!("regions[{index}].label"),
            "label is empty or exceeds the string quota",
        ));
    }
    if region.tags.len() > limits.max_tags_per_region
        || region
            .tags
            .iter()
            .any(|tag| !valid_string(tag, limits.max_string_bytes))
    {
        diagnostics.push(diagnostic(
            VoxelAnnotationDiagnosticCode::QuotaExceeded,
            format!("regions[{index}].tags"),
            "tag count or string quota exceeded",
        ));
    }
    if !region.bounds.is_valid() {
        diagnostics.push(diagnostic(
            VoxelAnnotationDiagnosticCode::InvalidBounds,
            format!("regions[{index}].bounds"),
            "inclusive bounds require min <= max",
        ));
    } else if !layer.target_bounds.contains_bounds(region.bounds) {
        diagnostics.push(diagnostic(
            VoxelAnnotationDiagnosticCode::RegionOutOfBounds,
            format!("regions[{index}].bounds"),
            "region bounds must stay inside target bounds",
        ));
    }
    let runs = &region.selection.sparse_runs;
    if runs.is_empty() || runs.len() > limits.max_runs_per_region {
        diagnostics.push(diagnostic(
            VoxelAnnotationDiagnosticCode::QuotaExceeded,
            format!("regions[{index}].selection.sparseRuns"),
            "selection must contain a bounded, non-empty run set",
        ));
    }
    let mut rows = BTreeMap::<(i64, i64), Vec<(i64, i64, usize)>>::new();
    for (run_index, run) in runs.iter().copied().enumerate() {
        *assigned = assigned.saturating_add(u64::from(run.length));
        let Some(bounds) = run.bounds() else {
            diagnostics.push(diagnostic(
                VoxelAnnotationDiagnosticCode::InvalidSparseRun,
                format!("regions[{index}].selection.sparseRuns[{run_index}]"),
                "run length must be positive and its end must not overflow",
            ));
            continue;
        };
        if !layer.target_bounds.contains_bounds(bounds) || !region.bounds.contains_bounds(bounds) {
            diagnostics.push(diagnostic(
                VoxelAnnotationDiagnosticCode::RegionOutOfBounds,
                format!("regions[{index}].selection.sparseRuns[{run_index}]"),
                "run must stay inside both region and target bounds",
            ));
        }
        rows.entry((run.start[2], run.start[1])).or_default().push((
            run.start[0],
            bounds.max[0],
            run_index,
        ));
    }
    for runs in rows.values_mut() {
        runs.sort_unstable();
        let mut prior_end = None;
        for (start, end, run_index) in runs {
            if prior_end.is_some_and(|prior| *start <= prior) {
                diagnostics.push(diagnostic(
                    VoxelAnnotationDiagnosticCode::DuplicateCell,
                    format!("regions[{index}].selection.sparseRuns[{run_index}]"),
                    "selection runs overlap",
                ));
            }
            prior_end = Some(prior_end.map_or(*end, |prior: i64| prior.max(*end)));
        }
    }
}

fn validate_parent_tree(
    parents: &BTreeMap<&str, Option<&str>>,
    diagnostics: &mut Vec<VoxelAnnotationDiagnostic>,
) {
    for (&region, &parent) in parents {
        let Some(parent) = parent else { continue };
        if !parents.contains_key(parent) {
            diagnostics.push(diagnostic(
                VoxelAnnotationDiagnosticCode::UnknownParentRegion,
                format!("regions[{region}].parentRegionId"),
                "parent region does not exist",
            ));
            continue;
        }
        let mut seen = BTreeSet::new();
        let mut cursor = Some(region);
        while let Some(current) = cursor {
            if !seen.insert(current) {
                diagnostics.push(diagnostic(
                    VoxelAnnotationDiagnosticCode::ParentCycle,
                    format!("regions[{region}].parentRegionId"),
                    "parent chain contains a cycle",
                ));
                break;
            }
            cursor = parents.get(current).copied().flatten();
        }
    }
}

fn normalize_layer(layer: &mut VoxelAnnotationLayer) {
    for region in &mut layer.regions {
        region.tags.sort();
        region.tags.dedup();
        region
            .selection
            .sparse_runs
            .sort_by_key(|run| (run.start[2], run.start[1], run.start[0], run.length));
        let mut merged: Vec<VoxelAnnotationSparseRun> = Vec::new();
        for run in region.selection.sparse_runs.iter().copied() {
            if let Some(previous) = merged.last_mut() {
                let adjacent = previous.start[1] == run.start[1]
                    && previous.start[2] == run.start[2]
                    && previous.end_x().and_then(|end| end.checked_add(1)) == Some(run.start[0]);
                if adjacent {
                    previous.length = previous.length.saturating_add(run.length);
                    continue;
                }
            }
            merged.push(run);
        }
        region.selection.sparse_runs = merged;
    }
    layer
        .regions
        .sort_by(|left, right| left.region_id.cmp(&right.region_id));
    layer.provenance.sort_by(|left, right| {
        (left.kind, &left.uri, &left.content_hash).cmp(&(
            right.kind,
            &right.uri,
            &right.content_hash,
        ))
    });
}

fn set_hashes(layer: &mut VoxelAnnotationLayer) {
    layer.content_hashes.membership_data = membership_data_hash(layer);
    layer.content_hashes.canonical_layer = canonical_layer_hash(layer);
}

fn canonical_layer_hash(layer: &VoxelAnnotationLayer) -> String {
    let mut canonical = layer.clone();
    normalize_layer(&mut canonical);
    canonical.content_hashes.canonical_layer.clear();
    canonical.content_hashes.membership_data.clear();
    let bytes = serde_json::to_vec(&canonical).expect("annotation layer serializes");
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn membership_data_hash(layer: &VoxelAnnotationLayer) -> String {
    let mut canonical = layer.clone();
    normalize_layer(&mut canonical);
    let mut bytes = Vec::new();
    for region in canonical.regions {
        bytes.extend_from_slice(&(region.region_id.len() as u64).to_le_bytes());
        bytes.extend_from_slice(region.region_id.as_bytes());
        for run in region.selection.sparse_runs {
            for coordinate in run.start {
                bytes.extend_from_slice(&coordinate.to_le_bytes());
            }
            bytes.extend_from_slice(&run.length.to_le_bytes());
        }
    }
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn valid_sha256(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

fn valid_string(value: &str, max_bytes: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max_bytes && !value.contains('\0')
}

fn valid_scoped_id(value: &str, prefix: &str, max_bytes: usize) -> bool {
    value.len() <= max_bytes
        && value.strip_prefix(prefix).is_some_and(|tail| {
            !tail.is_empty()
                && tail.split('/').all(|segment| {
                    !segment.is_empty()
                        && segment.bytes().all(|byte| {
                            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-'
                        })
                        && segment
                            .as_bytes()
                            .first()
                            .is_some_and(u8::is_ascii_alphanumeric)
                        && segment
                            .as_bytes()
                            .last()
                            .is_some_and(u8::is_ascii_alphanumeric)
                })
        })
}

fn diagnostic(
    code: VoxelAnnotationDiagnosticCode,
    path: impl Into<String>,
    message: impl Into<String>,
) -> VoxelAnnotationDiagnostic {
    VoxelAnnotationDiagnostic {
        code,
        path: path.into(),
        message: message.into(),
    }
}

fn json_path(path: &str) -> String {
    if path.is_empty() {
        "$".to_string()
    } else {
        path.to_string()
    }
}
