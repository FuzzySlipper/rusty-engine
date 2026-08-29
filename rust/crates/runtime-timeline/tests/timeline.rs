use runtime_lifecycle::{RuntimeInstanceId, RuntimeLifecycle, RuntimeLifecycleConfig};
use runtime_timeline::{
    ReleasedCompletionStatus, ReleasedTimelineEvent, RuntimeOpaqueData, RuntimeProvenance,
    RuntimeSourceKind, RuntimeTimelineError, TimelineCatalog, TimelineCompletionEnvelope,
    TimelineCompletionOutcome, TimelineCompletionSpec, TimelineCompletionTicket,
    TimelineDescriptor, TimelineOperationIdentity, TimelineOperationReplacement,
    TimelineOperationRevision, TimelineOperationSnapshot, TimelineOperationSpec,
    TimelineRecurrence, TimelineSnapshot, TimelineStepDescriptor, TimelineTicketSnapshot,
};
use serde_json::json;

fn catalog() -> TimelineCatalog {
    TimelineCatalog::new([TimelineDescriptor::new(
        "intro",
        [
            TimelineStepDescriptor::new("start", "timeline.start", json!({"scene": "opening"}))
                .unwrap(),
        ],
    )
    .unwrap()])
    .unwrap()
}

fn setup() -> (
    RuntimeLifecycle,
    runtime_timeline::RuntimeTimeline,
    runtime_lifecycle::RuntimePhaseToken,
) {
    let catalog = catalog();
    let mut lifecycle =
        RuntimeLifecycle::new(RuntimeInstanceId::new(91), RuntimeLifecycleConfig::Demand);
    lifecycle.start().expect("start");
    let timeline = catalog.bind(&lifecycle).expect("bind");
    let admission = lifecycle.admit_demand_step().expect("step");
    (
        lifecycle,
        timeline,
        admission.step_at(0).unwrap().phases().timeline(),
    )
}

fn provenance() -> RuntimeProvenance {
    RuntimeProvenance::new(
        "product-correlation-1",
        Some(RuntimeOpaqueData::new(json!({"slot": 2})).unwrap()),
    )
    .unwrap()
}

fn operation(id: u64, due: u64, recurrence: TimelineRecurrence) -> TimelineOperationSpec {
    TimelineOperationSpec::new(
        "intro",
        "start",
        TimelineOperationIdentity::new(id),
        runtime_lifecycle::SimulationStep::new(due),
        recurrence,
        provenance(),
    )
    .unwrap()
}

fn completion_spec(
    id: u64,
    revision: TimelineOperationRevision,
    correlation: &str,
) -> TimelineCompletionSpec {
    TimelineCompletionSpec::new(
        "intro",
        "start",
        TimelineOperationIdentity::new(id),
        revision,
        RuntimeSourceKind::External,
        correlation,
        "product-result-1",
        provenance(),
    )
    .unwrap()
}

#[test]
fn builds_neutral_descriptors_with_stable_inspection() {
    let first = catalog();
    let second = catalog();
    assert_eq!(first, second);
    let step = first.step("intro", "start").unwrap();
    assert_eq!(step.operation(), "timeline.start");
    assert_eq!(step.payload(), &json!({"scene": "opening"}));
    assert_eq!(
        first.inspection_json_newline().unwrap(),
        second.inspection_json_newline().unwrap()
    );
}

#[test]
fn equal_deadlines_release_by_lane_insertion_then_operation_identity() {
    let (lifecycle, mut timeline, token) = setup();
    timeline
        .schedule(&lifecycle, token, operation(9, 0, TimelineRecurrence::Once))
        .unwrap();
    timeline
        .schedule(&lifecycle, token, operation(2, 0, TimelineRecurrence::Once))
        .unwrap();
    let release = timeline.release_due(&lifecycle, token, 8).unwrap();
    let ids = release
        .events()
        .iter()
        .map(|event| match event {
            ReleasedTimelineEvent::Operation(operation) => operation.operation_id().value(),
            ReleasedTimelineEvent::Completion(_) => 99,
        })
        .collect::<Vec<_>>();
    assert_eq!(ids, [9, 2]);
}

