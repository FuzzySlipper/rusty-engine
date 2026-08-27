use std::{collections::BTreeMap, ffi::c_void};

use core_ids::EntityId;
use csharp_engine_abi::*;
use entity_state::{ComponentRevision, EntityAuthoringService, EntityDefinition, EntityState};
use gameplay_mechanics::{
    gameplay_component_registry, validate_state_against_catalog, ActiveEffectInstance,
    ActiveEffectsComponent, CapacityMetricDefinition, CatalogVersion, DamageKindDefinition,
    DamageKindSelector, DamageResponseDefinition, EffectDefinition, EffectStackingPolicy,
    EquipmentAssignment, EquipmentComponent, EquipmentSlotDefinition, ExactRatio,
    IntrinsicSourceBinding, IntrinsicSourcesComponent, InventoryCapacityLimit, InventoryComponent,
    ItemCapacityCost, ItemComponent, ItemDefinition, ItemEquipmentPolicy, ItemKind, ItemStack,
    MechanicsCatalog, MechanicsCatalogDefinition, MechanicsScalar, OperationId, SourceDefinition,
    SourceDefinitionId, SourceInstanceId, StackingGroupId, StackingPolicy, StatBaseMutationRequest,
    StatContribution, StatContributionDefinition, StatDefinition, StatId, StatService, StatValue,
    StatsComponent, TrackAdjustmentKind, TrackDefinition, TrackMaximum, TrackMutationRequest,
    TrackReconciliationPolicy, TrackReconciliationRequest, TrackService, TrackSetPolicy,
    TrackSetRequest, TrackValue, TracksComponent,
};

use crate::composition::{borrowed_utf8, ABI_OK};

#[derive(Clone)]
struct CatalogBuilder {
    version: CatalogVersion,
    stats: Vec<StatDefinition>,
    tracks: Vec<TrackDefinition>,
    sources: BTreeMap<SourceDefinitionId, SourceDefinition>,
    damage_kinds: Vec<DamageKindDefinition>,
    effects: Vec<EffectDefinition>,
    capacity_metrics: Vec<CapacityMetricDefinition>,
    items: Vec<ItemDefinition>,
    equipment_slots: Vec<EquipmentSlotDefinition>,
}

struct CatalogSlot {
    builder: Option<CatalogBuilder>,
    catalog: Option<MechanicsCatalog>,
    world: MechanicsWorld,
}

/// Catalog-scoped mechanism storage keyed by the product's canonical EntityWorld identity.
/// The bridge never allocates product entity identifiers: it only mirrors supplied ones.
struct MechanicsWorld {
    state: EntityState,
    lifecycle: BTreeMap<EntityId, LifecycleRecord>,
    next_stamp: u64,
}

#[derive(Clone, Copy)]
struct LifecycleRecord {
    lifecycle: NativeMechanicsEntityLifecycle,
    stamp: u64,
}

impl MechanicsWorld {
    fn new(state: EntityState) -> Self {
        Self {
            state,
            lifecycle: BTreeMap::new(),
            next_stamp: 1,
        }
    }

    fn admit(&mut self, entity: EntityId) -> Option<NativeMechanicsLifecycleReceipt> {
        let stamp = self.next_stamp;
        self.next_stamp = self.next_stamp.checked_add(1)?;
        self.lifecycle.insert(
            entity,
            LifecycleRecord {
                lifecycle: NativeMechanicsEntityLifecycle::Active,
                stamp,
            },
        );
        Some(self.lifecycle_receipt(entity))
    }

    fn lifecycle_receipt(&self, entity: EntityId) -> NativeMechanicsLifecycleReceipt {
        let record = self
            .lifecycle
            .get(&entity)
            .copied()
            .unwrap_or(LifecycleRecord {
                lifecycle: NativeMechanicsEntityLifecycle::Tombstoned,
                stamp: 0,
            });
        NativeMechanicsLifecycleReceipt {
            entity_id: entity.raw(),
            lifecycle: record.lifecycle,
            stamp: record.stamp,
        }
    }

    fn is_active(&self, entity: EntityId) -> bool {
        self.lifecycle
            .get(&entity)
            .is_some_and(|record| record.lifecycle == NativeMechanicsEntityLifecycle::Active)
    }

    fn set_lifecycle(
        &mut self,
        entity: EntityId,
        lifecycle: NativeMechanicsEntityLifecycle,
    ) -> Option<NativeMechanicsLifecycleReceipt> {
        let before = self.lifecycle.get(&entity).copied()?;
        if before.lifecycle == NativeMechanicsEntityLifecycle::Tombstoned {
            return None;
        }
        let next_stamp = self.next_stamp.checked_add(1)?;
        let state_revision = self.state.revision();
        let changed = match lifecycle {
            NativeMechanicsEntityLifecycle::Active => {
                EntityAuthoringService.enable(&mut self.state, state_revision, entity)
            }
            NativeMechanicsEntityLifecycle::Disabled => {
                EntityAuthoringService.disable(&mut self.state, state_revision, entity)
            }
            NativeMechanicsEntityLifecycle::Tombstoned => {
                EntityAuthoringService.destroy(&mut self.state, state_revision, entity)
            }
        };
        changed.ok()?;
        let stamp = self.next_stamp;
        self.next_stamp = next_stamp;
        self.lifecycle
            .insert(entity, LifecycleRecord { lifecycle, stamp });
        Some(self.lifecycle_receipt(entity))
    }
}

#[derive(Clone)]
struct EntityBinding {
    catalog: u64,
    entity: EntityId,
    identity: String,
    stats: Option<Vec<StatValue>>,
    tracks: Option<Vec<TrackValue>>,
    intrinsic_sources: Option<Vec<IntrinsicSourceBinding>>,
    active_effects: Option<Vec<ActiveEffectInstance>>,
    inventory: Option<(Vec<ItemStack>, Vec<InventoryCapacityLimit>)>,
    item: Option<gameplay_mechanics::ItemDefinitionId>,
    equipment: Option<Vec<EquipmentAssignment>>,
    initial_components_set: bool,
    committed: bool,
}

pub(crate) struct RuntimeMechanicsBridge {
    catalogs: BTreeMap<u64, CatalogSlot>,
    entities: BTreeMap<u64, EntityBinding>,
    /// A product canonical entity is admitted into at most one mechanics catalog world.
    canonical_entities: BTreeMap<EntityId, u64>,
    next_catalog: u64,
    next_entity: u64,
}

impl RuntimeMechanicsBridge {
    pub(crate) fn new() -> Self {
        Self {
            catalogs: BTreeMap::new(),
            entities: BTreeMap::new(),
            canonical_entities: BTreeMap::new(),
            next_catalog: 1,
            next_entity: 1,
        }
    }

    fn catalog_slot_mut(
        &mut self,
        handle: NativeMechanicsCatalogHandle,
    ) -> Option<&mut CatalogSlot> {
        self.catalogs.get_mut(&handle.value)
    }

    fn binding(&self, handle: NativeMechanicsEntityHandle) -> Option<&EntityBinding> {
        self.entities
            .get(&handle.value)
            .filter(|binding| binding.committed)
    }

    fn state_and_catalog_mut(
        &mut self,
        handle: NativeMechanicsEntityHandle,
    ) -> Option<(&mut EntityState, &MechanicsCatalog, EntityId)> {
        let binding = self.binding(handle)?.clone();
        let slot = self.catalogs.get_mut(&binding.catalog)?;
        if !slot.world.is_active(binding.entity) {
            return None;
        }
        Some((
            &mut slot.world.state,
            slot.catalog.as_ref()?,
            binding.entity,
        ))
    }
}

pub(crate) fn api(bridge: &mut RuntimeMechanicsBridge) -> NativeMechanicsApi {
    NativeMechanicsApi {
        context: (bridge as *mut RuntimeMechanicsBridge).cast(),
        create_catalog,
        define_stat,
        define_track,
        define_contribution,
        define_source,
        define_damage_kind,
        define_damage_response,
        define_effect,
        define_capacity_metric,
        define_item,
        define_equipment_slot,
        admit_catalog,
        destroy_catalog,
        bind_entity,
        rebind_entity,
        set_initial_stat,
        set_initial_track,
        bind_intrinsic_source,
        set_initial_components,
        commit_entity,
        set_entity_lifecycle,
        destroy_entity,
        read_stat,
        evaluate_stat,
        read_track,
        set_stat_base,
        set_track,
        spend_track,
        restore_track,
        reconcile_track,
    }
}

unsafe extern "C" fn create_catalog(
    context: *mut c_void,
    request: *const NativeMechanicsCatalogCreateRequest,
    result: *mut NativeMechanicsCatalogHandle,
) -> i32 {
    let Some((bridge, request, result)) = bridge_request_result(context, request, result) else {
        return 0;
    };
    let Ok(version) = unsafe { text(request.version, "mechanics catalog version") }
        .and_then(parse::<CatalogVersion>)
    else {
        return 0;
    };
    let Ok(registry) = gameplay_component_registry() else {
        return 0;
    };
    let Ok(state) = EntityState::from_definitions_with_registry(registry, []) else {
        return 0;
    };
    let handle = bridge.next_catalog;
    let Some(next) = handle.checked_add(1) else {
        return 0;
    };
    bridge.next_catalog = next;
    bridge.catalogs.insert(
        handle,
        CatalogSlot {
            builder: Some(CatalogBuilder {
                version,
                stats: Vec::new(),
                tracks: Vec::new(),
                sources: BTreeMap::new(),
                damage_kinds: Vec::new(),
                effects: Vec::new(),
                capacity_metrics: Vec::new(),
                items: Vec::new(),
                equipment_slots: Vec::new(),
            }),
            catalog: None,
            world: MechanicsWorld::new(state),
        },
    );
    *result = NativeMechanicsCatalogHandle { value: handle };
    ABI_OK
}

unsafe extern "C" fn define_stat(
    context: *mut c_void,
    request: *const NativeMechanicsStatDefinitionRequest,
) -> i32 {
    let Some((bridge, request)) = bridge_request(context, request) else {
        return 0;
    };
    let Ok(id) = unsafe { text(request.id, "mechanics stat id") }.and_then(parse::<StatId>) else {
        return 0;
    };
    let (Ok(minimum), Ok(maximum)) = (scalar(request.minimum), scalar(request.maximum)) else {
        return 0;
    };
    let Some(builder) = bridge
        .catalog_slot_mut(request.catalog)
        .and_then(|slot| slot.builder.as_mut())
    else {
        return 0;
    };
    builder.stats.push(StatDefinition {
        id,
        minimum,
        maximum,
    });
    ABI_OK
}

