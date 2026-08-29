//! Host-rooted execution of admitted [`content_store`] publications.
//!
//! This host adapter owns a small, named filesystem layout below a
//! caller-selected host root. It consumes the authoritative content-store model
//! as an upper host adapter rather than a lower `svc-*` mechanism. It
//! deliberately has no ambient/default path and exposes neither a generic
//! filesystem nor a product-byte persistence API.

#![forbid(unsafe_code)]

use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
    sync::Arc,
};

use content_store::{
    admit_source_batch, encode_manifest, is_safe_relative_path, AdmittedContentBatch, ContentBody,
    ContentManifest, ContentSavePlan, ContentSourceBatch, ContentStoreIdentity,
    ContentWriteCandidate, ContentWriteConfirmation, ContentWriteSetDraft, ContentWriteSetError,
    CONTENT_MANIFEST_MAX_BYTES, CONTENT_MANIFEST_PATH,
};
use fs2::FileExt;

const SERVICE_DIR: &str = ".rusty-content-store";
const GENERATIONS_DIR: &str = "generations";
const STAGING_DIR: &str = "staging";
const CURRENT_FILE: &str = "current";
const LOCK_FILE: &str = "lock";
const POINTER_VERSION: &str = "rusty-content-store-current-v1";

/// An explicit host root under which named content scopes may be opened.
#[derive(Debug, Clone)]
pub struct ContentStoreExecutor {
    host_root: PathBuf,
}

impl ContentStoreExecutor {
    /// Select the host-controlled root. No store is opened implicitly.
    pub fn new(host_root: impl Into<PathBuf>) -> Result<Self, ContentStoreExecutorError> {
        let host_root = host_root.into();
        if !host_root.is_absolute() {
            return Err(ContentStoreExecutorError::InvalidRoot(host_root));
        }
        ensure_directory_tree(&host_root)?;
        Ok(Self { host_root })
    }

    /// Open one safe, relative, caller-named content scope.
    pub fn open(&self, scope: &str) -> Result<ContentStore, ContentStoreExecutorError> {
        if !is_safe_relative_path(scope) {
            return Err(ContentStoreExecutorError::InvalidScope(scope.to_owned()));
        }
        let scope_path = self.host_root.join(scope);
        ensure_directory_tree(&scope_path)?;
        let state = scope_path.join(SERVICE_DIR);
        ensure_directory_tree(&state.join(GENERATIONS_DIR))?;
        ensure_directory_tree(&state.join(STAGING_DIR))?;
        let lock = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(state.join(LOCK_FILE))?;
        lock.lock_exclusive()?;
        let snapshot = (|| {
            let current = state.join(CURRENT_FILE);
            let snapshot = match fs::symlink_metadata(&current) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    return Err(ContentStoreExecutorError::InvalidPointer(
                        "current pointer must not be a symlink".to_owned(),
                    ));
                }
                Ok(_) => read_current(&state)?,
                Err(error) if error.kind() == io::ErrorKind::NotFound => initialize_empty(&state)?,
                Err(error) => return Err(error.into()),
            };
            cleanup_orphans(&state, &snapshot.0.generation)?;
            Ok(snapshot)
        })();
        let _ = FileExt::unlock(&lock);
        snapshot.map(|snapshot| ContentStore { state, snapshot })
    }
}

/// A retained, immutable view of one fully admitted generation.
#[derive(Debug, Clone)]
pub struct ContentStoreSnapshot(Arc<SnapshotData>);

#[derive(Debug)]
struct SnapshotData {
    identity: ContentStoreIdentity,
    manifest: ContentManifest,
    bodies: BTreeMap<String, Vec<u8>>,
    generation: String,
}

impl ContentStoreSnapshot {
    pub fn identity(&self) -> &ContentStoreIdentity {
        &self.0.identity
    }
    pub fn manifest(&self) -> &ContentManifest {
        &self.0.manifest
    }
    pub fn body(&self, path: &str) -> Option<&[u8]> {
        self.0.bodies.get(path).map(Vec::as_slice)
    }
    pub fn bodies(&self) -> impl Iterator<Item = (&str, &[u8])> {
        self.0
            .bodies
            .iter()
            .map(|(path, body)| (path.as_str(), body.as_slice()))
    }
}

