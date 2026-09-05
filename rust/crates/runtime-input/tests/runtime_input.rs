use runtime_input::{
    AxisValue, CompiledInputMappings, ControllerAxis, DirectInputIntentDescriptor, InputAxis,
    InputClearReason, InputContext, InputEdge, IntentPhase, IntentProvenance, IntentValueKind,
    KeyboardControl, PhysicalEdge, RuntimeDirectIntentClaim, RuntimeInputBinding,
    RuntimeInputError, RuntimeInputEvent, RuntimeInputFact, RuntimeInputIngress, RuntimeInputLane,
    RuntimeInputMapping, RuntimeInputTrigger, RuntimeIntentValue, RuntimeProductPayload,
};
use runtime_lifecycle::{
    RuntimeFault, RuntimeInstanceId, RuntimeLifecycle, RuntimeLifecycleConfig,
};

fn context() -> InputContext {
    InputContext::new("gameplay").unwrap()
}

fn physical(
    binding: RuntimeInputBinding,
    sequence: u64,
    fact: RuntimeInputFact,
) -> RuntimeInputEvent {
    RuntimeInputEvent::Physical(RuntimeInputIngress::new(binding, sequence, context(), fact))
}

fn axis(value: f32) -> AxisValue {
    AxisValue::new(value).unwrap()
}

fn lifecycle_and_binding() -> (RuntimeLifecycle, RuntimeInputBinding) {
    let mut lifecycle =
        RuntimeLifecycle::new(RuntimeInstanceId::new(41), RuntimeLifecycleConfig::Demand);
    let receipt = lifecycle.start().unwrap();
    let binding = RuntimeInputBinding::new(
        receipt.instance_id(),
        receipt.generation(),
        receipt.control_revision(),
    );
    (lifecycle, binding)
}

fn input_token(lifecycle: &mut RuntimeLifecycle) -> runtime_lifecycle::RuntimePhaseToken {
    lifecycle
        .admit_demand_step()
        .unwrap()
        .step_at(0)
        .unwrap()
        .phases()
        .input_snapshot()
}

fn snapshot(
    lane: &mut RuntimeInputLane,
    lifecycle: &mut RuntimeLifecycle,
) -> Result<
    (
        runtime_input::InputFrame,
        Vec<runtime_input::RuntimeIntentEnvelope>,
    ),
    RuntimeInputError,
> {
    let token = input_token(lifecycle);
    lane.snapshot_for_step(lifecycle, token)
}

fn key(edge: InputEdge) -> RuntimeInputTrigger {
    RuntimeInputTrigger::Key {
        code: KeyboardControl::KeyW,
        edge,
        chord: Vec::new(),
        context: Some(context()),
    }
}

