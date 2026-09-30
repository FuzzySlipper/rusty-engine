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
pub struct ProductDevRendererStatus {
    pub available: bool,
    pub widget: ProductDevRendererWidget,
    /// Why no renderer statistics are available.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub diagnostic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub renderer: Option<ProductDevRendererStatistics>,
}

/// The renderer metrics widget every mounted live-debug panel shares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProductDevRendererWidget {
    pub visible: bool,
}

/// The runtime renderer's adapter and what its recent frames cost.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProductDevRendererStatistics {
    pub adapter: String,
    pub output: ProductDevRenderOutput,
    /// The recent streamed frames; the desktop window streams nothing.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub stream: Option<ProductDevStreamStatistics>,
    /// Retained operations the renderer skipped, by kind.
    #[ts(type = "Record<string, number>")]
    pub skipped_ops: BTreeMap<String, u64>,
    pub last_skip: Option<String>,
}

/// What the recent streamed frames cost.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProductDevStreamStatistics {
    /// The size the most recent viewer asked for, in its pixels.
    pub viewer_size: Option<(u32, u32)>,
    pub recent_frames: usize,
    pub frames_per_second: f64,
    pub median_ms: ProductDevStreamMedians,
    pub median_bytes_per_frame: f64,
    pub bytes_per_second: f64,
}

/// Median milliseconds per frame for each stage of streaming it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProductDevStreamMedians {
    pub render: f64,
    pub readback: f64,
    pub encode: f64,
}

/// Answer to `engine.renderer.camera`, `.drawing` and `.frame`.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProductDevRendererInspection {
    pub drawing: ProductDevDrawingMode,
    pub output: ProductDevRenderOutput,
    /// The simulation is held, so frames show one step.
    pub held: bool,
    /// An observer camera replaces the product's primary camera.
    pub observer: bool,
    /// The observer camera, else the primary camera, when known.
    pub camera: Option<ProductDevCameraPose>,
    /// The frame the command drew, else the last one drawn. The desktop
    /// window does not number its frames.
    pub frame: Option<ProductDevDrawnFrame>,
}

/// A camera pose as `engine.renderer.camera` reports it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProductDevCameraPose {
    pub position: [f64; 3],
    pub pitch_degrees: f64,
    pub yaw_degrees: f64,
}

impl From<RendererCameraPose> for ProductDevCameraPose {
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
pub enum ProductDevRenderOutput {
    /// Streamed to the browser page.
    Stream,
    /// Presented to the desktop window, which draws every frame itself.
    Window,
}

impl ProductDevRenderOutput {
    /// Its name in `RUSTY_RENDER_OUTPUT` and the browser bootstrap.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Stream => "stream",
            Self::Window => "window",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum ProductDevDrawingMode {
    /// A frame for every change.
    Continuous,
    /// A frame only when a command asks for one.
    OnDemand,
}

/// A streamed frame: its sequence on the frame route and the step it shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProductDevDrawnFrame {
    #[ts(type = "number")]
    pub sequence: u64,
    #[ts(type = "number")]
    pub step: u64,
}

/// Answer to `engine.time`, `engine.time.mode` and `engine.time.advance`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProductDevTimeAnswer {
    pub mode: ProductDevTimeMode,
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
pub enum ProductDevTimeMode {
    Realtime,
    /// Only `engine.time.advance` advances it.
    Manual,
    /// Only playtest actions and `engine.time.advance` advance it.
    ActionDriven,
}
