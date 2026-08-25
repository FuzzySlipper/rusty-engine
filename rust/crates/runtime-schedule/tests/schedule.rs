use product_model::{
    admit_checked_product_composition, decode_compiled_composition, decode_product_manifest,
    link_admitted_product_composition, validate_compiled_composition, CapabilityAccess,
    CapabilityAvailability, CapabilityKind, CapabilityMetadata, CapabilityProvenance,
    CapabilityUses, ProductKernelCapabilityDescriptor, ScheduleCadence, ScheduleComposition,
};
use runtime_lifecycle::{
    ExternalStep, RuntimeInstanceId, RuntimeLifecycle, RuntimeLifecycleConfig,
};
use runtime_schedule::{
    CompiledRuntimeSchedule, RuntimeScheduleError, ScheduleDispatcher, ScheduleSystemInvocation,
};

const MANIFEST: &str = include_str!("../../../../fixtures/product-model/minimum.rusty.toml");
const COMPOSITION: &[u8] =
    include_bytes!("../../../../fixtures/product-model/minimum.compiled-composition.json");

fn linked() -> product_model::LinkedProductComposition {
    let composition = decode_compiled_composition(COMPOSITION).expect("fixture composition");
    link_composition(composition)
}

fn linked_with_cadence(
    every_steps: u32,
    offset_steps: u32,
) -> product_model::LinkedProductComposition {
    let checked = decode_compiled_composition(COMPOSITION).expect("fixture composition");
    let mut candidate = checked.candidate().clone();
    match &mut candidate.schedule[1].composition {
        ScheduleComposition::Append { systems } => {
            systems[0].cadence = ScheduleCadence::new(every_steps, offset_steps);
        }
        _ => panic!("fixture simulation phase must append"),
    }
    link_composition(validate_compiled_composition(candidate).expect("cadence composition"))
}

fn link_composition(
    composition: product_model::CompiledComposition,
) -> product_model::LinkedProductComposition {
    let manifest = decode_product_manifest(MANIFEST).expect("fixture manifest");
    let admitted = admit_checked_product_composition(&manifest, composition).expect("admission");
    link_admitted_product_composition(admitted, &kernel_capabilities()).expect("linkage")
}

fn kernel_capabilities() -> [ProductKernelCapabilityDescriptor; 3] {
    [
        ProductKernelCapabilityDescriptor::new(
            "camera-look",
            CapabilityMetadata::new(
                CapabilityKind::System,
                CapabilityUses::INPUT_MAP,
                CapabilityAvailability::Linkable,
                CapabilityAccess::new(&[], &[]),
                product_model::CapabilityBudget::new(1_024),
                CapabilityProvenance::new(
                    "example.product.kernel",
                    "kernel/src/input.rs",
                    "camera_look",
                ),
            ),
        ),
        ProductKernelCapabilityDescriptor::new(
            "apply-movement",
            CapabilityMetadata::new(
                CapabilityKind::System,
                CapabilityUses::SCHEDULE,
                CapabilityAvailability::Linkable,
                CapabilityAccess::new(&["input.motion", "state.transform"], &["state.transform"]),
                product_model::CapabilityBudget::new(1_024),
                CapabilityProvenance::new(
                    "example.product.kernel",
                    "kernel/src/movement.rs",
                    "apply_movement",
                ),
            ),
        ),
        ProductKernelCapabilityDescriptor::new(
            "start-timeline",
            CapabilityMetadata::new(
                CapabilityKind::Operation,
                CapabilityUses::TIMELINE,
                CapabilityAvailability::Linkable,
                CapabilityAccess::new(&[], &[]),
                product_model::CapabilityBudget::new(1_024),
                CapabilityProvenance::new(
                    "example.product.kernel",
                    "kernel/src/timeline.rs",
                    "start_timeline",
                ),
            ),
        ),
    ]
}

