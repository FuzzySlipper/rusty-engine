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
    /// A store for regenerable data: its writes and deletes skip the fsyncs,
    /// so a crash may lose them, and its blobs carry a CRC-32 of the payload.
    /// A damaged blob, torn by a crash or otherwise, reads as absent, so a
    /// save replaces it and a delete removes it.
    pub cache: bool,
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
    /// Decide the guard and revision now, copy the payload and write it on
    /// the persistence worker: the receipt says `Queued` and names a ticket.
    /// Saves are written in the order they were made, and a `Load` of the key
    /// sees the newest one at once. `ReadSaves` says when it is written.
    pub deferred: bool,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativePersistenceSaveOutcome {
    #[default]
    Saved = 0,
    RevisionConflict = 1,
    /// A deferred save was admitted and will be written with `revision`.
    Queued = 2,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativePersistenceSaveReceipt {
    /// Saved and Queued values describe the new blob. Conflict values
    /// describe the newest blob, queued or stored; revision zero means the
    /// key is absent.
    pub outcome: NativePersistenceSaveOutcome,
    pub revision: u64,
    /// The store's number for a queued save, counting from one; zero for a
    /// save that was not queued.
    pub ticket: u64,
}

/// A store's deferred saves as of this call.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativePersistenceSaveReadout {
    /// The last ticket issued.
    pub queued: u64,
    /// Every save up to this ticket has been written or has failed.
    pub finished: u64,
    /// How many of them failed.
    pub failed: u64,
    /// The last ticket that failed, or zero.
    pub last_failed: u64,
}

pub type NativeReadPersistenceSaves = unsafe extern "C" fn(
    *mut c_void,
    NativePersistenceStoreHandle,
    *mut NativePersistenceSaveReadout,
    *mut NativeOperationErrorReceipt,
) -> i32;
/// UTF-8 text of the store's last failed deferred save; empty if none failed.
pub type NativeReadPersistenceSaveFailure = unsafe extern "C" fn(
    *mut c_void,
    NativePersistenceStoreHandle,
    *mut NativeByteResult,
    *mut NativeOperationErrorReceipt,
) -> i32;

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
    *mut NativeOperationErrorReceipt,
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
    unsafe extern "C" fn(*mut c_void, NativePersistenceBlobHandle, *mut NativeByteResult) -> i32;
