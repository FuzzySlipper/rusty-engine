use core_ids::EntityId;
use entity_state::{
    encode_snapshot, ComponentRegistration, ComponentTypeId, EntityAuthoringService,
    EntityComponent, EntityDefinition, EntityState, EntityStateSnapshotError,
    RegisteredComponentSnapshotError,
};
use gameplay_mechanics::{
    decode_snapshot_with_catalog, decode_snapshot_with_catalog_and_registry,
    gameplay_component_registry, ActiveEffectInstance, ActiveEffectsComponent,
    CapacityMetricDefinition, CapacityMetricId, CatalogVersion, DamageKindDefinition, DamageKindId,
    DamageKindSelector, DamagePart, DamageRequest, DamageResponseDefinition, DamageService,
    EffectDefinition, EffectDefinitionId, EffectInstanceId, EffectStackingPolicy,
    EquipmentAssignment, EquipmentComponent, EquipmentSlotDefinition, EquipmentSlotId,
    IntrinsicSourceBinding, IntrinsicSourcesComponent, InventoryCapacityLimit, InventoryComponent,
    InventoryService, ItemCapacityCost, ItemClassificationId, ItemComponent, ItemDefinition,
    ItemDefinitionId, ItemEquipmentPolicy, ItemKind, ItemStack, MechanicsCatalog,
    MechanicsCatalogDefinition, MechanicsComponentDataError, MechanicsComponentKind,
    MechanicsError, MechanicsScalar, MechanicsSnapshotError, OperationId, SourceDefinition,
    SourceDefinitionId, SourceInstanceId, SourceInstanceIdentity, StackingGroupId, StackingPolicy,
    StatContribution, StatContributionDefinition, StatDefinition, StatId, StatService, StatValue,
    StatsComponent, TrackDefinition, TrackId, TrackMaximum, TrackValue, TracksComponent,
    ACTIVE_EFFECTS_COMPONENT_TYPE_ID, EQUIPMENT_COMPONENT_TYPE_ID,
    INTRINSIC_SOURCES_COMPONENT_TYPE_ID, INVENTORY_COMPONENT_TYPE_ID, ITEM_COMPONENT_TYPE_ID,
    MAX_DAMAGE_PARTS, MAX_STATS_PER_ENTITY, STATS_COMPONENT_TYPE_ID, TRACKS_COMPONENT_TYPE_ID,
};

const OWNER: EntityId = EntityId::new(1);
const ARMOR: EntityId = EntityId::new(2);
const SIMPLE: EntityId = EntityId::new(10_000);

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeMarker;

impl EntityComponent for RuntimeMarker {}

fn scalar(value: i64) -> MechanicsScalar {
    MechanicsScalar::new(value).unwrap()
}