/// One named store. Snapshots remain usable after a newer generation publishes.
#[derive(Debug)]
pub struct ContentStore {
    state: PathBuf,
    snapshot: ContentStoreSnapshot,
}

impl ContentStore {
    pub fn snapshot(&self) -> ContentStoreSnapshot {
        self.snapshot.clone()
    }

    /// Build and execute a typed write-set draft against the generation
    /// re-observed under the exclusive cross-process lock.
    pub fn publish(
        &mut self,
        expected_prior: &ContentStoreIdentity,
        draft: ContentWriteSetDraft,
    ) -> Result<ContentWriteConfirmation, ContentStoreExecutorError> {
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .open(self.state.join(LOCK_FILE))?;
        lock.lock_exclusive()?;
        let result = (|| {
            let prior = read_current(&self.state)?;
            if prior.identity() != expected_prior {
                return Err(ContentWriteSetError::StaleStore.into());
            }
            let candidate = ContentWriteCandidate::build_from_observed_prior(
                prior.identity().clone(),
                prior.manifest(),
                draft,
            )?;
            let authorized = candidate.authorize(prior.identity())?;
            let plan = ContentSavePlan::from_candidate(authorized.candidate());
            let next = materialize_next(&prior, authorized.candidate(), &plan)?;
            let admitted =
                admit_source_batch(next).map_err(ContentStoreExecutorError::Admission)?;
            let expected = authorized.candidate().expected_next().clone();
            let actual = ContentStoreIdentity::from_manifest(expected.revision, &admitted.manifest)
                .map_err(ContentWriteSetError::InvalidNextManifest)?;
            if actual != expected {
                return Err(ContentStoreExecutorError::PublicationMismatch);
            }
            let published = publish(&self.state, admitted, actual)?;
            let confirmation = authorized.confirm(published.identity())?;
            self.snapshot = published;
            cleanup_orphans(&self.state, &self.snapshot.0.generation)?;
            Ok(confirmation)
        })();
        let _ = FileExt::unlock(&lock);
        result
    }
}

#[derive(Debug)]
pub enum ContentStoreExecutorError {
    Io(io::Error),
    InvalidRoot(PathBuf),
    InvalidScope(String),
    InvalidPointer(String),
    InvalidGeneration(String),
    Admission(content_store::ContentSourceError),
    WriteSet(ContentWriteSetError),
    PublicationMismatch,
}

impl std::fmt::Display for ContentStoreExecutorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "content store executor error: {self:?}")
    }
}
impl std::error::Error for ContentStoreExecutorError {}
impl From<io::Error> for ContentStoreExecutorError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}
impl From<ContentWriteSetError> for ContentStoreExecutorError {
    fn from(value: ContentWriteSetError) -> Self {
        Self::WriteSet(value)
    }
}

fn materialize_next(
    prior: &ContentStoreSnapshot,
    candidate: &ContentWriteCandidate,
    plan: &ContentSavePlan,
) -> Result<ContentSourceBatch, ContentStoreExecutorError> {
    let mut bodies: BTreeMap<String, Vec<u8>> = prior.0.bodies.clone();
    for delete in &plan.deletes {
        bodies.remove(&delete.path);
    }
    for movement in &plan.moves {
        let body = bodies
            .remove(&movement.from)
            .ok_or_else(|| ContentStoreExecutorError::InvalidGeneration(movement.from.clone()))?;
        bodies.insert(movement.to.clone(), body);
    }
    for write in &plan.writes {
        bodies.insert(write.path().to_owned(), write.bytes().to_vec());
    }
    if plan.manifest_path != CONTENT_MANIFEST_PATH
        || plan.manifest_bytes != candidate.manifest_json().as_bytes()
    {
        return Err(ContentStoreExecutorError::PublicationMismatch);
    }
    Ok(ContentSourceBatch {
        manifest_json: candidate.manifest_json().to_owned(),
        bodies: bodies
            .into_iter()
            .map(|(path, bytes)| ContentBody::new(path, bytes))
            .collect(),
    })
}

