//! Answers to the Engine's own live-debug commands. The browser shell's
//! playtest adapter and the live-debug panel read them, so they are typed
//! here and emitted to TypeScript with the rest of the wire.

use std::collections::BTreeMap;

use render_host_contracts::RendererCameraPose;
use serde::Serialize;
use ts_rs::TS;

use crate::CanonicalU64;

/// Answer to `engine.renderer`, `.status`, `.show`, `.hide` and `.toggle`.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProductHostRendererStatus {
    pub available: bool,
    pub widget: ProductHostRendererWidget,
    /// Why no renderer statistics are available.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub diagnostic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub renderer: Option<ProductHostRendererStatistics>,
}

/// The renderer metrics widget every mounted live-debug panel shares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProductHostRendererWidget {
    pub visible: bool,
}

/// The runtime renderer's adapter and what its recent frames cost.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProductHostRendererStatistics {
    pub adapter: String,
    pub output: ProductHostRenderOutput,
    /// The recent streamed frames; the desktop window streams nothing.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub stream: Option<ProductHostStreamStatistics>,
    /// The recent frames the desktop window presented.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub window: Option<ProductHostWindowStatistics>,
    /// When each recent product update that received input finished, and
    /// the step it simulated: the first frame showing that step or a later
    /// one is the first to show the input.
    pub input_steps: Vec<ProductHostTimedStep>,
    /// Retained operations the renderer skipped, by kind.
    #[ts(type = "Record<string, number>")]
    pub skipped_ops: BTreeMap<String, u64>,
    pub last_skip: Option<String>,
    pub shadows: ProductHostShadowStatistics,
    /// The renderer's GPU passes: each timed pass's cost, the adapter's
    /// compute limits, and the ambient occlusion the last world view took.
    pub gpu: ProductHostGpuStatistics,
}

/// The scene's shadow layers and which requesting lights cast.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProductHostShadowStatistics {
    /// Layers in the shadow atlas, and the 2048² pages holding them.
    pub layers: usize,
    pub pages: usize,
    /// `RustyEngineProductShadowBudget` in layers, if the manifest sets one.
    pub budget: Option<u32>,
    /// Lights casting, and the renderer handles of those requesting a
    /// shadow that the budget left out.
    pub casting_lights: usize,
    #[ts(type = "number[]")]
    pub skipped_lights: Vec<u64>,
    /// Layers re-rendered in the last frame and the casters drawn into them.
    pub rendered_layers: u32,
    pub rendered_casters: u32,
}

/// What the renderer's GPU passes cost, from the device's timestamp
/// queries, with the adapter's compute limits.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProductHostGpuStatistics {
    /// The device has timestamp queries, so the passes are timed.
    pub timestamps: bool,
    pub limits: ProductHostComputeLimits,
    /// The timed passes, in frame order.
    pub passes: Vec<ProductHostGpuPass>,
    pub ambient_occlusion: ProductHostAmbientOcclusionStatistics,
}

/// The screen-space ambient occlusion of the last world view.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProductHostAmbientOcclusionStatistics {
    pub path: ProductHostAmbientOcclusionPath,
    /// Why the compute path cannot run on this device; absent while it can.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub compute_refused: Option<String>,
    /// Workgroups the last occlusion dispatch took; 0 on the raster path.
    pub workgroups: u32,
    /// The occlusion texture of the last view, in texels.
    pub texture: (u32, u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ProductHostAmbientOcclusionPath {
    Off,
    Compute,
    Raster,
}

/// One timed pass's GPU cost over the recent frames.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProductHostGpuPass {
    pub pass: String,
    /// Recent frames whose GPU time was read back.
    pub timed_frames: usize,
    /// Median GPU milliseconds of the pass over those frames; 0 with none.
    pub median_gpu_ms: f64,
}

/// The adapter's compute limits; the renderer's device takes wgpu's defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProductHostComputeLimits {
    pub workgroup_size: [u32; 3],
    pub invocations_per_workgroup: u32,
    pub workgroups_per_dimension: u32,
    pub workgroup_storage_bytes: u32,
    #[ts(type = "number")]
    pub storage_buffer_binding_bytes: u64,
}

