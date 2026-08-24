use product_model::SchedulePhase;
use runtime_lifecycle::{
    RuntimeControlRevision, RuntimeGeneration, RuntimeInstanceId, RuntimeLifecycle, RuntimePhase,
    RuntimePhaseToken, RuntimeState, SimulationStep,
};

use crate::{
    compile::{CompiledRuntimeSchedule, CompiledSystem},
    error::RuntimeScheduleError,
};

/// One immutable system invocation. It contains no mutable state, service
/// handle, component reference, callback, or host object.
#[derive(Debug, Clone, Copy)]
pub struct ScheduleSystemInvocation<'a> {
    phase: SchedulePhase,
    step: SimulationStep,
    system: &'a CompiledSystem,
}

impl<'a> ScheduleSystemInvocation<'a> {
    pub(crate) const fn new(
        phase: SchedulePhase,
        step: SimulationStep,
        system: &'a CompiledSystem,
    ) -> Self {
        Self {
            phase,
            step,
            system,
        }
    }

    pub const fn phase(self) -> SchedulePhase {
        self.phase
    }

    pub const fn step(self) -> SimulationStep {
        self.step
    }

    pub fn system_id(self) -> &'a str {
        self.system.id()
    }

    pub fn system(self) -> &'a CompiledSystem {
        self.system
    }
}

/// Explicit per-call dispatcher. Implementations may call named product or
/// Engine owners, but the schedule stores no dispatcher or owner table.
pub trait ScheduleDispatcher<C> {
    type Output;
    type Error;

    fn dispatch(
        &mut self,
        invocation: ScheduleSystemInvocation<'_>,
        context: &C,
    ) -> Result<Self::Output, Self::Error>;
}

impl<C, F, O, E> ScheduleDispatcher<C> for F
where
    F: FnMut(ScheduleSystemInvocation<'_>, &C) -> Result<O, E>,
{
    type Output = O;
    type Error = E;

    fn dispatch(
        &mut self,
        invocation: ScheduleSystemInvocation<'_>,
        context: &C,
    ) -> Result<Self::Output, Self::Error> {
        self(invocation, context)
    }
}

/// Typed receipt for one successfully dispatched schedule phase.
///
/// The schedule stages returned values before advancing its own progression;
/// effects performed inside a caller-owned dispatcher are outside this
/// receipt's rollback boundary.
#[derive(Debug, PartialEq, Eq)]
pub struct SchedulePhaseReceipt<O> {
    phase: SchedulePhase,
    step: SimulationStep,
    outputs: Vec<O>,
}

impl<O> SchedulePhaseReceipt<O> {
    pub const fn phase(&self) -> SchedulePhase {
        self.phase
    }

    pub const fn step(&self) -> SimulationStep {
        self.step
    }

    pub fn outputs(&self) -> &[O] {
        &self.outputs
    }

    pub fn into_outputs(self) -> Vec<O> {
        self.outputs
    }
}

/// Immutable progression readout for one bound schedule lane.
///
/// `invalidated_admission_count` counts simulation admissions that were
/// skipped because a same-generation lifecycle control revision changed
/// before the schedule could start them. Rebind should happen before
/// admitting new work under the resumed revision; any admissions not yet
/// represented by the schedule cursor are abandoned during reconciliation.
/// It is bounded by the lifecycle's `u64` admission counter and never
/// represents completed work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeScheduleReadout {
    instance_id: RuntimeInstanceId,
    generation: RuntimeGeneration,
    control_revision: RuntimeControlRevision,
    active_step: Option<SimulationStep>,
    last_completed_step: Option<SimulationStep>,
    next_expected_step: Option<u64>,
    invalidated_admission_count: u64,
    next_phase: Option<SchedulePhase>,
    disposed: bool,
}

impl RuntimeScheduleReadout {
    pub const fn instance_id(self) -> RuntimeInstanceId {
        self.instance_id
    }

    pub const fn generation(self) -> RuntimeGeneration {
        self.generation
    }

    pub const fn control_revision(self) -> RuntimeControlRevision {
        self.control_revision
    }

    pub const fn active_step(self) -> Option<SimulationStep> {
        self.active_step
    }

    pub const fn last_completed_step(self) -> Option<SimulationStep> {
        self.last_completed_step
    }

    pub const fn next_expected_step(self) -> Option<u64> {
        self.next_expected_step
    }

    pub const fn invalidated_admission_count(self) -> u64 {
        self.invalidated_admission_count
    }

    pub const fn next_phase(self) -> Option<SchedulePhase> {
        self.next_phase
    }