fn initialize_empty(state: &Path) -> Result<ContentStoreSnapshot, ContentStoreExecutorError> {
    let manifest = ContentManifest::new(vec![]);
    let identity = ContentStoreIdentity::from_manifest(0, &manifest)
        .map_err(ContentWriteSetError::InvalidNextManifest)?;
    let admitted = admit_source_batch(ContentSourceBatch {
        manifest_json: encode_manifest(&manifest).expect("empty manifest encodes"),
        bodies: vec![],
    })
    .map_err(ContentStoreExecutorError::Admission)?;
    publish(state, admitted, identity)
}

fn read_current(state: &Path) -> Result<ContentStoreSnapshot, ContentStoreExecutorError> {
    let current = state.join(CURRENT_FILE);
    if symlink_or_missing(&current)? {
        return Err(ContentStoreExecutorError::InvalidPointer(
            "missing or symlinked current pointer".to_owned(),
        ));
    }
    let pointer = parse_pointer(&fs::read_to_string(current)?)?;
    let generation = state.join(GENERATIONS_DIR).join(&pointer.generation);
    if symlink_or_missing(&generation)? {
        return Err(ContentStoreExecutorError::InvalidGeneration(
            pointer.generation,
        ));
    }
    let manifest_path = generation.join(CONTENT_MANIFEST_PATH);
    let manifest_bytes = read_bounded(&manifest_path, CONTENT_MANIFEST_MAX_BYTES)?;
    let manifest_json = String::from_utf8(manifest_bytes)
        .map_err(|_| ContentStoreExecutorError::InvalidGeneration(pointer.generation.clone()))?;
    let manifest = content_store::decode_manifest(&manifest_json)
        .map_err(|_| ContentStoreExecutorError::InvalidGeneration(pointer.generation.clone()))?;
    let mut bodies = Vec::new();
    for artifact in manifest.load_required() {
        let path = generation.join(&artifact.path);
        if symlink_or_missing(&path)? {
            return Err(ContentStoreExecutorError::InvalidGeneration(
                artifact.path.clone(),
            ));
        }
        bodies.push(ContentBody::new(
            &artifact.path,
            read_bounded(&path, content_store::CONTENT_BODY_MAX_BYTES)?,
        ));
    }
    let admitted = admit_source_batch(ContentSourceBatch {
        manifest_json,
        bodies,
    })
    .map_err(ContentStoreExecutorError::Admission)?;
    let identity = ContentStoreIdentity::from_manifest(pointer.revision, &admitted.manifest)
        .map_err(ContentWriteSetError::InvalidPriorManifest)?;
    if identity.manifest_hash != pointer.manifest_hash
        || identity.content_set_hash != pointer.content_set_hash
    {
        return Err(ContentStoreExecutorError::InvalidPointer(
            "pointer identity does not match its admitted generation".to_owned(),
        ));
    }
    Ok(snapshot_from_admitted(
        identity,
        admitted,
        pointer.generation,
    ))
}

fn publish(
    state: &Path,
    admitted: AdmittedContentBatch,
    identity: ContentStoreIdentity,
) -> Result<ContentStoreSnapshot, ContentStoreExecutorError> {
    let generation = generation_name(&identity);
    let staging = state.join(STAGING_DIR).join(format!("{generation}.next"));
    if staging.exists() {
        fs::remove_dir_all(&staging)?;
    }
    fs::create_dir(&staging)?;
    write_file(
        &staging.join(CONTENT_MANIFEST_PATH),
        encode_manifest(&admitted.manifest)
            .expect("admitted manifest encodes")
            .as_bytes(),
    )?;
    for (path, bytes) in admitted.bodies() {
        let target = staging.join(path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
            sync_dir(parent)?;
        }
        write_file(&target, bytes)?;
    }
    sync_tree_dirs(&staging)?;
    let final_path = state.join(GENERATIONS_DIR).join(&generation);
    if final_path.exists() {
        fs::remove_dir_all(&final_path)?;
    }
    fs::rename(&staging, &final_path)?;
    sync_dir(&state.join(GENERATIONS_DIR))?;
    write_pointer(
        state,
        &Pointer {
            revision: identity.revision,
            manifest_hash: identity.manifest_hash,
            content_set_hash: identity.content_set_hash,
            generation: generation.clone(),
        },
    )?;
    Ok(snapshot_from_admitted(identity, admitted, generation))
}

