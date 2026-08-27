//! Host-neutral, component-backed gameplay mechanics.
//!
//! This crate provides inert registered component types, one immutable admitted
//! catalog, and direct named services. It owns no entity registry, scheduler,
//! session, renderer, I/O path, or ambient event bus.
//!
//! A stat is an admitted base scalar evaluated with attributed active sources.
//! A track is durable current state constrained by fixed or stat-derived bounds.
//! Stat evaluation never rewrites a track; callers use explicit track services
//! for spend, restore, policy-governed set, and maximum reconciliation.

#![forbid(unsafe_code)]

mod catalog;
mod component;
mod damage;
mod effect;
mod error;
mod identity;
mod item;
mod scalar;
mod snapshot;
mod source;
mod stat;
mod track;
mod view;

pub use catalog::{
    CapacityMetricDefinition, CatalogError, DamageKindDefinition, DamageKindSelector,
    DamageResponseDefinition, EffectDefinition, EffectStackingPolicy, EquipmentSlotDefinition,
    ItemCapacityCost, ItemDefinition, ItemEquipmentPolicy, ItemKind, MechanicsCatalog,
    MechanicsCatalogDefinition, MechanicsCatalogView, SourceDefinition, StackingPolicy,
    StatContribution, StatContributionDefinition, StatDefinition, TrackDefinition, TrackMaximum,
    MAX_ABS_SOURCE_PRIORITY, MAX_CAPACITY_COSTS_PER_ITEM, MAX_CAPACITY_COST_UNITS,
    MAX_CATALOG_CAPACITY_METRICS, MAX_CATALOG_DAMAGE_KINDS, MAX_CATALOG_EFFECTS,
    MAX_CATALOG_EQUIPMENT_SLOTS, MAX_CATALOG_ITEMS, MAX_CATALOG_SOURCES, MAX_CATALOG_STATS,
    MAX_CATALOG_TRACKS, MAX_EFFECT_INSTANCES_PER_GROUP, MAX_EFFECT_STACKS,
    MAX_EQUIPMENT_SLOTS_PER_ITEM, MAX_ITEM_CLASSIFICATIONS, MAX_RESPONSES_PER_SOURCE,
    MAX_SOURCES_PER_EFFECT, MAX_SOURCES_PER_ITEM, MAX_STAT_CONTRIBUTIONS_PER_SOURCE,
};
pub use component::{
    gameplay_component_registry, register_gameplay_components, EquipmentAssignment,
    EquipmentComponent, IntrinsicSourceBinding, IntrinsicSourcesComponent, InventoryCapacityLimit,
    InventoryComponent, ItemComponent, ItemStack, MechanicsComponentDataError,
    MechanicsComponentKind, ObservedComponentRevision, StatValue, StatsComponent, TrackValue,
    TracksComponent, ACTIVE_EFFECTS_COMPONENT_CODEC_ID, ACTIVE_EFFECTS_COMPONENT_CODEC_VERSION,
    ACTIVE_EFFECTS_COMPONENT_TYPE_ID, EQUIPMENT_COMPONENT_CODEC_ID,
    EQUIPMENT_COMPONENT_CODEC_VERSION, EQUIPMENT_COMPONENT_TYPE_ID,
    INTRINSIC_SOURCES_COMPONENT_CODEC_ID, INTRINSIC_SOURCES_COMPONENT_CODEC_VERSION,
    INTRINSIC_SOURCES_COMPONENT_TYPE_ID, INVENTORY_COMPONENT_CODEC_ID,
    INVENTORY_COMPONENT_CODEC_VERSION, INVENTORY_COMPONENT_TYPE_ID, ITEM_COMPONENT_CODEC_ID,
    ITEM_COMPONENT_CODEC_VERSION, ITEM_COMPONENT_TYPE_ID, MAX_CAPACITY_LIMIT_UNITS,
    MAX_INVENTORY_CAPACITY_LIMITS, MAX_STACK_QUANTITY, MAX_STATS_PER_ENTITY, MAX_TRACKS_PER_ENTITY,
    STATS_COMPONENT_CODEC_ID, STATS_COMPONENT_CODEC_VERSION, STATS_COMPONENT_TYPE_ID,
    TRACKS_COMPONENT_CODEC_ID, TRACKS_COMPONENT_CODEC_VERSION, TRACKS_COMPONENT_TYPE_ID,
};
pub use damage::{
    DamageFact, DamagePart, DamagePartReceipt, DamagePreview, DamageReceipt, DamageRequest,
    DamageService, ResponseDecision, ResponseDecisionKind, TrackDamageChange, MAX_DAMAGE_FACTS,
    MAX_DAMAGE_PARTS, MAX_DAMAGE_RECEIPT_DECISIONS, MAX_DAMAGE_REQUEST_SOURCES,
};
pub use effect::{
    ActiveEffectInstance, ActiveEffectsComponent, EffectApplyRequest, EffectMutationKind,
    EffectMutationReceipt, EffectRefreshRequest, EffectRemovalRequest, EffectReplaceRequest,
    EffectService, EffectSourceActivation, MAX_ACTIVE_EFFECT_INSTANCES,
    MAX_EFFECT_SOURCE_ACTIVATIONS,
};
pub use error::{MechanicsError, MechanicsSnapshotError};
pub use identity::{
    CapacityMetricId, CatalogVersion, DamageKindId, EffectDefinitionId, EffectInstanceId,
    EquipmentExclusivityId, EquipmentSlotId, ItemClassificationId, ItemDefinitionId, OperationId,
    SourceDefinitionId, SourceInstanceId, StackingGroupId, StatId, TrackId, MAX_MECHANICS_ID_BYTES,
};
pub use item::{
    CapacityUsage, EquipmentEquipRequest, EquipmentMutationKind, EquipmentMutationReceipt,
    EquipmentService, EquipmentSlotChange, EquipmentSwapRequest, EquipmentUnequipRequest,
    InventoryMutationKind, InventoryMutationReceipt, InventoryMutationRequest, InventoryReadCost,
    InventoryService, InventoryTransferReceipt, InventoryTransferRequest, InventoryView,
    ItemDestroyReceipt, ItemDestroyRequest, ItemService, ItemTransferReceipt, ItemTransferRequest,
    UniqueInventoryItem, UniqueItemMaterializationReceipt, UniqueItemMaterializationRequest,
    MAX_CONTAINED_ENTITIES_PER_INVENTORY, MAX_EQUIPMENT_SOURCE_ACTIVATIONS,
};
pub use scalar::{
    CombinedRatio, ExactRatio, MechanicsArithmeticError, MechanicsScalar, RoundingPolicy,
    MAX_ABS_MECHANICS_SCALAR, MAX_RATIO_COMPONENT,
};
pub use snapshot::{
    decode_snapshot_with_catalog, decode_snapshot_with_catalog_and_registry,
    validate_state_against_catalog,
};
pub use source::{
    ActiveSource, DecisionOutcome, RequestSource, SourceCollectionCost, SourceInstanceIdentity,
    MAX_EQUIPMENT_ASSIGNMENTS, MAX_INTRINSIC_SOURCE_BINDINGS, MAX_INVENTORY_STACKS,
    MAX_REQUEST_SOURCES,
};
pub use stat::{
    StatBaseMutationReceipt, StatBaseMutationRequest, StatDecision, StatEvaluation, StatService,
    MAX_STAT_DECISIONS,
};
pub use track::{
    TrackAdjustmentKind, TrackMutationReceipt, TrackMutationRequest, TrackReadReceipt,
    TrackReconciliationPolicy, TrackReconciliationReceipt, TrackReconciliationRequest,
    TrackService, TrackSetPolicy, TrackSetReceipt, TrackSetRequest,
};
pub use view::{
    ActiveEffectsView, EquipmentView, IntrinsicSourcesView, InventoryComponentView, ItemView,
    MechanicsEntityView, StatsView, TracksView,
};
