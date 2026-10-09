//! The runtime's HTTP host, for both outputs: it serves the product's UI page
//! (the browser shell streaming frames, or the desktop window's Chromium
//! page), the page's input and lifecycle requests, the SSE output bus, and
//! the realtime scheduler with its input mailbox. A shipped window-mode
//! product runs it too.
//!
//! Development facilities and how each is enabled:
//! - `debug/catalog`, `debug/execute` and `diagnostics/read`: `--live-debug`
//!   (`server.liveDebug`);
//! - asset reload (`ProductHostAssetReload`): only under the `rusty dev`
//!   supervisor;
//! - the `frames` route: stream output only; the window streams nothing;
//! - `frames/capture`, `control/claim` and `control/release`: tools and
//!   harnesses, answered in every output;
//! - a LAN bind: an explicit `bindHost`.
//!
//! The routes answered without `--live-debug` are listed in `host.rs`'s
//! tests, so a new one is added deliberately.
//!
//! This is deliberately a small HTTP/1.1 implementation rather than a product
//! network service. It defaults to `127.0.0.1`; an explicit trusted bind may
//! expose the same closed same-origin route vocabulary to a LAN. The native
//! product remains the concrete runtime owner: every operation is a direct,
//! serialized trait call and returns its own bounded output batch. The host
//! stores neither gameplay callbacks nor product state.
//!
//! The implementation uses `std::net` instead of a general HTTP framework so
//! its trust boundary is auditable in one module. It supports only HTTP/1.1
//! request lines, fixed `Content-Length` POST bodies, and one SSE response;
//! it rejects connection reuse, chunked transfer, CORS, cookies, arbitrary
//! methods, arbitrary routes, and implicit non-loopback binds.

#![forbid(unsafe_code)]

mod activity;
mod audio;
mod bundle;
mod engine_debug;
mod error;
mod frames;
mod host;
mod log;
mod model;
mod presentation;
mod publication;
mod scheduler;
mod session;
mod timeline;
#[cfg(test)]
mod typescript;
mod video_options;

pub use activity::ProductHostActivity;
pub use audio::{
    ProductHostAudioStream, PRODUCT_HOST_AUDIO_CHANNELS, PRODUCT_HOST_AUDIO_PATH,
    PRODUCT_HOST_AUDIO_SAMPLE_RATE,
};
pub use bundle::{
    ProductHostBootstrapInput, ProductHostBootstrapProduct, ProductHostBootstrapRenderer,
    ProductHostBootstrapUi, ProductHostBootstrapUiProjection, ProductHostBrowserBootstrap,
    ProductHostBundle, ProductHostBundleEntry, ProductHostCursorMode,
    ProductHostPresentationAspect, PRODUCT_HOST_BOOTSTRAP_PATH, PRODUCT_HOST_INDEX_PATH,
};
pub use engine_debug::{
    ProductHostAmbientOcclusionPath, ProductHostAmbientOcclusionStatistics, ProductHostCameraPose,
    ProductHostComputeLimits, ProductHostDistanceFieldStatistics, ProductHostDrawingMode,
    ProductHostDrawnFrame, ProductHostGpuCullingStatistics, ProductHostGpuPass,
    ProductHostGpuStatistics, ProductHostIndirectLightStatistics,
    ProductHostLightClusterStatistics, ProductHostRenderOutput, ProductHostRendererInspection,
    ProductHostRendererSettingValues, ProductHostRendererSettings, ProductHostRendererStatistics,
    ProductHostRendererStatus, ProductHostRendererWidget, ProductHostShadowStatistics,
    ProductHostStreamMedians, ProductHostStreamStatistics, ProductHostTimeAnswer,
    ProductHostTimeMode, ProductHostTimedStep, ProductHostWindowMedians,
    ProductHostWindowStatistics,
};
pub use error::{ProductHostError, ProductHostRuntimeError};
pub use frames::{
    ProductHostCapture, ProductHostCaptureRequest, ProductHostFrame, ProductHostFrameCapture,
    ProductHostFrameFormat, ProductHostFrameStream, FRAME_REQUEST_WAIT, PRODUCT_HOST_FRAMES_PATH,
    PRODUCT_HOST_FRAME_CAPTURE_PATH,
};
pub use host::{
    ProductHost, ProductHostAssetReload, ProductHostConfig, ProductHostUiFiles, RunningProductHost,
};
pub use log::{
    ProductHostLog, ProductHostLogBatch, ProductHostLogConfig, ProductHostLogDisposition,
    ProductHostLogEvent, ProductHostLogSeverity, ProductHostLogSnapshot, ProductHostLogWriterState,
};
pub use model::{
    runtime_fault_disposition, CanonicalU64, ProductHostBrowserAttachment,
    ProductHostBrowserAttachmentBaseline, ProductHostBrowserConnectionState,
    ProductHostBrowserDiagnosticsReport, ProductHostBrowserDiagnosticsResult,
    ProductHostBrowserHostState, ProductHostBrowserPageDiagnostic,
    ProductHostBrowserPageDiagnosticKind, ProductHostBrowserTerminalDiagnostic,
    ProductHostControlOperation, ProductHostDebugCatalog, ProductHostDebugCommandDescriptor,
    ProductHostDebugCommandParameterDescriptor, ProductHostDebugResult,
    ProductHostFaultDisposition, ProductHostInputBatch, ProductHostInputResult,
    ProductHostLifecycleOperation, ProductHostOperationKind, ProductHostOperationResult,
    ProductHostRuntime, ProductHostRuntimeBinding, ProductHostRuntimeFault,
    ProductHostRuntimeOutput, ProductHostRuntimeReadout, ProductHostRuntimeReceipt,
    ProductHostRuntimeScheduleState, ProductHostRuntimeState, ProductHostTelemetrySnapshot,
    ProductHostTimelineCompletion, ProductHostTimelineCompletionResult,
    ProductHostUpdateAttribution, ProductHostUpdateAttributionSnapshot, PRODUCT_HOST_ARTIFACT,
    PRODUCT_HOST_RUNTIME_BASE_PATH,
};
pub use presentation::{
    ProductHostPresentation, ProductHostPresentationLayout, ProductHostPresentationReport,
    ProductHostViewportAnchorReport, PRODUCT_HOST_PRESENTATION_PATH,
};
pub use publication::{
    RuntimePublication, RuntimePublicationError, RuntimePublicationFrontier, RuntimeReceipt,
};
pub use scheduler::advance_realtime_with_input_and_publish;
pub use session::{ProductHostOperationOwner, RuntimeSession};
pub use timeline::TimelineCompletionOutcome;
pub use video_options::{
    ProductHostVideoOptions, ProductHostVideoOptionsRequest, PRODUCT_HOST_VIDEO_OPTIONS_PATH,
};

/// Upper bound for one HTTP request header block, including its terminator.
pub const MAX_REQUEST_HEADER_BYTES: usize = 16 * 1024;
/// Upper bound for one JSON request body or emitted JSON response body.
pub const MAX_REQUEST_BODY_BYTES: usize = 512 * 1024;
/// Unsent output events one SSE subscriber may hold. A subscriber that falls
/// this far behind is closed and reconnects for a fresh baseline.
pub const MAX_SUBSCRIBER_QUEUE_EVENTS: usize = 256;
/// Upper bound for live accepted TCP connections, including SSE clients.
pub const MAX_CONNECTIONS: usize = 32;
/// Upper bound for simultaneous SSE subscribers.
pub const MAX_SSE_SUBSCRIBERS: usize = 8;
