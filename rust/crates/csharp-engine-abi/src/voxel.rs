use crate::{
    NativeByteLease, NativeByteLeaseHandle, NativeByteSlice, NativeEngineDiagnosticLeaseHandle,
    NativeOperationErrorReceipt, NativeSpatialSessionHandle,
};
use std::ffi::c_void;

/// One signed voxel address in the session's canonical world grid.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelAddress {
    pub x: i64,
    pub y: i64,
    pub z: i64,
}

/// One signed resident chunk identity in the session's canonical world grid.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelChunkIdentity {
    pub x: i64,
    pub y: i64,
    pub z: i64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelSceneReadRequest {
    pub session: NativeSpatialSessionHandle,
}

/// Fixed facts about the one canonical scene and all of its derived spatial
/// projections. No mesh payload or renderer object crosses this boundary.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct NativeVoxelSceneReadout {
    pub present: bool,
    pub voxel_size: f64,
    pub chunk_size: u32,
    pub source_revision: u64,
    pub authority_hash: u64,
    pub collision_revision: u64,
    pub navigation_revision: u64,
    pub mesh_revision: u64,
    pub projection_version: u64,
    pub resident_chunk_count: u64,
    pub collider_chunk_count: u64,
    pub solid_voxel_count: u64,
    pub navigation_cell_count: u64,
    pub navigation_hash: u64,
    pub dirty_chunk_count: u32,
    pub rebuilt_mesh_chunks: u32,
    pub reused_mesh_chunks: u32,
    pub removed_mesh_chunks: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelReadRequest {
    pub session: NativeSpatialSessionHandle,
    pub address: NativeVoxelAddress,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelReadout {
    pub present: bool,
    pub address: NativeVoxelAddress,
    pub material_slot: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelAtRequest {
    pub session: NativeSpatialSessionHandle,
    pub index: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelAtReceipt {
    pub present: bool,
    pub address: NativeVoxelAddress,
    pub material_slot: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelChunkReadRequest {
    pub session: NativeSpatialSessionHandle,
    pub chunk: NativeVoxelChunkIdentity,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelChunkReadout {
    pub present: bool,
    pub chunk: NativeVoxelChunkIdentity,
    pub content_hash: u64,
    pub solid_voxel_count: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelResidentChunkAtRequest {
    pub session: NativeSpatialSessionHandle,
    pub index: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelDirtyChunkAtRequest {
    pub session: NativeSpatialSessionHandle,
    pub index: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelDirtyChunkAtReceipt {
    pub present: bool,
    pub chunk: NativeVoxelChunkIdentity,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeVoxelEditKind {
    #[default]
    Set = 0,
    Clear = 1,
}

/// The outcome of one voxel edit transaction. `NoChanges` and
/// `StaleRevision` are expected product-control results; all other failures
/// stay on the ABI diagnostic lane.
#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeVoxelEditStatus {
    #[default]
    Accepted = 0,
    NoChanges = 1,
    StaleRevision = 2,
}

/// Flat edit records are borrowed for one call and copied into the Engine
/// owner before any retained state is changed.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelEdit {
    pub kind: NativeVoxelEditKind,
    pub address: NativeVoxelAddress,
    pub material_slot: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeVoxelEditTransaction {
    pub session: NativeSpatialSessionHandle,
    pub expected_revision: u64,
    pub edits: *const NativeVoxelEdit,
    pub edits_len: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelEditReceipt {
    pub revision_before: u64,
    pub accepted_revision: u64,
    pub solid_voxel_count: u64,
    pub authority_hash: u64,
    pub collision_revision: u64,
    pub navigation_revision: u64,
    pub mesh_revision: u64,
    pub changed_voxels: u32,
    pub changed_min: NativeVoxelAddress,
    pub changed_max_inclusive: NativeVoxelAddress,
    pub dirty_chunk_count: u32,
    pub rebuilt_mesh_chunks: u32,
    pub reused_mesh_chunks: u32,
    pub removed_mesh_chunks: u32,
    pub status: NativeVoxelEditStatus,
    pub current_revision: u64,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeVoxelResidencyOperationKind {
    #[default]
    Admit = 0,
    Replace = 1,
    Evict = 2,
}

/// One flat chunk operation. For Admit/Replace, material_offset/count select
/// the dense u32 material-slot range carried by the enclosing transaction.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelResidencyOperation {
    pub kind: NativeVoxelResidencyOperationKind,
    pub chunk: NativeVoxelChunkIdentity,
    pub expected_content_hash: u64,
    pub material_offset: u32,
    pub material_count: u32,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeVoxelResidencyHistoryPolicy {
    #[default]
    RejectIfNonEmpty = 0,
    ResetToPublishedAuthority = 1,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeVoxelResidencyTransaction {
    pub session: NativeSpatialSessionHandle,
    pub expected_revision: u64,
    pub history_policy: NativeVoxelResidencyHistoryPolicy,
    pub operations: *const NativeVoxelResidencyOperation,
    pub operations_len: usize,
    pub material_slots: *const u32,
    pub material_slots_len: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelResidencyReceipt {
    pub revision_before: u64,
    pub accepted_revision: u64,
    pub admitted_count: u32,
    pub replaced_count: u32,
    pub evicted_count: u32,
    pub retained_count: u32,
    pub resident_chunk_count: u64,
    pub resident_solid_voxel_count: u64,
    pub residency_hash: u64,
    pub authority_hash: u64,
    pub collision_revision: u64,
    pub navigation_revision: u64,
    pub mesh_revision: u64,
    pub dirty_chunk_count: u32,
    pub rebuilt_mesh_chunks: u32,
    pub reused_mesh_chunks: u32,
    pub removed_mesh_chunks: u32,
    pub history_reset: bool,
    pub history_invalidated_entries: u64,
    pub history_invalidated_redo_entries: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelChunkLeaseHandle {
    pub value: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelChunkLeaseRequest {
    pub session: NativeSpatialSessionHandle,
    pub chunk: NativeVoxelChunkIdentity,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelChunkLeaseReadRequest {
    pub lease: NativeVoxelChunkLeaseHandle,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelChunkLeaseReadout {
    pub present: bool,
    pub chunk: NativeVoxelChunkIdentity,
    pub acquired_content_hash: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelHistoryCursorReadRequest {
    pub session: NativeSpatialSessionHandle,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelHistoryCursorReadout {
    pub present: bool,
    pub index: u64,
    pub entry_count: u64,
    pub applied_transaction_present: bool,
    pub applied_transaction_id: u64,
    pub undo_depth: u64,
    pub redo_depth: u64,
    pub authority_hash: u64,
    pub history_hash: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelHistoryEntryAtRequest {
    pub session: NativeSpatialSessionHandle,
    pub index: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelHistoryEntryReadout {
    pub present: bool,
    pub transaction_id: u64,
    pub parent_transaction_present: bool,
    pub parent_transaction_id: u64,
    pub before_hash: u64,
    pub after_hash: u64,
    pub delta_count: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelHistoryDeltaAtRequest {
    pub session: NativeSpatialSessionHandle,
    pub entry_index: u32,
    pub delta_index: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelHistoryDeltaReadout {
    pub present: bool,
    pub address: NativeVoxelAddress,
    pub before_material_present: bool,
    pub before_material: u32,
    pub after_material_present: bool,
    pub after_material: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelHistoryActionRequest {
    pub session: NativeSpatialSessionHandle,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelHistoryReceipt {
    pub applied: bool,
    pub cursor_before: u64,
    pub cursor_after: u64,
    pub undo_depth: u64,
    pub redo_depth: u64,
    pub authority_hash: u64,
    pub history_hash: u64,
    pub revision_before: u64,
    pub revision_after: u64,
    pub changed_voxels: u32,
    pub bounds_present: bool,
    pub changed_min: NativeVoxelAddress,
    pub changed_max_inclusive: NativeVoxelAddress,
}

/// Fixed facts of the Engine-owned voxel history document codec. Product code
/// uses this readout to persist the exact owner schema rather than carrying a
/// duplicate schema literal.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelHistoryCodecInfo {
    pub schema_version: u32,
    pub max_encoded_bytes: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelHistoryExportRequest {
    pub session: NativeSpatialSessionHandle,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeVoxelHistoryRestoreRequest {
    pub session: NativeSpatialSessionHandle,
    /// Borrowed for this direct call only. Engine decodes and validates the
    /// entire bounded document before replacing any live session state.
    pub payload: NativeByteSlice,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelHistoryRestoreReceipt {
    pub cursor: NativeVoxelHistoryCursorReadout,
    pub source_revision: u64,
}

pub type NativeReadVoxelScene = unsafe extern "C" fn(
    *mut c_void,
    NativeVoxelSceneReadRequest,
    *mut NativeVoxelSceneReadout,
) -> i32;
pub type NativeReadVoxel =
    unsafe extern "C" fn(*mut c_void, NativeVoxelReadRequest, *mut NativeVoxelReadout) -> i32;
pub type NativeReadVoxelAt =
    unsafe extern "C" fn(*mut c_void, NativeVoxelAtRequest, *mut NativeVoxelAtReceipt) -> i32;
pub type NativeReadVoxelChunk = unsafe extern "C" fn(
    *mut c_void,
    NativeVoxelChunkReadRequest,
    *mut NativeVoxelChunkReadout,
) -> i32;
pub type NativeReadVoxelResidentChunkAt = unsafe extern "C" fn(
    *mut c_void,
    NativeVoxelResidentChunkAtRequest,
    *mut NativeVoxelChunkReadout,
) -> i32;
pub type NativeApplyVoxelEdits = unsafe extern "C" fn(
    *mut c_void,
    *const NativeVoxelEditTransaction,
    *mut NativeVoxelEditReceipt,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadVoxelDirtyChunkAt = unsafe extern "C" fn(
    *mut c_void,
    NativeVoxelDirtyChunkAtRequest,
    *mut NativeVoxelDirtyChunkAtReceipt,
) -> i32;
pub type NativeApplyVoxelResidency = unsafe extern "C" fn(
    *mut c_void,
    *const NativeVoxelResidencyTransaction,
    *mut NativeVoxelResidencyReceipt,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeAcquireVoxelChunkLease = unsafe extern "C" fn(
    *mut c_void,
    NativeVoxelChunkLeaseRequest,
    *mut NativeVoxelChunkLeaseHandle,
) -> i32;
pub type NativeDestroyVoxelChunkLease =
    unsafe extern "C" fn(*mut c_void, NativeVoxelChunkLeaseHandle) -> i32;
pub type NativeReadVoxelChunkLease = unsafe extern "C" fn(
    *mut c_void,
    NativeVoxelChunkLeaseReadRequest,
    *mut NativeVoxelChunkLeaseReadout,
) -> i32;
pub type NativeReadVoxelHistoryCursor = unsafe extern "C" fn(
    *mut c_void,
    NativeVoxelHistoryCursorReadRequest,
    *mut NativeVoxelHistoryCursorReadout,
) -> i32;
pub type NativeReadVoxelHistoryEntryAt = unsafe extern "C" fn(
    *mut c_void,
    NativeVoxelHistoryEntryAtRequest,
    *mut NativeVoxelHistoryEntryReadout,
) -> i32;
pub type NativeReadVoxelHistoryDeltaAt = unsafe extern "C" fn(
    *mut c_void,
    NativeVoxelHistoryDeltaAtRequest,
    *mut NativeVoxelHistoryDeltaReadout,
) -> i32;
pub type NativeUndoVoxel = unsafe extern "C" fn(
    *mut c_void,
    NativeVoxelHistoryActionRequest,
    *mut NativeVoxelHistoryReceipt,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeRedoVoxel = unsafe extern "C" fn(
    *mut c_void,
    NativeVoxelHistoryActionRequest,
    *mut NativeVoxelHistoryReceipt,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyVoxelOperationDiagnosticLease =
    unsafe extern "C" fn(*mut c_void, NativeEngineDiagnosticLeaseHandle) -> i32;
pub type NativeReadVoxelHistoryCodecInfo =
    unsafe extern "C" fn(*mut c_void, *mut NativeVoxelHistoryCodecInfo) -> i32;
pub type NativeExportVoxelHistory =
    unsafe extern "C" fn(*mut c_void, NativeVoxelHistoryExportRequest, *mut NativeByteLease) -> i32;
pub type NativeDestroyVoxelHistoryExportLease =
    unsafe extern "C" fn(*mut c_void, NativeByteLeaseHandle) -> i32;
pub type NativeRestoreVoxelHistory = unsafe extern "C" fn(
    *mut c_void,
    *const NativeVoxelHistoryRestoreRequest,
    *mut NativeVoxelHistoryRestoreReceipt,
) -> i32;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeVoxelApi {
    pub context: *mut c_void,
    pub read_scene: NativeReadVoxelScene,
    pub read: NativeReadVoxel,
    pub read_at: NativeReadVoxelAt,
    pub read_chunk: NativeReadVoxelChunk,
    pub read_resident_chunk_at: NativeReadVoxelResidentChunkAt,
    pub apply_edits: NativeApplyVoxelEdits,
    pub read_dirty_chunk_at: NativeReadVoxelDirtyChunkAt,
    pub apply_residency: NativeApplyVoxelResidency,
    pub acquire_chunk_lease: NativeAcquireVoxelChunkLease,
    pub destroy_chunk_lease: NativeDestroyVoxelChunkLease,
    pub read_chunk_lease: NativeReadVoxelChunkLease,
    pub read_history_cursor: NativeReadVoxelHistoryCursor,
    pub read_history_entry_at: NativeReadVoxelHistoryEntryAt,
    pub read_history_delta_at: NativeReadVoxelHistoryDeltaAt,
    pub undo: NativeUndoVoxel,
    pub redo: NativeRedoVoxel,
    pub destroy_operation_diagnostic_lease: NativeDestroyVoxelOperationDiagnosticLease,
    pub read_history_codec_info: NativeReadVoxelHistoryCodecInfo,
    pub export_history: NativeExportVoxelHistory,
    pub destroy_history_export_lease: NativeDestroyVoxelHistoryExportLease,
    pub restore_history: NativeRestoreVoxelHistory,
}