unsafe extern "C" fn define_track(
    context: *mut c_void,
    request: *const NativeMechanicsTrackDefinitionRequest,
) -> i32 {
    let Some((bridge, request)) = bridge_request(context, request) else {
        return 0;
    };
    let Ok(id) = unsafe { text(request.id, "mechanics track id") }
        .and_then(parse::<gameplay_mechanics::TrackId>)
    else {
        return 0;
    };
    let Ok(minimum) = scalar(request.minimum) else {
        return 0;
    };
    let maximum = match request.maximum_kind {
        NativeMechanicsTrackMaximumKind::Fixed => {
            scalar(request.fixed_maximum).map(|value| TrackMaximum::Fixed { value })
        }
        NativeMechanicsTrackMaximumKind::Stat => {
            unsafe { text(request.maximum_stat, "mechanics track maximum stat") }
                .and_then(parse::<StatId>)
                .map(|stat| TrackMaximum::Stat { stat })
        }
    };
    let Ok(maximum) = maximum else {
        return 0;
    };
    let Some(builder) = bridge
        .catalog_slot_mut(request.catalog)
        .and_then(|slot| slot.builder.as_mut())
    else {
        return 0;
    };
    builder.tracks.push(TrackDefinition {
        id,
        minimum,
        maximum,
    });
    ABI_OK
}

unsafe extern "C" fn define_contribution(
    context: *mut c_void,
    request: *const NativeMechanicsContributionDefinitionRequest,
) -> i32 {
    let Some((bridge, request)) = bridge_request(context, request) else {
        return 0;
    };
    let Ok(source) = unsafe { text(request.source, "mechanics source id") }
        .and_then(parse::<SourceDefinitionId>)
    else {
        return 0;
    };
    let Ok(stat) =
        unsafe { text(request.stat, "mechanics contribution stat") }.and_then(parse::<StatId>)
    else {
        return 0;
    };
    let Ok(stacking_group) = unsafe { text(request.stacking_group, "mechanics stacking group") }
        .and_then(parse::<StackingGroupId>)
    else {
        return 0;
    };
    let Ok(priority) = i16::try_from(request.priority) else {
        return 0;
    };
    let contribution = match request.kind {
        NativeMechanicsContributionKind::Add => {
            scalar(request.amount).map(|amount| StatContribution::Add { amount })
        }
        NativeMechanicsContributionKind::Scale => {
            ratio(request.ratio_numerator, request.ratio_denominator)
                .map(|ratio| StatContribution::Scale { ratio })
        }
        NativeMechanicsContributionKind::Minimum => {
            scalar(request.amount).map(|value| StatContribution::Minimum { value })
        }
        NativeMechanicsContributionKind::Maximum => {
            scalar(request.amount).map(|value| StatContribution::Maximum { value })
        }
    };
    let stacking = match request.stacking {
        NativeMechanicsStackingPolicy::Sum => StackingPolicy::Sum,
        NativeMechanicsStackingPolicy::Highest => StackingPolicy::Highest,
        NativeMechanicsStackingPolicy::Lowest => StackingPolicy::Lowest,
        NativeMechanicsStackingPolicy::UniqueBySource => StackingPolicy::UniqueBySource,
    };
    let Ok(contribution) = contribution else {
        return 0;
    };
    let Some(builder) = bridge
        .catalog_slot_mut(request.catalog)
        .and_then(|slot| slot.builder.as_mut())
    else {
        return 0;
    };
    let definition = builder
        .sources
        .entry(source.clone())
        .or_insert_with(|| SourceDefinition {
            id: source,
            priority,
            stat_contributions: Vec::new(),
            damage_responses: Vec::new(),
        });
    if definition.priority != priority {
        return 0;
    }
    definition
        .stat_contributions
        .push(StatContributionDefinition {
            stat,
            contribution,
            stacking_group,
            stacking,
        });
    ABI_OK
}

unsafe extern "C" fn define_source(
    context: *mut c_void,
    request: *const NativeMechanicsSourceDefinitionRequest,
) -> i32 {
    let Some((bridge, request)) = bridge_request(context, request) else {
        return 0;
    };
    let (Ok(id), Ok(priority)) = (
        unsafe { text(request.id, "mechanics source id") }.and_then(parse::<SourceDefinitionId>),
        i16::try_from(request.priority),
    ) else {
        return 0;
    };
    let Some(builder) = bridge
        .catalog_slot_mut(request.catalog)
        .and_then(|slot| slot.builder.as_mut())
    else {
        return 0;
    };
    if builder.sources.contains_key(&id) {
        return 0;
    }
    builder.sources.insert(
        id.clone(),
        SourceDefinition {
            id,
            priority,
            stat_contributions: Vec::new(),
            damage_responses: Vec::new(),
        },
    );
    ABI_OK
}

unsafe extern "C" fn define_damage_kind(
    context: *mut c_void,
    request: *const NativeMechanicsDamageKindDefinitionRequest,
) -> i32 {
    let Some((bridge, request)) = bridge_request(context, request) else {
        return 0;
    };
    let Ok(id) = unsafe { text(request.id, "mechanics damage kind") }
        .and_then(parse::<gameplay_mechanics::DamageKindId>)
    else {
        return 0;
    };
    let Some(builder) = bridge
        .catalog_slot_mut(request.catalog)
        .and_then(|slot| slot.builder.as_mut())
    else {
        return 0;
    };
    builder.damage_kinds.push(DamageKindDefinition { id });
    ABI_OK
}

unsafe extern "C" fn define_damage_response(
    context: *mut c_void,
    request: *const NativeMechanicsDamageResponseDefinitionRequest,
) -> i32 {
    let Some((bridge, request)) = bridge_request(context, request) else {
        return 0;
    };
    let Ok(source) = unsafe { text(request.source, "mechanics damage response source") }
        .and_then(parse::<SourceDefinitionId>)
    else {
        return 0;
    };
    let selector = if request.selector_is_exact {
        match unsafe { text(request.selector_damage_kind, "mechanics damage selector") }
            .and_then(parse::<gameplay_mechanics::DamageKindId>)
        {
            Ok(damage_kind) => DamageKindSelector::Exact { damage_kind },
            Err(_) => return 0,
        }
    } else {
        DamageKindSelector::Any
    };
    let stacking = match request.stacking {
        NativeMechanicsStackingPolicy::Sum => StackingPolicy::Sum,
        NativeMechanicsStackingPolicy::Highest => StackingPolicy::Highest,
        NativeMechanicsStackingPolicy::Lowest => StackingPolicy::Lowest,
        NativeMechanicsStackingPolicy::UniqueBySource => StackingPolicy::UniqueBySource,
    };
    let response = match request.kind {
        NativeMechanicsDamageResponseKind::Prevent => {
            let Ok(stacking_group) =
                unsafe { text(request.stacking_group, "mechanics damage stacking group") }
                    .and_then(parse::<StackingGroupId>)
            else {
                return 0;
            };
            DamageResponseDefinition::Prevent {
                selector,
                stacking_group,
                stacking,
            }
        }
        NativeMechanicsDamageResponseKind::FlatReduction => {
            let (Ok(amount), Ok(stacking_group)) = (
                scalar(request.amount),
                unsafe { text(request.stacking_group, "mechanics damage stacking group") }
                    .and_then(parse::<StackingGroupId>),
            ) else {
                return 0;
            };
            DamageResponseDefinition::FlatReduction {
                selector,
                amount,
                stacking_group,
                stacking,
            }
        }
        NativeMechanicsDamageResponseKind::Scale => {
            let (Ok(ratio), Ok(stacking_group)) = (
                ratio(request.ratio_numerator, request.ratio_denominator),
                unsafe { text(request.stacking_group, "mechanics damage stacking group") }
                    .and_then(parse::<StackingGroupId>),
            ) else {
                return 0;
            };
            DamageResponseDefinition::Scale {
                selector,
                ratio,
                stacking_group,
                stacking,
            }
        }
        NativeMechanicsDamageResponseKind::Absorb => {
            let Ok(track) = unsafe { text(request.absorb_track, "mechanics absorb track") }
                .and_then(parse::<gameplay_mechanics::TrackId>)
            else {
                return 0;
            };
            DamageResponseDefinition::Absorb { selector, track }
        }
    };
    let Some(builder) = bridge
        .catalog_slot_mut(request.catalog)
        .and_then(|slot| slot.builder.as_mut())
    else {
        return 0;
    };
    let Some(definition) = builder.sources.get_mut(&source) else {
        return 0;
    };
    definition.damage_responses.push(response);
    ABI_OK
}

unsafe extern "C" fn define_effect(
    context: *mut c_void,
    request: *const NativeMechanicsEffectDefinitionRequest,
) -> i32 {
    let Some((bridge, request)) = bridge_request(context, request) else {
        return 0;
    };
    let (Ok(id), Ok(stacking_group), Ok(sources)) = (
        unsafe { text(request.id, "mechanics effect id") }
            .and_then(parse::<gameplay_mechanics::EffectDefinitionId>),
        unsafe { text(request.stacking_group, "mechanics effect stacking group") }
            .and_then(parse::<StackingGroupId>),
        unsafe {
            text_slice(
                request.sources,
                request.sources_len,
                "mechanics effect sources",
            )
        }
        .and_then(parse_text_values::<SourceDefinitionId>),
    ) else {
        return 0;
    };
    let stacking = match request.stacking {
        NativeMechanicsEffectStackingKind::IndependentByProvenance => {
            EffectStackingPolicy::IndependentByProvenance {
                maximum_instances: request.maximum_instances,
            }
        }
        NativeMechanicsEffectStackingKind::Refresh => EffectStackingPolicy::Refresh,
        NativeMechanicsEffectStackingKind::Replace => EffectStackingPolicy::Replace,
    };
    let Some(builder) = bridge
        .catalog_slot_mut(request.catalog)
        .and_then(|slot| slot.builder.as_mut())
    else {
        return 0;
    };
    builder.effects.push(EffectDefinition {
        id,
        stacking_group,
        stacking,
        maximum_stacks: request.maximum_stacks,
        sources,
    });
    ABI_OK
}

unsafe extern "C" fn define_capacity_metric(
    context: *mut c_void,
    request: *const NativeMechanicsCapacityMetricDefinitionRequest,
) -> i32 {
    let Some((bridge, request)) = bridge_request(context, request) else {
        return 0;
    };
    let Ok(id) = unsafe { text(request.id, "mechanics capacity metric") }
        .and_then(parse::<gameplay_mechanics::CapacityMetricId>)
    else {
        return 0;
    };
    let Some(builder) = bridge
        .catalog_slot_mut(request.catalog)
        .and_then(|slot| slot.builder.as_mut())
    else {
        return 0;
    };
    builder
        .capacity_metrics
        .push(CapacityMetricDefinition { id });
    ABI_OK
}

