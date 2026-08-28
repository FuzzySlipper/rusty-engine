//! Concrete Engine capability adapters behind the trusted NativeAOT ABI.

mod appearance;
mod audio;
mod camera_view;
mod composition;
mod dynamics;
mod look;
mod mechanics;
mod persistence;
mod rng;
mod rules;
mod spatial;
mod standard_continuous;
mod standard_exact;
mod ui;
mod voxel;
mod voxel_content;

pub use appearance::{CsharpRenderResource, CsharpRenderResourceKind};
pub use composition::{
    parse_runtime_appearance_catalog, CsharpAppearanceCatalog, CsharpEngineCallOutput,
    CsharpEngineServicesError, EngineServiceSet,
};
