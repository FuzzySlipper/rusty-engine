use core_ids::EntityId;
use entity_state::{
    encode_snapshot, EntityAuthoringService, EntityComponent, EntityDefinition, EntityState,
};
use gameplay_mechanics::{
    decode_snapshot_with_catalog, CatalogError, CatalogVersion, DecisionOutcome, ExactRatio,
    IntrinsicSourceBinding, IntrinsicSourcesComponent, MechanicsCatalog,
    MechanicsCatalogDefinition, MechanicsEntityView, MechanicsError, MechanicsScalar,
    MechanicsSnapshotError, OperationId, RequestSource, SourceDefinition, SourceDefinitionId,
    SourceInstanceId, SourceInstanceIdentity, StackingGroupId, StackingPolicy,
    StatBaseMutationRequest, StatContribution, StatContributionDefinition, StatDefinition, StatId,
    StatService, StatValue, StatsComponent, TrackAdjustmentKind, TrackDefinition, TrackId,
    TrackMaximum, TrackMutationRequest, TrackReconciliationPolicy, TrackReconciliationRequest,
    TrackService, TrackSetPolicy, TrackSetRequest, TrackValue, TracksComponent,
    INTRINSIC_SOURCES_COMPONENT_TYPE_ID, MAX_ABS_MECHANICS_SCALAR, STATS_COMPONENT_TYPE_ID,
};

const SHOOTER: EntityId = EntityId::new(101);
const INFRASTRUCTURE: EntityId = EntityId::new(102);
const D20_SHAPED: EntityId = EntityId::new(103);
const BASE_ONLY: EntityId = EntityId::new(104);

fn scalar(value: i64) -> MechanicsScalar {
    MechanicsScalar::new(value).unwrap()
}

fn id<T, E: std::fmt::Debug>(value: &str, parse: impl FnOnce(String) -> Result<T, E>) -> T {
    parse(value.to_string()).unwrap()
}

fn stat(value: &str) -> StatId {
    id(value, StatId::parse)
}

fn track(value: &str) -> TrackId {
    id(value, TrackId::parse)
}

fn source(value: &str) -> SourceDefinitionId {
    id(value, SourceDefinitionId::parse)
}

fn group(value: &str) -> StackingGroupId {
    id(value, StackingGroupId::parse)
}

fn operation(value: &str) -> OperationId {
    id(value, OperationId::parse)
}

fn request_identity(operation: &OperationId, instance: &str) -> SourceInstanceIdentity {
    SourceInstanceIdentity::Request {
        operation: operation.clone(),
        instance: id(instance, SourceInstanceId::parse),
    }
}

fn add(
    stat: &str,
    amount: i64,
    stacking_group: &str,
    stacking: StackingPolicy,
) -> StatContributionDefinition {
    StatContributionDefinition {
        stat: self::stat(stat),
        contribution: StatContribution::Add {
            amount: scalar(amount),
        },
        stacking_group: group(stacking_group),
        stacking,
    }
}

fn scale(
    stat: &str,
    numerator: u32,
    denominator: u32,
    stacking_group: &str,
) -> StatContributionDefinition {
    StatContributionDefinition {
        stat: self::stat(stat),
        contribution: StatContribution::Scale {
            ratio: ExactRatio::new(numerator, denominator).unwrap(),
        },
        stacking_group: group(stacking_group),
        stacking: StackingPolicy::Sum,
    }
}

fn source_definition(
    id: &str,
    priority: i16,
    stat_contributions: Vec<StatContributionDefinition>,
) -> SourceDefinition {
    SourceDefinition {
        id: source(id),
        priority,
        stat_contributions,
        damage_responses: vec![],
    }
}

