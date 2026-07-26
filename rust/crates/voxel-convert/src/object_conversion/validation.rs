use std::collections::BTreeSet;

use core_assets::{AssetId, AssetKind};
use serde::Serialize;
use sha2::{Digest, Sha256};
use voxel_asset::{MAX_STRING_BYTES, MAX_VOXEL_OBJECT_CLIPS};

use super::{
    VoxelObjectClipConversionRequest, VoxelObjectConversionPlan, VoxelObjectConversionPlanRequest,
    VoxelObjectConversionSettings,
};
use crate::{plan_settings_sha256, ConversionError, ImportedMeshSource};

pub(super) fn validate_request(
    request: &VoxelObjectConversionPlanRequest,
    source: &ImportedMeshSource,
    expected_kind: AssetKind,
) -> Result<(), ConversionError> {
    if request.source != source.receipt.source || request.source_path != source.receipt.source_path
    {
        return Err(ConversionError::one(
            "conversion.unsupportedSource",
            "source",
            "plan source identity or path does not match the imported mesh snapshot",
        ));
    }
    match AssetId::parse(&request.source.asset_id) {
        Ok(id) if id.kind() == expected_kind => {}
        Ok(id) => {
            return Err(ConversionError::one(
                "conversion.unsupportedSource",
                "source.assetId",
                format!("expected {expected_kind} identity, found {}", id.kind()),
            ));
        }
        Err(error) => {
            return Err(ConversionError::one(
                "conversion.unsupportedSource",
                "source.assetId",
                error.to_string(),
            ));
        }
    }
    if request.target_asset_id.len() > MAX_STRING_BYTES {
        return Err(ConversionError::one(
            "conversion.invalidTargetIdentity",
            "targetAssetId",
            format!("target asset identity exceeds {MAX_STRING_BYTES} UTF-8 bytes"),
        ));
    }
    match AssetId::parse(&request.target_asset_id) {
        Ok(id) if id.kind() == AssetKind::VoxelObject => {}
        Ok(id) => {
            return Err(ConversionError::one(
                "conversion.invalidTargetIdentity",
                "targetAssetId",
                format!("expected voxel-object identity, found {}", id.kind()),
            ));
        }
        Err(error) => {
            return Err(ConversionError::one(
                "conversion.invalidTargetIdentity",
                "targetAssetId",
                error.to_string(),
            ));
        }
    }
    crate::planning::validate_transform(request.settings.mesh.transform)?;
    if request.settings.mesh.conversion.origin != [0, 0, 0] {
        return Err(ConversionError::one(
            "conversion.invalidObjectGrid",
            "settings.mesh.conversion.origin",
            "voxel-object conversion requires object-local origin [0,0,0]",
        ));
    }
    if request
        .settings
        .pivot
        .iter()
        .any(|value| !value.is_finite() || value.abs() > 1_000_000.0)
    {
        return Err(ConversionError::one(
            "conversion.invalidObjectGrid",
            "settings.pivot",
            "pivot components must be finite and stay within +/-1,000,000 cells",
        ));
    }
    validate_bounded_string(&request.source_path, "sourcePath")?;
    if let Some(path) = &request.license_path {
        validate_bounded_string(path, "licensePath")?;
    }
    canonical_clip_requests(request)?;
    Ok(())
}

pub(super) fn canonical_clip_requests(
    request: &VoxelObjectConversionPlanRequest,
) -> Result<Vec<VoxelObjectClipConversionRequest>, ConversionError> {
    if request.clips.len() > MAX_VOXEL_OBJECT_CLIPS {
        return Err(aggregate_limit(
            "selected clip count exceeds the durable object limit",
        ));
    }
    let mut output_ids = BTreeSet::new();
    for clip in &request.clips {
        validate_bounded_string(&clip.source_clip_name, "clips.sourceClipName")?;
        validate_clip_id(&clip.output_clip_id)?;
        if !output_ids.insert(clip.output_clip_id.as_str()) {
            return Err(ConversionError::one(
                "conversion.invalidClipSelection",
                "clips.outputClipId",
                "output clip identities must be unique",
            ));
        }
        if let Some(name) = &clip.output_name {
            validate_bounded_string(name, "clips.outputName")?;
        }
    }
    if request
        .default_clip
        .as_ref()
        .is_some_and(|clip| !output_ids.contains(clip.as_str()))
    {
        return Err(ConversionError::one(
            "conversion.invalidClipSelection",
            "defaultClip",
            "defaultClip must name one selected output clip",
        ));
    }
    let mut clips = request.clips.clone();
    clips.sort_by(|left, right| left.output_clip_id.cmp(&right.output_clip_id));
    Ok(clips)
}

pub(super) fn object_settings_sha256(
    settings: &VoxelObjectConversionSettings,
    clips: &[VoxelObjectClipConversionRequest],
    default_clip: &Option<String>,
) -> String {
    let mut canonical_clips = clips.to_vec();
    canonical_clips.sort_by(|left, right| left.output_clip_id.cmp(&right.output_clip_id));
    sha256_json(&(
        plan_settings_sha256(&settings.mesh),
        settings.pivot,
        settings.anchor_policy,
        canonical_clips,
        default_clip,
    ))
}

pub(super) fn object_plan_id(
    request: &VoxelObjectConversionPlanRequest,
    settings_sha256: &str,
) -> String {
    sha256_json(&(
        "voxel-object-conversion-plan",
        &request.source,
        &request.source_path,
        &request.target_asset_id,
        &request.license_path,
        settings_sha256,
    ))
}

pub(super) fn object_plan_id_from_plan(plan: &VoxelObjectConversionPlan) -> String {
    sha256_json(&(
        "voxel-object-conversion-plan",
        &plan.source,
        &plan.source_path,
        &plan.target_asset_id,
        &plan.license_path,
        &plan.settings_sha256,
    ))
}

pub(super) fn aggregate_limit(message: &str) -> ConversionError {
    ConversionError::one("conversion.resourceLimit", "clips", message)
}

fn validate_clip_id(value: &str) -> Result<(), ConversionError> {
    let valid = !value.is_empty()
        && value.len() <= MAX_STRING_BYTES
        && value.split('/').all(|segment| {
            !segment.is_empty()
                && !segment.starts_with('-')
                && !segment.ends_with('-')
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        });
    if valid {
        Ok(())
    } else {
        Err(ConversionError::one(
            "conversion.invalidClipSelection",
            "clips.outputClipId",
            "output clip ids must be scoped kebab-case values",
        ))
    }
}

fn validate_bounded_string(value: &str, path: &'static str) -> Result<(), ConversionError> {
    if value.trim().is_empty() || value.len() > MAX_STRING_BYTES {
        return Err(ConversionError::one(
            "conversion.invalidString",
            path,
            format!("value must contain 1..={MAX_STRING_BYTES} UTF-8 bytes"),
        ));
    }
    Ok(())
}

pub(super) fn sha256_json(value: &impl Serialize) -> String {
    let bytes = serde_json::to_vec(value).expect("voxel-object planning models serialize");
    format!("sha256:{:x}", Sha256::digest(bytes))
}
