use std::{fmt, io};

use crate::ProductHostFaultDisposition;

/// A stable, bounded diagnostic emitted by the product host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductHostError {
    code: &'static str,
    detail: String,
}

impl ProductHostError {
    pub(crate) fn new(code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }

    pub const fn code(&self) -> &'static str {
        self.code
    }

    pub fn detail(&self) -> &str {
        &self.detail
    }

    pub(crate) fn io(code: &'static str, error: io::Error) -> Self {
        Self::new(code, error.to_string())
    }
}

impl fmt::Display for ProductHostError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.detail)
    }
}

impl std::error::Error for ProductHostError {}

impl From<runtime_diagnostics::RuntimeDiagnosticsError> for ProductHostError {
    fn from(value: runtime_diagnostics::RuntimeDiagnosticsError) -> Self {
        Self::new(value.code(), value.detail())
    }
}

/// A bounded runtime-owner diagnostic. It never crosses the transport as an
/// unconstrained error display or backtrace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductHostRuntimeError {
    code: String,
    diagnostic: String,
    disposition: ProductHostFaultDisposition,
}

impl ProductHostRuntimeError {
    /// Constructs a failure that may have crossed into product or Engine
    /// ownership. Known source conditions use one of the constructors below.
    pub fn new(code: impl Into<String>, diagnostic: impl Into<String>) -> Self {
        Self::with_disposition(code, diagnostic, ProductHostFaultDisposition::Terminal)
    }

    /// Constructs a failure known to have been rejected before mutation.
    pub fn new_not_applied(code: impl Into<String>, diagnostic: impl Into<String>) -> Self {
        Self::with_disposition(
            code,
            diagnostic,
            ProductHostFaultDisposition::RejectedRecoverable,
        )
    }

    /// Keeps the complete diagnostic, including multiline managed stack traces.
    fn with_disposition(
        code: impl Into<String>,
        diagnostic: impl Into<String>,
        disposition: ProductHostFaultDisposition,
    ) -> Self {
        Self {
            code: code.into(),
            diagnostic: diagnostic.into(),
            disposition,
        }
    }

    pub fn code(&self) -> &str {
        &self.code
    }

    pub fn diagnostic(&self) -> &str {
        &self.diagnostic
    }

    pub const fn disposition(&self) -> ProductHostFaultDisposition {
        self.disposition
    }
}