unsafe extern "C" fn define_item(
    context: *mut c_void,
    request: *const NativeMechanicsItemDefinitionRequest,
) -> i32 {
    let Some((bridge, request)) = bridge_request(context, request) else {
        return 0;
    };
    let (Ok(id), Ok(classifications), Ok(sources), Ok(costs)) = (
        unsafe { text(request.id, "mechanics item id") }
            .and_then(parse::<gameplay_mechanics::ItemDefinitionId>),
        unsafe {
            text_slice(
                request.classifications,
                request.classifications_len,
                "mechanics item classifications",
            )
        }
        .and_then(parse_text_values::<gameplay_mechanics::ItemClassificationId>),
        unsafe {
            text_slice(
                request.sources,
                request.sources_len,
                "mechanics item sources",
            )
        }
        .and_then(parse_text_values::<SourceDefinitionId>),
        unsafe {
            borrowed_slice(
                request.capacity_costs,
                request.capacity_costs_len,
                "mechanics item capacity costs",
            )
        }
        .and_then(parse_capacity_costs),
    ) else {
        return 0;
    };
    let equipment = if request.has_equipment {
        let exclusive_group = match unsafe {
            text(
                request.exclusive_group,
                "mechanics equipment exclusive group",
            )
        } {
            Ok("") => None,
            Ok(value) => match parse::<gameplay_mechanics::EquipmentExclusivityId>(value) {
                Ok(value) => Some(value),
                Err(_) => return 0,
            },
            Err(_) => return 0,
        };
        Some(ItemEquipmentPolicy {
            required_slots: request.required_slots,
            exclusive_group,
        })
    } else {
        None
    };
    let kind = match request.kind {
        NativeMechanicsItemKind::Fungible => ItemKind::Fungible,
        NativeMechanicsItemKind::Unique => ItemKind::Unique,
    };
    let Some(builder) = bridge
        .catalog_slot_mut(request.catalog)
        .and_then(|slot| slot.builder.as_mut())
    else {
        return 0;
    };
    builder.items.push(ItemDefinition {
        id,
        kind,
        maximum_quantity: request.maximum_quantity,
        classifications,
        capacity_costs: costs,
        equipment,
        sources,
    });
    ABI_OK
}

unsafe extern "C" fn define_equipment_slot(
    context: *mut c_void,
    request: *const NativeMechanicsEquipmentSlotDefinitionRequest,
) -> i32 {
    let Some((bridge, request)) = bridge_request(context, request) else {
        return 0;
    };
    let (Ok(id), Ok(allowed_classifications)) = (
        unsafe { text(request.id, "mechanics equipment slot") }
            .and_then(parse::<gameplay_mechanics::EquipmentSlotId>),
        unsafe {
            text_slice(
                request.allowed_classifications,
                request.allowed_classifications_len,
                "mechanics slot classifications",
            )
        }
        .and_then(parse_text_values::<gameplay_mechanics::ItemClassificationId>),
    ) else {
        return 0;
    };
    let Some(builder) = bridge
        .catalog_slot_mut(request.catalog)
        .and_then(|slot| slot.builder.as_mut())
    else {
        return 0;
    };
    builder.equipment_slots.push(EquipmentSlotDefinition {
        id,
        allowed_classifications,
    });
    ABI_OK
}

unsafe extern "C" fn admit_catalog(
    context: *mut c_void,
    handle: NativeMechanicsCatalogHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeMechanicsBridge>() };
    let Some(slot) = bridge.catalog_slot_mut(handle) else {
        return 0;
    };
    let Some(builder) = slot.builder.take() else {
        return 0;
    };
    let definition = MechanicsCatalogDefinition {
        version: builder.version.clone(),
        stats: builder.stats.clone(),
        tracks: builder.tracks.clone(),
        sources: builder.sources.values().cloned().collect(),
        damage_kinds: builder.damage_kinds.clone(),
        effects: builder.effects.clone(),
        capacity_metrics: builder.capacity_metrics.clone(),
        items: builder.items.clone(),
        equipment_slots: builder.equipment_slots.clone(),
    };
    match MechanicsCatalog::admit(definition) {
        Ok(catalog) => {
            slot.catalog = Some(catalog);
            ABI_OK
        }
        Err(_) => {
            slot.builder = Some(builder);
            0
        }
    }
}

unsafe extern "C" fn destroy_catalog(
    context: *mut c_void,
    handle: NativeMechanicsCatalogHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeMechanicsBridge>() };
    if bridge
        .entities
        .values()
        .any(|binding| binding.catalog == handle.value)
    {
        return 0;
    }
    let Some(slot) = bridge.catalogs.get(&handle.value) else {
        return 0;
    };
    if bridge.canonical_entities.iter().any(|(entity, catalog)| {
        *catalog == handle.value
            && slot.world.lifecycle.get(entity).is_some_and(|record| {
                record.lifecycle != NativeMechanicsEntityLifecycle::Tombstoned
            })
    }) {
        return 0;
    }
    bridge
        .canonical_entities
        .retain(|_, catalog| *catalog != handle.value);
    bridge.catalogs.remove(&handle.value);
    ABI_OK
}

unsafe extern "C" fn bind_entity(
    context: *mut c_void,
    request: *const NativeMechanicsEntityBindRequest,
    result: *mut NativeMechanicsEntityHandle,
) -> i32 {
    let Some((bridge, request, result)) = bridge_request_result(context, request, result) else {
        return 0;
    };
    if !bridge
        .catalogs
        .get(&request.catalog.value)
        .is_some_and(|slot| slot.catalog.is_some())
    {
        return 0;
    }
    let Ok(identity) =
        unsafe { text(request.identity, "mechanics entity identity") }.map(str::to_owned)
    else {
        return 0;
    };
    if identity.trim().is_empty() {
        return 0;
    }
    let handle = bridge.next_entity;
    let Some(next_handle) = handle.checked_add(1) else {
        return 0;
    };
    let state_entity = EntityId::new(request.entity_id);
    if bridge.canonical_entities.contains_key(&state_entity) {
        return 0;
    }
    bridge.next_entity = next_handle;
    bridge
        .canonical_entities
        .insert(state_entity, request.catalog.value);
    bridge.entities.insert(
        handle,
        EntityBinding {
            catalog: request.catalog.value,
            entity: state_entity,
            identity,
            stats: None,
            tracks: None,
            intrinsic_sources: None,
            active_effects: None,
            inventory: None,
            item: None,
            equipment: None,
            initial_components_set: false,
            committed: false,
        },
    );
    *result = NativeMechanicsEntityHandle { value: handle };
    ABI_OK
}

/// Acquires a new lease for a live canonical mechanics entity after a prior lease was released.
/// It never recreates product identity or changes the catalog-scoped component state.
unsafe extern "C" fn rebind_entity(
    context: *mut c_void,
    request: *const NativeMechanicsEntityRebindRequest,
    result: *mut NativeMechanicsEntityHandle,
) -> i32 {
    let Some((bridge, request, result)) = bridge_request_result(context, request, result) else {
        return 0;
    };
    let entity = EntityId::new(request.entity_id);
    if bridge.canonical_entities.get(&entity) != Some(&request.catalog.value)
        || bridge
            .entities
            .values()
            .any(|binding| binding.catalog == request.catalog.value && binding.entity == entity)
    {
        return 0;
    }
    let Some(slot) = bridge.catalogs.get(&request.catalog.value) else {
        return 0;
    };
    let lifecycle = slot.world.lifecycle_receipt(entity);
    if lifecycle.lifecycle == NativeMechanicsEntityLifecycle::Tombstoned
        || matches!(request.guard, NativeMechanicsLifecycleGuard::Exact)
            && lifecycle.stamp != request.expected_stamp
    {
        return 0;
    }
    let handle = bridge.next_entity;
    let Some(next_handle) = handle.checked_add(1) else {
        return 0;
    };
    bridge.next_entity = next_handle;
    bridge.entities.insert(
        handle,
        EntityBinding {
            catalog: request.catalog.value,
            entity,
            identity: String::new(),
            stats: None,
            tracks: None,
            intrinsic_sources: None,
            active_effects: None,
            inventory: None,
            item: None,
            equipment: None,
            initial_components_set: true,
            committed: true,
        },
    );
    *result = NativeMechanicsEntityHandle { value: handle };
    ABI_OK
}

unsafe extern "C" fn set_initial_stat(
    context: *mut c_void,
    request: *const NativeMechanicsInitialStatRequest,
) -> i32 {
    let Some((bridge, request)) = bridge_request(context, request) else {
        return 0;
    };
    let Ok(stat) =
        unsafe { text(request.stat, "mechanics initial stat") }.and_then(parse::<StatId>)
    else {
        return 0;
    };
    let Ok(base) = scalar(request.base) else {
        return 0;
    };
    let Some(binding) = bridge
        .entities
        .get_mut(&request.entity.value)
        .filter(|binding| !binding.committed)
    else {
        return 0;
    };
    binding
        .stats
        .get_or_insert_with(Vec::new)
        .push(StatValue::new(stat, base));
    ABI_OK
}

unsafe extern "C" fn set_initial_track(
    context: *mut c_void,
    request: *const NativeMechanicsInitialTrackRequest,
) -> i32 {
    let Some((bridge, request)) = bridge_request(context, request) else {
        return 0;
    };
    let Ok(track) = unsafe { text(request.track, "mechanics initial track") }
        .and_then(parse::<gameplay_mechanics::TrackId>)
    else {
        return 0;
    };
    let Ok(current) = scalar(request.current) else {
        return 0;
    };
    let Some(binding) = bridge
        .entities
        .get_mut(&request.entity.value)
        .filter(|binding| !binding.committed)
    else {
        return 0;
    };
    binding
        .tracks
        .get_or_insert_with(Vec::new)
        .push(TrackValue::new(track, current));
    ABI_OK
}

unsafe extern "C" fn bind_intrinsic_source(
    context: *mut c_void,
    request: *const NativeMechanicsIntrinsicSourceRequest,
) -> i32 {
    let Some((bridge, request)) = bridge_request(context, request) else {
        return 0;
    };
    let Ok(instance) = unsafe { text(request.instance, "mechanics source instance") }
        .and_then(parse::<SourceInstanceId>)
    else {
        return 0;
    };
    let Ok(definition) = unsafe { text(request.definition, "mechanics source definition") }
        .and_then(parse::<SourceDefinitionId>)
    else {
        return 0;
    };
    let Some(binding) = bridge
        .entities
        .get_mut(&request.entity.value)
        .filter(|binding| !binding.committed)
    else {
        return 0;
    };
    binding
        .intrinsic_sources
        .get_or_insert_with(Vec::new)
        .push(IntrinsicSourceBinding::new(instance, definition));
    ABI_OK
}

