use std::alloc::System;
use std::hint::black_box;

use core_ids::EntityId;
use entity_state::{
    encode_snapshot, ComponentRevision, EntityAuthoringService, EntityComponent, EntityDefinition,
    EntityState,
};
use gameplay_mechanics::{
    decode_snapshot_with_catalog, ActiveEffectsComponent, CatalogVersion, DamageKindDefinition,
    DamageKindId, DamageKindSelector, DamagePart, DamageRequest, DamageResponseDefinition,
    DamageService, DecisionOutcome, EffectApplyRequest, EffectDefinition, EffectDefinitionId,
    EffectInstanceId, EffectMutationKind, EffectRemovalRequest, EffectService,
    EffectStackingPolicy, EquipmentComponent, EquipmentEquipRequest, EquipmentService,
    EquipmentSlotDefinition, EquipmentSlotId, EquipmentUnequipRequest, IntrinsicSourcesComponent,
    InventoryComponent, ItemClassificationId, ItemComponent, ItemDefinition, ItemDefinitionId,
    ItemEquipmentPolicy, ItemKind, MechanicsArithmeticError, MechanicsCatalog,
    MechanicsCatalogDefinition, MechanicsError, MechanicsScalar, OperationId, SourceCollectionCost,
    SourceDefinition, SourceDefinitionId, SourceInstanceId, SourceInstanceIdentity,
    StackingGroupId, StackingPolicy, StatContribution, StatContributionDefinition, StatDefinition,
    StatEvaluation, StatId, StatService, StatValue, StatsComponent, TrackAdjustmentKind,
    TrackDefinition, TrackId, TrackMaximum, TrackMutationRequest, TrackReconciliationPolicy,
    TrackReconciliationRequest, TrackService, TrackSetPolicy, TrackSetRequest, TrackValue,
    TracksComponent, MAX_ACTIVE_EFFECT_INSTANCES, MAX_DAMAGE_PARTS, MAX_EFFECT_SOURCE_ACTIVATIONS,
    MAX_EQUIPMENT_ASSIGNMENTS, MAX_STAT_DECISIONS,
};
use serde::Deserialize;
use stats_alloc::{Region, StatsAlloc, INSTRUMENTED_SYSTEM};

const FACILITY: EntityId = EntityId::new(70_001);
const UNCONTAINED_MODULE: EntityId = EntityId::new(70_100);
const DECORATION: EntityId = EntityId::new(70_101);
const MAX_FIXTURE_MODULES: usize = 8;

#[global_allocator]
static FIXTURE_ALLOCATOR: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MeasuredAllocations {
    allocation_calls: u64,
    reallocation_calls: u64,
    allocated_bytes: u64,
    net_reallocated_bytes: i64,
}

fn measure_allocations(iterations: u64, mut operation: impl FnMut()) -> MeasuredAllocations {
    assert!(iterations > 0);
    let region = Region::new(FIXTURE_ALLOCATOR);
    for _ in 0..iterations {
        operation();
    }
    let statistics = region.change();
    let allocation_calls = statistics.allocations as u64;
    let reallocation_calls = statistics.reallocations as u64;
    let allocated_bytes = statistics.bytes_allocated as u64;
    let net_reallocated_bytes = statistics.bytes_reallocated as i64;
    assert_eq!(allocation_calls % iterations, 0);
    assert_eq!(reallocation_calls % iterations, 0);
    assert_eq!(allocated_bytes % iterations, 0);
    assert_eq!(net_reallocated_bytes % iterations as i64, 0);
    MeasuredAllocations {
        allocation_calls: allocation_calls / iterations,
        reallocation_calls: reallocation_calls / iterations,
        allocated_bytes: allocated_bytes / iterations,
        net_reallocated_bytes: net_reallocated_bytes / iterations as i64,
    }
}

fn scalar(value: i64) -> MechanicsScalar {
    MechanicsScalar::new(value).unwrap()
}

fn version() -> CatalogVersion {
    CatalogVersion::parse("gm7-builder.v1").unwrap()
}

fn stat(value: &str) -> StatId {
    StatId::parse(value).unwrap()
}

fn track(value: &str) -> TrackId {
    TrackId::parse(value).unwrap()
}

fn damage_kind(value: &str) -> DamageKindId {
    DamageKindId::parse(value).unwrap()
}

fn source(value: &str) -> SourceDefinitionId {
    SourceDefinitionId::parse(value).unwrap()
}

fn effect(value: &str) -> EffectDefinitionId {
    EffectDefinitionId::parse(value).unwrap()
}

fn effect_instance(value: &str) -> EffectInstanceId {
    EffectInstanceId::parse(value).unwrap()
}

fn classification(value: &str) -> ItemClassificationId {
    ItemClassificationId::parse(value).unwrap()
}

fn item(value: &str) -> ItemDefinitionId {
    ItemDefinitionId::parse(value).unwrap()
}

fn slot(index: usize) -> EquipmentSlotId {
    EquipmentSlotId::parse(format!("module_{index}")).unwrap()
}

fn operation(value: &str) -> OperationId {
    OperationId::parse(value).unwrap()
}

fn request_identity(operation: &OperationId, instance: &str) -> SourceInstanceIdentity {
    SourceInstanceIdentity::Request {
        operation: operation.clone(),
        instance: SourceInstanceId::parse(instance).unwrap(),
    }
}

