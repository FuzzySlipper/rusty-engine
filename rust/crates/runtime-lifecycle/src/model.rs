use std::fmt;

use product_model::{LifecycleMode, ProductManifest, MAX_REALTIME_CATCH_UP_STEPS, MAX_REALTIME_HZ};

pub(crate) const SCALED_NANOSECONDS_PER_SECOND: u128 = 1_000_000_000;

/// A caller-provided reading from one host-owned monotonic clock.
///
/// The runtime lifecycle deliberately stores only these supplied values. It
/// never reads `Instant`, `SystemTime`, or another ambient clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HostMonotonicTime(u64);

impl HostMonotonicTime {
    pub const fn from_nanoseconds(nanoseconds: u64) -> Self {
        Self(nanoseconds)
    }

    pub const fn nanoseconds(self) -> u64 {
        self.0
    }
}

/// Caller-owned identity for one explicit lifecycle instance.
///
/// The lifecycle never generates or globally registers instance identities.
/// A product must choose a distinct value for each concurrently live instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RuntimeInstanceId(u64);

impl RuntimeInstanceId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

/// A deterministic externally supplied simulation step number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExternalStep(u64);

impl ExternalStep {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

/// Stable identity for one started/restarted runtime instance.
///
/// Pausing and resuming retain this identity; they change the separately
/// tracked [`RuntimeControlRevision`] instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct RuntimeGeneration(pub(crate) u64);

impl RuntimeGeneration {
    pub const ZERO: Self = Self(0);

    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

/// Admission/control epoch inside one [`RuntimeGeneration`].
///
/// This changes across lifecycle discontinuities such as pause/resume or a
/// reported fault, making older admission tokens stale without claiming that
/// the underlying runtime was restarted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct RuntimeControlRevision(pub(crate) u64);

impl RuntimeControlRevision {
    pub const ZERO: Self = Self(0);

    /// Reconstitutes a correlation revision from a lossless host wire value.
    /// Lifecycle transitions remain the only mechanism that advances it.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

/// The one lifecycle family selected for this runtime instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeMode {
    Realtime,
    Demand,
    External,
}

/// Realtime settings for fixed-step admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RealtimeLifecycleConfig {
    fixed_step_hz: u32,
    max_catch_up_steps: u32,
}

impl RealtimeLifecycleConfig {
    /// Creates bounded, allocation-free fixed-step settings.
    pub const fn new(
        fixed_step_hz: u32,
        max_catch_up_steps: u32,
    ) -> Result<Self, RuntimeLifecycleConfigError> {
        if fixed_step_hz == 0 {
            return Err(RuntimeLifecycleConfigError::ZeroFixedStepHz);
        }
        if fixed_step_hz > MAX_REALTIME_HZ {
            return Err(RuntimeLifecycleConfigError::FixedStepHzExceedsManifestMaximum);
        }
        if max_catch_up_steps == 0 {
            return Err(RuntimeLifecycleConfigError::ZeroCatchUpSteps);
        }
        if max_catch_up_steps > MAX_REALTIME_CATCH_UP_STEPS {
            return Err(RuntimeLifecycleConfigError::CatchUpStepsExceedManifestMaximum);
        }
        Ok(Self {
            fixed_step_hz,
            max_catch_up_steps,
        })
    }

    pub const fn fixed_step_hz(self) -> u32 {
        self.fixed_step_hz
    }

    pub const fn max_catch_up_steps(self) -> u32 {
        self.max_catch_up_steps
    }
}

/// The complete lifecycle configuration for one runtime owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeLifecycleConfig {
    Realtime(RealtimeLifecycleConfig),
    Demand,
    External,
}

impl RuntimeLifecycleConfig {
    /// Selects this lifecycle from an already validated Product Manifest.
    ///
    /// This is the only Product Model linkage. The lifecycle neither loads a
    /// manifest nor interprets any other product field.
    pub fn from_product_manifest(
        manifest: &ProductManifest,
    ) -> Result<Self, RuntimeLifecycleConfigError> {
        Ok(match manifest.lifecycle() {
            LifecycleMode::Realtime => {
                let realtime = manifest
                    .realtime()
                    .ok_or(RuntimeLifecycleConfigError::MissingManifestRealtimeSettings)?;
                Self::Realtime(RealtimeLifecycleConfig {
                    fixed_step_hz: realtime.fixed_step_hz(),
                    max_catch_up_steps: realtime.max_catch_up_steps(),
                })
            }
            LifecycleMode::Demand => Self::Demand,
            LifecycleMode::External => Self::External,
        })
    }

