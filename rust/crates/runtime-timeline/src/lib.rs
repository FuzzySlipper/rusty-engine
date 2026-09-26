//! Host-neutral completion data transported to the C# product callback.
//!
//! Scheduling and ticket ownership belong to the product. This crate does not
//! retain a queue, invoke work, or own a clock or product state.

#![forbid(unsafe_code)]

mod model;

pub use model::{
    RuntimeOpaqueData, RuntimeProvenance, RuntimeTimelineBinding, RuntimeTimelineDataError,
    TimelineCompletionEnvelope, TimelineCompletionOutcome, TimelineCompletionTicketId,
    MAX_RUNTIME_CORRELATION_BYTES,
};
