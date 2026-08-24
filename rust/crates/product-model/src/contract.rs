use serde_json::{json, Value};

use crate::{
    MAX_CAPABILITY_BINDINGS, MAX_COMPILED_COMPOSITION_BYTES, MAX_GAMEPLAY_DEFINITIONS,
    MAX_IDENTITY_BYTES, MAX_INPUT_MAP_ENTRIES, MAX_OPAQUE_JSON_ARRAY_ENTRIES,
    MAX_OPAQUE_JSON_DEPTH, MAX_OPAQUE_JSON_NODES, MAX_OPAQUE_JSON_OBJECT_ENTRIES,
    MAX_OPAQUE_JSON_STRING_BYTES, MAX_SAFE_JSON_INTEGER, MAX_SCHEDULE_ACCESS_DECLARATIONS,
    MAX_SCHEDULE_ENTRIES, MAX_TIMELINES, MAX_TIMELINE_STEPS,
};

/// Encodes the current Rust-owned Compiled Composition descriptor consumed by
/// TypeScript generation and drift checks. It intentionally has no version or
/// compatibility family: it describes only the current strict schema.
pub fn encode_product_model_contract_descriptor() -> String {
    serde_json::to_string_pretty(&product_model_contract_descriptor())
        .expect("the static product-model descriptor is valid JSON")
        + "\n"
}

fn product_model_contract_descriptor() -> Value {
    json!({
        "artifact": "compiled-composition",
        "identity": {
            "maximumBytes": MAX_IDENTITY_BYTES,
            "alphabet": "lowercase-ascii-alphanumeric-dot-underscore-hyphen",
            "startsAndEndsAlphanumeric": true,
            "forbidAdjacentSeparators": true
        },
        "fields": {
            "compiledComposition": ["product", "inputMap", "schedule", "gameplayDefinitions", "timelines", "capabilityBindings"],
            "inputMap": ["id", "intent", "capability", "payload"],
            "schedule": ["id", "phase", "capability", "definition", "reads", "writes", "payload"],
            "gameplayDefinition": ["id", "payload"],
            "timeline": ["id", "steps"],
            "timelineStep": ["id", "capability", "payload"],
            "capabilityBinding": ["id", "target"]
        },
        "optionalFields": {
            "schedule": ["definition"]
        },
        "limits": {
            "maximumEncodedBytes": MAX_COMPILED_COMPOSITION_BYTES,
            "maximumInputMapEntries": MAX_INPUT_MAP_ENTRIES,
            "maximumScheduleEntries": MAX_SCHEDULE_ENTRIES,
            "maximumScheduleAccessDeclarations": MAX_SCHEDULE_ACCESS_DECLARATIONS,
            "maximumGameplayDefinitions": MAX_GAMEPLAY_DEFINITIONS,
            "maximumTimelines": MAX_TIMELINES,
            "maximumTimelineSteps": MAX_TIMELINE_STEPS,
            "maximumCapabilityBindings": MAX_CAPABILITY_BINDINGS,
            "maximumOpaqueJsonDepth": MAX_OPAQUE_JSON_DEPTH,
            "maximumOpaqueJsonNodes": MAX_OPAQUE_JSON_NODES,
            "maximumOpaqueJsonStringBytes": MAX_OPAQUE_JSON_STRING_BYTES,
            "maximumOpaqueJsonArrayEntries": MAX_OPAQUE_JSON_ARRAY_ENTRIES,
            "maximumOpaqueJsonObjectEntries": MAX_OPAQUE_JSON_OBJECT_ENTRIES,
            "maximumSafeJsonInteger": MAX_SAFE_JSON_INTEGER
        },
        "numberEncoding": {
            "finiteBinary64": "ecmascript-number-to-string",
            "negativeZero": "0",
            "integer": "base10"
        },
        "capabilityTargets": {
            "namespaces": ["engine", "kernel"],
            "separator": "."
        },
        "ordering": {
            "inputMap": "authored",
            "schedule": "authored",
            "gameplayDefinitions": "authored",
            "timelines": "authored",
            "timelineSteps": "authored",
            "capabilityBindings": "authored",
            "scheduleReads": "authored",
            "scheduleWrites": "authored",
            "opaqueObjectKeys": "canonical-bytewise",
            "opaqueArrays": "authored"
        },
        "failures": [
            "unknown-field", "missing-field", "duplicate-json-key", "duplicate-identity",
            "unknown-capability", "unknown-definition", "invalid-capability-target",
            "invalid-identity", "access-declaration-limit", "opaque-json-limit"
        ]
    })
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::encode_product_model_contract_descriptor;
    use crate::{MAX_IDENTITY_BYTES, MAX_SCHEDULE_ACCESS_DECLARATIONS};

    #[test]
    fn exported_descriptor_is_current_and_derived_from_schema_bounds() {
        let descriptor: Value =
            serde_json::from_str(&encode_product_model_contract_descriptor()).unwrap();
        assert_eq!(descriptor["artifact"], "compiled-composition");
        assert!(descriptor.get("contractVersion").is_none());
        assert!(descriptor.get("schemaVersion").is_none());
        assert_eq!(descriptor["identity"]["maximumBytes"], MAX_IDENTITY_BYTES);
        assert_eq!(
            descriptor["numberEncoding"]["finiteBinary64"],
            "ecmascript-number-to-string"
        );
        assert_eq!(
            descriptor["limits"]["maximumScheduleAccessDeclarations"],
            MAX_SCHEDULE_ACCESS_DECLARATIONS
        );
        assert_eq!(
            descriptor["fields"]["schedule"],
            serde_json::json!([
                "id",
                "phase",
                "capability",
                "definition",
                "reads",
                "writes",
                "payload"
            ])
        );
    }
}
