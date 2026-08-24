use std::fmt;

use product_model::SchedulePhase;
use runtime_lifecycle::{RuntimeGeneration, RuntimeInstanceId, RuntimeLifecycleError};

/// Rejection from schedule compilation, binding, or execution.
#[derive(Debug, PartialEq, Eq)]
pub enum RuntimeScheduleError<E = std::convert::Infallible> {
    /// Product Model linkage or structural data was not usable by the runtime
    /// schedule. The linked artifact remains owned by Product Model.
    InvalidComposition(String),
    DuplicateSystemId(String),
    UnknownCapability(String),
    CapabilityUnavailable(String),
    CapabilityUseMismatch {
        system: String,
        phase: SchedulePhase,
        kind: String,
    },
    InvalidCadence {
        system: String,
        every_steps: u32,
        offset_steps: u32,
    },
    UnknownDependency {
        system: String,
        dependency: String,
    },
    CrossPhaseDependency {
        system: String,
        dependency: String,
    },
    SelfDependency(String),
    DuplicateDependency {
        system: String,
        dependency: String,
    },
    DependencyCycle {
        phase: SchedulePhase,
    },
    PlacementConflict {
        phase: SchedulePhase,
        system: String,
        dependency: String,
    },
    AccessConflict {
        phase: SchedulePhase,
        first: String,
        second: String,
        resource: String,
    },
    PayloadTooLarge {
        system: String,
        actual: usize,
        maximum: usize,
    },
    BoundsExceeded(&'static str),
    Lifecycle(RuntimeLifecycleError),
    LifecycleNotRunning,
    LifecycleAlreadyAdvanced {
        admitted_steps: u64,
    },
    LifecycleBindingMismatch,
    RebindForeignInstance {
        expected: RuntimeInstanceId,
        received: RuntimeInstanceId,
    },
    RebindRegression {
        expected_generation: RuntimeGeneration,
        received_generation: RuntimeGeneration,
        expected_control_revision: u64,
        received_control_revision: u64,
    },
    RebindAdmissionRegression {
        expected_next_step: Option<u64>,
        admitted_steps: u64,
    },
    InvalidatedAdmissionOverflow,
    RebindActiveStep {
        step: u64,
    },
    Disposed,
    WrongPhase {
        expected: SchedulePhase,
        received: SchedulePhase,
    },
    PhaseOutOfOrder {
        expected: SchedulePhase,
        received: SchedulePhase,
    },
    StepOutOfOrder {
        expected: Option<u64>,
        received: u64,
    },
    StepMismatch {
        expected: u64,
        received: u64,
    },
    Dispatch(E),
    InspectionEncode(String),
}

impl<E> From<RuntimeLifecycleError> for RuntimeScheduleError<E> {
    fn from(error: RuntimeLifecycleError) -> Self {
        Self::Lifecycle(error)
    }
}

impl<E: fmt::Display> fmt::Display for RuntimeScheduleError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidComposition(message) => write!(formatter, "invalid schedule composition: {message}"),
            Self::DuplicateSystemId(id) => write!(formatter, "schedule system id `{id}` is duplicated"),
            Self::UnknownCapability(id) => write!(formatter, "schedule capability `{id}` is not linked"),
            Self::CapabilityUnavailable(target) => write!(formatter, "schedule capability `{target}` is unavailable"),
            Self::CapabilityUseMismatch { system, phase, kind } => write!(formatter, "system `{system}` in {phase:?} has incompatible capability kind `{kind}`"),
            Self::InvalidCadence { system, every_steps, offset_steps } => write!(formatter, "system `{system}` has invalid cadence everySteps={every_steps}, offsetSteps={offset_steps}"),
            Self::UnknownDependency { system, dependency } => write!(formatter, "system `{system}` depends on unknown system `{dependency}`"),
            Self::CrossPhaseDependency { system, dependency } => write!(formatter, "system `{system}` depends on a system in another phase: `{dependency}`"),
            Self::SelfDependency(system) => write!(formatter, "system `{system}` depends on itself"),
            Self::DuplicateDependency { system, dependency } => write!(formatter, "system `{system}` repeats dependency `{dependency}`"),
            Self::DependencyCycle { phase } => write!(formatter, "schedule dependency cycle in {phase:?}"),
            Self::PlacementConflict { phase, system, dependency } => write!(formatter, "schedule placement in {phase:?} conflicts with `{system}` after `{dependency}`"),
            Self::AccessConflict { phase, first, second, resource } => write!(formatter, "unordered access conflict in {phase:?} between `{first}` and `{second}` on `{resource}`"),
            Self::PayloadTooLarge { system, actual, maximum } => write!(formatter, "system `{system}` payload is {actual} bytes, over the {maximum}-byte budget"),
            Self::BoundsExceeded(name) => write!(formatter, "schedule bound exceeded: {name}"),
            Self::Lifecycle(error) => error.fmt(formatter),
            Self::LifecycleNotRunning => write!(formatter, "schedule must bind to a running lifecycle"),
            Self::LifecycleAlreadyAdvanced { admitted_steps } => write!(formatter, "schedule must bind before simulation admission; lifecycle has already admitted {admitted_steps} step(s)"),
            Self::LifecycleBindingMismatch => write!(formatter, "schedule token does not belong to its bound lifecycle"),
            Self::RebindForeignInstance { expected, received } => write!(formatter, "schedule rebind received foreign instance {:?}, expected {:?}", received.value(), expected.value()),
            Self::RebindRegression { expected_generation, received_generation, expected_control_revision, received_control_revision } => write!(formatter, "schedule rebind regressed binding: expected generation/revision {}/{} or newer, received {}/{}", expected_generation.value(), expected_control_revision, received_generation.value(), received_control_revision),
            Self::RebindAdmissionRegression { expected_next_step, admitted_steps } => write!(formatter, "schedule rebind regressed admitted-step cursor: expected next step {expected_next_step:?}, lifecycle has admitted {admitted_steps} step(s)"),
            Self::InvalidatedAdmissionOverflow => write!(formatter, "schedule rebind exhausted its invalidated-admission count"),
            Self::RebindActiveStep { step } => write!(formatter, "schedule cannot rebind while phase progression for step {step} is active"),
            Self::Disposed => write!(formatter, "runtime schedule is disposed"),
            Self::WrongPhase { expected, received } => write!(formatter, "schedule phase mismatch: expected {expected:?}, received {received:?}"),
            Self::PhaseOutOfOrder { expected, received } => write!(formatter, "schedule phase out of order: expected {expected:?}, received {received:?}"),
            Self::StepOutOfOrder { expected, received } => write!(formatter, "schedule step out of order: expected {expected:?}, received {received}"),
            Self::StepMismatch { expected, received } => write!(formatter, "schedule step mismatch: expected {expected}, received {received}"),
            Self::Dispatch(error) => write!(formatter, "schedule dispatcher failed: {error}"),
            Self::InspectionEncode(error) => write!(formatter, "schedule inspection encoding failed: {error}"),
        }
    }
}

impl<E: fmt::Debug + fmt::Display> std::error::Error for RuntimeScheduleError<E> {}
