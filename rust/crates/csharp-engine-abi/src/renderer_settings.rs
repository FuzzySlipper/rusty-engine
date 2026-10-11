use std::ffi::c_void;

use crate::{NativeOperationErrorReceipt, NativeUtf8Slice};

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

/// How finely volumetric fog is computed: off (the analytic distance fog
/// alone), or a froxel grid lit by the scene's lights and shadows at low or
/// high resolution. Needs compute shaders on a GPU; a software adapter draws
/// without it.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeVolumetricFogQuality {
    Off = 0,
    Low = 1,
    High = 2,
}

/// How the sky's clouds are drawn: the flat layer, or raymarched clouds
/// with thickness at low or high quality. A software adapter draws the flat
/// layer.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeVolumetricCloudsQuality {
    Off = 0,
    Low = 1,
    High = 2,
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
    /// With `shadows`, the brightest casting sun also shadows the backdrop
    /// (`CameraView.SetBackdrop`) through cascades of its own.
    pub backdrop_shadows: bool,
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
    /// Light the fog medium and fog volumes (`CameraView.SetVolumetricFog`,
    /// `SetFogVolume`) in a froxel grid.
    pub volumetric_fog: NativeVolumetricFogQuality,
    /// Draw the cloud layer raymarched, with thickness.
    pub volumetric_clouds: NativeVolumetricCloudsQuality,
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
    /// A software adapter draws without this GPU-only feature.
    SoftwareAdapter = 6,
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
    pub volumetric_fog_refusal: NativeRendererSettingRefusal,
    pub volumetric_clouds_refusal: NativeRendererSettingRefusal,
}

/// How a renderer setting is chosen.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeRendererSettingKind {
    /// On or off: its values are `true` and `false`.
    Toggle = 0,
    /// One of named values (`NativeRendererSettingChoiceReadout`).
    Choice = 1,
    /// A number from `min` to `max` in `step`s, in `unit`.
    Range = 2,
}

/// One renderer setting as the catalogue describes it, for a product's own
/// menu (`RendererSettings.Describe`). Values are in their text form, as the
/// video options and `rusty-scene-render --choose` write them: `true`, a
/// choice's value such as `4x` or `none`, or a number.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeRendererSettingOptionReadout {
    /// Stable identity: the video options' key for it.
    pub id: NativeUtf8Slice,
    pub label: NativeUtf8Slice,
    /// Display, Quality, Lighting or Advanced.
    pub group: NativeUtf8Slice,
    pub description: NativeUtf8Slice,
    pub kind: NativeRendererSettingKind,
    pub min: f64,
    pub max: f64,
    pub step: f64,
    pub unit: NativeUtf8Slice,
    /// The Engine's default value.
    pub engine_default: NativeUtf8Slice,
    /// What is asked of the device: the product's value with the player's
    /// video options over it.
    pub requested: NativeUtf8Slice,
    /// What draws.
    pub value: NativeUtf8Slice,
    /// Why the device draws it otherwise (`None` when it draws as asked).
    pub refusal: NativeRendererSettingRefusal,
    /// A change takes effect only when the product restarts.
    pub restart: bool,
    /// What it costs, as measured: a sentence for a menu.
    pub cost: NativeUtf8Slice,
}

/// One named value of a `Choice` setting.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeRendererSettingChoiceReadout {
    pub option_id: NativeUtf8Slice,
    pub value: NativeUtf8Slice,
    pub label: NativeUtf8Slice,
}

/// The renderer settings catalogue: every setting in a menu's order, and
/// the choices of those that have them. Borrowed until the next call on this
/// service; the generated binding copies it before returning to C#.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeRendererSettingsCatalogueResult {
    pub options: *const NativeRendererSettingOptionReadout,
    pub options_len: usize,
    pub choices: *const NativeRendererSettingChoiceReadout,
    pub choices_len: usize,
}

pub type NativeDescribeRendererSettings = unsafe extern "C" fn(
    context: *mut c_void,
    catalogue: *mut NativeRendererSettingsCatalogueResult,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32;

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
    pub describe: NativeDescribeRendererSettings,
}
