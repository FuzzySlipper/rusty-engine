use core_assets::{AssetId, AssetKind};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use voxel_asset::{
    MAX_CONVERSION_SOURCE_BYTES, MAX_CONVERSION_SOURCE_INDICES, MAX_CONVERSION_SOURCE_VERTICES,
};

use crate::{
    flatten_static_scene, import_animated_glb, import_static_glb_scene, sample_animation_bind_pose,
    texture_coordinate_source_hash, AnimationAnchorPolicy, AnimationBindPoseRequest,
    ConversionError, ImportedAnimatedModel, ImportedModelScene, ImportedPrimitiveGroup,
    ImportedStaticMesh, ImportedStaticTextureCoordinates,
};

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
    pub source_node_index: u32,
    pub source_mesh_index: u32,
    pub source_primitive_index: u32,
    pub index_start: u32,
    pub index_count: u32,
    pub bounds: MeshSourceBounds,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct MeshSourceNode {
    pub node_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub source_node_index: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_source_node_index: Option<u32>,
    pub child_source_node_indices: Vec<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_mesh_index: Option<u32>,
    pub local_transform: [f64; 16],
    pub model_transform: [f64; 16],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct MeshSourceTextureCoordinates {
    pub attribute_name: String,
    pub source_set_index: u32,
    pub source_hash: String,
    pub vertex_count: u32,
    pub missing_vertex_count: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct MeshSourceMetadata {
    pub source_scene_index: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_scene_name: Option<String>,
    pub source_bounds: MeshSourceBounds,
    pub vertex_count: u32,
    pub triangle_count: u32,
    pub groups: Vec<MeshSourceGroup>,
    pub material_slots: Vec<MeshSourceMaterialSlot>,
    pub nodes: Vec<MeshSourceNode>,
    pub texture_coordinates: Vec<MeshSourceTextureCoordinates>,
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
    pub scene: ImportedModelScene,
    pub mesh: ImportedStaticMesh,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportedAnimatedMeshSource {
    /// Bind-pose source view used for common metadata and material admission.
    pub source: ImportedMeshSource,
    /// Authority-bearing animation data used by explicit clip sampling.
    pub model: ImportedAnimatedModel,
}

pub fn source_sha256(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

pub fn import_mesh_source(
    request: &MeshSourceImportRequest,
) -> Result<ImportedMeshSource, ConversionError> {
    validate_import_request(request, AssetKind::StaticMesh)?;
    let source_hash = source_sha256(&request.source_bytes);
    validate_expected_source_hash(request, &source_hash)?;
    let scene = match request.format {
        MeshSourceFormat::Glb => import_static_glb_scene(&request.source_bytes)?,
    };
    let mesh = flatten_static_scene(&scene)?;
    let mesh = match request.mesh_primitive.as_deref() {
        Some(group_id) => select_primitive_group(mesh, group_id)?,
        None => mesh,
    };
    let metadata = mesh_metadata(&scene, &mesh)?;
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
        scene,
        mesh,
    })
}

pub fn import_animated_mesh_source(
    request: &MeshSourceImportRequest,
) -> Result<ImportedAnimatedMeshSource, ConversionError> {
    import_animated_mesh_source_with(request, false)
}

/// Imports the exact animated source for runtime visual-resource metadata.
/// Degenerate faces are omitted only from the derived bind-pose mesh; ordinary
/// voxel conversion and animation sampling remain strict.
pub fn import_animated_mesh_source_for_visual_metadata(
    request: &MeshSourceImportRequest,
) -> Result<ImportedAnimatedMeshSource, ConversionError> {
    import_animated_mesh_source_with(request, true)
}

fn import_animated_mesh_source_with(
    request: &MeshSourceImportRequest,
    visual_metadata: bool,
) -> Result<ImportedAnimatedMeshSource, ConversionError> {
    validate_import_request(request, AssetKind::AnimatedMesh)?;
    if request.mesh_primitive.is_some() {
        return Err(ConversionError::one(
            "conversion.unsupportedSource",
            "meshPrimitive",
            "animated object conversion requires the complete selected scene",
        ));
    }
    let source_hash = source_sha256(&request.source_bytes);
    validate_expected_source_hash(request, &source_hash)?;
    let model = match request.format {
        MeshSourceFormat::Glb => import_animated_glb(&request.source_bytes)?,
    };
    let bind_request = AnimationBindPoseRequest {
        expected_source_sha256: source_hash.clone(),
        anchor_policy: AnimationAnchorPolicy::PreserveSourceSpace,
    };
    let bind_pose = if visual_metadata {
        crate::animation::sample_animation_bind_pose_for_visual_metadata(&model, &bind_request)?
    } else {
        sample_animation_bind_pose(&model, &bind_request)?
    };
    let metadata = mesh_metadata(&model.scene, &bind_pose.mesh)?;
    let source = ImportedMeshSource {
        receipt: MeshSourceImportReceipt {
            source: MeshSourceRef {
                asset_id: request.source_asset_id.clone(),
                asset_version: request.asset_version,
                source_sha256: source_hash,
                mesh_primitive: None,
            },
            source_path: request.source_path.clone(),
            format: request.format,
            source_byte_count: request.source_bytes.len() as u64,
            metadata,
        },
        scene: model.scene.clone(),
        mesh: bind_pose.mesh,
    };
    Ok(ImportedAnimatedMeshSource { source, model })
}

fn select_primitive_group(
    mesh: ImportedStaticMesh,
    group_id: &str,
) -> Result<ImportedStaticMesh, ConversionError> {
    let selected_groups = if let Some(index) = group_id
        .strip_prefix("group/")
        .and_then(|value| value.parse::<usize>().ok())
    {
        mesh.primitive_groups
            .get(index)
            .copied()
            .into_iter()
            .collect::<Vec<_>>()
    } else if let Some(node_index) = group_id
        .strip_prefix("node/")
        .and_then(|value| value.parse::<u32>().ok())
    {
        mesh.primitive_groups
            .iter()
            .filter(|group| group.source_node_index == node_index)
            .copied()
            .collect::<Vec<_>>()
    } else {
        return Err(ConversionError::one(
            "conversion.unknownMeshPrimitive",
            "meshPrimitive",
            "meshPrimitive must name one imported group or mesh node such as group/0 or node/0",
        ));
    };
    if selected_groups.is_empty() {
        return Err(ConversionError::one(
            "conversion.unknownMeshPrimitive",
            "meshPrimitive",
            format!("source scene has no selectable subset `{group_id}`"),
        ));
    }
    let selected = selected_groups
        .iter()
        .flat_map(|group| {
            let start = group.triangle_start as usize;
            let end = start.saturating_add(group.triangle_count as usize);
            mesh.triangles[start..end].iter().copied()
        })
        .collect::<Vec<_>>();
    let mut remap = std::collections::BTreeMap::<u32, u32>::new();
    for index in selected.iter().flat_map(|triangle| triangle.indices) {
        let next = remap.len() as u32;
        remap.entry(index).or_insert(next);
    }
    let mut positions = vec![[0.0; 3]; remap.len()];
    for (source, target) in &remap {
        positions[*target as usize] = mesh.positions[*source as usize];
    }
    let triangles = selected
        .iter()
        .map(|triangle| crate::ImportedTriangle {
            indices: triangle.indices.map(|index| remap[&index]),
            source_material_slot: triangle.source_material_slot,
        })
        .collect::<Vec<_>>();
    let mut triangle_start = 0u32;
    let primitive_groups = selected_groups
        .iter()
        .map(|group| {
            let selected = ImportedPrimitiveGroup {
                triangle_start,
                ..*group
            };
            triangle_start = triangle_start.saturating_add(group.triangle_count);
            selected
        })
        .collect::<Vec<_>>();
    let selected_materials = selected_groups
        .iter()
        .map(|group| group.source_material_slot)
        .collect::<std::collections::BTreeSet<_>>();
    let materials = mesh
        .materials
        .into_iter()
        .filter(|material| selected_materials.contains(&material.source_material_slot))
        .collect::<Vec<_>>();
    let texture_coordinates = mesh
        .texture_coordinates
        .into_iter()
        .map(|source| {
            let mut coordinates = vec![None; remap.len()];
            for (source_index, target_index) in &remap {
                coordinates[*target_index as usize] = source.coordinates[*source_index as usize];
            }
            ImportedStaticTextureCoordinates {
                source_set_index: source.source_set_index,
                coordinates,
            }
        })
        .collect();
    Ok(ImportedStaticMesh {
        positions,
        texture_coordinates,
        triangles,
        primitive_groups,
        materials,
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

fn validate_import_request(
    request: &MeshSourceImportRequest,
    expected_kind: AssetKind,
) -> Result<(), ConversionError> {
    match AssetId::parse(&request.source_asset_id) {
        Ok(id) if id.kind() == expected_kind => {}
        Ok(id) => {
            return Err(ConversionError::one(
                "conversion.invalidSourceIdentity",
                "sourceAssetId",
                format!("expected {expected_kind} identity, found {}", id.kind()),
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

fn validate_expected_source_hash(
    request: &MeshSourceImportRequest,
    source_hash: &str,
) -> Result<(), ConversionError> {
    if request
        .expected_source_sha256
        .as_ref()
        .is_some_and(|expected| expected != source_hash)
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

pub(crate) fn mesh_metadata(
    scene: &ImportedModelScene,
    mesh: &ImportedStaticMesh,
) -> Result<MeshSourceMetadata, ConversionError> {
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
    for (group_index, primitive) in mesh.primitive_groups.iter().enumerate() {
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
        let material_label = mesh
            .materials
            .iter()
            .find(|material| material.source_material_slot == material_slot)
            .and_then(|material| material.source_material_name.as_deref());
        let node = scene
            .nodes
            .iter()
            .find(|node| node.source_node_index == primitive.source_node_index);
        let source_mesh = scene
            .meshes
            .iter()
            .find(|mesh| mesh.source_mesh_index == primitive.source_mesh_index);
        let label = [
            node.and_then(|node| node.source_node_name.as_deref()),
            source_mesh.and_then(|mesh| mesh.source_mesh_name.as_deref()),
            material_label,
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
        groups.push(MeshSourceGroup {
            group_id: format!("group/{group_index}"),
            label: (!label.is_empty()).then(|| label.join(" / ")),
            source_material_slot: material_slot,
            source_node_index: primitive.source_node_index,
            source_mesh_index: primitive.source_mesh_index,
            source_primitive_index: primitive.source_primitive_index,
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
        source_scene_index: scene.source_scene_index,
        source_scene_name: scene.source_scene_name.clone(),
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
        nodes: scene
            .nodes
            .iter()
            .map(|node| MeshSourceNode {
                node_id: format!("node/{}", node.source_node_index),
                label: node.source_node_name.clone(),
                source_node_index: node.source_node_index,
                parent_source_node_index: node.parent_node_index,
                child_source_node_indices: node.child_node_indices.clone(),
                source_mesh_index: node.source_mesh_index,
                local_transform: node.local_transform,
                model_transform: node.model_transform,
            })
            .collect(),
        texture_coordinates: mesh
            .texture_coordinates
            .iter()
            .map(|coordinates| {
                Ok(MeshSourceTextureCoordinates {
                    attribute_name: format!("TEXCOORD_{}", coordinates.source_set_index),
                    source_set_index: coordinates.source_set_index,
                    source_hash: texture_coordinate_source_hash(mesh, coordinates.source_set_index)
                        .expect("iterated texture coordinate set exists"),
                    vertex_count: u32::try_from(coordinates.coordinates.len()).map_err(|_| {
                        ConversionError::one(
                            "conversion.resourceLimit",
                            "source.textureCoordinates",
                            "texture coordinate count exceeds u32",
                        )
                    })?,
                    missing_vertex_count: u32::try_from(
                        coordinates
                            .coordinates
                            .iter()
                            .filter(|value| value.is_none())
                            .count(),
                    )
                    .map_err(|_| {
                        ConversionError::one(
                            "conversion.resourceLimit",
                            "source.textureCoordinates",
                            "missing texture coordinate count exceeds u32",
                        )
                    })?,
                })
            })
            .collect::<Result<Vec<_>, ConversionError>>()?,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ImportedMaterial, ImportedPrimitiveGroup, ImportedTriangle};

    #[test]
    fn primitive_group_selection_compacts_geometry_and_rejects_unknown_groups() {
        let mesh = ImportedStaticMesh {
            positions: vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [10.0, 0.0, 0.0],
                [11.0, 0.0, 0.0],
                [10.0, 1.0, 0.0],
            ],
            texture_coordinates: vec![ImportedStaticTextureCoordinates {
                source_set_index: 0,
                coordinates: vec![
                    Some([0.0, 0.0]),
                    Some([1.0, 0.0]),
                    Some([0.0, 1.0]),
                    Some([0.0, 0.0]),
                    Some([1.0, 0.0]),
                    Some([0.0, 1.0]),
                ],
            }],
            triangles: vec![
                ImportedTriangle {
                    indices: [0, 1, 2],
                    source_material_slot: 2,
                },
                ImportedTriangle {
                    indices: [3, 4, 5],
                    source_material_slot: 7,
                },
            ],
            primitive_groups: vec![
                ImportedPrimitiveGroup {
                    source_node_index: 0,
                    source_mesh_index: 0,
                    source_primitive_index: 0,
                    source_material_slot: 2,
                    triangle_start: 0,
                    triangle_count: 1,
                },
                ImportedPrimitiveGroup {
                    source_node_index: 1,
                    source_mesh_index: 1,
                    source_primitive_index: 1,
                    source_material_slot: 7,
                    triangle_start: 1,
                    triangle_count: 1,
                },
            ],
            materials: vec![
                ImportedMaterial {
                    source_material_slot: 2,
                    source_material_name: Some("left".to_string()),
                },
                ImportedMaterial {
                    source_material_slot: 7,
                    source_material_name: Some("right".to_string()),
                },
            ],
        };
        let selected = select_primitive_group(mesh.clone(), "group/1").unwrap();
        assert_eq!(selected.positions, mesh.positions[3..].to_vec());
        assert_eq!(selected.triangles[0].indices, [0, 1, 2]);
        assert_eq!(selected.primitive_groups[0].source_primitive_index, 1);
        assert_eq!(selected.materials[0].source_material_slot, 7);
        assert_eq!(
            selected.texture_coordinates[0].coordinates,
            vec![Some([0.0, 0.0]), Some([1.0, 0.0]), Some([0.0, 1.0])]
        );

        let selected_node = select_primitive_group(mesh.clone(), "node/0").unwrap();
        assert_eq!(selected_node.primitive_groups.len(), 1);
        assert_eq!(selected_node.primitive_groups[0].source_node_index, 0);

        let error = select_primitive_group(mesh, "group/9").unwrap_err();
        assert_eq!(
            error.diagnostics()[0].code,
            "conversion.unknownMeshPrimitive"
        );
    }
}
