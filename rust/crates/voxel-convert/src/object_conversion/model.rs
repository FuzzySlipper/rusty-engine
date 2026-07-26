use serde::{Deserialize, Serialize};
use voxel_asset::{VoxelAssetBounds, VoxelObjectAsset};

use crate::{AnimationAnchorPolicy, AnimationEndPolicy, ConversionPlanSettings, MeshSourceRef};

pub const VOXEL_OBJECT_CONVERSION_PLANNER_ID: &str = "rusty-engine.voxel-object-conversion.v1";
pub const VOXEL_OBJECT_CONVERTER_ID: &str = "rusty-engine.mesh-to-voxel-object.v1";
pub const MAX_VOXEL_OBJECT_CONVERSION_REQUEST_BYTES: usize = 1024 * 1024;
pub const MAX_VOXEL_OBJECT_CONVERSION_VOXELIZATION_WORK: u64 = 50_000_000;
pub const MAX_VOXEL_OBJECT_CONVERSION_DEFORMATION_WORK: u64 = 10_000_000;
/// Combined bind-pose and selected-clip topology retained by one object plan.
pub const MAX_VOXEL_OBJECT_CONVERSION_RETAINED_SNAPSHOT_BYTES: u64 = 32 * 1024 * 1024;
pub const MAX_VOXEL_OBJECT_PREVIEW_SAMPLES: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelObjectConversionSettings {
    /// Existing M12C geometry, material, transform, and grid policy.
    pub mesh: ConversionPlanSettings,
    /// Stable object-local pivot in voxel-cell coordinates.
    pub pivot: [f64; 3],
    /// Explicit animation anchoring applied to the bind pose and every sample.
    pub anchor_policy: AnimationAnchorPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelObjectClipConversionRequest {
    pub source_clip_name: String,
    pub output_clip_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_name: Option<String>,
    pub sample_rate_hz: u32,
    pub start_microseconds: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_microseconds: Option<u64>,
    pub end_policy: AnimationEndPolicy,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelObjectConversionPlanRequest {
    pub source: MeshSourceRef,
    pub source_path: String,
    pub target_asset_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license_path: Option<String>,
    pub settings: VoxelObjectConversionSettings,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clips: Vec<VoxelObjectClipConversionRequest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_clip: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelObjectClipPlanSummary {
    pub output_clip_id: String,
    pub source_clip_name: String,
    pub source_animation_index: u32,
    pub start_microseconds: u64,
    pub end_microseconds: u64,
    pub sample_rate_hz: u32,
    pub sampled_frame_count: usize,
    pub stored_frame_count: usize,
    pub duration_microseconds: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelObjectConversionPlan {
    pub plan_id: String,
    pub source: MeshSourceRef,
    pub source_path: String,
    pub target_asset_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license_path: Option<String>,
    pub settings: VoxelObjectConversionSettings,
    pub clips: Vec<VoxelObjectClipConversionRequest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_clip: Option<String>,
    pub planner: String,
    pub expected_source_sha256: String,
    pub settings_sha256: String,
    pub expected_output_content_hash: String,
    pub plan_hash: String,
    pub estimated_sampled_frames: usize,
    pub estimated_stored_frames: usize,
    pub estimated_aggregate_voxels: usize,
    pub estimated_artifact_bytes: usize,
    pub estimated_bounds: VoxelAssetBounds,
    pub clip_summaries: Vec<VoxelObjectClipPlanSummary>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreparedVoxelObjectConversion {
    pub(super) plan: VoxelObjectConversionPlan,
    pub(super) output: VoxelObjectConversionReceipt,
}

impl PreparedVoxelObjectConversion {
    pub fn plan(&self) -> &VoxelObjectConversionPlan {
        &self.plan
    }

    pub fn candidate(&self) -> &VoxelObjectConversionReceipt {
        &self.output
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoxelObjectConvertedFrameReadout {
    pub stored_frame_index: u32,
    pub source_timestamps_microseconds: Vec<u64>,
    pub duration_microseconds: u64,
    pub bounds: VoxelAssetBounds,
    pub voxel_count: usize,
    pub sparse_run_count: usize,
    pub voxel_data_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoxelObjectConvertedClipReadout {
    pub output_clip_id: String,
    pub source_clip_name: String,
    pub source_animation_index: u32,
    pub start_microseconds: u64,
    pub end_microseconds: u64,
    pub sample_rate_hz: u32,
    pub end_policy: AnimationEndPolicy,
    pub sampled_frame_count: usize,
    pub stored_frame_count: usize,
    pub duration_microseconds: u64,
    pub frames: Vec<VoxelObjectConvertedFrameReadout>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoxelObjectConversionReceipt {
    pub asset: VoxelObjectAsset,
    pub canonical_json: String,
    pub source_sha256: String,
    pub settings_sha256: String,
    pub content_hash: String,
    pub source_vertices: usize,
    pub source_triangles: usize,
    pub deformation_work: u64,
    pub voxelization_work: u64,
    pub sampled_frames: usize,
    pub stored_frames: usize,
    pub aggregate_voxels: usize,
    pub artifact_bytes: usize,
    pub bounds: VoxelAssetBounds,
    pub clips: Vec<VoxelObjectConvertedClipReadout>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelObjectConversionPreviewRequest {
    pub plan_id: String,
    pub expected_plan_hash: String,
    pub frame: VoxelObjectFrameSelection,
    pub max_samples: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    deny_unknown_fields,
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum VoxelObjectFrameSelection {
    Default,
    Clip { clip_id: String, frame_index: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoxelObjectPreviewVoxel {
    pub coordinate: [i64; 3],
    pub material_slot: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoxelObjectSelectedFramePreview {
    pub selection: VoxelObjectFrameSelection,
    pub bounds: VoxelAssetBounds,
    pub voxel_count: usize,
    pub sparse_run_count: usize,
    pub voxel_data_hash: String,
    pub duration_microseconds: Option<u64>,
    pub source_timestamps_microseconds: Vec<u64>,
    pub sample_voxels: Vec<VoxelObjectPreviewVoxel>,
    pub samples_truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoxelObjectConversionPreview {
    pub plan_id: String,
    pub plan_hash: String,
    pub output_hash: String,
    pub sampled_frame_count: usize,
    pub stored_frame_count: usize,
    pub aggregate_voxel_count: usize,
    pub artifact_bytes: usize,
    pub union_bounds: VoxelAssetBounds,
    pub clips: Vec<VoxelObjectConvertedClipReadout>,
    pub selected_frame: VoxelObjectSelectedFramePreview,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelObjectConversionApplyRequest {
    pub plan_id: String,
    pub expected_plan_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_output_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppliedVoxelObjectConversion {
    pub plan_id: String,
    pub plan_hash: String,
    pub output_hash: String,
    pub conversion: VoxelObjectConversionReceipt,
}
