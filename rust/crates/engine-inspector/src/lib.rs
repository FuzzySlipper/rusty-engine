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
mod mechanics;
mod persistence;
mod scene;
mod standard;
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
pub use mechanics::{
    inspect_damage_receipt, inspect_mechanics_entity_from_evidence,
    inspect_mechanics_entity_structural, inspect_mechanics_snapshot_json_v1_from_evidence,
    inspect_mechanics_snapshot_json_with_evidence, inspect_mechanics_snapshot_structural_json_v2,
    inspect_stat_evaluations, DamageReceiptDecisionInspection, DamageReceiptFactInspection,
    DamageReceiptInspection, DamageReceiptPartInspection, DamageReceiptTrackInspection,
    MechanicsCapacityInspection, MechanicsComponentInspection, MechanicsEffectInspection,
    MechanicsEnrichedInventoryInspection, MechanicsEntityInspection,
    MechanicsEquipmentAssignmentInspection, MechanicsEvaluatedStatInspection,
    MechanicsEvaluationReadoutInspection, MechanicsInspectionEvidence,
    MechanicsInventoryCostInspection, MechanicsInventoryInspection,
    MechanicsInventoryItemInspection, MechanicsItemInspection,
    MechanicsObservedComponentRevisionInspection, MechanicsSourceActivationInspection,
    MechanicsSourceBindingInspection, MechanicsSourceCostInspection,
    MechanicsStatDecisionInspection, MechanicsStatInspection, MechanicsStoredStatInspection,
    MechanicsStoredTrackInspection, MechanicsStructuralEntityInspection, MechanicsTrackInspection,
    MechanicsTrackMaximumInspection,
};
pub use persistence::{
    inspect_content_manifest, inspect_content_manifest_json, ContentLoadStepInspection,
    PersistenceInspection,
};
pub use scene::{inspect_scene, inspect_scene_json, SceneInspection};
pub use standard::{
    inspect_standard_borrowed_evidence, inspect_standard_plan,
    inspect_standard_plan_with_explanation, inspect_standard_plan_with_readouts,
    OptionalStandardResolutionProjection, StandardBorrowedEvidence, StandardBorrowedEvidenceParts,
    StandardInspection, StandardInspectionWithExplanation, StandardResolutionProjection,
};
pub use voxel::{
    describe_voxel_edit_rejection, inspect_voxel_asset, inspect_voxel_asset_json,
    inspect_voxel_state, VoxelAssetInspection, VoxelChunkInspection, VoxelMaterialCount,
    VoxelStateInspection,
};
