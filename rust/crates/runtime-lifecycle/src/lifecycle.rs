use crate::model::{
    ExternalStep, HostMonotonicTime, LifecycleOperation, LifecycleReceipt, PresentationAdmission,
    PresentationToken, RealtimeAdvance, RuntimeControlOperation, RuntimeControlRevision,
    RuntimeFault, RuntimeGeneration, RuntimeInstanceId, RuntimeLifecycleConfig,
    RuntimeLifecycleError, RuntimeLifecycleReadout, RuntimeMode, RuntimePhaseToken, RuntimeState,
    SimulationAdmission, SimulationStep, SimulationToken, SCALED_NANOSECONDS_PER_SECOND,
};
use product_model::ProductManifest;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RealtimeState {
    last_observed_time: Option<HostMonotonicTime>,
    scaled_remainder: u128,
}

impl RealtimeState {
    const fn new() -> Self {
        Self {
            last_observed_time: None,
            scaled_remainder: 0,
        }
    }

    fn reset(&mut self) {
        self.last_observed_time = None;
        self.scaled_remainder = 0;
    }
}

/// One instance-owned runtime lifecycle selected by a Product Manifest.
///
/// The lifecycle admits work but never performs it. A downstream product reads
/// an admission plan, gathers input, invokes named schedule/timeline/mutation
/// owners, and projects presentation itself. Re-validating one emitted token is
/// harmless correlation evidence; it is not a completion acknowledgement or
/// one-shot mutation right.
#[derive(Debug)]
pub struct RuntimeLifecycle {
    instance_id: RuntimeInstanceId,
    config: RuntimeLifecycleConfig,
    state: RuntimeState,
    generation: RuntimeGeneration,
    control_revision: RuntimeControlRevision,
    next_simulation_step: u64,
    next_presentation: u64,
    dropped_realtime_steps: u128,
    clock_regressions: u64,
    realtime: Option<RealtimeState>,
    fault: Option<RuntimeFault>,
}

impl RuntimeLifecycle {
    /// Creates a stopped lifecycle from an explicit configuration.
    pub const fn new(instance_id: RuntimeInstanceId, config: RuntimeLifecycleConfig) -> Self {
        let realtime = match config {
            RuntimeLifecycleConfig::Realtime(_) => Some(RealtimeState::new()),
            RuntimeLifecycleConfig::Demand | RuntimeLifecycleConfig::External => None,
        };
        Self {
            instance_id,
            config,
            state: RuntimeState::Created,
            generation: RuntimeGeneration::ZERO,
            control_revision: RuntimeControlRevision::ZERO,
            next_simulation_step: 0,
            next_presentation: 0,
            dropped_realtime_steps: 0,
            clock_regressions: 0,
            realtime,
            fault: None,
        }
    }

    /// Creates a stopped lifecycle from the manifest's validated selection.
    pub fn from_product_manifest(
        instance_id: RuntimeInstanceId,
        manifest: &ProductManifest,
    ) -> Result<Self, crate::RuntimeLifecycleConfigError> {
        Ok(Self::new(
            instance_id,
            RuntimeLifecycleConfig::from_product_manifest(manifest)?,
        ))
    }

    pub const fn instance_id(&self) -> RuntimeInstanceId {
        self.instance_id
    }

    pub const fn configuration(&self) -> RuntimeLifecycleConfig {
        self.config
    }

    pub const fn mode(&self) -> RuntimeMode {
        self.config.mode()
    }

    pub const fn state(&self) -> RuntimeState {
        self.state
    }

    pub const fn generation(&self) -> RuntimeGeneration {
        self.generation
    }

    pub const fn control_revision(&self) -> RuntimeControlRevision {
        self.control_revision
    }

    /// Starts the first runtime generation. Starting twice is rejected.
    pub fn start(&mut self) -> Result<LifecycleReceipt, RuntimeLifecycleError> {
        self.require_state(LifecycleOperation::Start, &[RuntimeState::Created])?;
        self.begin_new_generation(LifecycleOperation::Start)
    }

