//! Generated safe C# declarations for parts rigs: rig-c's 2D rigs
//! (`skeleton.model.json`, `rigc-compiled/3`), posed by the Engine and drawn
//! as their pieces on one upright plane in the world.
//!
//! A product loads a rig from its content, makes appearances that show one
//! or more rigs as views seen from different headings, and publishes them as
//! objects. Each published object plays its own clip; the calls that play,
//! stop, reskin, drive and read a rig name the object.

use std::ffi::c_void;

use crate::{
    NativeAnimationFeedbackText, NativeAppearanceHandle, NativeBillboardMode, NativeColor,
    NativeOperationErrorReceipt, NativeSpriteDepthPolicy, NativeSpriteMaterialDescriptor,
    NativeSpriteSizeMode, NativeTextureFilter, NativeUtf8Slice, NativeVec2, NativeVec3,
};

/// A rig the Engine read from product content, with its pages.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativePartsRigHandle {
    pub value: u64,
}

/// A rig's model document in product content. Its pages are read from the
/// files the document names, beside it.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePartsRigLoadRequest {
    pub path: NativeUtf8Slice,
    /// How the pages are sampled: `Nearest` for pixel art.
    pub filter: NativeTextureFilter,
}

/// A loaded rig and what it has.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePartsRigInfo {
    pub rig: NativePartsRigHandle,
    pub bones: u32,
    pub slots: u32,
    pub animations: u32,
    pub skins: u32,
    pub pages: u32,
}

/// One view of a rig appearance: the rig it shows from a heading.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePartsRigView {
    pub rig: NativePartsRigHandle,
    /// The bearing it is seen from, in radians about the object's up axis
    /// from its +z toward its +x: 0 is a camera in front of the object.
    pub heading: f32,
    /// Drawn mirrored across the rig's vertical axis.
    pub mirror: bool,
}

/// An appearance that shows a parts rig, facing the camera as a sprite does.
/// With more than one view, each reading of the scene (the window, the
/// stream, a capture) draws the view whose heading is nearest the bearing
/// its camera sees the object from. The views play one clip time and one
/// set of driven values.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePartsRigAppearanceRequest {
    /// Borrowed for this call; at least one.
    pub views: *const NativePartsRigView,
    pub views_len: usize,
    /// Metres per rig unit, or CSS pixels per rig unit for a pixel-sized
    /// rig, before the object's scale.
    pub scale: f32,
    pub size_mode: NativeSpriteSizeMode,
    pub billboard: NativeBillboardMode,
    pub render_order: i32,
    pub depth: NativeSpriteDepthPolicy,
    /// Linear RGBA multiplying every piece.
    pub tint: NativeColor,
    /// A sprite's lighting, alpha, shadow policy, emission and authored
    /// facing; texture maps, softness and wind are a sprite's alone.
    pub material: NativeSpriteMaterialDescriptor,
    /// Rig units per texel of the grid the figure draws on, for pixel art;
    /// 0 draws it smooth.
    pub texel_snap: f32,
}

/// Play a clip on a published rig object, from its start, fading in over
/// what played before. Every view's rig needs a clip of that name.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePartsRigPlayRequest {
    pub object_id: u64,
    pub clip: NativeUtf8Slice,
    pub looping: bool,
    /// Multiplies the time the object's clips and fades advance by; 1 plays
    /// as authored.
    pub speed: f32,
    pub crossfade_seconds: f32,
}

/// Fade a published rig object back to its setup pose.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePartsRigStopRequest {
    pub object_id: u64,
    pub crossfade_seconds: f32,
}

/// Show a named skin on a published rig object; empty shows the default
/// skin alone. Every view's rig needs a skin of that name.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePartsRigSkinRequest {
    pub object_id: u64,
    pub skin: NativeUtf8Slice,
}

/// One of a bone's local values, as a product drives it.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativePartsRigBoneChannel {
    /// Degrees counter-clockwise.
    Rotation = 0,
    X = 1,
    Y = 2,
    ScaleX = 3,
    ScaleY = 4,
    ShearX = 5,
    ShearY = 6,
}

/// Hold one of a named bone's local values on a published rig object, over
/// what its clips pose and before its constraints read it, in every view
/// whose rig has the bone: a driving value such as a `turn` bone. `held`
/// false lets the clips pose it again.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePartsRigDriveRequest {
    pub object_id: u64,
    pub bone: NativeUtf8Slice,
    pub channel: NativePartsRigBoneChannel,
    pub held: bool,
    pub value: f32,
}

