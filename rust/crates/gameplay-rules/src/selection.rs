//! Bounded selection of one canonical payload subtree from an admitted package.
//!
//! The caller owns the product layout and supplies structured path segments.  This is not a
//! JSON-pointer interpreter or a package lookup service: it can only walk the already-admitted
//! payload of one explicitly supplied package and retains that package's identity and fingerprint
//! beside the selected canonical bytes.

use std::fmt;

use serde_json::Value;

use crate::{
    canonical_rule_json_value_bytes, AdmittedRulePackage, RuleFingerprint, RulePackageIdentity,
    RulePackageSchemaVersion,
};

pub const MAX_RULE_PAYLOAD_PATH_SEGMENTS: usize = 64;
pub const MAX_RULE_PAYLOAD_PATH_FIELD_BYTES: usize = 96;
pub const MAX_RULE_PAYLOAD_PATH_DISPLAY_BYTES: usize = 512;
pub const MAX_RULE_PAYLOAD_PATH_INDEX: usize = 1_000_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RulePayloadPathError {
    Empty,
    TooManySegments { actual: usize, maximum: usize },
    InvalidField { field: String },
    IndexTooLarge { actual: usize, maximum: usize },
    DisplayTooLong { actual: usize, maximum: usize },
}

impl fmt::Display for RulePayloadPathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid rule payload path: {self:?}")
    }
}
impl std::error::Error for RulePayloadPathError {}

/// One unambiguous traversal segment. Object fields use a deliberately narrow authored-key
/// alphabet; arrays are addressed only by their canonical zero-based index.
#[derive(Debug, Clone, PartialEq, Eq)]
enum RulePayloadPathSegmentKind {
    Field(String),
    Index(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RulePayloadPathSegment(RulePayloadPathSegmentKind);

impl RulePayloadPathSegment {
    pub fn field(value: impl Into<String>) -> Result<Self, RulePayloadPathError> {
        let value = value.into();
        validate_field(&value)?;
        Ok(Self(RulePayloadPathSegmentKind::Field(value)))
    }

    pub fn index(value: usize) -> Result<Self, RulePayloadPathError> {
        if value > MAX_RULE_PAYLOAD_PATH_INDEX {
            return Err(RulePayloadPathError::IndexTooLarge {
                actual: value,
                maximum: MAX_RULE_PAYLOAD_PATH_INDEX,
            });
        }
        Ok(Self(RulePayloadPathSegmentKind::Index(value)))
    }
}

fn validate_field(value: &str) -> Result<(), RulePayloadPathError> {
    if value.is_empty()
        || value.len() > MAX_RULE_PAYLOAD_PATH_FIELD_BYTES
        || !value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_uppercase()
                || (index != 0 && byte.is_ascii_digit())
                || (index != 0 && (byte == b'_' || byte == b'-'))
        })
    {
        return Err(RulePayloadPathError::InvalidField {
            field: value.to_owned(),
        });
    }
    Ok(())
}

/// A caller-owned product payload location. Construction, rather than parsing a text pointer,
/// makes empty paths, escapes, duplicate spellings, and accidental service-locator use invalid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RulePayloadPath {
    segments: Vec<RulePayloadPathSegment>,
    display: String,
}

impl RulePayloadPath {
    pub fn new(segments: Vec<RulePayloadPathSegment>) -> Result<Self, RulePayloadPathError> {
        if segments.is_empty() {
            return Err(RulePayloadPathError::Empty);
        }
        if segments.len() > MAX_RULE_PAYLOAD_PATH_SEGMENTS {
            return Err(RulePayloadPathError::TooManySegments {
                actual: segments.len(),
                maximum: MAX_RULE_PAYLOAD_PATH_SEGMENTS,
            });
        }
        let mut display = String::from("payload");
        for segment in &segments {
            match &segment.0 {
                RulePayloadPathSegmentKind::Field(field) => {
                    display.push('.');
                    display.push_str(field);
                }
                RulePayloadPathSegmentKind::Index(index) => {
                    display.push('[');
                    display.push_str(&index.to_string());
                    display.push(']');
                }
            }
        }
        if display.len() > MAX_RULE_PAYLOAD_PATH_DISPLAY_BYTES {
            return Err(RulePayloadPathError::DisplayTooLong {
                actual: display.len(),
                maximum: MAX_RULE_PAYLOAD_PATH_DISPLAY_BYTES,
            });
        }
        Ok(Self { segments, display })
    }

