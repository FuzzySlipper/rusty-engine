use std::collections::{BTreeMap, BTreeSet};

use core_assets::{AssetId, AssetKind};
use sha2::{Digest, Sha256};

use crate::{
    VoxelAsset, VoxelAssetBounds, VoxelAssetMaterialMapping, VoxelRepresentationKind,
    VoxelSparseRun, VOXEL_ASSET_SCHEMA_VERSION,
};

pub const MAX_ARTIFACT_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_REPRESENTED_VOXELS: usize = 1_000_000;
pub const MAX_MATERIAL_MAPPINGS: usize = 4_095;
pub const MAX_STRING_BYTES: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoxelAssetDiagnostic {
    pub code: &'static str,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoxelAssetError {
    diagnostics: Vec<VoxelAssetDiagnostic>,
}

impl VoxelAssetError {
    pub fn diagnostics(&self) -> &[VoxelAssetDiagnostic] {
        &self.diagnostics
    }

    fn one(code: &'static str, path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            diagnostics: vec![diagnostic(code, path, message)],
        }
    }
}

impl std::fmt::Display for VoxelAssetError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let first = self
            .diagnostics
            .first()
            .expect("voxel asset error always has a diagnostic");
        write!(
            formatter,
            "{} at {}: {}",
            first.code, first.path, first.message
        )
    }
}

impl std::error::Error for VoxelAssetError {}

pub fn decode_voxel_asset(input: &str) -> Result<VoxelAsset, VoxelAssetError> {
    if input.len() > MAX_ARTIFACT_BYTES {
        return Err(VoxelAssetError::one(
            "voxelAsset.resourceLimit",
            "$",
            format!(
                "artifact has {} bytes; limit is {MAX_ARTIFACT_BYTES}",
                input.len()
            ),
        ));
    }
    let mut deserializer = serde_json::Deserializer::from_str(input);
    let asset: VoxelAsset =
        serde_path_to_error::deserialize(&mut deserializer).map_err(|error| {
            VoxelAssetError::one(
                "voxelAsset.decode",
                json_path(&error.path().to_string()),
                error.inner().to_string(),
            )
        })?;
    deserializer.end().map_err(|error| {
        VoxelAssetError::one(
            "voxelAsset.decode",
            "$",
            format!(
                "{} at line {}, column {}",
                error,
                error.line(),
                error.column()
            ),
        )
    })?;
    validate_voxel_asset(&asset)?;
    let mut canonical = asset;
    canonicalize(&mut canonical);
    Ok(canonical)
}

pub fn encode_voxel_asset(asset: &VoxelAsset) -> Result<String, VoxelAssetError> {
    validate_voxel_asset(asset)?;
    let mut canonical = asset.clone();
    canonicalize(&mut canonical);
    let mut encoded = serde_json::to_string_pretty(&canonical)
        .map_err(|error| VoxelAssetError::one("voxelAsset.encode", "$", error.to_string()))?;
    encoded.push('\n');
    if encoded.len() > MAX_ARTIFACT_BYTES {
        return Err(VoxelAssetError::one(
            "voxelAsset.resourceLimit",
            "$",
            format!(
                "encoded artifact has {} bytes; limit is {MAX_ARTIFACT_BYTES}",
                encoded.len()
            ),
        ));
    }
    Ok(encoded)
}

pub fn canonicalize_voxel_asset(asset: &VoxelAsset) -> Result<VoxelAsset, VoxelAssetError> {
    validate_voxel_asset(asset)?;
    let mut canonical = asset.clone();
    canonicalize(&mut canonical);
    Ok(canonical)
}

/// Populate the semantic content hash after validating every other field.
pub fn with_computed_content_hash(mut asset: VoxelAsset) -> Result<VoxelAsset, VoxelAssetError> {
    asset.content_hash.clear();
    let diagnostics = semantic_diagnostics(&asset);
    if !diagnostics.is_empty() {
        return Err(VoxelAssetError { diagnostics });
    }
    canonicalize(&mut asset);
    asset.content_hash = computed_content_hash(&asset);
    validate_voxel_asset(&asset)?;
    Ok(asset)
}

pub fn validate_voxel_asset(asset: &VoxelAsset) -> Result<(), VoxelAssetError> {
    let mut diagnostics = semantic_diagnostics(asset);
    if !valid_sha256(&asset.content_hash) {
        diagnostics.push(diagnostic(
            "voxelAsset.contentHashMismatch",
            "contentHash",
            "contentHash must be `sha256:` followed by 64 lowercase hexadecimal digits",
        ));
    } else if asset.content_hash != computed_content_hash(asset) {
        diagnostics.push(diagnostic(
            "voxelAsset.contentHashMismatch",
            "contentHash",
            "contentHash does not match the canonical semantic asset",
        ));
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(VoxelAssetError { diagnostics })
    }
}

