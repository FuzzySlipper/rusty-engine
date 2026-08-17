use serde_json::{json, Value};

use crate::{
    MAX_CANONICAL_RULE_PACKAGE_SET_BYTES, MAX_DEPENDENCIES_PER_RULE_PACKAGE,
    MAX_DEPENDENCIES_PER_RULE_PACKAGE_SET, MAX_DIAGNOSTIC_CODE_BYTES,
    MAX_DIAGNOSTIC_LOGICAL_PATH_BYTES, MAX_DIAGNOSTIC_MESSAGE_BYTES,
    MAX_ENCODED_RULE_PACKAGE_BYTES, MAX_JSON_NESTING_DEPTH, MAX_JSON_NODES_PER_RULE_PACKAGE,
    MAX_JSON_NODES_PER_RULE_PACKAGE_SET, MAX_JSON_STRING_BYTES, MAX_PROVENANCE_PER_RULE_PACKAGE,
    MAX_PROVENANCE_PER_RULE_PACKAGE_SET, MAX_RULE_DIAGNOSTICS, MAX_RULE_ID_BYTES,
    MAX_RULE_PACKAGES_PER_SET, MAX_SAFE_JSON_INTEGER, MAX_SOURCES_PER_RULE_PACKAGE,
    MAX_SOURCES_PER_RULE_PACKAGE_SET, MAX_SOURCE_PATH_BYTES, RULE_PACKAGE_ARTIFACT_KIND,
    RULE_PACKAGE_BINARY64_SCHEMA_VERSION, RULE_PACKAGE_SCHEMA_VERSION,
};

pub fn encode_rule_contract_descriptor() -> String {
    serde_json::to_string_pretty(&rule_contract_descriptor())
        .expect("the static gameplay-rules contract descriptor is valid JSON")
        + "\n"
}

