use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::fingerprint::fingerprint_hex;
use crate::{AssetGuid, GeneratedArtifact, ImportCode, ImportDiagnostic, IMPORTER_VERSION};

pub const IMPORT_MANIFEST_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ArtifactFingerprint {
    pub relative_path: String,
    pub content_hash: String,
    pub byte_len: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportManifest {
    pub schema_version: u32,
    pub source_uri: String,
    pub source_hash: String,
    pub source_schema_version: u32,
    pub importer_version: u32,
    pub mesh_asset_id: String,
    pub guid: Option<AssetGuid>,
    pub artifacts: Vec<ArtifactFingerprint>,
}

impl ImportManifest {
    pub fn artifact_hash(&self, path: &str) -> Option<&str> {
        self.artifacts
            .iter()
            .find(|artifact| artifact.relative_path == path)
            .map(|artifact| artifact.content_hash.as_str())
    }

    pub fn canonical(&self) -> Self {
        let mut manifest = self.clone();
        manifest.artifacts.sort_by(|left, right| {
            left.relative_path
                .cmp(&right.relative_path)
                .then_with(|| left.content_hash.cmp(&right.content_hash))
        });
        manifest
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportManifestCodecError {
    pub path: String,
    pub message: String,
}

impl ImportManifestCodecError {
    fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ImportManifestCodecError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.path, self.message)
    }
}

impl std::error::Error for ImportManifestCodecError {}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct StoredImportManifest {
    schema_version: u32,
    source_uri: String,
    source_hash: String,
    source_schema_version: u32,
    importer_version: u32,
    mesh_asset_id: String,
    guid: Option<String>,
    artifacts: Vec<ArtifactFingerprint>,
}

pub fn build_manifest(
    source_uri: impl Into<String>,
    source_bytes: &[u8],
    source_schema_version: u32,
    mesh_asset_id: impl Into<String>,
    guid: Option<AssetGuid>,
    artifacts: &[GeneratedArtifact],
) -> ImportManifest {
    build_manifest_with_source_hash(
        source_uri,
        fingerprint_hex(source_bytes),
        source_schema_version,
        mesh_asset_id,
        guid,
        artifacts,
    )
}

pub fn build_manifest_with_source_hash(
    source_uri: impl Into<String>,
    source_hash: impl Into<String>,
    source_schema_version: u32,
    mesh_asset_id: impl Into<String>,
    guid: Option<AssetGuid>,
    artifacts: &[GeneratedArtifact],
) -> ImportManifest {
    let mut fingerprints: Vec<_> = artifacts
        .iter()
        .map(|artifact| ArtifactFingerprint {
            relative_path: artifact.relative_path.clone(),
            content_hash: artifact
                .sha256
                .clone()
                .unwrap_or_else(|| fingerprint_hex(&artifact.bytes)),
            byte_len: artifact.bytes.len() as u64,
        })
        .collect();
    fingerprints.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    ImportManifest {
        schema_version: IMPORT_MANIFEST_SCHEMA_VERSION,
        source_uri: source_uri.into(),
        source_hash: source_hash.into(),
        source_schema_version,
        importer_version: IMPORTER_VERSION,
        mesh_asset_id: mesh_asset_id.into(),
        guid,
        artifacts: fingerprints,
    }
}

pub fn encode_import_manifest(manifest: &ImportManifest) -> String {
    let canonical = manifest.canonical();
    let stored = StoredImportManifest {
        schema_version: canonical.schema_version,
        source_uri: canonical.source_uri,
        source_hash: canonical.source_hash,
        source_schema_version: canonical.source_schema_version,
        importer_version: canonical.importer_version,
        mesh_asset_id: canonical.mesh_asset_id,
        guid: canonical.guid.map(|guid| guid.as_str().to_owned()),
        artifacts: canonical.artifacts,
    };
    let mut encoded = serde_json::to_string_pretty(&stored).expect("import manifest serializes");
    encoded.push('\n');
    encoded
}

/// Reads a previous import's manifest. Beyond its typed GUID only the schema
/// version is checked: the manifest is the Engine's own record, read back to
/// classify a reimport, and nothing touches storage by its paths.
pub fn decode_import_manifest(input: &str) -> Result<ImportManifest, ImportManifestCodecError> {
    let mut deserializer = serde_json::Deserializer::from_str(input);
    let stored: StoredImportManifest = serde_path_to_error::deserialize(&mut deserializer)
        .map_err(|error| {
            let path = error.path().to_string();
            ImportManifestCodecError::new(
                if path.is_empty() { "$" } else { path.as_str() },
                error.inner().to_string(),
            )
        })?;
    deserializer
        .end()
        .map_err(|error| ImportManifestCodecError::new("$", error.to_string()))?;
    if stored.schema_version != IMPORT_MANIFEST_SCHEMA_VERSION {
        return Err(ImportManifestCodecError::new(
            "schemaVersion",
            format!(
                "unsupported import manifest schema {}",
                stored.schema_version
            ),
        ));
    }
    let guid = stored
        .guid
        .map(|guid| {
            AssetGuid::parse(&guid)
                .ok_or_else(|| ImportManifestCodecError::new("guid", "invalid asset GUID"))
        })
        .transpose()?;
    let manifest = ImportManifest {
        schema_version: stored.schema_version,
        source_uri: stored.source_uri,
        source_hash: stored.source_hash,
        source_schema_version: stored.source_schema_version,
        importer_version: stored.importer_version,
        mesh_asset_id: stored.mesh_asset_id,
        guid,
        artifacts: stored.artifacts,
    };
    Ok(manifest.canonical())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReimportPlan {
    Noop,
    VisualUpdate {
        changed: Vec<String>,
    },
    StructuralReload {
        reason: String,
        changed: Vec<String>,
    },
}

impl ReimportPlan {
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Noop => "noop",
            Self::VisualUpdate { .. } => "visualUpdate",
            Self::StructuralReload { .. } => "structuralReload",
        }
    }
}

