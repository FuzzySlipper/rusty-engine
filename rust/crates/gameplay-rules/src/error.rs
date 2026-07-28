use std::fmt;

use crate::{
    RuleDomainId, RuleFingerprint, RulePackageDependency, RulePackageId, RulePackageIdentity,
    RuleSourceId, RuleSubjectId, RuleVersion,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RulePackageError {
    ArtifactQuotaExceeded {
        actual: usize,
        maximum: usize,
    },
    MalformedUtf8 {
        valid_up_to: usize,
    },
    MalformedJson {
        path: String,
        offset: usize,
        reason: String,
    },
    DuplicateJsonKey {
        path: String,
        key: String,
    },
    WrongArtifactKind {
        actual: String,
    },
    UnsupportedSchemaVersion {
        actual: String,
    },
    MissingField {
        path: String,
    },
    UnknownField {
        path: String,
    },
    InvalidFieldType {
        path: String,
        expected: &'static str,
    },
    InvalidIdentity {
        path: String,
        value: String,
        reason: &'static str,
    },
    InvalidVersion {
        path: String,
        value: String,
    },
    InvalidSourcePath {
        path: String,
        reason: &'static str,
    },
    InvalidSourceLocation {
        path: String,
        value: String,
    },
    InvalidFingerprint {
        path: String,
        value: String,
    },
    JsonIntegerOutOfRange {
        path: String,
        value: String,
    },
    QuotaExceeded {
        path: String,
        actual: usize,
        maximum: usize,
    },
    JsonDepthExceeded {
        path: String,
        actual: usize,
        maximum: usize,
    },
    JsonNodeQuotaExceeded {
        path: String,
        actual: usize,
        maximum: usize,
    },
    DuplicateDependency {
        dependency: RulePackageDependency,
    },
    DuplicateSource {
        source: RuleSourceId,
    },
    DuplicateProvenance {
        subject: RuleSubjectId,
    },
    UnknownProvenanceSource {
        subject: RuleSubjectId,
        source: RuleSourceId,
    },
    SelfDependency {
        dependency: RulePackageDependency,
    },
    NonCanonicalArtifact {
        canonical_fingerprint: RuleFingerprint,
    },
    ArithmeticOverflow {
        path: String,
    },
}

impl fmt::Display for RulePackageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for RulePackageError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RulePackageSetError {
    AggregateQuotaExceeded {
        field: &'static str,
        actual: usize,
        maximum: usize,
    },
    ArithmeticOverflow {
        field: &'static str,
    },
    DuplicatePackage {
        package: RulePackageIdentity,
    },
    ConflictingVersions {
        domain: RuleDomainId,
        package: RulePackageId,
        first: RuleVersion,
        second: RuleVersion,
    },
    MissingDependency {
        package: RulePackageIdentity,
        dependency: Box<RulePackageDependency>,
    },
    DependencyVersionMismatch {
        package: RulePackageIdentity,
        dependency: Box<RulePackageDependency>,
        available: RuleVersion,
    },
    DependencyFingerprintMismatch {
        package: RulePackageIdentity,
        dependency: Box<RulePackageDependency>,
        actual: RuleFingerprint,
    },
    DependencyCycle {
        packages: Vec<RulePackageIdentity>,
    },
}

impl fmt::Display for RulePackageSetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for RulePackageSetError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleDiagnosticError {
    QuotaExceeded { actual: usize, maximum: usize },
    InvalidCode { value: String, reason: &'static str },
    InvalidLogicalPath { value: String, reason: &'static str },
    InvalidMessage { reason: &'static str },
    InvalidSourceLocation { field: &'static str, value: u64 },
}

impl fmt::Display for RuleDiagnosticError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for RuleDiagnosticError {}