fn semantic_diagnostics(asset: &VoxelAsset) -> Vec<VoxelAssetDiagnostic> {
    let mut diagnostics = Vec::new();
    if asset.schema_version != VOXEL_ASSET_SCHEMA_VERSION {
        diagnostics.push(diagnostic(
            "voxelAsset.unsupportedSchema",
            "schemaVersion",
            format!(
                "expected schema {VOXEL_ASSET_SCHEMA_VERSION}, found {}",
                asset.schema_version
            ),
        ));
    }
    match AssetId::parse(&asset.asset_id) {
        Ok(id) if id.kind() == AssetKind::VoxelVolume => {}
        Ok(id) => diagnostics.push(diagnostic(
            "voxelAsset.invalidAssetId",
            "assetId",
            format!("expected voxel-volume identity, found {}", id.kind()),
        )),
        Err(error) => diagnostics.push(diagnostic(
            "voxelAsset.invalidAssetId",
            "assetId",
            error.to_string(),
        )),
    }
    validate_grid(asset, &mut diagnostics);
    let output_materials = validate_material_map(&asset.material_map, &mut diagnostics);
    validate_provenance(asset, &mut diagnostics);
    validate_sparse_runs(asset, &output_materials, &mut diagnostics);
    diagnostics
}

fn validate_grid(asset: &VoxelAsset, diagnostics: &mut Vec<VoxelAssetDiagnostic>) {
    if !asset.grid.cell_size.is_finite() || asset.grid.cell_size <= 0.0 {
        diagnostics.push(diagnostic(
            "voxelAsset.invalidGrid",
            "grid.cellSize",
            "cellSize must be finite and greater than zero",
        ));
    }
    if !(1..=64).contains(&asset.grid.chunk_size) {
        diagnostics.push(diagnostic(
            "voxelAsset.invalidGrid",
            "grid.chunkSize",
            "chunkSize must be in 1..=64",
        ));
    }
    if asset
        .grid
        .origin
        .iter()
        .any(|coordinate| coordinate.unsigned_abs() > 1_000_000)
    {
        diagnostics.push(diagnostic(
            "voxelAsset.resourceLimit",
            "grid.origin",
            "origin coordinates must stay within +/-1,000,000 cells",
        ));
    }
    if asset
        .bounds
        .min
        .iter()
        .zip(asset.bounds.max)
        .any(|(min, max)| *min > max)
    {
        diagnostics.push(diagnostic(
            "voxelAsset.invalidBounds",
            "bounds",
            "inclusive bounds require min <= max on every axis",
        ));
    }
}

fn validate_material_map(
    mappings: &[VoxelAssetMaterialMapping],
    diagnostics: &mut Vec<VoxelAssetDiagnostic>,
) -> BTreeSet<u16> {
    if mappings.is_empty() || mappings.len() > MAX_MATERIAL_MAPPINGS {
        diagnostics.push(diagnostic(
            "voxelAsset.resourceLimit",
            "materialMap",
            format!("materialMap must contain 1..={MAX_MATERIAL_MAPPINGS} entries"),
        ));
    }
    let mut source_slots = BTreeSet::new();
    let mut output_slots = BTreeSet::new();
    for (index, mapping) in mappings.iter().enumerate() {
        if !source_slots.insert(mapping.source_material_slot) {
            diagnostics.push(diagnostic(
                "voxelAsset.duplicateMaterialMapping",
                format!("materialMap[{index}].sourceMaterialSlot"),
                "source material slots must be unique",
            ));
        }
        if !(1..=4_095).contains(&mapping.voxel_material_slot) {
            diagnostics.push(diagnostic(
                "voxelAsset.invalidMaterialMapping",
                format!("materialMap[{index}].voxelMaterialSlot"),
                "voxel material slots must be in 1..=4095",
            ));
        } else {
            output_slots.insert(mapping.voxel_material_slot);
        }
        if let Some(name) = &mapping.source_material_name {
            validate_string(
                name,
                format!("materialMap[{index}].sourceMaterialName"),
                diagnostics,
            );
        }
    }
    output_slots
}

fn validate_provenance(asset: &VoxelAsset, diagnostics: &mut Vec<VoxelAssetDiagnostic>) {
    validate_string(
        &asset.provenance.source_path,
        "provenance.sourcePath",
        diagnostics,
    );
    validate_string(
        &asset.provenance.converter,
        "provenance.converter",
        diagnostics,
    );
    if let Some(path) = &asset.provenance.license_path {
        validate_string(path, "provenance.licensePath", diagnostics);
    }
    if !valid_sha256(&asset.provenance.source_sha256) {
        diagnostics.push(diagnostic(
            "voxelAsset.invalidProvenance",
            "provenance.sourceSha256",
            "sourceSha256 must be `sha256:` followed by 64 lowercase hexadecimal digits",
        ));
    }
    if !valid_sha256(&asset.provenance.settings_sha256) {
        diagnostics.push(diagnostic(
            "voxelAsset.invalidProvenance",
            "provenance.settingsSha256",
            "settingsSha256 must be `sha256:` followed by 64 lowercase hexadecimal digits",
        ));
    }
    if asset.provenance.source_byte_count == 0
        || asset.provenance.source_byte_count > crate::MAX_CONVERSION_SOURCE_BYTES
    {
        diagnostics.push(diagnostic(
            "voxelAsset.resourceLimit",
            "provenance.sourceByteCount",
            format!(
                "sourceByteCount must be in 1..={}",
                crate::MAX_CONVERSION_SOURCE_BYTES
            ),
        ));
    }
}

