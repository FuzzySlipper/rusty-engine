use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{
    mesh::validate_slots, validate_asset_id, MeshMaterialSlot, MeshMaterialSlotError,
    MeshPayloadDescriptor, RenderAssetError, RenderAssetKind, RenderMetadata, Transform,
    TransformError,
};

pub const MAX_RENDER_VOXEL_OBJECT_FRAMES: usize = 8_193;
pub const MAX_RENDER_VOXEL_OBJECT_MESHES: usize = 8_193;
pub const MAX_RENDER_VOXEL_OBJECT_VERTICES: u64 = 8_000_000;
pub const MAX_RENDER_VOXEL_OBJECT_INDICES: u64 = 12_000_000;

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

impl VoxelObjectRenderAsset {
    pub fn validate(&self) -> Result<(), VoxelObjectRenderAssetError> {
        validate_asset_id(&self.asset, RenderAssetKind::VoxelObject)
            .map_err(VoxelObjectRenderAssetError::Asset)?;
        if self.content_hash.trim().is_empty() {
            return Err(VoxelObjectRenderAssetError::EmptyContentHash);
        }
        if self.meshes.is_empty() || self.meshes.len() > MAX_RENDER_VOXEL_OBJECT_MESHES {
            return Err(VoxelObjectRenderAssetError::MeshCount {
                count: self.meshes.len(),
            });
        }
        if self.frames.is_empty() || self.frames.len() > MAX_RENDER_VOXEL_OBJECT_FRAMES {
            return Err(VoxelObjectRenderAssetError::FrameCount {
                count: self.frames.len(),
            });
        }
        validate_slots(&self.material_slots).map_err(VoxelObjectRenderAssetError::MaterialSlot)?;
        let bound_slots = self
            .material_slots
            .iter()
            .map(|binding| binding.slot)
            .collect::<BTreeSet<_>>();
        let mut vertices = 0_u64;
        let mut indices = 0_u64;
        for (index, mesh) in self.meshes.iter().enumerate() {
            mesh.payload
                .validate()
                .map_err(|source| VoxelObjectRenderAssetError::Mesh { index, source })?;
            vertices = vertices.saturating_add(u64::from(mesh.payload.layout.vertex_count));
            indices = indices.saturating_add(u64::from(mesh.payload.layout.index_count));
            if let Some(slot) = mesh
                .payload
                .groups
                .iter()
                .map(|group| group.material_slot)
                .find(|slot| !bound_slots.contains(slot))
            {
                return Err(VoxelObjectRenderAssetError::UnboundMeshSlot { index, slot });
            }
        }
        if vertices > MAX_RENDER_VOXEL_OBJECT_VERTICES || indices > MAX_RENDER_VOXEL_OBJECT_INDICES
        {
            return Err(VoxelObjectRenderAssetError::GeometryLimit { vertices, indices });
        }
        let mut frame_ids = BTreeSet::new();
        for (index, frame) in self.frames.iter().enumerate() {
            if frame.id.trim().is_empty() || !frame_ids.insert(frame.id.as_str()) {
                return Err(VoxelObjectRenderAssetError::InvalidFrameId { index });
            }
            if frame.mesh as usize >= self.meshes.len() {
                return Err(VoxelObjectRenderAssetError::FrameMeshOutOfRange {
                    index,
                    mesh: frame.mesh,
                    mesh_count: self.meshes.len(),
                });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoxelObjectRenderAssetError {
    Asset(RenderAssetError),
    EmptyContentHash,
    MeshCount {
        count: usize,
    },
    FrameCount {
        count: usize,
    },
    MaterialSlot(MeshMaterialSlotError),
    Mesh {
        index: usize,
        source: crate::MeshDescriptorError,
    },
    UnboundMeshSlot {
        index: usize,
        slot: u16,
    },
    InvalidFrameId {
        index: usize,
    },
    FrameMeshOutOfRange {
        index: usize,
        mesh: u32,
        mesh_count: usize,
    },
    GeometryLimit {
        vertices: u64,
        indices: u64,
    },
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
        self.transform
            .validate()
            .map_err(VoxelObjectInstanceError::Transform)?;
        validate_slots(&self.material_overrides).map_err(VoxelObjectInstanceError::MaterialSlot)?;
        self.metadata
            .validate()
            .map_err(VoxelObjectInstanceError::Metadata)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoxelObjectInstanceError {
    Asset(RenderAssetError),
    Transform(TransformError),
    MaterialSlot(MeshMaterialSlotError),
    Metadata(crate::NodeError),
}