fn snapshot_from_admitted(
    identity: ContentStoreIdentity,
    admitted: AdmittedContentBatch,
    generation: String,
) -> ContentStoreSnapshot {
    let bodies = admitted
        .bodies()
        .map(|(path, body)| (path.to_owned(), body.to_vec()))
        .collect();
    ContentStoreSnapshot(Arc::new(SnapshotData {
        identity,
        manifest: admitted.manifest,
        bodies,
        generation,
    }))
}

#[derive(Debug)]
struct Pointer {
    revision: u64,
    manifest_hash: content_store::ContentHash,
    content_set_hash: content_store::ContentHash,
    generation: String,
}

fn write_pointer(state: &Path, pointer: &Pointer) -> Result<(), ContentStoreExecutorError> {
    let next = state.join("current.next");
    let text = format!(
        "{POINTER_VERSION}\nrevision={}\nmanifest_hash={}\ncontent_set_hash={}\ngeneration={}\n",
        pointer.revision, pointer.manifest_hash, pointer.content_set_hash, pointer.generation
    );
    write_file(&next, text.as_bytes())?;
    fs::rename(next, state.join(CURRENT_FILE))?;
    sync_dir(state)?;
    Ok(())
}

fn parse_pointer(text: &str) -> Result<Pointer, ContentStoreExecutorError> {
    let mut lines = text.lines();
    if lines.next() != Some(POINTER_VERSION) {
        return Err(ContentStoreExecutorError::InvalidPointer(
            "unsupported pointer format".to_owned(),
        ));
    }
    let revision = pointer_field(&mut lines, "revision")?
        .parse()
        .map_err(|_| ContentStoreExecutorError::InvalidPointer("invalid revision".to_owned()))?;
    let manifest_hash =
        content_store::ContentHash::parse(pointer_field(&mut lines, "manifest_hash")?).map_err(
            |_| ContentStoreExecutorError::InvalidPointer("invalid manifest hash".to_owned()),
        )?;
    let content_set_hash =
        content_store::ContentHash::parse(pointer_field(&mut lines, "content_set_hash")?).map_err(
            |_| ContentStoreExecutorError::InvalidPointer("invalid content set hash".to_owned()),
        )?;
    let generation = pointer_field(&mut lines, "generation")?.to_owned();
    if lines.next().is_some()
        || !generation.starts_with("gen-")
        || !is_safe_relative_path(&generation)
    {
        return Err(ContentStoreExecutorError::InvalidPointer(
            "invalid generation".to_owned(),
        ));
    }
    Ok(Pointer {
        revision,
        manifest_hash,
        content_set_hash,
        generation,
    })
}
fn pointer_field<'a>(
    lines: &mut impl Iterator<Item = &'a str>,
    name: &str,
) -> Result<&'a str, ContentStoreExecutorError> {
    lines
        .next()
        .and_then(|line| line.strip_prefix(&format!("{name}=")))
        .ok_or_else(|| ContentStoreExecutorError::InvalidPointer(format!("missing {name}")))
}
fn generation_name(identity: &ContentStoreIdentity) -> String {
    format!(
        "gen-{:020}-{}",
        identity.revision,
        identity.manifest_hash.to_hex()
    )
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<(), ContentStoreExecutorError> {
    let mut file = File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}
fn read_bounded(path: &Path, maximum: usize) -> Result<Vec<u8>, ContentStoreExecutorError> {
    let file = File::open(path)?;
    let mut bytes = Vec::new();
    file.take((maximum as u64).saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() > maximum {
        return Err(ContentStoreExecutorError::InvalidGeneration(
            path.display().to_string(),
        ));
    }
    Ok(bytes)
}
fn sync_dir(path: &Path) -> Result<(), ContentStoreExecutorError> {
    File::open(path)?.sync_all()?;
    Ok(())
}
fn sync_tree_dirs(root: &Path) -> Result<(), ContentStoreExecutorError> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            sync_tree_dirs(&entry.path())?;
        }
    }
    sync_dir(root)
}
fn symlink_or_missing(path: &Path) -> Result<bool, ContentStoreExecutorError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(metadata.file_type().is_symlink()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(true),
        Err(error) => Err(error.into()),
    }
}
fn ensure_directory_tree(path: &Path) -> Result<(), ContentStoreExecutorError> {
    let mut current = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => current.push(prefix.as_os_str()),
            Component::RootDir => current.push(Path::new("/")),
            Component::Normal(segment) => {
                current.push(segment);
                match fs::symlink_metadata(&current) {
                    Ok(metadata) if metadata.file_type().is_symlink() => {
                        return Err(ContentStoreExecutorError::InvalidScope(
                            current.display().to_string(),
                        ))
                    }
                    Ok(metadata) if !metadata.is_dir() => {
                        return Err(ContentStoreExecutorError::InvalidScope(
                            current.display().to_string(),
                        ))
                    }
                    Ok(_) => {}
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {
                        match fs::create_dir(&current) {
                            Ok(()) => {}
                            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                                let metadata = fs::symlink_metadata(&current)?;
                                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                                    return Err(ContentStoreExecutorError::InvalidScope(
                                        current.display().to_string(),
                                    ));
                                }
                            }
                            Err(error) => return Err(error.into()),
                        }
                    }
                    Err(error) => return Err(error.into()),
                }
            }
            Component::CurDir | Component::ParentDir => {
                return Err(ContentStoreExecutorError::InvalidScope(
                    path.display().to_string(),
                ))
            }
        }
    }
    Ok(())
}
fn cleanup_orphans(
    state: &Path,
    current_generation: &str,
) -> Result<(), ContentStoreExecutorError> {
    let staging = state.join(STAGING_DIR);
    for entry in fs::read_dir(&staging)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            fs::remove_dir_all(entry.path())?;
        }
    }
    let generations = state.join(GENERATIONS_DIR);
    for entry in fs::read_dir(&generations)? {
        let entry = entry?;
        if entry.file_name() != current_generation && entry.file_type()?.is_dir() {
            fs::remove_dir_all(entry.path())?;
        }
    }
    sync_dir(&staging)?;
    sync_dir(&generations)
}

