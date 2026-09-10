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
