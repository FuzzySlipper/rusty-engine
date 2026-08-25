//! Host-neutral downstream UI projection transport.
//!
//! `runtime-ui` owns the narrow boundary between a typed, downstream-owned
//! projection DTO and a transport envelope that an application host may later
//! realize as DOM or another UI surface. It does not render, schedule, read a
//! clock, invoke callbacks, inspect a browser, or retain product state.
//!
//! A caller constructs a [`product_kernel::ProductProjectionContext`] with the
//! exact `RuntimePhase::Projection` token, runs an ordinary typed function to
//! produce an owned DTO, and submits that DTO here. The lane copies the DTO
//! into JSON before retaining or emitting it, so the resulting envelope cannot
//! alias mutable source data.

#![forbid(unsafe_code)]

mod channel;
mod model;

pub use channel::RuntimeUiProjection;
pub use model::{
    decode_runtime_ui_projection_json, encode_runtime_ui_projection_json,
    RuntimeUiProjectionEnvelope, RuntimeUiProjectionError, RuntimeUiProjectionReadout,
    RuntimeUiRuntimeBinding, MAX_RUNTIME_UI_PROJECTION_SAFE_INTEGER,
    MAX_RUNTIME_UI_PROJECTION_STREAMS, MAX_RUNTIME_UI_PROJECTION_VALUE_ARRAY_LENGTH,
    MAX_RUNTIME_UI_PROJECTION_VALUE_DEPTH, MAX_RUNTIME_UI_PROJECTION_VALUE_JSON_BYTES,
    MAX_RUNTIME_UI_PROJECTION_VALUE_NODES, MAX_RUNTIME_UI_PROJECTION_VALUE_OBJECT_KEYS,
    MAX_RUNTIME_UI_PROJECTION_VALUE_STRING_BYTES, MAX_RUNTIME_UI_PROJECTION_WIRE_BYTES,
    RUNTIME_UI_PROJECTION_ARTIFACT,
};

#[cfg(test)]
mod tests;
