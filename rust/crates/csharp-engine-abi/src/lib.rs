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
mod content;
mod content_store;
mod continuous_mechanics;
mod core;
mod dynamics;
mod input;
mod lifecycle;
mod look;
mod mechanics;
mod persistence;
mod product;
mod rng;
mod rules;
mod spatial;
mod standard_continuous;
mod standard_exact;
mod ui;
mod voxel;
mod voxel_content;

pub use animation::*;
pub use appearance::*;
pub use audio::*;
pub use camera_view::*;
pub use content::*;
pub use content_store::*;
pub use continuous_mechanics::*;
pub use core::*;
pub use dynamics::*;
pub use input::*;
pub use lifecycle::*;
pub use look::*;
pub use mechanics::*;
pub use persistence::*;
pub use product::*;
pub use rng::*;
pub use rules::*;
pub use spatial::*;
pub use standard_continuous::*;
pub use standard_exact::*;
pub use ui::*;
pub use voxel::*;
pub use voxel_content::*;
