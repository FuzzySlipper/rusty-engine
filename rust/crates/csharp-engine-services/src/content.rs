//! One Engine-owned immutable catalog of already-admitted product content.

use std::{collections::BTreeMap, ffi::c_void, sync::Arc};

use csharp_engine_abi::*;
use sha2::{Digest, Sha256};

use crate::{composition::borrowed_utf8, composition::ABI_OK};

mod bundles;
pub use bundles::ProductContentBundles;

#[derive(Clone)]
struct AdmittedContent {
    path: String,
    sha256: NativeContentSha256,
    bytes: Arc<[u8]>,
    transient: bool,
    files: Arc<BTreeMap<String, Arc<[u8]>>>,
}

#[derive(Clone)]
pub(crate) struct RetainedContent {
    pub(crate) path: String,
    pub(crate) sha256: NativeContentSha256,
    pub(crate) bytes: Arc<[u8]>,
    pub(crate) transient: bool,
    /// Immutable dependency context of this source, never other open bundles.
    pub(crate) files: Arc<BTreeMap<String, Arc<[u8]>>>,
}

struct ContentReferenceInfoLease {
    // Keeps `reference.path` alive until the matching lease is released.
    _paths: Vec<String>,
    references: Vec<NativeContentReferenceInfo>,
}

pub(crate) struct RuntimeContentBridge {
    catalog: BTreeMap<String, AdmittedContent>,
    references: BTreeMap<u64, AdmittedContent>,
    info_leases: BTreeMap<u64, ContentReferenceInfoLease>,
    byte_leases: BTreeMap<u64, Arc<[u8]>>,
    bundles: bundles::BundleState,
    next_reference: u64,
    next_info_lease: u64,
    next_byte_lease: u64,
}

impl RuntimeContentBridge {
    pub(crate) fn new(content_resources: BTreeMap<String, Arc<[u8]>>) -> Self {
        let files = Arc::new(content_resources.clone());
        let catalog = content_resources
            .into_iter()
            .map(|(path, bytes)| {
                let sha256 = sha256(&bytes);
                (
                    path.clone(),
                    AdmittedContent {
                        path,
                        sha256,
                        bytes,
                        transient: false,
                        files: Arc::clone(&files),
                    },
                )
            })
            .collect();
        Self {
            catalog,
            references: BTreeMap::new(),
            info_leases: BTreeMap::new(),
            byte_leases: BTreeMap::new(),
            bundles: bundles::BundleState::default(),
            next_reference: 1,
            next_info_lease: 1,
            next_byte_lease: 1,
        }
    }

