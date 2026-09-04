use serde::{Deserialize, Serialize};
use std::{fmt, io};

/// The small semantic recovery vocabulary shared by a runtime owner and its
/// development host.  This is deliberately not a generic operation-result
/// hierarchy: it only says whether an operation changed authoritative state,
/// which host-owned scope can no longer be trusted, and what the host may do
/// next.  The diagnostic code and text remain separate for observability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProductDevMutationCertainty {
    /// The condition was discovered before the operation could mutate state.
    NotApplied,
    /// The operation and its owned effects were committed.
    Committed,
    /// The operation crossed an ownership boundary and its effect is unknown.
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProductDevInvalidatedScope {
    /// No host-visible state needs to be re-established.
    None,
    /// The input cursor or held-input projection is no longer authoritative.
    Input,
    /// Retained output/projection state needs a fresh baseline.
    Outputs,
    /// The loaded runtime incarnation cannot be trusted to continue.
    Incarnation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProductDevNextAction {
    /// Continue using the current runtime and its current binding.
    Continue,
    /// Re-establish the invalidated scope before issuing dependent work.
    Rebaseline,
    /// Replace the runtime incarnation; the developer session may remain.
    ReplaceIncarnation,
}

/// Source-owned facts that let a host choose recovery without parsing a
/// diagnostic code or message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductDevRuntimeRecovery {
    pub mutation: ProductDevMutationCertainty,
    pub invalidated_scope: ProductDevInvalidatedScope,
    pub next_action: ProductDevNextAction,
}

impl ProductDevRuntimeRecovery {
    pub const fn committed() -> Self {
        Self {
            mutation: ProductDevMutationCertainty::Committed,
            invalidated_scope: ProductDevInvalidatedScope::None,
            next_action: ProductDevNextAction::Continue,
        }
    }

    pub const fn not_applied() -> Self {
        Self {
            mutation: ProductDevMutationCertainty::NotApplied,
            invalidated_scope: ProductDevInvalidatedScope::None,
            next_action: ProductDevNextAction::Continue,
        }
    }

    pub const fn output_rebaseline() -> Self {
        Self {
            mutation: ProductDevMutationCertainty::Unknown,
            invalidated_scope: ProductDevInvalidatedScope::Outputs,
            next_action: ProductDevNextAction::Rebaseline,
        }
    }

    pub const fn incarnation_tainted() -> Self {
        Self {
            mutation: ProductDevMutationCertainty::Unknown,
            invalidated_scope: ProductDevInvalidatedScope::Incarnation,
            next_action: ProductDevNextAction::ReplaceIncarnation,
        }
    }

    pub const fn mutation(self) -> ProductDevMutationCertainty {
        self.mutation
    }

    pub const fn invalidated_scope(self) -> ProductDevInvalidatedScope {
        self.invalidated_scope
    }

    pub const fn next_action(self) -> ProductDevNextAction {
        self.next_action
    }
}

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
