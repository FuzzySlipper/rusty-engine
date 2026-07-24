use std::collections::BTreeSet;

use core_assets::{AssetId, AssetKind};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::codec::{valid_sha256, MAX_MATERIAL_MAPPINGS, MAX_REPRESENTED_VOXELS, MAX_STRING_BYTES};

pub const MAX_CONVERSION_SOURCE_BYTES: u64 = 8 * 1024 * 1024;
pub const MAX_CONVERSION_SOURCE_VERTICES: usize = 250_000;
pub const MAX_CONVERSION_SOURCE_INDICES: usize = 750_000;
pub const MAX_CONVERSION_RESOLUTION_AXIS: u32 = 256;
pub const MAX_CONVERSION_CELLS: u64 = 16_777_216;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelConversionRequest {
    pub asset_id: String,
    pub source_path: String,
    pub expected_source_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license_path: Option<String>,
    pub settings: VoxelConversionSettings,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelConversionSettings {
    pub resolution: [u32; 3],
    pub cell_size: f64,
    pub chunk_size: u32,
    pub origin: [i64; 3],
    pub fit_policy: VoxelConversionFitPolicy,
    pub origin_policy: VoxelConversionOriginPolicy,
    pub mode: VoxelConversionMode,
    pub material_map: Vec<crate::VoxelAssetMaterialMapping>,
    pub max_output_voxels: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VoxelConversionFitPolicy {
    Contain,
    Stretch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VoxelConversionOriginPolicy {
    TargetMin,
    Centered,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VoxelConversionMode {
    Surface,
    Solid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoxelConversionInputDiagnostic {
    pub code: &'static str,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoxelConversionInputError {
    diagnostics: Vec<VoxelConversionInputDiagnostic>,
}

impl VoxelConversionInputError {
    pub fn diagnostics(&self) -> &[VoxelConversionInputDiagnostic] {
        &self.diagnostics
    }
}

impl std::fmt::Display for VoxelConversionInputError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let first = self
            .diagnostics
            .first()
            .expect("conversion input error always has a diagnostic");
        write!(
            formatter,
            "{} at {}: {}",
            first.code, first.path, first.message
        )
    }
}

impl std::error::Error for VoxelConversionInputError {}

pub fn validate_conversion_request(
    request: &VoxelConversionRequest,
    source_byte_count: u64,
) -> Result<(), VoxelConversionInputError> {
    let mut diagnostics = Vec::new();
    match AssetId::parse(&request.asset_id) {
        Ok(id) if id.kind() == AssetKind::VoxelVolume => {}
        Ok(id) => diagnostics.push(input_diagnostic(
            "conversion.invalidAssetId",
            "assetId",
            format!("expected voxel-volume identity, found {}", id.kind()),
        )),
        Err(error) => diagnostics.push(input_diagnostic(
            "conversion.invalidAssetId",
            "assetId",
            error.to_string(),
        )),
    }
    validate_string(&request.source_path, "sourcePath", &mut diagnostics);
    if !valid_sha256(&request.expected_source_sha256) {
        diagnostics.push(input_diagnostic(
            "conversion.invalidSourceIdentity",
            "expectedSourceSha256",
            "expectedSourceSha256 must be `sha256:` followed by 64 lowercase hexadecimal digits",
        ));
    }
    if let Some(path) = &request.license_path {
        validate_string(path, "licensePath", &mut diagnostics);
    }
    if source_byte_count == 0 || source_byte_count > MAX_CONVERSION_SOURCE_BYTES {
        diagnostics.push(input_diagnostic(
            "conversion.resourceLimit",
            "source",
            format!(
                "source byte count {source_byte_count} is outside 1..={MAX_CONVERSION_SOURCE_BYTES}"
            ),
        ));
    }

    validate_settings(&request.settings, &mut diagnostics);
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(VoxelConversionInputError { diagnostics })
    }
}

pub fn conversion_settings_sha256(settings: &VoxelConversionSettings) -> String {
    let mut canonical = settings.clone();
    canonical.material_map.sort_by(|left, right| {
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
    let bytes = serde_json::to_vec(&canonical).expect("conversion settings serialize");
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn validate_settings(
    settings: &VoxelConversionSettings,
    diagnostics: &mut Vec<VoxelConversionInputDiagnostic>,
) {
    let cells = settings
        .resolution
        .into_iter()
        .try_fold(1u64, |total, axis| total.checked_mul(u64::from(axis)));
    if settings
        .resolution
        .iter()
        .any(|axis| !(1..=MAX_CONVERSION_RESOLUTION_AXIS).contains(axis))
        || cells.is_none_or(|cells| cells > MAX_CONVERSION_CELLS)
    {
        diagnostics.push(input_diagnostic(
            "conversion.resourceLimit",
            "settings.resolution",
            format!(
                "each resolution axis must be 1..={MAX_CONVERSION_RESOLUTION_AXIS} and the grid may contain at most {MAX_CONVERSION_CELLS} cells"
            ),
        ));
    }
    if !settings.cell_size.is_finite() || settings.cell_size <= 0.0 {
        diagnostics.push(input_diagnostic(
            "conversion.invalidSettings",
            "settings.cellSize",
            "cellSize must be finite and greater than zero",
        ));
    }
    if !(1..=64).contains(&settings.chunk_size) {
        diagnostics.push(input_diagnostic(
            "conversion.invalidSettings",
            "settings.chunkSize",
            "chunkSize must be in 1..=64",
        ));
    }
    if settings
        .origin
        .iter()
        .any(|coordinate| coordinate.unsigned_abs() > 1_000_000)
    {
        diagnostics.push(input_diagnostic(
            "conversion.resourceLimit",
            "settings.origin",
            "origin coordinates must stay within +/-1,000,000 cells",
        ));
    }
    for (axis, (origin, resolution)) in settings.origin.iter().zip(settings.resolution).enumerate()
    {
        let mapped_max = origin.checked_add(i64::from(resolution.saturating_sub(1)));
        if mapped_max.is_none_or(|coordinate| coordinate.unsigned_abs() > 1_000_000) {
            diagnostics.push(input_diagnostic(
                "conversion.resourceLimit",
                format!("settings.origin[{axis}]"),
                "origin plus resolution exceeds +/-1,000,000 cells",
            ));
        }
    }
    if settings.max_output_voxels == 0
        || settings.max_output_voxels as usize > MAX_REPRESENTED_VOXELS
    {
        diagnostics.push(input_diagnostic(
            "conversion.resourceLimit",
            "settings.maxOutputVoxels",
            format!("maxOutputVoxels must be in 1..={MAX_REPRESENTED_VOXELS}"),
        ));
    }
    if settings.material_map.is_empty() || settings.material_map.len() > MAX_MATERIAL_MAPPINGS {
        diagnostics.push(input_diagnostic(
            "conversion.invalidMaterialMap",
            "settings.materialMap",
            format!("materialMap must contain 1..={MAX_MATERIAL_MAPPINGS} entries"),
        ));
    }
    let mut source_slots = BTreeSet::new();
    for (index, mapping) in settings.material_map.iter().enumerate() {
        if !source_slots.insert(mapping.source_material_slot) {
            diagnostics.push(input_diagnostic(
                "conversion.invalidMaterialMap",
                format!("settings.materialMap[{index}].sourceMaterialSlot"),
                "source material slots must be unique",
            ));
        }
        if !(1..=4_095).contains(&mapping.voxel_material_slot) {
            diagnostics.push(input_diagnostic(
                "conversion.invalidMaterialMap",
                format!("settings.materialMap[{index}].voxelMaterialSlot"),
                "voxel material slots must be in 1..=4095",
            ));
        }
        if let Some(name) = &mapping.source_material_name {
            validate_string(
                name,
                format!("settings.materialMap[{index}].sourceMaterialName"),
                diagnostics,
            );
        }
    }
}

fn validate_string(
    value: &str,
    path: impl Into<String>,
    diagnostics: &mut Vec<VoxelConversionInputDiagnostic>,
) {
    let path = path.into();
    if value.trim().is_empty() || value.len() > MAX_STRING_BYTES {
        diagnostics.push(input_diagnostic(
            "conversion.invalidString",
            path,
            format!("value must contain 1..={MAX_STRING_BYTES} UTF-8 bytes"),
        ));
    }
}

fn input_diagnostic(
    code: &'static str,
    path: impl Into<String>,
    message: impl Into<String>,
) -> VoxelConversionInputDiagnostic {
    VoxelConversionInputDiagnostic {
        code,
        path: path.into(),
        message: message.into(),
    }
}
