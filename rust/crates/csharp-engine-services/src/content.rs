//! One Engine-owned immutable catalog of already-admitted product content.

use std::{
    collections::BTreeMap,
    ffi::c_void,
    sync::{Arc, OnceLock},
};

use crate::operation_diagnostics::{clear_receipt, refuse};
use csharp_engine_abi::*;
use sha2::{Digest, Sha256};

use crate::{composition::borrowed_utf8, composition::ABI_OK};

mod bundles;
mod portable;
pub use bundles::ProductContentBundles;

/// A body's SHA-256, computed when first requested and shared by every clone
/// of the admitted content, so unused files are never hashed and used files
/// are hashed once.
#[derive(Clone, Default)]
pub(crate) struct ContentIdentity(Arc<OnceLock<NativeContentSha256>>);

impl ContentIdentity {
    /// An identity the owner already verified, such as a bundle manifest hash.
    fn known(value: NativeContentSha256) -> Self {
        Self(Arc::new(OnceLock::from(value)))
    }

    /// `bytes` must be the body this identity was admitted with.
    pub(crate) fn of(&self, bytes: &[u8]) -> NativeContentSha256 {
        *self.0.get_or_init(|| sha256(bytes))
    }

    /// The identity as `sha256:<hex>`, the form renderer resources name.
    pub(crate) fn content_hash(&self, bytes: &[u8]) -> String {
        let digest = self.of(bytes);
        format!(
            "sha256:{:016x}{:016x}{:016x}{:016x}",
            digest.word0, digest.word1, digest.word2, digest.word3
        )
    }
}

/// The immutable files a source's relative dependencies resolve in: a
/// snapshot, or an open bundle whose bodies are read once, when first used.
#[derive(Clone)]
pub(crate) enum ContentFiles {
    Snapshot(Arc<BTreeMap<String, Arc<[u8]>>>),
    Bundle(Arc<bundles::BundleFiles>),
}

impl ContentFiles {
    pub(crate) fn snapshot(files: BTreeMap<String, Arc<[u8]>>) -> Self {
        Self::Snapshot(Arc::new(files))
    }

    /// The file's bytes, or none when there is no such file. A bundle file
    /// that cannot be read refuses, naming the file.
    pub(crate) fn get(
        &self,
        path: &str,
    ) -> Result<Option<Arc<[u8]>>, crate::composition::CsharpEngineServicesError> {
        match self {
            Self::Snapshot(files) => Ok(files.get(path).cloned()),
            Self::Bundle(files) => files.bytes(path),
        }
    }
}

#[derive(Clone)]
struct AdmittedContent {
    path: String,
    identity: ContentIdentity,
    bytes: Arc<[u8]>,
    transient: bool,
    files: ContentFiles,
}

impl AdmittedContent {
    fn sha256(&self) -> NativeContentSha256 {
        self.identity.of(&self.bytes)
    }
}

#[derive(Clone)]
pub(crate) struct RetainedContent {
    pub(crate) path: String,
    pub(crate) identity: ContentIdentity,
    pub(crate) bytes: Arc<[u8]>,
    pub(crate) transient: bool,
    /// Immutable dependency context of this source, never other open bundles.
    pub(crate) files: ContentFiles,
}

impl RetainedContent {
    pub(crate) fn sha256(&self) -> NativeContentSha256 {
        self.identity.of(&self.bytes)
    }
}

pub(crate) struct RuntimeContentBridge {
    portable: portable::PortableState,
    /// Every loose file by content-relative path: the dependency context of
    /// each catalog entry.
    snapshot: ContentFiles,
    catalog: BTreeMap<String, AdmittedContent>,
    references: BTreeMap<u64, AdmittedContent>,
    /// Backing of the latest borrowed Content result.
    borrowed: crate::operation_diagnostics::BorrowedResult,
    bundles: bundles::BundleState,
    next_reference: u64,
}

impl RuntimeContentBridge {
    pub(crate) fn new(content_resources: BTreeMap<String, Arc<[u8]>>) -> Self {
        let files = ContentFiles::snapshot(content_resources.clone());
        let catalog = content_resources
            .into_iter()
            .map(|(path, bytes)| {
                (
                    path.clone(),
                    AdmittedContent {
                        path,
                        identity: ContentIdentity::default(),
                        bytes,
                        transient: false,
                        files: files.clone(),
                    },
                )
            })
            .collect();
        Self {
            portable: portable::PortableState::default(),
            snapshot: files,
            catalog,
            references: BTreeMap::new(),
            borrowed: Default::default(),
            bundles: bundles::BundleState::default(),
            next_reference: 1,
        }
    }