fn version() -> CatalogVersion {
    CatalogVersion::parse("gm5.v1").unwrap()
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

fn item(value: &str) -> ItemDefinitionId {
    ItemDefinitionId::parse(value).unwrap()
}

fn operation(value: &str) -> OperationId {
    OperationId::parse(value).unwrap()
}

fn request_identity(operation: &OperationId, value: &str) -> SourceInstanceIdentity {
    SourceInstanceIdentity::Request {
        operation: operation.clone(),
        instance: SourceInstanceId::parse(value).unwrap(),
    }
}

fn catalog_definition() -> MechanicsCatalogDefinition {
    MechanicsCatalogDefinition {
        version: version(),
        stats: vec![StatDefinition {
            id: stat("power"),
            minimum: scalar(1),
            maximum: scalar(100),
        }],
        tracks: vec![
            TrackDefinition {
                id: track("health"),
                minimum: scalar(0),
                maximum: TrackMaximum::Stat {
                    stat: stat("power"),
                },
            },
            TrackDefinition {
                id: track("shield"),
                minimum: scalar(0),
                maximum: TrackMaximum::Fixed { value: scalar(50) },
            },
        ],
        sources: vec![
            SourceDefinition {
                id: source("armor"),
                priority: 0,
                stat_contributions: vec![StatContributionDefinition {
                    stat: stat("power"),
                    contribution: StatContribution::Add { amount: scalar(5) },
                    stacking_group: StackingGroupId::parse("power_bonus").unwrap(),
                    stacking: StackingPolicy::Highest,
                }],
                damage_responses: vec![DamageResponseDefinition::FlatReduction {
                    selector: DamageKindSelector::Any,
                    amount: scalar(2),
                    stacking_group: StackingGroupId::parse("armor_reduction").unwrap(),
                    stacking: StackingPolicy::Highest,
                }],
            },
            SourceDefinition {
                id: source("ward"),
                priority: -1,
                stat_contributions: vec![],
                damage_responses: vec![DamageResponseDefinition::Prevent {
                    selector: DamageKindSelector::Any,
                    stacking_group: StackingGroupId::parse("ward_prevention").unwrap(),
                    stacking: StackingPolicy::UniqueBySource,
                }],
            },
        ],
        damage_kinds: vec![DamageKindDefinition {
            id: DamageKindId::parse("kinetic").unwrap(),
        }],
        effects: vec![EffectDefinition {
            id: EffectDefinitionId::parse("ward").unwrap(),
            stacking_group: StackingGroupId::parse("ward_lifecycle").unwrap(),
            stacking: EffectStackingPolicy::Refresh,
            maximum_stacks: 1,
            sources: vec![source("ward")],
        }],
        capacity_metrics: vec![CapacityMetricDefinition {
            id: CapacityMetricId::parse("mass").unwrap(),
        }],
        items: vec![
            ItemDefinition {
                id: item("ammunition"),
                kind: ItemKind::Fungible,
                maximum_quantity: 100,
                classifications: vec![],
                capacity_costs: vec![ItemCapacityCost {
                    metric: CapacityMetricId::parse("mass").unwrap(),
                    units: 1,
                }],
                equipment: None,
                sources: vec![],
            },
            ItemDefinition {
                id: item("armor"),
                kind: ItemKind::Unique,
                maximum_quantity: 1,
                classifications: vec![ItemClassificationId::parse("armor").unwrap()],
                capacity_costs: vec![ItemCapacityCost {
                    metric: CapacityMetricId::parse("mass").unwrap(),
                    units: 10,
                }],
                equipment: Some(ItemEquipmentPolicy {
                    required_slots: 1,
                    exclusive_group: None,
                }),
                sources: vec![source("armor")],
            },
        ],
        equipment_slots: vec![EquipmentSlotDefinition {
            id: EquipmentSlotId::parse("body").unwrap(),
            allowed_classifications: vec![ItemClassificationId::parse("armor").unwrap()],
        }],
    }
}

fn catalog() -> MechanicsCatalog {
    MechanicsCatalog::admit(catalog_definition()).unwrap()
}

fn registry() -> entity_state::ComponentRegistry {
    let mut registry = gameplay_component_registry().unwrap();
    registry
        .register(ComponentRegistration::<RuntimeMarker>::runtime_only(
            ComponentTypeId::parse("fixture.runtime-marker").unwrap(),
        ))
        .unwrap();
    registry
}

fn attach<T: EntityComponent>(state: &mut EntityState, entity: EntityId, value: T) {
    let revision = state.component_revision::<T>(entity).unwrap();
    EntityAuthoringService
        .attach_component(state, revision, entity, value)
        .unwrap();
}

fn full_state() -> EntityState {
    let mut state = EntityState::from_definitions_with_registry(
        registry(),
        [
            EntityDefinition::new(OWNER, "owner"),
            EntityDefinition::new(ARMOR, "armor").with_containment(OWNER),
        ],
    )
    .unwrap();
    attach(
        &mut state,
        OWNER,
        StatsComponent::new(version(), vec![StatValue::new(stat("power"), scalar(100))]).unwrap(),
    );
    attach(
        &mut state,
        OWNER,
        TracksComponent::new(
            version(),
            vec![
                TrackValue::new(track("health"), scalar(90)),
                TrackValue::new(track("shield"), scalar(20)),
            ],
        )
        .unwrap(),
    );
    attach(
        &mut state,
        OWNER,
        IntrinsicSourcesComponent::new(
            version(),
            vec![IntrinsicSourceBinding::new(
                SourceInstanceId::parse("trained").unwrap(),
                source("armor"),
            )],
        )
        .unwrap(),
    );
    let ward_operation = operation("seed_ward");
    attach(
        &mut state,
        OWNER,
        ActiveEffectsComponent::new(
            version(),
            vec![ActiveEffectInstance::new(
                EffectInstanceId::parse("ward_one").unwrap(),
                EffectDefinitionId::parse("ward").unwrap(),
                request_identity(&ward_operation, "caster"),
                1,
            )
            .unwrap()],
        )
        .unwrap(),
    );
    attach(
        &mut state,
        OWNER,
        InventoryComponent::with_capacity_limits(
            version(),
            vec![ItemStack {
                definition: item("ammunition"),
                quantity: 5,
            }],
            vec![InventoryCapacityLimit::new(
                CapacityMetricId::parse("mass").unwrap(),
                50,
            )],
        )
        .unwrap(),
    );
    attach(
        &mut state,
        ARMOR,
        ItemComponent::new(version(), item("armor")),
    );
    attach(
        &mut state,
        OWNER,
        EquipmentComponent::new(
            version(),
            vec![EquipmentAssignment {
                slot: EquipmentSlotId::parse("body").unwrap(),
                item: ARMOR,
            }],
        )
        .unwrap(),
    );
    attach(&mut state, OWNER, RuntimeMarker);
    state
}

fn damage_request(operation_name: &str) -> DamageRequest {
    let operation = operation(operation_name);
    DamageRequest {
        source: request_identity(&operation, "origin"),
        operation,
        actor: None,
        target: OWNER,
        target_track: track("health"),
        parts: vec![DamagePart {
            amount: scalar(10),
            kind: DamageKindId::parse("kinetic").unwrap(),
        }],
        request_sources: vec![],
        expected_tracks_revision: None,
    }
}

fn component<'a>(snapshot: &'a mut serde_json::Value, type_id: &str) -> &'a mut serde_json::Value {
    snapshot["registeredComponents"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|component| component["typeId"] == type_id)
        .unwrap()
}

