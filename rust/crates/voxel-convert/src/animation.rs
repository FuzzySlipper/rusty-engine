//! Host-neutral import and deterministic sampling of animated GLB geometry.

use serde::{Deserialize, Serialize};

use crate::{ImportedModelScene, ImportedStaticMesh};

mod import;
mod sample;

pub const ANIMATION_TIMESTAMP_TICKS_PER_SECOND: u64 = 1_000_000;
pub const MAX_IMPORTED_ANIMATION_CLIPS: usize = 64;
pub const MAX_IMPORTED_ANIMATION_CHANNELS: usize = 4_096;
pub const MAX_IMPORTED_ANIMATION_KEYFRAMES: usize = 1_000_000;
pub const MAX_IMPORTED_ANIMATION_VALUES: usize = 4_000_000;
pub const MAX_IMPORTED_SKINS: usize = 128;
pub const MAX_IMPORTED_JOINTS_PER_SKIN: usize = 256;
pub const MAX_IMPORTED_MORPH_TARGETS: usize = 64;
pub const MAX_IMPORTED_MORPH_POSITION_DELTAS: usize = 4_000_000;
pub const MAX_ANIMATION_SAMPLE_RATE_HZ: u32 = 240;
pub const MAX_ANIMATION_SAMPLE_FRAMES: usize = 4_096;
pub const MAX_ANIMATION_DURATION_MICROSECONDS: u64 = 3_600_000_000;
pub const MAX_ANIMATION_DEFORMATION_WORK: u64 = 10_000_000;

