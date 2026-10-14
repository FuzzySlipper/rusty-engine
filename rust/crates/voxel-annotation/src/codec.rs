use std::collections::{BTreeMap, BTreeSet};

use core_assets::{AssetId, AssetKind};
use sha2::{Digest, Sha256};
use voxel_asset::VoxelAsset;

use crate::{
    VoxelAnnotationContentHashes, VoxelAnnotationDiagnostic, VoxelAnnotationDiagnosticCode,
    VoxelAnnotationLayer, VoxelAnnotationLayerDraft, VoxelAnnotationRegion,
    VoxelAnnotationSparseRun, VOXEL_ANNOTATION_SCHEMA_VERSION,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoxelAnnotationError {
    Decode { path: String, message: String },
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
            Self::Invalid(diagnostics) => write!(
                formatter,
                "annotation failed validation with {} diagnostic(s)",
                diagnostics.len()
            ),
        }
    }
}

impl std::error::Error for VoxelAnnotationError {}

pub fn finalize_annotation_draft(
    draft: VoxelAnnotationLayerDraft,
    target: &VoxelAsset,
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
    normalize_and_rehash(&mut layer, Some(target))?;
    Ok(layer)
}

/// Checks a layer's meaning: schema, identities, bounds, regions and parents,
/// plus the supplied target when one is given. It does not recompute
/// `content_hashes`; finalize, decode and edits compute them whenever the
/// layer's contents change.
pub fn validate_annotation_layer(
    layer: &VoxelAnnotationLayer,
    target: Option<&VoxelAsset>,
) -> Result<(), VoxelAnnotationError> {
    let diagnostics = semantic_diagnostics(layer, target);
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(VoxelAnnotationError::Invalid(diagnostics))
    }
}

/// Serializes a layer as it is. Every layer comes from finalize, decode or an
/// edit, which all end in normalizing, checking and hashing its contents.
pub fn encode_annotation_layer(
    layer: &VoxelAnnotationLayer,
) -> Result<String, VoxelAnnotationError> {
    let mut encoded =
        serde_json::to_string_pretty(layer).map_err(|error| VoxelAnnotationError::Decode {
            path: "$".to_string(),
            message: error.to_string(),
        })?;
    encoded.push('\n');
    Ok(encoded)
}

/// Decodes a layer and establishes its identity from its contents. The
/// recorded `contentHashes` must equal the computed ones, so a layer whose
/// contents changed without a new identity cannot satisfy an expected-hash
/// check written against the old contents. `target`, when given, must be the
/// voxel asset the layer annotates.
pub fn decode_annotation_layer(
    input: &str,
    target: Option<&VoxelAsset>,
) -> Result<VoxelAnnotationLayer, VoxelAnnotationError> {
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
    let recorded = layer.content_hashes.clone();
    normalize_and_rehash(&mut layer, target)?;
    if layer.content_hashes != recorded {
        return Err(VoxelAnnotationError::Invalid(vec![diagnostic(
            VoxelAnnotationDiagnosticCode::ContentHashMismatch,
            "contentHashes",
            "recorded content hashes do not match the layer contents",
        )]));
    }
    Ok(layer)
}

/// Canonicalizes changed contents, checks their meaning and computes the
/// layer's identities once. Every path that changes contents ends here.
pub(crate) fn normalize_and_rehash(
    layer: &mut VoxelAnnotationLayer,
    target: Option<&VoxelAsset>,
) -> Result<(), VoxelAnnotationError> {
    normalize_layer(layer);
    validate_annotation_layer(layer, target)?;
    set_hashes(layer);
    Ok(())
}

fn semantic_diagnostics(
    layer: &VoxelAnnotationLayer,
    target: Option<&VoxelAsset>,
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
    if !valid_scoped_id(&layer.layer_id, "voxel-annotation/") {
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
    validate_provenance(layer, &mut diagnostics);
    validate_regions(layer, &mut diagnostics);
    diagnostics
}

fn validate_provenance(
    layer: &VoxelAnnotationLayer,
    diagnostics: &mut Vec<VoxelAnnotationDiagnostic>,
) {
    for (index, provenance) in layer.provenance.iter().enumerate() {
        if !valid_string(&provenance.uri) || !valid_sha256(&provenance.content_hash) {
            diagnostics.push(diagnostic(
                VoxelAnnotationDiagnosticCode::InvalidProvenance,
                format!("provenance[{index}]"),
                "provenance requires a URI and lowercase SHA-256 hash",
            ));
        }
    }
}

fn validate_regions(
    layer: &VoxelAnnotationLayer,
    diagnostics: &mut Vec<VoxelAnnotationDiagnostic>,
) {
    let mut ids = BTreeSet::new();
    let mut parents = BTreeMap::new();
    for (index, region) in layer.regions.iter().enumerate() {
        if !valid_scoped_id(&region.region_id, "region/") {
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
        validate_region(layer, region, index, diagnostics);
    }
    validate_parent_tree(&parents, diagnostics);
}

fn validate_region(
    layer: &VoxelAnnotationLayer,
    region: &VoxelAnnotationRegion,
    index: usize,
    diagnostics: &mut Vec<VoxelAnnotationDiagnostic>,
) {
    if !valid_string(&region.label) {
        diagnostics.push(diagnostic(
            VoxelAnnotationDiagnosticCode::InvalidText,
            format!("regions[{index}].label"),
            "label must not be empty or contain NUL",
        ));
    }
    if !region.tags.iter().all(|tag| valid_string(tag)) {
        diagnostics.push(diagnostic(
            VoxelAnnotationDiagnosticCode::InvalidText,
            format!("regions[{index}].tags"),
            "tags must not be empty or contain NUL",
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
    if runs.is_empty() {
        diagnostics.push(diagnostic(
            VoxelAnnotationDiagnosticCode::InvalidSparseRun,
            format!("regions[{index}].selection.sparseRuns"),
            "selection must not be empty",
        ));
    }
    let mut rows = BTreeMap::<(i64, i64), Vec<(i64, i64, usize)>>::new();
    for (run_index, run) in runs.iter().copied().enumerate() {
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

/// Computes both identities of an already normalized layer. The canonical
/// hash covers the layer serialized with empty hash fields.
fn set_hashes(layer: &mut VoxelAnnotationLayer) {
    layer.content_hashes.canonical_layer.clear();
    layer.content_hashes.membership_data.clear();
    let canonical = serde_json::to_vec(&*layer).expect("annotation layer serializes");
    layer.content_hashes.canonical_layer = format!("sha256:{:x}", Sha256::digest(canonical));
    let mut membership = Sha256::new();
    for region in &layer.regions {
        membership.update((region.region_id.len() as u64).to_le_bytes());
        membership.update(region.region_id.as_bytes());
        for run in &region.selection.sparse_runs {
            for coordinate in run.start {
                membership.update(coordinate.to_le_bytes());
            }
            membership.update(run.length.to_le_bytes());
        }
    }
    layer.content_hashes.membership_data = format!("sha256:{:x}", membership.finalize());
}

fn valid_sha256(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

fn valid_string(value: &str) -> bool {
    !value.trim().is_empty() && !value.contains('\0')
}

fn valid_scoped_id(value: &str, prefix: &str) -> bool {
    value.strip_prefix(prefix).is_some_and(|tail| {
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
