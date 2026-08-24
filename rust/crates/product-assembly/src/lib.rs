//! Deterministic, product-relative Product Assembly planning and publication.
//!
//! This crate is the filesystem-facing edge of the Product Model campaign. It
//! reads one validated product root, admits the compiled composition through
//! [`product_model`], copies only the declared source/content/artifact closure
//! into generated lanes, and publishes every generated output as one staged
//! transaction. Receipts intentionally have no independent schema or release
//! version: compatibility follows the actual bytes and current fields.
//!
//! The planner never stores an absolute path, follows a symlink, reaches into
//! a sibling checkout at runtime, retains a callback, or creates a dynamic
//! product registry. Generated Rust source links the optional product kernel
//! module by a relative `#[path]` and admits the compiled composition with a
//! relative `include_bytes!`.

#![forbid(unsafe_code)]

mod error;
mod filesystem;
mod publish;
mod receipt;
mod source;

#[cfg(test)]
mod tests;

pub use error::{AssemblyDiagnostic, ProductAssemblyError};
pub use publish::{
    publish_outputs, verify_outputs, AssemblyPublication, PublicationFile, PublicationOutput,
    PublicationOutputKind, PublishedOutputs,
};
pub use receipt::{
    decode_assembly_receipt, AssemblyClosureEntry, AssemblyEntryKind, AssemblyReceipt,
    AssemblyReceiptJson, GeneratedAssemblyFile, PRODUCT_ASSEMBLY_ARTIFACT,
};
pub use source::{
    plan_product_assembly, plan_product_assembly_with_kernel_capabilities,
    verify_existing_product_assembly, verify_existing_product_assembly_with_kernel_capabilities,
    verify_product_assembly, verify_product_assembly_with_kernel_capabilities,
    AssemblyGenerationInputs, AssemblyPlan, AssemblySourcePlan, BrowserBundleInputs,
};

/// Maximum number of files admitted from one source/content/artifact lane.
pub const MAX_ASSEMBLY_FILES: usize = 8_192;
/// Maximum byte length of one copied file.
pub const MAX_ASSEMBLY_FILE_BYTES: usize = 16 * 1024 * 1024;
/// Maximum total bytes retained by one plan and staged publication.
pub const MAX_ASSEMBLY_TOTAL_BYTES: usize = 64 * 1024 * 1024;
/// Maximum lexical depth accepted while walking a product lane.
pub const MAX_ASSEMBLY_PATH_DEPTH: usize = 64;
/// Maximum byte length of a generated source-plan file.
pub const MAX_GENERATED_SOURCE_BYTES: usize = 256 * 1024;
/// Maximum number of generated assembly files before publication.
pub const MAX_GENERATED_FILES: usize = 16_384;
