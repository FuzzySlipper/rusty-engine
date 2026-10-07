//! Deterministic retained projections over explicit object and spatial views.

#![forbid(unsafe_code)]

mod appearance;
mod material;
mod retained;
mod runtime_appearance;
mod voxel;
mod voxel_object;
mod voxel_scatter;

pub use appearance::*;
pub use material::*;
pub use retained::*;
pub use runtime_appearance::*;
pub use voxel::*;
pub use voxel_object::*;
pub use voxel_scatter::{
    VoxelScatter, VoxelScatterError, VoxelScatterField, VoxelScatterReadout, MAX_SCATTER_DENSITY,
    SCATTER_HYSTERESIS,
};
