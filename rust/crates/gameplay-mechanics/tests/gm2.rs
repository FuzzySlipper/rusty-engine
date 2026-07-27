use core_ids::EntityId;
use entity_state::{
    encode_snapshot, EntityAuthoringService, EntityComponent, EntityDefinition, EntityState,
};
use gameplay_mechanics::{
    decode_snapshot_with_catalog, ActiveEffectInstance, ActiveEffectsComponent, CatalogError,
    CatalogVersion, DamageKindDefinition, DamageKindId, DamageKindSelector, DamagePart,
    DamageRequest, DamageResponseDefinition, DamageService, DecisionOutcome, EffectApplyRequest,
    EffectDefinition, EffectDefinitionId, EffectInstanceId, EffectMutationKind,
    EffectRefreshRequest, EffectRemovalRequest, EffectReplaceRequest, EffectService,
    EffectStackingPolicy, ExactRatio, MechanicsCatalog, MechanicsCatalogDefinition,
    MechanicsComponentDataError, MechanicsEntityView, MechanicsError, MechanicsScalar,
    MechanicsSnapshotError, OperationId, SourceDefinition, SourceDefinitionId, SourceInstanceId,
    SourceInstanceIdentity, StackingGroupId, StackingPolicy, StatContribution,
    StatContributionDefinition, StatDefinition, StatId, StatService, StatValue, StatsComponent,
    TrackDefinition, TrackId, TrackMaximum, TrackMutationRequest, TrackReconciliationPolicy,
    TrackReconciliationRequest, TrackService, TrackValue, TracksComponent,
    ACTIVE_EFFECTS_COMPONENT_TYPE_ID, MAX_EFFECT_STACKS,
};

const TARGET: EntityId = EntityId::new(201);
const BUILDING: EntityId = EntityId::new(202);
const REALTIME_OWNER_TARGET: EntityId = EntityId::new(203);
const CITY_OWNER_TARGET: EntityId = EntityId::new(204);
const TABLETOP_OWNER_TARGET: EntityId = EntityId::new(205);
const MISSING_EFFECTS: EntityId = EntityId::new(206);

fn scalar(value: i64) -> MechanicsScalar {
    MechanicsScalar::new(value).unwrap()
}

