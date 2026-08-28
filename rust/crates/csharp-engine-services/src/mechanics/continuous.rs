//! Continuous Mechanics callbacks sharing the exact Mechanics entity world.
//!
//! Catalogs in this module are definitions and bounded-readout owners only.
//! They never allocate an `EntityState`: every request first resolves the
//! existing committed exact Mechanics binding and then mutates that one world.

use std::{collections::BTreeMap, ffi::c_void};

use core_ids::EntityId;
use csharp_engine_abi::*;
use entity_state::{
    ComponentRevision, EntityAuthoringService, EntityComponent, EntityLifecycle, EntityState,
};
use gameplay_continuous_mechanics::{
    validate_state_against_continuous_catalog, ContinuousActiveEffectInstance,
    ContinuousActiveEffectsComponent, ContinuousCatalogVersion, ContinuousDecisionOutcome,
    ContinuousEffectApplyRequest, ContinuousEffectDefinition, ContinuousEffectDefinitionId,
    ContinuousEffectRemoveRequest, ContinuousEffectService, ContinuousIntrinsicSourceBinding,
    ContinuousIntrinsicSourcesComponent, ContinuousMechanicsCatalog,
    ContinuousMechanicsCatalogDefinition, ContinuousMechanicsComponentKind, ContinuousOperationId,
    ContinuousSourceDefinition, ContinuousSourceDefinitionId, ContinuousSourceIdentity,
    ContinuousStackingPolicy, ContinuousStatBaseMutationRequest, ContinuousStatContribution,
    ContinuousStatContributionDefinition, ContinuousStatDefinition, ContinuousStatId,
    ContinuousStatService, ContinuousStatValue, ContinuousStatsComponent,
    ContinuousTrackAdjustmentKind, ContinuousTrackAdjustmentRequest, ContinuousTrackDefinition,
    ContinuousTrackId, ContinuousTrackMaximum, ContinuousTrackService, ContinuousTrackSetPolicy,
    ContinuousTrackSetRequest, ContinuousTrackValue, ContinuousTracksComponent, ContinuousValue,
};

use super::{bridge_request, bridge_request_result, native_utf8, RuntimeMechanicsBridge};
use crate::composition::{borrowed_slice, borrowed_utf8, ABI_OK};

#[derive(Default)]
pub(crate) struct ContinuousMechanicsBridgeState {
    catalogs: BTreeMap<u64, ContinuousMechanicsCatalog>,
    /// Canonical entity identity -> its one attached continuous catalog.
    pub(crate) associations: BTreeMap<EntityId, u64>,
    catalog_leases: BTreeMap<u64, Box<CatalogLease>>,
    component_leases: BTreeMap<u64, Box<ComponentLease>>,
    operation_leases: BTreeMap<u64, Box<OperationLease>>,
    world_export_leases: BTreeMap<u64, Box<ContinuousWorldExportLease>>,
    /// Continuous import leases borrow their receipt rows from the one exact
    /// import candidate identified by this value.
    pub(crate) world_import_leases: BTreeMap<u64, u64>,
    next_catalog: u64,
    next_catalog_lease: u64,
    next_component_lease: u64,
    next_operation_lease: u64,
    next_world_export_lease: u64,
    next_world_import_lease: u64,
}

struct ContinuousWorldExportLease {
    _text: TextPool,
    catalog_version: NativeUtf8Slice,
    catalog_fingerprint: NativeUtf8Slice,
    component_presence: Vec<NativeContinuousMechanicsWorldComponentPresenceRow>,
    stats: Vec<NativeContinuousMechanicsWorldStatRow>,
    tracks: Vec<NativeContinuousMechanicsWorldTrackRow>,
    intrinsic_sources: Vec<NativeContinuousMechanicsWorldIntrinsicSourceRow>,
    active_effects: Vec<NativeContinuousMechanicsWorldActiveEffectRow>,
}

/// Kept by the existing exact import candidate.  It has no independent publish
/// path and contains only continuous facts keyed by canonical entity IDs.
pub(crate) struct PreparedContinuousMechanicsWorldImportStage {
    pub(crate) catalog: u64,
    pub(crate) catalog_version: NativeUtf8Slice,
    pub(crate) catalog_fingerprint: NativeUtf8Slice,
    pub(crate) associations: BTreeMap<EntityId, u64>,
    pub(crate) revisions: Vec<NativeContinuousMechanicsRevisionRemapRow>,
    _text: TextPool,
}

struct CatalogLease {
    _text: TextPool,
    stats: Vec<NativeContinuousMechanicsCatalogStatRow>,
    tracks: Vec<NativeContinuousMechanicsCatalogTrackRow>,
    sources: Vec<NativeContinuousMechanicsCatalogSourceRow>,
    contributions: Vec<NativeContinuousMechanicsCatalogContributionRow>,
    effects: Vec<NativeContinuousMechanicsCatalogEffectRow>,
    effect_sources: Vec<NativeContinuousMechanicsCatalogEffectSourceRow>,
    version: NativeUtf8Slice,
    fingerprint: NativeUtf8Slice,
}

struct ComponentLease {
    _text: TextPool,
    catalog_version: NativeUtf8Slice,
    catalog_fingerprint: NativeUtf8Slice,
    components: Vec<NativeContinuousMechanicsComponentPresenceRow>,
    stats: Vec<NativeContinuousMechanicsInitialStatRow>,
    tracks: Vec<NativeContinuousMechanicsInitialTrackRow>,
    intrinsic_sources: Vec<NativeContinuousMechanicsInitialIntrinsicSourceRow>,
    active_effects: Vec<NativeContinuousMechanicsInitialActiveEffectRow>,
}

enum OperationLease {
    StatEvaluation {
        _text: TextPool,
        decisions: Vec<NativeContinuousMechanicsStatDecisionRow>,
        catalog_version: NativeUtf8Slice,
        catalog_fingerprint: NativeUtf8Slice,
        stat: NativeUtf8Slice,
    },
    StatMutation {
        _text: TextPool,
        catalog_version: NativeUtf8Slice,
        catalog_fingerprint: NativeUtf8Slice,
        operation: NativeUtf8Slice,
        stat: NativeUtf8Slice,
    },
    Track {
        _text: TextPool,
        catalog_version: NativeUtf8Slice,
        catalog_fingerprint: NativeUtf8Slice,
        operation: NativeUtf8Slice,
        track: NativeUtf8Slice,
    },
    Effect {
        _text: TextPool,
        catalog_version: NativeUtf8Slice,
        catalog_fingerprint: NativeUtf8Slice,
        operation: NativeUtf8Slice,
        instance: NativeUtf8Slice,
    },
}

#[derive(Default)]
struct TextPool {
    values: Vec<String>,
}

impl TextPool {
    fn copy(&mut self, value: impl AsRef<str>) -> NativeUtf8Slice {
        self.values.push(value.as_ref().to_owned());
        native_utf8(
            self.values
                .last()
                .expect("just copied continuous text")
                .as_bytes(),
        )
    }
}

pub(crate) fn api(bridge: &mut RuntimeMechanicsBridge) -> NativeContinuousMechanicsApi {
    NativeContinuousMechanicsApi {
        context: (bridge as *mut RuntimeMechanicsBridge).cast(),
        create_catalog: receipt_create_catalog,
        destroy_catalog: receipt_destroy_catalog,
        read_catalog: receipt_read_catalog,
        destroy_catalog_lease,
        set_initial_components: receipt_set_initial_components,
        read_components: receipt_read_components,
        destroy_component_lease,
        export_world: receipt_export_world,
        destroy_world_export_lease,
        stage_world_import: receipt_stage_world_import,
        destroy_world_import_lease,
        evaluate_stat: receipt_evaluate_stat,
        set_stat_base: receipt_set_stat_base,
        read_track: receipt_read_track,
        set_track: receipt_set_track,
        spend_track: receipt_spend_track,
        restore_track: receipt_restore_track,
        apply_effect: receipt_apply_effect,
        remove_effect: receipt_remove_effect,
        destroy_operation_lease,
        destroy_operation_diagnostic_lease,
    }
}

macro_rules! continuous_callback {
    ($name:ident($($argument:ident : $ty:ty),*) => $inner:path, $operation:literal) => {
        unsafe extern "C" fn $name(context: *mut c_void, $($argument: $ty,)* receipt: *mut NativeOperationErrorReceipt) -> i32 {
            unsafe {
                super::invoke_with_operation_diagnostic(
                    context, receipt, $operation, || $inner(context, $($argument),*),
                    |_| (b"CONTINUOUS_MECHANICS_OPERATION_FAILED", b"Continuous Mechanics operation failed.", String::new()),
                )
            }
        }
    };
}

continuous_callback!(receipt_create_catalog(request: *const NativeContinuousMechanicsCatalogCreateRequest, result: *mut NativeContinuousMechanicsCatalogHandle) => create_catalog, b"CreateCatalog");
continuous_callback!(receipt_destroy_catalog(handle: NativeContinuousMechanicsCatalogHandle) => destroy_catalog, b"DestroyCatalog");
continuous_callback!(receipt_read_catalog(handle: NativeContinuousMechanicsCatalogHandle, result: *mut NativeContinuousMechanicsCatalogLease) => read_catalog, b"ReadCatalog");
continuous_callback!(receipt_set_initial_components(request: *const NativeContinuousMechanicsInitialComponentsRequest) => set_initial_components, b"SetInitialComponents");
continuous_callback!(receipt_read_components(request: *const NativeContinuousMechanicsComponentReadRequest, result: *mut NativeContinuousMechanicsComponentLease) => read_components, b"ReadComponents");
continuous_callback!(receipt_export_world(request: *const NativeContinuousMechanicsWorldExportRequest, result: *mut NativeContinuousMechanicsWorldExportLease) => export_world, b"ExportWorld");
continuous_callback!(receipt_stage_world_import(request: *const NativeContinuousMechanicsWorldImportStageRequest, result: *mut NativeContinuousMechanicsWorldImportLease) => stage_world_import, b"StageWorldImport");
continuous_callback!(receipt_evaluate_stat(request: *const NativeContinuousMechanicsStatEvaluateRequest, result: *mut NativeContinuousMechanicsStatEvaluationLease) => evaluate_stat, b"EvaluateStat");
continuous_callback!(receipt_set_stat_base(request: *const NativeContinuousMechanicsStatBaseMutationRequest, result: *mut NativeContinuousMechanicsStatMutationLease) => set_stat_base, b"SetStatBase");
continuous_callback!(receipt_read_track(request: *const NativeContinuousMechanicsTrackReadRequest, result: *mut NativeContinuousMechanicsTrackLease) => read_track, b"ReadTrack");
continuous_callback!(receipt_set_track(request: *const NativeContinuousMechanicsTrackSetRequest, result: *mut NativeContinuousMechanicsTrackLease) => set_track, b"SetTrack");
continuous_callback!(receipt_spend_track(request: *const NativeContinuousMechanicsTrackAdjustmentRequest, result: *mut NativeContinuousMechanicsTrackLease) => spend_track, b"SpendTrack");
continuous_callback!(receipt_restore_track(request: *const NativeContinuousMechanicsTrackAdjustmentRequest, result: *mut NativeContinuousMechanicsTrackLease) => restore_track, b"RestoreTrack");
continuous_callback!(receipt_apply_effect(request: *const NativeContinuousMechanicsEffectApplyRequest, result: *mut NativeContinuousMechanicsEffectLease) => apply_effect, b"ApplyEffect");
continuous_callback!(receipt_remove_effect(request: *const NativeContinuousMechanicsEffectRemoveRequest, result: *mut NativeContinuousMechanicsEffectLease) => remove_effect, b"RemoveEffect");

unsafe extern "C" fn destroy_operation_diagnostic_lease(
    context: *mut c_void,
    handle: NativeEngineDiagnosticLeaseHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    i32::from(unsafe {
        (&mut *context.cast::<RuntimeMechanicsBridge>()).destroy_operation_diagnostic_lease(handle)
    })
}

unsafe extern "C" fn create_catalog(
    context: *mut c_void,
    request: *const NativeContinuousMechanicsCatalogCreateRequest,
    result: *mut NativeContinuousMechanicsCatalogHandle,
) -> i32 {
    let Some((bridge, request, result)) = bridge_request_result(context, request, result) else {
        return 0;
    };
    let Ok(definition) = (unsafe { parse_catalog(request) }) else {
        return 0;
    };
    let Ok(catalog) = ContinuousMechanicsCatalog::admit(definition) else {
        return 0;
    };
    let value = bridge.continuous.next_catalog.max(1);
    let Some(next) = value.checked_add(1) else {
        return 0;
    };
    bridge.continuous.next_catalog = next;
    bridge.continuous.catalogs.insert(value, catalog);
    *result = NativeContinuousMechanicsCatalogHandle { value };
    ABI_OK
}

unsafe extern "C" fn destroy_catalog(
    context: *mut c_void,
    handle: NativeContinuousMechanicsCatalogHandle,
) -> i32 {
    if context.is_null() || handle.value == 0 {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeMechanicsBridge>() };
    if !bridge.continuous.catalogs.contains_key(&handle.value) {
        return 0;
    }
    if bridge.prepared_world_imports.values().any(|import| {
        import
            .continuous_stage
            .as_ref()
            .is_some_and(|stage| stage.catalog == handle.value)
    }) {
        return 0;
    }
    // A catalog cannot be removed while it describes any live shared entity.
    if bridge
        .continuous
        .associations
        .iter()
        .any(|(entity, catalog)| {
            *catalog == handle.value && bridge.canonical_entity_is_live(*entity)
        })
    {
        return 0;
    }
    bridge
        .continuous
        .associations
        .retain(|_, catalog| *catalog != handle.value);
    bridge.continuous.catalogs.remove(&handle.value);
    ABI_OK
}

