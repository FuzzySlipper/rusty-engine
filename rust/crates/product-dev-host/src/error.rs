use std::{fmt, io};

/// Product-dev spellings remain public for transport and API continuity. The
/// host-neutral runtime/session vocabulary owns their representation and wire
/// names.
pub use runtime_session::{
    RuntimeInvalidatedScope as ProductDevInvalidatedScope,
    RuntimeMutationCertainty as ProductDevMutationCertainty,
    RuntimeNextAction as ProductDevNextAction, RuntimeRecovery as ProductDevRuntimeRecovery,
};

/// A stable, bounded diagnostic emitted by the development host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductDevHostError {
    code: &'static str,
    detail: String,
}

impl ProductDevHostError {
    /// Converts the one closed worker-activation failure into a bounded host
    /// transaction error without exposing a general worker error channel.
    pub fn worker_activation(detail: impl Into<String>) -> Self {
        Self::new("DEV_HOST_WORKER_ACTIVATE", detail)
    }

    pub(crate) fn new(code: &'static str, detail: impl Into<String>) -> Self {
        let mut detail = detail.into();
        const MAX_DETAIL_BYTES: usize = 512;
        if detail.len() > MAX_DETAIL_BYTES {
            detail.truncate(MAX_DETAIL_BYTES);
        }
        Self { code, detail }
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

impl fmt::Display for ProductDevHostError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.detail)
    }
}

impl std::error::Error for ProductDevHostError {}

impl From<runtime_diagnostics::RuntimeDiagnosticsError> for ProductDevHostError {
    fn from(value: runtime_diagnostics::RuntimeDiagnosticsError) -> Self {
        Self::new(value.code(), value.detail())
    }
}

/// A bounded runtime-owner diagnostic. It never crosses the transport as an
/// unconstrained error display or backtrace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductDevRuntimeError {
    code: String,
    diagnostic: String,
    recovery: ProductDevRuntimeRecovery,
}

impl ProductDevRuntimeError {
    /// Constructs an unclassified runtime failure.  Unknown failures are
    /// intentionally conservative: once a runtime operation may have crossed
    /// into product or Engine ownership, replacement is required.  Known
    /// source conditions should use one of the classified constructors below.
    pub fn new(
        code: impl Into<String>,
        diagnostic: impl Into<String>,
    ) -> Result<Self, ProductDevHostError> {
        Self::with_recovery(
            code,
            diagnostic,
            ProductDevRuntimeRecovery::incarnation_tainted(),
        )
    }

    /// Constructs a failure known to have been rejected before mutation.
    pub fn new_not_applied(
        code: impl Into<String>,
        diagnostic: impl Into<String>,
    ) -> Result<Self, ProductDevHostError> {
        Self::with_recovery(code, diagnostic, ProductDevRuntimeRecovery::not_applied())
    }

    /// Constructs a failure whose retained output projection must be rebuilt,
    /// but whose runtime incarnation is still the current owner.
    pub fn new_output_rebaseline(
        code: impl Into<String>,
        diagnostic: impl Into<String>,
    ) -> Result<Self, ProductDevHostError> {
        Self::with_recovery(
            code,
            diagnostic,
            ProductDevRuntimeRecovery::output_rebaseline(),
        )
    }

    pub fn with_recovery(
        code: impl Into<String>,
        diagnostic: impl Into<String>,
        recovery: ProductDevRuntimeRecovery,
    ) -> Result<Self, ProductDevHostError> {
        let code = code.into();
        let diagnostic = diagnostic.into();
        if !is_identity(&code) || diagnostic.len() > 1_024 {
            return Err(ProductDevHostError::new(
                "DEV_HOST_RUNTIME_DIAGNOSTIC",
                "runtime error code or diagnostic exceeds the closed host bounds",
            ));
        }
        Ok(Self {
            code,
            diagnostic,
            recovery,
        })
    }

    pub fn code(&self) -> &str {
        &self.code
    }

    pub fn diagnostic(&self) -> &str {
        &self.diagnostic
    }

    pub const fn recovery(&self) -> ProductDevRuntimeRecovery {
        self.recovery
    }
}

fn is_identity(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}
