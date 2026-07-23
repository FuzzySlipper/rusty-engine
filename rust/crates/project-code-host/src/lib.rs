//! Trusted external Game Project host over the shared Rust world kernel.
//!
//! Project code owns game-specific decisions and state. Rust supplies bounded
//! views, durable scheduling, structural state validation, and one atomic world
//! command application per invocation wave.

#![forbid(unsafe_code)]

mod runtime;
mod snapshot;
mod types;

pub use runtime::{ProjectCodeRuntime, ProjectHostError};
pub use snapshot::{
    decode_project_snapshot, encode_project_snapshot, ProjectHostSnapshot,
    PROJECT_HOST_SNAPSHOT_SCHEMA_VERSION,
};
pub use types::{
    BehaviorBinding, HostCollisionView, HostEngineFact, HostEntityView, HostInputEvent,
    HostRenderableView, ProjectApplyReceipt, ProjectDecision, ProjectDecisionBatch, ProjectDoorIds,
    ProjectFact, ProjectFactJournalEntry, ProjectInvocation, ProjectInvocationWave,
    ProjectRuntimeReadout, ProjectScheduleRequest, ProjectStateRecord, ProjectStateUpdate,
    ProjectWorldCommand, ScheduledProjectMessage, PROJECT_HOST_SCHEMA_VERSION,
};
