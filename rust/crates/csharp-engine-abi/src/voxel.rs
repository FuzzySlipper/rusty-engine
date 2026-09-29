use crate::{
    NativeEngineDiagnosticLeaseHandle, NativeOperationErrorReceipt, NativeSpatialSessionHandle,
};
use crate::{NativeLightDescriptor, NativeVec3};
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
    pub state: u32,
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

/// The outcome of one voxel edit transaction. `NoChanges` is an expected
/// product-control result; other failures stay on the ABI diagnostic lane.
#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeVoxelEditStatus {
    #[default]
    Accepted = 0,
    NoChanges = 1,
}

/// Flat edit records are borrowed for one call and copied into the Engine
/// owner before any retained state is changed.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelEdit {
    pub state: u32,
    pub kind: NativeVoxelEditKind,
    pub address: NativeVoxelAddress,
    pub material_slot: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeVoxelEditTransaction {
    pub session: NativeSpatialSessionHandle,
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
    pub material_offset: u32,
    pub material_count: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeVoxelResidencyTransaction {
    pub states: *const u32,
    pub states_len: usize,
    pub session: NativeSpatialSessionHandle,
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
    pub authority_hash: u64,
    pub collision_revision: u64,
    pub navigation_revision: u64,
    pub mesh_revision: u64,
    pub dirty_chunk_count: u32,
    pub rebuilt_mesh_chunks: u32,
    pub reused_mesh_chunks: u32,
    pub removed_mesh_chunks: u32,
}

/// Collision policy for one occupied material slot; visuals retain the cell.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelMaterialCollision {
    pub material_slot: u32,
    pub collidable: bool,
}

/// Configure a fresh session's product-owned material collision declarations.
/// Unlisted slots collide. Configure before residency or edits.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeVoxelMaterialCollisionRequest {
    pub session: NativeSpatialSessionHandle,
    pub materials: *const NativeVoxelMaterialCollision,
    pub materials_len: usize,
}

pub type NativeConfigureVoxelMaterialCollision = unsafe extern "C" fn(
    *mut c_void,
    *const NativeVoxelMaterialCollisionRequest,
    *mut NativeOperationErrorReceipt,
) -> i32;

pub type NativeReadVoxelScene = unsafe extern "C" fn(
    *mut c_void,
    NativeVoxelSceneReadRequest,
    *mut NativeVoxelSceneReadout,
) -> i32;
pub type NativeReadVoxel =
    unsafe extern "C" fn(*mut c_void, NativeVoxelReadRequest, *mut NativeVoxelReadout) -> i32;
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
pub type NativeDestroyVoxelOperationDiagnosticLease =
    unsafe extern "C" fn(*mut c_void, NativeEngineDiagnosticLeaseHandle) -> i32;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeVoxelApi {
    pub context: *mut c_void,
    pub configure_material_collision: NativeConfigureVoxelMaterialCollision,
    pub read_scene: NativeReadVoxelScene,
    pub read: NativeReadVoxel,
    pub sample_direct_lighting: NativeSampleVoxelDirectLighting,
    pub read_chunk: NativeReadVoxelChunk,
    pub read_resident_chunk_at: NativeReadVoxelResidentChunkAt,
    pub apply_edits: NativeApplyVoxelEdits,
    pub read_dirty_chunk_at: NativeReadVoxelDirtyChunkAt,
    pub apply_residency: NativeApplyVoxelResidency,
    pub destroy_operation_diagnostic_lease: NativeDestroyVoxelOperationDiagnosticLease,
}

/// Samples direct incident light at address + offset (in voxel units). Descriptors
/// use local world coordinates, identical to unparented Graphics lights. Zero
/// normal measures incident light; nonzero normals select surface irradiance.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeVoxelLightSampleRequest {
    pub session: NativeSpatialSessionHandle,
    pub address: NativeVoxelAddress,
    pub offset: NativeVec3,
    pub normal: NativeVec3,
    pub directional_distance: f32,
    pub lights: *const NativeLightDescriptor,
    pub lights_len: usize,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeVoxelLightSample {
    pub irradiance: NativeVec3,
    pub luminance: f32,
    pub contributing_lights: u32,
    pub occluded_lights: u32,
    pub source_revision: u64,
    pub collision_revision: u64,
    pub static_collision_revision: u64,
    pub rebase_revision: u64,
}
pub type NativeSampleVoxelDirectLighting = unsafe extern "C" fn(
    *mut c_void,
    *const NativeVoxelLightSampleRequest,
    *mut NativeVoxelLightSample,
    *mut NativeOperationErrorReceipt,
) -> i32;