/// Replaces the complete pre-commit component builder. Boolean presence fields
/// are intentional: an empty durable collection component is distinct from an
/// omitted component.
unsafe extern "C" fn set_initial_components(
    context: *mut c_void,
    request: *const NativeMechanicsInitialComponentsRequest,
) -> i32 {
    let Some((bridge, request)) = bridge_request(context, request) else {
        return 0;
    };
    let (
        Ok(stats),
        Ok(tracks),
        Ok(intrinsic_sources),
        Ok(active_effects),
        Ok(inventory_stacks),
        Ok(inventory_limits),
        Ok(equipment),
    ) = (
        unsafe { borrowed_slice(request.stats, request.stats_len, "mechanics initial stats") }
            .and_then(parse_initial_stats),
        unsafe {
            borrowed_slice(
                request.tracks,
                request.tracks_len,
                "mechanics initial tracks",
            )
        }
        .and_then(parse_initial_tracks),
        unsafe {
            borrowed_slice(
                request.intrinsic_sources,
                request.intrinsic_sources_len,
                "mechanics initial intrinsic sources",
            )
        }
        .and_then(parse_initial_intrinsic_sources),
        unsafe {
            borrowed_slice(
                request.active_effects,
                request.active_effects_len,
                "mechanics initial active effects",
            )
        }
        .and_then(parse_initial_active_effects),
        unsafe {
            borrowed_slice(
                request.inventory_stacks,
                request.inventory_stacks_len,
                "mechanics initial inventory stacks",
            )
        }
        .and_then(parse_initial_inventory_stacks),
        unsafe {
            borrowed_slice(
                request.inventory_capacity_limits,
                request.inventory_capacity_limits_len,
                "mechanics initial inventory capacity limits",
            )
        }
        .and_then(parse_initial_inventory_limits),
        unsafe {
            borrowed_slice(
                request.equipment_assignments,
                request.equipment_assignments_len,
                "mechanics initial equipment assignments",
            )
        }
        .and_then(parse_initial_equipment),
    )
    else {
        return 0;
    };
    let item = if request.has_item {
        match unsafe { text(request.item_definition, "mechanics initial item definition") }
            .and_then(parse::<gameplay_mechanics::ItemDefinitionId>)
        {
            Ok(value) => Some(value),
            Err(_) => return 0,
        }
    } else {
        None
    };
    let Some(binding) = bridge
        .entities
        .get_mut(&request.entity.value)
        .filter(|binding| !binding.committed && !binding.initial_components_set)
    else {
        return 0;
    };
    binding.stats = request.has_stats.then_some(stats);
    binding.tracks = request.has_tracks.then_some(tracks);
    binding.intrinsic_sources = request.has_intrinsic_sources.then_some(intrinsic_sources);
    binding.active_effects = request.has_active_effects.then_some(active_effects);
    binding.inventory = request
        .has_inventory
        .then_some((inventory_stacks, inventory_limits));
    binding.item = item;
    binding.equipment = request.has_equipment.then_some(equipment);
    binding.initial_components_set = true;
    ABI_OK
}

unsafe extern "C" fn commit_entity(
    context: *mut c_void,
    handle: NativeMechanicsEntityHandle,
    result: *mut NativeMechanicsEntityReceipt,
) -> i32 {
    if context.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeMechanicsBridge>() };
    let Some(binding) = bridge
        .entities
        .get(&handle.value)
        .filter(|binding| !binding.committed)
        .cloned()
    else {
        return 0;
    };
    let Some(slot) = bridge.catalogs.get_mut(&binding.catalog) else {
        return 0;
    };
    let Some(catalog) = slot.catalog.as_ref() else {
        return 0;
    };
    let mut candidate = slot.world.state.clone();
    let state_revision = candidate.revision();
    if EntityAuthoringService
        .admit(
            &mut candidate,
            state_revision,
            [EntityDefinition::new(
                binding.entity,
                binding.identity.clone(),
            )],
        )
        .is_err()
    {
        return 0;
    }
    if let Some(stats) = binding.stats {
        if attach(
            &mut candidate,
            binding.entity,
            StatsComponent::new(catalog.version().clone(), stats).ok(),
        )
        .is_err()
        {
            return 0;
        }
    }
    if let Some(tracks) = binding.tracks {
        if attach(
            &mut candidate,
            binding.entity,
            TracksComponent::new(catalog.version().clone(), tracks).ok(),
        )
        .is_err()
        {
            return 0;
        }
    }
    if let Some(intrinsic_sources) = binding.intrinsic_sources {
        if attach(
            &mut candidate,
            binding.entity,
            IntrinsicSourcesComponent::new(catalog.version().clone(), intrinsic_sources).ok(),
        )
        .is_err()
        {
            return 0;
        }
    }
    if let Some(active_effects) = binding.active_effects {
        if attach(
            &mut candidate,
            binding.entity,
            ActiveEffectsComponent::new(catalog.version().clone(), active_effects).ok(),
        )
        .is_err()
        {
            return 0;
        }
    }
    if let Some((stacks, capacity_limits)) = binding.inventory {
        if attach(
            &mut candidate,
            binding.entity,
            InventoryComponent::with_capacity_limits(
                catalog.version().clone(),
                stacks,
                capacity_limits,
            )
            .ok(),
        )
        .is_err()
        {
            return 0;
        }
    }
    if let Some(definition) = binding.item {
        if attach(
            &mut candidate,
            binding.entity,
            Some(ItemComponent::new(catalog.version().clone(), definition)),
        )
        .is_err()
        {
            return 0;
        }
    }
    if let Some(assignments) = binding.equipment {
        if attach(
            &mut candidate,
            binding.entity,
            EquipmentComponent::new(catalog.version().clone(), assignments).ok(),
        )
        .is_err()
        {
            return 0;
        }
    }
    if validate_state_against_catalog(&candidate, catalog).is_err() {
        return 0;
    }
    let stats_revision = candidate
        .component_revision::<StatsComponent>(binding.entity)
        .map(|revision| stats_revision(binding.entity, revision.revision()))
        .unwrap_or_default();
    let tracks_revision = candidate
        .component_revision::<TracksComponent>(binding.entity)
        .map(|revision| tracks_revision(binding.entity, revision.revision()))
        .unwrap_or_default();
    if slot.world.next_stamp == u64::MAX {
        return 0;
    }
    slot.world.state = candidate;
    let Some(lifecycle) = slot.world.admit(binding.entity) else {
        return 0;
    };
    if let Some(entry) = bridge.entities.get_mut(&handle.value) {
        entry.committed = true;
    }
    unsafe {
        *result = NativeMechanicsEntityReceipt {
            stats_revision,
            tracks_revision,
            lifecycle,
            stats_slot: component_revision::<StatsComponent>(
                &slot.world.state,
                binding.entity,
                NativeMechanicsRevisionComponent::Stats,
            ),
            tracks_slot: component_revision::<TracksComponent>(
                &slot.world.state,
                binding.entity,
                NativeMechanicsRevisionComponent::Tracks,
            ),
            intrinsic_sources_revision: component_revision::<IntrinsicSourcesComponent>(
                &slot.world.state,
                binding.entity,
                NativeMechanicsRevisionComponent::IntrinsicSources,
            ),
            active_effects_revision: component_revision::<ActiveEffectsComponent>(
                &slot.world.state,
                binding.entity,
                NativeMechanicsRevisionComponent::ActiveEffects,
            ),
            inventory_revision: component_revision::<InventoryComponent>(
                &slot.world.state,
                binding.entity,
                NativeMechanicsRevisionComponent::Inventory,
            ),
            item_revision: component_revision::<ItemComponent>(
                &slot.world.state,
                binding.entity,
                NativeMechanicsRevisionComponent::Item,
            ),
            equipment_revision: component_revision::<EquipmentComponent>(
                &slot.world.state,
                binding.entity,
                NativeMechanicsRevisionComponent::Equipment,
            ),
        }
    };
    ABI_OK
}

unsafe extern "C" fn destroy_entity(
    context: *mut c_void,
    handle: NativeMechanicsEntityHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeMechanicsBridge>() };
    let Some(binding) = bridge.entities.get(&handle.value).cloned() else {
        return 0;
    };
    bridge.entities.remove(&handle.value);
    if !binding.committed {
        bridge.canonical_entities.remove(&binding.entity);
    }
    ABI_OK
}

/// Mirrors a canonical managed EntityWorld transition. Releasing a handle does not call this;
/// a lease and a product tombstone are deliberately different operations.
unsafe extern "C" fn set_entity_lifecycle(
    context: *mut c_void,
    request: *const NativeMechanicsLifecycleRequest,
    result: *mut NativeMechanicsLifecycleReceipt,
) -> i32 {
    let Some((bridge, request, result)) = bridge_request_result(context, request, result) else {
        return 0;
    };
    let Some(binding) = bridge.binding(request.entity).cloned() else {
        return 0;
    };
    let Some(slot) = bridge.catalogs.get_mut(&binding.catalog) else {
        return 0;
    };
    let current = slot.world.lifecycle_receipt(binding.entity);
    if matches!(request.guard, NativeMechanicsLifecycleGuard::Exact)
        && current.stamp != request.expected_stamp
    {
        return 0;
    }
    let Some(receipt) = slot.world.set_lifecycle(binding.entity, request.lifecycle) else {
        return 0;
    };
    *result = receipt;
    ABI_OK
}

unsafe extern "C" fn read_stat(
    context: *mut c_void,
    request: *const NativeMechanicsStatReadRequest,
    result: *mut NativeMechanicsStatReadReceipt,
) -> i32 {
    let Some((bridge, request, result)) = bridge_request_result(context, request, result) else {
        return 0;
    };
    let Ok(stat) = unsafe { text(request.stat, "mechanics stat read") }.and_then(parse::<StatId>)
    else {
        return 0;
    };
    let Some((state, _, entity)) = bridge.state_and_catalog_mut(request.entity) else {
        return 0;
    };
    let Ok(component) = state.component::<StatsComponent>(entity) else {
        return 0;
    };
    let Some(component) = component else {
        return 0;
    };
    let Some(base) = component.base(&stat) else {
        return 0;
    };
    let Ok(revision) = state.component_revision::<StatsComponent>(entity) else {
        return 0;
    };
    *result = NativeMechanicsStatReadReceipt {
        base: base.get(),
        revision: stats_revision(entity, revision.revision()),
    };
    ABI_OK
}

unsafe extern "C" fn evaluate_stat(
    context: *mut c_void,
    request: *const NativeMechanicsStatOperationRequest,
    result: *mut NativeMechanicsStatEvaluationReceipt,
) -> i32 {
    let Some((bridge, request, result)) = bridge_request_result(context, request, result) else {
        return 0;
    };
    let (Ok(stat), Ok(operation)) = (
        unsafe { text(request.stat, "mechanics evaluation stat") }.and_then(parse::<StatId>),
        unsafe { text(request.operation, "mechanics evaluation operation") }
            .and_then(parse::<OperationId>),
    ) else {
        return 0;
    };
    let Some((state, catalog, entity)) = bridge.state_and_catalog_mut(request.entity) else {
        return 0;
    };
    let Ok(value) = StatService::evaluate(state, catalog, entity, &stat, &operation, &[]) else {
        return 0;
    };
    let Ok(revision) = state.component_revision::<StatsComponent>(entity) else {
        return 0;
    };
    *result = NativeMechanicsStatEvaluationReceipt {
        base: value.base.get(),
        value: value.value.get(),
        minimum: value.minimum.get(),
        maximum: value.maximum.get(),
        stats_revision: stats_revision(entity, revision.revision()),
    };
    ABI_OK
}

