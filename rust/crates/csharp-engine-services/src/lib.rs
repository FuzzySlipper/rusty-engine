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
mod spatial;
mod ui;

pub use appearance::{CsharpRenderResource, CsharpRenderResourceKind};
pub use composition::{
    parse_runtime_appearance_catalog, CsharpAppearanceCatalog, CsharpEngineCallOutput,
    CsharpEngineServicesError, EngineServiceSet,
};
