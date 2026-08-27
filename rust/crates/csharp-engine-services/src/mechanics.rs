use std::{collections::BTreeMap, ffi::c_void};

use core_ids::EntityId;
use csharp_engine_abi::*;
use entity_state::{ComponentRevision, EntityAuthoringService, EntityDefinition, EntityState};
use gameplay_mechanics::{
    gameplay_component_registry, validate_state_against_catalog, CatalogVersion, ExactRatio,
    IntrinsicSourceBinding, IntrinsicSourcesComponent, MechanicsCatalog,
    MechanicsCatalogDefinition, MechanicsScalar, OperationId, SourceDefinition, SourceDefinitionId,
    SourceInstanceId, StackingGroupId, StackingPolicy, StatBaseMutationRequest, StatContribution,
    StatContributionDefinition, StatDefinition, StatId, StatService, StatValue, StatsComponent,
    TrackAdjustmentKind, TrackDefinition, TrackMaximum, TrackMutationRequest,
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
}

struct CatalogSlot {
    builder: Option<CatalogBuilder>,
    catalog: Option<MechanicsCatalog>,
    state: EntityState,
}

#[derive(Clone)]
struct EntityBinding {
    catalog: u64,
    entity: EntityId,
    identity: String,
    stats: Vec<StatValue>,
    tracks: Vec<TrackValue>,
    intrinsic_sources: Vec<IntrinsicSourceBinding>,
    committed: bool,
}

pub(crate) struct RuntimeMechanicsBridge {
    catalogs: BTreeMap<u64, CatalogSlot>,
    entities: BTreeMap<u64, EntityBinding>,
    next_catalog: u64,
    next_entity: u64,
}

impl RuntimeMechanicsBridge {
    pub(crate) fn new() -> Self {
        Self {
            catalogs: BTreeMap::new(),
            entities: BTreeMap::new(),
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
        Some((&mut slot.state, slot.catalog.as_ref()?, binding.entity))
    }
}

pub(crate) fn api(bridge: &mut RuntimeMechanicsBridge) -> NativeMechanicsApi {
    NativeMechanicsApi {
        context: (bridge as *mut RuntimeMechanicsBridge).cast(),
        create_catalog,
        define_stat,
        define_track,
        define_contribution,
        admit_catalog,
        destroy_catalog,
        bind_entity,
        set_initial_stat,
        set_initial_track,
        bind_intrinsic_source,
        commit_entity,
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
            }),
            catalog: None,
            state,
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
        damage_kinds: Vec::new(),
        effects: Vec::new(),
        capacity_metrics: Vec::new(),
        items: Vec::new(),
        equipment_slots: Vec::new(),
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
    if bridge.catalogs.remove(&handle.value).is_some() {
        ABI_OK
    } else {
        0
    }
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
    if bridge
        .entities
        .values()
        .any(|binding| binding.catalog == request.catalog.value && binding.entity == state_entity)
    {
        return 0;
    }
    bridge.next_entity = next_handle;
    bridge.entities.insert(
        handle,
        EntityBinding {
            catalog: request.catalog.value,
            entity: state_entity,
            identity,
            stats: Vec::new(),
            tracks: Vec::new(),
            intrinsic_sources: Vec::new(),
            committed: false,
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
    binding.stats.push(StatValue::new(stat, base));
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
    binding.tracks.push(TrackValue::new(track, current));
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
        .push(IntrinsicSourceBinding::new(instance, definition));
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
    let mut candidate = slot.state.clone();
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
    if attach(
        &mut candidate,
        binding.entity,
        StatsComponent::new(catalog.version().clone(), binding.stats).ok(),
    )
    .is_err()
    {
        return 0;
    }
    if attach(
        &mut candidate,
        binding.entity,
        TracksComponent::new(catalog.version().clone(), binding.tracks).ok(),
    )
    .is_err()
    {
        return 0;
    }
    if !binding.intrinsic_sources.is_empty()
        && attach(
            &mut candidate,
            binding.entity,
            IntrinsicSourcesComponent::new(catalog.version().clone(), binding.intrinsic_sources)
                .ok(),
        )
        .is_err()
    {
        return 0;
    }
    if validate_state_against_catalog(&candidate, catalog).is_err() {
        return 0;
    }
    let stats_revision = candidate
        .component_revision::<StatsComponent>(binding.entity)
        .map(|revision| revision.revision())
        .unwrap_or_default();
    let tracks_revision = candidate
        .component_revision::<TracksComponent>(binding.entity)
        .map(|revision| revision.revision())
        .unwrap_or_default();
    slot.state = candidate;
    if let Some(entry) = bridge.entities.get_mut(&handle.value) {
        entry.committed = true;
    }
    unsafe {
        *result = NativeMechanicsEntityReceipt {
            stats_revision,
            tracks_revision,
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
    if binding.committed {
        let Some(slot) = bridge.catalogs.get_mut(&binding.catalog) else {
            return 0;
        };
        let state_revision = slot.state.revision();
        if EntityAuthoringService
            .destroy(&mut slot.state, state_revision, binding.entity)
            .is_err()
        {
            return 0;
        }
    }
    bridge.entities.remove(&handle.value);
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
        revision: revision.revision(),
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
        stats_revision: revision.revision(),
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
        revision: revision.revision(),
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
    let Some(expected_revision) =
        guarded_revision(request.revision_guard, request.expected_revision, actual)
    else {
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
        observed_revision: receipt.observed_stats_revision,
        committed_revision: receipt.committed_stats_revision,
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
    let Some(expected_revision) =
        guarded_revision(request.revision_guard, request.expected_revision, actual)
    else {
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
        observed_revision: receipt.observed_tracks_revision,
        committed_revision: receipt.committed_tracks_revision,
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
    let Some(expected_revision) =
        guarded_revision(request.revision_guard, request.expected_revision, actual)
    else {
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
    let Some(expected_revision) =
        guarded_revision(request.revision_guard, request.expected_revision, actual)
    else {
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
        observed_revision: receipt.observed_tracks_revision,
        committed_revision: receipt.committed_tracks_revision,
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
    OperationId
);
fn scalar(value: i64) -> Result<MechanicsScalar, ()> {
    MechanicsScalar::new(value).map_err(|_| ())
}
fn ratio(numerator: u32, denominator: u32) -> Result<ExactRatio, ()> {
    ExactRatio::new(numerator, denominator).map_err(|_| ())
}
fn guarded_revision(
    guard: NativeMechanicsRevisionGuard,
    expected: u64,
    actual: ComponentRevision,
) -> Option<Option<ComponentRevision>> {
    match guard {
        NativeMechanicsRevisionGuard::Unchecked => Some(None),
        NativeMechanicsRevisionGuard::Exact if expected == actual.revision() => Some(Some(actual)),
        NativeMechanicsRevisionGuard::Exact => None,
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
        observed_revision,
        committed_revision,
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
        assert_eq!(unsafe { destroy_entity(context, entity) }, ABI_OK);
        assert!(!bridge.catalogs[&catalog.value].state.is_alive(state_entity));
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
}
