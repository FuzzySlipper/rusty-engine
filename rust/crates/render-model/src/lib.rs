//! Renderer-neutral retained scene vocabulary.
//!
//! This crate is the stable border between Rust-owned authority/projection and
//! renderer hosts. It owns no renderer objects, filesystem access, catalog,
//! runtime session, or replay behavior.

#![forbid(unsafe_code)]

mod assets;
mod core;
mod editor_grid;
mod lighting;
mod mesh;
mod voxel_object;

pub use assets::*;
pub use core::*;
pub use editor_grid::*;
pub use lighting::*;
pub use mesh::*;
pub use voxel_object::*;
