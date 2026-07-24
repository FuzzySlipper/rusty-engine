//! Bounded offline conversion from one static GLB mesh to a durable voxel asset.
//!
//! This crate is an authoring/build tool. It has no dependency on `game-host`
//! and is never invoked while admitting or running a project.

#![forbid(unsafe_code)]

mod convert;
mod diagnostic;
mod import;
mod store;

pub use convert::{convert_glb, ConversionReceipt, CONVERTER_ID, MAX_SURFACE_SAMPLE_WORK};
pub use diagnostic::{ConversionDiagnostic, ConversionError};
pub use import::{import_static_glb, ImportedMaterial, ImportedStaticMesh, ImportedTriangle};
pub use store::{convert_and_install, decode_conversion_request, MAX_CONVERSION_REQUEST_BYTES};
pub use voxel_asset::MAX_CONVERSION_SOURCE_BYTES;
