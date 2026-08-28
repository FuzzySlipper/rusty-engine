use std::{collections::BTreeMap, ffi::c_void};

use core_ids::EntityId;
use csharp_engine_abi::*;
use entity_state::{
    ComponentRevision, EntityAuthoringService, EntityDefinition, EntityState, RelationshipCommand,
};
use gameplay_mechanics::{
    gameplay_component_registry, validate_state_against_catalog, ActiveEffectInstance,
    ActiveEffectsComponent, CapacityMetricDefinition, CatalogVersion, DamageFact,
    DamageKindDefinition, DamageKindSelector, DamagePart, DamagePartReceipt, DamageReceipt,
    DamageRequest, DamageResponseDefinition, DamageService, DecisionOutcome, EffectApplyRequest,
    EffectDefinition, EffectMutationKind, EffectRefreshRequest, EffectRemovalRequest,
    EffectReplaceRequest, EffectService, EffectSourceActivation, EffectStackingPolicy,
    EquipmentAssignment, EquipmentComponent, EquipmentEquipRequest, EquipmentMutationKind,
    EquipmentService, EquipmentSlotChange, EquipmentSlotDefinition, EquipmentSwapRequest,
    EquipmentUnequipRequest, ExactRatio, IntrinsicSourceBinding, IntrinsicSourcesComponent,
    InventoryCapacityLimit, InventoryComponent, InventoryMutationKind, InventoryMutationRequest,
    InventoryReadCost, InventoryService, InventoryTransferRequest, ItemCapacityCost, ItemComponent,
    ItemDefinition, ItemDestroyRequest, ItemEquipmentPolicy, ItemKind, ItemService, ItemStack,
    ItemTransferRequest, MechanicsCatalog, MechanicsCatalogDefinition, MechanicsComponentKind,
    MechanicsScalar, ObservedComponentRevision, OperationId, RequestSource, ResponseDecision,
    ResponseDecisionKind, RoundingPolicy, SourceCollectionCost, SourceDefinition,
    SourceDefinitionId, SourceInstanceId, SourceInstanceIdentity, StackingGroupId, StackingPolicy,
    StatBaseMutationRequest, StatContribution, StatContributionDefinition, StatDecision,
    StatDefinition, StatId, StatService, StatValue, StatsComponent, TrackAdjustmentKind,
    TrackDamageChange, TrackDefinition, TrackMaximum, TrackMutationRequest, TrackReadReceipt,
    TrackReconciliationPolicy, TrackReconciliationRequest, TrackService, TrackSetPolicy,
    TrackSetRequest, TrackValue, TracksComponent, UniqueItemMaterializationRequest,
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

#[derive(Clone)]
struct CatalogSlot {
    builder: Option<CatalogBuilder>,
    catalog: Option<MechanicsCatalog>,
    world: MechanicsWorld,
}

/// The rows and every UTF-8 allocation returned from catalog inspection live in one
/// service-owned box until the matching catalog lease is explicitly destroyed.
struct CatalogLeaseBacking {
    _text: Vec<String>,
    rows: CatalogLeaseRows,
}

enum CatalogLeaseRows {
    Identity(Vec<NativeMechanicsCatalogIdentityRow>),
    Stats(Vec<NativeMechanicsStatCatalogRow>),
    Tracks(Vec<NativeMechanicsTrackCatalogRow>),
    Sources(Vec<NativeMechanicsSourceCatalogRow>),
    StatContributions(Vec<NativeMechanicsStatContributionCatalogRow>),
    DamageKinds(Vec<NativeMechanicsDamageKindCatalogRow>),
    DamageResponses(Vec<NativeMechanicsDamageResponseCatalogRow>),
    Effects(Vec<NativeMechanicsEffectCatalogRow>),
    EffectSources(Vec<NativeMechanicsEffectSourceCatalogRow>),
    CapacityMetrics(Vec<NativeMechanicsCapacityMetricCatalogRow>),
    Items(Vec<NativeMechanicsItemCatalogRow>),
    ItemClassifications(Vec<NativeMechanicsItemClassificationCatalogRow>),
    ItemCapacityCosts(Vec<NativeMechanicsItemCapacityCostCatalogRow>),
    ItemEquipmentPolicies(Vec<NativeMechanicsItemEquipmentPolicyCatalogRow>),
    ItemSources(Vec<NativeMechanicsItemSourceCatalogRow>),
    EquipmentSlots(Vec<NativeMechanicsEquipmentSlotCatalogRow>),
    SlotClassifications(Vec<NativeMechanicsSlotClassificationCatalogRow>),
}

/// The rows and every UTF-8 allocation returned from component inspection live in one
/// service-owned box until the matching component lease is explicitly destroyed.
struct ComponentLeaseBacking {
    _text: Vec<String>,
    rows: ComponentLeaseRows,
}

enum ComponentLeaseRows {
    Stats(Vec<NativeMechanicsStatComponentRow>),
    Tracks(Vec<NativeMechanicsTrackComponentRow>),
    IntrinsicSources(Vec<NativeMechanicsIntrinsicSourceComponentRow>),
    ActiveEffects(Vec<NativeMechanicsActiveEffectComponentRow>),
    InventoryStacks(Vec<NativeMechanicsInventoryStackComponentRow>),
    InventoryCapacityLimits(Vec<NativeMechanicsInventoryCapacityLimitComponentRow>),
    Items(Vec<NativeMechanicsItemComponentRow>),
    EquipmentAssignments(Vec<NativeMechanicsEquipmentAssignmentComponentRow>),
}

/// Exact runtime-operation rows and their borrowed text share one owner. This
/// is distinct from catalog/component inspection because operation receipts
/// may contain several semantically different collections.
struct OperationLeaseBacking {
    _text: Vec<String>,
    rows: OperationLeaseRows,
}

/// One bounded diagnostic is retained per failed Mechanics ABI callback until
/// the generated facade copies it and invokes `destroy_operation_diagnostic_lease`.
/// This deliberately shares the engine-wide receipt contract rather than
/// introducing ambient Mechanics error state.
struct MechanicsOperationDiagnosticLease {
    _code: Box<[u8]>,
    _message: Box<[u8]>,
    _source: Box<[u8]>,
    diagnostics: Box<[NativeEngineDiagnostic]>,
}

impl MechanicsOperationDiagnosticLease {
    fn new(code: &'static [u8], message: &'static [u8], source: &str) -> Self {
        let code: Box<[u8]> = code.into();
        let message: Box<[u8]> = message.into();
        let source: Box<[u8]> = source.as_bytes().into();
        let diagnostics = vec![NativeEngineDiagnostic {
            code: native_utf8(&code),
            message: native_utf8(&message),
            source: native_utf8(&source),
        }]
        .into_boxed_slice();
        Self {
            _code: code,
            _message: message,
            _source: source,
            diagnostics,
        }
    }
}

fn native_utf8(bytes: &[u8]) -> NativeUtf8Slice {
    NativeUtf8Slice {
        bytes: bytes.as_ptr(),
        len: bytes.len(),
    }
}

enum OperationLeaseRows {
    StatEvaluation {
        decisions: Vec<NativeMechanicsStatDecisionRow>,
        observed_revisions: Vec<NativeMechanicsObservedComponentRevisionRow>,
    },
    StatMutation {
        observed_revisions: Vec<NativeMechanicsObservedComponentRevisionRow>,
    },
    Track {
        observed_revisions: Vec<NativeMechanicsObservedComponentRevisionRow>,
    },
    InventoryView {
        stacks: Vec<NativeMechanicsInventoryViewStackRow>,
        unique_items: Vec<NativeMechanicsInventoryViewUniqueItemRow>,
        capacity: Vec<NativeMechanicsInventoryViewCapacityUsageRow>,
    },
    InventoryMutation {
        capacity_before: Vec<NativeMechanicsInventoryViewCapacityUsageRow>,
        capacity_after: Vec<NativeMechanicsInventoryViewCapacityUsageRow>,
    },
    InventoryTransfer {
        from_capacity_before: Vec<NativeMechanicsInventoryViewCapacityUsageRow>,
        from_capacity_after: Vec<NativeMechanicsInventoryViewCapacityUsageRow>,
        to_capacity_before: Vec<NativeMechanicsInventoryViewCapacityUsageRow>,
        to_capacity_after: Vec<NativeMechanicsInventoryViewCapacityUsageRow>,
    },
    UniqueItemTransfer {
        from_capacity_before: Vec<NativeMechanicsInventoryViewCapacityUsageRow>,
        from_capacity_after: Vec<NativeMechanicsInventoryViewCapacityUsageRow>,
        to_capacity_before: Vec<NativeMechanicsInventoryViewCapacityUsageRow>,
        to_capacity_after: Vec<NativeMechanicsInventoryViewCapacityUsageRow>,
    },
    UniqueItemMaterialization,
    UniqueItemDestroy,
    EquipmentMutation {
        changes: Vec<NativeMechanicsEquipmentSlotChangeRow>,
        observed_item_revisions: Vec<NativeMechanicsObservedComponentRevisionRow>,
        observed_revisions: Vec<NativeMechanicsObservedComponentRevisionRow>,
    },
    Effect {
        removed: Vec<NativeMechanicsActiveEffectComponentRow>,
        activated_sources: Vec<NativeMechanicsEffectSourceActivationRow>,
        observed_revisions: Vec<NativeMechanicsObservedComponentRevisionRow>,
    },
    Damage {
        parts: Vec<NativeMechanicsDamagePartReceiptRow>,
        decisions: Vec<NativeMechanicsDamageDecisionRow>,
        track_changes: Vec<NativeMechanicsTrackDamageChangeRow>,
        protection_track_depletions: Vec<NativeMechanicsTrackDepletionRow>,
        target_track_depletions: Vec<NativeMechanicsTrackDepletionRow>,
        observed_revisions: Vec<NativeMechanicsObservedComponentRevisionRow>,
    },
}

#[derive(Default)]
struct CatalogLeaseText {
    values: Vec<String>,
}

impl CatalogLeaseText {
    fn copy(&mut self, value: impl AsRef<str>) -> NativeUtf8Slice {
        self.values.push(value.as_ref().to_owned());
        let value = self
            .values
            .last()
            .expect("just inserted catalog lease text");
        NativeUtf8Slice {
            bytes: value.as_ptr(),
            len: value.len(),
        }
    }
}

/// Catalog-scoped mechanism storage keyed by the product's canonical EntityWorld identity.
/// The bridge never allocates product entity identifiers: it only mirrors supplied ones.
#[derive(Clone)]
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
    initial_containment: Vec<EntityId>,
    expected_state_revision: Option<u64>,
    initial_components_set: bool,
    committed: bool,
}

/// The in-process checkpoint deliberately retains typed Engine state. It is not a persistence
/// format and therefore carries no schema/migration contract.
struct MechanicsWorldSnapshot {
    catalog: u64,
    catalog_fingerprint: String,
    world: MechanicsWorld,
    canonical_membership: BTreeMap<EntityId, u64>,
    canonical_identity: BTreeMap<EntityId, String>,
    binding_topology: BTreeMap<u64, (u64, EntityId, bool)>,
}

/// A validated native candidate plus the bounded facts the managed composition needs before its
/// own non-fallible assignment. Rows borrow only from this owner until publish/destroy.
struct PreparedMechanicsWorldRestore {
    catalog: u64,
    state_revision_before: u64,
    candidate: Option<MechanicsWorld>,
    revisions: Vec<NativeMechanicsRevisionRemapRow>,
    lifecycles: Vec<NativeMechanicsLifecycleReceipt>,
    published: bool,
}

pub(crate) struct RuntimeMechanicsBridge {
    catalogs: BTreeMap<u64, CatalogSlot>,
    entities: BTreeMap<u64, EntityBinding>,
    /// A product canonical entity is admitted into at most one mechanics catalog world.
    canonical_entities: BTreeMap<EntityId, u64>,
    catalog_leases: BTreeMap<u64, Box<CatalogLeaseBacking>>,
    component_leases: BTreeMap<u64, Box<ComponentLeaseBacking>>,
    world_snapshots: BTreeMap<u64, Box<MechanicsWorldSnapshot>>,
    world_snapshot_leases: BTreeMap<u64, u64>,
    prepared_world_restores: BTreeMap<u64, Box<PreparedMechanicsWorldRestore>>,
    world_restore_leases: BTreeMap<u64, u64>,
    operation_leases: BTreeMap<u64, Box<OperationLeaseBacking>>,
    diagnostic_leases: BTreeMap<u64, MechanicsOperationDiagnosticLease>,
    next_catalog: u64,
    next_entity: u64,
    next_catalog_lease: u64,
    next_component_lease: u64,
    next_world_snapshot: u64,
    next_world_snapshot_lease: u64,
    next_world_restore: u64,
    next_world_restore_lease: u64,
    next_operation_lease: u64,
    next_diagnostic_lease: u64,
}