fn demand_schedule() -> (
    RuntimeLifecycle,
    runtime_schedule::RuntimeSchedule,
    runtime_lifecycle::SimulationStepAdmission,
) {
    let linked = linked();
    let compiled = CompiledRuntimeSchedule::compile(&linked).expect("schedule compile");
    let mut lifecycle =
        RuntimeLifecycle::new(RuntimeInstanceId::new(11), RuntimeLifecycleConfig::Demand);
    lifecycle.start().expect("start");
    let schedule = compiled.bind(&lifecycle).expect("bind");
    let admission = lifecycle.admit_demand_step().expect("step admission");
    (lifecycle, schedule, admission.step_at(0).expect("step"))
}

fn execute_all_phases(
    lifecycle: &RuntimeLifecycle,
    schedule: &mut runtime_schedule::RuntimeSchedule,
    admission: runtime_lifecycle::SimulationStepAdmission,
    dispatcher: &mut impl ScheduleDispatcher<(), Output = String, Error = &'static str>,
) -> Result<Vec<String>, RuntimeScheduleError<&'static str>> {
    let phases = admission.phases();
    let mut outputs = Vec::new();
    for token in [
        phases.input_snapshot(),
        phases.schedule(),
        phases.timeline(),
        phases.mutation(),
        phases.projection(),
    ] {
        outputs.extend(
            schedule
                .execute_phase(lifecycle, token, &(), dispatcher)?
                .into_outputs(),
        );
    }
    Ok(outputs)
}

#[test]
fn compiles_five_phases_and_prints_stable_bounded_inspection() {
    let linked = linked();
    let first = CompiledRuntimeSchedule::compile(&linked).expect("compile");
    let second = CompiledRuntimeSchedule::compile(&linked).expect("compile again");
    assert_eq!(first, second);
    assert_eq!(first.phases().len(), 5);
    assert!(first
        .phase(product_model::SchedulePhase::Input)
        .final_order()
        .is_empty());
    assert_eq!(
        first
            .phase(product_model::SchedulePhase::Simulation)
            .final_order(),
        ["movement"]
    );
    assert_eq!(
        first.inspection().phases()[0].final_order(),
        ["Standard.input"]
    );
    assert_eq!(
        first.inspection().phases()[1].final_order(),
        ["Standard.simulation", "movement"]
    );
    assert_eq!(first.inspection().phases()[1].systems()[0].final_index(), 1);
    let bytes = first.inspection_json_newline().expect("inspection JSON");
    assert!(bytes.ends_with(b"\n"));
    assert!(!bytes.windows(7).any(|window| window == b"version"));
    assert!(!bytes.windows(6).any(|window| window == b"schema"));
    assert_eq!(bytes, second.inspection_json_newline().expect("same JSON"));
    assert!(bytes.len() < 1_048_576);
}

#[test]
fn initial_bind_rejects_lifecycle_that_already_admitted_steps() {
    let compiled = CompiledRuntimeSchedule::compile(&linked()).expect("schedule compile");
    let mut lifecycle =
        RuntimeLifecycle::new(RuntimeInstanceId::new(31), RuntimeLifecycleConfig::Demand);
    lifecycle.start().expect("start");
    lifecycle.admit_demand_step().expect("admit step");

    assert_eq!(
        compiled.bind(&lifecycle).unwrap_err(),
        RuntimeScheduleError::LifecycleAlreadyAdvanced { admitted_steps: 1 }
    );
}

#[test]
fn executes_in_exact_phase_order_and_only_due_systems() {
    let (lifecycle, mut schedule, admission) = demand_schedule();
    let mut seen = Vec::new();
    let mut dispatcher =
        |invocation: ScheduleSystemInvocation<'_>, _: &()| -> Result<String, &'static str> {
            seen.push((
                invocation.phase(),
                invocation.step().value(),
                invocation.system_id().to_owned(),
            ));
            Ok(invocation.system_id().to_owned())
        };
    let outputs = execute_all_phases(&lifecycle, &mut schedule, admission, &mut dispatcher)
        .expect("all phases");
    assert_eq!(outputs, ["movement", "render-projection"]);
    assert_eq!(
        seen,
        [
            (
                product_model::SchedulePhase::Simulation,
                0,
                "movement".to_owned()
            ),
            (
                product_model::SchedulePhase::Projection,
                0,
                "render-projection".to_owned()
            ),
        ]
    );
    assert_eq!(schedule.last_completed_step().unwrap().value(), 0);
    assert_eq!(
        schedule.next_phase(),
        Some(product_model::SchedulePhase::Input)
    );
}

