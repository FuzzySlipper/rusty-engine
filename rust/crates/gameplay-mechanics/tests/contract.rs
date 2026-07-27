use core_ids::EntityId;
use entity_state::{
    encode_snapshot, EntityAuthoringService, EntityComponent, EntityDefinition, EntityState,
};
use gameplay_mechanics::{
    decode_snapshot_with_catalog, ActiveEffectInstance, ActiveEffectsComponent, CatalogVersion,
    DamageKindDefinition, DamageKindId, DamageKindSelector, DamagePart, DamageRequest,
    DamageResponseDefinition, DamageService, DecisionOutcome, EffectDefinition, EffectDefinitionId,
    EffectInstanceId, EquipmentAssignment, EquipmentComponent, EquipmentService,
    EquipmentSlotDefinition, EquipmentSlotId, ExactRatio, IntrinsicSourceBinding,
    IntrinsicSourcesComponent, ItemComponent, ItemDefinition, ItemDefinitionId, ItemKind,
    ItemTransferRequest, MechanicsCatalog, MechanicsCatalogDefinition, MechanicsError,
    MechanicsScalar, MechanicsSnapshotError, OperationId, RequestSource, SourceDefinition,
    SourceDefinitionId, SourceInstanceId, SourceInstanceIdentity, StackingGroupId, StackingPolicy,
    StatContributionDefinition, StatDefinition, StatId, StatService, StatValue, StatsComponent,
    TrackAdjustmentKind, TrackDefinition, TrackId, TrackMaximum, TrackMutationRequest,
    TrackReconciliationRequest, TrackService, TrackValue, TracksComponent,
};

const SHOOTER: EntityId = EntityId::new(1);
const ARMOR_ITEM: EntityId = EntityId::new(2);
const BUILDING: EntityId = EntityId::new(3);
const TABLETOP_TARGET: EntityId = EntityId::new(4);
const SIMPLE_TARGET: EntityId = EntityId::new(5);
const FORTIFIED_TARGET: EntityId = EntityId::new(6);
const LATE_TARGET: EntityId = EntityId::new(7);
const LATE_ARMOR_ITEM: EntityId = EntityId::new(8);
const SECOND_OWNER: EntityId = EntityId::new(9);

fn scalar(value: i64) -> MechanicsScalar {
    MechanicsScalar::new(value).unwrap()
}

fn catalog_version() -> CatalogVersion {
    CatalogVersion::parse("contract.v1").unwrap()
}

fn stat_id() -> StatId {
    StatId::parse("maximum_health").unwrap()
}

fn health() -> TrackId {
    TrackId::parse("health").unwrap()
}

fn durability() -> TrackId {
    TrackId::parse("durability").unwrap()
}

fn armor_track() -> TrackId {
    TrackId::parse("armor").unwrap()
}

fn impact() -> DamageKindId {
    DamageKindId::parse("impact").unwrap()
}

fn energy() -> DamageKindId {
    DamageKindId::parse("energy").unwrap()
}

fn armor_source() -> SourceDefinitionId {
    SourceDefinitionId::parse("armor_source").unwrap()
}

fn invulnerability_source() -> SourceDefinitionId {
    SourceDefinitionId::parse("invulnerability_source").unwrap()
}

fn fortification_source() -> SourceDefinitionId {
    SourceDefinitionId::parse("fortification_source").unwrap()
}

fn vulnerability_source() -> SourceDefinitionId {
    SourceDefinitionId::parse("vulnerability_source").unwrap()
}

fn invulnerability_effect() -> EffectDefinitionId {
    EffectDefinitionId::parse("invulnerability").unwrap()
}

fn armor_item() -> ItemDefinitionId {
    ItemDefinitionId::parse("unique_armor").unwrap()
}