fn gm1_definition(version: &str) -> MechanicsCatalogDefinition {
    MechanicsCatalogDefinition {
        version: CatalogVersion::parse(version).unwrap(),
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
            StatDefinition {
                id: stat("armor_class"),
                minimum: scalar(0),
                maximum: scalar(100),
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
                "health_bonus_a",
                0,
                vec![add(
                    "max_health",
                    20,
                    "health_bonus",
                    StackingPolicy::Highest,
                )],
            ),
            source_definition(
                "health_bonus_b",
                0,
                vec![add(
                    "max_health",
                    20,
                    "health_bonus",
                    StackingPolicy::Highest,
                )],
            ),
            source_definition(
                "health_scale_half",
                10,
                vec![scale("max_health", 1, 2, "health_scale")],
            ),
            source_definition(
                "health_scale_three_halves",
                10,
                vec![scale("max_health", 3, 2, "health_scale")],
            ),
            source_definition(
                "health_floor",
                20,
                vec![StatContributionDefinition {
                    stat: stat("max_health"),
                    contribution: StatContribution::Minimum { value: scalar(95) },
                    stacking_group: group("health_floor"),
                    stacking: StackingPolicy::Highest,
                }],
            ),
            source_definition(
                "health_cap",
                20,
                vec![StatContributionDefinition {
                    stat: stat("max_health"),
                    contribution: StatContribution::Maximum { value: scalar(110) },
                    stacking_group: group("health_cap"),
                    stacking: StackingPolicy::Lowest,
                }],
            ),
            source_definition(
                "production_upgrade",
                5,
                vec![
                    add("production", 10, "production_bonus", StackingPolicy::Sum),
                    add(
                        "max_health",
                        0,
                        "production_health_marker",
                        StackingPolicy::Sum,
                    ),
                ],
            ),
            source_definition(
                "unrelated_production",
                5,
                vec![add(
                    "production",
                    1,
                    "unrelated_production",
                    StackingPolicy::Sum,
                )],
            ),
            source_definition(
                "shield_training",
                0,
                vec![add(
                    "armor_class",
                    2,
                    "armor_training",
                    StackingPolicy::UniqueBySource,
                )],
            ),
            source_definition(
                "armor_item_bonus",
                0,
                vec![add(
                    "armor_class",
                    3,
                    "armor_equipment",
                    StackingPolicy::Highest,
                )],
            ),
            source_definition(
                "overflow_bonus",
                30,
                vec![add(
                    "max_health",
                    MAX_ABS_MECHANICS_SCALAR,
                    "overflow",
                    StackingPolicy::Sum,
                )],
            ),
        ],
        damage_kinds: vec![],
        effects: vec![],
        capacity_metrics: vec![],
        items: vec![],
        equipment_slots: vec![],
    }
}

fn catalog() -> MechanicsCatalog {
    MechanicsCatalog::admit(gm1_definition("gm1.v1")).unwrap()
}

fn state(definitions: impl IntoIterator<Item = EntityDefinition>) -> EntityState {
    EntityState::from_definitions_with_registry(
        gameplay_mechanics::gameplay_component_registry().unwrap(),
        definitions,
    )
    .unwrap()
}

fn attach<T: EntityComponent>(state: &mut EntityState, entity: EntityId, value: T) {
    let revision = state.component_revision::<T>(entity).unwrap();
    EntityAuthoringService
        .attach_component(state, revision, entity, value)
        .unwrap();
}

fn stats(version: &str, values: &[(&str, i64)]) -> StatsComponent {
    StatsComponent::new(
        CatalogVersion::parse(version).unwrap(),
        values
            .iter()
            .map(|(id, base)| StatValue::new(stat(id), scalar(*base)))
            .collect(),
    )
    .unwrap()
}

fn tracks(version: &str, values: &[(&str, i64)]) -> TracksComponent {
    TracksComponent::new(
        CatalogVersion::parse(version).unwrap(),
        values
            .iter()
            .map(|(id, current)| TrackValue::new(track(id), scalar(*current)))
            .collect(),
    )
    .unwrap()
}

fn bindings(version: &str, values: &[(&str, &str)]) -> IntrinsicSourcesComponent {
    IntrinsicSourcesComponent::new(
        CatalogVersion::parse(version).unwrap(),
        values
            .iter()
            .map(|(instance, definition)| {
                IntrinsicSourceBinding::new(
                    id(instance, SourceInstanceId::parse),
                    source(definition),
                )
            })
            .collect(),
    )
    .unwrap()
}

