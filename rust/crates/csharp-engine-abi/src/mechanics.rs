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

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeMechanicsDamageResponseKind {
    #[default]
    Prevent = 0,
    FlatReduction = 1,
    Scale = 2,
    Absorb = 3,
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
#[derive(Debug, Clone, Copy)]
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
