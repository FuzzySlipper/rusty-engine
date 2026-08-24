use product_model::{
    admit_product_composition, link_admitted_product_composition, validate_product_manifest,
    CapabilityAccess, CapabilityAvailability, CapabilityBinding, CapabilityBudget, CapabilityKind,
    CapabilityMetadata, CapabilityProvenance, CapabilityUses, CompiledCompositionCandidate,
    ControllerAxis, InputAxis, InputEdge, InputMapEntry, InputTrigger, IntentValueKind,
    KeyboardControl, LifecycleMode, ProductIntentDescriptor, ProductKernelCapabilityDescriptor,
    ProductManifestCandidate,
};
use runtime_input::{
    AxisValue, CompiledInputMappings, InputClearReason, InputContext, IntentPhase,
    IntentProvenance, PhysicalEdge, RuntimeDirectIntentClaim, RuntimeInputBinding,
    RuntimeInputError, RuntimeInputEvent, RuntimeInputFact, RuntimeInputIngress, RuntimeInputLane,
    RuntimeIntentValue,
};
use runtime_lifecycle::{
    RuntimeControlRevision, RuntimeFault, RuntimeGeneration, RuntimeInstanceId, RuntimeLifecycle,
    RuntimeLifecycleConfig,
};

#[test]
fn physical_edges_and_direct_claims_keep_one_ingress_order() {
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
        envelopes[0].descriptor().capability_target(),
        "kernel.move-forward"
    );
    assert_eq!(
        envelopes[0].descriptor().capability_payload(),
        &serde_json::json!({ "semantic": "move-forward" })
    );
}

#[test]
fn held_mapping_emits_once_per_step_and_clear_drops_every_pending_state() {
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
    let (_, first) = snapshot(&mut lane, &mut lifecycle).unwrap();
    // The press is source-derived; the held readout is snapshot-synthetic and
    // therefore follows all real envelopes at the same source sequence.
    assert_eq!(
        first.iter().map(|entry| entry.phase()).collect::<Vec<_>>(),
        [IntentPhase::Pressed, IntentPhase::Held]
    );
    let (_, second) = snapshot(&mut lane, &mut lifecycle).unwrap();
    assert_eq!(
        second.iter().map(|entry| entry.phase()).collect::<Vec<_>>(),
        [IntentPhase::Held]
    );

    lane.ingest(physical(
        binding,
        1,
        RuntimeInputFact::Clear {
            reason: InputClearReason::FocusLoss,
        },
    ))
    .unwrap();
    let (_, cleared) = snapshot(&mut lane, &mut lifecycle).unwrap();
    assert!(cleared.is_empty());
}

#[test]
fn snapshot_requires_the_lifecycle_input_phase_and_exact_binding() {
    let (mut lifecycle, binding) = lifecycle_and_binding();
    let mut lane = RuntimeInputLane::new(compiled_mappings(), binding, context());
    let admission = lifecycle.admit_demand_step().unwrap().step_at(0).unwrap();
    assert!(matches!(
        lane.snapshot_for_step(&lifecycle, admission.phases().schedule()),
        Err(runtime_input::RuntimeInputError::WrongSnapshotPhase)
    ));
    lane.snapshot_for_step(&lifecycle, admission.phases().input_snapshot())
        .unwrap();
    assert!(matches!(
        lane.snapshot_for_step(&lifecycle, admission.phases().input_snapshot()),
        Err(runtime_input::RuntimeInputError::SnapshotOutOfOrder)
    ));
}

#[test]
fn canonical_u64_wire_parser_rejects_javascript_aliases() {
    assert_eq!(
        runtime_input::parse_canonical_u64("18446744073709551615").unwrap(),
        u64::MAX
    );
    for value in ["", "00", "01", "+1", "1.0", " 1", "18446744073709551616"] {
        assert!(runtime_input::parse_canonical_u64(value).is_err());
    }
}

#[test]
fn repeated_keydown_and_same_step_release_have_exact_edges() {
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
        RuntimeInputFact::Key {
            code: KeyboardControl::KeyW,
            edge: PhysicalEdge::Pressed,
        },
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
            .map(|entry| (entry.sequence(), entry.phase()))
            .collect::<Vec<_>>(),
        [(0, IntentPhase::Pressed), (2, IntentPhase::Released)]
    );
}