#[test]
fn schedule_batch_rejects_live_and_candidate_duplicates_without_changing_lane_state() {
    let (lifecycle, mut timeline, token) = setup();
    let before = timeline.readout();
    assert!(matches!(
        timeline.schedule_batch(
            &lifecycle,
            token,
            vec![
                operation(7, 0, TimelineRecurrence::Once),
                operation(7, 0, TimelineRecurrence::Once),
            ],
        ),
        Err(RuntimeTimelineError::OperationIdentityInUse(identity)) if identity.value() == 7
    ));
    assert_eq!(timeline.readout(), before);

    timeline
        .schedule(&lifecycle, token, operation(9, 0, TimelineRecurrence::Once))
        .expect("live operation");
    let before_live_duplicate = timeline.readout();
    assert!(matches!(
        timeline.schedule_batch(
            &lifecycle,
            token,
            vec![
                operation(10, 0, TimelineRecurrence::Once),
                operation(9, 0, TimelineRecurrence::Once),
            ],
        ),
        Err(RuntimeTimelineError::OperationIdentityInUse(identity)) if identity.value() == 9
    ));
    assert_eq!(timeline.readout(), before_live_duplicate);
}

#[test]
fn finite_recurrence_uses_admitted_steps_and_one_bounded_backlog_occurrence() {
    let (mut lifecycle, mut timeline, token) = setup();
    timeline
        .schedule(
            &lifecycle,
            token,
            operation(
                4,
                0,
                TimelineRecurrence::Every {
                    interval_steps: 2,
                    remaining: 3,
                },
            ),
        )
        .unwrap();
    assert_eq!(
        timeline
            .release_due(&lifecycle, token, 8)
            .unwrap()
            .events()
            .len(),
        1
    );
    let step_1 = lifecycle.admit_demand_step().unwrap().step_at(0).unwrap();
    assert!(timeline
        .release_due(&lifecycle, step_1.phases().timeline(), 8)
        .unwrap()
        .events()
        .is_empty());
    let step_2 = lifecycle.admit_demand_step().unwrap().step_at(0).unwrap();
    assert_eq!(
        timeline
            .release_due(&lifecycle, step_2.phases().timeline(), 8)
            .unwrap()
            .events()
            .len(),
        1
    );
    let step_3 = lifecycle.admit_demand_step().unwrap().step_at(0).unwrap();
    assert!(timeline
        .release_due(&lifecycle, step_3.phases().timeline(), 8)
        .unwrap()
        .events()
        .is_empty());
    let step_4 = lifecycle.admit_demand_step().unwrap().step_at(0).unwrap();
    assert_eq!(
        timeline
            .release_due(&lifecycle, step_4.phases().timeline(), 8)
            .unwrap()
            .events()
            .len(),
        1
    );
    assert_eq!(timeline.readout().operation_count(), 0);
}

#[test]
fn completion_arrival_order_does_not_change_issue_order_or_gap_behavior() {
    fn run(order: [usize; 2]) -> Vec<u64> {
        let (mut lifecycle, mut timeline, token_0) = setup();
        let first = timeline
            .register_completion(
                &lifecycle,
                token_0,
                completion_spec(1, TimelineOperationRevision::ZERO, "one"),
            )
            .unwrap();
        let second = timeline
            .register_completion(
                &lifecycle,
                token_0,
                completion_spec(2, TimelineOperationRevision::ZERO, "two"),
            )
            .unwrap();
        let tickets = [first, second];
        let mut released = Vec::new();
        for (index, ticket_index) in order.into_iter().enumerate() {
            let ticket = &tickets[ticket_index];
            let outcome = if ticket_index == 0 {
                TimelineCompletionOutcome::Failure(Some(
                    RuntimeOpaqueData::new(json!({"code": 7})).unwrap(),
                ))
            } else {
                TimelineCompletionOutcome::Success(None)
            };
            let envelope = TimelineCompletionEnvelope::new(
                ticket.id(),
                ticket.binding(),
                ticket.correlation(),
                outcome,
                ticket.provenance().clone(),
            )
            .unwrap();
            timeline.admit_completion(&lifecycle, envelope).unwrap();
            let token = if index == 0 {
                token_0
            } else {
                lifecycle
                    .admit_demand_step()
                    .unwrap()
                    .step_at(0)
                    .unwrap()
                    .phases()
                    .timeline()
            };
            let events = timeline
                .release_due(&lifecycle, token, 8)
                .unwrap()
                .into_events();
            released.extend(events.into_iter().filter_map(|event| match event {
                ReleasedTimelineEvent::Completion(completion) => {
                    Some(completion.ticket().id().value())
                }
                ReleasedTimelineEvent::Operation(_) => None,
            }));
        }
        released
    }

    assert_eq!(run([0, 1]), [0, 1]);
    assert_eq!(run([1, 0]), [0, 1]);
}