    /// Pauses simulation admission and clears the realtime clock baseline.
    ///
    /// Presentation remains independently admissible while paused, so a
    /// downstream menu or inspector can project the last accepted facts.
    pub fn pause(&mut self) -> Result<LifecycleReceipt, RuntimeLifecycleError> {
        self.require_state(LifecycleOperation::Pause, &[RuntimeState::Running])?;
        self.advance_control_revision()?;
        self.state = RuntimeState::Paused;
        self.clear_realtime_baseline();
        Ok(self.receipt(LifecycleOperation::Pause))
    }

    /// Resumes simulation admission in the same runtime generation.
    ///
    /// Resume changes the control revision and requires a new realtime clock
    /// baseline, preventing elapsed paused wall time from becoming debt.
    pub fn resume(&mut self) -> Result<LifecycleReceipt, RuntimeLifecycleError> {
        self.require_state(LifecycleOperation::Resume, &[RuntimeState::Paused])?;
        self.advance_control_revision()?;
        self.state = RuntimeState::Running;
        self.clear_realtime_baseline();
        Ok(self.receipt(LifecycleOperation::Resume))
    }

    /// Starts a fresh generation after an active, paused, or faulted run.
    ///
    /// Shutdown is terminal; callers that deliberately need a new product
    /// runtime create a new `RuntimeLifecycle` instance instead.
    pub fn restart(&mut self) -> Result<LifecycleReceipt, RuntimeLifecycleError> {
        self.require_state(
            LifecycleOperation::Restart,
            &[
                RuntimeState::Running,
                RuntimeState::Paused,
                RuntimeState::Faulted,
            ],
        )?;
        self.begin_new_generation(LifecycleOperation::Restart)
    }

    /// Stops this lifecycle permanently and invalidates every prior token.
    pub fn shutdown(&mut self) -> Result<LifecycleReceipt, RuntimeLifecycleError> {
        self.require_not_state(LifecycleOperation::Shutdown, RuntimeState::Shutdown)?;
        self.advance_control_revision()?;
        self.state = RuntimeState::Shutdown;
        self.reset_realtime_progress();
        Ok(self.receipt(LifecycleOperation::Shutdown))
    }

    /// Records an owner-reported failure without inventing a generic error bus.
    ///
    /// Detailed diagnostics remain with the named downstream owner. A restart
    /// creates a new generation after the caller has resolved that failure.
    pub fn report_fault(
        &mut self,
        fault: RuntimeFault,
    ) -> Result<LifecycleReceipt, RuntimeLifecycleError> {
        self.require_state(
            LifecycleOperation::ReportFault,
            &[RuntimeState::Running, RuntimeState::Paused],
        )?;
        self.advance_control_revision()?;
        self.state = RuntimeState::Faulted;
        self.fault = Some(fault);
        self.reset_realtime_progress();
        Ok(self.receipt(LifecycleOperation::ReportFault))
    }

    /// Replaces or releases the current control binding without pausing,
    /// restarting, or otherwise changing simulation/product state. Callers
    /// rebind their named input owner to the returned revision before later
    /// product input is admitted.
    pub fn change_control(
        &mut self,
        operation: RuntimeControlOperation,
    ) -> Result<LifecycleReceipt, RuntimeLifecycleError> {
        let lifecycle_operation = operation.lifecycle_operation();
        self.require_state(
            lifecycle_operation,
            &[RuntimeState::Running, RuntimeState::Paused],
        )?;
        self.advance_control_revision()?;
        Ok(self.receipt(lifecycle_operation))
    }

