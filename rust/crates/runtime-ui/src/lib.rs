//! The UI projection envelope: one product-owned JSON value with its stream,
//! contract, sequence and runtime binding, in the wire shape the browser host
//! realizes as DOM. It does not render, schedule or retain product state.

#![forbid(unsafe_code)]

mod model;

pub use model::{
    RuntimeUiProjectionEnvelope, RuntimeUiProjectionError, RuntimeUiProjectionWire,
    RuntimeUiRuntimeBinding, RuntimeUiRuntimeWire, MAX_RUNTIME_UI_PROJECTION_SAFE_INTEGER,
    RUNTIME_UI_PROJECTION_ARTIFACT,
};

#[cfg(test)]
mod tests;