#[test]
fn failed_dispatch_does_not_advance_phase_or_publish_partial_outputs() {
    let (lifecycle, mut schedule, admission) = demand_schedule();
    let phases = admission.phases();
    let mut calls = 0;
    let mut dispatcher =
        |_: ScheduleSystemInvocation<'_>, _: &()| -> Result<String, &'static str> {
            calls += 1;
            if calls == 1 {
                Err("intentional")
            } else {
                Ok("ok".to_owned())
            }
        };
    let error = schedule
        .execute_phase(&lifecycle, phases.input_snapshot(), &(), &mut dispatcher)
        .expect("empty input phase succeeds");
    assert!(error.outputs().is_empty());
    assert_eq!(
        schedule.next_phase(),
        Some(product_model::SchedulePhase::Simulation)
    );
    assert_eq!(
        schedule
            .execute_phase(&lifecycle, phases.schedule(), &(), &mut dispatcher)
            .unwrap_err(),
        RuntimeScheduleError::Dispatch("intentional")
    );
    assert_eq!(
        schedule.next_phase(),
        Some(product_model::SchedulePhase::Simulation)
    );
    let receipt = schedule
        .execute_phase(&lifecycle, phases.schedule(), &(), &mut dispatcher)
        .expect("retry");
    assert_eq!(receipt.outputs(), ["ok"]);
    assert_eq!(
        schedule.next_phase(),
        Some(product_model::SchedulePhase::Consequences)
    );
}

