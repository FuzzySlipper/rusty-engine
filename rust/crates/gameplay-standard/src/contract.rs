use serde_json::{json, Value};

use crate::composed::{COMPOSED_EXACT_FAMILY_ID, COMPOSED_EXACT_SEMANTICS_VERSION};
use crate::continuous::{
    CONTINUOUS_EVALUATOR_SEMANTICS_VERSION, MAX_CONTINUOUS_EVALUATION_WORK,
    MAX_CONTINUOUS_EXPRESSION_DEPTH, MAX_CONTINUOUS_EXPRESSION_INPUTS,
    MAX_CONTINUOUS_EXPRESSION_NODES, MAX_CONTINUOUS_MIN_MAX_ARITY,
};
use crate::exact::{
    EXACT_EVALUATOR_SEMANTICS_VERSION, MAX_EXACT_EVALUATION_WORK, MAX_EXACT_EXPRESSION_DEPTH,
    MAX_EXACT_EXPRESSION_INPUTS, MAX_EXACT_EXPRESSION_NODES, MAX_EXACT_MIN_MAX_ARITY,
};
use crate::input::{MAX_CAPABILITY_REQUIREMENTS_PER_ROLE, MAX_ROLE_ID_BYTES};
use gameplay_mechanics::MAX_ABS_MECHANICS_SCALAR;

/// Exports the standard-definition wire contract consumed by rules code generation.
pub fn encode_standard_contract_descriptor() -> String {
    serde_json::to_string_pretty(&standard_contract_descriptor())
        .expect("the static gameplay-standard contract descriptor is valid JSON")
        + "\n"
}

