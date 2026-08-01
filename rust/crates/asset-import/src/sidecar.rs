use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::fingerprint::fingerprint_hex;
use crate::ArtifactFingerprint;

pub const SIDECAR_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AssetGuid(String);

impl AssetGuid {
    pub fn mint(seed: &str) -> Self {
        let digest = Sha256::digest(format!("rusty-engine-asset-guid:{seed}").as_bytes());
        let mut text = String::with_capacity(32);
        for byte in &digest[..16] {
            use std::fmt::Write;
            let _ = write!(text, "{byte:02x}");
        }
        Self(text)
    }

    pub fn parse(text: &str) -> Option<Self> {
        (text.len() == 32
            && text
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
        .then(|| Self(text.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceUri {
    RelativePath(String),
    AbsolutePath(String),
    FileUrl(String),
    ContentAddressed(String),
}

impl SourceUri {
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::RelativePath(_) => "relativePath",
            Self::AbsolutePath(_) => "absolutePath",
            Self::FileUrl(_) => "fileUrl",
            Self::ContentAddressed(_) => "contentAddressed",
        }
    }

    pub fn value(&self) -> &str {
        match self {
            Self::RelativePath(value)
            | Self::AbsolutePath(value)
            | Self::FileUrl(value)
            | Self::ContentAddressed(value) => value,
        }
    }

    fn is_valid(&self) -> bool {
        match self {
            Self::RelativePath(path) => crate::artifact::is_safe_relative_path(path),
            Self::AbsolutePath(path) => path.starts_with('/') && !path.contains('\0'),
            Self::FileUrl(url) => url.starts_with("file://") && url.len() > "file://".len(),
            Self::ContentAddressed(hash) => {
                hash.len() == 64
                    && hash
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportSettings {
    pub scale: f32,
    pub generate_collision: bool,
    pub material_namespace: Option<String>,
}

impl Default for ImportSettings {
    fn default() -> Self {
        Self {
            scale: 1.0,
            generate_collision: false,
            material_namespace: None,
        }
    }
}

impl ImportSettings {
    pub fn is_valid(&self) -> bool {
        self.scale.is_finite()
            && self.scale > 0.0
            && self
                .material_namespace
                .as_ref()
                .is_none_or(|namespace| is_scoped_key(namespace))
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ProjectOverride {
    pub guid: Option<AssetGuid>,
    pub scale: Option<f32>,
    pub generate_collision: Option<bool>,
    pub material_namespace: Option<Option<String>>,
}

impl ProjectOverride {
    pub fn apply(
        &self,
        guid: &AssetGuid,
        base: &ImportSettings,
    ) -> Result<ImportSettings, SidecarOverrideError> {
        if self.guid.as_ref().is_some_and(|expected| expected != guid) {
            return Err(SidecarOverrideError::WrongGuid);
        }
        let settings = ImportSettings {
            scale: self.scale.unwrap_or(base.scale),
            generate_collision: self.generate_collision.unwrap_or(base.generate_collision),
            material_namespace: self
                .material_namespace
                .clone()
                .unwrap_or_else(|| base.material_namespace.clone()),
        };
        if !settings.is_valid() {
            return Err(SidecarOverrideError::InvalidSettings);
        }
        Ok(settings)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidecarOverrideError {
    WrongGuid,
    InvalidSettings,
}

impl std::fmt::Display for SidecarOverrideError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid project asset override: {self:?}")
    }
}

impl std::error::Error for SidecarOverrideError {}

#[derive(Debug, Clone, PartialEq)]
pub struct SidecarMetadata {
    pub schema_version: u32,
    pub guid: AssetGuid,
    pub source_uri: SourceUri,
    pub source_hash: String,
    pub importer_version: u32,
    pub declared_kind: String,
    pub labels: Vec<String>,
    pub import_settings: ImportSettings,
    pub generated_artifacts: Vec<ArtifactFingerprint>,
}

pub fn sidecar_path(source_path: &str) -> String {
    format!("{source_path}.rusty-meta.json")
}

pub fn init_metadata(
    source_uri: SourceUri,
    source_bytes: &[u8],
    declared_kind: impl Into<String>,
    importer_version: u32,
    settings: ImportSettings,
    uniqueness_salt: &str,
) -> SidecarMetadata {
    init_metadata_with_source_hash(
        source_uri,
        fingerprint_hex(source_bytes),
        declared_kind,
        importer_version,
        settings,
        uniqueness_salt,
    )
}

pub fn init_metadata_with_source_hash(
    source_uri: SourceUri,
    source_hash: String,
    declared_kind: impl Into<String>,
    importer_version: u32,
    settings: ImportSettings,
    uniqueness_salt: &str,
) -> SidecarMetadata {
    let guid = AssetGuid::mint(&format!("{}|{uniqueness_salt}", source_uri.value()));
    SidecarMetadata {
        schema_version: SIDECAR_SCHEMA_VERSION,
        guid,
        source_uri,
        source_hash,
        importer_version,
        declared_kind: declared_kind.into(),
        labels: Vec::new(),
        import_settings: settings,
        generated_artifacts: Vec::new(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SidecarStatus {
    MissingSidecar,
    Unchanged,
    MovedFile { from: String, to: String },
    ContentChanged { from: String, to: String },
}

impl SidecarStatus {
    pub const fn label(&self) -> &'static str {
        match self {
            Self::MissingSidecar => "missingSidecar",
            Self::Unchanged => "unchanged",
            Self::MovedFile { .. } => "movedFile",
            Self::ContentChanged { .. } => "contentChanged",
        }
    }
}

pub fn reconcile(
    prior: Option<&SidecarMetadata>,
    current_uri: &SourceUri,
    current_bytes: &[u8],
) -> SidecarStatus {
    reconcile_source_hash(prior, current_uri, fingerprint_hex(current_bytes))
}

pub fn reconcile_source_hash(
    prior: Option<&SidecarMetadata>,
    current_uri: &SourceUri,
    current_hash: String,
) -> SidecarStatus {
    let Some(prior) = prior else {
        return SidecarStatus::MissingSidecar;
    };
    if prior.source_hash != current_hash {
        SidecarStatus::ContentChanged {
            from: prior.source_hash.clone(),
            to: current_hash,
        }
    } else if &prior.source_uri != current_uri {
        SidecarStatus::MovedFile {
            from: prior.source_uri.value().to_owned(),
            to: current_uri.value().to_owned(),
        }
    } else {
        SidecarStatus::Unchanged
    }
}

pub fn detect_duplicate_guids(sidecars: &[SidecarMetadata]) -> Vec<AssetGuid> {
    let mut seen = BTreeSet::new();
    let mut duplicates = BTreeSet::new();
    for sidecar in sidecars {
        if !seen.insert(&sidecar.guid) {
            duplicates.insert(sidecar.guid.clone());
        }
    }
    duplicates.into_iter().collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidecarCodecError {
    pub path: String,
    pub message: String,
}

impl SidecarCodecError {
    fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for SidecarCodecError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.path, self.message)
    }
}

impl std::error::Error for SidecarCodecError {}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct StoredSidecar {
    schema_version: u32,
    guid: String,
    source_uri: StoredSourceUri,
    source_hash: String,
    importer_version: u32,
    declared_kind: String,
    labels: Vec<String>,
    import_settings: StoredImportSettings,
    generated_artifacts: Vec<ArtifactFingerprint>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct StoredSourceUri {
    kind: String,
    value: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct StoredImportSettings {
    scale: f32,
    generate_collision: bool,
    material_namespace: Option<String>,
}

pub fn encode_sidecar(sidecar: &SidecarMetadata) -> Result<String, SidecarCodecError> {
    validate_sidecar(sidecar)?;
    let mut labels = sidecar.labels.clone();
    labels.sort();
    labels.dedup();
    let mut generated_artifacts = sidecar.generated_artifacts.clone();
    generated_artifacts.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    let stored = StoredSidecar {
        schema_version: sidecar.schema_version,
        guid: sidecar.guid.as_str().to_owned(),
        source_uri: StoredSourceUri {
            kind: sidecar.source_uri.kind().to_owned(),
            value: sidecar.source_uri.value().to_owned(),
        },
        source_hash: sidecar.source_hash.clone(),
        importer_version: sidecar.importer_version,
        declared_kind: sidecar.declared_kind.clone(),
        labels,
        import_settings: StoredImportSettings {
            scale: sidecar.import_settings.scale,
            generate_collision: sidecar.import_settings.generate_collision,
            material_namespace: sidecar.import_settings.material_namespace.clone(),
        },
        generated_artifacts,
    };
    let mut encoded = serde_json::to_string_pretty(&stored)
        .map_err(|error| SidecarCodecError::new("$", error.to_string()))?;
    encoded.push('\n');
    Ok(encoded)
}

pub fn decode_sidecar(input: &str) -> Result<SidecarMetadata, SidecarCodecError> {
    let mut deserializer = serde_json::Deserializer::from_str(input);
    let stored: StoredSidecar =
        serde_path_to_error::deserialize(&mut deserializer).map_err(|error| {
            let path = error.path().to_string();
            SidecarCodecError::new(
                if path.is_empty() { "$" } else { path.as_str() },
                error.inner().to_string(),
            )
        })?;
    deserializer
        .end()
        .map_err(|error| SidecarCodecError::new("$", error.to_string()))?;
    let guid = AssetGuid::parse(&stored.guid)
        .ok_or_else(|| SidecarCodecError::new("guid", "invalid 128-bit GUID"))?;
    let source_uri = match stored.source_uri.kind.as_str() {
        "relativePath" => SourceUri::RelativePath(stored.source_uri.value),
        "absolutePath" => SourceUri::AbsolutePath(stored.source_uri.value),
        "fileUrl" => SourceUri::FileUrl(stored.source_uri.value),
        "contentAddressed" => SourceUri::ContentAddressed(stored.source_uri.value),
        _ => return Err(SidecarCodecError::new("sourceUri.kind", "unknown URI kind")),
    };
    let sidecar = SidecarMetadata {
        schema_version: stored.schema_version,
        guid,
        source_uri,
        source_hash: stored.source_hash,
        importer_version: stored.importer_version,
        declared_kind: stored.declared_kind,
        labels: stored.labels,
        import_settings: ImportSettings {
            scale: stored.import_settings.scale,
            generate_collision: stored.import_settings.generate_collision,
            material_namespace: stored.import_settings.material_namespace,
        },
        generated_artifacts: stored.generated_artifacts,
    };
    validate_sidecar(&sidecar)?;
    Ok(sidecar)
}

fn validate_sidecar(sidecar: &SidecarMetadata) -> Result<(), SidecarCodecError> {
    if sidecar.schema_version != SIDECAR_SCHEMA_VERSION {
        return Err(SidecarCodecError::new(
            "schemaVersion",
            format!("unsupported sidecar schema {}", sidecar.schema_version),
        ));
    }
    if !sidecar.source_uri.is_valid() {
        return Err(SidecarCodecError::new("sourceUri", "invalid source URI"));
    }
    if sidecar.source_hash.len() != 64
        || !sidecar
            .source_hash
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(SidecarCodecError::new(
            "sourceHash",
            "source hash must be SHA-256",
        ));
    }
    if sidecar.importer_version == 0
        || core_assets::AssetKind::from_prefix(&sidecar.declared_kind).is_none()
    {
        return Err(SidecarCodecError::new(
            "$",
            "importer version and a known declared asset kind are required",
        ));
    }
    if !sidecar.import_settings.is_valid() {
        return Err(SidecarCodecError::new(
            "importSettings",
            "invalid typed import settings",
        ));
    }
    let mut labels = BTreeSet::new();
    for (index, label) in sidecar.labels.iter().enumerate() {
        if !is_scoped_key(label) || !labels.insert(label.as_str()) {
            return Err(SidecarCodecError::new(
                format!("labels[{index}]"),
                "label is invalid or duplicated",
            ));
        }
    }
    let mut paths = BTreeSet::new();
    for (index, artifact) in sidecar.generated_artifacts.iter().enumerate() {
        if !crate::artifact::is_safe_relative_path(&artifact.relative_path)
            || !paths.insert(artifact.relative_path.as_str())
        {
            return Err(SidecarCodecError::new(
                format!("generatedArtifacts[{index}].relativePath"),
                "artifact path is invalid or duplicated",
            ));
        }
        if artifact.content_hash.len() != 64
            || !artifact
                .content_hash
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(SidecarCodecError::new(
                format!("generatedArtifacts[{index}].contentHash"),
                "artifact hash must be SHA-256",
            ));
        }
    }
    Ok(())
}

fn is_scoped_key(value: &str) -> bool {
    !value.is_empty()
        && value.split('/').all(|segment| {
            !segment.is_empty()
                && !segment.starts_with('-')
                && !segment.ends_with('-')
                && !segment.contains("--")
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        })
}