    /// Re-admits changed loose files in place: later opens by path read the
    /// new bytes under new identities, in a new dependency snapshot. Open
    /// references keep the content they hold. Returns each file's path,
    /// identity and length.
    pub(crate) fn reload(
        &mut self,
        changed: BTreeMap<String, Arc<[u8]>>,
    ) -> Vec<(String, NativeContentSha256, u64)> {
        let ContentFiles::Snapshot(previous) = &self.snapshot else {
            unreachable!("loose content is a snapshot");
        };
        let mut files = (**previous).clone();
        files.extend(
            changed
                .iter()
                .map(|(path, bytes)| (path.clone(), Arc::clone(bytes))),
        );
        self.snapshot = ContentFiles::snapshot(files);
        for content in self.catalog.values_mut() {
            content.files = self.snapshot.clone();
        }
        changed
            .into_iter()
            .map(|(path, bytes)| {
                let content = AdmittedContent {
                    path: path.clone(),
                    identity: ContentIdentity::default(),
                    bytes,
                    transient: false,
                    files: self.snapshot.clone(),
                };
                let identity = (content.sha256(), content.bytes.len() as u64);
                self.catalog.insert(path.clone(), content);
                (path, identity.0, identity.1)
            })
            .collect()
    }

    fn retain(&mut self, content: AdmittedContent) -> NativeContentReferenceHandle {
        let value = self.next_reference;
        self.next_reference += 1;
        self.references.insert(value, content);
        NativeContentReferenceHandle { value }
    }

    /// Engine-internal composition seam for semantic owners. The retained
    /// content handle remains authoritative; callers receive a cheap immutable
    /// clone and never route its bytes through the C# ABI.
    pub(crate) fn retained_bytes(
        &self,
        reference: NativeContentReferenceHandle,
    ) -> Option<Arc<[u8]>> {
        self.references
            .get(&reference.value)
            .map(|content| Arc::clone(&content.bytes))
    }

    pub(crate) fn retained_content(
        &self,
        reference: NativeContentReferenceHandle,
    ) -> Option<RetainedContent> {
        self.references
            .get(&reference.value)
            .map(|content| RetainedContent {
                path: content.path.clone(),
                identity: content.identity.clone(),
                bytes: Arc::clone(&content.bytes),
                transient: content.transient,
                files: content.files.clone(),
            })
    }

    /// Ordinary loose content by content-root-relative path; bundles require
    /// a reference so another open collection cannot change path resolution.
    pub(crate) fn retained_path(&self, path: &str) -> Option<RetainedContent> {
        let content = self.catalog.get(path)?;
        Some(RetainedContent {
            path: content.path.clone(),
            identity: content.identity.clone(),
            bytes: Arc::clone(&content.bytes),
            transient: content.transient,
            files: content.files.clone(),
        })
    }

    fn read_info(
        &mut self,
        reference: NativeContentReferenceHandle,
    ) -> Option<NativeContentReferenceInfoResult> {
        let content = self.references.get(&reference.value)?.clone();
        self.retain_info(vec![(
            content.path.clone(),
            content.sha256(),
            content.bytes.len() as u64,
        )])
    }

    fn retain_info(
        &mut self,
        entries: Vec<(String, NativeContentSha256, u64)>,
    ) -> Option<NativeContentReferenceInfoResult> {
        let paths: Vec<String> = entries.iter().map(|entry| entry.0.clone()).collect();
        let references = paths
            .iter()
            .zip(entries)
            .map(
                |(path, (_, sha256, byte_length))| NativeContentReferenceInfo {
                    path: NativeUtf8Slice {
                        bytes: path.as_ptr(),
                        len: path.len(),
                    },
                    sha256,
                    byte_length,
                },
            )
            .collect::<Vec<_>>();
        let result = NativeContentReferenceInfoResult {
            references: references.as_ptr(),
            references_len: references.len(),
        };
        self.borrowed.hold((paths, references));
        Some(result)
    }