fn rule_contract_descriptor() -> Value {
    json!({
        "contractVersion": 2,
        "artifactKind": RULE_PACKAGE_ARTIFACT_KIND,
        "schemaVersion": RULE_PACKAGE_SCHEMA_VERSION,
        "binary64SchemaVersion": RULE_PACKAGE_BINARY64_SCHEMA_VERSION,
        "brands": [
            "RuleDomainId",
            "RulePackageId",
            "RuleSourceId",
            "RuleSubjectId",
            "RuleFingerprint"
        ],
        "unions": [
            {
                "name": "RulePackageSchemaVersion",
                "values": [
                    RULE_PACKAGE_SCHEMA_VERSION,
                    RULE_PACKAGE_BINARY64_SCHEMA_VERSION
                ]
            },
            {
                "name": "RuleDiagnosticSeverity",
                "values": ["error", "warning"]
            }
        ],
        "records": [
            {
                "name": "RulePackageDependency",
                "fields": [
                    {"name": "domain", "type": "RuleDomainId"},
                    {"name": "package", "type": "RulePackageId"},
                    {"name": "version", "type": "number"},
                    {"name": "fingerprint", "type": "RuleFingerprint", "optional": true}
                ]
            },
            {
                "name": "RuleSource",
                "fields": [
                    {"name": "id", "type": "RuleSourceId"},
                    {"name": "path", "type": "string"}
                ]
            },
            {
                "name": "RuleProvenance",
                "fields": [
                    {"name": "subject", "type": "RuleSubjectId"},
                    {"name": "source", "type": "RuleSourceId"},
                    {"name": "line", "type": "number", "optional": true},
                    {"name": "column", "type": "number", "optional": true}
                ]
            },
            {
                "name": "RulePackage",
                "typeParameter": "Payload extends JsonValue = JsonValue",
                "fields": [
                    {"name": "kind", "type": "typeof RULE_PACKAGE_ARTIFACT_KIND"},
                    {"name": "schemaVersion", "type": "RulePackageSchemaVersion"},
                    {"name": "domain", "type": "RuleDomainId"},
                    {"name": "package", "type": "RulePackageId"},
                    {"name": "version", "type": "number"},
                    {"name": "dependencies", "type": "readonly RulePackageDependency[]"},
                    {"name": "sources", "type": "readonly RuleSource[]"},
                    {"name": "provenance", "type": "readonly RuleProvenance[]"},
                    {"name": "payload", "type": "Payload"}
                ]
            },
            {
                "name": "RuleDiagnosticCorrelation",
                "fields": [
                    {"name": "subject", "type": "RuleSubjectId"},
                    {"name": "source", "type": "RuleSourceId"},
                    {"name": "line", "type": "number", "optional": true},
                    {"name": "column", "type": "number", "optional": true}
                ]
            },
            {
                "name": "RuleDiagnostic",
                "fields": [
                    {"name": "code", "type": "string"},
                    {"name": "severity", "type": "RuleDiagnosticSeverity"},
                    {"name": "logicalPath", "type": "string"},
                    {"name": "message", "type": "string"},
                    {"name": "package", "type": "RulePackageIdentity", "optional": true},
                    {"name": "correlation", "type": "RuleDiagnosticCorrelation", "optional": true}
                ]
            },
            {
                "name": "RulePackageIdentity",
                "fields": [
                    {"name": "domain", "type": "RuleDomainId"},
                    {"name": "package", "type": "RulePackageId"},
                    {"name": "version", "type": "number"}
                ]
            }
        ],
        "limits": {
            "maxEncodedRulePackageBytes": MAX_ENCODED_RULE_PACKAGE_BYTES,
            "maxRulePackagesPerSet": MAX_RULE_PACKAGES_PER_SET,
            "maxCanonicalRulePackageSetBytes": MAX_CANONICAL_RULE_PACKAGE_SET_BYTES,
            "maxDependenciesPerRulePackage": MAX_DEPENDENCIES_PER_RULE_PACKAGE,
            "maxDependenciesPerRulePackageSet": MAX_DEPENDENCIES_PER_RULE_PACKAGE_SET,
            "maxSourcesPerRulePackage": MAX_SOURCES_PER_RULE_PACKAGE,
            "maxSourcesPerRulePackageSet": MAX_SOURCES_PER_RULE_PACKAGE_SET,
            "maxProvenancePerRulePackage": MAX_PROVENANCE_PER_RULE_PACKAGE,
            "maxProvenancePerRulePackageSet": MAX_PROVENANCE_PER_RULE_PACKAGE_SET,
            "maxRuleDiagnostics": MAX_RULE_DIAGNOSTICS,
            "maxRuleIdBytes": MAX_RULE_ID_BYTES,
            "maxDiagnosticCodeBytes": MAX_DIAGNOSTIC_CODE_BYTES,
            "maxSourcePathBytes": MAX_SOURCE_PATH_BYTES,
            "maxDiagnosticLogicalPathBytes": MAX_DIAGNOSTIC_LOGICAL_PATH_BYTES,
            "maxDiagnosticMessageBytes": MAX_DIAGNOSTIC_MESSAGE_BYTES,
            "maxJsonNestingDepth": MAX_JSON_NESTING_DEPTH,
            "maxJsonNodesPerRulePackage": MAX_JSON_NODES_PER_RULE_PACKAGE,
            "maxJsonNodesPerRulePackageSet": MAX_JSON_NODES_PER_RULE_PACKAGE_SET,
            "maxJsonStringBytes": MAX_JSON_STRING_BYTES,
            "maxSafeJsonInteger": MAX_SAFE_JSON_INTEGER
        },
        "fieldOrder": {
            "RulePackage": [
                "kind",
                "schemaVersion",
                "domain",
                "package",
                "version",
                "dependencies",
                "sources",
                "provenance",
                "payload"
            ],
            "RulePackageDependency": ["domain", "package", "version", "fingerprint"],
            "RuleSource": ["id", "path"],
            "RuleProvenance": ["subject", "source", "line", "column"]
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exported_descriptor_is_versioned_and_derived_from_runtime_limits() {
        let descriptor: Value = serde_json::from_str(&encode_rule_contract_descriptor()).unwrap();
        assert_eq!(descriptor["contractVersion"], 2);
        assert_eq!(descriptor["artifactKind"], RULE_PACKAGE_ARTIFACT_KIND);
        assert_eq!(
            descriptor["limits"]["maxEncodedRulePackageBytes"],
            MAX_ENCODED_RULE_PACKAGE_BYTES
        );
        assert_eq!(descriptor["fieldOrder"]["RulePackage"][8], "payload");
    }
}
