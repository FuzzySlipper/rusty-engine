use super::*;
use runtime_lifecycle::{
    RuntimeControlRevision, RuntimeGeneration, RuntimeInstanceId, RuntimeLifecycle,
    RuntimeLifecycleConfig, RuntimePhase, RuntimePhaseToken,
};

fn fresh_lifecycle() -> RuntimeLifecycle {
    let mut lifecycle =
        RuntimeLifecycle::new(RuntimeInstanceId::new(33), RuntimeLifecycleConfig::Demand);
    lifecycle.start().expect("start");
    lifecycle
}

fn projection_token(lifecycle: &mut RuntimeLifecycle) -> RuntimePhaseToken {
    lifecycle
        .admit_demand_step()
        .expect("admit")
        .step_at(0)
        .expect("step")
        .phases()
        .projection()
}

#[test]
fn direct_value_projection_emits_owned_deterministic_envelope() {
    let mut lifecycle = fresh_lifecycle();
    let token = projection_token(&mut lifecycle);
    let mut lane = RuntimeUiProjection::bind(&lifecycle).expect("bind");
    let envelope = lane
        .emit_value(
            &lifecycle,
            token,
            "stealth.hud",
            "stealth.ui.snapshot.v1",
            serde_json::json!({
                "selected": "target-1",
                "alerts": 2,
            }),
        )
        .expect("projection");
    let encoded = envelope.encode_json_string().expect("wire");
    assert_eq!(
        encoded,
        include_str!("../../../../fixtures/runtime-ui/stealth.ui-projection.json").trim()
    );
    assert_eq!(lane.readout().stream_count(), 1);
}

#[test]
fn direct_value_projection_uses_lifecycle_and_stream_guards() {
    let mut lifecycle = fresh_lifecycle();
    let token = projection_token(&mut lifecycle);
    let mut lane = RuntimeUiProjection::bind(&lifecycle).expect("bind");
    let envelope = lane
        .emit_value(
            &lifecycle,
            token,
            "stealth.hud",
            "stealth.ui.snapshot.v1",
            serde_json::json!({"selected": "target-1"}),
        )
        .expect("direct value projection");
    assert_eq!(envelope.sequence(), 0);
    assert_eq!(envelope.value()["selected"], "target-1");
    assert!(matches!(
        lane.emit_value(
            &lifecycle,
            token,
            "stealth.hud",
            "stealth.ui.snapshot.v1",
            serde_json::json!({}),
        ),
        Err(RuntimeUiProjectionError::DuplicateSequence { .. })
    ));
}