    fn retain(&mut self, content: AdmittedContent) -> Option<NativeContentReferenceHandle> {
        let value = self.next_reference;
        self.next_reference = value.checked_add(1)?;
        self.references.insert(value, content);
        Some(NativeContentReferenceHandle { value })
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
                sha256: content.sha256,
                bytes: Arc::clone(&content.bytes),
                transient: content.transient,
                files: Arc::clone(&content.files),
            })
    }

    /// Compatibility lookup for ordinary loose content; bundles require a
    /// reference so another open collection cannot change path resolution.
    pub(crate) fn retained_path(&self, path: &str) -> Option<RetainedContent> {
        let content = self
            .catalog
            .get(path.strip_prefix("content/").unwrap_or(path))?;
        Some(RetainedContent {
            path: content.path.clone(),
            sha256: content.sha256,
            bytes: Arc::clone(&content.bytes),
            transient: content.transient,
            files: Arc::clone(&content.files),
        })
    }

    fn read_info(
        &mut self,
        reference: NativeContentReferenceHandle,
    ) -> Option<NativeContentReferenceInfoLease> {
        let content = self.references.get(&reference.value)?.clone();
        self.retain_info(vec![(
            content.path,
            content.sha256,
            content.bytes.len() as u64,
        )])
    }

    fn retain_info(
        &mut self,
        entries: Vec<(String, NativeContentSha256, u64)>,
    ) -> Option<NativeContentReferenceInfoLease> {
        let value = self.next_info_lease;
        self.next_info_lease = value.checked_add(1)?;
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
            .collect();
        self.info_leases.insert(
            value,
            ContentReferenceInfoLease {
                _paths: paths,
                references,
            },
        );
        let lease = self.info_leases.get(&value)?;
        Some(NativeContentReferenceInfoLease {
            handle: NativeContentReferenceInfoLeaseHandle { value },
            references: lease.references.as_ptr(),
            references_len: lease.references.len(),
        })
    }

    pub(crate) fn bind_bundles(&mut self, bundles: ProductContentBundles) {
        self.bundles.source = bundles;
    }

    fn read_bytes(&mut self, request: NativeContentReadBytesRequest) -> Option<NativeByteLease> {
        let content = self.references.get(&request.reference.value)?;
        let offset = usize::try_from(request.offset).ok()?;
        if offset > content.bytes.len() {
            return None;
        }
        let len = usize::try_from(request.max_bytes)
            .ok()?
            .min(content.bytes.len().saturating_sub(offset));
        let bytes = Arc::clone(&content.bytes);
        let value = self.next_byte_lease;
        self.next_byte_lease = value.checked_add(1)?;
        let lease = NativeByteLease {
            handle: NativeByteLeaseHandle { value },
            bytes: bytes[offset..].as_ptr(),
            len,
        };
        self.byte_leases.insert(value, bytes);
        Some(lease)
    }
}

pub(crate) fn api(bridge: &mut RuntimeContentBridge) -> NativeContentApi {
    NativeContentApi {
        context: (bridge as *mut RuntimeContentBridge).cast(),
        list_bundles: bundles::list_bundles,
        destroy_bundle_info_lease: bundles::destroy_bundle_info_lease,
        open_bundle: bundles::open_bundle,
        destroy_bundle: bundles::destroy_bundle,
        read_bundle_files: bundles::read_bundle_files,
        open_bundle_reference: bundles::open_bundle_reference,
        admit_reference,
        open_reference,
        resolve_reference,
        destroy_reference,
        read_reference_info,
        destroy_reference_info_lease,
        read_bytes,
        destroy_byte_lease,
    }
}

/// Admission owns the only copy across the ABI. Normal decoders validate their
/// format when a resource is opened; no GLB/schema policy lives in Content.
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
        if !bundles::relative(path) {
            return None;
        }
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
        if files.insert(path, bytes).is_some() {
            return 0;
        }
    }
    let content = AdmittedContent {
        path,
        sha256: sha256(&bytes),
        bytes,
        transient: true,
        files: Arc::new(files),
    };
    let bridge = unsafe { &mut *context.cast::<RuntimeContentBridge>() };
    let Some(handle) = bridge.retain(content) else {
        return 0;
    };
    unsafe { *result = handle };
    ABI_OK
}

unsafe extern "C" fn open_reference(
    context: *mut c_void,
    request: *const NativeContentOpenRequest,
    result: *mut NativeContentReferenceHandle,
) -> i32 {
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let request = unsafe { &*request };
    let path = match unsafe { borrowed_utf8(request.path.bytes, request.path.len, "content path") }
    {
        Ok(path) => path,
        Err(_) => return 0,
    };
    let bridge = unsafe { &mut *context.cast::<RuntimeContentBridge>() };
    let Some(content) = bridge.catalog.get(path).cloned() else {
        return 0;
    };
    let Some(handle) = bridge.retain(content) else {
        return 0;
    };
    unsafe { *result = handle };
    ABI_OK
}