fn standard_contract_descriptor() -> Value {
    json!({
        "contractVersion": 1,
        "families": [
            {
                "id": "exact", "schemaVersion": 1,
                "evaluatorSemanticsVersion": EXACT_EVALUATOR_SEMANTICS_VERSION,
                "operations": ["literal", "input", "add", "subtract", "multiply", "floorDivide", "truncatingDivide", "fixedPower", "min", "max"],
                "inputKinds": [
                    {"tag":"parameter","fields":["kind","role","id"]},
                    {"tag":"fact","fields":["kind","role","id"]},
                    {"tag":"roll","fields":["kind","role","id"]},
                    {"tag":"boundedRoll","fields":["kind","role","id","minimum","maximum"]},
                    {"tag":"choice","fields":["kind","role","id"]},
                    {"tag":"standardStat","fields":["kind","role","stat"]},
                    {"tag":"standardTrackCurrent","fields":["kind","role","track"]},
                    {"tag":"standardTrackMaximum","fields":["kind","role","track"]}
                ],
                "literal": {"encoding":"safe-integer","field":"value","negativeZero":false}
            },
            {
                "id": "continuous", "schemaVersion": 2,
                "evaluatorSemanticsVersion": CONTINUOUS_EVALUATOR_SEMANTICS_VERSION,
                "operations": ["literal", "input", "add", "subtract", "multiply", "divide", "min", "max"],
                "inputKinds": [
                    {"tag":"parameter","fields":["kind","role","id"]},
                    {"tag":"fact","fields":["kind","role","id"]},
                    {"tag":"roll","fields":["kind","role","id"]},
                    {"tag":"choice","fields":["kind","role","id"]}
                ],
                "literal": {"encoding":"binary64-bits","field":"bits","alphabet":"0123456789abcdef","width":16,"lowercase":true,"finite":true,"negativeZero":false,"negativeZeroBits":"8000000000000000","exponentLeadingHexDigits":3,"exponentMask":2047}
            }
        ],
        "fieldOrder": {
            "Definition": ["family", "roles", "semanticsVersion", "source", "subject", "tree"],
            "Role": ["role", "capabilities"],
            "Literal": ["op", "value"],
            "ContinuousLiteral": ["op", "bits"],
            "Input": ["op", "input"],
            "Binary": ["op", "left", "right"],
            "Aggregate": ["op", "values"]
        },
        "identities": {
            "role": {"pattern":"^[a-z][a-z0-9._-]*$","maximumBytes":MAX_ROLE_ID_BYTES},
            "capability": {"pattern":"^[a-z][a-z0-9._-]*$","maximumBytes":MAX_ROLE_ID_BYTES},
            "input": {"pattern":"^[a-z][a-z0-9._-]*$","maximumBytes":MAX_ROLE_ID_BYTES},
            "mechanicsStat": {"pattern":"^[a-z][a-z0-9._-]*$","maximumBytes":MAX_ROLE_ID_BYTES},
            "mechanicsTrack": {"pattern":"^[a-z][a-z0-9._-]*$","maximumBytes":MAX_ROLE_ID_BYTES},
            "subject": {"pattern":"^[ -~]+$","trimmed":true,"maximumBytes":128},
            "source": {"pattern":"^[ -~]+$","trimmed":true,"maximumBytes":128},
            "extensionKind": {"pattern":"^[a-z][a-z0-9._-]*$","maximumBytes":MAX_ROLE_ID_BYTES}
        },
        "limits": {
            "maxRoleIdBytes": MAX_ROLE_ID_BYTES,
            "maxCapabilitiesPerRole": MAX_CAPABILITY_REQUIREMENTS_PER_ROLE,
            "exact": { "maximumDepth": MAX_EXACT_EXPRESSION_DEPTH, "maximumNodes": MAX_EXACT_EXPRESSION_NODES, "maximumInputs": MAX_EXACT_EXPRESSION_INPUTS, "maximumArity": MAX_EXACT_MIN_MAX_ARITY, "maximumWork": MAX_EXACT_EVALUATION_WORK, "minimumScalar": -MAX_ABS_MECHANICS_SCALAR, "maximumScalar": MAX_ABS_MECHANICS_SCALAR },
            "continuous": { "maximumDepth": MAX_CONTINUOUS_EXPRESSION_DEPTH, "maximumNodes": MAX_CONTINUOUS_EXPRESSION_NODES, "maximumInputs": MAX_CONTINUOUS_EXPRESSION_INPUTS, "maximumArity": MAX_CONTINUOUS_MIN_MAX_ARITY, "maximumWork": MAX_CONTINUOUS_EVALUATION_WORK }
        },
        "failures": [
            "unknown-field", "wrong-family", "unsupported-semantics-version", "invalid-identity",
            "non-canonical-roles", "undeclared-input-role", "invalid-literal", "invalid-node",
            "depth-quota-exceeded", "node-quota-exceeded", "input-quota-exceeded", "arity-quota-exceeded",
            "work-quota-exceeded", "source-correlation-mismatch", "extension-schema-mismatch", "extension-payload-too-large"
            ,"missing-product-capability"
        ],
        "extensions": {
            "family":"standardExtension",
            "fieldOrder":["family","kind","namespace","payload","schemaVersion","source","subject"],
            "namespacePattern":"^[a-z][a-z0-9.-]*$",
            "namespaceMaximumBytes":MAX_ROLE_ID_BYTES,
            "schemaVersionMaximum":u32::MAX,
            "maximumBytes":65536,
            "runtime":"downstream-rust-closed-enum"
        },
        "composedExact": {
            "family":COMPOSED_EXACT_FAMILY_ID,
            "schemaVersion":1,
            "semanticsVersion":COMPOSED_EXACT_SEMANTICS_VERSION,
            "definitionFieldOrder":["family","roles","semanticsVersion","source","subject","extension","tree"],
            "extensionFieldOrder":["namespace","schemaVersion"],
            "productFieldOrder":["op","kind","payload","source","subject"],
            "productOp":"product",
            "runtime":"downstream-rust-static-codec"
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exported_descriptor_has_standard_and_composed_exact_contracts() {
        let value: Value = serde_json::from_str(&encode_standard_contract_descriptor()).unwrap();
        assert_eq!(value["contractVersion"], 1);
        assert_eq!(value["families"][0]["schemaVersion"], 1);
        assert_eq!(value["families"][1]["schemaVersion"], 2);
        assert_eq!(value["composedExact"]["family"], COMPOSED_EXACT_FAMILY_ID);
    }
}