#[test]
fn public_registration_metadata_is_stable_and_every_mechanics_component_is_durable() {
    let registry = gameplay_component_registry().unwrap();
    let state = EntityState::with_registry(registry);
    let inspection = state.component_inspection();
    assert_eq!(MechanicsComponentKind::ALL.len(), 7);
    for kind in MechanicsComponentKind::ALL {
        let registered = inspection
            .kinds
            .iter()
            .find(|registered| registered.type_id.as_str() == kind.type_id())
            .unwrap();
        assert_eq!(
            registered.persistence,
            entity_state::ComponentPersistence::Durable {
                version: kind.codec_version()
            }
        );
        assert!(kind.codec_id().starts_with("rusty.mechanics."));
    }
}

#[test]
fn fresh_consumer_round_trips_all_components_and_continues_with_the_same_outcome() {
    let catalog = catalog();
    let mut original = full_state();
    let before = encode_snapshot(&original).unwrap();
    assert!(!before.contains("fixture.runtime-marker"));

    let mut restored =
        decode_snapshot_with_catalog_and_registry(&before, registry(), &catalog).unwrap();
    assert_eq!(encode_snapshot(&restored).unwrap(), before);
    assert!(restored
        .component::<RuntimeMarker>(OWNER)
        .unwrap()
        .is_none());
    assert_eq!(
        InventoryService::view(&restored, &catalog, OWNER)
            .unwrap()
            .unique_items()[0]
            .entity,
        ARMOR
    );

    let original_receipt =
        DamageService::apply(&mut original, &catalog, damage_request("continued_hit")).unwrap();
    let restored_receipt =
        DamageService::apply(&mut restored, &catalog, damage_request("continued_hit")).unwrap();
    assert_eq!(original_receipt.parts, restored_receipt.parts);
    assert_eq!(original_receipt.decisions, restored_receipt.decisions);
    assert_eq!(
        original.component::<TracksComponent>(OWNER).unwrap(),
        restored.component::<TracksComponent>(OWNER).unwrap()
    );
    assert_eq!(
        encode_snapshot(&original).unwrap(),
        encode_snapshot(&restored).unwrap()
    );
}

