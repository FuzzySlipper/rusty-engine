//! Generated safe C# declarations for Engine-owned animated GLB presentation.
//!
//! Products select immutable admitted GLB content and presentation facts.  The
//! Engine retains resource, instance, graph and controller lifetime; the
//! browser renderer remains an implementation detail behind the generated API.

use crate::{NativeAppearanceHandle, NativeRenderResourceHandle, NativeUtf8Slice};

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

/// Immutable GLB selected from admitted product content during Create. The
/// Engine copies and validates its bytes before this direct call returns.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAnimatedMeshResourceRequest {
    pub path: NativeUtf8Slice,
}

/// A separately-addressed immutable GLB containing animation clips. The
/// Engine copies and validates its bytes before this direct call returns. Its
/// rig and provenance are deliberately associated with a primary mesh in a
/// separate typed call because the current importer does not derive them.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAnimationClipPackResourceRequest {
    pub path: NativeUtf8Slice,
}

/// One declared joint in the explicit temporary rig metadata required for a
/// clip-pack association. `has_parent` distinguishes the root from a joint
/// named with an empty parent value.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAnimationRigJointRequest {
    pub id: NativeUtf8Slice,
    pub parent_id: NativeUtf8Slice,
    pub has_parent: bool,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAnimationBindRestConvention {
    LocalMatrixV1 = 1,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAnimationRootConvention {
    InPlace = 1,
    AuthoredRootTranslation = 2,
}

/// Attaches one admitted clip-pack GLB to an admitted primary animated mesh.
/// The Engine derives clips from the admitted pack bytes, validates the exact
/// assembled `AnimatedMeshAsset`, and only then retains the association.
///
/// Until importer work derives rig metadata, callers explicitly provide the
/// rig plus the producer/license facts required by `AnimationClipPack`. The
/// Engine derives the clip-pack asset identity and both provenance hashes from
/// the two admitted resources; products do not repeat Engine-owned facts.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAnimationClipPackAssociationRequest {
    pub primary_mesh: NativeRenderResourceHandle,
    pub clip_pack: NativeRenderResourceHandle,
    pub joints: *const NativeAnimationRigJointRequest,
    pub joints_len: usize,
    pub bind_rest_hash: NativeUtf8Slice,
    pub bind_rest_convention: NativeAnimationBindRestConvention,
    pub root_convention: NativeAnimationRootConvention,
    pub root_joint_id: NativeUtf8Slice,
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