    pub(crate) fn bind_bundles(&mut self, bundles: ProductContentBundles) {
        self.bundles.source = bundles;
    }

    fn read_bytes(&mut self, request: NativeContentReadBytesRequest) -> Option<NativeByteResult> {
        let content = self.references.get(&request.reference.value)?;
        let offset = usize::try_from(request.offset).ok()?;
        if offset > content.bytes.len() {
            return None;
        }
        let len = usize::try_from(request.max_bytes)
            .ok()?
            .min(content.bytes.len().saturating_sub(offset));
        // The reference stays retained until a later call closes it, so the
        // result borrows its bytes directly.
        Some(NativeByteResult {
            bytes: content.bytes[offset..].as_ptr(),
            len,
        })
    }
}

pub(crate) fn api(bridge: &mut RuntimeContentBridge) -> NativeContentApi {
    NativeContentApi {
        load_portable_asset: portable::load,
        destroy_portable_asset: portable::destroy,
        read_portable_asset: portable::read,
        open_portable_asset_member: portable::open_member,
        context: (bridge as *mut RuntimeContentBridge).cast(),
        list_bundles: bundles::list_bundles,
        open_bundle: bundles::open_bundle,
        open_container: bundles::open_container,
        pack_container: bundles::pack_container,
        read_bundle_identity: bundles::read_bundle_identity,
        destroy_bundle: bundles::destroy_bundle,
        read_bundle_files: bundles::read_bundle_files,
        open_bundle_reference: bundles::open_bundle_reference,
        admit_reference,
        open_reference,
        resolve_reference,
        destroy_reference,
        read_reference_info,
        read_bytes,
    }
}

/// Admission owns the only copy across the ABI. Normal decoders validate their
/// format when a resource is opened; no GLB/schema policy lives in Content.
/// The product's paths only key the source's dependency files: a dependency
/// is found by the exact path its source resolves, and a repeated path keeps
/// its last bytes.
pub(crate) unsafe extern "C" fn admit_reference(
    context: *mut c_void,
    request: *const NativeContentAdmissionRequest,
    result: *mut NativeContentReferenceHandle,
) -> i32 {
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let request = unsafe { &*request };
    let read_file = |path: NativeUtf8Slice,
                     bytes: NativeByteSlice|
     -> Option<(String, Arc<[u8]>)> {
        let path = unsafe { borrowed_utf8(path.bytes, path.len, "content path") }.ok()?;
        let bytes =
            unsafe { crate::composition::borrowed_slice(bytes.bytes, bytes.len, "content bytes") }
                .ok()?;
        Some((path.to_owned(), Arc::from(bytes)))
    };
    let Some((path, bytes)) = read_file(request.path, request.bytes) else {
        return 0;
    };
    let Ok(dependencies) = (unsafe {
        crate::composition::borrowed_slice(
            request.dependencies,
            request.dependencies_len,
            "content dependencies",
        )
    }) else {
        return 0;
    };
    let mut files = BTreeMap::from([(path.clone(), Arc::clone(&bytes))]);
    for dependency in dependencies {
        let Some((path, bytes)) = read_file(dependency.path, dependency.bytes) else {
            return 0;
        };
        files.insert(path, bytes);
    }
    let content = AdmittedContent {
        path,
        identity: ContentIdentity::default(),
        bytes,
        transient: true,
        files: ContentFiles::snapshot(files),
    };
    let bridge = unsafe { &mut *context.cast::<RuntimeContentBridge>() };
    let handle = bridge.retain(content);
    unsafe { *result = handle };
    ABI_OK
}

unsafe extern "C" fn open_reference(
    context: *mut c_void,
    request: *const NativeContentOpenRequest,
    result: *mut NativeContentReferenceHandle,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(error);
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let request = unsafe { &*request };
    let path = match unsafe { borrowed_utf8(request.path.bytes, request.path.len, "content path") }
    {
        Ok(path) => path,
        Err(refusal) => return refuse(&refusal, error),
    };
    let bridge = unsafe { &mut *context.cast::<RuntimeContentBridge>() };
    let Some(content) = bridge.catalog.get(path).cloned() else {
        return 0;
    };
    let handle = bridge.retain(content);
    unsafe { *result = handle };
    ABI_OK
}