fn module_entity(index: usize) -> EntityId {
    EntityId::new(70_010 + index as u64)
}

fn contribution(stat_id: &str, amount: i64, stacking_group: &str) -> StatContributionDefinition {
    StatContributionDefinition {
        stat: stat(stat_id),
        contribution: StatContribution::Add {
            amount: scalar(amount),
        },
        stacking_group: StackingGroupId::parse(stacking_group).unwrap(),
        stacking: StackingPolicy::Sum,
    }
}

fn catalog() -> MechanicsCatalog {
    MechanicsCatalog::admit(MechanicsCatalogDefinition {
        version: version(),
        stats: vec![
            StatDefinition {
                id: stat("durability_limit"),
                minimum: scalar(0),
                maximum: scalar(1_000),
            },
            StatDefinition {
                id: stat("production"),
                minimum: scalar(0),
                maximum: scalar(1_000),
            },
        ],
        tracks: vec![
            TrackDefinition {
                id: track("durability"),
                minimum: scalar(0),
                maximum: TrackMaximum::Stat {
                    stat: stat("durability_limit"),
                },
            },
            TrackDefinition {
                id: track("missing_insulation"),
                minimum: scalar(0),
                maximum: TrackMaximum::Fixed { value: scalar(50) },
            },
        ],
        sources: vec![
            SourceDefinition {
                id: source("installed_module"),
                priority: 0,
                stat_contributions: vec![
                    contribution("durability_limit", 50, "module_durability"),
                    contribution("production", 15, "module_production"),
                ],
                damage_responses: vec![DamageResponseDefinition::Absorb {
                    selector: DamageKindSelector::Exact {
                        damage_kind: damage_kind("corrosion"),
                    },
                    track: track("missing_insulation"),
                }],
            },
            SourceDefinition {
                id: source("temporary_improvement"),
                priority: 0,
                stat_contributions: vec![
                    contribution("durability_limit", 25, "improvement_durability"),
                    contribution("production", 5, "improvement_production"),
                ],
                damage_responses: vec![],
            },
        ],
        damage_kinds: vec![
            DamageKindDefinition {
                id: damage_kind("corrosion"),
            },
            DamageKindDefinition {
                id: damage_kind("impact"),
            },
        ],
        effects: vec![EffectDefinition {
            id: effect("temporary_improvement"),
            stacking_group: StackingGroupId::parse("temporary_improvement").unwrap(),
            stacking: EffectStackingPolicy::Refresh,
            maximum_stacks: 1,
            sources: vec![source("temporary_improvement")],
        }],
        capacity_metrics: vec![],
        items: vec![
            ItemDefinition {
                id: item("generator_module"),
                kind: ItemKind::Unique,
                maximum_quantity: 1,
                classifications: vec![classification("module")],
                capacity_costs: vec![],
                equipment: Some(ItemEquipmentPolicy {
                    required_slots: 1,
                    exclusive_group: None,
                }),
                sources: vec![source("installed_module")],
            },
            ItemDefinition {
                id: item("decoration"),
                kind: ItemKind::Unique,
                maximum_quantity: 1,
                classifications: vec![classification("decoration")],
                capacity_costs: vec![],
                equipment: None,
                sources: vec![],
            },
        ],
        equipment_slots: (0..MAX_FIXTURE_MODULES)
            .map(|index| EquipmentSlotDefinition {
                id: slot(index),
                allowed_classifications: vec![classification("module")],
            })
            .collect(),
    })
    .unwrap()
}

fn attach<T: EntityComponent>(state: &mut EntityState, entity: EntityId, value: T) {
    let revision = state.component_revision::<T>(entity).unwrap();
    EntityAuthoringService
        .attach_component(state, revision, entity, value)
        .unwrap();
}

fn initial_state(module_count: usize, include_invalid_items: bool) -> EntityState {
    assert!(module_count <= MAX_FIXTURE_MODULES);
    let mut definitions = vec![EntityDefinition::new(FACILITY, "fixture-facility")];
    definitions.extend((0..module_count).map(|index| {
        EntityDefinition::new(module_entity(index), format!("fixture-module-{index}"))
            .with_containment(FACILITY)
    }));
    if include_invalid_items {
        definitions.push(EntityDefinition::new(
            UNCONTAINED_MODULE,
            "fixture-uncontained-module",
        ));
        definitions.push(
            EntityDefinition::new(DECORATION, "fixture-decoration").with_containment(FACILITY),
        );
    }

    let mut state = EntityState::from_definitions_with_registry(
        gameplay_mechanics::gameplay_component_registry().unwrap(),
        definitions,
    )
    .unwrap();
    attach(
        &mut state,
        FACILITY,
        StatsComponent::new(
            version(),
            vec![
                StatValue::new(stat("durability_limit"), scalar(100)),
                StatValue::new(stat("production"), scalar(20)),
            ],
        )
        .unwrap(),
    );
    attach(
        &mut state,
        FACILITY,
        TracksComponent::new(
            version(),
            vec![TrackValue::new(track("durability"), scalar(100))],
        )
        .unwrap(),
    );
    attach(
        &mut state,
        FACILITY,
        ActiveEffectsComponent::new(version(), vec![]).unwrap(),
    );
    attach(
        &mut state,
        FACILITY,
        EquipmentComponent::new(version(), vec![]).unwrap(),
    );
    for index in 0..module_count {
        attach(
            &mut state,
            module_entity(index),
            ItemComponent::new(version(), item("generator_module")),
        );
    }
    if include_invalid_items {
        attach(
            &mut state,
            UNCONTAINED_MODULE,
            ItemComponent::new(version(), item("generator_module")),
        );
        attach(
            &mut state,
            DECORATION,
            ItemComponent::new(version(), item("decoration")),
        );
    }
    state
}

