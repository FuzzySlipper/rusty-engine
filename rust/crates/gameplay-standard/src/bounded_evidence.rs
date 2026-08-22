//! Bounded caller-supplied integer evidence.
//!
//! This module admits and validates named samples only. It never generates a
//! value, selects an outcome, schedules work, or interprets product meaning.

use std::{collections::BTreeMap, fmt};

pub const BOUNDED_SAMPLE_PLAN_SCHEMA_VERSION: u32 = 1;
pub const MAX_BOUNDED_SAMPLE_PLAN_IDENTITY_BYTES: usize = 96;
pub const MAX_BOUNDED_SAMPLE_KEY_BYTES: usize = 96;
pub const MAX_BOUNDED_SAMPLE_REQUIREMENTS: usize = 64;

/// A stable caller-owned identity for one evidence plan.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BoundedSamplePlanIdentity(String);

impl BoundedSamplePlanIdentity {
    pub fn parse(value: impl Into<String>) -> Result<Self, BoundedSamplePlanError> {
        let value = value.into();
        validate_identity(
            &value,
            MAX_BOUNDED_SAMPLE_PLAN_IDENTITY_BYTES,
            |byte_length, reason| BoundedSamplePlanError::InvalidPlanIdentity {
                byte_length,
                reason,
            },
        )?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A stable canonical key for one caller-supplied integer sample.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BoundedSampleKey(String);

impl BoundedSampleKey {
    pub fn parse(value: impl Into<String>) -> Result<Self, BoundedSamplePlanError> {
        let value = value.into();
        validate_identity(
            &value,
            MAX_BOUNDED_SAMPLE_KEY_BYTES,
            |byte_length, reason| BoundedSamplePlanError::InvalidSampleKey {
                byte_length,
                reason,
            },
        )?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A positive caller-owned plan version retained in each receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BoundedSamplePlanVersion(u32);

impl BoundedSamplePlanVersion {
    pub const fn new(value: u32) -> Result<Self, BoundedSamplePlanError> {
        if value == 0 {
            return Err(BoundedSamplePlanError::ZeroPlanVersion);
        }
        Ok(Self(value))
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

/// One required inclusive integer range, retained in caller-declared order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedSampleRequirement {
    key: BoundedSampleKey,
    minimum: i64,
    maximum: i64,
}

impl BoundedSampleRequirement {
    pub fn new(
        key: BoundedSampleKey,
        minimum: i64,
        maximum: i64,
    ) -> Result<Self, BoundedSamplePlanError> {
        if minimum > maximum {
            return Err(BoundedSamplePlanError::InvalidRequirementBounds {
                key,
                minimum,
                maximum,
            });
        }
        Ok(Self {
            key,
            minimum,
            maximum,
        })
    }

    pub fn key(&self) -> &BoundedSampleKey {
        &self.key
    }
    pub const fn minimum(&self) -> i64 {
        self.minimum
    }
    pub const fn maximum(&self) -> i64 {
        self.maximum
    }
}

/// One caller-supplied integer sample.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedSample {
    key: BoundedSampleKey,
    value: i64,
}

impl BoundedSample {
    pub const fn new(key: BoundedSampleKey, value: i64) -> Self {
        Self { key, value }
    }
    pub fn key(&self) -> &BoundedSampleKey {
        &self.key
    }
    pub const fn value(&self) -> i64 {
        self.value
    }
}

/// A bounded, named, caller-owned sample contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedSamplePlan {
    identity: BoundedSamplePlanIdentity,
    version: BoundedSamplePlanVersion,
    requirements: Vec<BoundedSampleRequirement>,
}

impl BoundedSamplePlan {
    /// Admits a non-empty ordered requirement list without silently merging keys.
    pub fn new(
        identity: BoundedSamplePlanIdentity,
        version: BoundedSamplePlanVersion,
        requirements: Vec<BoundedSampleRequirement>,
    ) -> Result<Self, BoundedSamplePlanError> {
        if requirements.is_empty() {
            return Err(BoundedSamplePlanError::EmptyRequirements);
        }
        if requirements.len() > MAX_BOUNDED_SAMPLE_REQUIREMENTS {
            return Err(BoundedSamplePlanError::RequirementQuotaExceeded {
                actual: requirements.len(),
                maximum: MAX_BOUNDED_SAMPLE_REQUIREMENTS,
            });
        }
        let mut keys = BTreeMap::new();
        for requirement in &requirements {
            if keys.insert(requirement.key.clone(), ()).is_some() {
                return Err(BoundedSamplePlanError::DuplicateRequirement {
                    key: requirement.key.clone(),
                });
            }
        }
        Ok(Self {
            identity,
            version,
            requirements,
        })
    }

    pub fn identity(&self) -> &BoundedSamplePlanIdentity {
        &self.identity
    }
    pub const fn version(&self) -> BoundedSamplePlanVersion {
        self.version
    }
    /// Returns the original caller-declared requirement order.
    pub fn requirements(&self) -> &[BoundedSampleRequirement] {
        &self.requirements
    }

    /// Validates a complete caller-supplied sample set and returns no receipt on rejection.
    pub fn validate(
        &self,
        samples: Vec<BoundedSample>,
    ) -> Result<BoundedSampleReceipt, BoundedSamplePlanError> {
        if samples.len() > MAX_BOUNDED_SAMPLE_REQUIREMENTS {
            return Err(BoundedSamplePlanError::SampleQuotaExceeded {
                actual: samples.len(),
                maximum: MAX_BOUNDED_SAMPLE_REQUIREMENTS,
            });
        }
        let mut supplied = BTreeMap::new();
        for sample in samples {
            if supplied.insert(sample.key.clone(), sample.value).is_some() {
                return Err(BoundedSamplePlanError::DuplicateSample { key: sample.key });
            }
        }
        let requirements_by_key = self
            .requirements
            .iter()
            .map(|requirement| (&requirement.key, requirement))
            .collect::<BTreeMap<_, _>>();
        for key in supplied.keys() {
            if !requirements_by_key.contains_key(key) {
                return Err(BoundedSamplePlanError::UnknownSample { key: key.clone() });
            }
        }
        let mut accepted_samples = Vec::with_capacity(self.requirements.len());
        for requirement in &self.requirements {
            let value = supplied.get(&requirement.key).copied().ok_or_else(|| {
                BoundedSamplePlanError::MissingSample {
                    key: requirement.key.clone(),
                }
            })?;
            if value < requirement.minimum || value > requirement.maximum {
                return Err(BoundedSamplePlanError::SampleOutOfRange {
                    key: requirement.key.clone(),
                    value,
                    minimum: requirement.minimum,
                    maximum: requirement.maximum,
                });
            }
            accepted_samples.push(BoundedSample::new(requirement.key.clone(), value));
        }
        Ok(BoundedSampleReceipt {
            identity: self.identity.clone(),
            version: self.version,
            requirements: self.requirements.clone(),
            accepted_samples,
        })
    }
}

/// A complete accepted evidence set, ordered by the plan's requirements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedSampleReceipt {
    identity: BoundedSamplePlanIdentity,
    version: BoundedSamplePlanVersion,
    requirements: Vec<BoundedSampleRequirement>,
    accepted_samples: Vec<BoundedSample>,
}

impl BoundedSampleReceipt {
    pub fn identity(&self) -> &BoundedSamplePlanIdentity {
        &self.identity
    }
    pub const fn version(&self) -> BoundedSamplePlanVersion {
        self.version
    }
    pub fn requirements(&self) -> &[BoundedSampleRequirement] {
        &self.requirements
    }
    pub fn accepted_samples(&self) -> &[BoundedSample] {
        &self.accepted_samples
    }
}

/// A rejected plan admission or sample validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundedSamplePlanError {
    InvalidPlanIdentity {
        byte_length: usize,
        reason: &'static str,
    },
    InvalidSampleKey {
        byte_length: usize,
        reason: &'static str,
    },
    ZeroPlanVersion,
    EmptyRequirements,
    RequirementQuotaExceeded {
        actual: usize,
        maximum: usize,
    },
    InvalidRequirementBounds {
        key: BoundedSampleKey,
        minimum: i64,
        maximum: i64,
    },
    DuplicateRequirement {
        key: BoundedSampleKey,
    },
    SampleQuotaExceeded {
        actual: usize,
        maximum: usize,
    },
    DuplicateSample {
        key: BoundedSampleKey,
    },
    UnknownSample {
        key: BoundedSampleKey,
    },
    MissingSample {
        key: BoundedSampleKey,
    },
    SampleOutOfRange {
        key: BoundedSampleKey,
        value: i64,
        minimum: i64,
        maximum: i64,
    },
}

impl fmt::Display for BoundedSamplePlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "bounded sample plan rejected: {self:?}")
    }
}
impl std::error::Error for BoundedSamplePlanError {}

