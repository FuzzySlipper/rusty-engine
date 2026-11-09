//! Posing rig-c parts rigs: 2D rigs of bones, slots and attachments that
//! [rig-c](https://github.com/rig-c/core) compiles into
//! `skeleton.model.json` (`rigc-compiled/3`) beside the pages they draw
//! from.
//!
//! This is a port of rig-c's MIT posing core (`src/core/`; its notice is
//! `LICENSE-rig-c`). It reads the model document and its `pages` only. [`Rig::from_json`] reads a rig once;
//! a [`Player`] plays one clip at a time on it, crossfading from the clip
//! before; and each [`Player::advance`] gives a [`Pose`]: every bone's world
//! transform, and the drawn slots in draw order with their vertices,
//! colour, blend mode and page. What each slot draws with (its page UVs and
//! triangles) is the rig's, in [`Rig::drawable`].
//!
//! IK, transform and physics constraints are applied in the document's
//! order after the bones' world transforms. Physics carries its state from
//! step to step on the player, which takes the rig's own motion
//! ([`Player::translate_physics`]) and the wind and gravity it answers
//! ([`Player::set_physics_environment`]). Path and slider constraints and
//! clipping are not evaluated yet, and a rig that declares any of them is
//! refused by name.

mod constraints;
mod document;
mod mixer;
mod physics;
mod pose;
mod rig;
mod timeline;
mod world;

pub use pose::{BoneChannel, FiredEvent, Piece, Play, Player, Pose};
pub use rig::{BlendMode, Drawable, Page, Rig, RigError, MODEL_SPEC};
pub use world::BoneWorld;
