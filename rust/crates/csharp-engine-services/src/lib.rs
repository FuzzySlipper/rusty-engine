//! Concrete Engine capability adapters behind the trusted NativeAOT ABI.

mod appearance;
mod audio;
mod authored_content;
mod camera_view;
mod composition;
mod content;
mod content_store;
mod dynamics;
mod kinematic;
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
pub use audio::AudioRealizationFact;
pub use composition::{
    parse_runtime_appearance_catalog, CsharpAppearanceCallOutput, CsharpAppearanceCatalog,
    CsharpEngineCallOutput, CsharpEngineServicesError, EngineServiceSet,
};
