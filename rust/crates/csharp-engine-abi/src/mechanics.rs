use crate::NativeUtf8Slice;

#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum NativeMechanicsTrackMaximumKind {
    Fixed = 0,
    Stat = 1,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default)]
pub enum NativeMechanicsContributionKind {
    #[default]
    Add = 0,
    Scale = 1,
    Minimum = 2,
    Maximum = 3,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default)]
pub enum NativeMechanicsStackingPolicy {
    #[default]
    Sum = 0,
    Highest = 1,
    Lowest = 2,
    UniqueBySource = 3,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default)]
pub enum NativeMechanicsTrackSetPolicy {
    #[default]
    RejectOutOfBounds = 0,
    ClampToBounds = 1,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default)]
pub enum NativeMechanicsTrackReconciliationPolicy {
    #[default]
    PreserveCurrent = 0,
    ClampToMaximum = 1,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeMechanicsTrackAdjustmentKind {
    #[default]
    Spend = 0,
    Restore = 1,
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

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeMechanicsDamageResponseKind {
    #[default]
    Prevent = 0,
    FlatReduction = 1,
    Scale = 2,
    Absorb = 3,
}

/// The exact response outcome selected by DamageService for one part/source
/// candidate. This is deliberately distinct from the catalog response kind:
/// `NoDamageResponse` is a real evaluated decision when an active source has
/// no configured response.
#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeMechanicsDamageDecisionKind {
    #[default]
    NoDamageResponse = 0,
    Prevent = 1,
    FlatReduction = 2,
    Scale = 3,
    Absorb = 4,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeMechanicsRoundingPolicy {
    #[default]
    TowardZero = 0,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeMechanicsEffectStackingKind {
    #[default]
    IndependentByProvenance = 0,
    Refresh = 1,
    Replace = 2,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeMechanicsEffectMutationKind {
    #[default]
    Apply = 0,
    Refresh = 1,
    Replace = 2,
    Remove = 3,
    Expire = 4,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeMechanicsItemKind {
    #[default]
    Fungible = 0,
    Unique = 1,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeMechanicsActiveEffectProvenanceKind {
    #[default]
    Intrinsic = 0,
    Effect = 1,
    EquippedItem = 2,
    Request = 3,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeMechanicsDecisionOutcome {
    Applied = 0,
    Suppressed = 1,
    #[default]
    Inapplicable = 2,
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
/// A typed owner for one exact, bounded catalog inspection result. Every
/// returned catalog row pointer remains valid only until `destroy_catalog_lease`.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeMechanicsCatalogLeaseHandle {
    pub value: u64,
}

/// A typed owner for one exact, bounded Mechanics component inspection result.
/// Its row pointer and metadata UTF-8 slices remain valid only until
/// `destroy_component_lease`.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeMechanicsComponentLeaseHandle {
    pub value: u64,
}

/// A typed owner for exact Mechanics operation receipts containing one or more
/// bounded collections. All borrowed rows and text remain valid until the
/// matching operation lease is destroyed.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeMechanicsOperationLeaseHandle {
    pub value: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsComponentReadMetadata {
    pub entity_id: u64,
    pub component: NativeMechanicsRevisionComponent,
    pub revision: u64,
    pub present: bool,
    pub catalog_id: u64,
    pub catalog_version: NativeUtf8Slice,
    pub catalog_fingerprint: NativeUtf8Slice,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsCatalogIdentityRow {
    pub version: NativeUtf8Slice,
    pub fingerprint: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsStatCatalogRow {
    pub id: NativeUtf8Slice,
    pub minimum: i64,
    pub maximum: i64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsTrackCatalogRow {
    pub id: NativeUtf8Slice,
    pub minimum: i64,
    pub maximum_kind: NativeMechanicsTrackMaximumKind,
    pub fixed_maximum: i64,
    pub maximum_stat: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsSourceCatalogRow {
    pub id: NativeUtf8Slice,
    pub priority: i16,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsStatContributionCatalogRow {
    pub source: NativeUtf8Slice,
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
pub struct NativeMechanicsDamageKindCatalogRow {
    pub id: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsDamageResponseCatalogRow {
    pub source: NativeUtf8Slice,
    pub kind: NativeMechanicsDamageResponseKind,
    pub selector_is_exact: bool,
    pub selector_damage_kind: NativeUtf8Slice,
    pub amount: i64,
    pub ratio_numerator: u32,
    pub ratio_denominator: u32,
    pub stacking_group: NativeUtf8Slice,
    pub stacking: NativeMechanicsStackingPolicy,
    pub absorb_track: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsEffectCatalogRow {
    pub id: NativeUtf8Slice,
    pub stacking_group: NativeUtf8Slice,
    pub stacking: NativeMechanicsEffectStackingKind,
    pub maximum_instances: u16,
    pub maximum_stacks: u16,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsEffectSourceCatalogRow {
    pub effect: NativeUtf8Slice,
    pub source: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsCapacityMetricCatalogRow {
    pub id: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsItemCatalogRow {
    pub id: NativeUtf8Slice,
    pub kind: NativeMechanicsItemKind,
    pub maximum_quantity: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsItemClassificationCatalogRow {
    pub item: NativeUtf8Slice,
    pub classification: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsItemCapacityCostCatalogRow {
    pub item: NativeUtf8Slice,
    pub metric: NativeUtf8Slice,
    pub units: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsItemEquipmentPolicyCatalogRow {
    pub item: NativeUtf8Slice,
    pub required_slots: u16,
    pub has_exclusive_group: bool,
    pub exclusive_group: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsItemSourceCatalogRow {
    pub item: NativeUtf8Slice,
    pub source: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsEquipmentSlotCatalogRow {
    pub id: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsSlotClassificationCatalogRow {
    pub slot: NativeUtf8Slice,
    pub classification: NativeUtf8Slice,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsCatalogIdentityLease {
    pub handle: NativeMechanicsCatalogLeaseHandle,
    pub entries: *const NativeMechanicsCatalogIdentityRow,
    pub entries_len: usize,
    pub catalog_id: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsStatCatalogLease {
    pub handle: NativeMechanicsCatalogLeaseHandle,
    pub entries: *const NativeMechanicsStatCatalogRow,
    pub entries_len: usize,
    pub catalog_id: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsTrackCatalogLease {
    pub handle: NativeMechanicsCatalogLeaseHandle,
    pub entries: *const NativeMechanicsTrackCatalogRow,
    pub entries_len: usize,
    pub catalog_id: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsSourceCatalogLease {
    pub handle: NativeMechanicsCatalogLeaseHandle,
    pub entries: *const NativeMechanicsSourceCatalogRow,
    pub entries_len: usize,
    pub catalog_id: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsStatContributionCatalogLease {
    pub handle: NativeMechanicsCatalogLeaseHandle,
    pub entries: *const NativeMechanicsStatContributionCatalogRow,
    pub entries_len: usize,
    pub catalog_id: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsDamageKindCatalogLease {
    pub handle: NativeMechanicsCatalogLeaseHandle,
    pub entries: *const NativeMechanicsDamageKindCatalogRow,
    pub entries_len: usize,
    pub catalog_id: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsDamageResponseCatalogLease {
    pub handle: NativeMechanicsCatalogLeaseHandle,
    pub entries: *const NativeMechanicsDamageResponseCatalogRow,
    pub entries_len: usize,
    pub catalog_id: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsEffectCatalogLease {
    pub handle: NativeMechanicsCatalogLeaseHandle,
    pub entries: *const NativeMechanicsEffectCatalogRow,
    pub entries_len: usize,
    pub catalog_id: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsEffectSourceCatalogLease {
    pub handle: NativeMechanicsCatalogLeaseHandle,
    pub entries: *const NativeMechanicsEffectSourceCatalogRow,
    pub entries_len: usize,
    pub catalog_id: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsCapacityMetricCatalogLease {
    pub handle: NativeMechanicsCatalogLeaseHandle,
    pub entries: *const NativeMechanicsCapacityMetricCatalogRow,
    pub entries_len: usize,
    pub catalog_id: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsItemCatalogLease {
    pub handle: NativeMechanicsCatalogLeaseHandle,
    pub entries: *const NativeMechanicsItemCatalogRow,
    pub entries_len: usize,
    pub catalog_id: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsItemClassificationCatalogLease {
    pub handle: NativeMechanicsCatalogLeaseHandle,
    pub entries: *const NativeMechanicsItemClassificationCatalogRow,
    pub entries_len: usize,
    pub catalog_id: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsItemCapacityCostCatalogLease {
    pub handle: NativeMechanicsCatalogLeaseHandle,
    pub entries: *const NativeMechanicsItemCapacityCostCatalogRow,
    pub entries_len: usize,
    pub catalog_id: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsItemEquipmentPolicyCatalogLease {
    pub handle: NativeMechanicsCatalogLeaseHandle,
    pub entries: *const NativeMechanicsItemEquipmentPolicyCatalogRow,
    pub entries_len: usize,
    pub catalog_id: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsItemSourceCatalogLease {
    pub handle: NativeMechanicsCatalogLeaseHandle,
    pub entries: *const NativeMechanicsItemSourceCatalogRow,
    pub entries_len: usize,
    pub catalog_id: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsEquipmentSlotCatalogLease {
    pub handle: NativeMechanicsCatalogLeaseHandle,
    pub entries: *const NativeMechanicsEquipmentSlotCatalogRow,
    pub entries_len: usize,
    pub catalog_id: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsSlotClassificationCatalogLease {
    pub handle: NativeMechanicsCatalogLeaseHandle,
    pub entries: *const NativeMechanicsSlotClassificationCatalogRow,
    pub entries_len: usize,
    pub catalog_id: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsStatComponentRow {
    pub stat: NativeUtf8Slice,
    pub base: i64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsTrackComponentRow {
    pub track: NativeUtf8Slice,
    pub current: i64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsIntrinsicSourceComponentRow {
    pub instance: NativeUtf8Slice,
    pub definition: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeMechanicsActiveEffectComponentRow {
    pub instance: NativeUtf8Slice,
    pub definition: NativeUtf8Slice,
    pub stacks: u16,
    pub provenance_kind: NativeMechanicsActiveEffectProvenanceKind,
    pub intrinsic_entity_id: u64,
    pub intrinsic_instance: NativeUtf8Slice,
    pub effect_entity_id: u64,
    pub effect_instance: NativeUtf8Slice,
    pub effect_stack: u16,
    pub effect_source: NativeUtf8Slice,
    pub equipped_owner_entity_id: u64,
    pub equipped_item_entity_id: u64,
    pub equipped_source: NativeUtf8Slice,
    pub request_operation: NativeUtf8Slice,
    pub request_instance: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsInventoryStackComponentRow {
    pub definition: NativeUtf8Slice,
    pub quantity: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsInventoryCapacityLimitComponentRow {
    pub metric: NativeUtf8Slice,
    pub maximum: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsItemComponentRow {
    pub definition: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsEquipmentAssignmentComponentRow {
    pub slot: NativeUtf8Slice,
    pub item_entity_id: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsStatComponentLease {
    pub handle: NativeMechanicsComponentLeaseHandle,
    pub entries: *const NativeMechanicsStatComponentRow,
    pub entries_len: usize,
    pub metadata: NativeMechanicsComponentReadMetadata,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsTrackComponentLease {
    pub handle: NativeMechanicsComponentLeaseHandle,
    pub entries: *const NativeMechanicsTrackComponentRow,
    pub entries_len: usize,
    pub metadata: NativeMechanicsComponentReadMetadata,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsIntrinsicSourceComponentLease {
    pub handle: NativeMechanicsComponentLeaseHandle,
    pub entries: *const NativeMechanicsIntrinsicSourceComponentRow,
    pub entries_len: usize,
    pub metadata: NativeMechanicsComponentReadMetadata,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsActiveEffectComponentLease {
    pub handle: NativeMechanicsComponentLeaseHandle,
    pub entries: *const NativeMechanicsActiveEffectComponentRow,
    pub entries_len: usize,
    pub metadata: NativeMechanicsComponentReadMetadata,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsInventoryStackComponentLease {
    pub handle: NativeMechanicsComponentLeaseHandle,
    pub entries: *const NativeMechanicsInventoryStackComponentRow,
    pub entries_len: usize,
    pub metadata: NativeMechanicsComponentReadMetadata,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsInventoryCapacityLimitComponentLease {
    pub handle: NativeMechanicsComponentLeaseHandle,
    pub entries: *const NativeMechanicsInventoryCapacityLimitComponentRow,
    pub entries_len: usize,
    pub metadata: NativeMechanicsComponentReadMetadata,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsItemComponentLease {
    pub handle: NativeMechanicsComponentLeaseHandle,
    pub entries: *const NativeMechanicsItemComponentRow,
    pub entries_len: usize,
    pub metadata: NativeMechanicsComponentReadMetadata,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsEquipmentAssignmentComponentLease {
    pub handle: NativeMechanicsComponentLeaseHandle,
    pub entries: *const NativeMechanicsEquipmentAssignmentComponentRow,
    pub entries_len: usize,
    pub metadata: NativeMechanicsComponentReadMetadata,
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
pub struct NativeMechanicsText {
    pub value: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsSourceDefinitionRequest {
    pub catalog: NativeMechanicsCatalogHandle,
    pub id: NativeUtf8Slice,
    pub priority: i32,
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
pub struct NativeMechanicsDamageKindDefinitionRequest {
    pub catalog: NativeMechanicsCatalogHandle,
    pub id: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsDamageResponseDefinitionRequest {
    pub catalog: NativeMechanicsCatalogHandle,
    pub source: NativeUtf8Slice,
    pub kind: NativeMechanicsDamageResponseKind,
    pub selector_is_exact: bool,
    pub selector_damage_kind: NativeUtf8Slice,
    pub amount: i64,
    pub ratio_numerator: u32,
    pub ratio_denominator: u32,
    pub stacking_group: NativeUtf8Slice,
    pub stacking: NativeMechanicsStackingPolicy,
    pub absorb_track: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsEffectDefinitionRequest {
    pub catalog: NativeMechanicsCatalogHandle,
    pub id: NativeUtf8Slice,
    pub stacking_group: NativeUtf8Slice,
    pub stacking: NativeMechanicsEffectStackingKind,
    pub maximum_instances: u16,
    pub maximum_stacks: u16,
    pub sources: *const NativeMechanicsText,
    pub sources_len: usize,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsCapacityMetricDefinitionRequest {
    pub catalog: NativeMechanicsCatalogHandle,
    pub id: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsItemCapacityCostInput {
    pub metric: NativeUtf8Slice,
    pub units: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsItemDefinitionRequest {
    pub catalog: NativeMechanicsCatalogHandle,
    pub id: NativeUtf8Slice,
    pub kind: NativeMechanicsItemKind,
    pub maximum_quantity: u64,
    pub classifications: *const NativeMechanicsText,
    pub classifications_len: usize,
    pub capacity_costs: *const NativeMechanicsItemCapacityCostInput,
    pub capacity_costs_len: usize,
    pub has_equipment: bool,
    pub required_slots: u16,
    pub exclusive_group: NativeUtf8Slice,
    pub sources: *const NativeMechanicsText,
    pub sources_len: usize,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsEquipmentSlotDefinitionRequest {
    pub catalog: NativeMechanicsCatalogHandle,
    pub id: NativeUtf8Slice,
    pub allowed_classifications: *const NativeMechanicsText,
    pub allowed_classifications_len: usize,
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
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsInitialStatValue {
    pub stat: NativeUtf8Slice,
    pub base: i64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsInitialTrackValue {
    pub track: NativeUtf8Slice,
    pub current: i64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsInitialIntrinsicSource {
    pub instance: NativeUtf8Slice,
    pub definition: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsInitialActiveEffect {
    pub instance: NativeUtf8Slice,
    pub definition: NativeUtf8Slice,
    pub provenance_kind: NativeMechanicsActiveEffectProvenanceKind,
    pub provenance_entity_id: u64,
    pub provenance_effect: NativeUtf8Slice,
    pub provenance_stack: u16,
    pub provenance_source: NativeUtf8Slice,
    pub provenance_item_entity_id: u64,
    pub provenance_operation: NativeUtf8Slice,
    pub provenance_instance: NativeUtf8Slice,
    pub stacks: u16,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsInitialInventoryStack {
    pub definition: NativeUtf8Slice,
    pub quantity: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsInitialInventoryCapacityLimit {
    pub metric: NativeUtf8Slice,
    pub maximum: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsInitialEquipmentAssignment {
    pub slot: NativeUtf8Slice,
    pub item_entity_id: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsInitialContainmentRequest {
    pub owner: NativeMechanicsEntityHandle,
    pub child_entity_id: u64,
    pub expected_state_revision: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsContainmentReadRequest {
    pub entity: NativeMechanicsEntityHandle,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeMechanicsContainmentReceipt {
    pub child_entity_id: u64,
    pub present: bool,
    pub container_entity_id: u64,
    pub state_revision: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsInitialComponentsRequest {
    pub entity: NativeMechanicsEntityHandle,
    pub has_stats: bool,
    pub stats: *const NativeMechanicsInitialStatValue,
    pub stats_len: usize,
    pub has_tracks: bool,
    pub tracks: *const NativeMechanicsInitialTrackValue,
    pub tracks_len: usize,
    pub has_intrinsic_sources: bool,
    pub intrinsic_sources: *const NativeMechanicsInitialIntrinsicSource,
    pub intrinsic_sources_len: usize,
    pub has_active_effects: bool,
    pub active_effects: *const NativeMechanicsInitialActiveEffect,
    pub active_effects_len: usize,
    pub has_inventory: bool,
    pub inventory_stacks: *const NativeMechanicsInitialInventoryStack,
    pub inventory_stacks_len: usize,
    pub inventory_capacity_limits: *const NativeMechanicsInitialInventoryCapacityLimit,
    pub inventory_capacity_limits_len: usize,
    pub has_item: bool,
    pub item_definition: NativeUtf8Slice,
    pub has_equipment: bool,
    pub equipment_assignments: *const NativeMechanicsInitialEquipmentAssignment,
    pub equipment_assignments_len: usize,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeMechanicsEntityReceipt {
    pub state_revision_before: u64,
    pub state_revision_after: u64,
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
pub struct NativeMechanicsTrackReadLease {
    pub handle: NativeMechanicsOperationLeaseHandle,
    pub observed_revisions: *const NativeMechanicsObservedComponentRevisionRow,
    pub observed_revisions_len: usize,
    pub catalog_id: u64,
    pub catalog_version: NativeUtf8Slice,
    pub catalog_fingerprint: NativeUtf8Slice,
    pub operation: NativeUtf8Slice,
    pub entity_id: u64,
    pub track: NativeUtf8Slice,
    pub current: i64,
    pub minimum: i64,
    pub maximum: i64,
    pub revision: NativeMechanicsTracksRevision,
    pub source_cost: NativeMechanicsSourceCollectionCost,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsStatOperationRequest {
    pub entity: NativeMechanicsEntityHandle,
    pub stat: NativeUtf8Slice,
    pub operation: NativeUtf8Slice,
    pub request_sources: *const NativeMechanicsRequestSource,
    pub request_sources_len: usize,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsRequestSource {
    pub instance: NativeUtf8Slice,
    pub definition: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeMechanicsU128 {
    pub low: u64,
    pub high: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeMechanicsSourceIdentity {
    pub kind: NativeMechanicsActiveEffectProvenanceKind,
    pub intrinsic_entity_id: u64,
    pub intrinsic_instance: NativeUtf8Slice,
    pub effect_entity_id: u64,
    pub effect_instance: NativeUtf8Slice,
    pub effect_stack: u16,
    pub effect_source: NativeUtf8Slice,
    pub equipped_owner_entity_id: u64,
    pub equipped_item_entity_id: u64,
    pub equipped_source: NativeUtf8Slice,
    pub request_operation: NativeUtf8Slice,
    pub request_instance: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeMechanicsSourceCollectionCost {
    pub intrinsic_entries_visited: u64,
    pub effect_entries_visited: u64,
    pub effect_source_activations_visited: u64,
    pub equipment_entries_visited: u64,
    pub item_components_read: u64,
    pub request_entries_visited: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeMechanicsObservedComponentRevisionRow {
    pub entity_id: u64,
    pub component: NativeMechanicsRevisionComponent,
    pub revision: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeMechanicsStatDecisionRow {
    pub source: NativeMechanicsSourceIdentity,
    pub source_definition: NativeUtf8Slice,
    pub has_contribution_index: bool,
    pub contribution_index: u16,
    pub outcome: NativeMechanicsDecisionOutcome,
    pub has_stacking_group: bool,
    pub stacking_group: NativeUtf8Slice,
    pub has_stacking: bool,
    pub stacking: NativeMechanicsStackingPolicy,
    pub has_contribution: bool,
    pub contribution_kind: NativeMechanicsContributionKind,
    pub contribution_amount: i64,
    pub contribution_ratio_numerator: u32,
    pub contribution_ratio_denominator: u32,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeMechanicsStatEvaluationLease {
    pub handle: NativeMechanicsOperationLeaseHandle,
    pub decisions: *const NativeMechanicsStatDecisionRow,
    pub decisions_len: usize,
    pub observed_revisions: *const NativeMechanicsObservedComponentRevisionRow,
    pub observed_revisions_len: usize,
    pub catalog_id: u64,
    pub catalog_version: NativeUtf8Slice,
    pub catalog_fingerprint: NativeUtf8Slice,
    pub entity_id: u64,
    pub stat: NativeUtf8Slice,
    pub base: i64,
    pub after_additions: i64,
    pub combined_scale_numerator: NativeMechanicsU128,
    pub combined_scale_denominator: NativeMechanicsU128,
    pub after_scaling: i64,
    pub unconstrained: i64,
    pub value: i64,
    pub minimum: i64,
    pub maximum: i64,
    pub stats_revision: NativeMechanicsStatsRevision,
    pub source_cost: NativeMechanicsSourceCollectionCost,
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
pub struct NativeMechanicsStatMutationLease {
    pub handle: NativeMechanicsOperationLeaseHandle,
    pub observed_revisions: *const NativeMechanicsObservedComponentRevisionRow,
    pub observed_revisions_len: usize,
    pub catalog_id: u64,
    pub catalog_version: NativeUtf8Slice,
    pub catalog_fingerprint: NativeUtf8Slice,
    pub operation: NativeUtf8Slice,
    pub source: NativeMechanicsSourceIdentity,
    pub entity_id: u64,
    pub stat: NativeUtf8Slice,
    pub before: i64,
    pub after: i64,
    pub minimum: i64,
    pub maximum: i64,
    pub observed_revision: NativeMechanicsStatsRevision,
    pub committed_revision: NativeMechanicsStatsRevision,
    pub source_cost: NativeMechanicsSourceCollectionCost,
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
pub struct NativeMechanicsTrackMutationLease {
    pub handle: NativeMechanicsOperationLeaseHandle,
    pub observed_revisions: *const NativeMechanicsObservedComponentRevisionRow,
    pub observed_revisions_len: usize,
    pub catalog_id: u64,
    pub catalog_version: NativeUtf8Slice,
    pub catalog_fingerprint: NativeUtf8Slice,
    pub operation: NativeUtf8Slice,
    pub source: NativeMechanicsSourceIdentity,
    pub entity_id: u64,
    pub track: NativeUtf8Slice,
    pub kind: NativeMechanicsTrackAdjustmentKind,
    pub requested_amount: i64,
    pub applied_amount: i64,
    pub before: i64,
    pub after: i64,
    pub minimum: i64,
    pub maximum: i64,
    pub observed_revision: NativeMechanicsTracksRevision,
    pub committed_revision: NativeMechanicsTracksRevision,
    pub source_cost: NativeMechanicsSourceCollectionCost,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeMechanicsTrackSetLease {
    pub handle: NativeMechanicsOperationLeaseHandle,
    pub observed_revisions: *const NativeMechanicsObservedComponentRevisionRow,
    pub observed_revisions_len: usize,
    pub catalog_id: u64,
    pub catalog_version: NativeUtf8Slice,
    pub catalog_fingerprint: NativeUtf8Slice,
    pub operation: NativeUtf8Slice,
    pub source: NativeMechanicsSourceIdentity,
    pub entity_id: u64,
    pub track: NativeUtf8Slice,
    pub policy: NativeMechanicsTrackSetPolicy,
    pub target: i64,
    pub before: i64,
    pub after: i64,
    pub minimum: i64,
    pub maximum: i64,
    pub observed_revision: NativeMechanicsTracksRevision,
    pub committed_revision: NativeMechanicsTracksRevision,
    pub source_cost: NativeMechanicsSourceCollectionCost,
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
pub struct NativeMechanicsTrackReconciliationLease {
    pub handle: NativeMechanicsOperationLeaseHandle,
    pub observed_revisions: *const NativeMechanicsObservedComponentRevisionRow,
    pub observed_revisions_len: usize,
    pub catalog_id: u64,
    pub catalog_version: NativeUtf8Slice,
    pub catalog_fingerprint: NativeUtf8Slice,
    pub operation: NativeUtf8Slice,
    pub source: NativeMechanicsSourceIdentity,
    pub entity_id: u64,
    pub track: NativeUtf8Slice,
    pub policy: NativeMechanicsTrackReconciliationPolicy,
    pub before: i64,
    pub after: i64,
    pub minimum: i64,
    pub current_maximum: i64,
    pub prospective_maximum: i64,
    pub observed_revision: NativeMechanicsTracksRevision,
    pub committed_revision: NativeMechanicsTracksRevision,
    pub source_cost: NativeMechanicsSourceCollectionCost,
}

/// A flattened borrowed provenance input for an EffectService operation. The
/// relevant fields are selected by `provenance_kind`; all supplied text is
/// borrowed only for the callback duration.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsEffectMutationRequest {
    pub entity: NativeMechanicsEntityHandle,
    pub operation: NativeUtf8Slice,
    pub instance: NativeUtf8Slice,
    pub definition: NativeUtf8Slice,
    pub provenance_kind: NativeMechanicsActiveEffectProvenanceKind,
    pub intrinsic_entity_id: u64,
    pub intrinsic_instance: NativeUtf8Slice,
    pub effect_entity_id: u64,
    pub effect_instance: NativeUtf8Slice,
    pub effect_stack: u16,
    pub effect_source: NativeUtf8Slice,
    pub equipped_owner_entity_id: u64,
    pub equipped_item_entity_id: u64,
    pub equipped_source: NativeUtf8Slice,
    pub request_operation: NativeUtf8Slice,
    pub request_instance: NativeUtf8Slice,
    pub stacks: u16,
    pub revision_guard: NativeMechanicsRevisionGuard,
    pub expected_revision: NativeMechanicsComponentRevision,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsEffectRefreshRequest {
    pub entity: NativeMechanicsEntityHandle,
    pub operation: NativeUtf8Slice,
    pub instance: NativeUtf8Slice,
    pub provenance_kind: NativeMechanicsActiveEffectProvenanceKind,
    pub intrinsic_entity_id: u64,
    pub intrinsic_instance: NativeUtf8Slice,
    pub effect_entity_id: u64,
    pub effect_instance: NativeUtf8Slice,
    pub effect_stack: u16,
    pub effect_source: NativeUtf8Slice,
    pub equipped_owner_entity_id: u64,
    pub equipped_item_entity_id: u64,
    pub equipped_source: NativeUtf8Slice,
    pub request_operation: NativeUtf8Slice,
    pub request_instance: NativeUtf8Slice,
    pub stacks: u16,
    pub revision_guard: NativeMechanicsRevisionGuard,
    pub expected_revision: NativeMechanicsComponentRevision,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsEffectRemovalRequest {
    pub entity: NativeMechanicsEntityHandle,
    pub operation: NativeUtf8Slice,
    pub instance: NativeUtf8Slice,
    pub revision_guard: NativeMechanicsRevisionGuard,
    pub expected_revision: NativeMechanicsComponentRevision,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeMechanicsEffectSourceActivationRow {
    pub identity: NativeMechanicsSourceIdentity,
    pub definition: NativeUtf8Slice,
}

/// One exact EffectService mutation receipt. `removed`, `activated_sources`,
/// and `observed_revisions` have distinct backing collections, all retained by
/// `handle` until `destroy_operation_lease`.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeMechanicsEffectOperationLease {
    pub handle: NativeMechanicsOperationLeaseHandle,
    pub removed: *const NativeMechanicsActiveEffectComponentRow,
    pub removed_len: usize,
    pub activated_sources: *const NativeMechanicsEffectSourceActivationRow,
    pub activated_sources_len: usize,
    pub observed_revisions: *const NativeMechanicsObservedComponentRevisionRow,
    pub observed_revisions_len: usize,
    pub catalog_id: u64,
    pub catalog_version: NativeUtf8Slice,
    pub catalog_fingerprint: NativeUtf8Slice,
    pub operation: NativeUtf8Slice,
    pub entity_id: u64,
    pub kind: NativeMechanicsEffectMutationKind,
    pub has_current: bool,
    pub current: NativeMechanicsActiveEffectComponentRow,
    pub observed_revision: NativeMechanicsComponentRevision,
    pub committed_revision: NativeMechanicsComponentRevision,
    pub tracks_validated: u64,
    pub source_cost: NativeMechanicsSourceCollectionCost,
}

/// One borrowed part of a DamageService request. `kind` is borrowed only for
/// the duration of the preview/apply callback.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsDamagePart {
    pub kind: NativeUtf8Slice,
    pub amount: i64,
}

/// A flattened borrowed DamageService request. The source fields retain the
/// exact `SourceInstanceIdentity` shape; `parts` and `request_sources` are
/// bounded by the upstream DamageService quotas.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMechanicsDamageRequest {
    pub operation: NativeUtf8Slice,
    pub source_kind: NativeMechanicsActiveEffectProvenanceKind,
    pub source_intrinsic_entity_id: u64,
    pub source_intrinsic_instance: NativeUtf8Slice,
    pub source_effect_entity_id: u64,
    pub source_effect_instance: NativeUtf8Slice,
    pub source_effect_stack: u16,
    pub source_effect_source: NativeUtf8Slice,
    pub source_equipped_owner_entity_id: u64,
    pub source_equipped_item_entity_id: u64,
    pub source_equipped_source: NativeUtf8Slice,
    pub source_request_operation: NativeUtf8Slice,
    pub source_request_instance: NativeUtf8Slice,
    pub has_actor: bool,
    pub actor_entity_id: u64,
    pub target: NativeMechanicsEntityHandle,
    pub target_track: NativeUtf8Slice,
    pub parts: *const NativeMechanicsDamagePart,
    pub parts_len: usize,
    pub request_sources: *const NativeMechanicsRequestSource,
    pub request_sources_len: usize,
    pub has_expected_tracks_revision: bool,
    pub expected_tracks_revision: NativeMechanicsTracksRevision,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeMechanicsDamagePartReceiptRow {
    pub index: u16,
    pub kind: NativeUtf8Slice,
    pub original: i64,
    pub prevented: bool,
    pub after_flat: i64,
    pub combined_scale_numerator: NativeMechanicsU128,
    pub combined_scale_denominator: NativeMechanicsU128,
    pub rounding: NativeMechanicsRoundingPolicy,
    pub after_scale: i64,
    pub absorbed: i64,
    pub applied: i64,
    pub unapplied: i64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeMechanicsDamageDecisionRow {
    pub part_index: u16,
    pub source: NativeMechanicsSourceIdentity,
    pub source_definition: NativeUtf8Slice,
    pub has_response_index: bool,
    pub response_index: u16,
    pub kind: NativeMechanicsDamageDecisionKind,
    pub amount: i64,
    pub ratio_numerator: u32,
    pub ratio_denominator: u32,
    pub absorb_track: NativeUtf8Slice,
    pub outcome: NativeMechanicsDecisionOutcome,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeMechanicsTrackDamageChangeRow {
    pub track: NativeUtf8Slice,
    pub before: i64,
    pub after: i64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeMechanicsTrackDepletionRow {
    pub track: NativeUtf8Slice,
    pub part_index: u16,
}

/// One exact DamageService preview or apply receipt. Every collection has a
/// separate backing vector and is retained, with all text, until
/// `destroy_operation_lease` releases `handle`.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeMechanicsDamageLease {
    pub handle: NativeMechanicsOperationLeaseHandle,
    pub parts: *const NativeMechanicsDamagePartReceiptRow,
    pub parts_len: usize,
    pub decisions: *const NativeMechanicsDamageDecisionRow,
    pub decisions_len: usize,
    pub track_changes: *const NativeMechanicsTrackDamageChangeRow,
    pub track_changes_len: usize,
    pub protection_track_depletions: *const NativeMechanicsTrackDepletionRow,
    pub protection_track_depletions_len: usize,
    pub target_track_depletions: *const NativeMechanicsTrackDepletionRow,
    pub target_track_depletions_len: usize,
    pub observed_revisions: *const NativeMechanicsObservedComponentRevisionRow,
    pub observed_revisions_len: usize,
    pub catalog_id: u64,
    pub catalog_version: NativeUtf8Slice,
    pub catalog_fingerprint: NativeUtf8Slice,
    pub operation: NativeUtf8Slice,
    pub source: NativeMechanicsSourceIdentity,
    pub has_actor: bool,
    pub actor_entity_id: u64,
    pub target_entity_id: u64,
    pub target_track: NativeUtf8Slice,
    pub observed_tracks_revision: NativeMechanicsTracksRevision,
    pub has_committed_tracks_revision: bool,
    pub committed_tracks_revision: NativeMechanicsTracksRevision,
    pub source_cost: NativeMechanicsSourceCollectionCost,
}
