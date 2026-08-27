use runtime_lifecycle::{
    ExternalStep, HostMonotonicTime, RealtimeLifecycleConfig, RuntimeFault, RuntimeInstanceId,
    RuntimeLifecycle, RuntimeLifecycleConfig, RuntimeLifecycleConfigError, RuntimeMode,
    RuntimePhase, RuntimeState,
};

fn realtime(hz: u32, catch_up: u32) -> RuntimeLifecycle {
    RuntimeLifecycle::new(
        RuntimeInstanceId::new(1),
        RuntimeLifecycleConfig::Realtime(RealtimeLifecycleConfig::new(hz, catch_up).unwrap()),
    )
}

fn demand() -> RuntimeLifecycle {
    RuntimeLifecycle::new(RuntimeInstanceId::new(1), RuntimeLifecycleConfig::Demand)
}

fn external() -> RuntimeLifecycle {
    RuntimeLifecycle::new(RuntimeInstanceId::new(1), RuntimeLifecycleConfig::External)
}

#[test]
fn realtime_uses_exact_scaled_accumulation_at_144_hz() {
    let mut lifecycle = realtime(144, 4);
    lifecycle.start().unwrap();
    lifecycle
        .advance_realtime(HostMonotonicTime::from_nanoseconds(0))
        .unwrap();

    let before_step = lifecycle
        .advance_realtime(HostMonotonicTime::from_nanoseconds(6_944_444))
        .unwrap();
    assert_eq!(before_step.simulation(), None);
    assert_eq!(before_step.scaled_remainder(), 999_999_936);

    let one_step = lifecycle
        .advance_realtime(HostMonotonicTime::from_nanoseconds(6_944_445))
        .unwrap();
    let admission = one_step.simulation().unwrap();
    assert_eq!(admission.step_count(), 1);
    assert_eq!(admission.first_step().value(), 0);
    assert_eq!(one_step.scaled_remainder(), 80);

    let step = admission.step_at(0).unwrap();
    assert_eq!(
        step.phases().input_snapshot().phase(),
        RuntimePhase::InputSnapshot
    );
    assert_eq!(step.phases().schedule().phase(), RuntimePhase::Schedule);
    assert_eq!(step.phases().timeline().phase(), RuntimePhase::Timeline);
    assert_eq!(step.phases().mutation().phase(), RuntimePhase::Mutation);
    assert_eq!(step.phases().projection().phase(), RuntimePhase::Projection);
    lifecycle
        .validate_phase_token(step.phases().projection(), RuntimePhase::Projection)
        .unwrap();
}

#[test]
fn realtime_caps_due_work_drops_whole_debt_and_retains_fraction() {
    let mut lifecycle = realtime(60, 4);
    lifecycle.start().unwrap();
    lifecycle
        .advance_realtime(HostMonotonicTime::from_nanoseconds(0))
        .unwrap();

    let advance = lifecycle
        .advance_realtime(HostMonotonicTime::from_nanoseconds(1_000_000_000))
        .unwrap();
    assert_eq!(advance.simulation().unwrap().step_count(), 4);
    assert_eq!(advance.dropped_steps(), 56);
    assert_eq!(advance.scaled_remainder(), 0);
    assert_eq!(lifecycle.readout().dropped_realtime_steps(), 56);

    let next = lifecycle
        .advance_realtime(HostMonotonicTime::from_nanoseconds(1_016_666_667))
        .unwrap();
    assert_eq!(next.simulation().unwrap().step_count(), 1);
    assert_eq!(next.scaled_remainder(), 20);
}

#[test]
fn realtime_handles_max_host_timestamp_without_integer_multiplication_overflow() {
    let mut lifecycle = realtime(240, 16);
    lifecycle.start().unwrap();
    lifecycle
        .advance_realtime(HostMonotonicTime::from_nanoseconds(0))
        .unwrap();

    let advance = lifecycle
        .advance_realtime(HostMonotonicTime::from_nanoseconds(u64::MAX))
        .unwrap();
    let due_steps = u128::from(u64::MAX) * 240 / 1_000_000_000;
    assert_eq!(advance.simulation().unwrap().step_count(), 16);
    assert_eq!(advance.dropped_steps(), due_steps - 16);
    assert!(advance.dropped_steps() > u128::from(u32::MAX));
}