unsafe extern "C" fn resolve_reference(
    context: *mut c_void,
    request: *const NativeContentResolveRequest,
    result: *mut NativeContentReferenceHandle,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(error);
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let request = unsafe { &*request };
    let path = match unsafe { borrowed_utf8(request.path.bytes, request.path.len, "content path") }
    {
        Ok(path) => path,
        Err(refusal) => return refuse(&refusal, error),
    };
    let bridge = unsafe { &mut *context.cast::<RuntimeContentBridge>() };
    let catalog = bridge
        .catalog
        .get(path)
        .filter(|content| content.sha256() == request.sha256)
        .cloned();
    let content = match catalog {
        Some(content) => content,
        None => match bridge.bundles.resolve(path, request.sha256) {
            Ok(Some(content)) => content,
            Ok(None) => return 0,
            Err(refusal) => return refuse(&refusal, error),
        },
    };
    let handle = bridge.retain(content);
    unsafe { *result = handle };
    ABI_OK
}

unsafe extern "C" fn destroy_reference(
    context: *mut c_void,
    reference: NativeContentReferenceHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeContentBridge>() };
    if bridge.references.remove(&reference.value).is_some() {
        ABI_OK
    } else {
        0
    }
}

unsafe extern "C" fn read_reference_info(
    context: *mut c_void,
    reference: NativeContentReferenceHandle,
    result: *mut NativeContentReferenceInfoResult,
) -> i32 {
    if context.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeContentBridge>() };
    let Some(value) = bridge.read_info(reference) else {
        return 0;
    };
    unsafe { *result = value };
    ABI_OK
}

unsafe extern "C" fn read_bytes(
    context: *mut c_void,
    request: *const NativeContentReadBytesRequest,
    result: *mut NativeByteResult,
) -> i32 {
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeContentBridge>() };
    let Some(value) = bridge.read_bytes(unsafe { *request }) else {
        return 0;
    };
    unsafe { *result = value };
    ABI_OK
}

fn sha256(bytes: &[u8]) -> NativeContentSha256 {
    #[cfg(test)]
    tests::BODY_HASHES.with(|count| count.set(count.get() + 1));
    sha256_words(&Sha256::digest(bytes))
}