#[test]
fn every_codec_is_strict_and_snapshot_structure_rejects_unknown_and_duplicate_values() {
    let catalog = catalog();
    let encoded = encode_snapshot(&full_state()).unwrap();
    let canonical: serde_json::Value = serde_json::from_str(&encoded).unwrap();

    for type_id in [
        STATS_COMPONENT_TYPE_ID,
        TRACKS_COMPONENT_TYPE_ID,
        INTRINSIC_SOURCES_COMPONENT_TYPE_ID,
        ACTIVE_EFFECTS_COMPONENT_TYPE_ID,
        INVENTORY_COMPONENT_TYPE_ID,
        ITEM_COMPONENT_TYPE_ID,
        EQUIPMENT_COMPONENT_TYPE_ID,
    ] {
        let mut invalid = canonical.clone();
        component(&mut invalid, type_id)["values"][0]["value"]["unexpected"] =
            serde_json::json!(true);
        assert!(matches!(
            decode_snapshot_with_catalog(&invalid.to_string(), &catalog),
            Err(MechanicsSnapshotError::EntityState(
                EntityStateSnapshotError::RegisteredComponent(
                    RegisteredComponentSnapshotError::DecodeFailed { .. }
                )
            ))
        ));
    }

    for kind in MechanicsComponentKind::ALL {
        let mut incompatible = canonical.clone();
        component(&mut incompatible, kind.type_id())["version"] =
            serde_json::json!(kind.codec_version() + 1);
        assert!(matches!(
            decode_snapshot_with_catalog(&incompatible.to_string(), &catalog),
            Err(MechanicsSnapshotError::EntityState(
                EntityStateSnapshotError::RegisteredComponent(
                    RegisteredComponentSnapshotError::CodecMismatch { .. }
                )
            ))
        ));
    }

    let mut unknown_required = canonical.clone();
    unknown_required["registeredComponents"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "typeId": "fixture.unknown",
            "codec": "fixture.unknown-json",
            "version": 1,
            "required": true,
            "values": []
        }));
    assert!(matches!(
        decode_snapshot_with_catalog(&unknown_required.to_string(), &catalog),
        Err(MechanicsSnapshotError::EntityState(
            EntityStateSnapshotError::RegisteredComponent(
                RegisteredComponentSnapshotError::UnknownRequiredType { .. }
            )
        ))
    ));

    let mut unknown_optional = canonical.clone();
    unknown_optional["registeredComponents"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "typeId": "fixture.optional",
            "codec": "fixture.optional-json",
            "version": 1,
            "required": false,
            "values": [{"entity": OWNER.raw(), "value": {"anything": true}}]
        }));
    decode_snapshot_with_catalog(&unknown_optional.to_string(), &catalog).unwrap();

    let mut duplicate_type = canonical.clone();
    let duplicate = duplicate_type["registeredComponents"][0].clone();
    duplicate_type["registeredComponents"]
        .as_array_mut()
        .unwrap()
        .push(duplicate);
    assert!(matches!(
        decode_snapshot_with_catalog(&duplicate_type.to_string(), &catalog),
        Err(MechanicsSnapshotError::EntityState(
            EntityStateSnapshotError::RegisteredComponent(
                RegisteredComponentSnapshotError::DuplicateType { .. }
            )
        ))
    ));

    let mut duplicate_value = canonical.clone();
    let duplicate = component(&mut duplicate_value, STATS_COMPONENT_TYPE_ID)["values"][0].clone();
    component(&mut duplicate_value, STATS_COMPONENT_TYPE_ID)["values"]
        .as_array_mut()
        .unwrap()
        .push(duplicate);
    assert!(matches!(
        decode_snapshot_with_catalog(&duplicate_value.to_string(), &catalog),
        Err(MechanicsSnapshotError::EntityState(
            EntityStateSnapshotError::RegisteredComponent(
                RegisteredComponentSnapshotError::DuplicateEntityValue { .. }
            )
        ))
    ));

    assert_eq!(
        encode_snapshot(&decode_snapshot_with_catalog(&encoded, &catalog).unwrap()).unwrap(),
        encoded
    );
}

