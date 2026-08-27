//! The sole Rust-owned C ABI declaration for trusted NativeAOT products.
//!
//! C# raw declarations and its safe direct facade are generated mechanically
//! from these family modules. The table names Engine service families; it
//! intentionally has no generic invocation, target strings, capability
//! catalogue, or JSON command protocol.

mod animation;
mod appearance;
mod audio;
mod camera_view;
mod core;
mod dynamics;
mod input;
mod lifecycle;
mod look;
mod mechanics;
mod persistence;
mod product;
mod rng;
mod spatial;
mod ui;
mod voxel;

pub use animation::*;
pub use appearance::*;
pub use audio::*;
pub use camera_view::*;
pub use core::*;
pub use dynamics::*;
pub use input::*;
pub use lifecycle::*;
pub use look::*;
pub use mechanics::*;
pub use persistence::*;
pub use product::*;
pub use rng::*;
pub use spatial::*;
pub use ui::*;
pub use voxel::*;