#[test]
fn catalog_canonicalizes_nested_definitions_and_separates_version_from_fingerprint() {
    let original = gm1_definition("gm1.v1");
    let mut reordered = original.clone();
    reordered.stats.reverse();
    reordered.tracks.reverse();
    reordered.sources.reverse();
    for definition in &mut reordered.sources {
        definition.stat_contributions.reverse();
    }
    let first = MechanicsCatalog::admit(original).unwrap();
    let second = MechanicsCatalog::admit(reordered).unwrap();
    assert_eq!(first.fingerprint(), second.fingerprint());
    assert_eq!(first.sources(), second.sources());

    let different_version = MechanicsCatalog::admit(gm1_definition("gm1.v2")).unwrap();
    assert_ne!(first.version(), different_version.version());
    assert_eq!(first.fingerprint(), different_version.fingerprint());

    let view = first.view();
    assert_eq!(view.version().as_str(), "gm1.v1");
    assert_eq!(view.fingerprint(), first.fingerprint());
    assert_eq!(view.stats().len(), 3);
    assert!(view.stats().windows(2).all(|pair| pair[0].id < pair[1].id));

    let mut inconsistent = gm1_definition("gm1.invalid");
    inconsistent.sources.push(source_definition(
        "mixed_operation",
        0,
        vec![StatContributionDefinition {
            stat: stat("max_health"),
            contribution: StatContribution::Scale {
                ratio: ExactRatio::new(1, 1).unwrap(),
            },
            stacking_group: group("health_bonus"),
            stacking: StackingPolicy::Highest,
        }],
    ));
    assert!(matches!(
        MechanicsCatalog::admit(inconsistent),
        Err(CatalogError::InconsistentStatContributionKind { .. })
    ));
}

#[test]
fn stat_evaluation_is_canonical_attributed_and_bounded() {
    let catalog = catalog();
    let mut state = state([EntityDefinition::new(SHOOTER, "shooter")]);
    attach(&mut state, SHOOTER, stats("gm1.v1", &[("max_health", 100)]));
    attach(
        &mut state,
        SHOOTER,
        bindings(
            "gm1.v1",
            &[
                ("z_bonus", "health_bonus_b"),
                ("a_bonus", "health_bonus_a"),
                ("floor", "health_floor"),
                ("cap", "health_cap"),
                ("irrelevant", "production_upgrade"),
                ("inapplicable", "unrelated_production"),
            ],
        ),
    );
    let evaluation_operation = operation("evaluate_max_health");
    let first = StatService::evaluate(
        &state,
        &catalog,
        SHOOTER,
        &stat("max_health"),
        &evaluation_operation,
        &[
            RequestSource {
                instance: id("z_scale", SourceInstanceId::parse),
                definition: source("health_scale_three_halves"),
            },
            RequestSource {
                instance: id("a_scale", SourceInstanceId::parse),
                definition: source("health_scale_half"),
            },
        ],
    )
    .unwrap();
    let second = StatService::evaluate(
        &state,
        &catalog,
        SHOOTER,
        &stat("max_health"),
        &evaluation_operation,
        &[
            RequestSource {
                instance: id("a_scale", SourceInstanceId::parse),
                definition: source("health_scale_half"),
            },
            RequestSource {
                instance: id("z_scale", SourceInstanceId::parse),
                definition: source("health_scale_three_halves"),
            },
        ],
    )
    .unwrap();
    assert_eq!(first, second);
    assert_eq!(first.base.get(), 100);
    assert_eq!(first.after_additions.get(), 120);
    assert_eq!(
        (
            first.combined_scale_numerator,
            first.combined_scale_denominator
        ),
        (3, 4)
    );
    assert_eq!(first.after_scaling.get(), 90);
    assert_eq!((first.minimum.get(), first.maximum.get()), (95, 110));
    assert_eq!(first.value.get(), 95);
    assert_eq!(
        first
            .decisions
            .iter()
            .filter(|decision| {
                matches!(
                    decision.contribution.as_ref(),
                    Some(StatContribution::Add { amount }) if *amount == scalar(20)
                )
            })
            .filter(|decision| decision.outcome == DecisionOutcome::Applied)
            .count(),
        1
    );
    let applied_bonus = first
        .decisions
        .iter()
        .find(|decision| {
            matches!(
                decision.contribution.as_ref(),
                Some(StatContribution::Add { amount }) if *amount == scalar(20)
            ) && decision.outcome == DecisionOutcome::Applied
        })
        .unwrap();
    assert_eq!(
        applied_bonus.source,
        SourceInstanceIdentity::Intrinsic {
            entity: SHOOTER,
            instance: id("a_bonus", SourceInstanceId::parse),
        }
    );
    assert!(first.decisions.iter().any(|decision| {
        decision.source_definition == source("unrelated_production")
            && decision.outcome == DecisionOutcome::Inapplicable
    }));
}