#[test]
fn cancel_replace_use_exact_cas_receipts_and_preserve_insertion_sequence() {
    let (lifecycle, mut timeline, token) = setup();
    let old = timeline
        .schedule(&lifecycle, token, operation(8, 0, TimelineRecurrence::Once))
        .unwrap();
    let replacement = TimelineOperationReplacement::new(
        "intro",
        "start",
        runtime_lifecycle::SimulationStep::new(3),
        TimelineRecurrence::Once,
        provenance(),
    )
    .unwrap();
    let current = timeline
        .replace(&lifecycle, token, old, replacement)
        .unwrap();
    assert_eq!(current.insertion_sequence(), old.insertion_sequence());
    assert!(matches!(
        timeline.cancel(&lifecycle, token, old),
        Err(RuntimeTimelineError::OperationReceiptMismatch { .. })
    ));
    timeline.cancel(&lifecycle, token, current).unwrap();
}

#[test]
fn cancel_or_replace_invalidates_bound_completion_ticket() {
    let (lifecycle, mut timeline, token) = setup();
    let operation = timeline
        .schedule(&lifecycle, token, operation(7, 0, TimelineRecurrence::Once))
        .unwrap();
    let ticket = timeline
        .register_completion(
            &lifecycle,
            token,
            completion_spec(7, operation.revision(), "bound"),
        )
        .unwrap();
    timeline.cancel(&lifecycle, token, operation).unwrap();
    let envelope = TimelineCompletionEnvelope::new(
        ticket.id(),
        ticket.binding(),
        ticket.correlation(),
        TimelineCompletionOutcome::Success(None),
        ticket.provenance().clone(),
    )
    .unwrap();
    assert!(matches!(
        timeline.admit_completion(&lifecycle, envelope),
        Err(RuntimeTimelineError::TicketCancelled(_))
    ));
    let release = timeline.release_due(&lifecycle, token, 8).unwrap();
    assert!(matches!(
        release.events().first(),
        Some(ReleasedTimelineEvent::Completion(completion))
            if matches!(completion.status(), ReleasedCompletionStatus::Cancelled)
    ));
}

#[test]
fn replacement_and_recurrence_revision_changes_close_ticket_gaps() {
    let (lifecycle, mut timeline, token) = setup();
    let replacement_operation = timeline
        .schedule(
            &lifecycle,
            token,
            operation(17, 0, TimelineRecurrence::Once),
        )
        .unwrap();
    let ticket = timeline
        .register_completion(
            &lifecycle,
            token,
            completion_spec(17, replacement_operation.revision(), "replace"),
        )
        .unwrap();
    let replacement = TimelineOperationReplacement::new(
        "intro",
        "start",
        runtime_lifecycle::SimulationStep::new(0),
        TimelineRecurrence::Once,
        provenance(),
    )
    .unwrap();
    timeline
        .replace(&lifecycle, token, replacement_operation, replacement)
        .unwrap();
    let release = timeline.release_due(&lifecycle, token, 8).unwrap();
    assert!(release.events().iter().any(|event| {
        matches!(event, ReleasedTimelineEvent::Completion(completion)
            if completion.ticket().id() == ticket.id()
                && matches!(completion.status(), ReleasedCompletionStatus::Cancelled))
    }));

    let (lifecycle, mut timeline, token) = setup();
    let recurring = timeline
        .schedule(
            &lifecycle,
            token,
            operation(
                18,
                0,
                TimelineRecurrence::Every {
                    interval_steps: 1,
                    remaining: 2,
                },
            ),
        )
        .unwrap();
    let ticket = timeline
        .register_completion(
            &lifecycle,
            token,
            completion_spec(18, recurring.revision(), "recurrence"),
        )
        .unwrap();
    let release = timeline.release_due(&lifecycle, token, 8).unwrap();
    assert!(release.events().iter().any(|event| {
        matches!(event, ReleasedTimelineEvent::Completion(completion)
            if completion.ticket().id() == ticket.id()
                && matches!(completion.status(), ReleasedCompletionStatus::Cancelled))
    }));
}