#[test]
fn realtime_rejects_clock_regression_without_resetting_its_baseline() {
    let mut lifecycle = realtime(60, 4);
    lifecycle.start().unwrap();
    lifecycle
        .advance_realtime(HostMonotonicTime::from_nanoseconds(100))
        .unwrap();

    let error = lifecycle
        .advance_realtime(HostMonotonicTime::from_nanoseconds(99))
        .unwrap_err();
    assert!(matches!(
        error,
        runtime_lifecycle::RuntimeLifecycleError::ClockRegression { .. }
    ));
    assert_eq!(
        lifecycle.readout().last_observed_time(),
        Some(HostMonotonicTime::from_nanoseconds(100))
    );
    assert_eq!(lifecycle.readout().clock_regressions(), 1);

    let recovered = lifecycle
        .advance_realtime(HostMonotonicTime::from_nanoseconds(16_666_767))
        .unwrap();
    assert_eq!(recovered.simulation().unwrap().step_count(), 1);
}

#[test]
fn pause_resume_resets_baseline_and_keeps_presentation_available() {
    let mut lifecycle = realtime(60, 16);
    lifecycle.start().unwrap();
    lifecycle
        .advance_realtime(HostMonotonicTime::from_nanoseconds(0))
        .unwrap();
    lifecycle
        .advance_realtime(HostMonotonicTime::from_nanoseconds(500_000_000))
        .unwrap();

    let generation = lifecycle.generation();
    let running_token = lifecycle.admit_presentation().unwrap().token();
    let paused = lifecycle.pause().unwrap();
    assert_eq!(paused.generation(), generation);
    assert_eq!(lifecycle.state(), RuntimeState::Paused);
    assert!(lifecycle
        .validate_presentation_token(running_token)
        .is_err());

    let paused_presentation = lifecycle.admit_presentation().unwrap().token();
    lifecycle
        .validate_presentation_token(paused_presentation)
        .unwrap();
    assert!(lifecycle.admit_demand_step().is_err());

    let resumed = lifecycle.resume().unwrap();
    assert_eq!(resumed.generation(), generation);
    assert!(lifecycle
        .validate_presentation_token(paused_presentation)
        .is_err());
    let after_resume_baseline = lifecycle
        .advance_realtime(HostMonotonicTime::from_nanoseconds(10_000_000_000))
        .unwrap();
    assert_eq!(after_resume_baseline.simulation(), None);
    let after_resume = lifecycle
        .advance_realtime(HostMonotonicTime::from_nanoseconds(10_100_000_000))
        .unwrap();
    assert_eq!(after_resume.simulation().unwrap().step_count(), 6);
}

#[test]
fn demand_mode_has_no_clock_and_rejects_other_mode_admissions() {
    let mut lifecycle = demand();
    lifecycle.start().unwrap();
    let first = lifecycle.admit_demand_step().unwrap();
    assert_eq!(first.first_step().value(), 0);
    assert_eq!(first.step_count(), 1);
    assert_eq!(lifecycle.readout().mode(), RuntimeMode::Demand);
    assert!(lifecycle
        .advance_realtime(HostMonotonicTime::from_nanoseconds(1))
        .is_err());
    assert!(lifecycle.admit_external_step(ExternalStep::new(0)).is_err());
}

#[test]
fn external_mode_requires_exact_caller_step_numbers() {
    let mut lifecycle = external();
    lifecycle.start().unwrap();
    let first = lifecycle.admit_external_step(ExternalStep::new(0)).unwrap();
    assert_eq!(first.first_step().value(), 0);
    assert!(matches!(
        lifecycle.admit_external_step(ExternalStep::new(0)),
        Err(runtime_lifecycle::RuntimeLifecycleError::ExternalStepOutOfOrder { .. })
    ));
    assert!(matches!(
        lifecycle.admit_external_step(ExternalStep::new(2)),
        Err(runtime_lifecycle::RuntimeLifecycleError::ExternalStepOutOfOrder { .. })
    ));
    assert_eq!(
        lifecycle
            .admit_external_step(ExternalStep::new(1))
            .unwrap()
            .first_step()
            .value(),
        1
    );
}