fn sha256_words(digest: &[u8]) -> NativeContentSha256 {
    let word =
        |start| u64::from_be_bytes(digest[start..start + 8].try_into().expect("SHA-256 word"));
    NativeContentSha256 {
        word0: word(0),
        word1: word(8),
        word2: word(16),
        word3: word(24),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    thread_local! {
        pub(super) static BODY_HASHES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    }

    /// Content bodies hashed on this thread so far.
    pub(crate) fn body_hashes() -> usize {
        BODY_HASHES.with(std::cell::Cell::get)
    }

    #[test]
    fn loose_content_is_hashed_once_on_first_identity_request() {
        let files = (0..200)
            .map(|index| {
                (
                    format!("unused/{index}.bin"),
                    Arc::from(vec![index as u8; 64 * 1024]),
                )
            })
            .chain([("used.bin".to_owned(), Arc::from(&b"first body"[..]))])
            .collect::<BTreeMap<String, Arc<[u8]>>>();
        let before = body_hashes();
        let mut bridge = RuntimeContentBridge::new(files);
        assert_eq!(body_hashes(), before, "startup hashes no bodies");

        let first = bridge.retain(bridge.catalog["used.bin"].clone());
        let second = bridge.retain(bridge.catalog["used.bin"].clone());
        assert_eq!(body_hashes(), before, "opening a reference hashes nothing");
        let identity = bridge.retained_content(first).unwrap().sha256();
        assert_eq!(identity, sha256_words(&Sha256::digest(b"first body")));
        assert_eq!(body_hashes(), before + 1, "first request hashes once");
        assert_eq!(bridge.retained_content(second).unwrap().sha256(), identity);
        assert_eq!(bridge.retained_path("used.bin").unwrap().sha256(), identity);
        assert_eq!(bridge.catalog["used.bin"].sha256(), identity);
        assert_eq!(
            body_hashes(),
            before + 1,
            "repeat reads share the memoized identity"
        );

        // A changed admitted body is a new catalog with its own identity.
        let changed = RuntimeContentBridge::new(BTreeMap::from([(
            "used.bin".to_owned(),
            Arc::from(&b"second body"[..]),
        )]));
        assert_ne!(changed.catalog["used.bin"].sha256(), identity);
    }

    #[test]
    fn live_admission_copies_private_dependencies_and_releases_ownership() {
        let mut bridge = RuntimeContentBridge::new(BTreeMap::new());
        let context = (&mut bridge as *mut RuntimeContentBridge).cast();
        let text = |s: &str| NativeUtf8Slice {
            bytes: s.as_ptr(),
            len: s.len(),
        };
        let mut source = [1, 2, 3];
        let dependency = [4, 5];
        let files = [NativeContentSourceFile {
            path: text("model/texture.png"),
            bytes: NativeByteSlice {
                bytes: dependency.as_ptr(),
                len: dependency.len(),
            },
        }];
        let request = NativeContentAdmissionRequest {
            path: text("model/scene.glb"),
            bytes: NativeByteSlice {
                bytes: source.as_ptr(),
                len: source.len(),
            },
            dependencies: files.as_ptr(),
            dependencies_len: files.len(),
        };
        let mut handle = NativeContentReferenceHandle::default();
        assert_eq!(
            unsafe { admit_reference(context, &request, &mut handle) },
            ABI_OK
        );
        source[0] = 99;
        assert_eq!(source, [99, 2, 3]);
        let retained = bridge.retained_content(handle).unwrap();
        assert_eq!(&*retained.bytes, &[1, 2, 3]);
        assert_eq!(
            &*retained.files.get("model/texture.png").unwrap().unwrap(),
            &[4, 5]
        );
        assert!(retained.transient);
        assert!(bridge.catalog.is_empty());
        let weak = Arc::downgrade(&retained.bytes);
        drop(retained);
        assert_eq!(unsafe { destroy_reference(context, handle) }, ABI_OK);
        assert!(weak.upgrade().is_none());
        assert!(bridge.retained_content(handle).is_none());
    }

    #[test]
    fn live_admission_keys_dependencies_by_the_products_exact_paths() {
        let mut bridge = RuntimeContentBridge::new(BTreeMap::new());
        let context = (&mut bridge as *mut RuntimeContentBridge).cast();
        let text = |s: &str| NativeUtf8Slice {
            bytes: s.as_ptr(),
            len: s.len(),
        };
        let bytes = |b: &[u8]| NativeByteSlice {
            bytes: b.as_ptr(),
            len: b.len(),
        };
        let files = [
            NativeContentSourceFile {
                path: text("../shared/texture.png"),
                bytes: bytes(&[1]),
            },
            NativeContentSourceFile {
                path: text("../shared/texture.png"),
                bytes: bytes(&[2]),
            },
        ];
        let request = NativeContentAdmissionRequest {
            path: text("/models/scene.glb"),
            bytes: bytes(&[9]),
            dependencies: files.as_ptr(),
            dependencies_len: files.len(),
        };
        let mut handle = NativeContentReferenceHandle::default();
        assert_eq!(
            unsafe { admit_reference(context, &request, &mut handle) },
            ABI_OK
        );
        let retained = bridge.retained_content(handle).unwrap();
        assert_eq!(retained.path, "/models/scene.glb");
        assert_eq!(
            &*retained
                .files
                .get("../shared/texture.png")
                .unwrap()
                .unwrap(),
            &[2],
            "a repeated path keeps its last bytes"
        );
        assert!(retained.files.get("shared/texture.png").unwrap().is_none());
    }

    #[test]
    fn byte_result_borrows_large_and_empty_ranges() {
        let body: Arc<[u8]> = Arc::from(vec![7; 1024 * 1024 + 13]);
        let mut bridge =
            RuntimeContentBridge::new(BTreeMap::from([("large".to_owned(), body.clone())]));
        let handle = bridge.retain(bridge.catalog["large"].clone());
        let backing = bridge
            .read_bytes(NativeContentReadBytesRequest {
                reference: handle,
                offset: 0,
                max_bytes: u32::MAX,
            })
            .unwrap();
        assert_eq!(backing.bytes, body.as_ptr());
        assert_eq!(backing.len, body.len());
        let empty = bridge
            .read_bytes(NativeContentReadBytesRequest {
                reference: handle,
                offset: body.len() as u64,
                max_bytes: 1,
            })
            .unwrap();
        assert_eq!(empty.len, 0);
        assert!(bridge
            .read_bytes(NativeContentReadBytesRequest {
                reference: handle,
                offset: body.len() as u64 + 1,
                max_bytes: 1,
            })
            .is_none());
    }

    #[test]
    fn resolves_only_the_exact_persistable_path_and_hash() {
        let mut catalog = BTreeMap::new();
        catalog.insert("state.bin".to_owned(), Arc::from(&b"persisted content"[..]));
        let mut bridge = RuntimeContentBridge::new(catalog);
        let context = (&mut bridge as *mut RuntimeContentBridge).cast();
        let path = b"state.bin";
        let mut reference = NativeContentReferenceHandle::default();
        assert_eq!(
            unsafe {
                open_reference(
                    context,
                    &NativeContentOpenRequest {
                        path: NativeUtf8Slice {
                            bytes: path.as_ptr(),
                            len: path.len(),
                        },
                    },
                    &mut reference,
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        let mut info = NativeContentReferenceInfoResult {
            references: std::ptr::null(),
            references_len: 0,
        };
        assert_eq!(
            unsafe { read_reference_info(context, reference, &mut info) },
            ABI_OK
        );
        assert_eq!(info.references_len, 1);
        let identity = unsafe { (*info.references).sha256 };
        let mut reopened = NativeContentReferenceHandle::default();
        assert_eq!(
            unsafe {
                resolve_reference(
                    context,
                    &NativeContentResolveRequest {
                        path: NativeUtf8Slice {
                            bytes: path.as_ptr(),
                            len: path.len(),
                        },
                        sha256: identity,
                    },
                    &mut reopened,
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        let mut wrong = identity;
        wrong.word3 ^= 1;
        assert_eq!(
            unsafe {
                resolve_reference(
                    context,
                    &NativeContentResolveRequest {
                        path: NativeUtf8Slice {
                            bytes: path.as_ptr(),
                            len: path.len(),
                        },
                        sha256: wrong,
                    },
                    &mut NativeContentReferenceHandle::default(),
                    std::ptr::null_mut(),
                )
            },
            0
        );
        let mut bytes = NativeByteResult {
            bytes: std::ptr::null(),
            len: 0,
        };
        assert_eq!(
            unsafe {
                read_bytes(
                    context,
                    &NativeContentReadBytesRequest {
                        reference: reopened,
                        offset: 2,
                        max_bytes: 4,
                    },
                    &mut bytes,
                )
            },
            ABI_OK
        );
        assert_eq!(
            bytes.bytes,
            bridge.references[&reopened.value].bytes[2..].as_ptr()
        );
        assert_eq!(unsafe { destroy_reference(context, reference) }, ABI_OK);
        assert_eq!(unsafe { destroy_reference(context, reopened) }, ABI_OK);
        bridge.catalog.clear();
        assert_eq!(
            unsafe { std::slice::from_raw_parts(bytes.bytes, bytes.len) },
            b"rsis"
        );
    }

    #[test]
    fn a_reloaded_file_opens_with_new_bytes_while_old_references_keep_theirs() {
        let files = BTreeMap::from([
            ("tuning.json".to_owned(), Arc::from(&b"{\"a\":1}"[..])),
            ("other.json".to_owned(), Arc::from(&b"[]"[..])),
        ]);
        let mut bridge = RuntimeContentBridge::new(files);
        let before = bridge.retain(bridge.catalog["tuning.json"].clone());
        let changed = bridge.reload(BTreeMap::from([(
            "tuning.json".to_owned(),
            Arc::from(&b"{\"a\":2}"[..]),
        )]));
        let sha = sha256_words(&Sha256::digest(b"{\"a\":2}"));
        assert_eq!(changed, [("tuning.json".to_owned(), sha, 7)]);
        assert_eq!(
            bridge.references[&before.value].bytes.as_ref(),
            b"{\"a\":1}"
        );
        let after = bridge.retained_path("tuning.json").unwrap();
        assert_eq!(after.bytes.as_ref(), b"{\"a\":2}");
        assert_eq!(after.sha256(), sha);
        let other = bridge.retained_path("other.json").unwrap();
        assert_eq!(
            other.files.get("tuning.json").unwrap().unwrap().as_ref(),
            b"{\"a\":2}",
            "dependencies resolve in the new snapshot"
        );
    }
}
