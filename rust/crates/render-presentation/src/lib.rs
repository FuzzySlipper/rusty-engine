//! Bounded, disposable animation, audio, billboard, particle, and telemetry
//! presentation mechanisms.
//!
//! This crate validates typed presentation intent and retains only the state a
//! host needs to realize it. It owns no renderer objects, gameplay authority,
//! project catalog, filesystem access, or persistence.

#![forbid(unsafe_code)]

mod animation;
mod asset_view;
mod audio;
mod billboard;
mod frame;
mod ghost_plate;
mod particle;
mod projector;
mod telemetry;
mod world;

pub use animation::*;
pub use asset_view::*;
pub use audio::*;
pub use billboard::*;
pub use frame::*;
pub use ghost_plate::*;
pub use particle::*;
pub use projector::*;
pub use telemetry::*;
pub use world::*;
