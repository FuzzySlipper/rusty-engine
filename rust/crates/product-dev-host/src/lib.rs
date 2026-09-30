//! Engine-owned browser development host for native products.
//!
//! This is deliberately a small HTTP/1.1 implementation rather than a product
//! network service. It defaults to `127.0.0.1`; an explicit trusted-development
//! bind may expose the same closed same-origin route vocabulary to a LAN.
//! The native product remains the concrete runtime owner: every
//! operation is a direct, serialized trait call and returns its own bounded
//! output batch. The host stores neither gameplay callbacks nor product state.
//!
//! The implementation uses `std::net` instead of a general HTTP framework so
//! its trust boundary is auditable in one module. It supports only HTTP/1.1
//! request lines, fixed `Content-Length` POST bodies, and one SSE response;
//! it rejects connection reuse, chunked transfer, CORS, cookies, arbitrary
//! methods, arbitrary routes, and implicit non-loopback binds.

#![forbid(unsafe_code)]

mod bundle;
mod engine_debug;
mod error;
mod frames;
mod host;
mod log;
mod model;
mod scheduler;
mod session;
#[cfg(test)]
mod typescript;

pub use bundle::{
    ProductDevBootstrapInput, ProductDevBootstrapLifecycle, ProductDevBootstrapProduct,
    ProductDevBootstrapRenderer, ProductDevBootstrapUi, ProductDevBootstrapUiProjection,
    ProductDevBrowserBootstrap, ProductDevBundle, ProductDevBundleEntry, ProductDevCursorMode,
    PRODUCT_DEV_BOOTSTRAP_PATH, PRODUCT_DEV_INDEX_PATH,
};
pub use engine_debug::{
    ProductDevCameraPose, ProductDevDrawingMode, ProductDevDrawnFrame, ProductDevRenderOutput,
    ProductDevRendererInspection, ProductDevRendererStatistics, ProductDevRendererStatus,
    ProductDevRendererWidget, ProductDevStreamMedians, ProductDevStreamStatistics,
    ProductDevTimeAnswer, ProductDevTimeMode, ProductDevTimedStep, ProductDevWindowMedians,
    ProductDevWindowStatistics,
};
pub use error::{ProductDevHostError, ProductDevRuntimeError};
pub use frames::{
    ProductDevCapture, ProductDevCaptureRequest, ProductDevFrame, ProductDevFrameCapture,
    ProductDevFrameFormat, ProductDevFrameStream, FRAME_REQUEST_WAIT, PRODUCT_DEV_FRAMES_PATH,
    PRODUCT_DEV_FRAME_CAPTURE_PATH,
};
pub use host::{
    ProductDevAssetReload, ProductDevHost, ProductDevHostConfig, RunningProductDevHost,
};
pub use log::{
    ProductDevLog, ProductDevLogBatch, ProductDevLogConfig, ProductDevLogDisposition,
    ProductDevLogEvent, ProductDevLogSeverity, ProductDevLogSnapshot, ProductDevLogWriterState,
};
pub use model::{
    runtime_fault_disposition, CanonicalU64, ProductDevBrowserAttachment,
    ProductDevBrowserAttachmentBaseline, ProductDevBrowserConnectionState,
    ProductDevBrowserDiagnosticsReport, ProductDevBrowserDiagnosticsResult,
    ProductDevBrowserHostState, ProductDevBrowserPageDiagnostic,
    ProductDevBrowserPageDiagnosticKind, ProductDevBrowserTerminalDiagnostic,
    ProductDevControlOperation, ProductDevDebugCatalog, ProductDevDebugCommandDescriptor,
    ProductDevDebugCommandParameterDescriptor, ProductDevDebugResult, ProductDevFaultDisposition,
    ProductDevInputBatch, ProductDevInputResult, ProductDevLifecycleOperation,
    ProductDevOperationKind, ProductDevOperationResult, ProductDevRuntime,
    ProductDevRuntimeBinding, ProductDevRuntimeFault, ProductDevRuntimeMode,
    ProductDevRuntimeOutput, ProductDevRuntimeReadout, ProductDevRuntimeReceipt,
    ProductDevRuntimeScheduleState, ProductDevRuntimeState, ProductDevTelemetrySnapshot,
    ProductDevTimelineCompletion, ProductDevTimelineCompletionResult, ProductDevUpdateAttribution,
    ProductDevUpdateAttributionSnapshot, PRODUCT_DEV_HOST_ARTIFACT, PRODUCT_DEV_RUNTIME_BASE_PATH,
};
pub use runtime_publication::{
    RuntimePublication, RuntimePublicationError, RuntimePublicationFrontier, RuntimeReceipt,
};
pub use scheduler::advance_realtime_with_input_and_publish;
pub use session::ProductDevOperationOwner;

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