#[test]
fn completed_bound_tickets_survive_operation_revision_invalidation() {
    let expected = TimelineCompletionOutcome::Success(Some(
        RuntimeOpaqueData::new(json!({"preserved": true})).unwrap(),
    ));

    let (lifecycle, mut timeline, token) = setup();
    let cancel_operation = timeline
        .schedule(
            &lifecycle,
            token,
            operation(27, 0, TimelineRecurrence::Once),
        )
        .unwrap();
    let ticket = timeline
        .register_completion(
            &lifecycle,
            token,
            completion_spec(27, cancel_operation.revision(), "completed-cancel"),
        )
        .unwrap();
    let envelope = TimelineCompletionEnvelope::new(
        ticket.id(),
        ticket.binding(),
        ticket.correlation(),
        expected.clone(),
        ticket.provenance().clone(),
    )
    .unwrap();
    timeline.admit_completion(&lifecycle, envelope).unwrap();
    timeline
        .cancel(&lifecycle, token, cancel_operation)
        .unwrap();
    let release = timeline.release_due(&lifecycle, token, 8).unwrap();
    assert!(release.events().iter().any(|event| {
        matches!(event, ReleasedTimelineEvent::Completion(completion)
            if completion.ticket().id() == ticket.id()
                && completion.status() == &ReleasedCompletionStatus::Completed(expected.clone()))
    }));

    let (lifecycle, mut timeline, token) = setup();
    let replacement_operation = timeline
        .schedule(
            &lifecycle,
            token,
            operation(28, 0, TimelineRecurrence::Once),
        )
        .unwrap();
    let ticket = timeline
        .register_completion(
            &lifecycle,
            token,
            completion_spec(28, replacement_operation.revision(), "completed-replace"),
        )
        .unwrap();
    let envelope = TimelineCompletionEnvelope::new(
        ticket.id(),
        ticket.binding(),
        ticket.correlation(),
        expected.clone(),
        ticket.provenance().clone(),
    )
    .unwrap();
    timeline.admit_completion(&lifecycle, envelope).unwrap();
    let replacement = TimelineOperationReplacement::new(
        "intro",
        "start",
        runtime_lifecycle::SimulationStep::new(0),
        TimelineRecurrence::Once,
        provenance(),
    )
    .unwrap();
    timeline
        .replace(&lifecycle, token, replacement_operation, replacement)
        .unwrap();
    let release = timeline.release_due(&lifecycle, token, 8).unwrap();
    assert!(release.events().iter().any(|event| {
        matches!(event, ReleasedTimelineEvent::Completion(completion)
            if completion.ticket().id() == ticket.id()
                && completion.status() == &ReleasedCompletionStatus::Completed(expected.clone()))
    }));

    let (lifecycle, mut timeline, token) = setup();
    let recurring_operation = timeline
        .schedule(
            &lifecycle,
            token,
            operation(
                29,
                0,
                TimelineRecurrence::Every {
                    interval_steps: 1,
                    remaining: 2,
                },
            ),
        )
        .unwrap();
    let ticket = timeline
        .register_completion(
            &lifecycle,
            token,
            completion_spec(29, recurring_operation.revision(), "completed-recurrence"),
        )
        .unwrap();
    let envelope = TimelineCompletionEnvelope::new(
        ticket.id(),
        ticket.binding(),
        ticket.correlation(),
        expected,
        ticket.provenance().clone(),
    )
    .unwrap();
    timeline.admit_completion(&lifecycle, envelope).unwrap();
    let release = timeline.release_due(&lifecycle, token, 8).unwrap();
    assert!(release.events().iter().any(|event| {
        matches!(event, ReleasedTimelineEvent::Completion(completion)
            if completion.ticket().id() == ticket.id()
                && matches!(completion.status(), ReleasedCompletionStatus::Completed(_)))
    }));
}