    pub const fn mode(self) -> RuntimeMode {
        match self {
            Self::Realtime(_) => RuntimeMode::Realtime,
            Self::Demand => RuntimeMode::Demand,
            Self::External => RuntimeMode::External,
        }
    }
}

/// Configuration rejected before a lifecycle owner exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeLifecycleConfigError {
    MissingManifestRealtimeSettings,
    ZeroFixedStepHz,
    FixedStepHzExceedsManifestMaximum,
    ZeroCatchUpSteps,
    CatchUpStepsExceedManifestMaximum,
}

impl fmt::Display for RuntimeLifecycleConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid runtime lifecycle configuration: {self:?}"
        )
    }
}

impl std::error::Error for RuntimeLifecycleConfigError {}

/// Observable lifecycle state. The lifecycle does not run an executor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeState {
    Created,
    Running,
    Paused,
    Faulted,
    Shutdown,
}

/// A bounded operation recorded by a lifecycle receipt or error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleOperation {
    Start,
    Pause,
    Resume,
    Restart,
    Shutdown,
    ReportFault,
    AdvanceRealtime,
    AdmitDemandStep,
    AdmitExternalStep,
    AdmitPresentation,
    ValidateSimulationToken,
    ValidatePresentationToken,
}

/// A lifecycle transition receipt. It is data only; no owner is invoked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LifecycleReceipt {
    instance_id: RuntimeInstanceId,
    operation: LifecycleOperation,
    state: RuntimeState,
    generation: RuntimeGeneration,
    control_revision: RuntimeControlRevision,
}

impl LifecycleReceipt {
    pub(crate) const fn new(
        instance_id: RuntimeInstanceId,
        operation: LifecycleOperation,
        state: RuntimeState,
        generation: RuntimeGeneration,
        control_revision: RuntimeControlRevision,
    ) -> Self {
        Self {
            instance_id,
            operation,
            state,
            generation,
            control_revision,
        }
    }

    pub const fn instance_id(self) -> RuntimeInstanceId {
        self.instance_id
    }

    pub const fn operation(self) -> LifecycleOperation {
        self.operation
    }

    pub const fn state(self) -> RuntimeState {
        self.state
    }

    pub const fn generation(self) -> RuntimeGeneration {
        self.generation
    }

    pub const fn control_revision(self) -> RuntimeControlRevision {
        self.control_revision
    }
}

/// The reason a caller or the lifecycle put the current run into `Faulted`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeFault {
    /// A named downstream owner failed. Its detailed diagnostics remain with
    /// that owner rather than becoming a generic Engine error payload.
    OwnerReported,
    /// An internal monotonic identifier or diagnostic counter was exhausted.
    CounterExhausted,
}

/// A simulation step sequence number admitted by this lifecycle instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SimulationStep(pub(crate) u64);

impl SimulationStep {
    /// Reconstitutes a deterministic step from a validated typed snapshot or
    /// another bounded runtime owner. Lifecycle admission remains the source
    /// of new live steps; this constructor does not admit work.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

/// Correlation evidence for an admitted simulation step.
///
/// Validating a token is idempotent and does not mark a step complete. The
/// lifecycle has no completion protocol, executor, or mutation authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SimulationToken {
    instance_id: RuntimeInstanceId,
    generation: RuntimeGeneration,
    control_revision: RuntimeControlRevision,
    step: SimulationStep,
}

impl SimulationToken {
    pub const fn instance_id(self) -> RuntimeInstanceId {
        self.instance_id
    }

    pub const fn generation(self) -> RuntimeGeneration {
        self.generation
    }

    pub const fn control_revision(self) -> RuntimeControlRevision {
        self.control_revision
    }

    pub const fn step(self) -> SimulationStep {
        self.step
    }
}

/// A named data handoff point in a downstream-owned simulation step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuntimePhase {
    InputSnapshot,
    Schedule,
    Timeline,
    Mutation,
    Projection,
}

/// Correlation evidence for one named phase of an admitted simulation step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RuntimePhaseToken {
    simulation: SimulationToken,
    phase: RuntimePhase,
}

impl RuntimePhaseToken {
    pub const fn simulation(self) -> SimulationToken {
        self.simulation
    }

    pub const fn phase(self) -> RuntimePhase {
        self.phase
    }
}