unsafe extern "C" fn read_catalog(
    context: *mut c_void,
    handle: NativeContinuousMechanicsCatalogHandle,
    result: *mut NativeContinuousMechanicsCatalogLease,
) -> i32 {
    if context.is_null() || result.is_null() || handle.value == 0 {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeMechanicsBridge>() };
    let Some(catalog) = bridge.continuous.catalogs.get(&handle.value) else {
        return 0;
    };
    let Some(lease) = catalog_lease(catalog) else {
        return 0;
    };
    let value = bridge.continuous.next_catalog_lease.max(1);
    let Some(next) = value.checked_add(1) else {
        return 0;
    };
    bridge.continuous.next_catalog_lease = next;
    bridge
        .continuous
        .catalog_leases
        .insert(value, Box::new(lease));
    let lease = bridge
        .continuous
        .catalog_leases
        .get(&value)
        .expect("just inserted catalog lease");
    *result = NativeContinuousMechanicsCatalogLease {
        handle: NativeContinuousMechanicsCatalogLeaseHandle { value },
        catalog_id: handle.value,
        version: lease.version,
        fingerprint: lease.fingerprint,
        stats: lease.stats.as_ptr(),
        stats_len: lease.stats.len(),
        tracks: lease.tracks.as_ptr(),
        tracks_len: lease.tracks.len(),
        sources: lease.sources.as_ptr(),
        sources_len: lease.sources.len(),
        contributions: lease.contributions.as_ptr(),
        contributions_len: lease.contributions.len(),
        effects: lease.effects.as_ptr(),
        effects_len: lease.effects.len(),
        effect_sources: lease.effect_sources.as_ptr(),
        effect_sources_len: lease.effect_sources.len(),
    };
    ABI_OK
}

unsafe extern "C" fn destroy_catalog_lease(
    context: *mut c_void,
    handle: NativeContinuousMechanicsCatalogLeaseHandle,
) -> i32 {
    if context.is_null() || handle.value == 0 {
        return 0;
    }
    i32::from(unsafe {
        (&mut *context.cast::<RuntimeMechanicsBridge>())
            .continuous
            .catalog_leases
            .remove(&handle.value)
            .is_some()
    })
}

unsafe extern "C" fn set_initial_components(
    context: *mut c_void,
    request: *const NativeContinuousMechanicsInitialComponentsRequest,
) -> i32 {
    let Some((bridge, request)) = bridge_request(context, request) else {
        return 0;
    };
    let Ok(values) = (unsafe { parse_initial_components(request) }) else {
        return 0;
    };
    let binding = match bridge.binding(request.entity).cloned() {
        Some(value) => value,
        None => return 0,
    };
    if !bridge.entity_matches_continuous_catalog(binding.entity, request.catalog) {
        return 0;
    }
    let Some(catalog) = bridge
        .continuous
        .catalogs
        .get(&request.catalog.value)
        .cloned()
    else {
        return 0;
    };
    let Some(slot) = bridge.catalogs.get_mut(&binding.catalog) else {
        return 0;
    };
    if !slot.world.is_active(binding.entity) {
        return 0;
    }
    // Build every component before replacing the shared state; invalid input has no effect.
    let (stats, tracks, intrinsic, effects) = match components_from_values(&catalog, values) {
        Ok(value) => value,
        Err(_) => return 0,
    };
    let mut candidate = slot.world.state.clone();
    if replace_optional(&mut candidate, binding.entity, stats).is_err()
        || replace_optional(&mut candidate, binding.entity, tracks).is_err()
        || replace_optional(&mut candidate, binding.entity, intrinsic).is_err()
        || replace_optional(&mut candidate, binding.entity, effects).is_err()
    {
        return 0;
    }
    slot.world.state = candidate;
    bridge
        .continuous
        .associations
        .insert(binding.entity, request.catalog.value);
    ABI_OK
}

unsafe extern "C" fn read_components(
    context: *mut c_void,
    request: *const NativeContinuousMechanicsComponentReadRequest,
    result: *mut NativeContinuousMechanicsComponentLease,
) -> i32 {
    let Some((bridge, request, result)) = bridge_request_result(context, request, result) else {
        return 0;
    };
    let Some((catalog_id, entity_id, state, continuous_catalog)) =
        resolve_read(bridge, request.catalog, request.entity)
    else {
        return 0;
    };
    let Some(lease) = component_lease(catalog_id, entity_id, state, continuous_catalog) else {
        return 0;
    };
    let value = bridge.continuous.next_component_lease.max(1);
    let Some(next) = value.checked_add(1) else {
        return 0;
    };
    bridge.continuous.next_component_lease = next;
    bridge
        .continuous
        .component_leases
        .insert(value, Box::new(lease));
    let lease = bridge
        .continuous
        .component_leases
        .get(&value)
        .expect("just inserted component lease");
    *result = NativeContinuousMechanicsComponentLease {
        handle: NativeContinuousMechanicsComponentLeaseHandle { value },
        catalog_id,
        catalog_version: lease.catalog_version,
        catalog_fingerprint: lease.catalog_fingerprint,
        entity_id: entity_id.raw(),
        components: lease.components.as_ptr(),
        components_len: lease.components.len(),
        stats: lease.stats.as_ptr(),
        stats_len: lease.stats.len(),
        tracks: lease.tracks.as_ptr(),
        tracks_len: lease.tracks.len(),
        intrinsic_sources: lease.intrinsic_sources.as_ptr(),
        intrinsic_sources_len: lease.intrinsic_sources.len(),
        active_effects: lease.active_effects.as_ptr(),
        active_effects_len: lease.active_effects.len(),
    };
    ABI_OK
}

unsafe extern "C" fn destroy_component_lease(
    context: *mut c_void,
    handle: NativeContinuousMechanicsComponentLeaseHandle,
) -> i32 {
    if context.is_null() || handle.value == 0 {
        return 0;
    }
    i32::from(unsafe {
        (&mut *context.cast::<RuntimeMechanicsBridge>())
            .continuous
            .component_leases
            .remove(&handle.value)
            .is_some()
    })
}

unsafe extern "C" fn export_world(
    context: *mut c_void,
    request: *const NativeContinuousMechanicsWorldExportRequest,
    result: *mut NativeContinuousMechanicsWorldExportLease,
) -> i32 {
    let Some((bridge, request, result)) = bridge_request_result(context, request, result) else {
        return 0;
    };
    let mechanics_catalog = request.mechanics_catalog;
    let catalog = request.continuous_catalog;
    if mechanics_catalog.value == 0 || catalog.value == 0 {
        return 0;
    }
    if bridge
        .entities
        .values()
        .any(|binding| binding.catalog == mechanics_catalog.value && !binding.committed)
    {
        return 0;
    }
    let Some(slot) = bridge.catalogs.get(&mechanics_catalog.value) else {
        return 0;
    };
    if slot.catalog.is_none() {
        return 0;
    }
    let Some(continuous_catalog) = bridge.continuous.catalogs.get(&catalog.value) else {
        return 0;
    };
    let mut text = TextPool::default();
    let catalog_version = text.copy(continuous_catalog.version().as_str());
    let catalog_fingerprint = text.copy(continuous_catalog.fingerprint());
    let mut component_presence = Vec::new();
    let mut stats = Vec::new();
    let mut tracks = Vec::new();
    let mut intrinsic_sources = Vec::new();
    let mut active_effects = Vec::new();
    for (&entity, _) in &slot.world.lifecycle {
        let lifecycle = slot.world.native_lifecycle(entity);
        let association = bridge.continuous.associations.get(&entity).copied();
        if association.is_some_and(|value| value != catalog.value) {
            return 0;
        }
        let present = NativeContinuousMechanicsComponentKind::all().map(|component| {
            (
                component,
                component_present(&slot.world.state, entity, component),
            )
        });
        if lifecycle == NativeMechanicsEntityLifecycle::Tombstoned
            && (association.is_some() || present.into_iter().any(|(_, present)| present))
        {
            return 0;
        }
        if association.is_none() && present.into_iter().any(|(_, present)| present) {
            return 0;
        }
        for (component, present) in present {
            component_presence.push(NativeContinuousMechanicsWorldComponentPresenceRow {
                entity_id: entity.raw(),
                component,
                present,
                revision: component_revision(&slot.world.state, entity, component),
            });
        }
        if lifecycle == NativeMechanicsEntityLifecycle::Tombstoned {
            continue;
        }
        if let Ok(Some(component)) = slot
            .world
            .state
            .component::<ContinuousStatsComponent>(entity)
        {
            stats.extend(component.values().iter().map(|row| {
                NativeContinuousMechanicsWorldStatRow {
                    entity_id: entity.raw(),
                    stat: text.copy(row.stat().as_str()),
                    base_bits: row.base().bits(),
                }
            }));
        }
        if let Ok(Some(component)) = slot
            .world
            .state
            .component::<ContinuousTracksComponent>(entity)
        {
            tracks.extend(component.values().iter().map(|row| {
                NativeContinuousMechanicsWorldTrackRow {
                    entity_id: entity.raw(),
                    track: text.copy(row.track().as_str()),
                    current_bits: row.current().bits(),
                }
            }));
        }
        if let Ok(Some(component)) = slot
            .world
            .state
            .component::<ContinuousIntrinsicSourcesComponent>(entity)
        {
            intrinsic_sources.extend(component.bindings().iter().map(|row| {
                NativeContinuousMechanicsWorldIntrinsicSourceRow {
                    entity_id: entity.raw(),
                    instance: text.copy(row.instance().as_str()),
                    definition: text.copy(row.definition().as_str()),
                }
            }));
        }
        if let Ok(Some(component)) = slot
            .world
            .state
            .component::<ContinuousActiveEffectsComponent>(entity)
        {
            active_effects.extend(component.effects().iter().map(|row| {
                NativeContinuousMechanicsWorldActiveEffectRow {
                    entity_id: entity.raw(),
                    instance: text.copy(row.instance().as_str()),
                    definition: text.copy(row.definition().as_str()),
                }
            }));
        }
    }
    let value = bridge.continuous.next_world_export_lease.max(1);
    let Some(next) = value.checked_add(1) else {
        return 0;
    };
    bridge.continuous.next_world_export_lease = next;
    let lease = Box::new(ContinuousWorldExportLease {
        _text: text,
        catalog_version,
        catalog_fingerprint,
        component_presence,
        stats,
        tracks,
        intrinsic_sources,
        active_effects,
    });
    *result = NativeContinuousMechanicsWorldExportLease {
        handle: NativeContinuousMechanicsWorldExportLeaseHandle { value },
        mechanics_catalog_id: mechanics_catalog.value,
        mechanics_state_revision: slot.world.state.revision(),
        continuous_catalog_id: catalog.value,
        continuous_catalog_version: lease.catalog_version,
        continuous_catalog_fingerprint: lease.catalog_fingerprint,
        component_presence: lease.component_presence.as_ptr(),
        component_presence_len: lease.component_presence.len(),
        stats: lease.stats.as_ptr(),
        stats_len: lease.stats.len(),
        tracks: lease.tracks.as_ptr(),
        tracks_len: lease.tracks.len(),
        intrinsic_sources: lease.intrinsic_sources.as_ptr(),
        intrinsic_sources_len: lease.intrinsic_sources.len(),
        active_effects: lease.active_effects.as_ptr(),
        active_effects_len: lease.active_effects.len(),
    };
    bridge.continuous.world_export_leases.insert(value, lease);
    ABI_OK
}

unsafe extern "C" fn destroy_world_export_lease(
    context: *mut c_void,
    handle: NativeContinuousMechanicsWorldExportLeaseHandle,
) -> i32 {
    if context.is_null() || handle.value == 0 {
        return 0;
    }
    i32::from(unsafe {
        (&mut *context.cast::<RuntimeMechanicsBridge>())
            .continuous
            .world_export_leases
            .remove(&handle.value)
            .is_some()
    })
}

unsafe extern "C" fn stage_world_import(
    context: *mut c_void,
    request: *const NativeContinuousMechanicsWorldImportStageRequest,
    result: *mut NativeContinuousMechanicsWorldImportLease,
) -> i32 {
    let Some((bridge, request, result)) = bridge_request_result(context, request, result) else {
        return 0;
    };
    if request.import.value == 0
        || request.mechanics_catalog.value == 0
        || request.continuous_catalog.value == 0
    {
        return 0;
    }
    let Some(continuous_catalog) = bridge
        .continuous
        .catalogs
        .get(&request.continuous_catalog.value)
        .cloned()
    else {
        return 0;
    };
    if unsafe { text(request.continuous_catalog_version) }
        != Ok(continuous_catalog.version().as_str())
        || unsafe { text(request.continuous_catalog_fingerprint) }
            != Ok(continuous_catalog.fingerprint())
    {
        return 0;
    }
    let Some(import) = bridge.prepared_world_imports.get(&request.import.value) else {
        return 0;
    };
    if import.published
        || import.catalog != request.mechanics_catalog.value
        || import.saved_state_revision != request.mechanics_state_revision
        || import.continuous_stage.is_some()
    {
        return 0;
    }
    let Some(candidate) = import.candidate.as_ref().cloned() else {
        return 0;
    };
    let membership = import
        .entities
        .iter()
        .map(|row| EntityId::new(row.entity_id))
        .collect::<Vec<_>>();
    let lifecycles = import.lifecycles.clone();
    let Some(current) = bridge
        .catalogs
        .get(&import.catalog)
        .map(|slot| slot.world.state.clone())
    else {
        return 0;
    };
    let Ok(rows) = (unsafe { parse_world_stage_rows(request, &membership, &lifecycles) }) else {
        return 0;
    };
    let Ok((candidate, stage)) = build_world_stage(
        candidate,
        &current,
        &continuous_catalog,
        request.continuous_catalog.value,
        request.mechanics_state_revision,
        rows,
    ) else {
        return 0;
    };
    let Some(import) = bridge.prepared_world_imports.get_mut(&request.import.value) else {
        return 0;
    };
    let Some(exact_revisions) =
        refresh_exact_revisions(&import.revisions, &current, &candidate.state)
    else {
        return 0;
    };
    let value = bridge.continuous.next_world_import_lease.max(1);
    let Some(next) = value.checked_add(1) else {
        return 0;
    };
    bridge.continuous.next_world_import_lease = next;
    *result = NativeContinuousMechanicsWorldImportLease {
        handle: NativeContinuousMechanicsWorldImportLeaseHandle { value },
        mechanics_catalog_id: import.catalog,
        mechanics_state_revision_before: import.state_revision_before,
        mechanics_state_revision_after: candidate.state.revision(),
        continuous_catalog_id: stage.catalog,
        continuous_catalog_version: stage.catalog_version,
        continuous_catalog_fingerprint: stage.catalog_fingerprint,
        revisions: stage.revisions.as_ptr(),
        revisions_len: stage.revisions.len(),
    };
    import.candidate = Some(candidate);
    import.revisions = exact_revisions;
    import.continuous_stage = Some(stage);
    bridge
        .continuous
        .world_import_leases
        .insert(value, request.import.value);
    ABI_OK
}