fn compiled_mappings() -> CompiledInputMappings {
    CompiledInputMappings::standard(
        vec![
            DirectInputIntentDescriptor::new("move.forward", IntentValueKind::Digital).unwrap(),
            DirectInputIntentDescriptor::new("look.horizontal", IntentValueKind::Axis).unwrap(),
            DirectInputIntentDescriptor::product_payload(
                "inventory.drop",
                "example.inventory.drop.v1",
            )
            .unwrap(),
        ],
        vec![
            RuntimeInputMapping::new("w-held", "move.forward", key(InputEdge::Held)).unwrap(),
            RuntimeInputMapping::new("w-pressed", "move.forward", key(InputEdge::Pressed)).unwrap(),
            RuntimeInputMapping::new("w-released", "move.forward", key(InputEdge::Released))
                .unwrap(),
            RuntimeInputMapping::new(
                "pointer-look",
                "look.horizontal",
                RuntimeInputTrigger::PointerAxis {
                    axis: InputAxis::X,
                    context: Some(context()),
                },
            )
            .unwrap(),
            RuntimeInputMapping::new(
                "controller-look",
                "look.horizontal",
                RuntimeInputTrigger::ControllerAxis {
                    axis: ControllerAxis::Axis0,
                    context: Some(context()),
                },
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

#[test]
fn input_batch_rolls_back_a_valid_prefix_and_preserves_pending_held_state() {
    let (mut lifecycle, binding) = lifecycle_and_binding();
    let mut lane = RuntimeInputLane::new(compiled_mappings(), binding, context());
    let initial = physical(
        binding,
        0,
        RuntimeInputFact::Key {
            code: KeyboardControl::KeyW,
            edge: PhysicalEdge::Pressed,
        },
    );
    lane.ingest(initial.clone()).unwrap();

    let invalid_batch = [
        physical(
            binding,
            1,
            RuntimeInputFact::PointerDelta {
                x: axis(0.5),
                y: axis(-0.25),
            },
        ),
        physical(
            binding,
            2,
            RuntimeInputFact::ControllerAxis {
                axis: ControllerAxis::Axis0,
                value: axis(1.01),
            },
        ),
    ];
    assert!(matches!(
        lane.ingest_batch(&invalid_batch),
        Err(RuntimeInputError::InvalidControllerAxisValue)
    ));
    assert_eq!(lane.last_sequence(), Some(0));

    let correct = physical(
        binding,
        1,
        RuntimeInputFact::PointerDelta {
            x: axis(0.25),
            y: axis(0.0),
        },
    );
    let receipt = lane.ingest_batch(std::slice::from_ref(&correct)).unwrap();
    assert_eq!(receipt.submitted_count(), 1);
    assert_eq!(receipt.accepted_count(), 1);
    assert_eq!(receipt.dropped_count(), 0);
    assert_eq!(receipt.accepted_through(), Some(1));
    assert_eq!(receipt.consumed_through(), Some(1));
    assert_eq!(receipt.next_sequence(), Some(2));
    assert_eq!(receipt.accepted_indices(), &[0]);

    // A lane that never saw the failed prefix must produce the same frame and
    // envelopes. This covers both the held key and the pending press/axis
    // mappings that are not directly exposed as mutable implementation state.
    let (mut expected_lifecycle, expected_binding) = lifecycle_and_binding();
    let mut expected = RuntimeInputLane::new(compiled_mappings(), expected_binding, context());
    expected.ingest(initial).unwrap();
    expected
        .ingest(physical(
            expected_binding,
            1,
            RuntimeInputFact::PointerDelta {
                x: axis(0.25),
                y: axis(0.0),
            },
        ))
        .unwrap();
    assert_eq!(
        snapshot(&mut lane, &mut lifecycle).unwrap(),
        snapshot(&mut expected, &mut expected_lifecycle).unwrap()
    );
}

#[test]
fn input_batch_drops_safe_stale_duplicates_without_changing_frontier_or_state() {
    let (mut lifecycle, binding) = lifecycle_and_binding();
    let mut lane = RuntimeInputLane::new(compiled_mappings(), binding, context());
    let initial = physical(
        binding,
        0,
        RuntimeInputFact::Key {
            code: KeyboardControl::KeyW,
            edge: PhysicalEdge::Pressed,
        },
    );
    lane.ingest(initial.clone()).unwrap();
    lane.ingest(physical(
        binding,
        1,
        RuntimeInputFact::PointerDelta {
            x: axis(0.5),
            y: axis(-0.25),
        },
    ))
    .unwrap();

    let stale = physical(
        binding,
        1,
        RuntimeInputFact::PointerDelta {
            x: axis(4.0),
            y: axis(4.0),
        },
    );
    let receipt = lane.ingest_batch(std::slice::from_ref(&stale)).unwrap();
    assert_eq!(receipt.submitted_count(), 1);
    assert_eq!(receipt.accepted_count(), 0);
    assert_eq!(receipt.dropped_count(), 1);
    assert_eq!(receipt.accepted_through(), None);
    assert_eq!(receipt.consumed_through(), Some(1));
    assert_eq!(receipt.next_sequence(), Some(2));
    assert!(receipt.accepted_indices().is_empty());
    assert_eq!(lane.last_sequence(), Some(1));

    let correct = physical(
        binding,
        2,
        RuntimeInputFact::PointerDelta {
            x: axis(0.25),
            y: axis(0.0),
        },
    );
    let correct_receipt = lane.ingest_batch(std::slice::from_ref(&correct)).unwrap();
    assert_eq!(correct_receipt.accepted_count(), 1);
    assert_eq!(correct_receipt.accepted_through(), Some(2));
    assert_eq!(correct_receipt.next_sequence(), Some(3));

    let (mut expected_lifecycle, expected_binding) = lifecycle_and_binding();
    let mut expected = RuntimeInputLane::new(compiled_mappings(), expected_binding, context());
    expected.ingest(initial).unwrap();
    expected
        .ingest(physical(
            expected_binding,
            1,
            RuntimeInputFact::PointerDelta {
                x: axis(0.5),
                y: axis(-0.25),
            },
        ))
        .unwrap();
    expected
        .ingest(physical(
            expected_binding,
            2,
            RuntimeInputFact::PointerDelta {
                x: axis(0.25),
                y: axis(0.0),
            },
        ))
        .unwrap();

    assert_eq!(
        snapshot(&mut lane, &mut lifecycle).unwrap(),
        snapshot(&mut expected, &mut expected_lifecycle).unwrap()
    );
}

fn assert_input_snapshot_is_lifecycle_fenced(transition: fn(&mut RuntimeLifecycle)) {
    let (mut lifecycle, binding) = lifecycle_and_binding();
    let mut lane = RuntimeInputLane::new(compiled_mappings(), binding, context());
    let token = input_token(&mut lifecycle);
    transition(&mut lifecycle);
    assert!(matches!(
        lane.snapshot_for_step(&lifecycle, token),
        Err(RuntimeInputError::LifecycleValidation)
    ));
}

#[test]
fn direct_runtime_descriptors_preserve_physical_edges_and_order() {
    let (mut lifecycle, binding) = lifecycle_and_binding();
    let mut lane = RuntimeInputLane::new(compiled_mappings(), binding, context());
    lane.ingest(physical(
        binding,
        0,
        RuntimeInputFact::Key {
            code: KeyboardControl::KeyW,
            edge: PhysicalEdge::Pressed,
        },
    ))
    .unwrap();
    lane.ingest(RuntimeInputEvent::DirectIntent(
        RuntimeDirectIntentClaim::new(
            binding,
            1,
            context(),
            "move.forward",
            RuntimeIntentValue::Digital { active: true },
        )
        .unwrap(),
    ))
    .unwrap();
    lane.ingest(physical(
        binding,
        2,
        RuntimeInputFact::Key {
            code: KeyboardControl::KeyW,
            edge: PhysicalEdge::Released,
        },
    ))
    .unwrap();

    let (_, envelopes) = snapshot(&mut lane, &mut lifecycle).unwrap();
    assert_eq!(
        envelopes
            .iter()
            .map(|entry| entry.sequence())
            .collect::<Vec<_>>(),
        [0, 1, 2]
    );
    assert_eq!(
        envelopes
            .iter()
            .map(|entry| entry.phase())
            .collect::<Vec<_>>(),
        [
            IntentPhase::Pressed,
            IntentPhase::DirectUi,
            IntentPhase::Released
        ]
    );
    assert!(envelopes
        .iter()
        .all(|entry| entry.intent() == "move.forward"));
    assert_eq!(
        envelopes[0].descriptor().payload(),
        &serde_json::Value::Null
    );
    assert_eq!(
        envelopes[0].provenance(),
        &IntentProvenance::Physical {
            mapping_id: "w-pressed".into()
        }
    );
}

#[test]
fn held_axes_and_clear_keep_state_owned_by_the_neutral_lane() {
    let (mut lifecycle, binding) = lifecycle_and_binding();
    let mut lane = RuntimeInputLane::new(compiled_mappings(), binding, context());
    lane.ingest(physical(
        binding,
        0,
        RuntimeInputFact::Key {
            code: KeyboardControl::KeyW,
            edge: PhysicalEdge::Pressed,
        },
    ))
    .unwrap();
    lane.ingest(physical(
        binding,
        1,
        RuntimeInputFact::PointerDelta {
            x: axis(0.5),
            y: axis(-0.25),
        },
    ))
    .unwrap();
    let (frame, envelopes) = snapshot(&mut lane, &mut lifecycle).unwrap();
    assert!(frame.keyboard().iter().any(|button| button.held()));
    assert_eq!(frame.pointer().0.value(), 0.5);
    assert!(envelopes.iter().any(|entry| {
        entry.intent() == "look.horizontal"
            && entry.value() == RuntimeIntentValue::Axis { value: axis(0.5) }
    }));

    lane.ingest(physical(
        binding,
        2,
        RuntimeInputFact::Clear {
            reason: InputClearReason::FocusLoss,
        },
    ))
    .unwrap();
    let (frame, envelopes) = snapshot(&mut lane, &mut lifecycle).unwrap();
    assert!(frame.keyboard().is_empty());
    assert_eq!(frame.pointer().0.value(), 0.0);
    assert!(envelopes.is_empty());
}

#[test]
fn lifecycle_tokens_fence_wrong_phase_foreign_and_stale_bindings() {
    let (mut lifecycle, binding) = lifecycle_and_binding();
    let mut lane = RuntimeInputLane::new(compiled_mappings(), binding, context());
    let admission = lifecycle.admit_demand_step().unwrap().step_at(0).unwrap();
    assert!(matches!(
        lane.snapshot_for_step(&lifecycle, admission.phases().schedule()),
        Err(RuntimeInputError::WrongSnapshotPhase)
    ));

    let mut foreign =
        RuntimeLifecycle::new(RuntimeInstanceId::new(42), RuntimeLifecycleConfig::Demand);
    foreign.start().unwrap();
    let foreign_token = input_token(&mut foreign);
    assert!(matches!(
        lane.snapshot_for_step(&foreign, foreign_token),
        Err(RuntimeInputError::BindingMismatch)
    ));

    lifecycle.pause().unwrap();
    assert!(matches!(
        lane.snapshot_for_step(&lifecycle, admission.phases().input_snapshot()),
        Err(RuntimeInputError::LifecycleValidation)
    ));

    assert_input_snapshot_is_lifecycle_fenced(|lifecycle| {
        lifecycle.report_fault(RuntimeFault::OwnerReported).unwrap();
    });
    assert_input_snapshot_is_lifecycle_fenced(|lifecycle| {
        lifecycle.shutdown().unwrap();
    });
}

#[test]
fn monotonic_rebind_clears_state_and_disposal_is_terminal() {
    let (mut lifecycle, binding) = lifecycle_and_binding();
    let mut lane = RuntimeInputLane::new(compiled_mappings(), binding, context());
    lane.ingest(physical(
        binding,
        0,
        RuntimeInputFact::Key {
            code: KeyboardControl::KeyW,
            edge: PhysicalEdge::Pressed,
        },
    ))
    .unwrap();

    lifecycle.restart().unwrap();
    let restart = RuntimeInputBinding::new(
        lifecycle.instance_id(),
        lifecycle.generation(),
        lifecycle.control_revision(),
    );
    lane.rebind(restart, context(), InputClearReason::Restart)
        .unwrap();
    let token = input_token(&mut lifecycle);
    let (frame, envelopes) = lane.snapshot_for_step(&lifecycle, token).unwrap();
    assert!(frame.keyboard().is_empty());
    assert!(envelopes.is_empty());

    assert!(matches!(
        lane.rebind(binding, context(), InputClearReason::Restart),
        Err(RuntimeInputError::InvalidRebindClear)
    ));
    lane.dispose();
    assert!(matches!(
        lane.ingest(physical(
            restart,
            1,
            RuntimeInputFact::Key {
                code: KeyboardControl::KeyW,
                edge: PhysicalEdge::Pressed,
            },
        )),
        Err(RuntimeInputError::Disposed)
    ));
}

#[test]
fn direct_payload_contract_is_checked_by_the_neutral_input_lane() {
    let (mut lifecycle, binding) = lifecycle_and_binding();
    let mut lane = RuntimeInputLane::new(compiled_mappings(), binding, context());
    let payload = RuntimeProductPayload::new(
        "example.inventory.drop.v1",
        serde_json::json!({"sourceSlot": 3, "targetSlot": 5}),
    )
    .unwrap();
    lane.ingest(RuntimeInputEvent::DirectIntent(
        RuntimeDirectIntentClaim::new(
            binding,
            0,
            context(),
            "inventory.drop",
            RuntimeIntentValue::ProductPayload { payload },
        )
        .unwrap(),
    ))
    .unwrap();
    let (_, envelopes) = snapshot(&mut lane, &mut lifecycle).unwrap();
    assert_eq!(envelopes.len(), 1);
    assert_eq!(
        envelopes[0].descriptor().payload_contract(),
        Some("example.inventory.drop.v1")
    );
    let RuntimeIntentValue::ProductPayload { payload } = envelopes[0].value() else {
        panic!("expected product payload");
    };
    assert_eq!(payload.contract(), "example.inventory.drop.v1");
    assert_eq!(
        payload.data(),
        &serde_json::json!({"sourceSlot": 3, "targetSlot": 5})
    );

    let mismatch = RuntimeDirectIntentClaim::new(
        binding,
        1,
        context(),
        "inventory.drop",
        RuntimeIntentValue::ProductPayload {
            payload: RuntimeProductPayload::new(
                "example.inventory.equip.v1",
                serde_json::json!({}),
            )
            .unwrap(),
        },
    )
    .unwrap();
    assert!(matches!(
        lane.ingest(RuntimeInputEvent::DirectIntent(mismatch)),
        Err(RuntimeInputError::ProductPayloadContractMismatch)
    ));
}

#[test]
fn direct_claims_fail_closed_for_unknown_kind_axis_range_and_pending_overflow() {
    let (mut lifecycle, binding) = lifecycle_and_binding();

    let mut unknown = RuntimeInputLane::new(compiled_mappings(), binding, context());
    assert!(matches!(
        unknown.ingest(RuntimeInputEvent::DirectIntent(
            RuntimeDirectIntentClaim::new(
                binding,
                0,
                context(),
                "missing.intent",
                RuntimeIntentValue::Digital { active: true },
            )
            .unwrap(),
        )),
        Err(RuntimeInputError::UnknownIntent)
    ));

    let mut mismatch = RuntimeInputLane::new(compiled_mappings(), binding, context());
    assert!(matches!(
        mismatch.ingest(RuntimeInputEvent::DirectIntent(
            RuntimeDirectIntentClaim::new(
                binding,
                0,
                context(),
                "move.forward",
                RuntimeIntentValue::Axis { value: axis(0.0) },
            )
            .unwrap(),
        )),
        Err(RuntimeInputError::IntentValueKindMismatch)
    ));
    assert!(matches!(
        RuntimeDirectIntentClaim::new(
            binding,
            0,
            context(),
            "look.horizontal",
            RuntimeIntentValue::Axis { value: axis(1.01) },
        ),
        Err(RuntimeInputError::InvalidDirectIntentAxisValue)
    ));

    let mut controller_bound = RuntimeInputLane::new(compiled_mappings(), binding, context());
    assert!(matches!(
        controller_bound.ingest(physical(
            binding,
            0,
            RuntimeInputFact::ControllerAxis {
                axis: ControllerAxis::Axis0,
                value: axis(1.01),
            },
        )),
        Err(RuntimeInputError::InvalidControllerAxisValue)
    ));

    let mut overflow = RuntimeInputLane::new(compiled_mappings(), binding, context());
    for sequence in 0..runtime_input::MAX_PENDING_INGRESS as u64 {
        overflow
            .ingest(RuntimeInputEvent::DirectIntent(
                RuntimeDirectIntentClaim::new(
                    binding,
                    sequence,
                    context(),
                    "move.forward",
                    RuntimeIntentValue::Digital { active: true },
                )
                .unwrap(),
            ))
            .unwrap();
    }
    assert!(matches!(
        overflow.ingest(RuntimeInputEvent::DirectIntent(
            RuntimeDirectIntentClaim::new(
                binding,
                runtime_input::MAX_PENDING_INGRESS as u64,
                context(),
                "move.forward",
                RuntimeIntentValue::Digital { active: true },
            )
            .unwrap(),
        )),
        Err(RuntimeInputError::PendingIngressOverflow)
    ));
    let (frame, envelopes) = snapshot(&mut overflow, &mut lifecycle).unwrap();
    assert!(frame.keyboard().is_empty());
    assert!(envelopes.is_empty());
}

#[test]
fn wire_decode_retains_canonical_host_facts_and_rejects_bad_values() {
    let events = runtime_input::decode_runtime_input_wire_events_json(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../fixtures/runtime-input/host-neutral-input-envelope.json"
    )))
    .unwrap();
    assert_eq!(events.len(), 17);
    assert_eq!(events[0].sequence(), 0);
    assert_eq!(events[16].sequence(), u64::MAX);
    assert_eq!(events[16].runtime().instance_id().value(), u64::MAX);
    for invalid in [
        br#"{"runtime":{"instanceId":"7","generation":"3","controlRevision":"11"},"sequence":"01","context":"gameplay","fact":{"kind":"clear","reason":"focus-loss"}}"# as &[u8],
        br#"{"runtime":{"instanceId":"7","generation":"3","controlRevision":"11"},"sequence":"0","context":"gameplay","intent":"move.forward","value":{"kind":"unknown","active":true}}"#,
        br#"{"runtime":{"instanceId":"7","generation":"3","controlRevision":"11"},"sequence":"0","context":"gameplay","intent":"look.horizontal","value":{"kind":"axis","value":1.1}}"#,
        br#"{"runtime":{"instanceId":"7","generation":"3","controlRevision":"11"},"sequence":"0","context":"gameplay","fact":{"kind":"controller-axis","axis":"axis-0","value":1.1}}"#,
        br#"{"runtime":{"instanceId":"7","generation":"3","controlRevision":"11"},"sequence":"0","context":"gameplay","fact":{"kind":"clear","reason":"focus-loss","extra":true}}"#,
    ] {
        assert!(runtime_input::decode_runtime_input_wire_event_json(invalid).is_err());
    }
    assert_eq!(axis(-0.0).value().to_bits(), 0.0_f32.to_bits());
}

#[test]
fn browser_digit_controls_use_the_same_strict_wire_names_as_letters() {
    let controls = [
        KeyboardControl::Digit0,
        KeyboardControl::Digit1,
        KeyboardControl::Digit2,
        KeyboardControl::Digit3,
        KeyboardControl::Digit4,
        KeyboardControl::Digit5,
        KeyboardControl::Digit6,
        KeyboardControl::Digit7,
        KeyboardControl::Digit8,
        KeyboardControl::Digit9,
    ];
    for (digit, control) in controls.into_iter().enumerate() {
        let code = format!("digit-{digit}");
        let wire = serde_json::json!({
            "runtime": {"instanceId": "41", "generation": "1", "controlRevision": "1"},
            "sequence": "14", "context": "gameplay.default",
            "fact": {"kind": "key", "code": code, "edge": "pressed"}
        });
        let decoded = runtime_input::decode_runtime_input_wire_event_json(
            &serde_json::to_vec(&wire).unwrap(),
        )
        .unwrap();
        assert!(matches!(decoded, RuntimeInputEvent::Physical(event)
            if event.fact() == &RuntimeInputFact::Key { code: control, edge: PhysicalEdge::Pressed }));
        assert_eq!(serde_json::to_value(control).unwrap(), code);
    }
}

#[test]
fn neutral_mapping_construction_validates_identity_and_value_kind() {
    assert_eq!(
        serde_json::to_string(&IntentValueKind::ProductPayload).unwrap(),
        "\"product-payload\""
    );
    assert!(matches!(
        DirectInputIntentDescriptor::new("inventory.drop", IntentValueKind::ProductPayload),
        Err(RuntimeInputError::DirectIntentPayloadUnsupported)
    ));
    assert!(matches!(
        CompiledInputMappings::standard(
            vec![
                DirectInputIntentDescriptor::new("move.forward", IntentValueKind::Digital).unwrap()
            ],
            vec![RuntimeInputMapping::new(
                "bad-axis",
                "move.forward",
                RuntimeInputTrigger::PointerAxis {
                    axis: InputAxis::X,
                    context: Some(context()),
                },
            )
            .unwrap()],
        ),
        Err(RuntimeInputError::IntentValueKindMismatch)
    ));
}
