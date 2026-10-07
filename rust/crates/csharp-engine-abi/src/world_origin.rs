use crate::{NativeSpatialSessionHandle, NativeTransform};

/// Opaque, disposable prepared rebase (target origin and rebased root
/// transforms) retained by the Engine until C# commits or cancels it. The
/// value has no product-state meaning.
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
/// Engine keeps only its rebased local transform, in the prepared handle, and
/// never retains it as an entity-world mirror.
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
    pub target_cell_x: i64,
    pub target_cell_y: i64,
    pub target_cell_z: i64,
    pub entities: *const NativeWorldOriginEntityRow,
    pub entities_len: usize,
    /// Leave out the rows whose local position in the target frame would
    /// fall outside the session's envelope, naming them in the prepared
    /// result's `excluded`, instead of refusing the whole request. Every
    /// other row is still rebased atomically at commit; the product retires
    /// or moves the excluded ones itself. False refuses as before.
    pub exclude_outside_envelope: bool,
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

/// One root's local transform in the prepared target frame.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeWorldOriginAffectedTransform {
    pub entity_id: u64,
    pub local_transform: NativeTransform,
}

/// One root a prepare excluded for falling outside the envelope.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeWorldOriginExcludedEntity {
    pub entity_id: u64,
}

/// Borrowed readout of one prepared rebase. `affected` and `excluded` point
/// into Spatial bridge storage and stay valid until the next call on the same
/// context; the generated managed binding copies them before returning.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeWorldOriginPreparedResult {
    pub affected: *const NativeWorldOriginAffectedTransform,
    pub affected_len: usize,
    /// The entity ids the request excluded for falling outside the
    /// envelope, in request order; empty unless it asked to exclude them.
    pub excluded: *const NativeWorldOriginExcludedEntity,
    pub excluded_len: usize,
    pub target_cell_x: i64,
    pub target_cell_y: i64,
    pub target_cell_z: i64,
    pub local_envelope: f32,
}

/// Moves the session origin to the prepared target and rebases the live
/// collision scene, keeping any edits made since prepare.
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
    /// Rows the prepare excluded for falling outside the envelope.
    pub excluded_entity_count: u32,
    pub local_envelope: f32,
}