#[test]
fn pause_resume_rebind_retains_operations_and_invalidates_tickets_restart_clears() {
    let (mut lifecycle, mut timeline, _token) = setup();
    let operation = timeline
        .schedule(
            &lifecycle,
            _token,
            operation(5, 3, TimelineRecurrence::Once),
        )
        .unwrap();
    let ticket = timeline
        .register_completion(
            &lifecycle,
            _token,
            completion_spec(6, TimelineOperationRevision::ZERO, "stale"),
        )
        .unwrap();
    lifecycle.pause().unwrap();
    assert!(matches!(
        timeline.rebind(&lifecycle),
        Err(RuntimeTimelineError::RebindNotRunning)
    ));
    lifecycle.resume().unwrap();
    let receipt = timeline.rebind(&lifecycle).unwrap();
    assert_eq!(receipt.retained_operations(), 1);
    assert_eq!(receipt.invalidated_tickets(), 1);
    assert_eq!(receipt.invalidated_admissions(), 1);
    assert_eq!(timeline.readout().operation_count(), 1);
    assert_eq!(timeline.readout().next_expected_step(), 1);
    assert_eq!(timeline.readout().invalidated_admission_count(), 1);
    let envelope = TimelineCompletionEnvelope::new(
        ticket.id(),
        ticket.binding(),
        ticket.correlation(),
        TimelineCompletionOutcome::Success(None),
        ticket.provenance().clone(),
    )
    .unwrap();
    assert!(matches!(
        timeline.admit_completion(&lifecycle, envelope),
        Err(RuntimeTimelineError::TicketNotFound(_))
    ));
    let refreshed = timeline
        .operation_receipt(operation.operation_id())
        .unwrap();
    let resumed_step = lifecycle.admit_demand_step().unwrap().step_at(0).unwrap();
    timeline
        .cancel(&lifecycle, resumed_step.phases().timeline(), refreshed)
        .unwrap();
    lifecycle.restart().unwrap();
    let reset = timeline.rebind(&lifecycle).unwrap();
    assert!(reset.generation_reset());
    assert_eq!(reset.cleared_operations(), 0);
    assert_eq!(timeline.readout().operation_count(), 0);
}

#[test]
fn rebind_reconciles_multiple_admitted_steps_and_next_token_recovers_release() {
    let (mut lifecycle, mut timeline, _token) = setup();
    lifecycle.pause().unwrap();
    lifecycle.resume().unwrap();
    lifecycle.admit_demand_step().unwrap();
    lifecycle.admit_demand_step().unwrap();
    let receipt = timeline.rebind(&lifecycle).unwrap();
    assert_eq!(receipt.invalidated_admissions(), 3);
    assert_eq!(timeline.readout().next_expected_step(), 3);
    assert_eq!(timeline.readout().invalidated_admission_count(), 3);
    assert_eq!(timeline.readout().last_release_step(), None);
    let next = lifecycle.admit_demand_step().unwrap().step_at(0).unwrap();
    let release = timeline
        .release_due(&lifecycle, next.phases().timeline(), 8)
        .unwrap();
    assert_eq!(release.step().value(), 3);
    assert_eq!(timeline.readout().last_release_step().unwrap().value(), 3);
    assert_eq!(timeline.readout().next_expected_step(), 4);

    let snapshot = timeline.snapshot();
    assert_eq!(snapshot.next_expected_step(), 4);
    assert_eq!(snapshot.invalidated_admission_count(), 3);
    timeline.restore_snapshot(&lifecycle, snapshot).unwrap();

    let (mut lifecycle, mut timeline, token) = setup();
    timeline.release_due(&lifecycle, token, 8).unwrap();
    lifecycle.pause().unwrap();
    lifecycle.resume().unwrap();
    lifecycle.admit_demand_step().unwrap();
    lifecycle.admit_demand_step().unwrap();
    let receipt = timeline.rebind(&lifecycle).unwrap();
    assert_eq!(receipt.invalidated_admissions(), 2);
    assert_eq!(timeline.readout().last_release_step().unwrap().value(), 0);
    assert_eq!(timeline.readout().next_expected_step(), 3);
    let snapshot = timeline.snapshot();
    timeline.restore_snapshot(&lifecycle, snapshot).unwrap();
    let next = lifecycle.admit_demand_step().unwrap().step_at(0).unwrap();
    assert_eq!(
        timeline
            .release_due(&lifecycle, next.phases().timeline(), 8)
            .unwrap()
            .step()
            .value(),
        3
    );
}

#[test]
fn malformed_completion_data_is_bounded_but_semantic_neutral() {
    let value = RuntimeOpaqueData::new(json!({
        "url": "https://product.example/result",
        "token": "product-owned-reference",
    }))
    .unwrap();
    assert_eq!(value.value()["token"], "product-owned-reference");
}

