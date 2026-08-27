use crate::*;

/// Opaque Engine-owned camera identity. A product may select, update, replace,
/// and dispose a camera, but never receives a renderer or backend object.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeCameraHandle {
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
