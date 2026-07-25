//! Deterministic, offline source-asset import for Rusty Engine content.
//!
//! Import produces ordinary catalog and renderer assets. No runtime importer,
//! project session, browser bridge, or replay mechanism is involved.

#![forbid(unsafe_code)]

mod artifact;
mod diagnostic;
mod fingerprint;
mod importer;
mod manifest;
mod plan;
mod publish;
mod sidecar;
mod source;

pub use artifact::{render_artifacts, ArtifactRenderError, GeneratedArtifact};
pub use diagnostic::{ImportCode, ImportDiagnostic, ImportSeverity};
pub use importer::{import, import_with_context, ImportContext, ImportOutcome, ImportedAssets};
pub use manifest::{
    build_manifest, decode_import_manifest, detect_source_drift, encode_import_manifest,
    plan_reimport, ArtifactFingerprint, ImportManifest, ImportManifestCodecError, ReimportPlan,
};
pub use plan::{plan_import, ImportMode, ImportPlan};
pub use publish::{
    publish_directory_atomically, publish_directory_with_sidecar_atomically, PublicationError,
    PublicationReceipt,
};
pub use sidecar::{
    decode_sidecar, detect_duplicate_guids, encode_sidecar, init_metadata, reconcile, sidecar_path,
    AssetGuid, ImportSettings, ProjectOverride, SidecarCodecError, SidecarMetadata,
    SidecarOverrideError, SidecarStatus, SourceUri, SIDECAR_SCHEMA_VERSION,
};
pub use source::{
    parse_source, SourceCollision, SourceGroup, SourceMaterial, SourceMesh, SourceParse,
    MAX_SOURCE_BYTES, MAX_SOURCE_INDICES, MAX_SOURCE_VERTICES, SUPPORTED_SOURCE_SCHEMA,
};

pub const IMPORTER_VERSION: u32 = 1;

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