#[test]
fn snapshot_restore_rejects_duplicate_items_without_mutating_live_state() {
    let (lifecycle, mut timeline, token) = setup();
    timeline
        .schedule(&lifecycle, token, operation(3, 0, TimelineRecurrence::Once))
        .unwrap();
    let original = timeline.snapshot();
    let malformed = TimelineSnapshot::from_parts(
        original.binding(),
        original.next_insertion_sequence(),
        original.next_ticket_id(),
        original.last_release_step(),
        0,
        vec![
            original.operations()[0].clone(),
            original.operations()[0].clone(),
        ],
        Vec::new(),
    );
    assert!(matches!(
        timeline.restore_snapshot(&lifecycle, malformed),
        Err(RuntimeTimelineError::SnapshotUnsortedOperations)
            | Err(RuntimeTimelineError::SnapshotDuplicateOperation(_))
    ));
    assert_eq!(timeline.snapshot(), original);
}

#[test]
fn snapshot_restore_rejects_missing_bound_operation_and_revision_or_descriptor_drift() {
    let (lifecycle, mut timeline, token) = setup();
    let operation = timeline
        .schedule(&lifecycle, token, operation(3, 0, TimelineRecurrence::Once))
        .unwrap();
    let ticket = timeline
        .register_completion(
            &lifecycle,
            token,
            completion_spec(3, operation.revision(), "bound"),
        )
        .unwrap();
    let original = timeline.snapshot();

    let missing_operation = TimelineSnapshot::from_parts(
        original.binding(),
        original.next_insertion_sequence(),
        original.next_ticket_id(),
        original.last_release_step(),
        0,
        Vec::new(),
        original.tickets().to_vec(),
    );
    assert!(matches!(
        timeline.restore_snapshot(&lifecycle, missing_operation),
        Err(RuntimeTimelineError::SnapshotBoundOperationMissing(_))
    ));
    assert_eq!(timeline.snapshot(), original);

    let drifted_revision = TimelineSnapshot::from_parts(
        original.binding(),
        original.next_insertion_sequence(),
        original.next_ticket_id(),
        original.last_release_step(),
        0,
        vec![TimelineOperationSnapshot::from_parts(
            operation.operation_id(),
            operation.insertion_sequence(),
            TimelineOperationRevision::new(1),
            "intro",
            "start",
            runtime_lifecycle::SimulationStep::new(0),
            TimelineRecurrence::Once,
            provenance(),
        )],
        original.tickets().to_vec(),
    );
    assert!(matches!(
        timeline.restore_snapshot(&lifecycle, drifted_revision),
        Err(RuntimeTimelineError::SnapshotBoundOperationRevisionMismatch { .. })
    ));
    assert_eq!(timeline.snapshot(), original);

    let malformed_ticket = TimelineCompletionTicket::from_parts(
        ticket.id(),
        ticket.issue_sequence(),
        ticket.binding(),
        ticket.operation_id(),
        ticket.operation_revision(),
        true,
        ticket.timeline_id(),
        ticket.step_id(),
        "timeline.other-operation",
        ticket.source(),
        ticket.correlation(),
        ticket.result_contract(),
        ticket.provenance().clone(),
    );
    let drifted_descriptor = TimelineSnapshot::from_parts(
        original.binding(),
        original.next_insertion_sequence(),
        original.next_ticket_id(),
        original.last_release_step(),
        0,
        original.operations().to_vec(),
        vec![TimelineTicketSnapshot::from_parts(
            malformed_ticket,
            original.tickets()[0].status().clone(),
        )],
    );
    assert!(matches!(
        timeline.restore_snapshot(&lifecycle, drifted_descriptor),
        Err(RuntimeTimelineError::SnapshotBoundOperationDescriptorMismatch(_))
    ));
    assert_eq!(timeline.snapshot(), original);
}