pub fn plan_reimport(prior: &ImportManifest, next: &ImportManifest) -> ReimportPlan {
    let changed = changed_artifacts(prior, next);
    if prior.importer_version != next.importer_version
        || prior.source_schema_version != next.source_schema_version
        || prior.mesh_asset_id != next.mesh_asset_id
    {
        return ReimportPlan::StructuralReload {
            reason: "importer, source schema, or mesh identity changed".to_owned(),
            changed,
        };
    }
    if changed.is_empty() && prior.source_hash == next.source_hash {
        return ReimportPlan::Noop;
    }
    if changed.iter().any(|path| {
        path.ends_with(".static-mesh.json")
            || path.ends_with(".animated-mesh.json")
            || path.ends_with(".glb")
    }) {
        ReimportPlan::StructuralReload {
            reason:
                "retained geometry, materials, animation, resources, or collision structure changed"
                    .to_owned(),
            changed,
        }
    } else {
        ReimportPlan::VisualUpdate { changed }
    }
}

fn changed_artifacts(prior: &ImportManifest, next: &ImportManifest) -> Vec<String> {
    let old: BTreeMap<_, _> = prior
        .artifacts
        .iter()
        .map(|artifact| {
            (
                artifact.relative_path.as_str(),
                artifact.content_hash.as_str(),
            )
        })
        .collect();
    let new: BTreeMap<_, _> = next
        .artifacts
        .iter()
        .map(|artifact| {
            (
                artifact.relative_path.as_str(),
                artifact.content_hash.as_str(),
            )
        })
        .collect();
    old.keys()
        .chain(new.keys())
        .filter(|path| old.get(**path) != new.get(**path))
        .map(|path| (*path).to_owned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub fn detect_source_drift(
    locked_hash: &str,
    current_hash: &str,
    mesh_asset_id: &str,
) -> Option<ImportDiagnostic> {
    (locked_hash != current_hash).then(|| {
        ImportDiagnostic::warning(
            ImportCode::SourceFingerprintChanged,
            mesh_asset_id,
            format!("source hash changed {locked_hash} -> {current_hash}"),
            "review the reimport plan before updating the asset lock",
        )
    })
}
