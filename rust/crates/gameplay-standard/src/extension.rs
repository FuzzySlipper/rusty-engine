use std::fmt;

use gameplay_rules::{
    admit_rule_package, AdmittedRulePackage, RulePackageError, RuleSourceId, RuleSubjectId,
};
use serde_json::{json, Value};

use crate::{CapabilityRequirementId, InputId, StandardPackageContext};

pub const STANDARD_EXTENSION_ARTIFACT_FAMILY: &str = "standardExtension";
pub const MAX_STANDARD_EXTENSION_PAYLOAD_BYTES: usize = 64 * 1024;

/// A declared product schema identity. Engine admits it but never evaluates it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StandardExtensionSchema {
    namespace: CapabilityRequirementId,
    version: u32,
}
impl StandardExtensionSchema {
    pub fn new(
        namespace: CapabilityRequirementId,
        version: u32,
    ) -> Result<Self, StandardExtensionError> {
        if version == 0 {
            return Err(StandardExtensionError::ZeroSchemaVersion);
        }
        if !namespace.as_str().bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-')
        }) {
            return Err(StandardExtensionError::InvalidNamespace {
                value: namespace.as_str().to_owned(),
            });
        }
        Ok(Self { namespace, version })
    }
    pub fn namespace(&self) -> &CapabilityRequirementId {
        &self.namespace
    }
    pub const fn version(&self) -> u32 {
        self.version
    }
}

/// One bounded, source-correlated exchange artifact for a downstream product schema.
#[derive(Debug, Clone, PartialEq)]
pub struct StandardExtensionArtifact {
    schema: StandardExtensionSchema,
    kind: InputId,
    subject: RuleSubjectId,
    source: RuleSourceId,
    payload: Value,
}
impl StandardExtensionArtifact {
    pub fn new(
        schema: StandardExtensionSchema,
        kind: InputId,
        subject: RuleSubjectId,
        source: RuleSourceId,
        payload: Value,
    ) -> Result<Self, StandardExtensionError> {
        let bytes = serde_json::to_vec(&payload).map_err(StandardExtensionError::Json)?;
        if bytes.len() > MAX_STANDARD_EXTENSION_PAYLOAD_BYTES {
            return Err(StandardExtensionError::PayloadQuotaExceeded {
                actual: bytes.len(),
                maximum: MAX_STANDARD_EXTENSION_PAYLOAD_BYTES,
            });
        }
        Ok(Self {
            schema,
            kind,
            subject,
            source,
            payload,
        })
    }
    pub fn schema(&self) -> &StandardExtensionSchema {
        &self.schema
    }
    pub fn kind(&self) -> &InputId {
        &self.kind
    }
    pub fn subject(&self) -> &RuleSubjectId {
        &self.subject
    }
    pub fn source(&self) -> &RuleSourceId {
        &self.source
    }
    pub fn payload(&self) -> &Value {
        &self.payload
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AdmittedStandardExtension {
    package: AdmittedRulePackage,
    artifact: StandardExtensionArtifact,
}
impl AdmittedStandardExtension {
    pub fn package(&self) -> &AdmittedRulePackage {
        &self.package
    }
    pub fn artifact(&self) -> &StandardExtensionArtifact {
        &self.artifact
    }
}

pub fn admit_standard_extension(
    context: &StandardPackageContext,
    artifact: StandardExtensionArtifact,
) -> Result<AdmittedStandardExtension, StandardExtensionError> {
    let package = admit_rule_package(context.candidate(extension_payload(&artifact)))
        .map_err(StandardExtensionError::Package)?;
    let decoded = decode_standard_extension(&package)?;
    if decoded != artifact {
        return Err(StandardExtensionError::NonConvergentPayload);
    }
    Ok(AdmittedStandardExtension { package, artifact })
}

pub fn decode_standard_extension(
    package: &AdmittedRulePackage,
) -> Result<StandardExtensionArtifact, StandardExtensionError> {
    let root = package
        .payload()
        .as_object()
        .ok_or(StandardExtensionError::Malformed(
            "payload must be an object",
        ))?;
    for field in root.keys() {
        if ![
            "family",
            "namespace",
            "schemaVersion",
            "kind",
            "subject",
            "source",
            "payload",
        ]
        .contains(&field.as_str())
        {
            return Err(StandardExtensionError::UnknownField {
                field: field.clone(),
            });
        }
    }
    if root.get("family").and_then(Value::as_str) != Some(STANDARD_EXTENSION_ARTIFACT_FAMILY) {
        return Err(StandardExtensionError::WrongFamily);
    }
    let namespace = string(root, "namespace")?;
    let version = root
        .get("schemaVersion")
        .and_then(|value| {
            value.as_u64().or_else(|| {
                value
                    .as_f64()
                    .filter(|number| {
                        number.is_finite() && number.fract() == 0.0 && *number <= u64::MAX as f64
                    })
                    .map(|number| number as u64)
            })
        })
        .ok_or(StandardExtensionError::Malformed(
            "schemaVersion must be an integer",
        ))?;
    let version = u32::try_from(version)
        .map_err(|_| StandardExtensionError::Malformed("schemaVersion exceeds u32"))?;
    let schema = StandardExtensionSchema::new(
        CapabilityRequirementId::parse(namespace).map_err(StandardExtensionError::Identity)?,
        version,
    )?;
    let kind = InputId::parse(string(root, "kind")?).map_err(StandardExtensionError::Identity)?;
    let subject =
        RuleSubjectId::parse(string(root, "subject")?).map_err(StandardExtensionError::Package)?;
    let source =
        RuleSourceId::parse(string(root, "source")?).map_err(StandardExtensionError::Package)?;
    match package.correlated_source(&subject) {
        Some((provenance, _)) if provenance.source() == &source => {}
        Some(_) => return Err(StandardExtensionError::SourceMismatch),
        None => return Err(StandardExtensionError::MissingCorrelation),
    }
    let payload = root
        .get("payload")
        .ok_or(StandardExtensionError::Malformed(
            "payload payload is required",
        ))?
        .clone();
    StandardExtensionArtifact::new(schema, kind, subject, source, payload)
}
fn string<'a>(
    root: &'a serde_json::Map<String, Value>,
    field: &str,
) -> Result<&'a str, StandardExtensionError> {
    root.get(field)
        .and_then(Value::as_str)
        .ok_or(StandardExtensionError::Malformed(
            "required extension string field is missing",
        ))
}
fn extension_payload(artifact: &StandardExtensionArtifact) -> Value {
    json!({"family":STANDARD_EXTENSION_ARTIFACT_FAMILY,"namespace":artifact.schema.namespace().as_str(),"schemaVersion":artifact.schema.version(),"kind":artifact.kind().as_str(),"subject":artifact.subject().as_str(),"source":artifact.source().as_str(),"payload":artifact.payload()})
}