    pub fn segments(&self) -> &[RulePayloadPathSegment] {
        &self.segments
    }

    pub fn display(&self) -> &str {
        &self.display
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleSubtreeSelectionError {
    Path(RulePayloadPathError),
    ParentFingerprintMismatch {
        expected: RuleFingerprint,
        actual: RuleFingerprint,
    },
    MissingField {
        path: String,
    },
    IndexOutOfBounds {
        path: String,
        index: usize,
        length: usize,
    },
    ExpectedObject {
        path: String,
    },
    ExpectedArray {
        path: String,
    },
    Canonical(crate::RulePackageError),
}

impl fmt::Display for RuleSubtreeSelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "rule payload subtree selection rejected: {self:?}"
        )
    }
}
impl std::error::Error for RuleSubtreeSelectionError {}

/// Immutable proof that one canonical value came from a particular admitted package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedRulePayloadSubtree {
    parent: AdmittedRulePackage,
    path: RulePayloadPath,
    value: Value,
    canonical_bytes: Vec<u8>,
}

impl SelectedRulePayloadSubtree {
    pub fn parent(&self) -> &AdmittedRulePackage {
        &self.parent
    }
    pub fn parent_identity(&self) -> &RulePackageIdentity {
        self.parent.identity()
    }
    pub fn parent_fingerprint(&self) -> &RuleFingerprint {
        self.parent.fingerprint()
    }
    pub fn parent_schema_version(&self) -> RulePackageSchemaVersion {
        self.parent.schema_version()
    }
    pub fn path(&self) -> &RulePayloadPath {
        &self.path
    }
    pub fn value(&self) -> &Value {
        &self.value
    }
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
}

/// Selects one payload subtree from precisely the admitted parent the caller expects.
pub fn select_rule_payload_subtree(
    package: &AdmittedRulePackage,
    expected_parent_fingerprint: &RuleFingerprint,
    path: RulePayloadPath,
) -> Result<SelectedRulePayloadSubtree, RuleSubtreeSelectionError> {
    if package.fingerprint() != expected_parent_fingerprint {
        return Err(RuleSubtreeSelectionError::ParentFingerprintMismatch {
            expected: expected_parent_fingerprint.clone(),
            actual: package.fingerprint().clone(),
        });
    }
    let mut value = package.payload();
    let mut at = String::from("payload");
    for segment in path.segments() {
        match &segment.0 {
            RulePayloadPathSegmentKind::Field(field) => {
                let object =
                    value
                        .as_object()
                        .ok_or_else(|| RuleSubtreeSelectionError::ExpectedObject {
                            path: at.clone(),
                        })?;
                at.push('.');
                at.push_str(field);
                value = object
                    .get(field)
                    .ok_or_else(|| RuleSubtreeSelectionError::MissingField { path: at.clone() })?;
            }
            RulePayloadPathSegmentKind::Index(index) => {
                let array = value
                    .as_array()
                    .ok_or_else(|| RuleSubtreeSelectionError::ExpectedArray { path: at.clone() })?;
                at.push('[');
                at.push_str(&index.to_string());
                at.push(']');
                value = array.get(*index).ok_or_else(|| {
                    RuleSubtreeSelectionError::IndexOutOfBounds {
                        path: at.clone(),
                        index: *index,
                        length: array.len(),
                    }
                })?;
            }
        }
    }
    debug_assert_eq!(at, path.display());
    let value = value.clone();
    let canonical_bytes = canonical_rule_json_value_bytes(
        &value,
        package.schema_version(),
        crate::MAX_ENCODED_RULE_PACKAGE_BYTES,
    )
    .map_err(RuleSubtreeSelectionError::Canonical)?;
    Ok(SelectedRulePayloadSubtree {
        parent: package.clone(),
        path,
        value,
        canonical_bytes,
    })
}
