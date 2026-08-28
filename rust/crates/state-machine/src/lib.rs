//! Small generic state-machine authority for entity-owned gameplay state.
//!
//! This crate owns allowed-state transitions and optimistic instance revisions.
//! It does not own animation sampling, scheduling, replay, or policy routing.

#![forbid(unsafe_code)]

mod model;
mod store;

pub use model::{
    apply_detached_transition, apply_transition_to_instance, DetachedMachineInstance,
    DetachedTransitionApplied, DetachedTransitionRequest, MachineInstance, StateMachineError,
    StateMachineFact, StateMachineSpec, TransitionApplied, TransitionRequest,
};
pub use store::StateMachineStore;

/// Maximum number of states admitted by a generated detached definition.
pub const MAX_DETACHED_DEFINITION_STATES: usize = 256;

/// Maximum number of directed transition edges admitted by a generated detached definition.
pub const MAX_DETACHED_DEFINITION_TRANSITIONS: usize = 1_024;
