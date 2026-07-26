use std::path::Path;

use core_assets::{AssetId, AssetKind};

use super::{
    apply_voxel_object_conversion_and_install, plan_animated_voxel_object_conversion,
    plan_static_voxel_object_conversion, VoxelObjectConversionApplyRequest,
    VoxelObjectConversionPlanRequest, VoxelObjectConversionReceipt,
    MAX_VOXEL_OBJECT_CONVERSION_REQUEST_BYTES,
};
use crate::{
    import_animated_mesh_source, import_mesh_source, ConversionError, MeshSourceFormat,
    MeshSourceImportRequest,
};

pub fn decode_voxel_object_conversion_request(
    input: &str,
) -> Result<VoxelObjectConversionPlanRequest, ConversionError> {
    if input.len() > MAX_VOXEL_OBJECT_CONVERSION_REQUEST_BYTES {
        return Err(ConversionError::one(
            "conversion.resourceLimit",
            "$",
            format!(
                "request has {} bytes; limit is {MAX_VOXEL_OBJECT_CONVERSION_REQUEST_BYTES}",
                input.len()
            ),
        ));
    }
    let mut deserializer = serde_json::Deserializer::from_str(input);
    let request = serde_path_to_error::deserialize(&mut deserializer).map_err(|error| {
        ConversionError::one(
            "conversion.requestDecode",
            if error.path().to_string().is_empty() {
                "$".to_owned()
            } else {
                error.path().to_string()
            },
            error.inner().to_string(),
        )
    })?;
    deserializer.end().map_err(|error| {
        ConversionError::one(
            "conversion.requestDecode",
            "$",
            format!(
                "{} at line {}, column {}",
                error,
                error.line(),
                error.column()
            ),
        )
    })?;
    Ok(request)
}

pub fn convert_voxel_object_and_install(
    request: &VoxelObjectConversionPlanRequest,
    source_bytes: &[u8],
    output_path: &Path,
) -> Result<VoxelObjectConversionReceipt, ConversionError> {
    let source_kind = AssetId::parse(&request.source.asset_id)
        .map_err(|error| {
            ConversionError::one(
                "conversion.invalidSourceIdentity",
                "source.assetId",
                error.to_string(),
            )
        })?
        .kind();
    let import_request = MeshSourceImportRequest {
        source_asset_id: request.source.asset_id.clone(),
        asset_version: request.source.asset_version,
        source_path: request.source_path.clone(),
        format: MeshSourceFormat::Glb,
        source_bytes: source_bytes.to_vec(),
        expected_source_sha256: Some(request.source.source_sha256.clone()),
        mesh_primitive: request.source.mesh_primitive.clone(),
    };
    let prepared = match source_kind {
        AssetKind::StaticMesh => {
            let source = import_mesh_source(&import_request)?;
            plan_static_voxel_object_conversion(request, &source)?
        }
        AssetKind::AnimatedMesh => {
            let source = import_animated_mesh_source(&import_request)?;
            plan_animated_voxel_object_conversion(request, &source)?
        }
        other => {
            return Err(ConversionError::one(
                "conversion.unsupportedSource",
                "source.assetId",
                format!("expected mesh or mesh-animation identity, found {other}"),
            ));
        }
    };
    let applied = apply_voxel_object_conversion_and_install(
        &VoxelObjectConversionApplyRequest {
            plan_id: prepared.plan().plan_id.clone(),
            expected_plan_hash: prepared.plan().plan_hash.clone(),
            expected_output_hash: Some(prepared.plan().expected_output_content_hash.clone()),
        },
        &prepared,
        output_path,
    )?;
    Ok(applied.conversion)
}