#[cfg(test)]
mod tests {
    use super::*;
    use content_store::{
        ArtifactRole, ContentArtifact, ContentDelete, ContentMove, ContentWrite,
        ContentWriteSetDraft,
    };
    use tempfile::tempdir;

    fn manifest(entries: Vec<(&str, &[u8])>) -> ContentManifest {
        ContentManifest::new(
            entries
                .into_iter()
                .map(|(path, bytes)| {
                    ContentArtifact::durable(
                        path,
                        ArtifactRole::Resource("resource:test".to_owned()),
                        bytes,
                    )
                })
                .collect(),
        )
    }
    fn draft(
        next: ContentManifest,
        writes: Vec<ContentWrite>,
        moves: Vec<ContentMove>,
        deletes: Vec<ContentDelete>,
    ) -> ContentWriteSetDraft {
        ContentWriteSetDraft {
            next_manifest: next,
            writes,
            moves,
            deletes,
        }
    }

    #[test]
    fn initial_open_persists_empty_revision() {
        let temp = tempdir().unwrap();
        let executor = ContentStoreExecutor::new(temp.path()).unwrap();
        let store = executor.open("campaign").unwrap();
        assert_eq!(store.snapshot().identity().revision, 0);
        drop(store);
        assert_eq!(
            executor
                .open("campaign")
                .unwrap()
                .snapshot()
                .identity()
                .revision,
            0
        );
    }

