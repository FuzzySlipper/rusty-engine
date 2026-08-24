use product_model::{
    admit_checked_product_composition, admit_product_composition, decode_compiled_composition,
    decode_product_manifest, encode_compiled_composition, validate_compiled_composition,
    validate_product_manifest, CapabilityBinding, CompiledCompositionCandidate, GameplayDefinition,
    InputMapEntry, LifecycleMode, ProductManifestCandidate, ProductPath, RealtimeClock,
    ReleaseChannel, ScheduleEntry, Timeline, TimelineStep, MAX_PRODUCT_MANIFEST_BYTES,
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
    assert_eq!(manifest.wrappers()[0].window_width(), 1280);
    assert_eq!(manifest.wrappers()[0].permissions(), ["window", "storage"]);
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
        ui_entry: "ui/main.ts".into(),
        content_root: "content".into(),
        compiled_composition_output: "generated/compiled-composition.json".into(),
        admitted_runtime_content_output: "generated/runtime-content".into(),
        product_assembly_output: "generated/product-assembly".into(),
        product_bundle_output: "generated/product-bundle".into(),
        wrappers: vec![product_model::WrapperCandidate {
            id: "desktop".into(),
            kind: product_model::WrapperKind::Tauri,
            application_id: "com.example.product".into(),
            title: "Example Product".into(),
            window_width: 1280,
            window_height: 720,
            resizable: true,
            permissions: vec!["window".into(), "storage".into()],
            storage_namespace: "example.product".into(),
            release_channel: ReleaseChannel::Development,
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
    assert_eq!(decoded.candidate().schedule[0].id, "movement");
    assert_eq!(decoded.candidate().schedule[1].id, "render-projection");
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
    assert_eq!(
        composition.canonical_bytes(),
        b"{\"product\":\"example.product\",\"inputMap\":[],\"schedule\":[],\"gameplayDefinitions\":[{\"id\":\"numeric\",\"payload\":{\"1\":\"one\",\"10\":\"ten\",\"2\":\"two\",\"negativeZero\":0,\"small\":0.000001,\"tiny\":0.0000012}}],\"timelines\":[],\"capabilityBindings\":[]}\n"
    );
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
    unknown_ref.schedule[0].capability = "missing.capability".into();
    assert_eq!(
        validate_compiled_composition(unknown_ref)
            .unwrap_err()
            .diagnostic()
            .code(),
        "COMPOSITION_UNKNOWN_CAPABILITY"
    );

    let mut unknown_definition = minimum_candidate();
    unknown_definition.schedule[0].definition = Some("missing-definition".into());
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
    duplicate_read.schedule[0].reads = vec!["state.transform".into(), "state.transform".into()];
    assert_eq!(
        validate_compiled_composition(duplicate_read)
            .unwrap_err()
            .diagnostic()
            .code(),
        "COMPOSITION_DUPLICATE_SCHEDULE_READ"
    );

    let mut duplicate_write = minimum_candidate();
    duplicate_write.schedule[0].writes = vec!["state.transform".into(), "state.transform".into()];
    assert_eq!(
        validate_compiled_composition(duplicate_write)
            .unwrap_err()
            .diagnostic()
            .code(),
        "COMPOSITION_DUPLICATE_SCHEDULE_WRITE"
    );

    let mut malformed_write = minimum_candidate();
    malformed_write.schedule[0].writes = vec!["Wrong identity".into()];
    assert_eq!(
        validate_compiled_composition(malformed_write)
            .unwrap_err()
            .diagnostic()
            .code(),
        "PRODUCT_INVALID_ID"
    );

    let mut over_bound = minimum_candidate();
    over_bound.schedule[0].reads = (0..=MAX_SCHEDULE_ACCESS_DECLARATIONS)
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
    writes_over_bound.schedule[0].writes = (0..=MAX_SCHEDULE_ACCESS_DECLARATIONS)
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
    read_modify_write.schedule[0].reads = vec!["state.first".into(), "state.second".into()];
    read_modify_write.schedule[0].writes = vec!["state.second".into(), "state.first".into()];
    let checked = validate_compiled_composition(read_modify_write).unwrap();
    assert_eq!(
        checked.candidate().schedule[0].reads,
        ["state.first", "state.second"]
    );
    assert_eq!(
        checked.candidate().schedule[0].writes,
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
    assert_eq!(from_checked.input_map()[0].capability().binding_index(), 0);
    assert_eq!(
        from_checked.input_map()[0].capability().target(),
        "engine.camera-look"
    );
    assert_eq!(from_checked.schedule()[0].id(), "movement");
    assert_eq!(from_checked.schedule()[0].capability().binding_index(), 1);
    assert_eq!(
        from_checked.schedule()[0].capability().target(),
        "kernel.apply-movement"
    );
    assert_eq!(
        from_checked.schedule()[0]
            .definition()
            .unwrap()
            .definition_index(),
        0
    );
    assert_eq!(
        from_checked.schedule()[0].reads(),
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
    candidate.schedule[0].definition = Some("missing-definition".into());

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
        br#"{"product":"example.product","inputMap":[],"schedule":[],"gameplayDefinitions":[{"id":"definition","payload":9007199254740992.0}],"timelines":[],"capabilityBindings":[]}"# as &[u8],
        br#"{"product":"example.product","inputMap":[],"schedule":[],"gameplayDefinitions":[{"id":"definition","payload":-9007199254740992.0}],"timelines":[],"capabilityBindings":[]}"# as &[u8],
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
    candidate.input_map[0].capability = "missing".into();
    let diagnostic = validate_compiled_composition(candidate)
        .unwrap_err()
        .diagnostic()
        .clone();
    assert_eq!(diagnostic.code(), "COMPOSITION_UNKNOWN_CAPABILITY");
    assert_eq!(diagnostic.source(), "compiled-composition.json");
    assert_eq!(diagnostic.path(), "inputMap[0].capability");
    assert!(diagnostic.message().contains("undeclared capability"));
    assert!(serde_json::to_string(&diagnostic)
        .unwrap()
        .contains("source"));
}

fn minimum_candidate() -> CompiledCompositionCandidate {
    CompiledCompositionCandidate {
        product: "example.product".into(),
        input_map: vec![InputMapEntry {
            id: "look".into(),
            intent: "look".into(),
            capability: "camera.look".into(),
            payload: json!({"axis": "x"}),
        }],
        schedule: vec![ScheduleEntry {
            id: "movement".into(),
            phase: "simulation".into(),
            capability: "movement.apply".into(),
            definition: Some("player".into()),
            reads: vec!["state.transform".into()],
            writes: vec!["state.transform".into()],
            payload: Value::Null,
        }],
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