#[test]
fn strict_decode_rejects_unknown_trailing_and_noncanonical_values() {
    let valid = br#"{"artifact":"rusty.product.ui-projection","runtime":{"instanceId":"33","generation":"1","controlRevision":"1"},"sequence":"0","stream":"stealth.hud","contract":"stealth.ui.snapshot.v1","value":{}}"#;
    assert!(RuntimeUiProjectionEnvelope::decode_json(valid).is_ok());
    let unknown = br#"{"artifact":"rusty.product.ui-projection","runtime":{"instanceId":"33","generation":"1","controlRevision":"1"},"sequence":"0","stream":"stealth.hud","contract":"stealth.ui.snapshot.v1","value":{},"extra":true}"#;
    assert_eq!(
        RuntimeUiProjectionEnvelope::decode_json(unknown),
        Err(RuntimeUiProjectionError::WireMalformed)
    );
    let trailing = [valid.as_slice(), br#"{}"#.as_slice()].concat();
    assert_eq!(
        RuntimeUiProjectionEnvelope::decode_json(&trailing),
        Err(RuntimeUiProjectionError::WireMalformed)
    );
    let noncanonical = br#"{"artifact":"rusty.product.ui-projection","runtime":{"instanceId":"033","generation":"1","controlRevision":"1"},"sequence":"0","stream":"stealth.hud","contract":"stealth.ui.snapshot.v1","value":{}}"#;
    assert_eq!(
        RuntimeUiProjectionEnvelope::decode_json(noncanonical),
        Err(RuntimeUiProjectionError::WireNonCanonicalInteger {
            field: "runtime.instanceId"
        })
    );
}

#[test]
fn wrong_phase_foreign_stale_duplicate_regression_rebind_and_dispose_fail_closed() {
    let mut lifecycle = fresh_lifecycle();
    let admission = lifecycle.admit_demand_step().expect("admit");
    let phases = admission.step_at(0).expect("step").phases();
    let mut lane = RuntimeUiProjection::bind(&lifecycle).expect("bind");
    assert!(matches!(
        lane.emit_value(
            &lifecycle,
            phases.mutation(),
            "stealth.hud",
            "stealth.ui.snapshot.v1",
            serde_json::json!({}),
        ),
        Err(RuntimeUiProjectionError::WrongPhase {
            expected: RuntimePhase::Projection,
            received: RuntimePhase::Mutation
        })
    ));
    lane.emit_value(
        &lifecycle,
        phases.projection(),
        "stealth.hud",
        "stealth.ui.snapshot.v1",
        serde_json::json!({"value": 1}),
    )
    .expect("first");
    lane.rebind(&lifecycle).expect("same binding is a no-op");
    assert!(matches!(
        lane.emit_value(
            &lifecycle,
            phases.projection(),
            "stealth.hud",
            "stealth.ui.snapshot.v1",
            serde_json::json!({"value": 2}),
        ),
        Err(RuntimeUiProjectionError::DuplicateSequence { .. })
    ));

    let mut foreign =
        RuntimeLifecycle::new(RuntimeInstanceId::new(34), RuntimeLifecycleConfig::Demand);
    foreign.start().expect("foreign start");
    assert!(matches!(
        lane.rebind(&foreign),
        Err(RuntimeUiProjectionError::RebindForeignInstance { .. })
    ));
    lifecycle.pause().expect("pause");
    assert!(matches!(
        lane.rebind(&lifecycle),
        Err(RuntimeUiProjectionError::RebindNotRunning { .. })
    ));
    lifecycle.resume().expect("resume");
    lane.rebind(&lifecycle).expect("new epoch");
    let older = fresh_lifecycle();
    assert!(matches!(
        lane.rebind(&older),
        Err(RuntimeUiProjectionError::RebindRegression { .. })
    ));
    let new_token = projection_token(&mut lifecycle);
    lane.emit_value(
        &lifecycle,
        new_token,
        "stealth.hud",
        "stealth.ui.snapshot.v1",
        serde_json::json!({"value": 3}),
    )
    .expect("rebound sequence resets");
    lane.dispose();
    assert!(matches!(
        lane.emit_value(
            &lifecycle,
            new_token,
            "stealth.hud",
            "stealth.ui.snapshot.v1",
            serde_json::json!({}),
        ),
        Err(RuntimeUiProjectionError::Disposed)
    ));
}

#[test]
fn stream_contract_and_value_bounds_are_checked_before_emission() {
    let mut lifecycle = fresh_lifecycle();
    let token = projection_token(&mut lifecycle);
    let mut lane = RuntimeUiProjection::bind(&lifecycle).expect("bind");
    let invalid = lane.emit_value(
        &lifecycle,
        token,
        "not valid",
        "stealth.ui.snapshot.v1",
        serde_json::json!({}),
    );
    assert!(matches!(
        invalid,
        Err(RuntimeUiProjectionError::InvalidIdentity {
            field: "stream",
            ..
        })
    ));
    let huge = "x".repeat(MAX_RUNTIME_UI_PROJECTION_VALUE_JSON_BYTES);
    let result = lane.emit_value(
        &lifecycle,
        token,
        "stealth.hud",
        "stealth.ui.snapshot.v1",
        serde_json::json!({"huge": huge}),
    );
    assert!(matches!(
        result,
        Err(RuntimeUiProjectionError::ValueTooLarge { .. })
    ));
    assert_eq!(lane.readout().stream_count(), 0);

    let first = lane
        .emit_value(
            &lifecycle,
            token,
            "stealth.hud",
            "stealth.ui.snapshot.v1",
            serde_json::json!({"ok": true}),
        )
        .expect("first contract");
    assert_eq!(first.contract(), "stealth.ui.snapshot.v1");
    assert!(matches!(
        lane.emit_value(
            &lifecycle,
            token,
            "stealth.hud",
            "stealth.ui.other.v1",
            serde_json::json!({}),
        ),
        Err(RuntimeUiProjectionError::ContractChanged { .. })
    ));
}

#[test]
fn shape_bounds_match_application_host_limits() {
    let mut lifecycle = fresh_lifecycle();
    let token = projection_token(&mut lifecycle);
    let mut lane = RuntimeUiProjection::bind(&lifecycle).expect("bind");
    let too_deep = (0..(MAX_RUNTIME_UI_PROJECTION_VALUE_DEPTH + 1))
        .fold(serde_json::json!(null), |value, _| {
            serde_json::json!([value])
        });
    let result = lane.emit_value(
        &lifecycle,
        token,
        "stealth.deep",
        "stealth.ui.snapshot.v1",
        too_deep,
    );
    assert!(matches!(
        result,
        Err(RuntimeUiProjectionError::ValueDepthLimit { .. })
    ));

    let mut too_many_nodes_object = serde_json::Map::new();
    for index in 0..MAX_RUNTIME_UI_PROJECTION_VALUE_OBJECT_KEYS {
        too_many_nodes_object.insert(
            index.to_string(),
            serde_json::Value::Array((0..8).map(|_| serde_json::json!(null)).collect()),
        );
    }
    let result = lane.emit_value(
        &lifecycle,
        token,
        "stealth.nodes",
        "stealth.ui.snapshot.v1",
        serde_json::Value::Object(too_many_nodes_object),
    );
    assert!(matches!(
        result,
        Err(RuntimeUiProjectionError::ValueNodeLimit { .. })
    ));

    let result = lane.emit_value(
        &lifecycle,
        token,
        "stealth.string",
        "stealth.ui.snapshot.v1",
        serde_json::json!({
            "value": "x".repeat(MAX_RUNTIME_UI_PROJECTION_VALUE_STRING_BYTES + 1)
        }),
    );
    assert!(matches!(
        result,
        Err(RuntimeUiProjectionError::ValueStringLimit { .. })
    ));

    let too_many_array_entries = serde_json::Value::Array(
        (0..(MAX_RUNTIME_UI_PROJECTION_VALUE_ARRAY_LENGTH + 1))
            .map(|_| serde_json::json!(null))
            .collect(),
    );
    let result = lane.emit_value(
        &lifecycle,
        token,
        "stealth.array",
        "stealth.ui.snapshot.v1",
        too_many_array_entries,
    );
    assert!(matches!(
        result,
        Err(RuntimeUiProjectionError::ValueArrayLimit { .. })
    ));

    let mut too_many_object_keys = serde_json::Map::new();
    for index in 0..(MAX_RUNTIME_UI_PROJECTION_VALUE_OBJECT_KEYS + 1) {
        too_many_object_keys.insert(index.to_string(), serde_json::json!(null));
    }
    let result = lane.emit_value(
        &lifecycle,
        token,
        "stealth.object",
        "stealth.ui.snapshot.v1",
        serde_json::Value::Object(too_many_object_keys),
    );
    assert!(matches!(
        result,
        Err(RuntimeUiProjectionError::ValueObjectLimit { .. })
    ));
}

#[test]
fn portable_numbers_admit_fractions_and_reject_unsafe_integers() {
    let runtime = RuntimeUiRuntimeBinding::new(
        RuntimeInstanceId::new(1),
        RuntimeGeneration::new(1),
        RuntimeControlRevision::new(1),
    );
    for value in [
        serde_json::json!(9_007_199_254_740_992_u64),
        serde_json::json!(-9_007_199_254_740_992_i64),
        serde_json::Value::Number(
            serde_json::Number::from_f64(9_007_199_254_740_992.0).expect("finite number"),
        ),
    ] {
        assert!(matches!(
            RuntimeUiProjectionEnvelope::new(
                runtime,
                0,
                "stealth.hud",
                "stealth.ui.snapshot.v1",
                value,
            ),
            Err(RuntimeUiProjectionError::ValueUnsafeInteger { .. })
        ));
    }
    RuntimeUiProjectionEnvelope::new(
        runtime,
        0,
        "stealth.hud",
        "stealth.ui.snapshot.v1",
        serde_json::json!({
            "minimum": -9_007_199_254_740_991_i64,
            "fraction": 1.25,
            "maximum": 9_007_199_254_740_991_u64
        }),
    )
    .expect("portable numbers");
}

#[test]
fn copied_value_does_not_alias_source_and_multiple_streams_share_step() {
    let mut lifecycle = fresh_lifecycle();
    let token = projection_token(&mut lifecycle);
    let mut source = serde_json::json!({
        "selected": "before",
        "alerts": 1,
    });
    let mut lane = RuntimeUiProjection::bind(&lifecycle).expect("bind");
    let first = lane
        .emit_value(
            &lifecycle,
            token,
            "stealth.hud",
            "stealth.ui.snapshot.v1",
            source.clone(),
        )
        .expect("first");
    source["selected"] = serde_json::json!("after");
    let second = lane
        .emit_value(
            &lifecycle,
            token,
            "stealth.overlay",
            "stealth.ui.snapshot.v1",
            source,
        )
        .expect("second stream");
    assert_eq!(first.value()["selected"], "before");
    assert_eq!(second.value()["selected"], "after");
    assert_eq!(lane.readout().stream_count(), 2);
}

#[test]
fn identity_bound_matches_retained_runtime_grammar() {
    assert!(RuntimeUiProjectionEnvelope::new(
        RuntimeUiRuntimeBinding::new(
            RuntimeInstanceId::new(1),
            RuntimeGeneration::new(1),
            RuntimeControlRevision::new(1),
        ),
        0,
        "stealth.hud",
        "stealth.ui.snapshot.v1",
        serde_json::json!({}),
    )
    .is_ok());
    assert!(RuntimeUiProjectionEnvelope::new(
        RuntimeUiRuntimeBinding::new(
            RuntimeInstanceId::new(1),
            RuntimeGeneration::new(1),
            RuntimeControlRevision::new(1),
        ),
        0,
        "Stealth.Hud",
        "stealth.ui.snapshot.v1",
        serde_json::json!({}),
    )
    .is_err());
}
