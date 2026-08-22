//! Host-neutral evidence for continuous values crossing one named exact boundary.
//!
//! Callers declare cadence and interval, retain carries, and choose persistence.
//! This module owns no clock, loop, component, resource, cap policy, or save format.

use std::fmt;

use gameplay_mechanics::MechanicsScalar;

use crate::{
    attempt_quantize_continuous_to_mechanics, ContinuousQuantizationAttempt,
    ContinuousQuantizationError, ContinuousQuantizationMode, ContinuousQuantizationReceipt,
    ContinuousQuantizationSource, ContinuousValue, ContinuousValueError,
    CONTINUOUS_EVALUATOR_SEMANTICS_VERSION, CONTINUOUS_QUANTIZATION_POLICY_VERSION,
};

pub const MAX_CADENCE_IDENTITY_BYTES: usize = 96;
pub const RESIDUAL_CARRY_SCHEMA_VERSION: u32 = 1;

/// A stable caller-supplied partition label; it is evidence, never an Engine clock.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CadenceIdentity(String);

impl CadenceIdentity {
    pub fn parse(value: impl Into<String>) -> Result<Self, ContinuousCadenceError> {
        let value = value.into();
        if value.is_empty() || value.len() > MAX_CADENCE_IDENTITY_BYTES {
            return Err(ContinuousCadenceError::InvalidCadenceIdentity {
                byte_length: value.len(),
                reason: "identity must contain 1 to 96 ASCII bytes",
            });
        }
        let mut bytes = value.bytes();
        if !bytes.next().is_some_and(|byte| byte.is_ascii_lowercase()) {
            return Err(ContinuousCadenceError::InvalidCadenceIdentity {
                byte_length: value.len(),
                reason: "identity must start with a lowercase ASCII letter",
            });
        }
        if !value
            .bytes()
            .last()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        {
            return Err(ContinuousCadenceError::InvalidCadenceIdentity {
                byte_length: value.len(),
                reason: "identity must end with a lowercase ASCII letter or digit",
            });
        }
        if !bytes.all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-' | b'_')
        }) {
            return Err(ContinuousCadenceError::InvalidCadenceIdentity {
                byte_length: value.len(),
                reason: "identity contains unsupported characters",
            });
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A canonical positive rational fraction of an unnamed caller-defined span.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CadenceInterval {
    numerator: u32,
    denominator: u32,
}

impl CadenceInterval {
    pub fn new(numerator: u32, denominator: u32) -> Result<Self, ContinuousCadenceError> {
        if numerator == 0 || denominator == 0 || numerator > denominator {
            return Err(ContinuousCadenceError::InvalidInterval {
                numerator,
                denominator,
            });
        }
        let divisor = greatest_common_divisor(numerator, denominator);
        Ok(Self {
            numerator: numerator / divisor,
            denominator: denominator / divisor,
        })
    }

    pub const fn numerator(self) -> u32 {
        self.numerator
    }

    pub const fn denominator(self) -> u32 {
        self.denominator
    }

    pub fn as_continuous(self) -> ContinuousValue {
        ContinuousValue::new(f64::from(self.numerator) / f64::from(self.denominator))
            .expect("positive u32 rational interval is always finite binary64")
    }

    pub fn bits(self) -> u64 {
        self.as_continuous().bits()
    }
}

/// The exact policy version a caller must retain beside its residual carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CadenceQuantizationPolicy {
    mode: ContinuousQuantizationMode,
    version: u32,
}

impl CadenceQuantizationPolicy {
    pub const fn v1(mode: ContinuousQuantizationMode) -> Self {
        Self {
            mode,
            version: CONTINUOUS_QUANTIZATION_POLICY_VERSION,
        }
    }

    pub fn new(
        mode: ContinuousQuantizationMode,
        version: u32,
    ) -> Result<Self, ContinuousCadenceError> {
        if version != CONTINUOUS_QUANTIZATION_POLICY_VERSION {
            return Err(ContinuousCadenceError::StalePolicyVersion {
                expected: CONTINUOUS_QUANTIZATION_POLICY_VERSION,
                actual: version,
            });
        }
        Ok(Self { mode, version })
    }

    pub const fn mode(self) -> ContinuousQuantizationMode {
        self.mode
    }

    pub const fn version(self) -> u32 {
        self.version
    }
}

/// An explicit caller-authorized change between two declared partitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CadenceTransition {
    previous_interval: CadenceInterval,
    previous_cadence: CadenceIdentity,
    next_interval: CadenceInterval,
    next_cadence: CadenceIdentity,
}

impl CadenceTransition {
    pub fn new(
        previous_interval: CadenceInterval,
        previous_cadence: CadenceIdentity,
        next_interval: CadenceInterval,
        next_cadence: CadenceIdentity,
    ) -> Result<Self, ContinuousCadenceError> {
        if previous_interval == next_interval && previous_cadence == next_cadence {
            return Err(ContinuousCadenceError::RedundantCadenceTransition);
        }
        Ok(Self {
            previous_interval,
            previous_cadence,
            next_interval,
            next_cadence,
        })
    }

    pub const fn previous_interval(&self) -> CadenceInterval {
        self.previous_interval
    }

    pub fn previous_cadence(&self) -> &CadenceIdentity {
        &self.previous_cadence
    }

    pub const fn next_interval(&self) -> CadenceInterval {
        self.next_interval
    }

