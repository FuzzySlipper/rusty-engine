use product_model::{
    admit_checked_product_composition, admit_product_composition, decode_compiled_composition,
    decode_product_manifest, encode_compiled_composition, engine_capability_descriptors,
    link_admitted_product_composition, validate_compiled_composition,
    validate_engine_capability_descriptors, validate_product_manifest, CapabilityAccess,
    CapabilityAvailability, CapabilityBinding, CapabilityBudget, CapabilityKind,
    CapabilityMetadata, CapabilityProvenance, CapabilityUses, CompiledCompositionCandidate,
    EngineCapability, GameplayDefinition, InputAxis, InputEdge, InputMapEntry, InputTrigger,
    IntentValueKind, KeyboardControl, LifecycleMode, LinkedCapabilityTarget,
    ProductIntentDescriptor, ProductKernelCapabilityDescriptor, ProductManifestCandidate,
    ProductPath, RealtimeClock, ReleaseChannel, ScheduleCadence, ScheduleComposition,
    SchedulePhase, SchedulePhaseDeclaration, ScheduleSystem, Timeline, TimelineStep,
    MAX_COMPILED_COMPOSITION_BYTES, MAX_PRODUCT_KERNEL_CAPABILITIES, MAX_PRODUCT_MANIFEST_BYTES,
    MAX_SCHEDULE_ACCESS_DECLARATIONS,
};
use serde_json::{json, Value};

const MANIFEST: &str = include_str!("../../../../fixtures/product-model/minimum.rusty.toml");
const COMPOSITION: &[u8] =
    include_bytes!("../../../../fixtures/product-model/minimum.compiled-composition.json");
const INVALID_MANIFEST: &str =
    include_str!("../../../../fixtures/product-model/invalid-path.rusty.toml");
const DUPLICATE_OPAQUE_KEY: &[u8] = include_bytes!(
    "../../../../fixtures/product-model/duplicate-opaque-key.compiled-composition.json"
);
const CANONICAL_NUMBERS: &[u8] = include_bytes!(
    "../../../../fixtures/product-model/canonical-numbers.compiled-composition.json"
);
const CANONICAL_NUMBERS_EXPECTED: &[u8] = include_bytes!(
    "../../../../fixtures/product-model/canonical-numbers.expected.compiled-composition.json"
);

const KERNEL_CAPABILITIES: [ProductKernelCapabilityDescriptor; 3] = [
    ProductKernelCapabilityDescriptor::new(
        "camera-look",
        CapabilityMetadata::new(
            CapabilityKind::System,
            CapabilityUses::INPUT_MAP,
            CapabilityAvailability::Linkable,
            CapabilityAccess::new(&[], &[]),
            CapabilityBudget::new(1_024),
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
            CapabilityBudget::new(1_024),
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
            CapabilityBudget::new(1_024),
            CapabilityProvenance::new(
                "example.product.kernel",
                "kernel/src/timeline.rs",
                "start_timeline",
            ),
        ),
    ),
];

#[test]
fn engine_binding_enum_and_descriptor_export_share_one_generated_closure() {
    let capabilities = EngineCapability::all();
    let descriptors = engine_capability_descriptors();

    assert_eq!(capabilities.len(), descriptors.len());
    for (capability, descriptor) in capabilities
        .iter()
        .copied()
        .zip(descriptors.iter().copied())
    {
        assert_eq!(descriptor.capability(), capability);
        assert_eq!(descriptor.target(), capability.target());
    }
}

#[test]
fn checked_minimum_product_layout_is_valid_and_fixed() {
    let manifest = decode_product_manifest(MANIFEST).unwrap();
    assert_eq!(manifest.product_id(), "example.product");
    assert_eq!(
        manifest.composition_entrypoints()[0].as_str(),
        "rules/main.ts"
    );
    assert_eq!(manifest.kernel_entry().unwrap().as_str(), "kernel/lib.rs");
    assert_eq!(manifest.ui_entry().as_str(), "ui/main.ts");
    assert_eq!(manifest.content_root().as_str(), "content");
    assert_eq!(
        manifest.wrappers()[0].application_id(),
        "com.example.product"
    );
    assert_eq!(manifest.wrappers()[0].version(), "0.1.0");
    assert_eq!(manifest.wrappers()[0].window_width(), 1280);
    assert_eq!(manifest.wrappers()[0].permissions(), ["window", "storage"]);
    assert!(manifest.wrappers()[0].singleton());
}

#[test]
fn packaged_kernel_layout_is_explicit_and_disjoint_from_legacy_source_linking() {
    let package = MANIFEST.replace(
        "entry = \"kernel/lib.rs\"",
        "package = \"kernel/Cargo.toml\"",
    );
    let manifest = decode_product_manifest(&package).expect("packaged kernel manifest");
    assert!(manifest.kernel_entry().is_none());
    assert_eq!(
        manifest.kernel_package().expect("kernel package").as_str(),
        "kernel/Cargo.toml"
    );
    assert!(manifest.has_kernel());

    let conflicting = package.replace(
        "package = \"kernel/Cargo.toml\"",
        "entry = \"kernel/lib.rs\"\npackage = \"kernel/Cargo.toml\"",
    );
    let error = decode_product_manifest(&conflicting).expect_err("ambiguous kernel mode");
    assert_eq!(error.diagnostic().code(), "PRODUCT_KERNEL_MODE_CONFLICT");

    let outside = package.replace("kernel/Cargo.toml", "rules/Cargo.toml");
    let error = decode_product_manifest(&outside).expect_err("package outside kernel lane");
    assert_eq!(error.diagnostic().code(), "PRODUCT_FIXED_LANE");
}