    /// Admits due fixed simulation steps from a caller-supplied monotonic time.
    ///
    /// It uses the exact scaled accumulator `delta_ns * hertz`, admits no more
    /// than the configured catch-up cap, drops excess *whole* steps, and keeps
    /// the fractional remainder. It never admits presentation work.
    pub fn advance_realtime(
        &mut self,
        observed_time: HostMonotonicTime,
    ) -> Result<RealtimeAdvance, RuntimeLifecycleError> {
        let config = match self.config {
            RuntimeLifecycleConfig::Realtime(config) => config,
            _ => {
                return Err(RuntimeLifecycleError::WrongMode {
                    operation: LifecycleOperation::AdvanceRealtime,
                    mode: self.mode(),
                })
            }
        };
        self.require_state(
            LifecycleOperation::AdvanceRealtime,
            &[RuntimeState::Running],
        )?;

        let realtime = match self.realtime {
            Some(realtime) => realtime,
            None => return self.counter_exhausted(),
        };
        let Some(previous) = realtime.last_observed_time else {
            self.realtime = Some(RealtimeState {
                last_observed_time: Some(observed_time),
                scaled_remainder: realtime.scaled_remainder,
            });
            return Ok(RealtimeAdvance::new(
                observed_time,
                None,
                0,
                scaled_remainder_u32(realtime.scaled_remainder),
            ));
        };

        if observed_time < previous {
            self.clock_regressions = match self.clock_regressions.checked_add(1) {
                Some(value) => value,
                None => return self.counter_exhausted(),
            };
            return Err(RuntimeLifecycleError::ClockRegression {
                previous,
                observed: observed_time,
            });
        }

        let elapsed_nanoseconds = observed_time.nanoseconds() - previous.nanoseconds();
        let elapsed_scaled = u128::from(elapsed_nanoseconds) * u128::from(config.fixed_step_hz());
        let scaled_total = match realtime.scaled_remainder.checked_add(elapsed_scaled) {
            Some(value) => value,
            None => return self.counter_exhausted(),
        };
        let due_steps = scaled_total / SCALED_NANOSECONDS_PER_SECOND;
        let scaled_remainder = scaled_total % SCALED_NANOSECONDS_PER_SECOND;
        let admitted_count = due_steps.min(u128::from(config.max_catch_up_steps())) as u32;
        let dropped_steps = due_steps - u128::from(admitted_count);

        let next_dropped = match self.dropped_realtime_steps.checked_add(dropped_steps) {
            Some(value) => value,
            None => return self.counter_exhausted(),
        };
        let simulation = if admitted_count == 0 {
            None
        } else {
            Some(self.prepare_simulation_admission(admitted_count)?)
        };

        self.realtime = Some(RealtimeState {
            last_observed_time: Some(observed_time),
            scaled_remainder,
        });
        self.dropped_realtime_steps = next_dropped;
        Ok(RealtimeAdvance::new(
            observed_time,
            simulation,
            dropped_steps,
            scaled_remainder_u32(scaled_remainder),
        ))
    }

    /// Admits one caller-demanded simulation step. It never reads a clock.
    pub fn admit_demand_step(&mut self) -> Result<SimulationAdmission, RuntimeLifecycleError> {
        self.require_mode(LifecycleOperation::AdmitDemandStep, RuntimeMode::Demand)?;
        self.require_state(
            LifecycleOperation::AdmitDemandStep,
            &[RuntimeState::Running],
        )?;
        self.prepare_simulation_admission(1)
    }

    /// Admits the exact next externally supplied deterministic step number.
    ///
    /// No timestamp enters this mode. Supplying a duplicate or skipped value is
    /// rejected without changing lifecycle state.
    pub fn admit_external_step(
        &mut self,
        external_step: ExternalStep,
    ) -> Result<SimulationAdmission, RuntimeLifecycleError> {
        self.require_mode(LifecycleOperation::AdmitExternalStep, RuntimeMode::External)?;
        self.require_state(
            LifecycleOperation::AdmitExternalStep,
            &[RuntimeState::Running],
        )?;
        let expected = ExternalStep::new(self.next_simulation_step);
        if external_step != expected {
            return Err(RuntimeLifecycleError::ExternalStepOutOfOrder {
                expected,
                received: external_step,
            });
        }
        self.prepare_simulation_admission(1)
    }

    /// Admits one presentation attempt without scheduling simulation.
    ///
    /// This remains available while paused for menus and inspection. It is not
    /// available before start, after fault, or after shutdown.
    pub fn admit_presentation(&mut self) -> Result<PresentationAdmission, RuntimeLifecycleError> {
        self.require_presentation_state(LifecycleOperation::AdmitPresentation)?;
        if self.next_presentation == u64::MAX {
            return self.counter_exhausted();
        }
        let token = PresentationToken::new(
            self.instance_id,
            self.generation,
            self.control_revision,
            self.next_presentation,
        );
        self.next_presentation += 1;
        Ok(PresentationAdmission::new(token))
    }

