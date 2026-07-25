use std::collections::{BTreeMap, BTreeSet};

use crate::{
    encode_manifest, is_safe_relative_path, ArtifactClass, ContentArtifact, ContentHash,
    ContentManifest, ManifestError,
};

pub const CONTENT_MANIFEST_PATH: &str = "content.manifest.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentStoreIdentity {
    pub revision: u64,
    pub manifest_hash: ContentHash,
    pub content_set_hash: ContentHash,
}

impl ContentStoreIdentity {
    pub fn from_manifest(revision: u64, manifest: &ContentManifest) -> Result<Self, ManifestError> {
        manifest.validate()?;
        let canonical = manifest.canonical();
        let encoded = encode_manifest(&canonical).expect("validated manifest encodes");
        Ok(Self {
            revision,
            manifest_hash: ContentHash::of(encoded.as_bytes()),
            content_set_hash: content_set_hash(&canonical),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentWrite {
    path: String,
    bytes: Vec<u8>,
    content_hash: ContentHash,
}

impl ContentWrite {
    pub fn new(path: impl Into<String>, bytes: impl Into<Vec<u8>>) -> Self {
        let path = path.into();
        let bytes = bytes.into();
        let content_hash = ContentHash::of(&bytes);
        Self {
            path,
            bytes,
            content_hash,
        }
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn content_hash(&self) -> ContentHash {
        self.content_hash
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentMove {
    pub from: String,
    pub to: String,
    pub expected_content_hash: Option<ContentHash>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentDelete {
    pub path: String,
    pub expected_content_hash: Option<ContentHash>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentWriteSetDraft {
    pub next_manifest: ContentManifest,
    pub writes: Vec<ContentWrite>,
    pub moves: Vec<ContentMove>,
    pub deletes: Vec<ContentDelete>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentWriteSetError {
    InvalidPriorManifest(ManifestError),
    InvalidNextManifest(ManifestError),
    RevisionOverflow,
    InvalidPath(String),
    ManifestPathReserved,
    DuplicateTarget(String),
    ConflictingOperation(String),
    MissingPriorArtifact(String),
    MissingNextArtifact(String),
    HashMismatch(String),
    MetadataMismatch(String),
    UnaccountedPriorChange(String),
    UnaccountedNextChange(String),
    StaleStore,
    PublicationMismatch,
}

impl std::fmt::Display for ContentWriteSetError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid content write set: {self:?}")
    }
}

impl std::error::Error for ContentWriteSetError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentWriteCandidate {
    expected_prior: ContentStoreIdentity,
    expected_next: ContentStoreIdentity,
    manifest_json: String,
    writes: Vec<ContentWrite>,
    moves: Vec<ContentMove>,
    deletes: Vec<ContentDelete>,
    candidate_hash: ContentHash,
}

impl ContentWriteCandidate {
    pub fn build(
        prior_revision: u64,
        prior_manifest: &ContentManifest,
        draft: ContentWriteSetDraft,
    ) -> Result<Self, ContentWriteSetError> {
        let expected_prior = ContentStoreIdentity::from_manifest(prior_revision, prior_manifest)
            .map_err(ContentWriteSetError::InvalidPriorManifest)?;
        Self::build_from_observed_prior(expected_prior, prior_manifest, draft)
    }

    /// Builds against an identity observed by a trusted host. This permits a
    /// canonical manifest rewrite while retaining exact compare-and-swap
    /// authority over the bytes actually observed by that host.
    pub fn build_from_observed_prior(
        observed_prior: ContentStoreIdentity,
        prior_manifest: &ContentManifest,
        mut draft: ContentWriteSetDraft,
    ) -> Result<Self, ContentWriteSetError> {
        prior_manifest
            .validate()
            .map_err(ContentWriteSetError::InvalidPriorManifest)?;
        draft
            .next_manifest
            .validate()
            .map_err(ContentWriteSetError::InvalidNextManifest)?;
        let canonical_prior =
            ContentStoreIdentity::from_manifest(observed_prior.revision, prior_manifest)
                .map_err(ContentWriteSetError::InvalidPriorManifest)?;
        if canonical_prior.content_set_hash != observed_prior.content_set_hash {
            return Err(ContentWriteSetError::StaleStore);
        }
        validate_operations(prior_manifest, &draft)?;
        let next_revision = observed_prior
            .revision
            .checked_add(1)
            .ok_or(ContentWriteSetError::RevisionOverflow)?;
        let expected_next =
            ContentStoreIdentity::from_manifest(next_revision, &draft.next_manifest)
                .map_err(ContentWriteSetError::InvalidNextManifest)?;
        let manifest_json =
            encode_manifest(&draft.next_manifest).expect("validated content manifest must encode");
        draft
            .writes
            .sort_by(|left, right| left.path.cmp(&right.path));
        draft.moves.sort_by(|left, right| {
            left.from
                .cmp(&right.from)
                .then_with(|| left.to.cmp(&right.to))
        });
        draft
            .deletes
            .sort_by(|left, right| left.path.cmp(&right.path));
        let candidate_hash = candidate_hash(
            &observed_prior,
            &expected_next,
            &manifest_json,
            &draft.writes,
            &draft.moves,
            &draft.deletes,
        );
        Ok(Self {
            expected_prior: observed_prior,
            expected_next,
            manifest_json,
            writes: draft.writes,
            moves: draft.moves,
            deletes: draft.deletes,
            candidate_hash,
        })
    }

    pub fn expected_prior(&self) -> &ContentStoreIdentity {
        &self.expected_prior
    }

    pub fn expected_next(&self) -> &ContentStoreIdentity {
        &self.expected_next
    }

    pub fn manifest_json(&self) -> &str {
        &self.manifest_json
    }

    pub fn writes(&self) -> &[ContentWrite] {
        &self.writes
    }

    pub fn moves(&self) -> &[ContentMove] {
        &self.moves
    }

    pub fn deletes(&self) -> &[ContentDelete] {
        &self.deletes
    }

    pub fn candidate_hash(&self) -> ContentHash {
        self.candidate_hash
    }

    pub fn authorize(
        self,
        observed: &ContentStoreIdentity,
    ) -> Result<AuthorizedContentWriteCandidate, ContentWriteSetError> {
        if observed != &self.expected_prior {
            return Err(ContentWriteSetError::StaleStore);
        }
        Ok(AuthorizedContentWriteCandidate { candidate: self })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizedContentWriteCandidate {
    candidate: ContentWriteCandidate,
}

impl AuthorizedContentWriteCandidate {
    pub fn candidate(&self) -> &ContentWriteCandidate {
        &self.candidate
    }

    pub fn confirm(
        self,
        observed: &ContentStoreIdentity,
    ) -> Result<ContentWriteConfirmation, ContentWriteSetError> {
        if observed != &self.candidate.expected_next {
            return Err(ContentWriteSetError::PublicationMismatch);
        }
        Ok(ContentWriteConfirmation {
            identity: self.candidate.expected_next,
            candidate_hash: self.candidate.candidate_hash,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentWriteConfirmation {
    pub identity: ContentStoreIdentity,
    pub candidate_hash: ContentHash,
}

fn validate_operations(
    prior: &ContentManifest,
    draft: &ContentWriteSetDraft,
) -> Result<(), ContentWriteSetError> {
    let next = &draft.next_manifest;
    let prior_by_path: BTreeMap<_, _> = prior
        .artifacts
        .iter()
        .map(|artifact| (artifact.path.as_str(), artifact))
        .collect();
    let next_by_path: BTreeMap<_, _> = next
        .artifacts
        .iter()
        .map(|artifact| (artifact.path.as_str(), artifact))
        .collect();
    let mut sources = BTreeSet::new();
    let mut targets = BTreeSet::new();

    for write in &draft.writes {
        validate_operation_path(&write.path)?;
        if !targets.insert(write.path.as_str()) {
            return Err(ContentWriteSetError::DuplicateTarget(write.path.clone()));
        }
        let artifact = next_by_path
            .get(write.path.as_str())
            .ok_or_else(|| ContentWriteSetError::MissingNextArtifact(write.path.clone()))?;
        if artifact.class == ArtifactClass::Cache
            || artifact.content_hash != Some(write.content_hash)
            || artifact.byte_len != Some(write.bytes.len() as u64)
        {
            return Err(ContentWriteSetError::HashMismatch(write.path.clone()));
        }
    }
    for movement in &draft.moves {
        validate_operation_path(&movement.from)?;
        validate_operation_path(&movement.to)?;
        if movement.from == movement.to {
            return Err(ContentWriteSetError::ConflictingOperation(
                movement.from.clone(),
            ));
        }
        if !sources.insert(movement.from.as_str()) {
            return Err(ContentWriteSetError::ConflictingOperation(
                movement.from.clone(),
            ));
        }
        if !targets.insert(movement.to.as_str()) {
            return Err(ContentWriteSetError::DuplicateTarget(movement.to.clone()));
        }
        let old = prior_by_path
            .get(movement.from.as_str())
            .ok_or_else(|| ContentWriteSetError::MissingPriorArtifact(movement.from.clone()))?;
        let new = next_by_path
            .get(movement.to.as_str())
            .ok_or_else(|| ContentWriteSetError::MissingNextArtifact(movement.to.clone()))?;
        if movement.expected_content_hash != old.content_hash {
            return Err(ContentWriteSetError::HashMismatch(movement.from.clone()));
        }
        if !same_metadata_and_content(old, new) {
            return Err(ContentWriteSetError::MetadataMismatch(movement.to.clone()));
        }
    }
    for deletion in &draft.deletes {
        validate_operation_path(&deletion.path)?;
        if !sources.insert(deletion.path.as_str()) {
            return Err(ContentWriteSetError::ConflictingOperation(
                deletion.path.clone(),
            ));
        }
        let old = prior_by_path
            .get(deletion.path.as_str())
            .ok_or_else(|| ContentWriteSetError::MissingPriorArtifact(deletion.path.clone()))?;
        if deletion.expected_content_hash != old.content_hash {
            return Err(ContentWriteSetError::HashMismatch(deletion.path.clone()));
        }
    }

    let paths: BTreeSet<_> = prior_by_path
        .keys()
        .chain(next_by_path.keys())
        .copied()
        .collect();
    for path in paths {
        match (prior_by_path.get(path), next_by_path.get(path)) {
            (Some(old), Some(new)) if *old == *new => {
                if sources.contains(path) || targets.contains(path) {
                    return Err(ContentWriteSetError::ConflictingOperation(path.to_owned()));
                }
            }
            (Some(_), Some(_)) => {
                if !targets.contains(path) {
                    return Err(ContentWriteSetError::UnaccountedNextChange(path.to_owned()));
                }
                if sources.contains(path) {
                    return Err(ContentWriteSetError::ConflictingOperation(path.to_owned()));
                }
            }
            (Some(_), None) if !sources.contains(path) => {
                return Err(ContentWriteSetError::UnaccountedPriorChange(
                    path.to_owned(),
                ));
            }
            (None, Some(_)) if !targets.contains(path) => {
                return Err(ContentWriteSetError::UnaccountedNextChange(path.to_owned()));
            }
            _ => {}
        }
    }
    Ok(())
}

fn validate_operation_path(path: &str) -> Result<(), ContentWriteSetError> {
    if path == CONTENT_MANIFEST_PATH {
        return Err(ContentWriteSetError::ManifestPathReserved);
    }
    if !is_safe_relative_path(path) {
        return Err(ContentWriteSetError::InvalidPath(path.to_owned()));
    }
    Ok(())
}

fn same_metadata_and_content(left: &ContentArtifact, right: &ContentArtifact) -> bool {
    left.class == right.class
        && left.role == right.role
        && left.content_hash == right.content_hash
        && left.byte_len == right.byte_len
}

fn content_set_hash(manifest: &ContentManifest) -> ContentHash {
    let mut bytes = Vec::new();
    for artifact in manifest.canonical().load_required() {
        bytes.extend_from_slice(artifact.path.as_bytes());
        bytes.push(0);
        bytes.extend_from_slice(artifact.class.tag().as_bytes());
        bytes.push(0);
        bytes.extend_from_slice(artifact.role.tag().as_bytes());
        bytes.push(0);
        if let Some(hash) = artifact.content_hash {
            bytes.extend_from_slice(hash.as_bytes());
        }
        bytes.extend_from_slice(&artifact.byte_len.unwrap_or_default().to_le_bytes());
    }
    ContentHash::of(&bytes)
}

fn candidate_hash(
    prior: &ContentStoreIdentity,
    next: &ContentStoreIdentity,
    manifest_json: &str,
    writes: &[ContentWrite],
    moves: &[ContentMove],
    deletes: &[ContentDelete],
) -> ContentHash {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&prior.revision.to_le_bytes());
    bytes.extend_from_slice(prior.manifest_hash.as_bytes());
    bytes.extend_from_slice(next.manifest_hash.as_bytes());
    bytes.extend_from_slice(manifest_json.as_bytes());
    for write in writes {
        bytes.extend_from_slice(write.path.as_bytes());
        bytes.push(0);
        bytes.extend_from_slice(write.content_hash.as_bytes());
    }
    for movement in moves {
        bytes.extend_from_slice(movement.from.as_bytes());
        bytes.push(0);
        bytes.extend_from_slice(movement.to.as_bytes());
        bytes.push(0);
    }
    for deletion in deletes {
        bytes.extend_from_slice(deletion.path.as_bytes());
        bytes.push(0);
    }
    ContentHash::of(&bytes)
}