/// Explicit downstream data handoff tokens for one simulation step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimePhasePlan {
    input_snapshot: RuntimePhaseToken,
    schedule: RuntimePhaseToken,
    timeline: RuntimePhaseToken,
    mutation: RuntimePhaseToken,
    projection: RuntimePhaseToken,
}

impl RuntimePhasePlan {
    pub(crate) const fn new(simulation: SimulationToken) -> Self {
        Self {
            input_snapshot: RuntimePhaseToken {
                simulation,
                phase: RuntimePhase::InputSnapshot,
            },
            schedule: RuntimePhaseToken {
                simulation,
                phase: RuntimePhase::Schedule,
            },
            timeline: RuntimePhaseToken {
                simulation,
                phase: RuntimePhase::Timeline,
            },
            mutation: RuntimePhaseToken {
                simulation,
                phase: RuntimePhase::Mutation,
            },
            projection: RuntimePhaseToken {
                simulation,
                phase: RuntimePhase::Projection,
            },
        }
    }

    pub const fn input_snapshot(self) -> RuntimePhaseToken {
        self.input_snapshot
    }

    pub const fn schedule(self) -> RuntimePhaseToken {
        self.schedule
    }

    pub const fn timeline(self) -> RuntimePhaseToken {
        self.timeline
    }

    pub const fn mutation(self) -> RuntimePhaseToken {
        self.mutation
    }

    pub const fn projection(self) -> RuntimePhaseToken {
        self.projection
    }
}

/// A single admitted simulation step and its named downstream handoff plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SimulationAdmission {
    instance_id: RuntimeInstanceId,
    generation: RuntimeGeneration,
    control_revision: RuntimeControlRevision,
    first_step: SimulationStep,
    step_count: u32,
}

impl SimulationAdmission {
    pub(crate) const fn new(
        instance_id: RuntimeInstanceId,
        generation: RuntimeGeneration,
        control_revision: RuntimeControlRevision,
        first_step: SimulationStep,
        step_count: u32,
    ) -> Self {
        Self {
            instance_id,
            generation,
            control_revision,
            first_step,
            step_count,
        }
    }

    pub const fn instance_id(self) -> RuntimeInstanceId {
        self.instance_id
    }

    pub const fn generation(self) -> RuntimeGeneration {
        self.generation
    }

    pub const fn control_revision(self) -> RuntimeControlRevision {
        self.control_revision
    }

    pub const fn first_step(self) -> SimulationStep {
        self.first_step
    }

    pub const fn step_count(self) -> u32 {
        self.step_count
    }

    /// Returns the requested admitted step without allocating an execution
    /// queue. `offset` must be less than `step_count`.
    pub fn step_at(self, offset: u32) -> Option<SimulationStepAdmission> {
        if offset >= self.step_count {
            return None;
        }
        let step = SimulationStep(self.first_step.0.checked_add(u64::from(offset))?);
        let token = SimulationToken {
            instance_id: self.instance_id,
            generation: self.generation,
            control_revision: self.control_revision,
            step,
        };
        Some(SimulationStepAdmission {
            token,
            phases: RuntimePhasePlan::new(token),
        })
    }
}

/// One simulation admission plus explicit phase handoffs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SimulationStepAdmission {
    token: SimulationToken,
    phases: RuntimePhasePlan,
}

impl SimulationStepAdmission {
    pub const fn token(self) -> SimulationToken {
        self.token
    }

    pub const fn phases(self) -> RuntimePhasePlan {
        self.phases
    }
}

/// Correlation evidence for one presentation attempt.
///
/// Presentation tokens are independent from simulation admission and remain
/// valid while the simulation lifecycle is paused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PresentationToken {
    instance_id: RuntimeInstanceId,
    generation: RuntimeGeneration,
    control_revision: RuntimeControlRevision,
    sequence: u64,
}

impl PresentationToken {
    pub(crate) const fn new(
        instance_id: RuntimeInstanceId,
        generation: RuntimeGeneration,
        control_revision: RuntimeControlRevision,
        sequence: u64,
    ) -> Self {
        Self {
            instance_id,
            generation,
            control_revision,
            sequence,
        }
    }

    pub const fn instance_id(self) -> RuntimeInstanceId {
        self.instance_id
    }

    pub const fn generation(self) -> RuntimeGeneration {
        self.generation
    }

    pub const fn control_revision(self) -> RuntimeControlRevision {
        self.control_revision
    }

    pub const fn sequence(self) -> u64 {
        self.sequence
    }
}

