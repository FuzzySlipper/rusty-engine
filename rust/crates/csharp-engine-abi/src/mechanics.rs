use crate::NativeUtf8Slice;

#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum NativeMechanicsTrackMaximumKind {
    Fixed = 0,
    Stat = 1,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum NativeMechanicsContributionKind {
    Add = 0,
    Scale = 1,
    Minimum = 2,
    Maximum = 3,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum NativeMechanicsStackingPolicy {
    Sum = 0,
    Highest = 1,
    Lowest = 2,
    UniqueBySource = 3,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum NativeMechanicsTrackSetPolicy {
    RejectOutOfBounds = 0,
    ClampToBounds = 1,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum NativeMechanicsTrackReconciliationPolicy {
    PreserveCurrent = 0,
    ClampToMaximum = 1,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum NativeMechanicsRevisionGuard {
    Unchecked = 0,
    Exact = 1,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeMechanicsRevisionComponent {
    #[default]
    Stats = 0,
    Tracks = 1,
    IntrinsicSources = 2,
    ActiveEffects = 3,
    Inventory = 4,
    Item = 5,
    Equipment = 6,
}

/// The mechanics mirror intentionally follows the product's EntityWorld lifecycle.
/// It is not a second source of product identity.
#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeMechanicsEntityLifecycle {
    #[default]
    Active = 0,
    Disabled = 1,
    Tombstoned = 2,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default)]
pub enum NativeMechanicsLifecycleGuard {
    #[default]
    Unchecked = 0,
    Exact = 1,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeMechanicsCatalogHandle {
    pub value: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeMechanicsEntityHandle {
    pub value: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeMechanicsStatsRevision {
    pub entity_id: u64,
    pub revision: u64,
    pub component: NativeMechanicsRevisionComponent,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeMechanicsTracksRevision {
    pub entity_id: u64,
    pub revision: u64,
    pub component: NativeMechanicsRevisionComponent,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeMechanicsComponentRevision {
    pub entity_id: u64,
    pub revision: u64,
    pub component: NativeMechanicsRevisionComponent,
    pub present: bool,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeMechanicsLifecycleReceipt {
    pub entity_id: u64,
    pub lifecycle: NativeMechanicsEntityLifecycle,
    pub stamp: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsCatalogCreateRequest {
    pub version: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsStatDefinitionRequest {
    pub catalog: NativeMechanicsCatalogHandle,
    pub id: NativeUtf8Slice,
    pub minimum: i64,
    pub maximum: i64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsTrackDefinitionRequest {
    pub catalog: NativeMechanicsCatalogHandle,
    pub id: NativeUtf8Slice,
    pub minimum: i64,
    pub maximum_kind: NativeMechanicsTrackMaximumKind,
    pub fixed_maximum: i64,
    pub maximum_stat: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsContributionDefinitionRequest {
    pub catalog: NativeMechanicsCatalogHandle,
    pub source: NativeUtf8Slice,
    pub priority: i32,
    pub stat: NativeUtf8Slice,
    pub kind: NativeMechanicsContributionKind,
    pub amount: i64,
    pub ratio_numerator: u32,
    pub ratio_denominator: u32,
    pub stacking_group: NativeUtf8Slice,
    pub stacking: NativeMechanicsStackingPolicy,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsEntityBindRequest {
    pub catalog: NativeMechanicsCatalogHandle,
    pub entity_id: u64,
    pub identity: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsEntityRebindRequest {
    pub catalog: NativeMechanicsCatalogHandle,
    pub entity_id: u64,
    pub guard: NativeMechanicsLifecycleGuard,
    pub expected_stamp: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsInitialStatRequest {
    pub entity: NativeMechanicsEntityHandle,
    pub stat: NativeUtf8Slice,
    pub base: i64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsInitialTrackRequest {
    pub entity: NativeMechanicsEntityHandle,
    pub track: NativeUtf8Slice,
    pub current: i64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsIntrinsicSourceRequest {
    pub entity: NativeMechanicsEntityHandle,
    pub instance: NativeUtf8Slice,
    pub definition: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeMechanicsEntityReceipt {
    pub stats_revision: NativeMechanicsStatsRevision,
    pub tracks_revision: NativeMechanicsTracksRevision,
    pub lifecycle: NativeMechanicsLifecycleReceipt,
    pub stats_slot: NativeMechanicsComponentRevision,
    pub tracks_slot: NativeMechanicsComponentRevision,
    pub intrinsic_sources_revision: NativeMechanicsComponentRevision,
    pub active_effects_revision: NativeMechanicsComponentRevision,
    pub inventory_revision: NativeMechanicsComponentRevision,
    pub item_revision: NativeMechanicsComponentRevision,
    pub equipment_revision: NativeMechanicsComponentRevision,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsLifecycleRequest {
    pub entity: NativeMechanicsEntityHandle,
    pub lifecycle: NativeMechanicsEntityLifecycle,
    pub guard: NativeMechanicsLifecycleGuard,
    pub expected_stamp: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsStatReadRequest {
    pub entity: NativeMechanicsEntityHandle,
    pub stat: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeMechanicsStatReadReceipt {
    pub base: i64,
    pub revision: NativeMechanicsStatsRevision,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsTrackReadRequest {
    pub entity: NativeMechanicsEntityHandle,
    pub track: NativeUtf8Slice,
    pub operation: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeMechanicsTrackReadReceipt {
    pub current: i64,
    pub minimum: i64,
    pub maximum: i64,
    pub revision: NativeMechanicsTracksRevision,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsStatOperationRequest {
    pub entity: NativeMechanicsEntityHandle,
    pub stat: NativeUtf8Slice,
    pub operation: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeMechanicsStatEvaluationReceipt {
    pub base: i64,
    pub value: i64,
    pub minimum: i64,
    pub maximum: i64,
    pub stats_revision: NativeMechanicsStatsRevision,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsStatBaseMutationRequest {
    pub entity: NativeMechanicsEntityHandle,
    pub operation: NativeUtf8Slice,
    pub source: NativeUtf8Slice,
    pub stat: NativeUtf8Slice,
    pub base: i64,
    pub revision_guard: NativeMechanicsRevisionGuard,
    pub expected_revision: NativeMechanicsStatsRevision,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeMechanicsStatMutationReceipt {
    pub before: i64,
    pub after: i64,
    pub minimum: i64,
    pub maximum: i64,
    pub observed_revision: NativeMechanicsStatsRevision,
    pub committed_revision: NativeMechanicsStatsRevision,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsTrackSetRequest {
    pub entity: NativeMechanicsEntityHandle,
    pub operation: NativeUtf8Slice,
    pub source: NativeUtf8Slice,
    pub track: NativeUtf8Slice,
    pub value: i64,
    pub policy: NativeMechanicsTrackSetPolicy,
    pub revision_guard: NativeMechanicsRevisionGuard,
    pub expected_revision: NativeMechanicsTracksRevision,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsTrackMutationRequest {
    pub entity: NativeMechanicsEntityHandle,
    pub operation: NativeUtf8Slice,
    pub source: NativeUtf8Slice,
    pub track: NativeUtf8Slice,
    pub amount: i64,
    pub revision_guard: NativeMechanicsRevisionGuard,
    pub expected_revision: NativeMechanicsTracksRevision,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeMechanicsTrackMutationReceipt {
    pub requested_amount: i64,
    pub applied_amount: i64,
    pub before: i64,
    pub after: i64,
    pub minimum: i64,
    pub maximum: i64,
    pub observed_revision: NativeMechanicsTracksRevision,
    pub committed_revision: NativeMechanicsTracksRevision,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeMechanicsTrackSetReceipt {
    pub target: i64,
    pub before: i64,
    pub after: i64,
    pub minimum: i64,
    pub maximum: i64,
    pub observed_revision: NativeMechanicsTracksRevision,
    pub committed_revision: NativeMechanicsTracksRevision,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsTrackReconciliationRequest {
    pub entity: NativeMechanicsEntityHandle,
    pub operation: NativeUtf8Slice,
    pub source: NativeUtf8Slice,
    pub track: NativeUtf8Slice,
    pub prospective_maximum: i64,
    pub policy: NativeMechanicsTrackReconciliationPolicy,
    pub revision_guard: NativeMechanicsRevisionGuard,
    pub expected_revision: NativeMechanicsTracksRevision,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeMechanicsTrackReconciliationReceipt {
    pub before: i64,
    pub after: i64,
    pub minimum: i64,
    pub current_maximum: i64,
    pub prospective_maximum: i64,
    pub observed_revision: NativeMechanicsTracksRevision,
    pub committed_revision: NativeMechanicsTracksRevision,
}