unsafe extern "C" fn read_track(
    context: *mut c_void,
    request: *const NativeMechanicsTrackReadRequest,
    result: *mut NativeMechanicsTrackReadReceipt,
) -> i32 {
    let Some((bridge, request, result)) = bridge_request_result(context, request, result) else {
        return 0;
    };
    let (Ok(track), Ok(operation)) = (
        unsafe { text(request.track, "mechanics track read") }
            .and_then(parse::<gameplay_mechanics::TrackId>),
        unsafe { text(request.operation, "mechanics track operation") }
            .and_then(parse::<OperationId>),
    ) else {
        return 0;
    };
    let Some((state, catalog, entity)) = bridge.state_and_catalog_mut(request.entity) else {
        return 0;
    };
    let Ok(component) = state.component::<TracksComponent>(entity) else {
        return 0;
    };
    let Some(component) = component else {
        return 0;
    };
    let Some(current) = component.current(&track) else {
        return 0;
    };
    let Some(definition) = catalog.track(&track) else {
        return 0;
    };
    let maximum = match &definition.maximum {
        TrackMaximum::Fixed { value } => Ok(*value),
        TrackMaximum::Stat { stat } => {
            StatService::evaluate(state, catalog, entity, stat, &operation, &[])
                .map(|evaluation| evaluation.value)
        }
    };
    let Ok(maximum) = maximum else {
        return 0;
    };
    let Ok(revision) = state.component_revision::<TracksComponent>(entity) else {
        return 0;
    };
    *result = NativeMechanicsTrackReadReceipt {
        current: current.get(),
        minimum: definition.minimum.get(),
        maximum: maximum.get(),
        revision: tracks_revision(entity, revision.revision()),
    };
    ABI_OK
}

unsafe extern "C" fn set_stat_base(
    context: *mut c_void,
    request: *const NativeMechanicsStatBaseMutationRequest,
    result: *mut NativeMechanicsStatMutationReceipt,
) -> i32 {
    let Some((bridge, request, result)) = bridge_request_result(context, request, result) else {
        return 0;
    };
    let (Ok(operation), Ok(source), Ok(stat), Ok(base)) = (
        unsafe { text(request.operation, "mechanics stat operation") }
            .and_then(parse::<OperationId>),
        unsafe { text(request.source, "mechanics stat source") }
            .and_then(parse::<SourceInstanceId>),
        unsafe { text(request.stat, "mechanics stat") }.and_then(parse::<StatId>),
        scalar(request.base),
    ) else {
        return 0;
    };
    let Some((state, catalog, entity)) = bridge.state_and_catalog_mut(request.entity) else {
        return 0;
    };
    let Ok(actual) = state.component_revision::<StatsComponent>(entity) else {
        return 0;
    };
    let Some(expected_revision) = guarded_revision(
        request.revision_guard,
        request.expected_revision.entity_id,
        request.expected_revision.revision,
        request.expected_revision.component,
        entity,
        actual,
        NativeMechanicsRevisionComponent::Stats,
    ) else {
        return 0;
    };
    let request_source = gameplay_mechanics::SourceInstanceIdentity::Request {
        operation: operation.clone(),
        instance: source,
    };
    let Ok(receipt) = StatService::set_base(
        state,
        catalog,
        StatBaseMutationRequest {
            operation,
            source: request_source,
            entity,
            stat,
            base,
            expected_revision,
        },
    ) else {
        return 0;
    };
    *result = NativeMechanicsStatMutationReceipt {
        before: receipt.before.get(),
        after: receipt.after.get(),
        minimum: receipt.minimum.get(),
        maximum: receipt.maximum.get(),
        observed_revision: stats_revision(entity, receipt.observed_stats_revision),
        committed_revision: stats_revision(entity, receipt.committed_stats_revision),
    };
    ABI_OK
}

unsafe extern "C" fn set_track(
    context: *mut c_void,
    request: *const NativeMechanicsTrackSetRequest,
    result: *mut NativeMechanicsTrackSetReceipt,
) -> i32 {
    let Some((bridge, request, result)) = bridge_request_result(context, request, result) else {
        return 0;
    };
    let (Ok(operation), Ok(source), Ok(track), Ok(value)) = (
        unsafe { text(request.operation, "mechanics track operation") }
            .and_then(parse::<OperationId>),
        unsafe { text(request.source, "mechanics track source") }
            .and_then(parse::<SourceInstanceId>),
        unsafe { text(request.track, "mechanics track") }
            .and_then(parse::<gameplay_mechanics::TrackId>),
        scalar(request.value),
    ) else {
        return 0;
    };
    let policy = match request.policy {
        NativeMechanicsTrackSetPolicy::RejectOutOfBounds => TrackSetPolicy::RejectOutOfBounds,
        NativeMechanicsTrackSetPolicy::ClampToBounds => TrackSetPolicy::ClampToBounds,
    };
    let Some((state, catalog, entity)) = bridge.state_and_catalog_mut(request.entity) else {
        return 0;
    };
    let Ok(actual) = state.component_revision::<TracksComponent>(entity) else {
        return 0;
    };
    let Some(expected_revision) = guarded_revision(
        request.revision_guard,
        request.expected_revision.entity_id,
        request.expected_revision.revision,
        request.expected_revision.component,
        entity,
        actual,
        NativeMechanicsRevisionComponent::Tracks,
    ) else {
        return 0;
    };
    let request_source = gameplay_mechanics::SourceInstanceIdentity::Request {
        operation: operation.clone(),
        instance: source,
    };
    let Ok(receipt) = TrackService::set_under_policy(
        state,
        catalog,
        TrackSetRequest {
            operation,
            source: request_source,
            entity,
            track,
            value,
            policy,
            expected_revision,
        },
    ) else {
        return 0;
    };
    *result = NativeMechanicsTrackSetReceipt {
        target: receipt.requested.get(),
        before: receipt.before.get(),
        after: receipt.after.get(),
        minimum: receipt.minimum.get(),
        maximum: receipt.maximum.get(),
        observed_revision: tracks_revision(entity, receipt.observed_tracks_revision),
        committed_revision: tracks_revision(entity, receipt.committed_tracks_revision),
    };
    ABI_OK
}

unsafe extern "C" fn spend_track(
    context: *mut c_void,
    request: *const NativeMechanicsTrackMutationRequest,
    result: *mut NativeMechanicsTrackMutationReceipt,
) -> i32 {
    mutate_track(context, request, result, TrackAdjustmentKind::Spend)
}

unsafe extern "C" fn restore_track(
    context: *mut c_void,
    request: *const NativeMechanicsTrackMutationRequest,
    result: *mut NativeMechanicsTrackMutationReceipt,
) -> i32 {
    mutate_track(context, request, result, TrackAdjustmentKind::Restore)
}

unsafe fn mutate_track(
    context: *mut c_void,
    request: *const NativeMechanicsTrackMutationRequest,
    result: *mut NativeMechanicsTrackMutationReceipt,
    kind: TrackAdjustmentKind,
) -> i32 {
    let Some((bridge, request, result)) = bridge_request_result(context, request, result) else {
        return 0;
    };
    let (Ok(operation), Ok(source), Ok(track), Ok(amount)) = (
        unsafe { text(request.operation, "mechanics adjustment operation") }
            .and_then(parse::<OperationId>),
        unsafe { text(request.source, "mechanics adjustment source") }
            .and_then(parse::<SourceInstanceId>),
        unsafe { text(request.track, "mechanics adjustment track") }
            .and_then(parse::<gameplay_mechanics::TrackId>),
        scalar(request.amount),
    ) else {
        return 0;
    };
    let Some((state, catalog, entity)) = bridge.state_and_catalog_mut(request.entity) else {
        return 0;
    };
    let Ok(actual) = state.component_revision::<TracksComponent>(entity) else {
        return 0;
    };
    let Some(expected_revision) = guarded_revision(
        request.revision_guard,
        request.expected_revision.entity_id,
        request.expected_revision.revision,
        request.expected_revision.component,
        entity,
        actual,
        NativeMechanicsRevisionComponent::Tracks,
    ) else {
        return 0;
    };
    let request = TrackMutationRequest {
        operation: operation.clone(),
        source: gameplay_mechanics::SourceInstanceIdentity::Request {
            operation,
            instance: source,
        },
        entity,
        track,
        amount,
        kind,
        expected_revision,
    };
    let receipt = match kind {
        TrackAdjustmentKind::Spend => TrackService::spend(state, catalog, request),
        TrackAdjustmentKind::Restore => TrackService::restore(state, catalog, request),
    };
    let Ok(receipt) = receipt else {
        return 0;
    };
    *result = track_mutation_receipt(
        entity,
        receipt.requested_amount.get(),
        receipt.applied_amount.get(),
        receipt.before.get(),
        receipt.after.get(),
        receipt.minimum.get(),
        receipt.maximum.get(),
        receipt.observed_tracks_revision,
        receipt.committed_tracks_revision,
    );
    ABI_OK
}

unsafe extern "C" fn reconcile_track(
    context: *mut c_void,
    request: *const NativeMechanicsTrackReconciliationRequest,
    result: *mut NativeMechanicsTrackReconciliationReceipt,
) -> i32 {
    let Some((bridge, request, result)) = bridge_request_result(context, request, result) else {
        return 0;
    };
    let (Ok(operation), Ok(source), Ok(track), Ok(prospective_maximum)) = (
        unsafe { text(request.operation, "mechanics reconciliation operation") }
            .and_then(parse::<OperationId>),
        unsafe { text(request.source, "mechanics reconciliation source") }
            .and_then(parse::<SourceInstanceId>),
        unsafe { text(request.track, "mechanics reconciliation track") }
            .and_then(parse::<gameplay_mechanics::TrackId>),
        scalar(request.prospective_maximum),
    ) else {
        return 0;
    };
    let policy = match request.policy {
        NativeMechanicsTrackReconciliationPolicy::PreserveCurrent => {
            TrackReconciliationPolicy::PreserveCurrent
        }
        NativeMechanicsTrackReconciliationPolicy::ClampToMaximum => {
            TrackReconciliationPolicy::ClampToMaximum
        }
    };
    let Some((state, catalog, entity)) = bridge.state_and_catalog_mut(request.entity) else {
        return 0;
    };
    let Ok(actual) = state.component_revision::<TracksComponent>(entity) else {
        return 0;
    };
    let Some(expected_revision) = guarded_revision(
        request.revision_guard,
        request.expected_revision.entity_id,
        request.expected_revision.revision,
        request.expected_revision.component,
        entity,
        actual,
        NativeMechanicsRevisionComponent::Tracks,
    ) else {
        return 0;
    };
    let request_source = gameplay_mechanics::SourceInstanceIdentity::Request {
        operation: operation.clone(),
        instance: source,
    };
    let Ok(receipt) = TrackService::reconcile_to_maximum(
        state,
        catalog,
        TrackReconciliationRequest {
            operation,
            source: request_source,
            entity,
            track,
            prospective_maximum,
            policy,
            expected_revision,
        },
    ) else {
        return 0;
    };
    *result = NativeMechanicsTrackReconciliationReceipt {
        before: receipt.before.get(),
        after: receipt.after.get(),
        minimum: receipt.minimum.get(),
        current_maximum: receipt.current_maximum.get(),
        prospective_maximum: receipt.prospective_maximum.get(),
        observed_revision: tracks_revision(entity, receipt.observed_tracks_revision),
        committed_revision: tracks_revision(entity, receipt.committed_tracks_revision),
    };
    ABI_OK
}