#[test]
fn direct_and_decoded_manifest_validation_converge() {
    let decoded = decode_product_manifest(MANIFEST).unwrap();
    let direct = validate_product_manifest(ProductManifestCandidate {
        product_id: "example.product".into(),
        composition_entrypoints: vec!["rules/main.ts".into()],
        lifecycle: LifecycleMode::Realtime,
        realtime: Some(RealtimeClock::new(60, 4)),
        kernel_entry: Some("kernel/lib.rs".into()),
        kernel_package: None,
        ui_entry: "ui/main.ts".into(),
        ui_projection_stream: None,
        ui_projection_contract: None,
        content_root: "content".into(),
        compiled_composition_output: "generated/compiled-composition.json".into(),
        admitted_runtime_content_output: "generated/runtime-content".into(),
        product_assembly_output: "generated/product-assembly".into(),
        product_bundle_output: "generated/product-bundle".into(),
        wrappers: vec![product_model::WrapperCandidate {
            id: "desktop".into(),
            kind: product_model::WrapperKind::Tauri,
            version: "0.1.0".into(),
            application_id: "com.example.product".into(),
            title: "Example Product".into(),
            window_width: 1280,
            window_height: 720,
            resizable: true,
            permissions: vec!["window".into(), "storage".into()],
            storage_namespace: "example.product".into(),
            release_channel: ReleaseChannel::Development,
            singleton: true,
        }],
    })
    .unwrap();
    assert_eq!(decoded, direct);
}

#[test]
fn manifest_rejects_unknown_fields_and_lifecycle_misuse() {
    let unknown = MANIFEST.replacen(
        "id = \"example.product\"",
        "id = \"example.product\"\nschemaVersion = 1",
        1,
    );
    assert_eq!(
        decode_product_manifest(&unknown)
            .unwrap_err()
            .diagnostic()
            .code(),
        "PRODUCT_MANIFEST_DECODE"
    );

    let incompatible = MANIFEST.replace("mode = \"realtime\"", "mode = \"demand\"");
    assert_eq!(
        decode_product_manifest(&incompatible)
            .unwrap_err()
            .diagnostic()
            .code(),
        "PRODUCT_REALTIME_SETTINGS_INCOMPATIBLE"
    );

    let missing = MANIFEST.replace(
        "[lifecycle.realtime]\nfixed_step_hz = 60\nmax_catch_up_steps = 4\n\n",
        "",
    );
    assert_eq!(
        decode_product_manifest(&missing)
            .unwrap_err()
            .diagnostic()
            .code(),
        "PRODUCT_REALTIME_SETTINGS_REQUIRED"
    );
}

#[test]
fn manifest_ui_projection_contract_is_paired_and_identity_checked() {
    let projected = MANIFEST.replace(
        "[ui]\nentry = \"ui/main.ts\"",
        "[ui]\nentry = \"ui/main.ts\"\nprojection_stream = \"counter\"\nprojection_contract = \"counter.v1\"",
    );
    let manifest = decode_product_manifest(&projected).unwrap();
    assert_eq!(manifest.ui_projection_stream(), Some("counter"));
    assert_eq!(manifest.ui_projection_contract(), Some("counter.v1"));

    let missing_contract = MANIFEST.replace(
        "[ui]\nentry = \"ui/main.ts\"",
        "[ui]\nentry = \"ui/main.ts\"\nprojection_stream = \"counter\"",
    );
    assert_eq!(
        decode_product_manifest(&missing_contract)
            .unwrap_err()
            .diagnostic()
            .code(),
        "PRODUCT_UI_PROJECTION_PAIR"
    );

    let malformed_contract = projected.replace("counter.v1", "Counter contract");
    assert_eq!(
        decode_product_manifest(&malformed_contract)
            .unwrap_err()
            .diagnostic()
            .code(),
        "PRODUCT_INVALID_ID"
    );
}

#[test]
fn manifest_rejects_oversized_input_and_invalid_wrapper_policy() {
    assert_eq!(
        decode_product_manifest(&"x".repeat(MAX_PRODUCT_MANIFEST_BYTES + 1))
            .unwrap_err()
            .diagnostic()
            .code(),
        "PRODUCT_MANIFEST_BYTES_EXCEEDED"
    );

    let duplicate_permission = MANIFEST.replace(
        "permissions = [\"window\", \"storage\"]",
        "permissions = [\"window\", \"window\"]",
    );
    assert_eq!(
        decode_product_manifest(&duplicate_permission)
            .unwrap_err()
            .diagnostic()
            .code(),
        "PRODUCT_DUPLICATE_WRAPPER_PERMISSION"
    );

    let invalid_window = MANIFEST.replace("window_width = 1280", "window_width = 1");
    assert_eq!(
        decode_product_manifest(&invalid_window)
            .unwrap_err()
            .diagnostic()
            .code(),
        "PRODUCT_WRAPPER_WINDOW_BOUNDS"
    );

    for invalid in ["01.0.0", "1.0", "1.0.0-01", "1.0.0+build+extra"] {
        let invalid_version =
            MANIFEST.replace("version = \"0.1.0\"", &format!("version = \"{invalid}\""));
        assert_eq!(
            decode_product_manifest(&invalid_version)
                .unwrap_err()
                .diagnostic()
                .code(),
            "PRODUCT_INVALID_WRAPPER_VERSION"
        );
    }
    let decorated_version =
        MANIFEST.replace("version = \"0.1.0\"", "version = \"1.2.3-rc.1+build.7\"");
    assert_eq!(
        decode_product_manifest(&decorated_version)
            .unwrap()
            .wrappers()[0]
            .version(),
        "1.2.3-rc.1+build.7"
    );

    let missing_singleton = MANIFEST.replace("singleton = true\n", "");
    assert_eq!(
        decode_product_manifest(&missing_singleton)
            .unwrap_err()
            .diagnostic()
            .code(),
        "PRODUCT_MANIFEST_DECODE"
    );
}

