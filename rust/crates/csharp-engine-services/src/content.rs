//! One Engine-owned immutable catalog of already-admitted product content.

use std::{collections::BTreeMap, ffi::c_void, sync::Arc};

use csharp_engine_abi::*;
use sha2::{Digest, Sha256};

use crate::{composition::borrowed_utf8, composition::ABI_OK};

const MAX_READ_BYTES: usize = 1024 * 1024;

#[derive(Clone)]
struct AdmittedContent {
    path: String,
    sha256: NativeContentSha256,
    bytes: Arc<[u8]>,
}

#[derive(Clone)]
pub(crate) struct RetainedContent {
    pub(crate) path: String,
    pub(crate) sha256: NativeContentSha256,
    pub(crate) bytes: Arc<[u8]>,
}

struct ContentReferenceInfoLease {
    // Keeps `reference.path` alive until the matching lease is released.
    _path: String,
    reference: NativeContentReferenceInfo,
}

pub(crate) struct RuntimeContentBridge {
    catalog: BTreeMap<String, AdmittedContent>,
    references: BTreeMap<u64, AdmittedContent>,
    info_leases: BTreeMap<u64, ContentReferenceInfoLease>,
    byte_leases: BTreeMap<u64, Arc<[u8]>>,
    next_reference: u64,
    next_info_lease: u64,
    next_byte_lease: u64,
}

impl RuntimeContentBridge {
    pub(crate) fn new(content_resources: BTreeMap<String, Arc<[u8]>>) -> Self {
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
                    },
                )
            })
            .collect();
        Self {
            catalog,
            references: BTreeMap::new(),
            info_leases: BTreeMap::new(),
            byte_leases: BTreeMap::new(),
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
            })
    }

    fn read_info(
        &mut self,
        reference: NativeContentReferenceHandle,
    ) -> Option<NativeContentReferenceInfoLease> {
        let content = self.references.get(&reference.value)?;
        let value = self.next_info_lease;
        self.next_info_lease = value.checked_add(1)?;
        let path = content.path.clone();
        let info = NativeContentReferenceInfo {
            path: NativeUtf8Slice {
                bytes: path.as_ptr(),
                len: path.len(),
            },
            sha256: content.sha256,
            byte_length: u64::try_from(content.bytes.len()).ok()?,
        };
        self.info_leases.insert(
            value,
            ContentReferenceInfoLease {
                _path: path,
                reference: info,
            },
        );
        let lease = self.info_leases.get(&value)?;
        Some(NativeContentReferenceInfoLease {
            handle: NativeContentReferenceInfoLeaseHandle { value },
            references: &lease.reference,
            references_len: 1,
        })
    }

    fn read_bytes(&mut self, request: NativeContentReadBytesRequest) -> Option<NativeByteLease> {
        let content = self.references.get(&request.reference.value)?;
        let offset = usize::try_from(request.offset).ok()?;
        if offset > content.bytes.len() || usize::try_from(request.max_bytes).ok()? > MAX_READ_BYTES
        {
            return None;
        }
        let len = usize::try_from(request.max_bytes)
            .ok()?
            .min(content.bytes.len().saturating_sub(offset));
        let bytes: Arc<[u8]> = Arc::from(content.bytes[offset..offset + len].to_vec());
        let value = self.next_byte_lease;
        self.next_byte_lease = value.checked_add(1)?;
        let lease = NativeByteLease {
            handle: NativeByteLeaseHandle { value },
            bytes: bytes.as_ptr(),
            len: bytes.len(),
        };
        self.byte_leases.insert(value, bytes);
        Some(lease)
    }
}

pub(crate) fn api(bridge: &mut RuntimeContentBridge) -> NativeContentApi {
    NativeContentApi {
        context: (bridge as *mut RuntimeContentBridge).cast(),
        open_reference,
        resolve_reference,
        destroy_reference,
        read_reference_info,
        destroy_reference_info_lease,
        read_bytes,
        destroy_byte_lease,
    }
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
    let digest = Sha256::digest(bytes);
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
            unsafe { std::slice::from_raw_parts(bytes.bytes, bytes.len) },
            b"rsis"
        );
        assert_eq!(unsafe { destroy_byte_lease(context, bytes.handle) }, ABI_OK);
        assert_eq!(unsafe { destroy_reference(context, reference) }, ABI_OK);
        assert_eq!(unsafe { destroy_reference(context, reopened) }, ABI_OK);
    }
}
