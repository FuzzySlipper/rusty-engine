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
fn direct_payload_contract_is_checked_without_a_product_model() {
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