#[test]
fn manifest_path_grammar_and_component_overlap_are_strict() {
    for (bad, code) in [
        ("/rules/main.ts", "PRODUCT_PATH_ABSOLUTE"),
        ("rules/../main.ts", "PRODUCT_PATH_TRAVERSAL"),
        ("rules//main.ts", "PRODUCT_PATH_AMBIGUOUS"),
        ("rules\\main.ts", "PRODUCT_PATH_BACKSLASH"),
        ("C:rules/main.ts", "PRODUCT_PATH_COLON"),
        ("rules/main file.ts", "PRODUCT_PATH_WHITESPACE"),
    ] {
        let encoded = bad.replace('\\', "\\\\");
        let input = MANIFEST.replace("rules/main.ts", &encoded);
        assert_eq!(
            decode_product_manifest(&input)
                .unwrap_err()
                .diagnostic()
                .code(),
            code,
            "{bad}"
        );
    }

    let nested_output = MANIFEST.replace(
        "product_assembly = \"generated/product-assembly\"",
        "product_assembly = \"generated/compiled-composition.json/assembly\"",
    );
    assert_eq!(
        decode_product_manifest(&nested_output)
            .unwrap_err()
            .diagnostic()
            .code(),
        "PRODUCT_OUTPUT_OVERLAP"
    );

    let distinct_components = MANIFEST.replace(
        "product_assembly = \"generated/product-assembly\"",
        "product_assembly = \"generated/compiled-composition-extra\"",
    );
    decode_product_manifest(&distinct_components).unwrap();

    let admitted_overlap = MANIFEST.replace(
        "admitted_runtime_content = \"generated/runtime-content\"",
        "admitted_runtime_content = \"generated/product-bundle/runtime-content\"",
    );
    assert_eq!(
        decode_product_manifest(&admitted_overlap)
            .unwrap_err()
            .diagnostic()
            .code(),
        "PRODUCT_OUTPUT_OVERLAP"
    );
}

#[test]
fn checked_invalid_manifest_fixture_fails_closed() {
    assert_eq!(
        decode_product_manifest(INVALID_MANIFEST)
            .unwrap_err()
            .diagnostic()
            .code(),
        "PRODUCT_PATH_TRAVERSAL"
    );
}

#[test]
fn public_product_path_parser_rejects_aliases_before_any_filesystem_access() {
    for value in [
        ".",
        "..",
        "rules/./main.ts",
        "rules/../main.ts",
        "rules//main.ts",
        "rules/\u{0}main.ts",
        "C:rules/main.ts",
        "rules/main file.ts",
    ] {
        assert!(ProductPath::parse(value).is_err(), "{value:?}");
    }
    assert_eq!(
        ProductPath::parse("rules/players/主.ts").unwrap().as_str(),
        "rules/players/主.ts"
    );
}

#[test]
fn identities_reject_separator_aliases_and_punctuation() {
    for id in [
        "-", "..", "a..b", "a--b", "a__b", ".alpha", "alpha-", "Alpha",
    ] {
        let input = MANIFEST.replace("example.product", id);
        assert_eq!(
            decode_product_manifest(&input)
                .unwrap_err()
                .diagnostic()
                .code(),
            "PRODUCT_INVALID_ID",
            "{id}"
        );
    }
}

#[test]
fn composition_direct_and_decoded_paths_converge_on_canonical_bytes() {
    let decoded = decode_compiled_composition(COMPOSITION).unwrap();
    let direct = validate_compiled_composition(decoded.candidate().clone()).unwrap();
    assert_eq!(decoded, direct);
    assert_eq!(encode_compiled_composition(&decoded), COMPOSITION);
    assert_eq!(decoded.candidate().schedule[0].phase, SchedulePhase::Input);
    assert_eq!(
        decoded.candidate().schedule[1].phase,
        SchedulePhase::Simulation
    );
    assert_eq!(
        decoded.candidate().schedule[4].phase,
        SchedulePhase::Projection
    );
}