pub trait CompileStandardExtension {
    type Output;
    type Error: std::error::Error + 'static;
    fn schema(&self) -> &StandardExtensionSchema;
    fn compile(&self, artifact: &StandardExtensionArtifact) -> Result<Self::Output, Self::Error>;
}
#[derive(Debug)]
pub enum StandardExtensionCompileError<E> {
    SchemaMismatch {
        expected: StandardExtensionSchema,
        actual: StandardExtensionSchema,
    },
    Product(E),
}
impl<E: fmt::Display> fmt::Display for StandardExtensionCompileError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaMismatch { .. } => {
                f.write_str("standard extension schema does not match the concrete compiler")
            }
            Self::Product(error) => {
                write!(f, "standard extension compiler rejected artifact: {error}")
            }
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for StandardExtensionCompileError<E> {}
pub struct StandardExtensionCompilation<'a, Output> {
    admitted: &'a AdmittedStandardExtension,
    output: Output,
}
impl<'a, Output> StandardExtensionCompilation<'a, Output> {
    pub fn admitted(&self) -> &'a AdmittedStandardExtension {
        self.admitted
    }
    pub fn output(&self) -> &Output {
        &self.output
    }
    pub fn into_output(self) -> Output {
        self.output
    }
}
pub fn compile_standard_extension<'a, Compiler: CompileStandardExtension>(
    admitted: &'a AdmittedStandardExtension,
    compiler: &Compiler,
) -> Result<
    StandardExtensionCompilation<'a, Compiler::Output>,
    StandardExtensionCompileError<Compiler::Error>,
> {
    if admitted.artifact.schema() != compiler.schema() {
        return Err(StandardExtensionCompileError::SchemaMismatch {
            expected: compiler.schema().clone(),
            actual: admitted.artifact.schema().clone(),
        });
    }
    let output = compiler
        .compile(&admitted.artifact)
        .map_err(StandardExtensionCompileError::Product)?;
    Ok(StandardExtensionCompilation { admitted, output })
}

#[derive(Debug)]
pub enum StandardExtensionError {
    Package(RulePackageError),
    Identity(crate::RoleRequirementError),
    Json(serde_json::Error),
    ZeroSchemaVersion,
    InvalidNamespace { value: String },
    PayloadQuotaExceeded { actual: usize, maximum: usize },
    WrongFamily,
    Malformed(&'static str),
    UnknownField { field: String },
    MissingCorrelation,
    SourceMismatch,
    NonConvergentPayload,
}
impl fmt::Display for StandardExtensionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "standard extension artifact rejected: {self:?}")
    }
}
impl std::error::Error for StandardExtensionError {}