#[test]
fn snapshot_restore_rejects_unbound_ticket_drift_identity_and_issue_cursor_atomically() {
    let (lifecycle, mut timeline, token) = setup();
    let ticket = timeline
        .register_completion(
            &lifecycle,
            token,
            completion_spec(99, TimelineOperationRevision::ZERO, "unbound"),
        )
        .unwrap();
    let original = timeline.snapshot();

    let wrong_target = TimelineCompletionTicket::from_parts(
        ticket.id(),
        ticket.issue_sequence(),
        ticket.binding(),
        ticket.operation_id(),
        ticket.operation_revision(),
        false,
        ticket.timeline_id(),
        ticket.step_id(),
        "timeline.not-the-linked-operation",
        ticket.source(),
        ticket.correlation(),
        ticket.result_contract(),
        ticket.provenance().clone(),
    );
    let malformed = TimelineSnapshot::from_parts(
        original.binding(),
        original.next_insertion_sequence(),
        original.next_ticket_id(),
        original.last_release_step(),
        original.invalidated_ticket_count(),
        Vec::new(),
        vec![TimelineTicketSnapshot::from_parts(
            wrong_target,
            original.tickets()[0].status().clone(),
        )],
    );
    assert!(matches!(
        timeline.restore_snapshot(&lifecycle, malformed),
        Err(RuntimeTimelineError::SnapshotBoundOperationDescriptorMismatch(_))
    ));
    assert_eq!(timeline.snapshot(), original);

    let invalid_identity = TimelineCompletionTicket::from_parts(
        ticket.id(),
        ticket.issue_sequence(),
        ticket.binding(),
        ticket.operation_id(),
        ticket.operation_revision(),
        false,
        ticket.timeline_id(),
        ticket.step_id(),
        ticket.operation(),
        ticket.source(),
        "bad//correlation",
        ticket.result_contract(),
        ticket.provenance().clone(),
    );
    let malformed = TimelineSnapshot::from_parts(
        original.binding(),
        original.next_insertion_sequence(),
        original.next_ticket_id(),
        original.last_release_step(),
        original.invalidated_ticket_count(),
        Vec::new(),
        vec![TimelineTicketSnapshot::from_parts(
            invalid_identity,
            original.tickets()[0].status().clone(),
        )],
    );
    assert!(matches!(
        timeline.restore_snapshot(&lifecycle, malformed),
        Err(RuntimeTimelineError::SnapshotInvariant(
            "ticket correlation"
        ))
    ));
    assert_eq!(timeline.snapshot(), original);

    let wrong_issue_sequence = TimelineCompletionTicket::from_parts(
        ticket.id(),
        runtime_timeline::TimelineInsertionSequence::new(99),
        ticket.binding(),
        ticket.operation_id(),
        ticket.operation_revision(),
        false,
        ticket.timeline_id(),
        ticket.step_id(),
        ticket.operation(),
        ticket.source(),
        ticket.correlation(),
        ticket.result_contract(),
        ticket.provenance().clone(),
    );
    let malformed = TimelineSnapshot::from_parts(
        original.binding(),
        original.next_insertion_sequence(),
        original.next_ticket_id(),
        original.last_release_step(),
        original.invalidated_ticket_count(),
        Vec::new(),
        vec![TimelineTicketSnapshot::from_parts(
            wrong_issue_sequence,
            original.tickets()[0].status().clone(),
        )],
    );
    assert!(matches!(
        timeline.restore_snapshot(&lifecycle, malformed),
        Err(RuntimeTimelineError::SnapshotCursorInvalid(
            "ticket issue sequence"
        ))
    ));
    assert_eq!(timeline.snapshot(), original);
}

#[test]
fn snapshot_restore_rejects_lifecycle_and_admission_cursor_mismatch_atomically() {
    let (mut lifecycle, mut timeline, _token) = setup();
    let original = timeline.snapshot();
    lifecycle.pause().unwrap();
    lifecycle.resume().unwrap();
    assert!(matches!(
        timeline.restore_snapshot(&lifecycle, original.clone()),
        Err(RuntimeTimelineError::StaleBinding { .. })
    ));
    assert_eq!(timeline.snapshot(), original);

    let candidate = TimelineSnapshot::from_parts_with_cursors(
        original.binding(),
        original.next_insertion_sequence(),
        original.next_ticket_id(),
        original.last_release_step(),
        99,
        original.invalidated_ticket_count(),
        99,
        original.operations().to_vec(),
        original.tickets().to_vec(),
    );
    assert!(matches!(
        timeline.restore_snapshot(&lifecycle, candidate),
        Err(RuntimeTimelineError::SnapshotBindingMismatch)
            | Err(RuntimeTimelineError::SnapshotCursorInvalid(_))
            | Err(RuntimeTimelineError::StaleBinding { .. })
    ));
}

