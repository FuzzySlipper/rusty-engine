//! Concrete Engine capability adapters behind the trusted NativeAOT ABI.

mod appearance;
mod composition;
mod look;
mod mechanics;
mod rng;
mod spatial;
mod ui;

pub use appearance::{CsharpRenderResource, CsharpRenderResourceKind};
pub use composition::{
    parse_runtime_appearance_catalog, CsharpAppearanceCatalog, CsharpEngineCallOutput,
    CsharpEngineServicesError, EngineServiceSet,
};
