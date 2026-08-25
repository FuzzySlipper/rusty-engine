use std::collections::BTreeMap;

use core_ids::EntityId;
use core_math::Vec3;
use engine_spatial::VoxelCollisionScene;
use entity_state::{
    ComponentRegistration, ComponentTypeId, EntityAuthoringService, EntityCommand,
    EntityCommandBatch, EntityComponent, EntityDefinition, EntityState,
};
use product_model::{
    admit_checked_product_composition, decode_compiled_composition, decode_product_manifest,
    link_admitted_product_composition, CapabilityAccess, CapabilityAvailability, CapabilityBudget,
    CapabilityKind, CapabilityMetadata, CapabilityProvenance, CapabilityUses,
    ProductKernelCapabilityDescriptor, SchedulePhase,
};
use runtime_lifecycle::{RuntimeInstanceId, RuntimeLifecycle, RuntimeLifecycleConfig};
use runtime_mutation::{
    CompiledMutationCatalog, MutationAuthority, MutationBatchId, MutationCapabilityDescriptor,
    MutationCausation, MutationOperationId, MutationOwnerEvidence, MutationPlanner,
    MutationProvenance, MutationStage, RuntimeMutation, RuntimeMutationError,
};
use runtime_schedule::{CompiledRuntimeSchedule, ScheduleSystemInvocation};
use runtime_standard_capabilities::{
    ObservePairsBatchIdentity, ObservePairsError, ObservePairsObserver, ObservePairsObserverFacts,
    ObservePairsPlan, ObservePairsTarget, OBSERVE_PAIRS_RESULT_KIND,
};

const MANIFEST: &str = include_str!("../../../../fixtures/product-model/minimum.rusty.toml");
const COMPOSITION: &[u8] = include_bytes!(
    "../../../../fixtures/runtime-standard-capabilities/stealth.observe-pairs.compiled-composition.json"
);

const OBSERVER: EntityId = EntityId::new(10);
const DISABLED_OBSERVER: EntityId = EntityId::new(11);
const CLEAR_TARGET: EntityId = EntityId::new(20);
const HIDDEN_RENDER_TARGET: EntityId = EntityId::new(21);
const BLOCKED_TARGET: EntityId = EntityId::new(22);
const OUTSIDE_DISTANCE: EntityId = EntityId::new(23);
const OUTSIDE_CONE: EntityId = EntityId::new(24);
const DISABLED_TARGET: EntityId = EntityId::new(25);
const ROLE_LIKE_ONLY: EntityId = EntityId::new(26);
const OCCLUDER: EntityId = EntityId::new(30);

#[derive(Debug, Clone, Copy, PartialEq)]
struct Vision {
    facts: ObservePairsObserverFacts,
}

impl EntityComponent for Vision {}

