use core_math::Vec3;
use project_code_host::{
    decode_project_snapshot, encode_project_snapshot, HostInputEvent, ProjectCodeRuntime,
    ProjectDecision, ProjectDecisionBatch, ProjectFact, ProjectHostError, ProjectScheduleRequest,
    ProjectStateUpdate, ProjectWorldCommand, PROJECT_HOST_SCHEMA_VERSION,
};
use serde_json::json;

fn initial_state(auto_close_ticks: Option<u64>) -> serde_json::Value {
    json!({
        "doorState": "closed",
        "closedTranslation": [0.0, 0.0, 0.0],
        "openTranslation": [0.0, 3.0, 0.0],
        "autoCloseTicks": auto_close_ticks,
    })
}

fn open_decision(
    wave: &project_code_host::ProjectInvocationWave,
    door: u64,
    auto_close_ticks: Option<u64>,
) -> ProjectDecisionBatch {
    let invocation = &wave.invocations[0];
    ProjectDecisionBatch {
        schema_version: PROJECT_HOST_SCHEMA_VERSION,
        expected_world_revision: wave.expected_world_revision,
        decisions: vec![ProjectDecision {
            invocation_id: invocation.invocation_id,
            commands: vec![
                ProjectWorldCommand::SetTranslation {
                    entity: door,
                    translation: [0.0, 3.0, 0.0],
                },
                ProjectWorldCommand::SetCollisionEnabled {
                    entity: door,
                    enabled: false,
                },
            ],
            state_update: Some(ProjectStateUpdate {
                expected_revision: invocation.state.revision,
                version: invocation.state.version,
                payload: initial_state(auto_close_ticks)
                    .as_object()
                    .map(|state| {
                        let mut state = state.clone();
                        state.insert("doorState".to_owned(), json!("open"));
                        serde_json::Value::Object(state)
                    })
                    .expect("object state"),
            }),
            schedules: auto_close_ticks
                .map(|due_after_ticks| {
                    vec![ProjectScheduleRequest::Upsert {
                        message_id: "close".to_owned(),
                        due_after_ticks,
                        message_kind: "close".to_owned(),
                        payload: json!({}),
                    }]
                })
                .unwrap_or_default(),
            facts: vec![ProjectFact {
                kind: "door.opened".to_owned(),
                version: 1,
                payload: json!({ "door": door }),
            }],
        }],
    }
}

fn close_decision(
    wave: &project_code_host::ProjectInvocationWave,
    door: u64,
) -> ProjectDecisionBatch {
    let invocation = &wave.invocations[0];
    let mut state = invocation
        .state
        .payload
        .as_object()
        .expect("object")
        .clone();
    state.insert("doorState".to_owned(), json!("closed"));
    ProjectDecisionBatch {
        schema_version: PROJECT_HOST_SCHEMA_VERSION,
        expected_world_revision: wave.expected_world_revision,
        decisions: vec![ProjectDecision {
            invocation_id: invocation.invocation_id,
            commands: vec![
                ProjectWorldCommand::SetCollisionEnabled {
                    entity: door,
                    enabled: true,
                },
                ProjectWorldCommand::SetTranslation {
                    entity: door,
                    translation: [0.0, 0.0, 0.0],
                },
            ],
            state_update: Some(ProjectStateUpdate {
                expected_revision: invocation.state.revision,
                version: invocation.state.version,
                payload: serde_json::Value::Object(state),
            }),
            schedules: Vec::new(),
            facts: vec![ProjectFact {
                kind: "door.closed".to_owned(),
                version: 1,
                payload: json!({ "door": door }),
            }],
        }],
    }
}

#[test]
fn project_decisions_open_and_close_through_one_batch_per_wave() {
    let (ids, mut runtime) =
        ProjectCodeRuntime::security_door(initial_state(Some(3))).expect("fixture");
    let interaction = runtime
        .begin_interaction(ids.actor, ids.switch)
        .expect("interaction wave");
    assert_eq!(interaction.invocations.len(), 1);
    assert!(matches!(
        interaction.invocations[0].events[0],
        HostInputEvent::Interaction { .. }
    ));
    assert_eq!(interaction.invocations[0].related[0].entity, ids.door.raw());

    let opened = runtime
        .apply_decisions(open_decision(&interaction, ids.door.raw(), Some(3)))
        .expect("open decision");
    assert_eq!(opened.revision_after, 1);
    assert_eq!(opened.engine_facts.len(), 2);
    assert_eq!(opened.project_facts[0].kind, "door.opened");
    assert_eq!(opened.pending_message_count, 1);
    let view = runtime.world().view(ids.door).expect("door");
    assert_eq!(
        view.transform.expect("transform").translation,
        Vec3::new(0.0, 3.0, 0.0)
    );
    assert!(!view.collision.expect("collision").enabled);

    assert!(runtime.advance_by(2).expect("advance").is_none());
    let due = runtime
        .advance_by(1)
        .expect("advance due")
        .expect("message wave");
    assert!(matches!(
        due.invocations[0].events[0],
        HostInputEvent::Message { .. }
    ));
    let closed = runtime
        .apply_decisions(close_decision(&due, ids.door.raw()))
        .expect("close decision");
    assert_eq!(closed.revision_after, 2);
    let view = runtime.world().view(ids.door).expect("door");
    assert_eq!(view.transform.expect("transform").translation, Vec3::ZERO);
    assert!(view.collision.expect("collision").enabled);
}

