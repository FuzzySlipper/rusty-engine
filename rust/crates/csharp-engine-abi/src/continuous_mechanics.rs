//! Typed continuous-mechanics ABI over the existing product EntityWorld.
//!
//! The continuous catalog is independent from the exact Mechanics catalog, but
//! continuous components attach to the already-bound `NativeMechanicsEntityHandle`.
//! All scalar values cross this boundary as admitted finite-binary64 `u64` bits.

use crate::{NativeMechanicsEntityHandle, NativeUtf8Slice};

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeContinuousMechanicsTrackMaximumKind {
    #[default]
    Fixed = 0,
    Stat = 1,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeContinuousMechanicsContributionKind {
    #[default]
    Add = 0,
    Minimum = 1,
    Maximum = 2,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeContinuousMechanicsStackingPolicy {
    #[default]
    Sum = 0,
    Highest = 1,
    Lowest = 2,
    UniqueBySource = 3,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeContinuousMechanicsComponentKind {
    #[default]
    Stats = 0,
    Tracks = 1,
    IntrinsicSources = 2,
    ActiveEffects = 3,
}

impl NativeContinuousMechanicsComponentKind {
    pub const fn all() -> [Self; 4] {
        [
            Self::Stats,
            Self::Tracks,
            Self::IntrinsicSources,
            Self::ActiveEffects,
        ]
    }
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeContinuousMechanicsRevisionGuard {
    #[default]
    Unchecked = 0,
    Exact = 1,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeContinuousMechanicsTrackSetPolicy {
    #[default]
    RejectOutOfBounds = 0,
    ClampToBounds = 1,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeContinuousMechanicsTrackAdjustmentKind {
    #[default]
    Spend = 0,
    Restore = 1,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeContinuousMechanicsCatalogHandle {
    pub value: u64,
}

/// Owns one copied, bounded continuous catalog inspection result.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeContinuousMechanicsCatalogLeaseHandle {
    pub value: u64,
}

/// Owns one copied, bounded continuous component inspection result.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeContinuousMechanicsComponentLeaseHandle {
    pub value: u64,
}

/// Owns one continuous operation readout, including all decision rows and text.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeContinuousMechanicsOperationLeaseHandle {
    pub value: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContinuousMechanicsCatalogStatRow {
    pub id: NativeUtf8Slice,
    pub minimum_bits: u64,
    pub maximum_bits: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContinuousMechanicsCatalogTrackRow {
    pub id: NativeUtf8Slice,
    pub minimum_bits: u64,
    pub maximum_kind: NativeContinuousMechanicsTrackMaximumKind,
    pub fixed_maximum_bits: u64,
    pub maximum_stat: NativeUtf8Slice,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContinuousMechanicsCatalogSourceRow {
    pub id: NativeUtf8Slice,
    pub priority: i16,
    pub contributions_start: u32,
    pub contributions_len: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContinuousMechanicsCatalogContributionRow {
    pub stat: NativeUtf8Slice,
    pub kind: NativeContinuousMechanicsContributionKind,
    pub value_bits: u64,
    pub stacking_group: NativeUtf8Slice,
    pub stacking: NativeContinuousMechanicsStackingPolicy,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContinuousMechanicsCatalogEffectRow {
    pub id: NativeUtf8Slice,
    pub sources_start: u32,
    pub sources_len: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContinuousMechanicsCatalogEffectSourceRow {
    pub source: NativeUtf8Slice,
}

/// One borrowed atomic continuous catalog definition. Source/effect spans index
/// the request's respective flat row arrays; no nested pointers are admitted.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContinuousMechanicsCatalogCreateRequest {
    pub version: NativeUtf8Slice,
    pub stats: *const NativeContinuousMechanicsCatalogStatRow,
    pub stats_len: usize,
    pub tracks: *const NativeContinuousMechanicsCatalogTrackRow,
    pub tracks_len: usize,
    pub sources: *const NativeContinuousMechanicsCatalogSourceRow,
    pub sources_len: usize,
    pub contributions: *const NativeContinuousMechanicsCatalogContributionRow,
    pub contributions_len: usize,
    pub effects: *const NativeContinuousMechanicsCatalogEffectRow,
    pub effects_len: usize,
    pub effect_sources: *const NativeContinuousMechanicsCatalogEffectSourceRow,
    pub effect_sources_len: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContinuousMechanicsCatalogLease {
    pub handle: NativeContinuousMechanicsCatalogLeaseHandle,
    pub catalog_id: u64,
    pub version: NativeUtf8Slice,
    pub fingerprint: NativeUtf8Slice,
    pub stats: *const NativeContinuousMechanicsCatalogStatRow,
    pub stats_len: usize,
    pub tracks: *const NativeContinuousMechanicsCatalogTrackRow,
    pub tracks_len: usize,
    pub sources: *const NativeContinuousMechanicsCatalogSourceRow,
    pub sources_len: usize,
    pub contributions: *const NativeContinuousMechanicsCatalogContributionRow,
    pub contributions_len: usize,
    pub effects: *const NativeContinuousMechanicsCatalogEffectRow,
    pub effects_len: usize,
    pub effect_sources: *const NativeContinuousMechanicsCatalogEffectSourceRow,
    pub effect_sources_len: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContinuousMechanicsInitialStatRow {
    pub stat: NativeUtf8Slice,
    pub base_bits: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContinuousMechanicsInitialTrackRow {
    pub track: NativeUtf8Slice,
    pub current_bits: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContinuousMechanicsInitialIntrinsicSourceRow {
    pub instance: NativeUtf8Slice,
    pub definition: NativeUtf8Slice,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContinuousMechanicsInitialActiveEffectRow {
    pub instance: NativeUtf8Slice,
    pub definition: NativeUtf8Slice,
}

/// Borrowed replacement values for all continuous component families. Each
/// `has_*` flag distinguishes an absent component from a present empty one.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContinuousMechanicsInitialComponentsRequest {
    pub catalog: NativeContinuousMechanicsCatalogHandle,
    pub entity: NativeMechanicsEntityHandle,
    pub has_stats: bool,
    pub stats: *const NativeContinuousMechanicsInitialStatRow,
    pub stats_len: usize,
    pub has_tracks: bool,
    pub tracks: *const NativeContinuousMechanicsInitialTrackRow,
    pub tracks_len: usize,
    pub has_intrinsic_sources: bool,
    pub intrinsic_sources: *const NativeContinuousMechanicsInitialIntrinsicSourceRow,
    pub intrinsic_sources_len: usize,
    pub has_active_effects: bool,
    pub active_effects: *const NativeContinuousMechanicsInitialActiveEffectRow,
    pub active_effects_len: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContinuousMechanicsComponentReadRequest {
    pub catalog: NativeContinuousMechanicsCatalogHandle,
    pub entity: NativeMechanicsEntityHandle,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeContinuousMechanicsComponentPresenceRow {
    pub component: NativeContinuousMechanicsComponentKind,
    pub present: bool,
    pub revision: u64,
}

/// A copied inspection of exactly the four continuous component families for
/// one existing Mechanics entity. Rows remain valid until `destroy_component_lease`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContinuousMechanicsComponentLease {
    pub handle: NativeContinuousMechanicsComponentLeaseHandle,
    pub catalog_id: u64,
    pub catalog_version: NativeUtf8Slice,
    pub catalog_fingerprint: NativeUtf8Slice,
    pub entity_id: u64,
    pub components: *const NativeContinuousMechanicsComponentPresenceRow,
    pub components_len: usize,
    pub stats: *const NativeContinuousMechanicsInitialStatRow,
    pub stats_len: usize,
    pub tracks: *const NativeContinuousMechanicsInitialTrackRow,
    pub tracks_len: usize,
    pub intrinsic_sources: *const NativeContinuousMechanicsInitialIntrinsicSourceRow,
    pub intrinsic_sources_len: usize,
    pub active_effects: *const NativeContinuousMechanicsInitialActiveEffectRow,
    pub active_effects_len: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContinuousMechanicsStatEvaluateRequest {
    pub catalog: NativeContinuousMechanicsCatalogHandle,
    pub entity: NativeMechanicsEntityHandle,
    pub stat: NativeUtf8Slice,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContinuousMechanicsStatDecisionRow {
    pub intrinsic: bool,
    pub source_instance: NativeUtf8Slice,
    pub effect_instance: NativeUtf8Slice,
    pub source_definition: NativeUtf8Slice,
    pub contribution_index: u16,
    pub outcome: NativeContinuousMechanicsDecisionOutcome,
    pub contribution_kind: NativeContinuousMechanicsContributionKind,
    pub contribution_value_bits: u64,
    pub stacking_group: NativeUtf8Slice,
    pub stacking: NativeContinuousMechanicsStackingPolicy,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeContinuousMechanicsDecisionOutcome {
    Applied = 0,
    Suppressed = 1,
    #[default]
    Inapplicable = 2,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeContinuousMechanicsObservedRevision {
    pub has_stats: bool,
    pub stats: u64,
    pub has_intrinsic_sources: bool,
    pub intrinsic_sources: u64,
    pub has_active_effects: bool,
    pub active_effects: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContinuousMechanicsStatEvaluationLease {
    pub handle: NativeContinuousMechanicsOperationLeaseHandle,
    pub decisions: *const NativeContinuousMechanicsStatDecisionRow,
    pub decisions_len: usize,
    pub catalog_id: u64,
    pub catalog_version: NativeUtf8Slice,
    pub catalog_fingerprint: NativeUtf8Slice,
    pub entity_id: u64,
    pub stat: NativeUtf8Slice,
    pub base_bits: u64,
    pub after_additions_bits: u64,
    pub unconstrained_bits: u64,
    pub minimum_bits: u64,
    pub maximum_bits: u64,
    pub value_bits: u64,
    pub observed_revisions: NativeContinuousMechanicsObservedRevision,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContinuousMechanicsStatBaseMutationRequest {
    pub catalog: NativeContinuousMechanicsCatalogHandle,
    pub entity: NativeMechanicsEntityHandle,
    pub operation: NativeUtf8Slice,
    pub stat: NativeUtf8Slice,
    pub base_bits: u64,
    pub revision_guard: NativeContinuousMechanicsRevisionGuard,
    pub expected_revision: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContinuousMechanicsTrackReadRequest {
    pub catalog: NativeContinuousMechanicsCatalogHandle,
    pub entity: NativeMechanicsEntityHandle,
    pub track: NativeUtf8Slice,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContinuousMechanicsTrackSetRequest {
    pub catalog: NativeContinuousMechanicsCatalogHandle,
    pub entity: NativeMechanicsEntityHandle,
    pub operation: NativeUtf8Slice,
    pub track: NativeUtf8Slice,
    pub value_bits: u64,
    pub policy: NativeContinuousMechanicsTrackSetPolicy,
    pub revision_guard: NativeContinuousMechanicsRevisionGuard,
    pub expected_revision: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContinuousMechanicsTrackAdjustmentRequest {
    pub catalog: NativeContinuousMechanicsCatalogHandle,
    pub entity: NativeMechanicsEntityHandle,
    pub operation: NativeUtf8Slice,
    pub track: NativeUtf8Slice,
    pub amount_bits: u64,
    pub revision_guard: NativeContinuousMechanicsRevisionGuard,
    pub expected_revision: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContinuousMechanicsTrackLease {
    pub handle: NativeContinuousMechanicsOperationLeaseHandle,
    pub catalog_id: u64,
    pub catalog_version: NativeUtf8Slice,
    pub catalog_fingerprint: NativeUtf8Slice,
    pub operation: NativeUtf8Slice,
    pub entity_id: u64,
    pub track: NativeUtf8Slice,
    pub requested_amount_bits: u64,
    pub applied_amount_bits: u64,
    pub before_bits: u64,
    pub after_bits: u64,
    pub minimum_bits: u64,
    pub maximum_bits: u64,
    pub has_adjustment: bool,
    pub adjustment_kind: NativeContinuousMechanicsTrackAdjustmentKind,
    pub observed_tracks_revision: u64,
    pub has_observed_stats_revision: bool,
    pub observed_stats_revision: u64,
    pub has_observed_intrinsic_sources_revision: bool,
    pub observed_intrinsic_sources_revision: u64,
    pub has_observed_active_effects_revision: bool,
    pub observed_active_effects_revision: u64,
    pub committed_tracks_revision: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContinuousMechanicsEffectApplyRequest {
    pub catalog: NativeContinuousMechanicsCatalogHandle,
    pub entity: NativeMechanicsEntityHandle,
    pub operation: NativeUtf8Slice,
    pub instance: NativeUtf8Slice,
    pub definition: NativeUtf8Slice,
    pub revision_guard: NativeContinuousMechanicsRevisionGuard,
    pub expected_revision: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContinuousMechanicsEffectRemoveRequest {
    pub catalog: NativeContinuousMechanicsCatalogHandle,
    pub entity: NativeMechanicsEntityHandle,
    pub operation: NativeUtf8Slice,
    pub instance: NativeUtf8Slice,
    pub revision_guard: NativeContinuousMechanicsRevisionGuard,
    pub expected_revision: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContinuousMechanicsEffectLease {
    pub handle: NativeContinuousMechanicsOperationLeaseHandle,
    pub catalog_id: u64,
    pub catalog_version: NativeUtf8Slice,
    pub catalog_fingerprint: NativeUtf8Slice,
    pub operation: NativeUtf8Slice,
    pub entity_id: u64,
    pub instance: NativeUtf8Slice,
    pub removed: bool,
    pub observed_revision: u64,
    pub committed_revision: u64,
}

/// Success receipt for a stat-base update. The lease owns borrowed text until
/// `destroy_operation_lease`, so it uses the same lifetime discipline as the
/// read and decision operations.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContinuousMechanicsStatMutationLease {
    pub handle: NativeContinuousMechanicsOperationLeaseHandle,
    pub catalog_id: u64,
    pub catalog_version: NativeUtf8Slice,
    pub catalog_fingerprint: NativeUtf8Slice,
    pub operation: NativeUtf8Slice,
    pub entity_id: u64,
    pub stat: NativeUtf8Slice,
    pub before_bits: u64,
    pub after_bits: u64,
    pub minimum_bits: u64,
    pub maximum_bits: u64,
    pub observed_revision: u64,
    pub committed_revision: u64,
}