unsafe extern "C" fn destroy_world_import_lease(
    context: *mut c_void,
    handle: NativeContinuousMechanicsWorldImportLeaseHandle,
) -> i32 {
    if context.is_null() || handle.value == 0 {
        return 0;
    }
    i32::from(unsafe {
        (&mut *context.cast::<RuntimeMechanicsBridge>())
            .continuous
            .world_import_leases
            .remove(&handle.value)
            .is_some()
    })
}

struct ParsedWorldStageRows {
    presence: BTreeMap<
        (EntityId, NativeContinuousMechanicsComponentKind),
        NativeContinuousMechanicsWorldComponentPresenceRow,
    >,
    stats: Vec<NativeContinuousMechanicsWorldStatRow>,
    tracks: Vec<NativeContinuousMechanicsWorldTrackRow>,
    intrinsic_sources: Vec<NativeContinuousMechanicsWorldIntrinsicSourceRow>,
    active_effects: Vec<NativeContinuousMechanicsWorldActiveEffectRow>,
}

unsafe fn parse_world_stage_rows(
    request: &NativeContinuousMechanicsWorldImportStageRequest,
    membership: &[EntityId],
    lifecycles: &[NativeMechanicsLifecycleReceipt],
) -> Result<ParsedWorldStageRows, ()> {
    let presence_rows = unsafe {
        borrowed_slice(
            request.component_presence,
            request.component_presence_len,
            "continuous world component presence",
        )
    }
    .map_err(|_| ())?;
    let stats =
        unsafe { borrowed_slice(request.stats, request.stats_len, "continuous world stats") }
            .map_err(|_| ())?;
    let tracks = unsafe {
        borrowed_slice(
            request.tracks,
            request.tracks_len,
            "continuous world tracks",
        )
    }
    .map_err(|_| ())?;
    let intrinsic_sources = unsafe {
        borrowed_slice(
            request.intrinsic_sources,
            request.intrinsic_sources_len,
            "continuous world intrinsic sources",
        )
    }
    .map_err(|_| ())?;
    let active_effects = unsafe {
        borrowed_slice(
            request.active_effects,
            request.active_effects_len,
            "continuous world active effects",
        )
    }
    .map_err(|_| ())?;
    let members = membership
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    if members.len() != membership.len() || lifecycles.len() != membership.len() {
        return Err(());
    }
    let lifecycle = lifecycles
        .iter()
        .map(|row| (EntityId::new(row.entity_id), row.lifecycle))
        .collect::<BTreeMap<_, _>>();
    if lifecycle.len() != members.len() || lifecycle.keys().any(|entity| !members.contains(entity))
    {
        return Err(());
    }
    let mut presence = BTreeMap::new();
    for row in presence_rows {
        let entity = EntityId::new(row.entity_id);
        if row.entity_id == 0
            || !members.contains(&entity)
            || presence.insert((entity, row.component), *row).is_some()
        {
            return Err(());
        }
    }
    if presence.len() != members.len() * NativeContinuousMechanicsComponentKind::all().len()
        || members.iter().any(|entity| {
            NativeContinuousMechanicsComponentKind::all()
                .into_iter()
                .any(|component| !presence.contains_key(&(*entity, component)))
        })
    {
        return Err(());
    }
    let typed_ok = |entity_id: u64, component| {
        entity_id != 0
            && presence
                .get(&(EntityId::new(entity_id), component))
                .is_some_and(|row| row.present)
    };
    if stats
        .iter()
        .any(|row| !typed_ok(row.entity_id, NativeContinuousMechanicsComponentKind::Stats))
        || tracks.iter().any(|row| {
            !typed_ok(
                row.entity_id,
                NativeContinuousMechanicsComponentKind::Tracks,
            )
        })
        || intrinsic_sources.iter().any(|row| {
            !typed_ok(
                row.entity_id,
                NativeContinuousMechanicsComponentKind::IntrinsicSources,
            )
        })
        || active_effects.iter().any(|row| {
            !typed_ok(
                row.entity_id,
                NativeContinuousMechanicsComponentKind::ActiveEffects,
            )
        })
    {
        return Err(());
    }
    for entity in &members {
        if lifecycle[entity] == NativeMechanicsEntityLifecycle::Tombstoned
            && NativeContinuousMechanicsComponentKind::all()
                .into_iter()
                .any(|component| presence[&(*entity, component)].present)
        {
            return Err(());
        }
    }
    Ok(ParsedWorldStageRows {
        presence,
        stats: stats.to_vec(),
        tracks: tracks.to_vec(),
        intrinsic_sources: intrinsic_sources.to_vec(),
        active_effects: active_effects.to_vec(),
    })
}

fn build_world_stage(
    mut candidate: super::MechanicsWorld,
    current: &EntityState,
    catalog: &ContinuousMechanicsCatalog,
    catalog_id: u64,
    saved_state_revision: u64,
    rows: ParsedWorldStageRows,
) -> Result<
    (
        super::MechanicsWorld,
        PreparedContinuousMechanicsWorldImportStage,
    ),
    (),
> {
    let entities = rows
        .presence
        .keys()
        .map(|(entity, _)| *entity)
        .collect::<std::collections::BTreeSet<_>>();
    let mut associations = BTreeMap::new();
    for entity in entities.iter().copied() {
        let stats = rows.presence[&(entity, NativeContinuousMechanicsComponentKind::Stats)]
            .present
            .then(|| {
                rows.stats
                    .iter()
                    .filter(|row| row.entity_id == entity.raw())
                    .map(|row| {
                        Ok(ContinuousStatValue::new(
                            parse_stat(unsafe { text(row.stat) }?)?,
                            ContinuousValue::from_bits(row.base_bits).map_err(|_| ())?,
                        ))
                    })
                    .collect::<Result<Vec<_>, ()>>()
            })
            .transpose()?;
        let tracks = rows.presence[&(entity, NativeContinuousMechanicsComponentKind::Tracks)]
            .present
            .then(|| {
                rows.tracks
                    .iter()
                    .filter(|row| row.entity_id == entity.raw())
                    .map(|row| {
                        Ok(ContinuousTrackValue::new(
                            parse_track(unsafe { text(row.track) }?)?,
                            ContinuousValue::from_bits(row.current_bits).map_err(|_| ())?,
                        ))
                    })
                    .collect::<Result<Vec<_>, ()>>()
            })
            .transpose()?;
        let intrinsic = rows.presence[&(
            entity,
            NativeContinuousMechanicsComponentKind::IntrinsicSources,
        )]
            .present
            .then(|| {
                rows.intrinsic_sources
                    .iter()
                    .filter(|row| row.entity_id == entity.raw())
                    .map(|row| {
                        Ok(ContinuousIntrinsicSourceBinding::new(
                            parse_source_instance(unsafe { text(row.instance) }?)?,
                            parse_source(unsafe { text(row.definition) }?)?,
                        ))
                    })
                    .collect::<Result<Vec<_>, ()>>()
            })
            .transpose()?;
        let effects = rows.presence[&(
            entity,
            NativeContinuousMechanicsComponentKind::ActiveEffects,
        )]
            .present
            .then(|| {
                rows.active_effects
                    .iter()
                    .filter(|row| row.entity_id == entity.raw())
                    .map(|row| {
                        Ok(ContinuousActiveEffectInstance::new(
                            parse_effect_instance(unsafe { text(row.instance) }?)?,
                            parse_effect(unsafe { text(row.definition) }?)?,
                        ))
                    })
                    .collect::<Result<Vec<_>, ()>>()
            })
            .transpose()?;
        let (stats, tracks, intrinsic, effects) = components_from_values(
            catalog,
            InitialComponents {
                stats,
                tracks,
                intrinsic,
                effects,
            },
        )?;
        replace_optional(&mut candidate.state, entity, stats)?;
        replace_optional(&mut candidate.state, entity, tracks)?;
        replace_optional(&mut candidate.state, entity, intrinsic)?;
        replace_optional(&mut candidate.state, entity, effects)?;
        if NativeContinuousMechanicsComponentKind::all()
            .into_iter()
            .any(|component| rows.presence[&(entity, component)].present)
        {
            associations.insert(entity, catalog_id);
        }
    }
    validate_state_against_continuous_catalog(&candidate.state, catalog).map_err(|_| ())?;
    let mut floors = BTreeMap::new();
    for ((entity, component), row) in &rows.presence {
        let type_id = continuous_component_type(&candidate.state, *component)?;
        floors.insert((*entity, type_id), row.revision);
    }
    if !candidate
        .state
        .rebase_replacement_revisions_after(current, saved_state_revision, &floors)
    {
        return Err(());
    }
    let revisions = continuous_revision_rows(
        &rows.presence,
        current,
        &candidate.state,
        entities.iter().copied(),
    )?;
    let mut text = TextPool::default();
    let catalog_version = text.copy(catalog.version().as_str());
    let catalog_fingerprint = text.copy(catalog.fingerprint());
    Ok((
        candidate,
        PreparedContinuousMechanicsWorldImportStage {
            catalog: catalog_id,
            catalog_version,
            catalog_fingerprint,
            associations,
            revisions,
            _text: text,
        },
    ))
}

fn continuous_component_type(
    state: &EntityState,
    component: NativeContinuousMechanicsComponentKind,
) -> Result<entity_state::ComponentTypeId, ()> {
    match component {
        NativeContinuousMechanicsComponentKind::Stats => state
            .component_type_id::<ContinuousStatsComponent>()
            .map_err(|_| ())
            .cloned(),
        NativeContinuousMechanicsComponentKind::Tracks => state
            .component_type_id::<ContinuousTracksComponent>()
            .map_err(|_| ())
            .cloned(),
        NativeContinuousMechanicsComponentKind::IntrinsicSources => state
            .component_type_id::<ContinuousIntrinsicSourcesComponent>()
            .map_err(|_| ())
            .cloned(),
        NativeContinuousMechanicsComponentKind::ActiveEffects => state
            .component_type_id::<ContinuousActiveEffectsComponent>()
            .map_err(|_| ())
            .cloned(),
    }
}

fn continuous_revision_rows(
    saved: &BTreeMap<
        (EntityId, NativeContinuousMechanicsComponentKind),
        NativeContinuousMechanicsWorldComponentPresenceRow,
    >,
    current: &EntityState,
    candidate: &EntityState,
    entities: impl IntoIterator<Item = EntityId>,
) -> Result<Vec<NativeContinuousMechanicsRevisionRemapRow>, ()> {
    let mut rows = Vec::new();
    for entity in entities {
        rows.push(continuous_revision_row::<ContinuousStatsComponent>(
            saved,
            current,
            candidate,
            entity,
            NativeContinuousMechanicsComponentKind::Stats,
        )?);
        rows.push(continuous_revision_row::<ContinuousTracksComponent>(
            saved,
            current,
            candidate,
            entity,
            NativeContinuousMechanicsComponentKind::Tracks,
        )?);
        rows.push(continuous_revision_row::<
            ContinuousIntrinsicSourcesComponent,
        >(
            saved,
            current,
            candidate,
            entity,
            NativeContinuousMechanicsComponentKind::IntrinsicSources,
        )?);
        rows.push(continuous_revision_row::<ContinuousActiveEffectsComponent>(
            saved,
            current,
            candidate,
            entity,
            NativeContinuousMechanicsComponentKind::ActiveEffects,
        )?);
    }
    Ok(rows)
}

fn continuous_revision_row<T: EntityComponent>(
    saved: &BTreeMap<
        (EntityId, NativeContinuousMechanicsComponentKind),
        NativeContinuousMechanicsWorldComponentPresenceRow,
    >,
    current: &EntityState,
    candidate: &EntityState,
    entity: EntityId,
    component: NativeContinuousMechanicsComponentKind,
) -> Result<NativeContinuousMechanicsRevisionRemapRow, ()> {
    let saved = saved.get(&(entity, component)).ok_or(())?;
    Ok(NativeContinuousMechanicsRevisionRemapRow {
        entity_id: entity.raw(),
        component,
        present: candidate.has_component::<T>(entity).map_err(|_| ())?,
        snapshot_revision: saved.revision,
        current_revision: current
            .component_revision::<T>(entity)
            .map_err(|_| ())?
            .revision(),
        restored_revision: candidate
            .component_revision::<T>(entity)
            .map_err(|_| ())?
            .revision(),
    })
}

fn refresh_exact_revisions(
    previous: &[NativeMechanicsRevisionRemapRow],
    current: &EntityState,
    candidate: &EntityState,
) -> Option<Vec<NativeMechanicsRevisionRemapRow>> {
    let mut saved = BTreeMap::new();
    let mut entities = std::collections::BTreeSet::new();
    for row in previous {
        let entity = EntityId::new(row.entity_id);
        saved.insert(
            (entity, row.component),
            NativeMechanicsWorldComponentPresenceRow {
                entity_id: row.entity_id,
                component: row.component,
                present: row.present,
                revision: row.snapshot_revision,
            },
        );
        entities.insert(entity);
    }
    super::import_revision_rows(&saved, current, candidate, entities)
}