#[test]
fn base_only_evaluation_visits_no_absent_feature_entries() {
    let catalog = catalog();
    let mut state = state([EntityDefinition::new(BASE_ONLY, "base-only")]);
    attach(
        &mut state,
        BASE_ONLY,
        stats("gm1.v1", &[("production", -11)]),
    );
    let evaluation = StatService::evaluate(
        &state,
        &catalog,
        BASE_ONLY,
        &stat("production"),
        &operation("base_only"),
        &[],
    )
    .unwrap();
    assert_eq!(evaluation.value.get(), -11);
    assert!(evaluation.decisions.is_empty());
    assert_eq!(evaluation.source_cost.intrinsic_entries_visited, 0);
    assert_eq!(evaluation.source_cost.effect_entries_visited, 0);
    assert_eq!(evaluation.source_cost.equipment_entries_visited, 0);
    assert_eq!(evaluation.source_cost.item_components_read, 0);
    assert_eq!(evaluation.source_cost.request_entries_visited, 0);
}

#[test]
fn duplicate_live_sources_and_arithmetic_failure_are_read_only() {
    let catalog = catalog();
    let mut state = state([EntityDefinition::new(SHOOTER, "shooter")]);
    attach(&mut state, SHOOTER, stats("gm1.v1", &[("max_health", 100)]));
    let before_revision = state.revision();
    let before_stats_revision = state.component_revision::<StatsComponent>(SHOOTER).unwrap();
    let duplicate_operation = operation("duplicate_request");
    assert!(matches!(
        StatService::evaluate(
            &state,
            &catalog,
            SHOOTER,
            &stat("max_health"),
            &duplicate_operation,
            &[
                RequestSource {
                    instance: id("same", SourceInstanceId::parse),
                    definition: source("health_bonus_a"),
                },
                RequestSource {
                    instance: id("same", SourceInstanceId::parse),
                    definition: source("health_scale_half"),
                },
            ],
        ),
        Err(MechanicsError::DuplicateSource { .. })
    ));
    assert_eq!(state.revision(), before_revision);
    assert_eq!(
        state.component_revision::<StatsComponent>(SHOOTER).unwrap(),
        before_stats_revision
    );

    assert!(matches!(
        StatService::evaluate(
            &state,
            &catalog,
            SHOOTER,
            &stat("max_health"),
            &operation("overflow"),
            &[RequestSource {
                instance: id("overflow", SourceInstanceId::parse),
                definition: source("overflow_bonus"),
            }],
        ),
        Err(MechanicsError::Arithmetic(_))
    ));
    assert_eq!(state.revision(), before_revision);
    assert_eq!(
        state.component_revision::<StatsComponent>(SHOOTER).unwrap(),
        before_stats_revision
    );
}

