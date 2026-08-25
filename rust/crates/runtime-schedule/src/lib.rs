//! Instance-owned execution of a linked Product Model schedule.
//!
//! `product-model` admits and links the closed authored vocabulary. This
//! crate resolves the five phase composition into a deterministic invocation
//! order and validates the additional execution facts (placement, ordering,
//! access ambiguity, and cadence) before a runtime instance exists. A bound
//! [`RuntimeSchedule`] only validates caller-supplied lifecycle tokens and
//! presents immutable invocation data to a dispatcher supplied for that call.
//! It never stores callbacks, discovers services, owns a clock, or mutates
//! product state.

#![forbid(unsafe_code)]

mod compile;
mod error;
mod inspection;
mod runtime;

pub use compile::{
    CompiledPhase, CompiledRuntimeSchedule, CompiledSystem, StandardAnchorStatus,
    MAX_RUNTIME_SCHEDULE_INSPECTION_BYTES, MAX_RUNTIME_SCHEDULE_SYSTEMS,
};
pub use error::RuntimeScheduleError;
pub use inspection::{
    CadenceInspection, CapabilityInspection, PhaseInspection, ScheduleInspection,
    ScheduleOrderItem, SystemInspection,
};
pub use runtime::{
    runtime_phase_for_schedule, RuntimeSchedule, RuntimeScheduleReadout, ScheduleDispatcher,
    SchedulePhaseReceipt, ScheduleSystemInvocation,
};