unsafe extern "C" fn evaluate_stat(
    context: *mut c_void,
    request: *const NativeContinuousMechanicsStatEvaluateRequest,
    result: *mut NativeContinuousMechanicsStatEvaluationLease,
) -> i32 {
    let Some((bridge, request, result)) = bridge_request_result(context, request, result) else {
        return 0;
    };
    let Ok(stat) = text(request.stat).and_then(parse_stat) else {
        return 0;
    };
    let (catalog_id, entity, receipt) = {
        let Some((catalog_id, entity, state, catalog)) =
            resolve_read(bridge, request.catalog, request.entity)
        else {
            return 0;
        };
        let Ok(receipt) = ContinuousStatService::evaluate(state, catalog, entity, &stat) else {
            return 0;
        };
        (catalog_id, entity, receipt)
    };
    let base_bits = receipt.base.bits();
    let after_additions_bits = receipt.after_additions.bits();
    let unconstrained_bits = receipt.unconstrained.bits();
    let minimum_bits = receipt.minimum.bits();
    let maximum_bits = receipt.maximum.bits();
    let value_bits = receipt.value.bits();
    let observed_revisions = NativeContinuousMechanicsObservedRevision {
        has_stats: true,
        stats: receipt.observed_stats_revision,
        has_intrinsic_sources: receipt.observed_sources_revision.is_some(),
        intrinsic_sources: receipt.observed_sources_revision.unwrap_or_default(),
        has_active_effects: receipt.observed_effects_revision.is_some(),
        active_effects: receipt.observed_effects_revision.unwrap_or_default(),
    };
    let Some(value) = insert_stat_evaluation(bridge, catalog_id, receipt) else {
        return 0;
    };
    let OperationLease::StatEvaluation {
        decisions,
        catalog_version,
        catalog_fingerprint,
        stat,
        ..
    } = bridge
        .continuous
        .operation_leases
        .get(&value)
        .expect("just inserted operation lease")
        .as_ref()
    else {
        unreachable!()
    };
    *result = NativeContinuousMechanicsStatEvaluationLease {
        handle: NativeContinuousMechanicsOperationLeaseHandle { value },
        decisions: decisions.as_ptr(),
        decisions_len: decisions.len(),
        catalog_id,
        catalog_version: *catalog_version,
        catalog_fingerprint: *catalog_fingerprint,
        entity_id: entity.raw(),
        stat: *stat,
        base_bits,
        after_additions_bits,
        unconstrained_bits,
        minimum_bits,
        maximum_bits,
        value_bits,
        observed_revisions,
    };
    ABI_OK
}

unsafe extern "C" fn set_stat_base(
    context: *mut c_void,
    request: *const NativeContinuousMechanicsStatBaseMutationRequest,
    result: *mut NativeContinuousMechanicsStatMutationLease,
) -> i32 {
    let Some((bridge, request, result)) = bridge_request_result(context, request, result) else {
        return 0;
    };
    let (Ok(operation), Ok(stat), Ok(base)) = (
        text(request.operation).and_then(parse_operation),
        text(request.stat).and_then(parse_stat),
        ContinuousValue::from_bits(request.base_bits).map_err(|_| ()),
    ) else {
        return 0;
    };
    let Some((catalog_id, entity, state, catalog)) =
        resolve_mut(bridge, request.catalog, request.entity)
    else {
        return 0;
    };
    let expected = guarded_revision::<ContinuousStatsComponent>(
        state,
        entity,
        request.revision_guard,
        request.expected_revision,
        ContinuousMechanicsComponentKind::Stats,
    );
    let Some(expected) = expected else {
        return 0;
    };
    let Ok(receipt) = ContinuousStatService::set_base(
        state,
        catalog,
        ContinuousStatBaseMutationRequest {
            operation,
            entity,
            stat,
            base,
            expected_revision: expected,
        },
    ) else {
        return 0;
    };
    let Some(value) = insert_stat_mutation(bridge, catalog_id, &receipt) else {
        return 0;
    };
    let OperationLease::StatMutation {
        catalog_version,
        catalog_fingerprint,
        operation,
        stat,
        ..
    } = bridge
        .continuous
        .operation_leases
        .get(&value)
        .expect("just inserted operation lease")
        .as_ref()
    else {
        unreachable!()
    };
    *result = NativeContinuousMechanicsStatMutationLease {
        handle: NativeContinuousMechanicsOperationLeaseHandle { value },
        catalog_id,
        catalog_version: *catalog_version,
        catalog_fingerprint: *catalog_fingerprint,
        operation: *operation,
        entity_id: entity.raw(),
        stat: *stat,
        before_bits: receipt.before.bits(),
        after_bits: receipt.after.bits(),
        minimum_bits: receipt.minimum.bits(),
        maximum_bits: receipt.maximum.bits(),
        observed_revision: receipt.observed_revision,
        committed_revision: receipt.committed_revision,
    };
    ABI_OK
}

unsafe extern "C" fn read_track(
    context: *mut c_void,
    request: *const NativeContinuousMechanicsTrackReadRequest,
    result: *mut NativeContinuousMechanicsTrackLease,
) -> i32 {
    track_operation(context, request, result, TrackOp::Read)
}
unsafe extern "C" fn set_track(
    context: *mut c_void,
    request: *const NativeContinuousMechanicsTrackSetRequest,
    result: *mut NativeContinuousMechanicsTrackLease,
) -> i32 {
    track_operation(context, request, result, TrackOp::Set)
}
unsafe extern "C" fn spend_track(
    context: *mut c_void,
    request: *const NativeContinuousMechanicsTrackAdjustmentRequest,
    result: *mut NativeContinuousMechanicsTrackLease,
) -> i32 {
    track_operation(context, request, result, TrackOp::Spend)
}
unsafe extern "C" fn restore_track(
    context: *mut c_void,
    request: *const NativeContinuousMechanicsTrackAdjustmentRequest,
    result: *mut NativeContinuousMechanicsTrackLease,
) -> i32 {
    track_operation(context, request, result, TrackOp::Restore)
}

#[derive(Clone, Copy)]
enum TrackOp {
    Read,
    Set,
    Spend,
    Restore,
}

unsafe fn track_operation<R>(
    context: *mut c_void,
    request: *const R,
    result: *mut NativeContinuousMechanicsTrackLease,
    kind: TrackOp,
) -> i32
where
    R: TrackRequest,
{
    let Some((bridge, request, result)) = bridge_request_result(context, request, result) else {
        return 0;
    };
    let request = request.track_request();
    let Ok(track) = text(request.track).and_then(parse_track) else {
        return 0;
    };
    let Some((catalog_id, entity, state, catalog)) = (if matches!(kind, TrackOp::Read) {
        resolve_read_mut(bridge, request.catalog, request.entity)
    } else {
        resolve_mut(bridge, request.catalog, request.entity)
    }) else {
        return 0;
    };
    let catalog_version_value = catalog.version().as_str().to_owned();
    let catalog_fingerprint_value = catalog.fingerprint().to_owned();
    let (
        operation,
        before,
        after,
        minimum,
        maximum,
        adjustment_kind,
        requested_amount,
        applied_amount,
        observed_tracks_revision,
        observed_stats_revision,
        observed_sources_revision,
        observed_effects_revision,
        committed_tracks_revision,
    ) = match kind {
        TrackOp::Read => {
            let Ok((minimum, maximum)) = gameplay_continuous_mechanics::continuous_track_bounds(
                state, catalog, entity, &track,
            ) else {
                return 0;
            };
            let Ok(component) = state.component::<ContinuousTracksComponent>(entity) else {
                return 0;
            };
            let Some(component) = component else {
                return 0;
            };
            let Some(current) = component.current(&track) else {
                return 0;
            };
            let observed = state
                .component_revision::<ContinuousTracksComponent>(entity)
                .ok()
                .map(|r| r.revision())
                .unwrap_or_default();
            (
                String::new(),
                current,
                current,
                minimum,
                maximum,
                NativeContinuousMechanicsTrackAdjustmentKind::Spend,
                ContinuousValue::new(0.0).expect("zero"),
                ContinuousValue::new(0.0).expect("zero"),
                observed,
                None,
                None,
                None,
                observed,
            )
        }
        TrackOp::Set => {
            let (Ok(operation), Ok(value)) = (
                text(request.operation).and_then(parse_operation),
                ContinuousValue::from_bits(request.value_bits).map_err(|_| ()),
            ) else {
                return 0;
            };
            let Some(expected) = guarded_revision::<ContinuousTracksComponent>(
                state,
                entity,
                request.revision_guard,
                request.expected_revision,
                ContinuousMechanicsComponentKind::Tracks,
            ) else {
                return 0;
            };
            let policy = match request.policy {
                NativeContinuousMechanicsTrackSetPolicy::RejectOutOfBounds => {
                    ContinuousTrackSetPolicy::RejectOutOfBounds
                }
                NativeContinuousMechanicsTrackSetPolicy::ClampToBounds => {
                    ContinuousTrackSetPolicy::ClampToBounds
                }
            };
            let Ok(receipt) = ContinuousTrackService::set(
                state,
                catalog,
                ContinuousTrackSetRequest {
                    operation: operation.clone(),
                    entity,
                    track: track.clone(),
                    value,
                    policy,
                    expected_revision: expected,
                },
            ) else {
                return 0;
            };
            (
                operation.to_string(),
                receipt.before,
                receipt.after,
                receipt.minimum,
                receipt.maximum,
                NativeContinuousMechanicsTrackAdjustmentKind::Spend,
                ContinuousValue::new(0.0).expect("zero"),
                ContinuousValue::new(0.0).expect("zero"),
                receipt.observed_revision,
                receipt.observed_stats_revision,
                receipt.observed_sources_revision,
                receipt.observed_effects_revision,
                receipt.committed_revision,
            )
        }
        TrackOp::Spend | TrackOp::Restore => {
            let (Ok(operation), Ok(amount)) = (
                text(request.operation).and_then(parse_operation),
                ContinuousValue::from_bits(request.value_bits).map_err(|_| ()),
            ) else {
                return 0;
            };
            let Some(expected) = guarded_revision::<ContinuousTracksComponent>(
                state,
                entity,
                request.revision_guard,
                request.expected_revision,
                ContinuousMechanicsComponentKind::Tracks,
            ) else {
                return 0;
            };
            let adjustment = if matches!(kind, TrackOp::Spend) {
                ContinuousTrackAdjustmentKind::Spend
            } else {
                ContinuousTrackAdjustmentKind::Restore
            };
            let receipt = ContinuousTrackService::adjust(
                state,
                catalog,
                ContinuousTrackAdjustmentRequest {
                    operation: operation.clone(),
                    entity,
                    track: track.clone(),
                    amount,
                    kind: adjustment,
                    expected_revision: expected,
                },
            );
            let Ok(receipt) = receipt else {
                return 0;
            };
            (
                operation.to_string(),
                receipt.before,
                receipt.after,
                receipt.minimum,
                receipt.maximum,
                if matches!(adjustment, ContinuousTrackAdjustmentKind::Spend) {
                    NativeContinuousMechanicsTrackAdjustmentKind::Spend
                } else {
                    NativeContinuousMechanicsTrackAdjustmentKind::Restore
                },
                receipt.requested_amount,
                receipt.applied_amount,
                receipt.observed_tracks_revision,
                receipt.observed_stats_revision,
                receipt.observed_sources_revision,
                receipt.observed_effects_revision,
                receipt.committed_tracks_revision,
            )
        }
    };
    let Some(value) = insert_track(
        bridge,
        catalog_id,
        &catalog_version_value,
        &catalog_fingerprint_value,
        entity,
        &operation,
        &track,
        kind,
        before,
        after,
        minimum,
        maximum,
        adjustment_kind,
        requested_amount,
        applied_amount,
        observed_tracks_revision,
        observed_stats_revision,
        observed_sources_revision,
        observed_effects_revision,
        committed_tracks_revision,
    ) else {
        return 0;
    };
    let OperationLease::Track {
        catalog_version,
        catalog_fingerprint,
        operation,
        track,
        ..
    } = bridge
        .continuous
        .operation_leases
        .get(&value)
        .expect("just inserted operation lease")
        .as_ref()
    else {
        unreachable!()
    };
    *result = NativeContinuousMechanicsTrackLease {
        handle: NativeContinuousMechanicsOperationLeaseHandle { value },
        catalog_id,
        catalog_version: *catalog_version,
        catalog_fingerprint: *catalog_fingerprint,
        operation: *operation,
        entity_id: entity.raw(),
        track: *track,
        requested_amount_bits: requested_amount.bits(),
        applied_amount_bits: applied_amount.bits(),
        before_bits: before.bits(),
        after_bits: after.bits(),
        minimum_bits: minimum.bits(),
        maximum_bits: maximum.bits(),
        has_adjustment: matches!(kind, TrackOp::Spend | TrackOp::Restore),
        adjustment_kind,
        observed_tracks_revision,
        has_observed_stats_revision: observed_stats_revision.is_some(),
        observed_stats_revision: observed_stats_revision.unwrap_or_default(),
        has_observed_intrinsic_sources_revision: observed_sources_revision.is_some(),
        observed_intrinsic_sources_revision: observed_sources_revision.unwrap_or_default(),
        has_observed_active_effects_revision: observed_effects_revision.is_some(),
        observed_active_effects_revision: observed_effects_revision.unwrap_or_default(),
        committed_tracks_revision,
    };
    ABI_OK
}