#[test]
fn guarded_base_change_requires_track_reconciliation_and_preserves_failures() {
    let catalog = catalog();
    let mut state = state([EntityDefinition::new(SHOOTER, "shooter")]);
    attach(&mut state, SHOOTER, stats("gm1.v1", &[("max_health", 100)]));
    attach(&mut state, SHOOTER, tracks("gm1.v1", &[("health", 100)]));
    let stats_revision = state.component_revision::<StatsComponent>(SHOOTER).unwrap();
    let global_before = state.revision();
    let set_operation = operation("lower_max_health");
    let request = StatBaseMutationRequest {
        operation: set_operation.clone(),
        source: request_identity(&set_operation, "base_change"),
        entity: SHOOTER,
        stat: stat("max_health"),
        base: scalar(50),
        expected_revision: Some(stats_revision.clone()),
    };
    assert!(matches!(
        StatService::set_base(&mut state, &catalog, request.clone()),
        Err(MechanicsError::TrackOutOfBounds {
            attempted: 100,
            maximum: 50,
            ..
        })
    ));
    assert_eq!(state.revision(), global_before);
    assert_eq!(
        state.component_revision::<StatsComponent>(SHOOTER).unwrap(),
        stats_revision
    );

    let reconcile_operation = operation("reconcile_health");
    let tracks_revision = state
        .component_revision::<TracksComponent>(SHOOTER)
        .unwrap();
    let global_before_reconcile = state.revision();
    let reconciliation = TrackService::reconcile_to_maximum(
        &mut state,
        &catalog,
        TrackReconciliationRequest {
            operation: reconcile_operation.clone(),
            source: request_identity(&reconcile_operation, "reconcile"),
            entity: SHOOTER,
            track: track("health"),
            prospective_maximum: scalar(50),
            policy: TrackReconciliationPolicy::ClampToMaximum,
            expected_revision: Some(tracks_revision),
        },
    )
    .unwrap();
    assert_eq!(
        (reconciliation.before.get(), reconciliation.after.get()),
        (100, 50)
    );
    assert_eq!(state.revision(), global_before_reconcile + 1);

    let global_before_base = state.revision();
    let receipt = StatService::set_base(&mut state, &catalog, request).unwrap();
    assert_eq!((receipt.before.get(), receipt.after.get()), (100, 50));
    assert_eq!(state.revision(), global_before_base + 1);
    assert_eq!(
        receipt.committed_stats_revision,
        receipt.observed_stats_revision + 1
    );
    assert_eq!(
        state
            .component::<TracksComponent>(SHOOTER)
            .unwrap()
            .unwrap()
            .current(&track("health"))
            .unwrap()
            .get(),
        50
    );

    let global_before_invalid = state.revision();
    let stats_before_invalid = state.component_revision::<StatsComponent>(SHOOTER).unwrap();
    let invalid_operation = operation("invalid_base");
    assert!(matches!(
        StatService::set_base(
            &mut state,
            &catalog,
            StatBaseMutationRequest {
                operation: invalid_operation.clone(),
                source: request_identity(&invalid_operation, "invalid"),
                entity: SHOOTER,
                stat: stat("max_health"),
                base: scalar(1_001),
                expected_revision: Some(stats_before_invalid.clone()),
            },
        ),
        Err(MechanicsError::StatOutOfBounds {
            attempted: 1_001,
            ..
        })
    ));
    assert_eq!(state.revision(), global_before_invalid);
    assert_eq!(
        state.component_revision::<StatsComponent>(SHOOTER).unwrap(),
        stats_before_invalid
    );

    let global_before_stale = state.revision();
    let component_before_stale = state
        .component::<StatsComponent>(SHOOTER)
        .unwrap()
        .unwrap()
        .clone();
    let stale_operation = operation("stale_base");
    assert!(matches!(
        StatService::set_base(
            &mut state,
            &catalog,
            StatBaseMutationRequest {
                operation: stale_operation.clone(),
                source: request_identity(&stale_operation, "stale"),
                entity: SHOOTER,
                stat: stat("max_health"),
                base: scalar(60),
                expected_revision: Some(stats_revision),
            },
        ),
        Err(MechanicsError::StaleComponentRevision { .. })
    ));
    assert_eq!(state.revision(), global_before_stale);
    assert_eq!(
        state.component::<StatsComponent>(SHOOTER).unwrap().unwrap(),
        &component_before_stale
    );
}