#[test]
fn completion_queue_requires_running_exact_lane_binding_without_mutating_on_failure() {
    let (mut lifecycle, mut timeline, token) = setup();
    let ticket = timeline
        .register_completion(
            &lifecycle,
            token,
            completion_spec(31, TimelineOperationRevision::ZERO, "queued"),
        )
        .unwrap();
    let original = timeline.snapshot();
    let envelope = TimelineCompletionEnvelope::new(
        ticket.id(),
        ticket.binding(),
        ticket.correlation(),
        TimelineCompletionOutcome::Success(None),
        ticket.provenance().clone(),
    )
    .unwrap();
    lifecycle.pause().unwrap();
    assert!(matches!(
        timeline.admit_completion(&lifecycle, envelope.clone()),
        Err(RuntimeTimelineError::LifecycleNotRunning)
    ));
    assert_eq!(timeline.snapshot(), original);
    lifecycle.resume().unwrap();
    assert!(matches!(
        timeline.admit_completion(&lifecycle, envelope),
        Err(RuntimeTimelineError::StaleBinding { .. })
    ));
    assert_eq!(timeline.snapshot(), original);
}

#[test]
fn foreign_stale_fault_shutdown_releases_fail_without_state_change() {
    let (mut lifecycle, mut timeline, token) = setup();
    timeline
        .schedule(
            &lifecycle,
            token,
            operation(11, 0, TimelineRecurrence::Once),
        )
        .unwrap();
    let original = timeline.snapshot();

    let mut foreign =
        RuntimeLifecycle::new(RuntimeInstanceId::new(92), RuntimeLifecycleConfig::Demand);
    foreign.start().unwrap();
    let foreign_token = foreign
        .admit_demand_step()
        .unwrap()
        .step_at(0)
        .unwrap()
        .phases()
        .timeline();
    assert!(matches!(
        timeline.release_due(&foreign, foreign_token, 8),
        Err(RuntimeTimelineError::ForeignInstance { .. })
    ));
    assert_eq!(timeline.snapshot(), original);

    lifecycle.pause().unwrap();
    assert!(matches!(
        timeline.release_due(&lifecycle, token, 8),
        Err(RuntimeTimelineError::WrongLifecycleState { .. })
    ));
    assert_eq!(timeline.snapshot(), original);
    lifecycle.resume().unwrap();
    let resumed = lifecycle.admit_demand_step().unwrap().step_at(0).unwrap();
    assert!(matches!(
        timeline.release_due(&lifecycle, resumed.phases().timeline(), 8),
        Err(RuntimeTimelineError::StaleBinding { .. })
    ));
    assert_eq!(timeline.snapshot(), original);

    let (mut faulted_lifecycle, mut faulted_timeline, faulted_token) = setup();
    let faulted_original = faulted_timeline.snapshot();
    faulted_lifecycle
        .report_fault(runtime_lifecycle::RuntimeFault::OwnerReported)
        .unwrap();
    assert!(matches!(
        faulted_timeline.release_due(&faulted_lifecycle, faulted_token, 8),
        Err(RuntimeTimelineError::LifecycleFaulted)
    ));
    assert_eq!(faulted_timeline.snapshot(), faulted_original);

    let (mut shutdown_lifecycle, mut shutdown_timeline, shutdown_token) = setup();
    let shutdown_original = shutdown_timeline.snapshot();
    shutdown_lifecycle.shutdown().unwrap();
    assert!(matches!(
        shutdown_timeline.release_due(&shutdown_lifecycle, shutdown_token, 8),
        Err(RuntimeTimelineError::LifecycleShutdown)
    ));
    assert_eq!(shutdown_timeline.snapshot(), shutdown_original);
}

#[test]
fn release_requires_exact_timeline_phase_and_next_admitted_step() {
    let (mut lifecycle, mut timeline, token) = setup();
    let next = lifecycle.admit_demand_step().unwrap().step_at(0).unwrap();
    assert!(matches!(
        timeline.release_due(&lifecycle, next.phases().schedule(), 8),
        Err(RuntimeTimelineError::WrongPhase { .. })
    ));
    timeline.release_due(&lifecycle, token, 8).unwrap();
    assert!(matches!(
        timeline.release_due(&lifecycle, token, 8),
        Err(RuntimeTimelineError::StepRegression { .. })
    ));
}
