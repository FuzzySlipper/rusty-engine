use std::{fmt, io};

use serde::Serialize;

const MAX_ERROR_TEXT_BYTES: usize = 2_048;

/// One bounded, machine-readable Product Assembly failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssemblyDiagnostic {
    code: String,
    path: String,
    message: String,
}

impl AssemblyDiagnostic {
    pub fn code(&self) -> &str {
        &self.code
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

/// A fail-closed planner or publisher error with recoverable diagnostic data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductAssemblyError {
    diagnostic: AssemblyDiagnostic,
}

impl ProductAssemblyError {
    pub(crate) fn new(
        code: impl Into<String>,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            diagnostic: AssemblyDiagnostic {
                code: bounded(code.into()),
                path: bounded(path.into()),
                message: bounded(message.into()),
            },
        }
    }

    pub(crate) fn io(code: impl Into<String>, path: impl Into<String>, error: io::Error) -> Self {
        Self::new(code, path, error.to_string())
    }

    pub fn diagnostic(&self) -> &AssemblyDiagnostic {
        &self.diagnostic
    }
}

impl fmt::Display for ProductAssemblyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let diagnostic = self.diagnostic();
        write!(
            formatter,
            "{} at {}: {}",
            diagnostic.code(),
            diagnostic.path(),
            diagnostic.message()
        )
    }
}

impl std::error::Error for ProductAssemblyError {}

fn bounded(value: String) -> String {
    if value.len() <= MAX_ERROR_TEXT_BYTES {
        return value;
    }
    const ELLIPSIS: &str = "…";
    let mut end = MAX_ERROR_TEXT_BYTES - ELLIPSIS.len();
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}{ELLIPSIS}", &value[..end])
}