impl RuntimeMechanicsBridge {
    pub(crate) fn new() -> Self {
        Self {
            catalogs: BTreeMap::new(),
            entities: BTreeMap::new(),
            canonical_entities: BTreeMap::new(),
            catalog_leases: BTreeMap::new(),
            component_leases: BTreeMap::new(),
            world_snapshots: BTreeMap::new(),
            world_snapshot_leases: BTreeMap::new(),
            prepared_world_restores: BTreeMap::new(),
            world_restore_leases: BTreeMap::new(),
            operation_leases: BTreeMap::new(),
            diagnostic_leases: BTreeMap::new(),
            next_catalog: 1,
            next_entity: 1,
            next_catalog_lease: 1,
            next_component_lease: 1,
            next_world_snapshot: 1,
            next_world_snapshot_lease: 1,
            next_world_restore: 1,
            next_world_restore_lease: 1,
            next_operation_lease: 1,
            next_diagnostic_lease: 1,
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

    fn insert_catalog_lease(
        &mut self,
        backing: CatalogLeaseBacking,
    ) -> Option<NativeMechanicsCatalogLeaseHandle> {
        let value = self.next_catalog_lease;
        self.next_catalog_lease = value.checked_add(1)?;
        self.catalog_leases.insert(value, Box::new(backing));
        Some(NativeMechanicsCatalogLeaseHandle { value })
    }

    fn insert_component_lease(
        &mut self,
        backing: ComponentLeaseBacking,
    ) -> Option<NativeMechanicsComponentLeaseHandle> {
        let value = self.next_component_lease;
        self.next_component_lease = value.checked_add(1)?;
        self.component_leases.insert(value, Box::new(backing));
        Some(NativeMechanicsComponentLeaseHandle { value })
    }

    fn insert_operation_lease(
        &mut self,
        backing: OperationLeaseBacking,
    ) -> Option<NativeMechanicsOperationLeaseHandle> {
        let value = self.next_operation_lease;
        self.next_operation_lease = value.checked_add(1)?;
        self.operation_leases.insert(value, Box::new(backing));
        Some(NativeMechanicsOperationLeaseHandle { value })
    }

    fn retain_operation_diagnostic(
        &mut self,
        code: &'static [u8],
        message: &'static [u8],
        source: &str,
    ) -> Option<NativeEngineDiagnosticLease> {
        let value = self.next_diagnostic_lease;
        self.next_diagnostic_lease = value.checked_add(1)?;
        let lease = MechanicsOperationDiagnosticLease::new(code, message, source);
        let diagnostics = NativeEngineDiagnosticLease {
            handle: NativeEngineDiagnosticLeaseHandle { value },
            diagnostics: lease.diagnostics.as_ptr(),
            diagnostics_len: lease.diagnostics.len(),
        };
        self.diagnostic_leases.insert(value, lease);
        Some(diagnostics)
    }

    fn destroy_operation_diagnostic_lease(
        &mut self,
        handle: NativeEngineDiagnosticLeaseHandle,
    ) -> bool {
        handle.value != 0 && self.diagnostic_leases.remove(&handle.value).is_some()
    }
}

pub(crate) fn api(bridge: &mut RuntimeMechanicsBridge) -> NativeMechanicsApi {
    NativeMechanicsApi {
        context: (bridge as *mut RuntimeMechanicsBridge).cast(),
        create_catalog: receipt_create_catalog,
        define_stat: receipt_define_stat,
        define_track: receipt_define_track,
        define_contribution: receipt_define_contribution,
        define_source: receipt_define_source,
        define_damage_kind: receipt_define_damage_kind,
        define_damage_response: receipt_define_damage_response,
        define_effect: receipt_define_effect,
        define_capacity_metric: receipt_define_capacity_metric,
        define_item: receipt_define_item,
        define_equipment_slot: receipt_define_equipment_slot,
        admit_catalog: receipt_admit_catalog,
        destroy_catalog: receipt_destroy_catalog,
        read_catalog_identity: receipt_read_catalog_identity,
        read_catalog_stats: receipt_read_catalog_stats,
        read_catalog_tracks: receipt_read_catalog_tracks,
        read_catalog_sources: receipt_read_catalog_sources,
        read_catalog_stat_contributions: receipt_read_catalog_stat_contributions,
        read_catalog_damage_kinds: receipt_read_catalog_damage_kinds,
        read_catalog_damage_responses: receipt_read_catalog_damage_responses,
        read_catalog_effects: receipt_read_catalog_effects,
        read_catalog_effect_sources: receipt_read_catalog_effect_sources,
        read_catalog_capacity_metrics: receipt_read_catalog_capacity_metrics,
        read_catalog_items: receipt_read_catalog_items,
        read_catalog_item_classifications: receipt_read_catalog_item_classifications,
        read_catalog_item_capacity_costs: receipt_read_catalog_item_capacity_costs,
        read_catalog_item_equipment_policies: receipt_read_catalog_item_equipment_policies,
        read_catalog_item_sources: receipt_read_catalog_item_sources,
        read_catalog_equipment_slots: receipt_read_catalog_equipment_slots,
        read_catalog_slot_classifications: receipt_read_catalog_slot_classifications,
        destroy_catalog_lease,
        read_stat_component: receipt_read_stat_component,
        read_track_component: receipt_read_track_component,
        read_intrinsic_source_component: receipt_read_intrinsic_source_component,
        read_active_effect_component: receipt_read_active_effect_component,
        read_inventory_stack_component: receipt_read_inventory_stack_component,
        read_inventory_capacity_limit_component: receipt_read_inventory_capacity_limit_component,
        read_item_component: receipt_read_item_component,
        read_equipment_assignment_component: receipt_read_equipment_assignment_component,
        destroy_component_lease,
        capture_world_snapshot: receipt_capture_world_snapshot,
        destroy_world_snapshot,
        read_world_snapshot: receipt_read_world_snapshot,
        destroy_world_snapshot_lease,
        prepare_world_restore: receipt_prepare_world_restore,
        destroy_world_restore,
        read_world_restore: receipt_read_world_restore,
        destroy_world_restore_lease,
        publish_world_restore,
        bind_entity: receipt_bind_entity,
        rebind_entity: receipt_rebind_entity,
        set_initial_stat: receipt_set_initial_stat,
        set_initial_track: receipt_set_initial_track,
        bind_intrinsic_source: receipt_bind_intrinsic_source,
        set_initial_components: receipt_set_initial_components,
        stage_initial_containment: receipt_stage_initial_containment,
        read_containment: receipt_read_containment,
        commit_entity: receipt_commit_entity,
        set_entity_lifecycle: receipt_set_entity_lifecycle,
        destroy_entity: receipt_destroy_entity,
        read_stat: receipt_read_stat,
        evaluate_stat: receipt_evaluate_stat,
        read_track: receipt_read_track,
        read_inventory_view: receipt_read_inventory_view,
        grant_inventory: receipt_grant_inventory,
        consume_inventory: receipt_consume_inventory,
        transfer_inventory: receipt_transfer_inventory,
        transfer_unique_item: receipt_transfer_unique_item,
        materialize_unique_item: receipt_materialize_unique_item,
        destroy_unique_item: receipt_destroy_unique_item,
        equip_equipment: receipt_equip_equipment,
        unequip_equipment: receipt_unequip_equipment,
        swap_equipment: receipt_swap_equipment,
        set_stat_base: receipt_set_stat_base,
        destroy_operation_lease,
        set_track: receipt_set_track,
        spend_track: receipt_spend_track,
        restore_track: receipt_restore_track,
        reconcile_track: receipt_reconcile_track,
        apply_effect: receipt_apply_effect,
        refresh_effect: receipt_refresh_effect,
        replace_effect: receipt_replace_effect,
        remove_effect: receipt_remove_effect,
        expire_effect: receipt_expire_effect,
        preview_damage: receipt_preview_damage,
        apply_damage: receipt_apply_damage,
        destroy_operation_diagnostic_lease,
    }
}

unsafe fn invoke_with_operation_diagnostic(
    context: *mut c_void,
    receipt: *mut NativeOperationErrorReceipt,
    operation: &'static [u8],
    callback: impl FnOnce() -> i32,
    diagnostic: impl FnOnce(&RuntimeMechanicsBridge) -> (&'static [u8], &'static [u8], String),
) -> i32 {
    if receipt.is_null() {
        return 0;
    }
    // SAFETY: receipt is borrowed only for this direct callback and starts with
    // no retained diagnostic on every observable path.
    unsafe { *receipt = std::mem::zeroed() };
    let status = callback();
    if status != ABI_OK && !context.is_null() {
        // SAFETY: every Mechanics callback uses the stable bridge context for
        // the product lifetime. The inner callback has returned before this
        // independent diagnostic lease is retained.
        let bridge = unsafe { &mut *context.cast::<RuntimeMechanicsBridge>() };
        let (code, message, source) = diagnostic(bridge);
        if let Some(diagnostics) = bridge.retain_operation_diagnostic(code, message, &source) {
            // SAFETY: receipt remains valid for this direct callback.
            unsafe {
                *receipt = NativeOperationErrorReceipt {
                    service: native_utf8(b"Mechanics"),
                    operation: native_utf8(operation),
                    status: 0,
                    diagnostics,
                };
            }
        }
    }
    status
}

macro_rules! mechanics_callback {
    ($name:ident($($argument:ident : $ty:ty),*) => $inner:path, $operation:literal, $source:expr) => {
        unsafe extern "C" fn $name(
            context: *mut c_void,
            $($argument: $ty,)*
            receipt: *mut NativeOperationErrorReceipt,
        ) -> i32 {
            // SAFETY: the ABI callback forwards its direct-call borrows to the
            // pre-existing Mechanics implementation before retaining a copied
            // diagnostic only on failure.
            unsafe {
                invoke_with_operation_diagnostic(
                    context,
                    receipt,
                    $operation,
                    || $inner(context, $($argument),*),
                    |_| (
                        b"MECHANICS_OPERATION_FAILED",
                        b"Mechanics operation failed.",
                        $source,
                    ),
                )
            }
        }
    };
}

fn catalog_source(handle: NativeMechanicsCatalogHandle) -> String {
    format!("catalog:{}", handle.value)
}

unsafe fn bind_request_source(request: *const NativeMechanicsEntityBindRequest) -> String {
    // SAFETY: the pointer is borrowed by the direct callback; avoid
    // dereferencing null so malformed pointer calls retain a bounded generic
    // diagnostic instead of a new validation taxonomy.
    unsafe {
        request
            .as_ref()
            .map(|value| format!("catalog:{} entity:{}", value.catalog.value, value.entity_id))
            .unwrap_or_default()
    }
}

unsafe fn rebind_request_source(request: *const NativeMechanicsEntityRebindRequest) -> String {
    unsafe {
        request
            .as_ref()
            .map(|value| {
                format!(
                    "catalog:{} entity:{} stamp:{}",
                    value.catalog.value, value.entity_id, value.expected_stamp
                )
            })
            .unwrap_or_default()
    }
}

unsafe fn lifecycle_failure_diagnostic(
    bridge: &RuntimeMechanicsBridge,
    request: *const NativeMechanicsLifecycleRequest,
) -> (&'static [u8], &'static [u8], String) {
    let Some(request) = (unsafe { request.as_ref() }) else {
        return (
            b"MECHANICS_OPERATION_FAILED",
            b"Mechanics operation failed.",
            String::new(),
        );
    };
    let Some(binding) = bridge.binding(request.entity) else {
        return (
            b"MECHANICS_ENTITY_NOT_FOUND",
            b"Mechanics entity was not found.",
            format!("entity-handle:{}", request.entity.value),
        );
    };
    let Some(slot) = bridge.catalogs.get(&binding.catalog) else {
        return (
            b"MECHANICS_CATALOG_NOT_FOUND",
            b"Mechanics catalog was not found.",
            format!(
                "catalog:{} entity:{}",
                binding.catalog,
                binding.entity.raw()
            ),
        );
    };
    let observed = slot.world.lifecycle_receipt(binding.entity);
    if matches!(request.guard, NativeMechanicsLifecycleGuard::Exact)
        && request.expected_stamp != observed.stamp
    {
        return (
            b"MECHANICS_LIFECYCLE_STALE",
            b"Mechanics entity lifecycle stamp was stale.",
            format!(
                "entity:{} expected-stamp:{} observed-stamp:{}",
                binding.entity.raw(),
                request.expected_stamp,
                observed.stamp,
            ),
        );
    }
    (
        b"MECHANICS_OPERATION_FAILED",
        b"Mechanics operation failed.",
        format!(
            "entity:{} lifecycle:{:?}",
            binding.entity.raw(),
            observed.lifecycle
        ),
    )
}

unsafe fn track_set_failure_diagnostic(
    bridge: &RuntimeMechanicsBridge,
    request: *const NativeMechanicsTrackSetRequest,
) -> (&'static [u8], &'static [u8], String) {
    let Some(request) = (unsafe { request.as_ref() }) else {
        return (
            b"MECHANICS_OPERATION_FAILED",
            b"Mechanics operation failed.",
            String::new(),
        );
    };
    let Some(binding) = bridge.binding(request.entity) else {
        return (
            b"MECHANICS_ENTITY_NOT_FOUND",
            b"Mechanics entity was not found.",
            format!("entity-handle:{}", request.entity.value),
        );
    };
    let Some(slot) = bridge.catalogs.get(&binding.catalog) else {
        return (
            b"MECHANICS_CATALOG_NOT_FOUND",
            b"Mechanics catalog was not found.",
            format!(
                "catalog:{} entity:{}",
                binding.catalog,
                binding.entity.raw()
            ),
        );
    };
    if let Ok(actual) = slot
        .world
        .state
        .component_revision::<TracksComponent>(binding.entity)
    {
        if matches!(request.revision_guard, NativeMechanicsRevisionGuard::Exact)
            && (request.expected_revision.entity_id != binding.entity.raw()
                || request.expected_revision.component != NativeMechanicsRevisionComponent::Tracks
                || request.expected_revision.revision != actual.revision())
        {
            return (
                b"MECHANICS_REVISION_STALE",
                b"Mechanics component revision was stale.",
                format!(
                    "entity:{} component:Tracks expected-revision:{} observed-revision:{}",
                    binding.entity.raw(),
                    request.expected_revision.revision,
                    actual.revision(),
                ),
            );
        }
    }
    (
        b"MECHANICS_OPERATION_FAILED",
        b"Mechanics operation failed.",
        format!("entity:{} component:Tracks", binding.entity.raw()),
    )
}

mechanics_callback!(receipt_create_catalog(request: *const NativeMechanicsCatalogCreateRequest, result: *mut NativeMechanicsCatalogHandle) => create_catalog, b"CreateCatalog", String::new());
mechanics_callback!(receipt_define_stat(request: *const NativeMechanicsStatDefinitionRequest) => define_stat, b"DefineStat", String::new());
mechanics_callback!(receipt_define_track(request: *const NativeMechanicsTrackDefinitionRequest) => define_track, b"DefineTrack", String::new());
mechanics_callback!(receipt_define_contribution(request: *const NativeMechanicsContributionDefinitionRequest) => define_contribution, b"DefineContribution", String::new());
mechanics_callback!(receipt_define_source(request: *const NativeMechanicsSourceDefinitionRequest) => define_source, b"DefineSource", String::new());
mechanics_callback!(receipt_define_damage_kind(request: *const NativeMechanicsDamageKindDefinitionRequest) => define_damage_kind, b"DefineDamageKind", String::new());
mechanics_callback!(receipt_define_damage_response(request: *const NativeMechanicsDamageResponseDefinitionRequest) => define_damage_response, b"DefineDamageResponse", String::new());
mechanics_callback!(receipt_define_effect(request: *const NativeMechanicsEffectDefinitionRequest) => define_effect, b"DefineEffect", String::new());
mechanics_callback!(receipt_define_capacity_metric(request: *const NativeMechanicsCapacityMetricDefinitionRequest) => define_capacity_metric, b"DefineCapacityMetric", String::new());
mechanics_callback!(receipt_define_item(request: *const NativeMechanicsItemDefinitionRequest) => define_item, b"DefineItem", String::new());
mechanics_callback!(receipt_define_equipment_slot(request: *const NativeMechanicsEquipmentSlotDefinitionRequest) => define_equipment_slot, b"DefineEquipmentSlot", String::new());
unsafe extern "C" fn receipt_admit_catalog(
    context: *mut c_void,
    handle: NativeMechanicsCatalogHandle,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe {
        invoke_with_operation_diagnostic(
            context,
            receipt,
            b"AdmitCatalog",
            || admit_catalog(context, handle),
            |bridge| {
                let (code, message) = if bridge.catalogs.contains_key(&handle.value) {
                    (
                        b"MECHANICS_CATALOG_REJECTED".as_slice(),
                        b"Mechanics catalog admission was rejected.".as_slice(),
                    )
                } else {
                    (
                        b"MECHANICS_CATALOG_NOT_FOUND".as_slice(),
                        b"Mechanics catalog was not found.".as_slice(),
                    )
                };
                (code, message, catalog_source(handle))
            },
        )
    }
}
mechanics_callback!(receipt_destroy_catalog(handle: NativeMechanicsCatalogHandle) => destroy_catalog, b"DestroyCatalog", catalog_source(handle));
mechanics_callback!(receipt_read_catalog_identity(handle: NativeMechanicsCatalogHandle, result: *mut NativeMechanicsCatalogIdentityLease) => read_catalog_identity, b"ReadCatalogIdentity", catalog_source(handle));
mechanics_callback!(receipt_read_catalog_stats(handle: NativeMechanicsCatalogHandle, result: *mut NativeMechanicsStatCatalogLease) => read_catalog_stats, b"ReadCatalogStats", catalog_source(handle));
mechanics_callback!(receipt_read_catalog_tracks(handle: NativeMechanicsCatalogHandle, result: *mut NativeMechanicsTrackCatalogLease) => read_catalog_tracks, b"ReadCatalogTracks", catalog_source(handle));
mechanics_callback!(receipt_read_catalog_sources(handle: NativeMechanicsCatalogHandle, result: *mut NativeMechanicsSourceCatalogLease) => read_catalog_sources, b"ReadCatalogSources", catalog_source(handle));
mechanics_callback!(receipt_read_catalog_stat_contributions(handle: NativeMechanicsCatalogHandle, result: *mut NativeMechanicsStatContributionCatalogLease) => read_catalog_stat_contributions, b"ReadCatalogStatContributions", catalog_source(handle));
mechanics_callback!(receipt_read_catalog_damage_kinds(handle: NativeMechanicsCatalogHandle, result: *mut NativeMechanicsDamageKindCatalogLease) => read_catalog_damage_kinds, b"ReadCatalogDamageKinds", catalog_source(handle));
mechanics_callback!(receipt_read_catalog_damage_responses(handle: NativeMechanicsCatalogHandle, result: *mut NativeMechanicsDamageResponseCatalogLease) => read_catalog_damage_responses, b"ReadCatalogDamageResponses", catalog_source(handle));
mechanics_callback!(receipt_read_catalog_effects(handle: NativeMechanicsCatalogHandle, result: *mut NativeMechanicsEffectCatalogLease) => read_catalog_effects, b"ReadCatalogEffects", catalog_source(handle));
mechanics_callback!(receipt_read_catalog_effect_sources(handle: NativeMechanicsCatalogHandle, result: *mut NativeMechanicsEffectSourceCatalogLease) => read_catalog_effect_sources, b"ReadCatalogEffectSources", catalog_source(handle));
mechanics_callback!(receipt_read_catalog_capacity_metrics(handle: NativeMechanicsCatalogHandle, result: *mut NativeMechanicsCapacityMetricCatalogLease) => read_catalog_capacity_metrics, b"ReadCatalogCapacityMetrics", catalog_source(handle));
mechanics_callback!(receipt_read_catalog_items(handle: NativeMechanicsCatalogHandle, result: *mut NativeMechanicsItemCatalogLease) => read_catalog_items, b"ReadCatalogItems", catalog_source(handle));
mechanics_callback!(receipt_read_catalog_item_classifications(handle: NativeMechanicsCatalogHandle, result: *mut NativeMechanicsItemClassificationCatalogLease) => read_catalog_item_classifications, b"ReadCatalogItemClassifications", catalog_source(handle));
mechanics_callback!(receipt_read_catalog_item_capacity_costs(handle: NativeMechanicsCatalogHandle, result: *mut NativeMechanicsItemCapacityCostCatalogLease) => read_catalog_item_capacity_costs, b"ReadCatalogItemCapacityCosts", catalog_source(handle));
mechanics_callback!(receipt_read_catalog_item_equipment_policies(handle: NativeMechanicsCatalogHandle, result: *mut NativeMechanicsItemEquipmentPolicyCatalogLease) => read_catalog_item_equipment_policies, b"ReadCatalogItemEquipmentPolicies", catalog_source(handle));
mechanics_callback!(receipt_read_catalog_item_sources(handle: NativeMechanicsCatalogHandle, result: *mut NativeMechanicsItemSourceCatalogLease) => read_catalog_item_sources, b"ReadCatalogItemSources", catalog_source(handle));
mechanics_callback!(receipt_read_catalog_equipment_slots(handle: NativeMechanicsCatalogHandle, result: *mut NativeMechanicsEquipmentSlotCatalogLease) => read_catalog_equipment_slots, b"ReadCatalogEquipmentSlots", catalog_source(handle));
mechanics_callback!(receipt_read_catalog_slot_classifications(handle: NativeMechanicsCatalogHandle, result: *mut NativeMechanicsSlotClassificationCatalogLease) => read_catalog_slot_classifications, b"ReadCatalogSlotClassifications", catalog_source(handle));
unsafe extern "C" fn receipt_read_stat_component(
    context: *mut c_void,
    handle: NativeMechanicsEntityHandle,
    result: *mut NativeMechanicsStatComponentLease,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe {
        invoke_with_operation_diagnostic(
            context,
            receipt,
            b"ReadStatComponent",
            || read_stat_component(context, handle, result),
            |bridge| {
                let (code, message) = if bridge.binding(handle).is_some() {
                    (
                        b"MECHANICS_COMPONENT_UNAVAILABLE".as_slice(),
                        b"Mechanics component was unavailable for the entity.".as_slice(),
                    )
                } else {
                    (
                        b"MECHANICS_ENTITY_NOT_FOUND".as_slice(),
                        b"Mechanics entity was not found.".as_slice(),
                    )
                };
                (code, message, format!("entity-handle:{}", handle.value))
            },
        )
    }
}
mechanics_callback!(receipt_read_track_component(handle: NativeMechanicsEntityHandle, result: *mut NativeMechanicsTrackComponentLease) => read_track_component, b"ReadTrackComponent", format!("entity-handle:{}", handle.value));
mechanics_callback!(receipt_read_intrinsic_source_component(handle: NativeMechanicsEntityHandle, result: *mut NativeMechanicsIntrinsicSourceComponentLease) => read_intrinsic_source_component, b"ReadIntrinsicSourceComponent", format!("entity-handle:{}", handle.value));
mechanics_callback!(receipt_read_active_effect_component(handle: NativeMechanicsEntityHandle, result: *mut NativeMechanicsActiveEffectComponentLease) => read_active_effect_component, b"ReadActiveEffectComponent", format!("entity-handle:{}", handle.value));
mechanics_callback!(receipt_read_inventory_stack_component(handle: NativeMechanicsEntityHandle, result: *mut NativeMechanicsInventoryStackComponentLease) => read_inventory_stack_component, b"ReadInventoryStackComponent", format!("entity-handle:{}", handle.value));
mechanics_callback!(receipt_read_inventory_capacity_limit_component(handle: NativeMechanicsEntityHandle, result: *mut NativeMechanicsInventoryCapacityLimitComponentLease) => read_inventory_capacity_limit_component, b"ReadInventoryCapacityLimitComponent", format!("entity-handle:{}", handle.value));
mechanics_callback!(receipt_read_item_component(handle: NativeMechanicsEntityHandle, result: *mut NativeMechanicsItemComponentLease) => read_item_component, b"ReadItemComponent", format!("entity-handle:{}", handle.value));
mechanics_callback!(receipt_read_equipment_assignment_component(handle: NativeMechanicsEntityHandle, result: *mut NativeMechanicsEquipmentAssignmentComponentLease) => read_equipment_assignment_component, b"ReadEquipmentAssignmentComponent", format!("entity-handle:{}", handle.value));
mechanics_callback!(receipt_bind_entity(request: *const NativeMechanicsEntityBindRequest, result: *mut NativeMechanicsEntityHandle) => bind_entity, b"BindEntity", bind_request_source(request));
mechanics_callback!(receipt_rebind_entity(request: *const NativeMechanicsEntityRebindRequest, result: *mut NativeMechanicsEntityHandle) => rebind_entity, b"RebindEntity", rebind_request_source(request));
mechanics_callback!(receipt_set_initial_stat(request: *const NativeMechanicsInitialStatRequest) => set_initial_stat, b"SetInitialStat", String::new());
mechanics_callback!(receipt_set_initial_track(request: *const NativeMechanicsInitialTrackRequest) => set_initial_track, b"SetInitialTrack", String::new());
mechanics_callback!(receipt_bind_intrinsic_source(request: *const NativeMechanicsIntrinsicSourceRequest) => bind_intrinsic_source, b"BindIntrinsicSource", String::new());
mechanics_callback!(receipt_set_initial_components(request: *const NativeMechanicsInitialComponentsRequest) => set_initial_components, b"SetInitialComponents", String::new());
mechanics_callback!(receipt_stage_initial_containment(request: *const NativeMechanicsInitialContainmentRequest) => stage_initial_containment, b"StageInitialContainment", String::new());
mechanics_callback!(receipt_read_containment(request: *const NativeMechanicsContainmentReadRequest, result: *mut NativeMechanicsContainmentReceipt) => read_containment, b"ReadContainment", String::new());
mechanics_callback!(receipt_commit_entity(handle: NativeMechanicsEntityHandle, result: *mut NativeMechanicsEntityReceipt) => commit_entity, b"CommitEntity", format!("entity-handle:{}", handle.value));
unsafe extern "C" fn receipt_set_entity_lifecycle(
    context: *mut c_void,
    request: *const NativeMechanicsLifecycleRequest,
    result: *mut NativeMechanicsLifecycleReceipt,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe {
        invoke_with_operation_diagnostic(
            context,
            receipt,
            b"SetEntityLifecycle",
            || set_entity_lifecycle(context, request, result),
            |bridge| lifecycle_failure_diagnostic(bridge, request),
        )
    }
}
mechanics_callback!(receipt_destroy_entity(handle: NativeMechanicsEntityHandle) => destroy_entity, b"DestroyEntity", format!("entity-handle:{}", handle.value));
mechanics_callback!(receipt_read_stat(request: *const NativeMechanicsStatReadRequest, result: *mut NativeMechanicsStatReadReceipt) => read_stat, b"ReadStat", String::new());
mechanics_callback!(receipt_evaluate_stat(request: *const NativeMechanicsStatOperationRequest, result: *mut NativeMechanicsStatEvaluationLease) => evaluate_stat, b"EvaluateStat", String::new());
mechanics_callback!(receipt_read_track(request: *const NativeMechanicsTrackReadRequest, result: *mut NativeMechanicsTrackReadLease) => read_track, b"ReadTrack", String::new());
mechanics_callback!(receipt_read_inventory_view(handle: NativeMechanicsEntityHandle, result: *mut NativeMechanicsInventoryViewLease) => read_inventory_view, b"ReadInventoryView", format!("entity-handle:{}", handle.value));
mechanics_callback!(receipt_grant_inventory(request: *const NativeMechanicsInventoryMutationRequest, result: *mut NativeMechanicsInventoryMutationLease) => grant_inventory, b"GrantInventory", String::new());
mechanics_callback!(receipt_consume_inventory(request: *const NativeMechanicsInventoryMutationRequest, result: *mut NativeMechanicsInventoryMutationLease) => consume_inventory, b"ConsumeInventory", String::new());
mechanics_callback!(receipt_transfer_inventory(request: *const NativeMechanicsInventoryTransferRequest, result: *mut NativeMechanicsInventoryTransferLease) => transfer_inventory, b"TransferInventory", String::new());
mechanics_callback!(receipt_transfer_unique_item(request: *const NativeMechanicsUniqueItemTransferRequest, result: *mut NativeMechanicsUniqueItemTransferLease) => transfer_unique_item, b"TransferUniqueItem", String::new());
mechanics_callback!(receipt_materialize_unique_item(request: *const NativeMechanicsUniqueItemMaterializationRequest, result: *mut NativeMechanicsUniqueItemMaterializationLease) => materialize_unique_item, b"MaterializeUniqueItem", String::new());
mechanics_callback!(receipt_destroy_unique_item(request: *const NativeMechanicsUniqueItemDestroyRequest, result: *mut NativeMechanicsUniqueItemDestroyLease) => destroy_unique_item, b"DestroyUniqueItem", String::new());
mechanics_callback!(receipt_equip_equipment(request: *const NativeMechanicsEquipmentEquipRequest, result: *mut NativeMechanicsEquipmentMutationLease) => equip_equipment, b"EquipEquipment", String::new());
mechanics_callback!(receipt_unequip_equipment(request: *const NativeMechanicsEquipmentUnequipRequest, result: *mut NativeMechanicsEquipmentMutationLease) => unequip_equipment, b"UnequipEquipment", String::new());
mechanics_callback!(receipt_swap_equipment(request: *const NativeMechanicsEquipmentSwapRequest, result: *mut NativeMechanicsEquipmentMutationLease) => swap_equipment, b"SwapEquipment", String::new());
mechanics_callback!(receipt_set_stat_base(request: *const NativeMechanicsStatBaseMutationRequest, result: *mut NativeMechanicsStatMutationLease) => set_stat_base, b"SetStatBase", String::new());
unsafe extern "C" fn receipt_set_track(
    context: *mut c_void,
    request: *const NativeMechanicsTrackSetRequest,
    result: *mut NativeMechanicsTrackSetLease,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe {
        invoke_with_operation_diagnostic(
            context,
            receipt,
            b"SetTrack",
            || set_track(context, request, result),
            |bridge| track_set_failure_diagnostic(bridge, request),
        )
    }
}
mechanics_callback!(receipt_spend_track(request: *const NativeMechanicsTrackMutationRequest, result: *mut NativeMechanicsTrackMutationLease) => spend_track, b"SpendTrack", String::new());
mechanics_callback!(receipt_restore_track(request: *const NativeMechanicsTrackMutationRequest, result: *mut NativeMechanicsTrackMutationLease) => restore_track, b"RestoreTrack", String::new());
mechanics_callback!(receipt_reconcile_track(request: *const NativeMechanicsTrackReconciliationRequest, result: *mut NativeMechanicsTrackReconciliationLease) => reconcile_track, b"ReconcileTrack", String::new());
mechanics_callback!(receipt_apply_effect(request: *const NativeMechanicsEffectMutationRequest, result: *mut NativeMechanicsEffectOperationLease) => apply_effect, b"ApplyEffect", String::new());
mechanics_callback!(receipt_refresh_effect(request: *const NativeMechanicsEffectRefreshRequest, result: *mut NativeMechanicsEffectOperationLease) => refresh_effect, b"RefreshEffect", String::new());
mechanics_callback!(receipt_replace_effect(request: *const NativeMechanicsEffectMutationRequest, result: *mut NativeMechanicsEffectOperationLease) => replace_effect, b"ReplaceEffect", String::new());
mechanics_callback!(receipt_remove_effect(request: *const NativeMechanicsEffectRemovalRequest, result: *mut NativeMechanicsEffectOperationLease) => remove_effect, b"RemoveEffect", String::new());
mechanics_callback!(receipt_expire_effect(request: *const NativeMechanicsEffectRemovalRequest, result: *mut NativeMechanicsEffectOperationLease) => expire_effect, b"ExpireEffect", String::new());
mechanics_callback!(receipt_preview_damage(request: *const NativeMechanicsDamageRequest, result: *mut NativeMechanicsDamageLease) => preview_damage, b"PreviewDamage", String::new());
mechanics_callback!(receipt_apply_damage(request: *const NativeMechanicsDamageRequest, result: *mut NativeMechanicsDamageLease) => apply_damage, b"ApplyDamage", String::new());

unsafe extern "C" fn destroy_operation_diagnostic_lease(
    context: *mut c_void,
    handle: NativeEngineDiagnosticLeaseHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    // SAFETY: context is the stable Mechanics bridge for the product lifetime.
    i32::from(unsafe {
        (&mut *context.cast::<RuntimeMechanicsBridge>()).destroy_operation_diagnostic_lease(handle)
    })
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

fn build_catalog_lease<F>(
    bridge: &RuntimeMechanicsBridge,
    handle: NativeMechanicsCatalogHandle,
    build: F,
) -> Option<CatalogLeaseBacking>
where
    F: FnOnce(&MechanicsCatalog, &mut CatalogLeaseText) -> CatalogLeaseRows,
{
    let catalog = bridge.catalogs.get(&handle.value)?.catalog.as_ref()?;
    let mut text = CatalogLeaseText::default();
    let rows = build(catalog, &mut text);
    Some(CatalogLeaseBacking {
        _text: text.values,
        rows,
    })
}

macro_rules! read_catalog_rows {
    ($function:ident, $lease:ident, $variant:ident, $row:ident, $build:expr) => {
        unsafe extern "C" fn $function(
            context: *mut c_void,
            catalog: NativeMechanicsCatalogHandle,
            result: *mut $lease,
        ) -> i32 {
            if context.is_null() || result.is_null() {
                return 0;
            }
            let bridge = unsafe { &mut *context.cast::<RuntimeMechanicsBridge>() };
            let Some(backing) = build_catalog_lease(bridge, catalog, $build) else {
                return 0;
            };
            let Some(handle) = bridge.insert_catalog_lease(backing) else {
                return 0;
            };
            let entries = match &bridge
                .catalog_leases
                .get(&handle.value)
                .expect("just inserted catalog lease")
                .rows
            {
                CatalogLeaseRows::$variant(entries) => entries,
                _ => unreachable!("catalog lease row kind matches its reader"),
            };
            unsafe {
                *result = $lease {
                    handle,
                    entries: entries.as_ptr(),
                    entries_len: entries.len(),
                    catalog_id: catalog.value,
                };
            }
            ABI_OK
        }
    };
}

read_catalog_rows!(
    read_catalog_identity,
    NativeMechanicsCatalogIdentityLease,
    Identity,
    NativeMechanicsCatalogIdentityRow,
    |catalog: &MechanicsCatalog, text: &mut CatalogLeaseText| {
        CatalogLeaseRows::Identity(vec![NativeMechanicsCatalogIdentityRow {
            version: text.copy(catalog.version().as_str()),
            fingerprint: text.copy(catalog.fingerprint()),
        }])
    }
);
read_catalog_rows!(
    read_catalog_stats,
    NativeMechanicsStatCatalogLease,
    Stats,
    NativeMechanicsStatCatalogRow,
    |catalog: &MechanicsCatalog, text: &mut CatalogLeaseText| {
        CatalogLeaseRows::Stats(
            catalog
                .view()
                .stats()
                .iter()
                .map(|stat| NativeMechanicsStatCatalogRow {
                    id: text.copy(stat.id.as_str()),
                    minimum: stat.minimum.get(),
                    maximum: stat.maximum.get(),
                })
                .collect(),
        )
    }
);
read_catalog_rows!(
    read_catalog_tracks,
    NativeMechanicsTrackCatalogLease,
    Tracks,
    NativeMechanicsTrackCatalogRow,
    |catalog: &MechanicsCatalog, text: &mut CatalogLeaseText| {
        CatalogLeaseRows::Tracks(
            catalog
                .view()
                .tracks()
                .iter()
                .map(|track| {
                    let (maximum_kind, fixed_maximum, maximum_stat) = match &track.maximum {
                        TrackMaximum::Fixed { value } => (
                            NativeMechanicsTrackMaximumKind::Fixed,
                            value.get(),
                            text.copy(""),
                        ),
                        TrackMaximum::Stat { stat } => (
                            NativeMechanicsTrackMaximumKind::Stat,
                            0,
                            text.copy(stat.as_str()),
                        ),
                    };
                    NativeMechanicsTrackCatalogRow {
                        id: text.copy(track.id.as_str()),
                        minimum: track.minimum.get(),
                        maximum_kind,
                        fixed_maximum,
                        maximum_stat,
                    }
                })
                .collect(),
        )
    }
);
read_catalog_rows!(
    read_catalog_sources,
    NativeMechanicsSourceCatalogLease,
    Sources,
    NativeMechanicsSourceCatalogRow,
    |catalog: &MechanicsCatalog, text: &mut CatalogLeaseText| {
        CatalogLeaseRows::Sources(
            catalog
                .view()
                .sources()
                .iter()
                .map(|source| NativeMechanicsSourceCatalogRow {
                    id: text.copy(source.id.as_str()),
                    priority: source.priority,
                })
                .collect(),
        )
    }
);
read_catalog_rows!(
    read_catalog_stat_contributions,
    NativeMechanicsStatContributionCatalogLease,
    StatContributions,
    NativeMechanicsStatContributionCatalogRow,
    |catalog: &MechanicsCatalog, text: &mut CatalogLeaseText| {
        CatalogLeaseRows::StatContributions(
            catalog
                .view()
                .sources()
                .iter()
                .flat_map(|source| {
                    source
                        .stat_contributions
                        .iter()
                        .map(move |entry| (source, entry))
                })
                .map(|(source, entry)| {
                    let (kind, amount, ratio_numerator, ratio_denominator) =
                        match entry.contribution {
                            StatContribution::Add { amount } => {
                                (NativeMechanicsContributionKind::Add, amount.get(), 0, 0)
                            }
                            StatContribution::Scale { ratio } => (
                                NativeMechanicsContributionKind::Scale,
                                0,
                                ratio.numerator(),
                                ratio.denominator(),
                            ),
                            StatContribution::Minimum { value } => {
                                (NativeMechanicsContributionKind::Minimum, value.get(), 0, 0)
                            }
                            StatContribution::Maximum { value } => {
                                (NativeMechanicsContributionKind::Maximum, value.get(), 0, 0)
                            }
                        };
                    NativeMechanicsStatContributionCatalogRow {
                        source: text.copy(source.id.as_str()),
                        stat: text.copy(entry.stat.as_str()),
                        kind,
                        amount,
                        ratio_numerator,
                        ratio_denominator,
                        stacking_group: text.copy(entry.stacking_group.as_str()),
                        stacking: native_stacking(entry.stacking),
                    }
                })
                .collect(),
        )
    }
);
read_catalog_rows!(
    read_catalog_damage_kinds,
    NativeMechanicsDamageKindCatalogLease,
    DamageKinds,
    NativeMechanicsDamageKindCatalogRow,
    |catalog: &MechanicsCatalog, text: &mut CatalogLeaseText| CatalogLeaseRows::DamageKinds(
        catalog
            .view()
            .damage_kinds()
            .iter()
            .map(|kind| NativeMechanicsDamageKindCatalogRow {
                id: text.copy(kind.id.as_str())
            })
            .collect()
    )
);
read_catalog_rows!(
    read_catalog_damage_responses,
    NativeMechanicsDamageResponseCatalogLease,
    DamageResponses,
    NativeMechanicsDamageResponseCatalogRow,
    |catalog: &MechanicsCatalog, text: &mut CatalogLeaseText| {
        CatalogLeaseRows::DamageResponses(
            catalog
                .view()
                .sources()
                .iter()
                .flat_map(|source| {
                    source
                        .damage_responses
                        .iter()
                        .map(move |entry| (source, entry))
                })
                .map(|(source, entry)| {
                    let (selector_is_exact, selector_damage_kind) = match entry.selector() {
                        DamageKindSelector::Any => (false, text.copy("")),
                        DamageKindSelector::Exact { damage_kind } => {
                            (true, text.copy(damage_kind.as_str()))
                        }
                    };
                    let (
                        kind,
                        amount,
                        ratio_numerator,
                        ratio_denominator,
                        stacking_group,
                        stacking,
                        absorb_track,
                    ) = match entry {
                        DamageResponseDefinition::Prevent {
                            stacking_group,
                            stacking,
                            ..
                        } => (
                            NativeMechanicsDamageResponseKind::Prevent,
                            0,
                            0,
                            0,
                            text.copy(stacking_group.as_str()),
                            native_stacking(*stacking),
                            text.copy(""),
                        ),
                        DamageResponseDefinition::FlatReduction {
                            amount,
                            stacking_group,
                            stacking,
                            ..
                        } => (
                            NativeMechanicsDamageResponseKind::FlatReduction,
                            amount.get(),
                            0,
                            0,
                            text.copy(stacking_group.as_str()),
                            native_stacking(*stacking),
                            text.copy(""),
                        ),
                        DamageResponseDefinition::Scale {
                            ratio,
                            stacking_group,
                            stacking,
                            ..
                        } => (
                            NativeMechanicsDamageResponseKind::Scale,
                            0,
                            ratio.numerator(),
                            ratio.denominator(),
                            text.copy(stacking_group.as_str()),
                            native_stacking(*stacking),
                            text.copy(""),
                        ),
                        DamageResponseDefinition::Absorb { track, .. } => (
                            NativeMechanicsDamageResponseKind::Absorb,
                            0,
                            0,
                            0,
                            text.copy(""),
                            NativeMechanicsStackingPolicy::Sum,
                            text.copy(track.as_str()),
                        ),
                    };
                    NativeMechanicsDamageResponseCatalogRow {
                        source: text.copy(source.id.as_str()),
                        kind,
                        selector_is_exact,
                        selector_damage_kind,
                        amount,
                        ratio_numerator,
                        ratio_denominator,
                        stacking_group,
                        stacking,
                        absorb_track,
                    }
                })
                .collect(),
        )
    }
);
read_catalog_rows!(
    read_catalog_effects,
    NativeMechanicsEffectCatalogLease,
    Effects,
    NativeMechanicsEffectCatalogRow,
    |catalog: &MechanicsCatalog, text: &mut CatalogLeaseText| {
        CatalogLeaseRows::Effects(
            catalog
                .view()
                .effects()
                .iter()
                .map(|effect| {
                    let (stacking, maximum_instances) = match effect.stacking {
                        EffectStackingPolicy::IndependentByProvenance { maximum_instances } => (
                            NativeMechanicsEffectStackingKind::IndependentByProvenance,
                            maximum_instances,
                        ),
                        EffectStackingPolicy::Refresh => {
                            (NativeMechanicsEffectStackingKind::Refresh, 0)
                        }
                        EffectStackingPolicy::Replace => {
                            (NativeMechanicsEffectStackingKind::Replace, 0)
                        }
                    };
                    NativeMechanicsEffectCatalogRow {
                        id: text.copy(effect.id.as_str()),
                        stacking_group: text.copy(effect.stacking_group.as_str()),
                        stacking,
                        maximum_instances,
                        maximum_stacks: effect.maximum_stacks,
                    }
                })
                .collect(),
        )
    }
);
read_catalog_rows!(
    read_catalog_effect_sources,
    NativeMechanicsEffectSourceCatalogLease,
    EffectSources,
    NativeMechanicsEffectSourceCatalogRow,
    |catalog: &MechanicsCatalog, text: &mut CatalogLeaseText| CatalogLeaseRows::EffectSources(
        catalog
            .view()
            .effects()
            .iter()
            .flat_map(|effect| effect.sources.iter().map(move |source| (effect, source)))
            .map(|(effect, source)| NativeMechanicsEffectSourceCatalogRow {
                effect: text.copy(effect.id.as_str()),
                source: text.copy(source.as_str())
            })
            .collect()
    )
);
read_catalog_rows!(
    read_catalog_capacity_metrics,
    NativeMechanicsCapacityMetricCatalogLease,
    CapacityMetrics,
    NativeMechanicsCapacityMetricCatalogRow,
    |catalog: &MechanicsCatalog, text: &mut CatalogLeaseText| CatalogLeaseRows::CapacityMetrics(
        catalog
            .view()
            .capacity_metrics()
            .iter()
            .map(|metric| NativeMechanicsCapacityMetricCatalogRow {
                id: text.copy(metric.id.as_str())
            })
            .collect()
    )
);
read_catalog_rows!(
    read_catalog_items,
    NativeMechanicsItemCatalogLease,
    Items,
    NativeMechanicsItemCatalogRow,
    |catalog: &MechanicsCatalog, text: &mut CatalogLeaseText| CatalogLeaseRows::Items(
        catalog
            .view()
            .items()
            .iter()
            .map(|item| NativeMechanicsItemCatalogRow {
                id: text.copy(item.id.as_str()),
                kind: match item.kind {
                    ItemKind::Fungible => NativeMechanicsItemKind::Fungible,
                    ItemKind::Unique => NativeMechanicsItemKind::Unique,
                },
                maximum_quantity: item.maximum_quantity
            })
            .collect()
    )
);
read_catalog_rows!(
    read_catalog_item_classifications,
    NativeMechanicsItemClassificationCatalogLease,
    ItemClassifications,
    NativeMechanicsItemClassificationCatalogRow,
    |catalog: &MechanicsCatalog, text: &mut CatalogLeaseText| CatalogLeaseRows::ItemClassifications(
        catalog
            .view()
            .items()
            .iter()
            .flat_map(|item| item
                .classifications
                .iter()
                .map(move |classification| (item, classification)))
            .map(
                |(item, classification)| NativeMechanicsItemClassificationCatalogRow {
                    item: text.copy(item.id.as_str()),
                    classification: text.copy(classification.as_str())
                }
            )
            .collect()
    )
);
read_catalog_rows!(
    read_catalog_item_capacity_costs,
    NativeMechanicsItemCapacityCostCatalogLease,
    ItemCapacityCosts,
    NativeMechanicsItemCapacityCostCatalogRow,
    |catalog: &MechanicsCatalog, text: &mut CatalogLeaseText| CatalogLeaseRows::ItemCapacityCosts(
        catalog
            .view()
            .items()
            .iter()
            .flat_map(|item| item.capacity_costs.iter().map(move |cost| (item, cost)))
            .map(|(item, cost)| NativeMechanicsItemCapacityCostCatalogRow {
                item: text.copy(item.id.as_str()),
                metric: text.copy(cost.metric.as_str()),
                units: cost.units
            })
            .collect()
    )
);
read_catalog_rows!(
    read_catalog_item_equipment_policies,
    NativeMechanicsItemEquipmentPolicyCatalogLease,
    ItemEquipmentPolicies,
    NativeMechanicsItemEquipmentPolicyCatalogRow,
    |catalog: &MechanicsCatalog, text: &mut CatalogLeaseText| {
        CatalogLeaseRows::ItemEquipmentPolicies(
            catalog
                .view()
                .items()
                .iter()
                .filter_map(|item| item.equipment.as_ref().map(|equipment| (item, equipment)))
                .map(
                    |(item, equipment)| NativeMechanicsItemEquipmentPolicyCatalogRow {
                        item: text.copy(item.id.as_str()),
                        required_slots: equipment.required_slots,
                        has_exclusive_group: equipment.exclusive_group.is_some(),
                        exclusive_group: text.copy(
                            equipment
                                .exclusive_group
                                .as_ref()
                                .map_or("", |value| value.as_str()),
                        ),
                    },
                )
                .collect(),
        )
    }
);
read_catalog_rows!(
    read_catalog_item_sources,
    NativeMechanicsItemSourceCatalogLease,
    ItemSources,
    NativeMechanicsItemSourceCatalogRow,
    |catalog: &MechanicsCatalog, text: &mut CatalogLeaseText| CatalogLeaseRows::ItemSources(
        catalog
            .view()
            .items()
            .iter()
            .flat_map(|item| item.sources.iter().map(move |source| (item, source)))
            .map(|(item, source)| NativeMechanicsItemSourceCatalogRow {
                item: text.copy(item.id.as_str()),
                source: text.copy(source.as_str())
            })
            .collect()
    )
);
read_catalog_rows!(
    read_catalog_equipment_slots,
    NativeMechanicsEquipmentSlotCatalogLease,
    EquipmentSlots,
    NativeMechanicsEquipmentSlotCatalogRow,
    |catalog: &MechanicsCatalog, text: &mut CatalogLeaseText| CatalogLeaseRows::EquipmentSlots(
        catalog
            .view()
            .equipment_slots()
            .iter()
            .map(|slot| NativeMechanicsEquipmentSlotCatalogRow {
                id: text.copy(slot.id.as_str())
            })
            .collect()
    )
);
read_catalog_rows!(
    read_catalog_slot_classifications,
    NativeMechanicsSlotClassificationCatalogLease,
    SlotClassifications,
    NativeMechanicsSlotClassificationCatalogRow,
    |catalog: &MechanicsCatalog, text: &mut CatalogLeaseText| CatalogLeaseRows::SlotClassifications(
        catalog
            .view()
            .equipment_slots()
            .iter()
            .flat_map(|slot| slot
                .allowed_classifications
                .iter()
                .map(move |classification| (slot, classification)))
            .map(
                |(slot, classification)| NativeMechanicsSlotClassificationCatalogRow {
                    slot: text.copy(slot.id.as_str()),
                    classification: text.copy(classification.as_str())
                }
            )
            .collect()
    )
);

unsafe extern "C" fn destroy_catalog_lease(
    context: *mut c_void,
    handle: NativeMechanicsCatalogLeaseHandle,
) -> i32 {
    if context.is_null() || handle.value == 0 {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeMechanicsBridge>() };
    i32::from(bridge.catalog_leases.remove(&handle.value).is_some())
}

fn build_component_lease<F>(
    bridge: &RuntimeMechanicsBridge,
    handle: NativeMechanicsEntityHandle,
    component: NativeMechanicsRevisionComponent,
    build: F,
) -> Option<(ComponentLeaseBacking, NativeMechanicsComponentReadMetadata)>
where
    F: FnOnce(&EntityState, EntityId, &mut CatalogLeaseText) -> ComponentLeaseRows,
{
    let binding = bridge.binding(handle)?.clone();
    let slot = bridge.catalogs.get(&binding.catalog)?;
    let lifecycle = slot.world.lifecycle.get(&binding.entity)?.lifecycle;
    if !matches!(
        lifecycle,
        NativeMechanicsEntityLifecycle::Active | NativeMechanicsEntityLifecycle::Disabled
    ) {
        return None;
    }
    let catalog = slot.catalog.as_ref()?;
    let mut text = CatalogLeaseText::default();
    let rows = build(&slot.world.state, binding.entity, &mut text);
    let metadata = NativeMechanicsComponentReadMetadata {
        entity_id: binding.entity.raw(),
        component,
        revision: component_read_revision(&slot.world.state, binding.entity, component),
        present: component_is_present(&slot.world.state, binding.entity, component),
        catalog_id: binding.catalog,
        catalog_version: text.copy(catalog.version().as_str()),
        catalog_fingerprint: text.copy(catalog.fingerprint()),
    };
    Some((
        ComponentLeaseBacking {
            _text: text.values,
            rows,
        },
        metadata,
    ))
}

macro_rules! read_component_rows {
    ($function:ident, $lease:ident, $variant:ident, $row:ident, $component:ty, $kind:expr, $build:expr) => {
        unsafe extern "C" fn $function(
            context: *mut c_void,
            entity: NativeMechanicsEntityHandle,
            result: *mut $lease,
        ) -> i32 {
            if context.is_null() || result.is_null() {
                return 0;
            }
            let bridge = unsafe { &mut *context.cast::<RuntimeMechanicsBridge>() };
            let Some((backing, metadata)) =
                build_component_lease(bridge, entity, $kind, |state, entity_id, text| {
                    let entries = state
                        .component::<$component>(entity_id)
                        .ok()
                        .flatten()
                        .map(|component| $build(component, text))
                        .unwrap_or_default();
                    ComponentLeaseRows::$variant(entries)
                })
            else {
                return 0;
            };
            let Some(handle) = bridge.insert_component_lease(backing) else {
                return 0;
            };
            let entries = match &bridge
                .component_leases
                .get(&handle.value)
                .expect("just inserted component lease")
                .rows
            {
                ComponentLeaseRows::$variant(entries) => entries,
                _ => unreachable!("component lease row kind matches its reader"),
            };
            unsafe {
                *result = $lease {
                    handle,
                    entries: entries.as_ptr(),
                    entries_len: entries.len(),
                    metadata,
                };
            }
            ABI_OK
        }
    };
}

read_component_rows!(
    read_stat_component,
    NativeMechanicsStatComponentLease,
    Stats,
    NativeMechanicsStatComponentRow,
    StatsComponent,
    NativeMechanicsRevisionComponent::Stats,
    |component: &StatsComponent, text: &mut CatalogLeaseText| {
        component
            .values()
            .iter()
            .map(|value| NativeMechanicsStatComponentRow {
                stat: text.copy(value.stat().as_str()),
                base: value.base().get(),
            })
            .collect()
    }
);
read_component_rows!(
    read_track_component,
    NativeMechanicsTrackComponentLease,
    Tracks,
    NativeMechanicsTrackComponentRow,
    TracksComponent,
    NativeMechanicsRevisionComponent::Tracks,
    |component: &TracksComponent, text: &mut CatalogLeaseText| {
        component
            .values()
            .iter()
            .map(|value| NativeMechanicsTrackComponentRow {
                track: text.copy(value.track().as_str()),
                current: value.current().get(),
            })
            .collect()
    }
);
read_component_rows!(
    read_intrinsic_source_component,
    NativeMechanicsIntrinsicSourceComponentLease,
    IntrinsicSources,
    NativeMechanicsIntrinsicSourceComponentRow,
    IntrinsicSourcesComponent,
    NativeMechanicsRevisionComponent::IntrinsicSources,
    |component: &IntrinsicSourcesComponent, text: &mut CatalogLeaseText| {
        component
            .bindings()
            .iter()
            .map(|binding| NativeMechanicsIntrinsicSourceComponentRow {
                instance: text.copy(binding.instance().as_str()),
                definition: text.copy(binding.definition().as_str()),
            })
            .collect()
    }
);
read_component_rows!(
    read_active_effect_component,
    NativeMechanicsActiveEffectComponentLease,
    ActiveEffects,
    NativeMechanicsActiveEffectComponentRow,
    ActiveEffectsComponent,
    NativeMechanicsRevisionComponent::ActiveEffects,
    |component: &ActiveEffectsComponent, text: &mut CatalogLeaseText| {
        component
            .effects()
            .iter()
            .map(|effect| {
                let mut row = NativeMechanicsActiveEffectComponentRow {
                    instance: text.copy(effect.instance().as_str()),
                    definition: text.copy(effect.definition().as_str()),
                    stacks: effect.stacks(),
                    provenance_kind: NativeMechanicsActiveEffectProvenanceKind::Intrinsic,
                    intrinsic_entity_id: 0,
                    intrinsic_instance: text.copy(""),
                    effect_entity_id: 0,
                    effect_instance: text.copy(""),
                    effect_stack: 0,
                    effect_source: text.copy(""),
                    equipped_owner_entity_id: 0,
                    equipped_item_entity_id: 0,
                    equipped_source: text.copy(""),
                    request_operation: text.copy(""),
                    request_instance: text.copy(""),
                };
                match effect.provenance() {
                    SourceInstanceIdentity::Intrinsic { entity, instance } => {
                        row.provenance_kind = NativeMechanicsActiveEffectProvenanceKind::Intrinsic;
                        row.intrinsic_entity_id = entity.raw();
                        row.intrinsic_instance = text.copy(instance.as_str());
                    }
                    SourceInstanceIdentity::Effect {
                        entity,
                        effect,
                        stack,
                        source,
                    } => {
                        row.provenance_kind = NativeMechanicsActiveEffectProvenanceKind::Effect;
                        row.effect_entity_id = entity.raw();
                        row.effect_instance = text.copy(effect.as_str());
                        row.effect_stack = *stack;
                        row.effect_source = text.copy(source.as_str());
                    }
                    SourceInstanceIdentity::EquippedItem {
                        owner,
                        item,
                        source,
                    } => {
                        row.provenance_kind =
                            NativeMechanicsActiveEffectProvenanceKind::EquippedItem;
                        row.equipped_owner_entity_id = owner.raw();
                        row.equipped_item_entity_id = item.raw();
                        row.equipped_source = text.copy(source.as_str());
                    }
                    SourceInstanceIdentity::Request {
                        operation,
                        instance,
                    } => {
                        row.provenance_kind = NativeMechanicsActiveEffectProvenanceKind::Request;
                        row.request_operation = text.copy(operation.as_str());
                        row.request_instance = text.copy(instance.as_str());
                    }
                }
                row
            })
            .collect()
    }
);
read_component_rows!(
    read_inventory_stack_component,
    NativeMechanicsInventoryStackComponentLease,
    InventoryStacks,
    NativeMechanicsInventoryStackComponentRow,
    InventoryComponent,
    NativeMechanicsRevisionComponent::Inventory,
    |component: &InventoryComponent, text: &mut CatalogLeaseText| {
        component
            .stacks()
            .iter()
            .map(|stack| NativeMechanicsInventoryStackComponentRow {
                definition: text.copy(stack.definition.as_str()),
                quantity: stack.quantity,
            })
            .collect()
    }
);
read_component_rows!(
    read_inventory_capacity_limit_component,
    NativeMechanicsInventoryCapacityLimitComponentLease,
    InventoryCapacityLimits,
    NativeMechanicsInventoryCapacityLimitComponentRow,
    InventoryComponent,
    NativeMechanicsRevisionComponent::Inventory,
    |component: &InventoryComponent, text: &mut CatalogLeaseText| {
        component
            .capacity_limits()
            .iter()
            .map(|limit| NativeMechanicsInventoryCapacityLimitComponentRow {
                metric: text.copy(limit.metric().as_str()),
                maximum: limit.maximum(),
            })
            .collect()
    }
);
read_component_rows!(
    read_item_component,
    NativeMechanicsItemComponentLease,
    Items,
    NativeMechanicsItemComponentRow,
    ItemComponent,
    NativeMechanicsRevisionComponent::Item,
    |component: &ItemComponent, text: &mut CatalogLeaseText| {
        vec![NativeMechanicsItemComponentRow {
            definition: text.copy(component.definition().as_str()),
        }]
    }
);
read_component_rows!(
    read_equipment_assignment_component,
    NativeMechanicsEquipmentAssignmentComponentLease,
    EquipmentAssignments,
    NativeMechanicsEquipmentAssignmentComponentRow,
    EquipmentComponent,
    NativeMechanicsRevisionComponent::Equipment,
    |component: &EquipmentComponent, text: &mut CatalogLeaseText| {
        component
            .assignments()
            .iter()
            .map(
                |assignment| NativeMechanicsEquipmentAssignmentComponentRow {
                    slot: text.copy(assignment.slot.as_str()),
                    item_entity_id: assignment.item.raw(),
                },
            )
            .collect()
    }
);

unsafe extern "C" fn destroy_component_lease(
    context: *mut c_void,
    handle: NativeMechanicsComponentLeaseHandle,
) -> i32 {
    if context.is_null() || handle.value == 0 {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeMechanicsBridge>() };
    i32::from(bridge.component_leases.remove(&handle.value).is_some())
}

fn catalog_membership(bridge: &RuntimeMechanicsBridge, catalog: u64) -> BTreeMap<EntityId, u64> {
    bridge
        .canonical_entities
        .iter()
        .filter_map(|(entity, mapped_catalog)| {
            (*mapped_catalog == catalog).then_some((*entity, *mapped_catalog))
        })
        .collect()
}

fn canonical_identity(world: &MechanicsWorld) -> Option<BTreeMap<EntityId, String>> {
    world
        .lifecycle
        .keys()
        .map(|entity| Some((*entity, world.state.core(*entity)?.name.clone())))
        .collect()
}

fn binding_topology(
    bridge: &RuntimeMechanicsBridge,
    catalog: u64,
) -> BTreeMap<u64, (u64, EntityId, bool)> {
    bridge
        .entities
        .iter()
        .filter_map(|(handle, binding)| {
            (binding.catalog == catalog).then_some((
                *handle,
                (binding.catalog, binding.entity, binding.committed),
            ))
        })
        .collect()
}

fn remap_component_revision<T: entity_state::EntityComponent>(
    snapshot: &EntityState,
    current: &EntityState,
    restored: &EntityState,
    entity: EntityId,
    component: NativeMechanicsRevisionComponent,
) -> Option<NativeMechanicsRevisionRemapRow> {
    let snapshot_revision = snapshot.component_revision::<T>(entity).ok()?.revision();
    let current_revision = current.component_revision::<T>(entity).ok()?.revision();
    let restored_revision = restored.component_revision::<T>(entity).ok()?.revision();
    let present = restored.has_component::<T>(entity).ok()?;
    Some(NativeMechanicsRevisionRemapRow {
        entity_id: entity.raw(),
        component,
        present,
        snapshot_revision,
        current_revision,
        restored_revision,
    })
}

fn revision_remaps(
    snapshot: &EntityState,
    current: &EntityState,
    restored: &EntityState,
    entities: impl IntoIterator<Item = EntityId>,
) -> Option<Vec<NativeMechanicsRevisionRemapRow>> {
    let mut rows = Vec::new();
    // EntityState's restore primitive has already verified the exact entity set. The seven
    // Mechanics families are intentionally enumerated here so the generated receipt remains a
    // stable product API rather than an erased component registry.
    for entity in entities {
        rows.push(remap_component_revision::<StatsComponent>(
            snapshot,
            current,
            restored,
            entity,
            NativeMechanicsRevisionComponent::Stats,
        )?);
        rows.push(remap_component_revision::<TracksComponent>(
            snapshot,
            current,
            restored,
            entity,
            NativeMechanicsRevisionComponent::Tracks,
        )?);
        rows.push(remap_component_revision::<IntrinsicSourcesComponent>(
            snapshot,
            current,
            restored,
            entity,
            NativeMechanicsRevisionComponent::IntrinsicSources,
        )?);
        rows.push(remap_component_revision::<ActiveEffectsComponent>(
            snapshot,
            current,
            restored,
            entity,
            NativeMechanicsRevisionComponent::ActiveEffects,
        )?);
        rows.push(remap_component_revision::<InventoryComponent>(
            snapshot,
            current,
            restored,
            entity,
            NativeMechanicsRevisionComponent::Inventory,
        )?);
        rows.push(remap_component_revision::<ItemComponent>(
            snapshot,
            current,
            restored,
            entity,
            NativeMechanicsRevisionComponent::Item,
        )?);
        rows.push(remap_component_revision::<EquipmentComponent>(
            snapshot,
            current,
            restored,
            entity,
            NativeMechanicsRevisionComponent::Equipment,
        )?);
    }
    Some(rows)
}

unsafe extern "C" fn capture_world_snapshot(
    context: *mut c_void,
    catalog: NativeMechanicsCatalogHandle,
    result: *mut NativeMechanicsWorldSnapshotHandle,
) -> i32 {
    if context.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeMechanicsBridge>() };
    if bridge
        .entities
        .values()
        .any(|binding| binding.catalog == catalog.value && !binding.committed)
    {
        return 0;
    }
    let Some(slot) = bridge.catalogs.get(&catalog.value) else {
        return 0;
    };
    let Some(native_catalog) = slot.catalog.as_ref() else {
        return 0;
    };
    let handle = bridge.next_world_snapshot;
    let Some(next_handle) = handle.checked_add(1) else {
        return 0;
    };
    let snapshot = MechanicsWorldSnapshot {
        catalog: catalog.value,
        catalog_fingerprint: native_catalog.fingerprint().to_owned(),
        world: slot.world.clone(),
        canonical_membership: catalog_membership(bridge, catalog.value),
        canonical_identity: match canonical_identity(&slot.world) {
            Some(identity) => identity,
            None => return 0,
        },
        binding_topology: binding_topology(bridge, catalog.value),
    };
    bridge.next_world_snapshot = next_handle;
    bridge.world_snapshots.insert(handle, Box::new(snapshot));
    unsafe { *result = NativeMechanicsWorldSnapshotHandle { value: handle } };
    ABI_OK
}

unsafe extern "C" fn receipt_capture_world_snapshot(
    context: *mut c_void,
    catalog: NativeMechanicsCatalogHandle,
    result: *mut NativeMechanicsWorldSnapshotHandle,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe {
        invoke_with_operation_diagnostic(
            context,
            receipt,
            b"CaptureWorldSnapshot",
            || capture_world_snapshot(context, catalog, result),
            |_bridge| {
                (
                    b"MECHANICS_SNAPSHOT_REJECTED".as_slice(),
                    b"Mechanics world snapshot requires one admitted catalog and committed bindings."
                        .as_slice(),
                    catalog_source(catalog),
                )
            },
        )
    }
}

unsafe extern "C" fn destroy_world_snapshot(
    context: *mut c_void,
    handle: NativeMechanicsWorldSnapshotHandle,
) -> i32 {
    if context.is_null() || handle.value == 0 {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeMechanicsBridge>() };
    if bridge
        .world_snapshot_leases
        .values()
        .any(|snapshot| *snapshot == handle.value)
    {
        return 0;
    }
    i32::from(bridge.world_snapshots.remove(&handle.value).is_some())
}

unsafe extern "C" fn read_world_snapshot(
    context: *mut c_void,
    handle: NativeMechanicsWorldSnapshotHandle,
    result: *mut NativeMechanicsWorldSnapshotLease,
) -> i32 {
    if context.is_null() || result.is_null() || handle.value == 0 {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeMechanicsBridge>() };
    let Some(snapshot) = bridge.world_snapshots.get(&handle.value) else {
        return 0;
    };
    let lease = bridge.next_world_snapshot_lease;
    let Some(next_lease) = lease.checked_add(1) else {
        return 0;
    };
    bridge.next_world_snapshot_lease = next_lease;
    bridge.world_snapshot_leases.insert(lease, handle.value);
    *result = NativeMechanicsWorldSnapshotLease {
        handle: NativeMechanicsWorldSnapshotLeaseHandle { value: lease },
        state_revision: snapshot.world.state.revision(),
    };
    ABI_OK
}

unsafe extern "C" fn receipt_read_world_snapshot(
    context: *mut c_void,
    handle: NativeMechanicsWorldSnapshotHandle,
    result: *mut NativeMechanicsWorldSnapshotLease,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe {
        invoke_with_operation_diagnostic(
            context,
            receipt,
            b"ReadWorldSnapshot",
            || read_world_snapshot(context, handle, result),
            |_bridge| {
                (
                    b"MECHANICS_SNAPSHOT_NOT_FOUND".as_slice(),
                    b"Mechanics world snapshot was not found.".as_slice(),
                    String::new(),
                )
            },
        )
    }
}

unsafe extern "C" fn destroy_world_snapshot_lease(
    context: *mut c_void,
    handle: NativeMechanicsWorldSnapshotLeaseHandle,
) -> i32 {
    if context.is_null() || handle.value == 0 {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeMechanicsBridge>() };
    i32::from(bridge.world_snapshot_leases.remove(&handle.value).is_some())
}

unsafe extern "C" fn prepare_world_restore(
    context: *mut c_void,
    request: *const NativeMechanicsWorldRestoreRequest,
    result: *mut NativeMechanicsWorldRestoreHandle,
) -> i32 {
    let Some((bridge, request, result)) = bridge_request_result(context, request, result) else {
        return 0;
    };
    if bridge
        .entities
        .values()
        .any(|binding| binding.catalog == request.catalog.value && !binding.committed)
    {
        return 0;
    }
    let Some(snapshot) = bridge.world_snapshots.get(&request.snapshot.value) else {
        return 0;
    };
    if snapshot.catalog != request.catalog.value
        || snapshot.canonical_membership != catalog_membership(bridge, request.catalog.value)
        || snapshot.binding_topology != binding_topology(bridge, request.catalog.value)
    {
        return 0;
    }
    let Some(slot) = bridge.catalogs.get(&request.catalog.value) else {
        return 0;
    };
    let Some(catalog) = slot.catalog.as_ref() else {
        return 0;
    };
    if catalog.fingerprint() != snapshot.catalog_fingerprint
        || canonical_identity(&slot.world).as_ref() != Some(&snapshot.canonical_identity)
        || slot.world.state.revision() != request.expected_state_revision
        || snapshot.world.lifecycle.len() != slot.world.lifecycle.len()
        || snapshot
            .world
            .lifecycle
            .iter()
            .any(|(entity, snapshot_record)| {
                let Some(current_record) = slot.world.lifecycle.get(entity) else {
                    return true;
                };
                (snapshot_record.lifecycle == NativeMechanicsEntityLifecycle::Tombstoned)
                    != (current_record.lifecycle == NativeMechanicsEntityLifecycle::Tombstoned)
            })
    {
        return 0;
    }

    let mut candidate = snapshot.world.clone();
    if !candidate.state.rebase_revisions_after(&slot.world.state)
        || validate_state_against_catalog(&candidate.state, catalog).is_err()
    {
        return 0;
    }
    let mut highest_stamp = candidate.next_stamp.max(slot.world.next_stamp);
    for (entity, record) in &mut candidate.lifecycle {
        let Some(current_record) = slot.world.lifecycle.get(entity) else {
            return 0;
        };
        let Some(stamp) = record.stamp.max(current_record.stamp).checked_add(1) else {
            return 0;
        };
        record.stamp = stamp;
        highest_stamp = highest_stamp.max(stamp);
    }
    let Some(next_stamp) = highest_stamp.checked_add(1) else {
        return 0;
    };
    candidate.next_stamp = next_stamp;

    let entities: Vec<_> = snapshot.canonical_membership.keys().copied().collect();
    let Some(revisions) = revision_remaps(
        &snapshot.world.state,
        &slot.world.state,
        &candidate.state,
        entities,
    ) else {
        return 0;
    };
    let lifecycles = candidate
        .lifecycle
        .keys()
        .copied()
        .map(|entity| candidate.lifecycle_receipt(entity))
        .collect();
    let handle = bridge.next_world_restore;
    let Some(next_handle) = handle.checked_add(1) else {
        return 0;
    };
    bridge.next_world_restore = next_handle;
    bridge.prepared_world_restores.insert(
        handle,
        Box::new(PreparedMechanicsWorldRestore {
            catalog: request.catalog.value,
            state_revision_before: slot.world.state.revision(),
            candidate: Some(candidate),
            revisions,
            lifecycles,
            published: false,
        }),
    );
    *result = NativeMechanicsWorldRestoreHandle { value: handle };
    ABI_OK
}

unsafe extern "C" fn receipt_prepare_world_restore(
    context: *mut c_void,
    request: *const NativeMechanicsWorldRestoreRequest,
    result: *mut NativeMechanicsWorldRestoreHandle,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe {
        invoke_with_operation_diagnostic(
            context,
            receipt,
            b"PrepareWorldRestore",
            || prepare_world_restore(context, request, result),
            |_bridge| {
                (
                    b"MECHANICS_RESTORE_REJECTED".as_slice(),
                    b"Mechanics restore requires matching catalog, topology, and current revision."
                        .as_slice(),
                    String::new(),
                )
            },
        )
    }
}

unsafe extern "C" fn destroy_world_restore(
    context: *mut c_void,
    handle: NativeMechanicsWorldRestoreHandle,
) -> i32 {
    if context.is_null() || handle.value == 0 {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeMechanicsBridge>() };
    if bridge
        .world_restore_leases
        .values()
        .any(|prepared| *prepared == handle.value)
    {
        return 0;
    }
    i32::from(
        bridge
            .prepared_world_restores
            .remove(&handle.value)
            .is_some(),
    )
}

unsafe extern "C" fn read_world_restore(
    context: *mut c_void,
    handle: NativeMechanicsWorldRestoreHandle,
    result: *mut NativeMechanicsWorldRestoreLease,
) -> i32 {
    if context.is_null() || result.is_null() || handle.value == 0 {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeMechanicsBridge>() };
    let Some(prepared) = bridge
        .prepared_world_restores
        .get(&handle.value)
        .filter(|prepared| !prepared.published)
    else {
        return 0;
    };
    let lease = bridge.next_world_restore_lease;
    let Some(next_lease) = lease.checked_add(1) else {
        return 0;
    };
    let value = NativeMechanicsWorldRestoreLease {
        handle: NativeMechanicsWorldRestoreLeaseHandle { value: lease },
        state_revision_before: prepared.state_revision_before,
        state_revision_after: prepared
            .candidate
            .as_ref()
            .expect("unpublished mechanics restore has a candidate")
            .state
            .revision(),
        revisions: prepared.revisions.as_ptr(),
        revisions_len: prepared.revisions.len(),
        lifecycles: prepared.lifecycles.as_ptr(),
        lifecycles_len: prepared.lifecycles.len(),
    };
    bridge.next_world_restore_lease = next_lease;
    bridge.world_restore_leases.insert(lease, handle.value);
    *result = value;
    ABI_OK
}

unsafe extern "C" fn receipt_read_world_restore(
    context: *mut c_void,
    handle: NativeMechanicsWorldRestoreHandle,
    result: *mut NativeMechanicsWorldRestoreLease,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe {
        invoke_with_operation_diagnostic(
            context,
            receipt,
            b"ReadWorldRestore",
            || read_world_restore(context, handle, result),
            |_bridge| {
                (
                    b"MECHANICS_RESTORE_NOT_FOUND".as_slice(),
                    b"Prepared Mechanics restore was not found.".as_slice(),
                    String::new(),
                )
            },
        )
    }
}

unsafe extern "C" fn destroy_world_restore_lease(
    context: *mut c_void,
    handle: NativeMechanicsWorldRestoreLeaseHandle,
) -> i32 {
    if context.is_null() || handle.value == 0 {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeMechanicsBridge>() };
    i32::from(bridge.world_restore_leases.remove(&handle.value).is_some())
}

unsafe extern "C" fn publish_world_restore(
    context: *mut c_void,
    handle: NativeMechanicsWorldRestoreHandle,
) -> i32 {
    if context.is_null() || handle.value == 0 {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeMechanicsBridge>() };
    let Some(prepared) = bridge.prepared_world_restores.get_mut(&handle.value) else {
        return 0;
    };
    if prepared.published {
        return 0;
    }
    let Some(slot) = bridge.catalogs.get_mut(&prepared.catalog) else {
        return 0;
    };
    // Both candidate construction and all observable failure paths are complete before this
    // assignment. The synchronous ABI has no concurrent mutation path between prepare/publish.
    slot.world = prepared
        .candidate
        .take()
        .expect("unpublished mechanics restore has a candidate");
    prepared.published = true;
    ABI_OK
}

fn native_stacking(value: StackingPolicy) -> NativeMechanicsStackingPolicy {
    match value {
        StackingPolicy::Sum => NativeMechanicsStackingPolicy::Sum,
        StackingPolicy::Highest => NativeMechanicsStackingPolicy::Highest,
        StackingPolicy::Lowest => NativeMechanicsStackingPolicy::Lowest,
        StackingPolicy::UniqueBySource => NativeMechanicsStackingPolicy::UniqueBySource,
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
            initial_containment: Vec::new(),
            expected_state_revision: None,
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
            initial_containment: Vec::new(),
            expected_state_revision: None,
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
    if (!request.has_stats && request.stats_len != 0)
        || (!request.has_tracks && request.tracks_len != 0)
        || (!request.has_intrinsic_sources && request.intrinsic_sources_len != 0)
        || (!request.has_active_effects && request.active_effects_len != 0)
        || (!request.has_inventory
            && (request.inventory_stacks_len != 0 || request.inventory_capacity_limits_len != 0))
        || (!request.has_equipment && request.equipment_assignments_len != 0)
        || (!request.has_item
            && !matches!(
                unsafe { text(request.item_definition, "mechanics absent initial item") },
                Ok("")
            ))
    {
        return 0;
    }
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

/// Stages one canonical child -> owner relationship for the same cloned commit that admits the
/// owner and validates its initial Equipment component. The destination revision is captured once
/// and rechecked when commit begins; staging itself never mutates Engine state.
unsafe extern "C" fn stage_initial_containment(
    context: *mut c_void,
    request: *const NativeMechanicsInitialContainmentRequest,
) -> i32 {
    let Some((bridge, request)) = bridge_request(context, request) else {
        return 0;
    };
    let child = EntityId::new(request.child_entity_id);
    let Some(owner) = bridge.entities.get(&request.owner.value).cloned() else {
        return 0;
    };
    if owner.committed
        || child == owner.entity
        || bridge.canonical_entities.get(&child) != Some(&owner.catalog)
        || !bridge
            .catalogs
            .get(&owner.catalog)
            .is_some_and(|slot| slot.world.state.is_alive(child))
    {
        return 0;
    }
    let Some(binding) = bridge.entities.get_mut(&request.owner.value) else {
        return 0;
    };
    if binding
        .expected_state_revision
        .is_some_and(|expected| expected != request.expected_state_revision)
        || binding.initial_containment.contains(&child)
    {
        return 0;
    }
    binding.expected_state_revision = Some(request.expected_state_revision);
    binding.initial_containment.push(child);
    ABI_OK
}

unsafe extern "C" fn read_containment(
    context: *mut c_void,
    request: *const NativeMechanicsContainmentReadRequest,
    result: *mut NativeMechanicsContainmentReceipt,
) -> i32 {
    let Some((bridge, request, result)) = bridge_request_result(context, request, result) else {
        return 0;
    };
    let Some(binding) = bridge
        .binding(request.entity)
        .filter(|binding| binding.committed)
    else {
        return 0;
    };
    let Some(slot) = bridge.catalogs.get(&binding.catalog) else {
        return 0;
    };
    if !slot.world.state.is_alive(binding.entity) {
        return 0;
    }
    let container = slot.world.state.contained_in(binding.entity);
    *result = NativeMechanicsContainmentReceipt {
        child_entity_id: binding.entity.raw(),
        present: container.is_some(),
        container_entity_id: container.map_or(0, EntityId::raw),
        state_revision: slot.world.state.revision(),
    };
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
    if binding
        .expected_state_revision
        .is_some_and(|expected| expected != state_revision)
    {
        return 0;
    }
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
    for child in &binding.initial_containment {
        let expected = candidate.revision();
        if candidate
            .apply_relationship(
                expected,
                RelationshipCommand::SetContainment {
                    child: *child,
                    container: binding.entity,
                },
            )
            .is_err()
        {
            return 0;
        }
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
    let state_revision_after = candidate.revision();
    slot.world.state = candidate;
    let Some(lifecycle) = slot.world.admit(binding.entity) else {
        return 0;
    };
    if let Some(entry) = bridge.entities.get_mut(&handle.value) {
        entry.committed = true;
    }
    unsafe {
        *result = NativeMechanicsEntityReceipt {
            state_revision_before: state_revision,
            state_revision_after,
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
    result: *mut NativeMechanicsStatEvaluationLease,
) -> i32 {
    let Some((bridge, request, result)) = bridge_request_result(context, request, result) else {
        return 0;
    };
    let (Ok(stat), Ok(operation), Ok(request_sources)) = (
        unsafe { text(request.stat, "mechanics evaluation stat") }.and_then(parse::<StatId>),
        unsafe { text(request.operation, "mechanics evaluation operation") }
            .and_then(parse::<OperationId>),
        unsafe {
            borrowed_slice(
                request.request_sources,
                request.request_sources_len,
                "mechanics evaluation request sources",
            )
        }
        .and_then(parse_request_sources),
    ) else {
        return 0;
    };
    let Some(catalog_id) = bridge
        .binding(request.entity)
        .map(|binding| binding.catalog)
    else {
        return 0;
    };
    let (value, revision) = {
        let Some((state, catalog, entity)) = bridge.state_and_catalog_mut(request.entity) else {
            return 0;
        };
        let Ok(value) =
            StatService::evaluate(state, catalog, entity, &stat, &operation, &request_sources)
        else {
            return 0;
        };
        let Ok(revision) = state.component_revision::<StatsComponent>(entity) else {
            return 0;
        };
        (value, revision.revision())
    };
    let mut lease_text = CatalogLeaseText::default();
    let decisions = value
        .decisions
        .iter()
        .map(|decision| native_stat_decision(decision, &mut lease_text))
        .collect::<Vec<_>>();
    let observed_revisions = value
        .observed_revisions
        .iter()
        .map(native_observed_revision)
        .collect::<Vec<_>>();
    let metadata = NativeMechanicsStatEvaluationLease {
        handle: NativeMechanicsOperationLeaseHandle::default(),
        decisions: std::ptr::null(),
        decisions_len: decisions.len(),
        observed_revisions: std::ptr::null(),
        observed_revisions_len: observed_revisions.len(),
        catalog_id,
        catalog_version: lease_text.copy(value.catalog_version.as_str()),
        catalog_fingerprint: lease_text.copy(&value.catalog_fingerprint),
        entity_id: value.entity.raw(),
        stat: lease_text.copy(value.stat.as_str()),
        base: value.base.get(),
        after_additions: value.after_additions.get(),
        combined_scale_numerator: native_u128(value.combined_scale_numerator),
        combined_scale_denominator: native_u128(value.combined_scale_denominator),
        after_scaling: value.after_scaling.get(),
        unconstrained: value.unconstrained.get(),
        value: value.value.get(),
        minimum: value.minimum.get(),
        maximum: value.maximum.get(),
        stats_revision: stats_revision(value.entity, revision),
        source_cost: native_source_cost(value.source_cost),
    };
    let backing = OperationLeaseBacking {
        _text: lease_text.values,
        rows: OperationLeaseRows::StatEvaluation {
            decisions,
            observed_revisions,
        },
    };
    let Some(handle) = bridge.insert_operation_lease(backing) else {
        return 0;
    };
    let OperationLeaseRows::StatEvaluation {
        decisions,
        observed_revisions,
    } = &bridge
        .operation_leases
        .get(&handle.value)
        .expect("just inserted stat evaluation lease")
        .rows
    else {
        unreachable!("stat evaluation lease row kind matches its reader")
    };
    *result = NativeMechanicsStatEvaluationLease {
        handle,
        decisions: decisions.as_ptr(),
        observed_revisions: observed_revisions.as_ptr(),
        ..metadata
    };
    ABI_OK
}

unsafe extern "C" fn read_track(
    context: *mut c_void,
    request: *const NativeMechanicsTrackReadRequest,
    result: *mut NativeMechanicsTrackReadLease,
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
    let Some(catalog_id) = bridge
        .binding(request.entity)
        .map(|binding| binding.catalog)
    else {
        return 0;
    };
    let receipt: TrackReadReceipt = {
        let Some((state, catalog, entity)) = bridge.state_and_catalog_mut(request.entity) else {
            return 0;
        };
        let Ok(receipt) = TrackService::read(state, catalog, entity, &track, &operation) else {
            return 0;
        };
        receipt
    };
    let mut lease_text = CatalogLeaseText::default();
    let observed_revisions = receipt
        .observed_revisions
        .iter()
        .map(native_observed_revision)
        .collect::<Vec<_>>();
    let metadata = NativeMechanicsTrackReadLease {
        handle: NativeMechanicsOperationLeaseHandle::default(),
        observed_revisions: std::ptr::null(),
        observed_revisions_len: observed_revisions.len(),
        catalog_id,
        catalog_version: lease_text.copy(receipt.catalog_version.as_str()),
        catalog_fingerprint: lease_text.copy(&receipt.catalog_fingerprint),
        operation: lease_text.copy(receipt.operation.as_str()),
        entity_id: receipt.entity.raw(),
        track: lease_text.copy(receipt.track.as_str()),
        current: receipt.current.get(),
        minimum: receipt.minimum.get(),
        maximum: receipt.maximum.get(),
        revision: tracks_revision(receipt.entity, receipt.observed_tracks_revision),
        source_cost: native_source_cost(receipt.source_cost),
    };
    let Some((handle, observed_revisions)) =
        insert_track_operation_lease(bridge, lease_text, observed_revisions)
    else {
        return 0;
    };
    *result = NativeMechanicsTrackReadLease {
        handle,
        observed_revisions,
        ..metadata
    };
    ABI_OK
}

/// Publishes the exact `InventoryService::view` result for one already-bound
/// canonical entity. The service owns the copied rows and text until the
/// ordinary operation-lease release; this deliberately does not expose the
/// mutable inventory component or relationship storage directly.
unsafe extern "C" fn read_inventory_view(
    context: *mut c_void,
    entity: NativeMechanicsEntityHandle,
    result: *mut NativeMechanicsInventoryViewLease,
) -> i32 {
    if context.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeMechanicsBridge>() };
    let Some(catalog_id) = bridge.binding(entity).map(|binding| binding.catalog) else {
        return 0;
    };
    let (view, catalog_fingerprint) = {
        let Some((state, catalog, owner)) = bridge.state_and_catalog_mut(entity) else {
            return 0;
        };
        let Ok(view) = InventoryService::view(state, catalog, owner) else {
            return 0;
        };
        (view, catalog.fingerprint().to_owned())
    };

    let mut lease_text = CatalogLeaseText::default();
    let stacks = view
        .stacks()
        .iter()
        .map(|stack| NativeMechanicsInventoryViewStackRow {
            definition: lease_text.copy(stack.definition.as_str()),
            quantity: stack.quantity,
        })
        .collect::<Vec<_>>();
    let unique_items = view
        .unique_items()
        .iter()
        .map(|item| NativeMechanicsInventoryViewUniqueItemRow {
            entity_id: item.entity.raw(),
            definition: lease_text.copy(item.definition.as_str()),
        })
        .collect::<Vec<_>>();
    let capacity = view
        .capacity()
        .iter()
        .map(|usage| NativeMechanicsInventoryViewCapacityUsageRow {
            metric: lease_text.copy(usage.metric.as_str()),
            used: usage.used,
            has_maximum: usage.maximum.is_some(),
            maximum: usage.maximum.unwrap_or_default(),
        })
        .collect::<Vec<_>>();
    let metadata = NativeMechanicsInventoryViewLease {
        handle: NativeMechanicsOperationLeaseHandle::default(),
        stacks: std::ptr::null(),
        stacks_len: stacks.len(),
        unique_items: std::ptr::null(),
        unique_items_len: unique_items.len(),
        capacity: std::ptr::null(),
        capacity_len: capacity.len(),
        catalog_id,
        catalog_version: lease_text.copy(view.catalog_version().as_str()),
        catalog_fingerprint: lease_text.copy(&catalog_fingerprint),
        owner_entity_id: view.owner().raw(),
        inventory_revision: NativeMechanicsComponentRevision {
            entity_id: view.owner().raw(),
            revision: view.revision().revision(),
            component: NativeMechanicsRevisionComponent::Inventory,
            present: true,
        },
        relationship_state_revision: view.relationship_revision(),
        read_cost: native_inventory_read_cost(view.read_cost()),
    };
    let Some(handle) = bridge.insert_operation_lease(OperationLeaseBacking {
        _text: lease_text.values,
        rows: OperationLeaseRows::InventoryView {
            stacks,
            unique_items,
            capacity,
        },
    }) else {
        return 0;
    };
    let OperationLeaseRows::InventoryView {
        stacks,
        unique_items,
        capacity,
    } = &bridge
        .operation_leases
        .get(&handle.value)
        .expect("just inserted inventory view lease")
        .rows
    else {
        unreachable!("inventory view operation lease row kind matches its reader")
    };
    *result = NativeMechanicsInventoryViewLease {
        handle,
        stacks: stacks.as_ptr(),
        unique_items: unique_items.as_ptr(),
        capacity: capacity.as_ptr(),
        ..metadata
    };
    ABI_OK
}

unsafe extern "C" fn grant_inventory(
    context: *mut c_void,
    request: *const NativeMechanicsInventoryMutationRequest,
    result: *mut NativeMechanicsInventoryMutationLease,
) -> i32 {
    inventory_mutation(context, request, result, InventoryMutationKind::Grant)
}

unsafe extern "C" fn consume_inventory(
    context: *mut c_void,
    request: *const NativeMechanicsInventoryMutationRequest,
    result: *mut NativeMechanicsInventoryMutationLease,
) -> i32 {
    inventory_mutation(context, request, result, InventoryMutationKind::Consume)
}

unsafe fn inventory_mutation(
    context: *mut c_void,
    request: *const NativeMechanicsInventoryMutationRequest,
    result: *mut NativeMechanicsInventoryMutationLease,
    kind: InventoryMutationKind,
) -> i32 {
    let Some((bridge, request, result)) = bridge_request_result(context, request, result) else {
        return 0;
    };
    let (Ok(operation), Ok(source), Ok(item)) = (
        unsafe { text(request.operation, "inventory operation") }.and_then(parse::<OperationId>),
        parse_inventory_request_source_identity(request),
        unsafe { text(request.item, "inventory item") }
            .and_then(parse::<gameplay_mechanics::ItemDefinitionId>),
    ) else {
        return 0;
    };
    let Some(catalog_id) = bridge.binding(request.owner).map(|binding| binding.catalog) else {
        return 0;
    };
    let receipt = {
        let Some((state, catalog, owner)) = bridge.state_and_catalog_mut(request.owner) else {
            return 0;
        };
        let Ok(actual) = state.component_revision::<InventoryComponent>(owner) else {
            return 0;
        };
        let Some(expected_revision) = guarded_revision(
            request.revision_guard,
            request.expected_revision.entity_id,
            request.expected_revision.revision,
            request.expected_revision.component,
            owner,
            actual,
            NativeMechanicsRevisionComponent::Inventory,
        ) else {
            return 0;
        };
        let request = InventoryMutationRequest {
            operation,
            source,
            owner,
            item,
            quantity: request.quantity,
            expected_revision,
        };
        match kind {
            InventoryMutationKind::Grant => InventoryService::grant(state, catalog, request),
            InventoryMutationKind::Consume => InventoryService::consume(state, catalog, request),
        }
    };
    let Ok(receipt) = receipt else {
        return 0;
    };
    let mut lease_text = CatalogLeaseText::default();
    let capacity_before = native_capacity_usage(&receipt.capacity_before, &mut lease_text);
    let capacity_after = native_capacity_usage(&receipt.capacity_after, &mut lease_text);
    let metadata = NativeMechanicsInventoryMutationLease {
        handle: NativeMechanicsOperationLeaseHandle::default(),
        capacity_before: std::ptr::null(),
        capacity_before_len: capacity_before.len(),
        capacity_after: std::ptr::null(),
        capacity_after_len: capacity_after.len(),
        catalog_id,
        catalog_version: lease_text.copy(receipt.catalog_version.as_str()),
        catalog_fingerprint: lease_text.copy(&receipt.catalog_fingerprint),
        operation: lease_text.copy(receipt.operation.as_str()),
        source: native_source_identity(&receipt.source, &mut lease_text),
        kind: match receipt.kind {
            InventoryMutationKind::Grant => NativeMechanicsInventoryMutationKind::Grant,
            InventoryMutationKind::Consume => NativeMechanicsInventoryMutationKind::Consume,
        },
        owner_entity_id: receipt.owner.raw(),
        item: lease_text.copy(receipt.item.as_str()),
        requested_quantity: receipt.requested_quantity,
        before_quantity: receipt.before_quantity,
        after_quantity: receipt.after_quantity,
        observed_inventory_revision: inventory_revision(
            receipt.owner,
            receipt.observed_inventory_revision,
        ),
        committed_inventory_revision: inventory_revision(
            receipt.owner,
            receipt.committed_inventory_revision,
        ),
        read_cost: native_inventory_read_cost(receipt.read_cost),
    };
    let Some(handle) = bridge.insert_operation_lease(OperationLeaseBacking {
        _text: lease_text.values,
        rows: OperationLeaseRows::InventoryMutation {
            capacity_before,
            capacity_after,
        },
    }) else {
        return 0;
    };
    let OperationLeaseRows::InventoryMutation {
        capacity_before,
        capacity_after,
    } = &bridge
        .operation_leases
        .get(&handle.value)
        .expect("just inserted inventory mutation lease")
        .rows
    else {
        unreachable!("inventory mutation lease row kind matches its reader")
    };
    *result = NativeMechanicsInventoryMutationLease {
        handle,
        capacity_before: capacity_before.as_ptr(),
        capacity_after: capacity_after.as_ptr(),
        ..metadata
    };
    ABI_OK
}

unsafe extern "C" fn transfer_inventory(
    context: *mut c_void,
    request: *const NativeMechanicsInventoryTransferRequest,
    result: *mut NativeMechanicsInventoryTransferLease,
) -> i32 {
    let Some((bridge, request, result)) = bridge_request_result(context, request, result) else {
        return 0;
    };
    let (Ok(operation), Ok(source), Ok(item)) = (
        unsafe { text(request.operation, "inventory transfer operation") }
            .and_then(parse::<OperationId>),
        parse_inventory_transfer_request_source_identity(request),
        unsafe { text(request.item, "inventory transfer item") }
            .and_then(parse::<gameplay_mechanics::ItemDefinitionId>),
    ) else {
        return 0;
    };
    let (Some(from_binding), Some(to_binding)) = (
        bridge.binding(request.from_owner).cloned(),
        bridge.binding(request.to_owner).cloned(),
    ) else {
        return 0;
    };
    if from_binding.catalog != to_binding.catalog {
        return 0;
    }
    let catalog_id = from_binding.catalog;
    let receipt = {
        let Some((state, catalog, from_owner)) = bridge.state_and_catalog_mut(request.from_owner)
        else {
            return 0;
        };
        let to_owner = to_binding.entity;
        let (Ok(from_actual), Ok(to_actual)) = (
            state.component_revision::<InventoryComponent>(from_owner),
            state.component_revision::<InventoryComponent>(to_owner),
        ) else {
            return 0;
        };
        let Some(expected_from_revision) = guarded_revision(
            request.from_revision_guard,
            request.expected_from_revision.entity_id,
            request.expected_from_revision.revision,
            request.expected_from_revision.component,
            from_owner,
            from_actual,
            NativeMechanicsRevisionComponent::Inventory,
        ) else {
            return 0;
        };
        let Some(expected_to_revision) = guarded_revision(
            request.to_revision_guard,
            request.expected_to_revision.entity_id,
            request.expected_to_revision.revision,
            request.expected_to_revision.component,
            to_owner,
            to_actual,
            NativeMechanicsRevisionComponent::Inventory,
        ) else {
            return 0;
        };
        InventoryService::transfer(
            state,
            catalog,
            InventoryTransferRequest {
                operation,
                source,
                from_owner,
                to_owner,
                item,
                quantity: request.quantity,
                expected_from_revision,
                expected_to_revision,
            },
        )
    };
    let Ok(receipt) = receipt else {
        return 0;
    };
    let mut lease_text = CatalogLeaseText::default();
    let from_capacity_before =
        native_capacity_usage(&receipt.from_capacity_before, &mut lease_text);
    let from_capacity_after = native_capacity_usage(&receipt.from_capacity_after, &mut lease_text);
    let to_capacity_before = native_capacity_usage(&receipt.to_capacity_before, &mut lease_text);
    let to_capacity_after = native_capacity_usage(&receipt.to_capacity_after, &mut lease_text);
    let metadata = NativeMechanicsInventoryTransferLease {
        handle: NativeMechanicsOperationLeaseHandle::default(),
        from_capacity_before: std::ptr::null(),
        from_capacity_before_len: from_capacity_before.len(),
        from_capacity_after: std::ptr::null(),
        from_capacity_after_len: from_capacity_after.len(),
        to_capacity_before: std::ptr::null(),
        to_capacity_before_len: to_capacity_before.len(),
        to_capacity_after: std::ptr::null(),
        to_capacity_after_len: to_capacity_after.len(),
        catalog_id,
        catalog_version: lease_text.copy(receipt.catalog_version.as_str()),
        catalog_fingerprint: lease_text.copy(&receipt.catalog_fingerprint),
        operation: lease_text.copy(receipt.operation.as_str()),
        source: native_source_identity(&receipt.source, &mut lease_text),
        from_owner_entity_id: receipt.from_owner.raw(),
        to_owner_entity_id: receipt.to_owner.raw(),
        item: lease_text.copy(receipt.item.as_str()),
        quantity: receipt.quantity,
        from_before_quantity: receipt.from_before,
        from_after_quantity: receipt.from_after,
        to_before_quantity: receipt.to_before,
        to_after_quantity: receipt.to_after,
        observed_from_inventory_revision: inventory_revision(
            receipt.from_owner,
            receipt.observed_from_revision,
        ),
        committed_from_inventory_revision: inventory_revision(
            receipt.from_owner,
            receipt.committed_from_revision,
        ),
        observed_to_inventory_revision: inventory_revision(
            receipt.to_owner,
            receipt.observed_to_revision,
        ),
        committed_to_inventory_revision: inventory_revision(
            receipt.to_owner,
            receipt.committed_to_revision,
        ),
        read_cost: native_inventory_read_cost(receipt.read_cost),
    };
    let Some(handle) = bridge.insert_operation_lease(OperationLeaseBacking {
        _text: lease_text.values,
        rows: OperationLeaseRows::InventoryTransfer {
            from_capacity_before,
            from_capacity_after,
            to_capacity_before,
            to_capacity_after,
        },
    }) else {
        return 0;
    };
    let OperationLeaseRows::InventoryTransfer {
        from_capacity_before,
        from_capacity_after,
        to_capacity_before,
        to_capacity_after,
    } = &bridge
        .operation_leases
        .get(&handle.value)
        .expect("just inserted inventory transfer lease")
        .rows
    else {
        unreachable!("inventory transfer lease row kind matches its reader")
    };
    *result = NativeMechanicsInventoryTransferLease {
        handle,
        from_capacity_before: from_capacity_before.as_ptr(),
        from_capacity_after: from_capacity_after.as_ptr(),
        to_capacity_before: to_capacity_before.as_ptr(),
        to_capacity_after: to_capacity_after.as_ptr(),
        ..metadata
    };
    ABI_OK
}

/// Delegates the named generated C# unique-item transfer capability directly
/// to `EquipmentService::transfer_unique_item`. Canonical same-catalog
/// bindings and optional Inventory guards are the only bridge concerns; item
/// eligibility, capacity, equipped-item rejection, and atomic containment
/// policy remain in `gameplay-mechanics`.
unsafe extern "C" fn transfer_unique_item(
    context: *mut c_void,
    request: *const NativeMechanicsUniqueItemTransferRequest,
    result: *mut NativeMechanicsUniqueItemTransferLease,
) -> i32 {
    let Some((bridge, request, result)) = bridge_request_result(context, request, result) else {
        return 0;
    };
    let (Ok(operation), Ok(source)) = (
        unsafe { text(request.operation, "unique item transfer operation") }
            .and_then(parse::<OperationId>),
        parse_unique_item_transfer_request_source_identity(request),
    ) else {
        return 0;
    };
    let (Some(item_binding), Some(from_binding), Some(to_binding)) = (
        bridge.binding(request.item).cloned(),
        bridge.binding(request.from_owner).cloned(),
        bridge.binding(request.to_owner).cloned(),
    ) else {
        return 0;
    };
    if item_binding.catalog != from_binding.catalog || item_binding.catalog != to_binding.catalog {
        return 0;
    }
    let catalog_id = item_binding.catalog;
    let receipt = {
        let Some((state, catalog, item)) = bridge.state_and_catalog_mut(request.item) else {
            return 0;
        };
        let from_owner = from_binding.entity;
        let to_owner = to_binding.entity;
        let (Ok(from_actual), Ok(to_actual)) = (
            state.component_revision::<InventoryComponent>(from_owner),
            state.component_revision::<InventoryComponent>(to_owner),
        ) else {
            return 0;
        };
        let Some(expected_from_inventory_revision) = guarded_revision(
            request.from_revision_guard,
            request.expected_from_revision.entity_id,
            request.expected_from_revision.revision,
            request.expected_from_revision.component,
            from_owner,
            from_actual,
            NativeMechanicsRevisionComponent::Inventory,
        ) else {
            return 0;
        };
        let Some(expected_to_inventory_revision) = guarded_revision(
            request.to_revision_guard,
            request.expected_to_revision.entity_id,
            request.expected_to_revision.revision,
            request.expected_to_revision.component,
            to_owner,
            to_actual,
            NativeMechanicsRevisionComponent::Inventory,
        ) else {
            return 0;
        };
        EquipmentService::transfer_unique_item(
            state,
            catalog,
            ItemTransferRequest {
                operation,
                source,
                item,
                from_owner,
                to_owner,
                expected_relationship_revision: request.expected_relationship_revision,
                expected_from_inventory_revision,
                expected_to_inventory_revision,
            },
        )
    };
    let Ok(receipt) = receipt else {
        return 0;
    };
    let mut lease_text = CatalogLeaseText::default();
    let from_capacity_before =
        native_capacity_usage(&receipt.from_capacity_before, &mut lease_text);
    let from_capacity_after = native_capacity_usage(&receipt.from_capacity_after, &mut lease_text);
    let to_capacity_before = native_capacity_usage(&receipt.to_capacity_before, &mut lease_text);
    let to_capacity_after = native_capacity_usage(&receipt.to_capacity_after, &mut lease_text);
    let metadata = NativeMechanicsUniqueItemTransferLease {
        handle: NativeMechanicsOperationLeaseHandle::default(),
        from_capacity_before: std::ptr::null(),
        from_capacity_before_len: from_capacity_before.len(),
        from_capacity_after: std::ptr::null(),
        from_capacity_after_len: from_capacity_after.len(),
        to_capacity_before: std::ptr::null(),
        to_capacity_before_len: to_capacity_before.len(),
        to_capacity_after: std::ptr::null(),
        to_capacity_after_len: to_capacity_after.len(),
        catalog_id,
        catalog_version: lease_text.copy(receipt.catalog_version.as_str()),
        catalog_fingerprint: lease_text.copy(&receipt.catalog_fingerprint),
        operation: lease_text.copy(receipt.operation.as_str()),
        source: native_source_identity(&receipt.source, &mut lease_text),
        item_entity_id: receipt.item.raw(),
        from_owner_entity_id: receipt.from_owner.raw(),
        to_owner_entity_id: receipt.to_owner.raw(),
        relationship_revision_before: receipt.revision_before,
        relationship_revision_after: receipt.revision_after,
        observed_from_inventory_revision: inventory_revision(
            receipt.from_owner,
            receipt.observed_from_inventory_revision,
        ),
        observed_to_inventory_revision: inventory_revision(
            receipt.to_owner,
            receipt.observed_to_inventory_revision,
        ),
        read_cost: native_inventory_read_cost(receipt.read_cost),
    };
    let Some(handle) = bridge.insert_operation_lease(OperationLeaseBacking {
        _text: lease_text.values,
        rows: OperationLeaseRows::UniqueItemTransfer {
            from_capacity_before,
            from_capacity_after,
            to_capacity_before,
            to_capacity_after,
        },
    }) else {
        return 0;
    };
    let OperationLeaseRows::UniqueItemTransfer {
        from_capacity_before,
        from_capacity_after,
        to_capacity_before,
        to_capacity_after,
    } = &bridge
        .operation_leases
        .get(&handle.value)
        .expect("just inserted unique-item transfer lease")
        .rows
    else {
        unreachable!("unique-item transfer lease row kind matches its reader")
    };
    *result = NativeMechanicsUniqueItemTransferLease {
        handle,
        from_capacity_before: from_capacity_before.as_ptr(),
        from_capacity_after: from_capacity_after.as_ptr(),
        to_capacity_before: to_capacity_before.as_ptr(),
        to_capacity_after: to_capacity_after.as_ptr(),
        ..metadata
    };
    ABI_OK
}

/// Delegates the named generated C# unique-item materialization capability to
/// `ItemService::materialize_unique`. The product has already allocated the
/// canonical active identity and supplied its name through `bind_entity`; this
/// callback promotes that uncommitted binding only with the owner's accepted
/// candidate state and lifecycle stamp.
unsafe extern "C" fn materialize_unique_item(
    context: *mut c_void,
    request: *const NativeMechanicsUniqueItemMaterializationRequest,
    result: *mut NativeMechanicsUniqueItemMaterializationLease,
) -> i32 {
    let Some((bridge, request, result)) = bridge_request_result(context, request, result) else {
        return 0;
    };
    let Ok(definition) = unsafe { text(request.definition, "unique item definition") }
        .and_then(parse::<gameplay_mechanics::ItemDefinitionId>)
    else {
        return 0;
    };
    let (Some(item_binding), Some(container_binding)) = (
        bridge.entities.get(&request.item.value).cloned(),
        bridge.binding(request.container).cloned(),
    ) else {
        return 0;
    };
    if item_binding.committed
        || item_binding.catalog != container_binding.catalog
        || item_binding.entity == container_binding.entity
        || bridge.next_operation_lease == u64::MAX
    {
        return 0;
    }
    let catalog_id = item_binding.catalog;
    let (receipt, lifecycle) = {
        let Some(slot) = bridge.catalogs.get_mut(&catalog_id) else {
            return 0;
        };
        let Some(catalog) = slot.catalog.as_ref() else {
            return 0;
        };
        if !slot.world.is_active(container_binding.entity) || slot.world.next_stamp == u64::MAX {
            return 0;
        }
        let mut candidate = slot.world.state.clone();
        let Ok(receipt) = ItemService::materialize_unique(
            &mut candidate,
            catalog,
            UniqueItemMaterializationRequest {
                entity: EntityDefinition::new(item_binding.entity, &item_binding.identity),
                item: definition,
                container: container_binding.entity,
                expected_state_revision: request.expected_state_revision,
            },
        ) else {
            return 0;
        };

        // Both candidate publication and the lifecycle stamp have been fully
        // preflighted. There is no fallible work after this point, so rejected
        // owner operations leave state, lifecycle, and binding commitment intact.
        let stamp = slot.world.next_stamp;
        slot.world.next_stamp += 1;
        slot.world.state = candidate;
        let lifecycle = NativeMechanicsLifecycleReceipt {
            entity_id: item_binding.entity.raw(),
            lifecycle: NativeMechanicsEntityLifecycle::Active,
            stamp,
        };
        slot.world.lifecycle.insert(
            item_binding.entity,
            LifecycleRecord {
                lifecycle: NativeMechanicsEntityLifecycle::Active,
                stamp,
            },
        );
        (receipt, lifecycle)
    };
    bridge
        .entities
        .get_mut(&request.item.value)
        .expect("materialized binding remains present")
        .committed = true;

    let mut lease_text = CatalogLeaseText::default();
    let metadata = NativeMechanicsUniqueItemMaterializationLease {
        handle: NativeMechanicsOperationLeaseHandle::default(),
        catalog_id,
        catalog_version: lease_text.copy(receipt.catalog_version.as_str()),
        catalog_fingerprint: lease_text.copy(&receipt.catalog_fingerprint),
        item_entity_id: receipt.entity.raw(),
        item_definition: lease_text.copy(receipt.item.as_str()),
        container_entity_id: receipt.container.raw(),
        observed_state_revision: receipt.observed_state_revision,
        admitted_state_revision: receipt.admitted_state_revision,
        attached_state_revision: receipt.attached_state_revision,
        committed_state_revision: receipt.committed_state_revision,
        observed_item_revision: receipt.observed_item_revision,
        committed_item_revision: receipt.committed_item_revision,
        had_containment_before: receipt.containment_before.is_some(),
        containment_before_entity_id: receipt.containment_before.map(EntityId::raw).unwrap_or(0),
        has_containment_after: receipt.containment_after.is_some(),
        containment_after_entity_id: receipt.containment_after.map(EntityId::raw).unwrap_or(0),
        lifecycle,
    };
    let Some(handle) = bridge.insert_operation_lease(OperationLeaseBacking {
        _text: lease_text.values,
        rows: OperationLeaseRows::UniqueItemMaterialization,
    }) else {
        unreachable!("operation lease counter was preflighted before publication")
    };
    *result = NativeMechanicsUniqueItemMaterializationLease { handle, ..metadata };
    ABI_OK
}

/// Delegates the named generated C# unique-item destruction capability to
/// `ItemService::destroy_unique`. Destruction is staged on a candidate so the
/// gameplay mutation and native terminal lifecycle record publish together.
/// The committed binding remains lease-owned until C# disposes it; release is
/// deliberately not a second lifecycle transition.
unsafe extern "C" fn destroy_unique_item(
    context: *mut c_void,
    request: *const NativeMechanicsUniqueItemDestroyRequest,
    result: *mut NativeMechanicsUniqueItemDestroyLease,
) -> i32 {
    let Some((bridge, request, result)) = bridge_request_result(context, request, result) else {
        return 0;
    };
    let (Ok(operation), Ok(source)) = (
        unsafe { text(request.operation, "unique item destroy operation") }
            .and_then(parse::<OperationId>),
        parse_unique_item_destroy_request_source_identity(request),
    ) else {
        return 0;
    };
    let Some(item_binding) = bridge.binding(request.item).cloned() else {
        return 0;
    };
    if bridge.next_operation_lease == u64::MAX {
        return 0;
    }
    let catalog_id = item_binding.catalog;
    let (receipt, lifecycle) = {
        let Some(slot) = bridge.catalogs.get_mut(&catalog_id) else {
            return 0;
        };
        let Some(catalog) = slot.catalog.as_ref() else {
            return 0;
        };
        if !slot.world.is_active(item_binding.entity) || slot.world.next_stamp == u64::MAX {
            return 0;
        }
        let mut candidate = slot.world.state.clone();
        let Ok(receipt) = ItemService::destroy_unique(
            &mut candidate,
            catalog,
            ItemDestroyRequest {
                operation,
                source,
                item: item_binding.entity,
                expected_state_revision: request.expected_state_revision,
            },
        ) else {
            return 0;
        };

        let stamp = slot.world.next_stamp;
        slot.world.next_stamp += 1;
        slot.world.state = candidate;
        let lifecycle = NativeMechanicsLifecycleReceipt {
            entity_id: item_binding.entity.raw(),
            lifecycle: NativeMechanicsEntityLifecycle::Tombstoned,
            stamp,
        };
        slot.world.lifecycle.insert(
            item_binding.entity,
            LifecycleRecord {
                lifecycle: NativeMechanicsEntityLifecycle::Tombstoned,
                stamp,
            },
        );
        (receipt, lifecycle)
    };

    let mut lease_text = CatalogLeaseText::default();
    let metadata = NativeMechanicsUniqueItemDestroyLease {
        handle: NativeMechanicsOperationLeaseHandle::default(),
        catalog_id,
        catalog_version: lease_text.copy(receipt.catalog_version.as_str()),
        catalog_fingerprint: lease_text.copy(&receipt.catalog_fingerprint),
        operation: lease_text.copy(receipt.operation.as_str()),
        source: native_source_identity(&receipt.source, &mut lease_text),
        item_entity_id: receipt.item.raw(),
        has_former_owner: receipt.former_owner.is_some(),
        former_owner_entity_id: receipt.former_owner.map(EntityId::raw).unwrap_or(0),
        revision_before: receipt.revision_before,
        revision_after: receipt.revision_after,
        lifecycle,
    };
    let Some(handle) = bridge.insert_operation_lease(OperationLeaseBacking {
        _text: lease_text.values,
        rows: OperationLeaseRows::UniqueItemDestroy,
    }) else {
        unreachable!("operation lease counter was preflighted before publication")
    };
    *result = NativeMechanicsUniqueItemDestroyLease { handle, ..metadata };
    ABI_OK
}

/// Delegates the named generated C# equip capability directly to
/// `EquipmentService::equip`. The only ABI policy is bounded foreign-span
/// validation and canonical binding identity; all item, slot, exclusivity,
/// catalog, and source behavior remains the upstream service's behavior.
unsafe extern "C" fn equip_equipment(
    context: *mut c_void,
    request: *const NativeMechanicsEquipmentEquipRequest,
    result: *mut NativeMechanicsEquipmentMutationLease,
) -> i32 {
    let Some((bridge, request, result)) = bridge_request_result(context, request, result) else {
        return 0;
    };
    if request.slots_len > usize::from(gameplay_mechanics::MAX_EQUIPMENT_SLOTS_PER_ITEM) {
        return 0;
    }
    let (Ok(operation), Ok(source), Ok(slots)) = (
        unsafe { text(request.operation, "equipment equip operation") }
            .and_then(parse::<OperationId>),
        parse_equipment_equip_source_identity(request),
        unsafe { borrowed_slice(request.slots, request.slots_len, "equipment equip slots") }
            .and_then(parse_equipment_slots),
    ) else {
        return 0;
    };
    let (Some(owner_binding), Some(item_binding)) = (
        bridge.binding(request.owner).cloned(),
        bridge.binding(request.item).cloned(),
    ) else {
        return 0;
    };
    if owner_binding.catalog != item_binding.catalog {
        return 0;
    }
    let catalog_id = owner_binding.catalog;
    let receipt = {
        let Some((state, catalog, owner)) = bridge.state_and_catalog_mut(request.owner) else {
            return 0;
        };
        let Ok(actual) = state.component_revision::<EquipmentComponent>(owner) else {
            return 0;
        };
        let Some(expected_equipment_revision) = guarded_revision(
            request.equipment_revision_guard,
            request.expected_equipment_revision.entity_id,
            request.expected_equipment_revision.revision,
            request.expected_equipment_revision.component,
            owner,
            actual,
            NativeMechanicsRevisionComponent::Equipment,
        ) else {
            return 0;
        };
        EquipmentService::equip(
            state,
            catalog,
            EquipmentEquipRequest {
                operation,
                source,
                owner,
                item: item_binding.entity,
                slots,
                expected_equipment_revision,
                expected_state_revision: request.expected_state_revision,
            },
        )
    };
    publish_equipment_mutation(bridge, result, catalog_id, receipt)
}

/// Delegates the named generated C# unequip capability directly to
/// `EquipmentService::unequip` while retaining the exact typed receipt.
unsafe extern "C" fn unequip_equipment(
    context: *mut c_void,
    request: *const NativeMechanicsEquipmentUnequipRequest,
    result: *mut NativeMechanicsEquipmentMutationLease,
) -> i32 {
    let Some((bridge, request, result)) = bridge_request_result(context, request, result) else {
        return 0;
    };
    let (Ok(operation), Ok(source)) = (
        unsafe { text(request.operation, "equipment unequip operation") }
            .and_then(parse::<OperationId>),
        parse_equipment_unequip_source_identity(request),
    ) else {
        return 0;
    };
    let (Some(owner_binding), Some(item_binding)) = (
        bridge.binding(request.owner).cloned(),
        bridge.binding(request.item).cloned(),
    ) else {
        return 0;
    };
    if owner_binding.catalog != item_binding.catalog {
        return 0;
    }
    let catalog_id = owner_binding.catalog;
    let receipt = {
        let Some((state, catalog, owner)) = bridge.state_and_catalog_mut(request.owner) else {
            return 0;
        };
        let Ok(actual) = state.component_revision::<EquipmentComponent>(owner) else {
            return 0;
        };
        let Some(expected_equipment_revision) = guarded_revision(
            request.equipment_revision_guard,
            request.expected_equipment_revision.entity_id,
            request.expected_equipment_revision.revision,
            request.expected_equipment_revision.component,
            owner,
            actual,
            NativeMechanicsRevisionComponent::Equipment,
        ) else {
            return 0;
        };
        EquipmentService::unequip(
            state,
            catalog,
            EquipmentUnequipRequest {
                operation,
                source,
                owner,
                item: item_binding.entity,
                expected_equipment_revision,
                expected_state_revision: request.expected_state_revision,
            },
        )
    };
    publish_equipment_mutation(bridge, result, catalog_id, receipt)
}

/// Delegates the named generated C# swap capability directly to
/// `EquipmentService::swap`. The service keeps atomicity and validates the
/// outgoing assignment before the incoming assignment replaces it.
unsafe extern "C" fn swap_equipment(
    context: *mut c_void,
    request: *const NativeMechanicsEquipmentSwapRequest,
    result: *mut NativeMechanicsEquipmentMutationLease,
) -> i32 {
    let Some((bridge, request, result)) = bridge_request_result(context, request, result) else {
        return 0;
    };
    if request.incoming_slots_len > usize::from(gameplay_mechanics::MAX_EQUIPMENT_SLOTS_PER_ITEM) {
        return 0;
    }
    let (Ok(operation), Ok(source), Ok(incoming_slots)) = (
        unsafe { text(request.operation, "equipment swap operation") }
            .and_then(parse::<OperationId>),
        parse_equipment_swap_source_identity(request),
        unsafe {
            borrowed_slice(
                request.incoming_slots,
                request.incoming_slots_len,
                "equipment swap incoming slots",
            )
        }
        .and_then(parse_equipment_slots),
    ) else {
        return 0;
    };
    let (Some(owner_binding), Some(outgoing_binding), Some(incoming_binding)) = (
        bridge.binding(request.owner).cloned(),
        bridge.binding(request.outgoing_item).cloned(),
        bridge.binding(request.incoming_item).cloned(),
    ) else {
        return 0;
    };
    if owner_binding.catalog != outgoing_binding.catalog
        || owner_binding.catalog != incoming_binding.catalog
    {
        return 0;
    }
    let catalog_id = owner_binding.catalog;
    let receipt = {
        let Some((state, catalog, owner)) = bridge.state_and_catalog_mut(request.owner) else {
            return 0;
        };
        let Ok(actual) = state.component_revision::<EquipmentComponent>(owner) else {
            return 0;
        };
        let Some(expected_equipment_revision) = guarded_revision(
            request.equipment_revision_guard,
            request.expected_equipment_revision.entity_id,
            request.expected_equipment_revision.revision,
            request.expected_equipment_revision.component,
            owner,
            actual,
            NativeMechanicsRevisionComponent::Equipment,
        ) else {
            return 0;
        };
        EquipmentService::swap(
            state,
            catalog,
            EquipmentSwapRequest {
                operation,
                source,
                owner,
                outgoing_item: outgoing_binding.entity,
                incoming_item: incoming_binding.entity,
                incoming_slots,
                expected_equipment_revision,
                expected_state_revision: request.expected_state_revision,
            },
        )
    };
    publish_equipment_mutation(bridge, result, catalog_id, receipt)
}

fn publish_equipment_mutation(
    bridge: &mut RuntimeMechanicsBridge,
    result: &mut NativeMechanicsEquipmentMutationLease,
    catalog_id: u64,
    receipt: Result<
        gameplay_mechanics::EquipmentMutationReceipt,
        gameplay_mechanics::MechanicsError,
    >,
) -> i32 {
    let Ok(receipt) = receipt else {
        return 0;
    };
    let mut lease_text = CatalogLeaseText::default();
    let changes = receipt
        .changes
        .iter()
        .map(|change| native_equipment_slot_change(change, &mut lease_text))
        .collect::<Vec<_>>();
    let observed_item_revisions = receipt
        .observed_item_revisions
        .iter()
        .map(native_observed_revision)
        .collect::<Vec<_>>();
    let observed_revisions = receipt
        .observed_revisions
        .iter()
        .map(native_observed_revision)
        .collect::<Vec<_>>();
    let metadata = NativeMechanicsEquipmentMutationLease {
        handle: NativeMechanicsOperationLeaseHandle::default(),
        changes: std::ptr::null(),
        changes_len: changes.len(),
        observed_item_revisions: std::ptr::null(),
        observed_item_revisions_len: observed_item_revisions.len(),
        observed_revisions: std::ptr::null(),
        observed_revisions_len: observed_revisions.len(),
        catalog_id,
        catalog_version: lease_text.copy(receipt.catalog_version.as_str()),
        catalog_fingerprint: lease_text.copy(&receipt.catalog_fingerprint),
        operation: lease_text.copy(receipt.operation.as_str()),
        source: native_source_identity(&receipt.source, &mut lease_text),
        kind: match receipt.kind {
            EquipmentMutationKind::Equip => NativeMechanicsEquipmentMutationKind::Equip,
            EquipmentMutationKind::Unequip => NativeMechanicsEquipmentMutationKind::Unequip,
            EquipmentMutationKind::Swap => NativeMechanicsEquipmentMutationKind::Swap,
        },
        owner_entity_id: receipt.owner.raw(),
        item_entity_id: receipt.item.raw(),
        has_replaced_item: receipt.replaced_item.is_some(),
        replaced_item_entity_id: receipt.replaced_item.map(EntityId::raw).unwrap_or_default(),
        observed_state_revision: receipt.observed_state_revision,
        committed_state_revision: receipt.committed_state_revision,
        observed_equipment_revision: equipment_revision(
            receipt.owner,
            receipt.observed_equipment_revision,
        ),
        committed_equipment_revision: equipment_revision(
            receipt.owner,
            receipt.committed_equipment_revision,
        ),
        source_activations: receipt.source_activations as u64,
        tracks_validated: receipt.tracks_validated as u64,
        source_cost: native_source_cost(receipt.source_cost),
    };
    let Some(handle) = bridge.insert_operation_lease(OperationLeaseBacking {
        _text: lease_text.values,
        rows: OperationLeaseRows::EquipmentMutation {
            changes,
            observed_item_revisions,
            observed_revisions,
        },
    }) else {
        return 0;
    };
    let OperationLeaseRows::EquipmentMutation {
        changes,
        observed_item_revisions,
        observed_revisions,
    } = &bridge
        .operation_leases
        .get(&handle.value)
        .expect("just inserted equipment mutation lease")
        .rows
    else {
        unreachable!("equipment mutation operation lease row kind matches its reader")
    };
    *result = NativeMechanicsEquipmentMutationLease {
        handle,
        changes: changes.as_ptr(),
        observed_item_revisions: observed_item_revisions.as_ptr(),
        observed_revisions: observed_revisions.as_ptr(),
        ..metadata
    };
    ABI_OK
}

unsafe extern "C" fn set_stat_base(
    context: *mut c_void,
    request: *const NativeMechanicsStatBaseMutationRequest,
    result: *mut NativeMechanicsStatMutationLease,
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
    let Some(catalog_id) = bridge
        .binding(request.entity)
        .map(|binding| binding.catalog)
    else {
        return 0;
    };
    let request_source = gameplay_mechanics::SourceInstanceIdentity::Request {
        operation: operation.clone(),
        instance: source,
    };
    let receipt = {
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
        receipt
    };
    let mut lease_text = CatalogLeaseText::default();
    let observed_revisions = receipt
        .observed_revisions
        .iter()
        .map(native_observed_revision)
        .collect::<Vec<_>>();
    let metadata = NativeMechanicsStatMutationLease {
        handle: NativeMechanicsOperationLeaseHandle::default(),
        observed_revisions: std::ptr::null(),
        observed_revisions_len: observed_revisions.len(),
        catalog_id,
        catalog_version: lease_text.copy(receipt.catalog_version.as_str()),
        catalog_fingerprint: lease_text.copy(&receipt.catalog_fingerprint),
        operation: lease_text.copy(receipt.operation.as_str()),
        source: native_source_identity(&receipt.source, &mut lease_text),
        entity_id: receipt.entity.raw(),
        stat: lease_text.copy(receipt.stat.as_str()),
        before: receipt.before.get(),
        after: receipt.after.get(),
        minimum: receipt.minimum.get(),
        maximum: receipt.maximum.get(),
        observed_revision: stats_revision(receipt.entity, receipt.observed_stats_revision),
        committed_revision: stats_revision(receipt.entity, receipt.committed_stats_revision),
        source_cost: native_source_cost(receipt.source_cost),
    };
    let backing = OperationLeaseBacking {
        _text: lease_text.values,
        rows: OperationLeaseRows::StatMutation { observed_revisions },
    };
    let Some(handle) = bridge.insert_operation_lease(backing) else {
        return 0;
    };
    let OperationLeaseRows::StatMutation { observed_revisions } = &bridge
        .operation_leases
        .get(&handle.value)
        .expect("just inserted stat mutation lease")
        .rows
    else {
        unreachable!("stat mutation lease row kind matches its reader")
    };
    *result = NativeMechanicsStatMutationLease {
        handle,
        observed_revisions: observed_revisions.as_ptr(),
        ..metadata
    };
    ABI_OK
}

unsafe extern "C" fn destroy_operation_lease(
    context: *mut c_void,
    handle: NativeMechanicsOperationLeaseHandle,
) -> i32 {
    if context.is_null() || handle.value == 0 {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeMechanicsBridge>() };
    i32::from(bridge.operation_leases.remove(&handle.value).is_some())
}

unsafe extern "C" fn set_track(
    context: *mut c_void,
    request: *const NativeMechanicsTrackSetRequest,
    result: *mut NativeMechanicsTrackSetLease,
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
    let Some(catalog_id) = bridge
        .binding(request.entity)
        .map(|binding| binding.catalog)
    else {
        return 0;
    };
    let request_source = gameplay_mechanics::SourceInstanceIdentity::Request {
        operation: operation.clone(),
        instance: source,
    };
    let receipt = {
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
        receipt
    };
    let mut lease_text = CatalogLeaseText::default();
    let observed_revisions = receipt
        .observed_revisions
        .iter()
        .map(native_observed_revision)
        .collect::<Vec<_>>();
    let metadata = NativeMechanicsTrackSetLease {
        handle: NativeMechanicsOperationLeaseHandle::default(),
        observed_revisions: std::ptr::null(),
        observed_revisions_len: observed_revisions.len(),
        catalog_id,
        catalog_version: lease_text.copy(receipt.catalog_version.as_str()),
        catalog_fingerprint: lease_text.copy(&receipt.catalog_fingerprint),
        operation: lease_text.copy(receipt.operation.as_str()),
        source: native_source_identity(&receipt.source, &mut lease_text),
        entity_id: receipt.entity.raw(),
        track: lease_text.copy(receipt.track.as_str()),
        policy: match receipt.policy {
            TrackSetPolicy::RejectOutOfBounds => NativeMechanicsTrackSetPolicy::RejectOutOfBounds,
            TrackSetPolicy::ClampToBounds => NativeMechanicsTrackSetPolicy::ClampToBounds,
        },
        target: receipt.requested.get(),
        before: receipt.before.get(),
        after: receipt.after.get(),
        minimum: receipt.minimum.get(),
        maximum: receipt.maximum.get(),
        observed_revision: tracks_revision(receipt.entity, receipt.observed_tracks_revision),
        committed_revision: tracks_revision(receipt.entity, receipt.committed_tracks_revision),
        source_cost: native_source_cost(receipt.source_cost),
    };
    let Some((handle, observed_revisions)) =
        insert_track_operation_lease(bridge, lease_text, observed_revisions)
    else {
        return 0;
    };
    *result = NativeMechanicsTrackSetLease {
        handle,
        observed_revisions,
        ..metadata
    };
    ABI_OK
}

unsafe extern "C" fn spend_track(
    context: *mut c_void,
    request: *const NativeMechanicsTrackMutationRequest,
    result: *mut NativeMechanicsTrackMutationLease,
) -> i32 {
    mutate_track(context, request, result, TrackAdjustmentKind::Spend)
}

unsafe extern "C" fn restore_track(
    context: *mut c_void,
    request: *const NativeMechanicsTrackMutationRequest,
    result: *mut NativeMechanicsTrackMutationLease,
) -> i32 {
    mutate_track(context, request, result, TrackAdjustmentKind::Restore)
}

unsafe fn mutate_track(
    context: *mut c_void,
    request: *const NativeMechanicsTrackMutationRequest,
    result: *mut NativeMechanicsTrackMutationLease,
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
    let Some(catalog_id) = bridge
        .binding(request.entity)
        .map(|binding| binding.catalog)
    else {
        return 0;
    };
    let receipt = {
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
        receipt
    };
    let mut lease_text = CatalogLeaseText::default();
    let observed_revisions = receipt
        .observed_revisions
        .iter()
        .map(native_observed_revision)
        .collect::<Vec<_>>();
    let metadata = NativeMechanicsTrackMutationLease {
        handle: NativeMechanicsOperationLeaseHandle::default(),
        observed_revisions: std::ptr::null(),
        observed_revisions_len: observed_revisions.len(),
        catalog_id,
        catalog_version: lease_text.copy(receipt.catalog_version.as_str()),
        catalog_fingerprint: lease_text.copy(&receipt.catalog_fingerprint),
        operation: lease_text.copy(receipt.operation.as_str()),
        source: native_source_identity(&receipt.source, &mut lease_text),
        entity_id: receipt.entity.raw(),
        track: lease_text.copy(receipt.track.as_str()),
        kind: match receipt.kind {
            TrackAdjustmentKind::Spend => NativeMechanicsTrackAdjustmentKind::Spend,
            TrackAdjustmentKind::Restore => NativeMechanicsTrackAdjustmentKind::Restore,
        },
        requested_amount: receipt.requested_amount.get(),
        applied_amount: receipt.applied_amount.get(),
        before: receipt.before.get(),
        after: receipt.after.get(),
        minimum: receipt.minimum.get(),
        maximum: receipt.maximum.get(),
        observed_revision: tracks_revision(receipt.entity, receipt.observed_tracks_revision),
        committed_revision: tracks_revision(receipt.entity, receipt.committed_tracks_revision),
        source_cost: native_source_cost(receipt.source_cost),
    };
    let Some((handle, observed_revisions)) =
        insert_track_operation_lease(bridge, lease_text, observed_revisions)
    else {
        return 0;
    };
    *result = NativeMechanicsTrackMutationLease {
        handle,
        observed_revisions,
        ..metadata
    };
    ABI_OK
}

unsafe extern "C" fn reconcile_track(
    context: *mut c_void,
    request: *const NativeMechanicsTrackReconciliationRequest,
    result: *mut NativeMechanicsTrackReconciliationLease,
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
    let Some(catalog_id) = bridge
        .binding(request.entity)
        .map(|binding| binding.catalog)
    else {
        return 0;
    };
    let request_source = gameplay_mechanics::SourceInstanceIdentity::Request {
        operation: operation.clone(),
        instance: source,
    };
    let receipt = {
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
        receipt
    };
    let mut lease_text = CatalogLeaseText::default();
    let observed_revisions = receipt
        .observed_revisions
        .iter()
        .map(native_observed_revision)
        .collect::<Vec<_>>();
    let metadata = NativeMechanicsTrackReconciliationLease {
        handle: NativeMechanicsOperationLeaseHandle::default(),
        observed_revisions: std::ptr::null(),
        observed_revisions_len: observed_revisions.len(),
        catalog_id,
        catalog_version: lease_text.copy(receipt.catalog_version.as_str()),
        catalog_fingerprint: lease_text.copy(&receipt.catalog_fingerprint),
        operation: lease_text.copy(receipt.operation.as_str()),
        source: native_source_identity(&receipt.source, &mut lease_text),
        entity_id: receipt.entity.raw(),
        track: lease_text.copy(receipt.track.as_str()),
        policy: match receipt.policy {
            TrackReconciliationPolicy::PreserveCurrent => {
                NativeMechanicsTrackReconciliationPolicy::PreserveCurrent
            }
            TrackReconciliationPolicy::ClampToMaximum => {
                NativeMechanicsTrackReconciliationPolicy::ClampToMaximum
            }
        },
        before: receipt.before.get(),
        after: receipt.after.get(),
        minimum: receipt.minimum.get(),
        current_maximum: receipt.current_maximum.get(),
        prospective_maximum: receipt.prospective_maximum.get(),
        observed_revision: tracks_revision(receipt.entity, receipt.observed_tracks_revision),
        committed_revision: tracks_revision(receipt.entity, receipt.committed_tracks_revision),
        source_cost: native_source_cost(receipt.source_cost),
    };
    let Some((handle, observed_revisions)) =
        insert_track_operation_lease(bridge, lease_text, observed_revisions)
    else {
        return 0;
    };
    *result = NativeMechanicsTrackReconciliationLease {
        handle,
        observed_revisions,
        ..metadata
    };
    ABI_OK
}

unsafe extern "C" fn apply_effect(
    context: *mut c_void,
    request: *const NativeMechanicsEffectMutationRequest,
    result: *mut NativeMechanicsEffectOperationLease,
) -> i32 {
    mutate_effect(
        context,
        request,
        result,
        NativeMechanicsEffectMutationKind::Apply,
    )
}

unsafe extern "C" fn refresh_effect(
    context: *mut c_void,
    request: *const NativeMechanicsEffectRefreshRequest,
    result: *mut NativeMechanicsEffectOperationLease,
) -> i32 {
    if request.is_null() {
        return 0;
    }
    let request = unsafe { &*request };
    let normalized = NativeMechanicsEffectMutationRequest {
        entity: request.entity,
        operation: request.operation,
        instance: request.instance,
        definition: NativeUtf8Slice::default(),
        provenance_kind: request.provenance_kind,
        intrinsic_entity_id: request.intrinsic_entity_id,
        intrinsic_instance: request.intrinsic_instance,
        effect_entity_id: request.effect_entity_id,
        effect_instance: request.effect_instance,
        effect_stack: request.effect_stack,
        effect_source: request.effect_source,
        equipped_owner_entity_id: request.equipped_owner_entity_id,
        equipped_item_entity_id: request.equipped_item_entity_id,
        equipped_source: request.equipped_source,
        request_operation: request.request_operation,
        request_instance: request.request_instance,
        stacks: request.stacks,
        revision_guard: request.revision_guard,
        expected_revision: request.expected_revision,
    };
    mutate_effect(
        context,
        &normalized,
        result,
        NativeMechanicsEffectMutationKind::Refresh,
    )
}

unsafe extern "C" fn replace_effect(
    context: *mut c_void,
    request: *const NativeMechanicsEffectMutationRequest,
    result: *mut NativeMechanicsEffectOperationLease,
) -> i32 {
    mutate_effect(
        context,
        request,
        result,
        NativeMechanicsEffectMutationKind::Replace,
    )
}

unsafe fn mutate_effect(
    context: *mut c_void,
    request: *const NativeMechanicsEffectMutationRequest,
    result: *mut NativeMechanicsEffectOperationLease,
    kind: NativeMechanicsEffectMutationKind,
) -> i32 {
    let Some((bridge, request, result)) = bridge_request_result(context, request, result) else {
        return 0;
    };
    let (Ok(operation), Ok(instance), Ok(provenance)) = (
        unsafe { text(request.operation, "mechanics effect operation") }
            .and_then(parse::<OperationId>),
        unsafe { text(request.instance, "mechanics effect instance") }
            .and_then(parse::<gameplay_mechanics::EffectInstanceId>),
        parse_effect_provenance(request),
    ) else {
        return 0;
    };
    let Some(catalog_id) = bridge
        .binding(request.entity)
        .map(|binding| binding.catalog)
    else {
        return 0;
    };
    let receipt = {
        let Some((state, catalog, entity)) = bridge.state_and_catalog_mut(request.entity) else {
            return 0;
        };
        let Ok(actual) = state.component_revision::<ActiveEffectsComponent>(entity) else {
            return 0;
        };
        let Some(expected_revision) = guarded_revision(
            request.revision_guard,
            request.expected_revision.entity_id,
            request.expected_revision.revision,
            request.expected_revision.component,
            entity,
            actual,
            NativeMechanicsRevisionComponent::ActiveEffects,
        ) else {
            return 0;
        };
        let receipt = match kind {
            NativeMechanicsEffectMutationKind::Apply => {
                let Ok(definition) =
                    unsafe { text(request.definition, "mechanics effect definition") }
                        .and_then(parse::<gameplay_mechanics::EffectDefinitionId>)
                else {
                    return 0;
                };
                EffectService::apply(
                    state,
                    catalog,
                    EffectApplyRequest {
                        operation,
                        entity,
                        instance,
                        definition,
                        provenance,
                        stacks: request.stacks,
                        expected_revision,
                    },
                )
            }
            NativeMechanicsEffectMutationKind::Refresh => EffectService::refresh(
                state,
                catalog,
                EffectRefreshRequest {
                    operation,
                    entity,
                    instance,
                    provenance,
                    stacks: request.stacks,
                    expected_revision,
                },
            ),
            NativeMechanicsEffectMutationKind::Replace => {
                let Ok(definition) =
                    unsafe { text(request.definition, "mechanics effect definition") }
                        .and_then(parse::<gameplay_mechanics::EffectDefinitionId>)
                else {
                    return 0;
                };
                EffectService::replace(
                    state,
                    catalog,
                    EffectReplaceRequest {
                        operation,
                        entity,
                        instance,
                        definition,
                        provenance,
                        stacks: request.stacks,
                        expected_revision,
                    },
                )
            }
            NativeMechanicsEffectMutationKind::Remove
            | NativeMechanicsEffectMutationKind::Expire => {
                unreachable!("mutation callback excludes removal kinds")
            }
        };
        let Ok(receipt) = receipt else {
            return 0;
        };
        receipt
    };
    write_effect_operation_lease(bridge, catalog_id, receipt, result)
}

unsafe extern "C" fn remove_effect(
    context: *mut c_void,
    request: *const NativeMechanicsEffectRemovalRequest,
    result: *mut NativeMechanicsEffectOperationLease,
) -> i32 {
    remove_effect_with_kind(
        context,
        request,
        result,
        NativeMechanicsEffectMutationKind::Remove,
    )
}

unsafe extern "C" fn expire_effect(
    context: *mut c_void,
    request: *const NativeMechanicsEffectRemovalRequest,
    result: *mut NativeMechanicsEffectOperationLease,
) -> i32 {
    remove_effect_with_kind(
        context,
        request,
        result,
        NativeMechanicsEffectMutationKind::Expire,
    )
}

unsafe fn remove_effect_with_kind(
    context: *mut c_void,
    request: *const NativeMechanicsEffectRemovalRequest,
    result: *mut NativeMechanicsEffectOperationLease,
    kind: NativeMechanicsEffectMutationKind,
) -> i32 {
    let Some((bridge, request, result)) = bridge_request_result(context, request, result) else {
        return 0;
    };
    let (Ok(operation), Ok(instance)) = (
        unsafe { text(request.operation, "mechanics effect operation") }
            .and_then(parse::<OperationId>),
        unsafe { text(request.instance, "mechanics effect instance") }
            .and_then(parse::<gameplay_mechanics::EffectInstanceId>),
    ) else {
        return 0;
    };
    let Some(catalog_id) = bridge
        .binding(request.entity)
        .map(|binding| binding.catalog)
    else {
        return 0;
    };
    let receipt = {
        let Some((state, catalog, entity)) = bridge.state_and_catalog_mut(request.entity) else {
            return 0;
        };
        let Ok(actual) = state.component_revision::<ActiveEffectsComponent>(entity) else {
            return 0;
        };
        let Some(expected_revision) = guarded_revision(
            request.revision_guard,
            request.expected_revision.entity_id,
            request.expected_revision.revision,
            request.expected_revision.component,
            entity,
            actual,
            NativeMechanicsRevisionComponent::ActiveEffects,
        ) else {
            return 0;
        };
        let request = EffectRemovalRequest {
            operation,
            entity,
            instance,
            expected_revision,
        };
        let receipt = match kind {
            NativeMechanicsEffectMutationKind::Remove => {
                EffectService::remove(state, catalog, request)
            }
            NativeMechanicsEffectMutationKind::Expire => {
                EffectService::expire(state, catalog, request)
            }
            NativeMechanicsEffectMutationKind::Apply
            | NativeMechanicsEffectMutationKind::Refresh
            | NativeMechanicsEffectMutationKind::Replace => {
                unreachable!("removal callback only accepts removal kinds")
            }
        };
        let Ok(receipt) = receipt else {
            return 0;
        };
        receipt
    };
    write_effect_operation_lease(bridge, catalog_id, receipt, result)
}

fn write_effect_operation_lease(
    bridge: &mut RuntimeMechanicsBridge,
    catalog_id: u64,
    receipt: gameplay_mechanics::EffectMutationReceipt,
    result: &mut NativeMechanicsEffectOperationLease,
) -> i32 {
    let mut lease_text = CatalogLeaseText::default();
    let removed = receipt
        .removed
        .iter()
        .map(|effect| native_active_effect_row(effect, &mut lease_text))
        .collect::<Vec<_>>();
    let activated_sources = receipt
        .activated_sources
        .iter()
        .map(|activation| native_effect_source_activation(activation, &mut lease_text))
        .collect::<Vec<_>>();
    let observed_revisions = receipt
        .observed_revisions
        .iter()
        .map(native_observed_revision)
        .collect::<Vec<_>>();
    let metadata = NativeMechanicsEffectOperationLease {
        handle: NativeMechanicsOperationLeaseHandle::default(),
        removed: std::ptr::null(),
        removed_len: removed.len(),
        activated_sources: std::ptr::null(),
        activated_sources_len: activated_sources.len(),
        observed_revisions: std::ptr::null(),
        observed_revisions_len: observed_revisions.len(),
        catalog_id,
        catalog_version: lease_text.copy(receipt.catalog_version.as_str()),
        catalog_fingerprint: lease_text.copy(&receipt.catalog_fingerprint),
        operation: lease_text.copy(receipt.operation.as_str()),
        entity_id: receipt.entity.raw(),
        kind: native_effect_mutation_kind(receipt.kind),
        has_current: receipt.current.is_some(),
        current: receipt
            .current
            .as_ref()
            .map(|effect| native_active_effect_row(effect, &mut lease_text))
            .unwrap_or_default(),
        observed_revision: active_effects_revision(
            receipt.entity,
            receipt.observed_effects_revision,
        ),
        committed_revision: active_effects_revision(
            receipt.entity,
            receipt.committed_effects_revision,
        ),
        tracks_validated: receipt.tracks_validated as u64,
        source_cost: native_source_cost(receipt.source_cost),
    };
    let handle = bridge.insert_operation_lease(OperationLeaseBacking {
        _text: lease_text.values,
        rows: OperationLeaseRows::Effect {
            removed,
            activated_sources,
            observed_revisions,
        },
    });
    let Some(handle) = handle else {
        return 0;
    };
    let OperationLeaseRows::Effect {
        removed,
        activated_sources,
        observed_revisions,
    } = &bridge
        .operation_leases
        .get(&handle.value)
        .expect("just inserted effect operation lease")
        .rows
    else {
        unreachable!("effect operation lease row kind matches its reader")
    };
    *result = NativeMechanicsEffectOperationLease {
        handle,
        removed: removed.as_ptr(),
        activated_sources: activated_sources.as_ptr(),
        observed_revisions: observed_revisions.as_ptr(),
        ..metadata
    };
    ABI_OK
}

unsafe extern "C" fn preview_damage(
    context: *mut c_void,
    request: *const NativeMechanicsDamageRequest,
    result: *mut NativeMechanicsDamageLease,
) -> i32 {
    damage_operation(context, request, result, false)
}

unsafe extern "C" fn apply_damage(
    context: *mut c_void,
    request: *const NativeMechanicsDamageRequest,
    result: *mut NativeMechanicsDamageLease,
) -> i32 {
    damage_operation(context, request, result, true)
}

unsafe fn damage_operation(
    context: *mut c_void,
    request: *const NativeMechanicsDamageRequest,
    result: *mut NativeMechanicsDamageLease,
    apply: bool,
) -> i32 {
    let Some((bridge, request, result)) = bridge_request_result(context, request, result) else {
        return 0;
    };
    if request.parts_len > gameplay_mechanics::MAX_DAMAGE_PARTS
        || request.request_sources_len > gameplay_mechanics::MAX_DAMAGE_REQUEST_SOURCES
    {
        return 0;
    }
    let (Ok(operation), Ok(source), Ok(target_track), Ok(parts), Ok(request_sources)) = (
        unsafe { text(request.operation, "mechanics damage operation") }
            .and_then(parse::<OperationId>),
        parse_damage_source_identity(request),
        unsafe { text(request.target_track, "mechanics damage target track") }
            .and_then(parse::<gameplay_mechanics::TrackId>),
        unsafe { parse_damage_parts(request.parts, request.parts_len) },
        unsafe {
            borrowed_slice(
                request.request_sources,
                request.request_sources_len,
                "mechanics damage request sources",
            )
        }
        .and_then(parse_request_sources),
    ) else {
        return 0;
    };
    let Some(catalog_id) = bridge
        .binding(request.target)
        .map(|binding| binding.catalog)
    else {
        return 0;
    };
    let receipt = {
        let Some((state, catalog, target)) = bridge.state_and_catalog_mut(request.target) else {
            return 0;
        };
        let Ok(actual) = state.component_revision::<TracksComponent>(target) else {
            return 0;
        };
        let expected_tracks_revision = if request.has_expected_tracks_revision {
            if !revision_guard_matches(
                NativeMechanicsRevisionGuard::Exact,
                request.expected_tracks_revision.entity_id,
                request.expected_tracks_revision.revision,
                request.expected_tracks_revision.component,
                target,
                actual.revision(),
                NativeMechanicsRevisionComponent::Tracks,
            ) {
                return 0;
            }
            Some(actual)
        } else {
            None
        };
        let damage_request = DamageRequest {
            operation,
            source,
            actor: request
                .has_actor
                .then(|| EntityId::new(request.actor_entity_id)),
            target,
            target_track,
            parts,
            request_sources,
            expected_tracks_revision,
        };
        let receipt = if apply {
            DamageService::apply(state, catalog, damage_request)
        } else {
            DamageService::preview(state, catalog, &damage_request)
                .map(|preview| preview.receipt().clone())
        };
        let Ok(receipt) = receipt else {
            return 0;
        };
        receipt
    };
    write_damage_lease(bridge, catalog_id, receipt, result)
}

unsafe fn parse_damage_parts(
    values: *const NativeMechanicsDamagePart,
    len: usize,
) -> Result<Vec<DamagePart>, ()> {
    unsafe { borrowed_slice(values, len, "mechanics damage parts") }?
        .iter()
        .map(|value| {
            Ok(DamagePart {
                kind: unsafe { text(value.kind, "mechanics damage part kind") }
                    .and_then(parse::<gameplay_mechanics::DamageKindId>)?,
                amount: scalar(value.amount)?,
            })
        })
        .collect()
}

fn parse_source_identity(
    source: &NativeMechanicsSourceIdentity,
) -> Result<SourceInstanceIdentity, ()> {
    match source.kind {
        NativeMechanicsActiveEffectProvenanceKind::Intrinsic => {
            Ok(SourceInstanceIdentity::Intrinsic {
                entity: EntityId::new(source.intrinsic_entity_id),
                instance: unsafe {
                    text(
                        source.intrinsic_instance,
                        "inventory intrinsic source instance",
                    )
                }
                .and_then(parse::<SourceInstanceId>)?,
            })
        }
        NativeMechanicsActiveEffectProvenanceKind::Effect => Ok(SourceInstanceIdentity::Effect {
            entity: EntityId::new(source.effect_entity_id),
            effect: unsafe { text(source.effect_instance, "inventory effect source instance") }
                .and_then(parse::<gameplay_mechanics::EffectInstanceId>)?,
            stack: source.effect_stack,
            source: unsafe { text(source.effect_source, "inventory effect source definition") }
                .and_then(parse::<SourceDefinitionId>)?,
        }),
        NativeMechanicsActiveEffectProvenanceKind::EquippedItem => {
            Ok(SourceInstanceIdentity::EquippedItem {
                owner: EntityId::new(source.equipped_owner_entity_id),
                item: EntityId::new(source.equipped_item_entity_id),
                source: unsafe {
                    text(
                        source.equipped_source,
                        "inventory equipped source definition",
                    )
                }
                .and_then(parse::<SourceDefinitionId>)?,
            })
        }
        NativeMechanicsActiveEffectProvenanceKind::Request => Ok(SourceInstanceIdentity::Request {
            operation: unsafe {
                text(
                    source.request_operation,
                    "inventory request source operation",
                )
            }
            .and_then(parse::<OperationId>)?,
            instance: unsafe { text(source.request_instance, "inventory request source instance") }
                .and_then(parse::<SourceInstanceId>)?,
        }),
    }
}

fn parse_inventory_request_source_identity(
    request: &NativeMechanicsInventoryMutationRequest,
) -> Result<SourceInstanceIdentity, ()> {
    parse_source_identity(&NativeMechanicsSourceIdentity {
        kind: request.source_kind,
        intrinsic_entity_id: request.source_intrinsic_entity_id,
        intrinsic_instance: request.source_intrinsic_instance,
        effect_entity_id: request.source_effect_entity_id,
        effect_instance: request.source_effect_instance,
        effect_stack: request.source_effect_stack,
        effect_source: request.source_effect_source,
        equipped_owner_entity_id: request.source_equipped_owner_entity_id,
        equipped_item_entity_id: request.source_equipped_item_entity_id,
        equipped_source: request.source_equipped_source,
        request_operation: request.source_request_operation,
        request_instance: request.source_request_instance,
    })
}

fn parse_inventory_transfer_request_source_identity(
    request: &NativeMechanicsInventoryTransferRequest,
) -> Result<SourceInstanceIdentity, ()> {
    parse_source_identity(&NativeMechanicsSourceIdentity {
        kind: request.source_kind,
        intrinsic_entity_id: request.source_intrinsic_entity_id,
        intrinsic_instance: request.source_intrinsic_instance,
        effect_entity_id: request.source_effect_entity_id,
        effect_instance: request.source_effect_instance,
        effect_stack: request.source_effect_stack,
        effect_source: request.source_effect_source,
        equipped_owner_entity_id: request.source_equipped_owner_entity_id,
        equipped_item_entity_id: request.source_equipped_item_entity_id,
        equipped_source: request.source_equipped_source,
        request_operation: request.source_request_operation,
        request_instance: request.source_request_instance,
    })
}
fn parse_unique_item_transfer_request_source_identity(
    request: &NativeMechanicsUniqueItemTransferRequest,
) -> Result<SourceInstanceIdentity, ()> {
    parse_source_identity(&NativeMechanicsSourceIdentity {
        kind: request.source_kind,
        intrinsic_entity_id: request.source_intrinsic_entity_id,
        intrinsic_instance: request.source_intrinsic_instance,
        effect_entity_id: request.source_effect_entity_id,
        effect_instance: request.source_effect_instance,
        effect_stack: request.source_effect_stack,
        effect_source: request.source_effect_source,
        equipped_owner_entity_id: request.source_equipped_owner_entity_id,
        equipped_item_entity_id: request.source_equipped_item_entity_id,
        equipped_source: request.source_equipped_source,
        request_operation: request.source_request_operation,
        request_instance: request.source_request_instance,
    })
}
fn parse_unique_item_destroy_request_source_identity(
    request: &NativeMechanicsUniqueItemDestroyRequest,
) -> Result<SourceInstanceIdentity, ()> {
    parse_source_identity(&NativeMechanicsSourceIdentity {
        kind: request.source_kind,
        intrinsic_entity_id: request.source_intrinsic_entity_id,
        intrinsic_instance: request.source_intrinsic_instance,
        effect_entity_id: request.source_effect_entity_id,
        effect_instance: request.source_effect_instance,
        effect_stack: request.source_effect_stack,
        effect_source: request.source_effect_source,
        equipped_owner_entity_id: request.source_equipped_owner_entity_id,
        equipped_item_entity_id: request.source_equipped_item_entity_id,
        equipped_source: request.source_equipped_source,
        request_operation: request.source_request_operation,
        request_instance: request.source_request_instance,
    })
}
fn parse_equipment_equip_source_identity(
    request: &NativeMechanicsEquipmentEquipRequest,
) -> Result<SourceInstanceIdentity, ()> {
    parse_source_identity(&NativeMechanicsSourceIdentity {
        kind: request.source_kind,
        intrinsic_entity_id: request.source_intrinsic_entity_id,
        intrinsic_instance: request.source_intrinsic_instance,
        effect_entity_id: request.source_effect_entity_id,
        effect_instance: request.source_effect_instance,
        effect_stack: request.source_effect_stack,
        effect_source: request.source_effect_source,
        equipped_owner_entity_id: request.source_equipped_owner_entity_id,
        equipped_item_entity_id: request.source_equipped_item_entity_id,
        equipped_source: request.source_equipped_source,
        request_operation: request.source_request_operation,
        request_instance: request.source_request_instance,
    })
}
fn parse_equipment_unequip_source_identity(
    request: &NativeMechanicsEquipmentUnequipRequest,
) -> Result<SourceInstanceIdentity, ()> {
    parse_source_identity(&NativeMechanicsSourceIdentity {
        kind: request.source_kind,
        intrinsic_entity_id: request.source_intrinsic_entity_id,
        intrinsic_instance: request.source_intrinsic_instance,
        effect_entity_id: request.source_effect_entity_id,
        effect_instance: request.source_effect_instance,
        effect_stack: request.source_effect_stack,
        effect_source: request.source_effect_source,
        equipped_owner_entity_id: request.source_equipped_owner_entity_id,
        equipped_item_entity_id: request.source_equipped_item_entity_id,
        equipped_source: request.source_equipped_source,
        request_operation: request.source_request_operation,
        request_instance: request.source_request_instance,
    })
}
fn parse_equipment_swap_source_identity(
    request: &NativeMechanicsEquipmentSwapRequest,
) -> Result<SourceInstanceIdentity, ()> {
    parse_source_identity(&NativeMechanicsSourceIdentity {
        kind: request.source_kind,
        intrinsic_entity_id: request.source_intrinsic_entity_id,
        intrinsic_instance: request.source_intrinsic_instance,
        effect_entity_id: request.source_effect_entity_id,
        effect_instance: request.source_effect_instance,
        effect_stack: request.source_effect_stack,
        effect_source: request.source_effect_source,
        equipped_owner_entity_id: request.source_equipped_owner_entity_id,
        equipped_item_entity_id: request.source_equipped_item_entity_id,
        equipped_source: request.source_equipped_source,
        request_operation: request.source_request_operation,
        request_instance: request.source_request_instance,
    })
}
fn parse_equipment_slots(
    values: &[NativeMechanicsText],
) -> Result<Vec<gameplay_mechanics::EquipmentSlotId>, ()> {
    values
        .iter()
        .map(|value| {
            unsafe { text(value.value, "equipment requested slot") }
                .and_then(parse::<gameplay_mechanics::EquipmentSlotId>)
        })
        .collect()
}

fn parse_damage_source_identity(
    request: &NativeMechanicsDamageRequest,
) -> Result<SourceInstanceIdentity, ()> {
    match request.source_kind {
        NativeMechanicsActiveEffectProvenanceKind::Intrinsic => {
            Ok(SourceInstanceIdentity::Intrinsic {
                entity: EntityId::new(request.source_intrinsic_entity_id),
                instance: unsafe {
                    text(
                        request.source_intrinsic_instance,
                        "mechanics damage intrinsic source instance",
                    )
                }
                .and_then(parse::<SourceInstanceId>)?,
            })
        }
        NativeMechanicsActiveEffectProvenanceKind::Effect => Ok(SourceInstanceIdentity::Effect {
            entity: EntityId::new(request.source_effect_entity_id),
            effect: unsafe {
                text(
                    request.source_effect_instance,
                    "mechanics damage effect source instance",
                )
            }
            .and_then(parse::<gameplay_mechanics::EffectInstanceId>)?,
            stack: request.source_effect_stack,
            source: unsafe {
                text(
                    request.source_effect_source,
                    "mechanics damage effect source definition",
                )
            }
            .and_then(parse::<SourceDefinitionId>)?,
        }),
        NativeMechanicsActiveEffectProvenanceKind::EquippedItem => {
            Ok(SourceInstanceIdentity::EquippedItem {
                owner: EntityId::new(request.source_equipped_owner_entity_id),
                item: EntityId::new(request.source_equipped_item_entity_id),
                source: unsafe {
                    text(
                        request.source_equipped_source,
                        "mechanics damage equipped source definition",
                    )
                }
                .and_then(parse::<SourceDefinitionId>)?,
            })
        }
        NativeMechanicsActiveEffectProvenanceKind::Request => Ok(SourceInstanceIdentity::Request {
            operation: unsafe {
                text(
                    request.source_request_operation,
                    "mechanics damage request source operation",
                )
            }
            .and_then(parse::<OperationId>)?,
            instance: unsafe {
                text(
                    request.source_request_instance,
                    "mechanics damage request source instance",
                )
            }
            .and_then(parse::<SourceInstanceId>)?,
        }),
    }
}

fn native_damage_part_receipt(
    value: &DamagePartReceipt,
    text: &mut CatalogLeaseText,
) -> NativeMechanicsDamagePartReceiptRow {
    NativeMechanicsDamagePartReceiptRow {
        index: value.index,
        kind: text.copy(value.kind.as_str()),
        original: value.original.get(),
        prevented: value.prevented,
        after_flat: value.after_flat.get(),
        combined_scale_numerator: native_u128(value.combined_scale_numerator),
        combined_scale_denominator: native_u128(value.combined_scale_denominator),
        rounding: match value.rounding {
            RoundingPolicy::TowardZero => NativeMechanicsRoundingPolicy::TowardZero,
        },
        after_scale: value.after_scale.get(),
        absorbed: value.absorbed.get(),
        applied: value.applied.get(),
        unapplied: value.unapplied.get(),
    }
}

fn native_damage_decision(
    value: &ResponseDecision,
    text: &mut CatalogLeaseText,
) -> NativeMechanicsDamageDecisionRow {
    let mut row = NativeMechanicsDamageDecisionRow {
        part_index: value.part_index,
        source: native_source_identity(&value.source, text),
        source_definition: text.copy(value.source_definition.as_str()),
        has_response_index: value.response_index.is_some(),
        response_index: value.response_index.unwrap_or_default(),
        kind: NativeMechanicsDamageDecisionKind::NoDamageResponse,
        amount: 0,
        ratio_numerator: 0,
        ratio_denominator: 0,
        absorb_track: text.copy(""),
        outcome: match value.outcome {
            DecisionOutcome::Applied => NativeMechanicsDecisionOutcome::Applied,
            DecisionOutcome::Suppressed => NativeMechanicsDecisionOutcome::Suppressed,
            DecisionOutcome::Inapplicable => NativeMechanicsDecisionOutcome::Inapplicable,
        },
    };
    match &value.kind {
        ResponseDecisionKind::NoDamageResponse => {}
        ResponseDecisionKind::Prevent => row.kind = NativeMechanicsDamageDecisionKind::Prevent,
        ResponseDecisionKind::FlatReduction { amount } => {
            row.kind = NativeMechanicsDamageDecisionKind::FlatReduction;
            row.amount = amount.get();
        }
        ResponseDecisionKind::Scale { ratio } => {
            row.kind = NativeMechanicsDamageDecisionKind::Scale;
            row.ratio_numerator = ratio.numerator();
            row.ratio_denominator = ratio.denominator();
        }
        ResponseDecisionKind::Absorb { track } => {
            row.kind = NativeMechanicsDamageDecisionKind::Absorb;
            row.absorb_track = text.copy(track.as_str());
        }
    }
    row
}

fn native_track_damage_change(
    value: &TrackDamageChange,
    text: &mut CatalogLeaseText,
) -> NativeMechanicsTrackDamageChangeRow {
    NativeMechanicsTrackDamageChangeRow {
        track: text.copy(value.track.as_str()),
        before: value.before.get(),
        after: value.after.get(),
    }
}

fn native_track_depletion(
    track: &gameplay_mechanics::TrackId,
    part_index: u16,
    text: &mut CatalogLeaseText,
) -> NativeMechanicsTrackDepletionRow {
    NativeMechanicsTrackDepletionRow {
        track: text.copy(track.as_str()),
        part_index,
    }
}

fn write_damage_lease(
    bridge: &mut RuntimeMechanicsBridge,
    catalog_id: u64,
    receipt: DamageReceipt,
    result: &mut NativeMechanicsDamageLease,
) -> i32 {
    let mut lease_text = CatalogLeaseText::default();
    let parts = receipt
        .parts
        .iter()
        .map(|value| native_damage_part_receipt(value, &mut lease_text))
        .collect::<Vec<_>>();
    let decisions = receipt
        .decisions
        .iter()
        .map(|value| native_damage_decision(value, &mut lease_text))
        .collect::<Vec<_>>();
    let track_changes = receipt
        .track_changes
        .iter()
        .map(|value| native_track_damage_change(value, &mut lease_text))
        .collect::<Vec<_>>();
    let mut protection_track_depletions = Vec::new();
    let mut target_track_depletions = Vec::new();
    for fact in &receipt.facts {
        match fact {
            DamageFact::ProtectionTrackDepleted { track, part_index } => {
                protection_track_depletions.push(native_track_depletion(
                    track,
                    *part_index,
                    &mut lease_text,
                ));
            }
            DamageFact::TargetTrackDepleted { track, part_index } => {
                target_track_depletions.push(native_track_depletion(
                    track,
                    *part_index,
                    &mut lease_text,
                ));
            }
        }
    }
    let observed_revisions = receipt
        .observed_revisions
        .iter()
        .map(native_observed_revision)
        .collect::<Vec<_>>();
    let (has_committed_tracks_revision, committed_tracks_revision) =
        match receipt.committed_tracks_revision {
            Some(revision) => (true, tracks_revision(receipt.target, revision)),
            None => (false, NativeMechanicsTracksRevision::default()),
        };
    let metadata = NativeMechanicsDamageLease {
        handle: NativeMechanicsOperationLeaseHandle::default(),
        parts: std::ptr::null(),
        parts_len: parts.len(),
        decisions: std::ptr::null(),
        decisions_len: decisions.len(),
        track_changes: std::ptr::null(),
        track_changes_len: track_changes.len(),
        protection_track_depletions: std::ptr::null(),
        protection_track_depletions_len: protection_track_depletions.len(),
        target_track_depletions: std::ptr::null(),
        target_track_depletions_len: target_track_depletions.len(),
        observed_revisions: std::ptr::null(),
        observed_revisions_len: observed_revisions.len(),
        catalog_id,
        catalog_version: lease_text.copy(receipt.catalog_version.as_str()),
        catalog_fingerprint: lease_text.copy(&receipt.catalog_fingerprint),
        operation: lease_text.copy(receipt.operation.as_str()),
        source: native_source_identity(&receipt.source, &mut lease_text),
        has_actor: receipt.actor.is_some(),
        actor_entity_id: receipt.actor.map_or(0, EntityId::raw),
        target_entity_id: receipt.target.raw(),
        target_track: lease_text.copy(receipt.target_track.as_str()),
        observed_tracks_revision: tracks_revision(receipt.target, receipt.observed_tracks_revision),
        has_committed_tracks_revision,
        committed_tracks_revision,
        source_cost: native_source_cost(receipt.source_cost),
    };
    let Some(handle) = bridge.insert_operation_lease(OperationLeaseBacking {
        _text: lease_text.values,
        rows: OperationLeaseRows::Damage {
            parts,
            decisions,
            track_changes,
            protection_track_depletions,
            target_track_depletions,
            observed_revisions,
        },
    }) else {
        return 0;
    };
    let OperationLeaseRows::Damage {
        parts,
        decisions,
        track_changes,
        protection_track_depletions,
        target_track_depletions,
        observed_revisions,
    } = &bridge
        .operation_leases
        .get(&handle.value)
        .expect("just inserted damage operation lease")
        .rows
    else {
        unreachable!("damage operation lease row kind matches its reader")
    };
    *result = NativeMechanicsDamageLease {
        handle,
        parts: parts.as_ptr(),
        decisions: decisions.as_ptr(),
        track_changes: track_changes.as_ptr(),
        protection_track_depletions: protection_track_depletions.as_ptr(),
        target_track_depletions: target_track_depletions.as_ptr(),
        observed_revisions: observed_revisions.as_ptr(),
        ..metadata
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
fn parse_request_sources(
    values: &[NativeMechanicsRequestSource],
) -> Result<Vec<RequestSource>, ()> {
    values
        .iter()
        .map(|value| {
            Ok(RequestSource {
                instance: unsafe { text(value.instance, "mechanics request source instance") }
                    .and_then(parse::<SourceInstanceId>)?,
                definition: unsafe {
                    text(value.definition, "mechanics request source definition")
                }
                .and_then(parse::<SourceDefinitionId>)?,
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
fn native_u128(value: u128) -> NativeMechanicsU128 {
    NativeMechanicsU128 {
        low: value as u64,
        high: (value >> 64) as u64,
    }
}
fn native_source_cost(value: SourceCollectionCost) -> NativeMechanicsSourceCollectionCost {
    NativeMechanicsSourceCollectionCost {
        intrinsic_entries_visited: value.intrinsic_entries_visited as u64,
        effect_entries_visited: value.effect_entries_visited as u64,
        effect_source_activations_visited: value.effect_source_activations_visited as u64,
        equipment_entries_visited: value.equipment_entries_visited as u64,
        item_components_read: value.item_components_read as u64,
        request_entries_visited: value.request_entries_visited as u64,
    }
}
fn native_inventory_read_cost(value: InventoryReadCost) -> NativeMechanicsInventoryReadCost {
    NativeMechanicsInventoryReadCost {
        stack_entries_visited: value.stack_entries_visited as u64,
        containment_entries_visited: value.containment_entries_visited as u64,
        item_components_read: value.item_components_read as u64,
        capacity_limits_visited: value.capacity_limits_visited as u64,
        capacity_costs_visited: value.capacity_costs_visited as u64,
    }
}
fn native_capacity_usage(
    values: &[gameplay_mechanics::CapacityUsage],
    text: &mut CatalogLeaseText,
) -> Vec<NativeMechanicsInventoryViewCapacityUsageRow> {
    values
        .iter()
        .map(|value| NativeMechanicsInventoryViewCapacityUsageRow {
            metric: text.copy(value.metric.as_str()),
            used: value.used,
            has_maximum: value.maximum.is_some(),
            maximum: value.maximum.unwrap_or_default(),
        })
        .collect()
}
fn inventory_revision(entity: EntityId, revision: u64) -> NativeMechanicsComponentRevision {
    NativeMechanicsComponentRevision {
        entity_id: entity.raw(),
        revision,
        component: NativeMechanicsRevisionComponent::Inventory,
        present: true,
    }
}
fn equipment_revision(entity: EntityId, revision: u64) -> NativeMechanicsComponentRevision {
    NativeMechanicsComponentRevision {
        entity_id: entity.raw(),
        revision,
        component: NativeMechanicsRevisionComponent::Equipment,
        present: true,
    }
}
fn native_equipment_slot_change(
    value: &EquipmentSlotChange,
    text: &mut CatalogLeaseText,
) -> NativeMechanicsEquipmentSlotChangeRow {
    NativeMechanicsEquipmentSlotChangeRow {
        slot: text.copy(value.slot.as_str()),
        has_before_item: value.before.is_some(),
        before_item_entity_id: value.before.map(EntityId::raw).unwrap_or_default(),
        has_after_item: value.after.is_some(),
        after_item_entity_id: value.after.map(EntityId::raw).unwrap_or_default(),
    }
}
fn native_observed_revision(
    value: &ObservedComponentRevision,
) -> NativeMechanicsObservedComponentRevisionRow {
    NativeMechanicsObservedComponentRevisionRow {
        entity_id: value.entity.raw(),
        component: match value.component {
            MechanicsComponentKind::Stats => NativeMechanicsRevisionComponent::Stats,
            MechanicsComponentKind::Tracks => NativeMechanicsRevisionComponent::Tracks,
            MechanicsComponentKind::IntrinsicSources => {
                NativeMechanicsRevisionComponent::IntrinsicSources
            }
            MechanicsComponentKind::ActiveEffects => {
                NativeMechanicsRevisionComponent::ActiveEffects
            }
            MechanicsComponentKind::Inventory => NativeMechanicsRevisionComponent::Inventory,
            MechanicsComponentKind::Item => NativeMechanicsRevisionComponent::Item,
            MechanicsComponentKind::Equipment => NativeMechanicsRevisionComponent::Equipment,
        },
        revision: value.revision,
    }
}
fn native_source_identity(
    value: &SourceInstanceIdentity,
    text: &mut CatalogLeaseText,
) -> NativeMechanicsSourceIdentity {
    let mut native = NativeMechanicsSourceIdentity {
        intrinsic_instance: text.copy(""),
        effect_instance: text.copy(""),
        effect_source: text.copy(""),
        equipped_source: text.copy(""),
        request_operation: text.copy(""),
        request_instance: text.copy(""),
        ..NativeMechanicsSourceIdentity::default()
    };
    match value {
        SourceInstanceIdentity::Intrinsic { entity, instance } => {
            native.kind = NativeMechanicsActiveEffectProvenanceKind::Intrinsic;
            native.intrinsic_entity_id = entity.raw();
            native.intrinsic_instance = text.copy(instance.as_str());
        }
        SourceInstanceIdentity::Effect {
            entity,
            effect,
            stack,
            source,
        } => {
            native.kind = NativeMechanicsActiveEffectProvenanceKind::Effect;
            native.effect_entity_id = entity.raw();
            native.effect_instance = text.copy(effect.as_str());
            native.effect_stack = *stack;
            native.effect_source = text.copy(source.as_str());
        }
        SourceInstanceIdentity::EquippedItem {
            owner,
            item,
            source,
        } => {
            native.kind = NativeMechanicsActiveEffectProvenanceKind::EquippedItem;
            native.equipped_owner_entity_id = owner.raw();
            native.equipped_item_entity_id = item.raw();
            native.equipped_source = text.copy(source.as_str());
        }
        SourceInstanceIdentity::Request {
            operation,
            instance,
        } => {
            native.kind = NativeMechanicsActiveEffectProvenanceKind::Request;
            native.request_operation = text.copy(operation.as_str());
            native.request_instance = text.copy(instance.as_str());
        }
    }
    native
}
fn parse_effect_provenance(
    request: &NativeMechanicsEffectMutationRequest,
) -> Result<SourceInstanceIdentity, ()> {
    match request.provenance_kind {
        NativeMechanicsActiveEffectProvenanceKind::Intrinsic => {
            Ok(SourceInstanceIdentity::Intrinsic {
                entity: EntityId::new(request.intrinsic_entity_id),
                instance: unsafe {
                    text(
                        request.intrinsic_instance,
                        "mechanics effect intrinsic provenance",
                    )
                }
                .and_then(parse::<SourceInstanceId>)?,
            })
        }
        NativeMechanicsActiveEffectProvenanceKind::Effect => Ok(SourceInstanceIdentity::Effect {
            entity: EntityId::new(request.effect_entity_id),
            effect: unsafe {
                text(
                    request.effect_instance,
                    "mechanics effect provenance effect",
                )
            }
            .and_then(parse::<gameplay_mechanics::EffectInstanceId>)?,
            stack: request.effect_stack,
            source: unsafe { text(request.effect_source, "mechanics effect provenance source") }
                .and_then(parse::<SourceDefinitionId>)?,
        }),
        NativeMechanicsActiveEffectProvenanceKind::EquippedItem => {
            Ok(SourceInstanceIdentity::EquippedItem {
                owner: EntityId::new(request.equipped_owner_entity_id),
                item: EntityId::new(request.equipped_item_entity_id),
                source: unsafe {
                    text(
                        request.equipped_source,
                        "mechanics effect equipment provenance",
                    )
                }
                .and_then(parse::<SourceDefinitionId>)?,
            })
        }
        NativeMechanicsActiveEffectProvenanceKind::Request => Ok(SourceInstanceIdentity::Request {
            operation: unsafe {
                text(
                    request.request_operation,
                    "mechanics effect request provenance operation",
                )
            }
            .and_then(parse::<OperationId>)?,
            instance: unsafe {
                text(
                    request.request_instance,
                    "mechanics effect request provenance instance",
                )
            }
            .and_then(parse::<SourceInstanceId>)?,
        }),
    }
}
fn native_active_effect_row(
    effect: &ActiveEffectInstance,
    text: &mut CatalogLeaseText,
) -> NativeMechanicsActiveEffectComponentRow {
    let mut row = NativeMechanicsActiveEffectComponentRow {
        instance: text.copy(effect.instance().as_str()),
        definition: text.copy(effect.definition().as_str()),
        stacks: effect.stacks(),
        provenance_kind: NativeMechanicsActiveEffectProvenanceKind::Intrinsic,
        intrinsic_entity_id: 0,
        intrinsic_instance: text.copy(""),
        effect_entity_id: 0,
        effect_instance: text.copy(""),
        effect_stack: 0,
        effect_source: text.copy(""),
        equipped_owner_entity_id: 0,
        equipped_item_entity_id: 0,
        equipped_source: text.copy(""),
        request_operation: text.copy(""),
        request_instance: text.copy(""),
    };
    match effect.provenance() {
        SourceInstanceIdentity::Intrinsic { entity, instance } => {
            row.provenance_kind = NativeMechanicsActiveEffectProvenanceKind::Intrinsic;
            row.intrinsic_entity_id = entity.raw();
            row.intrinsic_instance = text.copy(instance.as_str());
        }
        SourceInstanceIdentity::Effect {
            entity,
            effect,
            stack,
            source,
        } => {
            row.provenance_kind = NativeMechanicsActiveEffectProvenanceKind::Effect;
            row.effect_entity_id = entity.raw();
            row.effect_instance = text.copy(effect.as_str());
            row.effect_stack = *stack;
            row.effect_source = text.copy(source.as_str());
        }
        SourceInstanceIdentity::EquippedItem {
            owner,
            item,
            source,
        } => {
            row.provenance_kind = NativeMechanicsActiveEffectProvenanceKind::EquippedItem;
            row.equipped_owner_entity_id = owner.raw();
            row.equipped_item_entity_id = item.raw();
            row.equipped_source = text.copy(source.as_str());
        }
        SourceInstanceIdentity::Request {
            operation,
            instance,
        } => {
            row.provenance_kind = NativeMechanicsActiveEffectProvenanceKind::Request;
            row.request_operation = text.copy(operation.as_str());
            row.request_instance = text.copy(instance.as_str());
        }
    }
    row
}
fn native_effect_source_activation(
    value: &EffectSourceActivation,
    text: &mut CatalogLeaseText,
) -> NativeMechanicsEffectSourceActivationRow {
    NativeMechanicsEffectSourceActivationRow {
        identity: native_source_identity(&value.identity, text),
        definition: text.copy(value.definition.as_str()),
    }
}
fn native_effect_mutation_kind(value: EffectMutationKind) -> NativeMechanicsEffectMutationKind {
    match value {
        EffectMutationKind::Apply => NativeMechanicsEffectMutationKind::Apply,
        EffectMutationKind::Refresh => NativeMechanicsEffectMutationKind::Refresh,
        EffectMutationKind::Replace => NativeMechanicsEffectMutationKind::Replace,
        EffectMutationKind::Remove => NativeMechanicsEffectMutationKind::Remove,
        EffectMutationKind::Expire => NativeMechanicsEffectMutationKind::Expire,
    }
}
fn active_effects_revision(entity: EntityId, revision: u64) -> NativeMechanicsComponentRevision {
    NativeMechanicsComponentRevision {
        entity_id: entity.raw(),
        revision,
        component: NativeMechanicsRevisionComponent::ActiveEffects,
        present: true,
    }
}
fn native_stat_decision(
    value: &StatDecision,
    text: &mut CatalogLeaseText,
) -> NativeMechanicsStatDecisionRow {
    let (
        has_contribution,
        contribution_kind,
        contribution_amount,
        ratio_numerator,
        ratio_denominator,
    ) = match &value.contribution {
        Some(StatContribution::Add { amount }) => (
            true,
            NativeMechanicsContributionKind::Add,
            amount.get(),
            0,
            0,
        ),
        Some(StatContribution::Scale { ratio }) => (
            true,
            NativeMechanicsContributionKind::Scale,
            0,
            ratio.numerator(),
            ratio.denominator(),
        ),
        Some(StatContribution::Minimum { value }) => (
            true,
            NativeMechanicsContributionKind::Minimum,
            value.get(),
            0,
            0,
        ),
        Some(StatContribution::Maximum { value }) => (
            true,
            NativeMechanicsContributionKind::Maximum,
            value.get(),
            0,
            0,
        ),
        None => (false, NativeMechanicsContributionKind::Add, 0, 0, 0),
    };
    NativeMechanicsStatDecisionRow {
        source: native_source_identity(&value.source, text),
        source_definition: text.copy(value.source_definition.as_str()),
        has_contribution_index: value.contribution_index.is_some(),
        contribution_index: value.contribution_index.unwrap_or_default(),
        outcome: match value.outcome {
            DecisionOutcome::Applied => NativeMechanicsDecisionOutcome::Applied,
            DecisionOutcome::Suppressed => NativeMechanicsDecisionOutcome::Suppressed,
            DecisionOutcome::Inapplicable => NativeMechanicsDecisionOutcome::Inapplicable,
        },
        has_stacking_group: value.stacking_group.is_some(),
        stacking_group: text.copy(
            value
                .stacking_group
                .as_ref()
                .map_or("", |group| group.as_str()),
        ),
        has_stacking: value.stacking.is_some(),
        stacking: match value.stacking.unwrap_or(StackingPolicy::Sum) {
            StackingPolicy::Sum => NativeMechanicsStackingPolicy::Sum,
            StackingPolicy::Highest => NativeMechanicsStackingPolicy::Highest,
            StackingPolicy::Lowest => NativeMechanicsStackingPolicy::Lowest,
            StackingPolicy::UniqueBySource => NativeMechanicsStackingPolicy::UniqueBySource,
        },
        has_contribution,
        contribution_kind,
        contribution_amount,
        contribution_ratio_numerator: ratio_numerator,
        contribution_ratio_denominator: ratio_denominator,
    }
}
fn insert_track_operation_lease(
    bridge: &mut RuntimeMechanicsBridge,
    text: CatalogLeaseText,
    observed_revisions: Vec<NativeMechanicsObservedComponentRevisionRow>,
) -> Option<(
    NativeMechanicsOperationLeaseHandle,
    *const NativeMechanicsObservedComponentRevisionRow,
)> {
    let handle = bridge.insert_operation_lease(OperationLeaseBacking {
        _text: text.values,
        rows: OperationLeaseRows::Track { observed_revisions },
    })?;
    let OperationLeaseRows::Track { observed_revisions } = &bridge
        .operation_leases
        .get(&handle.value)
        .expect("just inserted track operation lease")
        .rows
    else {
        unreachable!("track operation lease row kind matches its reader")
    };
    Some((handle, observed_revisions.as_ptr()))
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

fn component_read_revision(
    state: &EntityState,
    entity: EntityId,
    component: NativeMechanicsRevisionComponent,
) -> u64 {
    match component {
        NativeMechanicsRevisionComponent::Stats => state
            .component_revision::<StatsComponent>(entity)
            .map(|value| value.revision())
            .unwrap_or_default(),
        NativeMechanicsRevisionComponent::Tracks => state
            .component_revision::<TracksComponent>(entity)
            .map(|value| value.revision())
            .unwrap_or_default(),
        NativeMechanicsRevisionComponent::IntrinsicSources => state
            .component_revision::<IntrinsicSourcesComponent>(entity)
            .map(|value| value.revision())
            .unwrap_or_default(),
        NativeMechanicsRevisionComponent::ActiveEffects => state
            .component_revision::<ActiveEffectsComponent>(entity)
            .map(|value| value.revision())
            .unwrap_or_default(),
        NativeMechanicsRevisionComponent::Inventory => state
            .component_revision::<InventoryComponent>(entity)
            .map(|value| value.revision())
            .unwrap_or_default(),
        NativeMechanicsRevisionComponent::Item => state
            .component_revision::<ItemComponent>(entity)
            .map(|value| value.revision())
            .unwrap_or_default(),
        NativeMechanicsRevisionComponent::Equipment => state
            .component_revision::<EquipmentComponent>(entity)
            .map(|value| value.revision())
            .unwrap_or_default(),
    }
}

fn component_is_present(
    state: &EntityState,
    entity: EntityId,
    component: NativeMechanicsRevisionComponent,
) -> bool {
    match component {
        NativeMechanicsRevisionComponent::Stats => state
            .component::<StatsComponent>(entity)
            .ok()
            .flatten()
            .is_some(),
        NativeMechanicsRevisionComponent::Tracks => state
            .component::<TracksComponent>(entity)
            .ok()
            .flatten()
            .is_some(),
        NativeMechanicsRevisionComponent::IntrinsicSources => state
            .component::<IntrinsicSourcesComponent>(entity)
            .ok()
            .flatten()
            .is_some(),
        NativeMechanicsRevisionComponent::ActiveEffects => state
            .component::<ActiveEffectsComponent>(entity)
            .ok()
            .flatten()
            .is_some(),
        NativeMechanicsRevisionComponent::Inventory => state
            .component::<InventoryComponent>(entity)
            .ok()
            .flatten()
            .is_some(),
        NativeMechanicsRevisionComponent::Item => state
            .component::<ItemComponent>(entity)
            .ok()
            .flatten()
            .is_some(),
        NativeMechanicsRevisionComponent::Equipment => state
            .component::<EquipmentComponent>(entity)
            .ok()
            .flatten()
            .is_some(),
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
#[cfg(test)]
mod tests {
    use super::*;

    fn utf8(value: &'static str) -> NativeUtf8Slice {
        NativeUtf8Slice {
            bytes: value.as_ptr(),
            len: value.len(),
        }
    }

    fn empty_initial_components(
        entity: NativeMechanicsEntityHandle,
    ) -> NativeMechanicsInitialComponentsRequest {
        NativeMechanicsInitialComponentsRequest {
            entity,
            has_stats: false,
            stats: std::ptr::null(),
            stats_len: 0,
            has_tracks: false,
            tracks: std::ptr::null(),
            tracks_len: 0,
            has_intrinsic_sources: false,
            intrinsic_sources: std::ptr::null(),
            intrinsic_sources_len: 0,
            has_active_effects: false,
            active_effects: std::ptr::null(),
            active_effects_len: 0,
            has_inventory: false,
            inventory_stacks: std::ptr::null(),
            inventory_stacks_len: 0,
            inventory_capacity_limits: std::ptr::null(),
            inventory_capacity_limits_len: 0,
            has_item: false,
            item_definition: utf8(""),
            has_equipment: false,
            equipment_assignments: std::ptr::null(),
            equipment_assignments_len: 0,
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
        assert_eq!(
            unsafe {
                define_track(
                    context,
                    &NativeMechanicsTrackDefinitionRequest {
                        catalog,
                        id: utf8("shield"),
                        minimum: 0,
                        maximum_kind: NativeMechanicsTrackMaximumKind::Fixed,
                        fixed_maximum: 3,
                        maximum_stat: utf8(""),
                    },
                )
            },
            ABI_OK
        );
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
        for source in [
            "prevent_source",
            "flat_source",
            "scale_source",
            "absorb_source",
        ] {
            assert_eq!(
                unsafe {
                    define_source(
                        context,
                        &NativeMechanicsSourceDefinitionRequest {
                            catalog,
                            id: utf8(source),
                            priority: 0,
                        },
                    )
                },
                ABI_OK
            );
        }
        for kind in ["physical", "prevented"] {
            assert_eq!(
                unsafe {
                    define_damage_kind(
                        context,
                        &NativeMechanicsDamageKindDefinitionRequest {
                            catalog,
                            id: utf8(kind),
                        },
                    )
                },
                ABI_OK
            );
        }
        for (source, kind, selector, amount, numerator, denominator, absorb_track) in [
            (
                "prevent_source",
                NativeMechanicsDamageResponseKind::Prevent,
                "prevented",
                0,
                0,
                0,
                "",
            ),
            (
                "flat_source",
                NativeMechanicsDamageResponseKind::FlatReduction,
                "",
                2,
                0,
                0,
                "",
            ),
            (
                "scale_source",
                NativeMechanicsDamageResponseKind::Scale,
                "",
                0,
                1,
                2,
                "",
            ),
            (
                "absorb_source",
                NativeMechanicsDamageResponseKind::Absorb,
                "",
                0,
                0,
                0,
                "shield",
            ),
        ] {
            assert_eq!(
                unsafe {
                    define_damage_response(
                        context,
                        &NativeMechanicsDamageResponseDefinitionRequest {
                            catalog,
                            source: utf8(source),
                            kind,
                            selector_is_exact: !selector.is_empty(),
                            selector_damage_kind: utf8(selector),
                            amount,
                            ratio_numerator: numerator,
                            ratio_denominator: denominator,
                            stacking_group: utf8(source),
                            stacking: NativeMechanicsStackingPolicy::Sum,
                            absorb_track: utf8(absorb_track),
                        },
                    )
                },
                ABI_OK
            );
        }
        let effect_sources = [NativeMechanicsText {
            value: utf8("bonus"),
        }];
        for (id, stacking_group, stacking) in [
            (
                "applied",
                "applied_group",
                NativeMechanicsEffectStackingKind::IndependentByProvenance,
            ),
            (
                "refreshing",
                "refreshing_group",
                NativeMechanicsEffectStackingKind::Refresh,
            ),
            (
                "replacing",
                "replacing_group",
                NativeMechanicsEffectStackingKind::Replace,
            ),
        ] {
            assert_eq!(
                unsafe {
                    define_effect(
                        context,
                        &NativeMechanicsEffectDefinitionRequest {
                            catalog,
                            id: utf8(id),
                            stacking_group: utf8(stacking_group),
                            stacking,
                            maximum_instances: 3,
                            maximum_stacks: 3,
                            sources: effect_sources.as_ptr(),
                            sources_len: effect_sources.len(),
                        },
                    )
                },
                ABI_OK
            );
        }
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
        assert_eq!(
            unsafe {
                set_initial_components(
                    context,
                    &NativeMechanicsInitialComponentsRequest {
                        entity,
                        has_stats: false,
                        stats: std::ptr::null(),
                        stats_len: 0,
                        has_tracks: false,
                        tracks: std::ptr::null(),
                        tracks_len: 0,
                        has_intrinsic_sources: false,
                        intrinsic_sources: std::ptr::null(),
                        intrinsic_sources_len: 0,
                        has_active_effects: true,
                        active_effects: std::ptr::null(),
                        active_effects_len: 0,
                        has_inventory: false,
                        inventory_stacks: std::ptr::null(),
                        inventory_stacks_len: 0,
                        inventory_capacity_limits: std::ptr::null(),
                        inventory_capacity_limits_len: 0,
                        has_item: false,
                        item_definition: utf8(""),
                        has_equipment: false,
                        equipment_assignments: std::ptr::null(),
                        equipment_assignments_len: 0,
                    },
                )
            },
            ABI_OK
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
        assert_eq!(
            unsafe {
                set_initial_track(
                    context,
                    &NativeMechanicsInitialTrackRequest {
                        entity,
                        track: utf8("shield"),
                        current: 3,
                    },
                )
            },
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
        for (instance, definition) in [
            ("prevent_instance", "prevent_source"),
            ("flat_instance", "flat_source"),
            ("scale_instance", "scale_source"),
            ("absorb_instance", "absorb_source"),
        ] {
            assert_eq!(
                unsafe {
                    bind_intrinsic_source(
                        context,
                        &NativeMechanicsIntrinsicSourceRequest {
                            entity,
                            instance: utf8(instance),
                            definition: utf8(definition),
                        },
                    )
                },
                ABI_OK
            );
        }
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
        assert!(entity_receipt.active_effects_revision.present);
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
        let mut rejected_cross_entity = NativeMechanicsTrackMutationLease::default();
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
        let mut rejected_cross_component = NativeMechanicsStatMutationLease::default();
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

        let request_sources = [NativeMechanicsRequestSource {
            instance: utf8("request_bonus"),
            definition: utf8("bonus"),
        }];
        let evaluation_request = NativeMechanicsStatOperationRequest {
            entity,
            stat: utf8("strength"),
            operation: utf8("evaluate"),
            request_sources: request_sources.as_ptr(),
            request_sources_len: request_sources.len(),
        };
        let mut evaluation = NativeMechanicsStatEvaluationLease::default();
        assert_eq!(
            unsafe { evaluate_stat(context, &evaluation_request, &mut evaluation) },
            ABI_OK
        );
        assert_eq!(evaluation.value, 14);
        assert_eq!(evaluation.decisions_len, 6);
        assert!(evaluation.observed_revisions_len >= 2);
        assert_eq!(evaluation.source_cost.request_entries_visited, 1);
        let decisions =
            unsafe { std::slice::from_raw_parts(evaluation.decisions, evaluation.decisions_len) };
        assert!(decisions.iter().any(|decision| {
            decision.source.kind == NativeMechanicsActiveEffectProvenanceKind::Request
        }));
        assert_eq!(
            unsafe { destroy_operation_lease(context, evaluation.handle) },
            ABI_OK
        );

        let mut track_read = NativeMechanicsTrackReadLease::default();
        assert_eq!(
            unsafe {
                read_track(
                    context,
                    &NativeMechanicsTrackReadRequest {
                        entity,
                        track: utf8("stamina"),
                        operation: utf8("read_stamina"),
                    },
                    &mut track_read,
                )
            },
            ABI_OK
        );
        assert_eq!(track_read.current, 12);
        assert_eq!(track_read.maximum, 12);
        assert!(track_read.observed_revisions_len >= 2);
        assert_eq!(
            unsafe { destroy_operation_lease(context, track_read.handle) },
            ABI_OK
        );

        let spend_request = NativeMechanicsTrackMutationRequest {
            entity,
            operation: utf8("spend"),
            source: utf8("spend_source"),
            track: utf8("stamina"),
            amount: 2,
            revision_guard: NativeMechanicsRevisionGuard::Exact,
            expected_revision: entity_receipt.tracks_revision,
        };
        let mut spend = NativeMechanicsTrackMutationLease::default();
        assert_eq!(
            unsafe { spend_track(context, &spend_request, &mut spend) },
            ABI_OK
        );
        assert_eq!(spend.before, 12);
        assert_eq!(spend.after, 10);
        assert_eq!(spend.applied_amount, 2);
        assert_eq!(spend.kind, NativeMechanicsTrackAdjustmentKind::Spend);
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
        let mut set = NativeMechanicsTrackSetLease::default();
        assert_eq!(
            unsafe { set_track(context, &set_request, &mut set) },
            ABI_OK
        );
        assert_eq!(set.target, 9);
        assert_eq!(set.after, 9);
        let mut restore = NativeMechanicsTrackMutationLease::default();
        assert_eq!(
            unsafe {
                restore_track(
                    context,
                    &NativeMechanicsTrackMutationRequest {
                        entity,
                        operation: utf8("restore"),
                        source: utf8("restore_source"),
                        track: utf8("stamina"),
                        amount: 10,
                        revision_guard: NativeMechanicsRevisionGuard::Exact,
                        expected_revision: set.committed_revision,
                    },
                    &mut restore,
                )
            },
            ABI_OK
        );
        assert_eq!(restore.after, 12);
        assert_eq!(restore.applied_amount, 3);
        assert_eq!(restore.kind, NativeMechanicsTrackAdjustmentKind::Restore);
        let mut reconciliation = NativeMechanicsTrackReconciliationLease::default();
        assert_eq!(
            unsafe {
                reconcile_track(
                    context,
                    &NativeMechanicsTrackReconciliationRequest {
                        entity,
                        operation: utf8("reconcile"),
                        source: utf8("reconcile_source"),
                        track: utf8("stamina"),
                        prospective_maximum: 10,
                        policy: NativeMechanicsTrackReconciliationPolicy::ClampToMaximum,
                        revision_guard: NativeMechanicsRevisionGuard::Exact,
                        expected_revision: restore.committed_revision,
                    },
                    &mut reconciliation,
                )
            },
            ABI_OK
        );
        assert_eq!(reconciliation.after, 10);
        assert_eq!(reconciliation.current_maximum, 12);
        assert_eq!(reconciliation.prospective_maximum, 10);
        assert_eq!(
            unsafe { destroy_operation_lease(context, spend.handle) },
            ABI_OK
        );
        assert_eq!(
            unsafe { destroy_operation_lease(context, set.handle) },
            ABI_OK
        );
        assert_eq!(
            unsafe { destroy_operation_lease(context, restore.handle) },
            ABI_OK
        );
        assert_eq!(
            unsafe { destroy_operation_lease(context, reconciliation.handle) },
            ABI_OK
        );
        let damage_parts = [
            NativeMechanicsDamagePart {
                kind: utf8("physical"),
                amount: 28,
            },
            NativeMechanicsDamagePart {
                kind: utf8("prevented"),
                amount: 10,
            },
        ];
        let damage_request = |expected_tracks_revision| NativeMechanicsDamageRequest {
            operation: utf8("damage_operation"),
            source_kind: NativeMechanicsActiveEffectProvenanceKind::Request,
            source_intrinsic_entity_id: 0,
            source_intrinsic_instance: utf8(""),
            source_effect_entity_id: 0,
            source_effect_instance: utf8(""),
            source_effect_stack: 0,
            source_effect_source: utf8(""),
            source_equipped_owner_entity_id: 0,
            source_equipped_item_entity_id: 0,
            source_equipped_source: utf8(""),
            source_request_operation: utf8("damage_source_operation"),
            source_request_instance: utf8("damage_source_instance"),
            has_actor: true,
            actor_entity_id: 78,
            target: entity,
            target_track: utf8("stamina"),
            parts: damage_parts.as_ptr(),
            parts_len: damage_parts.len(),
            request_sources: request_sources.as_ptr(),
            request_sources_len: request_sources.len(),
            has_expected_tracks_revision: true,
            expected_tracks_revision,
        };
        let mut damage_preview = NativeMechanicsDamageLease::default();
        assert_eq!(
            unsafe {
                preview_damage(
                    context,
                    &damage_request(reconciliation.committed_revision),
                    &mut damage_preview,
                )
            },
            ABI_OK
        );
        assert!(!damage_preview.has_committed_tracks_revision);
        assert_eq!(damage_preview.catalog_id, catalog.value);
        assert_eq!(damage_preview.parts_len, 2);
        assert!(damage_preview.decisions_len >= 10);
        assert_eq!(damage_preview.track_changes_len, 2);
        assert_eq!(damage_preview.protection_track_depletions_len, 1);
        assert_eq!(damage_preview.target_track_depletions_len, 1);
        assert!(damage_preview.observed_revisions_len >= 2);
        assert_eq!(damage_preview.source_cost.request_entries_visited, 1);
        assert!(damage_preview.has_actor);
        assert_eq!(damage_preview.actor_entity_id, 78);
        let preview_parts =
            unsafe { std::slice::from_raw_parts(damage_preview.parts, damage_preview.parts_len) };
        assert_eq!(preview_parts[0].combined_scale_numerator.low, 1);
        assert_eq!(preview_parts[0].combined_scale_numerator.high, 0);
        assert_eq!(preview_parts[0].combined_scale_denominator.low, 2);
        assert_eq!(preview_parts[0].combined_scale_denominator.high, 0);
        let preview_decisions = unsafe {
            std::slice::from_raw_parts(damage_preview.decisions, damage_preview.decisions_len)
        };
        assert!(preview_decisions
            .iter()
            .any(|value| { value.kind == NativeMechanicsDamageDecisionKind::NoDamageResponse }));
        assert!(preview_decisions
            .iter()
            .any(|value| { value.kind == NativeMechanicsDamageDecisionKind::Prevent }));
        assert!(preview_decisions.iter().any(|value| {
            value.kind == NativeMechanicsDamageDecisionKind::FlatReduction && value.amount == 2
        }));
        assert!(preview_decisions.iter().any(|value| {
            value.kind == NativeMechanicsDamageDecisionKind::Scale
                && value.ratio_numerator == 1
                && value.ratio_denominator == 2
        }));
        assert!(preview_decisions
            .iter()
            .any(|value| { value.kind == NativeMechanicsDamageDecisionKind::Absorb }));
        let mut damage_applied = NativeMechanicsDamageLease::default();
        assert_eq!(
            unsafe {
                apply_damage(
                    context,
                    &damage_request(damage_preview.observed_tracks_revision),
                    &mut damage_applied,
                )
            },
            ABI_OK
        );
        assert!(damage_applied.has_committed_tracks_revision);
        assert_eq!(
            damage_applied.committed_tracks_revision.component,
            NativeMechanicsRevisionComponent::Tracks
        );
        assert_eq!(
            unsafe { destroy_operation_lease(context, damage_preview.handle) },
            ABI_OK
        );
        assert_eq!(
            unsafe { destroy_operation_lease(context, damage_applied.handle) },
            ABI_OK
        );
        let mut stat_mutation = NativeMechanicsStatMutationLease::default();
        assert_eq!(
            unsafe {
                set_stat_base(
                    context,
                    &NativeMechanicsStatBaseMutationRequest {
                        entity,
                        operation: utf8("set_base"),
                        source: utf8("set_base_source"),
                        stat: utf8("strength"),
                        base: 13,
                        revision_guard: NativeMechanicsRevisionGuard::Exact,
                        expected_revision: entity_receipt.stats_revision,
                    },
                    &mut stat_mutation,
                )
            },
            ABI_OK
        );
        assert_eq!(stat_mutation.before, 10);
        assert_eq!(stat_mutation.after, 13);
        assert!(stat_mutation.observed_revisions_len >= 1);
        assert_eq!(
            stat_mutation.source.kind,
            NativeMechanicsActiveEffectProvenanceKind::Request
        );
        assert_eq!(
            unsafe { destroy_operation_lease(context, stat_mutation.handle) },
            ABI_OK
        );
        let effect_request = |operation, instance, definition, expected_revision| {
            NativeMechanicsEffectMutationRequest {
                entity,
                operation: utf8(operation),
                instance: utf8(instance),
                definition: utf8(definition),
                provenance_kind: NativeMechanicsActiveEffectProvenanceKind::Request,
                intrinsic_entity_id: 0,
                intrinsic_instance: utf8(""),
                effect_entity_id: 0,
                effect_instance: utf8(""),
                effect_stack: 0,
                effect_source: utf8(""),
                equipped_owner_entity_id: 0,
                equipped_item_entity_id: 0,
                equipped_source: utf8(""),
                request_operation: utf8("effect_provenance"),
                request_instance: utf8("effect_request"),
                stacks: 1,
                revision_guard: NativeMechanicsRevisionGuard::Exact,
                expected_revision,
            }
        };
        let mut applied = NativeMechanicsEffectOperationLease::default();
        assert_eq!(
            unsafe {
                apply_effect(
                    context,
                    &effect_request(
                        "apply_effect",
                        "applied_instance",
                        "applied",
                        entity_receipt.active_effects_revision,
                    ),
                    &mut applied,
                )
            },
            ABI_OK
        );
        assert_eq!(applied.kind, NativeMechanicsEffectMutationKind::Apply);
        assert!(applied.has_current);
        assert_eq!(applied.activated_sources_len, 1);
        assert!(applied.observed_revisions_len >= 2);
        assert_eq!(
            unsafe { (*applied.activated_sources).identity.kind },
            NativeMechanicsActiveEffectProvenanceKind::Effect
        );
        let mut refreshing = NativeMechanicsEffectOperationLease::default();
        assert_eq!(
            unsafe {
                apply_effect(
                    context,
                    &effect_request(
                        "apply_refreshing",
                        "refreshing_instance",
                        "refreshing",
                        applied.committed_revision,
                    ),
                    &mut refreshing,
                )
            },
            ABI_OK
        );
        let mut refreshed = NativeMechanicsEffectOperationLease::default();
        let refresh_request = NativeMechanicsEffectRefreshRequest {
            entity,
            operation: utf8("refresh_effect"),
            instance: utf8("refreshing_instance"),
            provenance_kind: NativeMechanicsActiveEffectProvenanceKind::Request,
            intrinsic_entity_id: 0,
            intrinsic_instance: utf8(""),
            effect_entity_id: 0,
            effect_instance: utf8(""),
            effect_stack: 0,
            effect_source: utf8(""),
            equipped_owner_entity_id: 0,
            equipped_item_entity_id: 0,
            equipped_source: utf8(""),
            request_operation: utf8("effect_provenance"),
            request_instance: utf8("effect_request"),
            stacks: 1,
            revision_guard: NativeMechanicsRevisionGuard::Exact,
            expected_revision: refreshing.committed_revision,
        };
        assert_eq!(
            unsafe { refresh_effect(context, &refresh_request, &mut refreshed) },
            ABI_OK
        );
        assert_eq!(refreshed.kind, NativeMechanicsEffectMutationKind::Refresh);
        assert_eq!(refreshed.removed_len, 1);
        assert_eq!(refreshed.activated_sources_len, 1);
        let mut replacing = NativeMechanicsEffectOperationLease::default();
        assert_eq!(
            unsafe {
                apply_effect(
                    context,
                    &effect_request(
                        "apply_replacing",
                        "replacing_old",
                        "replacing",
                        refreshed.committed_revision,
                    ),
                    &mut replacing,
                )
            },
            ABI_OK
        );
        let mut replaced = NativeMechanicsEffectOperationLease::default();
        assert_eq!(
            unsafe {
                replace_effect(
                    context,
                    &effect_request(
                        "replace_effect",
                        "replacing_new",
                        "replacing",
                        replacing.committed_revision,
                    ),
                    &mut replaced,
                )
            },
            ABI_OK
        );
        assert_eq!(replaced.kind, NativeMechanicsEffectMutationKind::Replace);
        assert_eq!(replaced.removed_len, 1);
        assert_eq!(replaced.activated_sources_len, 1);
        let remove_request = NativeMechanicsEffectRemovalRequest {
            entity,
            operation: utf8("remove_effect"),
            instance: utf8("applied_instance"),
            revision_guard: NativeMechanicsRevisionGuard::Exact,
            expected_revision: replaced.committed_revision,
        };
        let mut removed = NativeMechanicsEffectOperationLease::default();
        assert_eq!(
            unsafe { remove_effect(context, &remove_request, &mut removed) },
            ABI_OK
        );
        assert_eq!(removed.kind, NativeMechanicsEffectMutationKind::Remove);
        assert!(!removed.has_current);
        assert_eq!(removed.removed_len, 1);
        assert_eq!(removed.activated_sources_len, 0);
        let expire_request = NativeMechanicsEffectRemovalRequest {
            entity,
            operation: utf8("expire_effect"),
            instance: utf8("refreshing_instance"),
            revision_guard: NativeMechanicsRevisionGuard::Exact,
            expected_revision: removed.committed_revision,
        };
        let mut expired = NativeMechanicsEffectOperationLease::default();
        assert_eq!(
            unsafe { expire_effect(context, &expire_request, &mut expired) },
            ABI_OK
        );
        assert_eq!(expired.kind, NativeMechanicsEffectMutationKind::Expire);
        assert_eq!(expired.removed_len, 1);
        assert_eq!(expired.activated_sources_len, 0);
        for handle in [
            applied.handle,
            refreshing.handle,
            refreshed.handle,
            replacing.handle,
            replaced.handle,
            removed.handle,
            expired.handle,
        ] {
            assert_eq!(unsafe { destroy_operation_lease(context, handle) }, ABI_OK);
        }
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

    fn copied_diagnostic(value: NativeUtf8Slice) -> String {
        unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(value.bytes, value.len)) }
            .to_owned()
    }

    fn assert_operation_diagnostic(
        api: &NativeMechanicsApi,
        receipt: NativeOperationErrorReceipt,
        code: &str,
        source_fragment: &str,
    ) {
        assert_eq!(copied_diagnostic(receipt.service), "Mechanics");
        assert_eq!(receipt.status, 0);
        assert_eq!(receipt.diagnostics.diagnostics_len, 1);
        let diagnostic = unsafe { *receipt.diagnostics.diagnostics };
        assert_eq!(copied_diagnostic(diagnostic.code), code);
        assert!(copied_diagnostic(diagnostic.source).contains(source_fragment));
        assert_eq!(
            unsafe {
                (api.destroy_operation_diagnostic_lease)(api.context, receipt.diagnostics.handle)
            },
            ABI_OK
        );
        assert_eq!(
            unsafe {
                (api.destroy_operation_diagnostic_lease)(api.context, receipt.diagnostics.handle)
            },
            0
        );
    }

    #[test]
    fn generated_mechanics_callbacks_copy_and_release_typed_owner_diagnostics() {
        let mut bridge = RuntimeMechanicsBridge::new();
        let api = api(&mut bridge);

        let mut receipt = unsafe { std::mem::zeroed::<NativeOperationErrorReceipt>() };
        assert_eq!(
            unsafe {
                (api.admit_catalog)(
                    api.context,
                    NativeMechanicsCatalogHandle { value: 999 },
                    &mut receipt,
                )
            },
            0
        );
        assert_operation_diagnostic(&api, receipt, "MECHANICS_CATALOG_NOT_FOUND", "catalog:999");

        let mut component = unsafe { std::mem::zeroed::<NativeMechanicsStatComponentLease>() };
        receipt = unsafe { std::mem::zeroed() };
        assert_eq!(
            unsafe {
                (api.read_stat_component)(
                    api.context,
                    NativeMechanicsEntityHandle { value: 888 },
                    &mut component,
                    &mut receipt,
                )
            },
            0
        );
        assert_operation_diagnostic(
            &api,
            receipt,
            "MECHANICS_ENTITY_NOT_FOUND",
            "entity-handle:888",
        );

        let mut catalog = NativeMechanicsCatalogHandle::default();
        assert_eq!(
            unsafe {
                create_catalog(
                    api.context,
                    &NativeMechanicsCatalogCreateRequest {
                        version: utf8("diagnostic_fixture"),
                    },
                    &mut catalog,
                )
            },
            ABI_OK
        );
        assert_eq!(
            unsafe {
                define_track(
                    api.context,
                    &NativeMechanicsTrackDefinitionRequest {
                        catalog,
                        id: utf8("stamina"),
                        minimum: 0,
                        maximum_kind: NativeMechanicsTrackMaximumKind::Fixed,
                        fixed_maximum: 10,
                        maximum_stat: utf8(""),
                    },
                )
            },
            ABI_OK
        );
        assert_eq!(unsafe { admit_catalog(api.context, catalog) }, ABI_OK);
        let mut entity = NativeMechanicsEntityHandle::default();
        assert_eq!(
            unsafe {
                bind_entity(
                    api.context,
                    &NativeMechanicsEntityBindRequest {
                        catalog,
                        entity_id: 77,
                        identity: utf8("diagnostic_entity"),
                    },
                    &mut entity,
                )
            },
            ABI_OK
        );
        assert_eq!(
            unsafe {
                set_initial_track(
                    api.context,
                    &NativeMechanicsInitialTrackRequest {
                        entity,
                        track: utf8("stamina"),
                        current: 5,
                    },
                )
            },
            ABI_OK
        );
        let mut committed = NativeMechanicsEntityReceipt::default();
        assert_eq!(
            unsafe { commit_entity(api.context, entity, &mut committed) },
            ABI_OK
        );

        let mut lifecycle = NativeMechanicsLifecycleReceipt::default();
        receipt = unsafe { std::mem::zeroed() };
        assert_eq!(
            unsafe {
                (api.set_entity_lifecycle)(
                    api.context,
                    &NativeMechanicsLifecycleRequest {
                        entity,
                        lifecycle: NativeMechanicsEntityLifecycle::Disabled,
                        guard: NativeMechanicsLifecycleGuard::Exact,
                        expected_stamp: committed.lifecycle.stamp + 1,
                    },
                    &mut lifecycle,
                    &mut receipt,
                )
            },
            0
        );
        assert_operation_diagnostic(
            &api,
            receipt,
            "MECHANICS_LIFECYCLE_STALE",
            "entity:77 expected-stamp:",
        );

        let mut track = NativeMechanicsTrackSetLease::default();
        receipt = unsafe { std::mem::zeroed() };
        assert_eq!(
            unsafe {
                (api.set_track)(
                    api.context,
                    &NativeMechanicsTrackSetRequest {
                        entity,
                        operation: utf8("diagnostic_set_track"),
                        source: utf8("diagnostic_source"),
                        track: utf8("stamina"),
                        value: 4,
                        policy: NativeMechanicsTrackSetPolicy::RejectOutOfBounds,
                        revision_guard: NativeMechanicsRevisionGuard::Exact,
                        expected_revision: NativeMechanicsTracksRevision {
                            entity_id: 77,
                            revision: 999,
                            component: NativeMechanicsRevisionComponent::Tracks,
                        },
                    },
                    &mut track,
                    &mut receipt,
                )
            },
            0
        );
        assert_operation_diagnostic(
            &api,
            receipt,
            "MECHANICS_REVISION_STALE",
            "entity:77 component:Tracks expected-revision:999",
        );
    }

    #[test]
    fn catalog_stat_lease_is_copied_from_service_owned_storage_and_released_once() {
        let mut bridge = RuntimeMechanicsBridge::new();
        let context = (&mut bridge as *mut RuntimeMechanicsBridge).cast::<c_void>();
        let mut catalog = NativeMechanicsCatalogHandle::default();
        assert_eq!(
            unsafe {
                create_catalog(
                    context,
                    &NativeMechanicsCatalogCreateRequest {
                        version: utf8("catalog_lease"),
                    },
                    &mut catalog,
                )
            },
            ABI_OK
        );
        assert_eq!(
            unsafe {
                define_stat(
                    context,
                    &NativeMechanicsStatDefinitionRequest {
                        catalog,
                        id: utf8("strength"),
                        minimum: -2,
                        maximum: 9,
                    },
                )
            },
            ABI_OK
        );
        assert_eq!(unsafe { admit_catalog(context, catalog) }, ABI_OK);

        let mut identity = std::mem::MaybeUninit::<NativeMechanicsCatalogIdentityLease>::uninit();
        assert_eq!(
            unsafe { read_catalog_identity(context, catalog, identity.as_mut_ptr()) },
            ABI_OK
        );
        let identity = unsafe { identity.assume_init() };
        assert_eq!(identity.entries_len, 1);
        let identity_row = unsafe { &*identity.entries };
        assert_eq!(
            unsafe {
                std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                    identity_row.version.bytes,
                    identity_row.version.len,
                ))
            },
            "catalog_lease"
        );
        assert!(unsafe {
            std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                identity_row.fingerprint.bytes,
                identity_row.fingerprint.len,
            ))
        }
        .starts_with("sha256:"));
        assert_eq!(
            unsafe { destroy_catalog_lease(context, identity.handle) },
            ABI_OK
        );

        let mut lease = std::mem::MaybeUninit::<NativeMechanicsStatCatalogLease>::uninit();
        assert_eq!(
            unsafe { read_catalog_stats(context, catalog, lease.as_mut_ptr()) },
            ABI_OK
        );
        let lease = unsafe { lease.assume_init() };
        assert_eq!(lease.catalog_id, catalog.value);
        assert_eq!(lease.entries_len, 1);
        let row = unsafe { &*lease.entries };
        assert_eq!(
            unsafe {
                std::str::from_utf8_unchecked(std::slice::from_raw_parts(row.id.bytes, row.id.len))
            },
            "strength"
        );
        assert_eq!((row.minimum, row.maximum), (-2, 9));
        assert_eq!(
            unsafe { destroy_catalog_lease(context, lease.handle) },
            ABI_OK
        );
        assert_eq!(unsafe { destroy_catalog_lease(context, lease.handle) }, 0);
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
    fn component_leases_preserve_non_empty_empty_present_and_absent_components() {
        let mut bridge = RuntimeMechanicsBridge::new();
        let context = (&mut bridge as *mut RuntimeMechanicsBridge).cast::<c_void>();
        let mut catalog = NativeMechanicsCatalogHandle::default();
        assert_eq!(
            unsafe {
                create_catalog(
                    context,
                    &NativeMechanicsCatalogCreateRequest {
                        version: utf8("component-lease"),
                    },
                    &mut catalog,
                )
            },
            ABI_OK
        );
        assert_eq!(
            unsafe {
                define_stat(
                    context,
                    &NativeMechanicsStatDefinitionRequest {
                        catalog,
                        id: utf8("strength"),
                        minimum: 0,
                        maximum: 20,
                    },
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
                        entity_id: 99,
                        identity: utf8("readback"),
                    },
                    &mut entity,
                )
            },
            ABI_OK
        );
        let stats = [NativeMechanicsInitialStatValue {
            stat: utf8("strength"),
            base: 12,
        }];
        assert_eq!(
            unsafe {
                set_initial_components(
                    context,
                    &NativeMechanicsInitialComponentsRequest {
                        entity,
                        has_stats: true,
                        stats: stats.as_ptr(),
                        stats_len: stats.len(),
                        has_tracks: true,
                        tracks: std::ptr::null(),
                        tracks_len: 0,
                        has_intrinsic_sources: false,
                        intrinsic_sources: std::ptr::null(),
                        intrinsic_sources_len: 0,
                        has_active_effects: false,
                        active_effects: std::ptr::null(),
                        active_effects_len: 0,
                        has_inventory: false,
                        inventory_stacks: std::ptr::null(),
                        inventory_stacks_len: 0,
                        inventory_capacity_limits: std::ptr::null(),
                        inventory_capacity_limits_len: 0,
                        has_item: false,
                        item_definition: utf8(""),
                        has_equipment: false,
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

        let mut stats_lease = std::mem::MaybeUninit::<NativeMechanicsStatComponentLease>::uninit();
        assert_eq!(
            unsafe { read_stat_component(context, entity, stats_lease.as_mut_ptr()) },
            ABI_OK
        );
        let stats_lease = unsafe { stats_lease.assume_init() };
        assert!(stats_lease.metadata.present);
        assert_eq!(stats_lease.metadata.entity_id, 99);
        assert_eq!(
            stats_lease.metadata.component,
            NativeMechanicsRevisionComponent::Stats
        );
        assert_eq!(
            unsafe {
                std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                    stats_lease.metadata.catalog_version.bytes,
                    stats_lease.metadata.catalog_version.len,
                ))
            },
            "component-lease"
        );
        assert!(unsafe {
            std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                stats_lease.metadata.catalog_fingerprint.bytes,
                stats_lease.metadata.catalog_fingerprint.len,
            ))
        }
        .starts_with("sha256:"));
        assert_eq!(stats_lease.entries_len, 1);
        assert_eq!(unsafe { (*stats_lease.entries).base }, 12);
        assert_eq!(
            unsafe { destroy_component_lease(context, stats_lease.handle) },
            ABI_OK
        );
        assert_eq!(
            unsafe { destroy_component_lease(context, stats_lease.handle) },
            0
        );

        let mut disabled = NativeMechanicsLifecycleReceipt::default();
        assert_eq!(
            unsafe {
                set_entity_lifecycle(
                    context,
                    &NativeMechanicsLifecycleRequest {
                        entity,
                        lifecycle: NativeMechanicsEntityLifecycle::Disabled,
                        guard: NativeMechanicsLifecycleGuard::Unchecked,
                        expected_stamp: 0,
                    },
                    &mut disabled,
                )
            },
            ABI_OK
        );
        let mut tracks_lease =
            std::mem::MaybeUninit::<NativeMechanicsTrackComponentLease>::uninit();
        assert_eq!(
            unsafe { read_track_component(context, entity, tracks_lease.as_mut_ptr()) },
            ABI_OK
        );
        let tracks_lease = unsafe { tracks_lease.assume_init() };
        assert!(tracks_lease.metadata.present);
        assert_eq!(tracks_lease.entries_len, 0);
        assert_eq!(
            unsafe { destroy_component_lease(context, tracks_lease.handle) },
            ABI_OK
        );

        let mut item_lease = std::mem::MaybeUninit::<NativeMechanicsItemComponentLease>::uninit();
        assert_eq!(
            unsafe { read_item_component(context, entity, item_lease.as_mut_ptr()) },
            ABI_OK
        );
        let item_lease = unsafe { item_lease.assume_init() };
        assert!(!item_lease.metadata.present);
        assert_eq!(item_lease.entries_len, 0);
        assert_eq!(
            unsafe { destroy_component_lease(context, item_lease.handle) },
            ABI_OK
        );
    }

    #[test]
    fn prepared_world_restore_remaps_all_families_and_preserves_failed_candidates() {
        let mut bridge = RuntimeMechanicsBridge::new();
        let context = (&mut bridge as *mut RuntimeMechanicsBridge).cast::<c_void>();
        let mut catalog = NativeMechanicsCatalogHandle::default();
        assert_eq!(
            unsafe {
                create_catalog(
                    context,
                    &NativeMechanicsCatalogCreateRequest {
                        version: utf8("restore-components"),
                    },
                    &mut catalog,
                )
            },
            ABI_OK
        );
        assert_eq!(
            unsafe {
                define_stat(
                    context,
                    &NativeMechanicsStatDefinitionRequest {
                        catalog,
                        id: utf8("strength"),
                        minimum: 0,
                        maximum: 20,
                    },
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
                        entity_id: 501,
                        identity: utf8("restore-actor"),
                    },
                    &mut entity,
                )
            },
            ABI_OK
        );
        let stats = [NativeMechanicsInitialStatValue {
            stat: utf8("strength"),
            base: 12,
        }];
        assert_eq!(
            unsafe {
                set_initial_components(
                    context,
                    &NativeMechanicsInitialComponentsRequest {
                        entity,
                        has_stats: true,
                        stats: stats.as_ptr(),
                        stats_len: stats.len(),
                        // An empty Tracks component is distinct from the five absent families.
                        has_tracks: true,
                        tracks: std::ptr::null(),
                        tracks_len: 0,
                        has_intrinsic_sources: false,
                        intrinsic_sources: std::ptr::null(),
                        intrinsic_sources_len: 0,
                        has_active_effects: false,
                        active_effects: std::ptr::null(),
                        active_effects_len: 0,
                        has_inventory: false,
                        inventory_stacks: std::ptr::null(),
                        inventory_stacks_len: 0,
                        inventory_capacity_limits: std::ptr::null(),
                        inventory_capacity_limits_len: 0,
                        has_item: false,
                        item_definition: utf8(""),
                        has_equipment: false,
                        equipment_assignments: std::ptr::null(),
                        equipment_assignments_len: 0,
                    },
                )
            },
            ABI_OK
        );
        let mut committed = NativeMechanicsEntityReceipt::default();
        assert_eq!(
            unsafe { commit_entity(context, entity, &mut committed) },
            ABI_OK
        );

        let mut snapshot = NativeMechanicsWorldSnapshotHandle::default();
        assert_eq!(
            unsafe { capture_world_snapshot(context, catalog, &mut snapshot) },
            ABI_OK
        );
        let mut disabled = NativeMechanicsLifecycleReceipt::default();
        assert_eq!(
            unsafe {
                set_entity_lifecycle(
                    context,
                    &NativeMechanicsLifecycleRequest {
                        entity,
                        lifecycle: NativeMechanicsEntityLifecycle::Disabled,
                        guard: NativeMechanicsLifecycleGuard::Exact,
                        expected_stamp: committed.lifecycle.stamp,
                    },
                    &mut disabled,
                )
            },
            ABI_OK
        );
        let current_revision = bridge.catalogs[&catalog.value].world.state.revision();
        let before_failed_prepare = bridge.catalogs[&catalog.value].world.clone();
        let invalid_request = NativeMechanicsWorldRestoreRequest {
            catalog: NativeMechanicsCatalogHandle {
                value: catalog.value + 1,
            },
            snapshot,
            expected_state_revision: current_revision,
        };
        let mut rejected = NativeMechanicsWorldRestoreHandle::default();
        assert_eq!(
            unsafe { prepare_world_restore(context, &invalid_request, &mut rejected) },
            0
        );
        assert_eq!(
            bridge.catalogs[&catalog.value].world.state.revision(),
            before_failed_prepare.state.revision()
        );
        assert_eq!(
            bridge.catalogs[&catalog.value]
                .world
                .lifecycle_receipt(EntityId::new(501))
                .lifecycle,
            NativeMechanicsEntityLifecycle::Disabled
        );

        let request = NativeMechanicsWorldRestoreRequest {
            catalog,
            snapshot,
            expected_state_revision: current_revision,
        };
        let mut prepared = NativeMechanicsWorldRestoreHandle::default();
        assert_eq!(
            unsafe { prepare_world_restore(context, &request, &mut prepared) },
            ABI_OK
        );
        let mut lease = std::mem::MaybeUninit::<NativeMechanicsWorldRestoreLease>::uninit();
        assert_eq!(
            unsafe { read_world_restore(context, prepared, lease.as_mut_ptr()) },
            ABI_OK
        );
        let lease = unsafe { lease.assume_init() };
        let remaps = unsafe { std::slice::from_raw_parts(lease.revisions, lease.revisions_len) };
        assert_eq!(remaps.len(), 7);
        assert!(remaps
            .iter()
            .all(|row| row.restored_revision > row.snapshot_revision
                && row.restored_revision > row.current_revision));
        assert!(remaps
            .iter()
            .any(|row| row.component == NativeMechanicsRevisionComponent::Tracks && row.present));
        assert!(remaps.iter().any(|row| row.component
            == NativeMechanicsRevisionComponent::Inventory
            && !row.present));
        assert_eq!(
            unsafe { destroy_world_restore_lease(context, lease.handle) },
            ABI_OK
        );
        assert_eq!(unsafe { publish_world_restore(context, prepared) }, ABI_OK);
        assert_eq!(
            bridge.catalogs[&catalog.value]
                .world
                .lifecycle_receipt(EntityId::new(501))
                .lifecycle,
            NativeMechanicsEntityLifecycle::Active
        );
        assert_eq!(unsafe { destroy_world_restore(context, prepared) }, ABI_OK);
        assert_eq!(unsafe { destroy_world_snapshot(context, snapshot) }, ABI_OK);
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

    #[test]
    fn staged_containment_precedes_non_empty_equipment_validation() {
        let mut bridge = RuntimeMechanicsBridge::new();
        let context = (&mut bridge as *mut RuntimeMechanicsBridge).cast::<c_void>();
        let mut catalog = NativeMechanicsCatalogHandle::default();
        assert_eq!(
            unsafe {
                create_catalog(
                    context,
                    &NativeMechanicsCatalogCreateRequest {
                        version: utf8("containment"),
                    },
                    &mut catalog,
                )
            },
            ABI_OK
        );
        let classifications = [NativeMechanicsText {
            value: utf8("weapon"),
        }];
        assert_eq!(
            unsafe {
                define_item(
                    context,
                    &NativeMechanicsItemDefinitionRequest {
                        catalog,
                        id: utf8("sword"),
                        kind: NativeMechanicsItemKind::Unique,
                        maximum_quantity: 1,
                        classifications: classifications.as_ptr(),
                        classifications_len: classifications.len(),
                        capacity_costs: std::ptr::null(),
                        capacity_costs_len: 0,
                        has_equipment: true,
                        required_slots: 1,
                        exclusive_group: utf8(""),
                        sources: std::ptr::null(),
                        sources_len: 0,
                    },
                )
            },
            ABI_OK
        );
        assert_eq!(
            unsafe {
                define_equipment_slot(
                    context,
                    &NativeMechanicsEquipmentSlotDefinitionRequest {
                        catalog,
                        id: utf8("hand"),
                        allowed_classifications: classifications.as_ptr(),
                        allowed_classifications_len: classifications.len(),
                    },
                )
            },
            ABI_OK
        );
        assert_eq!(unsafe { admit_catalog(context, catalog) }, ABI_OK);

        let mut item = NativeMechanicsEntityHandle::default();
        assert_eq!(
            unsafe {
                bind_entity(
                    context,
                    &NativeMechanicsEntityBindRequest {
                        catalog,
                        entity_id: 2,
                        identity: utf8("sword-instance"),
                    },
                    &mut item,
                )
            },
            ABI_OK
        );
        assert_eq!(
            unsafe {
                set_initial_components(
                    context,
                    &NativeMechanicsInitialComponentsRequest {
                        entity: item,
                        has_stats: false,
                        stats: std::ptr::null(),
                        stats_len: 0,
                        has_tracks: false,
                        tracks: std::ptr::null(),
                        tracks_len: 0,
                        has_intrinsic_sources: false,
                        intrinsic_sources: std::ptr::null(),
                        intrinsic_sources_len: 0,
                        has_active_effects: false,
                        active_effects: std::ptr::null(),
                        active_effects_len: 0,
                        has_inventory: false,
                        inventory_stacks: std::ptr::null(),
                        inventory_stacks_len: 0,
                        inventory_capacity_limits: std::ptr::null(),
                        inventory_capacity_limits_len: 0,
                        has_item: true,
                        item_definition: utf8("sword"),
                        has_equipment: false,
                        equipment_assignments: std::ptr::null(),
                        equipment_assignments_len: 0,
                    },
                )
            },
            ABI_OK
        );
        let mut item_receipt = NativeMechanicsEntityReceipt::default();
        assert_eq!(
            unsafe { commit_entity(context, item, &mut item_receipt) },
            ABI_OK
        );
        let mut before = NativeMechanicsContainmentReceipt::default();
        assert_eq!(
            unsafe {
                read_containment(
                    context,
                    &NativeMechanicsContainmentReadRequest { entity: item },
                    &mut before,
                )
            },
            ABI_OK
        );
        assert!(!before.present);

        let mut owner = NativeMechanicsEntityHandle::default();
        assert_eq!(
            unsafe {
                bind_entity(
                    context,
                    &NativeMechanicsEntityBindRequest {
                        catalog,
                        entity_id: 1,
                        identity: utf8("owner"),
                    },
                    &mut owner,
                )
            },
            ABI_OK
        );
        let assignments = [NativeMechanicsInitialEquipmentAssignment {
            slot: utf8("hand"),
            item_entity_id: 2,
        }];
        assert_eq!(
            unsafe {
                set_initial_components(
                    context,
                    &NativeMechanicsInitialComponentsRequest {
                        entity: owner,
                        has_stats: false,
                        stats: std::ptr::null(),
                        stats_len: 0,
                        has_tracks: false,
                        tracks: std::ptr::null(),
                        tracks_len: 0,
                        has_intrinsic_sources: false,
                        intrinsic_sources: std::ptr::null(),
                        intrinsic_sources_len: 0,
                        has_active_effects: false,
                        active_effects: std::ptr::null(),
                        active_effects_len: 0,
                        has_inventory: false,
                        inventory_stacks: std::ptr::null(),
                        inventory_stacks_len: 0,
                        inventory_capacity_limits: std::ptr::null(),
                        inventory_capacity_limits_len: 0,
                        has_item: false,
                        item_definition: utf8(""),
                        has_equipment: true,
                        equipment_assignments: assignments.as_ptr(),
                        equipment_assignments_len: assignments.len(),
                    },
                )
            },
            ABI_OK
        );
        assert_eq!(
            unsafe {
                stage_initial_containment(
                    context,
                    &NativeMechanicsInitialContainmentRequest {
                        owner,
                        child_entity_id: 2,
                        expected_state_revision: before.state_revision,
                    },
                )
            },
            ABI_OK
        );
        let mut owner_receipt = NativeMechanicsEntityReceipt::default();
        assert_eq!(
            unsafe { commit_entity(context, owner, &mut owner_receipt) },
            ABI_OK
        );
        assert_eq!(owner_receipt.state_revision_before, before.state_revision);
        assert!(owner_receipt.state_revision_after > owner_receipt.state_revision_before);
        let mut after = NativeMechanicsContainmentReceipt::default();
        assert_eq!(
            unsafe {
                read_containment(
                    context,
                    &NativeMechanicsContainmentReadRequest { entity: item },
                    &mut after,
                )
            },
            ABI_OK
        );
        assert!(after.present);
        assert_eq!(after.container_entity_id, 1);
        assert_eq!(after.state_revision, owner_receipt.state_revision_after);
    }

    #[test]
    fn inventory_view_uses_canonical_binding_and_copies_all_exact_collections() {
        let mut bridge = RuntimeMechanicsBridge::new();
        let context = (&mut bridge as *mut RuntimeMechanicsBridge).cast::<c_void>();
        let mut catalog = NativeMechanicsCatalogHandle::default();
        assert_eq!(
            unsafe {
                create_catalog(
                    context,
                    &NativeMechanicsCatalogCreateRequest {
                        version: utf8("inventory-view"),
                    },
                    &mut catalog,
                )
            },
            ABI_OK
        );
        assert_eq!(
            unsafe {
                define_capacity_metric(
                    context,
                    &NativeMechanicsCapacityMetricDefinitionRequest {
                        catalog,
                        id: utf8("weight"),
                    },
                )
            },
            ABI_OK
        );
        assert_eq!(
            unsafe {
                define_capacity_metric(
                    context,
                    &NativeMechanicsCapacityMetricDefinitionRequest {
                        catalog,
                        id: utf8("volume"),
                    },
                )
            },
            ABI_OK
        );
        let rations_capacity = [
            NativeMechanicsItemCapacityCostInput {
                metric: utf8("weight"),
                units: 2,
            },
            NativeMechanicsItemCapacityCostInput {
                metric: utf8("volume"),
                units: 1,
            },
        ];
        assert_eq!(
            unsafe {
                define_item(
                    context,
                    &NativeMechanicsItemDefinitionRequest {
                        catalog,
                        id: utf8("rations"),
                        kind: NativeMechanicsItemKind::Fungible,
                        maximum_quantity: 99,
                        classifications: std::ptr::null(),
                        classifications_len: 0,
                        capacity_costs: rations_capacity.as_ptr(),
                        capacity_costs_len: rations_capacity.len(),
                        has_equipment: false,
                        required_slots: 0,
                        exclusive_group: utf8(""),
                        sources: std::ptr::null(),
                        sources_len: 0,
                    },
                )
            },
            ABI_OK
        );
        let sword_capacity = [NativeMechanicsItemCapacityCostInput {
            metric: utf8("weight"),
            units: 5,
        }];
        assert_eq!(
            unsafe {
                define_item(
                    context,
                    &NativeMechanicsItemDefinitionRequest {
                        catalog,
                        id: utf8("sword"),
                        kind: NativeMechanicsItemKind::Unique,
                        maximum_quantity: 1,
                        classifications: std::ptr::null(),
                        classifications_len: 0,
                        capacity_costs: sword_capacity.as_ptr(),
                        capacity_costs_len: sword_capacity.len(),
                        has_equipment: false,
                        required_slots: 0,
                        exclusive_group: utf8(""),
                        sources: std::ptr::null(),
                        sources_len: 0,
                    },
                )
            },
            ABI_OK
        );
        assert_eq!(unsafe { admit_catalog(context, catalog) }, ABI_OK);

        let mut sword = NativeMechanicsEntityHandle::default();
        assert_eq!(
            unsafe {
                bind_entity(
                    context,
                    &NativeMechanicsEntityBindRequest {
                        catalog,
                        entity_id: 2,
                        identity: utf8("sword-instance"),
                    },
                    &mut sword,
                )
            },
            ABI_OK
        );
        assert_eq!(
            unsafe {
                set_initial_components(
                    context,
                    &NativeMechanicsInitialComponentsRequest {
                        has_item: true,
                        item_definition: utf8("sword"),
                        ..empty_initial_components(sword)
                    },
                )
            },
            ABI_OK
        );
        let mut sword_receipt = NativeMechanicsEntityReceipt::default();
        assert_eq!(
            unsafe { commit_entity(context, sword, &mut sword_receipt) },
            ABI_OK
        );
        let mut sword_containment = NativeMechanicsContainmentReceipt::default();
        assert_eq!(
            unsafe {
                read_containment(
                    context,
                    &NativeMechanicsContainmentReadRequest { entity: sword },
                    &mut sword_containment,
                )
            },
            ABI_OK
        );

        let mut owner = NativeMechanicsEntityHandle::default();
        assert_eq!(
            unsafe {
                bind_entity(
                    context,
                    &NativeMechanicsEntityBindRequest {
                        catalog,
                        entity_id: 1,
                        identity: utf8("owner"),
                    },
                    &mut owner,
                )
            },
            ABI_OK
        );
        let stacks = [NativeMechanicsInitialInventoryStack {
            definition: utf8("rations"),
            quantity: 3,
        }];
        let capacity_limits = [NativeMechanicsInitialInventoryCapacityLimit {
            metric: utf8("weight"),
            maximum: 20,
        }];
        assert_eq!(
            unsafe {
                set_initial_components(
                    context,
                    &NativeMechanicsInitialComponentsRequest {
                        has_inventory: true,
                        inventory_stacks: stacks.as_ptr(),
                        inventory_stacks_len: stacks.len(),
                        inventory_capacity_limits: capacity_limits.as_ptr(),
                        inventory_capacity_limits_len: capacity_limits.len(),
                        ..empty_initial_components(owner)
                    },
                )
            },
            ABI_OK
        );
        assert_eq!(
            unsafe {
                stage_initial_containment(
                    context,
                    &NativeMechanicsInitialContainmentRequest {
                        owner,
                        child_entity_id: 2,
                        expected_state_revision: sword_containment.state_revision,
                    },
                )
            },
            ABI_OK
        );
        let mut owner_receipt = NativeMechanicsEntityReceipt::default();
        assert_eq!(
            unsafe { commit_entity(context, owner, &mut owner_receipt) },
            ABI_OK
        );

        let mut view = NativeMechanicsInventoryViewLease::default();
        assert_eq!(
            unsafe { read_inventory_view(context, owner, &mut view) },
            ABI_OK
        );
        assert_eq!(view.catalog_id, catalog.value);
        assert_eq!(view.owner_entity_id, 1);
        assert_eq!(
            view.inventory_revision.entity_id,
            owner_receipt.inventory_revision.entity_id
        );
        assert_eq!(
            view.inventory_revision.revision,
            owner_receipt.inventory_revision.revision
        );
        assert_eq!(
            view.inventory_revision.component,
            NativeMechanicsRevisionComponent::Inventory
        );
        assert!(view.inventory_revision.present);
        assert_eq!(
            view.relationship_state_revision,
            owner_receipt.state_revision_after
        );
        assert_eq!(view.stacks_len, 1);
        assert_eq!(view.unique_items_len, 1);
        assert_eq!(view.capacity_len, 2);
        let stack_definition = unsafe {
            std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                (*view.stacks).definition.bytes,
                (*view.stacks).definition.len,
            ))
        };
        assert_eq!(stack_definition, "rations");
        assert_eq!(unsafe { (*view.stacks).quantity }, 3);
        assert_eq!(unsafe { (*view.unique_items).entity_id }, 2);
        let unique_definition = unsafe {
            std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                (*view.unique_items).definition.bytes,
                (*view.unique_items).definition.len,
            ))
        };
        assert_eq!(unique_definition, "sword");
        let capacity = unsafe { std::slice::from_raw_parts(view.capacity, view.capacity_len) };
        let volume_metric = unsafe {
            std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                capacity[0].metric.bytes,
                capacity[0].metric.len,
            ))
        };
        assert_eq!(volume_metric, "volume");
        assert_eq!(capacity[0].used, 3);
        assert!(!capacity[0].has_maximum);
        assert_eq!(capacity[0].maximum, 0);
        let weight_metric = unsafe {
            std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                capacity[1].metric.bytes,
                capacity[1].metric.len,
            ))
        };
        assert_eq!(weight_metric, "weight");
        assert_eq!(capacity[1].used, 11);
        assert!(capacity[1].has_maximum);
        assert_eq!(capacity[1].maximum, 20);
        assert_eq!(view.read_cost.stack_entries_visited, 1);
        assert_eq!(view.read_cost.containment_entries_visited, 1);
        assert_eq!(view.read_cost.item_components_read, 1);
        assert_eq!(view.read_cost.capacity_limits_visited, 1);
        assert_eq!(view.read_cost.capacity_costs_visited, 3);
        assert_eq!(
            unsafe { destroy_operation_lease(context, view.handle) },
            ABI_OK
        );
        assert_eq!(unsafe { destroy_operation_lease(context, view.handle) }, 0);

        let mut grant = NativeMechanicsInventoryMutationLease::default();
        assert_eq!(
            unsafe {
                grant_inventory(
                    context,
                    &NativeMechanicsInventoryMutationRequest {
                        owner,
                        operation: utf8("grant-rations"),
                        source_kind: NativeMechanicsActiveEffectProvenanceKind::Request,
                        source_intrinsic_entity_id: 0,
                        source_intrinsic_instance: utf8(""),
                        source_effect_entity_id: 0,
                        source_effect_instance: utf8(""),
                        source_effect_stack: 0,
                        source_effect_source: utf8(""),
                        source_equipped_owner_entity_id: 0,
                        source_equipped_item_entity_id: 0,
                        source_equipped_source: utf8(""),
                        source_request_operation: utf8("inventory-fixture"),
                        source_request_instance: utf8("fixture"),
                        item: utf8("rations"),
                        quantity: 2,
                        revision_guard: NativeMechanicsRevisionGuard::Exact,
                        expected_revision: owner_receipt.inventory_revision,
                    },
                    &mut grant,
                )
            },
            ABI_OK
        );
        assert_eq!(grant.kind, NativeMechanicsInventoryMutationKind::Grant);
        assert_eq!((grant.before_quantity, grant.after_quantity), (3, 5));
        assert_eq!(
            (grant.capacity_before_len, grant.capacity_after_len),
            (2, 2)
        );

        let mut consume = NativeMechanicsInventoryMutationLease::default();
        assert_eq!(
            unsafe {
                consume_inventory(
                    context,
                    &NativeMechanicsInventoryMutationRequest {
                        owner,
                        operation: utf8("consume-rations"),
                        source_kind: NativeMechanicsActiveEffectProvenanceKind::Request,
                        source_intrinsic_entity_id: 0,
                        source_intrinsic_instance: utf8(""),
                        source_effect_entity_id: 0,
                        source_effect_instance: utf8(""),
                        source_effect_stack: 0,
                        source_effect_source: utf8(""),
                        source_equipped_owner_entity_id: 0,
                        source_equipped_item_entity_id: 0,
                        source_equipped_source: utf8(""),
                        source_request_operation: utf8("inventory-fixture"),
                        source_request_instance: utf8("fixture"),
                        item: utf8("rations"),
                        quantity: 1,
                        revision_guard: NativeMechanicsRevisionGuard::Exact,
                        expected_revision: grant.committed_inventory_revision,
                    },
                    &mut consume,
                )
            },
            ABI_OK
        );
        assert_eq!(consume.kind, NativeMechanicsInventoryMutationKind::Consume);
        assert_eq!((consume.before_quantity, consume.after_quantity), (5, 4));
        assert_eq!(
            unsafe { destroy_operation_lease(context, grant.handle) },
            ABI_OK
        );
        assert_eq!(
            unsafe { destroy_operation_lease(context, consume.handle) },
            ABI_OK
        );

        let mut rejected = NativeMechanicsInventoryMutationLease::default();
        assert_eq!(
            unsafe {
                consume_inventory(
                    context,
                    &NativeMechanicsInventoryMutationRequest {
                        owner,
                        operation: utf8("reject-rations"),
                        source_kind: NativeMechanicsActiveEffectProvenanceKind::Request,
                        source_intrinsic_entity_id: 0,
                        source_intrinsic_instance: utf8(""),
                        source_effect_entity_id: 0,
                        source_effect_instance: utf8(""),
                        source_effect_stack: 0,
                        source_effect_source: utf8(""),
                        source_equipped_owner_entity_id: 0,
                        source_equipped_item_entity_id: 0,
                        source_equipped_source: utf8(""),
                        source_request_operation: utf8("inventory-fixture"),
                        source_request_instance: utf8("fixture"),
                        item: utf8("rations"),
                        quantity: 99,
                        revision_guard: NativeMechanicsRevisionGuard::Exact,
                        expected_revision: consume.committed_inventory_revision,
                    },
                    &mut rejected,
                )
            },
            0
        );
        let mut preserved = NativeMechanicsInventoryViewLease::default();
        assert_eq!(
            unsafe { read_inventory_view(context, owner, &mut preserved) },
            ABI_OK
        );
        assert_eq!(unsafe { (*preserved.stacks).quantity }, 4);
        assert_eq!(
            preserved.inventory_revision.revision,
            consume.committed_inventory_revision.revision
        );
        assert_eq!(
            unsafe { destroy_operation_lease(context, preserved.handle) },
            ABI_OK
        );

        let mut recipient = NativeMechanicsEntityHandle::default();
        assert_eq!(
            unsafe {
                bind_entity(
                    context,
                    &NativeMechanicsEntityBindRequest {
                        catalog,
                        entity_id: 3,
                        identity: utf8("recipient"),
                    },
                    &mut recipient,
                )
            },
            ABI_OK
        );
        assert_eq!(
            unsafe {
                set_initial_components(
                    context,
                    &NativeMechanicsInitialComponentsRequest {
                        has_inventory: true,
                        inventory_stacks: std::ptr::null(),
                        inventory_stacks_len: 0,
                        inventory_capacity_limits: capacity_limits.as_ptr(),
                        inventory_capacity_limits_len: capacity_limits.len(),
                        ..empty_initial_components(recipient)
                    },
                )
            },
            ABI_OK
        );
        let mut recipient_receipt = NativeMechanicsEntityReceipt::default();
        assert_eq!(
            unsafe { commit_entity(context, recipient, &mut recipient_receipt) },
            ABI_OK
        );
        let mut transfer = NativeMechanicsInventoryTransferLease::default();
        assert_eq!(
            unsafe {
                transfer_inventory(
                    context,
                    &NativeMechanicsInventoryTransferRequest {
                        from_owner: owner,
                        to_owner: recipient,
                        operation: utf8("transfer-rations"),
                        source_kind: NativeMechanicsActiveEffectProvenanceKind::Request,
                        source_intrinsic_entity_id: 0,
                        source_intrinsic_instance: utf8(""),
                        source_effect_entity_id: 0,
                        source_effect_instance: utf8(""),
                        source_effect_stack: 0,
                        source_effect_source: utf8(""),
                        source_equipped_owner_entity_id: 0,
                        source_equipped_item_entity_id: 0,
                        source_equipped_source: utf8(""),
                        source_request_operation: utf8("inventory-fixture"),
                        source_request_instance: utf8("fixture"),
                        item: utf8("rations"),
                        quantity: 2,
                        from_revision_guard: NativeMechanicsRevisionGuard::Exact,
                        expected_from_revision: consume.committed_inventory_revision,
                        to_revision_guard: NativeMechanicsRevisionGuard::Exact,
                        expected_to_revision: recipient_receipt.inventory_revision,
                    },
                    &mut transfer,
                )
            },
            ABI_OK
        );
        assert_eq!(
            (
                transfer.from_before_quantity,
                transfer.from_after_quantity,
                transfer.to_before_quantity,
                transfer.to_after_quantity
            ),
            (4, 2, 0, 2)
        );
        assert_eq!(
            (
                transfer.from_capacity_before_len,
                transfer.from_capacity_after_len,
                transfer.to_capacity_before_len,
                transfer.to_capacity_after_len
            ),
            (2, 2, 1, 2)
        );
        let unique_transfer_relationship_revision = bridge
            .catalog_slot_mut(catalog)
            .expect("admitted catalog remains available")
            .world
            .state
            .revision();
        let mut unique_transfer = NativeMechanicsUniqueItemTransferLease::default();
        assert_eq!(
            unsafe {
                transfer_unique_item(
                    context,
                    &NativeMechanicsUniqueItemTransferRequest {
                        item: sword,
                        from_owner: owner,
                        to_owner: recipient,
                        operation: utf8("transfer-sword"),
                        source_kind: NativeMechanicsActiveEffectProvenanceKind::Request,
                        source_intrinsic_entity_id: 0,
                        source_intrinsic_instance: utf8(""),
                        source_effect_entity_id: 0,
                        source_effect_instance: utf8(""),
                        source_effect_stack: 0,
                        source_effect_source: utf8(""),
                        source_equipped_owner_entity_id: 0,
                        source_equipped_item_entity_id: 0,
                        source_equipped_source: utf8(""),
                        source_request_operation: utf8("inventory-fixture"),
                        source_request_instance: utf8("fixture"),
                        expected_relationship_revision: unique_transfer_relationship_revision,
                        from_revision_guard: NativeMechanicsRevisionGuard::Exact,
                        expected_from_revision: transfer.committed_from_inventory_revision,
                        to_revision_guard: NativeMechanicsRevisionGuard::Exact,
                        expected_to_revision: transfer.committed_to_inventory_revision,
                    },
                    &mut unique_transfer,
                )
            },
            ABI_OK
        );
        assert_eq!(
            (
                unique_transfer.item_entity_id,
                unique_transfer.from_owner_entity_id,
                unique_transfer.to_owner_entity_id,
            ),
            (2, 1, 3)
        );
        assert_eq!(unique_transfer.catalog_id, catalog.value);
        assert_eq!(
            (
                unique_transfer.observed_from_inventory_revision.entity_id,
                unique_transfer.observed_from_inventory_revision.revision,
                unique_transfer.observed_from_inventory_revision.component as u32,
                unique_transfer.observed_from_inventory_revision.present,
            ),
            (
                transfer.committed_from_inventory_revision.entity_id,
                transfer.committed_from_inventory_revision.revision,
                transfer.committed_from_inventory_revision.component as u32,
                transfer.committed_from_inventory_revision.present,
            )
        );
        assert_eq!(
            (
                unique_transfer.observed_to_inventory_revision.entity_id,
                unique_transfer.observed_to_inventory_revision.revision,
                unique_transfer.observed_to_inventory_revision.component as u32,
                unique_transfer.observed_to_inventory_revision.present,
            ),
            (
                transfer.committed_to_inventory_revision.entity_id,
                transfer.committed_to_inventory_revision.revision,
                transfer.committed_to_inventory_revision.component as u32,
                transfer.committed_to_inventory_revision.present,
            )
        );
        assert_eq!(
            (
                unique_transfer.relationship_revision_before,
                unique_transfer.relationship_revision_after,
            ),
            (
                unique_transfer_relationship_revision,
                unique_transfer_relationship_revision + 1,
            )
        );
        assert_eq!(
            (
                unique_transfer.from_capacity_before_len,
                unique_transfer.from_capacity_after_len,
                unique_transfer.to_capacity_before_len,
                unique_transfer.to_capacity_after_len,
            ),
            (2, 2, 2, 2)
        );
        assert_ne!(
            unique_transfer.from_capacity_before,
            unique_transfer.from_capacity_after
        );
        assert_ne!(
            unique_transfer.from_capacity_before,
            unique_transfer.to_capacity_before
        );
        assert_ne!(
            unique_transfer.to_capacity_before,
            unique_transfer.to_capacity_after
        );
        assert_eq!(
            unsafe { destroy_operation_lease(context, unique_transfer.handle) },
            ABI_OK
        );
        assert_eq!(
            unsafe { destroy_operation_lease(context, unique_transfer.handle) },
            0
        );

        let mut stale_unique_transfer = NativeMechanicsUniqueItemTransferLease::default();
        assert_eq!(
            unsafe {
                transfer_unique_item(
                    context,
                    &NativeMechanicsUniqueItemTransferRequest {
                        item: sword,
                        from_owner: owner,
                        to_owner: recipient,
                        operation: utf8("reject-stale-sword-transfer"),
                        source_kind: NativeMechanicsActiveEffectProvenanceKind::Request,
                        source_intrinsic_entity_id: 0,
                        source_intrinsic_instance: utf8(""),
                        source_effect_entity_id: 0,
                        source_effect_instance: utf8(""),
                        source_effect_stack: 0,
                        source_effect_source: utf8(""),
                        source_equipped_owner_entity_id: 0,
                        source_equipped_item_entity_id: 0,
                        source_equipped_source: utf8(""),
                        source_request_operation: utf8("inventory-fixture"),
                        source_request_instance: utf8("fixture"),
                        expected_relationship_revision: unique_transfer.relationship_revision_after,
                        from_revision_guard: NativeMechanicsRevisionGuard::Exact,
                        expected_from_revision: transfer.committed_from_inventory_revision,
                        to_revision_guard: NativeMechanicsRevisionGuard::Exact,
                        expected_to_revision: transfer.committed_to_inventory_revision,
                    },
                    &mut stale_unique_transfer,
                )
            },
            0
        );
        let mut sword_after_transfer = NativeMechanicsContainmentReceipt::default();
        assert_eq!(
            unsafe {
                read_containment(
                    context,
                    &NativeMechanicsContainmentReadRequest { entity: sword },
                    &mut sword_after_transfer,
                )
            },
            ABI_OK
        );
        assert!(sword_after_transfer.present);
        assert_eq!(sword_after_transfer.container_entity_id, 3);
        assert_eq!(
            unsafe { destroy_operation_lease(context, transfer.handle) },
            ABI_OK
        );
    }

    #[test]
    fn equipment_callbacks_delegate_atomically_and_retain_typed_receipt_rows() {
        let mut bridge = RuntimeMechanicsBridge::new();
        let context = (&mut bridge as *mut RuntimeMechanicsBridge).cast::<c_void>();
        let mut catalog = NativeMechanicsCatalogHandle::default();
        assert_eq!(
            unsafe {
                create_catalog(
                    context,
                    &NativeMechanicsCatalogCreateRequest {
                        version: utf8("equipment-callbacks"),
                    },
                    &mut catalog,
                )
            },
            ABI_OK
        );
        let weapon = [NativeMechanicsText {
            value: utf8("weapon"),
        }];
        for item in ["pistol", "rifle"] {
            assert_eq!(
                unsafe {
                    define_item(
                        context,
                        &NativeMechanicsItemDefinitionRequest {
                            catalog,
                            id: utf8(item),
                            kind: NativeMechanicsItemKind::Unique,
                            maximum_quantity: 1,
                            classifications: weapon.as_ptr(),
                            classifications_len: weapon.len(),
                            capacity_costs: std::ptr::null(),
                            capacity_costs_len: 0,
                            has_equipment: true,
                            required_slots: 1,
                            exclusive_group: utf8(""),
                            sources: std::ptr::null(),
                            sources_len: 0,
                        },
                    )
                },
                ABI_OK
            );
        }
        for slot in ["hand_left", "hand_right"] {
            assert_eq!(
                unsafe {
                    define_equipment_slot(
                        context,
                        &NativeMechanicsEquipmentSlotDefinitionRequest {
                            catalog,
                            id: utf8(slot),
                            allowed_classifications: std::ptr::null(),
                            allowed_classifications_len: 0,
                        },
                    )
                },
                ABI_OK
            );
        }
        assert_eq!(unsafe { admit_catalog(context, catalog) }, ABI_OK);

        let bind = |entity_id, identity| {
            let mut entity = NativeMechanicsEntityHandle::default();
            assert_eq!(
                unsafe {
                    bind_entity(
                        context,
                        &NativeMechanicsEntityBindRequest {
                            catalog,
                            entity_id,
                            identity: utf8(identity),
                        },
                        &mut entity,
                    )
                },
                ABI_OK
            );
            entity
        };
        let pistol = bind(2, "pistol-instance");
        let rifle = bind(3, "rifle-instance");
        for (entity, definition) in [(pistol, "pistol"), (rifle, "rifle")] {
            assert_eq!(
                unsafe {
                    set_initial_components(
                        context,
                        &NativeMechanicsInitialComponentsRequest {
                            has_item: true,
                            item_definition: utf8(definition),
                            ..empty_initial_components(entity)
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
        }
        let owner = bind(1, "owner");
        assert_eq!(
            unsafe {
                set_initial_components(
                    context,
                    &NativeMechanicsInitialComponentsRequest {
                        has_equipment: true,
                        equipment_assignments: std::ptr::null(),
                        equipment_assignments_len: 0,
                        ..empty_initial_components(owner)
                    },
                )
            },
            ABI_OK
        );
        let containment_state_revision = bridge
            .catalog_slot_mut(catalog)
            .expect("admitted catalog remains available")
            .world
            .state
            .revision();
        for child in [2, 3] {
            assert_eq!(
                unsafe {
                    stage_initial_containment(
                        context,
                        &NativeMechanicsInitialContainmentRequest {
                            owner,
                            child_entity_id: child,
                            expected_state_revision: containment_state_revision,
                        },
                    )
                },
                ABI_OK
            );
        }
        let mut owner_receipt = NativeMechanicsEntityReceipt::default();
        assert_eq!(
            unsafe { commit_entity(context, owner, &mut owner_receipt) },
            ABI_OK
        );

        let left = [NativeMechanicsText {
            value: utf8("hand_left"),
        }];
        let mut equipped = NativeMechanicsEquipmentMutationLease::default();
        assert_eq!(
            unsafe {
                equip_equipment(
                    context,
                    &NativeMechanicsEquipmentEquipRequest {
                        owner,
                        item: pistol,
                        operation: utf8("equip-pistol"),
                        source_kind: NativeMechanicsActiveEffectProvenanceKind::Request,
                        source_intrinsic_entity_id: 0,
                        source_intrinsic_instance: utf8(""),
                        source_effect_entity_id: 0,
                        source_effect_instance: utf8(""),
                        source_effect_stack: 0,
                        source_effect_source: utf8(""),
                        source_equipped_owner_entity_id: 0,
                        source_equipped_item_entity_id: 0,
                        source_equipped_source: utf8(""),
                        source_request_operation: utf8("equipment-fixture"),
                        source_request_instance: utf8("fixture"),
                        slots: left.as_ptr(),
                        slots_len: left.len(),
                        equipment_revision_guard: NativeMechanicsRevisionGuard::Exact,
                        expected_equipment_revision: owner_receipt.equipment_revision,
                        expected_state_revision: owner_receipt.state_revision_after,
                    },
                    &mut equipped,
                )
            },
            ABI_OK
        );
        assert_eq!(equipped.kind, NativeMechanicsEquipmentMutationKind::Equip);
        assert_eq!(equipped.changes_len, 1);
        assert_eq!(equipped.observed_item_revisions_len, 1);
        assert_eq!(equipped.owner_entity_id, 1);
        assert_eq!(equipped.item_entity_id, 2);
        assert!(equipped.observed_equipment_revision.present);

        let right = [NativeMechanicsText {
            value: utf8("hand_right"),
        }];
        let mut swapped = NativeMechanicsEquipmentMutationLease::default();
        assert_eq!(
            unsafe {
                swap_equipment(
                    context,
                    &NativeMechanicsEquipmentSwapRequest {
                        owner,
                        outgoing_item: pistol,
                        incoming_item: rifle,
                        operation: utf8("swap-pistol-rifle"),
                        source_kind: NativeMechanicsActiveEffectProvenanceKind::Request,
                        source_intrinsic_entity_id: 0,
                        source_intrinsic_instance: utf8(""),
                        source_effect_entity_id: 0,
                        source_effect_instance: utf8(""),
                        source_effect_stack: 0,
                        source_effect_source: utf8(""),
                        source_equipped_owner_entity_id: 0,
                        source_equipped_item_entity_id: 0,
                        source_equipped_source: utf8(""),
                        source_request_operation: utf8("equipment-fixture"),
                        source_request_instance: utf8("fixture"),
                        incoming_slots: right.as_ptr(),
                        incoming_slots_len: right.len(),
                        equipment_revision_guard: NativeMechanicsRevisionGuard::Exact,
                        expected_equipment_revision: equipped.committed_equipment_revision,
                        expected_state_revision: equipped.committed_state_revision,
                    },
                    &mut swapped,
                )
            },
            ABI_OK
        );
        assert_eq!(swapped.kind, NativeMechanicsEquipmentMutationKind::Swap);
        assert!(swapped.has_replaced_item);
        assert_eq!(swapped.replaced_item_entity_id, 2);
        assert_eq!(swapped.changes_len, 2);

        let mut unequipped = NativeMechanicsEquipmentMutationLease::default();
        assert_eq!(
            unsafe {
                unequip_equipment(
                    context,
                    &NativeMechanicsEquipmentUnequipRequest {
                        owner,
                        item: rifle,
                        operation: utf8("unequip-rifle"),
                        source_kind: NativeMechanicsActiveEffectProvenanceKind::Request,
                        source_intrinsic_entity_id: 0,
                        source_intrinsic_instance: utf8(""),
                        source_effect_entity_id: 0,
                        source_effect_instance: utf8(""),
                        source_effect_stack: 0,
                        source_effect_source: utf8(""),
                        source_equipped_owner_entity_id: 0,
                        source_equipped_item_entity_id: 0,
                        source_equipped_source: utf8(""),
                        source_request_operation: utf8("equipment-fixture"),
                        source_request_instance: utf8("fixture"),
                        equipment_revision_guard: NativeMechanicsRevisionGuard::Exact,
                        expected_equipment_revision: swapped.committed_equipment_revision,
                        expected_state_revision: swapped.committed_state_revision,
                    },
                    &mut unequipped,
                )
            },
            ABI_OK
        );
        assert_eq!(
            unequipped.kind,
            NativeMechanicsEquipmentMutationKind::Unequip
        );
        assert_eq!(unequipped.changes_len, 1);

        let mut rejected = NativeMechanicsEquipmentMutationLease::default();
        assert_eq!(
            unsafe {
                equip_equipment(
                    context,
                    &NativeMechanicsEquipmentEquipRequest {
                        owner,
                        item: pistol,
                        operation: utf8("stale-equip-pistol"),
                        source_kind: NativeMechanicsActiveEffectProvenanceKind::Request,
                        source_intrinsic_entity_id: 0,
                        source_intrinsic_instance: utf8(""),
                        source_effect_entity_id: 0,
                        source_effect_instance: utf8(""),
                        source_effect_stack: 0,
                        source_effect_source: utf8(""),
                        source_equipped_owner_entity_id: 0,
                        source_equipped_item_entity_id: 0,
                        source_equipped_source: utf8(""),
                        source_request_operation: utf8("equipment-fixture"),
                        source_request_instance: utf8("fixture"),
                        slots: left.as_ptr(),
                        slots_len: left.len(),
                        equipment_revision_guard: NativeMechanicsRevisionGuard::Exact,
                        expected_equipment_revision: unequipped.committed_equipment_revision,
                        expected_state_revision: swapped.committed_state_revision,
                    },
                    &mut rejected,
                )
            },
            0
        );
        let mut equipment = NativeMechanicsEquipmentAssignmentComponentLease {
            handle: NativeMechanicsComponentLeaseHandle::default(),
            entries: std::ptr::null(),
            entries_len: 0,
            metadata: NativeMechanicsComponentReadMetadata {
                entity_id: 0,
                component: NativeMechanicsRevisionComponent::Equipment,
                revision: 0,
                present: false,
                catalog_id: 0,
                catalog_version: utf8(""),
                catalog_fingerprint: utf8(""),
            },
        };
        assert_eq!(
            unsafe { read_equipment_assignment_component(context, owner, &mut equipment) },
            ABI_OK
        );
        assert_eq!(equipment.entries_len, 0);
        assert_eq!(
            equipment.metadata.revision,
            unequipped.committed_equipment_revision.revision
        );
        assert_eq!(
            unsafe { destroy_component_lease(context, equipment.handle) },
            ABI_OK
        );
        for receipt in [equipped, swapped, unequipped] {
            assert_eq!(
                unsafe { destroy_operation_lease(context, receipt.handle) },
                ABI_OK
            );
            assert_eq!(
                unsafe { destroy_operation_lease(context, receipt.handle) },
                0
            );
        }
    }

    #[test]
    fn unique_item_lifecycle_stages_owner_mutation_and_terminal_binding_lifecycle() {
        let mut bridge = RuntimeMechanicsBridge::new();
        let context = (&mut bridge as *mut RuntimeMechanicsBridge).cast::<c_void>();
        let mut catalog = NativeMechanicsCatalogHandle::default();
        assert_eq!(
            unsafe {
                create_catalog(
                    context,
                    &NativeMechanicsCatalogCreateRequest {
                        version: utf8("unique-lifecycle"),
                    },
                    &mut catalog,
                )
            },
            ABI_OK
        );
        assert_eq!(
            unsafe {
                define_item(
                    context,
                    &NativeMechanicsItemDefinitionRequest {
                        catalog,
                        id: utf8("blade"),
                        kind: NativeMechanicsItemKind::Unique,
                        maximum_quantity: 1,
                        classifications: std::ptr::null(),
                        classifications_len: 0,
                        capacity_costs: std::ptr::null(),
                        capacity_costs_len: 0,
                        has_equipment: false,
                        required_slots: 0,
                        exclusive_group: utf8(""),
                        sources: std::ptr::null(),
                        sources_len: 0,
                    },
                )
            },
            ABI_OK
        );
        assert_eq!(unsafe { admit_catalog(context, catalog) }, ABI_OK);

        let mut owner = NativeMechanicsEntityHandle::default();
        assert_eq!(
            unsafe {
                bind_entity(
                    context,
                    &NativeMechanicsEntityBindRequest {
                        catalog,
                        entity_id: 1,
                        identity: utf8("owner"),
                    },
                    &mut owner,
                )
            },
            ABI_OK
        );
        assert_eq!(
            unsafe {
                set_initial_components(
                    context,
                    &NativeMechanicsInitialComponentsRequest {
                        has_inventory: true,
                        inventory_stacks: std::ptr::null(),
                        inventory_stacks_len: 0,
                        inventory_capacity_limits: std::ptr::null(),
                        inventory_capacity_limits_len: 0,
                        ..empty_initial_components(owner)
                    },
                )
            },
            ABI_OK
        );
        let mut owner_receipt = NativeMechanicsEntityReceipt::default();
        assert_eq!(
            unsafe { commit_entity(context, owner, &mut owner_receipt) },
            ABI_OK
        );

        let mut item = NativeMechanicsEntityHandle::default();
        assert_eq!(
            unsafe {
                bind_entity(
                    context,
                    &NativeMechanicsEntityBindRequest {
                        catalog,
                        entity_id: 2,
                        identity: utf8("caller-item-name"),
                    },
                    &mut item,
                )
            },
            ABI_OK
        );
        let mut materialized = NativeMechanicsUniqueItemMaterializationLease::default();
        assert_eq!(
            unsafe {
                materialize_unique_item(
                    context,
                    &NativeMechanicsUniqueItemMaterializationRequest {
                        item,
                        container: owner,
                        definition: utf8("blade"),
                        expected_state_revision: owner_receipt.state_revision_after,
                    },
                    &mut materialized,
                )
            },
            ABI_OK
        );
        assert_eq!(materialized.item_entity_id, 2);
        assert_eq!(materialized.container_entity_id, 1);
        assert_eq!(
            materialized.lifecycle.lifecycle,
            NativeMechanicsEntityLifecycle::Active
        );
        let mut containment = NativeMechanicsContainmentReceipt::default();
        assert_eq!(
            unsafe {
                read_containment(
                    context,
                    &NativeMechanicsContainmentReadRequest { entity: item },
                    &mut containment,
                )
            },
            ABI_OK
        );
        assert!(containment.present);
        assert_eq!(containment.container_entity_id, 1);
        assert_eq!(
            containment.state_revision,
            materialized.committed_state_revision
        );

        let mut rejected = NativeMechanicsEntityHandle::default();
        assert_eq!(
            unsafe {
                bind_entity(
                    context,
                    &NativeMechanicsEntityBindRequest {
                        catalog,
                        entity_id: 3,
                        identity: utf8("rejected-item"),
                    },
                    &mut rejected,
                )
            },
            ABI_OK
        );
        let before_failure = bridge.catalogs[&catalog.value].world.state.revision();
        let mut failed = NativeMechanicsUniqueItemMaterializationLease::default();
        assert_eq!(
            unsafe {
                materialize_unique_item(
                    context,
                    &NativeMechanicsUniqueItemMaterializationRequest {
                        item: rejected,
                        container: owner,
                        definition: utf8("unknown-item"),
                        expected_state_revision: before_failure,
                    },
                    &mut failed,
                )
            },
            0
        );
        assert_eq!(
            bridge.catalogs[&catalog.value].world.state.revision(),
            before_failure
        );
        assert_eq!(
            bridge.catalogs[&catalog.value]
                .world
                .lifecycle_receipt(EntityId::new(3))
                .lifecycle,
            NativeMechanicsEntityLifecycle::Tombstoned
        );
        assert_eq!(unsafe { destroy_entity(context, rejected) }, ABI_OK);

        let mut destroyed = NativeMechanicsUniqueItemDestroyLease::default();
        assert_eq!(
            unsafe {
                destroy_unique_item(
                    context,
                    &NativeMechanicsUniqueItemDestroyRequest {
                        item,
                        operation: utf8("destroy-blade"),
                        source_kind: NativeMechanicsActiveEffectProvenanceKind::Request,
                        source_intrinsic_entity_id: 0,
                        source_intrinsic_instance: utf8(""),
                        source_effect_entity_id: 0,
                        source_effect_instance: utf8(""),
                        source_effect_stack: 0,
                        source_effect_source: utf8(""),
                        source_equipped_owner_entity_id: 0,
                        source_equipped_item_entity_id: 0,
                        source_equipped_source: utf8(""),
                        source_request_operation: utf8("fixture"),
                        source_request_instance: utf8("destroy"),
                        expected_state_revision: materialized.committed_state_revision,
                    },
                    &mut destroyed,
                )
            },
            ABI_OK
        );
        assert!(destroyed.has_former_owner);
        assert_eq!(destroyed.former_owner_entity_id, 1);
        assert_eq!(
            destroyed.lifecycle.lifecycle,
            NativeMechanicsEntityLifecycle::Tombstoned
        );
        assert_eq!(
            bridge.catalogs[&catalog.value]
                .world
                .lifecycle_receipt(EntityId::new(2))
                .stamp,
            destroyed.lifecycle.stamp
        );
        assert_eq!(unsafe { destroy_entity(context, item) }, ABI_OK);
        let mut rebound = NativeMechanicsEntityHandle::default();
        assert_eq!(
            unsafe {
                rebind_entity(
                    context,
                    &NativeMechanicsEntityRebindRequest {
                        catalog,
                        entity_id: 2,
                        guard: NativeMechanicsLifecycleGuard::Exact,
                        expected_stamp: destroyed.lifecycle.stamp,
                    },
                    &mut rebound,
                )
            },
            0
        );
        for lease in [materialized.handle, destroyed.handle] {
            assert_eq!(unsafe { destroy_operation_lease(context, lease) }, ABI_OK);
        }
    }
}
