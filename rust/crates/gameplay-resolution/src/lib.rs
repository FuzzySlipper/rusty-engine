//! Host-neutral lifecycle for downstream-owned gameplay resolutions.
//!
//! The crate owns bounded structural traversal, deterministic phase ordering,
//! correlation, staged transaction control, and generic receipts. Every
//! gameplay meaning and every authoritative state owner remains downstream.

#![forbid(unsafe_code)]

mod identity;
mod limits;
mod policy;
mod program;
mod receipt;
mod resolver;
mod trace;
mod transaction;

pub use identity::{CorrelationId, ResolutionId, ResolutionIdentity, ResolutionIdentityError};
pub use limits::{ResolutionLimitError, ResolutionLimits};
pub use policy::{
    ChildResolution, PolicyFailure, PolicyOutcome, PolicyPlan, PolicyProgram, PolicyResult,
    ResolutionPlan, ResolutionPolicy,
};
pub use program::Program;
pub use receipt::{
    AttemptReceipt, AttemptStatus, CommitStatus, ResolutionMode, ResolutionReceipt,
    ResolutionRequest,
};
pub use resolver::StandardResolver;
pub use trace::{ResolutionPhase, ResolutionTraceKind, ResolutionTraceRecord, ResolutionTraceSink};
pub use transaction::ResolutionTransaction;
