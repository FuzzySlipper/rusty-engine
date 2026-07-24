//! Typed spatial coordinate foundation for ASHA voxel systems.
//!
//! # Lane
//!
//! `rust-foundation` — `std`-only, zero external dependencies, no knowledge of
//! state, protocol, render, services, or TypeScript. Voxel value/material,
//! chunk storage, partitioning, meshing, collision, and rendering all build on
//! the typed spaces defined here (voxel-capability-01).
//!
//! # Design soul
//!
//! The point of this crate is to make it a **compile-time error to mix spaces**.
//! Continuous world positions ([`WorldPos`]), integer voxel cells
//! ([`VoxelCoord`]), chunk coordinates ([`ChunkCoord`]), and chunk-local
//! addresses ([`LocalVoxelCoord`]) are distinct newtypes; you cannot accidentally
//! add a chunk coordinate to a world position.
//!
//! There is **no single universal voxel size**. Every world↔grid conversion goes
//! through an explicit [`VoxelGridSpec`], so terrain, character/object, and local
//! object grids can use different scales and chunk shapes simultaneously. The same
//! [`WorldPos`] resolves to different [`VoxelCoord`]s under different specs.
//!
//! # Conventions (voxel-capability-01 §"Current accepted guidance")
//!
//! - **Y-up**, right-handed (matches Three.js defaults).
//! - Voxel `(0,0,0)` occupies `[0,1)³` in *grid units*; its center is
//!   `(0.5,0.5,0.5)` grid units. World size of a cell is `voxel_size`.
//! - **Floor division** for negative coordinates (not truncation), so the grid is
//!   uniform across the origin.
//! - World positions are `f64`-backed ([`WorldScalar`]) for precision/large worlds;
//!   the alias keeps the scalar swappable without rewriting consumers.
//! - Grid coordinates are `i64`-backed for headroom; chunk-local is `u32`.
//! - Rebasing is not implemented, but [`VoxelGridSpec`] carries an explicit
//!   [`GridId`] and origin hook (`origin_world`) so an origin shift can be
//!   introduced later without changing the conversion call sites.

#![forbid(unsafe_code)]

mod direction;
mod grid;
mod region;
mod voxel;
mod world;

pub use direction::{Axis, Direction6, Face};
pub use grid::{ChunkDims, GridId, SpatialCellCoord, SpatialGridSpec, VoxelGridSpec};
pub use region::{ChunkRegion, VoxelRegion};
pub use voxel::{ChunkCoord, LocalVoxelCoord, VoxelCoord};
pub use world::{WorldPos, WorldScalar, WorldVec};

/// Floor division of `a` by a strictly positive `b` (`b > 0`).
///
/// Unlike `/`, this rounds toward negative infinity so the voxel/chunk grid is
/// uniform across the origin: `floor_div(-1, 16) == -1`, not `0`.
#[inline]
pub(crate) const fn floor_div(a: i64, b: i64) -> i64 {
    debug_assert!(b > 0);
    let q = a / b;
    let r = a % b;
    if r != 0 && (r < 0) != (b < 0) {
        q - 1
    } else {
        q
    }
}

/// The non-negative remainder of `a` modulo a strictly positive `b` (`b > 0`).
///
/// Pairs with [`floor_div`]: `a == floor_div(a, b) * b + rem_euclid(a, b)` and the
/// result is always in `0..b`.
#[inline]
pub(crate) const fn rem_euclid(a: i64, b: i64) -> i64 {
    let r = a % b;
    if r < 0 {
        r + b
    } else {
        r
    }
}

#[cfg(test)]
mod arith_tests {
    use super::{floor_div, rem_euclid};

    #[test]
    fn floor_div_rounds_toward_negative_infinity() {
        assert_eq!(floor_div(0, 16), 0);
        assert_eq!(floor_div(15, 16), 0);
        assert_eq!(floor_div(16, 16), 1);
        assert_eq!(floor_div(-1, 16), -1);
        assert_eq!(floor_div(-16, 16), -1);
        assert_eq!(floor_div(-17, 16), -2);
    }

    #[test]
    fn rem_euclid_is_always_non_negative() {
        assert_eq!(rem_euclid(0, 16), 0);
        assert_eq!(rem_euclid(-1, 16), 15);
        assert_eq!(rem_euclid(-16, 16), 0);
        assert_eq!(rem_euclid(17, 16), 1);
    }

    #[test]
    fn floor_div_and_rem_reconstruct_the_input() {
        for a in -40i64..40 {
            for b in [1i64, 2, 3, 16, 32] {
                assert_eq!(floor_div(a, b) * b + rem_euclid(a, b), a);
            }
        }
    }
}
