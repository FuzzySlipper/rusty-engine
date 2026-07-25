use core_assets::{AssetId, AssetKind};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use voxel_asset::{
    MAX_CONVERSION_SOURCE_BYTES, MAX_CONVERSION_SOURCE_INDICES, MAX_CONVERSION_SOURCE_VERTICES,
};

use crate::{import_static_glb, ConversionError, ImportedStaticMesh};

pub const MAX_MESH_SOURCE_ASSET_ID_BYTES: usize = 1_024;
pub const MAX_MESH_SOURCE_PATH_BYTES: usize = 8_192;
pub const MAX_MESH_PRIMITIVE_BYTES: usize = 1_024;
pub const MAX_MESH_IMPORT_REQUEST_BYTES: u64 = MAX_CONVERSION_SOURCE_BYTES * 4 + 32_768;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MeshSourceFormat {
    Glb,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct MeshSourceRef {
    pub asset_id: String,
    pub asset_version: u64,
    pub source_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mesh_primitive: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct MeshSourceImportRequest {
    pub source_asset_id: String,
    pub asset_version: u64,
    pub source_path: String,
    pub format: MeshSourceFormat,
    pub source_bytes: Vec<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_source_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mesh_primitive: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct MeshSourceBounds {
    pub min: [f64; 3],
    pub max: [f64; 3],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct MeshSourceMaterialSlot {
    pub source_material_slot: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_material_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct MeshSourceGroup {
    pub group_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub source_material_slot: u32,
    pub index_start: u32,
    pub index_count: u32,
    pub bounds: MeshSourceBounds,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct MeshSourceMetadata {
    pub source_bounds: MeshSourceBounds,
    pub vertex_count: u32,
    pub triangle_count: u32,
    pub groups: Vec<MeshSourceGroup>,
    pub material_slots: Vec<MeshSourceMaterialSlot>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct MeshSourceImportReceipt {
    pub source: MeshSourceRef,
    pub source_path: String,
    pub format: MeshSourceFormat,
    pub source_byte_count: u64,
    pub metadata: MeshSourceMetadata,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportedMeshSource {
    pub receipt: MeshSourceImportReceipt,
    pub mesh: ImportedStaticMesh,
}

pub fn source_sha256(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

pub fn import_mesh_source(
    request: &MeshSourceImportRequest,
) -> Result<ImportedMeshSource, ConversionError> {
    validate_import_request(request)?;
    let source_hash = source_sha256(&request.source_bytes);
    if request
        .expected_source_sha256
        .as_ref()
        .is_some_and(|expected| expected != &source_hash)
    {
        return Err(ConversionError::one(
            "conversion.sourceHashMismatch",
            "expectedSourceSha256",
            format!(
                "expected {}, computed {source_hash}",
                request
                    .expected_source_sha256
                    .as_deref()
                    .expect("checked expected source hash")
            ),
        ));
    }
    let mesh = match request.format {
        MeshSourceFormat::Glb => import_static_glb(&request.source_bytes)?,
    };
    let metadata = mesh_metadata(&mesh)?;
    Ok(ImportedMeshSource {
        receipt: MeshSourceImportReceipt {
            source: MeshSourceRef {
                asset_id: request.source_asset_id.clone(),
                asset_version: request.asset_version,
                source_sha256: source_hash,
                mesh_primitive: request.mesh_primitive.clone(),
            },
            source_path: request.source_path.clone(),
            format: request.format,
            source_byte_count: request.source_bytes.len() as u64,
            metadata,
        },
        mesh,
    })
}

pub fn decode_mesh_source_import_request(
    input: &str,
) -> Result<MeshSourceImportRequest, ConversionError> {
    if input.len() as u64 > MAX_MESH_IMPORT_REQUEST_BYTES {
        return Err(ConversionError::one(
            "conversion.resourceLimit",
            "$",
            format!(
                "mesh import request has {} bytes; limit is {MAX_MESH_IMPORT_REQUEST_BYTES}",
                input.len()
            ),
        ));
    }
    let mut deserializer = serde_json::Deserializer::from_str(input);
    let request = serde_path_to_error::deserialize(&mut deserializer).map_err(|error| {
        ConversionError::one(
            "conversion.requestDecode",
            if error.path().to_string().is_empty() {
                "$".to_string()
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

fn validate_import_request(request: &MeshSourceImportRequest) -> Result<(), ConversionError> {
    match AssetId::parse(&request.source_asset_id) {
        Ok(id) if id.kind() == AssetKind::StaticMesh => {}
        Ok(id) => {
            return Err(ConversionError::one(
                "conversion.invalidSourceIdentity",
                "sourceAssetId",
                format!("expected static mesh identity, found {}", id.kind()),
            ));
        }
        Err(error) => {
            return Err(ConversionError::one(
                "conversion.invalidSourceIdentity",
                "sourceAssetId",
                error.to_string(),
            ));
        }
    }
    if request.asset_version == 0 {
        return Err(ConversionError::one(
            "conversion.invalidSourceIdentity",
            "assetVersion",
            "assetVersion must be positive",
        ));
    }
    validate_string(
        &request.source_asset_id,
        "sourceAssetId",
        MAX_MESH_SOURCE_ASSET_ID_BYTES,
    )?;
    validate_string(
        &request.source_path,
        "sourcePath",
        MAX_MESH_SOURCE_PATH_BYTES,
    )?;
    if let Some(mesh_primitive) = &request.mesh_primitive {
        validate_string(mesh_primitive, "meshPrimitive", MAX_MESH_PRIMITIVE_BYTES)?;
    }
    if request.source_bytes.is_empty()
        || request.source_bytes.len() as u64 > MAX_CONVERSION_SOURCE_BYTES
    {
        return Err(ConversionError::one(
            "conversion.resourceLimit",
            "sourceBytes",
            format!(
                "source byte count {} is outside 1..={MAX_CONVERSION_SOURCE_BYTES}",
                request.source_bytes.len()
            ),
        ));
    }
    if let Some(expected) = &request.expected_source_sha256 {
        if !valid_sha256(expected) {
            return Err(ConversionError::one(
                "conversion.invalidSourceIdentity",
                "expectedSourceSha256",
                "expectedSourceSha256 must be a lowercase sha256 identity",
            ));
        }
    }
    Ok(())
}

fn validate_string(value: &str, path: &'static str, limit: usize) -> Result<(), ConversionError> {
    if value.trim().is_empty() || value.len() > limit {
        return Err(ConversionError::one(
            "conversion.invalidString",
            path,
            format!("value must contain 1..={limit} UTF-8 bytes"),
        ));
    }
    Ok(())
}

fn mesh_metadata(mesh: &ImportedStaticMesh) -> Result<MeshSourceMetadata, ConversionError> {
    if mesh.positions.len() > MAX_CONVERSION_SOURCE_VERTICES
        || mesh.triangles.len().saturating_mul(3) > MAX_CONVERSION_SOURCE_INDICES
    {
        return Err(ConversionError::one(
            "conversion.resourceLimit",
            "source.geometry",
            "canonical mesh geometry exceeds conversion limits",
        ));
    }
    let source_bounds = bounds_for_positions(&mesh.positions).ok_or_else(|| {
        ConversionError::one(
            "conversion.invalidGeometry",
            "source.positions",
            "mesh has no positions",
        )
    })?;
    let mut groups = Vec::with_capacity(mesh.primitive_groups.len());
    let mut expected_start = 0usize;
    for primitive in &mesh.primitive_groups {
        let group_start = primitive.triangle_start as usize;
        let group_end = group_start.saturating_add(primitive.triangle_count as usize);
        if primitive.triangle_count == 0
            || group_start != expected_start
            || group_end > mesh.triangles.len()
            || mesh.triangles[group_start..group_end]
                .iter()
                .any(|triangle| triangle.source_material_slot != primitive.source_material_slot)
        {
            return Err(ConversionError::one(
                "conversion.invalidGeometry",
                "source.groups",
                "primitive groups must be non-empty, ordered, exhaustive, and material-consistent",
            ));
        }
        let material_slot = primitive.source_material_slot;
        let group_positions = mesh.triangles[group_start..group_end]
            .iter()
            .flat_map(|triangle| triangle.indices)
            .map(|index| mesh.positions[index as usize])
            .collect::<Vec<_>>();
        let label = mesh
            .materials
            .iter()
            .find(|material| material.source_material_slot == material_slot)
            .and_then(|material| material.source_material_name.clone());
        groups.push(MeshSourceGroup {
            group_id: format!("group/{}", primitive.source_primitive_index),
            label,
            source_material_slot: material_slot,
            index_start: u32::try_from(group_start.saturating_mul(3)).map_err(|_| {
                ConversionError::one(
                    "conversion.resourceLimit",
                    "source.groups",
                    "group start exceeds u32",
                )
            })?,
            index_count: u32::try_from((group_end - group_start).saturating_mul(3)).map_err(
                |_| {
                    ConversionError::one(
                        "conversion.resourceLimit",
                        "source.groups",
                        "group count exceeds u32",
                    )
                },
            )?,
            bounds: bounds_for_positions(&group_positions).expect("group contains triangles"),
        });
        expected_start = group_end;
    }
    if expected_start != mesh.triangles.len() {
        return Err(ConversionError::one(
            "conversion.invalidGeometry",
            "source.groups",
            "primitive groups do not cover every imported triangle",
        ));
    }
    Ok(MeshSourceMetadata {
        source_bounds,
        vertex_count: u32::try_from(mesh.positions.len()).map_err(|_| {
            ConversionError::one(
                "conversion.resourceLimit",
                "source.positions",
                "vertex count exceeds u32",
            )
        })?,
        triangle_count: u32::try_from(mesh.triangles.len()).map_err(|_| {
            ConversionError::one(
                "conversion.resourceLimit",
                "source.triangles",
                "triangle count exceeds u32",
            )
        })?,
        groups,
        material_slots: mesh
            .materials
            .iter()
            .map(|material| MeshSourceMaterialSlot {
                source_material_slot: material.source_material_slot,
                source_material_name: material.source_material_name.clone(),
            })
            .collect(),
    })
}

fn bounds_for_positions(positions: &[[f64; 3]]) -> Option<MeshSourceBounds> {
    let first = *positions.first()?;
    let mut min = first;
    let mut max = first;
    for position in positions.iter().skip(1) {
        for axis in 0..3 {
            min[axis] = min[axis].min(position[axis]);
            max[axis] = max[axis].max(position[axis]);
        }
    }
    Some(MeshSourceBounds { min, max })
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
