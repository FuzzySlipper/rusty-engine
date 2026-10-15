//! Generated safe C# declarations for Engine-owned animated GLB presentation.
//!
//! Products select immutable admitted GLB content and presentation facts.  The
//! Engine retains resource, instance, graph and controller lifetime; the
//! renderer stays an implementation detail behind the generated API.

use crate::{
    NativeAppearanceHandle, NativeColor, NativeMeshMaterialBinding, NativeQuat,
    NativeRenderResourceHandle, NativeTransform, NativeUtf8Slice, NativeVec3,
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
    Once = 0,
    Repeat = 1,
    PingPong = 2,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAnimationPlaybackKind {
    Play = 0,
    Stop = 1,
    Sample = 2,
    Pause = 3,
    Resume = 4,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAnimationParameterKind {
    Float = 0,
    Bool = 1,
    Trigger = 2,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAnimationMotionKind {
    Clip = 0,
    LinearBlend = 1,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAnimationConditionKind {
    FloatGreaterThan = 0,
    FloatLessThanOrEqual = 1,
    BoolEquals = 2,
    TriggerSet = 3,
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

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAnimatedMeshInspectionRequest {
    pub appearance: NativeAppearanceHandle,
    pub wireframe: bool,
    pub matte: bool,
    pub whole_voxel_normals: bool,
    pub bounds_request: u32,
}

/// Replaces the complete Engine-owned material selection for one animated
/// appearance. Bindings name importer-derived embedded GLB slots and retain
/// the selected Engine material handles for ordinary lifetime checks.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAnimatedMeshMaterialUpdateRequest {
    pub appearance: NativeAppearanceHandle,
    pub bindings: *const NativeMeshMaterialBinding,
    pub bindings_len: usize,
}

/// New factors for one embedded GLB material slot of one appearance. The
/// slot keeps its textures, maps and texture transforms: its base-colour
/// texture multiplies `base_color`, and its emissive texture multiplies the
/// emission.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMeshMaterialFactors {
    pub material_slot: u32,
    /// Replace the slot's `baseColorFactor` with `base_color` (linear RGBA,
    /// not negative; alpha 0 to 1).
    pub override_base_color: bool,
    /// Linear RGBA, not negative; alpha 0 to 1.
    pub base_color: NativeColor,
    /// Replace the slot's `emissiveFactor` (not negative) and emissive
    /// strength (finite, not negative).
    pub override_emission: bool,
    /// Linear RGB, not negative.
    pub emissive_factor: NativeVec3,
    pub emissive_strength: f32,
    /// Replace the slot's texture tint (the material's own without; a GLB
    /// slot's is 1) with `texture_tint`, which multiplies the colour.
    pub override_texture_tint: bool,
    /// Linear RGBA.
    pub texture_tint: NativeColor,
}

/// Replaces the complete set of factor overrides for one animated (GLB)
/// appearance; an empty set restores the GLB's own factors. Appearances of
/// one resource each carry their own set.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAnimatedMeshMaterialFactorsRequest {
    pub appearance: NativeAppearanceHandle,
    pub factors: *const NativeMeshMaterialFactors,
    pub factors_len: usize,
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

/// The space a pose value is given in: a joint's parent (`Local`), the
/// animated instance's own space (`Model`), or the scene (`World`).
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativePoseSpace {
    Local = 0,
    Model = 1,
    World = 2,
}

/// One joint's rotation and/or translation over the evaluated pose, mixed in
/// by `weight` (0 to 1). `joint` indexes `ReadJoints`. A `Local` value
/// replaces the joint's local value, or with `additive` composes onto it. A
/// `Model` or `World` value places the joint there and its descendants
/// follow; the joint keeps its evaluated scale.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeJointOverride {
    pub joint: u32,
    pub space: NativePoseSpace,
    pub additive: bool,
    pub override_rotation: bool,
    pub rotation: NativeQuat,
    pub override_translation: bool,
    pub translation: NativeVec3,
    pub weight: f32,
}

/// Rotates `root` and `mid` so `end` reaches `target`, bending toward
/// `pole`, mixed in by `weight`. `mid` descends from `root` and `end` from
/// `mid`. `space` is `Model` or `World`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeTwoBoneIk {
    pub root: u32,
    pub mid: u32,
    pub end: u32,
    pub space: NativePoseSpace,
    pub target: NativeVec3,
    pub pole: NativeVec3,
    pub weight: f32,
}

/// Replaces one instance's pose controls: two-bone IK then joint overrides,
/// applied after its clips and before skinning. Empty lists clear them.
/// `report_joints` reports the evaluated joints after each call for
/// `ReadJointPose`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAnimationPoseRequest {
    pub instance: NativeAnimationInstanceHandle,
    pub two_bone_ik: *const NativeTwoBoneIk,
    pub two_bone_ik_len: usize,
    pub overrides: *const NativeJointOverride,
    pub overrides_len: usize,
    pub report_joints: bool,
}