fn validate_sparse_runs(
    asset: &VoxelAsset,
    output_materials: &BTreeSet<u16>,
    diagnostics: &mut Vec<VoxelAssetDiagnostic>,
) {
    if asset.representation.kind != VoxelRepresentationKind::SparseRuns {
        diagnostics.push(diagnostic(
            "voxelAsset.unsupportedRepresentation",
            "representation.kind",
            "schema 1 supports only sparseRuns",
        ));
    }
    if asset.representation.sparse_runs.is_empty() {
        diagnostics.push(diagnostic(
            "voxelAsset.invalidSparseRun",
            "representation.sparseRuns",
            "at least one solid sparse run is required",
        ));
        return;
    }

    let mut represented = 0usize;
    let mut actual_min = [i64::MAX; 3];
    let mut actual_max = [i64::MIN; 3];
    let mut rows = BTreeMap::<(i64, i64), Vec<(i64, i64, usize)>>::new();
    for (index, run) in asset.representation.sparse_runs.iter().enumerate() {
        if run.length == 0 {
            diagnostics.push(diagnostic(
                "voxelAsset.invalidSparseRun",
                format!("representation.sparseRuns[{index}].length"),
                "run length must be greater than zero",
            ));
            continue;
        }
        represented = represented.saturating_add(run.length as usize);
        let Some(end_x) = run.start[0].checked_add(i64::from(run.length) - 1) else {
            diagnostics.push(diagnostic(
                "voxelAsset.invalidSparseRun",
                format!("representation.sparseRuns[{index}]"),
                "run end coordinate overflowed",
            ));
            continue;
        };
        if !output_materials.contains(&run.material_slot) {
            diagnostics.push(diagnostic(
                "voxelAsset.unknownMaterial",
                format!("representation.sparseRuns[{index}].materialSlot"),
                format!(
                    "voxel material {} is not produced by materialMap",
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

        for axis in 0..3 {
            let endpoints = if axis == 0 {
                [run.start[axis], end_x]
            } else {
                [run.start[axis], run.start[axis]]
            };
            for local in endpoints {
                match asset.grid.origin[axis].checked_add(local) {
                    Some(address) if address.unsigned_abs() <= 1_000_000 => {}
                    _ => diagnostics.push(diagnostic(
                        "voxelAsset.resourceLimit",
                        format!("representation.sparseRuns[{index}].start[{axis}]"),
                        "mapped engine address exceeds +/-1,000,000 cells",
                    )),
                }
            }
        }
    }
    if represented > MAX_REPRESENTED_VOXELS {
        diagnostics.push(diagnostic(
            "voxelAsset.resourceLimit",
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
                    "voxelAsset.duplicateVoxel",
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
    if asset.bounds != actual_bounds {
        diagnostics.push(diagnostic(
            "voxelAsset.invalidBounds",
            "bounds",
            format!(
                "declared bounds {:?} do not equal represented bounds {:?}",
                asset.bounds, actual_bounds
            ),
        ));
    }
}

fn computed_content_hash(asset: &VoxelAsset) -> String {
    let mut canonical = asset.clone();
    canonical.content_hash.clear();
    canonicalize(&mut canonical);
    let bytes = serde_json::to_vec(&canonical).expect("voxel asset serializes");
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn canonicalize(asset: &mut VoxelAsset) {
    asset.material_map.sort_by(|left, right| {
        (
            left.source_material_slot,
            left.voxel_material_slot,
            &left.source_material_name,
        )
            .cmp(&(
                right.source_material_slot,
                right.voxel_material_slot,
                &right.source_material_name,
            ))
    });
    asset.representation.sparse_runs.sort_by_key(|run| {
        (
            run.start[1],
            run.start[2],
            run.start[0],
            run.material_slot,
            run.length,
        )
    });

    let mut merged: Vec<VoxelSparseRun> =
        Vec::with_capacity(asset.representation.sparse_runs.len());
    for run in asset.representation.sparse_runs.iter().copied() {
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
    asset.representation.sparse_runs = merged;
}

fn validate_string(
    value: &str,
    path: impl Into<String>,
    diagnostics: &mut Vec<VoxelAssetDiagnostic>,
) {
    let path = path.into();
    if value.trim().is_empty() || value.len() > MAX_STRING_BYTES {
        diagnostics.push(diagnostic(
            "voxelAsset.invalidString",
            path,
            format!("value must contain 1..={MAX_STRING_BYTES} UTF-8 bytes"),
        ));
    }
}

pub(crate) fn valid_sha256(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

fn diagnostic(
    code: &'static str,
    path: impl Into<String>,
    message: impl Into<String>,
) -> VoxelAssetDiagnostic {
    VoxelAssetDiagnostic {
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