#[test]
fn opaque_object_key_order_does_not_change_canonical_composition_bytes() {
    let mut left = minimum_candidate();
    left.gameplay_definitions[0].payload =
        serde_json::from_str(r#"{"z":{"b":2,"a":1},"a":[2,1]}"#).unwrap();
    let mut right = minimum_candidate();
    right.gameplay_definitions[0].payload =
        serde_json::from_str(r#"{"a":[2,1],"z":{"a":1,"b":2}}"#).unwrap();
    let left = validate_compiled_composition(left).unwrap();
    let right = validate_compiled_composition(right).unwrap();
    assert_eq!(left.canonical_bytes(), right.canonical_bytes());
}

#[test]
fn canonical_numbers_follow_ecmascript_policy_and_sort_numeric_looking_keys_bytewise() {
    let composition = decode_compiled_composition(CANONICAL_NUMBERS).unwrap();
    assert_eq!(composition.canonical_bytes(), CANONICAL_NUMBERS_EXPECTED);
}

#[test]
fn composition_rejects_unknown_missing_duplicate_and_unknown_references() {
    let unknown = br#"{"product":"example.product","schemaVersion":1,"inputMap":[],"schedule":[],"gameplayDefinitions":[],"timelines":[],"capabilityBindings":[]}"#;
    assert_eq!(
        decode_compiled_composition(unknown)
            .unwrap_err()
            .diagnostic()
            .code(),
        "COMPOSITION_DECODE"
    );

    let missing = br#"{"product":"example.product","inputMap":[],"schedule":[],"gameplayDefinitions":[],"timelines":[]}"#;
    assert_eq!(
        decode_compiled_composition(missing)
            .unwrap_err()
            .diagnostic()
            .code(),
        "COMPOSITION_DECODE"
    );

    assert_eq!(
        decode_compiled_composition(DUPLICATE_OPAQUE_KEY)
            .unwrap_err()
            .diagnostic()
            .code(),
        "COMPOSITION_DUPLICATE_JSON_KEY"
    );

    let root_duplicate = br#"{"product":"example.product","product":"other.product","inputMap":[],"schedule":[],"gameplayDefinitions":[],"timelines":[],"capabilityBindings":[]}"#;
    assert_eq!(
        decode_compiled_composition(root_duplicate)
            .unwrap_err()
            .diagnostic()
            .code(),
        "COMPOSITION_DUPLICATE_JSON_KEY"
    );

    let mut trailing = COMPOSITION.to_vec();
    trailing.extend_from_slice(b"trailing");
    assert_eq!(
        decode_compiled_composition(&trailing)
            .unwrap_err()
            .diagnostic()
            .code(),
        "COMPOSITION_TRAILING_DATA"
    );

    let mut duplicate = minimum_candidate();
    duplicate
        .capability_bindings
        .push(duplicate.capability_bindings[0].clone());
    assert_eq!(
        validate_compiled_composition(duplicate)
            .unwrap_err()
            .diagnostic()
            .code(),
        "COMPOSITION_DUPLICATE_CAPABILITY"
    );

    let mut unknown_ref = minimum_candidate();
    simulation_system_mut(&mut unknown_ref).capability = "missing.capability".into();
    assert_eq!(
        validate_compiled_composition(unknown_ref)
            .unwrap_err()
            .diagnostic()
            .code(),
        "COMPOSITION_UNKNOWN_CAPABILITY"
    );

    let mut unknown_definition = minimum_candidate();
    simulation_system_mut(&mut unknown_definition).definition = Some("missing-definition".into());
    assert_eq!(
        validate_compiled_composition(unknown_definition)
            .unwrap_err()
            .diagnostic()
            .code(),
        "COMPOSITION_UNKNOWN_DEFINITION"
    );
}

#[test]
fn schedule_access_declarations_are_required_bounded_and_nonduplicating() {
    let missing = br#"{"product":"example.product","inputMap":[],"schedule":[{"id":"entry","phase":"simulation","capability":"movement.apply","payload":null}],"gameplayDefinitions":[],"timelines":[],"capabilityBindings":[{"id":"movement.apply","target":"kernel.apply-movement"}]}"#;
    assert_eq!(
        decode_compiled_composition(missing)
            .unwrap_err()
            .diagnostic()
            .code(),
        "COMPOSITION_DECODE"
    );

    let mut duplicate_read = minimum_candidate();
    simulation_system_mut(&mut duplicate_read).reads =
        vec!["state.transform".into(), "state.transform".into()];
    assert_eq!(
        validate_compiled_composition(duplicate_read)
            .unwrap_err()
            .diagnostic()
            .code(),
        "COMPOSITION_DUPLICATE_SCHEDULE_READ"
    );

    let mut duplicate_write = minimum_candidate();
    simulation_system_mut(&mut duplicate_write).writes =
        vec!["state.transform".into(), "state.transform".into()];
    assert_eq!(
        validate_compiled_composition(duplicate_write)
            .unwrap_err()
            .diagnostic()
            .code(),
        "COMPOSITION_DUPLICATE_SCHEDULE_WRITE"
    );

    let mut malformed_write = minimum_candidate();
    simulation_system_mut(&mut malformed_write).writes = vec!["Wrong identity".into()];
    assert_eq!(
        validate_compiled_composition(malformed_write)
            .unwrap_err()
            .diagnostic()
            .code(),
        "PRODUCT_INVALID_ID"
    );

    let mut over_bound = minimum_candidate();
    simulation_system_mut(&mut over_bound).reads = (0..=MAX_SCHEDULE_ACCESS_DECLARATIONS)
        .map(|index| format!("state.value-{index}"))
        .collect();
    assert_eq!(
        validate_compiled_composition(over_bound)
            .unwrap_err()
            .diagnostic()
            .code(),
        "COMPOSITION_SCHEDULE_READ_COUNT"
    );

    let mut writes_over_bound = minimum_candidate();
    simulation_system_mut(&mut writes_over_bound).writes = (0..=MAX_SCHEDULE_ACCESS_DECLARATIONS)
        .map(|index| format!("state.value-{index}"))
        .collect();
    assert_eq!(
        validate_compiled_composition(writes_over_bound)
            .unwrap_err()
            .diagnostic()
            .code(),
        "COMPOSITION_SCHEDULE_WRITE_COUNT"
    );

    let mut read_modify_write = minimum_candidate();
    simulation_system_mut(&mut read_modify_write).reads =
        vec!["state.first".into(), "state.second".into()];
    simulation_system_mut(&mut read_modify_write).writes =
        vec!["state.second".into(), "state.first".into()];
    let checked = validate_compiled_composition(read_modify_write).unwrap();
    assert_eq!(
        simulation_system(&checked.candidate().schedule[1]).reads,
        ["state.first", "state.second"]
    );
    assert_eq!(
        simulation_system(&checked.candidate().schedule[1]).writes,
        ["state.second", "state.first"]
    );
}

#[test]
fn product_composition_admission_links_checked_artifact_to_layout_immutably() {
    let manifest = decode_product_manifest(MANIFEST).unwrap();
    let checked = decode_compiled_composition(COMPOSITION).unwrap();
    let from_checked = admit_checked_product_composition(&manifest, checked.clone()).unwrap();
    let from_direct = admit_product_composition(&manifest, checked.candidate().clone()).unwrap();

    assert_eq!(from_checked, from_direct);
    assert_eq!(from_checked.product_id(), "example.product");
    assert_eq!(from_checked.lifecycle(), LifecycleMode::Realtime);
    assert_eq!(from_checked.realtime(), Some(RealtimeClock::new(60, 4)));
    assert_eq!(from_checked.canonical_bytes(), COMPOSITION);
    assert_eq!(from_checked.input_map()[0].id(), "look");
    assert_eq!(
        from_checked.input_map()[0]
            .intent_descriptor()
            .descriptor_index(),
        0
    );
    assert_eq!(
        from_checked.intent_descriptors()[0].capability().target(),
        "kernel.camera-look"
    );
    assert_eq!(from_checked.schedule()[1].systems()[0].id(), "movement");
    assert_eq!(
        from_checked.schedule()[1].systems()[0]
            .capability()
            .binding_index(),
        1
    );
    assert_eq!(
        from_checked.schedule()[1].systems()[0]
            .capability()
            .target(),
        "kernel.apply-movement"
    );
    assert_eq!(
        from_checked.schedule()[1].systems()[0]
            .definition()
            .unwrap()
            .definition_index(),
        0
    );
    assert_eq!(
        from_checked.schedule()[1].systems()[0].reads(),
        ["input.motion", "state.transform"]
    );
    assert_eq!(from_checked.gameplay_definitions()[0].id(), "player");
    assert_eq!(from_checked.gameplay_definitions()[0].index(), 0);
    assert_eq!(from_checked.timelines()[0].id(), "intro");
    assert_eq!(
        from_checked.timelines()[0].steps()[0]
            .capability()
            .binding_index(),
        3
    );
    assert_eq!(from_checked.capability_bindings()[0].id(), "camera.look");
    assert_eq!(from_checked.capability_bindings()[0].index(), 0);
    assert_eq!(from_checked.composition().canonical_bytes(), COMPOSITION);
}

#[test]
fn input_mappings_are_descriptor_first_and_have_only_typed_trigger_meaning() {
    let mut missing_intent = minimum_candidate();
    missing_intent.input_map[0].intent = "missing.intent".into();
    assert_eq!(
        validate_compiled_composition(missing_intent)
            .unwrap_err()
            .diagnostic()
            .code(),
        "COMPOSITION_UNKNOWN_INTENT_DESCRIPTOR"
    );

    let mut wrong_value_kind = minimum_candidate();
    wrong_value_kind.input_map[0].trigger = InputTrigger::Key {
        code: KeyboardControl::KeyW,
        edge: InputEdge::Pressed,
        chord: vec![],
        context: Some("gameplay".into()),
    };
    assert_eq!(
        validate_compiled_composition(wrong_value_kind)
            .unwrap_err()
            .diagnostic()
            .code(),
        "COMPOSITION_INPUT_TRIGGER_VALUE_KIND"
    );

    let mut duplicate_chord = minimum_candidate();
    duplicate_chord.input_map[0].trigger = InputTrigger::Key {
        code: KeyboardControl::KeyW,
        edge: InputEdge::Held,
        chord: vec![KeyboardControl::ShiftLeft, KeyboardControl::ShiftLeft],
        context: Some("gameplay".into()),
    };
    assert_eq!(
        validate_compiled_composition(duplicate_chord)
            .unwrap_err()
            .diagnostic()
            .code(),
        "COMPOSITION_DUPLICATE_INPUT_CHORD_CONTROL"
    );

    let mut product_payload = minimum_candidate();
    product_payload.intent_descriptors[0].value_kind = IntentValueKind::ProductPayload;
    product_payload.intent_descriptors[0].payload_contract =
        Some("example.camera.payload.v1".into());
    assert_eq!(
        validate_compiled_composition(product_payload)
            .unwrap_err()
            .diagnostic()
            .code(),
        "COMPOSITION_PHYSICAL_INPUT_PRODUCT_PAYLOAD"
    );

    let mut missing_contract = minimum_candidate();
    missing_contract.intent_descriptors[0].value_kind = IntentValueKind::ProductPayload;
    assert_eq!(
        validate_compiled_composition(missing_contract)
            .unwrap_err()
            .diagnostic()
            .code(),
        "COMPOSITION_PRODUCT_PAYLOAD_CONTRACT_REQUIRED"
    );
}

#[test]
fn closed_catalog_links_every_declared_binding_to_static_engine_or_kernel_data() {
    validate_engine_capability_descriptors().unwrap();
    let manifest = decode_product_manifest(MANIFEST).unwrap();
    let admitted = admit_checked_product_composition(
        &manifest,
        decode_compiled_composition(COMPOSITION).unwrap(),
    )
    .unwrap();
    let linked = link_admitted_product_composition(admitted, &KERNEL_CAPABILITIES).unwrap();

    assert_eq!(linked.capability_bindings().len(), 4);
    assert_eq!(linked.admitted().canonical_bytes(), COMPOSITION);
    assert!(matches!(
        linked.capability_bindings()[0].resolved_target(),
        LinkedCapabilityTarget::ProductKernel(index) if index.index() == 1
    ));
    assert!(matches!(
        linked.capability_bindings()[1].resolved_target(),
        LinkedCapabilityTarget::ProductKernel(index) if index.index() == 0
    ));
    assert!(matches!(
        linked.capability_bindings()[2].resolved_target(),
        LinkedCapabilityTarget::Engine(EngineCapability::EntityRenderProject)
    ));
    assert_eq!(
        linked.capability_bindings()[2]
            .metadata()
            .provenance()
            .logical_path(),
        "EntityRenderProjector::project"
    );
    assert!(matches!(
        linked.capability_bindings()[3].resolved_target(),
        LinkedCapabilityTarget::ProductKernel(index) if index.index() == 2
    ));

    let reordered = [
        KERNEL_CAPABILITIES[2],
        KERNEL_CAPABILITIES[0],
        KERNEL_CAPABILITIES[1],
    ];
    let re_admitted = admit_checked_product_composition(
        &manifest,
        decode_compiled_composition(COMPOSITION).unwrap(),
    )
    .unwrap();
    let relinked = link_admitted_product_composition(re_admitted, &reordered).unwrap();
    assert_eq!(
        linked
            .capability_bindings()
            .iter()
            .map(|binding| binding.resolved_target())
            .collect::<Vec<_>>(),
        relinked
            .capability_bindings()
            .iter()
            .map(|binding| binding.resolved_target())
            .collect::<Vec<_>>(),
        "Kernel linkage ordinals are stable authored identities, not declaration positions",
    );
}

#[test]
fn closed_catalog_rejects_unknown_unavailable_incompatible_and_duplicate_bindings_with_paths() {
    let manifest = decode_product_manifest(MANIFEST).unwrap();

    let mut unknown = decode_compiled_composition(COMPOSITION)
        .unwrap()
        .candidate()
        .clone();
    unknown.capability_bindings.push(CapabilityBinding {
        id: "stale".into(),
        target: "kernel.stale-capability".into(),
    });
    let error = link_admitted_product_composition(
        admit_product_composition(&manifest, unknown).unwrap(),
        &KERNEL_CAPABILITIES,
    )
    .unwrap_err();
    assert_eq!(
        error.diagnostic().code(),
        "RUNTIME_CAPABILITY_UNKNOWN_KERNEL_TARGET"
    );
    assert_eq!(error.diagnostic().source(), "compiled-composition.json");
    assert_eq!(error.diagnostic().path(), "capabilityBindings[4].target");

    let unavailable = [
        KERNEL_CAPABILITIES[0],
        KERNEL_CAPABILITIES[1],
        ProductKernelCapabilityDescriptor::new(
            "start-timeline",
            CapabilityMetadata::new(
                CapabilityKind::Operation,
                CapabilityUses::TIMELINE,
                CapabilityAvailability::Unavailable {
                    reason: "the generated Product Assembly did not select the timeline operation",
                },
                CapabilityAccess::new(&[], &[]),
                CapabilityBudget::new(1_024),
                KERNEL_CAPABILITIES[2].metadata().provenance(),
            ),
        ),
    ];
    let error = link_fixture(&manifest, &unavailable).unwrap_err();
    assert_eq!(error.diagnostic().code(), "RUNTIME_CAPABILITY_UNAVAILABLE");
    assert_eq!(error.diagnostic().path(), "capabilityBindings[3].target");

    let incompatible = [
        ProductKernelCapabilityDescriptor::new(
            "camera-look",
            CapabilityMetadata::new(
                CapabilityKind::System,
                CapabilityUses::SCHEDULE,
                CapabilityAvailability::Linkable,
                CapabilityAccess::new(&[], &[]),
                CapabilityBudget::new(1_024),
                KERNEL_CAPABILITIES[0].metadata().provenance(),
            ),
        ),
        KERNEL_CAPABILITIES[1],
        KERNEL_CAPABILITIES[2],
    ];
    let error = link_fixture(&manifest, &incompatible).unwrap_err();
    assert_eq!(
        error.diagnostic().code(),
        "RUNTIME_CAPABILITY_INCOMPATIBLE_USE"
    );
    assert_eq!(error.diagnostic().path(), "intentDescriptors[0].capability");

    let mut duplicate = decode_compiled_composition(COMPOSITION)
        .unwrap()
        .candidate()
        .clone();
    duplicate.capability_bindings.push(CapabilityBinding {
        id: "projection-alias".into(),
        target: "engine.render.entity-project".into(),
    });
    let error = link_admitted_product_composition(
        admit_product_composition(&manifest, duplicate).unwrap(),
        &KERNEL_CAPABILITIES,
    )
    .unwrap_err();
    assert_eq!(
        error.diagnostic().code(),
        "RUNTIME_CAPABILITY_DUPLICATE_TARGET"
    );
    assert_eq!(error.diagnostic().path(), "capabilityBindings[4].target");
}

#[test]
fn closed_catalog_rejects_schedule_contract_and_kernel_descriptor_bounds() {
    let manifest = decode_product_manifest(MANIFEST).unwrap();
    let mut mismatched = decode_compiled_composition(COMPOSITION)
        .unwrap()
        .candidate()
        .clone();
    simulation_system_mut(&mut mismatched).reads = vec!["state.transform".into()];
    let error = link_admitted_product_composition(
        admit_product_composition(&manifest, mismatched).unwrap(),
        &KERNEL_CAPABILITIES,
    )
    .unwrap_err();
    assert_eq!(
        error.diagnostic().code(),
        "RUNTIME_CAPABILITY_ACCESS_MISMATCH"
    );
    assert_eq!(error.diagnostic().path(), "schedule[1].systems[0].reads");

    let oversized = vec![KERNEL_CAPABILITIES[0]; MAX_PRODUCT_KERNEL_CAPABILITIES + 1];
    let error = link_fixture(&manifest, &oversized).unwrap_err();
    assert_eq!(
        error.diagnostic().code(),
        "RUNTIME_CAPABILITY_KERNEL_DESCRIPTOR_COUNT"
    );
    assert_eq!(error.diagnostic().source(), "product-kernel-capabilities");

    let duplicate = [KERNEL_CAPABILITIES[0], KERNEL_CAPABILITIES[0]];
    let error = link_fixture(&manifest, &duplicate).unwrap_err();
    assert_eq!(
        error.diagnostic().code(),
        "RUNTIME_CAPABILITY_DUPLICATE_KERNEL_DESCRIPTOR"
    );

    let over_budget = [
        ProductKernelCapabilityDescriptor::new(
            "camera-look",
            CapabilityMetadata::new(
                CapabilityKind::System,
                CapabilityUses::INPUT_MAP,
                CapabilityAvailability::Linkable,
                CapabilityAccess::new(&[], &[]),
                CapabilityBudget::new(MAX_COMPILED_COMPOSITION_BYTES + 1),
                KERNEL_CAPABILITIES[0].metadata().provenance(),
            ),
        ),
        KERNEL_CAPABILITIES[1],
        KERNEL_CAPABILITIES[2],
    ];
    let error = link_fixture(&manifest, &over_budget).unwrap_err();
    assert_eq!(
        error.diagnostic().code(),
        "RUNTIME_CAPABILITY_PAYLOAD_BUDGET_BOUNDS"
    );
}

fn link_fixture(
    manifest: &product_model::ProductManifest,
    kernel_capabilities: &[ProductKernelCapabilityDescriptor],
) -> Result<product_model::LinkedProductComposition, product_model::ProductModelError> {
    link_admitted_product_composition(
        admit_checked_product_composition(
            manifest,
            decode_compiled_composition(COMPOSITION).unwrap(),
        )
        .unwrap(),
        kernel_capabilities,
    )
}

#[test]
fn product_composition_admission_rejects_layout_product_mismatch_after_checked_validation() {
    let manifest = decode_product_manifest(MANIFEST).unwrap();
    let mut candidate = minimum_candidate();
    candidate.product = "another.product".into();
    let checked = validate_compiled_composition(candidate.clone()).unwrap();

    assert_eq!(
        admit_checked_product_composition(&manifest, checked)
            .unwrap_err()
            .diagnostic()
            .code(),
        "PRODUCT_COMPOSITION_PRODUCT_MISMATCH"
    );
    assert_eq!(
        admit_product_composition(&manifest, candidate)
            .unwrap_err()
            .diagnostic()
            .code(),
        "PRODUCT_COMPOSITION_PRODUCT_MISMATCH"
    );
}

#[test]
fn direct_product_admission_returns_no_readout_for_incomplete_references() {
    let manifest = decode_product_manifest(MANIFEST).unwrap();
    let mut candidate = minimum_candidate();
    simulation_system_mut(&mut candidate).definition = Some("missing-definition".into());

    assert_eq!(
        admit_product_composition(&manifest, candidate)
            .unwrap_err()
            .diagnostic()
            .code(),
        "COMPOSITION_UNKNOWN_DEFINITION"
    );
}

#[test]
fn composition_rejects_wrong_capability_kind_and_unsafe_payloads() {
    let mut target = minimum_candidate();
    target.capability_bindings[0].target = "browser.event".into();
    assert_eq!(
        validate_compiled_composition(target)
            .unwrap_err()
            .diagnostic()
            .code(),
        "COMPOSITION_CAPABILITY_TARGET_NAMESPACE"
    );

    let mut unsafe_integer = minimum_candidate();
    unsafe_integer.gameplay_definitions[0].payload = json!({"count": 9_007_199_254_740_992u64});
    assert_eq!(
        validate_compiled_composition(unsafe_integer)
            .unwrap_err()
            .diagnostic()
            .code(),
        "COMPOSITION_OPAQUE_JSON_NUMBER"
    );

    for value in [9_007_199_254_740_992.0_f64, -9_007_199_254_740_992.0_f64] {
        let mut unsafe_float_integer = minimum_candidate();
        unsafe_float_integer.gameplay_definitions[0].payload = json!({"count": value});
        assert_eq!(
            validate_compiled_composition(unsafe_float_integer)
                .unwrap_err()
                .diagnostic()
                .code(),
            "COMPOSITION_OPAQUE_JSON_NUMBER"
        );
    }

    for raw in [
        br#"{"product":"example.product","intentDescriptors":[],"inputMap":[],"schedule":[],"gameplayDefinitions":[{"id":"definition","payload":9007199254740992.0}],"timelines":[],"capabilityBindings":[]}"# as &[u8],
        br#"{"product":"example.product","intentDescriptors":[],"inputMap":[],"schedule":[],"gameplayDefinitions":[{"id":"definition","payload":-9007199254740992.0}],"timelines":[],"capabilityBindings":[]}"# as &[u8],
    ] {
        assert_eq!(
            decode_compiled_composition(raw)
                .unwrap_err()
                .diagnostic()
                .code(),
            "COMPOSITION_OPAQUE_JSON_NUMBER"
        );
    }

    let mut large_array = minimum_candidate();
    large_array.gameplay_definitions[0].payload = Value::Array(vec![Value::Null; 1_025]);
    assert_eq!(
        validate_compiled_composition(large_array)
            .unwrap_err()
            .diagnostic()
            .code(),
        "COMPOSITION_OPAQUE_JSON_ARRAY_ENTRIES"
    );

    let payload = Value::Array(vec![
        Value::Array(vec![Value::Null; 1_024]),
        Value::Array(vec![Value::Null; 1_024]),
        Value::Array(vec![Value::Null; 1_024]),
    ]);
    let mut aggregate = minimum_candidate();
    aggregate.gameplay_definitions[0].payload = payload.clone();
    aggregate.gameplay_definitions.push(GameplayDefinition {
        id: "second-definition".into(),
        payload,
    });
    assert_eq!(
        validate_compiled_composition(aggregate)
            .unwrap_err()
            .diagnostic()
            .code(),
        "COMPOSITION_OPAQUE_JSON_NODE_COUNT"
    );

    let non_finite = br#"{"product":"example.product","inputMap":[],"schedule":[],"gameplayDefinitions":[{"id":"definition","payload":NaN}],"timelines":[],"capabilityBindings":[]}"#;
    assert_eq!(
        decode_compiled_composition(non_finite)
            .unwrap_err()
            .diagnostic()
            .code(),
        "COMPOSITION_DECODE"
    );
}

#[test]
fn diagnostics_preserve_source_path_code_and_action() {
    let mut candidate = minimum_candidate();
    candidate.intent_descriptors[0].capability = "missing".into();
    let diagnostic = validate_compiled_composition(candidate)
        .unwrap_err()
        .diagnostic()
        .clone();
    assert_eq!(diagnostic.code(), "COMPOSITION_UNKNOWN_CAPABILITY");
    assert_eq!(diagnostic.source(), "compiled-composition.json");
    assert_eq!(diagnostic.path(), "intentDescriptors[0].capability");
    assert!(diagnostic.message().contains("undeclared capability"));
    assert!(serde_json::to_string(&diagnostic)
        .unwrap()
        .contains("source"));
}

fn minimum_candidate() -> CompiledCompositionCandidate {
    CompiledCompositionCandidate {
        product: "example.product".into(),
        intent_descriptors: vec![ProductIntentDescriptor {
            id: "look".into(),
            value_kind: IntentValueKind::Axis,
            payload_contract: None,
            capability: "camera.look".into(),
            payload: json!({"axis": "x"}),
        }],
        input_map: vec![InputMapEntry {
            id: "look".into(),
            intent: "look".into(),
            trigger: InputTrigger::PointerAxis {
                axis: InputAxis::X,
                context: None,
            },
        }],
        schedule: vec![
            SchedulePhaseDeclaration {
                phase: SchedulePhase::Input,
                composition: ScheduleComposition::Append { systems: vec![] },
            },
            SchedulePhaseDeclaration {
                phase: SchedulePhase::Simulation,
                composition: ScheduleComposition::Append {
                    systems: vec![ScheduleSystem {
                        id: "movement".into(),
                        capability: "movement.apply".into(),
                        definition: Some("player".into()),
                        after: vec![],
                        reads: vec!["state.transform".into()],
                        writes: vec!["state.transform".into()],
                        cadence: ScheduleCadence::new(1, 0),
                        payload: Value::Null,
                    }],
                },
            },
            SchedulePhaseDeclaration {
                phase: SchedulePhase::Consequences,
                composition: ScheduleComposition::Append { systems: vec![] },
            },
            SchedulePhaseDeclaration {
                phase: SchedulePhase::Commit,
                composition: ScheduleComposition::Append { systems: vec![] },
            },
            SchedulePhaseDeclaration {
                phase: SchedulePhase::Projection,
                composition: ScheduleComposition::Append { systems: vec![] },
            },
        ],
        gameplay_definitions: vec![GameplayDefinition {
            id: "player".into(),
            payload: json!({"opaque": true}),
        }],
        timelines: vec![Timeline {
            id: "intro".into(),
            steps: vec![TimelineStep {
                id: "start".into(),
                capability: "timeline.start".into(),
                payload: Value::Null,
            }],
        }],
        capability_bindings: vec![
            CapabilityBinding {
                id: "camera.look".into(),
                target: "engine.camera-look".into(),
            },
            CapabilityBinding {
                id: "movement.apply".into(),
                target: "kernel.apply-movement".into(),
            },
            CapabilityBinding {
                id: "timeline.start".into(),
                target: "kernel.start-timeline".into(),
            },
        ],
    }
}

fn simulation_system_mut(candidate: &mut CompiledCompositionCandidate) -> &mut ScheduleSystem {
    match &mut candidate.schedule[1].composition {
        ScheduleComposition::Append { systems } => &mut systems[0],
        _ => panic!("minimum candidate simulation phase must use append"),
    }
}

fn simulation_system(phase: &SchedulePhaseDeclaration) -> &ScheduleSystem {
    match &phase.composition {
        ScheduleComposition::Append { systems } => &systems[0],
        _ => panic!("minimum candidate simulation phase must use append"),
    }
}
