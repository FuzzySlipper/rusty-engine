use crate::*;

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
    /// An explicit product-selected root. Host default-root policy is separate.
    pub root: NativeUtf8Slice,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePersistenceSaveRequest {
    pub store: NativePersistenceStoreHandle,
    /// A storage-safe relative identity, never a method name or dispatch key.
    pub key: NativeUtf8Slice,
    pub schema_version: u32,
    pub revision_guard: NativePersistenceRevisionGuard,
    pub expected_revision: u64,
    /// Opaque product-defined state bytes copied before disk commit.
    pub payload: NativeByteSlice,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativePersistenceSaveReceipt {
    pub revision: u64,
    pub schema_version: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePersistenceLoadRequest {
    pub store: NativePersistenceStoreHandle,
    pub key: NativeUtf8Slice,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePersistenceBlobInfo {
    pub present: bool,
    pub schema_version: u32,
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