#[test]
fn chord_activates_on_final_member_and_releases_when_any_member_leaves() {
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
        RuntimeInputFact::Key {
            code: KeyboardControl::ShiftLeft,
            edge: PhysicalEdge::Pressed,
        },
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
    let sprint = envelopes
        .iter()
        .filter_map(|entry| match entry.provenance() {
            IntentProvenance::Physical { mapping_id } if mapping_id.starts_with("sprint-") => {
                Some((entry.sequence(), entry.phase()))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        sprint,
        [(1, IntentPhase::Pressed), (2, IntentPhase::Released)]
    );
}

#[test]
fn pointer_wheel_and_controller_state_have_bounded_step_semantics() {
    let (mut lifecycle, binding) = lifecycle_and_binding();
    let mut lane = RuntimeInputLane::new(compiled_mappings(), binding, context());
    lane.ingest(physical(
        binding,
        0,
        RuntimeInputFact::PointerDelta {
            x: axis(1.0),
            y: axis(2.0),
        },
    ))
    .unwrap();
    lane.ingest(physical(
        binding,
        1,
        RuntimeInputFact::PointerDelta {
            x: axis(3.0),
            y: axis(4.0),
        },
    ))
    .unwrap();
    lane.ingest(physical(
        binding,
        2,
        RuntimeInputFact::Wheel {
            x: axis(0.0),
            y: axis(1.0),
        },
    ))
    .unwrap();
    lane.ingest(physical(
        binding,
        3,
        RuntimeInputFact::ControllerAxis {
            axis: ControllerAxis::Axis0,
            value: axis(0.5),
        },
    ))
    .unwrap();
    let (first, first_envelopes) = snapshot(&mut lane, &mut lifecycle).unwrap();
    assert_eq!(
        (first.pointer().0.value(), first.pointer().1.value()),
        (4.0, 6.0)
    );
    assert_eq!(
        (first.wheel().0.value(), first.wheel().1.value()),
        (0.0, 1.0)
    );
    assert_eq!(
        first
            .controller_axis(ControllerAxis::Axis0)
            .unwrap()
            .value(),
        0.5
    );
    assert!(first_envelopes.iter().any(|entry| {
        entry.sequence() == 3
            && entry.phase() == IntentPhase::Axis
            && entry.value() == RuntimeIntentValue::Axis { value: axis(0.5) }
    }));

    lane.ingest(physical(
        binding,
        4,
        RuntimeInputFact::ControllerAxis {
            axis: ControllerAxis::Axis0,
            value: axis(0.0),
        },
    ))
    .unwrap();
    let (second, envelopes) = snapshot(&mut lane, &mut lifecycle).unwrap();
    assert_eq!(
        (second.pointer().0.value(), second.pointer().1.value()),
        (0.0, 0.0)
    );
    assert_eq!(
        (second.wheel().0.value(), second.wheel().1.value()),
        (0.0, 0.0)
    );
    assert_eq!(
        second
            .controller_axis(ControllerAxis::Axis0)
            .unwrap()
            .value(),
        0.0
    );
    assert!(envelopes.iter().any(|entry| {
        entry.sequence() == 4
            && entry.phase() == IntentPhase::Axis
            && entry.value() == RuntimeIntentValue::Axis { value: axis(0.0) }
    }));
    let (_, persistent) = snapshot(&mut lane, &mut lifecycle).unwrap();
    assert!(persistent.iter().any(|entry| {
        entry.sequence() == 4
            && entry.phase() == IntentPhase::Axis
            && entry.value() == RuntimeIntentValue::Axis { value: axis(0.0) }
    }));
}

#[test]
fn physical_accumulation_failure_clears_held_and_pending_state() {
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
            x: axis(runtime_input::MAX_AXIS_MAGNITUDE),
            y: axis(0.0),
        },
    ))
    .unwrap();
    assert!(matches!(
        lane.ingest(physical(
            binding,
            2,
            RuntimeInputFact::PointerDelta {
                x: axis(runtime_input::MAX_AXIS_MAGNITUDE),
                y: axis(0.0),
            },
        )),
        Err(RuntimeInputError::InvalidAxisValue)
    ));
    let (frame, envelopes) = snapshot(&mut lane, &mut lifecycle).unwrap();
    assert!(frame.keyboard().is_empty());
    assert_eq!(
        (frame.pointer().0.value(), frame.pointer().1.value()),
        (0.0, 0.0)
    );
    assert!(envelopes.is_empty());
}

