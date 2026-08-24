//! Host-neutral Product Runtime Composition.
//!
//! This crate owns one explicit lifecycle and the five instance-owned runtime
//! lanes.  It supplies ordering and token plumbing only: product state,
//! meaning, static Product Kernel dispatch, mutation authority, and product
//! projections remain with the adapter supplied by the downstream product.
//! There is no clock read, host object, callback registry, dynamic dispatch,
//! or persistence policy here.

#![forbid(unsafe_code)]

mod adapter;
mod error;
mod root;

pub use adapter::{
    ProductRuntimeAdapter, ProductRuntimeOutputError, ProductRuntimeOutputs, ProductRuntimeUi,
    MAX_PRODUCT_RUNTIME_UI_OUTPUTS,
};
pub use error::{RuntimeCompositionBindError, RuntimeCompositionError};
pub use root::{
    MutationStepReceipt, ProductRuntimeStep, RuntimeComposition, RuntimeCompositionInputs,
    RuntimeCompositionStep,
};

pub use product_kernel;
pub use render_model;
pub use render_presentation;
pub use runtime_input;
pub use runtime_lifecycle;
pub use runtime_mutation;
pub use runtime_schedule;
pub use runtime_timeline;
pub use runtime_ui;
