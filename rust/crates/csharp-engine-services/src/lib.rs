//! Concrete Engine capability adapters behind the trusted NativeAOT ABI.

mod appearance;
mod audio;
mod authored_content;
mod camera_view;
mod composition;
mod content;
mod diagnostics;
mod dynamics;
mod gameplay_time;
mod http;
mod implicit_surfaces;
mod input;
mod kinematic;
mod magica_vox;
mod motion;
mod operation_diagnostics;
mod perception;
mod persistence;
mod presentation;
mod render_output;
mod render_resources;
mod rng;
mod spatial;
mod ui;
mod video;
mod voxel;
mod voxel_content;
mod voxel_scene_presentation;
mod world_origin;

pub use appearance::{AnimationRealizationFact, GhostPlateRealizationFact};
pub use audio::AudioRealizationFact;
pub use composition::{
    parse_runtime_appearance_catalog, CsharpAppearanceCallOutput, CsharpAppearanceCatalog,
    CsharpEngineCallOutput, CsharpEngineServicesError, EngineServiceSet,
};
pub use gameplay_time::{rate_value as gameplay_rate_value, GameplayTimeRequest};
pub use render_output::RenderOutputWork;
pub use render_resources::{CsharpRenderResource, CsharpRenderResourceKind};
pub use video::VideoRealizationFact;

pub use content::ProductContentBundles;
pub use ui::{UiFile, UiFiles};