unsafe extern "C" fn resolve_reference(
    context: *mut c_void,
    request: *const NativeContentResolveRequest,
    result: *mut NativeContentReferenceHandle,
) -> i32 {
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let request = unsafe { &*request };
    let path = match unsafe { borrowed_utf8(request.path.bytes, request.path.len, "content path") }
    {
        Ok(path) => path,
        Err(_) => return 0,
    };
    let bridge = unsafe { &mut *context.cast::<RuntimeContentBridge>() };
    let Some(content) = bridge
        .catalog
        .get(path)
        .filter(|content| content.sha256 == request.sha256)
        .cloned()
        .or_else(|| bridge.bundles.resolve(path, request.sha256))
    else {
        return 0;
    };
    let Some(handle) = bridge.retain(content) else {
        return 0;
    };
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
    result: *mut NativeContentReferenceInfoLease,
) -> i32 {
    if context.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeContentBridge>() };
    let Some(lease) = bridge.read_info(reference) else {
        return 0;
    };
    unsafe { *result = lease };
    ABI_OK
}

unsafe extern "C" fn destroy_reference_info_lease(
    context: *mut c_void,
    lease: NativeContentReferenceInfoLeaseHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeContentBridge>() };
    if bridge.info_leases.remove(&lease.value).is_some() {
        ABI_OK
    } else {
        0
    }
}

unsafe extern "C" fn read_bytes(
    context: *mut c_void,
    request: *const NativeContentReadBytesRequest,
    result: *mut NativeByteLease,
) -> i32 {
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeContentBridge>() };
    let Some(lease) = bridge.read_bytes(unsafe { *request }) else {
        return 0;
    };
    unsafe { *result = lease };
    ABI_OK
}

unsafe extern "C" fn destroy_byte_lease(context: *mut c_void, lease: NativeByteLeaseHandle) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeContentBridge>() };
    if bridge.byte_leases.remove(&lease.value).is_some() {
        ABI_OK
    } else {
        0
    }
}

fn sha256(bytes: &[u8]) -> NativeContentSha256 {
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
mod tests {
    use super::*;

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
        assert_eq!(&*retained.files["model/texture.png"], &[4, 5]);
        assert!(retained.transient);
        assert!(bridge.catalog.is_empty());
        let weak = Arc::downgrade(&retained.bytes);
        drop(retained);
        assert_eq!(unsafe { destroy_reference(context, handle) }, ABI_OK);
        assert!(weak.upgrade().is_none());
        assert!(bridge.retained_content(handle).is_none());
    }

    #[test]
    fn byte_lease_borrows_large_and_empty_ranges() {
        let body: Arc<[u8]> = Arc::from(vec![7; 1024 * 1024 + 13]);
        let mut bridge =
            RuntimeContentBridge::new(BTreeMap::from([("large".to_owned(), body.clone())]));
        let handle = bridge.retain(bridge.catalog["large"].clone()).unwrap();
        let lease = bridge
            .read_bytes(NativeContentReadBytesRequest {
                reference: handle,
                offset: 0,
                max_bytes: u32::MAX,
            })
            .unwrap();
        assert_eq!(lease.bytes, body.as_ptr());
        assert_eq!(lease.len, body.len());
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
    fn resolves_only_the_exact_persistable_path_and_hash_and_releases_leases() {
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
                )
            },
            ABI_OK
        );
        let mut info = NativeContentReferenceInfoLease {
            handle: NativeContentReferenceInfoLeaseHandle::default(),
            references: std::ptr::null(),
            references_len: 0,
        };
        assert_eq!(
            unsafe { read_reference_info(context, reference, &mut info) },
            ABI_OK
        );
        assert_eq!(info.references_len, 1);
        let identity = unsafe { (*info.references).sha256 };
        assert_eq!(
            unsafe { destroy_reference_info_lease(context, info.handle) },
            ABI_OK
        );
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
                )
            },
            0
        );
        let mut bytes = NativeByteLease {
            handle: NativeByteLeaseHandle::default(),
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
        assert_eq!(unsafe { destroy_byte_lease(context, bytes.handle) }, ABI_OK);
        assert_eq!(unsafe { destroy_byte_lease(context, bytes.handle) }, 0);
    }
}
