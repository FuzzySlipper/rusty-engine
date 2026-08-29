//! Generated-ABI bridge for the Engine-owned content-store executor.
//!
//! C# supplies product meaning (the next artifact definitions and typed
//! operations). This bridge copies those borrowed rows, derives all stored
//! identities from retained bytes, then invokes the reusable executor.

use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::c_void,
    path::PathBuf,
    sync::Arc,
};

use content_store::{
    ArtifactClass, ArtifactRole, ContentArtifact, ContentDelete, ContentHash, ContentLoadPlan,
    ContentLoadStage, ContentManifest, ContentMove, ContentStoreIdentity, ContentWrite,
    ContentWriteSetDraft,
};
use csharp_engine_abi::*;
use content_store_host::{
    ContentStore, ContentStoreExecutor, ContentStoreExecutorError, ContentStoreSnapshot,
};

use crate::{
    composition::{borrowed_utf8, CsharpEngineServicesError, ABI_OK},
    persistence::borrowed_bytes,
};

const MAX_READ_BYTES: usize = 1024 * 1024;

struct RetainedStore {
    scope: String,
    store: ContentStore,
}
struct SnapshotLease {
    // Own every byte referenced by the ABI rows until exact lease release.
    _paths: Vec<String>,
    _resource_roles: Vec<String>,
    artifacts: Vec<NativeContentStoreArtifactReadoutRow>,
    _load_paths: Vec<String>,
    load_plan: Vec<NativeContentStoreLoadPlanRow>,
}

pub(crate) struct RuntimeContentStoreBridge {
    executor: Option<ContentStoreExecutor>,
    stores: BTreeMap<u64, RetainedStore>,
    snapshots: BTreeMap<u64, ContentStoreSnapshot>,
    snapshot_leases: BTreeMap<u64, SnapshotLease>,
    byte_leases: BTreeMap<u64, Arc<[u8]>>,
    next_store: u64,
    next_snapshot: u64,
    next_snapshot_lease: u64,
    next_byte_lease: u64,
}

/// Result of replacing one exact durable owner artifact through the existing
/// ContentStore publication path. The body hash is present only on a real
/// publication; stale mirrors the public ContentStore callback by returning
/// the refreshed observed identity instead of claiming a write succeeded.
pub(crate) enum SemanticOwnerPublish {
    Published {
        identity: ContentStoreIdentity,
        body_hash: ContentHash,
    },
    Stale {
        identity: ContentStoreIdentity,
    },
}