/// A presentation plan deliberately separate from simulation admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresentationAdmission {
    token: PresentationToken,
}

impl PresentationAdmission {
    pub(crate) const fn new(token: PresentationToken) -> Self {
        Self { token }
    }

    pub const fn token(self) -> PresentationToken {
        self.token
    }
}

/// Result of a supplied realtime clock reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RealtimeAdvance {
    observed_time: HostMonotonicTime,
    simulation: Option<SimulationAdmission>,
    dropped_steps: u128,
    scaled_remainder: u32,
}

impl RealtimeAdvance {
    pub(crate) const fn new(
        observed_time: HostMonotonicTime,
        simulation: Option<SimulationAdmission>,
        dropped_steps: u128,
        scaled_remainder: u32,
    ) -> Self {
        Self {
            observed_time,
            simulation,
            dropped_steps,
            scaled_remainder,
        }
    }

    pub const fn observed_time(self) -> HostMonotonicTime {
        self.observed_time
    }

    pub const fn simulation(self) -> Option<SimulationAdmission> {
        self.simulation
    }

    pub const fn dropped_steps(self) -> u128 {
        self.dropped_steps
    }

    /// Fractional `(nanoseconds * hertz)` debt below one second.
    pub const fn scaled_remainder(self) -> u32 {
        self.scaled_remainder
    }
}

/// Bounded readout from one lifecycle instance. It is observation only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeLifecycleReadout {
    pub(crate) instance_id: RuntimeInstanceId,
    pub(crate) mode: RuntimeMode,
    pub(crate) state: RuntimeState,
    pub(crate) generation: RuntimeGeneration,
    pub(crate) control_revision: RuntimeControlRevision,
    pub(crate) admitted_simulation_steps: u64,
    pub(crate) admitted_presentations: u64,
    pub(crate) dropped_realtime_steps: u128,
    pub(crate) clock_regressions: u64,
    pub(crate) scaled_remainder: Option<u32>,
    pub(crate) last_observed_time: Option<HostMonotonicTime>,
    pub(crate) fault: Option<RuntimeFault>,
}

impl RuntimeLifecycleReadout {
    pub const fn instance_id(self) -> RuntimeInstanceId {
        self.instance_id
    }

    pub const fn mode(self) -> RuntimeMode {
        self.mode
    }
    pub const fn state(self) -> RuntimeState {
        self.state
    }
    pub const fn generation(self) -> RuntimeGeneration {
        self.generation
    }
    pub const fn control_revision(self) -> RuntimeControlRevision {
        self.control_revision
    }
    pub const fn admitted_simulation_steps(self) -> u64 {
        self.admitted_simulation_steps
    }
    pub const fn admitted_presentations(self) -> u64 {
        self.admitted_presentations
    }
    pub const fn dropped_realtime_steps(self) -> u128 {
        self.dropped_realtime_steps
    }
    pub const fn clock_regressions(self) -> u64 {
        self.clock_regressions
    }
    pub const fn scaled_remainder(self) -> Option<u32> {
        self.scaled_remainder
    }
    pub const fn last_observed_time(self) -> Option<HostMonotonicTime> {
        self.last_observed_time
    }
    pub const fn fault(self) -> Option<RuntimeFault> {
        self.fault
    }
}

/// A caller error or safe rejection from the lifecycle admission boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeLifecycleError {
    WrongMode {
        operation: LifecycleOperation,
        mode: RuntimeMode,
    },
    WrongState {
        operation: LifecycleOperation,
        state: RuntimeState,
    },
    ClockRegression {
        previous: HostMonotonicTime,
        observed: HostMonotonicTime,
    },
    ExternalStepOutOfOrder {
        expected: ExternalStep,
        received: ExternalStep,
    },
    StaleToken {
        expected_generation: RuntimeGeneration,
        expected_control_revision: RuntimeControlRevision,
        received_generation: RuntimeGeneration,
        received_control_revision: RuntimeControlRevision,
    },
    ForeignInstance {
        expected: RuntimeInstanceId,
        received: RuntimeInstanceId,
    },
    WrongPhaseToken {
        expected: RuntimePhase,
        received: RuntimePhase,
    },
    UnknownSimulationStep {
        step: SimulationStep,
    },
    UnknownPresentation {
        sequence: u64,
    },
    CounterExhausted,
}

impl fmt::Display for RuntimeLifecycleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "runtime lifecycle rejected operation: {self:?}")
    }
}

impl std::error::Error for RuntimeLifecycleError {}