    pub const fn is_disposed(self) -> bool {
        self.disposed
    }
}

/// A bound, instance-owned execution lane. It is deliberately not `Clone`:
/// copying phase progression would create two authorities for one lifecycle
/// generation.
#[derive(Debug)]
pub struct RuntimeSchedule {
    compiled: CompiledRuntimeSchedule,
    instance_id: RuntimeInstanceId,
    generation: RuntimeGeneration,
    control_revision: RuntimeControlRevision,
    active_step: Option<SimulationStep>,
    last_completed_step: Option<SimulationStep>,
    next_expected_step: Option<u64>,
    invalidated_admission_count: u64,
    next_phase: usize,
    disposed: bool,
}

impl RuntimeSchedule {
    pub(crate) fn bind(
        compiled: CompiledRuntimeSchedule,
        lifecycle: &RuntimeLifecycle,
    ) -> Result<Self, RuntimeScheduleError> {
        if lifecycle.state() != RuntimeState::Running {
            return Err(RuntimeScheduleError::LifecycleNotRunning);
        }
        let admitted_steps = lifecycle.readout().admitted_simulation_steps();
        if admitted_steps != 0 {
            return Err(RuntimeScheduleError::LifecycleAlreadyAdvanced { admitted_steps });
        }
        Ok(Self {
            compiled,
            instance_id: lifecycle.instance_id(),
            generation: lifecycle.generation(),
            control_revision: lifecycle.control_revision(),
            active_step: None,
            last_completed_step: None,
            next_expected_step: Some(0),
            invalidated_admission_count: 0,
            next_phase: 0,
            disposed: false,
        })
    }

    pub fn compiled(&self) -> &CompiledRuntimeSchedule {
        &self.compiled
    }

    pub fn inspection(&self) -> &crate::ScheduleInspection {
        self.compiled.inspection()
    }

    pub fn instance_id(&self) -> RuntimeInstanceId {
        self.instance_id
    }

    pub const fn generation(&self) -> RuntimeGeneration {
        self.generation
    }

    pub const fn control_revision(&self) -> RuntimeControlRevision {
        self.control_revision
    }

    pub const fn active_step(&self) -> Option<SimulationStep> {
        self.active_step
    }

    pub const fn last_completed_step(&self) -> Option<SimulationStep> {
        self.last_completed_step
    }

    /// Returns the next lifecycle step this lane can begin. This cursor is
    /// independent of `last_completed_step` because stale admissions may be
    /// invalidated without being completed.
    pub const fn next_expected_step(&self) -> Option<u64> {
        self.next_expected_step
    }

    /// Returns the number of same-generation admissions invalidated during
    /// explicit lifecycle rebinds. Invalidated admissions are not completed.
    pub const fn invalidated_admission_count(&self) -> u64 {
        self.invalidated_admission_count
    }

    pub const fn next_phase(&self) -> Option<SchedulePhase> {
        if self.next_phase < SchedulePhase::ALL.len() {
            Some(SchedulePhase::ALL[self.next_phase])
        } else {
            None
        }
    }

    pub const fn is_disposed(&self) -> bool {
        self.disposed
    }

    pub const fn readout(&self) -> RuntimeScheduleReadout {
        RuntimeScheduleReadout {
            instance_id: self.instance_id,
            generation: self.generation,
            control_revision: self.control_revision,
            active_step: self.active_step,
            last_completed_step: self.last_completed_step,
            next_expected_step: self.next_expected_step,
            invalidated_admission_count: self.invalidated_admission_count,
            next_phase: self.next_phase(),
            disposed: self.disposed,
        }
    }

    /// Terminally disposes this lane. Later execution attempts fail and no
    /// in-flight phase can be resumed through this instance.
    pub fn dispose(&mut self) {
        self.active_step = None;
        self.next_phase = SchedulePhase::ALL.len();
        self.disposed = true;
    }

