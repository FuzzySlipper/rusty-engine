use std::collections::{BTreeMap, BTreeSet};

use crate::{decode_manifest, ArtifactClass, ContentHash, ContentManifest};

pub const CONTENT_MANIFEST_MAX_BYTES: usize = 2 * 1024 * 1024;
pub const CONTENT_MAX_BODIES: usize = 16_384;
pub const CONTENT_BODY_MAX_BYTES: usize = 256 * 1024 * 1024;
pub const CONTENT_TOTAL_MAX_BYTES: usize = 512 * 1024 * 1024;

/// One host-supplied body. Class, role, length, and identity are deliberately
/// resolved from the canonical manifest rather than duplicated at the border.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentBody {
    pub path: String,
    pub bytes: Vec<u8>,
}

impl ContentBody {
    pub fn new(path: impl Into<String>, bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            path: path.into(),
            bytes: bytes.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentSourceBatch {
    pub manifest_json: String,
    pub bodies: Vec<ContentBody>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedContentBatch {
    pub manifest: ContentManifest,
    bodies: BTreeMap<String, Vec<u8>>,
}

impl AdmittedContentBatch {
    pub fn body(&self, path: &str) -> Option<&[u8]> {
        self.bodies.get(path).map(Vec::as_slice)
    }

    pub fn bodies(&self) -> impl Iterator<Item = (&str, &[u8])> {
        self.bodies
            .iter()
            .map(|(path, bytes)| (path.as_str(), bytes.as_slice()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentSourceErrorCode {
    ManifestTooLarge,
    ManifestInvalid,
    TooManyBodies,
    DuplicateBody,
    MissingBody,
    ExtraBody,
    CacheBodyForbidden,
    BodyTooLarge,
    TotalTooLarge,
    LengthMismatch,
    HashMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentSourceError {
    pub code: ContentSourceErrorCode,
    pub path: Option<String>,
    pub message: String,
}

impl ContentSourceError {
    fn new(code: ContentSourceErrorCode, path: Option<&str>, message: impl Into<String>) -> Self {
        Self {
            code,
            path: path.map(str::to_owned),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ContentSourceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.path {
            Some(path) => write!(formatter, "{:?} at `{path}`: {}", self.code, self.message),
            None => write!(formatter, "{:?}: {}", self.code, self.message),
        }
    }
}

impl std::error::Error for ContentSourceError {}

pub fn admit_source_batch(
    batch: ContentSourceBatch,
) -> Result<AdmittedContentBatch, ContentSourceError> {
    if batch.manifest_json.len() > CONTENT_MANIFEST_MAX_BYTES {
        return Err(ContentSourceError::new(
            ContentSourceErrorCode::ManifestTooLarge,
            None,
            "manifest exceeds the bounded admission limit",
        ));
    }
    let manifest = decode_manifest(&batch.manifest_json).map_err(|error| {
        ContentSourceError::new(
            ContentSourceErrorCode::ManifestInvalid,
            None,
            error.to_string(),
        )
    })?;
    if batch.bodies.len() > CONTENT_MAX_BODIES {
        return Err(ContentSourceError::new(
            ContentSourceErrorCode::TooManyBodies,
            None,
            "body count exceeds the bounded admission limit",
        ));
    }

    let expected: BTreeSet<_> = manifest
        .load_required()
        .map(|artifact| artifact.path.as_str())
        .collect();
    let cache: BTreeSet<_> = manifest
        .artifacts
        .iter()
        .filter(|artifact| artifact.class == ArtifactClass::Cache)
        .map(|artifact| artifact.path.as_str())
        .collect();
    let mut bodies = BTreeMap::new();
    let mut total = 0_usize;
    for body in batch.bodies {
        if cache.contains(body.path.as_str()) {
            return Err(ContentSourceError::new(
                ContentSourceErrorCode::CacheBodyForbidden,
                Some(&body.path),
                "cache entries are optional host state and cannot enter content admission",
            ));
        }
        let Some(artifact) = manifest.artifact(&body.path) else {
            return Err(ContentSourceError::new(
                ContentSourceErrorCode::ExtraBody,
                Some(&body.path),
                "body is not declared by the manifest",
            ));
        };
        if bodies.contains_key(&body.path) {
            return Err(ContentSourceError::new(
                ContentSourceErrorCode::DuplicateBody,
                Some(&body.path),
                "body path occurs more than once",
            ));
        }
        if body.bytes.len() > CONTENT_BODY_MAX_BYTES {
            return Err(ContentSourceError::new(
                ContentSourceErrorCode::BodyTooLarge,
                Some(&body.path),
                "body exceeds the per-body limit",
            ));
        }
        total = total.checked_add(body.bytes.len()).ok_or_else(|| {
            ContentSourceError::new(
                ContentSourceErrorCode::TotalTooLarge,
                None,
                "body byte total overflowed",
            )
        })?;
        if total > CONTENT_TOTAL_MAX_BYTES {
            return Err(ContentSourceError::new(
                ContentSourceErrorCode::TotalTooLarge,
                None,
                "body bytes exceed the aggregate limit",
            ));
        }
        if artifact.byte_len != Some(body.bytes.len() as u64) {
            return Err(ContentSourceError::new(
                ContentSourceErrorCode::LengthMismatch,
                Some(&body.path),
                "body length does not match the manifest",
            ));
        }
        if artifact.content_hash != Some(ContentHash::of(&body.bytes)) {
            return Err(ContentSourceError::new(
                ContentSourceErrorCode::HashMismatch,
                Some(&body.path),
                "body content does not match the manifest hash",
            ));
        }
        bodies.insert(body.path, body.bytes);
    }

    if let Some(path) = expected.iter().find(|path| !bodies.contains_key(**path)) {
        return Err(ContentSourceError::new(
            ContentSourceErrorCode::MissingBody,
            Some(path),
            "manifest-required body is absent",
        ));
    }
    Ok(AdmittedContentBatch { manifest, bodies })
}
