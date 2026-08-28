use crate::{NativeSpatialSessionHandle, NativeTransform};

/// Opaque, disposable prepared rebase retained by the Engine until C# commits
/// or cancels it. The value has no product-state meaning.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeWorldOriginPreparedHandle {
    pub value: u64,
}

/// Exact integer cell plus finite canonical fractional offset for one global
/// product position. This avoids lossy large-world f32/f64 flattening.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct NativeWorldOriginGlobalPosition {
    pub cell_x: i64,
    pub cell_y: i64,
    pub cell_z: i64,
    pub offset_x: f64,
    pub offset_y: f64,
    pub offset_z: f64,
}

/// One product-owned root entity supplied for a single rebase attempt. The
/// Engine copies the row into a call-local validation state and never retains
/// it as an entity-world mirror.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeWorldOriginEntityRow {
    pub entity_id: u64,
    pub local_transform: NativeTransform,
    pub global_position: NativeWorldOriginGlobalPosition,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeWorldOriginPrepareRequest {
    pub session: NativeSpatialSessionHandle,
    pub expected_origin_revision: u64,
    pub expected_voxel_source_revision: u64,
    pub expected_static_mesh_revision: u64,
    pub target_cell_x: i64,
    pub target_cell_y: i64,
    pub target_cell_z: i64,
    pub entities: *const NativeWorldOriginEntityRow,
    pub entities_len: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeWorldOriginReadRequest {
    pub session: NativeSpatialSessionHandle,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct NativeWorldOriginReadout {
    pub cell_x: i64,
    pub cell_y: i64,
    pub cell_z: i64,
    pub revision: u64,
    pub local_envelope: f32,
    pub voxel_source_revision: u64,
    pub static_mesh_revision: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeWorldOriginPreparedReadRequest {
    pub prepared: NativeWorldOriginPreparedHandle,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct NativeWorldOriginPreparedReadout {
    pub present: bool,
    pub target_cell_x: i64,
    pub target_cell_y: i64,
    pub target_cell_z: i64,
    pub candidate_revision: u64,
    pub candidate_voxel_source_revision: u64,
    pub candidate_static_mesh_revision: u64,
    pub affected_entity_count: u32,
    pub local_envelope: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeWorldOriginAffectedAtRequest {
    pub prepared: NativeWorldOriginPreparedHandle,
    pub index: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeWorldOriginAffectedAtReceipt {
    pub present: bool,
    pub entity_id: u64,
    pub local_transform: NativeTransform,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeWorldOriginCommitRequest {
    pub prepared: NativeWorldOriginPreparedHandle,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct NativeWorldOriginCommitReceipt {
    pub revision_before: u64,
    pub revision_after: u64,
    pub origin_before_cell_x: i64,
    pub origin_before_cell_y: i64,
    pub origin_before_cell_z: i64,
    pub origin_after_cell_x: i64,
    pub origin_after_cell_y: i64,
    pub origin_after_cell_z: i64,
    pub voxel_source_revision: u64,
    pub static_mesh_revision: u64,
    pub affected_entity_count: u32,
    pub local_envelope: f32,
}
