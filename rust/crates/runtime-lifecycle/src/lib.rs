//! Instance-owned, host-neutral product runtime lifecycle admission.
//!
//! This crate turns explicit lifecycle configuration into simulation and
//! presentation admission plans. Format adapters belong at their owning host
//! edge. The lifecycle does
//! not read a clock, execute a schedule, invoke callbacks, own input, mutate
//! gameplay state, or render. Callers supply monotonic time for realtime
//! products and carry the resulting tokens into their own named owners.

#![forbid(unsafe_code)]

mod lifecycle;
mod model;

pub use lifecycle::RuntimeLifecycle;
pub use model::{
    validate_runtime_identity, GameplayRate, GameplayTime, HostMonotonicTime, LifecycleOperation,
    LifecycleReceipt, PresentationAdmission, PresentationToken, RealtimeAdvance,
    RuntimeControlOperation, RuntimeControlRevision, RuntimeFault, RuntimeGeneration,
    RuntimeIdentityError, RuntimeInstanceId, RuntimeLifecycleConfig, RuntimeLifecycleConfigError,
    RuntimeLifecycleError, RuntimeLifecycleReadout, RuntimePhase, RuntimePhasePlan,
    RuntimePhaseToken, RuntimeState, SimulationAdmission, SimulationStep, SimulationStepAdmission,
    SimulationToken, GAMEPLAY_RATE_REALTIME_PPM, MAX_RUNTIME_IDENTITY_BYTES,
};
