use serde::{Deserialize, Serialize};

use crate::{
    mesh::validate_slots, validate_asset_id, MeshMaterialSlot, MeshMaterialSlotError,
    MeshPayloadDescriptor, RenderAssetError, RenderAssetKind, RenderMetadata, Transform,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VoxelObjectRenderMesh {
    pub payload: MeshPayloadDescriptor,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VoxelObjectRenderFrame {
    pub id: String,
    pub mesh: u32,
}

/// Renderer-neutral frame resources for one admitted voxel object.
///
/// Collision and navigation are intentionally absent: this resource is
/// presentation-only, and frame changes cannot imply authority mutation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VoxelObjectRenderAsset {
    pub asset: String,
    pub content_hash: String,
    pub meshes: Vec<VoxelObjectRenderMesh>,
    pub frames: Vec<VoxelObjectRenderFrame>,
    pub material_slots: Vec<MeshMaterialSlot>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VoxelObjectInstanceDescriptor {
    pub asset: String,
    pub frame: u32,
    pub transform: Transform,
    pub visible: bool,
    pub material_overrides: Vec<MeshMaterialSlot>,
    pub metadata: RenderMetadata,
}

impl VoxelObjectInstanceDescriptor {
    pub fn validate(&self) -> Result<(), VoxelObjectInstanceError> {
        validate_asset_id(&self.asset, RenderAssetKind::VoxelObject)
            .map_err(VoxelObjectInstanceError::Asset)?;
        validate_slots(&self.material_overrides).map_err(VoxelObjectInstanceError::MaterialSlot)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoxelObjectInstanceError {
    Asset(RenderAssetError),
    MaterialSlot(MeshMaterialSlotError),
}
