use crate::*;
use std::ffi::c_void;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativePersistenceStoreHandle {
    pub value: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativePersistenceBlobHandle {
    pub value: u64,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativePersistenceRevisionGuard {
    Any = 0,
    Exact = 1,
    Absent = 2,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePersistenceOpenRequest {
    /// A product-selected relative scope beneath the host-selected root.
    /// The host owns the base root; product code never supplies its path.
    pub scope: NativeUtf8Slice,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePersistenceSaveRequest {
    pub store: NativePersistenceStoreHandle,
    /// A storage-safe relative identity, never a method name or dispatch key.
    pub key: NativeUtf8Slice,
    pub revision_guard: NativePersistenceRevisionGuard,
    pub expected_revision: u64,
    /// Opaque product-defined state bytes copied before disk commit.
    pub payload: NativeByteSlice,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativePersistenceSaveOutcome {
    #[default]
    Saved = 0,
    RevisionConflict = 1,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativePersistenceSaveReceipt {
    /// Saved values describe the new blob. Conflict values describe the
    /// existing blob; revision zero means the key is absent.
    pub outcome: NativePersistenceSaveOutcome,
    pub revision: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePersistenceLoadRequest {
    pub store: NativePersistenceStoreHandle,
    pub key: NativeUtf8Slice,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePersistenceDeleteRequest {
    pub store: NativePersistenceStoreHandle,
    pub key: NativeUtf8Slice,
    pub revision_guard: NativePersistenceRevisionGuard,
    pub expected_revision: u64,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativePersistenceDeleteOutcome {
    #[default]
    Missing = 0,
    Deleted = 1,
    RevisionConflict = 2,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativePersistenceDeleteReceipt {
    pub outcome: NativePersistenceDeleteOutcome,
    /// Deleted: removed revision. Conflict: current revision. Missing: zero.
    pub revision: u64,
}

pub type NativeDeletePersistence = unsafe extern "C" fn(
    *mut c_void,
    *const NativePersistenceDeleteRequest,
    *mut NativePersistenceDeleteReceipt,
) -> i32;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePersistenceBlobInfo {
    pub present: bool,
    pub revision: u64,
    pub payload_len: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePersistenceCopyBlobRequest {
    pub blob: NativePersistenceBlobHandle,
    /// Must exactly match the blob's payload length.
    pub destination: NativeWritableByteSlice,
}

pub type NativeReadPersistenceBlobBytes =
    unsafe extern "C" fn(*mut c_void, NativePersistenceBlobHandle, *mut NativeByteLease) -> i32;
pub type NativeDestroyPersistenceByteLease =
    unsafe extern "C" fn(*mut c_void, NativeByteLeaseHandle) -> i32;
