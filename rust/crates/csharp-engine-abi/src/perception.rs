use crate::*;
use std::ffi::c_void;

/// Outcome of one distance-qualified observer/target pair.
#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativePerceptionPairKind {
    #[default]
    Visible = 0,
    FacingRejected = 1,
    Occluded = 2,
}

/// Caller-owned world-space observer facts. The Engine evaluates the supplied thresholds and
/// evidence but does not assign gameplay meaning to them.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativePerceptionObserver {
    pub entity: u64,
    pub origin: NativeVec3,
    pub forward: NativeVec3,
    pub maximum_distance: f64,
    pub minimum_facing_cosine: f64,
    pub evidence: f64,
}

/// Caller-owned world-space target center.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativePerceptionTarget {
    pub entity: u64,
    pub center: NativeVec3,
}

/// Borrowed observer, target, and occluder facts for one read-only perception query. All input
/// slices remain valid only for this direct callback.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePerceptionQueryRequest {
    pub session: NativeSpatialSessionHandle,
    pub observers: *const NativePerceptionObserver,
    pub observers_len: usize,
    pub targets: *const NativePerceptionTarget,
    pub targets_len: usize,
    pub occluders: *const NativeSpatialEntityCollider,
    pub occluders_len: usize,
    /// Zero starts a new deterministic read; later pages must echo the observed publication
    /// identity. This is not a projection-local counter and cannot alias after a rebuild.
    pub expected_projection_identity: u64,
    /// Zero is the first qualified pair. A non-zero cursor is rejected when the published scene
    /// identity changed.
    pub pair_cursor: u32,
    /// Bounded requested pair count. Zero is rejected rather than inferred or silently capped.
    pub page_size: u32,
}

/// One distance-qualified typed pair fact. Distance-rejected pairs are counted in the readout
/// and omitted from this collection.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativePerceptionPair {
    pub observer: u64,
    pub target: u64,
    pub distance: f64,
    pub facing_cosine: f64,
    pub kind: NativePerceptionPairKind,
    pub evidence: f64,
}

/// Deterministic visible-observer reduction for one target.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativePerceptionAggregate {
    pub target: u64,
    pub visible_observer_count: u64,
    pub evidence_total: f64,
}

/// Owner for copied perception rows returned from one query.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativePerceptionReadoutLeaseHandle {
    pub value: u64,
}

/// Copied pair facts, target reductions, and bounded query counters. Generated C# copies both
/// collections before calling the matching named destroy operation.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativePerceptionReadoutLease {
    pub handle: NativePerceptionReadoutLeaseHandle,
    pub pairs: *const NativePerceptionPair,
    pub pairs_len: usize,
    pub aggregates: *const NativePerceptionAggregate,
    pub aggregates_len: usize,
    pub pair_total: u32,
    pub has_next_pair_cursor: bool,
    pub next_pair_cursor: u32,
    pub projection_identity: u64,
    pub selected_observers: u32,
    pub selected_targets: u32,
    pub selection_comparisons: u64,
    pub distance_rejects: u32,
    pub facing_rejects: u32,
    pub visibility_casts: u32,
    pub occlusion_rejects: u32,
}

/// Direct named perception/visibility service family.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePerceptionApi {
    pub context: *mut c_void,
    pub query_visibility: NativeQueryPerception,
    pub destroy_readout_lease: NativeDestroyPerceptionReadoutLease,
}