#[test]
fn track_set_and_reconciliation_policies_are_explicit_and_fail_atomic() {
    let catalog = catalog();
    let mut state = state([EntityDefinition::new(INFRASTRUCTURE, "infrastructure")]);
    attach(
        &mut state,
        INFRASTRUCTURE,
        tracks("gm1.v1", &[("durability", 100)]),
    );
    let before = state
        .component::<TracksComponent>(INFRASTRUCTURE)
        .unwrap()
        .unwrap()
        .clone();
    let before_revision = state
        .component_revision::<TracksComponent>(INFRASTRUCTURE)
        .unwrap();
    let before_global = state.revision();
    let rejected_operation = operation("reject_set");
    assert!(matches!(
        TrackService::set_under_policy(
            &mut state,
            &catalog,
            TrackSetRequest {
                operation: rejected_operation.clone(),
                source: request_identity(&rejected_operation, "set"),
                entity: INFRASTRUCTURE,
                track: track("durability"),
                value: scalar(250),
                policy: TrackSetPolicy::RejectOutOfBounds,
                expected_revision: Some(before_revision.clone()),
            },
        ),
        Err(MechanicsError::TrackOutOfBounds { attempted: 250, .. })
    ));
    assert_eq!(state.revision(), before_global);
    assert_eq!(
        state
            .component::<TracksComponent>(INFRASTRUCTURE)
            .unwrap()
            .unwrap(),
        &before
    );
    assert_eq!(
        state
            .component_revision::<TracksComponent>(INFRASTRUCTURE)
            .unwrap(),
        before_revision
    );

    let clamp_operation = operation("clamp_set");
    let global_before_clamp = state.revision();
    let clamped = TrackService::set_under_policy(
        &mut state,
        &catalog,
        TrackSetRequest {
            operation: clamp_operation.clone(),
            source: request_identity(&clamp_operation, "set"),
            entity: INFRASTRUCTURE,
            track: track("durability"),
            value: scalar(250),
            policy: TrackSetPolicy::ClampToBounds,
            expected_revision: None,
        },
    )
    .unwrap();
    assert_eq!((clamped.requested.get(), clamped.after.get()), (250, 200));
    assert_eq!(state.revision(), global_before_clamp + 1);
    assert_eq!(
        clamped.committed_tracks_revision,
        clamped.observed_tracks_revision + 1
    );

    let stale_operation = operation("stale_set");
    let stale_global = state.revision();
    let stale_component = state
        .component::<TracksComponent>(INFRASTRUCTURE)
        .unwrap()
        .unwrap()
        .clone();
    assert!(matches!(
        TrackService::set_under_policy(
            &mut state,
            &catalog,
            TrackSetRequest {
                operation: stale_operation.clone(),
                source: request_identity(&stale_operation, "set"),
                entity: INFRASTRUCTURE,
                track: track("durability"),
                value: scalar(175),
                policy: TrackSetPolicy::RejectOutOfBounds,
                expected_revision: Some(before_revision),
            },
        ),
        Err(MechanicsError::StaleComponentRevision { .. })
    ));
    assert_eq!(state.revision(), stale_global);
    assert_eq!(
        state
            .component::<TracksComponent>(INFRASTRUCTURE)
            .unwrap()
            .unwrap(),
        &stale_component
    );

    let preserve_operation = operation("preserve_reconcile");
    let global_before_preserve = state.revision();
    assert!(matches!(
        TrackService::reconcile_to_maximum(
            &mut state,
            &catalog,
            TrackReconciliationRequest {
                operation: preserve_operation.clone(),
                source: request_identity(&preserve_operation, "reconcile"),
                entity: INFRASTRUCTURE,
                track: track("durability"),
                prospective_maximum: scalar(150),
                policy: TrackReconciliationPolicy::PreserveCurrent,
                expected_revision: None,
            },
        ),
        Err(MechanicsError::TrackOutOfBounds {
            attempted: 200,
            maximum: 150,
            ..
        })
    ));
    assert_eq!(state.revision(), global_before_preserve);

    let clamp_reconcile_operation = operation("clamp_reconcile");
    let reconciled = TrackService::reconcile_to_maximum(
        &mut state,
        &catalog,
        TrackReconciliationRequest {
            operation: clamp_reconcile_operation.clone(),
            source: request_identity(&clamp_reconcile_operation, "reconcile"),
            entity: INFRASTRUCTURE,
            track: track("durability"),
            prospective_maximum: scalar(150),
            policy: TrackReconciliationPolicy::ClampToMaximum,
            expected_revision: None,
        },
    )
    .unwrap();
    assert_eq!(reconciled.after.get(), 150);

    let preserve_revision = state
        .component_revision::<TracksComponent>(INFRASTRUCTURE)
        .unwrap();
    let preserve_global = state.revision();
    let preserve_valid_operation = operation("preserve_valid");
    let preserved = TrackService::reconcile_to_maximum(
        &mut state,
        &catalog,
        TrackReconciliationRequest {
            operation: preserve_valid_operation.clone(),
            source: request_identity(&preserve_valid_operation, "reconcile"),
            entity: INFRASTRUCTURE,
            track: track("durability"),
            prospective_maximum: scalar(150),
            policy: TrackReconciliationPolicy::PreserveCurrent,
            expected_revision: Some(preserve_revision.clone()),
        },
    )
    .unwrap();
    assert_eq!((preserved.before.get(), preserved.after.get()), (150, 150));
    assert_eq!(state.revision(), preserve_global);
    assert_eq!(
        state
            .component_revision::<TracksComponent>(INFRASTRUCTURE)
            .unwrap(),
        preserve_revision
    );

    let restore_operation = operation("bounded_restore");
    let restored = TrackService::restore(
        &mut state,
        &catalog,
        TrackMutationRequest {
            operation: restore_operation.clone(),
            source: request_identity(&restore_operation, "restore"),
            entity: INFRASTRUCTURE,
            track: track("durability"),
            amount: scalar(100),
            kind: TrackAdjustmentKind::Spend,
            expected_revision: None,
        },
    )
    .unwrap();
    assert_eq!(
        (restored.applied_amount.get(), restored.after.get()),
        (50, 200)
    );

    let overspend_operation = operation("overspend");
    let overspend_global = state.revision();
    let overspend_revision = state
        .component_revision::<TracksComponent>(INFRASTRUCTURE)
        .unwrap();
    assert!(matches!(
        TrackService::spend(
            &mut state,
            &catalog,
            TrackMutationRequest {
                operation: overspend_operation.clone(),
                source: request_identity(&overspend_operation, "spend"),
                entity: INFRASTRUCTURE,
                track: track("durability"),
                amount: scalar(201),
                kind: TrackAdjustmentKind::Restore,
                expected_revision: Some(overspend_revision.clone()),
            },
        ),
        Err(MechanicsError::TrackOutOfBounds { .. })
    ));
    assert_eq!(state.revision(), overspend_global);
    assert_eq!(
        state
            .component_revision::<TracksComponent>(INFRASTRUCTURE)
            .unwrap(),
        overspend_revision
    );

    let negative_operation = operation("negative_spend");
    let before_negative = state.revision();
    assert!(matches!(
        TrackService::spend(
            &mut state,
            &catalog,
            TrackMutationRequest {
                operation: negative_operation.clone(),
                source: request_identity(&negative_operation, "spend"),
                entity: INFRASTRUCTURE,
                track: track("durability"),
                amount: scalar(-1),
                kind: TrackAdjustmentKind::Restore,
                expected_revision: None,
            },
        ),
        Err(MechanicsError::Arithmetic(_))
    ));
    assert_eq!(state.revision(), before_negative);
}