#[test]
fn context_transition_is_an_ordered_new_context_clear_and_frame_controls_are_canonical() {
    let (mut lifecycle, binding) = lifecycle_and_binding();
    let mut lane = RuntimeInputLane::new(compiled_mappings(), binding, context());
    let menu = InputContext::new("interface.menu").unwrap();
    lane.ingest(RuntimeInputEvent::Physical(RuntimeInputIngress::new(
        binding,
        0,
        menu.clone(),
        RuntimeInputFact::Clear {
            reason: InputClearReason::InteractionModeLoss,
        },
    )))
    .unwrap();
    assert_eq!(lane.context(), &menu);
    lane.ingest(RuntimeInputEvent::DirectIntent(
        RuntimeDirectIntentClaim::new(
            binding,
            1,
            menu,
            "move.forward",
            RuntimeIntentValue::Digital { active: true },
        )
        .unwrap(),
    ))
    .unwrap();
    assert_eq!(snapshot(&mut lane, &mut lifecycle).unwrap().1.len(), 1);

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
        RuntimeInputFact::Key {
            code: KeyboardControl::KeyA,
            edge: PhysicalEdge::Pressed,
        },
    ))
    .unwrap();
    let (frame, _) = snapshot(&mut lane, &mut lifecycle).unwrap();
    assert_eq!(
        frame
            .keyboard()
            .iter()
            .map(|entry| entry.control())
            .collect::<Vec<_>>(),
        [KeyboardControl::KeyA, KeyboardControl::KeyW]
    );
}