/// What a published rig object plays, as it was last posed.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativePartsRigReadout {
    pub views: u32,
    pub playing: bool,
    /// The playing clip's name, empty when none plays.
    pub clip: NativeAnimationFeedbackText,
    /// The playing clip's time in seconds, which every view shares.
    pub clip_seconds: f64,
    pub speed: f32,
    /// Whether it has been posed: false until the call that published it
    /// settles.
    pub posed: bool,
}

/// One event a rig object's clip passed.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativePartsRigEvent {
    pub object_id: u64,
    /// The event's name, cut to 96 bytes.
    pub name: NativeAnimationFeedbackText,
    /// The key's time in its clip, in seconds.
    pub clip_seconds: f64,
    pub int_value: i64,
    pub float_value: f32,
    pub volume: f32,
    pub balance: f32,
}

/// The events rig objects' clips passed as the calls since the previous
/// update settled, object by object and each in time order, borrowed until
/// the next PartsRigs call. A view set fires its first view's events.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePartsRigEventResult {
    pub events: *const NativePartsRigEvent,
    pub events_len: usize,
}

/// A named bone of one view of a published rig object.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePartsRigBoneRequest {
    pub object_id: u64,
    pub bone: NativeUtf8Slice,
    /// The view's index in the appearance's views.
    pub view: u32,
}

/// A bone as the rig object was last posed.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativePartsRigBoneReadout {
    /// In rig units, x right and y up from the rig's origin, as authored
    /// (before the view's mirror).
    pub position: NativeVec2,
    /// The bone's x axis, degrees counter-clockwise, as authored.
    pub rotation_degrees: f32,
    /// The lengths of its x and y axes.
    pub scale: NativeVec2,
    /// Where it lies in the object's space on the rig's unturned plane
    /// (facing the object's +z), in the units of the appearance's scale and
    /// with the view's mirror: metres for a world-sized rig.
    pub offset: NativeVec3,
}

pub type NativeLoadPartsRig = unsafe extern "C" fn(
    context: *mut c_void,
    request: *const NativePartsRigLoadRequest,
    result: *mut NativePartsRigInfo,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32;

pub type NativeDestroyPartsRig = unsafe extern "C" fn(
    context: *mut c_void,
    rig: NativePartsRigHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32;

pub type NativeCreatePartsRigAppearance = unsafe extern "C" fn(
    context: *mut c_void,
    request: *const NativePartsRigAppearanceRequest,
    result: *mut NativeAppearanceHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32;

pub type NativePlayPartsRig = unsafe extern "C" fn(
    context: *mut c_void,
    request: *const NativePartsRigPlayRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32;

pub type NativeStopPartsRig = unsafe extern "C" fn(
    context: *mut c_void,
    request: NativePartsRigStopRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32;

pub type NativeSetPartsRigSkin = unsafe extern "C" fn(
    context: *mut c_void,
    request: *const NativePartsRigSkinRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32;

pub type NativeDrivePartsRigBone = unsafe extern "C" fn(
    context: *mut c_void,
    request: *const NativePartsRigDriveRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32;

pub type NativeReadPartsRig = unsafe extern "C" fn(
    context: *mut c_void,
    object_id: u64,
    readout: *mut NativePartsRigReadout,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32;

pub type NativeReadPartsRigEvents = unsafe extern "C" fn(
    context: *mut c_void,
    result: *mut NativePartsRigEventResult,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32;

pub type NativeReadPartsRigBone = unsafe extern "C" fn(
    context: *mut c_void,
    request: *const NativePartsRigBoneRequest,
    readout: *mut NativePartsRigBoneReadout,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32;

/// Parts rigs read from product content, posed by the Engine once per call
/// and drawn as upright pieces in the world.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePartsRigsApi {
    pub context: *mut c_void,
    pub load: NativeLoadPartsRig,
    pub destroy: NativeDestroyPartsRig,
    pub create_appearance: NativeCreatePartsRigAppearance,
    pub destroy_appearance: crate::NativeDestroyAppearance,
    pub play: NativePlayPartsRig,
    pub stop: NativeStopPartsRig,
    pub set_skin: NativeSetPartsRigSkin,
    pub drive_bone: NativeDrivePartsRigBone,
    pub read: NativeReadPartsRig,
    pub read_events: NativeReadPartsRigEvents,
    pub read_bone: NativeReadPartsRigBone,
}