impl RuntimeContentStoreBridge {
    pub(crate) fn new(
        content_store_root: Option<PathBuf>,
    ) -> Result<Self, CsharpEngineServicesError> {
        // Runtime admission has already made this an explicit absolute root.
        // A configured root must never silently degrade into an absent service.
        let executor = content_store_root
            .map(ContentStoreExecutor::new)
            .transpose()
            .map_err(|error| {
                CsharpEngineServicesError::new("CSHARP_CONTENT_STORE_ROOT", error.to_string())
            })?;
        Ok(Self {
            executor,
            stores: BTreeMap::new(),
            snapshots: BTreeMap::new(),
            snapshot_leases: BTreeMap::new(),
            byte_leases: BTreeMap::new(),
            next_store: 1,
            next_snapshot: 1,
            next_snapshot_lease: 1,
            next_byte_lease: 1,
        })
    }
    fn next(counter: &mut u64) -> Option<u64> {
        let value = *counter;
        *counter = value.checked_add(1)?;
        Some(value)
    }
    fn retain_snapshot(
        &mut self,
        snapshot: ContentStoreSnapshot,
    ) -> Option<NativeContentStoreSnapshotHandle> {
        let value = Self::next(&mut self.next_snapshot)?;
        self.snapshots.insert(value, snapshot);
        Some(NativeContentStoreSnapshotHandle { value })
    }
    fn retain_bytes(&mut self, bytes: Arc<[u8]>) -> Option<NativeByteLease> {
        let value = Self::next(&mut self.next_byte_lease)?;
        let lease = NativeByteLease {
            handle: NativeByteLeaseHandle { value },
            bytes: bytes.as_ptr(),
            len: bytes.len(),
        };
        self.byte_leases.insert(value, bytes);
        Some(lease)
    }
    fn retain_snapshot_lease(
        &mut self,
        snapshot: &ContentStoreSnapshot,
    ) -> Option<NativeContentStoreSnapshotLease> {
        let value = Self::next(&mut self.next_snapshot_lease)?;
        let manifest = snapshot.manifest();
        let paths: Vec<String> = manifest
            .artifacts
            .iter()
            .map(|artifact| artifact.path.clone())
            .collect();
        let roles: Vec<String> = manifest
            .artifacts
            .iter()
            .map(|artifact| resource_role(&artifact.role))
            .collect();
        let artifacts = manifest
            .artifacts
            .iter()
            .enumerate()
            .map(|(index, artifact)| NativeContentStoreArtifactReadoutRow {
                path: native_utf8(&paths[index]),
                class: native_class(artifact.class),
                role_kind: native_role_kind(&artifact.role),
                resource_role: native_utf8(&roles[index]),
                has_hash: artifact.content_hash.is_some(),
                hash: artifact
                    .content_hash
                    .map(native_store_hash)
                    .unwrap_or_default(),
                has_byte_length: artifact.byte_len.is_some(),
                byte_length: artifact.byte_len.unwrap_or_default(),
            })
            .collect();
        let plan = ContentLoadPlan::build(manifest).ok()?;
        let load_paths: Vec<String> = plan.steps.iter().map(|step| step.path.clone()).collect();
        let load_plan = plan
            .steps
            .iter()
            .enumerate()
            .map(|(index, step)| NativeContentStoreLoadPlanRow {
                path: native_utf8(&load_paths[index]),
                stage: native_load_stage(step.stage),
            })
            .collect();
        self.snapshot_leases.insert(
            value,
            SnapshotLease {
                _paths: paths,
                _resource_roles: roles,
                artifacts,
                _load_paths: load_paths,
                load_plan,
            },
        );
        let retained = self.snapshot_leases.get(&value)?;
        Some(NativeContentStoreSnapshotLease {
            handle: NativeContentStoreSnapshotLeaseHandle { value },
            identity: native_identity(snapshot.identity()),
            artifacts: retained.artifacts.as_ptr(),
            artifacts_len: retained.artifacts.len(),
            load_plan: retained.load_plan.as_ptr(),
            load_plan_len: retained.load_plan.len(),
        })
    }

    /// Publish exactly one typed durable owner body while preserving every
    /// other artifact and body in the current admitted generation. This is
    /// deliberately crate-private composition, not a parallel C# manifest or
    /// CAS API.
    pub(crate) fn publish_semantic_owner(
        &mut self,
        store: NativeContentStoreHandle,
        expected: ContentStoreIdentity,
        path: String,
        role: ArtifactRole,
        body: Vec<u8>,
    ) -> Result<SemanticOwnerPublish, String> {
        let retained = self
            .stores
            .get_mut(&store.value)
            .ok_or_else(|| "unknown content store handle".to_owned())?;
        let prior = retained.store.snapshot();
        if let Some(existing) = prior.manifest().artifact(&path) {
            if existing.class != ArtifactClass::Durable || existing.role != role {
                return Err(
                    "semantic artifact path is occupied by a different durable role".to_owned(),
                );
            }
        }
        let mut artifacts = prior.manifest().artifacts.clone();
        let replacement = ContentArtifact::durable(path.clone(), role, &body);
        match artifacts.iter_mut().find(|artifact| artifact.path == path) {
            Some(existing) => *existing = replacement,
            None => artifacts.push(replacement),
        }
        let body_hash = ContentHash::of(&body);
        let draft = ContentWriteSetDraft {
            next_manifest: ContentManifest::new(artifacts),
            writes: vec![ContentWrite::new(path, body)],
            moves: Vec::new(),
            deletes: Vec::new(),
        };
        match retained.store.publish(&expected, draft) {
            Ok(confirmation) => Ok(SemanticOwnerPublish::Published {
                identity: confirmation.identity,
                body_hash,
            }),
            Err(ContentStoreExecutorError::WriteSet(
                content_store::ContentWriteSetError::StaleStore,
            )) => {
                let executor = self
                    .executor
                    .as_ref()
                    .ok_or_else(|| "content store executor is unavailable".to_owned())?;
                let observed = executor
                    .open(&retained.scope)
                    .map_err(|error| error.to_string())?;
                let identity = observed.snapshot().identity().clone();
                retained.store = observed;
                Ok(SemanticOwnerPublish::Stale { identity })
            }
            Err(error) => Err(error.to_string()),
        }
    }