fn body_slot() -> EquipmentSlotId {
    EquipmentSlotId::parse("body").unwrap()
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

fn catalog() -> MechanicsCatalog {
    MechanicsCatalog::admit(MechanicsCatalogDefinition {
        version: catalog_version(),
        stats: vec![StatDefinition {
            id: stat_id(),
            minimum: scalar(1),
            maximum: scalar(1_000),
        }],
        tracks: vec![
            TrackDefinition {
                id: health(),
                minimum: scalar(0),
                maximum: TrackMaximum::Stat { stat: stat_id() },
            },
            TrackDefinition {
                id: durability(),
                minimum: scalar(0),
                maximum: TrackMaximum::Fixed { value: scalar(200) },
            },
            TrackDefinition {
                id: armor_track(),
                minimum: scalar(0),
                maximum: TrackMaximum::Fixed { value: scalar(50) },
            },
        ],
        sources: vec![
            SourceDefinition {
                id: armor_source(),
                priority: 10,
                stat_contributions: vec![],
                damage_responses: vec![
                    DamageResponseDefinition::FlatReduction {
                        selector: DamageKindSelector::Exact {
                            damage_kind: impact(),
                        },
                        amount: scalar(3),
                        stacking_group: StackingGroupId::parse("armor_flat").unwrap(),
                        stacking: StackingPolicy::Highest,
                    },
                    DamageResponseDefinition::Scale {
                        selector: DamageKindSelector::Exact {
                            damage_kind: impact(),
                        },
                        ratio: ExactRatio::new(1, 2).unwrap(),
                        stacking_group: StackingGroupId::parse("armor_scale").unwrap(),
                        stacking: StackingPolicy::Lowest,
                    },
                    DamageResponseDefinition::Absorb {
                        selector: DamageKindSelector::Exact {
                            damage_kind: impact(),
                        },
                        track: armor_track(),
                    },
                ],
            },
            SourceDefinition {
                id: invulnerability_source(),
                priority: -10,
                stat_contributions: vec![],
                damage_responses: vec![DamageResponseDefinition::Prevent {
                    selector: DamageKindSelector::Any,
                    stacking_group: StackingGroupId::parse("prevention").unwrap(),
                    stacking: StackingPolicy::UniqueBySource,
                }],
            },
            SourceDefinition {
                id: fortification_source(),
                priority: 0,
                stat_contributions: vec![StatContributionDefinition {
                    stat: stat_id(),
                    amount: scalar(20),
                    stacking_group: StackingGroupId::parse("maximum_health_bonus").unwrap(),
                    stacking: StackingPolicy::Sum,
                }],
                damage_responses: vec![],
            },
            SourceDefinition {
                id: vulnerability_source(),
                priority: 20,
                stat_contributions: vec![],
                damage_responses: vec![DamageResponseDefinition::Scale {
                    selector: DamageKindSelector::Exact {
                        damage_kind: impact(),
                    },
                    ratio: ExactRatio::new(3, 2).unwrap(),
                    stacking_group: StackingGroupId::parse("vulnerability_scale").unwrap(),
                    stacking: StackingPolicy::Sum,
                }],
            },
        ],
        damage_kinds: vec![
            DamageKindDefinition { id: impact() },
            DamageKindDefinition { id: energy() },
        ],
        effects: vec![EffectDefinition {
            id: invulnerability_effect(),
            sources: vec![invulnerability_source()],
        }],
        items: vec![ItemDefinition {
            id: armor_item(),
            kind: ItemKind::Unique,
            sources: vec![armor_source()],
        }],
        equipment_slots: vec![EquipmentSlotDefinition { id: body_slot() }],
    })
    .unwrap()
}

fn state() -> EntityState {
    let registry = gameplay_mechanics::gameplay_component_registry().unwrap();
    let mut state = EntityState::from_definitions_with_registry(
        registry,
        [
            EntityDefinition::new(SHOOTER, "shooter"),
            EntityDefinition::new(ARMOR_ITEM, "armor").with_containment(SHOOTER),
            EntityDefinition::new(BUILDING, "building"),
            EntityDefinition::new(TABLETOP_TARGET, "tabletop-target"),
            EntityDefinition::new(SIMPLE_TARGET, "simple-target"),
            EntityDefinition::new(FORTIFIED_TARGET, "fortified-target"),
            EntityDefinition::new(LATE_TARGET, "late-target"),
            EntityDefinition::new(LATE_ARMOR_ITEM, "late-armor").with_containment(LATE_TARGET),
            EntityDefinition::new(SECOND_OWNER, "second-owner"),
        ],
    )
    .unwrap();

    attach(
        &mut state,
        SHOOTER,
        StatsComponent::new(
            catalog_version(),
            vec![StatValue {
                stat: stat_id(),
                base: scalar(100),
            }],
        )
        .unwrap(),
    );
    attach(
        &mut state,
        SHOOTER,
        TracksComponent::new(
            catalog_version(),
            vec![
                TrackValue {
                    track: health(),
                    current: scalar(100),
                },
                TrackValue {
                    track: armor_track(),
                    current: scalar(20),
                },
            ],
        )
        .unwrap(),
    );
    attach(
        &mut state,
        SHOOTER,
        EquipmentComponent::new(
            catalog_version(),
            vec![EquipmentAssignment {
                slot: body_slot(),
                item: ARMOR_ITEM,
            }],
        )
        .unwrap(),
    );
    attach(
        &mut state,
        ARMOR_ITEM,
        ItemComponent::new(catalog_version(), armor_item()),
    );

    attach(
        &mut state,
        BUILDING,
        TracksComponent::new(
            catalog_version(),
            vec![TrackValue {
                track: durability(),
                current: scalar(200),
            }],
        )
        .unwrap(),
    );

    for entity in [
        TABLETOP_TARGET,
        SIMPLE_TARGET,
        FORTIFIED_TARGET,
        LATE_TARGET,
    ] {
        attach(
            &mut state,
            entity,
            StatsComponent::new(
                catalog_version(),
                vec![StatValue {
                    stat: stat_id(),
                    base: scalar(100),
                }],
            )
            .unwrap(),
        );
        attach(
            &mut state,
            entity,
            TracksComponent::new(
                catalog_version(),
                vec![TrackValue {
                    track: health(),
                    current: scalar(100),
                }],
            )
            .unwrap(),
        );
    }
    attach(
        &mut state,
        TABLETOP_TARGET,
        ActiveEffectsComponent::new(catalog_version(), vec![]).unwrap(),
    );
    attach(
        &mut state,
        FORTIFIED_TARGET,
        IntrinsicSourcesComponent::new(
            catalog_version(),
            vec![IntrinsicSourceBinding {
                instance: SourceInstanceId::parse("fortification_binding").unwrap(),
                definition: fortification_source(),
            }],
        )
        .unwrap(),
    );
    attach(
        &mut state,
        LATE_TARGET,
        EquipmentComponent::new(
            catalog_version(),
            vec![EquipmentAssignment {
                slot: body_slot(),
                item: LATE_ARMOR_ITEM,
            }],
        )
        .unwrap(),
    );
    attach(
        &mut state,
        LATE_ARMOR_ITEM,
        ItemComponent::new(catalog_version(), armor_item()),
    );
    state
}

fn attach<T: EntityComponent>(state: &mut EntityState, entity: EntityId, value: T) {
    let revision = state.component_revision::<T>(entity).unwrap();
    EntityAuthoringService
        .attach_component(state, revision, entity, value)
        .unwrap();
}

fn damage_request(
    target: EntityId,
    target_track: TrackId,
    operation_id: &str,
    parts: Vec<DamagePart>,
) -> DamageRequest {
    let operation = operation(operation_id);
    DamageRequest {
        source: request_identity(&operation, "damage_origin"),
        operation,
        actor: Some(SHOOTER),
        target,
        target_track,
        parts,
        request_sources: vec![],
        expected_tracks_revision: None,
    }
}

#[test]
fn shooter_damage_is_one_direct_fixed_pipeline_call_with_attributed_receipt() {
    let catalog = catalog();
    let mut state = state();
    let other_revision = state
        .component_revision::<TracksComponent>(BUILDING)
        .unwrap();
    let equipment_revision = state
        .component_revision::<EquipmentComponent>(SHOOTER)
        .unwrap();

    let mut request = damage_request(
        SHOOTER,
        health(),
        "shooter_hit",
        vec![DamagePart {
            amount: scalar(50),
            kind: impact(),
        }],
    );
    request.request_sources.push(RequestSource {
        instance: SourceInstanceId::parse("vulnerability_context").unwrap(),
        definition: vulnerability_source(),
    });
    let receipt = DamageService::apply(&mut state, &catalog, request).unwrap();

    assert_eq!(receipt.parts.len(), 1);
    assert_eq!(receipt.parts[0].after_flat.get(), 47);
    assert_eq!(
        (
            receipt.parts[0].combined_scale_numerator,
            receipt.parts[0].combined_scale_denominator,
        ),
        (3, 4)
    );
    assert_eq!(receipt.parts[0].after_scale.get(), 35);
    assert_eq!(receipt.parts[0].absorbed.get(), 20);
    assert_eq!(receipt.parts[0].applied.get(), 15);
    assert_eq!(
        state
            .component::<TracksComponent>(SHOOTER)
            .unwrap()
            .unwrap()
            .current(&health())
            .unwrap()
            .get(),
        85
    );
    assert_eq!(
        state
            .component::<TracksComponent>(SHOOTER)
            .unwrap()
            .unwrap()
            .current(&armor_track())
            .unwrap()
            .get(),
        0
    );
    assert!(receipt
        .decisions
        .iter()
        .filter(|decision| decision.source_definition == armor_source())
        .all(|decision| decision.outcome == DecisionOutcome::Applied));
    assert_eq!(
        state
            .component_revision::<TracksComponent>(BUILDING)
            .unwrap(),
        other_revision
    );
    assert_eq!(
        state
            .component_revision::<EquipmentComponent>(SHOOTER)
            .unwrap(),
        equipment_revision
    );
}

#[test]
fn equal_stacking_candidates_use_canonical_source_identity_ties() {
    let catalog = catalog();
    let state = state();
    let mut request = damage_request(
        SHOOTER,
        health(),
        "stacking_tie_preview",
        vec![DamagePart {
            amount: scalar(10),
            kind: impact(),
        }],
    );
    request.request_sources.push(RequestSource {
        instance: SourceInstanceId::parse("second_armor_context").unwrap(),
        definition: armor_source(),
    });
    let preview = DamageService::preview(&state, &catalog, &request).unwrap();
    let suppressed_request_decisions = preview
        .receipt()
        .decisions
        .iter()
        .filter(|decision| {
            matches!(decision.source, SourceInstanceIdentity::Request { .. })
                && decision.source_definition == armor_source()
                && decision.outcome == DecisionOutcome::Suppressed
        })
        .count();
    assert_eq!(suppressed_request_decisions, 2);
}

#[test]
fn building_damage_and_repair_use_the_same_track_mechanisms() {
    let catalog = catalog();
    let mut state = state();
    let damage = DamageService::apply(
        &mut state,
        &catalog,
        damage_request(
            BUILDING,
            durability(),
            "building_damage",
            vec![DamagePart {
                amount: scalar(30),
                kind: energy(),
            }],
        ),
    )
    .unwrap();
    assert_eq!(damage.parts[0].applied.get(), 30);

    let repair_operation = operation("building_repair");
    let repair = TrackService::restore(
        &mut state,
        &catalog,
        TrackMutationRequest {
            operation: repair_operation.clone(),
            source: request_identity(&repair_operation, "repair_origin"),
            entity: BUILDING,
            track: durability(),
            amount: scalar(25),
            kind: TrackAdjustmentKind::Spend,
            expected_revision: None,
        },
    )
    .unwrap();
    assert_eq!((repair.before.get(), repair.after.get()), (170, 195));
    assert_eq!(repair.applied_amount.get(), 25);
}

#[test]
fn tabletop_preview_is_pure_and_fresh_apply_recomputes_after_reaction() {
    let catalog = catalog();
    let mut state = state();
    let request = damage_request(
        TABLETOP_TARGET,
        health(),
        "tabletop_preview",
        vec![DamagePart {
            amount: scalar(30),
            kind: energy(),
        }],
    );
    let preview = DamageService::preview(&state, &catalog, &request).unwrap();
    assert_eq!(preview.receipt().parts[0].applied.get(), 30);
    assert_eq!(
        state
            .component::<TracksComponent>(TABLETOP_TARGET)
            .unwrap()
            .unwrap()
            .current(&health())
            .unwrap()
            .get(),
        100
    );

    let effect_revision = state
        .component_revision::<ActiveEffectsComponent>(TABLETOP_TARGET)
        .unwrap();
    EntityAuthoringService
        .replace_component(
            &mut state,
            effect_revision,
            TABLETOP_TARGET,
            ActiveEffectsComponent::new(
                catalog_version(),
                vec![ActiveEffectInstance {
                    instance: EffectInstanceId::parse("shield_reaction").unwrap(),
                    definition: invulnerability_effect(),
                }],
            )
            .unwrap(),
        )
        .unwrap();

    let apply_operation = operation("tabletop_apply");
    let receipt = DamageService::apply(
        &mut state,
        &catalog,
        DamageRequest {
            operation: apply_operation.clone(),
            source: request_identity(&apply_operation, "damage_origin"),
            expected_tracks_revision: Some(preview.observed_revision().clone()),
            ..damage_request(
                TABLETOP_TARGET,
                health(),
                "discarded_template",
                vec![DamagePart {
                    amount: scalar(30),
                    kind: energy(),
                }],
            )
        },
    )
    .unwrap();
    assert!(receipt.parts[0].prevented);
    assert_eq!(receipt.parts[0].applied, MechanicsScalar::zero());
    assert!(receipt.decisions.iter().any(|decision| {
        decision.source_definition == invulnerability_source()
            && decision.outcome == DecisionOutcome::Applied
    }));
    assert_eq!(
        state
            .component::<TracksComponent>(TABLETOP_TARGET)
            .unwrap()
            .unwrap()
            .current(&health())
            .unwrap()
            .get(),
        100
    );
}

#[test]
fn stats_are_base_values_plus_canonically_attributed_sources() {
    let catalog = catalog();
    let state = state();
    let evaluation = StatService::evaluate(
        &state,
        &catalog,
        FORTIFIED_TARGET,
        &stat_id(),
        &operation("evaluate_fortification"),
        &[],
    )
    .unwrap();
    assert_eq!(evaluation.base.get(), 100);
    assert_eq!(evaluation.value.get(), 120);
    assert_eq!(evaluation.source_cost.intrinsic_entries_visited, 1);
    assert!(evaluation.decisions.iter().any(|decision| {
        decision.source_definition == fortification_source()
            && decision.outcome == DecisionOutcome::Applied
    }));
}

#[test]
fn bound_lowering_uses_a_valid_state_reconciliation_then_source_change() {
    let catalog = catalog();
    let mut state = state();
    let fill_operation = operation("fill_fortified_health");
    TrackService::restore(
        &mut state,
        &catalog,
        TrackMutationRequest {
            operation: fill_operation.clone(),
            source: request_identity(&fill_operation, "fill_origin"),
            entity: FORTIFIED_TARGET,
            track: health(),
            amount: scalar(20),
            kind: TrackAdjustmentKind::Restore,
            expected_revision: None,
        },
    )
    .unwrap();
    assert_eq!(
        state
            .component::<TracksComponent>(FORTIFIED_TARGET)
            .unwrap()
            .unwrap()
            .current(&health())
            .unwrap()
            .get(),
        120
    );

    let reconcile_operation = operation("reconcile_before_source_removal");
    let receipt = TrackService::reconcile_to_maximum(
        &mut state,
        &catalog,
        TrackReconciliationRequest {
            operation: reconcile_operation.clone(),
            source: request_identity(&reconcile_operation, "reconcile_origin"),
            entity: FORTIFIED_TARGET,
            track: health(),
            prospective_maximum: scalar(100),
            expected_revision: None,
        },
    )
    .unwrap();
    assert_eq!((receipt.before.get(), receipt.after.get()), (120, 100));

    let source_revision = state
        .component_revision::<IntrinsicSourcesComponent>(FORTIFIED_TARGET)
        .unwrap();
    EntityAuthoringService
        .replace_component(
            &mut state,
            source_revision,
            FORTIFIED_TARGET,
            IntrinsicSourcesComponent::new(catalog_version(), vec![]).unwrap(),
        )
        .unwrap();
    gameplay_mechanics::validate_state_against_catalog(&state, &catalog).unwrap();
}

#[test]
fn simple_damage_path_has_zero_absent_feature_traversal() {
    let catalog = catalog();
    let mut state = state();
    let receipt = DamageService::apply(
        &mut state,
        &catalog,
        damage_request(
            SIMPLE_TARGET,
            health(),
            "simple_hit",
            vec![DamagePart {
                amount: scalar(10),
                kind: energy(),
            }],
        ),
    )
    .unwrap();
    assert_eq!(receipt.decisions.len(), 0);
    assert_eq!(receipt.source_cost.intrinsic_entries_visited, 0);
    assert_eq!(receipt.source_cost.effect_entries_visited, 0);
    assert_eq!(receipt.source_cost.equipment_entries_visited, 0);
    assert_eq!(receipt.source_cost.item_components_read, 0);
    assert_eq!(receipt.source_cost.request_entries_visited, 0);
}

#[test]
fn stale_duplicate_invalid_and_late_rejections_do_not_mutate_tracks() {
    let catalog = catalog();
    let mut state = state();

    let stale = state
        .component_revision::<TracksComponent>(SIMPLE_TARGET)
        .unwrap();
    let spend_operation = operation("spend_before_stale");
    TrackService::spend(
        &mut state,
        &catalog,
        TrackMutationRequest {
            operation: spend_operation.clone(),
            source: request_identity(&spend_operation, "spend_origin"),
            entity: SIMPLE_TARGET,
            track: health(),
            amount: scalar(1),
            kind: TrackAdjustmentKind::Restore,
            expected_revision: None,
        },
    )
    .unwrap();
    let before_stale = state
        .component::<TracksComponent>(SIMPLE_TARGET)
        .unwrap()
        .unwrap()
        .clone();
    let before_stale_revision = state
        .component_revision::<TracksComponent>(SIMPLE_TARGET)
        .unwrap();
    let mut stale_request = damage_request(
        SIMPLE_TARGET,
        health(),
        "stale_hit",
        vec![DamagePart {
            amount: scalar(10),
            kind: energy(),
        }],
    );
    stale_request.expected_tracks_revision = Some(stale);
    assert!(matches!(
        DamageService::apply(&mut state, &catalog, stale_request),
        Err(MechanicsError::StaleComponentRevision { .. })
    ));
    assert_eq!(
        state
            .component::<TracksComponent>(SIMPLE_TARGET)
            .unwrap()
            .unwrap(),
        &before_stale
    );
    assert_eq!(
        state
            .component_revision::<TracksComponent>(SIMPLE_TARGET)
            .unwrap(),
        before_stale_revision
    );

    let duplicate_operation = operation("duplicate_source_hit");
    let duplicate_instance = SourceInstanceId::parse("same_context").unwrap();
    let duplicate_request = DamageRequest {
        operation: duplicate_operation.clone(),
        source: request_identity(&duplicate_operation, "damage_origin"),
        actor: None,
        target: SIMPLE_TARGET,
        target_track: health(),
        parts: vec![DamagePart {
            amount: scalar(10),
            kind: impact(),
        }],
        request_sources: vec![
            RequestSource {
                instance: duplicate_instance.clone(),
                definition: armor_source(),
            },
            RequestSource {
                instance: duplicate_instance,
                definition: invulnerability_source(),
            },
        ],
        expected_tracks_revision: None,
    };
    let before_duplicate = state
        .component::<TracksComponent>(SIMPLE_TARGET)
        .unwrap()
        .unwrap()
        .clone();
    let before_duplicate_revision = state
        .component_revision::<TracksComponent>(SIMPLE_TARGET)
        .unwrap();
    assert!(matches!(
        DamageService::apply(&mut state, &catalog, duplicate_request),
        Err(MechanicsError::DuplicateSource { .. })
    ));
    assert_eq!(
        state
            .component::<TracksComponent>(SIMPLE_TARGET)
            .unwrap()
            .unwrap(),
        &before_duplicate
    );
    assert_eq!(
        state
            .component_revision::<TracksComponent>(SIMPLE_TARGET)
            .unwrap(),
        before_duplicate_revision
    );

    let invalid = IntrinsicSourcesComponent::new(
        catalog_version(),
        vec![IntrinsicSourceBinding {
            instance: SourceInstanceId::parse("unknown_binding").unwrap(),
            definition: SourceDefinitionId::parse("missing_source").unwrap(),
        }],
    )
    .unwrap();
    attach(&mut state, SIMPLE_TARGET, invalid);
    let before_invalid = state
        .component::<TracksComponent>(SIMPLE_TARGET)
        .unwrap()
        .unwrap()
        .clone();
    let before_invalid_revision = state
        .component_revision::<TracksComponent>(SIMPLE_TARGET)
        .unwrap();
    assert!(matches!(
        DamageService::apply(
            &mut state,
            &catalog,
            damage_request(
                SIMPLE_TARGET,
                health(),
                "invalid_source_hit",
                vec![DamagePart {
                    amount: scalar(10),
                    kind: energy(),
                }],
            ),
        ),
        Err(MechanicsError::UnknownSource { .. })
    ));
    assert_eq!(
        state
            .component::<TracksComponent>(SIMPLE_TARGET)
            .unwrap()
            .unwrap(),
        &before_invalid
    );
    assert_eq!(
        state
            .component_revision::<TracksComponent>(SIMPLE_TARGET)
            .unwrap(),
        before_invalid_revision
    );

    let before_late = state
        .component::<TracksComponent>(LATE_TARGET)
        .unwrap()
        .unwrap()
        .clone();
    let before_late_revision = state
        .component_revision::<TracksComponent>(LATE_TARGET)
        .unwrap();
    assert!(matches!(
        DamageService::apply(
            &mut state,
            &catalog,
            damage_request(
                LATE_TARGET,
                health(),
                "late_multipart_hit",
                vec![
                    DamagePart {
                        amount: scalar(10),
                        kind: energy(),
                    },
                    DamagePart {
                        amount: scalar(10),
                        kind: impact(),
                    },
                ],
            ),
        ),
        Err(MechanicsError::MissingTrack { track, .. }) if track == armor_track()
    ));
    assert_eq!(
        state
            .component::<TracksComponent>(LATE_TARGET)
            .unwrap()
            .unwrap(),
        &before_late
    );
    assert_eq!(
        state
            .component_revision::<TracksComponent>(LATE_TARGET)
            .unwrap(),
        before_late_revision
    );
}

#[test]
fn unique_item_transfer_requires_explicit_unequip_then_changes_only_containment() {
    let catalog = catalog();
    let mut state = state();
    let transfer_operation = operation("transfer_equipped_armor");
    let before_revision = state.revision();
    assert!(matches!(
        EquipmentService::transfer_unique_item(
            &mut state,
            &catalog,
            ItemTransferRequest {
                operation: transfer_operation.clone(),
                source: request_identity(&transfer_operation, "transfer_origin"),
                item: ARMOR_ITEM,
                from_owner: SHOOTER,
                to_owner: SECOND_OWNER,
                expected_relationship_revision: before_revision,
            },
        ),
        Err(MechanicsError::ItemEquipped { .. })
    ));
    assert_eq!(state.revision(), before_revision);
    assert_eq!(state.contained_in(ARMOR_ITEM), Some(SHOOTER));

    let unequip_operation = operation("unequip_for_transfer");
    EquipmentService::unequip(
        &mut state,
        &catalog,
        unequip_operation.clone(),
        request_identity(&unequip_operation, "unequip_origin"),
        SHOOTER,
        body_slot(),
        None,
    )
    .unwrap();
    let item_revision = state
        .component_revision::<ItemComponent>(ARMOR_ITEM)
        .unwrap();
    let transfer_operation = operation("transfer_unequipped_armor");
    let relationship_revision = state.revision();
    let receipt = EquipmentService::transfer_unique_item(
        &mut state,
        &catalog,
        ItemTransferRequest {
            operation: transfer_operation.clone(),
            source: request_identity(&transfer_operation, "transfer_origin"),
            item: ARMOR_ITEM,
            from_owner: SHOOTER,
            to_owner: SECOND_OWNER,
            expected_relationship_revision: relationship_revision,
        },
    )
    .unwrap();
    assert_eq!(state.contained_in(ARMOR_ITEM), Some(SECOND_OWNER));
    assert_eq!(receipt.revision_after, receipt.revision_before + 1);
    assert_eq!(
        state
            .component_revision::<ItemComponent>(ARMOR_ITEM)
            .unwrap(),
        item_revision
    );
}

#[test]
fn strict_component_snapshot_round_trips_and_rejects_unresolved_catalog_references() {
    let catalog = catalog();
    let state = state();
    gameplay_mechanics::validate_state_against_catalog(&state, &catalog).unwrap();
    let encoded = encode_snapshot(&state).unwrap();
    let restored = decode_snapshot_with_catalog(&encoded, &catalog).unwrap();
    assert_eq!(encode_snapshot(&restored).unwrap(), encoded);
    assert_eq!(restored.contained_in(ARMOR_ITEM), Some(SHOOTER));
    assert_eq!(
        restored
            .component::<ItemComponent>(ARMOR_ITEM)
            .unwrap()
            .unwrap()
            .definition(),
        &armor_item()
    );

    let mut corrupt: serde_json::Value = serde_json::from_str(&encoded).unwrap();
    let item_snapshot = corrupt["registeredComponents"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|component| component["typeId"] == gameplay_mechanics::ITEM_COMPONENT_TYPE_ID)
        .unwrap();
    item_snapshot["values"][0]["value"]["definition"] =
        serde_json::Value::String("missing_item".to_string());
    assert!(matches!(
        decode_snapshot_with_catalog(&corrupt.to_string(), &catalog),
        Err(MechanicsSnapshotError::Mechanics(
            MechanicsError::InvalidCatalogReference { .. }
        ))
    ));
}

#[test]
fn entity_lifecycle_removes_all_registered_mechanics_components_without_reconciliation() {
    let mut state = state();
    assert!(state
        .component::<TracksComponent>(SIMPLE_TARGET)
        .unwrap()
        .is_some());
    let revision = state.revision();
    EntityAuthoringService
        .destroy(&mut state, revision, SIMPLE_TARGET)
        .unwrap();
    assert!(state
        .component::<TracksComponent>(SIMPLE_TARGET)
        .unwrap()
        .is_none());
    assert!(state
        .component::<StatsComponent>(SIMPLE_TARGET)
        .unwrap()
        .is_none());
}
