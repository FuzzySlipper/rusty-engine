use std::sync::Arc;

use svc_mesh::{MeshPayload, SurfaceMode};
use voxel_asset::{
    VoxelFrameCell, VoxelObjectAsset, VoxelObjectFrameAnchor, VoxelObjectFrameCollision,
};

/// Runtime work limits are deliberately tighter than durable schema limits for
/// allocations created while resolving and meshing a live object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoxelObjectRuntimeLimits {
    pub max_frames: u32,
    pub max_resolved_voxels: u64,
    /// Aggregate visible unit faces before deterministic greedy compression.
    pub max_unique_mesh_faces: u64,
    pub max_unique_mesh_vertices: u64,
    pub max_unique_mesh_indices: u64,
    pub max_sampled_cells: u64,
    pub max_temporary_field_bytes: u64,
    pub max_material_partitions: u32,
}

impl Default for VoxelObjectRuntimeLimits {
    fn default() -> Self {
        Self {
            max_frames: 8_193,
            max_resolved_voxels: 16_777_216,
            max_unique_mesh_faces: 2_000_000,
            max_unique_mesh_vertices: 8_000_000,
            max_unique_mesh_indices: 12_000_000,
            max_sampled_cells: 16_000_000,
            max_temporary_field_bytes: 256 * 1024 * 1024,
            max_material_partitions: 4_096,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct VoxelObjectAdmissionOptions {
    pub limits: VoxelObjectRuntimeLimits,
    pub surface_mode: SurfaceMode,
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
    pub anchors: Arc<[VoxelObjectFrameAnchor]>,
    pub collision: Option<VoxelObjectFrameCollision>,
}

impl VoxelObjectRuntimeFrame {
    pub fn anchor(&self, id: &str) -> Option<&VoxelObjectFrameAnchor> {
        self.anchors
            .binary_search_by_key(&id, |anchor| anchor.id.as_str())
            .ok()
            .map(|index| &self.anchors[index])
    }
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
    surface_mode: SurfaceMode,
}

impl AdmittedVoxelObject {
    pub(crate) fn new(
        source: VoxelObjectAsset,
        frames: Vec<VoxelObjectRuntimeFrame>,
        clips: Vec<VoxelObjectRuntimeClip>,
        meshes: Vec<Arc<MeshPayload>>,
        surface_mode: SurfaceMode,
    ) -> Self {
        Self {
            source,
            frames,
            clips,
            meshes,
            surface_mode,
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

    pub fn surface_mode(&self) -> SurfaceMode {
        self.surface_mode
    }

    pub fn readout(&self) -> VoxelObjectReadout<'_> {
        VoxelObjectReadout {
            asset_id: self.asset_id(),
            content_hash: self.content_hash(),
            default_frame: 0,
            frame_count: self.frames.len() as u32,
            clip_count: self.clips.len() as u32,
            unique_mesh_count: self.meshes.len() as u32,
            surface_mode: self.surface_mode,
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
    pub surface_mode: SurfaceMode,
}
