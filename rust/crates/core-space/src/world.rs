//! Continuous world space: float-backed positions and displacements.

use core::ops::{Add, Sub};

/// The scalar backing continuous world quantities.
///
/// `f64` is chosen for precision and large-world headroom on the authority side;
/// the render border down-converts to `f32`. Kept as an alias so the backing can
/// be measured-and-changed without touching every consumer (voxel-capability-01
/// §"Float vs integer position posture").
pub type WorldScalar = f64;

/// A continuous position in world space (Y-up, right-handed).
///
/// Distinct from [`WorldVec`]: a position is a point, a vector is a displacement.
/// Subtracting two positions yields a [`WorldVec`]; adding a vector to a position
/// yields a position.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldPos {
    pub x: WorldScalar,
    pub y: WorldScalar,
    pub z: WorldScalar,
}

/// A continuous displacement/direction in world space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldVec {
    pub x: WorldScalar,
    pub y: WorldScalar,
    pub z: WorldScalar,
}

impl WorldPos {
    pub const ORIGIN: WorldPos = WorldPos {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    pub const fn new(x: WorldScalar, y: WorldScalar, z: WorldScalar) -> Self {
        Self { x, y, z }
    }

    /// `[x, y, z]` array (the render-border order).
    pub const fn to_array(self) -> [WorldScalar; 3] {
        [self.x, self.y, self.z]
    }
}

impl WorldVec {
    pub const ZERO: WorldVec = WorldVec {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    pub const fn new(x: WorldScalar, y: WorldScalar, z: WorldScalar) -> Self {
        Self { x, y, z }
    }

    pub const fn to_array(self) -> [WorldScalar; 3] {
        [self.x, self.y, self.z]
    }

    pub fn dot(self, rhs: WorldVec) -> WorldScalar {
        self.x * rhs.x + self.y * rhs.y + self.z * rhs.z
    }

    pub fn length_squared(self) -> WorldScalar {
        self.dot(self)
    }

    pub fn length(self) -> WorldScalar {
        self.length_squared().sqrt()
    }
}

impl Add<WorldVec> for WorldPos {
    type Output = WorldPos;
    fn add(self, rhs: WorldVec) -> WorldPos {
        WorldPos::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }
}

impl Sub<WorldVec> for WorldPos {
    type Output = WorldPos;
    fn sub(self, rhs: WorldVec) -> WorldPos {
        WorldPos::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }
}

/// Position − position = the displacement between them.
impl Sub<WorldPos> for WorldPos {
    type Output = WorldVec;
    fn sub(self, rhs: WorldPos) -> WorldVec {
        WorldVec::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }
}

impl Add for WorldVec {
    type Output = WorldVec;
    fn add(self, rhs: WorldVec) -> WorldVec {
        WorldVec::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }
}

impl Sub for WorldVec {
    type Output = WorldVec;
    fn sub(self, rhs: WorldVec) -> WorldVec {
        WorldVec::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_minus_position_is_a_displacement() {
        let a = WorldPos::new(3.0, 5.0, 9.0);
        let b = WorldPos::new(1.0, 2.0, 4.0);
        assert_eq!(a - b, WorldVec::new(2.0, 3.0, 5.0));
    }

    #[test]
    fn position_plus_vector_round_trips() {
        let p = WorldPos::new(1.5, -2.0, 0.25);
        let v = WorldVec::new(0.5, 2.0, -0.25);
        assert_eq!((p + v) - v, p);
    }
}