trait TrackRequest {
    fn track_request(&self) -> TrackRequestView;
}
struct TrackRequestView {
    catalog: NativeContinuousMechanicsCatalogHandle,
    entity: NativeMechanicsEntityHandle,
    track: NativeUtf8Slice,
    operation: NativeUtf8Slice,
    value_bits: u64,
    revision_guard: NativeContinuousMechanicsRevisionGuard,
    expected_revision: u64,
    policy: NativeContinuousMechanicsTrackSetPolicy,
}
impl TrackRequest for NativeContinuousMechanicsTrackReadRequest {
    fn track_request(&self) -> TrackRequestView {
        TrackRequestView {
            catalog: self.catalog,
            entity: self.entity,
            track: self.track,
            operation: NativeUtf8Slice::default(),
            value_bits: 0,
            revision_guard: NativeContinuousMechanicsRevisionGuard::Unchecked,
            expected_revision: 0,
            policy: NativeContinuousMechanicsTrackSetPolicy::RejectOutOfBounds,
        }
    }
}
impl TrackRequest for NativeContinuousMechanicsTrackSetRequest {
    fn track_request(&self) -> TrackRequestView {
        TrackRequestView {
            catalog: self.catalog,
            entity: self.entity,
            track: self.track,
            operation: self.operation,
            value_bits: self.value_bits,
            revision_guard: self.revision_guard,
            expected_revision: self.expected_revision,
            policy: self.policy,
        }
    }
}
impl TrackRequest for NativeContinuousMechanicsTrackAdjustmentRequest {
    fn track_request(&self) -> TrackRequestView {
        TrackRequestView {
            catalog: self.catalog,
            entity: self.entity,
            track: self.track,
            operation: self.operation,
            value_bits: self.amount_bits,
            revision_guard: self.revision_guard,
            expected_revision: self.expected_revision,
            policy: NativeContinuousMechanicsTrackSetPolicy::RejectOutOfBounds,
        }
    }
}

unsafe extern "C" fn apply_effect(
    context: *mut c_void,
    request: *const NativeContinuousMechanicsEffectApplyRequest,
    result: *mut NativeContinuousMechanicsEffectLease,
) -> i32 {
    effect_operation(context, request, result, false)
}
unsafe extern "C" fn remove_effect(
    context: *mut c_void,
    request: *const NativeContinuousMechanicsEffectRemoveRequest,
    result: *mut NativeContinuousMechanicsEffectLease,
) -> i32 {
    effect_operation(context, request, result, true)
}

unsafe fn effect_operation<R>(
    context: *mut c_void,
    request: *const R,
    result: *mut NativeContinuousMechanicsEffectLease,
    remove: bool,
) -> i32
where
    R: EffectRequest,
{
    let Some((bridge, request, result)) = bridge_request_result(context, request, result) else {
        return 0;
    };
    let request = request.effect_request();
    let (Ok(operation), Ok(instance)) = (
        text(request.operation).and_then(parse_operation),
        text(request.instance).and_then(parse_effect_instance),
    ) else {
        return 0;
    };
    let Some((catalog_id, entity, state, catalog)) =
        resolve_mut(bridge, request.catalog, request.entity)
    else {
        return 0;
    };
    let Some(expected) = guarded_revision::<ContinuousActiveEffectsComponent>(
        state,
        entity,
        request.guard,
        request.expected_revision,
        ContinuousMechanicsComponentKind::ActiveEffects,
    ) else {
        return 0;
    };
    let receipt = if remove {
        ContinuousEffectService::remove(
            state,
            catalog,
            ContinuousEffectRemoveRequest {
                operation: operation.clone(),
                entity,
                instance: instance.clone(),
                expected_revision: expected,
            },
        )
    } else {
        let Ok(definition) = parse_effect(text(request.definition).unwrap_or("")) else {
            return 0;
        };
        ContinuousEffectService::apply(
            state,
            catalog,
            ContinuousEffectApplyRequest {
                operation: operation.clone(),
                entity,
                effect: ContinuousActiveEffectInstance::new(instance.clone(), definition),
                expected_revision: expected,
            },
        )
    };
    let Ok(receipt) = receipt else {
        return 0;
    };
    let Some(value) = insert_effect(bridge, catalog_id, &receipt) else {
        return 0;
    };
    let OperationLease::Effect {
        catalog_version,
        catalog_fingerprint,
        operation,
        instance,
        ..
    } = bridge
        .continuous
        .operation_leases
        .get(&value)
        .expect("just inserted operation lease")
        .as_ref()
    else {
        unreachable!()
    };
    *result = NativeContinuousMechanicsEffectLease {
        handle: NativeContinuousMechanicsOperationLeaseHandle { value },
        catalog_id,
        catalog_version: *catalog_version,
        catalog_fingerprint: *catalog_fingerprint,
        operation: *operation,
        entity_id: entity.raw(),
        instance: *instance,
        removed: receipt.removed,
        observed_revision: receipt.observed_revision,
        committed_revision: receipt.committed_revision,
    };
    ABI_OK
}

trait EffectRequest {
    fn effect_request(&self) -> EffectRequestView;
}
struct EffectRequestView {
    catalog: NativeContinuousMechanicsCatalogHandle,
    entity: NativeMechanicsEntityHandle,
    operation: NativeUtf8Slice,
    instance: NativeUtf8Slice,
    definition: NativeUtf8Slice,
    guard: NativeContinuousMechanicsRevisionGuard,
    expected_revision: u64,
}
impl EffectRequest for NativeContinuousMechanicsEffectApplyRequest {
    fn effect_request(&self) -> EffectRequestView {
        EffectRequestView {
            catalog: self.catalog,
            entity: self.entity,
            operation: self.operation,
            instance: self.instance,
            definition: self.definition,
            guard: self.revision_guard,
            expected_revision: self.expected_revision,
        }
    }
}
impl EffectRequest for NativeContinuousMechanicsEffectRemoveRequest {
    fn effect_request(&self) -> EffectRequestView {
        EffectRequestView {
            catalog: self.catalog,
            entity: self.entity,
            operation: self.operation,
            instance: self.instance,
            definition: NativeUtf8Slice::default(),
            guard: self.revision_guard,
            expected_revision: self.expected_revision,
        }
    }
}

unsafe extern "C" fn destroy_operation_lease(
    context: *mut c_void,
    handle: NativeContinuousMechanicsOperationLeaseHandle,
) -> i32 {
    if context.is_null() || handle.value == 0 {
        return 0;
    }
    i32::from(unsafe {
        (&mut *context.cast::<RuntimeMechanicsBridge>())
            .continuous
            .operation_leases
            .remove(&handle.value)
            .is_some()
    })
}

unsafe fn parse_catalog(
    request: &NativeContinuousMechanicsCatalogCreateRequest,
) -> Result<ContinuousMechanicsCatalogDefinition, ()> {
    let version = parse_version(text(request.version)?)?;
    let stats =
        unsafe { borrowed_slice(request.stats, request.stats_len, "continuous catalog stats") }
            .map_err(|_| ())?
            .iter()
            .map(|row| {
                Ok(ContinuousStatDefinition::new(
                    parse_stat(text(row.id)?)?,
                    ContinuousValue::from_bits(row.minimum_bits).map_err(|_| ())?,
                    ContinuousValue::from_bits(row.maximum_bits).map_err(|_| ())?,
                )
                .map_err(|_| ())?)
            })
            .collect::<Result<Vec<_>, ()>>()?;
    let tracks = unsafe {
        borrowed_slice(
            request.tracks,
            request.tracks_len,
            "continuous catalog tracks",
        )
    }
    .map_err(|_| ())?
    .iter()
    .map(|row| {
        let maximum = match row.maximum_kind {
            NativeContinuousMechanicsTrackMaximumKind::Fixed => ContinuousTrackMaximum::fixed(
                ContinuousValue::from_bits(row.fixed_maximum_bits).map_err(|_| ())?,
            ),
            NativeContinuousMechanicsTrackMaximumKind::Stat => ContinuousTrackMaximum::Stat {
                stat: parse_stat(text(row.maximum_stat)?)?,
            },
        };
        ContinuousTrackDefinition::new(
            parse_track(text(row.id)?)?,
            ContinuousValue::from_bits(row.minimum_bits).map_err(|_| ())?,
            maximum,
        )
        .map_err(|_| ())
    })
    .collect::<Result<Vec<_>, ()>>()?;
    let contribution_rows = unsafe {
        borrowed_slice(
            request.contributions,
            request.contributions_len,
            "continuous catalog contributions",
        )
    }
    .map_err(|_| ())?;
    let sources = unsafe {
        borrowed_slice(
            request.sources,
            request.sources_len,
            "continuous catalog sources",
        )
    }
    .map_err(|_| ())?
    .iter()
    .map(|row| {
        let start = row.contributions_start as usize;
        let end = start
            .checked_add(row.contributions_len as usize)
            .filter(|end| *end <= contribution_rows.len())
            .ok_or(())?;
        Ok(ContinuousSourceDefinition {
            id: parse_source(text(row.id)?)?,
            priority: row.priority,
            stat_contributions: contribution_rows[start..end]
                .iter()
                .map(|row| unsafe { parse_contribution(row) })
                .collect::<Result<Vec<_>, ()>>()?,
        })
    })
    .collect::<Result<Vec<_>, ()>>()?;
    let effect_sources = unsafe {
        borrowed_slice(
            request.effect_sources,
            request.effect_sources_len,
            "continuous catalog effect sources",
        )
    }
    .map_err(|_| ())?;
    let effects = unsafe {
        borrowed_slice(
            request.effects,
            request.effects_len,
            "continuous catalog effects",
        )
    }
    .map_err(|_| ())?
    .iter()
    .map(|row| {
        let start = row.sources_start as usize;
        let end = start
            .checked_add(row.sources_len as usize)
            .filter(|end| *end <= effect_sources.len())
            .ok_or(())?;
        Ok(ContinuousEffectDefinition {
            id: parse_effect(text(row.id)?)?,
            sources: effect_sources[start..end]
                .iter()
                .map(|source| parse_source(text(source.source)?))
                .collect::<Result<Vec<_>, ()>>()?,
        })
    })
    .collect::<Result<Vec<_>, ()>>()?;
    Ok(ContinuousMechanicsCatalogDefinition {
        version,
        stats,
        tracks,
        sources,
        effects,
    })
}

unsafe fn parse_contribution(
    row: &NativeContinuousMechanicsCatalogContributionRow,
) -> Result<ContinuousStatContributionDefinition, ()> {
    Ok(ContinuousStatContributionDefinition {
        stat: parse_stat(text(row.stat)?)?,
        contribution: match row.kind {
            NativeContinuousMechanicsContributionKind::Add => ContinuousStatContribution::add(
                ContinuousValue::from_bits(row.value_bits).map_err(|_| ())?,
            ),
            NativeContinuousMechanicsContributionKind::Minimum => {
                ContinuousStatContribution::minimum(
                    ContinuousValue::from_bits(row.value_bits).map_err(|_| ())?,
                )
            }
            NativeContinuousMechanicsContributionKind::Maximum => {
                ContinuousStatContribution::maximum(
                    ContinuousValue::from_bits(row.value_bits).map_err(|_| ())?,
                )
            }
        },
        stacking_group: gameplay_continuous_mechanics::ContinuousStackingGroupId::parse(
            text(row.stacking_group)?.to_owned(),
        )
        .map_err(|_| ())?,
        stacking: match row.stacking {
            NativeContinuousMechanicsStackingPolicy::Sum => ContinuousStackingPolicy::Sum,
            NativeContinuousMechanicsStackingPolicy::Highest => ContinuousStackingPolicy::Highest,
            NativeContinuousMechanicsStackingPolicy::Lowest => ContinuousStackingPolicy::Lowest,
            NativeContinuousMechanicsStackingPolicy::UniqueBySource => {
                ContinuousStackingPolicy::UniqueBySource
            }
        },
    })
}

struct InitialComponents {
    stats: Option<Vec<ContinuousStatValue>>,
    tracks: Option<Vec<ContinuousTrackValue>>,
    intrinsic: Option<Vec<ContinuousIntrinsicSourceBinding>>,
    effects: Option<Vec<ContinuousActiveEffectInstance>>,
}
unsafe fn parse_initial_components(
    request: &NativeContinuousMechanicsInitialComponentsRequest,
) -> Result<InitialComponents, ()> {
    Ok(InitialComponents {
        stats: request
            .has_stats
            .then(|| {
                unsafe {
                    borrowed_slice(request.stats, request.stats_len, "continuous initial stats")
                }
                .map_err(|_| ())?
                .iter()
                .map(|row| {
                    Ok(ContinuousStatValue::new(
                        parse_stat(text(row.stat)?)?,
                        ContinuousValue::from_bits(row.base_bits).map_err(|_| ())?,
                    ))
                })
                .collect::<Result<Vec<_>, ()>>()
            })
            .transpose()?,
        tracks: request
            .has_tracks
            .then(|| {
                unsafe {
                    borrowed_slice(
                        request.tracks,
                        request.tracks_len,
                        "continuous initial tracks",
                    )
                }
                .map_err(|_| ())?
                .iter()
                .map(|row| {
                    Ok(ContinuousTrackValue::new(
                        parse_track(text(row.track)?)?,
                        ContinuousValue::from_bits(row.current_bits).map_err(|_| ())?,
                    ))
                })
                .collect::<Result<Vec<_>, ()>>()
            })
            .transpose()?,
        intrinsic: request
            .has_intrinsic_sources
            .then(|| {
                unsafe {
                    borrowed_slice(
                        request.intrinsic_sources,
                        request.intrinsic_sources_len,
                        "continuous initial intrinsic sources",
                    )
                }
                .map_err(|_| ())?
                .iter()
                .map(|row| {
                    Ok(ContinuousIntrinsicSourceBinding::new(
                        parse_source_instance(text(row.instance)?)?,
                        parse_source(text(row.definition)?)?,
                    ))
                })
                .collect::<Result<Vec<_>, ()>>()
            })
            .transpose()?,
        effects: request
            .has_active_effects
            .then(|| {
                unsafe {
                    borrowed_slice(
                        request.active_effects,
                        request.active_effects_len,
                        "continuous initial active effects",
                    )
                }
                .map_err(|_| ())?
                .iter()
                .map(|row| {
                    Ok(ContinuousActiveEffectInstance::new(
                        parse_effect_instance(text(row.instance)?)?,
                        parse_effect(text(row.definition)?)?,
                    ))
                })
                .collect::<Result<Vec<_>, ()>>()
            })
            .transpose()?,
    })
}

fn components_from_values(
    catalog: &ContinuousMechanicsCatalog,
    values: InitialComponents,
) -> Result<
    (
        Option<ContinuousStatsComponent>,
        Option<ContinuousTracksComponent>,
        Option<ContinuousIntrinsicSourcesComponent>,
        Option<ContinuousActiveEffectsComponent>,
    ),
    (),