fn catalog_version() -> CatalogVersion {
    CatalogVersion::parse("gm2.v1").unwrap()
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

fn effect_instance(value: &str) -> EffectInstanceId {
    EffectInstanceId::parse(value).unwrap()
}

fn group(value: &str) -> StackingGroupId {
    StackingGroupId::parse(value).unwrap()
}

fn operation(value: &str) -> OperationId {
    OperationId::parse(value).unwrap()
}

fn provenance(operation_id: &str, instance: &str) -> SourceInstanceIdentity {
    SourceInstanceIdentity::Request {
        operation: operation(operation_id),
        instance: SourceInstanceId::parse(instance).unwrap(),
    }
}

fn add(stat_id: &str, amount: i64, stacking_group: &str) -> StatContributionDefinition {
    StatContributionDefinition {
        stat: stat(stat_id),
        contribution: StatContribution::Add {
            amount: scalar(amount),
        },
        stacking_group: group(stacking_group),
        stacking: StackingPolicy::Sum,
    }
}

fn source_definition(
    id: &str,
    priority: i16,
    stat_contributions: Vec<StatContributionDefinition>,
    damage_responses: Vec<DamageResponseDefinition>,
) -> SourceDefinition {
    SourceDefinition {
        id: source(id),
        priority,
        stat_contributions,
        damage_responses,
    }
}

fn effect_definition(
    id: &str,
    stacking_group: &str,
    stacking: EffectStackingPolicy,
    maximum_stacks: u16,
    sources: &[&str],
) -> EffectDefinition {
    EffectDefinition {
        id: effect(id),
        stacking_group: group(stacking_group),
        stacking,
        maximum_stacks,
        sources: sources.iter().map(|source_id| source(source_id)).collect(),
    }
}

fn definition() -> MechanicsCatalogDefinition {
    MechanicsCatalogDefinition {
        version: catalog_version(),
        stats: vec![
            StatDefinition {
                id: stat("max_health"),
                minimum: scalar(1),
                maximum: scalar(1_000),
            },
            StatDefinition {
                id: stat("production"),
                minimum: scalar(-1_000),
                maximum: scalar(1_000),
            },
        ],
        tracks: vec![
            TrackDefinition {
                id: track("health"),
                minimum: scalar(0),
                maximum: TrackMaximum::Stat {
                    stat: stat("max_health"),
                },
            },
            TrackDefinition {
                id: track("durability"),
                minimum: scalar(0),
                maximum: TrackMaximum::Fixed { value: scalar(200) },
            },
        ],
        sources: vec![
            source_definition(
                "temporary_maximum",
                0,
                vec![add("max_health", 50, "temporary_maximum")],
                vec![],
            ),
            source_definition(
                "temporary_production",
                5,
                vec![add("production", 10, "temporary_production")],
                vec![],
            ),
            source_definition(
                "temporary_prevention",
                -10,
                vec![],
                vec![DamageResponseDefinition::Prevent {
                    selector: DamageKindSelector::Any,
                    stacking_group: group("temporary_prevention"),
                    stacking: StackingPolicy::UniqueBySource,
                }],
            ),
            source_definition(
                "stance_low_source",
                10,
                vec![StatContributionDefinition {
                    stat: stat("production"),
                    contribution: StatContribution::Add { amount: scalar(1) },
                    stacking_group: group("stance_bonus"),
                    stacking: StackingPolicy::Highest,
                }],
                vec![],
            ),
            source_definition(
                "stance_high_source",
                10,
                vec![StatContributionDefinition {
                    stat: stat("production"),
                    contribution: StatContribution::Add { amount: scalar(2) },
                    stacking_group: group("stance_bonus"),
                    stacking: StackingPolicy::Highest,
                }],
                vec![],
            ),
        ],
        damage_kinds: vec![DamageKindDefinition {
            id: DamageKindId::parse("impact").unwrap(),
        }],
        effects: vec![
            effect_definition(
                "ward",
                "ward_lifecycle",
                EffectStackingPolicy::Refresh,
                2,
                &["temporary_maximum", "temporary_prevention"],
            ),
            effect_definition(
                "improvement",
                "improvement_lifecycle",
                EffectStackingPolicy::IndependentByProvenance {
                    maximum_instances: 2,
                },
                3,
                &["temporary_maximum", "temporary_production"],
            ),
            effect_definition(
                "stance_low",
                "stance_lifecycle",
                EffectStackingPolicy::Replace,
                1,
                &["stance_low_source"],
            ),
            effect_definition(
                "stance_high",
                "stance_lifecycle",
                EffectStackingPolicy::Replace,
                1,
                &["stance_high_source"],
            ),
        ],
        items: vec![],
        equipment_slots: vec![],
    }
}

fn catalog() -> MechanicsCatalog {
    MechanicsCatalog::admit(definition()).unwrap()
}

fn state() -> EntityState {
    let mut state = EntityState::from_definitions_with_registry(
        gameplay_mechanics::gameplay_component_registry().unwrap(),
        [
            EntityDefinition::new(TARGET, "target"),
            EntityDefinition::new(BUILDING, "building"),
            EntityDefinition::new(REALTIME_OWNER_TARGET, "realtime-target"),
            EntityDefinition::new(CITY_OWNER_TARGET, "city-target"),
            EntityDefinition::new(TABLETOP_OWNER_TARGET, "tabletop-target"),
            EntityDefinition::new(MISSING_EFFECTS, "missing-effects"),
        ],
    )
    .unwrap();
    for entity in [
        TARGET,
        BUILDING,
        REALTIME_OWNER_TARGET,
        CITY_OWNER_TARGET,
        TABLETOP_OWNER_TARGET,
        MISSING_EFFECTS,
    ] {
        attach(
            &mut state,
            entity,
            StatsComponent::new(
                catalog_version(),
                vec![
                    StatValue::new(stat("max_health"), scalar(100)),
                    StatValue::new(stat("production"), scalar(0)),
                ],
            )
            .unwrap(),
        );
        attach(
            &mut state,
            entity,
            TracksComponent::new(
                catalog_version(),
                vec![
                    TrackValue::new(track("health"), scalar(100)),
                    TrackValue::new(track("durability"), scalar(200)),
                ],
            )
            .unwrap(),
        );
        if entity != MISSING_EFFECTS {
            attach(
                &mut state,
                entity,
                ActiveEffectsComponent::new(catalog_version(), vec![]).unwrap(),
            );
        }
    }
    state
}

fn attach<T: EntityComponent>(state: &mut EntityState, entity: EntityId, value: T) {
    let revision = state.component_revision::<T>(entity).unwrap();
    EntityAuthoringService
        .attach_component(state, revision, entity, value)
        .unwrap();
}

fn apply_request(
    entity: EntityId,
    operation_id: &str,
    instance: &str,
    definition: &str,
    provenance_id: &str,
    stacks: u16,
) -> EffectApplyRequest {
    EffectApplyRequest {
        operation: operation(operation_id),
        entity,
        instance: effect_instance(instance),
        definition: effect(definition),
        provenance: provenance(operation_id, provenance_id),
        stacks,
        expected_revision: None,
    }
}

fn removal_request(entity: EntityId, operation_id: &str, instance: &str) -> EffectRemovalRequest {
    EffectRemovalRequest {
        operation: operation(operation_id),
        entity,
        instance: effect_instance(instance),
        expected_revision: None,
    }
}

#[test]
fn catalog_admits_only_bounded_explicit_effect_policies() {
    let admitted = catalog();
    let ward = admitted.effect(&effect("ward")).unwrap();
    assert_eq!(ward.maximum_stacks, 2);
    assert_eq!(ward.stacking, EffectStackingPolicy::Refresh);
    assert_eq!(
        ward.sources,
        vec![source("temporary_maximum"), source("temporary_prevention")]
    );

    let mut empty = definition();
    empty.effects[0].sources.clear();
    assert!(matches!(
        MechanicsCatalog::admit(empty),
        Err(CatalogError::EmptyReferences {
            namespace: "source",
            ..
        })
    ));

    let mut excessive_stacks = definition();
    excessive_stacks.effects[0].maximum_stacks = MAX_EFFECT_STACKS + 1;
    assert!(matches!(
        MechanicsCatalog::admit(excessive_stacks),
        Err(CatalogError::InvalidEffectLimit {
            field: "maximumStacks",
            ..
        })
    ));

    let mut inconsistent_group = definition();
    inconsistent_group
        .effects
        .iter_mut()
        .find(|definition| definition.id == effect("stance_high"))
        .unwrap()
        .stacking = EffectStackingPolicy::Refresh;
    assert!(matches!(
        MechanicsCatalog::admit(inconsistent_group),
        Err(CatalogError::InconsistentEffectStackingPolicy { .. })
    ));

    assert!(matches!(
        ActiveEffectInstance::new(
            effect_instance("invalid_stacks"),
            effect("ward"),
            provenance("invalid_stacks", "origin"),
            0,
        ),
        Err(MechanicsComponentDataError::InvalidEffectStacks { stacks: 0, .. })
    ));
}

#[test]
fn apply_refresh_replace_remove_and_expire_are_exact_and_fail_atomic() {
    let catalog = catalog();
    let mut state = state();

    let apply = EffectService::apply(
        &mut state,
        &catalog,
        apply_request(TARGET, "apply_ward", "ward_one", "ward", "caster_a", 2),
    )
    .unwrap();
    assert_eq!(apply.kind, EffectMutationKind::Apply);
    assert_eq!(apply.activated_sources.len(), 4);
    assert_eq!(apply.current.as_ref().unwrap().stacks(), 2);
    assert_eq!(
        apply.committed_effects_revision,
        apply.observed_effects_revision + 1
    );

    let before_duplicate = state
        .component::<ActiveEffectsComponent>(TARGET)
        .unwrap()
        .unwrap()
        .clone();
    let before_duplicate_revision = state
        .component_revision::<ActiveEffectsComponent>(TARGET)
        .unwrap();
    let before_duplicate_global = state.revision();
    assert!(matches!(
        EffectService::apply(
            &mut state,
            &catalog,
            apply_request(TARGET, "duplicate_ward", "ward_one", "ward", "caster_b", 1,),
        ),
        Err(MechanicsError::DuplicateEffectInstance { .. })
    ));
    assert!(matches!(
        EffectService::apply(
            &mut state,
            &catalog,
            apply_request(
                TARGET,
                "conflicting_ward",
                "ward_two",
                "ward",
                "caster_b",
                1,
            ),
        ),
        Err(MechanicsError::EffectStackingConflict { .. })
    ));
    assert_eq!(
        state
            .component::<ActiveEffectsComponent>(TARGET)
            .unwrap()
            .unwrap(),
        &before_duplicate
    );
    assert_eq!(
        state
            .component_revision::<ActiveEffectsComponent>(TARGET)
            .unwrap(),
        before_duplicate_revision
    );
    assert_eq!(state.revision(), before_duplicate_global);

    let stale_revision = before_duplicate_revision.clone();
    let refreshed = EffectService::refresh(
        &mut state,
        &catalog,
        EffectRefreshRequest {
            operation: operation("refresh_ward"),
            entity: TARGET,
            instance: effect_instance("ward_one"),
            provenance: provenance("refresh_ward", "caster_b"),
            stacks: 1,
            expected_revision: Some(before_duplicate_revision),
        },
    )
    .unwrap();
    assert_eq!(refreshed.kind, EffectMutationKind::Refresh);
    assert_eq!(refreshed.removed.len(), 1);
    assert_eq!(
        refreshed.current.as_ref().unwrap().instance(),
        &effect_instance("ward_one")
    );
    assert_eq!(
        refreshed.current.as_ref().unwrap().provenance(),
        &provenance("refresh_ward", "caster_b")
    );

    let stale_global = state.revision();
    assert!(matches!(
        EffectService::refresh(
            &mut state,
            &catalog,
            EffectRefreshRequest {
                operation: operation("stale_refresh"),
                entity: TARGET,
                instance: effect_instance("ward_one"),
                provenance: provenance("stale_refresh", "caster_c"),
                stacks: 1,
                expected_revision: Some(stale_revision),
            },
        ),
        Err(MechanicsError::StaleComponentRevision { .. })
    ));
    assert_eq!(state.revision(), stale_global);

    let first_stance = EffectService::replace(
        &mut state,
        &catalog,
        EffectReplaceRequest {
            operation: operation("replace_stance_low"),
            entity: TARGET,
            instance: effect_instance("stance_low_one"),
            definition: effect("stance_low"),
            provenance: provenance("replace_stance_low", "owner"),
            stacks: 1,
            expected_revision: None,
        },
    )
    .unwrap();
    assert!(first_stance.removed.is_empty());
    let second_stance = EffectService::replace(
        &mut state,
        &catalog,
        EffectReplaceRequest {
            operation: operation("replace_stance_high"),
            entity: TARGET,
            instance: effect_instance("stance_high_one"),
            definition: effect("stance_high"),
            provenance: provenance("replace_stance_high", "owner"),
            stacks: 1,
            expected_revision: None,
        },
    )
    .unwrap();
    assert_eq!(second_stance.kind, EffectMutationKind::Replace);
    assert_eq!(second_stance.removed.len(), 1);
    assert_eq!(
        second_stance.removed[0].instance(),
        &effect_instance("stance_low_one")
    );

    let removed = EffectService::remove(
        &mut state,
        &catalog,
        removal_request(TARGET, "remove_stance", "stance_high_one"),
    )
    .unwrap();
    assert_eq!(removed.kind, EffectMutationKind::Remove);
    assert_eq!(removed.removed.len(), 1);
    let expired = EffectService::expire(
        &mut state,
        &catalog,
        removal_request(TARGET, "expire_ward", "ward_one"),
    )
    .unwrap();
    assert_eq!(expired.kind, EffectMutationKind::Expire);
    assert!(expired.current.is_none());

    let before_missing = state.revision();
    assert!(matches!(
        EffectService::remove(
            &mut state,
            &catalog,
            removal_request(TARGET, "remove_missing", "missing"),
        ),
        Err(MechanicsError::MissingEffectInstance { .. })
    ));
    assert!(matches!(
        EffectService::apply(
            &mut state,
            &catalog,
            apply_request(
                MISSING_EFFECTS,
                "missing_component",
                "ward_missing",
                "ward",
                "origin",
                1,
            ),
        ),
        Err(MechanicsError::MissingComponent {
            component: "ActiveEffectsComponent",
            ..
        })
    ));
    assert!(matches!(
        EffectService::apply(
            &mut state,
            &catalog,
            apply_request(
                TARGET,
                "unknown_effect",
                "unknown_one",
                "unknown_effect",
                "origin",
                1,
            ),
        ),
        Err(MechanicsError::UnknownEffect { .. })
    ));
    assert!(matches!(
        EffectService::apply(
            &mut state,
            &catalog,
            apply_request(TARGET, "too_many_stacks", "ward_many", "ward", "origin", 3),
        ),
        Err(MechanicsError::EffectStackLimitExceeded {
            stacks: 3,
            maximum: 2,
            ..
        })
    ));
    assert_eq!(state.revision(), before_missing);
}

#[test]
fn independent_effects_are_provenance_unique_and_group_bounded() {
    let catalog = catalog();
    let mut state = state();
    EffectService::apply(
        &mut state,
        &catalog,
        apply_request(
            TARGET,
            "apply_improvement_a",
            "improvement_a",
            "improvement",
            "source_a",
            1,
        ),
    )
    .unwrap();
    let before_conflict = state.revision();
    assert!(matches!(
        EffectService::apply(
            &mut state,
            &catalog,
            apply_request(
                TARGET,
                "apply_improvement_a",
                "improvement_duplicate_provenance",
                "improvement",
                "source_a",
                1,
            ),
        ),
        Err(MechanicsError::EffectProvenanceConflict { .. })
    ));
    assert_eq!(state.revision(), before_conflict);

    EffectService::apply(
        &mut state,
        &catalog,
        apply_request(
            TARGET,
            "apply_improvement_b",
            "improvement_b",
            "improvement",
            "source_b",
            1,
        ),
    )
    .unwrap();
    let before_limit = state.revision();
    assert!(matches!(
        EffectService::apply(
            &mut state,
            &catalog,
            apply_request(
                TARGET,
                "apply_improvement_c",
                "improvement_c",
                "improvement",
                "source_c",
                1,
            ),
        ),
        Err(MechanicsError::EffectGroupLimitExceeded {
            actual: 3,
            maximum: 2,
            ..
        })
    ));
    assert_eq!(state.revision(), before_limit);
}

#[test]
fn effect_stacks_activate_attributed_sources_for_stats_and_damage() {
    let catalog = catalog();
    let mut state = state();
    EffectService::apply(
        &mut state,
        &catalog,
        apply_request(
            TARGET,
            "apply_stacked_improvement",
            "stacked_improvement",
            "improvement",
            "builder",
            2,
        ),
    )
    .unwrap();
    EffectService::apply(
        &mut state,
        &catalog,
        apply_request(
            TARGET,
            "apply_damage_ward",
            "damage_ward",
            "ward",
            "ward_owner",
            2,
        ),
    )
    .unwrap();

    let max_health = StatService::evaluate(
        &state,
        &catalog,
        TARGET,
        &stat("max_health"),
        &operation("inspect_maximum"),
        &[],
    )
    .unwrap();
    assert_eq!(max_health.value, scalar(300));
    let effect_stacks = max_health
        .decisions
        .iter()
        .filter_map(|decision| match &decision.source {
            SourceInstanceIdentity::Effect {
                effect,
                stack,
                source: source_id,
                ..
            } if source_id == &source("temporary_maximum") => Some((effect.clone(), *stack)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        effect_stacks,
        vec![
            (effect_instance("damage_ward"), 1),
            (effect_instance("damage_ward"), 2),
            (effect_instance("stacked_improvement"), 1),
            (effect_instance("stacked_improvement"), 2),
        ]
    );
    assert!(max_health
        .decisions
        .iter()
        .filter(|decision| decision.source_definition == source("temporary_maximum"))
        .all(|decision| decision.outcome == DecisionOutcome::Applied));

    let damage_operation = operation("preview_warded_damage");
    let preview = DamageService::preview(
        &state,
        &catalog,
        &DamageRequest {
            operation: damage_operation.clone(),
            source: provenance("preview_warded_damage", "damage_origin"),
            actor: None,
            target: TARGET,
            target_track: track("health"),
            parts: vec![DamagePart {
                amount: scalar(20),
                kind: DamageKindId::parse("impact").unwrap(),
            }],
            request_sources: vec![],
            expected_tracks_revision: None,
        },
    )
    .unwrap();
    assert!(preview.receipt().parts[0].prevented);
    assert!(preview.receipt().decisions.iter().any(|decision| {
        decision.source_definition == source("temporary_prevention")
            && decision.outcome == DecisionOutcome::Applied
    }));

    let view = MechanicsEntityView::read(&state, TARGET).unwrap();
    let effects = view.active_effects().unwrap();
    assert_eq!(
        effects.revision(),
        &state
            .component_revision::<ActiveEffectsComponent>(TARGET)
            .unwrap()
    );
    assert_eq!(effects.effects().len(), 2);
    assert_eq!(effects.activated_sources(&catalog).unwrap().len(), 8);
    assert_eq!(
        effects.effects()[0].provenance(),
        &provenance("apply_damage_ward", "ward_owner")
    );
}

#[test]
fn bound_lowering_rejects_then_uses_explicit_track_reconciliation() {
    let catalog = catalog();
    let mut state = state();
    let applied = EffectService::apply(
        &mut state,
        &catalog,
        apply_request(
            BUILDING,
            "apply_building_improvement",
            "building_improvement",
            "improvement",
            "phase_owner",
            1,
        ),
    )
    .unwrap();
    TrackService::restore(
        &mut state,
        &catalog,
        TrackMutationRequest {
            operation: operation("restore_to_improved_maximum"),
            source: provenance("restore_to_improved_maximum", "repair"),
            entity: BUILDING,
            track: track("health"),
            amount: scalar(50),
            kind: gameplay_mechanics::TrackAdjustmentKind::Restore,
            expected_revision: None,
        },
    )
    .unwrap();
    assert_eq!(
        state
            .component::<TracksComponent>(BUILDING)
            .unwrap()
            .unwrap()
            .current(&track("health")),
        Some(scalar(150))
    );

    let effects_before = state
        .component::<ActiveEffectsComponent>(BUILDING)
        .unwrap()
        .unwrap()
        .clone();
    let effects_revision_before = state
        .component_revision::<ActiveEffectsComponent>(BUILDING)
        .unwrap();
    let tracks_revision_before = state
        .component_revision::<TracksComponent>(BUILDING)
        .unwrap();
    let global_before = state.revision();
    assert!(matches!(
        EffectService::remove(
            &mut state,
            &catalog,
            EffectRemovalRequest {
                operation: operation("remove_building_improvement"),
                entity: BUILDING,
                instance: effect_instance("building_improvement"),
                expected_revision: Some(effects_revision_before.clone()),
            },
        ),
        Err(MechanicsError::EffectWouldInvalidateTrack {
            current: 150,
            prospective_maximum: 100,
            ..
        })
    ));
    assert_eq!(
        state
            .component::<ActiveEffectsComponent>(BUILDING)
            .unwrap()
            .unwrap(),
        &effects_before
    );
    assert_eq!(
        state
            .component_revision::<ActiveEffectsComponent>(BUILDING)
            .unwrap(),
        effects_revision_before
    );
    assert_eq!(
        state
            .component_revision::<TracksComponent>(BUILDING)
            .unwrap(),
        tracks_revision_before
    );
    assert_eq!(state.revision(), global_before);

    TrackService::reconcile_to_maximum(
        &mut state,
        &catalog,
        TrackReconciliationRequest {
            operation: operation("reconcile_building_improvement"),
            source: provenance("reconcile_building_improvement", "phase_owner"),
            entity: BUILDING,
            track: track("health"),
            prospective_maximum: scalar(100),
            policy: TrackReconciliationPolicy::ClampToMaximum,
            expected_revision: Some(tracks_revision_before),
        },
    )
    .unwrap();
    let removed = EffectService::remove(
        &mut state,
        &catalog,
        EffectRemovalRequest {
            operation: operation("remove_building_improvement"),
            entity: BUILDING,
            instance: effect_instance("building_improvement"),
            expected_revision: Some(effects_revision_before),
        },
    )
    .unwrap();
    assert_eq!(removed.kind, EffectMutationKind::Remove);
    assert_eq!(
        removed.observed_effects_revision,
        applied.committed_effects_revision
    );
    assert_eq!(
        state
            .component::<TracksComponent>(BUILDING)
            .unwrap()
            .unwrap()
            .current(&track("health")),
        Some(scalar(100))
    );
}

#[test]
fn explicit_expiry_calls_fit_realtime_phase_and_turn_owners_without_time_state() {
    let catalog = catalog();
    let mut state = state();
    for (entity, owner_kind) in [
        (REALTIME_OWNER_TARGET, "realtime_tick"),
        (CITY_OWNER_TARGET, "city_phase"),
        (TABLETOP_OWNER_TARGET, "tabletop_turn"),
    ] {
        let instance = format!("{owner_kind}_effect");
        EffectService::apply(
            &mut state,
            &catalog,
            apply_request(
                entity,
                &format!("{owner_kind}_apply"),
                &instance,
                "improvement",
                &format!("{owner_kind}_owner"),
                1,
            ),
        )
        .unwrap();
        let expired = EffectService::expire(
            &mut state,
            &catalog,
            removal_request(entity, &format!("{owner_kind}_expire"), &instance),
        )
        .unwrap();
        assert_eq!(expired.kind, EffectMutationKind::Expire);
        assert!(state
            .component::<ActiveEffectsComponent>(entity)
            .unwrap()
            .unwrap()
            .effects()
            .is_empty());
    }
}

#[test]
fn strict_snapshot_persists_only_authoritative_effect_state_and_reconstructs_views() {
    let catalog = catalog();
    let mut state = state();
    EffectService::apply(
        &mut state,
        &catalog,
        apply_request(
            TARGET,
            "snapshot_apply",
            "snapshot_effect",
            "improvement",
            "snapshot_owner",
            2,
        ),
    )
    .unwrap();
    let encoded = encode_snapshot(&state).unwrap();
    assert!(encoded.contains(ACTIVE_EFFECTS_COMPONENT_TYPE_ID));
    let encoded_value: serde_json::Value = serde_json::from_str(&encoded).unwrap();
    let active_effects = encoded_value["registeredComponents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|component| component["typeId"] == ACTIVE_EFFECTS_COMPONENT_TYPE_ID)
        .unwrap();
    assert_eq!(active_effects["version"], 2);
    for forbidden in [
        "timestamp",
        "duration",
        "remainingTurns",
        "scheduler",
        "callback",
    ] {
        assert!(!encoded.contains(forbidden));
    }

    let restored = decode_snapshot_with_catalog(&encoded, &catalog).unwrap();
    assert_eq!(
        restored
            .component::<ActiveEffectsComponent>(TARGET)
            .unwrap()
            .unwrap(),
        state
            .component::<ActiveEffectsComponent>(TARGET)
            .unwrap()
            .unwrap()
    );
    let before = StatService::evaluate(
        &state,
        &catalog,
        TARGET,
        &stat("production"),
        &operation("snapshot_before"),
        &[],
    )
    .unwrap();
    let after = StatService::evaluate(
        &restored,
        &catalog,
        TARGET,
        &stat("production"),
        &operation("snapshot_after"),
        &[],
    )
    .unwrap();
    assert_eq!(before.value, after.value);
    assert_eq!(before.decisions.len(), after.decisions.len());

    let mut unknown_field: serde_json::Value = serde_json::from_str(&encoded).unwrap();
    let active_effects = unknown_field["registeredComponents"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|component| component["typeId"] == ACTIVE_EFFECTS_COMPONENT_TYPE_ID)
        .unwrap();
    active_effects["values"][0]["value"]["effects"][0]["timestamp"] = serde_json::Value::from(1);
    assert!(matches!(
        decode_snapshot_with_catalog(&unknown_field.to_string(), &catalog),
        Err(MechanicsSnapshotError::EntityState(_))
    ));

    let mut unresolved_definition = definition();
    unresolved_definition
        .effects
        .retain(|definition| definition.id != effect("improvement"));
    let unresolved_catalog = MechanicsCatalog::admit(unresolved_definition).unwrap();
    assert!(matches!(
        decode_snapshot_with_catalog(&encoded, &unresolved_catalog),
        Err(MechanicsSnapshotError::Mechanics(
            MechanicsError::InvalidCatalogReference { .. }
        ))
    ));

    let destroy_revision = state.revision();
    EntityAuthoringService
        .destroy(&mut state, destroy_revision, TARGET)
        .unwrap();
    assert!(matches!(
        MechanicsEntityView::read(&state, TARGET),
        Err(MechanicsError::MissingEntity { entity }) if entity == TARGET
    ));
}

#[test]
fn exact_ratio_source_behavior_remains_unrelated_to_effect_lifecycle_policy() {
    let mut definition = definition();
    definition.sources.push(source_definition(
        "ratio_source",
        20,
        vec![StatContributionDefinition {
            stat: stat("production"),
            contribution: StatContribution::Scale {
                ratio: ExactRatio::new(3, 2).unwrap(),
            },
            stacking_group: group("ratio_source"),
            stacking: StackingPolicy::Sum,
        }],
        vec![],
    ));
    definition.effects.push(effect_definition(
        "ratio_effect",
        "ratio_lifecycle",
        EffectStackingPolicy::Refresh,
        1,
        &["ratio_source"],
    ));
    let catalog = MechanicsCatalog::admit(definition).unwrap();
    let mut state = state();
    EffectService::apply(
        &mut state,
        &catalog,
        apply_request(
            TARGET,
            "apply_ratio_effect",
            "ratio_effect_one",
            "ratio_effect",
            "ratio_owner",
            1,
        ),
    )
    .unwrap();
    let evaluated = StatService::evaluate(
        &state,
        &catalog,
        TARGET,
        &stat("production"),
        &operation("inspect_ratio_effect"),
        &[],
    )
    .unwrap();
    assert_eq!(evaluated.value, scalar(0));
    assert_eq!(
        (
            evaluated.combined_scale_numerator,
            evaluated.combined_scale_denominator,
        ),
        (3, 2)
    );
}
