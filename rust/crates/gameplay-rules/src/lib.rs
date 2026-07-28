//! Host-neutral support for downstream-owned rules packages.
//!
//! This crate owns a strict bounded envelope, canonical JSON, exact package
//! dependencies, source correlation, diagnostics, and deterministic package-set
//! resolution. Opaque payload meaning, semantic compilation, runtime state, and
//! gameplay execution remain downstream.

#![forbid(unsafe_code)]

mod contract;
mod diagnostic;
mod error;
mod identity;
mod json;
mod package;
mod resolve;

pub use contract::encode_rule_contract_descriptor;
pub use diagnostic::{
    RuleDiagnostic, RuleDiagnosticCorrelation, RuleDiagnosticReport, RuleDiagnosticSeverity,
    MAX_DIAGNOSTIC_CODE_BYTES, MAX_DIAGNOSTIC_LOGICAL_PATH_BYTES, MAX_DIAGNOSTIC_MESSAGE_BYTES,
    MAX_RULE_DIAGNOSTICS,
};
pub use error::{RuleDiagnosticError, RulePackageError, RulePackageSetError};
pub use identity::{
    RuleDomainId, RuleFingerprint, RulePackageDependency, RulePackageId, RulePackageIdentity,
    RuleSourceId, RuleSubjectId, RuleVersion, MAX_RULE_ID_BYTES, MAX_SAFE_JSON_INTEGER,
};
pub use package::{
    admit_rule_package, decode_canonical_rule_package, decode_rule_package, encode_rule_package,
    AdmittedRulePackage, RulePackageCandidate, RuleProvenance, RuleSource,
    MAX_DEPENDENCIES_PER_RULE_PACKAGE, MAX_ENCODED_RULE_PACKAGE_BYTES, MAX_JSON_NESTING_DEPTH,
    MAX_JSON_NODES_PER_RULE_PACKAGE, MAX_JSON_STRING_BYTES, MAX_PROVENANCE_PER_RULE_PACKAGE,
    MAX_SOURCES_PER_RULE_PACKAGE, MAX_SOURCE_PATH_BYTES, RULE_PACKAGE_ARTIFACT_KIND,
    RULE_PACKAGE_SCHEMA_VERSION,
};
pub use resolve::{
    resolve_rule_packages, ResolvedRulePackages, MAX_CANONICAL_RULE_PACKAGE_SET_BYTES,
    MAX_DEPENDENCIES_PER_RULE_PACKAGE_SET, MAX_JSON_NODES_PER_RULE_PACKAGE_SET,
    MAX_PROVENANCE_PER_RULE_PACKAGE_SET, MAX_RULE_PACKAGES_PER_SET,
    MAX_SOURCES_PER_RULE_PACKAGE_SET,
};
