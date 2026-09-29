//! Generated safe C# declarations for Engine-owned animated GLB presentation.
//!
//! Products select immutable admitted GLB content and presentation facts.  The
//! Engine retains resource, instance, graph and controller lifetime; the
//! browser renderer remains an implementation detail behind the generated API.

use crate::{
    NativeAppearanceHandle, NativeMeshMaterialBinding, NativeRenderResourceHandle, NativeUtf8Slice,
};

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeAnimationInstanceHandle {
    pub value: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeAnimationGraphHandle {
    pub value: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeAnimationTransitionHandle {
    pub value: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeAnimationControllerHandle {
    pub value: u64,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAnimationLoopMode {
    Once = 1,
    Repeat = 2,
    PingPong = 3,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAnimationPlaybackKind {
    Play = 1,
    Stop = 2,
    Sample = 3,
    Pause = 4,
    Resume = 5,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAnimationParameterKind {
    Float = 1,
    Bool = 2,
    Trigger = 3,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAnimationMotionKind {
    Clip = 1,
    LinearBlend = 2,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAnimationConditionKind {
    FloatGreaterThan = 1,
    FloatLessThanOrEqual = 2,
    BoolEquals = 3,
    TriggerSet = 4,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeAnimationTransitionMoment {
    #[default]
    None = 0,
    Started = 1,
    Completed = 2,
}

/// Inline copied UTF-8 identity used by animation feedback reads. It never
/// borrows browser or Rust storage; overlong ingress values are rejected.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeAnimationFeedbackText {
    pub len: u32,
    pub bytes: [u8; 96],
}

impl Default for NativeAnimationFeedbackText {
    fn default() -> Self {
        Self {
            len: 0,
            bytes: [0; 96],
        }
    }
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeAnimationRealizationFactKind {
    #[default]
    None = 0,
    PlaybackObservation = 1,
    Diagnostic = 2,
    Stopped = 4,
    NaturalCompletion = 5,
    MeshInspection = 6,
}

/// Borrowed realization feedback: every retained fact. `facts` points into
/// Animation bridge storage and stays valid until the next call on the same
/// context; the generated managed binding copies it before returning.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAnimationRealizationResult {
    pub facts: *const NativeAnimationRealizationFact,
    pub facts_len: usize,
    pub evicted_fact_count: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeAnimationRealizationFact {
    pub bounds_request: u32,
    pub bounds_min: crate::NativeVec3,
    pub bounds_max: crate::NativeVec3,
    pub has_bounds: bool,
    pub voxel_normal_meshes: u32,
    pub kind: NativeAnimationRealizationFactKind,
    pub fact_id: u64,
    pub object_id: u64,
    pub generation: u64,
    pub has_object_id: bool,
    pub has_generation: bool,
    pub sequence: u32,
    pub status: NativeAnimationFeedbackText,
    pub clip: NativeAnimationFeedbackText,
    pub diagnostic_code: NativeAnimationFeedbackText,
    pub reason: NativeAnimationFeedbackText,
    pub sampled_millis: u64,
    pub has_sampled_millis: bool,
}

/// Immutable GLB selected from admitted product content during Create. The
/// Engine copies and validates its bytes before this direct call returns.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAnimatedMeshResourceRequest {
    pub path: NativeUtf8Slice,
}

/// A separately-addressed immutable GLB containing animation clips. The
/// Engine copies, imports, and retains its rig metadata before this direct
/// call returns.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAnimationClipPackResourceRequest {
    pub path: NativeUtf8Slice,
}

/// Attaches one admitted clip-pack GLB to an admitted primary animated mesh.
/// The Engine derives clips from the admitted pack bytes, validates the exact
/// assembled `AnimatedMeshAsset`, and only then retains the association.
///
/// Resource handles name Engine-imported rig metadata. Products retain only
/// their producer/license provenance policy; source and target hashes are
/// derived from the immutable admitted resources.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAnimationClipPackAssociationRequest {
    pub primary_mesh: NativeRenderResourceHandle,
    pub clip_pack: NativeRenderResourceHandle,
    pub producer: NativeUtf8Slice,
    pub license: NativeUtf8Slice,
}

/// Adds an animated appearance to the ordinary Engine-owned appearance
/// snapshot. Its associated render resource must have been opened through
/// `OpenAnimatedMesh` during product Create.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAnimatedMeshAppearanceRequest {
    pub resource: NativeRenderResourceHandle,
}

/// Replaces the complete Engine-owned material selection for one animated
/// appearance. Bindings name importer-derived embedded GLB slots and retain
/// the selected Engine material handles for ordinary lifetime checks.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAnimatedMeshInspectionRequest {
    pub appearance: NativeAppearanceHandle,
    pub wireframe: bool,
    pub matte: bool,
    pub whole_voxel_normals: bool,
    pub bounds_request: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAnimatedMeshMaterialUpdateRequest {
    pub appearance: NativeAppearanceHandle,
    pub bindings: *const NativeMeshMaterialBinding,
    pub bindings_len: usize,
}

/// Retained instance identity tied to one product object and one animated
/// appearance. The object becomes renderer-visible only through the regular
/// complete appearance snapshot.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAnimationInstanceRequest {
    pub appearance: NativeAppearanceHandle,
    pub object_id: u64,
}

/// Direct playback command for one retained animation instance. `Sample`
/// holds a normalized point in a clip; `Pause` and `Resume` retain the current
/// backend sample. `fade_seconds` is ignored for commands without a fade.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAnimationPlaybackRequest {
    pub instance: NativeAnimationInstanceHandle,
    pub kind: NativeAnimationPlaybackKind,
    pub clip: NativeUtf8Slice,
    pub loop_mode: NativeAnimationLoopMode,
    pub speed: f32,
    pub weight: f32,
    pub restart: bool,
    pub fade_seconds: f32,
    pub has_fade: bool,
    pub normalized_time: f32,
}

/// Starts a retained, explicitly assembled non-legacy animation graph.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAnimationGraphCreateRequest {
    pub resource: NativeRenderResourceHandle,
    pub graph_id: NativeUtf8Slice,
    pub version: u32,
    pub initial_state_id: NativeUtf8Slice,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAnimationParameterDefinitionRequest {
    pub graph: NativeAnimationGraphHandle,
    pub parameter_id: NativeUtf8Slice,
    pub kind: NativeAnimationParameterKind,
    pub float_default_milli: i32,
    pub bool_default: bool,
}

/// A state either plays `clip_a` or linearly blends it with `clip_b` according
/// to `parameter_id`. All clip and parameter names are validated against the
/// admitted GLB and this graph when the controller is created.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAnimationStateDefinitionRequest {
    pub graph: NativeAnimationGraphHandle,
    pub state_id: NativeUtf8Slice,
    pub motion_kind: NativeAnimationMotionKind,
    pub clip_a: NativeUtf8Slice,
    pub clip_b: NativeUtf8Slice,
    pub parameter_id: NativeUtf8Slice,
    pub minimum_milli: i32,
    pub maximum_milli: i32,
    pub speed_milli: i32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAnimationTransitionDefinitionRequest {
    pub graph: NativeAnimationGraphHandle,
    pub transition_id: NativeUtf8Slice,
    pub from_state_id: NativeUtf8Slice,
    pub to_state_id: NativeUtf8Slice,
    pub priority: u32,
    pub duration_ticks: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAnimationConditionDefinitionRequest {
    pub transition: NativeAnimationTransitionHandle,
    pub kind: NativeAnimationConditionKind,
    pub parameter_id: NativeUtf8Slice,
    pub threshold_milli: i32,
    pub bool_value: bool,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAnimationControllerCreateRequest {
    pub graph: NativeAnimationGraphHandle,
    pub instance: NativeAnimationInstanceHandle,
    pub tick_duration_millis: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAnimationSetFloatRequest {
    pub controller: NativeAnimationControllerHandle,
    pub parameter_id: NativeUtf8Slice,
    pub value_milli: i32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAnimationSetBoolRequest {
    pub controller: NativeAnimationControllerHandle,
    pub parameter_id: NativeUtf8Slice,
    pub value: bool,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAnimationFireTriggerRequest {
    pub controller: NativeAnimationControllerHandle,
    pub parameter_id: NativeUtf8Slice,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAnimationTickRequest {
    pub controller: NativeAnimationControllerHandle,
    pub tick: u64,
}

/// Bounded controller observation. State and clip indexes refer to the exact
/// insertion order supplied to this graph and to the admitted GLB's clip list;
/// `u32::MAX` denotes no secondary clip or no active transition.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeAnimationControllerReadout {
    pub state_index: u32,
    pub clip_a_index: u32,
    pub clip_b_index: u32,
    pub blend_weight_milli: i32,
    pub speed_milli: i32,
    pub revision: u64,
    pub controller_tick: u64,
    pub transition_from_state_index: u32,
    pub transition_to_state_index: u32,
    pub transition_elapsed_ticks: u32,
    pub transition_duration_ticks: u32,
    pub transition_moment: NativeAnimationTransitionMoment,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeAnimationReadout {
    pub admitted_meshes: u32,
    pub admitted_clip_packs: u32,
    pub retained_clip_pack_associations: u32,
    pub retained_instances: u32,
    pub retained_graphs: u32,
    pub retained_controllers: u32,
    pub pending_playback_commands: u32,
}

/// Copied facts from the Engine-admitted GLB, for editor framing and controls.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAnimatedMeshInfo {
    pub bounds_min: crate::NativeVec3,
    pub bounds_max: crate::NativeVec3,
    pub clip_count: u32,
    pub material_count: u32,
    pub joint_count: u32,
}

/// Strings borrow the clip-info result; the safe SDK copies them.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAnimationClipInfo {
    pub id: NativeUtf8Slice,
    pub name: NativeUtf8Slice,
    pub duration_seconds: f32,
    pub has_duration: bool,
}

pub type NativeReadAnimatedMeshInfo = unsafe extern "C" fn(
    *mut std::ffi::c_void,
    NativeRenderResourceHandle,
    *mut NativeAnimatedMeshInfo,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAnimationClipInfoResult {
    pub clips: *const NativeAnimationClipInfo,
    pub clips_len: usize,
}
pub type NativeReadAnimationClips = unsafe extern "C" fn(
    *mut std::ffi::c_void,
    NativeRenderResourceHandle,
    *mut NativeAnimationClipInfoResult,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