impl ObservePairsObserver for Vision {
    fn facts(&self) -> ObservePairsObserverFacts {
        self.facts
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct StealthTarget {
    center: Vec3,
}

impl EntityComponent for StealthTarget {}

impl ObservePairsTarget for StealthTarget {
    fn local_center(&self) -> Vec3 {
        self.center
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AlertAuthority {
    revision: u64,
    alert_levels: BTreeMap<u64, u64>,
}

impl MutationAuthority for AlertAuthority {
    type Guard = (u64, BTreeMap<u64, u64>);

    fn guard(&self) -> Self::Guard {
        (self.revision, self.alert_levels.clone())
    }

    fn publication_domain(&self) -> &str {
        "stealth.alert-state"
    }
}

#[derive(Debug, Default)]
struct AlertPlanner {
    calls: usize,
    reject: bool,
}

impl MutationPlanner<AlertAuthority, &'static str> for AlertPlanner {
    type Error = &'static str;

    fn stage(
        &mut self,
        authority: &AlertAuthority,
        batch: &runtime_mutation::MutationResolvedBatch,
    ) -> Result<MutationStage<AlertAuthority, &'static str>, Self::Error> {
        self.calls += 1;
        if self.reject {
            return Err("product kernel rejected alert evidence");
        }
        let mut candidate = authority.clone();
        for operation in batch.operations() {
            assert_eq!(operation.owner(), "stealth.product-kernel");
            assert_eq!(operation.payload()["kind"], OBSERVE_PAIRS_RESULT_KIND);
            for result in operation.payload()["results"]
                .as_array()
                .expect("fixed result array")
            {
                let target = result["target"].as_u64().expect("target identity");
                let evidence = result["evidenceTotal"].as_f64().expect("finite evidence");
                // Alert thresholds and state meaning live only in this test Product Kernel owner.
                if evidence >= 0.5 {
                    *candidate.alert_levels.entry(target).or_default() += 1;
                }
            }
        }
        candidate.revision += 1;
        let evidence = batch
            .operations()
            .iter()
            .map(|operation| MutationOwnerEvidence::for_operation(operation, "advance-alert"))
            .collect();
        Ok(MutationStage::new(candidate, evidence))
    }
}

fn linked() -> product_model::LinkedProductComposition {
    let composition = decode_compiled_composition(COMPOSITION).expect("stealth composition");
    let manifest_source = MANIFEST.replace("example.product", "stealth.pressure");
    let manifest = decode_product_manifest(&manifest_source).expect("manifest");
    let admitted =
        admit_checked_product_composition(&manifest, composition).expect("composition admission");
    link_admitted_product_composition(admitted, &[kernel_capability()]).expect("linkage")
}

fn kernel_capability() -> ProductKernelCapabilityDescriptor {
    ProductKernelCapabilityDescriptor::new(
        "advance-alert",
        CapabilityMetadata::new(
            CapabilityKind::Operation,
            CapabilityUses::TIMELINE,
            CapabilityAvailability::Linkable,
            CapabilityAccess::new(&[], &[]),
            CapabilityBudget::new(16_384),
            CapabilityProvenance::new(
                "stealth.product-kernel",
                "product-kernel/alert.rs",
                "advance_alert",
            ),
        ),
    )
}

fn compile_plan() -> (
    product_model::LinkedProductComposition,
    CompiledRuntimeSchedule,
    CompiledMutationCatalog,
    ObservePairsPlan,
) {
    let linked = linked();
    let schedule = CompiledRuntimeSchedule::compile(&linked).expect("schedule");
    let mutations = CompiledMutationCatalog::compile(
        &linked,
        &[MutationCapabilityDescriptor::new(
            "stealth.advance-alert",
            "kernel.advance-alert",
            "stealth.alert-state",
            "stealth.product-kernel",
            OBSERVE_PAIRS_RESULT_KIND,
        )],
    )
    .expect("mutation catalog");
    let system = schedule
        .phase(SchedulePhase::Simulation)
        .systems()
        .iter()
        .find(|system| system.id() == "stealth.detect")
        .expect("stealth system");
    let plan = ObservePairsPlan::compile_system(system, &mutations)
        .expect("runtime-composition retained artifacts compile the observe plan");
    (linked, schedule, mutations, plan)
}

fn component_id(value: &str) -> ComponentTypeId {
    ComponentTypeId::parse(value).expect("component identity")
}

fn attach<T: EntityComponent>(state: &mut EntityState, entity: EntityId, component: T) {
    let revision = state
        .component_revision::<T>(entity)
        .expect("registered component revision");
    EntityAuthoringService
        .attach_component(state, revision, entity, component)
        .expect("attach typed fact");
}

fn entities(reverse_definitions: bool) -> EntityState {
    let mut definitions = vec![
        EntityDefinition::new(OBSERVER, "observer").with_transform(Vec3::ZERO),
        EntityDefinition::new(DISABLED_OBSERVER, "disabled observer")
            .with_transform(Vec3::new(0.0, 0.0, 1.0)),
        EntityDefinition::new(CLEAR_TARGET, "clear target")
            .with_transform(Vec3::new(4.0, 0.0, 2.0)),
        EntityDefinition::new(HIDDEN_RENDER_TARGET, "hidden render target")
            .with_transform(Vec3::new(5.0, 0.0, -2.0))
            .with_renderable("mesh/target", false),
        EntityDefinition::new(BLOCKED_TARGET, "blocked target")
            .with_transform(Vec3::new(6.0, 0.0, 0.0)),
        EntityDefinition::new(OUTSIDE_DISTANCE, "distant target")
            .with_transform(Vec3::new(20.0, 0.0, 0.0)),
        EntityDefinition::new(OUTSIDE_CONE, "behind observer")
            .with_transform(Vec3::new(-4.0, 0.0, 0.0)),
        EntityDefinition::new(DISABLED_TARGET, "disabled target")
            .with_transform(Vec3::new(3.0, 0.0, 3.0)),
        EntityDefinition::new(ROLE_LIKE_ONLY, "stealth.target but untyped")
            .with_transform(Vec3::new(2.0, 0.0, 2.0)),
        EntityDefinition::new(OCCLUDER, "active hidden collider")
            .with_transform(Vec3::new(3.0, 0.0, 0.0))
            .with_bounds(Vec3::splat(-0.5), Vec3::splat(0.5))
            .with_collision(true, true)
            .with_renderable("mesh/door", false),
    ];
    if reverse_definitions {
        definitions.reverse();
    }
    let mut state = EntityState::from_definitions(definitions).expect("entity facts");
    state
        .register_component(ComponentRegistration::<Vision>::runtime_only(component_id(
            "stealth.vision",
        )))
        .expect("vision registration");
    state
        .register_component(ComponentRegistration::<StealthTarget>::runtime_only(
            component_id("stealth.target"),
        ))
        .expect("target registration");
    let facts = Vision {
        facts: ObservePairsObserverFacts {
            local_origin: Vec3::ZERO,
            local_forward: Vec3::new(1.0, 0.0, 0.0),
            maximum_distance: 10.0,
            minimum_facing_cosine: 0.5,
            evidence: 0.75,
        },
    };
    attach(&mut state, OBSERVER, facts);
    attach(&mut state, DISABLED_OBSERVER, facts);
    for target in [
        CLEAR_TARGET,
        HIDDEN_RENDER_TARGET,
        BLOCKED_TARGET,
        OUTSIDE_DISTANCE,
        OUTSIDE_CONE,
        DISABLED_TARGET,
    ] {
        attach(&mut state, target, StealthTarget { center: Vec3::ZERO });
    }
    let service = EntityAuthoringService;
    let revision = state.revision();
    service
        .disable(&mut state, revision, DISABLED_OBSERVER)
        .expect("disable observer");
    let revision = state.revision();
    service
        .disable(&mut state, revision, DISABLED_TARGET)
        .expect("disable target");
    state
}

fn empty_scene() -> VoxelCollisionScene {
    VoxelCollisionScene::from_solid_voxels(1.0, 8, []).expect("empty canonical scene")
}

#[test]
fn cadence_six_runs_steps_zero_six_twelve_and_publishes_through_product_kernel() {
    let (_, compiled_schedule, catalog, plan) = compile_plan();
    assert_eq!(plan.cadence_every_steps(), 6);
    assert_eq!(plan.cadence_offset_steps(), 0);
    let entities = entities(false);
    let scene = empty_scene();
    let mut lifecycle =
        RuntimeLifecycle::new(RuntimeInstanceId::new(7259), RuntimeLifecycleConfig::Demand);
    lifecycle.start().expect("start lifecycle");
    let mut schedule = compiled_schedule.bind(&lifecycle).expect("bind schedule");
    let mut mutations = RuntimeMutation::<AlertAuthority, &'static str>::bind(catalog, &lifecycle)
        .expect("bind mutations");
    let mut authority = AlertAuthority {
        revision: 0,
        alert_levels: BTreeMap::new(),
    };
    let mut planner = AlertPlanner::default();
    let mut due_steps = Vec::new();

    for expected_step in 0..=12 {
        let admission = lifecycle
            .admit_demand_step()
            .expect("admit step")
            .step_at(0)
            .expect("one demand step");
        assert_eq!(admission.token().step().value(), expected_step);
        let phases = admission.phases();
        let mut dispatcher = |invocation: ScheduleSystemInvocation<'_>, _: &()| {
            assert_eq!(invocation.system_id(), "stealth.detect");
            plan.evaluate_and_batch::<Vision, StealthTarget>(
                &entities,
                &scene,
                ObservePairsBatchIdentity {
                    batch_id: MutationBatchId::new(format!("stealth-step-{expected_step}"))
                        .expect("batch id"),
                    causation: MutationCausation::new("runtime-schedule").expect("causation"),
                    provenance: MutationProvenance::new("stealth.product-kernel")
                        .expect("provenance"),
                    operation_id: MutationOperationId::new(expected_step),
                },
            )
            .map(|emission| emission.batch)
        };
        schedule
            .execute_phase(&lifecycle, phases.input_snapshot(), &(), &mut dispatcher)
            .expect("input phase");
        let batches = schedule
            .execute_phase(&lifecycle, phases.schedule(), &(), &mut dispatcher)
            .expect("simulation phase")
            .into_outputs();
        schedule
            .execute_phase(&lifecycle, phases.timeline(), &(), &mut dispatcher)
            .expect("consequences phase");
        schedule
            .execute_phase(&lifecycle, phases.mutation(), &(), &mut dispatcher)
            .expect("commit phase");
        match batches.as_slice() {
            [batch] => {
                due_steps.push(expected_step);
                let calls_before = planner.calls;
                let receipt = mutations
                    .apply_batch(
                        &lifecycle,
                        phases.mutation(),
                        &mut authority,
                        &mut planner,
                        batch.clone(),
                    )
                    .expect("guarded Product Kernel publication");
                assert_eq!(receipt.operations().len(), 1);
                assert_eq!(receipt.operations()[0].owner(), "stealth.product-kernel");
                assert_eq!(
                    receipt.operations()[0].provenance_source(),
                    "product-kernel/alert.rs"
                );
                assert_eq!(receipt.operations()[0].kind(), "operation");
                if expected_step == 0 {
                    let retry = mutations
                        .apply_batch(
                            &lifecycle,
                            phases.mutation(),
                            &mut authority,
                            &mut planner,
                            batch.clone(),
                        )
                        .expect("exact retained retry");
                    assert_eq!(retry, receipt);
                    assert_eq!(planner.calls, calls_before + 1);
                    let conflicting = plan
                        .mutation_batch(
                            &plan
                                .evaluate::<Vision, StealthTarget>(&entities, &scene)
                                .expect("same readout"),
                            MutationBatchId::new(batch.id().as_str()).expect("same batch id"),
                            MutationCausation::new("conflicting-causation").unwrap(),
                            MutationProvenance::new("stealth.product-kernel").unwrap(),
                            MutationOperationId::new(expected_step),
                        )
                        .unwrap();
                    assert!(matches!(
                        mutations.apply_batch(
                            &lifecycle,
                            phases.mutation(),
                            &mut authority,
                            &mut planner,
                            conflicting
                        ),
                        Err(RuntimeMutationError::BatchIdentityConflict { .. })
                    ));
                    assert_eq!(planner.calls, calls_before + 1);
                }
            }
            [] => {
                let completion = mutations
                    .complete_empty_step(&lifecycle, phases.mutation())
                    .expect("explicit empty mutation step");
                assert_eq!(completion.step().value(), expected_step);
            }
            _ => panic!("closed schedule emits at most one stealth batch"),
        }
        schedule
            .execute_phase(&lifecycle, phases.projection(), &(), &mut dispatcher)
            .expect("projection phase");
    }

    assert_eq!(due_steps, [0, 6, 12]);
    assert_eq!(planner.calls, 3);
    assert_eq!(authority.revision, 3);
    assert_eq!(authority.alert_levels.len(), 2);
    assert_eq!(authority.alert_levels[&CLEAR_TARGET.raw()], 3);
    assert_eq!(authority.alert_levels[&HIDDEN_RENDER_TARGET.raw()], 3);
    assert_eq!(
        schedule.last_completed_step().expect("completed").value(),
        12
    );
    assert_eq!(
        mutations
            .readout()
            .last_completed_step()
            .expect("mutation completion")
            .value(),
        12
    );
}

#[test]
fn typed_selection_geometry_occlusion_and_order_are_bounded_and_deterministic() {
    let (_, _, _, plan) = compile_plan();
    let scene = empty_scene();
    let first_state = entities(false);
    let second_state = entities(true);
    let first = plan
        .evaluate::<Vision, StealthTarget>(&first_state, &scene)
        .expect("first evaluation");
    let second = plan
        .evaluate::<Vision, StealthTarget>(&second_state, &scene)
        .expect("reordered evaluation");
    assert_eq!(first, second);
    assert_eq!(first.selected_observers, 1);
    assert_eq!(first.selected_targets, 5);
    assert_eq!(first.selection_comparisons, 5);
    assert_eq!(first.pairs_examined, 4);
    assert_eq!(first.distance_rejects, 1);
    assert_eq!(first.facing_rejects, 1);
    assert_eq!(first.visibility_casts, 3);
    assert_eq!(first.occlusion_rejects, 1);
    assert_eq!(first.visible_pairs, 2);
    assert_eq!(
        first
            .aggregates
            .iter()
            .map(|aggregate| aggregate.target)
            .collect::<Vec<_>>(),
        [CLEAR_TARGET, HIDDEN_RENDER_TARGET]
    );

    let first_batch = plan
        .mutation_batch(
            &first,
            MutationBatchId::new("order-proof").unwrap(),
            MutationCausation::new("fixture").unwrap(),
            MutationProvenance::new("fixture").unwrap(),
            MutationOperationId::new(1),
        )
        .unwrap();
    let second_batch = plan
        .mutation_batch(
            &second,
            MutationBatchId::new("order-proof").unwrap(),
            MutationCausation::new("fixture").unwrap(),
            MutationProvenance::new("fixture").unwrap(),
            MutationOperationId::new(1),
        )
        .unwrap();
    assert_eq!(first_batch.fingerprint(), second_batch.fingerprint());

    let mut no_occluder = first_state.clone();
    no_occluder
        .apply_batch(EntityCommandBatch::new([
            EntityCommand::SetCollisionEnabled {
                entity: OCCLUDER,
                enabled: false,
            },
        ]))
        .expect("disable collider");
    let unblocked = plan
        .evaluate::<Vision, StealthTarget>(&no_occluder, &scene)
        .expect("disabled collider is not authority");
    assert_eq!(unblocked.occlusion_rejects, 0);
    assert_eq!(unblocked.visible_pairs, 3);
}

#[test]
fn invalid_fact_and_closed_payload_fail_before_any_mutation() {
    let (_, compiled_schedule, catalog, plan) = compile_plan();
    let mut invalid = entities(false);
    let revision = invalid
        .component_revision::<Vision>(OBSERVER)
        .expect("vision revision");
    let current_facts = invalid
        .component::<Vision>(OBSERVER)
        .unwrap()
        .unwrap()
        .facts;
    EntityAuthoringService
        .replace_component(
            &mut invalid,
            revision,
            OBSERVER,
            Vision {
                facts: ObservePairsObserverFacts {
                    local_forward: Vec3::ZERO,
                    ..current_facts
                },
            },
        )
        .expect("install invalid test fact");
    let stable_revision = invalid.revision();
    assert!(matches!(
        plan.evaluate::<Vision, StealthTarget>(&invalid, &empty_scene()),
        Err(ObservePairsError::InvalidObserverFacts(entity)) if entity == OBSERVER
    ));
    assert_eq!(invalid.revision(), stable_revision);

    let mut value: serde_json::Value = serde_json::from_slice(COMPOSITION).unwrap();
    value["schedule"][1]["systems"][0]["payload"]["visibility"] =
        serde_json::json!({"kind": "arbitrary-volume", "samples": 99});
    let composition = decode_compiled_composition(&serde_json::to_vec(&value).unwrap()).unwrap();
    let manifest_source = MANIFEST.replace("example.product", "stealth.pressure");
    let manifest = decode_product_manifest(&manifest_source).unwrap();
    let admitted = admit_checked_product_composition(&manifest, composition).unwrap();
    let rejected_linked =
        link_admitted_product_composition(admitted, &[kernel_capability()]).unwrap();
    let rejected_schedule = CompiledRuntimeSchedule::compile(&rejected_linked).unwrap();
    let rejected_system = rejected_schedule
        .phase(SchedulePhase::Simulation)
        .systems()
        .first()
        .unwrap();
    assert!(matches!(
        ObservePairsPlan::compile(&rejected_linked, rejected_system, &catalog),
        Err(ObservePairsError::InvalidPlan(_))
    ));

    let mut quota_value: serde_json::Value = serde_json::from_slice(COMPOSITION).unwrap();
    quota_value["schedule"][1]["systems"][0]["payload"]["quotas"]["observers"] =
        serde_json::json!(1);
    let quota_composition =
        decode_compiled_composition(&serde_json::to_vec(&quota_value).unwrap()).unwrap();
    let quota_manifest_source = MANIFEST.replace("example.product", "stealth.pressure");
    let quota_manifest = decode_product_manifest(&quota_manifest_source).unwrap();
    let quota_admitted =
        admit_checked_product_composition(&quota_manifest, quota_composition).unwrap();
    let quota_linked =
        link_admitted_product_composition(quota_admitted, &[kernel_capability()]).unwrap();
    let quota_schedule = CompiledRuntimeSchedule::compile(&quota_linked).unwrap();
    let quota_system = quota_schedule
        .phase(SchedulePhase::Simulation)
        .systems()
        .first()
        .unwrap();
    let quota_plan = ObservePairsPlan::compile(&quota_linked, quota_system, &catalog).unwrap();
    let mut quota_state = entities(false);
    assert_eq!(
        quota_plan
            .evaluate::<Vision, StealthTarget>(&quota_state, &empty_scene())
            .unwrap()
            .selected_observers,
        1
    );
    let quota_revision = quota_state.revision();
    EntityAuthoringService
        .enable(&mut quota_state, quota_revision, DISABLED_OBSERVER)
        .unwrap();
    let stable_revision = quota_state.revision();
    assert!(matches!(
        quota_plan.evaluate::<Vision, StealthTarget>(&quota_state, &empty_scene()),
        Err(ObservePairsError::QuotaExceeded("observers"))
    ));
    assert_eq!(quota_state.revision(), stable_revision);

    let _ = compiled_schedule;
}

#[test]
fn product_kernel_rejection_is_atomic() {
    let (_, compiled_schedule, catalog, plan) = compile_plan();
    let entities = entities(false);
    let readout = plan
        .evaluate::<Vision, StealthTarget>(&entities, &empty_scene())
        .unwrap();
    let batch = plan
        .mutation_batch(
            &readout,
            MutationBatchId::new("rejected-step").unwrap(),
            MutationCausation::new("fixture").unwrap(),
            MutationProvenance::new("fixture").unwrap(),
            MutationOperationId::new(0),
        )
        .unwrap();
    let mut lifecycle =
        RuntimeLifecycle::new(RuntimeInstanceId::new(99), RuntimeLifecycleConfig::Demand);
    lifecycle.start().unwrap();
    let _schedule = compiled_schedule.bind(&lifecycle).unwrap();
    let mut mutations =
        RuntimeMutation::<AlertAuthority, &'static str>::bind(catalog, &lifecycle).unwrap();
    let admission = lifecycle.admit_demand_step().unwrap().step_at(0).unwrap();
    let mut authority = AlertAuthority {
        revision: 0,
        alert_levels: BTreeMap::new(),
    };
    let before = authority.clone();
    let mut planner = AlertPlanner {
        calls: 0,
        reject: true,
    };
    assert!(matches!(
        mutations.apply_batch(
            &lifecycle,
            admission.phases().mutation(),
            &mut authority,
            &mut planner,
            batch
        ),
        Err(RuntimeMutationError::Planner(
            "product kernel rejected alert evidence"
        ))
    ));
    assert_eq!(authority, before);
    assert_eq!(mutations.readout().next_expected_step(), Some(0));
}