    /// Reconciles this schedule lane with a lifecycle that advanced its
    /// control revision or started a new generation. Rebinding is explicit so
    /// a stale lane can never silently accept a token from a new epoch.
    ///
    /// A same-generation control revision change (for example pause/resume)
    /// retains completed progress and reconciles the next-step cursor with
    /// cumulative lifecycle admissions. Admissions not represented by the
    /// cursor are counted as invalidated rather than falsely marked completed.
    /// A newer
    /// generation (restart) resets step progression and that count to zero.
    /// Rebinding is rejected while a phase is active, for a foreign instance,
    /// for a non-running lifecycle, or for an older/equal binding.
    pub fn rebind(&mut self, lifecycle: &RuntimeLifecycle) -> Result<(), RuntimeScheduleError> {
        if self.disposed {
            return Err(RuntimeScheduleError::Disposed);
        }
        if lifecycle.instance_id() != self.instance_id {
            return Err(RuntimeScheduleError::RebindForeignInstance {
                expected: self.instance_id,
                received: lifecycle.instance_id(),
            });
        }
        if lifecycle.state() != RuntimeState::Running {
            return Err(RuntimeScheduleError::LifecycleNotRunning);
        }
        if let Some(step) = self.active_step {
            return Err(RuntimeScheduleError::RebindActiveStep { step: step.value() });
        }
        let newer_generation = lifecycle.generation().value() > self.generation.value();
        let newer_same_generation = lifecycle.generation() == self.generation
            && lifecycle.control_revision().value() > self.control_revision.value();
        if !newer_generation && !newer_same_generation {
            return Err(RuntimeScheduleError::RebindRegression {
                expected_generation: self.generation,
                received_generation: lifecycle.generation(),
                expected_control_revision: self.control_revision.value(),
                received_control_revision: lifecycle.control_revision().value(),
            });
        }
        let (next_expected_step, invalidated_admission_count) = if newer_generation {
            (Some(0), 0)
        } else {
            reconcile_admitted_steps(
                self.next_expected_step,
                lifecycle.readout().admitted_simulation_steps(),
                self.invalidated_admission_count,
            )?
        };
        self.generation = lifecycle.generation();
        self.control_revision = lifecycle.control_revision();
        self.next_expected_step = next_expected_step;
        self.invalidated_admission_count = invalidated_admission_count;
        if newer_generation {
            self.active_step = None;
            self.last_completed_step = None;
            self.next_phase = 0;
        }
        Ok(())
    }

    /// Alias for callers that describe lifecycle revision changes as
    /// synchronization rather than rebinding.
    pub fn synchronize(
        &mut self,
        lifecycle: &RuntimeLifecycle,
    ) -> Result<(), RuntimeScheduleError> {
        self.rebind(lifecycle)
    }

    /// Executes exactly one expected phase. Outputs are staged in a local
    /// vector; phase progression changes only after every due dispatch returns
    /// success. A failed dispatch therefore leaves the lane retryable at the
    /// same phase and step. Effects performed by the caller-owned dispatcher
    /// are not rolled back by this data-only lane.
    pub fn execute_phase<C, D>(
        &mut self,
        lifecycle: &RuntimeLifecycle,
        token: RuntimePhaseToken,
        context: &C,
        dispatcher: &mut D,
    ) -> Result<SchedulePhaseReceipt<D::Output>, RuntimeScheduleError<D::Error>>
    where
        D: ScheduleDispatcher<C>,
    {
        if self.disposed {
            return Err(RuntimeScheduleError::Disposed);
        }
        let expected_phase = self
            .next_phase()
            .ok_or(RuntimeScheduleError::PhaseOutOfOrder {
                expected: SchedulePhase::Input,
                received: SchedulePhase::Projection,
            })?;
        let received_phase = schedule_phase_for_runtime(token.phase()).ok_or(
            RuntimeScheduleError::PhaseOutOfOrder {
                expected: expected_phase,
                received: SchedulePhase::Projection,
            },
        )?;
        if received_phase != expected_phase {
            return Err(RuntimeScheduleError::PhaseOutOfOrder {
                expected: expected_phase,
                received: received_phase,
            });
        }
        let runtime_phase = runtime_phase_for_schedule(expected_phase);
        lifecycle.validate_phase_token(token, runtime_phase)?;
        let simulation = token.simulation();
        if simulation.instance_id() != self.instance_id
            || simulation.generation() != self.generation
            || simulation.control_revision() != self.control_revision
        {
            return Err(RuntimeScheduleError::LifecycleBindingMismatch);
        }
        let step = simulation.step();
        match self.active_step {
            Some(active) if active != step => {
                return Err(RuntimeScheduleError::StepMismatch {
                    expected: active.value(),
                    received: step.value(),
                });
            }
            None => {
                if expected_phase != SchedulePhase::Input {
                    return Err(RuntimeScheduleError::PhaseOutOfOrder {
                        expected: SchedulePhase::Input,
                        received: expected_phase,
                    });
                }
                if self.next_expected_step != Some(step.value()) {
                    return Err(RuntimeScheduleError::StepOutOfOrder {
                        expected: self.next_expected_step,
                        received: step.value(),
                    });
                }
                self.active_step = Some(step);
            }
            Some(_) => {}
        }

        let phase = self.compiled.phase(expected_phase);
        let mut outputs = Vec::new();
        for system_id in phase.final_order() {
            let system = phase
                .systems()
                .iter()
                .find(|system| system.id() == system_id)
                .expect("compiled final order contains only compiled system ids");
            if !system.cadence().is_due(step.value()) {
                continue;
            }
            let invocation = ScheduleSystemInvocation::new(expected_phase, step, system);
            let output = dispatcher
                .dispatch(invocation, context)
                .map_err(RuntimeScheduleError::Dispatch)?;
            outputs.push(output);
        }

        self.next_phase += 1;
        if self.next_phase == SchedulePhase::ALL.len() {
            if let Some(completed_step) = self.active_step.take() {
                self.last_completed_step = Some(completed_step);
                self.next_expected_step = next_step_after(completed_step.value());
            }
            self.next_phase = 0;
        }
        Ok(SchedulePhaseReceipt {
            phase: expected_phase,
            step,
            outputs,
        })
    }
}

