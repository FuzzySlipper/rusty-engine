//! Renderer-neutral retained scene vocabulary.
//!
//! This crate is the stable border between canonical retained data/projection
//! and renderer hosts. It owns no renderer objects, filesystem access, catalog,
//! runtime session, or replay behavior.

#![forbid(unsafe_code)]

mod assets;
mod audio_resource;
mod core;
mod lighting;
mod mesh;
mod mesh_partition;
mod mesh_resource;
mod pose;
mod scatter;
mod settings_options;
mod voxel_object;

pub use assets::*;
pub use audio_resource::*;
pub use core::*;
pub use lighting::*;
pub use mesh::*;
pub use mesh_partition::*;
pub use mesh_resource::*;
pub use pose::*;
pub use scatter::*;
pub use settings_options::*;
pub use voxel_object::*;

mod irradiance;
pub use irradiance::*;