fn equip_modules(state: &mut EntityState, catalog: &MechanicsCatalog, module_count: usize) {
    for index in 0..module_count {
        let operation = operation(&format!("install_module_{index}"));
        let expected_state_revision = state.revision();
        let receipt = EquipmentService::equip(
            state,
            catalog,
            EquipmentEquipRequest {
                operation: operation.clone(),
                source: request_identity(&operation, "fixture_owner"),
                owner: FACILITY,
                item: module_entity(index),
                slots: vec![slot(index)],
                expected_equipment_revision: None,
                expected_state_revision,
            },
        )
        .unwrap();
        assert_eq!(receipt.tracks_validated, 1);
        assert_eq!(receipt.source_activations, index + 1);
    }
}

fn apply_improvement(state: &mut EntityState, catalog: &MechanicsCatalog) {
    let operation = operation("apply_temporary_improvement");
    let expected_revision = state
        .component_revision::<ActiveEffectsComponent>(FACILITY)
        .unwrap();
    let receipt = EffectService::apply(
        state,
        catalog,
        EffectApplyRequest {
            operation: operation.clone(),
            entity: FACILITY,
            instance: effect_instance("improvement_1"),
            definition: effect("temporary_improvement"),
            provenance: request_identity(&operation, "fixture_phase_owner"),
            stacks: 1,
            expected_revision: Some(expected_revision),
        },
    )
    .unwrap();
    assert_eq!(receipt.tracks_validated, 1);
    assert_eq!(receipt.activated_sources.len(), 1);
}

fn configured_state(
    catalog: &MechanicsCatalog,
    module_count: usize,
    include_invalid_items: bool,
) -> EntityState {
    let mut state = initial_state(module_count, include_invalid_items);
    equip_modules(&mut state, catalog, module_count);
    apply_improvement(&mut state, catalog);
    state
}

fn evaluate(
    state: &EntityState,
    catalog: &MechanicsCatalog,
    stat_id: &str,
    operation_id: &str,
) -> StatEvaluation {
    StatService::evaluate(
        state,
        catalog,
        FACILITY,
        &stat(stat_id),
        &operation(operation_id),
        &[],
    )
    .unwrap()
}

fn damage_request(
    operation_id: &str,
    parts: Vec<DamagePart>,
    expected_tracks_revision: Option<entity_state::ComponentRevision>,
) -> DamageRequest {
    let operation = operation(operation_id);
    DamageRequest {
        operation: operation.clone(),
        source: request_identity(&operation, "fixture_hazard"),
        actor: None,
        target: FACILITY,
        target_track: track("durability"),
        parts,
        request_sources: vec![],
        expected_tracks_revision,
    }
}

fn repair_request(
    operation_id: &str,
    amount: i64,
    expected_revision: Option<entity_state::ComponentRevision>,
) -> TrackMutationRequest {
    let operation = operation(operation_id);
    TrackMutationRequest {
        operation: operation.clone(),
        source: request_identity(&operation, "fixture_maintenance"),
        entity: FACILITY,
        track: track("durability"),
        amount: scalar(amount),
        kind: TrackAdjustmentKind::Restore,
        expected_revision,
    }
}

#[derive(Debug, PartialEq, Eq)]
struct FixtureStateEvidence {
    snapshot: String,
    state_revision: u64,
    slots: Vec<FixtureSlotEvidence>,
}

#[derive(Debug, PartialEq, Eq)]
struct FixtureSlotEvidence {
    entity: EntityId,
    stats: ComponentRevision,
    tracks: ComponentRevision,
    intrinsic_sources: ComponentRevision,
    effects: ComponentRevision,
    inventory: ComponentRevision,
    equipment: ComponentRevision,
    item: ComponentRevision,
    contained_in: Option<EntityId>,
}

fn capture_state(state: &EntityState) -> FixtureStateEvidence {
    FixtureStateEvidence {
        snapshot: encode_snapshot(state).unwrap(),
        state_revision: state.revision(),
        slots: state
            .entities()
            .map(|core| FixtureSlotEvidence {
                entity: core.id,
                stats: state.component_revision::<StatsComponent>(core.id).unwrap(),
                tracks: state
                    .component_revision::<TracksComponent>(core.id)
                    .unwrap(),
                intrinsic_sources: state
                    .component_revision::<IntrinsicSourcesComponent>(core.id)
                    .unwrap(),
                effects: state
                    .component_revision::<ActiveEffectsComponent>(core.id)
                    .unwrap(),
                inventory: state
                    .component_revision::<InventoryComponent>(core.id)
                    .unwrap(),
                equipment: state
                    .component_revision::<EquipmentComponent>(core.id)
                    .unwrap(),
                item: state.component_revision::<ItemComponent>(core.id).unwrap(),
                contained_in: state.contained_in(core.id),
            })
            .collect(),
    }
}

