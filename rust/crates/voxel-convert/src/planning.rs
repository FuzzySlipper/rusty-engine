use std::path::Path;

use core_assets::{AssetId, AssetKind};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use voxel_asset::{VoxelAssetBounds, VoxelConversionRequest, VoxelConversionSettings};

use crate::{
    convert::{convert_imported_mesh, replace_settings_identity},
    material::{canonicalize_material_policy, resolve_material_map, ConversionMaterialPolicy},
    query::occupied_voxels,
    store::install_canonical_asset,
    ConversionError, ConversionReceipt, ImportedMeshSource, ImportedStaticMesh, MeshSourceRef,
};

pub const CONVERSION_PLANNER_ID: &str = "rusty-engine.voxel-conversion.v1";
pub const MAX_CONVERSION_PREVIEW_SAMPLES: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ConversionPlanSettings {
    pub conversion: VoxelConversionSettings,
    pub transform: [f64; 16],
    pub material_policy: ConversionMaterialPolicy,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ConversionPlanRequest {
    pub source: MeshSourceRef,
    pub target_asset_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license_path: Option<String>,
    pub settings: ConversionPlanSettings,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelConversionPlan {
    pub plan_id: String,
    pub source: MeshSourceRef,
    pub target_asset_id: String,
    pub settings: ConversionPlanSettings,
    pub planner: String,
    pub expected_source_sha256: String,
    pub settings_sha256: String,
    pub plan_hash: String,
    pub estimated_output_voxels: usize,
    pub estimated_bounds: VoxelAssetBounds,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreparedVoxelConversion {
    pub plan: VoxelConversionPlan,
    output: ConversionReceipt,
}

impl PreparedVoxelConversion {
    pub fn candidate(&self) -> &ConversionReceipt {
        &self.output
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ConversionPreviewRequest {
    pub plan_id: String,
    pub expected_plan_hash: String,
    pub max_samples: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ConversionPreviewVoxel {
    pub coordinate: [i64; 3],
    pub material_slot: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelConversionPreview {
    pub plan_id: String,
    pub plan_hash: String,
    pub output_hash: String,
    pub output_voxel_count: usize,
    pub output_bounds: VoxelAssetBounds,
    pub sample_voxels: Vec<ConversionPreviewVoxel>,
    pub samples_truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ConversionApplyRequest {
    pub plan_id: String,
    pub expected_plan_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_output_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppliedVoxelConversion {
    pub plan_id: String,
    pub plan_hash: String,
    pub output_hash: String,
    pub conversion: ConversionReceipt,
}

pub fn identity_transform() -> [f64; 16] {
    [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ]
}

pub fn plan_conversion(
    request: &ConversionPlanRequest,
    source: &ImportedMeshSource,
) -> Result<PreparedVoxelConversion, ConversionError> {
    validate_source(request, source)?;
    validate_target(&request.target_asset_id)?;
    validate_transform(request.settings.transform)?;

    let mut effective_settings = request.settings.conversion.clone();
    effective_settings.material_map = resolve_material_map(request, source)?;
    let transformed_mesh = transform_mesh(&source.mesh, request.settings.transform)?;
    let conversion_request = VoxelConversionRequest {
        asset_id: request.target_asset_id.clone(),
        source_path: source.receipt.source_path.clone(),
        expected_source_sha256: source.receipt.source.source_sha256.clone(),
        license_path: request.license_path.clone(),
        settings: effective_settings,
    };
    let settings_sha256 = plan_settings_sha256(&request.settings);
    let output = replace_settings_identity(
        convert_imported_mesh(
            &conversion_request,
            &transformed_mesh,
            source.receipt.source.source_sha256.clone(),
            source.receipt.source_byte_count,
        )?,
        settings_sha256.clone(),
    )?;

    let plan_id = sha256_json(&(
        "voxel-conversion-plan",
        &request.source,
        &request.target_asset_id,
        &settings_sha256,
    ));
    let mut plan = VoxelConversionPlan {
        plan_id,
        source: request.source.clone(),
        target_asset_id: request.target_asset_id.clone(),
        settings: request.settings.clone(),
        planner: CONVERSION_PLANNER_ID.to_string(),
        expected_source_sha256: source.receipt.source.source_sha256.clone(),
        settings_sha256,
        plan_hash: String::new(),
        estimated_output_voxels: output.output_voxels,
        estimated_bounds: output.bounds,
    };
    plan.plan_hash = conversion_plan_hash(&plan);
    Ok(PreparedVoxelConversion { plan, output })
}

pub fn conversion_plan_hash(plan: &VoxelConversionPlan) -> String {
    sha256_json(&(
        &plan.plan_id,
        &plan.source,
        &plan.target_asset_id,
        &plan.settings,
        &plan.planner,
        &plan.expected_source_sha256,
        &plan.settings_sha256,
        plan.estimated_output_voxels,
        plan.estimated_bounds,
    ))
}

pub fn preview_conversion(
    request: &ConversionPreviewRequest,
    prepared: &PreparedVoxelConversion,
) -> Result<VoxelConversionPreview, ConversionError> {
    validate_prepared_identity(&request.plan_id, &request.expected_plan_hash, prepared)?;
    let max_samples = request.max_samples as usize;
    if !(1..=MAX_CONVERSION_PREVIEW_SAMPLES).contains(&max_samples) {
        return Err(ConversionError::one(
            "conversion.queryQuotaExceeded",
            "maxSamples",
            format!("maxSamples must be in 1..={MAX_CONVERSION_PREVIEW_SAMPLES}"),
        ));
    }
    let occupied = occupied_voxels(&prepared.output.asset);
    let mut ordered = occupied.into_iter().collect::<Vec<_>>();
    ordered.sort_by_key(|(coordinate, _)| (coordinate[2], coordinate[1], coordinate[0]));
    let sample_voxels = ordered
        .iter()
        .take(max_samples)
        .map(|(coordinate, material_slot)| ConversionPreviewVoxel {
            coordinate: *coordinate,
            material_slot: *material_slot,
        })
        .collect();
    Ok(VoxelConversionPreview {
        plan_id: prepared.plan.plan_id.clone(),
        plan_hash: prepared.plan.plan_hash.clone(),
        output_hash: prepared.output.asset.voxel_data_hash.clone(),
        output_voxel_count: prepared.output.output_voxels,
        output_bounds: prepared.output.bounds,
        sample_voxels,
        samples_truncated: ordered.len() > max_samples,
    })
}

pub fn apply_conversion(
    request: &ConversionApplyRequest,
    prepared: &PreparedVoxelConversion,
) -> Result<AppliedVoxelConversion, ConversionError> {
    validate_prepared_identity(&request.plan_id, &request.expected_plan_hash, prepared)?;
    let output_hash = prepared.output.asset.voxel_data_hash.clone();
    if request
        .expected_output_hash
        .as_ref()
        .is_some_and(|expected| expected != &output_hash)
    {
        return Err(ConversionError::one(
            "conversion.staleOutput",
            "expectedOutputHash",
            "apply request expected a different prepared voxel output",
        ));
    }
    Ok(AppliedVoxelConversion {
        plan_id: prepared.plan.plan_id.clone(),
        plan_hash: prepared.plan.plan_hash.clone(),
        output_hash,
        conversion: prepared.output.clone(),
    })
}

pub fn apply_conversion_and_install(
    request: &ConversionApplyRequest,
    prepared: &PreparedVoxelConversion,
    output_path: &Path,
) -> Result<AppliedVoxelConversion, ConversionError> {
    let applied = apply_conversion(request, prepared)?;
    install_canonical_asset(&applied.conversion.canonical_json, output_path)?;
    Ok(applied)
}

pub fn plan_settings_sha256(settings: &ConversionPlanSettings) -> String {
    let mut canonical = settings.clone();
    canonical
        .conversion
        .material_palette
        .sort_by(|left, right| {
            (
                left.material_slot,
                &left.material_asset_id,
                &left.display_name,
            )
                .cmp(&(
                    right.material_slot,
                    &right.material_asset_id,
                    &right.display_name,
                ))
        });
    canonical.conversion.material_map.sort_by(|left, right| {
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
    canonicalize_material_policy(&mut canonical.material_policy);
    sha256_json(&canonical)
}

fn validate_source(
    request: &ConversionPlanRequest,
    source: &ImportedMeshSource,
) -> Result<(), ConversionError> {
    if request.source.asset_id != source.receipt.source.asset_id
        || request.source.asset_version != source.receipt.source.asset_version
        || request.source.mesh_primitive != source.receipt.source.mesh_primitive
    {
        return Err(ConversionError::one(
            "conversion.unsupportedSource",
            "source",
            "plan source identity does not match the imported static mesh",
        ));
    }
    if request.source.source_sha256 != source.receipt.source.source_sha256 {
        return Err(ConversionError::one(
            "conversion.sourceHashMismatch",
            "source.sourceSha256",
            "plan source hash does not match the imported static mesh snapshot",
        ));
    }
    Ok(())
}

fn validate_target(target_asset_id: &str) -> Result<(), ConversionError> {
    match AssetId::parse(target_asset_id) {
        Ok(id) if id.kind() == AssetKind::VoxelVolume => Ok(()),
        Ok(id) => Err(ConversionError::one(
            "conversion.invalidTargetIdentity",
            "targetAssetId",
            format!("expected voxel-volume identity, found {}", id.kind()),
        )),
        Err(error) => Err(ConversionError::one(
            "conversion.invalidTargetIdentity",
            "targetAssetId",
            error.to_string(),
        )),
    }
}

fn validate_transform(transform: [f64; 16]) -> Result<(), ConversionError> {
    if transform.iter().any(|value| !value.is_finite())
        || transform[3].abs() > f64::EPSILON
        || transform[7].abs() > f64::EPSILON
        || transform[11].abs() > f64::EPSILON
        || (transform[15] - 1.0).abs() > f64::EPSILON
    {
        return Err(ConversionError::one(
            "conversion.invalidTransform",
            "settings.transform",
            "transform must be a finite affine column-major matrix",
        ));
    }
    Ok(())
}

fn transform_mesh(
    mesh: &ImportedStaticMesh,
    transform: [f64; 16],
) -> Result<ImportedStaticMesh, ConversionError> {
    let mut transformed = mesh.clone();
    for position in &mut transformed.positions {
        let [x, y, z] = *position;
        *position = [
            transform[0] * x + transform[4] * y + transform[8] * z + transform[12],
            transform[1] * x + transform[5] * y + transform[9] * z + transform[13],
            transform[2] * x + transform[6] * y + transform[10] * z + transform[14],
        ];
        if position.iter().any(|component| !component.is_finite()) {
            return Err(ConversionError::one(
                "conversion.invalidTransform",
                "settings.transform",
                "transform produced a non-finite source position",
            ));
        }
    }
    Ok(transformed)
}

fn validate_prepared_identity(
    plan_id: &str,
    expected_plan_hash: &str,
    prepared: &PreparedVoxelConversion,
) -> Result<(), ConversionError> {
    if plan_id != prepared.plan.plan_id
        || expected_plan_hash != prepared.plan.plan_hash
        || conversion_plan_hash(&prepared.plan) != prepared.plan.plan_hash
    {
        return Err(ConversionError::one(
            "conversion.stalePlan",
            "plan",
            "request does not match the prepared conversion plan",
        ));
    }
    Ok(())
}

fn sha256_json(value: &impl Serialize) -> String {
    let bytes = serde_json::to_vec(value).expect("conversion planning models serialize");
    format!("sha256:{:x}", Sha256::digest(bytes))
}
