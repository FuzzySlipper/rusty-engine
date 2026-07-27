use std::fmt;

use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

pub const MAX_ABS_MECHANICS_SCALAR: i64 = 1_000_000_000_000;
pub const MAX_RATIO_COMPONENT: u32 = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MechanicsScalar(i64);

impl MechanicsScalar {
    pub fn new(value: i64) -> Result<Self, MechanicsArithmeticError> {
        if value.unsigned_abs() > MAX_ABS_MECHANICS_SCALAR as u64 {
            return Err(MechanicsArithmeticError::ScalarOutOfRange { value });
        }
        Ok(Self(value))
    }

    pub const fn zero() -> Self {
        Self(0)
    }

    pub const fn get(self) -> i64 {
        self.0
    }

    pub fn checked_add(self, other: Self) -> Result<Self, MechanicsArithmeticError> {
        let value = self
            .0
            .checked_add(other.0)
            .ok_or(MechanicsArithmeticError::Overflow)?;
        Self::new(value)
    }

    pub fn checked_sub(self, other: Self) -> Result<Self, MechanicsArithmeticError> {
        let value = self
            .0
            .checked_sub(other.0)
            .ok_or(MechanicsArithmeticError::Overflow)?;
        Self::new(value)
    }

    pub fn clamp(self, minimum: Self, maximum: Self) -> Self {
        Self(self.0.clamp(minimum.0, maximum.0))
    }

    pub fn require_nonnegative(self) -> Result<Self, MechanicsArithmeticError> {
        if self.0 < 0 {
            return Err(MechanicsArithmeticError::NegativeAmount { value: self.0 });
        }
        Ok(self)
    }

    pub(crate) fn capped_nonnegative_distance_from(
        self,
        lower: Self,
        cap: Self,
    ) -> Result<Self, MechanicsArithmeticError> {
        let cap = cap.require_nonnegative()?;
        let distance = i128::from(self.0) - i128::from(lower.0);
        if distance < 0 {
            return Err(MechanicsArithmeticError::Overflow);
        }
        let bounded = distance.min(i128::from(cap.0));
        let bounded = i64::try_from(bounded).map_err(|_| MechanicsArithmeticError::Overflow)?;
        Self::new(bounded)
    }
}

impl Serialize for MechanicsScalar {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_i64(self.0)
    }
}

impl<'de> Deserialize<'de> for MechanicsScalar {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = i64::deserialize(deserializer)?;
        Self::new(value).map_err(de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExactRatio {
    numerator: u32,
    denominator: u32,
}

impl<'de> Deserialize<'de> for ExactRatio {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields, rename_all = "camelCase")]
        struct WireRatio {
            numerator: u32,
            denominator: u32,
        }

        let value = WireRatio::deserialize(deserializer)?;
        Self::new(value.numerator, value.denominator).map_err(de::Error::custom)
    }
}

impl ExactRatio {
    pub fn new(numerator: u32, denominator: u32) -> Result<Self, MechanicsArithmeticError> {
        if denominator == 0 {
            return Err(MechanicsArithmeticError::ZeroDenominator);
        }
        if numerator > MAX_RATIO_COMPONENT || denominator > MAX_RATIO_COMPONENT {
            return Err(MechanicsArithmeticError::RatioComponentOutOfRange {
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundingPolicy {
    TowardZero,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CombinedRatio {
    numerator: u128,
    denominator: u128,
}

impl CombinedRatio {
    pub const fn one() -> Self {
        Self {
            numerator: 1,
            denominator: 1,
        }
    }

    pub const fn numerator(self) -> u128 {
        self.numerator
    }

    pub const fn denominator(self) -> u128 {
        self.denominator
    }

    pub fn include(&mut self, ratio: ExactRatio) -> Result<(), MechanicsArithmeticError> {
        let numerator = self
            .numerator
            .checked_mul(u128::from(ratio.numerator))
            .ok_or(MechanicsArithmeticError::Overflow)?;
        let denominator = self
            .denominator
            .checked_mul(u128::from(ratio.denominator))
            .ok_or(MechanicsArithmeticError::Overflow)?;
        let divisor = greatest_common_divisor_u128(numerator, denominator);
        self.numerator = numerator / divisor;
        self.denominator = denominator / divisor;
        Ok(())
    }

    pub fn apply_nonnegative(
        self,
        value: MechanicsScalar,
        policy: RoundingPolicy,
    ) -> Result<MechanicsScalar, MechanicsArithmeticError> {
        let value = value.require_nonnegative()?;
        let product = (value.get() as u128)
            .checked_mul(self.numerator)
            .ok_or(MechanicsArithmeticError::Overflow)?;
        let scaled = match policy {
            RoundingPolicy::TowardZero => product / self.denominator,
        };
        let scaled = i64::try_from(scaled).map_err(|_| MechanicsArithmeticError::Overflow)?;
        MechanicsScalar::new(scaled)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MechanicsArithmeticError {
    ScalarOutOfRange { value: i64 },
    RatioComponentOutOfRange { numerator: u32, denominator: u32 },
    ZeroDenominator,
    NegativeAmount { value: i64 },
    Overflow,
}

impl fmt::Display for MechanicsArithmeticError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "mechanics arithmetic rejected: {self:?}")
    }
}

impl std::error::Error for MechanicsArithmeticError {}

const fn greatest_common_divisor(mut left: u32, mut right: u32) -> u32 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

const fn greatest_common_divisor_u128(mut left: u128, mut right: u128) -> u128 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

#[cfg(test)]
mod tests {
    use super::{CombinedRatio, ExactRatio, MechanicsScalar, RoundingPolicy};

    #[test]
    fn ratios_normalize_combine_and_round_once() {
        let half = ExactRatio::new(2, 4).unwrap();
        assert_eq!((half.numerator(), half.denominator()), (1, 2));
        let mut combined = CombinedRatio::one();
        combined.include(half).unwrap();
        combined.include(ExactRatio::new(3, 2).unwrap()).unwrap();
        assert_eq!((combined.numerator(), combined.denominator()), (3, 4));
        assert_eq!(
            combined
                .apply_nonnegative(
                    MechanicsScalar::new(11).unwrap(),
                    RoundingPolicy::TowardZero
                )
                .unwrap()
                .get(),
            8
        );
        assert!(serde_json::from_str::<ExactRatio>(r#"{"numerator":1,"denominator":0}"#).is_err());
        assert_eq!(
            serde_json::from_str::<ExactRatio>(r#"{"numerator":2,"denominator":4}"#).unwrap(),
            half
        );
    }
}
