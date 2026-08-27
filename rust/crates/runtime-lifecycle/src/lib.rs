//! Instance-owned, host-neutral product runtime lifecycle admission.
//!
//! This crate turns a validated Product Manifest lifecycle selection into
//! explicit simulation and presentation admission plans. It does not read a
//! clock, execute a schedule, invoke callbacks, own input, mutate gameplay
//! state, or render. Callers supply monotonic time for realtime products and
//! carry the resulting tokens into their own named owners.

#![forbid(unsafe_code)]

mod lifecycle;
mod model;

pub use lifecycle::RuntimeLifecycle;
pub use model::{
    ExternalStep, HostMonotonicTime, LifecycleOperation, LifecycleReceipt, PresentationAdmission,
    PresentationToken, RealtimeAdvance, RealtimeLifecycleConfig, RuntimeControlOperation,
    RuntimeControlRevision, RuntimeFault, RuntimeGeneration, RuntimeInstanceId,
    RuntimeLifecycleConfig, RuntimeLifecycleConfigError, RuntimeLifecycleError,
    RuntimeLifecycleReadout, RuntimeMode, RuntimePhase, RuntimePhasePlan, RuntimePhaseToken,
    RuntimeState, SimulationAdmission, SimulationStep, SimulationStepAdmission, SimulationToken,
};
