//! Concrete Engine capability adapters behind the trusted NativeAOT ABI.

mod appearance;
mod audio;
mod authored_content;
mod camera_view;
mod collision_inspection;
mod composition;
mod content;
mod content_source;
mod diagnostics;
mod dynamics;
mod gameplay_time;
mod heightfield;
mod host;
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
mod renderer_settings;
mod rng;
mod session;
mod spatial;
mod tween;
mod ui;
mod vegetation;
mod video;
mod voxel;
mod voxel_content;
mod voxel_scene_presentation;
mod water_field;
mod world_origin;

pub use appearance::{
    AnimationRealizationFact, GhostPlateRealizationFact, LandmarkPairDrawn, LandmarkPairReport,
    LandmarkSetReport,
};
pub use audio::AudioRealizationFact;
pub use composition::{
    parse_runtime_appearance_catalog, CsharpAppearanceCallOutput, CsharpAppearanceCatalog,
    CsharpEngineCallOutput, CsharpEngineServicesError, EngineServiceSet,
};
pub use gameplay_time::{rate_value as gameplay_rate_value, GameplayTimeRequest};
pub use render_output::RenderOutputWork;
pub use render_resources::{CsharpRenderResource, CsharpRenderResourceKind, RenderOutputResources};
pub use renderer_settings::renderer_settings_request;
pub use tween::TweenTime;
pub use video::VideoRealizationFact;

pub use collision_inspection::{
    CollisionInspection, CollisionLine, CollisionLineKind, CollisionSource, InspectedCollision,
    COLLISION_INSPECTION_LINES,
};
pub use content::{ContentBytes, ProductContentBundles};
pub use content_source::{
    parse_edits as parse_content_source_edits, ContentSource, ContentSourceRoots, JsonEdit,
};
pub use ui::{UiFile, UiFiles};
