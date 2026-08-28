//! Concrete Engine capability adapters behind the trusted NativeAOT ABI.

mod appearance;
mod audio;
mod camera_view;
mod composition;
mod content;
mod content_store;
mod dynamics;
mod look;
mod mechanics;
mod motion;
mod persistence;
mod presentation;
mod resolution;
mod rng;
mod rules;
mod spatial;
mod standard_continuous;
mod standard_exact;
mod state_machine;
mod ui;
mod voxel;
mod voxel_content;
mod world_origin;

pub use appearance::{CsharpRenderResource, CsharpRenderResourceKind};
pub use composition::{
    parse_runtime_appearance_catalog, CsharpAppearanceCatalog, CsharpEngineCallOutput,
    CsharpEngineServicesError, EngineServiceSet,
};
