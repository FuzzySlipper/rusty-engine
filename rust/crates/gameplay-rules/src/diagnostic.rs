use std::cmp::Ordering;

use crate::{
    RuleDiagnosticError, RulePackageIdentity, RuleSourceId, RuleSubjectId, MAX_SAFE_JSON_INTEGER,
};

pub const MAX_RULE_DIAGNOSTICS: usize = 256;
pub const MAX_DIAGNOSTIC_CODE_BYTES: usize = 64;
pub const MAX_DIAGNOSTIC_LOGICAL_PATH_BYTES: usize = 512;
pub const MAX_DIAGNOSTIC_MESSAGE_BYTES: usize = 2_048;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RuleDiagnosticSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuleDiagnosticCorrelation {
    subject: RuleSubjectId,
    source: RuleSourceId,
    line: Option<u64>,
    column: Option<u64>,
}

impl RuleDiagnosticCorrelation {
    pub fn new(
        subject: RuleSubjectId,
        source: RuleSourceId,
        line: Option<u64>,
        column: Option<u64>,
    ) -> Result<Self, RuleDiagnosticError> {
        validate_location("line", line)?;
        validate_location("column", column)?;
        Ok(Self {
            subject,
            source,
            line,
            column,
        })
    }

    pub const fn subject(&self) -> &RuleSubjectId {
        &self.subject
    }

    pub const fn source(&self) -> &RuleSourceId {
        &self.source
    }

    pub const fn line(&self) -> Option<u64> {
        self.line
    }

    pub const fn column(&self) -> Option<u64> {
        self.column
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleDiagnostic {
    code: String,
    severity: RuleDiagnosticSeverity,
    logical_path: String,
    message: String,
    package: Option<RulePackageIdentity>,
    correlation: Option<RuleDiagnosticCorrelation>,
}

impl RuleDiagnostic {
    pub fn new(
        code: impl Into<String>,
        severity: RuleDiagnosticSeverity,
        logical_path: impl Into<String>,
        message: impl Into<String>,
        package: Option<RulePackageIdentity>,
        correlation: Option<RuleDiagnosticCorrelation>,
    ) -> Result<Self, RuleDiagnosticError> {
        let code = code.into();
        validate_code(&code)?;
        let logical_path = logical_path.into();
        validate_logical_path(&logical_path)?;
        let message = message.into();
        validate_message(&message)?;
        Ok(Self {
            code,
            severity,
            logical_path,
            message,
            package,
            correlation,
        })
    }

    pub fn code(&self) -> &str {
        &self.code
    }

    pub const fn severity(&self) -> RuleDiagnosticSeverity {
        self.severity
    }

    pub fn logical_path(&self) -> &str {
        &self.logical_path
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub const fn package(&self) -> Option<&RulePackageIdentity> {
        self.package.as_ref()
    }

    pub const fn correlation(&self) -> Option<&RuleDiagnosticCorrelation> {
        self.correlation.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleDiagnosticReport {
    diagnostics: Vec<RuleDiagnostic>,
    has_errors: bool,
}

impl RuleDiagnosticReport {
    pub fn new(mut diagnostics: Vec<RuleDiagnostic>) -> Result<Self, RuleDiagnosticError> {
        if diagnostics.len() > MAX_RULE_DIAGNOSTICS {
            return Err(RuleDiagnosticError::QuotaExceeded {
                actual: diagnostics.len(),
                maximum: MAX_RULE_DIAGNOSTICS,
            });
        }
        diagnostics.sort_by(compare_diagnostics);
        let has_errors = diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == RuleDiagnosticSeverity::Error);
        Ok(Self {
            diagnostics,
            has_errors,
        })
    }

    pub fn diagnostics(&self) -> &[RuleDiagnostic] {
        &self.diagnostics
    }

    pub const fn has_errors(&self) -> bool {
        self.has_errors
    }

    pub fn into_diagnostics(self) -> Vec<RuleDiagnostic> {
        self.diagnostics
    }
}

fn compare_diagnostics(left: &RuleDiagnostic, right: &RuleDiagnostic) -> Ordering {
    left.package
        .cmp(&right.package)
        .then_with(|| left.logical_path.cmp(&right.logical_path))
        .then_with(|| left.code.cmp(&right.code))
        .then_with(|| left.severity.cmp(&right.severity))
        .then_with(|| left.message.cmp(&right.message))
        .then_with(|| left.correlation.cmp(&right.correlation))
}

fn validate_code(value: &str) -> Result<(), RuleDiagnosticError> {
    validate_nonempty_trimmed_ascii(value, MAX_DIAGNOSTIC_CODE_BYTES).map_err(|reason| {
        RuleDiagnosticError::InvalidCode {
            value: value.to_string(),
            reason,
        }
    })
}

fn validate_logical_path(value: &str) -> Result<(), RuleDiagnosticError> {
    if value.is_empty() {
        return Err(RuleDiagnosticError::InvalidLogicalPath {
            value: value.to_string(),
            reason: "logical path is empty",
        });
    }
    if value.len() > MAX_DIAGNOSTIC_LOGICAL_PATH_BYTES {
        return Err(RuleDiagnosticError::InvalidLogicalPath {
            value: value.to_string(),
            reason: "logical path exceeds the byte limit",
        });
    }
    if value.trim() != value {
        return Err(RuleDiagnosticError::InvalidLogicalPath {
            value: value.to_string(),
            reason: "logical path has leading or trailing whitespace",
        });
    }
    if value.chars().any(char::is_control) {
        return Err(RuleDiagnosticError::InvalidLogicalPath {
            value: value.to_string(),
            reason: "logical path contains a control character",
        });
    }
    Ok(())
}

fn validate_message(value: &str) -> Result<(), RuleDiagnosticError> {
    if value.is_empty() {
        return Err(RuleDiagnosticError::InvalidMessage {
            reason: "message is empty",
        });
    }
    if value.len() > MAX_DIAGNOSTIC_MESSAGE_BYTES {
        return Err(RuleDiagnosticError::InvalidMessage {
            reason: "message exceeds the byte limit",
        });
    }
    Ok(())
}

fn validate_nonempty_trimmed_ascii(value: &str, maximum: usize) -> Result<(), &'static str> {
    if value.is_empty() {
        return Err("value is empty");
    }
    if value.len() > maximum {
        return Err("value exceeds the byte limit");
    }
    if value.trim() != value {
        return Err("value has leading or trailing whitespace");
    }
    if !value.bytes().all(|byte| (0x20..=0x7e).contains(&byte)) {
        return Err("value must contain printable ASCII only");
    }
    Ok(())
}

fn validate_location(field: &'static str, value: Option<u64>) -> Result<(), RuleDiagnosticError> {
    if let Some(value) = value {
        if value == 0 || value > MAX_SAFE_JSON_INTEGER {
            return Err(RuleDiagnosticError::InvalidSourceLocation { field, value });
        }
    }
    Ok(())
}
