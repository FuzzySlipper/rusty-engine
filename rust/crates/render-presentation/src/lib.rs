//! The retained world and the presentation effects the renderer realizes.
//!
//! `world` owns the canonical retained graphics graph: projectors apply typed
//! render changes to it and the renderer reads its deltas. The effect
//! projectors (animation, audio, billboard, particle, ghost plate and video)
//! validate typed presentation intent, retain only the state realization
//! needs, and emit `PresentationFrameDiff` (`frame`); `asset_view` resolves
//! the assets they name. The crate owns no renderer objects, gameplay
//! authority, project catalog, filesystem access, or persistence.

#![forbid(unsafe_code)]

mod animation;
mod asset_view;
mod audio;
mod billboard;
mod frame;
mod ghost_plate;
mod particle;
mod video;
mod world;

pub use animation::*;
pub use asset_view::*;
pub use audio::*;
pub use billboard::*;
pub use frame::*;
pub use ghost_plate::*;
pub use particle::*;
pub use video::*;
pub use world::*;
