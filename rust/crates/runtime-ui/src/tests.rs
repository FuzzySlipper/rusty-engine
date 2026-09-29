use super::*;
use runtime_lifecycle::{RuntimeControlRevision, RuntimeGeneration, RuntimeInstanceId};

fn binding(instance: u64) -> RuntimeUiRuntimeBinding {
    RuntimeUiRuntimeBinding::new(
        RuntimeInstanceId::new(instance),
        RuntimeGeneration::new(1),
        RuntimeControlRevision::new(1),
    )
}

fn envelope(
    stream: &str,
    value: serde_json::Value,
) -> Result<RuntimeUiProjectionEnvelope, RuntimeUiProjectionError> {
    RuntimeUiProjectionEnvelope::new(binding(33), 0, stream, "stealth.ui.snapshot.v1", value)
}

#[test]
fn envelope_encodes_the_fixture_wire_shape() {
    let envelope = envelope(
        "stealth.hud",
        serde_json::json!({
            "selected": "target-1",
            "alerts": 2,
        }),
    )
    .expect("envelope");
    let encoded = String::from_utf8(envelope.encode_json().expect("wire")).expect("utf-8");
    assert_eq!(
        encoded,
        include_str!("../../../../fixtures/runtime-ui/stealth.ui-projection.json").trim()
    );
}

#[test]
fn identities_are_checked() {
    assert!(matches!(
        envelope("not valid", serde_json::json!({})),
        Err(RuntimeUiProjectionError::InvalidIdentity {
            field: "stream",
            ..
        })
    ));
    assert!(envelope("Stealth.Hud", serde_json::json!({})).is_err());
    assert!(envelope("stealth.hud", serde_json::json!({})).is_ok());
}

#[test]
fn large_and_deep_values_are_admitted_without_recursive_validation() {
    let deeply_nested = (0..1_024).fold(serde_json::json!(null), |value, _| {
        serde_json::json!([value])
    });
    let mut object = serde_json::Map::new();
    for index in 0..300 {
        object.insert(index.to_string(), serde_json::json!(index));
    }
    let result = envelope(
        "stealth.large",
        serde_json::json!({
            "deeplyNested": deeply_nested,
            "longText": "x".repeat(9_000),
            "array": (0..600).collect::<Vec<_>>(),
            "object": object,
        }),
    );
    assert!(result.is_ok());
}

#[test]
fn portable_numbers_admit_fractions_and_reject_unsafe_integers() {
    for value in [
        serde_json::json!(9_007_199_254_740_992_u64),
        serde_json::json!(-9_007_199_254_740_992_i64),
        serde_json::Value::Number(
            serde_json::Number::from_f64(9_007_199_254_740_992.0).expect("finite number"),
        ),
    ] {
        assert!(matches!(
            envelope("stealth.hud", value),
            Err(RuntimeUiProjectionError::ValueUnsafeInteger { .. })
        ));
    }
    envelope(
        "stealth.hud",
        serde_json::json!({
            "minimum": -9_007_199_254_740_991_i64,
            "fraction": 1.25,
            "maximum": 9_007_199_254_740_991_u64
        }),
    )
    .expect("portable numbers");
}

#[test]
fn deserialize_rejects_unknown_fields_and_noncanonical_integers() {
    let decode = |text: &str| serde_json::from_str::<RuntimeUiProjectionEnvelope>(text);
    let valid = r#"{"artifact":"rusty.product.ui-projection","runtime":{"instanceId":"33","generation":"1","controlRevision":"1"},"sequence":"0","stream":"stealth.hud","contract":"stealth.ui.snapshot.v1","value":{}}"#;
    assert!(decode(valid).is_ok());
    assert!(decode(&valid.replace(r#""value":{}"#, r#""value":{},"extra":true"#)).is_err());
    assert!(decode(&valid.replace(r#""instanceId":"33""#, r#""instanceId":"033""#)).is_err());
}
