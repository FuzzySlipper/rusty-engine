use serde_json::{json, Value};

use crate::{
    engine_capability_descriptors, MAX_CAPABILITY_BINDINGS, MAX_COMPILED_COMPOSITION_BYTES,
    MAX_GAMEPLAY_DEFINITIONS, MAX_IDENTITY_BYTES, MAX_INPUT_CHORD_CONTROLS, MAX_INPUT_MAP_ENTRIES,
    MAX_INTENT_DESCRIPTORS, MAX_OPAQUE_JSON_ARRAY_ENTRIES, MAX_OPAQUE_JSON_DEPTH,
    MAX_OPAQUE_JSON_NODES, MAX_OPAQUE_JSON_OBJECT_ENTRIES, MAX_OPAQUE_JSON_STRING_BYTES,
    MAX_SAFE_JSON_INTEGER, MAX_SCHEDULE_ACCESS_DECLARATIONS, MAX_SCHEDULE_DEPENDENCIES,
    MAX_SCHEDULE_ENTRIES, MAX_TIMELINES, MAX_TIMELINE_STEPS, SCHEDULE_PHASE_COUNT,
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
            "compiledComposition": ["product", "intentDescriptors", "inputMap", "schedule", "gameplayDefinitions", "timelines", "capabilityBindings"],
            "intentDescriptor": ["id", "valueKind", "capability", "payload"],
            "inputMap": ["id", "intent", "trigger"],
            "schedule": ["phase", "mode", "systems", "before", "after"],
            "scheduleSystem": ["id", "capability", "definition", "after", "reads", "writes", "cadence", "payload"],
            "scheduleCadence": ["everySteps", "offsetSteps"],
            "gameplayDefinition": ["id", "payload"],
            "timeline": ["id", "steps"],
            "timelineStep": ["id", "capability", "payload"],
            "capabilityBinding": ["id", "target"]
        },
        "optionalFields": {
            "scheduleSystem": ["definition"]
        },
        "limits": {
            "maximumEncodedBytes": MAX_COMPILED_COMPOSITION_BYTES,
            "maximumInputMapEntries": MAX_INPUT_MAP_ENTRIES,
            "maximumIntentDescriptors": MAX_INTENT_DESCRIPTORS,
            "maximumInputChordControls": MAX_INPUT_CHORD_CONTROLS,
            "maximumScheduleEntries": MAX_SCHEDULE_ENTRIES,
            "maximumScheduleDependencies": MAX_SCHEDULE_DEPENDENCIES,
            "schedulePhaseCount": SCHEDULE_PHASE_COUNT,
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
        "schedule": {
            "phases": ["input", "simulation", "consequences", "commit", "projection"],
            "modes": ["append", "prepend", "extend", "replace"],
            "placements": ["append", "prepend", "extend-before", "extend-after", "replace"],
            "defaultCadence": {"everySteps": 1, "offsetSteps": 0}
        },
        "capabilityCatalog": capability_catalog_descriptor(),
        "ordering": {
            "intentDescriptors": "authored",
            "inputMap": "authored",
            "schedule": "canonical-phases",
            "scheduleSystems": "authored",
            "scheduleAfter": "authored",
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
        ,"input": {
            "intentValueKinds": ["digital", "axis"],
            "edges": ["held", "pressed", "released"],
            "triggerKinds": ["key", "pointer-button", "pointer-axis", "wheel", "controller-button", "controller-axis"],
            "keyboardControls": ["key-a", "key-b", "key-c", "key-d", "key-e", "key-f", "key-g", "key-h", "key-i", "key-j", "key-k", "key-l", "key-m", "key-n", "key-o", "key-p", "key-q", "key-r", "key-s", "key-t", "key-u", "key-v", "key-w", "key-x", "key-y", "key-z", "digit-0", "digit-1", "digit-2", "digit-3", "digit-4", "digit-5", "digit-6", "digit-7", "digit-8", "digit-9", "space", "enter", "escape", "shift-left", "shift-right", "control-left", "control-right", "alt-left", "alt-right"],
            "pointerButtons": ["primary", "secondary", "middle"],
            "axes": ["x", "y"],
            "controllerButtons": ["button-0", "button-1", "button-2", "button-3", "button-4", "button-5", "button-6", "button-7", "button-8", "button-9", "button-10", "button-11", "button-12", "button-13", "button-14", "button-15"],
            "controllerAxes": ["axis-0", "axis-1", "axis-2", "axis-3"]
        }
    })
}

fn capability_catalog_descriptor() -> Value {
    crate::validate_engine_capability_descriptors()
        .expect("the static Engine capability catalog is valid");
    let mut engine = engine_capability_descriptors()
        .iter()
        .map(|descriptor| {
            let metadata = descriptor.metadata();
            let uses = [
                (crate::CapabilityUse::InputMap, "input-map"),
                (crate::CapabilityUse::Schedule, "schedule"),
                (crate::CapabilityUse::Timeline, "timeline"),
            ]
            .into_iter()
            .filter_map(|(usage, name)| metadata.uses().contains(usage).then_some(name))
            .collect::<Vec<_>>();
            json!({
                "target": descriptor.target(),
                "kind": metadata.kind().as_str(),
                "uses": uses,
                "availability": metadata.availability().as_str(),
                "access": {
                    "reads": metadata.access().reads(),
                    "writes": metadata.access().writes()
                },
                "budget": {
                    "maximumCompactJsonPayloadBytes": metadata.budget().maximum_compact_json_payload_bytes()
                },
                "provenance": {
                    "owner": metadata.provenance().owner(),
                    "source": metadata.provenance().source(),
                    "logicalPath": metadata.provenance().logical_path()
                }
            })
        })
        .collect::<Vec<_>>();
    engine.sort_by(|left, right| left["target"].as_str().cmp(&right["target"].as_str()));
    json!({
        "kinds": ["system", "operation", "query", "projection", "migration"],
        "engine": engine
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
            serde_json::json!(["phase", "mode", "systems", "before", "after"])
        );
        assert_eq!(
            descriptor["capabilityCatalog"]["engine"][0]["target"],
            "engine.render.entity-project"
        );
        assert_eq!(
            descriptor["capabilityCatalog"]["engine"][0]["provenance"]["logicalPath"],
            "EntityRenderProjector::project"
        );
    }
}
