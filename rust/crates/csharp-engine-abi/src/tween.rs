use std::ffi::c_void;

use crate::{NativeOperationErrorReceipt, NativeVec3, NativeVec4};

/// One tween the Engine plays over a published object. Handles are values:
/// a tween that ended reads as `Ended` and needs no release.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeTweenHandle {
    pub value: u64,
}

/// What a segment animates. Values are `xyz` for translation and scale,
/// an `xyzw` quaternion for rotation and RGBA for tint.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeTweenChannel {
    /// Parent-space offset added to the published translation.
    Translation = 0,
    /// Rotation applied in the object's local frame after the published one.
    Rotation = 1,
    /// Local scale multiplying the published scale.
    Scale = 2,
    /// Multiplies a sprite's tint or a primitive's colour.
    Tint = 3,
}

/// Each channel shows its latest-started base segment, holding its end value
/// (and, before the first starts, that segment's start). An additive segment
/// shows nothing before it starts and holds its end value after it.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeTweenLayer {
    Base = 0,
    Additive = 1,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeTweenShape {
    /// `from` to `to` by the easing; on translation, `arc` peaks mid-way on a
    /// parabola in linear time (a hop).
    Tween = 0,
    /// Swings from `from` toward `to` and back `frequency` times, the swing
    /// shrinking to nothing by the easing.
    Punch = 1,
    /// Smooth noise around `from` reaching up to `to - from` per component,
    /// `frequency` changes per second, fading by the easing; `seed` selects
    /// the noise.
    Shake = 2,
    /// A Catmull-Rom span from `from` to `to` through `before` and `after`.
    Spline = 3,
}

/// Easing curves. Each `*In`/`*Out`/`*InOut` triple is the standard curve;
/// `CubicBezier` takes CSS control values `(x1, y1, x2, y2)` in
/// `parameter_0..3`; `Steps` holds `parameter_0` levels; `Spring` takes
/// stiffness and damping in `parameter_0` and `parameter_1` (a unit-mass
/// spring released toward the target, its settle time stretched over the
/// segment; 0 selects a stiff, critically damped spring).
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeTweenEasingKind {
    Linear = 0,
    QuadIn = 1,
    QuadOut = 2,
    QuadInOut = 3,
    CubicIn = 4,
    CubicOut = 5,
    CubicInOut = 6,
    QuartIn = 7,
    QuartOut = 8,
    QuartInOut = 9,
    QuintIn = 10,
    QuintOut = 11,
    QuintInOut = 12,
    SineIn = 13,
    SineOut = 14,
    SineInOut = 15,
    ExpoIn = 16,
    ExpoOut = 17,
    ExpoInOut = 18,
    CircIn = 19,
    CircOut = 20,
    CircInOut = 21,
    BackIn = 22,
    BackOut = 23,
    BackInOut = 24,
    ElasticIn = 25,
    ElasticOut = 26,
    ElasticInOut = 27,
    BounceIn = 28,
    BounceOut = 29,
    BounceInOut = 30,
    CubicBezier = 31,
    Steps = 32,
    Spring = 33,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeTweenEasing {
    pub kind: NativeTweenEasingKind,
    pub parameter_0: f32,
    pub parameter_1: f32,
    pub parameter_2: f32,
    pub parameter_3: f32,
}

/// One timeline segment; the timeline's iteration ends with its last
/// segment. `arc` applies to translation tweens, `frequency` to punch and
/// shake, `seed` to shake, `before` and `after` to splines.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeTweenSegment {
    pub start_seconds: f32,
    pub duration_seconds: f32,
    pub channel: NativeTweenChannel,
    pub layer: NativeTweenLayer,
    pub shape: NativeTweenShape,
    pub easing: NativeTweenEasing,
    pub from: NativeVec4,
    pub to: NativeVec4,
    pub arc: NativeVec3,
    pub frequency: f32,
    pub seed: u32,
    pub before: NativeVec4,
    pub after: NativeVec4,
}

