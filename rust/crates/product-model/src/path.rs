use std::fmt;

use crate::{diagnostic::failure, ProductModelError};

pub const MAX_PRODUCT_PATH_BYTES: usize = 512;

/// A normalized, product-relative UTF-8 path. It is not a filesystem handle.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProductPath(String);

impl ProductPath {
    pub fn parse(value: impl Into<String>) -> Result<Self, ProductModelError> {
        Self::parse_at(value.into(), "product-path", "$")
    }

    pub(crate) fn parse_at(
        value: String,
        source: &str,
        path: &str,
    ) -> Result<Self, ProductModelError> {
        if value.is_empty() {
            return Err(failure(
                "PRODUCT_PATH_EMPTY",
                source,
                path,
                "product paths must not be empty",
            ));
        }
        if value.len() > MAX_PRODUCT_PATH_BYTES {
            return Err(failure(
                "PRODUCT_PATH_TOO_LONG",
                source,
                path,
                format!("product paths are limited to {MAX_PRODUCT_PATH_BYTES} UTF-8 bytes"),
            ));
        }
        if value.contains('\\') {
            return Err(failure(
                "PRODUCT_PATH_BACKSLASH",
                source,
                path,
                "use slash-separated product-relative paths; backslashes are ambiguous across hosts",
            ));
        }
        if value.starts_with('/') || value.starts_with("//") {
            return Err(failure(
                "PRODUCT_PATH_ABSOLUTE",
                source,
                path,
                "product paths must be relative to the product root",
            ));
        }
        if value.bytes().any(|byte| byte.is_ascii_control()) {
            return Err(failure(
                "PRODUCT_PATH_CONTROL_CHARACTER",
                source,
                path,
                "product paths must not contain control characters",
            ));
        }
        if value.contains(':') {
            return Err(failure(
                "PRODUCT_PATH_COLON",
                source,
                path,
                "product paths must not contain colons, drive prefixes, or URI-like aliases",
            ));
        }
        if value.chars().any(char::is_whitespace) {
            return Err(failure(
                "PRODUCT_PATH_WHITESPACE",
                source,
                path,
                "product paths must not contain whitespace aliases",
            ));
        }
        for segment in value.split('/') {
            if segment.is_empty() || segment == "." {
                return Err(failure(
                    "PRODUCT_PATH_AMBIGUOUS",
                    source,
                    path,
                    "product paths must not contain empty or dot segments",
                ));
            }
            if segment == ".." {
                return Err(failure(
                    "PRODUCT_PATH_TRAVERSAL",
                    source,
                    path,
                    "product paths must not traverse above their product lane",
                ));
            }
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn is_within_or_equal(&self, parent: &Self) -> bool {
        self == parent
            || self
                .as_str()
                .strip_prefix(parent.as_str())
                .is_some_and(|suffix| suffix.starts_with('/'))
    }

    pub(crate) fn starts_in_lane(&self, lane: &str) -> bool {
        self.as_str() == lane
            || self
                .as_str()
                .strip_prefix(lane)
                .is_some_and(|suffix| suffix.starts_with('/'))
    }
}

impl fmt::Display for ProductPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