#[test]
fn shooter_infrastructure_and_d20_shaped_compositions_share_the_generic_contract() {
    let catalog = catalog();
    let mut state = state([
        EntityDefinition::new(SHOOTER, "shooter"),
        EntityDefinition::new(INFRASTRUCTURE, "infrastructure"),
        EntityDefinition::new(D20_SHAPED, "d20-shaped"),
    ]);
    attach(&mut state, SHOOTER, stats("gm1.v1", &[("max_health", 100)]));
    attach(&mut state, SHOOTER, tracks("gm1.v1", &[("health", 100)]));
    attach(
        &mut state,
        INFRASTRUCTURE,
        stats("gm1.v1", &[("production", 40)]),
    );
    attach(
        &mut state,
        INFRASTRUCTURE,
        tracks("gm1.v1", &[("durability", 150)]),
    );
    attach(
        &mut state,
        INFRASTRUCTURE,
        bindings("gm1.v1", &[("upgrade", "production_upgrade")]),
    );
    attach(
        &mut state,
        D20_SHAPED,
        stats("gm1.v1", &[("armor_class", 10)]),
    );

    assert_eq!(
        StatService::evaluate(
            &state,
            &catalog,
            SHOOTER,
            &stat("max_health"),
            &operation("shooter_max_health"),
            &[],
        )
        .unwrap()
        .value
        .get(),
        100
    );
    let production = StatService::evaluate(
        &state,
        &catalog,
        INFRASTRUCTURE,
        &stat("production"),
        &operation("building_production"),
        &[],
    )
    .unwrap();
    assert_eq!(production.value.get(), 50);
    let repair_operation = operation("building_repair");
    let repaired = TrackService::restore(
        &mut state,
        &catalog,
        TrackMutationRequest {
            operation: repair_operation.clone(),
            source: request_identity(&repair_operation, "repair"),
            entity: INFRASTRUCTURE,
            track: track("durability"),
            amount: scalar(25),
            kind: TrackAdjustmentKind::Spend,
            expected_revision: None,
        },
    )
    .unwrap();
    assert_eq!(repaired.after.get(), 175);

    let d20_operation = operation("downstream_armor_class");
    let d20 = StatService::evaluate(
        &state,
        &catalog,
        D20_SHAPED,
        &stat("armor_class"),
        &d20_operation,
        &[
            RequestSource {
                instance: id("training_a", SourceInstanceId::parse),
                definition: source("shield_training"),
            },
            RequestSource {
                instance: id("training_b", SourceInstanceId::parse),
                definition: source("shield_training"),
            },
            RequestSource {
                instance: id("equipment", SourceInstanceId::parse),
                definition: source("armor_item_bonus"),
            },
        ],
    )
    .unwrap();
    assert_eq!(d20.value.get(), 15);
    assert_eq!(
        d20.decisions
            .iter()
            .filter(|decision| decision.source_definition == source("shield_training"))
            .filter(|decision| decision.outcome == DecisionOutcome::Applied)
            .count(),
        1
    );
}