#[test]
fn rejected_world_batch_preserves_project_state_and_schedule() {
    let (ids, mut runtime) =
        ProjectCodeRuntime::security_door(initial_state(Some(3))).expect("fixture");
    let interaction = runtime
        .begin_interaction(ids.actor, ids.switch)
        .expect("interaction");
    let invocation = &interaction.invocations[0];
    let invalid = ProjectDecisionBatch {
        schema_version: PROJECT_HOST_SCHEMA_VERSION,
        expected_world_revision: interaction.expected_world_revision,
        decisions: vec![ProjectDecision {
            invocation_id: invocation.invocation_id,
            commands: vec![ProjectWorldCommand::SetTranslation {
                entity: ids.door.raw(),
                translation: [0.0, 3.0, 0.0],
            }],
            state_update: Some(ProjectStateUpdate {
                expected_revision: 0,
                version: 1,
                payload: json!({ "doorState": "open" }),
            }),
            schedules: vec![ProjectScheduleRequest::Upsert {
                message_id: "close".to_owned(),
                due_after_ticks: 3,
                message_kind: "close".to_owned(),
                payload: json!({}),
            }],
            facts: Vec::new(),
        }],
    };
    assert!(matches!(
        runtime.apply_decisions(invalid),
        Err(ProjectHostError::WorldBatch(_))
    ));
    let readout = runtime.readout();
    assert_eq!(readout.world_revision, 0);
    assert_eq!(readout.pending_message_count, 0);
    assert_eq!(readout.state_records[0].revision, 0);
    assert_eq!(readout.state_records[0].payload["doorState"], "closed");
    assert!(readout.pending_invocation);

    runtime
        .apply_decisions(open_decision(&interaction, ids.door.raw(), Some(3)))
        .expect("retry valid decision");
    assert!(!runtime.readout().pending_invocation);
}

#[test]
fn save_reopen_preserves_project_state_and_stable_message() {
    let (ids, mut runtime) =
        ProjectCodeRuntime::security_door(initial_state(Some(5))).expect("fixture");
    let interaction = runtime
        .begin_interaction(ids.actor, ids.switch)
        .expect("interaction");
    runtime
        .apply_decisions(open_decision(&interaction, ids.door.raw(), Some(5)))
        .expect("open");
    runtime.advance_by(2).expect("advance");
    let saved = encode_project_snapshot(&runtime).expect("save");

    let mut restored = decode_project_snapshot(&saved).expect("restore");
    let readout = restored.readout();
    assert_eq!(readout.tick, 2);
    assert_eq!(readout.pending_message_count, 1);
    assert_eq!(readout.state_records[0].payload["doorState"], "open");
    assert!(readout.project_facts.is_empty());

    let due = restored
        .advance_by(3)
        .expect("advance")
        .expect("due message");
    restored
        .apply_decisions(close_decision(&due, ids.door.raw()))
        .expect("close");
    assert_eq!(
        restored.readout().state_records[0].payload["doorState"],
        "closed"
    );
}

#[test]
fn snapshot_is_rejected_while_project_code_owns_an_invocation() {
    let (ids, mut runtime) =
        ProjectCodeRuntime::security_door(initial_state(None)).expect("fixture");
    runtime
        .begin_interaction(ids.actor, ids.switch)
        .expect("interaction");
    assert!(matches!(
        encode_project_snapshot(&runtime),
        Err(ProjectHostError::SnapshotWhileInvocationPending)
    ));
}

#[test]
fn tagged_wire_variants_use_camel_case_for_their_fields() {
    let request = ProjectScheduleRequest::Upsert {
        message_id: "close".to_owned(),
        due_after_ticks: 3,
        message_kind: "close".to_owned(),
        payload: json!({}),
    };
    assert_eq!(
        serde_json::to_value(request).expect("serialize schedule"),
        json!({
            "op": "upsert",
            "messageId": "close",
            "dueAfterTicks": 3,
            "messageKind": "close",
            "payload": {},
        })
    );

    let event: HostInputEvent = serde_json::from_value(json!({
        "kind": "message",
        "messageId": "close",
        "messageKind": "close",
        "payload": {},
    }))
    .expect("deserialize message event");
    assert!(matches!(
        event,
        HostInputEvent::Message {
            message_id,
            message_kind,
            ..
        } if message_id == "close" && message_kind == "close"
    ));
}