/// Maps the Product Model phase to the corresponding lifecycle admission
/// token. The five names remain distinct even where products choose an empty
/// standard fragment.
pub const fn runtime_phase_for_schedule(phase: SchedulePhase) -> RuntimePhase {
    match phase {
        SchedulePhase::Input => RuntimePhase::InputSnapshot,
        SchedulePhase::Simulation => RuntimePhase::Schedule,
        SchedulePhase::Consequences => RuntimePhase::Timeline,
        SchedulePhase::Commit => RuntimePhase::Mutation,
        SchedulePhase::Projection => RuntimePhase::Projection,
    }
}

fn schedule_phase_for_runtime(phase: RuntimePhase) -> Option<SchedulePhase> {
    Some(match phase {
        RuntimePhase::InputSnapshot => SchedulePhase::Input,
        RuntimePhase::Schedule => SchedulePhase::Simulation,
        RuntimePhase::Timeline => SchedulePhase::Consequences,
        RuntimePhase::Mutation => SchedulePhase::Commit,
        RuntimePhase::Projection => SchedulePhase::Projection,
    })
}

fn next_step_after(step: u64) -> Option<u64> {
    step.checked_add(1)
}

fn reconcile_admitted_steps(
    next_expected_step: Option<u64>,
    admitted_steps: u64,
    invalidated_admission_count: u64,
) -> Result<(Option<u64>, u64), RuntimeScheduleError> {
    let expected_admissions = next_expected_step.unwrap_or(u64::MAX);
    if admitted_steps < expected_admissions {
        return Err(RuntimeScheduleError::RebindAdmissionRegression {
            expected_next_step: next_expected_step,
            admitted_steps,
        });
    }
    let newly_invalidated = admitted_steps - expected_admissions;
    let invalidated_admission_count = invalidated_admission_count
        .checked_add(newly_invalidated)
        .ok_or(RuntimeScheduleError::InvalidatedAdmissionOverflow)?;
    let next_expected_step = if admitted_steps == u64::MAX {
        None
    } else {
        Some(admitted_steps)
    };
    Ok((next_expected_step, invalidated_admission_count))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_step_rejects_gaps_and_u64_exhaustion_without_overflow() {
        assert_eq!(next_step_after(4), Some(5));
        assert_eq!(next_step_after(u64::MAX), None);
    }

    #[test]
    fn admitted_reconciliation_counts_stale_steps_without_claiming_completion() {
        assert_eq!(
            reconcile_admitted_steps(Some(0), 3, 0).unwrap(),
            (Some(3), 3)
        );
        assert_eq!(
            reconcile_admitted_steps(Some(3), 3, 3).unwrap(),
            (Some(3), 3)
        );
        assert_eq!(
            reconcile_admitted_steps(None, u64::MAX, 7).unwrap(),
            (None, 7)
        );
    }

    #[test]
    fn admitted_reconciliation_rejects_regression_and_count_overflow() {
        assert_eq!(
            reconcile_admitted_steps(Some(3), 2, 0),
            Err(RuntimeScheduleError::RebindAdmissionRegression {
                expected_next_step: Some(3),
                admitted_steps: 2,
            })
        );
        assert_eq!(
            reconcile_admitted_steps(Some(0), 1, u64::MAX),
            Err(RuntimeScheduleError::InvalidatedAdmissionOverflow)
        );
    }
}