    pub fn next_cadence(&self) -> &CadenceIdentity {
        &self.next_cadence
    }
}

/// Caller-owned fractional amount and the complete contract that produced it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResidualCarry {
    value: ContinuousValue,
    schema_version: u32,
    evaluator_semantics_version: u32,
    mode: ContinuousQuantizationMode,
    policy_version: u32,
    source: ContinuousQuantizationSource,
    interval: CadenceInterval,
    cadence: CadenceIdentity,
}

impl ResidualCarry {
    pub const fn value(&self) -> ContinuousValue {
        self.value
    }

    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub const fn evaluator_semantics_version(&self) -> u32 {
        self.evaluator_semantics_version
    }

    pub const fn mode(&self) -> ContinuousQuantizationMode {
        self.mode
    }

    pub const fn policy_version(&self) -> u32 {
        self.policy_version
    }

    pub fn source(&self) -> &ContinuousQuantizationSource {
        &self.source
    }

    pub const fn interval(&self) -> CadenceInterval {
        self.interval
    }

    pub fn cadence(&self) -> &CadenceIdentity {
        &self.cadence
    }

    pub fn snapshot(&self) -> ResidualCarrySnapshot {
        ResidualCarrySnapshot {
            value_bits: self.value.bits(),
            schema_version: self.schema_version,
            evaluator_semantics_version: self.evaluator_semantics_version,
            mode: self.mode,
            policy_version: self.policy_version,
            source: self.source.clone(),
            interval: self.interval,
            cadence: self.cadence.clone(),
        }
    }
}

/// An untrusted caller persistence record for `ResidualCarry`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResidualCarrySnapshot {
    value_bits: u64,
    schema_version: u32,
    evaluator_semantics_version: u32,
    mode: ContinuousQuantizationMode,
    policy_version: u32,
    source: ContinuousQuantizationSource,
    interval: CadenceInterval,
    cadence: CadenceIdentity,
}

impl ResidualCarrySnapshot {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        value_bits: u64,
        schema_version: u32,
        evaluator_semantics_version: u32,
        mode: ContinuousQuantizationMode,
        policy_version: u32,
        source: ContinuousQuantizationSource,
        interval: CadenceInterval,
        cadence: CadenceIdentity,
    ) -> Self {
        Self {
            value_bits,
            schema_version,
            evaluator_semantics_version,
            mode,
            policy_version,
            source,
            interval,
            cadence,
        }
    }

    pub const fn value_bits(&self) -> u64 {
        self.value_bits
    }

    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub const fn evaluator_semantics_version(&self) -> u32 {
        self.evaluator_semantics_version
    }

    pub const fn mode(&self) -> ContinuousQuantizationMode {
        self.mode
    }

    pub const fn policy_version(&self) -> u32 {
        self.policy_version
    }

    pub fn source(&self) -> &ContinuousQuantizationSource {
        &self.source
    }

    pub const fn interval(&self) -> CadenceInterval {
        self.interval
    }

    pub fn cadence(&self) -> &CadenceIdentity {
        &self.cadence
    }

    pub fn reopen(
        self,
        policy: CadenceQuantizationPolicy,
    ) -> Result<ResidualCarry, ContinuousCadenceError> {
        if self.schema_version != RESIDUAL_CARRY_SCHEMA_VERSION {
            return Err(ContinuousCadenceError::StaleResidualSchema {
                expected: RESIDUAL_CARRY_SCHEMA_VERSION,
                actual: self.schema_version,
            });
        }
        if self.evaluator_semantics_version != CONTINUOUS_EVALUATOR_SEMANTICS_VERSION {
            return Err(ContinuousCadenceError::StaleResidualEvaluator {
                expected: CONTINUOUS_EVALUATOR_SEMANTICS_VERSION,
                actual: self.evaluator_semantics_version,
            });
        }
        if self.policy_version != policy.version {
            return Err(ContinuousCadenceError::StaleResidualPolicy {
                expected: policy.version,
                actual: self.policy_version,
            });
        }
        if self.mode != policy.mode {
            return Err(ContinuousCadenceError::StaleResidualMode {
                expected: policy.mode,
                actual: self.mode,
            });
        }
        let value = ContinuousValue::from_bits(self.value_bits)
            .map_err(ContinuousCadenceError::InvalidResidual)?;
        if !remainder_matches_mode(value, self.mode) {
            return Err(ContinuousCadenceError::InvalidResidualRemainder {
                bits: self.value_bits,
                mode: self.mode,
            });
        }
        Ok(ResidualCarry {
            value,
            schema_version: self.schema_version,
            evaluator_semantics_version: self.evaluator_semantics_version,
            mode: self.mode,
            policy_version: self.policy_version,
            source: self.source,
            interval: self.interval,
            cadence: self.cadence,
        })
    }
}

/// The full named-boundary evidence for one caller-declared cadence step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CadencedQuantizationReceipt {
    quantization: ContinuousQuantizationReceipt,
    interval: CadenceInterval,
    interval_bits: u64,
    cadence: CadenceIdentity,
    transition: Option<CadenceTransition>,
}