> {
    Ok((
        values
            .stats
            .map(|values| {
                ContinuousStatsComponent::new(catalog.version().clone(), values).map_err(|_| ())
            })
            .transpose()?,
        values
            .tracks
            .map(|values| {
                ContinuousTracksComponent::new(catalog.version().clone(), values).map_err(|_| ())
            })
            .transpose()?,
        values
            .intrinsic
            .map(|values| {
                ContinuousIntrinsicSourcesComponent::new(catalog.version().clone(), values)
                    .map_err(|_| ())
            })
            .transpose()?,
        values
            .effects
            .map(|values| {
                ContinuousActiveEffectsComponent::new(catalog.version().clone(), values)
                    .map_err(|_| ())
            })
            .transpose()?,
    ))
}

fn replace_optional<T: EntityComponent + PartialEq>(
    state: &mut EntityState,
    entity: EntityId,
    value: Option<T>,
) -> Result<(), ()> {
    let expected = state.component_revision::<T>(entity).map_err(|_| ())?;
    match value {
        Some(value) => {
            if state.component::<T>(entity).map_err(|_| ())?.is_some() {
                EntityAuthoringService
                    .replace_component(state, expected, entity, value)
                    .map(|_| ())
                    .map_err(|_| ())
            } else {
                EntityAuthoringService
                    .attach_component(state, expected, entity, value)
                    .map(|_| ())
                    .map_err(|_| ())
            }
        }
        None => {
            if state.component::<T>(entity).map_err(|_| ())?.is_some() {
                EntityAuthoringService
                    .detach_component::<T>(state, expected, entity)
                    .map(|_| ())
                    .map_err(|_| ())
            } else {
                Ok(())
            }
        }
    }
}