fn validate_identity(
    value: &str,
    maximum_bytes: usize,
    invalid: impl FnOnce(usize, &'static str) -> BoundedSamplePlanError,
) -> Result<(), BoundedSamplePlanError> {
    if value.is_empty() || value.len() > maximum_bytes {
        return Err(invalid(
            value.len(),
            "identity must contain 1 to the configured maximum number of ASCII bytes",
        ));
    }
    let mut bytes = value.bytes();
    if !bytes.next().is_some_and(|byte| byte.is_ascii_lowercase()) {
        return Err(invalid(
            value.len(),
            "identity must start with a lowercase ASCII letter",
        ));
    }
    if !value
        .bytes()
        .last()
        .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
    {
        return Err(invalid(
            value.len(),
            "identity must end with a lowercase ASCII letter or digit",
        ));
    }
    if !bytes.all(|byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-' | b'_')
    }) {
        return Err(invalid(
            value.len(),
            "identity contains unsupported characters",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn key(value: &str) -> BoundedSampleKey {
        BoundedSampleKey::parse(value).unwrap()
    }
    fn requirement(value: &str, minimum: i64, maximum: i64) -> BoundedSampleRequirement {
        BoundedSampleRequirement::new(key(value), minimum, maximum).unwrap()
    }
    fn plan(requirements: Vec<BoundedSampleRequirement>) -> BoundedSamplePlan {
        BoundedSamplePlan::new(
            BoundedSamplePlanIdentity::parse("caller-evidence").unwrap(),
            BoundedSamplePlanVersion::new(7).unwrap(),
            requirements,
        )
        .unwrap()
    }

    #[test]
    fn identities_are_bounded_and_canonical() {
        for value in ["", "Evidence", "evidence-", "evidence space"] {
            assert!(matches!(
                BoundedSampleKey::parse(value),
                Err(BoundedSamplePlanError::InvalidSampleKey { .. })
            ));
        }
        assert!(BoundedSampleKey::parse("evidence.roll_1").is_ok());
        assert!(matches!(
            BoundedSamplePlanVersion::new(0),
            Err(BoundedSamplePlanError::ZeroPlanVersion)
        ));
    }

    #[test]
    fn admission_rejects_empty_invalid_duplicate_and_over_quota_requirements() {
        assert!(matches!(
            BoundedSamplePlan::new(
                BoundedSamplePlanIdentity::parse("empty").unwrap(),
                BoundedSamplePlanVersion::new(1).unwrap(),
                vec![]
            ),
            Err(BoundedSamplePlanError::EmptyRequirements)
        ));
        assert!(matches!(
            BoundedSampleRequirement::new(key("invalid"), 2, 1),
            Err(BoundedSamplePlanError::InvalidRequirementBounds { .. })
        ));
        assert!(matches!(
            BoundedSamplePlan::new(
                BoundedSamplePlanIdentity::parse("duplicate").unwrap(),
                BoundedSamplePlanVersion::new(1).unwrap(),
                vec![
                    requirement("one", 0, 1),
                    requirement("two", 0, 1),
                    requirement("one", 0, 1)
                ]
            ),
            Err(BoundedSamplePlanError::DuplicateRequirement { .. })
        ));
        let requirements = (0..=MAX_BOUNDED_SAMPLE_REQUIREMENTS)
            .map(|index| requirement(&format!("key-{index}"), 0, 1))
            .collect();
        assert!(
            matches!(BoundedSamplePlan::new(BoundedSamplePlanIdentity::parse("over-quota").unwrap(), BoundedSamplePlanVersion::new(1).unwrap(), requirements), Err(BoundedSamplePlanError::RequirementQuotaExceeded { actual, maximum: MAX_BOUNDED_SAMPLE_REQUIREMENTS }) if actual == MAX_BOUNDED_SAMPLE_REQUIREMENTS + 1)
        );
    }

    #[test]
    fn exact_requirement_and_sample_quotas_are_accepted() {
        let requirements = (0..MAX_BOUNDED_SAMPLE_REQUIREMENTS)
            .map(|index| requirement(&format!("key-{index}"), index as i64, index as i64))
            .collect();
        let plan = plan(requirements);
        let samples = (0..MAX_BOUNDED_SAMPLE_REQUIREMENTS)
            .rev()
            .map(|index| BoundedSample::new(key(&format!("key-{index}")), index as i64))
            .collect();
        let receipt = plan.validate(samples).unwrap();
        assert_eq!(
            receipt.requirements().len(),
            MAX_BOUNDED_SAMPLE_REQUIREMENTS
        );
        assert_eq!(
            receipt.accepted_samples().len(),
            MAX_BOUNDED_SAMPLE_REQUIREMENTS
        );
        assert_eq!(receipt.accepted_samples()[0].key().as_str(), "key-0");
    }

    #[test]
    fn validation_rejects_duplicate_unknown_missing_and_over_quota_samples() {
        let plan = plan(vec![requirement("one", 0, 1), requirement("two", 0, 1)]);
        assert!(matches!(
            plan.validate(vec![
                BoundedSample::new(key("one"), 0),
                BoundedSample::new(key("one"), 1),
                BoundedSample::new(key("two"), 0)
            ]),
            Err(BoundedSamplePlanError::DuplicateSample { .. })
        ));
        assert!(matches!(
            plan.validate(vec![
                BoundedSample::new(key("one"), 0),
                BoundedSample::new(key("other"), 0)
            ]),
            Err(BoundedSamplePlanError::UnknownSample { .. })
        ));
        assert!(matches!(
            plan.validate(vec![BoundedSample::new(key("one"), 0)]),
            Err(BoundedSamplePlanError::MissingSample { .. })
        ));
        assert!(plan
            .validate(vec![
                BoundedSample::new(key("one"), 0),
                BoundedSample::new(key("two"), 1),
            ])
            .is_ok());
        let too_many = (0..=MAX_BOUNDED_SAMPLE_REQUIREMENTS)
            .map(|index| BoundedSample::new(key(&format!("key-{index}")), 0))
            .collect();
        assert!(
            matches!(plan.validate(too_many), Err(BoundedSamplePlanError::SampleQuotaExceeded { actual, maximum: MAX_BOUNDED_SAMPLE_REQUIREMENTS }) if actual == MAX_BOUNDED_SAMPLE_REQUIREMENTS + 1)
        );
    }

    #[test]
    fn validation_uses_inclusive_i64_comparisons_without_endpoint_arithmetic() {
        let endpoints = plan(vec![requirement("endpoints", i64::MIN, i64::MAX)]);
        assert!(endpoints
            .validate(vec![BoundedSample::new(key("endpoints"), i64::MIN)])
            .is_ok());
        assert!(endpoints
            .validate(vec![BoundedSample::new(key("endpoints"), i64::MAX)])
            .is_ok());
        let near_minimum = plan(vec![requirement("near-min", i64::MIN + 1, i64::MIN + 1)]);
        assert!(matches!(
            near_minimum.validate(vec![BoundedSample::new(key("near-min"), i64::MIN)]),
            Err(BoundedSamplePlanError::SampleOutOfRange { .. })
        ));
        let near_maximum = plan(vec![requirement("near-max", i64::MAX - 1, i64::MAX - 1)]);
        assert!(matches!(
            near_maximum.validate(vec![BoundedSample::new(key("near-max"), i64::MAX)]),
            Err(BoundedSamplePlanError::SampleOutOfRange { .. })
        ));
    }

    #[test]
    fn receipt_preserves_identity_version_and_requirement_order() {
        let plan = plan(vec![
            requirement("first", -2, 2),
            requirement("second", 3, 7),
        ]);
        let receipt = plan
            .validate(vec![
                BoundedSample::new(key("second"), 7),
                BoundedSample::new(key("first"), -2),
            ])
            .unwrap();
        assert_eq!(receipt.identity().as_str(), "caller-evidence");
        assert_eq!(receipt.version().get(), 7);
        assert_eq!(
            receipt
                .requirements()
                .iter()
                .map(|requirement| requirement.key().as_str())
                .collect::<Vec<_>>(),
            vec!["first", "second"]
        );
        assert_eq!(
            receipt
                .accepted_samples()
                .iter()
                .map(|sample| (sample.key().as_str(), sample.value()))
                .collect::<Vec<_>>(),
            vec![("first", -2), ("second", 7)]
        );
    }
}