/// Reported each iteration the timeline passes `time_seconds`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeTweenMarker {
    pub marker_id: u64,
    pub time_seconds: f32,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeTweenClock {
    /// World presentation time: advances with admitted steps, so it holds
    /// and slows with gameplay time.
    World = 0,
    /// Unscaled host time (`HostElapsedSeconds`): keeps moving while the
    /// world is held.
    Realtime = 1,
}

/// How a tween starts on an object that may already be tweening.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeTweenStart {
    /// Ends the object's tweens and plays this one as given.
    Replace = 0,
    /// Ends the object's tweens; each channel's first base tween or spline
    /// segment starts from the pose and tint shown, so motion continues
    /// without a jump.
    FromPresented = 1,
    /// Plays alongside the object's tweens; their offsets compose.
    Layer = 2,
}

/// Starts a tween on a published object. It plays from the end of the
/// call, advances by its clock at the start of each update, and shows its
/// offset over the object's published transform and colour. A republish
/// changes the published values under it; removing the object ends it.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeTweenStartRequest {
    pub object_id: u64,
    pub segments: *const NativeTweenSegment,
    pub segments_len: usize,
    pub markers: *const NativeTweenMarker,
    pub markers_len: usize,
    /// Iterations to play; 0 and 1 both play once.
    pub iterations: u32,
    pub forever: bool,
    /// Every second iteration plays backwards.
    pub yoyo: bool,
    pub clock: NativeTweenClock,
    pub start: NativeTweenStart,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeTweenState {
    Playing = 0,
    Paused = 1,
    /// Reached its end in this update; it ends after the update.
    Completed = 2,
    /// Completed, cancelled, replaced, or its object removed. Its offset no
    /// longer shows.
    Ended = 3,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeTweenControl {
    Pause = 0,
    Resume = 1,
    /// Jumps to the end; reported as completed at the next update. A tween
    /// that plays forever stops where it is.
    Complete = 2,
    /// Ends now: the published values show from the end of the call.
    Cancel = 3,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeTweenControlRequest {
    pub tween: NativeTweenHandle,
    pub control: NativeTweenControl,
}

/// `total_seconds` is infinite for a tween that plays forever.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeTweenReadout {
    pub tween: NativeTweenHandle,
    pub object_id: u64,
    pub state: NativeTweenState,
    pub elapsed_seconds: f64,
    pub iteration_seconds: f64,
    pub total_seconds: f64,
    pub iteration: u32,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeTweenEventKind {
    Marker = 0,
    Completed = 1,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeTweenEvent {
    pub tween: NativeTweenHandle,
    pub object_id: u64,
    pub kind: NativeTweenEventKind,
    pub marker_id: u64,
    pub iteration: u32,
}

/// The latest update's events, tween by tween in start order and each
/// tween's in time order, borrowed until the next Tween call.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeTweenEventResult {
    pub events: *const NativeTweenEvent,
    pub events_len: usize,
}

pub type NativeStartTween = unsafe extern "C" fn(
    context: *mut c_void,
    request: *const NativeTweenStartRequest,
    readout: *mut NativeTweenReadout,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32;

pub type NativeControlTween = unsafe extern "C" fn(
    context: *mut c_void,
    request: NativeTweenControlRequest,
    readout: *mut NativeTweenReadout,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32;

pub type NativeReadTween = unsafe extern "C" fn(
    context: *mut c_void,
    tween: NativeTweenHandle,
    readout: *mut NativeTweenReadout,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32;

pub type NativeReadTweenEvents = unsafe extern "C" fn(
    context: *mut c_void,
    result: *mut NativeTweenEventResult,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32;

/// Engine-played presentation tweens over published appearance objects.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeTweenApi {
    pub context: *mut c_void,
    pub start: NativeStartTween,
    pub control: NativeControlTween,
    pub read: NativeReadTween,
    pub read_events: NativeReadTweenEvents,
}
