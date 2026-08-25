//! Source-linked downstream Product Kernel contracts.
//!
//! This crate supplies the narrow provider boundary for product-specific
//! systems and operations. A downstream product declares its closed owners
//! once with [`product_kernel_declaration!`]. The declaration expands to an
//! owner enum, an immutable Product Model descriptor catalog, and typed
//! contract metadata used by [`ProductAssembly::link`] before a runtime
//! lifecycle exists.
//!
//! The crate intentionally contains no handler table, trait-object storage,
//! registry, dynamic loader, scheduler, mutation executor, or generic invoke
//! operation. A downstream product keeps its concrete functions and closed
//! snapshot/request/result/error types and may use the generated owner enum as
//! an ordinary matchable value.

#![forbid(unsafe_code)]

mod assembly;
mod context;
mod declaration;
mod execution;
mod render;

pub use assembly::{
    validate_declaration, DeclarationError, LinkedProductKernelSelection, ProductAssembly,
    ProductAssemblyError,
};
pub use context::{
    ProductKernelContextError, ProductOperationContext, ProductProjectionContext,
    ProductSystemContext,
};
pub use declaration::{
    ProductKernelCapabilityContract, ProductKernelCapabilityEntry, ProductKernelDeclaration,
    ProductKernelMigrationContract, ProductKernelMigrationDescriptor, ProductKernelOwner,
    ProductKernelRuntimeDefinition, ProductKernelRuntimeMutationDescriptor,
    ProductKernelRuntimeSelection, ProductKernelSchemaContract, ProductKernelSchemaDescriptor,
    ProductKernelSelection, ProductKernelStandardCapabilityBindError, ProductRuntimeResource,
    ProductRuntimeResources, MAX_PRODUCT_KERNEL_CONTRACT_TEXT_BYTES,
    MAX_PRODUCT_KERNEL_IDENTITY_BYTES,
};
pub use execution::{
    render_product_kernel_execution_arms, validate_product_kernel_execution,
    validate_product_kernel_execution_declaration, ProductKernelExecutionError,
    ProductKernelExecutionLink, ProductKernelOperationExecutor, ProductKernelProjectionExecutor,
    ProductKernelRuntimeAdapter, ProductKernelSystemExecutor,
};
pub use product_model;
pub use product_model::{
    CapabilityAccess, CapabilityAvailability, CapabilityBudget, CapabilityKind, CapabilityMetadata,
    CapabilityProvenance, CapabilityUse, CapabilityUses, ProductKernelCapabilityDescriptor,
};
pub use render::{render_contract_json, render_contract_typescript};
#[cfg(test)]
pub(crate) use render::{render_contract_json_unchecked, render_contract_typescript_unchecked};
pub use runtime_standard_capabilities;

#[cfg(test)]
mod tests;
