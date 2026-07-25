//! Small generic state-machine authority for entity-owned gameplay state.
//!
//! This crate owns allowed-state transitions and optimistic instance revisions.
//! It does not own animation sampling, scheduling, replay, or policy routing.

#![forbid(unsafe_code)]

mod model;
mod store;

pub use model::{
    apply_transition_to_instance, MachineInstance, StateMachineError, StateMachineFact,
    StateMachineSpec, TransitionApplied, TransitionRequest,
};
pub use store::StateMachineStore;