/// One rig joint. Its position in `ReadJoints` is the index pose controls
/// use; `parent` is the parent joint's index, `u32::MAX` for a root. The
/// strings borrow the joint-info result; the safe SDK copies them.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAnimationJointInfo {
    pub id: NativeUtf8Slice,
    pub parent: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAnimationJointInfoResult {
    pub joints: *const NativeAnimationJointInfo,
    pub joints_len: usize,
}

/// One evaluated joint, in the instance's space and in the scene.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeJointPose {
    pub model: NativeTransform,
    pub world: NativeTransform,
}

/// One two-bone IK chain of an evaluated pose, from the solve that ran: its
/// target and the end joint as drawn (after overrides and layers), in the
/// instance's space and in the scene. `shortfall` is the scene distance from
/// the end joint to the target; `straightened` is true when the target lay
/// beyond the chain's reach and the solve straightened the chain toward it.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeIkChainReach {
    pub target_model: NativeVec3,
    pub target_world: NativeVec3,
    pub end_model: NativeVec3,
    pub end_world: NativeVec3,
    pub shortfall: f32,
    pub straightened: bool,
}

/// The pose the renderer evaluated for the previous call, per joint in
/// `ReadJoints` order and per two-bone IK chain in the order the pose gave
/// them, with the instance's world placement and the Engine time it was
/// evaluated at. `reported` is false until a reporting instance was first
/// evaluated. `joints` and `ik_chains` point into Animation bridge storage
/// until the next call on the same context.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAnimationJointPoseResult {
    pub joints: *const NativeJointPose,
    pub joints_len: usize,
    pub ik_chains: *const NativeIkChainReach,
    pub ik_chains_len: usize,
    pub world: NativeTransform,
    pub seconds: f64,
    pub reported: bool,
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
pub type NativeReadAnimationJoints = unsafe extern "C" fn(
    *mut std::ffi::c_void,
    NativeRenderResourceHandle,
    *mut NativeAnimationJointInfoResult,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeSetAnimationPose = unsafe extern "C" fn(
    *mut std::ffi::c_void,
    *const NativeAnimationPoseRequest,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
/// Points to find a model's nearest surface to: a GLB model resource
/// (`OpenAnimatedMesh`; its mesh nodes at rest, under their node
/// transforms), where it stands in the caller's space, and points in that
/// space. A point with no surface within `max_distance` finds none.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeModelSurfaceRequest {
    pub model: NativeRenderResourceHandle,
    pub placement: crate::NativeTransform,
    pub points: *const crate::NativeVec3,
    pub points_len: usize,
    pub max_distance: f32,
}

/// The model's surface nearest one point, in the caller's space.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeModelSurfacePoint {
    pub found: bool,
    pub point: crate::NativeVec3,
    /// The outward normal (by winding) of the triangle the point lies on.
    pub normal: crate::NativeVec3,
    pub distance: f32,
    /// The query point lies behind the surface there: inside a closed mesh.
    pub inside: bool,
}

/// One per point, in order; borrowed until the next Animation call.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeModelSurfacePointsResult {
    pub points: *const NativeModelSurfacePoint,
    pub points_len: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeModelRay {
    pub origin: crate::NativeVec3,
    /// Any length but zero.
    pub direction: crate::NativeVec3,
}

/// Rays to meet a model's surface with: the model and its placement as a
/// surface request gives them, and rays in the caller's space, each reaching
/// `max_distance`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeModelRayRequest {
    pub model: NativeRenderResourceHandle,
    pub placement: crate::NativeTransform,
    pub rays: *const NativeModelRay,
    pub rays_len: usize,
    pub max_distance: f32,
}

/// Where one ray first meets the model's surface, in the caller's space.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeModelRayHit {
    pub hit: bool,
    pub point: crate::NativeVec3,
    /// The outward normal (by winding) of the triangle hit.
    pub normal: crate::NativeVec3,
    pub distance: f32,
    /// The ray met the triangle from behind: from inside a closed mesh.
    pub back_face: bool,
}

/// One per ray, in order; borrowed until the next Animation call.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeModelRayHitsResult {
    pub hits: *const NativeModelRayHit,
    pub hits_len: usize,
}

pub type NativeQueryModelSurface = unsafe extern "C" fn(
    *mut std::ffi::c_void,
    *const NativeModelSurfaceRequest,
    *mut NativeModelSurfacePointsResult,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeCastModelRays = unsafe extern "C" fn(
    *mut std::ffi::c_void,
    *const NativeModelRayRequest,
    *mut NativeModelRayHitsResult,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadAnimationJointPose = unsafe extern "C" fn(
    *mut std::ffi::c_void,
    NativeAnimationInstanceHandle,
    *mut NativeAnimationJointPoseResult,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