    /// Validates a simulation token as idempotent correlation evidence.
    pub fn validate_simulation_token(
        &self,
        token: SimulationToken,
    ) -> Result<(), RuntimeLifecycleError> {
        self.require_state(
            LifecycleOperation::ValidateSimulationToken,
            &[RuntimeState::Running],
        )?;
        self.require_current_token(
            token.instance_id(),
            token.generation(),
            token.control_revision(),
            token.step().value() < self.next_simulation_step,
            Some(token.step()),
            None,
        )
    }

    /// Validates a phase token as its underlying simulation token.
    pub fn validate_phase_token(
        &self,
        token: RuntimePhaseToken,
        expected_phase: crate::RuntimePhase,
    ) -> Result<(), RuntimeLifecycleError> {
        if token.phase() != expected_phase {
            return Err(RuntimeLifecycleError::WrongPhaseToken {
                expected: expected_phase,
                received: token.phase(),
            });
        }
        self.validate_simulation_token(token.simulation())
    }

    /// Validates a presentation token while running or paused.
    pub fn validate_presentation_token(
        &self,
        token: PresentationToken,
    ) -> Result<(), RuntimeLifecycleError> {
        self.require_presentation_state(LifecycleOperation::ValidatePresentationToken)?;
        self.require_current_token(
            token.instance_id(),
            token.generation(),
            token.control_revision(),
            token.sequence() < self.next_presentation,
            None,
            Some(token.sequence()),
        )
    }

    /// Returns current lifecycle facts without advancing time or admitting work.
    pub fn readout(&self) -> RuntimeLifecycleReadout {
        let (scaled_remainder, last_observed_time) = self
            .realtime
            .map(|realtime| {
                (
                    Some(scaled_remainder_u32(realtime.scaled_remainder)),
                    realtime.last_observed_time,
                )
            })
            .unwrap_or((None, None));
        RuntimeLifecycleReadout {
            instance_id: self.instance_id,
            mode: self.mode(),
            state: self.state,
            generation: self.generation,
            control_revision: self.control_revision,
            admitted_simulation_steps: self.next_simulation_step,
            admitted_presentations: self.next_presentation,
            dropped_realtime_steps: self.dropped_realtime_steps,
            clock_regressions: self.clock_regressions,
            scaled_remainder,
            last_observed_time,
            fault: self.fault,
        }
    }

    fn begin_new_generation(
        &mut self,
        operation: LifecycleOperation,
    ) -> Result<LifecycleReceipt, RuntimeLifecycleError> {
        let generation = match self.generation.0.checked_add(1) {
            Some(value) => RuntimeGeneration(value),
            None => return self.counter_exhausted(),
        };
        self.advance_control_revision()?;
        self.generation = generation;
        self.state = RuntimeState::Running;
        self.next_simulation_step = 0;
        self.next_presentation = 0;
        self.dropped_realtime_steps = 0;
        self.clock_regressions = 0;
        self.fault = None;
        self.reset_realtime_progress();
        Ok(self.receipt(operation))
    }

    fn prepare_simulation_admission(
        &mut self,
        step_count: u32,
    ) -> Result<SimulationAdmission, RuntimeLifecycleError> {
        debug_assert!(step_count > 0);
        let count = u64::from(step_count);
        let next_step = match self.next_simulation_step.checked_add(count) {
            Some(value) => value,
            None => return self.counter_exhausted(),
        };
        let admission = SimulationAdmission::new(
            self.instance_id,
            self.generation,
            self.control_revision,
            SimulationStep(self.next_simulation_step),
            step_count,
        );
        self.next_simulation_step = next_step;
        Ok(admission)
    }

    fn advance_control_revision(&mut self) -> Result<(), RuntimeLifecycleError> {
        self.control_revision = match self.control_revision.0.checked_add(1) {
            Some(value) => RuntimeControlRevision(value),
            None => return self.counter_exhausted(),
        };
        Ok(())
    }

