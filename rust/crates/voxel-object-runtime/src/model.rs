use std::sync::Arc;

use svc_mesh::MeshPayload;
use voxel_asset::{VoxelFrameCell, VoxelObjectAsset};

/// Runtime work limits are deliberately tighter than durable schema limits for
/// allocations created while resolving and meshing a live object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoxelObjectRuntimeLimits {
    pub max_frames: u32,
    pub max_resolved_voxels: u64,
    pub max_unique_mesh_faces: u64,
}

impl Default for VoxelObjectRuntimeLimits {
    fn default() -> Self {
        Self {
            max_frames: 8_193,
            max_resolved_voxels: 16_777_216,
            max_unique_mesh_faces: 2_000_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoxelObjectFrameSource {
    Default,
    Clip { clip: String, frame: u32 },
}

#[derive(Debug, Clone)]
pub struct VoxelObjectRuntimeFrame {
    pub index: u32,
    pub source: VoxelObjectFrameSource,
    pub voxel_data_hash: String,
    pub cells: Arc<[VoxelFrameCell]>,
    pub mesh_index: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoxelObjectRuntimeClip {
    pub id: String,
    pub name: Option<String>,
    pub frame_indices: Vec<u32>,
    pub frame_durations_micros: Vec<u64>,
    pub duration_micros: u64,
}

#[derive(Debug, Clone)]
pub struct AdmittedVoxelObject {
    source: VoxelObjectAsset,
    frames: Vec<VoxelObjectRuntimeFrame>,
    clips: Vec<VoxelObjectRuntimeClip>,
    meshes: Vec<Arc<MeshPayload>>,
}

impl AdmittedVoxelObject {
    pub(crate) fn new(
        source: VoxelObjectAsset,
        frames: Vec<VoxelObjectRuntimeFrame>,
        clips: Vec<VoxelObjectRuntimeClip>,
        meshes: Vec<Arc<MeshPayload>>,
    ) -> Self {
        Self {
            source,
            frames,
            clips,
            meshes,
        }
    }

    pub fn source(&self) -> &VoxelObjectAsset {
        &self.source
    }

    pub fn asset_id(&self) -> &str {
        &self.source.asset_id
    }

    pub fn content_hash(&self) -> &str {
        &self.source.content_hash
    }

    pub fn default_frame(&self) -> &VoxelObjectRuntimeFrame {
        &self.frames[0]
    }

    pub fn frame(&self, index: u32) -> Option<&VoxelObjectRuntimeFrame> {
        self.frames.get(index as usize)
    }

    pub fn frames(&self) -> &[VoxelObjectRuntimeFrame] {
        &self.frames
    }

    pub fn clip(&self, id: &str) -> Option<&VoxelObjectRuntimeClip> {
        self.clips.iter().find(|clip| clip.id == id)
    }

    pub fn clips(&self) -> &[VoxelObjectRuntimeClip] {
        &self.clips
    }

    pub fn mesh(&self, index: u32) -> Option<&Arc<MeshPayload>> {
        self.meshes.get(index as usize)
    }

    pub fn meshes(&self) -> &[Arc<MeshPayload>] {
        &self.meshes
    }

    pub fn readout(&self) -> VoxelObjectReadout<'_> {
        VoxelObjectReadout {
            asset_id: self.asset_id(),
            content_hash: self.content_hash(),
            default_frame: 0,
            frame_count: self.frames.len() as u32,
            clip_count: self.clips.len() as u32,
            unique_mesh_count: self.meshes.len() as u32,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoxelObjectReadout<'a> {
    pub asset_id: &'a str,
    pub content_hash: &'a str,
    pub default_frame: u32,
    pub frame_count: u32,
    pub clip_count: u32,
    pub unique_mesh_count: u32,
}
