use std::fmt;

use crate::{
    ContinuousInputReference, ContinuousValue, ContinuousValueError, StandardDefinitionIdentity,
    CONTINUOUS_EVALUATOR_SEMANTICS_VERSION,
};
use gameplay_mechanics::{
    CatalogVersion, ExactRatio, MechanicsScalar, SourceDefinitionId, MAX_ABS_MECHANICS_SCALAR,
};

pub const CONTINUOUS_QUANTIZATION_POLICY_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContinuousQuantizationMode {
    TowardZero,
    Floor,
    Ceil,
    NearestTiesToEven,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContinuousQuantizationSource {
    /// An explicit immutable evaluator input, never an ambient fact lookup.
    DirectInput { input: ContinuousInputReference },
    /// A value evaluated from a previously admitted standard definition.
    AdmittedDefinition {
        definition: StandardDefinitionIdentity,
        catalog: Option<ContinuousQuantizationCatalogProvenance>,
    },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContinuousQuantizationCatalogProvenance {
    catalog_version: CatalogVersion,
    catalog_fingerprint: String,
    source_identity: SourceDefinitionId,
}
impl ContinuousQuantizationCatalogProvenance {
    pub fn new(
        catalog_version: CatalogVersion,
        catalog_fingerprint: impl Into<String>,
        source_identity: SourceDefinitionId,
    ) -> Self {
        Self {
            catalog_version,
            catalog_fingerprint: catalog_fingerprint.into(),
            source_identity,
        }
    }
    pub fn catalog_version(&self) -> &CatalogVersion {
        &self.catalog_version
    }
    pub fn catalog_fingerprint(&self) -> &str {
        &self.catalog_fingerprint
    }
    pub const fn source_identity(&self) -> &SourceDefinitionId {
        &self.source_identity
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContinuousQuantizationReceipt {
    source: ContinuousQuantizationSource,
    source_bits: u64,
    evaluator_semantics_version: u32,
    mode: ContinuousQuantizationMode,
    policy_version: u32,
    minimum: i64,
    maximum: i64,
    rounded_candidate: Option<i64>,
    result: Option<MechanicsScalar>,
    remainder: Option<ContinuousValue>,
    catalog_version: Option<CatalogVersion>,
    catalog_fingerprint: Option<String>,
    source_identity: Option<SourceDefinitionId>,
}
#[derive(Debug, Clone, PartialEq)]
pub enum ContinuousQuantizationAttempt {
    Accepted(ContinuousQuantizationReceipt),
    Rejected {
        source: ContinuousQuantizationSource,
        source_bits: u64,
        evaluator_semantics_version: u32,
        mode: ContinuousQuantizationMode,
        policy_version: u32,
        minimum: i64,
        maximum: i64,
        error: ContinuousQuantizationError,
    },
}
impl ContinuousQuantizationReceipt {
    pub fn source_bits(&self) -> u64 {
        self.source_bits
    }
    pub fn mode(&self) -> ContinuousQuantizationMode {
        self.mode
    }
    pub fn result(&self) -> Option<MechanicsScalar> {
        self.result
    }
    pub fn remainder(&self) -> Option<ContinuousValue> {
        self.remainder
    }
    pub fn rounded_candidate(&self) -> Option<i64> {
        self.rounded_candidate
    }
    pub fn source(&self) -> &ContinuousQuantizationSource {
        &self.source
    }
    pub const fn evaluator_semantics_version(&self) -> u32 {
        self.evaluator_semantics_version
    }
    pub const fn policy_version(&self) -> u32 {
        self.policy_version
    }
    pub const fn minimum(&self) -> i64 {
        self.minimum
    }
    pub const fn maximum(&self) -> i64 {
        self.maximum
    }
    pub fn catalog_version(&self) -> Option<&CatalogVersion> {
        self.catalog_version.as_ref()
    }
    pub fn catalog_fingerprint(&self) -> Option<&str> {
        self.catalog_fingerprint.as_deref()
    }
    pub fn source_identity(&self) -> Option<&SourceDefinitionId> {
        self.source_identity.as_ref()
    }
}
#[derive(Debug, Clone, PartialEq)]
pub enum ContinuousQuantizationError {
    NonFiniteCandidate {
        source_bits: u64,
        mode: ContinuousQuantizationMode,
    },
    OutOfRange {
        source_bits: u64,
        mode: ContinuousQuantizationMode,
        rounded: f64,
        minimum: i64,
        maximum: i64,
    },
    Value(ContinuousValueError),
}
impl fmt::Display for ContinuousQuantizationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "continuous quantization rejected: {self:?}")
    }
}
impl std::error::Error for ContinuousQuantizationError {}
pub fn widen_mechanics_scalar_to_continuous(value: MechanicsScalar) -> ContinuousValue {
    ContinuousValue::new(value.get() as f64)
        .expect("current mechanics range is exact finite binary64")
}
pub fn approximate_exact_ratio_to_continuous(value: ExactRatio) -> ContinuousValue {
    ContinuousValue::new(f64::from(value.numerator()) / f64::from(value.denominator()))
        .expect("bounded exact ratios produce finite binary64")
}
pub fn quantize_continuous_to_mechanics(
    source: ContinuousValue,
    mode: ContinuousQuantizationMode,
    origin: ContinuousQuantizationSource,
) -> Result<ContinuousQuantizationReceipt, ContinuousQuantizationError> {
    match attempt_quantize_continuous_to_mechanics(source, mode, origin) {
        ContinuousQuantizationAttempt::Accepted(receipt) => Ok(receipt),
        ContinuousQuantizationAttempt::Rejected { error, .. } => Err(error),
    }
}
pub fn attempt_quantize_continuous_to_mechanics(
    source: ContinuousValue,
    mode: ContinuousQuantizationMode,
    origin: ContinuousQuantizationSource,
) -> ContinuousQuantizationAttempt {
    let (catalog_version, catalog_fingerprint, source_identity) = provenance_fields(&origin);
    let raw = source.raw();
    let rounded = match mode {
        ContinuousQuantizationMode::TowardZero => raw.trunc(),
        ContinuousQuantizationMode::Floor => raw.floor(),
        ContinuousQuantizationMode::Ceil => raw.ceil(),
        ContinuousQuantizationMode::NearestTiesToEven => raw.round_ties_even(),
    };
    if !rounded.is_finite() {
        return ContinuousQuantizationAttempt::Rejected {
            source: origin,
            source_bits: source.bits(),
            evaluator_semantics_version: CONTINUOUS_EVALUATOR_SEMANTICS_VERSION,
            mode,
            policy_version: CONTINUOUS_QUANTIZATION_POLICY_VERSION,
            minimum: -MAX_ABS_MECHANICS_SCALAR,
            maximum: MAX_ABS_MECHANICS_SCALAR,
            error: ContinuousQuantizationError::NonFiniteCandidate {
                source_bits: source.bits(),
                mode,
            },
        };
    }
    let minimum = -MAX_ABS_MECHANICS_SCALAR;
    let maximum = MAX_ABS_MECHANICS_SCALAR;
    if rounded < minimum as f64 || rounded > maximum as f64 {
        return ContinuousQuantizationAttempt::Rejected {
            source: origin,
            source_bits: source.bits(),
            evaluator_semantics_version: CONTINUOUS_EVALUATOR_SEMANTICS_VERSION,
            mode,
            policy_version: CONTINUOUS_QUANTIZATION_POLICY_VERSION,
            minimum,
            maximum,
            error: ContinuousQuantizationError::OutOfRange {
                source_bits: source.bits(),
                mode,
                rounded,
                minimum,
                maximum,
            },
        };
    }
    let candidate = rounded as i64;
    let result = MechanicsScalar::new(candidate).expect("checked mechanics range");
    let remainder = match source.checked_sub(widen_mechanics_scalar_to_continuous(result)) {
        Ok(value) => value,
        Err(error) => {
            return ContinuousQuantizationAttempt::Rejected {
                source: origin,
                source_bits: source.bits(),
                evaluator_semantics_version: CONTINUOUS_EVALUATOR_SEMANTICS_VERSION,
                mode,
                policy_version: CONTINUOUS_QUANTIZATION_POLICY_VERSION,
                minimum,
                maximum,
                error: ContinuousQuantizationError::Value(error),
            }
        }
    };
    ContinuousQuantizationAttempt::Accepted(ContinuousQuantizationReceipt {
        source: origin,
        source_bits: source.bits(),
        evaluator_semantics_version: CONTINUOUS_EVALUATOR_SEMANTICS_VERSION,
        mode,
        policy_version: CONTINUOUS_QUANTIZATION_POLICY_VERSION,
        minimum,
        maximum,
        rounded_candidate: Some(candidate),
        result: Some(result),
        remainder: Some(remainder),
        catalog_version,
        catalog_fingerprint,
        source_identity,
    })
}
fn provenance_fields(
    origin: &ContinuousQuantizationSource,
) -> (
    Option<CatalogVersion>,
    Option<String>,
    Option<SourceDefinitionId>,
) {
    match origin {
        ContinuousQuantizationSource::DirectInput { .. } => (None, None, None),
        ContinuousQuantizationSource::AdmittedDefinition { catalog, .. } => catalog
            .as_ref()
            .map(|catalog| {
                (
                    Some(catalog.catalog_version.clone()),
                    Some(catalog.catalog_fingerprint.clone()),
                    Some(catalog.source_identity.clone()),
                )
            })
            .unwrap_or((None, None, None)),
    }
}
