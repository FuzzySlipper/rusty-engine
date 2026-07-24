//! Orthogonal axes, the six face directions, and cube faces.

use crate::world::WorldVec;

/// One of the three orthogonal axes (Y is up).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Axis {
    X,
    Y,
    Z,
}

impl Axis {
    pub const ALL: [Axis; 3] = [Axis::X, Axis::Y, Axis::Z];

    /// Index into an `[x, y, z]` array.
    pub const fn index(self) -> usize {
        match self {
            Axis::X => 0,
            Axis::Y => 1,
            Axis::Z => 2,
        }
    }
}

/// One of the six axis-aligned directions / outward face normals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Direction6 {
    PosX,
    NegX,
    PosY,
    NegY,
    PosZ,
    NegZ,
}

impl Direction6 {
    /// Deterministic, stable order — used for face iteration in meshing.
    pub const ALL: [Direction6; 6] = [
        Direction6::PosX,
        Direction6::NegX,
        Direction6::PosY,
        Direction6::NegY,
        Direction6::PosZ,
        Direction6::NegZ,
    ];

    /// The integer unit step for this direction, as `[x, y, z]` in `{-1,0,1}`.
    pub const fn offset(self) -> [i32; 3] {
        match self {
            Direction6::PosX => [1, 0, 0],
            Direction6::NegX => [-1, 0, 0],
            Direction6::PosY => [0, 1, 0],
            Direction6::NegY => [0, -1, 0],
            Direction6::PosZ => [0, 0, 1],
            Direction6::NegZ => [0, 0, -1],
        }
    }

    /// The outward normal as a continuous world vector.
    pub fn normal(self) -> WorldVec {
        let [x, y, z] = self.offset();
        WorldVec::new(x as f64, y as f64, z as f64)
    }

    /// The axis this direction runs along.
    pub const fn axis(self) -> Axis {
        match self {
            Direction6::PosX | Direction6::NegX => Axis::X,
            Direction6::PosY | Direction6::NegY => Axis::Y,
            Direction6::PosZ | Direction6::NegZ => Axis::Z,
        }
    }

    /// `true` for the positive direction along its axis.
    pub const fn is_positive(self) -> bool {
        matches!(self, Direction6::PosX | Direction6::PosY | Direction6::PosZ)
    }

    /// The opposing direction.
    pub const fn opposite(self) -> Direction6 {
        match self {
            Direction6::PosX => Direction6::NegX,
            Direction6::NegX => Direction6::PosX,
            Direction6::PosY => Direction6::NegY,
            Direction6::NegY => Direction6::PosY,
            Direction6::PosZ => Direction6::NegZ,
            Direction6::NegZ => Direction6::PosZ,
        }
    }
}

/// A face of a voxel cube, identified by its outward normal direction.
///
/// Aliased to [`Direction6`] because a cube's six faces correspond exactly to the
/// six axis-aligned directions; meshing/picking can name faces without a separate
/// enum. (Kept as a named alias so intent reads clearly at call sites.)
pub type Face = Direction6;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opposite_is_an_involution_and_flips_sign() {
        for d in Direction6::ALL {
            assert_eq!(d.opposite().opposite(), d);
            assert_ne!(d.opposite(), d);
            assert_eq!(d.axis(), d.opposite().axis());
            assert_ne!(d.is_positive(), d.opposite().is_positive());
        }
    }

    #[test]
    fn normal_matches_offset_and_axis() {
        for d in Direction6::ALL {
            let [x, y, z] = d.offset();
            assert_eq!(d.normal(), WorldVec::new(x as f64, y as f64, z as f64));
            // Exactly one component is non-zero, on the direction's axis.
            let comps = [x, y, z];
            assert_eq!(comps.iter().filter(|c| **c != 0).count(), 1);
            assert_ne!(comps[d.axis().index()], 0);
        }
    }

    #[test]
    fn all_six_directions_are_distinct() {
        let all = Direction6::ALL;
        for (i, a) in all.iter().enumerate() {
            for b in &all[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }
}