    fn reset_realtime_progress(&mut self) {
        if let Some(realtime) = &mut self.realtime {
            realtime.reset();
        }
    }

    fn clear_realtime_baseline(&mut self) {
        if let Some(realtime) = &mut self.realtime {
            realtime.last_observed_time = None;
        }
    }

    fn receipt(&self, operation: LifecycleOperation) -> LifecycleReceipt {
        LifecycleReceipt::new(
            self.instance_id,
            operation,
            self.state,
            self.generation,
            self.control_revision,
        )
    }

    fn require_mode(
        &self,
        operation: LifecycleOperation,
        expected: RuntimeMode,
    ) -> Result<(), RuntimeLifecycleError> {
        if self.mode() == expected {
            Ok(())
        } else {
            Err(RuntimeLifecycleError::WrongMode {
                operation,
                mode: self.mode(),
            })
        }
    }

    fn require_state(
        &self,
        operation: LifecycleOperation,
        allowed: &[RuntimeState],
    ) -> Result<(), RuntimeLifecycleError> {
        if allowed.contains(&self.state) {
            Ok(())
        } else {
            Err(RuntimeLifecycleError::WrongState {
                operation,
                state: self.state,
            })
        }
    }

    fn require_not_state(
        &self,
        operation: LifecycleOperation,
        prohibited: RuntimeState,
    ) -> Result<(), RuntimeLifecycleError> {
        if self.state == prohibited {
            Err(RuntimeLifecycleError::WrongState {
                operation,
                state: self.state,
            })
        } else {
            Ok(())
        }
    }

    fn require_presentation_state(
        &self,
        operation: LifecycleOperation,
    ) -> Result<(), RuntimeLifecycleError> {
        self.require_state(operation, &[RuntimeState::Running, RuntimeState::Paused])
    }

    fn require_current_token(
        &self,
        received_instance_id: RuntimeInstanceId,
        received_generation: RuntimeGeneration,
        received_control_revision: RuntimeControlRevision,
        known: bool,
        step: Option<SimulationStep>,
        presentation_sequence: Option<u64>,
    ) -> Result<(), RuntimeLifecycleError> {
        if received_instance_id != self.instance_id {
            return Err(RuntimeLifecycleError::ForeignInstance {
                expected: self.instance_id,
                received: received_instance_id,
            });
        }
        if received_generation != self.generation
            || received_control_revision != self.control_revision
        {
            return Err(RuntimeLifecycleError::StaleToken {
                expected_generation: self.generation,
                expected_control_revision: self.control_revision,
                received_generation,
                received_control_revision,
            });
        }
        if known {
            return Ok(());
        }
        match (step, presentation_sequence) {
            (Some(step), None) => Err(RuntimeLifecycleError::UnknownSimulationStep { step }),
            (None, Some(sequence)) => Err(RuntimeLifecycleError::UnknownPresentation { sequence }),
            _ => unreachable!("token validation identifies exactly one token family"),
        }
    }

    fn counter_exhausted<T>(&mut self) -> Result<T, RuntimeLifecycleError> {
        self.state = RuntimeState::Faulted;
        self.fault = Some(RuntimeFault::CounterExhausted);
        self.reset_realtime_progress();
        Err(RuntimeLifecycleError::CounterExhausted)
    }
}

fn scaled_remainder_u32(value: u128) -> u32 {
    debug_assert!(value < SCALED_NANOSECONDS_PER_SECOND);
    value as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RuntimeInstanceId;

    #[test]
    fn simulation_sequence_exhaustion_faults_instead_of_wrapping() {
        let mut lifecycle =
            RuntimeLifecycle::new(RuntimeInstanceId::new(7), RuntimeLifecycleConfig::Demand);
        lifecycle.start().unwrap();
        lifecycle.next_simulation_step = u64::MAX;

        assert_eq!(
            lifecycle.admit_demand_step(),
            Err(RuntimeLifecycleError::CounterExhausted)
        );
        assert_eq!(lifecycle.state(), RuntimeState::Faulted);
        assert_eq!(
            lifecycle.readout().fault(),
            Some(RuntimeFault::CounterExhausted)
        );
    }
}