impl CadencedQuantizationReceipt {
    pub fn source_bits(&self) -> u64 {
        self.quantization.source_bits()
    }
    pub fn mode(&self) -> ContinuousQuantizationMode {
        self.quantization.mode()
    }
    pub const fn policy_version(&self) -> u32 {
        self.quantization.policy_version()
    }
    pub fn result(&self) -> MechanicsScalar {
        self.quantization
            .result()
            .expect("accepted quantization has a result")
    }
    pub fn remainder(&self) -> ContinuousValue {
        self.quantization
            .remainder()
            .expect("accepted quantization has a remainder")
    }
    pub const fn interval(&self) -> CadenceInterval {
        self.interval
    }
    pub const fn interval_bits(&self) -> u64 {
        self.interval_bits
    }
    pub fn cadence(&self) -> &CadenceIdentity {
        &self.cadence
    }
    pub fn transition(&self) -> Option<&CadenceTransition> {
        self.transition.as_ref()
    }
    pub fn quantization(&self) -> &ContinuousQuantizationReceipt {
        &self.quantization
    }
}

/// A pure result; the caller retains the carry and applies the exact delta itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CadencedQuantizationStep {
    receipt: CadencedQuantizationReceipt,
    next_residual: ResidualCarry,
}

/// Rejected named-boundary evidence retains the same caller-declared context as an acceptance.
#[derive(Debug, Clone, PartialEq)]
pub struct CadencedQuantizationRejection {
    source_bits: u64,
    source: ContinuousQuantizationSource,
    mode: ContinuousQuantizationMode,
    policy_version: u32,
    interval: CadenceInterval,
    interval_bits: u64,
    cadence: CadenceIdentity,
    transition: Option<CadenceTransition>,
    error: ContinuousQuantizationError,
}