#[test]
fn immutable_views_and_strict_snapshots_preserve_catalog_compatibility_rules() {
    let catalog = catalog();
    let mut state = state([EntityDefinition::new(SHOOTER, "shooter")]);
    attach(&mut state, SHOOTER, stats("gm1.v1", &[("max_health", 100)]));
    attach(&mut state, SHOOTER, tracks("gm1.v1", &[("health", 100)]));
    attach(
        &mut state,
        SHOOTER,
        bindings("gm1.v1", &[("bonus", "health_bonus_a")]),
    );
    let view = MechanicsEntityView::read(&state, SHOOTER).unwrap();
    assert_eq!(view.entity(), SHOOTER);
    assert_eq!(
        view.stats().unwrap().values()[0].stat(),
        &stat("max_health")
    );
    assert_eq!(
        view.tracks().unwrap().revision(),
        &state
            .component_revision::<TracksComponent>(SHOOTER)
            .unwrap()
    );
    assert_eq!(view.intrinsic_sources().unwrap().bindings().len(), 1);

    let encoded = encode_snapshot(&state).unwrap();
    let restored = decode_snapshot_with_catalog(&encoded, &catalog).unwrap();
    assert_eq!(encode_snapshot(&restored).unwrap(), encoded);

    let mut balance_change = gm1_definition("gm1.v1");
    balance_change
        .stats
        .iter_mut()
        .find(|definition| definition.id == stat("max_health"))
        .unwrap()
        .maximum = scalar(900);
    let balance_change = MechanicsCatalog::admit(balance_change).unwrap();
    assert_ne!(catalog.fingerprint(), balance_change.fingerprint());
    decode_snapshot_with_catalog(&encoded, &balance_change).unwrap();

    let incompatible = MechanicsCatalog::admit(gm1_definition("gm1.v2")).unwrap();
    assert!(matches!(
        decode_snapshot_with_catalog(&encoded, &incompatible),
        Err(MechanicsSnapshotError::Mechanics(
            MechanicsError::CatalogVersionMismatch { .. }
        ))
    ));

    let mut unresolved: serde_json::Value = serde_json::from_str(&encoded).unwrap();
    let intrinsic = unresolved["registeredComponents"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|component| component["typeId"] == INTRINSIC_SOURCES_COMPONENT_TYPE_ID)
        .unwrap();
    intrinsic["values"][0]["value"]["bindings"][0]["definition"] =
        serde_json::Value::String("missing_source".to_string());
    assert!(matches!(
        decode_snapshot_with_catalog(&unresolved.to_string(), &catalog),
        Err(MechanicsSnapshotError::Mechanics(
            MechanicsError::InvalidCatalogReference { .. }
        ))
    ));

    let mut unknown_field: serde_json::Value = serde_json::from_str(&encoded).unwrap();
    let stats = unknown_field["registeredComponents"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|component| component["typeId"] == STATS_COMPONENT_TYPE_ID)
        .unwrap();
    stats["values"][0]["value"]["unchecked"] = serde_json::Value::Bool(true);
    assert!(matches!(
        decode_snapshot_with_catalog(&unknown_field.to_string(), &catalog),
        Err(MechanicsSnapshotError::EntityState(_))
    ));

    let destroy_revision = state.revision();
    EntityAuthoringService
        .destroy(&mut state, destroy_revision, SHOOTER)
        .unwrap();
    assert!(matches!(
        MechanicsEntityView::read(&state, SHOOTER),
        Err(MechanicsError::MissingEntity { entity }) if entity == SHOOTER
    ));
}