    /// Copies one exact stored body only after identity, path, durable role,
    /// and body hash agree with the caller's typed selection.
    pub(crate) fn semantic_owner_body(
        &self,
        snapshot: NativeContentStoreSnapshotHandle,
        expected: ContentStoreIdentity,
        path: &str,
        role: ArtifactRole,
        body_hash: ContentHash,
    ) -> Result<Vec<u8>, String> {
        let snapshot = self
            .snapshots
            .get(&snapshot.value)
            .ok_or_else(|| "unknown content store snapshot handle".to_owned())?;
        if snapshot.identity() != &expected {
            return Err("content store snapshot identity does not match request".to_owned());
        }
        let artifact = snapshot
            .manifest()
            .artifact(path)
            .ok_or_else(|| "semantic artifact path is absent from snapshot".to_owned())?;
        if artifact.class != ArtifactClass::Durable || artifact.role != role {
            return Err("semantic artifact role does not match typed owner".to_owned());
        }
        if artifact.content_hash != Some(body_hash) {
            return Err("semantic artifact body hash does not match request".to_owned());
        }
        let body = snapshot
            .body(path)
            .ok_or_else(|| "semantic artifact body is absent from snapshot".to_owned())?;
        if ContentHash::of(body) != body_hash {
            return Err("semantic artifact body hash does not match stored bytes".to_owned());
        }
        Ok(body.to_vec())
    }
}

pub(crate) fn api(bridge: &mut RuntimeContentStoreBridge) -> NativeContentStoreApi {
    NativeContentStoreApi {
        context: (bridge as *mut RuntimeContentStoreBridge).cast(),
        open_store,
        destroy_store,
        capture_snapshot,
        destroy_snapshot,
        read_snapshot,
        destroy_snapshot_lease,
        read_body,
        destroy_byte_lease,
        publish,
    }
}

