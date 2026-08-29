//! Instance-owned, host-neutral Runtime Composition timeline operations.
//!
//! Product Model timelines are static descriptive templates. This crate
//! compiles those templates after closed capability linkage, then owns one
//! bounded queue and completion-ticket lane per explicit runtime instance.
//! It emits immutable operation/completion data for downstream mutation
//! owners; it never invokes capabilities, stores callbacks, reads a clock, or
//! owns product state or persistence.

#![forbid(unsafe_code)]

mod compile;
mod error;
mod inspection;
mod model;
mod runtime;

pub use compile::{
    CompiledTimeline, CompiledTimelineCapability, CompiledTimelineCatalog, CompiledTimelineStep,
    MAX_COMPILED_TIMELINES, MAX_COMPILED_TIMELINE_STEPS, MAX_RUNTIME_TIMELINE_INSPECTION_BYTES,
};
pub use error::RuntimeTimelineError;
pub use inspection::{
    RuntimeTimelineInspection, TimelineCapabilityInspection, TimelineInspection,
    TimelineStepInspection,
};
/// Compatibility vocabulary for callers that name the closed completion
/// source dimension directly.
pub use model::RuntimeSourceKind as CompletionSourceKind;
pub use model::{
    RuntimeOpaqueData, RuntimeProvenance, RuntimeSourceKind, RuntimeTimelineBinding,
    RuntimeTimelineDataError, TimelineCompletionEnvelope, TimelineCompletionOutcome,
    TimelineCompletionTicketId, TimelineInsertionSequence, TimelineOperationIdentity,
    TimelineOperationReplacement, TimelineOperationRevision, TimelineOperationSpec,
    TimelineRecurrence, MAX_RECURRENCE_OCCURRENCES, MAX_RUNTIME_CORRELATION_BYTES,
    MAX_RUNTIME_OPAQUE_DATA_BYTES, MAX_RUNTIME_OPAQUE_DATA_DEPTH, MAX_TIMELINE_COMPLETION_TICKETS,
    MAX_TIMELINE_OPERATIONS, MAX_TIMELINE_RELEASE_PREFIX, MAX_TIMELINE_SNAPSHOT_ITEMS,
};
pub use runtime::{
    ReleasedCompletionStatus, ReleasedTimelineCompletion, ReleasedTimelineEvent,
    ReleasedTimelineOperation, RuntimeTimeline, RuntimeTimelineReadout,
    TimelineCompletionAdmission, TimelineCompletionSpec, TimelineCompletionTicket,
    TimelineOperationReceipt, TimelineOperationSnapshot, TimelineRebindReceipt, TimelineRelease,
    TimelineSnapshot, TimelineTicketSnapshot, TimelineTicketSnapshotStatus,
};
