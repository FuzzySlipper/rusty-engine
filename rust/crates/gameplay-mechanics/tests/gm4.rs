use core_ids::EntityId;
use entity_state::{
    encode_snapshot, EntityAuthoringService, EntityComponent, EntityDefinition, EntityState,
};
use gameplay_mechanics::{
    decode_snapshot_with_catalog, ActiveEffectInstance, ActiveEffectsComponent,
    CapacityMetricDefinition, CapacityMetricId, CatalogError, CatalogVersion, DamageKindDefinition,
    DamageKindId, DamageKindSelector, DamagePart, DamageRequest, DamageResponseDefinition,
    DamageService, EffectDefinition, EffectDefinitionId, EffectInstanceId, EffectStackingPolicy,
    EquipmentAssignment, EquipmentComponent, EquipmentEquipRequest, EquipmentExclusivityId,
    EquipmentService, EquipmentSlotDefinition, EquipmentSlotId, EquipmentSwapRequest,
    EquipmentUnequipRequest, InventoryCapacityLimit, InventoryComponent, InventoryMutationRequest,
    InventoryService, InventoryTransferRequest, ItemCapacityCost, ItemClassificationId,
    ItemComponent, ItemDefinition, ItemDefinitionId, ItemDestroyRequest, ItemEquipmentPolicy,
    ItemKind, ItemService, ItemStack, ItemTransferRequest, MechanicsCatalog,
    MechanicsCatalogDefinition, MechanicsEntityView, MechanicsError, MechanicsScalar,
    MechanicsSnapshotError, OperationId, SourceDefinition, SourceDefinitionId, SourceInstanceId,
    SourceInstanceIdentity, StackingGroupId, StackingPolicy, StatContribution,
    StatContributionDefinition, StatDefinition, StatId, StatService, StatValue, StatsComponent,
    TrackAdjustmentKind, TrackDefinition, TrackId, TrackMaximum, TrackMutationRequest,
    TrackService, TrackValue, TracksComponent, INVENTORY_COMPONENT_TYPE_ID,
    MAX_CONTAINED_ENTITIES_PER_INVENTORY, MAX_EQUIPMENT_SOURCE_ACTIVATIONS,
};

const PLAYER: EntityId = EntityId::new(1_001);
const SECOND_OWNER: EntityId = EntityId::new(1_002);
const BUILDING: EntityId = EntityId::new(1_003);
const RIFLE: EntityId = EntityId::new(1_010);
const PISTOL: EntityId = EntityId::new(1_011);
const SHIELD: EntityId = EntityId::new(1_012);
const ARMOR: EntityId = EntityId::new(1_013);
const MODULE: EntityId = EntityId::new(1_014);
const BUILDING_MODULE: EntityId = EntityId::new(1_015);
const TRINKET: EntityId = EntityId::new(1_016);
const DECOR: EntityId = EntityId::new(1_017);

fn scalar(value: i64) -> MechanicsScalar {
    MechanicsScalar::new(value).unwrap()
}

fn version() -> CatalogVersion {
    CatalogVersion::parse("gm4.v1").unwrap()
}

fn capacity(value: &str) -> CapacityMetricId {
    CapacityMetricId::parse(value).unwrap()
}

fn classification(value: &str) -> ItemClassificationId {
    ItemClassificationId::parse(value).unwrap()
}

fn item(value: &str) -> ItemDefinitionId {
    ItemDefinitionId::parse(value).unwrap()
}

fn slot(value: &str) -> EquipmentSlotId {
    EquipmentSlotId::parse(value).unwrap()
}

fn exclusivity(value: &str) -> EquipmentExclusivityId {
    EquipmentExclusivityId::parse(value).unwrap()
}

fn stat(value: &str) -> StatId {
    StatId::parse(value).unwrap()
}

fn track(value: &str) -> TrackId {
    TrackId::parse(value).unwrap()
}

fn source(value: &str) -> SourceDefinitionId {
    SourceDefinitionId::parse(value).unwrap()
}

