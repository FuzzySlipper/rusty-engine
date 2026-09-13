//! Retained references to immutable product content already admitted by Engine.
//!
//! This family deliberately exposes neither filesystem access nor a product
//! loader. A reference names one entry in the host-admitted immutable catalog;
//! its path and SHA-256 identity can be persisted by product code and resolved
//! exactly in a later runtime.

use crate::{NativeByteLease, NativeByteLeaseHandle, NativeUtf8Slice};
use std::ffi::c_void;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeContentReferenceHandle {
    pub value: u64,
}

/// SHA-256 digest represented as the four big-endian u64 words of its
/// canonical hexadecimal form. It is a fixed C ABI value, not borrowed text.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeContentSha256 {
    pub word0: u64,
    pub word1: u64,
    pub word2: u64,
    pub word3: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContentOpenRequest {
    /// Exact normalized path from the host-admitted product content catalog.
    pub path: NativeUtf8Slice,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContentResolveRequest {
    /// Product-persistable content identity. Both fields must match one
    /// admitted immutable catalog entry exactly.
    pub path: NativeUtf8Slice,
    pub sha256: NativeContentSha256,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContentReadBytesRequest {
    pub reference: NativeContentReferenceHandle,
    pub offset: u64,
    /// Bounded by the Content service. Zero reads an empty range.
    pub max_bytes: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContentReferenceInfo {
    pub path: NativeUtf8Slice,
    pub sha256: NativeContentSha256,
    pub byte_length: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeContentReferenceInfoLeaseHandle {
    pub value: u64,
}

/// Exact immutable reference identity copied by generated bindings before
/// `destroy_reference_info_lease` consumes the lease.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContentReferenceInfoLease {
    pub handle: NativeContentReferenceInfoLeaseHandle,
    pub references: *const NativeContentReferenceInfo,
    pub references_len: usize,
}

pub type NativeOpenContentReference = unsafe extern "C" fn(
    *mut c_void,
    *const NativeContentOpenRequest,
    *mut NativeContentReferenceHandle,
) -> i32;
pub type NativeResolveContentReference = unsafe extern "C" fn(
    *mut c_void,
    *const NativeContentResolveRequest,
    *mut NativeContentReferenceHandle,
) -> i32;
pub type NativeDestroyContentReference =
    unsafe extern "C" fn(*mut c_void, NativeContentReferenceHandle) -> i32;
pub type NativeReadContentReferenceInfo = unsafe extern "C" fn(
    *mut c_void,
    NativeContentReferenceHandle,
    *mut NativeContentReferenceInfoLease,
) -> i32;
pub type NativeDestroyContentReferenceInfoLease =
    unsafe extern "C" fn(*mut c_void, NativeContentReferenceInfoLeaseHandle) -> i32;
pub type NativeReadContentBytes = unsafe extern "C" fn(
    *mut c_void,
    *const NativeContentReadBytesRequest,
    *mut NativeByteLease,
) -> i32;
pub type NativeDestroyContentByteLease =
    unsafe extern "C" fn(*mut c_void, NativeByteLeaseHandle) -> i32;

/// An independently admitted collection. File references retain their bytes
/// independently of this handle; closing it does not invalidate consumers.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeContentBundleHandle {
    pub value: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContentBundleInfo {
    pub id: NativeUtf8Slice,
    pub file_count: u64,
    pub byte_length: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeContentBundleInfoLeaseHandle {
    pub value: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContentBundleInfoLease {
    pub handle: NativeContentBundleInfoLeaseHandle,
    pub bundles: *const NativeContentBundleInfo,
    pub bundles_len: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContentBundleOpenRequest {
    pub id: NativeUtf8Slice,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContentBundleReferenceRequest {
    pub bundle: NativeContentBundleHandle,
    pub path: NativeUtf8Slice,
}

pub type NativeListContentBundles =
    unsafe extern "C" fn(*mut c_void, *mut NativeContentBundleInfoLease) -> i32;
pub type NativeDestroyContentBundleInfoLease =
    unsafe extern "C" fn(*mut c_void, NativeContentBundleInfoLeaseHandle) -> i32;
pub type NativeOpenContentBundle = unsafe extern "C" fn(
    *mut c_void,
    *const NativeContentBundleOpenRequest,
    *mut NativeContentBundleHandle,
) -> i32;
pub type NativeDestroyContentBundle =
    unsafe extern "C" fn(*mut c_void, NativeContentBundleHandle) -> i32;
pub type NativeReadContentBundleFiles = unsafe extern "C" fn(
    *mut c_void,
    NativeContentBundleHandle,
    *mut NativeContentReferenceInfoLease,
) -> i32;
pub type NativeOpenContentBundleReference = unsafe extern "C" fn(
    *mut c_void,
    *const NativeContentBundleReferenceRequest,
    *mut NativeContentReferenceHandle,
) -> i32;
