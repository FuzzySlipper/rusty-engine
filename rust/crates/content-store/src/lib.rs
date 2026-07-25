//! Host-neutral content persistence, prefab authoring, and atomic publication plans.
//!
//! This crate describes bytes and safe relative paths. It does not own a
//! project-wide runtime, filesystem, replay topology, or host bridge.

#![forbid(unsafe_code)]

mod batch;
mod hash;
mod manifest;
mod owner_codec;
mod plan;
mod prefab;
mod prefab_codec;
mod prefab_resolution;
mod write_set;

pub use batch::{
    admit_source_batch, AdmittedContentBatch, ContentBody, ContentSourceBatch, ContentSourceError,
    ContentSourceErrorCode, CONTENT_BODY_MAX_BYTES, CONTENT_MANIFEST_MAX_BYTES, CONTENT_MAX_BODIES,
    CONTENT_TOTAL_MAX_BYTES,
};
pub use hash::{ContentHash, ContentHashError};
pub use manifest::{
    decode_manifest, encode_manifest, is_safe_relative_path, ArtifactClass, ArtifactRole,
    ContentArtifact, ContentManifest, ManifestCodecError, ManifestError,
    CONTENT_MANIFEST_SCHEMA_VERSION,
};
pub use owner_codec::{
    asset_catalog_body, asset_lock_body, durable_entity_state_body, prefab_registry_body,
    scene_document_body, voxel_annotation_body, voxel_asset_body, OwnerCodecError,
};
pub use plan::{ContentLoadPlan, ContentLoadStage, ContentLoadStep, ContentSavePlan};
pub use prefab::{
    validate_prefab_registry, PrefabDefinition, PrefabDiagnostic, PrefabDiagnosticCode,
    PrefabInstanceRecord, PrefabOverride, PrefabOverrideValue, PrefabPart, PrefabPartReference,
    PrefabPartRoleBinding, PrefabPartSource, PrefabRegistry, PrefabRegistryValidationContext,
    PrefabTransform, PrefabValidationReport, PrefabVariantDelta, ValidatedPrefabRegistry,
    PREFAB_DEFINITION_SCHEMA_VERSION, PREFAB_REGISTRY_SCHEMA_VERSION,
};
pub use prefab_codec::{decode_prefab_registry, encode_prefab_registry, PrefabCodecError};
pub use prefab_resolution::{
    resolve_prefab, PrefabResolutionError, ResolvedPrefab, ResolvedPrefabPart,
};
pub use write_set::{
    AuthorizedContentWriteCandidate, ContentDelete, ContentMove, ContentStoreIdentity,
    ContentWrite, ContentWriteCandidate, ContentWriteConfirmation, ContentWriteSetDraft,
    ContentWriteSetError, CONTENT_MANIFEST_PATH,
};
