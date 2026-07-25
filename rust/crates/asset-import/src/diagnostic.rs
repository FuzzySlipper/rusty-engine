#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportSeverity {
    Warning,
    Error,
}

impl ImportSeverity {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ImportCode {
    SourceTooLarge,
    UnsupportedSchema,
    MalformedSource,
    UnsupportedFeature,
    UnsupportedTopology,
    AttributeLengthMismatch,
    IndexOutOfRange,
    NonFiniteValue,
    MissingTexture,
    DuplicateAssetId,
    DuplicateMaterialSlot,
    GroupSlotUnbound,
    InvalidGroupRange,
    InvalidDescriptor,
    InvalidImportSettings,
    SourceFingerprintChanged,
}

impl ImportCode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::SourceTooLarge => "sourceTooLarge",
            Self::UnsupportedSchema => "unsupportedSchema",
            Self::MalformedSource => "malformedSource",
            Self::UnsupportedFeature => "unsupportedFeature",
            Self::UnsupportedTopology => "unsupportedTopology",
            Self::AttributeLengthMismatch => "attributeLengthMismatch",
            Self::IndexOutOfRange => "indexOutOfRange",
            Self::NonFiniteValue => "nonFiniteValue",
            Self::MissingTexture => "missingTexture",
            Self::DuplicateAssetId => "duplicateAssetId",
            Self::DuplicateMaterialSlot => "duplicateMaterialSlot",
            Self::GroupSlotUnbound => "groupSlotUnbound",
            Self::InvalidGroupRange => "invalidGroupRange",
            Self::InvalidDescriptor => "invalidDescriptor",
            Self::InvalidImportSettings => "invalidImportSettings",
            Self::SourceFingerprintChanged => "sourceFingerprintChanged",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportDiagnostic {
    pub severity: ImportSeverity,
    pub code: ImportCode,
    pub locus: String,
    pub message: String,
    pub remedy: String,
}

impl ImportDiagnostic {
    pub fn error(
        code: ImportCode,
        locus: impl Into<String>,
        message: impl Into<String>,
        remedy: impl Into<String>,
    ) -> Self {
        Self {
            severity: ImportSeverity::Error,
            code,
            locus: locus.into(),
            message: message.into(),
            remedy: remedy.into(),
        }
    }

    pub fn warning(
        code: ImportCode,
        locus: impl Into<String>,
        message: impl Into<String>,
        remedy: impl Into<String>,
    ) -> Self {
        Self {
            severity: ImportSeverity::Warning,
            code,
            locus: locus.into(),
            message: message.into(),
            remedy: remedy.into(),
        }
    }

    pub fn is_error(&self) -> bool {
        self.severity == ImportSeverity::Error
    }

    pub fn render(&self) -> String {
        format!(
            "{} [{}] {}: {} (remedy: {})",
            self.severity.label(),
            self.code.label(),
            self.locus,
            self.message,
            self.remedy
        )
    }
}
