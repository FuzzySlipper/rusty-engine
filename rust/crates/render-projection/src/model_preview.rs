use render_model::{
    MaterialDescriptorError, RenderDiff, RenderFrameDiff, RenderFrameError, RenderHandle,
    RenderMaterialDescriptor, RenderMetadata, StaticMeshAsset, StaticMeshError,
    StaticMeshInstanceDescriptor, Transform,
};
use serde::{Deserialize, Serialize};

/// Resolved, renderer-neutral input for a model/material preview.
///
/// Catalog lookup remains with the asset owner. This projection accepts the
/// resulting material descriptor directly rather than restoring the donor's
/// old universal runtime facade or catalog-owned renderer facade.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelMaterialPreviewRequest {
    pub material: RenderMaterialDescriptor,
    pub mesh_asset: StaticMeshAsset,
    pub instance_handle: RenderHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ModelMaterialPreviewClassification {
    ReferencePreview,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelMaterialPreviewDiagnostic {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelMaterialPreviewSnapshot {
    pub material: RenderMaterialDescriptor,
    pub mesh_asset: StaticMeshAsset,
    pub preview_frame: RenderFrameDiff,
    pub renderer_classification: ModelMaterialPreviewClassification,
    pub diagnostics: Vec<ModelMaterialPreviewDiagnostic>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ModelMaterialPreviewError {
    InvalidMaterial(MaterialDescriptorError),
    InvalidMesh(StaticMeshError),
    MaterialNotBound {
        mesh_asset: String,
        material: String,
    },
    InvalidFrame(RenderFrameError),
}

impl std::fmt::Display for ModelMaterialPreviewError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "model/material preview rejected: {self:?}")
    }
}

impl std::error::Error for ModelMaterialPreviewError {}

/// Builds the complete retained resource/instance frame needed by both Studio
/// and standalone preview hosts. No renderer object or runtime world is touched.
pub fn build_model_material_preview(
    request: ModelMaterialPreviewRequest,
) -> Result<ModelMaterialPreviewSnapshot, ModelMaterialPreviewError> {
    request
        .material
        .validate()
        .map_err(ModelMaterialPreviewError::InvalidMaterial)?;
    request
        .mesh_asset
        .validate()
        .map_err(ModelMaterialPreviewError::InvalidMesh)?;
    if !request
        .mesh_asset
        .material_slots
        .iter()
        .any(|slot| slot.material == request.material.id)
    {
        return Err(ModelMaterialPreviewError::MaterialNotBound {
            mesh_asset: request.mesh_asset.asset.clone(),
            material: request.material.id.clone(),
        });
    }

    let preview_frame = RenderFrameDiff::try_from_ops(vec![
        RenderDiff::DefineMaterial {
            material: request.material.clone(),
        },
        RenderDiff::DefineStaticMesh {
            asset: request.mesh_asset.clone(),
        },
        RenderDiff::CreateStaticMeshInstance {
            handle: request.instance_handle,
            parent: None,
            instance: StaticMeshInstanceDescriptor {
                asset: request.mesh_asset.asset.clone(),
                transform: Transform::IDENTITY,
                visible: true,
                material_overrides: Vec::new(),
                metadata: RenderMetadata {
                    source_entity: None,
                    source_scene_node: None,
                    tags: Vec::new(),
                    label: Some(format!("Preview {}", request.mesh_asset.asset)),
                },
            },
        },
    ])
    .map_err(ModelMaterialPreviewError::InvalidFrame)?;

    Ok(ModelMaterialPreviewSnapshot {
        material: request.material,
        mesh_asset: request.mesh_asset,
        preview_frame,
        renderer_classification: ModelMaterialPreviewClassification::ReferencePreview,
        diagnostics: Vec::new(),
    })
}
