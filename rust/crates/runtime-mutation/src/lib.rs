//! Instance-owned, fail-atomic Runtime Composition mutation.
//!
//! Product Model remains semantic-neutral: authored capability bindings are
//! linked before runtime, while an immutable Product Assembly supplies the
//! explicit operation selection for this lane. A caller supplies a planner
//! for each admitted Mutation token. The planner receives only an immutable
//! authority view and a resolved, bounded operation batch; it returns an owned
//! candidate and named-owner evidence. After every Result-producing check,
//! this crate performs one assignment into the caller-owned authority bundle.
//!
//! The atomicity boundary is deliberately narrow: one nonempty admitted batch,
//! one in-memory `MutationAuthority` value, and one publication assignment.
//! Durable persistence, rendering, projection, notifications, and owners that
//! cannot produce an owned candidate remain outside this transaction. The
//! guarantee covers ordinary Result failures under a trusted Product Assembly;
//! panic, allocation failure, interior mutation, and destructor behavior are
//! explicitly outside it.

#![forbid(unsafe_code)]

mod compile;
mod error;
mod inspection;
mod model;
mod runtime;

pub use compile::{
    CompiledMutationCapability, CompiledMutationCatalog, MutationCapabilityDescriptor,
    MAX_COMPILED_MUTATION_CAPABILITIES, MAX_RUNTIME_MUTATION_INSPECTION_BYTES,
};
pub use error::RuntimeMutationError;
pub use inspection::{MutationCapabilityInspection, RuntimeMutationInspection};
pub use model::{
    MutationBatch, MutationBatchId, MutationCatalogIdentity, MutationCausation, MutationDataError,
    MutationFingerprint, MutationOperation, MutationOperationId, MutationOwnerEvidence,
    MutationProvenance, MutationResolvedBatch, MutationResolvedOperation, MutationStage,
    MAX_MUTATION_BATCH_ID_BYTES, MAX_MUTATION_BATCH_OPERATIONS, MAX_MUTATION_CAUSATION_BYTES,
    MAX_MUTATION_PAYLOAD_BYTES, MAX_MUTATION_PROVENANCE_BYTES, MAX_MUTATION_RECEIPTS,
};
pub use runtime::{
    MutationAuthority, MutationPlanner, MutationReceipt, RuntimeMutation, RuntimeMutationBinding,
    RuntimeMutationReadout,
};
