use product_model::{
    admit_checked_product_composition, decode_compiled_composition, decode_product_manifest,
    link_admitted_product_composition, CapabilityAccess, CapabilityAvailability, CapabilityKind,
    CapabilityMetadata, CapabilityProvenance, CapabilityUses, ProductKernelCapabilityDescriptor,
};
use runtime_lifecycle::{RuntimeInstanceId, RuntimeLifecycle, RuntimeLifecycleConfig};
use runtime_mutation::{
    CompiledMutationCatalog, MutationAuthority, MutationBatch, MutationBatchId,
    MutationCapabilityDescriptor, MutationCausation, MutationDataError, MutationOperation,
    MutationOperationId, MutationOwnerEvidence, MutationPlanner, MutationProvenance, MutationStage,
    RuntimeMutation, RuntimeMutationError,
};
use serde_json::json;
use std::{cell::Cell, rc::Rc};

const MANIFEST: &str = include_str!("../../../../fixtures/product-model/minimum.rusty.toml");
const COMPOSITION: &[u8] =
    include_bytes!("../../../../fixtures/product-model/minimum.compiled-composition.json");

fn linked() -> product_model::LinkedProductComposition {
    let composition = decode_compiled_composition(COMPOSITION).expect("composition");
    let manifest = decode_product_manifest(MANIFEST).expect("manifest");
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
                CapabilityProvenance::new("example.product.kernel", "kernel/input.rs", "look"),
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
                CapabilityProvenance::new("example.product.kernel", "kernel/move.rs", "move"),
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
                CapabilityProvenance::new("example.product.kernel", "kernel/timeline.rs", "start"),
            ),
        ),
    ]
}

