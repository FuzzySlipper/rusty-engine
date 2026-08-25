use std::{fmt, io};

/// A stable, bounded diagnostic emitted by the development host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductDevHostError {
    code: &'static str,
    detail: String,
}

impl ProductDevHostError {
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
}

impl ProductDevRuntimeError {
    pub fn new(
        code: impl Into<String>,
        diagnostic: impl Into<String>,
    ) -> Result<Self, ProductDevHostError> {
        let code = code.into();
        let diagnostic = diagnostic.into();
        if !is_identity(&code) || diagnostic.len() > 1_024 {
            return Err(ProductDevHostError::new(
                "DEV_HOST_RUNTIME_DIAGNOSTIC",
                "runtime error code or diagnostic exceeds the closed host bounds",
            ));
        }
        Ok(Self { code, diagnostic })
    }

    pub fn code(&self) -> &str {
        &self.code
    }

    pub fn diagnostic(&self) -> &str {
        &self.diagnostic
    }
}

fn is_identity(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}
