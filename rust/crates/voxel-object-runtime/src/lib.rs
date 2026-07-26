//! Host-neutral admission and explicit-time playback for durable voxel objects.
//!
//! This crate owns no renderer, scheduler, collision world, navigation world,
//! filesystem, or ambient clock. It resolves a validated object into bounded,
//! reusable frame meshes and lets a named caller drive playback with integer
//! microsecond timestamps.

#![forbid(unsafe_code)]

mod admission;
mod collision;
mod model;
mod player;

pub use admission::*;
pub use collision::*;
pub use model::*;
pub use player::*;