#[test]
fn rejects_wrong_order_stale_foreign_and_disposed_tokens() {
    let (lifecycle, mut schedule, admission) = demand_schedule();
    let phases = admission.phases();
    let mut dispatcher = |_: ScheduleSystemInvocation<'_>, _: &()| -> Result<(), ()> { Ok(()) };
    let error = schedule
        .execute_phase(&lifecycle, phases.schedule(), &(), &mut dispatcher)
        .unwrap_err();
    assert!(matches!(
        error,
        RuntimeScheduleError::PhaseOutOfOrder { .. }
    ));

    let mut lifecycle = lifecycle;
    for token in [
        phases.input_snapshot(),
        phases.schedule(),
        phases.timeline(),
        phases.mutation(),
        phases.projection(),
    ] {
        schedule
            .execute_phase(&lifecycle, token, &(), &mut dispatcher)
            .expect("initial step");
    }
    assert_eq!(schedule.last_completed_step().unwrap().value(), 0);
    assert_eq!(schedule.next_expected_step(), Some(1));
    assert_eq!(schedule.invalidated_admission_count(), 0);
    lifecycle.pause().expect("pause");
    lifecycle.resume().expect("resume");
    let error = schedule
        .execute_phase(&lifecycle, phases.input_snapshot(), &(), &mut dispatcher)
        .unwrap_err();
    assert!(matches!(error, RuntimeScheduleError::Lifecycle(_)));

    schedule
        .rebind(&lifecycle)
        .expect("rebind after pause/resume");
    assert_eq!(schedule.next_expected_step(), Some(1));
    assert_eq!(schedule.invalidated_admission_count(), 0);
    let continued_admission = lifecycle.admit_demand_step().expect("continued step");
    let continued_step = continued_admission.step_at(0).unwrap();
    for token in [
        continued_step.phases().input_snapshot(),
        continued_step.phases().schedule(),
        continued_step.phases().timeline(),
        continued_step.phases().mutation(),
        continued_step.phases().projection(),
    ] {
        schedule
            .execute_phase(&lifecycle, token, &(), &mut dispatcher)
            .expect("rebound schedule accepts continued step");
    }
    assert_eq!(schedule.last_completed_step().unwrap().value(), 1);

    lifecycle
        .restart()
        .expect("restart creates a new generation");
    let fresh_admission = lifecycle.admit_demand_step().expect("fresh step");
    let fresh_step = fresh_admission.step_at(0).unwrap();
    schedule.rebind(&lifecycle).expect("rebind after restart");
    for token in [
        fresh_step.phases().input_snapshot(),
        fresh_step.phases().schedule(),
        fresh_step.phases().timeline(),
        fresh_step.phases().mutation(),
        fresh_step.phases().projection(),
    ] {
        schedule
            .execute_phase(&lifecycle, token, &(), &mut dispatcher)
            .expect("rebound schedule accepts the new generation");
    }
    assert_eq!(schedule.last_completed_step().unwrap().value(), 0);

    let mut foreign =
        RuntimeLifecycle::new(RuntimeInstanceId::new(12), RuntimeLifecycleConfig::Demand);
    foreign.start().expect("foreign start");
    let foreign_step = foreign.admit_demand_step().expect("foreign step");
    let error = schedule
        .execute_phase(
            &foreign,
            foreign_step.step_at(0).unwrap().phases().input_snapshot(),
            &(),
            &mut dispatcher,
        )
        .unwrap_err();
    assert_eq!(error, RuntimeScheduleError::LifecycleBindingMismatch);

    let active_admission = lifecycle.admit_demand_step().expect("active step");
    let active_step = active_admission.step_at(0).unwrap();
    schedule
        .execute_phase(
            &lifecycle,
            active_step.phases().input_snapshot(),
            &(),
            &mut dispatcher,
        )
        .expect("input phase starts active progression");
    lifecycle.pause().expect("pause active lane");
    lifecycle.resume().expect("resume active lane");
    assert!(matches!(
        schedule.rebind(&lifecycle),
        Err(RuntimeScheduleError::RebindActiveStep { step: 1 })
    ));

    lifecycle
        .restart()
        .expect("restart abandons the interrupted phase chain");
    assert!(matches!(
        schedule.rebind(&lifecycle),
        Err(RuntimeScheduleError::RebindActiveStep { step: 1 })
    ));
    let mut fresh_schedule = schedule
        .compiled()
        .clone()
        .bind(&lifecycle)
        .expect("new generation accepts a fresh lane");
    let fresh_admission = lifecycle.admit_demand_step().expect("fresh step");
    let fresh_step = fresh_admission.step_at(0).expect("fresh simulation step");
    for token in [
        fresh_step.phases().input_snapshot(),
        fresh_step.phases().schedule(),
        fresh_step.phases().timeline(),
        fresh_step.phases().mutation(),
        fresh_step.phases().projection(),
    ] {
        fresh_schedule
            .execute_phase(&lifecycle, token, &(), &mut dispatcher)
            .expect("fresh lane accepts the restarted step");
    }
    assert_eq!(fresh_schedule.last_completed_step().unwrap().value(), 0);

    schedule.dispose();
    let mut lifecycle =
        RuntimeLifecycle::new(RuntimeInstanceId::new(13), RuntimeLifecycleConfig::Demand);
    lifecycle.start().expect("new start");
    let step = lifecycle.admit_demand_step().unwrap().step_at(0).unwrap();
    let error = schedule
        .execute_phase(
            &lifecycle,
            step.phases().input_snapshot(),
            &(),
            &mut dispatcher,
        )
        .unwrap_err();
    assert_eq!(error, RuntimeScheduleError::Disposed);
}