#[test]
fn every_component_reference_and_catalog_version_is_checked_after_decode() {
    let catalog = catalog();
    let canonical: serde_json::Value =
        serde_json::from_str(&encode_snapshot(&full_state()).unwrap()).unwrap();
    let unresolved_paths = [
        (STATS_COMPONENT_TYPE_ID, "values", 0, "stat"),
        (TRACKS_COMPONENT_TYPE_ID, "values", 0, "track"),
        (
            INTRINSIC_SOURCES_COMPONENT_TYPE_ID,
            "bindings",
            0,
            "definition",
        ),
        (ACTIVE_EFFECTS_COMPONENT_TYPE_ID, "effects", 0, "definition"),
        (INVENTORY_COMPONENT_TYPE_ID, "stacks", 0, "definition"),
        (ITEM_COMPONENT_TYPE_ID, "", 0, "definition"),
        (EQUIPMENT_COMPONENT_TYPE_ID, "assignments", 0, "slot"),
    ];
    for (type_id, collection, index, field) in unresolved_paths {
        let mut invalid = canonical.clone();
        let value = &mut component(&mut invalid, type_id)["values"][0]["value"];
        if collection.is_empty() {
            value[field] = serde_json::json!("missing");
        } else {
            value[collection][index][field] = serde_json::json!("missing");
        }
        assert!(matches!(
            decode_snapshot_with_catalog(&invalid.to_string(), &catalog),
            Err(MechanicsSnapshotError::Mechanics(
                MechanicsError::InvalidCatalogReference { .. }
                    | MechanicsError::UnknownSource { .. }
                    | MechanicsError::UnknownEffect { .. }
                    | MechanicsError::UnknownTrack { .. }
            ))
        ));
    }

    for type_id in [
        STATS_COMPONENT_TYPE_ID,
        TRACKS_COMPONENT_TYPE_ID,
        INTRINSIC_SOURCES_COMPONENT_TYPE_ID,
        ACTIVE_EFFECTS_COMPONENT_TYPE_ID,
        INVENTORY_COMPONENT_TYPE_ID,
        ITEM_COMPONENT_TYPE_ID,
        EQUIPMENT_COMPONENT_TYPE_ID,
    ] {
        let mut incompatible = canonical.clone();
        component(&mut incompatible, type_id)["values"][0]["value"]["catalogVersion"] =
            serde_json::json!("gm5.v2");
        assert!(matches!(
            decode_snapshot_with_catalog(&incompatible.to_string(), &catalog),
            Err(MechanicsSnapshotError::Mechanics(
                MechanicsError::CatalogVersionMismatch { .. }
            ))
        ));
    }

    let mut balance_change = catalog_definition();
    balance_change.damage_kinds.push(DamageKindDefinition {
        id: DamageKindId::parse("thermal").unwrap(),
    });
    let balance_change = MechanicsCatalog::admit(balance_change).unwrap();
    assert_ne!(catalog.fingerprint(), balance_change.fingerprint());
    decode_snapshot_with_catalog(&canonical.to_string(), &balance_change).unwrap();
}