unsafe extern "C" fn open_store(
    context: *mut c_void,
    request: *const NativeContentStoreOpenRequest,
    result: *mut NativeContentStoreHandle,
) -> i32 {
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let scope = match unsafe {
        borrowed_utf8(
            (*request).scope.bytes,
            (*request).scope.len,
            "content store scope",
        )
    } {
        Ok(value) => value.to_owned(),
        Err(_) => return 0,
    };
    let bridge = unsafe { &mut *context.cast::<RuntimeContentStoreBridge>() };
    let Some(executor) = bridge.executor.as_ref() else {
        return 0;
    };
    let Ok(store) = executor.open(&scope) else {
        return 0;
    };
    let Some(value) = RuntimeContentStoreBridge::next(&mut bridge.next_store) else {
        return 0;
    };
    bridge.stores.insert(value, RetainedStore { scope, store });
    unsafe {
        *result = NativeContentStoreHandle { value };
    }
    ABI_OK
}
unsafe extern "C" fn destroy_store(context: *mut c_void, store: NativeContentStoreHandle) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeContentStoreBridge>() };
    if bridge.stores.remove(&store.value).is_some() {
        ABI_OK
    } else {
        0
    }
}
unsafe extern "C" fn capture_snapshot(
    context: *mut c_void,
    store: NativeContentStoreHandle,
    result: *mut NativeContentStoreSnapshotHandle,
) -> i32 {
    if context.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeContentStoreBridge>() };
    let Some(snapshot) = bridge
        .stores
        .get(&store.value)
        .map(|value| value.store.snapshot())
    else {
        return 0;
    };
    let Some(handle) = bridge.retain_snapshot(snapshot) else {
        return 0;
    };
    unsafe {
        *result = handle;
    }
    ABI_OK
}
unsafe extern "C" fn destroy_snapshot(
    context: *mut c_void,
    snapshot: NativeContentStoreSnapshotHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeContentStoreBridge>() };
    if bridge.snapshots.remove(&snapshot.value).is_some() {
        ABI_OK
    } else {
        0
    }
}
unsafe extern "C" fn read_snapshot(
    context: *mut c_void,
    snapshot: NativeContentStoreSnapshotHandle,
    result: *mut NativeContentStoreSnapshotLease,
) -> i32 {
    if context.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeContentStoreBridge>() };
    let Some(snapshot) = bridge.snapshots.get(&snapshot.value).cloned() else {
        return 0;
    };
    let Some(lease) = bridge.retain_snapshot_lease(&snapshot) else {
        return 0;
    };
    unsafe {
        *result = lease;
    }
    ABI_OK
}
unsafe extern "C" fn destroy_snapshot_lease(
    context: *mut c_void,
    lease: NativeContentStoreSnapshotLeaseHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeContentStoreBridge>() };
    if bridge.snapshot_leases.remove(&lease.value).is_some() {
        ABI_OK
    } else {
        0
    }
}
unsafe extern "C" fn read_body(
    context: *mut c_void,
    request: *const NativeContentStoreBodyRequest,
    result: *mut NativeByteLease,
) -> i32 {
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let request = unsafe { &*request };
    if request.max_bytes as usize > MAX_READ_BYTES {
        return 0;
    }
    let path = match unsafe {
        borrowed_utf8(
            request.path.bytes,
            request.path.len,
            "content store body path",
        )
    } {
        Ok(value) => value,
        Err(_) => return 0,
    };
    let bridge = unsafe { &mut *context.cast::<RuntimeContentStoreBridge>() };
    let Some(snapshot) = bridge.snapshots.get(&request.snapshot.value) else {
        return 0;
    };
    let Some(body) = snapshot.body(path) else {
        return 0;
    };
    let Ok(offset) = usize::try_from(request.offset) else {
        return 0;
    };
    if offset > body.len() {
        return 0;
    }
    let len = (request.max_bytes as usize).min(body.len() - offset);
    let Some(lease) = bridge.retain_bytes(Arc::from(body[offset..offset + len].to_vec())) else {
        return 0;
    };
    unsafe {
        *result = lease;
    }
    ABI_OK
}
unsafe extern "C" fn destroy_byte_lease(context: *mut c_void, lease: NativeByteLeaseHandle) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeContentStoreBridge>() };
    if bridge.byte_leases.remove(&lease.value).is_some() {
        ABI_OK
    } else {
        0
    }
}
unsafe extern "C" fn publish(
    context: *mut c_void,
    request: *const NativeContentStorePublishRequest,
    receipt: *mut NativeContentStorePublishReceipt,
) -> i32 {
    if context.is_null() || request.is_null() || receipt.is_null() {
        return 0;
    }
    let request = unsafe { &*request };
    let draft =
        match unsafe { draft_from_request(request, &*context.cast::<RuntimeContentStoreBridge>()) }
        {
            Ok(value) => value,
            Err(()) => return 0,
        };
    let expected = from_native_identity(request.expected);
    let bridge = unsafe { &mut *context.cast::<RuntimeContentStoreBridge>() };
    let Some(retained) = bridge.stores.get_mut(&request.store.value) else {
        return 0;
    };
    match retained.store.publish(&expected, draft) {
        Ok(confirmation) => {
            unsafe {
                *receipt = NativeContentStorePublishReceipt {
                    status: NativeContentStorePublishStatus::Published,
                    identity: native_identity(&confirmation.identity),
                    candidate_hash: native_hash(confirmation.candidate_hash),
                };
            }
            ABI_OK
        }
        Err(ContentStoreExecutorError::WriteSet(
            content_store::ContentWriteSetError::StaleStore,
        )) => {
            let Some(executor) = bridge.executor.as_ref() else {
                return 0;
            };
            let Ok(observed) = executor.open(&retained.scope) else {
                return 0;
            };
            let identity = native_identity(observed.snapshot().identity());
            retained.store = observed;
            unsafe {
                *receipt = NativeContentStorePublishReceipt {
                    status: NativeContentStorePublishStatus::Stale,
                    identity,
                    candidate_hash: NativeContentSha256::default(),
                };
            }
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe fn draft_from_request(
    request: &NativeContentStorePublishRequest,
    bridge: &RuntimeContentStoreBridge,
) -> Result<ContentWriteSetDraft, ()> {
    let store = bridge.stores.get(&request.store.value).ok_or(())?;
    let artifacts = unsafe { borrowed_rows(request.artifacts, request.artifacts_len) }?
        .iter()
        .map(|row| {
            let path = unsafe { borrowed_str(row.path, "content artifact path") }?.to_owned();
            let role = role_from_native(row.role_kind, unsafe {
                borrowed_str(row.resource_role, "content artifact role")
            }?)?;
            Ok((path, class_from_native(row.class)?, role))
        })
        .collect::<Result<Vec<_>, ()>>()?;
    let writes = unsafe { borrowed_rows(request.writes, request.writes_len) }?
        .iter()
        .map(|row| {
            Ok(ContentWrite::new(
                unsafe { borrowed_str(row.path, "content write path") }?,
                unsafe { borrowed_bytes(row.bytes, "content write bytes") }?.to_vec(),
            ))
        })
        .collect::<Result<Vec<_>, ()>>()?;
    let prior = store.store.snapshot();
    let moves = unsafe { borrowed_rows(request.moves, request.moves_len) }?
        .iter()
        .map(|row| {
            let from = unsafe { borrowed_str(row.from, "content move source") }?.to_owned();
            Ok(ContentMove {
                expected_content_hash: prior.manifest().artifact(&from).ok_or(())?.content_hash,
                from,
                to: unsafe { borrowed_str(row.to, "content move target") }?.to_owned(),
            })
        })
        .collect::<Result<Vec<_>, ()>>()?;
    let deletes = unsafe { borrowed_rows(request.deletes, request.deletes_len) }?
        .iter()
        .map(|row| {
            let path = unsafe { borrowed_str(row.path, "content delete path") }?.to_owned();
            Ok(ContentDelete {
                expected_content_hash: prior.manifest().artifact(&path).ok_or(())?.content_hash,
                path,
            })
        })
        .collect::<Result<Vec<_>, ()>>()?;
    let bodies = derived_next_bodies(store.store.snapshot(), &writes, &moves, &deletes)?;
    let artifacts = artifacts
        .into_iter()
        .map(|(path, class, role)| match class {
            ArtifactClass::Cache => Ok(ContentArtifact::cache(path)),
            class => {
                let bytes = bodies.get(&path).ok_or(())?;
                Ok(ContentArtifact {
                    path,
                    class,
                    role,
                    content_hash: Some(ContentHash::of(bytes)),
                    byte_len: Some(bytes.len() as u64),
                })
            }
        })
        .collect::<Result<Vec<_>, ()>>()?;
    Ok(ContentWriteSetDraft {
        next_manifest: ContentManifest::new(artifacts),
        writes,
        moves,
        deletes,
    })
}
fn derived_next_bodies(
    snapshot: ContentStoreSnapshot,
    writes: &[ContentWrite],
    moves: &[ContentMove],
    deletes: &[ContentDelete],
) -> Result<BTreeMap<String, Vec<u8>>, ()> {
    let mut bodies = snapshot
        .bodies()
        .map(|(path, body)| (path.to_owned(), body.to_vec()))
        .collect::<BTreeMap<_, _>>();
    let mut targets = BTreeSet::new();
    for delete in deletes {
        if !targets.insert(delete.path.clone()) || bodies.remove(&delete.path).is_none() {
            return Err(());
        }
    }
    for movement in moves {
        if !targets.insert(movement.to.clone()) {
            return Err(());
        }
        let body = bodies.remove(&movement.from).ok_or(())?;
        if bodies.insert(movement.to.clone(), body).is_some() {
            return Err(());
        }
    }
    for write in writes {
        if !targets.insert(write.path().to_owned()) {
            return Err(());
        }
        bodies.insert(write.path().to_owned(), write.bytes().to_vec());
    }
    Ok(bodies)
}
unsafe fn borrowed_rows<'a, T>(pointer: *const T, len: usize) -> Result<&'a [T], ()> {
    if len > 0 && pointer.is_null() {
        return Err(());
    }
    Ok(if len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(pointer, len) }
    })
}
unsafe fn borrowed_str<'a>(value: NativeUtf8Slice, field: &'static str) -> Result<&'a str, ()> {
    unsafe { borrowed_utf8(value.bytes, value.len, field) }.map_err(|_| ())
}
fn native_utf8(value: &str) -> NativeUtf8Slice {
    NativeUtf8Slice {
        bytes: value.as_ptr(),
        len: value.len(),
    }
}
pub(crate) fn native_hash(value: ContentHash) -> NativeContentSha256 {
    let bytes = value.as_bytes();
    NativeContentSha256 {
        word0: u64::from_be_bytes(bytes[0..8].try_into().unwrap()),
        word1: u64::from_be_bytes(bytes[8..16].try_into().unwrap()),
        word2: u64::from_be_bytes(bytes[16..24].try_into().unwrap()),
        word3: u64::from_be_bytes(bytes[24..32].try_into().unwrap()),
    }
}
pub(crate) fn native_store_hash(value: ContentHash) -> NativeContentStoreHash {
    let hash = native_hash(value);
    NativeContentStoreHash {
        word0: hash.word0,
        word1: hash.word1,
        word2: hash.word2,
        word3: hash.word3,
    }
}
pub(crate) fn from_native_store_hash(value: NativeContentStoreHash) -> ContentHash {
    from_native_hash(NativeContentSha256 {
        word0: value.word0,
        word1: value.word1,
        word2: value.word2,
        word3: value.word3,
    })
}
pub(crate) fn from_native_hash(value: NativeContentSha256) -> ContentHash {
    let mut bytes = [0; 32];
    bytes[0..8].copy_from_slice(&value.word0.to_be_bytes());
    bytes[8..16].copy_from_slice(&value.word1.to_be_bytes());
    bytes[16..24].copy_from_slice(&value.word2.to_be_bytes());
    bytes[24..32].copy_from_slice(&value.word3.to_be_bytes());
    ContentHash::parse(
        &bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
    )
    .expect("native SHA-256 always encodes")
}
pub(crate) fn native_identity(value: &ContentStoreIdentity) -> NativeContentStoreIdentity {
    NativeContentStoreIdentity {
        revision: value.revision,
        manifest_hash: native_hash(value.manifest_hash),
        content_set_hash: native_hash(value.content_set_hash),
    }
}
pub(crate) fn from_native_identity(value: NativeContentStoreIdentity) -> ContentStoreIdentity {
    ContentStoreIdentity {
        revision: value.revision,
        manifest_hash: from_native_hash(value.manifest_hash),
        content_set_hash: from_native_hash(value.content_set_hash),
    }
}
fn native_class(value: ArtifactClass) -> NativeContentStoreArtifactClass {
    match value {
        ArtifactClass::Durable => NativeContentStoreArtifactClass::Durable,
        ArtifactClass::Generated => NativeContentStoreArtifactClass::Generated,
        ArtifactClass::Cache => NativeContentStoreArtifactClass::Cache,
    }
}
fn class_from_native(value: NativeContentStoreArtifactClass) -> Result<ArtifactClass, ()> {
    match value {
        NativeContentStoreArtifactClass::Durable => Ok(ArtifactClass::Durable),
        NativeContentStoreArtifactClass::Generated => Ok(ArtifactClass::Generated),
        NativeContentStoreArtifactClass::Cache => Ok(ArtifactClass::Cache),
    }
}
fn native_role_kind(value: &ArtifactRole) -> NativeContentStoreArtifactRoleKind {
    match value {
        ArtifactRole::AssetCatalog => NativeContentStoreArtifactRoleKind::AssetCatalog,
        ArtifactRole::AssetLock => NativeContentStoreArtifactRoleKind::AssetLock,
        ArtifactRole::SceneDocument => NativeContentStoreArtifactRoleKind::SceneDocument,
        ArtifactRole::PrefabRegistry => NativeContentStoreArtifactRoleKind::PrefabRegistry,
        ArtifactRole::EntityStateSnapshot => {
            NativeContentStoreArtifactRoleKind::EntityStateSnapshot
        }
        ArtifactRole::VoxelAsset => NativeContentStoreArtifactRoleKind::VoxelAsset,
        ArtifactRole::VoxelObject => NativeContentStoreArtifactRoleKind::VoxelObject,
        ArtifactRole::VoxelAnnotation => NativeContentStoreArtifactRoleKind::VoxelAnnotation,
        ArtifactRole::ImportedAsset => NativeContentStoreArtifactRoleKind::ImportedAsset,
        ArtifactRole::GeneratedMetadata => NativeContentStoreArtifactRoleKind::GeneratedMetadata,
        ArtifactRole::Resource(_) => NativeContentStoreArtifactRoleKind::Resource,
        ArtifactRole::Cache => NativeContentStoreArtifactRoleKind::Cache,
    }
}
fn resource_role(value: &ArtifactRole) -> String {
    match value {
        ArtifactRole::Resource(value) => value.clone(),
        _ => String::new(),
    }
}
fn role_from_native(
    value: NativeContentStoreArtifactRoleKind,
    resource: &str,
) -> Result<ArtifactRole, ()> {
    match value {
        NativeContentStoreArtifactRoleKind::AssetCatalog if resource.is_empty() => {
            Ok(ArtifactRole::AssetCatalog)
        }
        NativeContentStoreArtifactRoleKind::AssetLock if resource.is_empty() => {
            Ok(ArtifactRole::AssetLock)
        }
        NativeContentStoreArtifactRoleKind::SceneDocument if resource.is_empty() => {
            Ok(ArtifactRole::SceneDocument)
        }
        NativeContentStoreArtifactRoleKind::PrefabRegistry if resource.is_empty() => {
            Ok(ArtifactRole::PrefabRegistry)
        }
        NativeContentStoreArtifactRoleKind::EntityStateSnapshot if resource.is_empty() => {
            Ok(ArtifactRole::EntityStateSnapshot)
        }
        NativeContentStoreArtifactRoleKind::VoxelAsset if resource.is_empty() => {
            Ok(ArtifactRole::VoxelAsset)
        }
        NativeContentStoreArtifactRoleKind::VoxelObject if resource.is_empty() => {
            Ok(ArtifactRole::VoxelObject)
        }
        NativeContentStoreArtifactRoleKind::VoxelAnnotation if resource.is_empty() => {
            Ok(ArtifactRole::VoxelAnnotation)
        }
        NativeContentStoreArtifactRoleKind::ImportedAsset if resource.is_empty() => {
            Ok(ArtifactRole::ImportedAsset)
        }
        NativeContentStoreArtifactRoleKind::GeneratedMetadata if resource.is_empty() => {
            Ok(ArtifactRole::GeneratedMetadata)
        }
        NativeContentStoreArtifactRoleKind::Resource if !resource.is_empty() => {
            Ok(ArtifactRole::Resource(resource.to_owned()))
        }
        NativeContentStoreArtifactRoleKind::Cache if resource.is_empty() => Ok(ArtifactRole::Cache),
        _ => Err(()),
    }
}
fn native_load_stage(value: ContentLoadStage) -> NativeContentStoreLoadStage {
    match value {
        ContentLoadStage::AssetAuthority => NativeContentStoreLoadStage::AssetAuthority,
        ContentLoadStage::AssetData => NativeContentStoreLoadStage::AssetData,
        ContentLoadStage::Annotations => NativeContentStoreLoadStage::Annotations,
        ContentLoadStage::Prefabs => NativeContentStoreLoadStage::Prefabs,
        ContentLoadStage::Scenes => NativeContentStoreLoadStage::Scenes,
        ContentLoadStage::EntityState => NativeContentStoreLoadStage::EntityState,
        ContentLoadStage::Resources => NativeContentStoreLoadStage::Resources,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(value: &[u8]) -> NativeUtf8Slice {
        NativeUtf8Slice {
            bytes: value.as_ptr(),
            len: value.len(),
        }
    }

    #[test]
    fn executes_typed_write_and_retains_old_snapshot_across_stale_publish() {
        let root = tempfile::tempdir().unwrap();
        let mut bridge = RuntimeContentStoreBridge::new(Some(root.path().to_path_buf())).unwrap();
        let context = (&mut bridge as *mut RuntimeContentStoreBridge).cast();
        let scope = b"fixture";
        let mut store = NativeContentStoreHandle::default();
        assert_eq!(
            unsafe {
                open_store(
                    context,
                    &NativeContentStoreOpenRequest { scope: text(scope) },
                    &mut store,
                )
            },
            ABI_OK
        );
        let mut old = NativeContentStoreSnapshotHandle::default();
        assert_eq!(
            unsafe { capture_snapshot(context, store, &mut old) },
            ABI_OK
        );
        let mut old_lease = NativeContentStoreSnapshotLease {
            handle: NativeContentStoreSnapshotLeaseHandle::default(),
            identity: NativeContentStoreIdentity::default(),
            artifacts: std::ptr::null(),
            artifacts_len: 0,
            load_plan: std::ptr::null(),
            load_plan_len: 0,
        };
        assert_eq!(
            unsafe { read_snapshot(context, old, &mut old_lease) },
            ABI_OK
        );
        assert_eq!(old_lease.identity.revision, 0);
        assert_eq!(old_lease.artifacts_len, 0);
        assert_eq!(
            unsafe { destroy_snapshot_lease(context, old_lease.handle) },
            ABI_OK
        );
        let path = b"state.bin";
        let role = b"resource:fixture-state";
        let body = b"retained typed bytes";
        let artifacts = [NativeContentStoreArtifactDefinition {
            path: text(path),
            class: NativeContentStoreArtifactClass::Durable,
            role_kind: NativeContentStoreArtifactRoleKind::Resource,
            resource_role: text(role),
        }];
        let writes = [NativeContentStoreWriteRow {
            path: text(path),
            bytes: NativeByteSlice {
                bytes: body.as_ptr(),
                len: body.len(),
            },
        }];
        let request = NativeContentStorePublishRequest {
            store,
            expected: old_lease.identity,
            artifacts: artifacts.as_ptr(),
            artifacts_len: artifacts.len(),
            writes: writes.as_ptr(),
            writes_len: writes.len(),
            moves: std::ptr::null(),
            moves_len: 0,
            deletes: std::ptr::null(),
            deletes_len: 0,
        };
        let mut published = NativeContentStorePublishReceipt {
            status: NativeContentStorePublishStatus::Stale,
            identity: NativeContentStoreIdentity::default(),
            candidate_hash: NativeContentSha256::default(),
        };
        assert_eq!(
            unsafe { publish(context, &request, &mut published) },
            ABI_OK
        );
        assert_eq!(published.status, NativeContentStorePublishStatus::Published);
        assert_eq!(published.identity.revision, 1);
        assert_ne!(published.candidate_hash, NativeContentSha256::default());
        let mut current = NativeContentStoreSnapshotHandle::default();
        assert_eq!(
            unsafe { capture_snapshot(context, store, &mut current) },
            ABI_OK
        );
        let body_request = NativeContentStoreBodyRequest {
            snapshot: current,
            path: text(path),
            offset: 0,
            max_bytes: 1024,
        };
        let mut bytes = NativeByteLease {
            handle: NativeByteLeaseHandle::default(),
            bytes: std::ptr::null(),
            len: 0,
        };
        assert_eq!(
            unsafe { read_body(context, &body_request, &mut bytes) },
            ABI_OK
        );
        assert_eq!(
            unsafe { std::slice::from_raw_parts(bytes.bytes, bytes.len) },
            body
        );
        assert_eq!(unsafe { destroy_byte_lease(context, bytes.handle) }, ABI_OK);
        let stale_request = NativeContentStorePublishRequest {
            expected: old_lease.identity,
            ..request
        };
        let mut stale = NativeContentStorePublishReceipt {
            status: NativeContentStorePublishStatus::Published,
            identity: NativeContentStoreIdentity::default(),
            candidate_hash: NativeContentSha256 {
                word0: 1,
                ..NativeContentSha256::default()
            },
        };
        assert_eq!(
            unsafe { publish(context, &stale_request, &mut stale) },
            ABI_OK
        );
        assert_eq!(stale.status, NativeContentStorePublishStatus::Stale);
        assert_eq!(stale.identity, published.identity);
        assert_eq!(stale.candidate_hash, NativeContentSha256::default());
        let mut old_after = NativeContentStoreSnapshotLease {
            handle: NativeContentStoreSnapshotLeaseHandle::default(),
            identity: NativeContentStoreIdentity::default(),
            artifacts: std::ptr::null(),
            artifacts_len: 0,
            load_plan: std::ptr::null(),
            load_plan_len: 0,
        };
        assert_eq!(
            unsafe { read_snapshot(context, old, &mut old_after) },
            ABI_OK
        );
        assert_eq!(old_after.identity.revision, 0);
        assert_eq!(
            unsafe { destroy_snapshot_lease(context, old_after.handle) },
            ABI_OK
        );
        assert_eq!(unsafe { destroy_snapshot(context, old) }, ABI_OK);
        assert_eq!(unsafe { destroy_snapshot(context, current) }, ABI_OK);
        assert_eq!(unsafe { destroy_store(context, store) }, ABI_OK);
    }
}