#[test]
fn rebind_reconciles_admitted_unstarted_steps_as_invalidated() {
    let linked = linked();
    let compiled = CompiledRuntimeSchedule::compile(&linked).expect("schedule compile");
    let mut lifecycle =
        RuntimeLifecycle::new(RuntimeInstanceId::new(41), RuntimeLifecycleConfig::Demand);
    lifecycle.start().expect("start");
    let mut schedule = compiled.bind(&lifecycle).expect("bind");
    lifecycle.admit_demand_step().expect("admit step zero");

    lifecycle.pause().expect("pause");
    lifecycle.resume().expect("resume");
    schedule.rebind(&lifecycle).expect("rebind stale admission");
    assert_eq!(schedule.last_completed_step(), None);
    assert_eq!(schedule.next_expected_step(), Some(1));
    assert_eq!(schedule.invalidated_admission_count(), 1);

    let admission = lifecycle.admit_demand_step().expect("admit step one");
    let step = admission.step_at(0).expect("step one");
    let mut dispatcher = |_: ScheduleSystemInvocation<'_>, _: &()| -> Result<(), ()> { Ok(()) };
    for token in [
        step.phases().input_snapshot(),
        step.phases().schedule(),
        step.phases().timeline(),
        step.phases().mutation(),
        step.phases().projection(),
    ] {
        schedule
            .execute_phase(&lifecycle, token, &(), &mut dispatcher)
            .expect("reconciled lane accepts next admission");
    }
    assert_eq!(schedule.last_completed_step().unwrap().value(), 1);
    assert_eq!(schedule.next_expected_step(), Some(2));
    assert_eq!(schedule.invalidated_admission_count(), 1);
}

#[test]
fn rebind_reconciles_multiple_stale_admissions_without_marking_them_complete() {
    let linked = linked();
    let compiled = CompiledRuntimeSchedule::compile(&linked).expect("schedule compile");
    let mut lifecycle =
        RuntimeLifecycle::new(RuntimeInstanceId::new(42), RuntimeLifecycleConfig::Demand);
    lifecycle.start().expect("start");
    let mut schedule = compiled.bind(&lifecycle).expect("bind");
    lifecycle.admit_demand_step().expect("admit step zero");
    lifecycle.admit_demand_step().expect("admit step one");

    lifecycle.pause().expect("pause");
    lifecycle.resume().expect("resume");
    schedule
        .rebind(&lifecycle)
        .expect("rebind stale admissions");
    let readout = schedule.readout();
    assert_eq!(readout.last_completed_step(), None);
    assert_eq!(readout.next_expected_step(), Some(2));
    assert_eq!(readout.invalidated_admission_count(), 2);

    let admission = lifecycle.admit_demand_step().expect("admit step two");
    let step = admission.step_at(0).expect("step two");
    let mut dispatcher = |_: ScheduleSystemInvocation<'_>, _: &()| -> Result<(), ()> { Ok(()) };
    for token in [
        step.phases().input_snapshot(),
        step.phases().schedule(),
        step.phases().timeline(),
        step.phases().mutation(),
        step.phases().projection(),
    ] {
        schedule
            .execute_phase(&lifecycle, token, &(), &mut dispatcher)
            .expect("reconciled lane accepts catch-up admission");
    }
    assert_eq!(schedule.last_completed_step().unwrap().value(), 2);
    assert_eq!(schedule.next_expected_step(), Some(3));
    assert_eq!(schedule.invalidated_admission_count(), 2);
}

#[test]
fn external_tokens_and_cadence_keep_step_determinism_without_a_clock() {
    let linked = linked_with_cadence(2, 1);
    let compiled = CompiledRuntimeSchedule::compile(&linked).unwrap();
    let mut lifecycle =
        RuntimeLifecycle::new(RuntimeInstanceId::new(21), RuntimeLifecycleConfig::External);
    lifecycle.start().unwrap();
    let mut schedule = compiled.bind(&lifecycle).unwrap();
    let mut seen = Vec::new();
    let mut dispatcher = |invocation: ScheduleSystemInvocation<'_>, _: &()| -> Result<(), ()> {
        seen.push((invocation.step().value(), invocation.system_id().to_owned()));
        Ok(())
    };
    for value in [0, 1, 2] {
        let admission = lifecycle
            .admit_external_step(ExternalStep::new(value))
            .unwrap();
        let step = admission.step_at(0).unwrap();
        for token in [
            step.phases().input_snapshot(),
            step.phases().schedule(),
            step.phases().timeline(),
            step.phases().mutation(),
            step.phases().projection(),
        ] {
            schedule
                .execute_phase(&lifecycle, token, &(), &mut dispatcher)
                .unwrap();
        }
    }
    assert_eq!(
        seen,
        [
            (0, "render-projection".to_owned()),
            (1, "movement".to_owned()),
            (1, "render-projection".to_owned()),
            (2, "render-projection".to_owned()),
        ]
    );
}