#[test]
fn unrelated_entities_do_not_amplify_the_simple_stat_or_damage_path() {
    let catalog = MechanicsCatalog::admit(MechanicsCatalogDefinition {
        version: version(),
        stats: vec![StatDefinition {
            id: stat("power"),
            minimum: scalar(0),
            maximum: scalar(100),
        }],
        tracks: vec![TrackDefinition {
            id: track("health"),
            minimum: scalar(0),
            maximum: TrackMaximum::Fixed { value: scalar(100) },
        }],
        sources: vec![],
        damage_kinds: vec![DamageKindDefinition {
            id: DamageKindId::parse("kinetic").unwrap(),
        }],
        effects: vec![],
        capacity_metrics: vec![],
        items: vec![],
        equipment_slots: vec![],
    })
    .unwrap();
    let definitions = std::iter::once(EntityDefinition::new(SIMPLE, "simple"))
        .chain(
            (0..2_048)
                .map(|index| EntityDefinition::new(EntityId::new(index + 20_000), "unrelated")),
        )
        .collect::<Vec<_>>();
    let mut state = EntityState::from_definitions_with_registry(
        gameplay_component_registry().unwrap(),
        definitions,
    )
    .unwrap();
    attach(
        &mut state,
        SIMPLE,
        StatsComponent::new(version(), vec![StatValue::new(stat("power"), scalar(50))]).unwrap(),
    );
    attach(
        &mut state,
        SIMPLE,
        TracksComponent::new(
            version(),
            vec![TrackValue::new(track("health"), scalar(50))],
        )
        .unwrap(),
    );
    let evaluation = StatService::evaluate(
        &state,
        &catalog,
        SIMPLE,
        &stat("power"),
        &operation("simple_stat"),
        &[],
    )
    .unwrap();
    assert_eq!(evaluation.source_cost, Default::default());

    let request_operation = operation("simple_damage");
    let receipt = DamageService::apply(
        &mut state,
        &catalog,
        DamageRequest {
            source: request_identity(&request_operation, "origin"),
            operation: request_operation,
            actor: None,
            target: SIMPLE,
            target_track: track("health"),
            parts: vec![DamagePart {
                amount: scalar(1),
                kind: DamageKindId::parse("kinetic").unwrap(),
            }],
            request_sources: vec![],
            expected_tracks_revision: None,
        },
    )
    .unwrap();
    assert_eq!(receipt.source_cost, Default::default());
    assert_eq!(receipt.parts[0].applied, scalar(1));
}

#[test]
fn public_collection_quotas_reject_before_mutation() {
    let values = (0..=MAX_STATS_PER_ENTITY)
        .map(|index| {
            StatValue::new(
                StatId::parse(format!("stat_{index:03}")).unwrap(),
                scalar(0),
            )
        })
        .collect();
    assert!(matches!(
        StatsComponent::new(version(), values),
        Err(MechanicsComponentDataError::QuotaExceeded {
            field: "stats",
            maximum: MAX_STATS_PER_ENTITY,
            ..
        })
    ));

    let catalog = catalog();
    let mut state = full_state();
    let before = encode_snapshot(&state).unwrap();
    let operation = operation("too_many_parts");
    let error = DamageService::apply(
        &mut state,
        &catalog,
        DamageRequest {
            source: request_identity(&operation, "origin"),
            operation,
            actor: None,
            target: OWNER,
            target_track: track("health"),
            parts: vec![
                DamagePart {
                    amount: scalar(1),
                    kind: DamageKindId::parse("kinetic").unwrap(),
                };
                MAX_DAMAGE_PARTS + 1
            ],
            request_sources: vec![],
            expected_tracks_revision: None,
        },
    )
    .unwrap_err();
    assert!(matches!(
        error,
        MechanicsError::RequestQuotaExceeded {
            field: "damageParts",
            maximum: MAX_DAMAGE_PARTS,
            ..
        }
    ));
    assert_eq!(encode_snapshot(&state).unwrap(), before);
}