#[test]
fn start_restart_and_control_revisions_make_old_tokens_stale() {
    let mut lifecycle = demand();
    let start = lifecycle.start().unwrap();
    assert_eq!(start.generation().value(), 1);
    let token = lifecycle
        .admit_demand_step()
        .unwrap()
        .step_at(0)
        .unwrap()
        .token();
    lifecycle.validate_simulation_token(token).unwrap();

    lifecycle.pause().unwrap();
    lifecycle.resume().unwrap();
    assert!(matches!(
        lifecycle.validate_simulation_token(token),
        Err(runtime_lifecycle::RuntimeLifecycleError::StaleToken { .. })
    ));

    let restarted = lifecycle.restart().unwrap();
    assert_eq!(restarted.generation().value(), 2);
    assert!(matches!(
        lifecycle.validate_simulation_token(token),
        Err(runtime_lifecycle::RuntimeLifecycleError::StaleToken { .. })
    ));
}

#[test]
fn fault_stops_simulation_until_a_new_generation_restarts() {
    let mut lifecycle = demand();
    lifecycle.start().unwrap();
    let before_fault = lifecycle
        .admit_demand_step()
        .unwrap()
        .step_at(0)
        .unwrap()
        .token();
    lifecycle.report_fault(RuntimeFault::OwnerReported).unwrap();
    assert_eq!(lifecycle.state(), RuntimeState::Faulted);
    assert_eq!(
        lifecycle.readout().fault(),
        Some(RuntimeFault::OwnerReported)
    );
    assert!(lifecycle.admit_demand_step().is_err());
    assert!(lifecycle.validate_simulation_token(before_fault).is_err());

    lifecycle.restart().unwrap();
    assert_eq!(lifecycle.state(), RuntimeState::Running);
    assert_eq!(lifecycle.readout().fault(), None);
}

#[test]
fn shutdown_is_terminal_and_invalidates_every_admission_path() {
    let mut lifecycle = demand();
    lifecycle.start().unwrap();
    let token = lifecycle.admit_presentation().unwrap().token();
    lifecycle.shutdown().unwrap();
    assert_eq!(lifecycle.state(), RuntimeState::Shutdown);
    assert!(lifecycle.start().is_err());
    assert!(lifecycle.restart().is_err());
    assert!(lifecycle.admit_demand_step().is_err());
    assert!(lifecycle.admit_presentation().is_err());
    assert!(lifecycle.validate_presentation_token(token).is_err());
}

#[test]
fn presentation_is_never_implicitly_admitted_by_realtime_advance() {
    let mut lifecycle = realtime(60, 4);
    lifecycle.start().unwrap();
    let presentation = lifecycle.admit_presentation().unwrap();
    assert_eq!(lifecycle.readout().admitted_simulation_steps(), 0);
    assert_eq!(lifecycle.readout().admitted_presentations(), 1);
    lifecycle
        .validate_presentation_token(presentation.token())
        .unwrap();

    lifecycle
        .advance_realtime(HostMonotonicTime::from_nanoseconds(0))
        .unwrap();
    lifecycle
        .advance_realtime(HostMonotonicTime::from_nanoseconds(16_666_667))
        .unwrap();
    assert_eq!(lifecycle.readout().admitted_simulation_steps(), 1);
    assert_eq!(lifecycle.readout().admitted_presentations(), 1);
}

#[test]
fn realtime_configuration_enforces_runtime_bounds() {
    assert_eq!(
        RealtimeLifecycleConfig::new(0, 1),
        Err(RuntimeLifecycleConfigError::ZeroFixedStepHz)
    );
    assert_eq!(
        RealtimeLifecycleConfig::new(241, 1),
        Err(RuntimeLifecycleConfigError::FixedStepHzExceedsMaximum)
    );
    assert_eq!(
        RealtimeLifecycleConfig::new(1, 0),
        Err(RuntimeLifecycleConfigError::ZeroCatchUpSteps)
    );
    assert_eq!(
        RealtimeLifecycleConfig::new(1, 17),
        Err(RuntimeLifecycleConfigError::CatchUpStepsExceedMaximum)
    );
    assert_eq!(
        RealtimeLifecycleConfig::new(240, 16)
            .unwrap()
            .fixed_step_hz(),
        240
    );
}

