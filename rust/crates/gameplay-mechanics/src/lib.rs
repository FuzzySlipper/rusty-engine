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
    CatalogError, DamageKindDefinition, DamageKindSelector, DamageResponseDefinition,
    EffectDefinition, EquipmentSlotDefinition, ItemDefinition, ItemKind, MechanicsCatalog,
    MechanicsCatalogDefinition, MechanicsCatalogView, SourceDefinition, StackingPolicy,
    StatContribution, StatContributionDefinition, StatDefinition, TrackDefinition, TrackMaximum,
    MAX_CATALOG_DAMAGE_KINDS, MAX_CATALOG_EFFECTS, MAX_CATALOG_EQUIPMENT_SLOTS, MAX_CATALOG_ITEMS,
    MAX_CATALOG_SOURCES, MAX_CATALOG_STATS, MAX_CATALOG_TRACKS, MAX_RESPONSES_PER_SOURCE,
    MAX_STAT_CONTRIBUTIONS_PER_SOURCE,
};
pub use component::{
    gameplay_component_registry, register_gameplay_components, ActiveEffectInstance,
    ActiveEffectsComponent, EquipmentAssignment, EquipmentComponent, IntrinsicSourceBinding,
    IntrinsicSourcesComponent, InventoryComponent, ItemComponent, ItemStack,
    MechanicsComponentDataError, MechanicsComponentKind, ObservedComponentRevision, StatValue,
    StatsComponent, TrackValue, TracksComponent, ACTIVE_EFFECTS_COMPONENT_TYPE_ID,
    EQUIPMENT_COMPONENT_TYPE_ID, INTRINSIC_SOURCES_COMPONENT_TYPE_ID, INVENTORY_COMPONENT_TYPE_ID,
    ITEM_COMPONENT_TYPE_ID, STATS_COMPONENT_TYPE_ID, TRACKS_COMPONENT_TYPE_ID,
};
pub use damage::{
    DamageFact, DamagePart, DamagePartReceipt, DamagePreview, DamageReceipt, DamageRequest,
    DamageService, ResponseDecision, ResponseDecisionKind, TrackDamageChange, MAX_DAMAGE_PARTS,
    MAX_DAMAGE_RECEIPT_DECISIONS, MAX_DAMAGE_REQUEST_SOURCES,
};
pub use error::{MechanicsError, MechanicsSnapshotError};
pub use identity::{
    CatalogVersion, DamageKindId, EffectDefinitionId, EffectInstanceId, EquipmentSlotId,
    ItemDefinitionId, OperationId, SourceDefinitionId, SourceInstanceId, StackingGroupId, StatId,
    TrackId, MAX_MECHANICS_ID_BYTES,
};
pub use item::{
    EquipmentMutationReceipt, EquipmentService, ItemTransferReceipt, ItemTransferRequest,
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
    MAX_ACTIVE_EFFECT_INSTANCES, MAX_EQUIPMENT_ASSIGNMENTS, MAX_INTRINSIC_SOURCE_BINDINGS,
    MAX_INVENTORY_STACKS, MAX_REQUEST_SOURCES,
};
pub use stat::{
    StatBaseMutationReceipt, StatBaseMutationRequest, StatDecision, StatEvaluation, StatService,
    MAX_STAT_DECISIONS,
};
pub use track::{
    TrackAdjustmentKind, TrackMutationReceipt, TrackMutationRequest, TrackReconciliationPolicy,
    TrackReconciliationReceipt, TrackReconciliationRequest, TrackService, TrackSetPolicy,
    TrackSetReceipt, TrackSetRequest,
};
pub use view::{IntrinsicSourcesView, MechanicsEntityView, StatsView, TracksView};