fn assert_state_unchanged(state: &EntityState, before: &FixtureStateEvidence) {
    assert_eq!(&capture_state(state), before);
}

#[derive(Debug, Clone, Copy)]
struct FixtureClock {
    day: u16,
    phase: u8,
}

impl FixtureClock {
    fn advance_to(&mut self, day: u16, phase: u8) {
        assert!((day, phase) > (self.day, self.phase));
        self.day = day;
        self.phase = phase;
    }
}

#[test]
fn builder_composition_uses_attributed_sources_explicit_expiry_and_reopen() {
    let catalog = catalog();
    let mut state = configured_state(&catalog, 1, false);

    let production = evaluate(&state, &catalog, "production", "inspect_production");
    assert_eq!(production.value, scalar(40));
    assert_eq!(production.decisions.len(), 2);
    assert!(production.decisions.iter().all(|decision| {
        decision.outcome == DecisionOutcome::Applied
            && matches!(
                decision.source,
                SourceInstanceIdentity::EquippedItem {
                    owner: FACILITY,
                    ..
                } | SourceInstanceIdentity::Effect {
                    entity: FACILITY,
                    ..
                }
            )
    }));
    assert_eq!(
        production.source_cost,
        SourceCollectionCost {
            effect_entries_visited: 1,
            effect_source_activations_visited: 1,
            equipment_entries_visited: 1,
            item_components_read: 1,
            ..SourceCollectionCost::default()
        }
    );

    let durability_limit = evaluate(
        &state,
        &catalog,
        "durability_limit",
        "inspect_durability_limit",
    );
    assert_eq!(durability_limit.value, scalar(175));
    assert_eq!(durability_limit.decisions.len(), 2);

    let restored = TrackService::restore(
        &mut state,
        &catalog,
        repair_request("restore_to_improved_limit", 75, None),
    )
    .unwrap();
    assert_eq!(
        (restored.before, restored.after),
        (scalar(100), scalar(175))
    );
    assert_eq!(restored.maximum, scalar(175));
    assert_eq!(restored.source_cost.effect_entries_visited, 1);
    assert_eq!(restored.source_cost.equipment_entries_visited, 1);

    let damage = DamageService::apply(
        &mut state,
        &catalog,
        damage_request(
            "facility_impact",
            vec![DamagePart {
                amount: scalar(20),
                kind: damage_kind("impact"),
            }],
            None,
        ),
    )
    .unwrap();
    assert_eq!(damage.parts[0].applied, scalar(20));
    assert_eq!(
        damage.track_changes[0],
        gameplay_mechanics::TrackDamageChange {
            track: track("durability"),
            before: scalar(175),
            after: scalar(155),
        }
    );
    assert!(damage.decisions.iter().any(|decision| {
        matches!(
            decision.source,
            SourceInstanceIdentity::EquippedItem {
                owner: FACILITY,
                item,
                ..
            } if item == module_entity(0)
        )
    }));

    let repaired = TrackService::restore(
        &mut state,
        &catalog,
        repair_request("facility_repair", 5, None),
    )
    .unwrap();
    assert_eq!(
        (repaired.before, repaired.after),
        (scalar(155), scalar(160))
    );

    let mut clock = FixtureClock { day: 4, phase: 1 };
    clock.advance_to(4, 2);
    assert_eq!((clock.day, clock.phase), (4, 2));

    let effects_revision = state
        .component_revision::<ActiveEffectsComponent>(FACILITY)
        .unwrap();
    let tracks_revision = state
        .component_revision::<TracksComponent>(FACILITY)
        .unwrap();
    let before_expiry_rejection = capture_state(&state);
    assert!(matches!(
        EffectService::expire(
            &mut state,
            &catalog,
            EffectRemovalRequest {
                operation: operation("expire_improvement_at_phase"),
                entity: FACILITY,
                instance: effect_instance("improvement_1"),
                expected_revision: Some(effects_revision.clone()),
            },
        ),
        Err(MechanicsError::EffectWouldInvalidateTrack {
            entity: FACILITY,
            current: 160,
            prospective_minimum: 0,
            prospective_maximum: 150,
            ..
        })
    ));
    assert_state_unchanged(&state, &before_expiry_rejection);

    let reconcile_operation = operation("reconcile_for_improvement_expiry");
    let reconcile = TrackService::reconcile_to_maximum(
        &mut state,
        &catalog,
        TrackReconciliationRequest {
            operation: reconcile_operation.clone(),
            source: request_identity(&reconcile_operation, "fixture_phase_owner"),
            entity: FACILITY,
            track: track("durability"),
            prospective_maximum: scalar(150),
            policy: TrackReconciliationPolicy::ClampToMaximum,
            expected_revision: Some(tracks_revision),
        },
    )
    .unwrap();
    assert_eq!(
        (reconcile.before, reconcile.after),
        (scalar(160), scalar(150))
    );

    let expired = EffectService::expire(
        &mut state,
        &catalog,
        EffectRemovalRequest {
            operation: operation("expire_improvement_at_phase"),
            entity: FACILITY,
            instance: effect_instance("improvement_1"),
            expected_revision: Some(effects_revision),
        },
    )
    .unwrap();
    assert_eq!(expired.kind, EffectMutationKind::Expire);
    assert_eq!(expired.tracks_validated, 1);
    assert_eq!(expired.source_cost.effect_entries_visited, 0);
    assert_eq!(expired.source_cost.equipment_entries_visited, 1);
    assert_eq!(
        evaluate(&state, &catalog, "production", "inspect_after_expiry").value,
        scalar(35)
    );

    let snapshot = encode_snapshot(&state).unwrap();
    let mut reopened = decode_snapshot_with_catalog(&snapshot, &catalog).unwrap();
    assert_eq!(encode_snapshot(&reopened).unwrap(), snapshot);
    let mut original_evaluation = evaluate(&state, &catalog, "production", "compare_after_reopen");
    let mut reopened_evaluation =
        evaluate(&reopened, &catalog, "production", "compare_after_reopen");
    assert_eq!(
        reopened_evaluation
            .observed_revisions
            .iter()
            .map(|observed| (observed.entity, observed.component))
            .collect::<Vec<_>>(),
        original_evaluation
            .observed_revisions
            .iter()
            .map(|observed| (observed.entity, observed.component))
            .collect::<Vec<_>>()
    );
    original_evaluation.observed_revisions.clear();
    reopened_evaluation.observed_revisions.clear();
    assert_eq!(reopened_evaluation, original_evaluation);

    let continued_damage = damage_request(
        "continued_impact",
        vec![DamagePart {
            amount: scalar(12),
            kind: damage_kind("impact"),
        }],
        None,
    );
    let original_receipt =
        DamageService::apply(&mut state, &catalog, continued_damage.clone()).unwrap();
    let reopened_receipt = DamageService::apply(&mut reopened, &catalog, continued_damage).unwrap();
    assert_eq!(reopened_receipt.parts, original_receipt.parts);
    assert_eq!(reopened_receipt.decisions, original_receipt.decisions);
    assert_eq!(
        reopened_receipt.track_changes,
        original_receipt.track_changes
    );
    assert_eq!(reopened_receipt.facts, original_receipt.facts);
    assert_eq!(reopened_receipt.source_cost, original_receipt.source_cost);
    assert_eq!(
        encode_snapshot(&reopened).unwrap(),
        encode_snapshot(&state).unwrap()
    );
}