#[test]
fn explicit_runtime_configuration_selects_each_lifecycle_mode() {
    for (config, expected_mode) in [
        (
            RuntimeLifecycleConfig::Realtime(RealtimeLifecycleConfig::new(60, 4).unwrap()),
            RuntimeMode::Realtime,
        ),
        (RuntimeLifecycleConfig::Demand, RuntimeMode::Demand),
        (RuntimeLifecycleConfig::External, RuntimeMode::External),
    ] {
        let lifecycle = RuntimeLifecycle::new(RuntimeInstanceId::new(9), config);
        assert_eq!(lifecycle.mode(), expected_mode);
        assert_eq!(lifecycle.readout().instance_id(), RuntimeInstanceId::new(9));
    }
}

#[test]
fn tokens_reject_another_explicit_lifecycle_instance() {
    let mut first =
        RuntimeLifecycle::new(RuntimeInstanceId::new(1), RuntimeLifecycleConfig::Demand);
    let mut second =
        RuntimeLifecycle::new(RuntimeInstanceId::new(2), RuntimeLifecycleConfig::Demand);
    first.start().unwrap();
    second.start().unwrap();
    let token = first
        .admit_demand_step()
        .unwrap()
        .step_at(0)
        .unwrap()
        .token();

    assert!(matches!(
        second.validate_simulation_token(token),
        Err(runtime_lifecycle::RuntimeLifecycleError::ForeignInstance { .. })
    ));
}

#[test]
fn phase_validation_rejects_a_token_for_the_wrong_named_handoff() {
    let mut lifecycle = demand();
    lifecycle.start().unwrap();
    let step = lifecycle.admit_demand_step().unwrap().step_at(0).unwrap();
    assert!(matches!(
        lifecycle.validate_phase_token(step.phases().schedule(), RuntimePhase::Timeline),
        Err(runtime_lifecycle::RuntimeLifecycleError::WrongPhaseToken { .. })
    ));
    lifecycle
        .validate_phase_token(step.phases().schedule(), RuntimePhase::Schedule)
        .unwrap();
}

#[test]
fn pause_resume_retains_fractional_realtime_debt_but_requires_a_new_baseline() {
    let mut lifecycle = realtime(144, 4);
    lifecycle.start().unwrap();
    lifecycle
        .advance_realtime(HostMonotonicTime::from_nanoseconds(0))
        .unwrap();
    lifecycle
        .advance_realtime(HostMonotonicTime::from_nanoseconds(6_944_444))
        .unwrap();
    assert_eq!(lifecycle.readout().scaled_remainder(), Some(999_999_936));

    lifecycle.pause().unwrap();
    lifecycle.resume().unwrap();
    assert_eq!(lifecycle.readout().scaled_remainder(), Some(999_999_936));
    assert_eq!(lifecycle.readout().last_observed_time(), None);
    assert_eq!(
        lifecycle
            .advance_realtime(HostMonotonicTime::from_nanoseconds(100))
            .unwrap()
            .simulation(),
        None
    );
    assert_eq!(
        lifecycle
            .advance_realtime(HostMonotonicTime::from_nanoseconds(102))
            .unwrap()
            .simulation()
            .unwrap()
            .step_count(),
        1
    );
}

#[test]
fn repeated_token_validation_is_idempotent_correlation_evidence() {
    let mut lifecycle = demand();
    lifecycle.start().unwrap();
    let token = lifecycle
        .admit_demand_step()
        .unwrap()
        .step_at(0)
        .unwrap()
        .token();
    lifecycle.validate_simulation_token(token).unwrap();
    lifecycle.validate_simulation_token(token).unwrap();
}