impl CadencedQuantizationRejection {
    pub const fn source_bits(&self) -> u64 {
        self.source_bits
    }
    pub fn source(&self) -> &ContinuousQuantizationSource {
        &self.source
    }
    pub const fn mode(&self) -> ContinuousQuantizationMode {
        self.mode
    }
    pub const fn policy_version(&self) -> u32 {
        self.policy_version
    }
    pub const fn interval(&self) -> CadenceInterval {
        self.interval
    }
    pub const fn interval_bits(&self) -> u64 {
        self.interval_bits
    }
    pub fn cadence(&self) -> &CadenceIdentity {
        &self.cadence
    }
    pub fn transition(&self) -> Option<&CadenceTransition> {
        self.transition.as_ref()
    }
    pub fn error(&self) -> &ContinuousQuantizationError {
        &self.error
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CadencedQuantizationAttempt {
    Accepted(Box<CadencedQuantizationStep>),
    Rejected(Box<CadencedQuantizationRejection>),
}

impl CadencedQuantizationStep {
    pub fn receipt(&self) -> &CadencedQuantizationReceipt {
        &self.receipt
    }
    pub fn next_residual(&self) -> &ResidualCarry {
        &self.next_residual
    }
}

/// Multiplies a rate by a declared interval and adds only a compatible caller-held carry.
#[allow(clippy::too_many_arguments)]
pub fn quantize_rate_with_caller_residual(
    rate: ContinuousValue,
    interval: CadenceInterval,
    cadence: CadenceIdentity,
    policy: CadenceQuantizationPolicy,
    residual: Option<ResidualCarry>,
    transition: Option<CadenceTransition>,
    origin: ContinuousQuantizationSource,
) -> Result<CadencedQuantizationStep, ContinuousCadenceError> {
    match attempt_quantize_rate_with_caller_residual(
        rate, interval, cadence, policy, residual, transition, origin,
    )? {
        CadencedQuantizationAttempt::Accepted(step) => Ok(*step),
        CadencedQuantizationAttempt::Rejected(rejection) => {
            Err(ContinuousCadenceError::Quantization(rejection.error))
        }
    }
}

/// Attempts named cadence quantization while retaining full evidence on conversion rejection.
#[allow(clippy::too_many_arguments)]
pub fn attempt_quantize_rate_with_caller_residual(
    rate: ContinuousValue,
    interval: CadenceInterval,
    cadence: CadenceIdentity,
    policy: CadenceQuantizationPolicy,
    residual: Option<ResidualCarry>,
    transition: Option<CadenceTransition>,
    origin: ContinuousQuantizationSource,
) -> Result<CadencedQuantizationAttempt, ContinuousCadenceError> {
    let source = match residual {
        Some(residual) => {
            ensure_residual_current(
                &residual,
                policy,
                &origin,
                interval,
                &cadence,
                transition.as_ref(),
            )?;
            rate.checked_mul(interval.as_continuous())
                .and_then(|step| step.checked_add(residual.value))
                .map_err(ContinuousCadenceError::NonFiniteStep)?
        }
        None => {
            if transition.is_some() {
                return Err(ContinuousCadenceError::UnexpectedCadenceTransition);
            }
            rate.checked_mul(interval.as_continuous())
                .map_err(ContinuousCadenceError::NonFiniteStep)?
        }
    };
    match attempt_quantize_continuous_to_mechanics(source, policy.mode, origin.clone()) {
        ContinuousQuantizationAttempt::Accepted(quantization) => {
            let remainder = quantization
                .remainder()
                .expect("accepted quantization has a remainder");
            Ok(CadencedQuantizationAttempt::Accepted(Box::new(
                CadencedQuantizationStep {
                    receipt: CadencedQuantizationReceipt {
                        quantization,
                        interval,
                        interval_bits: interval.bits(),
                        cadence: cadence.clone(),
                        transition,
                    },
                    next_residual: ResidualCarry {
                        value: remainder,
                        schema_version: RESIDUAL_CARRY_SCHEMA_VERSION,
                        evaluator_semantics_version: CONTINUOUS_EVALUATOR_SEMANTICS_VERSION,
                        mode: policy.mode,
                        policy_version: policy.version,
                        source: origin,
                        interval,
                        cadence,
                    },
                },
            )))
        }
        ContinuousQuantizationAttempt::Rejected {
            source_bits, error, ..
        } => Ok(CadencedQuantizationAttempt::Rejected(Box::new(
            CadencedQuantizationRejection {
                source_bits,
                source: origin,
                mode: policy.mode,
                policy_version: policy.version,
                interval,
                interval_bits: interval.bits(),
                cadence,
                transition,
                error,
            },
        ))),
    }
}

/// Pure direct binary64 accumulation for comparison; it never quantizes or carries a residual.
pub fn accumulate_rate_binary64(
    current: ContinuousValue,
    rate: ContinuousValue,
    interval: CadenceInterval,
) -> Result<ContinuousValue, ContinuousCadenceError> {
    current
        .checked_add(
            rate.checked_mul(interval.as_continuous())
                .map_err(ContinuousCadenceError::NonFiniteStep)?,
        )
        .map_err(ContinuousCadenceError::NonFiniteStep)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Binary64AccumulatorSnapshot {
    value_bits: u64,
}

impl Binary64AccumulatorSnapshot {
    pub const fn new(value_bits: u64) -> Self {
        Self { value_bits }
    }
    pub const fn value_bits(self) -> u64 {
        self.value_bits
    }
    pub fn reopen(self) -> Result<ContinuousValue, ContinuousCadenceError> {
        ContinuousValue::from_bits(self.value_bits)
            .map_err(ContinuousCadenceError::InvalidAccumulator)
    }
}

/// Exact-deadline comparison state for an already exact whole mechanics-unit rate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExactDeadlineAccumulator {
    rate_per_span: MechanicsScalar,
    interval: CadenceInterval,
    elapsed_steps: u64,
    emitted: i64,
}

impl ExactDeadlineAccumulator {
    pub const fn new(rate_per_span: MechanicsScalar, interval: CadenceInterval) -> Self {
        Self {
            rate_per_span,
            interval,
            elapsed_steps: 0,
            emitted: 0,
        }
    }
    pub const fn emitted(self) -> i64 {
        self.emitted
    }
    pub const fn elapsed_steps(self) -> u64 {
        self.elapsed_steps
    }
    pub fn snapshot(&self) -> ExactDeadlineAccumulatorSnapshot {
        ExactDeadlineAccumulatorSnapshot {
            rate_per_span: self.rate_per_span.get(),
            interval: self.interval,
            elapsed_steps: self.elapsed_steps,
            emitted: self.emitted,
        }
    }
    pub fn advance(&mut self) -> Result<MechanicsScalar, ContinuousCadenceError> {
        let next_steps = self
            .elapsed_steps
            .checked_add(1)
            .ok_or(ContinuousCadenceError::DeadlineOverflow)?;
        let total = exact_deadline_total(self.rate_per_span, self.interval, next_steps)?;
        let delta = total
            .checked_sub(self.emitted)
            .ok_or(ContinuousCadenceError::DeadlineOverflow)?;
        let delta = MechanicsScalar::new(delta).map_err(ContinuousCadenceError::MechanicsScalar)?;
        self.elapsed_steps = next_steps;
        self.emitted = total;
        Ok(delta)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExactDeadlineAccumulatorSnapshot {
    rate_per_span: i64,
    interval: CadenceInterval,
    elapsed_steps: u64,
    emitted: i64,
}

impl ExactDeadlineAccumulatorSnapshot {
    pub const fn new(
        rate_per_span: i64,
        interval: CadenceInterval,
        elapsed_steps: u64,
        emitted: i64,
    ) -> Self {
        Self {
            rate_per_span,
            interval,
            elapsed_steps,
            emitted,
        }
    }

    pub const fn rate_per_span(&self) -> i64 {
        self.rate_per_span
    }

    pub const fn interval(&self) -> CadenceInterval {
        self.interval
    }

    pub const fn elapsed_steps(&self) -> u64 {
        self.elapsed_steps
    }

    pub const fn emitted(&self) -> i64 {
        self.emitted
    }

    pub fn reopen(self) -> Result<ExactDeadlineAccumulator, ContinuousCadenceError> {
        let rate = MechanicsScalar::new(self.rate_per_span)
            .map_err(ContinuousCadenceError::MechanicsScalar)?;
        let expected = exact_deadline_total(rate, self.interval, self.elapsed_steps)?;
        if expected != self.emitted {
            return Err(ContinuousCadenceError::InvalidDeadlineSnapshot {
                expected,
                actual: self.emitted,
            });
        }
        Ok(ExactDeadlineAccumulator {
            rate_per_span: rate,
            interval: self.interval,
            elapsed_steps: self.elapsed_steps,
            emitted: self.emitted,
        })
    }
}

fn exact_deadline_total(
    rate_per_span: MechanicsScalar,
    interval: CadenceInterval,
    elapsed_steps: u64,
) -> Result<i64, ContinuousCadenceError> {
    let numerator = i128::from(rate_per_span.get())
        .checked_mul(i128::from(interval.numerator))
        .and_then(|value| value.checked_mul(i128::from(elapsed_steps)))
        .ok_or(ContinuousCadenceError::DeadlineOverflow)?;
    i64::try_from(numerator / i128::from(interval.denominator))
        .map_err(|_| ContinuousCadenceError::DeadlineOverflow)
}

fn ensure_residual_current(
    residual: &ResidualCarry,
    policy: CadenceQuantizationPolicy,
    source: &ContinuousQuantizationSource,
    interval: CadenceInterval,
    cadence: &CadenceIdentity,
    transition: Option<&CadenceTransition>,
) -> Result<(), ContinuousCadenceError> {
    if residual.schema_version != RESIDUAL_CARRY_SCHEMA_VERSION {
        return Err(ContinuousCadenceError::StaleResidualSchema {
            expected: RESIDUAL_CARRY_SCHEMA_VERSION,
            actual: residual.schema_version,
        });
    }
    if residual.evaluator_semantics_version != CONTINUOUS_EVALUATOR_SEMANTICS_VERSION {
        return Err(ContinuousCadenceError::StaleResidualEvaluator {
            expected: CONTINUOUS_EVALUATOR_SEMANTICS_VERSION,
            actual: residual.evaluator_semantics_version,
        });
    }
    if residual.policy_version != policy.version {
        return Err(ContinuousCadenceError::StaleResidualPolicy {
            expected: policy.version,
            actual: residual.policy_version,
        });
    }
    if residual.mode != policy.mode {
        return Err(ContinuousCadenceError::StaleResidualMode {
            expected: policy.mode,
            actual: residual.mode,
        });
    }
    if residual.source != *source {
        return Err(ContinuousCadenceError::StaleResidualSource);
    }
    let changed = residual.interval != interval || residual.cadence != *cadence;
    match (changed, transition) {
        (false, None) => Ok(()),
        (false, Some(_)) => Err(ContinuousCadenceError::RedundantCadenceTransition),
        (true, None) => Err(ContinuousCadenceError::HiddenCadenceTransition),
        (true, Some(transition))
            if transition.previous_interval == residual.interval
                && transition.previous_cadence == residual.cadence
                && transition.next_interval == interval
                && transition.next_cadence == *cadence =>
        {
            Ok(())
        }
        (true, Some(_)) => Err(ContinuousCadenceError::MismatchedCadenceTransition),
    }
}

fn remainder_matches_mode(value: ContinuousValue, mode: ContinuousQuantizationMode) -> bool {
    let value = value.get();
    match mode {
        ContinuousQuantizationMode::TowardZero => (-1.0..1.0).contains(&value),
        ContinuousQuantizationMode::Floor => (0.0..1.0).contains(&value),
        ContinuousQuantizationMode::Ceil => (-1.0..=0.0).contains(&value),
        ContinuousQuantizationMode::NearestTiesToEven => (-0.5..=0.5).contains(&value),
    }
}

fn greatest_common_divisor(mut left: u32, mut right: u32) -> u32 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

#[derive(Debug, Clone, PartialEq)]
pub enum ContinuousCadenceError {
    InvalidCadenceIdentity {
        byte_length: usize,
        reason: &'static str,
    },
    InvalidInterval {
        numerator: u32,
        denominator: u32,
    },
    NonFiniteStep(ContinuousValueError),
    Quantization(ContinuousQuantizationError),
    StalePolicyVersion {
        expected: u32,
        actual: u32,
    },
    StaleResidualSchema {
        expected: u32,
        actual: u32,
    },
    StaleResidualEvaluator {
        expected: u32,
        actual: u32,
    },
    StaleResidualPolicy {
        expected: u32,
        actual: u32,
    },
    StaleResidualMode {
        expected: ContinuousQuantizationMode,
        actual: ContinuousQuantizationMode,
    },
    StaleResidualSource,
    HiddenCadenceTransition,
    MismatchedCadenceTransition,
    RedundantCadenceTransition,
    UnexpectedCadenceTransition,
    InvalidResidual(ContinuousValueError),
    InvalidResidualRemainder {
        bits: u64,
        mode: ContinuousQuantizationMode,
    },
    InvalidAccumulator(ContinuousValueError),
    DeadlineOverflow,
    InvalidDeadlineSnapshot {
        expected: i64,
        actual: i64,
    },
    MechanicsScalar(gameplay_mechanics::MechanicsArithmeticError),
}

impl fmt::Display for ContinuousCadenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "continuous cadence experiment rejected: {self:?}"
        )
    }
}
impl std::error::Error for ContinuousCadenceError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        quantize_continuous_to_mechanics, CapabilityRoleId, ContinuousEvaluator, ContinuousExpr,
        ContinuousExprLimits, ContinuousInputBundle, ContinuousInputReference, InputId,
    };
    use gameplay_mechanics::MAX_ABS_MECHANICS_SCALAR;

    fn value(value: f64) -> ContinuousValue {
        ContinuousValue::new(value).unwrap()
    }
    fn interval(hz: u32) -> CadenceInterval {
        CadenceInterval::new(1, hz).unwrap()
    }
    fn cadence(hz: u32) -> CadenceIdentity {
        CadenceIdentity::parse(format!("hz-{hz}")).unwrap()
    }
    fn origin() -> ContinuousQuantizationSource {
        ContinuousQuantizationSource::DirectInput {
            input: ContinuousInputReference::Parameter {
                role: CapabilityRoleId::parse("cadence").unwrap(),
                id: InputId::parse("rate").unwrap(),
            },
        }
    }
    fn run_binary(rate: ContinuousValue, hz: u32, spans: u32) -> ContinuousValue {
        let mut current = value(0.0);
        for _ in 0..hz * spans {
            current = accumulate_rate_binary64(current, rate, interval(hz)).unwrap();
        }
        current
    }
    fn run_residual(rate: ContinuousValue, hz: u32, spans: u32) -> (i64, ResidualCarry) {
        let policy = CadenceQuantizationPolicy::v1(ContinuousQuantizationMode::TowardZero);
        let mut total = 0;
        let mut carry = None;
        for _ in 0..hz * spans {
            let step = quantize_rate_with_caller_residual(
                rate,
                interval(hz),
                cadence(hz),
                policy,
                carry,
                None,
                origin(),
            )
            .unwrap();
            total += step.receipt().result().get();
            carry = Some(step.next_residual().clone());
        }
        (total, carry.unwrap())
    }
    fn run_deadline(rate: MechanicsScalar, hz: u32, spans: u32) -> i64 {
        let mut accumulator = ExactDeadlineAccumulator::new(rate, interval(hz));
        (0..hz * spans)
            .map(|_| accumulator.advance().unwrap().get())
            .sum()
    }
    fn authored_rate() -> ContinuousValue {
        let input = ContinuousInputReference::Parameter {
            role: CapabilityRoleId::parse("cadence").unwrap(),
            id: InputId::parse("rate").unwrap(),
        };
        ContinuousEvaluator::evaluate(
            &ContinuousExpr::Input(input.clone()),
            &ContinuousInputBundle::new(vec![(input, value(7.0))]),
            ContinuousExprLimits::default(),
        )
        .unwrap()
    }

    fn exact_rate_from_authored(rate: ContinuousValue) -> MechanicsScalar {
        quantize_continuous_to_mechanics(
            rate,
            ContinuousQuantizationMode::NearestTiesToEven,
            origin(),
        )
        .unwrap()
        .result()
        .expect("accepted exact authored rate has a mechanics result")
    }

    #[test]
    fn concrete_short_and_long_partition_matrix() {
        for (spans, direct, residual) in [
            (
                1,
                [
                    0x401c000000000000,
                    0x401c000000000004,
                    0x401bfffffffffff5,
                    0x401c000000000006,
                ],
                [7, 6, 7, 7],
            ),
            (
                10_000,
                [
                    0x40f1170000000000,
                    0x40f116ffffff8527,
                    0x40f11700000069fa,
                    0x40f117000000d13d,
                ],
                [70_000, 69_999, 70_000, 70_000],
            ),
        ] {
            let authored_rate = authored_rate();
            let exact_rate = exact_rate_from_authored(authored_rate);
            let negative_authored_rate = authored_rate.checked_mul(value(-1.0)).unwrap();
            let negative_exact_rate = exact_rate_from_authored(negative_authored_rate);
            let direct_actual =
                [1, 35, 60, 120].map(|hz| run_binary(authored_rate, hz, spans).bits());
            let residual_actual =
                [1, 35, 60, 120].map(|hz| run_residual(authored_rate, hz, spans).0);
            let negative_residual_actual =
                [1, 35, 60, 120].map(|hz| run_residual(negative_authored_rate, hz, spans).0);
            assert_eq!(direct_actual, direct);
            assert_eq!(residual_actual, residual);
            assert_eq!(negative_residual_actual, residual.map(|total| -total));
            for hz in [1, 35, 60, 120] {
                assert_eq!(run_deadline(exact_rate, hz, spans), i64::from(7 * spans));
                assert_eq!(
                    run_deadline(negative_exact_rate, hz, spans),
                    -i64::from(7 * spans)
                );
            }
        }
    }

    #[test]
    fn snapshots_continue_all_three_paths_and_receipts_record_context() {
        let rate = authored_rate();
        let policy = CadenceQuantizationPolicy::v1(ContinuousQuantizationMode::TowardZero);
        let mut direct = value(0.0);
        let mut carry = None;
        let mut exact = ExactDeadlineAccumulator::new(exact_rate_from_authored(rate), interval(60));
        let mut residual_total = 0;
        let mut deadline_total = 0;
        for index in 0..120 {
            direct = accumulate_rate_binary64(direct, rate, interval(60)).unwrap();
            let step = quantize_rate_with_caller_residual(
                rate,
                interval(60),
                cadence(60),
                policy,
                carry,
                None,
                origin(),
            )
            .unwrap();
            assert_eq!(step.receipt().interval_bits(), interval(60).bits());
            if index == 0 {
                assert_eq!(step.receipt().source_bits(), 0x3fbdddddddddddde);
                assert_eq!(step.receipt().result().get(), 0);
                assert_eq!(step.receipt().remainder().bits(), 0x3fbdddddddddddde);
            }
            residual_total += step.receipt().result().get();
            carry = Some(step.next_residual().clone());
            deadline_total += exact.advance().unwrap().get();
            if index == 59 {
                direct = Binary64AccumulatorSnapshot::new(direct.bits())
                    .reopen()
                    .unwrap();
                let residual_snapshot = carry.unwrap().snapshot();
                carry = Some(
                    ResidualCarrySnapshot::new(
                        residual_snapshot.value_bits(),
                        residual_snapshot.schema_version(),
                        residual_snapshot.evaluator_semantics_version(),
                        residual_snapshot.mode(),
                        residual_snapshot.policy_version(),
                        residual_snapshot.source().clone(),
                        residual_snapshot.interval(),
                        residual_snapshot.cadence().clone(),
                    )
                    .reopen(policy)
                    .unwrap(),
                );
                let deadline_snapshot = exact.snapshot();
                exact = ExactDeadlineAccumulatorSnapshot::new(
                    deadline_snapshot.rate_per_span(),
                    deadline_snapshot.interval(),
                    deadline_snapshot.elapsed_steps(),
                    deadline_snapshot.emitted(),
                )
                .reopen()
                .unwrap();
            }
        }
        assert_eq!(direct, run_binary(rate, 60, 2));
        assert_eq!(residual_total, run_residual(rate, 60, 2).0);
        assert_eq!(
            deadline_total,
            run_deadline(exact_rate_from_authored(rate), 60, 2)
        );
    }

    #[test]
    fn carry_requires_explicit_cadence_transition_and_exact_source_identity() {
        let policy = CadenceQuantizationPolicy::v1(ContinuousQuantizationMode::TowardZero);
        let first = quantize_rate_with_caller_residual(
            value(7.0),
            interval(60),
            cadence(60),
            policy,
            None,
            None,
            origin(),
        )
        .unwrap();
        let carry = first.next_residual().clone();
        assert!(matches!(
            quantize_rate_with_caller_residual(
                value(7.0),
                interval(35),
                cadence(35),
                policy,
                Some(carry.clone()),
                None,
                origin()
            ),
            Err(ContinuousCadenceError::HiddenCadenceTransition)
        ));
        let transition =
            CadenceTransition::new(interval(60), cadence(60), interval(35), cadence(35)).unwrap();
        let transitioned = quantize_rate_with_caller_residual(
            value(7.0),
            interval(35),
            cadence(35),
            policy,
            Some(carry),
            Some(transition),
            origin(),
        )
        .unwrap();
        assert!(transitioned.receipt().transition().is_some());
        let fact_source = ContinuousQuantizationSource::DirectInput {
            input: ContinuousInputReference::Fact {
                role: CapabilityRoleId::parse("cadence").unwrap(),
                id: InputId::parse("rate").unwrap(),
            },
        };
        assert!(matches!(
            quantize_rate_with_caller_residual(
                value(7.0),
                interval(35),
                cadence(35),
                policy,
                Some(transitioned.next_residual().clone()),
                None,
                fact_source
            ),
            Err(ContinuousCadenceError::StaleResidualSource)
        ));
    }

    #[test]
    fn failures_cover_invalid_persistence_bounds_and_deadline_overflow() {
        assert!(CadenceIdentity::parse("bad identity").is_err());
        assert!(CadenceIdentity::parse("é").is_err());
        for value in ["cadence.", "cadence-", "cadence_"] {
            assert!(CadenceIdentity::parse(value).is_err());
        }
        assert_eq!(
            CadenceInterval::new(1, 60).unwrap(),
            CadenceInterval::new(2, 120).unwrap()
        );
        assert!(matches!(
            CadenceInterval::new(0, 60),
            Err(ContinuousCadenceError::InvalidInterval { .. })
        ));
        assert!(matches!(
            CadenceInterval::new(61, 60),
            Err(ContinuousCadenceError::InvalidInterval { .. })
        ));
        assert!(matches!(
            Binary64AccumulatorSnapshot::new(f64::NAN.to_bits()).reopen(),
            Err(ContinuousCadenceError::InvalidAccumulator(
                ContinuousValueError::NonFinite { .. }
            ))
        ));
        let policy = CadenceQuantizationPolicy::v1(ContinuousQuantizationMode::TowardZero);
        assert!(matches!(
            CadenceQuantizationPolicy::new(ContinuousQuantizationMode::Floor, 2),
            Err(ContinuousCadenceError::StalePolicyVersion { .. })
        ));
        let snapshot = ResidualCarrySnapshot::new(
            1.0_f64.to_bits(),
            1,
            CONTINUOUS_EVALUATOR_SEMANTICS_VERSION,
            ContinuousQuantizationMode::TowardZero,
            1,
            origin(),
            interval(60),
            cadence(60),
        );
        assert!(matches!(
            snapshot.reopen(policy),
            Err(ContinuousCadenceError::InvalidResidualRemainder { .. })
        ));
        let stale = ResidualCarrySnapshot::new(
            0,
            2,
            CONTINUOUS_EVALUATOR_SEMANTICS_VERSION,
            ContinuousQuantizationMode::Floor,
            1,
            origin(),
            interval(60),
            cadence(60),
        );
        assert!(matches!(
            stale.reopen(policy),
            Err(ContinuousCadenceError::StaleResidualSchema { .. })
        ));
        let stale_evaluator = ResidualCarrySnapshot::new(
            0,
            RESIDUAL_CARRY_SCHEMA_VERSION,
            CONTINUOUS_EVALUATOR_SEMANTICS_VERSION + 1,
            ContinuousQuantizationMode::TowardZero,
            CONTINUOUS_QUANTIZATION_POLICY_VERSION,
            origin(),
            interval(60),
            cadence(60),
        );
        assert!(matches!(
            stale_evaluator.reopen(policy),
            Err(ContinuousCadenceError::StaleResidualEvaluator { .. })
        ));
        let stale_mode = ResidualCarrySnapshot::new(
            0,
            RESIDUAL_CARRY_SCHEMA_VERSION,
            CONTINUOUS_EVALUATOR_SEMANTICS_VERSION,
            ContinuousQuantizationMode::Floor,
            CONTINUOUS_QUANTIZATION_POLICY_VERSION,
            origin(),
            interval(60),
            cadence(60),
        );
        assert!(matches!(
            stale_mode.reopen(policy),
            Err(ContinuousCadenceError::StaleResidualMode { .. })
        ));
        let boundary = quantize_rate_with_caller_residual(
            value(MAX_ABS_MECHANICS_SCALAR as f64),
            CadenceInterval::new(1, 1).unwrap(),
            CadenceIdentity::parse("reference").unwrap(),
            policy,
            None,
            None,
            origin(),
        )
        .unwrap();
        assert_eq!(boundary.receipt().result().get(), MAX_ABS_MECHANICS_SCALAR);
        assert!(MechanicsScalar::new(MAX_ABS_MECHANICS_SCALAR)
            .unwrap()
            .checked_add(MechanicsScalar::new(1).unwrap())
            .is_err());
        assert!(matches!(
            quantize_rate_with_caller_residual(
                value((MAX_ABS_MECHANICS_SCALAR + 1) as f64),
                CadenceInterval::new(1, 1).unwrap(),
                CadenceIdentity::parse("reference").unwrap(),
                policy,
                None,
                None,
                origin()
            ),
            Err(ContinuousCadenceError::Quantization(
                ContinuousQuantizationError::OutOfRange { .. }
            ))
        ));
        assert!(matches!(
            quantize_rate_with_caller_residual(
                value(-(MAX_ABS_MECHANICS_SCALAR + 1) as f64),
                CadenceInterval::new(1, 1).unwrap(),
                CadenceIdentity::parse("reference").unwrap(),
                policy,
                None,
                None,
                origin()
            ),
            Err(ContinuousCadenceError::Quantization(
                ContinuousQuantizationError::OutOfRange { .. }
            ))
        ));
        let rejected = attempt_quantize_rate_with_caller_residual(
            value((MAX_ABS_MECHANICS_SCALAR + 1) as f64),
            CadenceInterval::new(1, 1).unwrap(),
            CadenceIdentity::parse("reference").unwrap(),
            policy,
            None,
            None,
            origin(),
        )
        .unwrap();
        match rejected {
            CadencedQuantizationAttempt::Rejected(rejection) => {
                assert_eq!(
                    rejection.interval_bits(),
                    CadenceInterval::new(1, 1).unwrap().bits()
                );
                assert_eq!(rejection.cadence().as_str(), "reference");
                assert_eq!(rejection.source(), &origin());
                assert!(matches!(
                    rejection.error(),
                    ContinuousQuantizationError::OutOfRange { .. }
                ));
            }
            CadencedQuantizationAttempt::Accepted(_) => {
                panic!("out-of-range rate unexpectedly accepted")
            }
        }
        assert!(matches!(
            accumulate_rate_binary64(
                value(f64::MAX),
                value(f64::MAX),
                CadenceInterval::new(1, 1).unwrap()
            ),
            Err(ContinuousCadenceError::NonFiniteStep(
                ContinuousValueError::NonFinite { .. }
            ))
        ));
        let mut hostile = ExactDeadlineAccumulator {
            rate_per_span: MechanicsScalar::new(MAX_ABS_MECHANICS_SCALAR).unwrap(),
            interval: CadenceInterval::new(1, 1).unwrap(),
            elapsed_steps: u64::MAX - 1,
            emitted: 0,
        };
        assert!(matches!(
            hostile.advance(),
            Err(ContinuousCadenceError::DeadlineOverflow)
        ));
        assert_eq!(hostile.elapsed_steps(), u64::MAX - 1);
    }

    #[test]
    fn cap_is_caller_policy_not_quantization_authority() {
        let (positive, _) = run_residual(value(7.0), 35, 1);
        let (negative, _) = run_residual(value(-7.0), 35, 1);
        let cap = MechanicsScalar::new(5).unwrap();
        let zero = MechanicsScalar::zero();
        let after_positive = zero
            .checked_add(MechanicsScalar::new(positive).unwrap())
            .unwrap()
            .clamp(zero, cap);
        let after_negative = after_positive
            .checked_add(MechanicsScalar::new(negative).unwrap())
            .unwrap()
            .clamp(zero, cap);
        assert_eq!((after_positive.get(), after_negative.get()), (5, 0));
        let tie = quantize_rate_with_caller_residual(
            value(0.5),
            CadenceInterval::new(1, 1).unwrap(),
            CadenceIdentity::parse("reference").unwrap(),
            CadenceQuantizationPolicy::v1(ContinuousQuantizationMode::NearestTiesToEven),
            None,
            None,
            origin(),
        )
        .unwrap();
        assert_eq!(
            (tie.receipt().result().get(), tie.receipt().remainder()),
            (0, value(0.5))
        );
        let negative_tie = quantize_rate_with_caller_residual(
            value(-0.5),
            CadenceInterval::new(1, 1).unwrap(),
            CadenceIdentity::parse("reference").unwrap(),
            CadenceQuantizationPolicy::v1(ContinuousQuantizationMode::NearestTiesToEven),
            None,
            None,
            origin(),
        )
        .unwrap();
        assert_eq!(
            (
                negative_tie.receipt().result().get(),
                negative_tie.receipt().remainder()
            ),
            (0, value(-0.5))
        );
    }
}