#[derive(Debug, Clone, PartialEq)]
pub struct ImportedAnimatedModel {
    pub source_sha256: String,
    pub scene: ImportedModelScene,
    pub nodes: Vec<ImportedAnimationNode>,
    pub skins: Vec<ImportedSkin>,
    pub primitive_deformations: Vec<ImportedPrimitiveDeformation>,
    pub clips: Vec<ImportedAnimationClip>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportedAnimationNode {
    pub source_node_index: u32,
    pub source_skin_index: Option<u32>,
    pub base_transform: ImportedNodeTransform,
    pub base_morph_weights: Vec<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ImportedNodeTransform {
    Matrix([f64; 16]),
    Decomposed {
        translation: [f64; 3],
        rotation: [f64; 4],
        scale: [f64; 3],
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportedSkin {
    pub source_skin_index: u32,
    pub source_skin_name: Option<String>,
    pub skeleton_node_index: Option<u32>,
    pub joint_node_indices: Vec<u32>,
    pub inverse_bind_matrices: Vec<[f64; 16]>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportedPrimitiveDeformation {
    pub source_mesh_index: u32,
    pub source_primitive_index: u32,
    pub vertex_joints: Option<Vec<[u16; 4]>>,
    pub vertex_weights: Option<Vec<[f64; 4]>>,
    pub morph_position_deltas: Vec<Vec<[f64; 3]>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportedAnimationClip {
    pub source_animation_index: u32,
    pub name: String,
    pub duration_microseconds: u64,
    pub channels: Vec<ImportedAnimationChannel>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportedAnimationChannel {
    pub source_channel_index: u32,
    pub target_node_index: u32,
    pub property: AnimationProperty,
    pub interpolation: AnimationInterpolation,
    pub timestamps_microseconds: Vec<u64>,
    pub values: AnimationChannelValues,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationProperty {
    Translation,
    Rotation,
    Scale,
    MorphWeights,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationInterpolation {
    Step,
    Linear,
    CubicSpline,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AnimationChannelValues {
    Translations(Vec<[f64; 3]>),
    Rotations(Vec<[f64; 4]>),
    Scales(Vec<[f64; 3]>),
    MorphWeights { target_count: u32, values: Vec<f64> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AnimationEndPolicy {
    /// Sample zero and every rate tick, then include the exact quantized clip end once.
    IncludeClipEnd,
    /// Sample zero and rate ticks strictly before the end, omitting a duplicate loop seam.
    ExcludeLoopSeam,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(
    tag = "kind",
    deny_unknown_fields,
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum AnimationAnchorPolicy {
    /// Preserve authored motion in the selected scene's object coordinates.
    PreserveSourceSpace,
    /// Keep one reachable node at its bind-pose model transform in every sample.
    LockNodeToBindPose { source_node_index: u32 },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct AnimationAnchorPolicyWire {
    kind: String,
    #[serde(default)]
    source_node_index: Option<u32>,
}

impl<'de> Deserialize<'de> for AnimationAnchorPolicy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = AnimationAnchorPolicyWire::deserialize(deserializer)?;
        match (wire.kind.as_str(), wire.source_node_index) {
            ("preserveSourceSpace", None) => Ok(Self::PreserveSourceSpace),
            ("lockNodeToBindPose", Some(source_node_index)) => {
                Ok(Self::LockNodeToBindPose { source_node_index })
            }
            ("preserveSourceSpace", Some(_)) => Err(serde::de::Error::custom(
                "preserveSourceSpace does not accept sourceNodeIndex",
            )),
            ("lockNodeToBindPose", None) => Err(serde::de::Error::missing_field("sourceNodeIndex")),
            _ => Err(serde::de::Error::unknown_variant(
                &wire.kind,
                &["preserveSourceSpace", "lockNodeToBindPose"],
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnimationSampleRequest {
    pub expected_source_sha256: String,
    pub clip_name: String,
    pub sample_rate_hz: u32,
    pub end_policy: AnimationEndPolicy,
    pub anchor_policy: AnimationAnchorPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnimationSampleRangeRequest {
    pub expected_source_sha256: String,
    pub clip_name: String,
    pub sample_rate_hz: u32,
    pub start_microseconds: u64,
    pub end_microseconds: u64,
    pub end_policy: AnimationEndPolicy,
    pub anchor_policy: AnimationAnchorPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnimationBindPoseRequest {
    pub expected_source_sha256: String,
    pub anchor_policy: AnimationAnchorPolicy,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnimationMeshSnapshot {
    pub timestamp_microseconds: u64,
    pub mesh: ImportedStaticMesh,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnimationSampleReceipt {
    pub source_sha256: String,
    pub source_animation_index: u32,
    pub clip_name: String,
    pub duration_microseconds: u64,
    pub sample_rate_hz: u32,
    pub end_policy: AnimationEndPolicy,
    pub anchor_policy: AnimationAnchorPolicy,
    pub deformation_work: u64,
    pub snapshots: Vec<AnimationMeshSnapshot>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnimationSampleRangeReceipt {
    pub source_sha256: String,
    pub source_animation_index: u32,
    pub clip_name: String,
    pub clip_duration_microseconds: u64,
    pub start_microseconds: u64,
    pub end_microseconds: u64,
    pub sample_rate_hz: u32,
    pub end_policy: AnimationEndPolicy,
    pub anchor_policy: AnimationAnchorPolicy,
    pub deformation_work: u64,
    pub snapshots: Vec<AnimationMeshSnapshot>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnimationBindPoseReceipt {
    pub source_sha256: String,
    pub anchor_policy: AnimationAnchorPolicy,
    pub deformation_work: u64,
    pub mesh: ImportedStaticMesh,
}

pub fn import_animated_glb(source: &[u8]) -> Result<ImportedAnimatedModel, crate::ConversionError> {
    import::import_animated_glb(source)
}

pub fn sample_animation_clip(
    model: &ImportedAnimatedModel,
    request: &AnimationSampleRequest,
) -> Result<AnimationSampleReceipt, crate::ConversionError> {
    sample::sample_animation_clip(model, request)
}

pub fn sample_animation_clip_range(
    model: &ImportedAnimatedModel,
    request: &AnimationSampleRangeRequest,
) -> Result<AnimationSampleRangeReceipt, crate::ConversionError> {
    sample::sample_animation_clip_range(model, request)
}

pub fn sample_animation_bind_pose(
    model: &ImportedAnimatedModel,
    request: &AnimationBindPoseRequest,
) -> Result<AnimationBindPoseReceipt, crate::ConversionError> {
    sample::sample_animation_bind_pose(model, request)
}
