//! Offline output from a frozen retained scene. Requests settle after the
//! current product callback; completion includes resource admission and readback.
use crate::*;
use std::ffi::c_void;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeRenderOutputHandle {
    pub value: u64,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeRenderOutputState {
    Pending = 0,
    Completed = 1,
    Failed = 2,
    Cancelled = 3,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeCaptureToneMapping {
    None = 0,
    AcesFilmic = 1,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCaptureImageRequest {
    /// Retained product object subtree, including ancestor transforms and lights.
    pub source_object_id: u64,
    pub camera: NativeCameraHandle,
    pub width: u32,
    pub height: u32,
    /// Linear background RGBA; alpha zero produces a transparent PNG.
    pub background: NativeColor,
    /// Use the retained CameraView sky/color instead of the explicit clear color.
    pub use_camera_background: bool,
    pub exposure: f32,
    pub tone_mapping: NativeCaptureToneMapping,
    pub samples: u32,
    /// Zero keeps the frozen retained pose. Otherwise sample this animated node.
    pub pose_object_id: u64,
    pub pose_clip: NativeUtf8Slice,
    /// Exact normalized clip time, including the endpoint; no wall-clock advance.
    pub pose_time: f64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeExportSceneGlbRequest {
    pub source_object_id: u64,
    pub include_animations: bool,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeRenderOutputReadout {
    pub state: NativeRenderOutputState,
    pub byte_length: usize,
}

pub type NativeCaptureImage = unsafe extern "C" fn(
    *mut c_void,
    *const NativeCaptureImageRequest,
    *mut NativeRenderOutputHandle,
) -> i32;
pub type NativeExportSceneGlb = unsafe extern "C" fn(
    *mut c_void,
    *const NativeExportSceneGlbRequest,
    *mut NativeRenderOutputHandle,
) -> i32;
pub type NativeReadRenderOutput = unsafe extern "C" fn(
    *mut c_void,
    NativeRenderOutputHandle,
    *mut NativeRenderOutputReadout,
) -> i32;
pub type NativeReadRenderOutputBytes =
    unsafe extern "C" fn(*mut c_void, NativeRenderOutputHandle, *mut NativeByteLease) -> i32;
pub type NativeReadRenderOutputDiagnostic =
    unsafe extern "C" fn(*mut c_void, NativeRenderOutputHandle, *mut NativeByteLease) -> i32;
pub type NativeCancelRenderOutput =
    unsafe extern "C" fn(*mut c_void, NativeRenderOutputHandle) -> i32;
pub type NativeDestroyRenderOutput =
    unsafe extern "C" fn(*mut c_void, NativeRenderOutputHandle) -> i32;
pub type NativeDestroyRenderOutputByteLease =
    unsafe extern "C" fn(*mut c_void, NativeByteLeaseHandle) -> i32;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeRenderOutputApi {
    pub context: *mut c_void,
    pub capture_image: NativeCaptureImage,
    pub export_scene_glb: NativeExportSceneGlb,
    pub read: NativeReadRenderOutput,
    pub read_bytes: NativeReadRenderOutputBytes,
    /// UTF-8 error text; empty for a successful or pending job.
    pub read_diagnostic: NativeReadRenderOutputDiagnostic,
    pub cancel: NativeCancelRenderOutput,
    pub destroy: NativeDestroyRenderOutput,
    pub destroy_byte_lease: NativeDestroyRenderOutputByteLease,
}