fn catalog_lease(catalog: &ContinuousMechanicsCatalog) -> Option<CatalogLease> {
    let mut text = TextPool::default();
    let version = text.copy(catalog.version().as_str());
    let fingerprint = text.copy(catalog.fingerprint());
    let stats = catalog
        .definition()
        .stats
        .iter()
        .map(|row| NativeContinuousMechanicsCatalogStatRow {
            id: text.copy(row.id.as_str()),
            minimum_bits: row.minimum().bits(),
            maximum_bits: row.maximum().bits(),
        })
        .collect();
    let tracks = catalog
        .definition()
        .tracks
        .iter()
        .map(|row| {
            let (kind, fixed, stat) = match &row.maximum {
                ContinuousTrackMaximum::Fixed { value } => (
                    NativeContinuousMechanicsTrackMaximumKind::Fixed,
                    value.bits(),
                    "",
                ),
                ContinuousTrackMaximum::Stat { stat } => (
                    NativeContinuousMechanicsTrackMaximumKind::Stat,
                    0,
                    stat.as_str(),
                ),
            };
            NativeContinuousMechanicsCatalogTrackRow {
                id: text.copy(row.id.as_str()),
                minimum_bits: row.minimum().bits(),
                maximum_kind: kind,
                fixed_maximum_bits: fixed,
                maximum_stat: text.copy(stat),
            }
        })
        .collect();
    let mut contributions = Vec::new();
    let sources = catalog
        .definition()
        .sources
        .iter()
        .map(|source| {
            let start = u32::try_from(contributions.len()).ok()?;
            for row in &source.stat_contributions {
                contributions.push(native_contribution(row, &mut text));
            }
            Some(NativeContinuousMechanicsCatalogSourceRow {
                id: text.copy(source.id.as_str()),
                priority: source.priority,
                contributions_start: start,
                contributions_len: u32::try_from(contributions.len())
                    .ok()?
                    .checked_sub(start)?,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let mut effect_sources = Vec::new();
    let effects = catalog
        .definition()
        .effects
        .iter()
        .map(|effect| {
            let start = u32::try_from(effect_sources.len()).ok()?;
            for source in &effect.sources {
                effect_sources.push(NativeContinuousMechanicsCatalogEffectSourceRow {
                    source: text.copy(source.as_str()),
                });
            }
            Some(NativeContinuousMechanicsCatalogEffectRow {
                id: text.copy(effect.id.as_str()),
                sources_start: start,
                sources_len: u32::try_from(effect_sources.len())
                    .ok()?
                    .checked_sub(start)?,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    Some(CatalogLease {
        _text: text,
        stats,
        tracks,
        sources,
        contributions,
        effects,
        effect_sources,
        version,
        fingerprint,
    })
}
fn native_contribution(
    row: &ContinuousStatContributionDefinition,
    text: &mut TextPool,
) -> NativeContinuousMechanicsCatalogContributionRow {
    let (kind, value_bits) = match &row.contribution {
        ContinuousStatContribution::Add { amount } => (
            NativeContinuousMechanicsContributionKind::Add,
            amount.bits(),
        ),
        ContinuousStatContribution::Minimum { value } => (
            NativeContinuousMechanicsContributionKind::Minimum,
            value.bits(),
        ),
        ContinuousStatContribution::Maximum { value } => (
            NativeContinuousMechanicsContributionKind::Maximum,
            value.bits(),
        ),
    };
    NativeContinuousMechanicsCatalogContributionRow {
        stat: text.copy(row.stat.as_str()),
        kind,
        value_bits,
        stacking_group: text.copy(row.stacking_group.as_str()),
        stacking: native_stacking(row.stacking),
    }
}
fn native_stacking(value: ContinuousStackingPolicy) -> NativeContinuousMechanicsStackingPolicy {
    match value {
        ContinuousStackingPolicy::Sum => NativeContinuousMechanicsStackingPolicy::Sum,
        ContinuousStackingPolicy::Highest => NativeContinuousMechanicsStackingPolicy::Highest,
        ContinuousStackingPolicy::Lowest => NativeContinuousMechanicsStackingPolicy::Lowest,
        ContinuousStackingPolicy::UniqueBySource => {
            NativeContinuousMechanicsStackingPolicy::UniqueBySource
        }
    }
}

fn component_lease(
    _catalog_id: u64,
    entity: EntityId,
    state: &EntityState,
    catalog: &ContinuousMechanicsCatalog,
) -> Option<ComponentLease> {
    let mut text = TextPool::default();
    let catalog_version = text.copy(catalog.version().as_str());
    let catalog_fingerprint = text.copy(catalog.fingerprint());
    let components = [
        NativeContinuousMechanicsComponentKind::Stats,
        NativeContinuousMechanicsComponentKind::Tracks,
        NativeContinuousMechanicsComponentKind::IntrinsicSources,
        NativeContinuousMechanicsComponentKind::ActiveEffects,
    ]
    .into_iter()
    .map(|component| NativeContinuousMechanicsComponentPresenceRow {
        component,
        present: component_present(state, entity, component),
        revision: component_revision(state, entity, component),
    })
    .collect();
    let stats = state
        .component::<ContinuousStatsComponent>(entity)
        .ok()
        .flatten()
        .map(|value| {
            value
                .values()
                .iter()
                .map(|row| NativeContinuousMechanicsInitialStatRow {
                    stat: text.copy(row.stat().as_str()),
                    base_bits: row.base().bits(),
                })
                .collect()
        })
        .unwrap_or_default();
    let tracks = state
        .component::<ContinuousTracksComponent>(entity)
        .ok()
        .flatten()
        .map(|value| {
            value
                .values()
                .iter()
                .map(|row| NativeContinuousMechanicsInitialTrackRow {
                    track: text.copy(row.track().as_str()),
                    current_bits: row.current().bits(),
                })
                .collect()
        })
        .unwrap_or_default();
    let intrinsic_sources = state
        .component::<ContinuousIntrinsicSourcesComponent>(entity)
        .ok()
        .flatten()
        .map(|value| {
            value
                .bindings()
                .iter()
                .map(|row| NativeContinuousMechanicsInitialIntrinsicSourceRow {
                    instance: text.copy(row.instance().as_str()),
                    definition: text.copy(row.definition().as_str()),
                })
                .collect()
        })
        .unwrap_or_default();
    let active_effects = state
        .component::<ContinuousActiveEffectsComponent>(entity)
        .ok()
        .flatten()
        .map(|value| {
            value
                .effects()
                .iter()
                .map(|row| NativeContinuousMechanicsInitialActiveEffectRow {
                    instance: text.copy(row.instance().as_str()),
                    definition: text.copy(row.definition().as_str()),
                })
                .collect()
        })
        .unwrap_or_default();
    Some(ComponentLease {
        _text: text,
        catalog_version,
        catalog_fingerprint,
        components,
        stats,
        tracks,
        intrinsic_sources,
        active_effects,
    })
}

fn component_present(
    state: &EntityState,
    entity: EntityId,
    component: NativeContinuousMechanicsComponentKind,
) -> bool {
    match component {
        NativeContinuousMechanicsComponentKind::Stats => state
            .component::<ContinuousStatsComponent>(entity)
            .ok()
            .flatten()
            .is_some(),
        NativeContinuousMechanicsComponentKind::Tracks => state
            .component::<ContinuousTracksComponent>(entity)
            .ok()
            .flatten()
            .is_some(),
        NativeContinuousMechanicsComponentKind::IntrinsicSources => state
            .component::<ContinuousIntrinsicSourcesComponent>(entity)
            .ok()
            .flatten()
            .is_some(),
        NativeContinuousMechanicsComponentKind::ActiveEffects => state
            .component::<ContinuousActiveEffectsComponent>(entity)
            .ok()
            .flatten()
            .is_some(),
    }
}
fn component_revision(
    state: &EntityState,
    entity: EntityId,
    component: NativeContinuousMechanicsComponentKind,
) -> u64 {
    match component {
        NativeContinuousMechanicsComponentKind::Stats => state
            .component_revision::<ContinuousStatsComponent>(entity)
            .map(|value| value.revision())
            .unwrap_or_default(),
        NativeContinuousMechanicsComponentKind::Tracks => state
            .component_revision::<ContinuousTracksComponent>(entity)
            .map(|value| value.revision())
            .unwrap_or_default(),
        NativeContinuousMechanicsComponentKind::IntrinsicSources => state
            .component_revision::<ContinuousIntrinsicSourcesComponent>(entity)
            .map(|value| value.revision())
            .unwrap_or_default(),
        NativeContinuousMechanicsComponentKind::ActiveEffects => state
            .component_revision::<ContinuousActiveEffectsComponent>(entity)
            .map(|value| value.revision())
            .unwrap_or_default(),
    }
}

fn resolve_read<'a>(
    bridge: &'a RuntimeMechanicsBridge,
    catalog: NativeContinuousMechanicsCatalogHandle,
    handle: NativeMechanicsEntityHandle,
) -> Option<(
    u64,
    EntityId,
    &'a EntityState,
    &'a ContinuousMechanicsCatalog,
)> {
    let binding = bridge.binding(handle)?;
    if bridge.continuous.associations.get(&binding.entity) != Some(&catalog.value) {
        return None;
    }
    let slot = bridge.catalogs.get(&binding.catalog)?;
    if !slot.world.is_active(binding.entity) {
        return None;
    }
    Some((
        catalog.value,
        binding.entity,
        &slot.world.state,
        bridge.continuous.catalogs.get(&catalog.value)?,
    ))
}
fn resolve_read_mut<'a>(
    bridge: &'a mut RuntimeMechanicsBridge,
    catalog: NativeContinuousMechanicsCatalogHandle,
    handle: NativeMechanicsEntityHandle,
) -> Option<(
    u64,
    EntityId,
    &'a mut EntityState,
    &'a ContinuousMechanicsCatalog,
)> {
    let RuntimeMechanicsBridge {
        catalogs,
        entities,
        continuous,
        ..
    } = bridge;
    let binding = entities.get(&handle.value)?.clone();
    if !binding.committed || continuous.associations.get(&binding.entity) != Some(&catalog.value) {
        return None;
    }
    let continuous_catalog = continuous.catalogs.get(&catalog.value)?;
    let slot = catalogs.get_mut(&binding.catalog)?;
    if !slot.world.is_active(binding.entity) {
        return None;
    }
    Some((
        catalog.value,
        binding.entity,
        &mut slot.world.state,
        continuous_catalog,
    ))
}
fn resolve_mut<'a>(
    bridge: &'a mut RuntimeMechanicsBridge,
    catalog: NativeContinuousMechanicsCatalogHandle,
    handle: NativeMechanicsEntityHandle,
) -> Option<(
    u64,
    EntityId,
    &'a mut EntityState,
    &'a ContinuousMechanicsCatalog,
)> {
    resolve_read_mut(bridge, catalog, handle)
}
fn guarded_revision<T: EntityComponent>(
    state: &EntityState,
    entity: EntityId,
    guard: NativeContinuousMechanicsRevisionGuard,
    expected_revision: u64,
    _kind: ContinuousMechanicsComponentKind,
) -> Option<Option<ComponentRevision>> {
    match guard {
        NativeContinuousMechanicsRevisionGuard::Unchecked => Some(None),
        NativeContinuousMechanicsRevisionGuard::Exact => {
            let actual = state.component_revision::<T>(entity).ok()?;
            (actual.revision() == expected_revision).then_some(Some(actual))
        }
    }
}

impl RuntimeMechanicsBridge {
    fn canonical_entity_is_live(&self, entity: EntityId) -> bool {
        self.catalogs.values().any(|slot| {
            !matches!(
                slot.world.state.lifecycle(entity),
                Some(EntityLifecycle::Tombstoned) | None
            )
        })
    }
    fn entity_matches_continuous_catalog(
        &self,
        entity: EntityId,
        catalog: NativeContinuousMechanicsCatalogHandle,
    ) -> bool {
        self.continuous.catalogs.contains_key(&catalog.value)
            && self
                .continuous
                .associations
                .get(&entity)
                .is_none_or(|existing| *existing == catalog.value)
    }
}

fn insert_operation(bridge: &mut RuntimeMechanicsBridge, lease: OperationLease) -> Option<u64> {
    let value = bridge.continuous.next_operation_lease.max(1);
    bridge.continuous.next_operation_lease = value.checked_add(1)?;
    bridge
        .continuous
        .operation_leases
        .insert(value, Box::new(lease));
    Some(value)
}
fn insert_stat_evaluation(
    bridge: &mut RuntimeMechanicsBridge,
    catalog_id: u64,
    receipt: gameplay_continuous_mechanics::ContinuousStatEvaluation,
) -> Option<u64> {
    let mut text = TextPool::default();
    let catalog_version = text.copy(receipt.catalog_version.as_str());
    let catalog_fingerprint = text.copy(&receipt.catalog_fingerprint);
    let stat = text.copy(receipt.stat.as_str());
    let catalog = bridge.continuous.catalogs.get(&catalog_id)?;
    let decisions = receipt
        .decisions
        .iter()
        .map(|value| native_decision(catalog, value, &mut text))
        .collect();
    insert_operation(
        bridge,
        OperationLease::StatEvaluation {
            _text: text,
            decisions,
            catalog_version,
            catalog_fingerprint,
            stat,
        },
    )
}
fn native_decision(
    catalog: &ContinuousMechanicsCatalog,
    value: &gameplay_continuous_mechanics::ContinuousStatDecision,
    text: &mut TextPool,
) -> NativeContinuousMechanicsStatDecisionRow {
    let (intrinsic, source_instance, effect_instance, source_definition) = match &value.source {
        ContinuousSourceIdentity::Intrinsic(instance) => (
            true,
            text.copy(instance.as_str()),
            text.copy(""),
            text.copy(value.source_definition.as_str()),
        ),
        ContinuousSourceIdentity::Effect { effect, source } => (
            false,
            text.copy(""),
            text.copy(effect.as_str()),
            text.copy(source.as_str()),
        ),
    };
    let (contribution_kind, contribution_value_bits) = match &value.contribution {
        ContinuousStatContribution::Add { amount } => (
            NativeContinuousMechanicsContributionKind::Add,
            amount.bits(),
        ),
        ContinuousStatContribution::Minimum { value } => (
            NativeContinuousMechanicsContributionKind::Minimum,
            value.bits(),
        ),
        ContinuousStatContribution::Maximum { value } => (
            NativeContinuousMechanicsContributionKind::Maximum,
            value.bits(),
        ),
    };
    let source = catalog
        .source(&value.source_definition)
        .expect("service decision references admitted source");
    let contribution = source
        .stat_contributions
        .get(usize::from(value.contribution_index))
        .expect("service decision contribution index is in the admitted source");
    NativeContinuousMechanicsStatDecisionRow {
        intrinsic,
        source_instance,
        effect_instance,
        source_definition,
        contribution_index: value.contribution_index,
        outcome: match value.outcome {
            ContinuousDecisionOutcome::Applied => NativeContinuousMechanicsDecisionOutcome::Applied,
            ContinuousDecisionOutcome::Suppressed => {
                NativeContinuousMechanicsDecisionOutcome::Suppressed
            }
            ContinuousDecisionOutcome::Inapplicable => {
                NativeContinuousMechanicsDecisionOutcome::Inapplicable
            }
        },
        contribution_kind,
        contribution_value_bits,
        stacking_group: text.copy(contribution.stacking_group.as_str()),
        stacking: native_stacking(contribution.stacking),
    }
}
fn insert_stat_mutation(
    bridge: &mut RuntimeMechanicsBridge,
    _catalog_id: u64,
    receipt: &gameplay_continuous_mechanics::ContinuousStatBaseMutationReceipt,
) -> Option<u64> {
    let mut text = TextPool::default();
    let catalog_version = text.copy(receipt.catalog_version.as_str());
    let catalog_fingerprint = text.copy(&receipt.catalog_fingerprint);
    let operation = text.copy(receipt.operation.as_str());
    let stat = text.copy(receipt.stat.as_str());
    insert_operation(
        bridge,
        OperationLease::StatMutation {
            _text: text,
            catalog_version,
            catalog_fingerprint,
            operation,
            stat,
        },
    )
}
#[allow(clippy::too_many_arguments)]
fn insert_track(
    bridge: &mut RuntimeMechanicsBridge,
    _catalog_id: u64,
    catalog_version_value: &str,
    catalog_fingerprint_value: &str,
    _entity: EntityId,
    operation: &str,
    track: &ContinuousTrackId,
    _kind: TrackOp,
    _before: ContinuousValue,
    _after: ContinuousValue,
    _minimum: ContinuousValue,
    _maximum: ContinuousValue,
    _adjustment_kind: NativeContinuousMechanicsTrackAdjustmentKind,
    _requested: ContinuousValue,
    _applied: ContinuousValue,
    _observed: u64,
    _stats: Option<u64>,
    _sources: Option<u64>,
    _effects: Option<u64>,
    _committed: u64,
) -> Option<u64> {
    let mut text = TextPool::default();
    let catalog_version = text.copy(catalog_version_value);
    let catalog_fingerprint = text.copy(catalog_fingerprint_value);
    let operation = text.copy(operation);
    let track = text.copy(track.as_str());
    insert_operation(
        bridge,
        OperationLease::Track {
            _text: text,
            catalog_version,
            catalog_fingerprint,
            operation,
            track,
        },
    )
}
fn insert_effect(
    bridge: &mut RuntimeMechanicsBridge,
    _catalog_id: u64,
    receipt: &gameplay_continuous_mechanics::ContinuousEffectMutationReceipt,
) -> Option<u64> {
    let mut text = TextPool::default();
    let catalog_version = text.copy(receipt.catalog_version.as_str());
    let catalog_fingerprint = text.copy(&receipt.catalog_fingerprint);
    let operation = text.copy(receipt.operation.as_str());
    let instance = text.copy(receipt.instance.as_str());
    insert_operation(
        bridge,
        OperationLease::Effect {
            _text: text,
            catalog_version,
            catalog_fingerprint,
            operation,
            instance,
        },
    )
}

unsafe fn text<'a>(value: NativeUtf8Slice) -> Result<&'a str, ()> {
    unsafe { borrowed_utf8(value.bytes, value.len, "continuous mechanics text") }.map_err(|_| ())
}
fn parse_version(value: &str) -> Result<ContinuousCatalogVersion, ()> {
    ContinuousCatalogVersion::parse(value.to_owned()).map_err(|_| ())
}
fn parse_stat(value: &str) -> Result<ContinuousStatId, ()> {
    ContinuousStatId::parse(value.to_owned()).map_err(|_| ())
}
fn parse_track(value: &str) -> Result<ContinuousTrackId, ()> {
    ContinuousTrackId::parse(value.to_owned()).map_err(|_| ())
}
fn parse_source(value: &str) -> Result<ContinuousSourceDefinitionId, ()> {
    ContinuousSourceDefinitionId::parse(value.to_owned()).map_err(|_| ())
}
fn parse_source_instance(
    value: &str,
) -> Result<gameplay_continuous_mechanics::ContinuousSourceInstanceId, ()> {
    gameplay_continuous_mechanics::ContinuousSourceInstanceId::parse(value.to_owned())
        .map_err(|_| ())
}
fn parse_effect(value: &str) -> Result<ContinuousEffectDefinitionId, ()> {
    ContinuousEffectDefinitionId::parse(value.to_owned()).map_err(|_| ())
}
fn parse_effect_instance(
    value: &str,
) -> Result<gameplay_continuous_mechanics::ContinuousEffectInstanceId, ()> {
    gameplay_continuous_mechanics::ContinuousEffectInstanceId::parse(value.to_owned())
        .map_err(|_| ())
}
fn parse_operation(value: &str) -> Result<ContinuousOperationId, ()> {
    ContinuousOperationId::parse(value.to_owned()).map_err(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mechanics::{CatalogSlot, EntityBinding, MechanicsWorld};
    use entity_state::{EntityDefinition, EntityLifecycle};
    use gameplay_mechanics::{CatalogVersion, MechanicsCatalog, MechanicsCatalogDefinition};

    fn utf8(value: &str) -> NativeUtf8Slice {
        NativeUtf8Slice {
            bytes: value.as_ptr(),
            len: value.len(),
        }
    }

    fn exact_bound_bridge() -> (RuntimeMechanicsBridge, NativeMechanicsEntityHandle) {
        let mut bridge = RuntimeMechanicsBridge::new();
        let exact_catalog = MechanicsCatalog::admit(MechanicsCatalogDefinition {
            version: CatalogVersion::parse("exact-v1".to_owned()).unwrap(),
            stats: vec![],
            tracks: vec![],
            sources: vec![],
            damage_kinds: vec![],
            effects: vec![],
            capacity_metrics: vec![],
            items: vec![],
            equipment_slots: vec![],
        })
        .unwrap();
        let entity = EntityId::new(7439);
        let state = EntityState::from_definitions_with_registry(
            gameplay_continuous_mechanics::combined_gameplay_component_registry().unwrap(),
            [EntityDefinition::new(entity, "continuous-hero")],
        )
        .unwrap();
        let mut world = MechanicsWorld::new(state);
        world.admit(entity).unwrap();
        bridge.catalogs.insert(
            1,
            CatalogSlot {
                builder: None,
                catalog: Some(exact_catalog),
                world,
            },
        );
        bridge.entities.insert(
            1,
            EntityBinding {
                catalog: 1,
                entity,
                identity: "continuous-hero".to_owned(),
                stats: None,
                tracks: None,
                intrinsic_sources: None,
                active_effects: None,
                inventory: None,
                item: None,
                equipment: None,
                initial_containment: vec![],
                expected_state_revision: None,
                initial_components_set: true,
                committed: true,
            },
        );
        (bridge, NativeMechanicsEntityHandle { value: 1 })
    }

    #[test]
    fn continuous_bridge_uses_one_committed_exact_entity_and_releases_bounded_leases() {
        let (mut bridge, entity) = exact_bound_bridge();
        let context = (&mut bridge as *mut RuntimeMechanicsBridge).cast::<c_void>();
        let stats = [NativeContinuousMechanicsCatalogStatRow {
            id: utf8("focus"),
            minimum_bits: 0,
            maximum_bits: 10.0f64.to_bits(),
        }];
        let create = NativeContinuousMechanicsCatalogCreateRequest {
            version: utf8("continuous-v1"),
            stats: stats.as_ptr(),
            stats_len: stats.len(),
            tracks: std::ptr::null(),
            tracks_len: 0,
            sources: std::ptr::null(),
            sources_len: 0,
            contributions: std::ptr::null(),
            contributions_len: 0,
            effects: std::ptr::null(),
            effects_len: 0,
            effect_sources: std::ptr::null(),
            effect_sources_len: 0,
        };
        let mut catalog = NativeContinuousMechanicsCatalogHandle::default();
        assert_eq!(
            unsafe { create_catalog(context, &create, &mut catalog) },
            ABI_OK
        );

        let initial_stats = [NativeContinuousMechanicsInitialStatRow {
            stat: utf8("focus"),
            base_bits: 1u64,
        }];
        let initial = NativeContinuousMechanicsInitialComponentsRequest {
            catalog,
            entity,
            has_stats: true,
            stats: initial_stats.as_ptr(),
            stats_len: initial_stats.len(),
            has_tracks: true,
            tracks: std::ptr::null(),
            tracks_len: 0,
            has_intrinsic_sources: true,
            intrinsic_sources: std::ptr::null(),
            intrinsic_sources_len: 0,
            has_active_effects: true,
            active_effects: std::ptr::null(),
            active_effects_len: 0,
        };
        assert_eq!(unsafe { set_initial_components(context, &initial) }, ABI_OK);

        let mut components = NativeContinuousMechanicsComponentLease {
            handle: Default::default(),
            catalog_id: 0,
            catalog_version: Default::default(),
            catalog_fingerprint: Default::default(),
            entity_id: 0,
            components: std::ptr::null(),
            components_len: 0,
            stats: std::ptr::null(),
            stats_len: 0,
            tracks: std::ptr::null(),
            tracks_len: 0,
            intrinsic_sources: std::ptr::null(),
            intrinsic_sources_len: 0,
            active_effects: std::ptr::null(),
            active_effects_len: 0,
        };
        let components_request = NativeContinuousMechanicsComponentReadRequest { catalog, entity };
        assert_eq!(
            unsafe { read_components(context, &components_request, &mut components) },
            ABI_OK
        );
        let component_rows =
            unsafe { std::slice::from_raw_parts(components.components, components.components_len) };
        assert!(component_rows.iter().all(|row| row.present));
        let stat_rows =
            unsafe { std::slice::from_raw_parts(components.stats, components.stats_len) };
        assert_eq!(stat_rows[0].base_bits, 1, "subnormal bits remain exact");
        assert_eq!(
            unsafe { destroy_component_lease(context, components.handle) },
            ABI_OK
        );

        let mut evaluation = NativeContinuousMechanicsStatEvaluationLease {
            handle: Default::default(),
            decisions: std::ptr::null(),
            decisions_len: 0,
            catalog_id: 0,
            catalog_version: Default::default(),
            catalog_fingerprint: Default::default(),
            entity_id: 0,
            stat: Default::default(),
            base_bits: 0,
            after_additions_bits: 0,
            unconstrained_bits: 0,
            minimum_bits: 0,
            maximum_bits: 0,
            value_bits: 0,
            observed_revisions: Default::default(),
        };
        let evaluate = NativeContinuousMechanicsStatEvaluateRequest {
            catalog,
            entity,
            stat: utf8("focus"),
        };
        assert_eq!(
            unsafe { evaluate_stat(context, &evaluate, &mut evaluation) },
            ABI_OK
        );
        assert_eq!(evaluation.value_bits, 1);
        assert_eq!(
            unsafe { destroy_operation_lease(context, evaluation.handle) },
            ABI_OK
        );

        let update = NativeContinuousMechanicsStatBaseMutationRequest {
            catalog,
            entity,
            operation: utf8("set-focus"),
            stat: utf8("focus"),
            base_bits: (-0.0f64).to_bits(),
            revision_guard: NativeContinuousMechanicsRevisionGuard::Unchecked,
            expected_revision: 0,
        };
        let mut mutation = NativeContinuousMechanicsStatMutationLease {
            handle: Default::default(),
            catalog_id: 0,
            catalog_version: Default::default(),
            catalog_fingerprint: Default::default(),
            operation: Default::default(),
            entity_id: 0,
            stat: Default::default(),
            before_bits: 0,
            after_bits: 0,
            minimum_bits: 0,
            maximum_bits: 0,
            observed_revision: 0,
            committed_revision: 0,
        };
        assert_eq!(
            unsafe { set_stat_base(context, &update, &mut mutation) },
            ABI_OK
        );
        assert_eq!(mutation.before_bits, 1);
        assert_eq!(
            mutation.after_bits, 0,
            "negative zero normalizes at the Engine owner boundary"
        );
        assert_eq!(
            unsafe { destroy_operation_lease(context, mutation.handle) },
            ABI_OK
        );

        let stale = NativeContinuousMechanicsStatBaseMutationRequest {
            catalog,
            entity,
            operation: utf8("set-focus"),
            stat: utf8("focus"),
            base_bits: 3.0f64.to_bits(),
            revision_guard: NativeContinuousMechanicsRevisionGuard::Exact,
            expected_revision: 0,
        };
        let mut stale_mutation = NativeContinuousMechanicsStatMutationLease {
            handle: Default::default(),
            catalog_id: 0,
            catalog_version: Default::default(),
            catalog_fingerprint: Default::default(),
            operation: Default::default(),
            entity_id: 0,
            stat: Default::default(),
            before_bits: 0,
            after_bits: 0,
            minimum_bits: 0,
            maximum_bits: 0,
            observed_revision: 0,
            committed_revision: 0,
        };
        assert_eq!(
            unsafe { set_stat_base(context, &stale, &mut stale_mutation) },
            0
        );
        assert_eq!(
            unsafe { evaluate_stat(context, &evaluate, &mut evaluation) },
            ABI_OK
        );
        assert_eq!(
            evaluation.value_bits, 0,
            "stale mutation preserves shared state"
        );
        assert_eq!(
            unsafe { destroy_operation_lease(context, evaluation.handle) },
            ABI_OK
        );

        bridge
            .catalogs
            .get_mut(&1)
            .unwrap()
            .world
            .set_lifecycle(
                EntityId::new(7439),
                NativeMechanicsEntityLifecycle::Disabled,
            )
            .unwrap();
        assert_eq!(
            bridge.catalogs[&1]
                .world
                .state
                .lifecycle(EntityId::new(7439)),
            Some(EntityLifecycle::Disabled)
        );
        assert_eq!(
            unsafe { evaluate_stat(context, &evaluate, &mut evaluation) },
            0,
            "exact lifecycle fences continuous operations"
        );
    }

    #[test]
    fn continuous_world_export_stages_with_the_exact_candidate_and_publishes_once() {
        let (mut bridge, entity) = exact_bound_bridge();
        let context = (&mut bridge as *mut RuntimeMechanicsBridge).cast::<c_void>();
        let stats = [NativeContinuousMechanicsCatalogStatRow {
            id: utf8("focus"),
            minimum_bits: 0,
            maximum_bits: 10.0f64.to_bits(),
        }];
        let tracks = [NativeContinuousMechanicsCatalogTrackRow {
            id: utf8("health"),
            minimum_bits: 0,
            maximum_kind: NativeContinuousMechanicsTrackMaximumKind::Fixed,
            fixed_maximum_bits: 10.0f64.to_bits(),
            maximum_stat: utf8(""),
        }];
        let sources = [NativeContinuousMechanicsCatalogSourceRow {
            id: utf8("aura"),
            priority: 0,
            contributions_start: 0,
            contributions_len: 0,
        }];
        let effects = [NativeContinuousMechanicsCatalogEffectRow {
            id: utf8("bless"),
            sources_start: 0,
            sources_len: 1,
        }];
        let effect_sources = [NativeContinuousMechanicsCatalogEffectSourceRow {
            source: utf8("aura"),
        }];
        let create = NativeContinuousMechanicsCatalogCreateRequest {
            version: utf8("continuous-v1"),
            stats: stats.as_ptr(),
            stats_len: 1,
            tracks: tracks.as_ptr(),
            tracks_len: 1,
            sources: sources.as_ptr(),
            sources_len: 1,
            contributions: std::ptr::null(),
            contributions_len: 0,
            effects: effects.as_ptr(),
            effects_len: 1,
            effect_sources: effect_sources.as_ptr(),
            effect_sources_len: 1,
        };
        let mut continuous_catalog = NativeContinuousMechanicsCatalogHandle::default();
        assert_eq!(
            unsafe { create_catalog(context, &create, &mut continuous_catalog) },
            ABI_OK
        );
        let initial_stats = [NativeContinuousMechanicsInitialStatRow {
            stat: utf8("focus"),
            base_bits: 1,
        }];
        let initial_tracks = [NativeContinuousMechanicsInitialTrackRow {
            track: utf8("health"),
            current_bits: (-0.0f64).to_bits(),
        }];
        let initial_sources = [NativeContinuousMechanicsInitialIntrinsicSourceRow {
            instance: utf8("origin"),
            definition: utf8("aura"),
        }];
        let initial_effects = [NativeContinuousMechanicsInitialActiveEffectRow {
            instance: utf8("bless-1"),
            definition: utf8("bless"),
        }];
        let initial = NativeContinuousMechanicsInitialComponentsRequest {
            catalog: continuous_catalog,
            entity,
            has_stats: true,
            stats: initial_stats.as_ptr(),
            stats_len: 1,
            has_tracks: true,
            tracks: initial_tracks.as_ptr(),
            tracks_len: 1,
            has_intrinsic_sources: true,
            intrinsic_sources: initial_sources.as_ptr(),
            intrinsic_sources_len: 1,
            has_active_effects: true,
            active_effects: initial_effects.as_ptr(),
            active_effects_len: 1,
        };
        assert_eq!(unsafe { set_initial_components(context, &initial) }, ABI_OK);

        let export_request = NativeContinuousMechanicsWorldExportRequest {
            mechanics_catalog: NativeMechanicsCatalogHandle { value: 1 },
            continuous_catalog,
        };
        let mut exported =
            std::mem::MaybeUninit::<NativeContinuousMechanicsWorldExportLease>::uninit();
        assert_eq!(
            unsafe { export_world(context, &export_request, exported.as_mut_ptr()) },
            ABI_OK
        );
        let exported = unsafe { exported.assume_init() };
        assert_eq!(exported.component_presence_len, 4);
        let exported_tracks =
            unsafe { std::slice::from_raw_parts(exported.tracks, exported.tracks_len) };
        assert_eq!(
            exported_tracks[0].current_bits, 0,
            "negative zero is normalized before export"
        );
        assert_eq!(
            unsafe { destroy_world_export_lease(context, exported.handle) },
            ABI_OK
        );

        let entities = [NativeMechanicsWorldEntityRow {
            entity_id: 7439,
            identity: utf8("continuous-hero"),
            lifecycle: NativeMechanicsEntityLifecycle::Active,
            lifecycle_stamp: 1,
        }];
        let exact_presence = NativeMechanicsRevisionComponent::all().map(|component| {
            NativeMechanicsWorldComponentPresenceRow {
                entity_id: 7439,
                component,
                present: false,
                revision: 0,
            }
        });
        let exact_import = NativeMechanicsWorldImportRequest {
            catalog: NativeMechanicsCatalogHandle { value: 1 },
            state_revision: exported.mechanics_state_revision,
            catalog_version: utf8("exact-v1"),
            catalog_fingerprint: utf8(bridge.catalogs[&1].catalog.as_ref().unwrap().fingerprint()),
            entities: entities.as_ptr(),
            entities_len: 1,
            containment: std::ptr::null(),
            containment_len: 0,
            component_presence: exact_presence.as_ptr(),
            component_presence_len: exact_presence.len(),
            stats: std::ptr::null(),
            stats_len: 0,
            tracks: std::ptr::null(),
            tracks_len: 0,
            intrinsic_sources: std::ptr::null(),
            intrinsic_sources_len: 0,
            active_effects: std::ptr::null(),
            active_effects_len: 0,
            inventory_stacks: std::ptr::null(),
            inventory_stacks_len: 0,
            inventory_capacity_limits: std::ptr::null(),
            inventory_capacity_limits_len: 0,
            items: std::ptr::null(),
            items_len: 0,
            equipment_assignments: std::ptr::null(),
            equipment_assignments_len: 0,
        };
        let mut import = NativeMechanicsWorldImportHandle::default();
        assert_eq!(
            unsafe { super::super::prepare_world_import(context, &exact_import, &mut import) },
            ABI_OK
        );

        let presence = NativeContinuousMechanicsComponentKind::all().map(|component| {
            NativeContinuousMechanicsWorldComponentPresenceRow {
                entity_id: 7439,
                component,
                present: true,
                revision: 0,
            }
        });
        let stage_stats = [NativeContinuousMechanicsWorldStatRow {
            entity_id: 7439,
            stat: utf8("focus"),
            base_bits: 1,
        }];
        let stage_tracks = [NativeContinuousMechanicsWorldTrackRow {
            entity_id: 7439,
            track: utf8("health"),
            current_bits: (-0.0f64).to_bits(),
        }];
        let stage_sources = [NativeContinuousMechanicsWorldIntrinsicSourceRow {
            entity_id: 7439,
            instance: utf8("origin"),
            definition: utf8("aura"),
        }];
        let stage_effects = [NativeContinuousMechanicsWorldActiveEffectRow {
            entity_id: 7439,
            instance: utf8("bless-1"),
            definition: utf8("bless"),
        }];
        let stage =
            |import, mechanics_state_revision| NativeContinuousMechanicsWorldImportStageRequest {
                import,
                mechanics_catalog: NativeMechanicsCatalogHandle { value: 1 },
                mechanics_state_revision,
                continuous_catalog,
                continuous_catalog_version: utf8("continuous-v1"),
                continuous_catalog_fingerprint: utf8(
                    bridge.continuous.catalogs[&continuous_catalog.value].fingerprint(),
                ),
                component_presence: presence.as_ptr(),
                component_presence_len: presence.len(),
                stats: stage_stats.as_ptr(),
                stats_len: 1,
                tracks: stage_tracks.as_ptr(),
                tracks_len: 1,
                intrinsic_sources: stage_sources.as_ptr(),
                intrinsic_sources_len: 1,
                active_effects: stage_effects.as_ptr(),
                active_effects_len: 1,
            };
        let mut rejected =
            std::mem::MaybeUninit::<NativeContinuousMechanicsWorldImportLease>::uninit();
        let mut catalog_mismatch = stage(import, exported.mechanics_state_revision);
        catalog_mismatch.continuous_catalog_version = utf8("wrong-continuous-catalog");
        assert_eq!(
            unsafe { stage_world_import(context, &catalog_mismatch, rejected.as_mut_ptr()) },
            0,
            "catalog mismatch preserves the exact candidate"
        );
        assert_eq!(
            unsafe {
                stage_world_import(
                    context,
                    &stage(import, exported.mechanics_state_revision + 1),
                    rejected.as_mut_ptr(),
                )
            },
            0,
            "catalog/state mismatch preserves the exact candidate"
        );
        let mut staged =
            std::mem::MaybeUninit::<NativeContinuousMechanicsWorldImportLease>::uninit();
        assert_eq!(
            unsafe {
                stage_world_import(
                    context,
                    &stage(import, exported.mechanics_state_revision),
                    staged.as_mut_ptr(),
                )
            },
            ABI_OK
        );
        let staged = unsafe { staged.assume_init() };
        let remaps = unsafe { std::slice::from_raw_parts(staged.revisions, staged.revisions_len) };
        assert_eq!(remaps.len(), 4);
        assert!(remaps
            .iter()
            .all(|row| row.restored_revision > row.snapshot_revision
                && row.restored_revision > row.current_revision));
        assert_eq!(
            unsafe { destroy_world_import_lease(context, staged.handle) },
            ABI_OK
        );
        assert_eq!(
            unsafe { super::super::publish_world_import(context, import) },
            ABI_OK
        );
        assert_eq!(
            bridge.continuous.associations.get(&EntityId::new(7439)),
            Some(&continuous_catalog.value)
        );
        assert_eq!(
            bridge.catalogs[&1]
                .world
                .state
                .component::<ContinuousTracksComponent>(EntityId::new(7439))
                .unwrap()
                .unwrap()
                .values()[0]
                .current()
                .bits(),
            0
        );

        let mut canceled = NativeMechanicsWorldImportHandle::default();
        assert_eq!(
            unsafe { super::super::prepare_world_import(context, &exact_import, &mut canceled) },
            ABI_OK
        );
        let mut canceled_stage =
            std::mem::MaybeUninit::<NativeContinuousMechanicsWorldImportLease>::uninit();
        assert_eq!(
            unsafe {
                stage_world_import(
                    context,
                    &stage(canceled, exported.mechanics_state_revision),
                    canceled_stage.as_mut_ptr(),
                )
            },
            ABI_OK
        );
        let canceled_stage = unsafe { canceled_stage.assume_init() };
        assert_eq!(
            unsafe { destroy_world_import_lease(context, canceled_stage.handle) },
            ABI_OK
        );
        assert_eq!(
            unsafe { super::super::destroy_world_import(context, canceled) },
            ABI_OK
        );
        assert_eq!(
            bridge.continuous.associations.get(&EntityId::new(7439)),
            Some(&continuous_catalog.value),
            "cancellation leaves live continuous state untouched"
        );

        let mut cleanup = NativeMechanicsWorldImportHandle::default();
        assert_eq!(
            unsafe { super::super::prepare_world_import(context, &exact_import, &mut cleanup) },
            ABI_OK
        );
        assert_eq!(
            unsafe { super::super::publish_world_import(context, cleanup) },
            ABI_OK
        );
        assert!(
            !bridge
                .continuous
                .associations
                .contains_key(&EntityId::new(7439)),
            "exact-only publication clears stale continuous association"
        );
    }
}
