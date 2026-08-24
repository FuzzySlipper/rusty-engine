//! Engine-owned, loopback-only browser development host for generated Products.
//!
//! This is deliberately a small HTTP/1.1 implementation rather than a product
//! network service. It binds only `127.0.0.1`, accepts a closed same-origin
//! route vocabulary, and serves an immutable bundle admitted before startup.
//! The generated Product Assembly remains the concrete runtime owner: every
//! operation is a direct, serialized trait call and returns its own bounded
//! output batch. The host stores neither gameplay callbacks nor product state.
//!
//! The implementation uses `std::net` instead of a general HTTP framework so
//! its trust boundary is auditable in one module. It supports only HTTP/1.1
//! request lines, fixed `Content-Length` POST bodies, and one SSE response;
//! it rejects connection reuse, chunked transfer, CORS, cookies, arbitrary
//! methods, arbitrary routes, and non-loopback binds.

#![forbid(unsafe_code)]

mod bundle;
mod error;
mod host;
mod model;

pub use bundle::{ProductDevBundle, ProductDevBundleEntry, PRODUCT_DEV_INDEX_PATH};
pub use error::{ProductDevHostError, ProductDevRuntimeError};
pub use host::{ProductDevHost, ProductDevHostConfig, RunningProductDevHost};
pub use model::{
    CanonicalU64, ProductDevInputBatch, ProductDevInputResult, ProductDevLifecycleOperation,
    ProductDevOperationKind, ProductDevOperationResult, ProductDevRuntime,
    ProductDevRuntimeBinding, ProductDevRuntimeFault, ProductDevRuntimeMode,
    ProductDevRuntimeOutput, ProductDevRuntimeReadout, ProductDevRuntimeReceipt,
    ProductDevRuntimeState, ProductDevTimelineCompletion, ProductDevTimelineCompletionResult,
    PRODUCT_DEV_HOST_ARTIFACT, PRODUCT_DEV_RUNTIME_BASE_PATH,
};

/// Upper bound for one HTTP request header block, including its terminator.
pub const MAX_REQUEST_HEADER_BYTES: usize = 16 * 1024;
/// Upper bound for one JSON request body or emitted JSON response body.
pub const MAX_REQUEST_BODY_BYTES: usize = 512 * 1024;
/// Upper bound for one immutable bundle resource.
pub const MAX_BUNDLE_RESOURCE_BYTES: usize = 16 * 1024 * 1024;
/// Upper bound for all immutable bundle resource bytes.
pub const MAX_BUNDLE_BYTES: usize = 64 * 1024 * 1024;
/// Upper bound for output items retained for reconnecting SSE clients.
pub const MAX_OUTPUT_QUEUE_ITEMS: usize = 256;
/// Upper bound for one output event after JSON encoding.
pub const MAX_OUTPUT_EVENT_BYTES: usize = 256 * 1024;
/// Upper bound for live accepted TCP connections, including SSE clients.
pub const MAX_CONNECTIONS: usize = 32;
/// Upper bound for simultaneous SSE subscribers.
pub const MAX_SSE_SUBSCRIBERS: usize = 8;