fn attach<T: entity_state::EntityComponent>(
    state: &mut EntityState,
    entity: EntityId,
    value: Option<T>,
) -> Result<(), ()> {
    let Some(value) = value else {
        return Err(());
    };
    let revision = state.component_revision::<T>(entity).map_err(|_| ())?;
    EntityAuthoringService
        .attach_component(state, revision, entity, value)
        .map(|_| ())
        .map_err(|_| ())
}

fn parse<T>(value: &str) -> Result<T, ()>
where
    T: FromMechanicsText,
{
    T::parse_text(value)
}
unsafe fn borrowed_slice<'a, T>(
    values: *const T,
    len: usize,
    _field: &'static str,
) -> Result<&'a [T], ()> {
    const MAX_BORROWED_ITEMS: usize = 1_048_576;
    if len > MAX_BORROWED_ITEMS || (len != 0 && values.is_null()) {
        return Err(());
    }
    if len == 0 {
        return Ok(&[]);
    }
    Ok(unsafe { std::slice::from_raw_parts(values, len) })
}
unsafe fn text_slice<'a>(
    values: *const NativeMechanicsText,
    len: usize,
    field: &'static str,
) -> Result<&'a [NativeMechanicsText], ()> {
    unsafe { borrowed_slice(values, len, field) }
}
fn parse_text_values<T>(values: &[NativeMechanicsText]) -> Result<Vec<T>, ()>
where
    T: FromMechanicsText,
{
    values
        .iter()
        .map(|value| unsafe { text(value.value, "mechanics text array") }.and_then(parse::<T>))
        .collect()
}
fn parse_capacity_costs(
    values: &[NativeMechanicsItemCapacityCostInput],
) -> Result<Vec<ItemCapacityCost>, ()> {
    values
        .iter()
        .map(|value| {
            Ok(ItemCapacityCost {
                metric: unsafe { text(value.metric, "mechanics item capacity metric") }
                    .and_then(parse::<gameplay_mechanics::CapacityMetricId>)?,
                units: value.units,
            })
        })
        .collect()
}
fn parse_initial_stats(values: &[NativeMechanicsInitialStatValue]) -> Result<Vec<StatValue>, ()> {
    values
        .iter()
        .map(|value| {
            Ok(StatValue::new(
                unsafe { text(value.stat, "mechanics initial stat") }.and_then(parse::<StatId>)?,
                scalar(value.base)?,
            ))
        })
        .collect()
}
fn parse_initial_tracks(
    values: &[NativeMechanicsInitialTrackValue],
) -> Result<Vec<TrackValue>, ()> {
    values
        .iter()
        .map(|value| {
            Ok(TrackValue::new(
                unsafe { text(value.track, "mechanics initial track") }
                    .and_then(parse::<gameplay_mechanics::TrackId>)?,
                scalar(value.current)?,
            ))
        })
        .collect()
}
fn parse_initial_intrinsic_sources(
    values: &[NativeMechanicsInitialIntrinsicSource],
) -> Result<Vec<IntrinsicSourceBinding>, ()> {
    values
        .iter()
        .map(|value| {
            Ok(IntrinsicSourceBinding::new(
                unsafe { text(value.instance, "mechanics initial source instance") }
                    .and_then(parse::<SourceInstanceId>)?,
                unsafe { text(value.definition, "mechanics initial source definition") }
                    .and_then(parse::<SourceDefinitionId>)?,
            ))
        })
        .collect()
}
fn parse_initial_active_effects(
    values: &[NativeMechanicsInitialActiveEffect],
) -> Result<Vec<ActiveEffectInstance>, ()> {
    values
        .iter()
        .map(|value| {
            let provenance = match value.provenance_kind {
                NativeMechanicsActiveEffectProvenanceKind::Intrinsic => {
                    gameplay_mechanics::SourceInstanceIdentity::Intrinsic {
                        entity: EntityId::new(value.provenance_entity_id),
                        instance: unsafe {
                            text(
                                value.provenance_instance,
                                "mechanics intrinsic effect provenance",
                            )
                        }
                        .and_then(parse::<SourceInstanceId>)?,
                    }
                }
                NativeMechanicsActiveEffectProvenanceKind::Effect => {
                    gameplay_mechanics::SourceInstanceIdentity::Effect {
                        entity: EntityId::new(value.provenance_entity_id),
                        effect: unsafe {
                            text(
                                value.provenance_effect,
                                "mechanics effect provenance effect",
                            )
                        }
                        .and_then(parse::<gameplay_mechanics::EffectInstanceId>)?,
                        stack: value.provenance_stack,
                        source: unsafe {
                            text(
                                value.provenance_source,
                                "mechanics effect provenance source",
                            )
                        }
                        .and_then(parse::<SourceDefinitionId>)?,
                    }
                }
                NativeMechanicsActiveEffectProvenanceKind::EquippedItem => {
                    gameplay_mechanics::SourceInstanceIdentity::EquippedItem {
                        owner: EntityId::new(value.provenance_entity_id),
                        item: EntityId::new(value.provenance_item_entity_id),
                        source: unsafe {
                            text(
                                value.provenance_source,
                                "mechanics equipment effect provenance source",
                            )
                        }
                        .and_then(parse::<SourceDefinitionId>)?,
                    }
                }
                NativeMechanicsActiveEffectProvenanceKind::Request => {
                    gameplay_mechanics::SourceInstanceIdentity::Request {
                        operation: unsafe {
                            text(
                                value.provenance_operation,
                                "mechanics request effect operation",
                            )
                        }
                        .and_then(parse::<OperationId>)?,
                        instance: unsafe {
                            text(
                                value.provenance_instance,
                                "mechanics request effect provenance",
                            )
                        }
                        .and_then(parse::<SourceInstanceId>)?,
                    }
                }
            };
            ActiveEffectInstance::new(
                unsafe { text(value.instance, "mechanics initial effect instance") }
                    .and_then(parse::<gameplay_mechanics::EffectInstanceId>)?,
                unsafe { text(value.definition, "mechanics initial effect definition") }
                    .and_then(parse::<gameplay_mechanics::EffectDefinitionId>)?,
                provenance,
                value.stacks,
            )
            .map_err(|_| ())
        })
        .collect()
}
fn parse_initial_inventory_stacks(
    values: &[NativeMechanicsInitialInventoryStack],
) -> Result<Vec<ItemStack>, ()> {
    values
        .iter()
        .map(|value| {
            Ok(ItemStack {
                definition: unsafe { text(value.definition, "mechanics initial inventory item") }
                    .and_then(parse::<gameplay_mechanics::ItemDefinitionId>)?,
                quantity: value.quantity,
            })
        })
        .collect()
}
fn parse_initial_inventory_limits(
    values: &[NativeMechanicsInitialInventoryCapacityLimit],
) -> Result<Vec<InventoryCapacityLimit>, ()> {
    values
        .iter()
        .map(|value| {
            Ok(InventoryCapacityLimit::new(
                unsafe { text(value.metric, "mechanics initial inventory metric") }
                    .and_then(parse::<gameplay_mechanics::CapacityMetricId>)?,
                value.maximum,
            ))
        })
        .collect()
}
fn parse_initial_equipment(
    values: &[NativeMechanicsInitialEquipmentAssignment],
) -> Result<Vec<EquipmentAssignment>, ()> {
    values
        .iter()
        .map(|value| {
            Ok(EquipmentAssignment {
                slot: unsafe { text(value.slot, "mechanics initial equipment slot") }
                    .and_then(parse::<gameplay_mechanics::EquipmentSlotId>)?,
                item: EntityId::new(value.item_entity_id),
            })
        })
        .collect()
}
trait FromMechanicsText: Sized {
    fn parse_text(value: &str) -> Result<Self, ()>;
}
macro_rules! identity { ($($type:ty),+ $(,)?) => { $(impl FromMechanicsText for $type { fn parse_text(value: &str) -> Result<Self, ()> { <$type>::parse(value.to_owned()).map_err(|_| ()) } })+ }; }
identity!(
    CatalogVersion,
    StatId,
    gameplay_mechanics::TrackId,
    SourceDefinitionId,
    SourceInstanceId,
    StackingGroupId,
    OperationId,
    gameplay_mechanics::DamageKindId,
    gameplay_mechanics::EffectDefinitionId,
    gameplay_mechanics::EffectInstanceId,
    gameplay_mechanics::CapacityMetricId,
    gameplay_mechanics::ItemDefinitionId,
    gameplay_mechanics::ItemClassificationId,
    gameplay_mechanics::EquipmentExclusivityId,
    gameplay_mechanics::EquipmentSlotId,
);
fn scalar(value: i64) -> Result<MechanicsScalar, ()> {
    MechanicsScalar::new(value).map_err(|_| ())
}
fn ratio(numerator: u32, denominator: u32) -> Result<ExactRatio, ()> {
    ExactRatio::new(numerator, denominator).map_err(|_| ())
}
fn guarded_revision(
    guard: NativeMechanicsRevisionGuard,
    expected_entity: u64,
    expected_revision: u64,
    expected_component: NativeMechanicsRevisionComponent,
    entity: EntityId,
    actual: ComponentRevision,
    component: NativeMechanicsRevisionComponent,
) -> Option<Option<ComponentRevision>> {
    if !revision_guard_matches(
        guard,
        expected_entity,
        expected_revision,
        expected_component,
        entity,
        actual.revision(),
        component,
    ) {
        return None;
    }
    match guard {
        NativeMechanicsRevisionGuard::Unchecked => Some(None),
        NativeMechanicsRevisionGuard::Exact => Some(Some(actual)),
    }
}
fn revision_guard_matches(
    guard: NativeMechanicsRevisionGuard,
    expected_entity: u64,
    expected_revision: u64,
    expected_component: NativeMechanicsRevisionComponent,
    entity: EntityId,
    actual_revision: u64,
    component: NativeMechanicsRevisionComponent,
) -> bool {
    match guard {
        NativeMechanicsRevisionGuard::Unchecked => true,
        NativeMechanicsRevisionGuard::Exact => {
            expected_entity == entity.raw()
                && expected_revision == actual_revision
                && expected_component as u32 == component as u32
        }
    }
}
fn stats_revision(entity: EntityId, revision: u64) -> NativeMechanicsStatsRevision {
    NativeMechanicsStatsRevision {
        entity_id: entity.raw(),
        revision,
        component: NativeMechanicsRevisionComponent::Stats,
    }
}
fn tracks_revision(entity: EntityId, revision: u64) -> NativeMechanicsTracksRevision {
    NativeMechanicsTracksRevision {
        entity_id: entity.raw(),
        revision,
        component: NativeMechanicsRevisionComponent::Tracks,
    }
}