fn effect(value: &str) -> EffectDefinitionId {
    EffectDefinitionId::parse(value).unwrap()
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

fn capacity_cost(metric: &str, units: u64) -> ItemCapacityCost {
    ItemCapacityCost {
        metric: capacity(metric),
        units,
    }
}

fn unique_item(
    id: &str,
    classifications: &[&str],
    costs: &[(&str, u64)],
    required_slots: Option<u16>,
    exclusive_group: Option<&str>,
    sources: &[&str],
) -> ItemDefinition {
    ItemDefinition {
        id: item(id),
        kind: ItemKind::Unique,
        maximum_quantity: 1,
        classifications: classifications
            .iter()
            .map(|value| classification(value))
            .collect(),
        capacity_costs: costs
            .iter()
            .map(|(metric, units)| capacity_cost(metric, *units))
            .collect(),
        equipment: required_slots.map(|required_slots| ItemEquipmentPolicy {
            required_slots,
            exclusive_group: exclusive_group.map(exclusivity),
        }),
        sources: sources.iter().map(|value| source(value)).collect(),
    }
}

fn catalog_definition() -> MechanicsCatalogDefinition {
    MechanicsCatalogDefinition {
        version: version(),
        stats: vec![
            StatDefinition {
                id: stat("aim"),
                minimum: scalar(0),
                maximum: scalar(100),
            },
            StatDefinition {
                id: stat("output"),
                minimum: scalar(0),
                maximum: scalar(100),
            },
        ],
        tracks: vec![
            TrackDefinition {
                id: track("durability"),
                minimum: scalar(0),
                maximum: TrackMaximum::Fixed { value: scalar(100) },
            },
            TrackDefinition {
                id: track("health"),
                minimum: scalar(0),
                maximum: TrackMaximum::Fixed { value: scalar(100) },
            },
        ],
        sources: vec![
            SourceDefinition {
                id: source("guard"),
                priority: 0,
                stat_contributions: vec![],
                damage_responses: vec![DamageResponseDefinition::FlatReduction {
                    selector: DamageKindSelector::Any,
                    amount: scalar(3),
                    stacking_group: StackingGroupId::parse("guard_reduction").unwrap(),
                    stacking: StackingPolicy::Highest,
                }],
            },
            SourceDefinition {
                id: source("precision"),
                priority: 0,
                stat_contributions: vec![StatContributionDefinition {
                    stat: stat("aim"),
                    contribution: StatContribution::Add { amount: scalar(5) },
                    stacking_group: StackingGroupId::parse("precision_bonus").unwrap(),
                    stacking: StackingPolicy::Sum,
                }],
                damage_responses: vec![],
            },
            SourceDefinition {
                id: source("production"),
                priority: 0,
                stat_contributions: vec![StatContributionDefinition {
                    stat: stat("output"),
                    contribution: StatContribution::Add { amount: scalar(7) },
                    stacking_group: StackingGroupId::parse("production_bonus").unwrap(),
                    stacking: StackingPolicy::Sum,
                }],
                damage_responses: vec![],
            },
        ],
        damage_kinds: vec![DamageKindDefinition {
            id: DamageKindId::parse("impact").unwrap(),
        }],
        effects: vec![EffectDefinition {
            id: effect("reinforced"),
            stacking_group: StackingGroupId::parse("reinforced_lifecycle").unwrap(),
            stacking: EffectStackingPolicy::Refresh,
            maximum_stacks: 1,
            sources: vec![source("guard")],
        }],
        capacity_metrics: vec![
            CapacityMetricDefinition {
                id: capacity("mass"),
            },
            CapacityMetricDefinition {
                id: capacity("power"),
            },
        ],
        items: vec![
            ItemDefinition {
                id: item("ammunition"),
                kind: ItemKind::Fungible,
                maximum_quantity: 100,
                classifications: vec![],
                capacity_costs: vec![capacity_cost("mass", 1)],
                equipment: None,
                sources: vec![],
            },
            ItemDefinition {
                id: item("material"),
                kind: ItemKind::Fungible,
                maximum_quantity: 50,
                classifications: vec![],
                capacity_costs: vec![capacity_cost("mass", 2)],
                equipment: None,
                sources: vec![],
            },
            unique_item(
                "armor",
                &["armor"],
                &[("mass", 15)],
                Some(1),
                Some("armor_slot"),
                &["guard"],
            ),
            unique_item(
                "module",
                &["module"],
                &[("mass", 4), ("power", 3)],
                Some(1),
                Some("module_slot"),
                &["production"],
            ),
            unique_item(
                "pistol",
                &["weapon"],
                &[("mass", 5)],
                Some(1),
                Some("weapon_set"),
                &["precision"],
            ),
            unique_item(
                "rifle",
                &["weapon"],
                &[("mass", 12)],
                Some(2),
                Some("weapon_set"),
                &["precision"],
            ),
            unique_item(
                "shield",
                &["weapon"],
                &[("mass", 7)],
                Some(1),
                Some("weapon_set"),
                &["guard"],
            ),
            unique_item("trinket", &["collectible"], &[("mass", 1)], None, None, &[]),
        ],
        equipment_slots: vec![
            EquipmentSlotDefinition {
                id: slot("body"),
                allowed_classifications: vec![classification("armor")],
            },
            EquipmentSlotDefinition {
                id: slot("hand_left"),
                allowed_classifications: vec![classification("weapon")],
            },
            EquipmentSlotDefinition {
                id: slot("hand_right"),
                allowed_classifications: vec![classification("weapon")],
            },
            EquipmentSlotDefinition {
                id: slot("module"),
                allowed_classifications: vec![classification("module")],
            },
        ],
    }
}

fn catalog() -> MechanicsCatalog {
    MechanicsCatalog::admit(catalog_definition()).unwrap()
}

fn inventory(stacks: &[(&str, u64)], limits: &[(&str, u64)]) -> InventoryComponent {
    InventoryComponent::with_capacity_limits(
        version(),
        stacks
            .iter()
            .map(|(definition, quantity)| ItemStack {
                definition: item(definition),
                quantity: *quantity,
            })
            .collect(),
        limits
            .iter()
            .map(|(metric, maximum)| InventoryCapacityLimit::new(capacity(metric), *maximum))
            .collect(),
    )
    .unwrap()
}

fn attach<T: EntityComponent>(state: &mut EntityState, entity: EntityId, value: T) {
    let revision = state.component_revision::<T>(entity).unwrap();
    EntityAuthoringService
        .attach_component(state, revision, entity, value)
        .unwrap();
}

fn state() -> EntityState {
    let mut definitions = vec![
        EntityDefinition::new(PLAYER, "player"),
        EntityDefinition::new(SECOND_OWNER, "second-owner"),
        EntityDefinition::new(BUILDING, "building"),
        EntityDefinition::new(RIFLE, "rifle").with_containment(PLAYER),
        EntityDefinition::new(PISTOL, "pistol").with_containment(PLAYER),
        EntityDefinition::new(SHIELD, "shield").with_containment(PLAYER),
        EntityDefinition::new(ARMOR, "armor").with_containment(PLAYER),
        EntityDefinition::new(MODULE, "module").with_containment(PLAYER),
        EntityDefinition::new(BUILDING_MODULE, "building-module").with_containment(BUILDING),
        EntityDefinition::new(TRINKET, "trinket").with_containment(PLAYER),
        EntityDefinition::new(DECOR, "inventory-decor").with_containment(PLAYER),
    ];
    definitions.extend(
        (0..24).map(|index| EntityDefinition::new(EntityId::new(2_000 + index), "unrelated")),
    );
    let registry = gameplay_mechanics::gameplay_component_registry().unwrap();
    let mut state = EntityState::from_definitions_with_registry(registry, definitions).unwrap();

    attach(
        &mut state,
        PLAYER,
        inventory(
            &[("material", 5), ("ammunition", 10)],
            &[("power", 10), ("mass", 80)],
        ),
    );
    attach(
        &mut state,
        SECOND_OWNER,
        inventory(&[], &[("mass", 8), ("power", 2)]),
    );
    attach(
        &mut state,
        BUILDING,
        inventory(&[], &[("power", 5), ("mass", 20)]),
    );
    attach(
        &mut state,
        PLAYER,
        EquipmentComponent::new(version(), vec![]).unwrap(),
    );
    attach(
        &mut state,
        SECOND_OWNER,
        EquipmentComponent::new(version(), vec![]).unwrap(),
    );
    attach(
        &mut state,
        BUILDING,
        EquipmentComponent::new(version(), vec![]).unwrap(),
    );
    for (entity, definition) in [
        (RIFLE, "rifle"),
        (PISTOL, "pistol"),
        (SHIELD, "shield"),
        (ARMOR, "armor"),
        (MODULE, "module"),
        (BUILDING_MODULE, "module"),
        (TRINKET, "trinket"),
    ] {
        attach(
            &mut state,
            entity,
            ItemComponent::new(version(), item(definition)),
        );
    }
    attach(
        &mut state,
        PLAYER,
        StatsComponent::new(version(), vec![StatValue::new(stat("aim"), scalar(10))]).unwrap(),
    );
    attach(
        &mut state,
        BUILDING,
        StatsComponent::new(version(), vec![StatValue::new(stat("output"), scalar(2))]).unwrap(),
    );
    attach(
        &mut state,
        PLAYER,
        TracksComponent::new(
            version(),
            vec![TrackValue::new(track("health"), scalar(100))],
        )
        .unwrap(),
    );
    attach(
        &mut state,
        RIFLE,
        TracksComponent::new(
            version(),
            vec![TrackValue::new(track("durability"), scalar(60))],
        )
        .unwrap(),
    );
    let provenance_operation = operation("reinforced_rifle");
    attach(
        &mut state,
        RIFLE,
        ActiveEffectsComponent::new(
            version(),
            vec![ActiveEffectInstance::new(
                EffectInstanceId::parse("reinforced_instance").unwrap(),
                effect("reinforced"),
                request_identity(&provenance_operation, "reinforcement"),
                1,
            )
            .unwrap()],
        )
        .unwrap(),
    );
    state
}

fn usage(view: &[gameplay_mechanics::CapacityUsage], metric: &str) -> u64 {
    view.iter()
        .find(|entry| entry.metric == capacity(metric))
        .unwrap()
        .used
}

#[test]
fn inventory_grant_consume_and_transfer_are_bounded_atomic_and_auditable() {
    let catalog = catalog();
    let mut state = state();
    let initial = InventoryService::view(&state, &catalog, PLAYER).unwrap();
    assert_eq!(
        initial
            .stacks()
            .iter()
            .map(|stack| (stack.definition.as_str(), stack.quantity))
            .collect::<Vec<_>>(),
        vec![("ammunition", 10), ("material", 5)]
    );
    assert_eq!(usage(initial.capacity(), "mass"), 64);
    assert_eq!(usage(initial.capacity(), "power"), 3);

    let grant_operation = operation("grant_ammunition");
    let grant_revision = state
        .component_revision::<InventoryComponent>(PLAYER)
        .unwrap();
    let grant = InventoryService::grant(
        &mut state,
        &catalog,
        InventoryMutationRequest {
            operation: grant_operation.clone(),
            source: request_identity(&grant_operation, "pickup"),
            owner: PLAYER,
            item: item("ammunition"),
            quantity: 5,
            expected_revision: Some(grant_revision.clone()),
        },
    )
    .unwrap();
    assert_eq!((grant.before_quantity, grant.after_quantity), (10, 15));
    assert_eq!(usage(&grant.capacity_before, "mass"), 64);
    assert_eq!(usage(&grant.capacity_after, "mass"), 69);

    let before_stale = state
        .component::<InventoryComponent>(PLAYER)
        .unwrap()
        .unwrap()
        .clone();
    let before_stale_revision = state.revision();
    let stale_operation = operation("stale_inventory_mutation");
    assert!(matches!(
        InventoryService::consume(
            &mut state,
            &catalog,
            InventoryMutationRequest {
                operation: stale_operation.clone(),
                source: request_identity(&stale_operation, "stale"),
                owner: PLAYER,
                item: item("ammunition"),
                quantity: 1,
                expected_revision: Some(grant_revision),
            },
        ),
        Err(MechanicsError::StaleComponentRevision { .. })
    ));
    assert_eq!(state.revision(), before_stale_revision);
    assert_eq!(
        state
            .component::<InventoryComponent>(PLAYER)
            .unwrap()
            .unwrap(),
        &before_stale
    );

    let consume_operation = operation("craft_with_material");
    let consume = InventoryService::consume(
        &mut state,
        &catalog,
        InventoryMutationRequest {
            operation: consume_operation.clone(),
            source: request_identity(&consume_operation, "craft"),
            owner: PLAYER,
            item: item("material"),
            quantity: 2,
            expected_revision: None,
        },
    )
    .unwrap();
    assert_eq!((consume.before_quantity, consume.after_quantity), (5, 3));

    let transfer_operation = operation("transfer_ammunition");
    let transfer = InventoryService::transfer(
        &mut state,
        &catalog,
        InventoryTransferRequest {
            operation: transfer_operation.clone(),
            source: request_identity(&transfer_operation, "trade"),
            from_owner: PLAYER,
            to_owner: SECOND_OWNER,
            item: item("ammunition"),
            quantity: 4,
            expected_from_revision: None,
            expected_to_revision: None,
        },
    )
    .unwrap();
    assert_eq!((transfer.from_before, transfer.from_after), (15, 11));
    assert_eq!((transfer.to_before, transfer.to_after), (0, 4));
    assert_eq!(
        transfer.committed_from_revision,
        transfer.observed_from_revision + 1
    );
    assert_eq!(
        transfer.committed_to_revision,
        transfer.observed_to_revision + 1
    );
    assert_eq!(usage(&transfer.to_capacity_after, "mass"), 4);

    let before_revision = state.revision();
    let before_player = state
        .component::<InventoryComponent>(PLAYER)
        .unwrap()
        .unwrap()
        .clone();
    let before_second = state
        .component::<InventoryComponent>(SECOND_OWNER)
        .unwrap()
        .unwrap()
        .clone();
    let rejected_operation = operation("over_capacity_material");
    assert!(matches!(
        InventoryService::transfer(
            &mut state,
            &catalog,
            InventoryTransferRequest {
                operation: rejected_operation.clone(),
                source: request_identity(&rejected_operation, "trade"),
                from_owner: PLAYER,
                to_owner: SECOND_OWNER,
                item: item("material"),
                quantity: 3,
                expected_from_revision: None,
                expected_to_revision: None,
            },
        ),
        Err(MechanicsError::InventoryCapacityExceeded {
            owner: SECOND_OWNER,
            ..
        })
    ));
    assert_eq!(state.revision(), before_revision);
    assert_eq!(
        state
            .component::<InventoryComponent>(PLAYER)
            .unwrap()
            .unwrap(),
        &before_player
    );
    assert_eq!(
        state
            .component::<InventoryComponent>(SECOND_OWNER)
            .unwrap()
            .unwrap(),
        &before_second
    );

    let invalid_operation = operation("invalid_inventory_requests");
    assert!(matches!(
        InventoryService::grant(
            &mut state,
            &catalog,
            InventoryMutationRequest {
                operation: invalid_operation.clone(),
                source: request_identity(&invalid_operation, "invalid"),
                owner: PLAYER,
                item: item("missing"),
                quantity: 1,
                expected_revision: None,
            },
        ),
        Err(MechanicsError::UnknownItem { .. })
    ));
    assert!(matches!(
        InventoryService::consume(
            &mut state,
            &catalog,
            InventoryMutationRequest {
                operation: invalid_operation.clone(),
                source: request_identity(&invalid_operation, "invalid"),
                owner: PLAYER,
                item: item("ammunition"),
                quantity: 100,
                expected_revision: None,
            },
        ),
        Err(MechanicsError::InventoryInsufficientQuantity { .. })
    ));
    assert!(matches!(
        InventoryService::grant(
            &mut state,
            &catalog,
            InventoryMutationRequest {
                operation: invalid_operation.clone(),
                source: request_identity(&invalid_operation, "invalid"),
                owner: PLAYER,
                item: item("pistol"),
                quantity: 1,
                expected_revision: None,
            },
        ),
        Err(MechanicsError::InventoryItemKindMismatch { .. })
    ));
    assert!(matches!(
        InventoryService::grant(
            &mut state,
            &catalog,
            InventoryMutationRequest {
                operation: invalid_operation.clone(),
                source: request_identity(&invalid_operation, "invalid"),
                owner: PLAYER,
                item: item("ammunition"),
                quantity: 100,
                expected_revision: None,
            },
        ),
        Err(MechanicsError::InventoryQuantityLimitExceeded { .. })
    ));
    assert_eq!(state.revision(), before_revision);
}

#[test]
fn inventory_projection_is_indexed_immutable_and_snapshot_stable() {
    let catalog = catalog();
    let state = state();
    let view = InventoryService::view(&state, &catalog, PLAYER).unwrap();
    assert_eq!(view.read_cost().containment_entries_visited, 7);
    assert_eq!(view.read_cost().item_components_read, 6);
    assert_eq!(view.read_cost().stack_entries_visited, 2);
    assert_eq!(
        view.unique_items()
            .iter()
            .map(|value| value.entity)
            .collect::<Vec<_>>(),
        vec![RIFLE, PISTOL, SHIELD, ARMOR, MODULE, TRINKET]
    );
    assert!(state.entities().count() > view.read_cost().containment_entries_visited);

    let entity_view = MechanicsEntityView::read(&state, PLAYER).unwrap();
    let inventory = entity_view.inventory().unwrap();
    assert_eq!(inventory.stacks(), view.stacks());
    assert_eq!(inventory.capacity_limits().len(), 2);
    assert!(entity_view.item().is_none());
    assert!(entity_view.equipment().is_some());

    gameplay_mechanics::validate_state_against_catalog(&state, &catalog).unwrap();
    let encoded = encode_snapshot(&state).unwrap();
    let snapshot: serde_json::Value = serde_json::from_str(&encoded).unwrap();
    let inventory_snapshot = snapshot["registeredComponents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|component| component["typeId"] == INVENTORY_COMPONENT_TYPE_ID)
        .unwrap();
    assert_eq!(inventory_snapshot["version"], 2);
    let restored = decode_snapshot_with_catalog(&encoded, &catalog).unwrap();
    let restored_view = InventoryService::view(&restored, &catalog, PLAYER).unwrap();
    assert_eq!(restored_view.stacks(), view.stacks());
    assert_eq!(restored_view.unique_items(), view.unique_items());
    assert_eq!(restored_view.capacity(), view.capacity());
    assert_eq!(restored_view.read_cost(), view.read_cost());
    assert_eq!(
        restored.contained_entities(PLAYER).collect::<Vec<_>>(),
        state.contained_entities(PLAYER).collect::<Vec<_>>()
    );
}

#[test]
fn equipment_enforces_slots_classification_exclusivity_and_atomic_swap() {
    let catalog = catalog();
    let mut state = state();
    let before = state
        .component::<EquipmentComponent>(PLAYER)
        .unwrap()
        .unwrap()
        .clone();
    let before_revision = state.revision();
    let empty_operation = operation("equip_with_no_slots");
    assert!(matches!(
        EquipmentService::equip(
            &mut state,
            &catalog,
            EquipmentEquipRequest {
                operation: empty_operation.clone(),
                source: request_identity(&empty_operation, "loadout"),
                owner: PLAYER,
                item: PISTOL,
                slots: vec![],
                expected_equipment_revision: None,
                expected_state_revision: before_revision,
            },
        ),
        Err(MechanicsError::EquipmentSlotCountMismatch {
            item: PISTOL,
            expected: 1,
            actual: 0,
        })
    ));
    assert_eq!(state.revision(), before_revision);
    assert_eq!(
        state
            .component::<EquipmentComponent>(PLAYER)
            .unwrap()
            .unwrap(),
        &before
    );

    let mismatch_operation = operation("equip_armor_in_hand");
    let mismatch_state_revision = state.revision();
    assert!(matches!(
        EquipmentService::equip(
            &mut state,
            &catalog,
            EquipmentEquipRequest {
                operation: mismatch_operation.clone(),
                source: request_identity(&mismatch_operation, "loadout"),
                owner: PLAYER,
                item: ARMOR,
                slots: vec![slot("hand_left")],
                expected_equipment_revision: None,
                expected_state_revision: mismatch_state_revision,
            },
        ),
        Err(MechanicsError::EquipmentSlotClassificationMismatch { item: ARMOR, .. })
    ));

    let unknown_slot_operation = operation("equip_unknown_slot");
    let unknown_slot_state_revision = state.revision();
    assert!(matches!(
        EquipmentService::equip(
            &mut state,
            &catalog,
            EquipmentEquipRequest {
                operation: unknown_slot_operation.clone(),
                source: request_identity(&unknown_slot_operation, "loadout"),
                owner: PLAYER,
                item: PISTOL,
                slots: vec![slot("missing_slot")],
                expected_equipment_revision: None,
                expected_state_revision: unknown_slot_state_revision,
            },
        ),
        Err(MechanicsError::UnknownEquipmentSlot { .. })
    ));
    assert_eq!(state.revision(), unknown_slot_state_revision);

    let pistol_operation = operation("equip_pistol");
    let pistol_state_revision = state.revision();
    EquipmentService::equip(
        &mut state,
        &catalog,
        EquipmentEquipRequest {
            operation: pistol_operation.clone(),
            source: request_identity(&pistol_operation, "loadout"),
            owner: PLAYER,
            item: PISTOL,
            slots: vec![slot("hand_left")],
            expected_equipment_revision: None,
            expected_state_revision: pistol_state_revision,
        },
    )
    .unwrap();
    let after_pistol_revision = state.revision();

    let shield_operation = operation("equip_exclusive_shield");
    let before_exclusive = state
        .component::<EquipmentComponent>(PLAYER)
        .unwrap()
        .unwrap()
        .clone();
    assert!(matches!(
        EquipmentService::equip(
            &mut state,
            &catalog,
            EquipmentEquipRequest {
                operation: shield_operation.clone(),
                source: request_identity(&shield_operation, "loadout"),
                owner: PLAYER,
                item: SHIELD,
                slots: vec![slot("hand_right")],
                expected_equipment_revision: None,
                expected_state_revision: after_pistol_revision,
            },
        ),
        Err(MechanicsError::EquipmentExclusivityConflict {
            existing: PISTOL,
            requested: SHIELD,
            ..
        })
    ));
    assert_eq!(state.revision(), after_pistol_revision);
    assert_eq!(
        state
            .component::<EquipmentComponent>(PLAYER)
            .unwrap()
            .unwrap(),
        &before_exclusive
    );

    let swap_operation = operation("swap_pistol_for_rifle");
    let swap = EquipmentService::swap(
        &mut state,
        &catalog,
        EquipmentSwapRequest {
            operation: swap_operation.clone(),
            source: request_identity(&swap_operation, "loadout"),
            owner: PLAYER,
            outgoing_item: PISTOL,
            incoming_item: RIFLE,
            incoming_slots: vec![slot("hand_right"), slot("hand_left")],
            expected_equipment_revision: None,
            expected_state_revision: after_pistol_revision,
        },
    )
    .unwrap();
    assert_eq!(swap.replaced_item, Some(PISTOL));
    assert_eq!(swap.changes.len(), 2);
    assert_eq!(swap.source_activations, 1);
    assert_eq!(swap.observed_item_revisions.len(), 1);
    assert_eq!(
        state
            .component::<EquipmentComponent>(PLAYER)
            .unwrap()
            .unwrap()
            .assignments(),
        &[
            EquipmentAssignment {
                slot: slot("hand_left"),
                item: RIFLE,
            },
            EquipmentAssignment {
                slot: slot("hand_right"),
                item: RIFLE,
            },
        ]
    );

    let stale_operation = operation("stale_equip_armor");
    let after_swap = state
        .component::<EquipmentComponent>(PLAYER)
        .unwrap()
        .unwrap()
        .clone();
    assert!(matches!(
        EquipmentService::equip(
            &mut state,
            &catalog,
            EquipmentEquipRequest {
                operation: stale_operation.clone(),
                source: request_identity(&stale_operation, "loadout"),
                owner: PLAYER,
                item: ARMOR,
                slots: vec![slot("body")],
                expected_equipment_revision: None,
                expected_state_revision: after_pistol_revision,
            },
        ),
        Err(MechanicsError::Relationship(_))
    ));
    assert_eq!(
        state
            .component::<EquipmentComponent>(PLAYER)
            .unwrap()
            .unwrap(),
        &after_swap
    );
}

#[test]
fn equipped_item_sources_activate_once_with_item_provenance_for_any_owner_kind() {
    let catalog = catalog();
    let mut state = state();
    let equip_rifle = operation("equip_rifle_sources");
    let expected_state_revision = state.revision();
    EquipmentService::equip(
        &mut state,
        &catalog,
        EquipmentEquipRequest {
            operation: equip_rifle.clone(),
            source: request_identity(&equip_rifle, "loadout"),
            owner: PLAYER,
            item: RIFLE,
            slots: vec![slot("hand_left"), slot("hand_right")],
            expected_equipment_revision: None,
            expected_state_revision,
        },
    )
    .unwrap();
    let evaluation_operation = operation("evaluate_equipped_aim");
    let aim = StatService::evaluate(
        &state,
        &catalog,
        PLAYER,
        &stat("aim"),
        &evaluation_operation,
        &[],
    )
    .unwrap();
    assert_eq!(aim.value, scalar(15));
    assert_eq!(aim.source_cost.equipment_entries_visited, 2);
    assert_eq!(aim.source_cost.item_components_read, 1);
    assert_eq!(
        aim.decisions
            .iter()
            .filter(|decision| {
                matches!(
                    decision.source,
                    SourceInstanceIdentity::EquippedItem {
                        owner: PLAYER,
                        item: RIFLE,
                        ..
                    }
                )
            })
            .count(),
        1
    );

    let equip_armor = operation("equip_armor_sources");
    let expected_state_revision = state.revision();
    EquipmentService::equip(
        &mut state,
        &catalog,
        EquipmentEquipRequest {
            operation: equip_armor.clone(),
            source: request_identity(&equip_armor, "loadout"),
            owner: PLAYER,
            item: ARMOR,
            slots: vec![slot("body")],
            expected_equipment_revision: None,
            expected_state_revision,
        },
    )
    .unwrap();
    let damage_operation = operation("damage_equipped_player");
    let preview = DamageService::preview(
        &state,
        &catalog,
        &DamageRequest {
            operation: damage_operation.clone(),
            source: request_identity(&damage_operation, "impact"),
            actor: None,
            target: PLAYER,
            target_track: track("health"),
            parts: vec![DamagePart {
                amount: scalar(10),
                kind: DamageKindId::parse("impact").unwrap(),
            }],
            request_sources: vec![],
            expected_tracks_revision: None,
        },
    )
    .unwrap();
    assert_eq!(preview.receipt().parts[0].applied, scalar(7));
    assert!(preview.receipt().decisions.iter().any(|decision| {
        matches!(
            decision.source,
            SourceInstanceIdentity::EquippedItem {
                owner: PLAYER,
                item: ARMOR,
                ..
            }
        ) && decision.source_definition == source("guard")
    }));

    let equip_module = operation("equip_building_module");
    let expected_state_revision = state.revision();
    EquipmentService::equip(
        &mut state,
        &catalog,
        EquipmentEquipRequest {
            operation: equip_module.clone(),
            source: request_identity(&equip_module, "construction"),
            owner: BUILDING,
            item: BUILDING_MODULE,
            slots: vec![slot("module")],
            expected_equipment_revision: None,
            expected_state_revision,
        },
    )
    .unwrap();
    let output_operation = operation("evaluate_building_output");
    let output = StatService::evaluate(
        &state,
        &catalog,
        BUILDING,
        &stat("output"),
        &output_operation,
        &[],
    )
    .unwrap();
    assert_eq!(output.value, scalar(9));
    assert!(output.decisions.iter().any(|decision| {
        matches!(
            decision.source,
            SourceInstanceIdentity::EquippedItem {
                owner: BUILDING,
                item: BUILDING_MODULE,
                ..
            }
        )
    }));

    let unequip_rifle = operation("unequip_rifle_sources");
    let expected_state_revision = state.revision();
    EquipmentService::unequip(
        &mut state,
        &catalog,
        EquipmentUnequipRequest {
            operation: unequip_rifle.clone(),
            source: request_identity(&unequip_rifle, "loadout"),
            owner: PLAYER,
            item: RIFLE,
            expected_equipment_revision: None,
            expected_state_revision,
        },
    )
    .unwrap();
    let after_operation = operation("evaluate_unequipped_aim");
    let after = StatService::evaluate(
        &state,
        &catalog,
        PLAYER,
        &stat("aim"),
        &after_operation,
        &[],
    )
    .unwrap();
    assert_eq!(after.value, scalar(10));
    assert!(!after.decisions.iter().any(|decision| {
        matches!(
            decision.source,
            SourceInstanceIdentity::EquippedItem {
                owner: PLAYER,
                item: RIFLE,
                ..
            }
        )
    }));
}

#[test]
fn equipment_source_quota_is_preflighted_before_the_ninth_bundle_mutates() {
    let mut definition = catalog_definition();
    let bundle_sources = (0..32)
        .map(|index| source(&format!("bundle_source_{index:02}")))
        .collect::<Vec<_>>();
    definition
        .sources
        .extend(bundle_sources.iter().cloned().map(|id| SourceDefinition {
            id,
            priority: 0,
            stat_contributions: vec![],
            damage_responses: vec![],
        }));
    definition.items.push(ItemDefinition {
        id: item("source_bundle"),
        kind: ItemKind::Unique,
        maximum_quantity: 1,
        classifications: vec![classification("module")],
        capacity_costs: vec![capacity_cost("power", 1)],
        equipment: Some(ItemEquipmentPolicy {
            required_slots: 1,
            exclusive_group: None,
        }),
        sources: bundle_sources,
    });
    definition
        .equipment_slots
        .extend((0..9).map(|index| EquipmentSlotDefinition {
            id: slot(&format!("quota_slot_{index}")),
            allowed_classifications: vec![classification("module")],
        }));
    let catalog = MechanicsCatalog::admit(definition).unwrap();

    let owner = EntityId::new(3_000);
    let bundle_entities = (0..9)
        .map(|index| EntityId::new(3_010 + index))
        .collect::<Vec<_>>();
    let mut definitions = vec![EntityDefinition::new(owner, "quota-owner")];
    definitions.extend(bundle_entities.iter().enumerate().map(|(index, entity)| {
        EntityDefinition::new(*entity, format!("bundle-{index}")).with_containment(owner)
    }));
    let registry = gameplay_mechanics::gameplay_component_registry().unwrap();
    let mut state = EntityState::from_definitions_with_registry(registry, definitions).unwrap();
    attach(
        &mut state,
        owner,
        EquipmentComponent::new(version(), vec![]).unwrap(),
    );
    for entity in &bundle_entities {
        attach(
            &mut state,
            *entity,
            ItemComponent::new(version(), item("source_bundle")),
        );
    }

    for (index, entity) in bundle_entities.iter().take(8).enumerate() {
        let operation = operation(&format!("equip_bundle_{index}"));
        let expected_state_revision = state.revision();
        let receipt = EquipmentService::equip(
            &mut state,
            &catalog,
            EquipmentEquipRequest {
                operation: operation.clone(),
                source: request_identity(&operation, "quota_loadout"),
                owner,
                item: *entity,
                slots: vec![slot(&format!("quota_slot_{index}"))],
                expected_equipment_revision: None,
                expected_state_revision,
            },
        )
        .unwrap();
        assert_eq!(receipt.source_activations, (index + 1) * 32);
    }
    assert_eq!(
        state
            .component::<EquipmentComponent>(owner)
            .unwrap()
            .unwrap()
            .assignments()
            .len(),
        8
    );
    let before_revision = state.revision();
    let before_equipment = state
        .component::<EquipmentComponent>(owner)
        .unwrap()
        .unwrap()
        .clone();
    let operation = operation("equip_ninth_source_bundle");
    assert!(matches!(
        EquipmentService::equip(
            &mut state,
            &catalog,
            EquipmentEquipRequest {
                operation: operation.clone(),
                source: request_identity(&operation, "quota_loadout"),
                owner,
                item: bundle_entities[8],
                slots: vec![slot("quota_slot_8")],
                expected_equipment_revision: None,
                expected_state_revision: before_revision,
            },
        ),
        Err(MechanicsError::EquipmentSourceQuotaExceeded {
            actual: 288,
            maximum: MAX_EQUIPMENT_SOURCE_ACTIVATIONS,
        })
    ));
    assert_eq!(state.revision(), before_revision);
    assert_eq!(
        state
            .component::<EquipmentComponent>(owner)
            .unwrap()
            .unwrap(),
        &before_equipment
    );
}

#[test]
fn unique_transfer_requires_unequip_and_enforces_target_capacity_before_commit() {
    let catalog = catalog();
    let mut state = state();
    let equip_operation = operation("equip_pistol_for_transfer");
    let expected_state_revision = state.revision();
    EquipmentService::equip(
        &mut state,
        &catalog,
        EquipmentEquipRequest {
            operation: equip_operation.clone(),
            source: request_identity(&equip_operation, "loadout"),
            owner: PLAYER,
            item: PISTOL,
            slots: vec![slot("hand_left")],
            expected_equipment_revision: None,
            expected_state_revision,
        },
    )
    .unwrap();
    let equipped_revision = state.revision();
    let blocked_operation = operation("transfer_equipped_pistol");
    assert!(matches!(
        EquipmentService::transfer_unique_item(
            &mut state,
            &catalog,
            ItemTransferRequest {
                operation: blocked_operation.clone(),
                source: request_identity(&blocked_operation, "trade"),
                item: PISTOL,
                from_owner: PLAYER,
                to_owner: SECOND_OWNER,
                expected_relationship_revision: equipped_revision,
                expected_from_inventory_revision: None,
                expected_to_inventory_revision: None,
            },
        ),
        Err(MechanicsError::ItemEquipped { item: PISTOL, .. })
    ));
    assert_eq!(state.revision(), equipped_revision);
    assert_eq!(state.contained_in(PISTOL), Some(PLAYER));

    let unequip_operation = operation("unequip_pistol_for_transfer");
    EquipmentService::unequip(
        &mut state,
        &catalog,
        EquipmentUnequipRequest {
            operation: unequip_operation.clone(),
            source: request_identity(&unequip_operation, "loadout"),
            owner: PLAYER,
            item: PISTOL,
            expected_equipment_revision: None,
            expected_state_revision: equipped_revision,
        },
    )
    .unwrap();
    let from_inventory_revision = state
        .component_revision::<InventoryComponent>(PLAYER)
        .unwrap();
    let to_inventory_revision = state
        .component_revision::<InventoryComponent>(SECOND_OWNER)
        .unwrap();
    let transfer_operation = operation("transfer_unequipped_pistol");
    let relationship_revision = state.revision();
    let transfer = EquipmentService::transfer_unique_item(
        &mut state,
        &catalog,
        ItemTransferRequest {
            operation: transfer_operation.clone(),
            source: request_identity(&transfer_operation, "trade"),
            item: PISTOL,
            from_owner: PLAYER,
            to_owner: SECOND_OWNER,
            expected_relationship_revision: relationship_revision,
            expected_from_inventory_revision: Some(from_inventory_revision.clone()),
            expected_to_inventory_revision: Some(to_inventory_revision.clone()),
        },
    )
    .unwrap();
    assert_eq!(state.contained_in(PISTOL), Some(SECOND_OWNER));
    assert_eq!(usage(&transfer.to_capacity_after, "mass"), 5);
    assert_eq!(
        state
            .component_revision::<InventoryComponent>(PLAYER)
            .unwrap(),
        from_inventory_revision
    );
    assert_eq!(
        state
            .component_revision::<InventoryComponent>(SECOND_OWNER)
            .unwrap(),
        to_inventory_revision
    );

    let before_capacity_failure = state.revision();
    let rejected_operation = operation("transfer_armor_over_capacity");
    assert!(matches!(
        EquipmentService::transfer_unique_item(
            &mut state,
            &catalog,
            ItemTransferRequest {
                operation: rejected_operation.clone(),
                source: request_identity(&rejected_operation, "trade"),
                item: ARMOR,
                from_owner: PLAYER,
                to_owner: SECOND_OWNER,
                expected_relationship_revision: before_capacity_failure,
                expected_from_inventory_revision: None,
                expected_to_inventory_revision: None,
            },
        ),
        Err(MechanicsError::InventoryCapacityExceeded {
            owner: SECOND_OWNER,
            ..
        })
    ));
    assert_eq!(state.revision(), before_capacity_failure);
    assert_eq!(state.contained_in(ARMOR), Some(PLAYER));
    assert_eq!(state.contained_in(PISTOL), Some(SECOND_OWNER));

    let wrong_owner_operation = operation("transfer_from_wrong_owner");
    assert!(matches!(
        EquipmentService::transfer_unique_item(
            &mut state,
            &catalog,
            ItemTransferRequest {
                operation: wrong_owner_operation.clone(),
                source: request_identity(&wrong_owner_operation, "trade"),
                item: PISTOL,
                from_owner: PLAYER,
                to_owner: BUILDING,
                expected_relationship_revision: before_capacity_failure,
                expected_from_inventory_revision: None,
                expected_to_inventory_revision: None,
            },
        ),
        Err(MechanicsError::ItemNotContained {
            actual_owner: Some(SECOND_OWNER),
            ..
        })
    ));
    assert_eq!(state.revision(), before_capacity_failure);
}

#[test]
fn unique_transfer_preflights_the_direct_containment_quota_without_scanning_or_mutating() {
    let catalog = catalog();
    let mut state = state();
    let expected_state_revision = state.revision();
    EntityAuthoringService
        .admit(
            &mut state,
            expected_state_revision,
            (0..MAX_CONTAINED_ENTITIES_PER_INVENTORY).map(|index| {
                EntityDefinition::new(
                    EntityId::new(4_000 + index as u64),
                    format!("contained-marker-{index}"),
                )
                .with_containment(SECOND_OWNER)
            }),
        )
        .unwrap();
    assert_eq!(
        state.contained_entity_count(SECOND_OWNER),
        MAX_CONTAINED_ENTITIES_PER_INVENTORY
    );
    let before_revision = state.revision();
    let operation = operation("transfer_over_containment_quota");
    assert!(matches!(
        EquipmentService::transfer_unique_item(
            &mut state,
            &catalog,
            ItemTransferRequest {
                operation: operation.clone(),
                source: request_identity(&operation, "trade"),
                item: PISTOL,
                from_owner: PLAYER,
                to_owner: SECOND_OWNER,
                expected_relationship_revision: before_revision,
                expected_from_inventory_revision: None,
                expected_to_inventory_revision: None,
            },
        ),
        Err(MechanicsError::InventoryContainmentQuotaExceeded {
            owner: SECOND_OWNER,
            actual,
            maximum: MAX_CONTAINED_ENTITIES_PER_INVENTORY,
        }) if actual == MAX_CONTAINED_ENTITIES_PER_INVENTORY + 1
    ));
    assert_eq!(state.revision(), before_revision);
    assert_eq!(state.contained_in(PISTOL), Some(PLAYER));
}

#[test]
fn item_and_owner_lifecycle_leave_no_dangling_equipment_or_containment() {
    let catalog = catalog();
    let mut item_state = state();
    let spend_operation = operation("damage_rifle_durability");
    let spent = TrackService::spend(
        &mut item_state,
        &catalog,
        TrackMutationRequest {
            operation: spend_operation.clone(),
            source: request_identity(&spend_operation, "wear"),
            entity: RIFLE,
            track: track("durability"),
            amount: scalar(10),
            kind: TrackAdjustmentKind::Spend,
            expected_revision: None,
        },
    )
    .unwrap();
    assert_eq!(spent.after, scalar(50));
    assert_eq!(
        item_state
            .component::<ActiveEffectsComponent>(RIFLE)
            .unwrap()
            .unwrap()
            .effects()
            .len(),
        1
    );

    let equip_operation = operation("equip_rifle_before_destroy");
    let expected_state_revision = item_state.revision();
    EquipmentService::equip(
        &mut item_state,
        &catalog,
        EquipmentEquipRequest {
            operation: equip_operation.clone(),
            source: request_identity(&equip_operation, "loadout"),
            owner: PLAYER,
            item: RIFLE,
            slots: vec![slot("hand_left"), slot("hand_right")],
            expected_equipment_revision: None,
            expected_state_revision,
        },
    )
    .unwrap();
    let destroy_operation = operation("destroy_equipped_rifle");
    let equipped_revision = item_state.revision();
    assert!(matches!(
        ItemService::destroy_unique(
            &mut item_state,
            &catalog,
            ItemDestroyRequest {
                operation: destroy_operation.clone(),
                source: request_identity(&destroy_operation, "destruction"),
                item: RIFLE,
                expected_state_revision: equipped_revision,
            },
        ),
        Err(MechanicsError::ItemEquipped { item: RIFLE, .. })
    ));
    assert!(item_state.is_alive(RIFLE));
    assert_eq!(item_state.revision(), equipped_revision);

    let unequip_operation = operation("unequip_rifle_before_destroy");
    EquipmentService::unequip(
        &mut item_state,
        &catalog,
        EquipmentUnequipRequest {
            operation: unequip_operation.clone(),
            source: request_identity(&unequip_operation, "loadout"),
            owner: PLAYER,
            item: RIFLE,
            expected_equipment_revision: None,
            expected_state_revision: equipped_revision,
        },
    )
    .unwrap();
    let destroy_operation = operation("destroy_unequipped_rifle");
    let expected_state_revision = item_state.revision();
    let destroyed = ItemService::destroy_unique(
        &mut item_state,
        &catalog,
        ItemDestroyRequest {
            operation: destroy_operation.clone(),
            source: request_identity(&destroy_operation, "destruction"),
            item: RIFLE,
            expected_state_revision,
        },
    )
    .unwrap();
    assert_eq!(destroyed.former_owner, Some(PLAYER));
    assert!(!item_state.is_alive(RIFLE));
    assert_eq!(item_state.contained_in(RIFLE), None);
    assert!(!item_state
        .contained_entities(PLAYER)
        .any(|item| item == RIFLE));

    let mut owner_state = state();
    let equip_armor = operation("equip_armor_before_owner_destroy");
    let expected_state_revision = owner_state.revision();
    EquipmentService::equip(
        &mut owner_state,
        &catalog,
        EquipmentEquipRequest {
            operation: equip_armor.clone(),
            source: request_identity(&equip_armor, "loadout"),
            owner: PLAYER,
            item: ARMOR,
            slots: vec![slot("body")],
            expected_equipment_revision: None,
            expected_state_revision,
        },
    )
    .unwrap();
    let expected_state_revision = owner_state.revision();
    EntityAuthoringService
        .destroy(&mut owner_state, expected_state_revision, PLAYER)
        .unwrap();
    assert!(!owner_state.is_alive(PLAYER));
    assert_eq!(owner_state.contained_entity_count(PLAYER), 0);
    assert_eq!(owner_state.contained_in(ARMOR), None);
    assert!(owner_state.is_alive(ARMOR));
    assert!(owner_state
        .component::<EquipmentComponent>(PLAYER)
        .unwrap()
        .is_none());
}

#[test]
fn gm4_catalog_and_snapshot_validation_reject_invalid_structural_references() {
    let mut fungible_source = catalog_definition();
    fungible_source
        .items
        .iter_mut()
        .find(|definition| definition.id == item("ammunition"))
        .unwrap()
        .sources = vec![source("precision")];
    assert!(matches!(
        MechanicsCatalog::admit(fungible_source),
        Err(CatalogError::InvalidItemPolicy { .. })
    ));

    let mut missing_metric = catalog_definition();
    missing_metric
        .items
        .iter_mut()
        .find(|definition| definition.id == item("material"))
        .unwrap()
        .capacity_costs = vec![capacity_cost("missing_metric", 1)];
    assert!(matches!(
        MechanicsCatalog::admit(missing_metric),
        Err(CatalogError::UnknownReference {
            namespace: "capacity metric",
            ..
        })
    ));

    let catalog = catalog();
    let mut state = state();
    let invalid_inventory = InventoryComponent::with_capacity_limits(
        version(),
        vec![ItemStack {
            definition: item("ammunition"),
            quantity: 1,
        }],
        vec![InventoryCapacityLimit::new(capacity("missing_metric"), 10)],
    )
    .unwrap();
    let revision = state
        .component_revision::<InventoryComponent>(SECOND_OWNER)
        .unwrap();
    EntityAuthoringService
        .replace_component(&mut state, revision, SECOND_OWNER, invalid_inventory)
        .unwrap();
    assert!(matches!(
        gameplay_mechanics::validate_state_against_catalog(&state, &catalog),
        Err(MechanicsError::InvalidCatalogReference {
            namespace: "capacity metric",
            ..
        })
    ));
    let encoded = encode_snapshot(&state).unwrap();
    assert!(matches!(
        decode_snapshot_with_catalog(&encoded, &catalog),
        Err(MechanicsSnapshotError::Mechanics(
            MechanicsError::InvalidCatalogReference {
                namespace: "capacity metric",
                ..
            }
        ))
    ));
}