#[test]
fn builder_failures_are_typed_atomic_and_reconcile_equipment_removal() {
    let catalog = catalog();
    let mut state = configured_state(&catalog, 1, true);

    let equipment_revision = state
        .component_revision::<EquipmentComponent>(FACILITY)
        .unwrap();
    let before_uncontained = capture_state(&state);
    let uncontained_operation = operation("install_uncontained_module");
    let expected_state_revision = state.revision();
    assert!(matches!(
        EquipmentService::equip(
            &mut state,
            &catalog,
            EquipmentEquipRequest {
                operation: uncontained_operation.clone(),
                source: request_identity(&uncontained_operation, "fixture_owner"),
                owner: FACILITY,
                item: UNCONTAINED_MODULE,
                slots: vec![slot(1)],
                expected_equipment_revision: Some(equipment_revision.clone()),
                expected_state_revision,
            },
        ),
        Err(MechanicsError::ItemNotContained {
            item: UNCONTAINED_MODULE,
            expected_owner: FACILITY,
            actual_owner: None,
        })
    ));
    assert_state_unchanged(&state, &before_uncontained);

    let before_not_equippable = capture_state(&state);
    let decoration_operation = operation("install_decoration");
    let expected_state_revision = state.revision();
    assert!(matches!(
        EquipmentService::equip(
            &mut state,
            &catalog,
            EquipmentEquipRequest {
                operation: decoration_operation.clone(),
                source: request_identity(&decoration_operation, "fixture_owner"),
                owner: FACILITY,
                item: DECORATION,
                slots: vec![slot(1)],
                expected_equipment_revision: Some(equipment_revision),
                expected_state_revision,
            },
        ),
        Err(MechanicsError::ItemNotEquippable {
            item: DECORATION,
            definition,
        }) if definition == item("decoration")
    ));
    assert_state_unchanged(&state, &before_not_equippable);

    let stale_tracks_revision = state
        .component_revision::<TracksComponent>(FACILITY)
        .unwrap();
    TrackService::spend(
        &mut state,
        &catalog,
        TrackMutationRequest {
            operation: operation("maintenance_probe_advance"),
            source: request_identity(
                &operation("maintenance_probe_advance"),
                "fixture_maintenance",
            ),
            entity: FACILITY,
            track: track("durability"),
            amount: scalar(1),
            kind: TrackAdjustmentKind::Spend,
            expected_revision: None,
        },
    )
    .unwrap();
    let before_stale = capture_state(&state);
    assert!(matches!(
        DamageService::apply(
            &mut state,
            &catalog,
            damage_request(
                "stale_hazard",
                vec![DamagePart {
                    amount: scalar(1),
                    kind: damage_kind("impact"),
                }],
                Some(stale_tracks_revision),
            ),
        ),
        Err(MechanicsError::StaleComponentRevision { .. })
    ));
    assert_state_unchanged(&state, &before_stale);

    let before_quota = capture_state(&state);
    assert!(matches!(
        DamageService::apply(
            &mut state,
            &catalog,
            damage_request(
                "oversized_hazard",
                (0..=MAX_DAMAGE_PARTS)
                    .map(|_| DamagePart {
                        amount: scalar(1),
                        kind: damage_kind("impact"),
                    })
                    .collect(),
                None,
            ),
        ),
        Err(MechanicsError::RequestQuotaExceeded {
            field: "damageParts",
            actual,
            maximum: MAX_DAMAGE_PARTS,
        }) if actual == MAX_DAMAGE_PARTS + 1
    ));
    assert_state_unchanged(&state, &before_quota);

    let before_bound = capture_state(&state);
    let bound_operation = operation("reject_invalid_durability");
    assert!(matches!(
        TrackService::set_under_policy(
            &mut state,
            &catalog,
            TrackSetRequest {
                operation: bound_operation.clone(),
                source: request_identity(&bound_operation, "fixture_maintenance"),
                entity: FACILITY,
                track: track("durability"),
                value: scalar(999),
                policy: TrackSetPolicy::RejectOutOfBounds,
                expected_revision: None,
            },
        ),
        Err(MechanicsError::TrackOutOfBounds {
            entity: FACILITY,
            attempted: 999,
            maximum: 175,
            ..
        })
    ));
    assert_state_unchanged(&state, &before_bound);

    let before_late_damage = capture_state(&state);
    assert!(matches!(
        DamageService::apply(
            &mut state,
            &catalog,
            damage_request(
                "late_corrosion_failure",
                vec![
                    DamagePart {
                        amount: scalar(5),
                        kind: damage_kind("impact"),
                    },
                    DamagePart {
                        amount: scalar(5),
                        kind: damage_kind("corrosion"),
                    },
                ],
                None,
            ),
        ),
        Err(MechanicsError::MissingTrack {
            entity: FACILITY,
            track: missing,
        }) if missing == track("missing_insulation")
    ));
    assert_state_unchanged(&state, &before_late_damage);

    let before_restore_failure = capture_state(&state);
    assert!(matches!(
        TrackService::restore(
            &mut state,
            &catalog,
            repair_request("negative_repair", -1, None),
        ),
        Err(MechanicsError::Arithmetic(
            MechanicsArithmeticError::NegativeAmount { value: -1 }
        ))
    ));
    assert_state_unchanged(&state, &before_restore_failure);

    TrackService::restore(
        &mut state,
        &catalog,
        repair_request("restore_for_removal_probes", 100, None),
    )
    .unwrap();
    let effects_revision = state
        .component_revision::<ActiveEffectsComponent>(FACILITY)
        .unwrap();
    let tracks_revision = state
        .component_revision::<TracksComponent>(FACILITY)
        .unwrap();
    let before_effect_removal = capture_state(&state);
    assert!(matches!(
        EffectService::expire(
            &mut state,
            &catalog,
            EffectRemovalRequest {
                operation: operation("expire_bound_probe"),
                entity: FACILITY,
                instance: effect_instance("improvement_1"),
                expected_revision: Some(effects_revision.clone()),
            },
        ),
        Err(MechanicsError::EffectWouldInvalidateTrack {
            entity: FACILITY,
            current: 175,
            prospective_maximum: 150,
            ..
        })
    ));
    assert_state_unchanged(&state, &before_effect_removal);

    let reconcile_effect_operation = operation("reconcile_effect_bound_probe");
    TrackService::reconcile_to_maximum(
        &mut state,
        &catalog,
        TrackReconciliationRequest {
            operation: reconcile_effect_operation.clone(),
            source: request_identity(&reconcile_effect_operation, "fixture_phase_owner"),
            entity: FACILITY,
            track: track("durability"),
            prospective_maximum: scalar(150),
            policy: TrackReconciliationPolicy::ClampToMaximum,
            expected_revision: Some(tracks_revision),
        },
    )
    .unwrap();
    EffectService::expire(
        &mut state,
        &catalog,
        EffectRemovalRequest {
            operation: operation("expire_bound_probe"),
            entity: FACILITY,
            instance: effect_instance("improvement_1"),
            expected_revision: Some(effects_revision),
        },
    )
    .unwrap();

    let equipment_revision = state
        .component_revision::<EquipmentComponent>(FACILITY)
        .unwrap();
    let tracks_revision = state
        .component_revision::<TracksComponent>(FACILITY)
        .unwrap();
    let stale_state_revision = state.revision();
    let before_equipment_removal = capture_state(&state);
    let unequip_operation = operation("remove_installed_module");
    assert!(matches!(
        EquipmentService::unequip(
            &mut state,
            &catalog,
            EquipmentUnequipRequest {
                operation: unequip_operation.clone(),
                source: request_identity(&unequip_operation, "fixture_owner"),
                owner: FACILITY,
                item: module_entity(0),
                expected_equipment_revision: Some(equipment_revision.clone()),
                expected_state_revision: stale_state_revision,
            },
        ),
        Err(MechanicsError::EquipmentWouldInvalidateTrack {
            owner: FACILITY,
            current: 150,
            prospective_maximum: 100,
            ..
        })
    ));
    assert_state_unchanged(&state, &before_equipment_removal);

    let reconcile_equipment_operation = operation("reconcile_module_bound_probe");
    TrackService::reconcile_to_maximum(
        &mut state,
        &catalog,
        TrackReconciliationRequest {
            operation: reconcile_equipment_operation.clone(),
            source: request_identity(&reconcile_equipment_operation, "fixture_owner"),
            entity: FACILITY,
            track: track("durability"),
            prospective_maximum: scalar(100),
            policy: TrackReconciliationPolicy::ClampToMaximum,
            expected_revision: Some(tracks_revision),
        },
    )
    .unwrap();

    let before_stale_unequip = capture_state(&state);
    assert!(matches!(
        EquipmentService::unequip(
            &mut state,
            &catalog,
            EquipmentUnequipRequest {
                operation: unequip_operation.clone(),
                source: request_identity(&unequip_operation, "fixture_owner"),
                owner: FACILITY,
                item: module_entity(0),
                expected_equipment_revision: Some(equipment_revision.clone()),
                expected_state_revision: stale_state_revision,
            },
        ),
        Err(MechanicsError::Relationship(
            entity_state::RelationshipError::StaleRevision { .. }
        ))
    ));
    assert_state_unchanged(&state, &before_stale_unequip);

    let expected_state_revision = state.revision();
    let unequipped = EquipmentService::unequip(
        &mut state,
        &catalog,
        EquipmentUnequipRequest {
            operation: unequip_operation.clone(),
            source: request_identity(&unequip_operation, "fixture_owner"),
            owner: FACILITY,
            item: module_entity(0),
            expected_equipment_revision: Some(equipment_revision),
            expected_state_revision,
        },
    )
    .unwrap();
    assert_eq!(unequipped.tracks_validated, 1);
    assert_eq!(unequipped.source_activations, 0);
    assert_eq!(state.contained_in(module_entity(0)), Some(FACILITY));
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BuilderEvidence {
    schema_version: u16,
    scope: String,
    simple: CostEvidence,
    stressed: CostEvidence,
    quotas: QuotaEvidence,
    memory_accounting: MemoryAccounting,
    release_measurement: ReleaseMeasurement,
    api_amplification: ApiAmplification,
    non_claims: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CostEvidence {
    modules: usize,
    active_effects: usize,
    stat_decisions: usize,
    source_cost: SourceCostEvidence,
    snapshot_bytes: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SourceCostEvidence {
    intrinsic_entries_visited: usize,
    effect_entries_visited: usize,
    effect_source_activations_visited: usize,
    equipment_entries_visited: usize,
    item_components_read: usize,
    request_entries_visited: usize,
}

impl SourceCostEvidence {
    fn assert_matches(&self, actual: SourceCollectionCost) {
        assert_eq!(
            self.intrinsic_entries_visited,
            actual.intrinsic_entries_visited
        );
        assert_eq!(self.effect_entries_visited, actual.effect_entries_visited);
        assert_eq!(
            self.effect_source_activations_visited,
            actual.effect_source_activations_visited
        );
        assert_eq!(
            self.equipment_entries_visited,
            actual.equipment_entries_visited
        );
        assert_eq!(self.item_components_read, actual.item_components_read);
        assert_eq!(self.request_entries_visited, actual.request_entries_visited);
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct QuotaEvidence {
    max_equipment_assignments: usize,
    max_damage_parts: usize,
    max_stat_decisions: usize,
    max_active_effect_instances: usize,
    max_effect_source_activations: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MemoryAccounting {
    allocations: String,
    clones: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReleaseMeasurement {
    profile: String,
    rustc: String,
    target: String,
    iterations: u64,
    simple_stat_evaluation: AllocationEvidence,
    stressed_stat_evaluation: AllocationEvidence,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AllocationEvidence {
    allocation_calls: u64,
    reallocation_calls: u64,
    allocated_bytes: u64,
    net_reallocated_bytes: i64,
}

impl AllocationEvidence {
    fn assert_matches(&self, actual: MeasuredAllocations) {
        assert_eq!(self.allocation_calls, actual.allocation_calls);
        assert_eq!(self.reallocation_calls, actual.reallocation_calls);
        assert_eq!(self.allocated_bytes, actual.allocated_bytes);
        assert_eq!(self.net_reallocated_bytes, actual.net_reallocated_bytes);
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ApiAmplification {
    stat_evaluation: ApiOperationEvidence,
    one_part_damage_apply: ApiOperationEvidence,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ApiOperationEvidence {
    public_service_calls: usize,
    component_slot_writes: usize,
}

#[test]
fn checked_builder_evidence_records_bounded_costs_sizes_and_non_claims() {
    let catalog = catalog();
    let simple = configured_state(&catalog, 1, false);
    let stressed = configured_state(&catalog, MAX_FIXTURE_MODULES, false);
    let simple_evaluation = evaluate(&simple, &catalog, "production", "measure_simple");
    let stressed_evaluation = evaluate(&stressed, &catalog, "production", "measure_stressed");
    let simple_snapshot = encode_snapshot(&simple).unwrap();
    let stressed_snapshot = encode_snapshot(&stressed).unwrap();

    let evidence: BuilderEvidence = serde_json::from_str(include_str!(
        "../../../../fixtures/gameplay-mechanics/builder-evidence-v1.json"
    ))
    .unwrap();
    assert_eq!(evidence.schema_version, 2);
    assert_eq!(
        evidence.scope,
        "headless-gameplay-mechanics-builder-fixture"
    );

    assert_eq!(
        evidence.simple.modules,
        simple
            .component::<EquipmentComponent>(FACILITY)
            .unwrap()
            .unwrap()
            .assignments()
            .len()
    );
    assert_eq!(
        evidence.simple.active_effects,
        simple
            .component::<ActiveEffectsComponent>(FACILITY)
            .unwrap()
            .unwrap()
            .effects()
            .len()
    );
    assert_eq!(
        evidence.simple.stat_decisions,
        simple_evaluation.decisions.len()
    );
    evidence
        .simple
        .source_cost
        .assert_matches(simple_evaluation.source_cost);
    assert_eq!(evidence.simple.snapshot_bytes, simple_snapshot.len());

    assert_eq!(
        evidence.stressed.modules,
        stressed
            .component::<EquipmentComponent>(FACILITY)
            .unwrap()
            .unwrap()
            .assignments()
            .len()
    );
    assert_eq!(
        evidence.stressed.active_effects,
        stressed
            .component::<ActiveEffectsComponent>(FACILITY)
            .unwrap()
            .unwrap()
            .effects()
            .len()
    );
    assert_eq!(
        evidence.stressed.stat_decisions,
        stressed_evaluation.decisions.len()
    );
    evidence
        .stressed
        .source_cost
        .assert_matches(stressed_evaluation.source_cost);
    assert_eq!(evidence.stressed.snapshot_bytes, stressed_snapshot.len());

    assert_eq!(
        evidence.quotas.max_equipment_assignments,
        MAX_EQUIPMENT_ASSIGNMENTS
    );
    assert_eq!(evidence.quotas.max_damage_parts, MAX_DAMAGE_PARTS);
    assert_eq!(evidence.quotas.max_stat_decisions, MAX_STAT_DECISIONS);
    assert_eq!(
        evidence.quotas.max_active_effect_instances,
        MAX_ACTIVE_EFFECT_INSTANCES
    );
    assert_eq!(
        evidence.quotas.max_effect_source_activations,
        MAX_EFFECT_SOURCE_ACTIVATIONS
    );
    assert_eq!(
        evidence.memory_accounting.allocations,
        "isolated single-test System allocator observations for release stat evaluation; not a normative performance budget"
    );
    assert_eq!(
        evidence.memory_accounting.clones,
        "not exposed by public APIs; visits and canonical bytes are recorded instead"
    );
    assert_eq!(evidence.release_measurement.profile, "release");
    assert_eq!(
        evidence.release_measurement.rustc,
        "rustc 1.96.0 (ac68faa20 2026-05-25)"
    );
    assert_eq!(
        evidence.release_measurement.target,
        "x86_64-unknown-linux-gnu"
    );
    assert_eq!(evidence.release_measurement.iterations, 1_000);
    assert_eq!(
        evidence
            .api_amplification
            .stat_evaluation
            .public_service_calls,
        1
    );
    assert_eq!(
        evidence
            .api_amplification
            .stat_evaluation
            .component_slot_writes,
        0
    );
    assert_eq!(
        evidence
            .api_amplification
            .one_part_damage_apply
            .public_service_calls,
        1
    );
    assert_eq!(
        evidence
            .api_amplification
            .one_part_damage_apply
            .component_slot_writes,
        1
    );

    if !cfg!(debug_assertions) {
        let simple_stat = stat("production");
        let simple_operation = operation("measure_simple_allocations");
        let stressed_stat = stat("production");
        let stressed_operation = operation("measure_stressed_allocations");

        // Warm both paths before counting so the evidence describes the
        // steady direct service call rather than one-time test initialization.
        black_box(
            StatService::evaluate(
                &simple,
                &catalog,
                FACILITY,
                &simple_stat,
                &simple_operation,
                &[],
            )
            .unwrap(),
        );
        black_box(
            StatService::evaluate(
                &stressed,
                &catalog,
                FACILITY,
                &stressed_stat,
                &stressed_operation,
                &[],
            )
            .unwrap(),
        );

        let simple_allocations =
            measure_allocations(evidence.release_measurement.iterations, || {
                black_box(
                    StatService::evaluate(
                        &simple,
                        &catalog,
                        FACILITY,
                        &simple_stat,
                        &simple_operation,
                        &[],
                    )
                    .unwrap(),
                );
            });
        let stressed_allocations =
            measure_allocations(evidence.release_measurement.iterations, || {
                black_box(
                    StatService::evaluate(
                        &stressed,
                        &catalog,
                        FACILITY,
                        &stressed_stat,
                        &stressed_operation,
                        &[],
                    )
                    .unwrap(),
                );
            });
        println!("simple stat allocations: {simple_allocations:?}");
        println!("stressed stat allocations: {stressed_allocations:?}");
        evidence
            .release_measurement
            .simple_stat_evaluation
            .assert_matches(simple_allocations);
        evidence
            .release_measurement
            .stressed_stat_evaluation
            .assert_matches(stressed_allocations);
    }
    assert_eq!(
        evidence.non_claims,
        vec![
            "headless regression fixture, not an external consumer or live product proof",
            "release allocation observations are compiler/platform-specific and not an API budget",
            "no clone count is inferred from visit or allocation counts",
            "not a promotion vote for gameplay-rules",
        ]
    );
}
