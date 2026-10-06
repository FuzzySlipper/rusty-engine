use std::ffi::c_void;

use crate::NativeOperationErrorReceipt;

/// Which pass darkens where surfaces meet.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAmbientOcclusionMode {
    Disabled = 0,
    /// From the view's depth: what the view shows occludes.
    ScreenSpace = 1,
    /// From the voxel chunks' distance fields: the world around a surface
    /// occludes, on screen or off. Needs compute shaders; a device without
    /// them takes the screen-space pass.
    DistanceField = 2,
}

/// Samples per pixel of the primary destination (the streamed frame or the
/// window). Offscreen camera targets and image captures are single-sample.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAntialiasing {
    Off = 1,
    Msaa2 = 2,
    Msaa4 = 4,
}

/// The renderer's settings: which pipeline features draw and at what
/// quality. The product manifest supplies the initial values
/// (`RustyEngineProduct*` properties); `Set` replaces them all from the next
/// frame and `Read` returns what draws.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeRendererSettingsRequest {
    /// Render shadow maps for lights whose shadow intent requests them.
    pub shadows: bool,
    /// At most this many shadow layers at once, the requesting lights chosen
    /// by priority then distance from the camera; 0 for no limit.
    pub shadow_budget: u32,
    pub ambient_occlusion: NativeAmbientOcclusionMode,
    /// 0 draws without occlusion; 1 is the full occlusion. Finite, at least
    /// 0.
    pub ambient_occlusion_strength: f32,
    /// How far a surface darkens its neighbours, in world units. Finite,
    /// above 0.
    pub ambient_occlusion_radius: f32,
    pub antialiasing: NativeAntialiasing,
    /// The fraction of the streamed frame's or window's size the world,
    /// viewmodel, labels and effects draw at before being upscaled into it:
    /// 0.5 to 1.
    pub render_scale: f32,
    /// Window output waits for the display's refresh before presenting.
    pub vsync: bool,
    /// Bin each world view's lights into view-frustum clusters before
    /// shading, instead of shading every light per fragment.
    pub clustered_lighting: bool,
    /// Test each view's opaque parts against its frustum on the GPU and draw
    /// them indirectly, instead of building the draw list on the CPU.
    pub gpu_culling: bool,
}

/// Why the device draws a setting differently from the request.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeRendererSettingRefusal {
    None = 0,
    NoComputeShaders = 1,
    NoIndirectDraws = 2,
    UnsupportedSampleCount = 3,
    /// Streamed output has no display to synchronise with.
    NoDisplay = 4,
    /// The window's display can present only in step with its refresh, so
    /// vsync stays on.
    VsyncOnly = 5,
}

/// What the renderer draws with: the last request (the manifest's values
/// until the product sets others), the settings in effect after the
/// device's refusals, and each refusal.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeRendererSettingsReadout {
    pub requested: NativeRendererSettingsRequest,
    pub effective: NativeRendererSettingsRequest,
    pub ambient_occlusion_refusal: NativeRendererSettingRefusal,
    pub antialiasing_refusal: NativeRendererSettingRefusal,
    pub vsync_refusal: NativeRendererSettingRefusal,
    pub clustered_lighting_refusal: NativeRendererSettingRefusal,
    pub gpu_culling_refusal: NativeRendererSettingRefusal,
}

pub type NativeReadRendererSettings = unsafe extern "C" fn(
    context: *mut c_void,
    readout: *mut NativeRendererSettingsReadout,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32;

pub type NativeSetRendererSettings = unsafe extern "C" fn(
    context: *mut c_void,
    request: *const NativeRendererSettingsRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32;

/// Renderer settings: read what draws, and select the pipeline features and
/// quality the renderer draws with.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeRendererSettingsApi {
    pub context: *mut c_void,
    pub read: NativeReadRendererSettings,
    pub set: NativeSetRendererSettings,
}
