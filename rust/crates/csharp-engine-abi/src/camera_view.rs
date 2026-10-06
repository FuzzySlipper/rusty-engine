use crate::*;

/// Opaque Engine-owned camera identity. A product may select, update, replace,
/// and dispose a camera, but never receives a renderer or backend object.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeCameraHandle {
    pub value: u64,
}

/// Opaque Engine-owned offscreen target identity. The renderer owns the GPU
/// target; products only select typed dimensions and sampling facts.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeCameraTargetHandle {
    pub value: u64,
}

/// Copied target reference used by a composition view. Zero names the Engine
/// primary surface; nonzero values refer to one live Engine-owned target.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeCameraTargetReference {
    pub value: u64,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeCameraBasisMode {
    Derived = 0,
    Explicit = 1,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeCameraProjectionKind {
    Perspective = 1,
    Orthographic = 2,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeCameraTargetColor {
    Rgba8Srgb = 0,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeCameraTargetDepth {
    Depth24 = 0,
    None = 1,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeCameraTargetSampling {
    Linear = 0,
    Nearest = 1,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeCameraPose {
    pub position: NativeVec3,
    pub pitch_degrees: f64,
    pub yaw_degrees: f64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeCameraBasis {
    pub forward: NativeVec3,
    pub right: NativeVec3,
    pub up: NativeVec3,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCameraProjection {
    pub kind: NativeCameraProjectionKind,
    pub fov_y_degrees: f64,
    pub vertical_size: f64,
    pub near: f64,
    pub far: f64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeCameraViewport {
    /// Normalized to the current Engine-owned presentation surface.
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// One retained offscreen target. Its revision is Engine-owned and changes
/// only when a live target is updated or replaced.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCameraTargetDescriptor {
    pub width: u32,
    pub height: u32,
    pub color: NativeCameraTargetColor,
    pub depth: NativeCameraTargetDepth,
    pub sampling: NativeCameraTargetSampling,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCameraTargetUpdateRequest {
    pub target: NativeCameraTargetHandle,
    pub descriptor: NativeCameraTargetDescriptor,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCameraTargetReplaceRequest {
    /// The prior handle becomes a tombstone if replacement succeeds.
    pub target: NativeCameraTargetHandle,
    pub replacement: NativeCameraTargetDescriptor,
}

/// A target reference of zero selects the Engine primary surface. Nonzero
/// references select retained Engine-owned offscreen targets.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCameraCompositionView {
    pub camera: NativeCameraHandle,
    pub target: NativeCameraTargetReference,
    pub viewport: NativeCameraViewport,
    pub order: u64,
}

/// Presents one retained offscreen target to the Engine primary surface.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCameraCompositionPresentation {
    pub source_target: NativeCameraTargetHandle,
    pub destination: NativeCameraViewport,
    pub order: u64,
}

/// Borrowed composition slices are copied before the direct call returns.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCameraCompositionRequest {
    pub views: *const NativeCameraCompositionView,
    pub views_len: usize,
    pub presentations: *const NativeCameraCompositionPresentation,
    pub presentations_len: usize,
}

/// Typed product facts for one Engine-owned view. The viewport is normalized,
/// so the Engine host realizes it against current resize observations.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCameraDescriptor {
    pub pose: NativeCameraPose,
    pub basis_mode: NativeCameraBasisMode,
    pub basis: NativeCameraBasis,
    pub projection: NativeCameraProjection,
    pub viewport: NativeCameraViewport,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCameraUpdateRequest {
    pub camera: NativeCameraHandle,
    pub descriptor: NativeCameraDescriptor,
}

/// Selects the opt-in renderer presentation sampling applied to a camera
/// update. `Latest` publishes the descriptor immediately; the interpolation
/// variants retain the supplied product timeline facts for the renderer.
/// The generated safe C# API exposes only these values; raw table consumers
/// must likewise pass one of these declared discriminants.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeCameraInterpolation {
    Latest = 0,
    Position = 1,
    Pose = 2,
}

/// The presentation surface camera viewports are normalized to, as the
/// page showing the product last reported it, in stream and window output
/// alike. `reported` is false until a page reports. `revision` changes with
/// every change, so a product notices a resize by comparing it.
///
/// `watching` is false only in stream output while no page pulls frames: it
/// turns false within 1.25 seconds of the last watching page leaving and true
/// as soon as one asks for a frame. In window output, and with no render
/// output, it is always true. It is not part of `revision`.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct NativeCameraSurfaceReadout {
    pub reported: bool,
    pub watching: bool,
    pub css_width: f64,
    pub css_height: f64,
    /// The surface in device pixels: its CSS size times the pixel ratio.
    pub device_width: f64,
    pub device_height: f64,
    pub device_pixel_ratio: f64,
    /// The scale the product UI applies to itself (`ui.setScale`).
    pub ui_scale: f64,
    pub revision: u64,
}

/// Makes a camera's primary views follow the product UI element the UI
/// anchors under `anchor` (`viewport.anchor(name, element)` in the UI
/// context): while the page reports that element's rect, the views draw there
/// instead of at their viewports, on resize and layout change with no product
/// call. An empty `anchor` removes it.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCameraViewportAnchorRequest {
    pub camera: NativeCameraHandle,
    pub anchor: crate::NativeUtf8Slice,
}

/// Names the product UI anchor (`viewport.anchor(name, element)`) to read.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCameraViewportAnchorReadRequest {
    pub anchor: crate::NativeUtf8Slice,
}

/// The rect of the UI element anchored under a name, as the page last
/// reported it: normalized to the presentation surface with a bottom-left
/// origin, like a camera viewport, and clipped to the surface. `reported` is
/// false, and the rect zero, while the page reports no element under the name
/// or one wholly outside the surface. `revision` is the surface revision of
/// the report it comes from ([`NativeCameraSurfaceReadout::revision`]).
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct NativeCameraViewportAnchorReadout {
    pub reported: bool,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub revision: u64,
}

/// One camera descriptor update with an optional renderer sampling contract.
/// The product supplies its admitted timeline and explicit cuts; the Engine
/// owns retained metadata and the opaque sample identity.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCameraSampleRequest {
    pub camera: NativeCameraHandle,
    pub descriptor: NativeCameraDescriptor,
    pub sample_time_seconds: f64,
    pub delay_seconds: f64,
    pub interpolation: NativeCameraInterpolation,
    pub cut: u8,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCameraReplaceRequest {
    /// This handle becomes a tombstone if replacement succeeds.
    pub camera: NativeCameraHandle,
    pub replacement: NativeCameraDescriptor,
}

/// Explicit empty requests keep the generated direct API uniformly typed.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeClearActiveCameraRequest {
    pub reserved: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeClearSkyBackgroundRequest {
    pub reserved: u32,
}

/// Selects one opaque renderer-owned viewport clear color. Selecting a sky later
/// replaces this color, and clearing the sky returns to the Engine default.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeSetBackgroundColorRequest {
    pub color: NativeColor,
}

/// Distance fog mode. `Off` clears the fog.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeFogMode {
    Off = 0,
    /// No fog before `start`, full fog at `end` and beyond (metres).
    Linear = 1,
    /// Visibility `exp(-density × distance)`.
    Exponential = 2,
    /// Visibility `exp(-(density × distance)²)`: clearer near, denser far.
    ExponentialSquared = 3,
}

/// Distance fog over everything drawn in the world, by distance from the
/// camera, toward `color` (linear RGB; alpha is ignored). It blends after
/// exposure and tone mapping and never touches the background, so a fog
/// colour equal to the background colour fades geometry exactly into it.
/// Linear fog reads `start` and `end`; the exponential modes read `density`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeFogRequest {
    pub mode: NativeFogMode,
    pub color: NativeColor,
    pub start: f32,
    pub end: f32,
    pub density: f32,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeToneMappingOperator {
    /// Colour clamps at the target's range (the default).
    None = 0,
    /// Khronos PBR Neutral: base colours stay true, highlights compress.
    Neutral = 1,
    /// ACES filmic: film-like contrast, highlights roll toward white.
    AcesFilmic = 2,
}

/// How lit colour reaches the output: a linear exposure multiplier (1 by
/// default), then the operator. Applies to everything drawn in the world, not
/// the background. Image captures use their own exposure and tone mapping.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeToneMappingRequest {
    pub operator: NativeToneMappingOperator,
    pub exposure: f32,
}

/// Bloom: the world's light above `threshold` (with a soft knee below it)
/// spreads into a glow added at `intensity` before exposure and tone
/// mapping. Intensity 0 (the default) turns it off; at most 16.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeBloomRequest {
    pub threshold: f32,
    pub intensity: f32,
}

/// Auto exposure, when `enabled`: the exposure moves toward the one that
/// brings the world's average luminance to middle grey, kept between
/// `min_exposure` and `max_exposure`, closing `1 - e^(-speed·t)` of the gap
/// in `t` seconds of Engine presentation time (none while it is held). The
/// tone mapping exposure multiplies it.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAutoExposureRequest {
    pub enabled: bool,
    pub speed: f32,
    pub min_exposure: f32,
    pub max_exposure: f32,
}

/// Blends two retained equirectangular panoramas. Amount is in [0,1]; the
/// product supplies its clock-derived value. Neither texture is recreated.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSkyBackgroundBlendRequest {
    pub first: NativeRenderResourceHandle,
    pub second: NativeRenderResourceHandle,
    pub amount: f32,
}