    #[test]
    fn host_root_must_be_explicit_and_absolute() {
        assert!(matches!(
            ContentStoreExecutor::new("relative-content-root"),
            Err(ContentStoreExecutorError::InvalidRoot(_))
        ));
    }
    #[test]
    fn atomic_write_move_delete() {
        let temp = tempdir().unwrap();
        let executor = ContentStoreExecutor::new(temp.path()).unwrap();
        let mut store = executor.open("campaign").unwrap();
        let initial = manifest(vec![("old.bin", b"old"), ("gone.bin", b"gone")]);
        let expected = store.snapshot().identity().clone();
        store
            .publish(
                &expected,
                draft(
                    initial,
                    vec![
                        ContentWrite::new("old.bin", b"old"),
                        ContentWrite::new("gone.bin", b"gone"),
                    ],
                    vec![],
                    vec![],
                ),
            )
            .unwrap();
        let before = store.snapshot();
        let next = manifest(vec![("moved.bin", b"old"), ("new.bin", b"new")]);
        store
            .publish(
                before.identity(),
                draft(
                    next,
                    vec![ContentWrite::new("new.bin", b"new")],
                    vec![ContentMove {
                        from: "old.bin".to_owned(),
                        to: "moved.bin".to_owned(),
                        expected_content_hash: Some(content_store::ContentHash::of(b"old")),
                    }],
                    vec![ContentDelete {
                        path: "gone.bin".to_owned(),
                        expected_content_hash: Some(content_store::ContentHash::of(b"gone")),
                    }],
                ),
            )
            .unwrap();
        assert_eq!(before.body("old.bin"), Some(&b"old"[..]));
        assert_eq!(store.snapshot().body("moved.bin"), Some(&b"old"[..]));
        assert_eq!(store.snapshot().body("gone.bin"), None);
    }
    #[test]
    fn stale_cas_preserves_current_generation() {
        let temp = tempdir().unwrap();
        let executor = ContentStoreExecutor::new(temp.path()).unwrap();
        let mut store = executor.open("campaign").unwrap();
        let stale = store.snapshot();
        store
            .publish(
                stale.identity(),
                draft(
                    manifest(vec![("first.bin", b"one")]),
                    vec![ContentWrite::new("first.bin", b"one")],
                    vec![],
                    vec![],
                ),
            )
            .unwrap();
        assert!(matches!(
            store.publish(
                stale.identity(),
                draft(
                    manifest(vec![("other.bin", b"two")]),
                    vec![ContentWrite::new("other.bin", b"two")],
                    vec![],
                    vec![]
                )
            ),
            Err(ContentStoreExecutorError::WriteSet(
                ContentWriteSetError::StaleStore
            ))
        ));
        assert_eq!(store.snapshot().body("first.bin"), Some(&b"one"[..]));
    }
    #[test]
    fn orphan_generation_is_never_recovered_or_published() {
        let temp = tempdir().unwrap();
        let executor = ContentStoreExecutor::new(temp.path()).unwrap();
        let store = executor.open("campaign").unwrap();
        let state = temp.path().join("campaign").join(SERVICE_DIR);
        fs::create_dir(
            state
                .join(GENERATIONS_DIR)
                .join("gen-99999999999999999999-deadbeef"),
        )
        .unwrap();
        fs::create_dir(state.join(STAGING_DIR).join("discard.next")).unwrap();
        drop(store);
        let reopened = executor.open("campaign").unwrap();
        assert_eq!(reopened.snapshot().identity().revision, 0);
        assert!(!state
            .join(GENERATIONS_DIR)
            .join("gen-99999999999999999999-deadbeef")
            .exists());
        assert!(!state.join(STAGING_DIR).join("discard.next").exists());
    }
}
