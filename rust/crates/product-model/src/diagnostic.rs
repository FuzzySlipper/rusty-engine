use std::fmt;

use serde::Serialize;

pub const MAX_DIAGNOSTIC_MESSAGE_BYTES: usize = 1_024;

/// Stable machine-readable failure information for one product artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductModelDiagnostic {
    code: String,
    source: String,
    path: String,
    message: String,
}

impl ProductModelDiagnostic {
    pub(crate) fn new(
        code: impl Into<String>,
        source: impl Into<String>,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            source: bounded(source.into()),
            path: bounded(path.into()),
            message: bounded(message.into()),
        }
    }

    pub fn code(&self) -> &str {
        &self.code
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

fn bounded(value: String) -> String {
    if value.len() <= MAX_DIAGNOSTIC_MESSAGE_BYTES {
        return value;
    }
    const ELLIPSIS: &str = "…";
    let mut end = MAX_DIAGNOSTIC_MESSAGE_BYTES - ELLIPSIS.len();
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}{ELLIPSIS}", &value[..end])
}

/// A validation failure that deliberately preserves its structured diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductModelError {
    diagnostic: ProductModelDiagnostic,
}

impl ProductModelError {
    pub(crate) fn new(diagnostic: ProductModelDiagnostic) -> Self {
        Self { diagnostic }
    }

    pub fn diagnostic(&self) -> &ProductModelDiagnostic {
        &self.diagnostic
    }
}

impl fmt::Display for ProductModelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let diagnostic = self.diagnostic();
        write!(
            formatter,
            "{} at {} ({}): {}",
            diagnostic.code(),
            diagnostic.path(),
            diagnostic.source(),
            diagnostic.message()
        )
    }
}

impl std::error::Error for ProductModelError {}

pub(crate) fn failure(
    code: &str,
    source: &str,
    path: impl Into<String>,
    message: impl Into<String>,
) -> ProductModelError {
    ProductModelError::new(ProductModelDiagnostic::new(code, source, path, message))
}

#[cfg(test)]
mod tests {
    use super::{bounded, MAX_DIAGNOSTIC_MESSAGE_BYTES};

    #[test]
    fn diagnostic_truncation_includes_ellipsis_in_its_utf8_bound() {
        let exact = "a".repeat(MAX_DIAGNOSTIC_MESSAGE_BYTES);
        assert_eq!(bounded(exact.clone()), exact);

        let non_ascii = "é".repeat(MAX_DIAGNOSTIC_MESSAGE_BYTES);
        let bounded = bounded(non_ascii);
        assert!(bounded.len() <= MAX_DIAGNOSTIC_MESSAGE_BYTES);
        assert!(bounded.ends_with('…'));
        assert!(bounded.is_char_boundary(bounded.len()));
    }
}