#[test]
fn rebind_requires_a_monotonic_epoch_and_disposal_is_terminal() {
    let (_lifecycle, binding) = lifecycle_and_binding();
    let mut lane = RuntimeInputLane::new(compiled_mappings(), binding, context());
    let restart = RuntimeInputBinding::new(
        binding.instance_id(),
        RuntimeGeneration::new(binding.generation().value() + 1),
        RuntimeControlRevision::new(binding.control_revision().value() + 1),
    );
    lane.ingest(physical_at(
        restart,
        0,
        context(),
        RuntimeInputFact::Clear {
            reason: InputClearReason::Restart,
        },
    ))
    .unwrap();
    let revision = RuntimeInputBinding::new(
        restart.instance_id(),
        restart.generation(),
        RuntimeControlRevision::new(restart.control_revision().value() + 1),
    );
    lane.ingest(physical_at(
        revision,
        0,
        context(),
        RuntimeInputFact::Clear {
            reason: InputClearReason::ControlRevisionChange,
        },
    ))
    .unwrap();
    assert!(matches!(
        lane.ingest(physical_at(
            restart,
            0,
            context(),
            RuntimeInputFact::Clear {
                reason: InputClearReason::Restart
            },
        )),
        Err(RuntimeInputError::InvalidRebindClear)
    ));
    lane.dispose();
    assert!(matches!(
        lane.ingest(physical_at(
            revision,
            1,
            context(),
            RuntimeInputFact::Key {
                code: KeyboardControl::KeyW,
                edge: PhysicalEdge::Pressed
            },
        )),
        Err(RuntimeInputError::Disposed)
    ));

    let mut host_disposed = RuntimeInputLane::new(compiled_mappings(), binding, context());
    host_disposed
        .ingest(physical(
            binding,
            0,
            RuntimeInputFact::Clear {
                reason: InputClearReason::Dispose,
            },
        ))
        .unwrap();
    assert!(matches!(
        host_disposed.ingest(physical(
            binding,
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
fn lifecycle_validation_rejects_paused_faulted_and_shutdown_tokens() {
    for transition in [
        LifecycleTransition::Pause,
        LifecycleTransition::Fault,
        LifecycleTransition::Shutdown,
    ] {
        let (mut lifecycle, binding) = lifecycle_and_binding();
        let mut lane = RuntimeInputLane::new(compiled_mappings(), binding, context());
        let token = next_input_token(&mut lifecycle);
        match transition {
            LifecycleTransition::Pause => {
                lifecycle.pause().unwrap();
            }
            LifecycleTransition::Fault => {
                lifecycle.report_fault(RuntimeFault::OwnerReported).unwrap();
            }
            LifecycleTransition::Shutdown => {
                lifecycle.shutdown().unwrap();
            }
        }
        assert!(matches!(
            lane.snapshot_for_step(&lifecycle, token),
            Err(RuntimeInputError::LifecycleValidation)
        ));
    }
}

#[test]
fn direct_claims_fail_closed_for_unknown_kind_and_axis_range_and_pending_overflow() {
    let (_lifecycle, binding) = lifecycle_and_binding();
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
}

#[test]
fn wire_fixture_is_strict_bounded_and_host_parity_complete() {
    let events = runtime_input::decode_runtime_input_wire_events_json(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../fixtures/runtime-input/host-neutral-input-envelope.json"
    )))
    .unwrap();
    assert_eq!(events.len(), 16);
    assert_eq!(events[0].sequence(), 0);
    assert_eq!(events[15].sequence(), u64::MAX);
    assert_eq!(events[15].runtime().instance_id().value(), u64::MAX);
    for invalid in [
        br#"{"runtime":{"instanceId":"7","generation":"3","controlRevision":"11"},"sequence":"0","context":"gameplay","fact":{"kind":"clear","reason":"focus-loss"},"intent":null,"value":null}"# as &[u8],
        br#"{"runtime":{"instanceId":"7","generation":"3","controlRevision":"11"},"sequence":"01","context":"gameplay","fact":{"kind":"clear","reason":"focus-loss"}}"#,
        br#"{"runtime":{"instanceId":"7","generation":"3","controlRevision":"11"},"sequence":"0","context":"gameplay","intent":"look.horizontal","value":{"kind":"axis","value":1.1}}"#,
        br#"{"runtime":{"instanceId":"7","generation":"3","controlRevision":"11"},"sequence":"0","context":"gameplay","fact":{"kind":"controller-axis","axis":"axis-0","value":1.1}}"#,
        br#"{"runtime":{"instanceId":"7","generation":"3","controlRevision":"11"},"sequence":"0","context":"gameplay","fact":{"kind":"clear","reason":"focus-loss","extra":true}}"#,
    ] {
        assert!(runtime_input::decode_runtime_input_wire_event_json(invalid).is_err());
    }
    assert_eq!(axis(-0.0).value().to_bits(), 0.0_f32.to_bits());
}

#[derive(Clone, Copy)]
enum LifecycleTransition {
    Pause,
    Fault,
    Shutdown,
}

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

fn physical_at(
    binding: RuntimeInputBinding,
    sequence: u64,
    input_context: InputContext,
    fact: RuntimeInputFact,
) -> RuntimeInputEvent {
    RuntimeInputEvent::Physical(RuntimeInputIngress::new(
        binding,
        sequence,
        input_context,
        fact,
    ))
}

fn lifecycle_and_binding() -> (RuntimeLifecycle, RuntimeInputBinding) {
    let instance = RuntimeInstanceId::new(41);
    let mut lifecycle = RuntimeLifecycle::new(instance, RuntimeLifecycleConfig::Demand);
    let receipt = lifecycle.start().unwrap();
    let binding = RuntimeInputBinding::new(
        receipt.instance_id(),
        receipt.generation(),
        receipt.control_revision(),
    );
    (lifecycle, binding)
}

fn next_input_token(lifecycle: &mut RuntimeLifecycle) -> runtime_lifecycle::RuntimePhaseToken {
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
    runtime_input::RuntimeInputError,
> {
    let token = next_input_token(lifecycle);
    lane.snapshot_for_step(lifecycle, token)
}

fn compiled_mappings() -> CompiledInputMappings {
    let manifest = validate_product_manifest(ProductManifestCandidate {
        product_id: "example.product".into(),
        composition_entrypoints: vec!["rules/main.ts".into()],
        lifecycle: LifecycleMode::Demand,
        realtime: None,
        kernel_entry: Some("kernel/lib.rs".into()),
        ui_entry: "ui/main.ts".into(),
        content_root: "content".into(),
        compiled_composition_output: "generated/compiled-composition.json".into(),
        admitted_runtime_content_output: "generated/runtime-content".into(),
        product_assembly_output: "generated/product-assembly".into(),
        product_bundle_output: "generated/product-bundle".into(),
        wrappers: vec![],
    })
    .unwrap();
    let admitted = admit_product_composition(
        &manifest,
        CompiledCompositionCandidate {
            product: "example.product".into(),
            intent_descriptors: vec![
                ProductIntentDescriptor {
                    id: "move.forward".into(),
                    value_kind: IntentValueKind::Digital,
                    capability: "move.forward".into(),
                    payload: serde_json::json!({ "semantic": "move-forward" }),
                },
                ProductIntentDescriptor {
                    id: "look.horizontal".into(),
                    value_kind: IntentValueKind::Axis,
                    capability: "look.horizontal".into(),
                    payload: serde_json::json!({ "semantic": "look-horizontal" }),
                },
            ],
            input_map: vec![
                InputMapEntry {
                    id: "w-held".into(),
                    intent: "move.forward".into(),
                    trigger: key(InputEdge::Held),
                },
                InputMapEntry {
                    id: "w-pressed".into(),
                    intent: "move.forward".into(),
                    trigger: key(InputEdge::Pressed),
                },
                InputMapEntry {
                    id: "w-released".into(),
                    intent: "move.forward".into(),
                    trigger: key(InputEdge::Released),
                },
                InputMapEntry {
                    id: "sprint-pressed".into(),
                    intent: "move.forward".into(),
                    trigger: InputTrigger::Key {
                        code: KeyboardControl::KeyW,
                        edge: InputEdge::Pressed,
                        chord: vec![KeyboardControl::ShiftLeft],
                        context: Some("gameplay".into()),
                    },
                },
                InputMapEntry {
                    id: "sprint-released".into(),
                    intent: "move.forward".into(),
                    trigger: InputTrigger::Key {
                        code: KeyboardControl::KeyW,
                        edge: InputEdge::Released,
                        chord: vec![KeyboardControl::ShiftLeft],
                        context: Some("gameplay".into()),
                    },
                },
                InputMapEntry {
                    id: "pointer-look".into(),
                    intent: "look.horizontal".into(),
                    trigger: InputTrigger::PointerAxis {
                        axis: InputAxis::X,
                        context: Some("gameplay".into()),
                    },
                },
                InputMapEntry {
                    id: "wheel-look".into(),
                    intent: "look.horizontal".into(),
                    trigger: InputTrigger::Wheel {
                        axis: InputAxis::Y,
                        context: Some("gameplay".into()),
                    },
                },
                InputMapEntry {
                    id: "controller-look".into(),
                    intent: "look.horizontal".into(),
                    trigger: InputTrigger::ControllerAxis {
                        axis: ControllerAxis::Axis0,
                        context: Some("gameplay".into()),
                    },
                },
            ],
            schedule: vec![],
            gameplay_definitions: vec![],
            timelines: vec![],
            capability_bindings: vec![
                CapabilityBinding {
                    id: "move.forward".into(),
                    target: "kernel.move-forward".into(),
                },
                CapabilityBinding {
                    id: "look.horizontal".into(),
                    target: "kernel.look-horizontal".into(),
                },
            ],
        },
    )
    .unwrap();
    let linked = link_admitted_product_composition(
        admitted,
        &[
            ProductKernelCapabilityDescriptor::new(
                "move-forward",
                CapabilityMetadata::new(
                    CapabilityKind::System,
                    CapabilityUses::INPUT_MAP,
                    CapabilityAvailability::Linkable,
                    CapabilityAccess::new(&[], &[]),
                    CapabilityBudget::new(1_024),
                    CapabilityProvenance::new("example.product", "kernel/input.rs", "move_forward"),
                ),
            ),
            ProductKernelCapabilityDescriptor::new(
                "look-horizontal",
                CapabilityMetadata::new(
                    CapabilityKind::System,
                    CapabilityUses::INPUT_MAP,
                    CapabilityAvailability::Linkable,
                    CapabilityAccess::new(&[], &[]),
                    CapabilityBudget::new(1_024),
                    CapabilityProvenance::new(
                        "example.product",
                        "kernel/input.rs",
                        "look_horizontal",
                    ),
                ),
            ),
        ],
    )
    .unwrap();
    CompiledInputMappings::compile(&linked).unwrap()
}

fn key(edge: InputEdge) -> InputTrigger {
    InputTrigger::Key {
        code: KeyboardControl::KeyW,
        edge,
        chord: vec![],
        context: Some("gameplay".into()),
    }
}

fn axis(value: f32) -> AxisValue {
    AxisValue::new(value).unwrap()
}