fn catalog() -> CompiledMutationCatalog {
    CompiledMutationCatalog::compile(
        &linked(),
        &[MutationCapabilityDescriptor::new(
            "timeline.start",
            "kernel.start-timeline",
            "world",
            "product.timeline",
            "product.timeline.start.v1",
        )],
    )
    .expect("mutation catalog")
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct World {
    value: i64,
    revision: u64,
    domain: &'static str,
    guard_source: Rc<Cell<u64>>,
}

impl MutationAuthority for World {
    type Guard = (i64, u64);

    fn guard(&self) -> Self::Guard {
        (self.value, self.revision ^ self.guard_source.get())
    }

    fn publication_domain(&self) -> &str {
        self.domain
    }
}

#[derive(Debug, Default)]
struct Planner {
    calls: usize,
    fail: bool,
    bad_evidence: bool,
    wrong_candidate_domain: bool,
    mutate_during_stage: bool,
}

impl MutationPlanner<World, u32> for Planner {
    type Error = &'static str;

    fn stage(
        &mut self,
        authority: &World,
        batch: &runtime_mutation::MutationResolvedBatch,
    ) -> Result<MutationStage<World, u32>, Self::Error> {
        self.calls += 1;
        if self.mutate_during_stage {
            // Deliberately violates the planner side-effect contract to prove
            // that the final live guard check catches a stale authority.
            authority.guard_source.set(99);
        }
        if self.fail {
            return Err("named owner rejected operation");
        }
        let mut candidate = authority.clone();
        candidate.value += batch.operations().len() as i64;
        candidate.revision += 1;
        if self.wrong_candidate_domain {
            candidate.domain = "other-world";
        }
        let evidence = batch
            .operations()
            .iter()
            .enumerate()
            .map(|(index, operation)| {
                if self.bad_evidence {
                    MutationOwnerEvidence::new(
                        MutationOperationId::new(999),
                        operation.binding_id(),
                        operation.target(),
                        operation.resolved_target(),
                        operation.publication_domain(),
                        operation.owner(),
                        index as u32,
                    )
                } else {
                    MutationOwnerEvidence::for_operation(operation, index as u32)
                }
            })
            .collect();
        Ok(MutationStage::new(candidate, evidence))
    }
}

fn batch(id: &str, operations: Vec<MutationOperation>) -> MutationBatch {
    MutationBatch::new(
        MutationBatchId::new(id).unwrap(),
        MutationCausation::new("input:1").unwrap(),
        MutationProvenance::new("product:test").unwrap(),
        operations,
    )
    .unwrap()
}

fn operation(id: u64) -> MutationOperation {
    MutationOperation::new(
        MutationOperationId::new(id),
        "timeline.start",
        "kernel.start-timeline",
        json!({"slot": id}),
    )
    .unwrap()
}

fn setup() -> (
    RuntimeLifecycle,
    RuntimeMutation<World, u32>,
    runtime_lifecycle::SimulationStepAdmission,
    World,
) {
    let mut lifecycle =
        RuntimeLifecycle::new(RuntimeInstanceId::new(7), RuntimeLifecycleConfig::Demand);
    lifecycle.start().unwrap();
    let lane = RuntimeMutation::<World, u32>::bind(catalog(), &lifecycle).unwrap();
    let step = lifecycle.admit_demand_step().unwrap().step_at(0).unwrap();
    (
        lifecycle,
        lane,
        step,
        World {
            value: 0,
            revision: 0,
            domain: "world",
            guard_source: Rc::new(Cell::new(0)),
        },
    )
}

#[test]
fn stages_multiple_operations_and_publishes_one_candidate_with_receipt() {
    let (lifecycle, mut lane, admission, mut world) = setup();
    let mut planner = Planner::default();
    let receipt = lane
        .apply_batch(
            &lifecycle,
            admission.phases().mutation(),
            &mut world,
            &mut planner,
            batch("batch-1", vec![operation(1), operation(2)]),
        )
        .unwrap();
    assert_eq!(world.value, 2);
    assert_eq!(world.revision, 1);
    assert_eq!(receipt.operations().len(), 2);
    assert_eq!(receipt.operations()[0].binding_index(), 3);
    assert_eq!(receipt.operations()[0].kind(), "operation");
    assert_eq!(receipt.operations()[0].owner(), "product.timeline");
    assert_eq!(
        receipt.operations()[0].provenance_source(),
        "kernel/timeline.rs"
    );
    assert_eq!(receipt.operations()[0].provenance_path(), "start");
    assert_eq!(receipt.observed_guard(), &(0, 0));
    assert_eq!(receipt.committed_guard(), &(2, 1));
    assert_eq!(planner.calls, 1);
}

#[test]
fn exact_retry_returns_prior_receipt_without_republishing() {
    let (mut lifecycle, mut lane, admission, mut world) = setup();
    let mut planner = Planner::default();
    let first_batch = batch("batch-1", vec![operation(1)]);
    let first = lane
        .apply_batch(
            &lifecycle,
            admission.phases().mutation(),
            &mut world,
            &mut planner,
            first_batch.clone(),
        )
        .unwrap();
    let second = lane
        .apply_batch(
            &lifecycle,
            admission.phases().mutation(),
            &mut world,
            &mut planner,
            first_batch,
        )
        .unwrap();
    assert_eq!(first, second);
    assert_eq!(planner.calls, 1);
    assert_eq!(world.value, 1);
    lifecycle.admit_demand_step().unwrap();
    let delayed = lane
        .apply_batch(
            &lifecycle,
            admission.phases().mutation(),
            &mut world,
            &mut planner,
            batch("batch-1", vec![operation(1)]),
        )
        .unwrap();
    assert_eq!(first, delayed);
    assert_eq!(planner.calls, 1);
    assert_eq!(world.value, 1);
    assert_eq!(
        lane.receipt_for_step(admission.phases().mutation().simulation().step())
            .unwrap(),
        &first
    );
}

#[test]
fn empty_steps_allow_sparse_cadence_without_authority_publication() {
    let (mut lifecycle, mut lane, admission, mut world) = setup();
    let mut planner = Planner::default();
    lane.apply_batch(
        &lifecycle,
        admission.phases().mutation(),
        &mut world,
        &mut planner,
        batch("step-0", vec![operation(1)]),
    )
    .unwrap();

    let mut first_empty = None;
    for step in 1..=5 {
        let admission = lifecycle.admit_demand_step().unwrap().step_at(0).unwrap();
        let token = admission.phases().mutation();
        let receipt = lane.complete_empty_step(&lifecycle, token).unwrap();
        assert_eq!(receipt.step().value(), step);
        assert_eq!(world.value, 1);
        if step == 1 {
            first_empty = Some((token, receipt));
        }
    }
    let (first_token, first_receipt) = first_empty.unwrap();
    assert_eq!(
        lane.complete_empty_step(&lifecycle, first_token).unwrap(),
        first_receipt
    );
    assert!(matches!(
        lane.apply_batch(
            &lifecycle,
            first_token,
            &mut world,
            &mut planner,
            batch("conflicts-with-empty", vec![operation(2)]),
        ),
        Err(RuntimeMutationError::StepCompletedEmpty { .. })
    ));

    let sixth = lifecycle.admit_demand_step().unwrap().step_at(0).unwrap();
    lane.apply_batch(
        &lifecycle,
        sixth.phases().mutation(),
        &mut world,
        &mut planner,
        batch("step-6", vec![operation(6)]),
    )
    .unwrap();
    assert_eq!(world.value, 2);
    assert_eq!(lane.readout().last_completed_step().unwrap().value(), 6);

    lifecycle.pause().unwrap();
    lifecycle.resume().unwrap();
    lane.rebind(&lifecycle).unwrap();
    assert_eq!(
        lane.empty_completion_for_step(first_receipt.step()),
        Some(first_receipt)
    );
}

#[test]
fn empty_completion_rejects_wrong_phase_stale_and_batch_completed_steps() {
    let (mut lifecycle, mut lane, admission, mut world) = setup();
    let mut planner = Planner::default();
    assert!(matches!(
        lane.complete_empty_step(&lifecycle, admission.phases().timeline()),
        Err(RuntimeMutationError::WrongPhase { .. })
    ));
    lane.apply_batch(
        &lifecycle,
        admission.phases().mutation(),
        &mut world,
        &mut planner,
        batch("step-0", vec![operation(1)]),
    )
    .unwrap();
    assert!(matches!(
        lane.complete_empty_step(&lifecycle, admission.phases().mutation()),
        Err(RuntimeMutationError::StepAlreadyCompletedWithBatch { .. })
    ));
    lifecycle.pause().unwrap();
    lifecycle.resume().unwrap();
    lane.rebind(&lifecycle).unwrap();
    assert!(matches!(
        lane.complete_empty_step(&lifecycle, admission.phases().mutation()),
        Err(RuntimeMutationError::StaleBinding { .. })
    ));
}

#[test]
fn receipt_history_is_bounded_and_old_steps_never_republish_after_eviction() {
    let (mut lifecycle, mut lane, first_admission, mut world) = setup();
    let first_token = first_admission.phases().mutation();
    let first_step = first_token.simulation().step();
    let mut planner = Planner::default();

    for index in 0..=runtime_mutation::MAX_MUTATION_RECEIPTS {
        let token = if index == 0 {
            first_token
        } else {
            lifecycle
                .admit_demand_step()
                .unwrap()
                .step_at(0)
                .unwrap()
                .phases()
                .mutation()
        };
        lane.apply_batch(
            &lifecycle,
            token,
            &mut world,
            &mut planner,
            batch(&format!("batch-{index}"), vec![operation(index as u64 + 1)]),
        )
        .unwrap();
    }

    assert_eq!(lane.readout().evicted_receipt_count(), 1);
    assert!(lane.receipt_for_step(first_step).is_none());
    let error = lane
        .apply_batch(
            &lifecycle,
            first_token,
            &mut world,
            &mut planner,
            batch("batch-0", vec![operation(1)]),
        )
        .unwrap_err();
    assert!(matches!(error, RuntimeMutationError::StepOutOfOrder { .. }));
    assert_eq!(
        world.value,
        (runtime_mutation::MAX_MUTATION_RECEIPTS + 1) as i64
    );
}

#[test]
fn same_generation_rebind_retains_applied_receipt_readback() {
    let (mut lifecycle, mut lane, admission, mut world) = setup();
    let step = admission.phases().mutation().simulation().step();
    let mut planner = Planner::default();
    let receipt = lane
        .apply_batch(
            &lifecycle,
            admission.phases().mutation(),
            &mut world,
            &mut planner,
            batch("batch-1", vec![operation(1)]),
        )
        .unwrap();

    lifecycle.pause().unwrap();
    lifecycle.resume().unwrap();
    lane.rebind(&lifecycle).unwrap();

    assert_eq!(lane.receipt_for_step(step), Some(&receipt));
    assert_eq!(lane.readout().invalidated_admission_count(), 0);
}

#[test]
fn batch_identity_conflict_and_wrong_authority_domain_never_publish() {
    let (lifecycle, mut lane, admission, mut world) = setup();
    let mut planner = Planner::default();
    world.domain = "inventory";
    let error = lane
        .apply_batch(
            &lifecycle,
            admission.phases().mutation(),
            &mut world,
            &mut planner,
            batch("batch-1", vec![operation(1)]),
        )
        .unwrap_err();
    assert!(matches!(error, RuntimeMutationError::DomainMismatch { .. }));
    assert_eq!(planner.calls, 0);
    assert_eq!(world.value, 0);

    world.domain = "world";
    lane.apply_batch(
        &lifecycle,
        admission.phases().mutation(),
        &mut world,
        &mut planner,
        batch("batch-1", vec![operation(1)]),
    )
    .unwrap();
    let error = lane
        .apply_batch(
            &lifecycle,
            admission.phases().mutation(),
            &mut world,
            &mut planner,
            batch("batch-1", vec![operation(2)]),
        )
        .unwrap_err();
    assert!(matches!(
        error,
        RuntimeMutationError::BatchIdentityConflict { .. }
    ));
    assert_eq!(planner.calls, 1);
    assert_eq!(world.value, 1);
}

#[test]
fn applied_batch_identity_cannot_satisfy_a_different_step() {
    let (mut lifecycle, mut lane, admission, mut world) = setup();
    let mut planner = Planner::default();
    lane.apply_batch(
        &lifecycle,
        admission.phases().mutation(),
        &mut world,
        &mut planner,
        batch("batch-1", vec![operation(1)]),
    )
    .unwrap();
    let next = lifecycle.admit_demand_step().unwrap().step_at(0).unwrap();
    let error = lane
        .apply_batch(
            &lifecycle,
            next.phases().mutation(),
            &mut world,
            &mut planner,
            batch("batch-1", vec![operation(1)]),
        )
        .unwrap_err();
    assert!(matches!(
        error,
        RuntimeMutationError::BatchAlreadyApplied { .. }
    ));
    lane.apply_batch(
        &lifecycle,
        next.phases().mutation(),
        &mut world,
        &mut planner,
        batch("batch-2", vec![operation(2)]),
    )
    .unwrap();
    assert_eq!(world.value, 2);
    assert_eq!(planner.calls, 2);
}

#[test]
fn candidate_domain_drift_is_rejected_before_publication() {
    let (lifecycle, mut lane, admission, mut world) = setup();
    let mut planner = Planner {
        wrong_candidate_domain: true,
        ..Planner::default()
    };
    let error = lane
        .apply_batch(
            &lifecycle,
            admission.phases().mutation(),
            &mut world,
            &mut planner,
            batch("domain-drift", vec![operation(1)]),
        )
        .unwrap_err();
    assert!(matches!(error, RuntimeMutationError::DomainMismatch { .. }));
    assert_eq!(world.value, 0);
    assert_eq!(lane.readout().last_applied_step(), None);
}

#[test]
fn stale_live_guard_after_staging_publishes_nothing() {
    let (lifecycle, mut lane, admission, mut world) = setup();
    let mut planner = Planner {
        mutate_during_stage: true,
        ..Planner::default()
    };
    let error = lane
        .apply_batch(
            &lifecycle,
            admission.phases().mutation(),
            &mut world,
            &mut planner,
            batch("stale-guard", vec![operation(1)]),
        )
        .unwrap_err();
    assert!(matches!(error, RuntimeMutationError::AuthorityGuardChanged));
    assert_eq!(world.value, 0);
    assert_eq!(lane.readout().last_applied_step(), None);
}

#[test]
fn planner_and_evidence_failures_leave_live_authority_unchanged_and_retryable() {
    let (lifecycle, mut lane, admission, mut world) = setup();
    let mut planner = Planner {
        fail: true,
        ..Planner::default()
    };
    let request = batch("batch-1", vec![operation(1)]);
    let error = lane
        .apply_batch(
            &lifecycle,
            admission.phases().mutation(),
            &mut world,
            &mut planner,
            request.clone(),
        )
        .unwrap_err();
    assert!(matches!(
        error,
        RuntimeMutationError::Planner("named owner rejected operation")
    ));
    assert_eq!(world.value, 0);
    planner.fail = false;
    planner.bad_evidence = true;
    let error = lane
        .apply_batch(
            &lifecycle,
            admission.phases().mutation(),
            &mut world,
            &mut planner,
            request.clone(),
        )
        .unwrap_err();
    assert!(matches!(
        error,
        RuntimeMutationError::OwnerEvidenceMismatch { .. }
    ));
    assert_eq!(world.value, 0);
    planner.bad_evidence = false;
    lane.apply_batch(
        &lifecycle,
        admission.phases().mutation(),
        &mut world,
        &mut planner,
        request,
    )
    .unwrap();
    assert_eq!(world.value, 1);
}

#[test]
fn duplicate_and_static_linkage_errors_are_preflighted_before_planner() {
    let (lifecycle, mut lane, admission, mut world) = setup();
    let mut planner = Planner::default();
    let duplicate = batch("duplicate", vec![operation(1), operation(1)]);
    let error = lane
        .apply_batch(
            &lifecycle,
            admission.phases().mutation(),
            &mut world,
            &mut planner,
            duplicate,
        )
        .unwrap_err();
    assert!(matches!(
        error,
        RuntimeMutationError::DuplicateOperationId(_)
    ));
    assert_eq!(planner.calls, 0);
    assert_eq!(world.value, 0);
    let wrong_target = MutationOperation::new(
        MutationOperationId::new(3),
        "timeline.start",
        "kernel.other",
        json!({}),
    )
    .unwrap();
    let error = lane
        .apply_batch(
            &lifecycle,
            admission.phases().mutation(),
            &mut world,
            &mut planner,
            batch("wrong-target", vec![wrong_target]),
        )
        .unwrap_err();
    assert!(matches!(
        error,
        RuntimeMutationError::OperationTargetMismatch { .. }
    ));
    assert_eq!(planner.calls, 0);
}

#[test]
fn lifecycle_and_same_generation_rebind_invalidate_uncommitted_admission() {
    let (mut lifecycle, mut lane, admission, mut world) = setup();
    let mut planner = Planner::default();
    lifecycle.pause().unwrap();
    lifecycle.resume().unwrap();
    assert!(lane.rebind(&lifecycle).is_ok());
    assert_eq!(lane.readout().invalidated_admission_count(), 1);
    assert!(matches!(
        lane.apply_batch(
            &lifecycle,
            admission.phases().mutation(),
            &mut world,
            &mut planner,
            batch("stale", vec![operation(1)]),
        ),
        Err(RuntimeMutationError::StaleBinding { .. })
    ));
    let fresh = lifecycle.admit_demand_step().unwrap().step_at(0).unwrap();
    lane.apply_batch(
        &lifecycle,
        fresh.phases().mutation(),
        &mut world,
        &mut planner,
        batch("fresh", vec![operation(1)]),
    )
    .unwrap();
}

#[test]
fn wrong_phase_pause_fault_shutdown_and_foreign_tokens_never_publish() {
    let (mut lifecycle, mut lane, admission, mut world) = setup();
    let mut planner = Planner::default();
    let request = batch("b", vec![operation(1)]);
    let error = lane
        .apply_batch(
            &lifecycle,
            admission.phases().timeline(),
            &mut world,
            &mut planner,
            request.clone(),
        )
        .unwrap_err();
    assert!(matches!(error, RuntimeMutationError::WrongPhase { .. }));
    lifecycle.pause().unwrap();
    assert!(matches!(
        lane.apply_batch(
            &lifecycle,
            admission.phases().mutation(),
            &mut world,
            &mut planner,
            request.clone(),
        ),
        Err(RuntimeMutationError::LifecycleNotRunning)
    ));
    lifecycle.resume().unwrap();
    lifecycle
        .report_fault(runtime_lifecycle::RuntimeFault::OwnerReported)
        .unwrap();
    assert!(matches!(
        lane.apply_batch(
            &lifecycle,
            admission.phases().mutation(),
            &mut world,
            &mut planner,
            request,
        ),
        Err(RuntimeMutationError::StaleBinding { .. })
            | Err(RuntimeMutationError::LifecycleNotRunning)
    ));
    assert_eq!(world.value, 0);
}

#[test]
fn constructor_bounds_identity_and_payload() {
    assert!(matches!(
        MutationBatchId::new(""),
        Err(MutationDataError::EmptyIdentity("batch id"))
    ));
    let oversized = "x".repeat(runtime_mutation::MAX_MUTATION_PAYLOAD_BYTES + 1);
    assert!(matches!(
        MutationOperation::new(
            MutationOperationId::new(1),
            "timeline.start",
            "kernel.start-timeline",
            json!(oversized),
        ),
        Err(MutationDataError::PayloadTooLarge { .. })
    ));
}

#[test]
fn assembly_catalog_rejects_unknown_target_target_drift_and_non_operation_kind() {
    let linked = linked();
    assert!(matches!(
        CompiledMutationCatalog::compile(&linked, &[]),
        Err(RuntimeMutationError::EmptyCatalog)
    ));
    let unknown = CompiledMutationCatalog::compile(
        &linked,
        &[MutationCapabilityDescriptor::new(
            "missing",
            "kernel.missing",
            "world",
            "owner",
            "test.operation.v1",
        )],
    )
    .unwrap_err();
    assert!(matches!(unknown, RuntimeMutationError::UnknownBinding(_)));

    let drift = CompiledMutationCatalog::compile(
        &linked,
        &[MutationCapabilityDescriptor::new(
            "timeline.start",
            "kernel.other",
            "world",
            "owner",
            "test.operation.v1",
        )],
    )
    .unwrap_err();
    assert!(matches!(
        drift,
        RuntimeMutationError::BindingTargetMismatch { .. }
    ));

    let wrong_kind = CompiledMutationCatalog::compile(
        &linked,
        &[MutationCapabilityDescriptor::new(
            "movement.apply",
            "kernel.apply-movement",
            "world",
            "owner",
            "test.operation.v1",
        )],
    )
    .unwrap_err();
    assert!(matches!(
        wrong_kind,
        RuntimeMutationError::CapabilityKindMismatch { .. }
    ));
}