/// What the recent streamed frames cost.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProductHostStreamStatistics {
    /// The size the most recent viewer asked for, in its pixels.
    pub viewer_size: Option<(u32, u32)>,
    pub recent_frames: usize,
    pub frames_per_second: f64,
    pub median_ms: ProductHostStreamMedians,
    pub median_bytes_per_frame: f64,
    pub bytes_per_second: f64,
    /// When each recent frame was published, and the step it showed.
    pub shown: Vec<ProductHostTimedStep>,
}

/// What the recent frames the desktop window presented cost, and when they
/// reached it.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProductHostWindowStatistics {
    pub recent_frames: usize,
    pub frames_per_second: f64,
    pub median_ms: ProductHostWindowMedians,
    /// When each recent frame was presented, and the step it showed.
    pub shown: Vec<ProductHostTimedStep>,
    /// When recent key and mouse button events reached the window, in Unix
    /// milliseconds.
    pub inputs_received_at_unix_ms: Vec<f64>,
}

/// Median milliseconds per frame for each stage of presenting it: waiting
/// for the swapchain image, waiting for the scene, encoding and submitting
/// the frame, and presenting it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProductHostWindowMedians {
    pub acquire: f64,
    pub lock: f64,
    pub draw: f64,
    pub present: f64,
}

/// A simulation step and when something happened to it, in Unix
/// milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProductHostTimedStep {
    pub at_unix_ms: f64,
    #[ts(type = "number")]
    pub step: u64,
}

/// Median milliseconds per frame for each stage of streaming it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProductHostStreamMedians {
    pub render: f64,
    pub readback: f64,
    pub encode: f64,
}

/// Answer to `engine.renderer.camera`, `.drawing` and `.frame`.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProductHostRendererInspection {
    pub drawing: ProductHostDrawingMode,
    pub output: ProductHostRenderOutput,
    /// The simulation is held, so frames show one step.
    pub held: bool,
    /// An observer camera replaces the product's primary camera.
    pub observer: bool,
    /// The observer camera, else the primary camera, when known.
    pub camera: Option<ProductHostCameraPose>,
    /// The frame the command drew, else the last one drawn. The desktop
    /// window does not number its frames.
    pub frame: Option<ProductHostDrawnFrame>,
}

/// A camera pose as `engine.renderer.camera` reports it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProductHostCameraPose {
    pub position: [f64; 3],
    pub pitch_degrees: f64,
    pub yaw_degrees: f64,
}

impl From<RendererCameraPose> for ProductHostCameraPose {
    fn from(pose: RendererCameraPose) -> Self {
        Self {
            position: pose.position,
            pitch_degrees: pose.pitch_degrees,
            yaw_degrees: pose.yaw_degrees,
        }
    }
}

/// Where the runtime presents the frames it renders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum ProductHostRenderOutput {
    /// Streamed to the browser page.
    Stream,
    /// Presented to the desktop window, which draws every frame itself.
    Window,
}

impl ProductHostRenderOutput {
    /// Its name in `renderer.output` and the browser bootstrap.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Stream => "stream",
            Self::Window => "window",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum ProductHostDrawingMode {
    /// A frame for every change.
    Continuous,
    /// A frame only when a command asks for one.
    OnDemand,
}

/// A streamed frame: its sequence on the frame route and the step it shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProductHostDrawnFrame {
    #[ts(type = "number")]
    pub sequence: u64,
    #[ts(type = "number")]
    pub step: u64,
}

/// Answer to `engine.time`, `engine.time.mode` and `engine.time.advance`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProductHostTimeAnswer {
    pub mode: ProductHostTimeMode,
    /// The last admitted simulation step.
    pub simulation_step: CanonicalU64,
    pub fixed_step_hz: u32,
    /// Simulation time this command advanced.
    pub advanced_ms: f64,
    /// The world waits for commands to advance it.
    pub world_held: bool,
}

/// Who advances simulation time. Inspection time moves only forward.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum ProductHostTimeMode {
    Realtime,
    /// Only `engine.time.advance` advances it.
    Manual,
    /// Only playtest actions and `engine.time.advance` advance it.
    ActionDriven,
}