fn component_revision<T: entity_state::EntityComponent>(
    state: &EntityState,
    entity: EntityId,
    component: NativeMechanicsRevisionComponent,
) -> NativeMechanicsComponentRevision {
    let revision = state
        .component_revision::<T>(entity)
        .map(|value| value.revision())
        .unwrap_or_default();
    let present = state.component::<T>(entity).ok().flatten().is_some();
    NativeMechanicsComponentRevision {
        entity_id: entity.raw(),
        revision,
        component,
        present,
    }
}
unsafe fn text<'a>(value: NativeUtf8Slice, field: &'static str) -> Result<&'a str, ()> {
    unsafe { borrowed_utf8(value.bytes, value.len, field) }.map_err(|_| ())
}
unsafe fn bridge_request<'a, T>(
    context: *mut c_void,
    request: *const T,
) -> Option<(&'a mut RuntimeMechanicsBridge, &'a T)> {
    if context.is_null() || request.is_null() {
        None
    } else {
        Some((unsafe { &mut *context.cast() }, unsafe { &*request }))
    }
}
unsafe fn bridge_request_result<'a, T, R>(
    context: *mut c_void,
    request: *const T,
    result: *mut R,
) -> Option<(&'a mut RuntimeMechanicsBridge, &'a T, &'a mut R)> {
    if result.is_null() {
        None
    } else {
        unsafe { bridge_request(context, request) }
            .map(|(bridge, request)| (bridge, request, unsafe { &mut *result }))
    }
}
fn track_mutation_receipt(
    entity: EntityId,
    requested_amount: i64,
    applied_amount: i64,
    before: i64,
    after: i64,
    minimum: i64,
    maximum: i64,
    observed_revision: u64,
    committed_revision: u64,
) -> NativeMechanicsTrackMutationReceipt {
    NativeMechanicsTrackMutationReceipt {
        requested_amount,
        applied_amount,
        before,
        after,
        minimum,
        maximum,
        observed_revision: tracks_revision(entity, observed_revision),
        committed_revision: tracks_revision(entity, committed_revision),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utf8(value: &'static str) -> NativeUtf8Slice {
        NativeUtf8Slice {
            bytes: value.as_ptr(),
            len: value.len(),
        }
    }

    #[test]
    fn direct_mechanics_callbacks_admit_evaluate_spend_and_release_exact_tracks() {
        let mut bridge = RuntimeMechanicsBridge::new();
        let context = (&mut bridge as *mut RuntimeMechanicsBridge).cast::<c_void>();
        let catalog_request = NativeMechanicsCatalogCreateRequest {
            version: utf8("bridge_test"),
        };
        let mut catalog = NativeMechanicsCatalogHandle::default();
        assert_eq!(
            unsafe { create_catalog(context, &catalog_request, &mut catalog) },
            ABI_OK
        );
        let stat = NativeMechanicsStatDefinitionRequest {
            catalog,
            id: utf8("strength"),
            minimum: 0,
            maximum: 100,
        };
        assert_eq!(unsafe { define_stat(context, &stat) }, ABI_OK);
        let track = NativeMechanicsTrackDefinitionRequest {
            catalog,
            id: utf8("stamina"),
            minimum: 0,
            maximum_kind: NativeMechanicsTrackMaximumKind::Stat,
            fixed_maximum: 0,
            maximum_stat: utf8("strength"),
        };
        assert_eq!(unsafe { define_track(context, &track) }, ABI_OK);
        let contribution = NativeMechanicsContributionDefinitionRequest {
            catalog,
            source: utf8("bonus"),
            priority: 0,
            stat: utf8("strength"),
            kind: NativeMechanicsContributionKind::Add,
            amount: 2,
            ratio_numerator: 0,
            ratio_denominator: 0,
            stacking_group: utf8("bonus"),
            stacking: NativeMechanicsStackingPolicy::Sum,
        };
        assert_eq!(
            unsafe { define_contribution(context, &contribution) },
            ABI_OK
        );
        assert!(ratio(3, 2).is_ok());
        assert!(ratio(u32::MAX, 1).is_err());
        assert_eq!(unsafe { admit_catalog(context, catalog) }, ABI_OK);

        let entity_request = NativeMechanicsEntityBindRequest {
            catalog,
            entity_id: 77,
            identity: utf8("actor"),
        };
        let mut entity = NativeMechanicsEntityHandle::default();
        assert_eq!(
            unsafe { bind_entity(context, &entity_request, &mut entity) },
            ABI_OK
        );
        let mut other_catalog = NativeMechanicsCatalogHandle::default();
        assert_eq!(
            unsafe {
                create_catalog(
                    context,
                    &NativeMechanicsCatalogCreateRequest {
                        version: utf8("other_bridge_test"),
                    },
                    &mut other_catalog,
                )
            },
            ABI_OK
        );
        assert_eq!(unsafe { admit_catalog(context, other_catalog) }, ABI_OK);
        let mut cross_catalog_entity = NativeMechanicsEntityHandle::default();
        assert_eq!(
            unsafe {
                bind_entity(
                    context,
                    &NativeMechanicsEntityBindRequest {
                        catalog: other_catalog,
                        entity_id: 77,
                        identity: utf8("same_product_entity"),
                    },
                    &mut cross_catalog_entity,
                )
            },
            0
        );
        assert_eq!(unsafe { destroy_catalog(context, other_catalog) }, ABI_OK);
        let mut duplicate_entity = NativeMechanicsEntityHandle::default();
        assert_eq!(
            unsafe { bind_entity(context, &entity_request, &mut duplicate_entity) },
            0
        );
        let initial_stat = NativeMechanicsInitialStatRequest {
            entity,
            stat: utf8("strength"),
            base: 10,
        };
        assert_eq!(unsafe { set_initial_stat(context, &initial_stat) }, ABI_OK);
        let initial_track = NativeMechanicsInitialTrackRequest {
            entity,
            track: utf8("stamina"),
            current: 12,
        };
        assert_eq!(
            unsafe { set_initial_track(context, &initial_track) },
            ABI_OK
        );
        let intrinsic = NativeMechanicsIntrinsicSourceRequest {
            entity,
            instance: utf8("bonus_instance"),
            definition: utf8("bonus"),
        };
        assert_eq!(
            unsafe { bind_intrinsic_source(context, &intrinsic) },
            ABI_OK
        );
        let mut entity_receipt = NativeMechanicsEntityReceipt::default();
        assert_eq!(
            unsafe { commit_entity(context, entity, &mut entity_receipt) },
            ABI_OK
        );
        assert_eq!(
            entity_receipt.stats_slot.component,
            NativeMechanicsRevisionComponent::Stats
        );
        assert!(entity_receipt.stats_slot.present);
        assert_eq!(
            entity_receipt.tracks_slot.component,
            NativeMechanicsRevisionComponent::Tracks
        );
        assert!(entity_receipt.tracks_slot.present);
        assert_eq!(
            entity_receipt.intrinsic_sources_revision.component,
            NativeMechanicsRevisionComponent::IntrinsicSources
        );
        assert!(entity_receipt.intrinsic_sources_revision.present);
        assert_eq!(
            entity_receipt.active_effects_revision.component,
            NativeMechanicsRevisionComponent::ActiveEffects
        );
        assert!(!entity_receipt.active_effects_revision.present);
        assert_eq!(
            entity_receipt.inventory_revision.component,
            NativeMechanicsRevisionComponent::Inventory
        );
        assert!(!entity_receipt.inventory_revision.present);
        assert_eq!(
            entity_receipt.item_revision.component,
            NativeMechanicsRevisionComponent::Item
        );
        assert!(!entity_receipt.item_revision.present);
        assert_eq!(
            entity_receipt.equipment_revision.component,
            NativeMechanicsRevisionComponent::Equipment
        );
        assert!(!entity_receipt.equipment_revision.present);
        let second_entity_request = NativeMechanicsEntityBindRequest {
            catalog,
            entity_id: 78,
            identity: utf8("second_actor"),
        };
        let mut second_entity = NativeMechanicsEntityHandle::default();
        assert_eq!(
            unsafe { bind_entity(context, &second_entity_request, &mut second_entity) },
            ABI_OK
        );
        assert_eq!(
            unsafe {
                set_initial_stat(
                    context,
                    &NativeMechanicsInitialStatRequest {
                        entity: second_entity,
                        stat: utf8("strength"),
                        base: 10,
                    },
                )
            },
            ABI_OK
        );
        assert_eq!(
            unsafe {
                set_initial_track(
                    context,
                    &NativeMechanicsInitialTrackRequest {
                        entity: second_entity,
                        track: utf8("stamina"),
                        current: 10,
                    },
                )
            },
            ABI_OK
        );
        let mut second_receipt = NativeMechanicsEntityReceipt::default();
        assert_eq!(
            unsafe { commit_entity(context, second_entity, &mut second_receipt) },
            ABI_OK
        );
        assert_eq!(
            entity_receipt.tracks_revision.revision,
            second_receipt.tracks_revision.revision
        );
        let cross_entity_request = NativeMechanicsTrackMutationRequest {
            entity: second_entity,
            operation: utf8("foreign_spend"),
            source: utf8("foreign_source"),
            track: utf8("stamina"),
            amount: 1,
            revision_guard: NativeMechanicsRevisionGuard::Exact,
            expected_revision: entity_receipt.tracks_revision,
        };
        let mut rejected_cross_entity = NativeMechanicsTrackMutationReceipt::default();
        assert_eq!(
            unsafe { spend_track(context, &cross_entity_request, &mut rejected_cross_entity) },
            0
        );
        let cross_component_request = NativeMechanicsStatBaseMutationRequest {
            entity,
            operation: utf8("cross_component"),
            source: utf8("cross_component_source"),
            stat: utf8("strength"),
            base: 11,
            revision_guard: NativeMechanicsRevisionGuard::Exact,
            expected_revision: NativeMechanicsStatsRevision {
                entity_id: entity_receipt.stats_revision.entity_id,
                revision: entity_receipt.stats_revision.revision,
                component: NativeMechanicsRevisionComponent::Tracks,
            },
        };
        let mut rejected_cross_component = NativeMechanicsStatMutationReceipt::default();
        assert_eq!(
            unsafe {
                set_stat_base(
                    context,
                    &cross_component_request,
                    &mut rejected_cross_component,
                )
            },
            0
        );

        let evaluation_request = NativeMechanicsStatOperationRequest {
            entity,
            stat: utf8("strength"),
            operation: utf8("evaluate"),
        };
        let mut evaluation = NativeMechanicsStatEvaluationReceipt::default();
        assert_eq!(
            unsafe { evaluate_stat(context, &evaluation_request, &mut evaluation) },
            ABI_OK
        );
        assert_eq!(evaluation.value, 12);

        let spend_request = NativeMechanicsTrackMutationRequest {
            entity,
            operation: utf8("spend"),
            source: utf8("spend_source"),
            track: utf8("stamina"),
            amount: 2,
            revision_guard: NativeMechanicsRevisionGuard::Exact,
            expected_revision: entity_receipt.tracks_revision,
        };
        let mut spend = NativeMechanicsTrackMutationReceipt::default();
        assert_eq!(
            unsafe { spend_track(context, &spend_request, &mut spend) },
            ABI_OK
        );
        assert_eq!(spend.before, 12);
        assert_eq!(spend.after, 10);
        assert_eq!(spend.applied_amount, 2);
        let set_request = NativeMechanicsTrackSetRequest {
            entity,
            operation: utf8("set"),
            source: utf8("set_source"),
            track: utf8("stamina"),
            value: 9,
            policy: NativeMechanicsTrackSetPolicy::RejectOutOfBounds,
            revision_guard: NativeMechanicsRevisionGuard::Exact,
            expected_revision: spend.committed_revision,
        };
        let mut set = NativeMechanicsTrackSetReceipt::default();
        assert_eq!(
            unsafe { set_track(context, &set_request, &mut set) },
            ABI_OK
        );
        assert_eq!(set.target, 9);
        assert_eq!(set.after, 9);
        assert_eq!(unsafe { destroy_catalog(context, catalog) }, 0);
        let state_entity = bridge.entities[&entity.value].entity;
        let released_entity = entity;
        assert_eq!(unsafe { destroy_entity(context, released_entity) }, ABI_OK);
        assert!(bridge.catalogs[&catalog.value]
            .world
            .state
            .is_alive(state_entity));
        let mut rebound_entity = NativeMechanicsEntityHandle::default();
        assert_eq!(
            unsafe {
                rebind_entity(
                    context,
                    &NativeMechanicsEntityRebindRequest {
                        catalog,
                        entity_id: 77,
                        guard: NativeMechanicsLifecycleGuard::Exact,
                        expected_stamp: entity_receipt.lifecycle.stamp,
                    },
                    &mut rebound_entity,
                )
            },
            ABI_OK
        );
        let mut rebound_stat = NativeMechanicsStatReadReceipt::default();
        assert_eq!(
            unsafe {
                read_stat(
                    context,
                    &NativeMechanicsStatReadRequest {
                        entity: rebound_entity,
                        stat: utf8("strength"),
                    },
                    &mut rebound_stat,
                )
            },
            ABI_OK
        );
        entity = rebound_entity;
        let mut disabled = NativeMechanicsLifecycleReceipt::default();
        assert_eq!(
            unsafe {
                set_entity_lifecycle(
                    context,
                    &NativeMechanicsLifecycleRequest {
                        entity,
                        lifecycle: NativeMechanicsEntityLifecycle::Disabled,
                        guard: NativeMechanicsLifecycleGuard::Exact,
                        expected_stamp: entity_receipt.lifecycle.stamp,
                    },
                    &mut disabled,
                )
            },
            ABI_OK
        );
        assert_eq!(disabled.lifecycle, NativeMechanicsEntityLifecycle::Disabled);
        let mut rejected_stale = NativeMechanicsLifecycleReceipt::default();
        assert_eq!(
            unsafe {
                set_entity_lifecycle(
                    context,
                    &NativeMechanicsLifecycleRequest {
                        entity,
                        lifecycle: NativeMechanicsEntityLifecycle::Tombstoned,
                        guard: NativeMechanicsLifecycleGuard::Exact,
                        expected_stamp: entity_receipt.lifecycle.stamp,
                    },
                    &mut rejected_stale,
                )
            },
            0
        );
        let mut enabled = NativeMechanicsLifecycleReceipt::default();
        assert_eq!(
            unsafe {
                set_entity_lifecycle(
                    context,
                    &NativeMechanicsLifecycleRequest {
                        entity,
                        lifecycle: NativeMechanicsEntityLifecycle::Active,
                        guard: NativeMechanicsLifecycleGuard::Exact,
                        expected_stamp: disabled.stamp,
                    },
                    &mut enabled,
                )
            },
            ABI_OK
        );
        let mut tombstone = NativeMechanicsLifecycleReceipt::default();
        assert_eq!(
            unsafe {
                set_entity_lifecycle(
                    context,
                    &NativeMechanicsLifecycleRequest {
                        entity,
                        lifecycle: NativeMechanicsEntityLifecycle::Tombstoned,
                        guard: NativeMechanicsLifecycleGuard::Exact,
                        expected_stamp: enabled.stamp,
                    },
                    &mut tombstone,
                )
            },
            ABI_OK
        );
        assert_eq!(
            tombstone.lifecycle,
            NativeMechanicsEntityLifecycle::Tombstoned
        );
        assert_eq!(unsafe { destroy_entity(context, entity) }, ABI_OK);
        assert!(!bridge.catalogs[&catalog.value]
            .world
            .state
            .is_alive(state_entity));
        assert_eq!(unsafe { destroy_catalog(context, catalog) }, 0);
        let mut second_tombstone = NativeMechanicsLifecycleReceipt::default();
        assert_eq!(
            unsafe {
                set_entity_lifecycle(
                    context,
                    &NativeMechanicsLifecycleRequest {
                        entity: second_entity,
                        lifecycle: NativeMechanicsEntityLifecycle::Tombstoned,
                        guard: NativeMechanicsLifecycleGuard::Exact,
                        expected_stamp: second_receipt.lifecycle.stamp,
                    },
                    &mut second_tombstone,
                )
            },
            ABI_OK
        );
        assert_eq!(unsafe { destroy_entity(context, second_entity) }, ABI_OK);
        assert_eq!(unsafe { destroy_catalog(context, catalog) }, ABI_OK);
        let mut after_release = NativeMechanicsStatReadReceipt::default();
        assert_eq!(
            unsafe {
                read_stat(
                    context,
                    &NativeMechanicsStatReadRequest {
                        entity,
                        stat: utf8("strength"),
                    },
                    &mut after_release,
                )
            },
            0
        );
    }

    #[test]
    fn revision_guards_accept_zero_only_when_the_scope_also_matches() {
        assert!(revision_guard_matches(
            NativeMechanicsRevisionGuard::Exact,
            77,
            0,
            NativeMechanicsRevisionComponent::Stats,
            EntityId::new(77),
            0,
            NativeMechanicsRevisionComponent::Stats,
        ));
        assert!(!revision_guard_matches(
            NativeMechanicsRevisionGuard::Exact,
            78,
            0,
            NativeMechanicsRevisionComponent::Stats,
            EntityId::new(77),
            0,
            NativeMechanicsRevisionComponent::Stats,
        ));
        assert!(!revision_guard_matches(
            NativeMechanicsRevisionGuard::Exact,
            77,
            0,
            NativeMechanicsRevisionComponent::Tracks,
            EntityId::new(77),
            0,
            NativeMechanicsRevisionComponent::Stats,
        ));
        assert!(revision_guard_matches(
            NativeMechanicsRevisionGuard::Unchecked,
            0,
            0,
            NativeMechanicsRevisionComponent::Stats,
            EntityId::new(77),
            12,
            NativeMechanicsRevisionComponent::Tracks,
        ));
    }

    #[test]
    fn initial_component_bundle_preserves_empty_present_component_slots() {
        let mut bridge = RuntimeMechanicsBridge::new();
        let context = (&mut bridge as *mut RuntimeMechanicsBridge).cast::<c_void>();
        let mut catalog = NativeMechanicsCatalogHandle::default();
        assert_eq!(
            unsafe {
                create_catalog(
                    context,
                    &NativeMechanicsCatalogCreateRequest {
                        version: utf8("empty-components"),
                    },
                    &mut catalog,
                )
            },
            ABI_OK
        );
        assert_eq!(unsafe { admit_catalog(context, catalog) }, ABI_OK);
        let mut entity = NativeMechanicsEntityHandle::default();
        assert_eq!(
            unsafe {
                bind_entity(
                    context,
                    &NativeMechanicsEntityBindRequest {
                        catalog,
                        entity_id: 91,
                        identity: utf8("item-owner"),
                    },
                    &mut entity,
                )
            },
            ABI_OK
        );
        assert_eq!(
            unsafe {
                set_initial_components(
                    context,
                    &NativeMechanicsInitialComponentsRequest {
                        entity,
                        has_stats: true,
                        stats: std::ptr::null(),
                        stats_len: 0,
                        has_tracks: true,
                        tracks: std::ptr::null(),
                        tracks_len: 0,
                        has_intrinsic_sources: true,
                        intrinsic_sources: std::ptr::null(),
                        intrinsic_sources_len: 0,
                        has_active_effects: true,
                        active_effects: std::ptr::null(),
                        active_effects_len: 0,
                        has_inventory: true,
                        inventory_stacks: std::ptr::null(),
                        inventory_stacks_len: 0,
                        inventory_capacity_limits: std::ptr::null(),
                        inventory_capacity_limits_len: 0,
                        has_item: false,
                        item_definition: utf8(""),
                        has_equipment: true,
                        equipment_assignments: std::ptr::null(),
                        equipment_assignments_len: 0,
                    },
                )
            },
            ABI_OK
        );
        let mut receipt = NativeMechanicsEntityReceipt::default();
        assert_eq!(
            unsafe { commit_entity(context, entity, &mut receipt) },
            ABI_OK
        );
        assert!(receipt.stats_slot.present);
        assert!(receipt.tracks_slot.present);
        assert!(receipt.intrinsic_sources_revision.present);
        assert!(receipt.active_effects_revision.present);
        assert!(receipt.inventory_revision.present);
        assert!(!receipt.item_revision.present);
        assert!(receipt.equipment_revision.present);
    }
}
