//! Read-only inspection of Rusty Engine-owned content and live state.
//!
//! This crate may see across content owners because it is a leaf tool. Runtime
//! crates never depend on it, and every API accepts immutable references or
//! stored text. It reports what an owner already knows; it does not repair,
//! authorize, replay, or become a second source of truth.

#![forbid(unsafe_code)]

mod catalog;
mod diagnostic;
mod entity;
mod imports;
mod persistence;
mod scene;
mod voxel;

pub use catalog::{
    inspect_catalog, inspect_catalog_json, CatalogInspection, CatalogLockInspection, NamedCount,
};
pub use diagnostic::{
    Diagnostic, DiagnosticDomain, DiagnosticLocation, DiagnosticSet, DiagnosticSeverity, Remedy,
    RemedyAction,
};
pub use entity::{
    entity_ids_in_category, inspect_entity, inspect_entity_state, inspect_entity_state_json,
    EntityCategory, EntityInspection, EntityStateInspection,
};
pub use imports::{
    inspect_import_manifest, inspect_import_manifest_json, inspect_import_source,
    ImportManifestInspection, ImportSourceInspection,
};
pub use persistence::{
    inspect_content_manifest, inspect_content_manifest_json, ContentLoadStepInspection,
    PersistenceInspection,
};
pub use scene::{inspect_scene, inspect_scene_json, SceneInspection};
pub use voxel::{
    describe_voxel_edit_rejection, inspect_voxel_asset, inspect_voxel_asset_json,
    inspect_voxel_state, VoxelAssetInspection, VoxelChunkInspection, VoxelMaterialCount,
    VoxelStateInspection,
};
