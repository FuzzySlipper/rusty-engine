//! Deterministic, offline source-asset import for Rusty Engine content.
//!
//! Import produces ordinary catalog and renderer assets. No runtime importer,
//! project session, browser bridge, or replay mechanism is involved.

#![forbid(unsafe_code)]

mod animated_glb;
mod artifact;
mod diagnostic;
mod fingerprint;
mod gltf_package;
mod importer;
mod manifest;
mod plan;
mod publish;
mod sidecar;
mod source;

pub use animated_glb::{
    import_animated_glb_asset, AnimatedGlbImportOutcome, AnimatedGlbImportReceipt,
    GlbAnimationKind, ImportedAnimatedGlb, MAX_ANIMATED_GLB_EMBEDDED_IMAGE_BYTES,
    MAX_ANIMATED_GLB_EMBEDDED_IMAGE_TOTAL_BYTES, MAX_ANIMATED_GLB_IMAGES, MAX_ANIMATED_GLB_JOINTS,
    MAX_ANIMATED_GLB_MATERIALS, MAX_ANIMATED_GLB_TEXTURES, SUPPORTED_ANIMATED_GLB_VERSION,
};
pub use artifact::{
    render_animated_glb_artifacts, render_artifacts, ArtifactRenderError, GeneratedArtifact,
};
pub use diagnostic::{ImportCode, ImportDiagnostic, ImportSeverity};
pub use gltf_package::{
    admit_glb_source, admit_gltf_source, glb_relative_resource_uris, gltf_relative_resource_uris,
    GlbSourceClosure, GltfResource, GltfSourceClosure, PackedGltfSource, MAX_GLTF_RESOURCE_BYTES,
    MAX_GLTF_RESOURCE_COUNT, MAX_GLTF_TOTAL_RESOURCE_BYTES,
};
pub use importer::{import, import_with_context, ImportContext, ImportOutcome, ImportedAssets};
pub use manifest::{
    build_manifest, build_manifest_with_source_hash, decode_import_manifest, detect_source_drift,
    encode_import_manifest, plan_reimport, validate_import_manifest, ArtifactFingerprint,
    ImportManifest, ImportManifestCodecError, ReimportPlan,
};
pub use plan::{
    plan_animated_glb_import, plan_animated_gltf_import, plan_import, ImportMode, ImportPlan,
};
pub use publish::{
    publish_directory_atomically, publish_directory_with_sidecar_atomically, PublicationError,
    PublicationReceipt,
};
pub use sidecar::{
    decode_sidecar, detect_duplicate_guids, encode_sidecar, init_metadata,
    init_metadata_with_source_hash, reconcile, reconcile_source_hash, sidecar_path, AssetGuid,
    ImportSettings, ProjectOverride, SidecarCodecError, SidecarMetadata, SidecarOverrideError,
    SidecarStatus, SourceUri, SIDECAR_SCHEMA_VERSION,
};
pub use source::{
    parse_source, SourceCollision, SourceGroup, SourceMaterial, SourceMesh, SourceParse,
    MAX_SOURCE_BYTES, MAX_SOURCE_INDICES, MAX_SOURCE_VERTICES, SUPPORTED_SOURCE_SCHEMA,
};

pub const IMPORTER_VERSION: u32 = 2;

pub fn import_text(text: &str, locus: &str, context: &ImportContext) -> ImportOutcome {
    let parsed = parse_source(text, locus);
    let Some(mesh) = parsed.mesh else {
        return ImportOutcome {
            assets: None,
            diagnostics: parsed.diagnostics,
        };
    };
    let mut outcome = import_with_context(&mesh, context);
    let mut diagnostics = parsed.diagnostics;
    diagnostics.append(&mut outcome.diagnostics);
    outcome.diagnostics = diagnostics;
    outcome
}
