use crate::{NativeTransform, NativeVec3};

/// One caller-owned entity fact used only for a single motion resolution.
/// Rows are copied into a call-local Engine state and never retained.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeMotionEntityRow {
    pub entity_id: u64,
    pub transform: NativeTransform,
    pub bounds_min: NativeVec3,
    pub bounds_max: NativeVec3,
    pub collision_enabled: bool,
    pub collision_static: bool,
    pub has_transform_parent: bool,
    pub transform_parent_id: u64,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeMotionOutcome {
    #[default]
    Moved = 0,
    Blocked = 1,
    Slid = 2,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMotionResolveRequest {
    pub target_entity_id: u64,
    pub delta: NativeVec3,
    pub entities: *const NativeMotionEntityRow,
    pub entities_len: usize,
}

/// Pure resolution facts. `candidate_transform` is never committed natively;
/// product code chooses whether to publish it into its canonical EntityWorld.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeMotionResolveReceipt {
    pub outcome: NativeMotionOutcome,
    pub blocked_x: bool,
    pub blocked_y: bool,
    pub blocked_z: bool,
    pub has_hit: bool,
    pub hit_entity_id: u64,
    pub from: NativeVec3,
    pub resolved_position: NativeVec3,
    pub candidate_transform: NativeTransform,
}
