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
mod magica_vox;
mod motion;
mod perception;
mod persistence;
mod presentation;
mod rng;
mod spatial;
mod ui;
mod voxel;
mod voxel_content;
mod voxel_scene_presentation;
mod world_origin;

pub use appearance::{
    AnimationCueDefinition, AnimationRealizationFact, CsharpRenderResource,
    CsharpRenderResourceKind,
};
pub use audio::AudioRealizationFact;
pub use composition::{
    parse_runtime_appearance_catalog, CsharpAppearanceCallOutput, CsharpAppearanceCatalog,
    CsharpEngineCallOutput, CsharpEngineServicesError, EngineServiceSet,
};
