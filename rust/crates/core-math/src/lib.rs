//! Small deterministic vector primitives for the ASHA workspace.
//!
//! # Lane
//!
//! `rust-foundation` — `std`-only, zero external dependencies, no knowledge of
//! state, protocol, render, services, or TypeScript.
//!
//! # Design
//!
//! [`Vec2`] and [`Vec3`] are plain `f32` value types with explicit constructors
//! and the handful of operations render/spatial code needs soon (add, sub,
//! scalar multiply, dot, cross, squared length). `f32` is chosen to match the
//! border `Transform` shapes in `protocol-render`.
//!
//! Operations that stay in integer-valued territory (add/sub/scale, dot of
//! integer components) are exact, so tests assert them exactly via whole-vector
//! equality. Only `length` introduces a `sqrt`; prefer `length_squared` when an
//! exact comparison matters.
//!
//! # Non-goals
//!
//! No matrices, quaternions, affine `Transform3`, easing, or geometry
//! algorithms — those are render/service concerns, not foundation. No `f64`
//! surface until a caller justifies it.

#![forbid(unsafe_code)]

use std::ops::{Add, Mul, Sub};

/// A 2-D vector of `f32` components.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Vec2 = Vec2 { x: 0.0, y: 0.0 };
    pub const ONE: Vec2 = Vec2 { x: 1.0, y: 1.0 };

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// A vector with both components set to `v`.
    pub const fn splat(v: f32) -> Self {
        Self { x: v, y: v }
    }

    pub fn dot(self, rhs: Vec2) -> f32 {
        self.x * rhs.x + self.y * rhs.y
    }

    pub fn length_squared(self) -> f32 {
        self.dot(self)
    }

    pub fn length(self) -> f32 {
        self.length_squared().sqrt()
    }
}

impl Add for Vec2 {
    type Output = Vec2;
    fn add(self, rhs: Vec2) -> Vec2 {
        Vec2::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl Sub for Vec2 {
    type Output = Vec2;
    fn sub(self, rhs: Vec2) -> Vec2 {
        Vec2::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl Mul<f32> for Vec2 {
    type Output = Vec2;
    fn mul(self, scalar: f32) -> Vec2 {
        Vec2::new(self.x * scalar, self.y * scalar)
    }
}

/// A 3-D vector of `f32` components.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const ZERO: Vec3 = Vec3 {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    pub const ONE: Vec3 = Vec3 {
        x: 1.0,
        y: 1.0,
        z: 1.0,
    };

    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub const fn splat(v: f32) -> Self {
        Self { x: v, y: v, z: v }
    }

    pub fn dot(self, rhs: Vec3) -> f32 {
        self.x * rhs.x + self.y * rhs.y + self.z * rhs.z
    }

    pub fn cross(self, rhs: Vec3) -> Vec3 {
        Vec3::new(
            self.y * rhs.z - self.z * rhs.y,
            self.z * rhs.x - self.x * rhs.z,
            self.x * rhs.y - self.y * rhs.x,
        )
    }

    pub fn length_squared(self) -> f32 {
        self.dot(self)
    }

    pub fn length(self) -> f32 {
        self.length_squared().sqrt()
    }

    /// The component-wise array, in `[x, y, z]` order (matches the render border).
    pub const fn to_array(self) -> [f32; 3] {
        [self.x, self.y, self.z]
    }
}

impl Add for Vec3 {
    type Output = Vec3;
    fn add(self, rhs: Vec3) -> Vec3 {
        Vec3::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }
}

impl Sub for Vec3 {
    type Output = Vec3;
    fn sub(self, rhs: Vec3) -> Vec3 {
        Vec3::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }
}

impl Mul<f32> for Vec3 {
    type Output = Vec3;
    fn mul(self, scalar: f32) -> Vec3 {
        Vec3::new(self.x * scalar, self.y * scalar, self.z * scalar)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vec2_add_sub_scale_are_exact() {
        let a = Vec2::new(1.0, 2.0);
        let b = Vec2::new(3.0, 5.0);
        assert_eq!(a + b, Vec2::new(4.0, 7.0));
        assert_eq!(b - a, Vec2::new(2.0, 3.0));
        assert_eq!(a * 3.0, Vec2::new(3.0, 6.0));
        assert_eq!(Vec2::splat(4.0), Vec2::new(4.0, 4.0));
        assert_eq!(Vec2::ZERO + Vec2::ONE, Vec2::ONE);
    }

    #[test]
    fn vec3_add_sub_scale_are_exact() {
        let a = Vec3::new(1.0, 2.0, 3.0);
        let b = Vec3::new(4.0, 5.0, 6.0);
        assert_eq!(a + b, Vec3::new(5.0, 7.0, 9.0));
        assert_eq!(b - a, Vec3::new(3.0, 3.0, 3.0));
        assert_eq!(a * 2.0, Vec3::new(2.0, 4.0, 6.0));
        assert_eq!(Vec3::splat(2.0), Vec3::new(2.0, 2.0, 2.0));
        assert_eq!(a.to_array(), [1.0, 2.0, 3.0]);
    }

    #[test]
    fn vec3_cross_is_exact_for_basis_vectors() {
        let x = Vec3::new(1.0, 0.0, 0.0);
        let y = Vec3::new(0.0, 1.0, 0.0);
        assert_eq!(x.cross(y), Vec3::new(0.0, 0.0, 1.0));
        assert_eq!(y.cross(x), Vec3::new(0.0, 0.0, -1.0));
    }

    #[test]
    fn vec3_dot_and_length() {
        let v = Vec3::new(1.0, 2.0, 2.0);
        // 1*1 + 2*2 + 2*2 = 9, all integer-valued and exact.
        assert_eq!(v.length_squared(), 9.0);
        assert_eq!(v.dot(Vec3::new(2.0, 0.0, 1.0)), 4.0);
        // length is sqrt(9) = 3 exactly.
        assert_eq!(v.length(), 3.0);
    }

    #[test]
    fn vec2_dot_and_length() {
        let v = Vec2::new(3.0, 4.0);
        assert_eq!(v.length_squared(), 25.0);
        assert_eq!(v.length(), 5.0);
    }
}
