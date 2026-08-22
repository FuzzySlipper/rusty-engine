use std::fmt;

use gameplay_rules::{RulePackageError, RuleSourceId, RuleSubjectId};

use crate::composed::ComposedExactLeafKindId;
use crate::{
    CapabilityRoleId, ExactEvaluationError, RoleRequirementError, StandardExtensionError,
    StandardExtensionSchema,
};

#[derive(Debug)]
pub enum ComposedExactDefinitionError {
    Package(RulePackageError),
    Role(RoleRequirementError),
    Extension(StandardExtensionError),
    Json(serde_json::Error),
    ExactLiteral {
        path: String,
        error: gameplay_mechanics::MechanicsArithmeticError,
    },
    MalformedPayload {
        path: String,
        reason: String,
    },
    WrongSchema {
        expected: u64,
        actual: u64,
    },
    WrongFamily,
    UnsupportedSemanticsVersion,
    MissingCorrelation {
        subject: RuleSubjectId,
        source: RuleSourceId,
    },
    SourceMismatch {
        subject: RuleSubjectId,
        expected: RuleSourceId,
        actual: RuleSourceId,
    },
    PayloadQuotaExceeded {
        actual: usize,
        maximum: usize,
    },
    DepthQuotaExceeded {
        actual: usize,
        maximum: usize,
    },
    NodeQuotaExceeded {
        actual: usize,
        maximum: usize,
    },
    ArityQuotaExceeded {
        actual: usize,
        maximum: usize,
    },
    EmptyAggregate,
    UndeclaredInputRole {
        role: CapabilityRoleId,
    },
    MissingProductCapability {
        role: CapabilityRoleId,
        capability: crate::CapabilityRequirementId,
    },
}
impl fmt::Display for ComposedExactDefinitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "composed exact definition rejected: {self:?}")
    }
}
impl std::error::Error for ComposedExactDefinitionError {}

#[derive(Debug)]
pub enum ComposedExactError<E> {
    Wire(ComposedExactDefinitionError),
    Package(RulePackageError),
    SchemaMismatch {
        expected: StandardExtensionSchema,
        actual: StandardExtensionSchema,
    },
    ProductEncode {
        context: Box<ComposedExactProductContext>,
        error: Box<E>,
    },
    ProductDecode {
        context: Box<ComposedExactProductContext>,
        error: Box<E>,
    },
    ProductCompile {
        context: Box<ComposedExactProductContext>,
        error: Box<E>,
    },
    ProductRequirementMismatch {
        context: Box<ComposedExactProductContext>,
    },
    ProductNonConvergentPayload {
        context: Box<ComposedExactProductContext>,
    },
    Standard(ExactEvaluationError),
    NonConvergentPayload,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposedExactProductContext {
    path: String,
    schema: StandardExtensionSchema,
    kind: ComposedExactLeafKindId,
    subject: RuleSubjectId,
    source: RuleSourceId,
}

impl ComposedExactProductContext {
    pub(crate) fn new(
        path: String,
        schema: StandardExtensionSchema,
        kind: ComposedExactLeafKindId,
        subject: RuleSubjectId,
        source: RuleSourceId,
    ) -> Self {
        Self {
            path,
            schema,
            kind,
            subject,
            source,
        }
    }

    pub fn path(&self) -> &str {
        &self.path
    }
    pub fn schema(&self) -> &StandardExtensionSchema {
        &self.schema
    }
    pub fn kind(&self) -> &ComposedExactLeafKindId {
        &self.kind
    }
    pub fn subject(&self) -> &RuleSubjectId {
        &self.subject
    }
    pub fn source(&self) -> &RuleSourceId {
        &self.source
    }
}
impl<E: fmt::Display> fmt::Display for ComposedExactError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Wire(error) => write!(f, "composed exact wire rejected: {error}"),
            Self::Package(error) => write!(f, "composed exact package rejected: {error}"),
            Self::SchemaMismatch { .. } => {
                f.write_str("composed exact extension schema does not match its concrete codec")
            }
            Self::ProductEncode { error, .. } => {
                write!(f, "composed exact product encoding failed: {error}")
            }
            Self::ProductDecode { error, .. } => {
                write!(f, "composed exact product decode failed: {error}")
            }
            Self::ProductCompile { error, .. } => {
                write!(f, "composed exact product compilation failed: {error}")
            }
            Self::ProductRequirementMismatch { .. } => f.write_str(
                "composed exact product requirement declaration does not match compiled expression",
            ),
            Self::ProductNonConvergentPayload { .. } => f.write_str(
                "composed exact product payload does not converge through its strict codec",
            ),
            Self::Standard(error) => {
                write!(f, "composed exact standard expression rejected: {error}")
            }
            Self::NonConvergentPayload => f.write_str(
                "composed exact typed construction does not converge on canonical payload",
            ),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for ComposedExactError<E> {}
impl<E> From<ComposedExactDefinitionError> for ComposedExactError<E> {
    fn from(value: ComposedExactDefinitionError) -> Self {
        Self::Wire(value)
    }
}
