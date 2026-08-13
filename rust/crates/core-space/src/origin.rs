use serde::{Deserialize, Serialize};

/// Exact signed integer origin for one bounded local simulation frame.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct WorldOrigin([i64; 3]);

impl WorldOrigin {
    pub const ZERO: Self = Self([0; 3]);

    pub const fn new(cell: [i64; 3]) -> Self {
        Self(cell)
    }

    pub const fn cell(self) -> [i64; 3] {
        self.0
    }
}

/// Canonical large-world point represented by an exact signed unit cell and a
/// normalized fractional offset in `[0, 1)`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GlobalPosition {
    cell: [i64; 3],
    offset: [f64; 3],
}

impl<'de> Deserialize<'de> for GlobalPosition {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields, rename_all = "camelCase")]
        struct StoredGlobalPosition {
            cell: [i64; 3],
            offset: [f64; 3],
        }

        let stored = StoredGlobalPosition::deserialize(deserializer)?;
        let value = Self::new(stored.cell, stored.offset).map_err(serde::de::Error::custom)?;
        if value.cell != stored.cell || value.offset != stored.offset {
            return Err(serde::de::Error::custom(
                "global position cell/offset pair is not canonical",
            ));
        }
        Ok(value)
    }
}

impl GlobalPosition {
    const I64_MAX_EXCLUSIVE_AS_F64: f64 = 9_223_372_036_854_775_808.0;

    pub const ORIGIN: Self = Self {
        cell: [0; 3],
        offset: [0.0; 3],
    };

    pub fn new(cell: [i64; 3], offset: [f64; 3]) -> Result<Self, GlobalPositionError> {
        let mut canonical_cell = cell;
        let mut canonical_offset = [0.0; 3];
        for axis in 0..3 {
            let value = offset[axis];
            if !value.is_finite() {
                return Err(GlobalPositionError::NonFiniteOffset { axis });
            }
            let whole = value.floor();
            // `i64::MAX as f64` rounds up to 2^63, so this upper bound must be
            // exclusive before the saturating float-to-integer cast.
            if whole < i64::MIN as f64 || whole >= Self::I64_MAX_EXCLUSIVE_AS_F64 {
                return Err(GlobalPositionError::CellOverflow { axis });
            }
            canonical_cell[axis] = canonical_cell[axis]
                .checked_add(whole as i64)
                .ok_or(GlobalPositionError::CellOverflow { axis })?;
            let fraction = value - whole;
            canonical_offset[axis] = if fraction == 0.0 { 0.0 } else { fraction };
        }
        Ok(Self {
            cell: canonical_cell,
            offset: canonical_offset,
        })
    }

    pub fn from_world(position: [f64; 3]) -> Result<Self, GlobalPositionError> {
        Self::new([0; 3], position)
    }

    pub fn from_local(origin: WorldOrigin, local: [f32; 3]) -> Result<Self, GlobalPositionError> {
        if let Some(axis) = local.iter().position(|value| !value.is_finite()) {
            return Err(GlobalPositionError::NonFiniteLocal { axis });
        }
        Self::new(origin.cell(), local.map(f64::from))
    }

    pub const fn cell(self) -> [i64; 3] {
        self.cell
    }

    pub const fn offset(self) -> [f64; 3] {
        self.offset
    }

    pub fn local(
        self,
        origin: WorldOrigin,
        envelope: f32,
    ) -> Result<[f32; 3], GlobalPositionError> {
        if !envelope.is_finite() || envelope <= 0.0 {
            return Err(GlobalPositionError::InvalidEnvelope);
        }
        let origin = origin.cell();
        let mut local = [0.0; 3];
        for axis in 0..3 {
            let delta = i128::from(self.cell[axis]) - i128::from(origin[axis]);
            let value = delta as f64 + self.offset[axis];
            if !value.is_finite() || value.abs() > f64::from(envelope) {
                return Err(GlobalPositionError::OutsideLocalEnvelope { axis });
            }
            let narrowed = value as f32;
            if !narrowed.is_finite() {
                return Err(GlobalPositionError::OutsideLocalEnvelope { axis });
            }
            local[axis] = narrowed;
        }
        Ok(local)
    }

    pub fn to_world(self) -> [f64; 3] {
        std::array::from_fn(|axis| self.cell[axis] as f64 + self.offset[axis])
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlobalPositionError {
    NonFiniteOffset { axis: usize },
    NonFiniteLocal { axis: usize },
    CellOverflow { axis: usize },
    InvalidEnvelope,
    OutsideLocalEnvelope { axis: usize },
}

impl std::fmt::Display for GlobalPositionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "global position rejected: {self:?}")
    }
}

impl std::error::Error for GlobalPositionError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_positions_are_canonical_and_rebase_without_accumulation() {
        let position = GlobalPosition::new([-70_001, 4, 90_000], [0.75, -0.25, 1.5]).unwrap();
        assert_eq!(position.cell(), [-70_001, 3, 90_001]);
        assert_eq!(position.offset(), [0.75, 0.75, 0.5]);
        let first = position
            .local(WorldOrigin::new([-70_000, 0, 90_000]), 16_384.0)
            .unwrap();
        assert_eq!(first, [-0.25, 3.75, 1.5]);
        let second = position
            .local(WorldOrigin::new([-80_000, 0, 80_000]), 16_384.0)
            .unwrap();
        assert_eq!(second, [9_999.75, 3.75, 10_001.5]);
        assert_eq!(
            GlobalPosition::from_local(WorldOrigin::new([-70_000, 0, 90_000]), first).unwrap(),
            position
        );
    }

    #[test]
    fn json_round_trip_preserves_exact_cell_and_fraction_bits() {
        let position = GlobalPosition::new([-900_000, 12, 800_000], [0.125, 0.5, 0.875]).unwrap();
        let encoded = serde_json::to_vec(&position).unwrap();
        assert_eq!(
            serde_json::from_slice::<GlobalPosition>(&encoded).unwrap(),
            position
        );
    }

    #[test]
    fn local_envelope_and_overflow_fail_closed() {
        let position = GlobalPosition::new([20_000, 0, 0], [0.0; 3]).unwrap();
        assert_eq!(
            position.local(WorldOrigin::ZERO, 16_384.0),
            Err(GlobalPositionError::OutsideLocalEnvelope { axis: 0 })
        );
        assert_eq!(
            GlobalPosition::new([i64::MAX, 0, 0], [1.0, 0.0, 0.0]),
            Err(GlobalPositionError::CellOverflow { axis: 0 })
        );
    }

    #[test]
    fn floating_integer_boundaries_do_not_saturate_or_reject_valid_cells() {
        let positive_limit = GlobalPosition::I64_MAX_EXCLUSIVE_AS_F64;
        assert_eq!(
            GlobalPosition::from_world([positive_limit, 0.0, 0.0]),
            Err(GlobalPositionError::CellOverflow { axis: 0 })
        );

        let largest_below_positive_limit = f64::from_bits(positive_limit.to_bits() - 1);
        let positive = GlobalPosition::from_world([largest_below_positive_limit, 0.0, 0.0])
            .expect("the largest f64 below 2^63 fits in i64");
        assert_eq!(positive.cell()[0], largest_below_positive_limit as i64);
        assert_eq!(positive.offset()[0], 0.0);

        let negative = GlobalPosition::from_world([i64::MIN as f64, 0.0, 0.0])
            .expect("the exact i64::MIN boundary remains valid");
        assert_eq!(negative.cell()[0], i64::MIN);
        assert_eq!(negative.offset()[0], 0.0);
    }
}
