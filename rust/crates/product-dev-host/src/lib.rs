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
mod error;
mod host;
mod log;
mod model;
mod session;

pub use bundle::{
    product_dev_renderer_preload_entries, ProductDevBundle, ProductDevBundleEntry,
    ProductDevRendererResource, ProductDevRendererResourceKind, PRODUCT_DEV_INDEX_PATH,
    PRODUCT_DEV_RENDERER_PRELOAD_PATH,
};
pub use error::{
    ProductDevHostError, ProductDevInvalidatedScope, ProductDevMutationCertainty,
    ProductDevNextAction, ProductDevRuntimeError, ProductDevRuntimeRecovery,
};
pub use host::{ProductDevHost, ProductDevHostConfig, RunningProductDevHost};
pub use log::{
    ProductDevLog, ProductDevLogBatch, ProductDevLogConfig, ProductDevLogDisposition,
    ProductDevLogEvent, ProductDevLogSeverity, ProductDevLogSnapshot, ProductDevLogWriterState,
};
pub use model::{
    runtime_fault_disposition, CanonicalU64, ProductDevAnimationCueDefinition,
    ProductDevAnimationCueSignalDomain, ProductDevAnimationFeedback,
    ProductDevAnimationFeedbackFact, ProductDevAnimationFeedbackResult,
    ProductDevAudioCompletionSource, ProductDevAudioFeedback, ProductDevAudioFeedbackFact,
    ProductDevAudioFeedbackResult, ProductDevBrowserConnectionState,
    ProductDevBrowserDiagnosticsReport, ProductDevBrowserDiagnosticsResult,
    ProductDevBrowserHostState, ProductDevBrowserPageDiagnostic,
    ProductDevBrowserPageDiagnosticKind, ProductDevBrowserTerminalDiagnostic,
    ProductDevControlOperation, ProductDevDebugCatalog, ProductDevDebugCommandDescriptor,
    ProductDevDebugCommandParameterDescriptor, ProductDevDebugResult, ProductDevFaultDisposition,
    ProductDevGhostPlateFallbackReason, ProductDevGhostPlateFeedback,
    ProductDevGhostPlateFeedbackFact, ProductDevGhostPlateFeedbackResult, ProductDevInputBatch,
    ProductDevInputResult, ProductDevLifecycleOperation, ProductDevOperationKind,
    ProductDevOperationResult, ProductDevRendererDiagnosticsFeedback,
    ProductDevRendererDiagnosticsFeedbackResult, ProductDevRuntime, ProductDevRuntimeBinding,
    ProductDevRuntimeFault, ProductDevRuntimeMode, ProductDevRuntimeOutput,
    ProductDevRuntimeReadout, ProductDevRuntimeReceipt, ProductDevRuntimeScheduleState,
    ProductDevRuntimeState, ProductDevTelemetrySnapshot, ProductDevTimelineCompletion,
    ProductDevTimelineCompletionResult, ProductDevUpdateAttribution,
    ProductDevUpdateAttributionSnapshot, PRODUCT_DEV_HOST_ARTIFACT, PRODUCT_DEV_RUNTIME_BASE_PATH,
};
pub use session::ProductDevOperationOwner;

/// Upper bound for one HTTP request header block, including its terminator.
pub const MAX_REQUEST_HEADER_BYTES: usize = 16 * 1024;
/// Upper bound for one JSON request body or emitted JSON response body.
pub const MAX_REQUEST_BODY_BYTES: usize = 512 * 1024;
/// Upper bound for one immutable bundle resource.
pub const MAX_BUNDLE_RESOURCE_BYTES: usize = 16 * 1024 * 1024;
/// Upper bound for immutable entries admitted to one generated browser bundle.
pub const MAX_BUNDLE_ENTRIES: usize = 4_096;
/// Upper bound for all immutable bundle resource bytes.
pub const MAX_BUNDLE_BYTES: usize = 64 * 1024 * 1024;
/// Upper bound for output items retained for reconnecting SSE clients.
pub const MAX_OUTPUT_QUEUE_ITEMS: usize = 256;
/// Upper bound for one output event after JSON encoding.
pub const MAX_OUTPUT_EVENT_BYTES: usize = 256 * 1024;
/// Upper bound for one complete typed output before bounded SSE fragmentation.
pub const MAX_OUTPUT_AGGREGATE_BYTES: usize = 16 * 1024 * 1024;
/// Payload target for one fragment. The serialized fragment envelope remains
/// below `MAX_OUTPUT_EVENT_BYTES` even when JSON quotes and escapes the slice.
pub const MAX_OUTPUT_FRAGMENT_DATA_BYTES: usize = 96 * 1024;
/// Upper bound for live accepted TCP connections, including SSE clients.
pub const MAX_CONNECTIONS: usize = 32;
/// Upper bound for simultaneous SSE subscribers.
pub const MAX_SSE_SUBSCRIBERS: usize = 8;
