use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::ContentHash;

pub const CONTENT_MANIFEST_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ArtifactClass {
    Durable,
    Generated,
    Cache,
}

impl ArtifactClass {
    pub const fn tag(self) -> &'static str {
        match self {
            Self::Durable => "durable",
            Self::Generated => "generated",
            Self::Cache => "cache",
        }
    }

    pub const fn is_load_required(self) -> bool {
        !matches!(self, Self::Cache)
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "durable" => Some(Self::Durable),
            "generated" => Some(Self::Generated),
            "cache" => Some(Self::Cache),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ArtifactRole {
    AssetCatalog,
    AssetLock,
    SceneDocument,
    PrefabRegistry,
    EntityStateSnapshot,
    VoxelAsset,
    VoxelObject,
    VoxelAnnotation,
    ImportedAsset,
    GeneratedMetadata,
    Resource(String),
    Cache,
}

impl ArtifactRole {
    pub fn tag(&self) -> &str {
        match self {
            Self::AssetCatalog => "assetCatalog",
            Self::AssetLock => "assetLock",
            Self::SceneDocument => "sceneDocument",
            Self::PrefabRegistry => "prefabRegistry",
            Self::EntityStateSnapshot => "entityStateSnapshot",
            Self::VoxelAsset => "voxelAsset",
            Self::VoxelObject => "voxelObject",
            Self::VoxelAnnotation => "voxelAnnotation",
            Self::ImportedAsset => "importedAsset",
            Self::GeneratedMetadata => "generatedMetadata",
            Self::Resource(value) => value,
            Self::Cache => "cache",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "assetCatalog" => Self::AssetCatalog,
            "assetLock" => Self::AssetLock,
            "sceneDocument" => Self::SceneDocument,
            "prefabRegistry" => Self::PrefabRegistry,
            "entityStateSnapshot" => Self::EntityStateSnapshot,
            "voxelAsset" => Self::VoxelAsset,
            "voxelObject" => Self::VoxelObject,
            "voxelAnnotation" => Self::VoxelAnnotation,
            "importedAsset" => Self::ImportedAsset,
            "generatedMetadata" => Self::GeneratedMetadata,
            "cache" => Self::Cache,
            resource if valid_resource_role(resource) => Self::Resource(resource.to_string()),
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentArtifact {
    pub path: String,
    pub class: ArtifactClass,
    pub role: ArtifactRole,
    pub content_hash: Option<ContentHash>,
    pub byte_len: Option<u64>,
}

impl ContentArtifact {
    pub fn durable(path: impl Into<String>, role: ArtifactRole, bytes: &[u8]) -> Self {
        Self::stored(path, ArtifactClass::Durable, role, bytes)
    }

    pub fn generated(path: impl Into<String>, role: ArtifactRole, bytes: &[u8]) -> Self {
        Self::stored(path, ArtifactClass::Generated, role, bytes)
    }

    pub fn cache(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            class: ArtifactClass::Cache,
            role: ArtifactRole::Cache,
            content_hash: None,
            byte_len: None,
        }
    }

    fn stored(
        path: impl Into<String>,
        class: ArtifactClass,
        role: ArtifactRole,
        bytes: &[u8],
    ) -> Self {
        Self {
            path: path.into(),
            class,
            role,
            content_hash: Some(ContentHash::of(bytes)),
            byte_len: Some(bytes.len() as u64),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentManifest {
    pub schema_version: u32,
    pub artifacts: Vec<ContentArtifact>,
}

impl ContentManifest {
    pub fn new(artifacts: Vec<ContentArtifact>) -> Self {
        Self {
            schema_version: CONTENT_MANIFEST_SCHEMA_VERSION,
            artifacts,
        }
    }

    pub fn canonical(&self) -> Self {
        let mut manifest = self.clone();
        manifest.schema_version = CONTENT_MANIFEST_SCHEMA_VERSION;
        manifest.artifacts.sort_by(|left, right| {
            left.path
                .cmp(&right.path)
                .then_with(|| left.role.cmp(&right.role))
        });
        manifest
    }

    pub fn artifact(&self, path: &str) -> Option<&ContentArtifact> {
        self.artifacts.iter().find(|artifact| artifact.path == path)
    }

    pub fn load_required(&self) -> impl Iterator<Item = &ContentArtifact> {
        self.artifacts
            .iter()
            .filter(|artifact| artifact.class.is_load_required())
    }

    pub fn validate(&self) -> Result<(), ManifestError> {
        if self.schema_version != CONTENT_MANIFEST_SCHEMA_VERSION {
            return Err(ManifestError::UnsupportedSchema {
                found: self.schema_version,
                supported: CONTENT_MANIFEST_SCHEMA_VERSION,
            });
        }
        let mut paths = BTreeSet::new();
        for artifact in &self.artifacts {
            if !is_safe_relative_path(&artifact.path) {
                return Err(ManifestError::InvalidPath {
                    path: artifact.path.clone(),
                });
            }
            if !paths.insert(artifact.path.as_str()) {
                return Err(ManifestError::DuplicatePath {
                    path: artifact.path.clone(),
                });
            }
            if artifact.class.is_load_required()
                && (artifact.content_hash.is_none() || artifact.byte_len.is_none())
            {
                return Err(ManifestError::StoredArtifactMissingIdentity {
                    path: artifact.path.clone(),
                });
            }
            if artifact.class == ArtifactClass::Cache
                && (!matches!(artifact.role, ArtifactRole::Cache)
                    || artifact.content_hash.is_some()
                    || artifact.byte_len.is_some())
            {
                return Err(ManifestError::InvalidCacheMetadata {
                    path: artifact.path.clone(),
                });
            }
            if matches!(artifact.role, ArtifactRole::Cache)
                && artifact.class != ArtifactClass::Cache
            {
                return Err(ManifestError::InvalidCacheMetadata {
                    path: artifact.path.clone(),
                });
            }
            if matches!(&artifact.role, ArtifactRole::Resource(role) if !valid_resource_role(role))
            {
                return Err(ManifestError::InvalidRole {
                    path: artifact.path.clone(),
                });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestError {
    UnsupportedSchema { found: u32, supported: u32 },
    InvalidPath { path: String },
    DuplicatePath { path: String },
    StoredArtifactMissingIdentity { path: String },
    InvalidCacheMetadata { path: String },
    InvalidRole { path: String },
}

impl std::fmt::Display for ManifestError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid content manifest: {self:?}")
    }
}

impl std::error::Error for ManifestError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestCodecError {
    pub path: String,
    pub message: String,
}

impl ManifestCodecError {
    fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ManifestCodecError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.path, self.message)
    }
}

impl std::error::Error for ManifestCodecError {}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct StoredManifest {
    schema_version: u32,
    artifacts: Vec<StoredArtifact>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct StoredArtifact {
    path: String,
    class: String,
    role: String,
    content_hash: Option<String>,
    byte_len: Option<u64>,
}

pub fn encode_manifest(manifest: &ContentManifest) -> Result<String, ManifestCodecError> {
    manifest
        .validate()
        .map_err(|error| ManifestCodecError::new("$", error.to_string()))?;
    let canonical = manifest.canonical();
    let stored = StoredManifest {
        schema_version: canonical.schema_version,
        artifacts: canonical
            .artifacts
            .into_iter()
            .map(|artifact| StoredArtifact {
                path: artifact.path,
                class: artifact.class.tag().to_string(),
                role: artifact.role.tag().to_string(),
                content_hash: artifact.content_hash.map(ContentHash::to_hex),
                byte_len: artifact.byte_len,
            })
            .collect(),
    };
    let mut encoded = serde_json::to_string_pretty(&stored)
        .map_err(|error| ManifestCodecError::new("$", error.to_string()))?;
    encoded.push('\n');
    Ok(encoded)
}

pub fn decode_manifest(input: &str) -> Result<ContentManifest, ManifestCodecError> {
    let manifest = decode_manifest_unvalidated(input)?;
    manifest
        .validate()
        .map_err(|error| ManifestCodecError::new("$", error.to_string()))?;
    Ok(manifest.canonical())
}

/// Decode the strict stored shape while retaining semantic manifest errors for
/// read-only authoring diagnostics. Content admission must use
/// [`decode_manifest`], which validates before returning.
pub fn decode_manifest_unvalidated(input: &str) -> Result<ContentManifest, ManifestCodecError> {
    let mut deserializer = serde_json::Deserializer::from_str(input);
    let stored: StoredManifest =
        serde_path_to_error::deserialize(&mut deserializer).map_err(|error| {
            let path = error.path().to_string();
            ManifestCodecError::new(
                if path.is_empty() { "$" } else { path.as_str() },
                error.inner().to_string(),
            )
        })?;
    deserializer
        .end()
        .map_err(|error| ManifestCodecError::new("$", error.to_string()))?;
    let artifacts = stored
        .artifacts
        .into_iter()
        .enumerate()
        .map(|(index, artifact)| {
            let class = ArtifactClass::parse(&artifact.class).ok_or_else(|| {
                ManifestCodecError::new(
                    format!("artifacts[{index}].class"),
                    format!("unknown artifact class {}", artifact.class),
                )
            })?;
            let role = ArtifactRole::parse(&artifact.role).ok_or_else(|| {
                ManifestCodecError::new(
                    format!("artifacts[{index}].role"),
                    format!("unknown artifact role {}", artifact.role),
                )
            })?;
            let content_hash = artifact
                .content_hash
                .map(|hash| {
                    ContentHash::parse(&hash).map_err(|error| {
                        ManifestCodecError::new(
                            format!("artifacts[{index}].contentHash"),
                            error.to_string(),
                        )
                    })
                })
                .transpose()?;
            Ok(ContentArtifact {
                path: artifact.path,
                class,
                role,
                content_hash,
                byte_len: artifact.byte_len,
            })
        })
        .collect::<Result<_, ManifestCodecError>>()?;
    Ok(ContentManifest {
        schema_version: stored.schema_version,
        artifacts,
    })
}

pub fn is_safe_relative_path(path: &str) -> bool {
    !path.is_empty()
        && path.trim() == path
        && !path.starts_with('/')
        && !path.contains('\\')
        && !path.contains('\0')
        && path
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}

fn valid_resource_role(value: &str) -> bool {
    value.strip_prefix("resource:").is_some_and(|kind| {
        !kind.is_empty()
            && kind
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    })
}
