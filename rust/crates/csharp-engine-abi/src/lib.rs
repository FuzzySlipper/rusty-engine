//! The sole Rust-owned C ABI declaration for trusted NativeAOT products.
//!
//! C# raw declarations and its safe direct facade are generated mechanically
//! from these family modules. The table names Engine service families; it
//! intentionally has no generic invocation, target strings, capability
//! catalogue, or JSON command protocol.

mod animation;
mod appearance;
mod audio;
mod authored_content;
mod camera_view;
mod content;
mod content_store;
mod core;
mod diagnostics;
mod dynamics;
mod input;
mod kinematic;
mod lifecycle;
mod look;
mod motion;
mod perception;
mod persistence;
mod presentation;
mod product;
mod rng;
mod spatial;
mod ui;
mod voxel;
mod voxel_content;
mod voxel_scene_presentation;
mod world_origin;

pub use animation::*;
pub use appearance::*;
pub use audio::*;
pub use authored_content::*;
pub use camera_view::*;
pub use content::*;
pub use content_store::*;
pub use core::*;
pub use diagnostics::*;
pub use dynamics::*;
pub use input::*;
pub use kinematic::*;
pub use lifecycle::*;
pub use look::*;
pub use motion::*;
pub use perception::*;
pub use persistence::*;
pub use presentation::*;
pub use product::*;
pub use rng::*;
pub use spatial::*;
pub use ui::*;
pub use voxel::*;
pub use voxel_content::*;
pub use voxel_scene_presentation::*;
pub use world_origin::*;
