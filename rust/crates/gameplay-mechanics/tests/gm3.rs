use core_ids::EntityId;
use entity_state::{
    encode_snapshot, ComponentRevision, EntityAuthoringService, EntityComponent, EntityDefinition,
    EntityState,
};
use gameplay_mechanics::{
    decode_snapshot_with_catalog, ActiveEffectsComponent, CatalogVersion, DamageFact,
    DamageKindDefinition, DamageKindId, DamageKindSelector, DamagePart, DamageRequest,
    DamageResponseDefinition, DamageService, DecisionOutcome, EffectApplyRequest, EffectDefinition,
    EffectDefinitionId, EffectInstanceId, EffectService, EffectStackingPolicy, ExactRatio,
    IntrinsicSourceBinding, IntrinsicSourcesComponent, MechanicsArithmeticError, MechanicsCatalog,
    MechanicsCatalogDefinition, MechanicsError, MechanicsScalar, OperationId, RequestSource,
    ResponseDecisionKind, RoundingPolicy, SourceDefinition, SourceDefinitionId, SourceInstanceId,
    SourceInstanceIdentity, StackingGroupId, StackingPolicy, StatContribution,
    StatContributionDefinition, StatDefinition, StatId, StatValue, StatsComponent,
    TrackAdjustmentKind, TrackDefinition, TrackId, TrackMaximum, TrackMutationRequest,
    TrackService, TrackSetPolicy, TrackSetRequest, TrackValue, TracksComponent,
    MAX_ABS_MECHANICS_SCALAR,
};

const ATTACKER: EntityId = EntityId::new(1);
const SHOOTER: EntityId = EntityId::new(2);
const BUILDING: EntityId = EntityId::new(3);
const TABLETOP: EntityId = EntityId::new(4);
const SIMPLE: EntityId = EntityId::new(5);
const ABSORB_ORDER: EntityId = EntityId::new(6);
const LATE_FAILURE: EntityId = EntityId::new(7);
const MISSING_TRACKS: EntityId = EntityId::new(8);
const SCALE_OVERFLOW: EntityId = EntityId::new(9);
const FLAT_OVERFLOW: EntityId = EntityId::new(10);
const EFFECT_COST: EntityId = EntityId::new(11);

fn scalar(value: i64) -> MechanicsScalar {
    MechanicsScalar::new(value).unwrap()
}

fn version() -> CatalogVersion {
    CatalogVersion::parse("gm3.v1").unwrap()
}

fn stat(value: &str) -> StatId {
    StatId::parse(value).unwrap()
}

fn track(value: &str) -> TrackId {
    TrackId::parse(value).unwrap()
}

fn kind(value: &str) -> DamageKindId {
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

fn group(value: &str) -> StackingGroupId {
    StackingGroupId::parse(value).unwrap()
}

fn operation(value: &str) -> OperationId {
    OperationId::parse(value).unwrap()
}

fn request_identity(operation_id: &OperationId, instance: &str) -> SourceInstanceIdentity {
    SourceInstanceIdentity::Request {
        operation: operation_id.clone(),
        instance: SourceInstanceId::parse(instance).unwrap(),
    }
}

fn request_source(instance: &str, definition: &str) -> RequestSource {
    RequestSource {
        instance: SourceInstanceId::parse(instance).unwrap(),
        definition: source(definition),
    }
}

fn response_source(
    id: &str,
    priority: i16,
    responses: Vec<DamageResponseDefinition>,
) -> SourceDefinition {
    SourceDefinition {
        id: source(id),
        priority,
        stat_contributions: vec![],
        damage_responses: responses,
    }
}

fn catalog() -> MechanicsCatalog {
    let mut sources = vec![
        response_source(
            "armor",
            0,
            vec![
                DamageResponseDefinition::FlatReduction {
                    selector: DamageKindSelector::Exact {
                        damage_kind: kind("impact"),
                    },
                    amount: scalar(3),
                    stacking_group: group("armor_flat"),
                    stacking: StackingPolicy::Highest,
                },
                DamageResponseDefinition::Scale {
                    selector: DamageKindSelector::Exact {
                        damage_kind: kind("impact"),
                    },
                    ratio: ExactRatio::new(1, 2).unwrap(),
                    stacking_group: group("armor_scale"),
                    stacking: StackingPolicy::Lowest,
                },
                DamageResponseDefinition::Absorb {
                    selector: DamageKindSelector::Exact {
                        damage_kind: kind("impact"),
                    },
                    track: track("armor"),
                },
            ],
        ),
        response_source(
            "vulnerability",
            5,
            vec![DamageResponseDefinition::Scale {
                selector: DamageKindSelector::Exact {
                    damage_kind: kind("impact"),
                },
                ratio: ExactRatio::new(3, 2).unwrap(),
                stacking_group: group("vulnerability_scale"),
                stacking: StackingPolicy::Sum,
            }],
        ),
        response_source(
            "invulnerability",
            -10,
            vec![DamageResponseDefinition::Prevent {
                selector: DamageKindSelector::Any,
                stacking_group: group("prevention"),
                stacking: StackingPolicy::UniqueBySource,
            }],
        ),
        response_source(
            "low_flat",
            0,
            vec![DamageResponseDefinition::FlatReduction {
                selector: DamageKindSelector::Any,
                amount: scalar(2),
                stacking_group: group("competing_flat"),
                stacking: StackingPolicy::Highest,
            }],
        ),
        response_source(
            "high_flat",
            0,
            vec![DamageResponseDefinition::FlatReduction {
                selector: DamageKindSelector::Any,
                amount: scalar(5),
                stacking_group: group("competing_flat"),
                stacking: StackingPolicy::Highest,
            }],
        ),
        response_source(
            "half_scale",
            0,
            vec![DamageResponseDefinition::Scale {
                selector: DamageKindSelector::Any,
                ratio: ExactRatio::new(1, 2).unwrap(),
                stacking_group: group("half_scale"),
                stacking: StackingPolicy::Sum,
            }],
        ),
        response_source(
            "reserve_absorber",
            10,
            vec![DamageResponseDefinition::Absorb {
                selector: DamageKindSelector::Exact {
                    damage_kind: kind("impact"),
                },
                track: track("reserve"),
            }],
        ),
        response_source(
            "missing_absorber",
            0,
            vec![DamageResponseDefinition::Absorb {
                selector: DamageKindSelector::Exact {
                    damage_kind: kind("impact"),
                },
                track: track("missing_protection"),
            }],
        ),
        response_source(
            "flat_bound_a",
            0,
            vec![DamageResponseDefinition::FlatReduction {
                selector: DamageKindSelector::Any,
                amount: scalar(MAX_ABS_MECHANICS_SCALAR),
                stacking_group: group("flat_bound"),
                stacking: StackingPolicy::Sum,
            }],
        ),
        response_source(
            "flat_bound_b",
            0,
            vec![DamageResponseDefinition::FlatReduction {
                selector: DamageKindSelector::Any,
                amount: scalar(MAX_ABS_MECHANICS_SCALAR),
                stacking_group: group("flat_bound"),
                stacking: StackingPolicy::Sum,
            }],
        ),
    ];
    sources.push(response_source(
        "scale_overflow",
        0,
        (0..8)
            .map(|index| DamageResponseDefinition::Scale {
                selector: DamageKindSelector::Any,
                ratio: ExactRatio::new(1_000_000, 1).unwrap(),
                stacking_group: group(&format!("overflow_scale_{index}")),
                stacking: StackingPolicy::Sum,
            })
            .collect(),
    ));
    sources.push(SourceDefinition {
        id: source("maximum_bonus"),
        priority: 0,
        stat_contributions: vec![StatContributionDefinition {
            stat: stat("max_health"),
            contribution: StatContribution::Add { amount: scalar(50) },
            stacking_group: group("maximum_bonus"),
            stacking: StackingPolicy::Sum,
        }],
        damage_responses: vec![],
    });

    MechanicsCatalog::admit(MechanicsCatalogDefinition {
        version: version(),
        stats: vec![StatDefinition {
            id: stat("max_health"),
            minimum: scalar(0),
            maximum: scalar(1_000),
        }],
        tracks: vec![
            TrackDefinition {
                id: track("health"),
                minimum: scalar(0),
                maximum: TrackMaximum::Stat {
                    stat: stat("max_health"),
                },
            },
            TrackDefinition {
                id: track("armor"),
                minimum: scalar(0),
                maximum: TrackMaximum::Fixed { value: scalar(100) },
            },
            TrackDefinition {
                id: track("reserve"),
                minimum: scalar(0),
                maximum: TrackMaximum::Fixed { value: scalar(100) },
            },
            TrackDefinition {
                id: track("durability"),
                minimum: scalar(0),
                maximum: TrackMaximum::Fixed { value: scalar(300) },
            },
            TrackDefinition {
                id: track("missing_protection"),
                minimum: scalar(0),
                maximum: TrackMaximum::Fixed { value: scalar(100) },
            },
        ],
        sources,
        damage_kinds: vec![
            DamageKindDefinition { id: kind("energy") },
            DamageKindDefinition { id: kind("impact") },
        ],
        effects: vec![
            EffectDefinition {
                id: effect("fortify"),
                stacking_group: group("fortify"),
                stacking: EffectStackingPolicy::IndependentByProvenance {
                    maximum_instances: 2,
                },
                maximum_stacks: 1,
                sources: vec![source("maximum_bonus")],
            },
            EffectDefinition {
                id: effect("ward"),
                stacking_group: group("ward"),
                stacking: EffectStackingPolicy::Refresh,
                maximum_stacks: 1,
                sources: vec![source("invulnerability")],
            },
        ],
        items: vec![],
        equipment_slots: vec![],
    })
    .unwrap()
}

fn stats(value: i64) -> StatsComponent {
    StatsComponent::new(
        version(),
        vec![StatValue::new(stat("max_health"), scalar(value))],
    )
    .unwrap()
}

fn tracks(values: &[(&str, i64)]) -> TracksComponent {
    TracksComponent::new(
        version(),
        values
            .iter()
            .map(|(id, value)| TrackValue::new(track(id), scalar(*value)))
            .collect(),
    )
    .unwrap()
}

fn intrinsic(bindings: &[(&str, &str)]) -> IntrinsicSourcesComponent {
    IntrinsicSourcesComponent::new(
        version(),
        bindings
            .iter()
            .map(|(instance, definition)| {
                IntrinsicSourceBinding::new(
                    SourceInstanceId::parse(*instance).unwrap(),
                    source(definition),
                )
            })
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
    let registry = gameplay_mechanics::gameplay_component_registry().unwrap();
    let mut state = EntityState::from_definitions_with_registry(
        registry,
        [
            EntityDefinition::new(ATTACKER, "attacker"),
            EntityDefinition::new(SHOOTER, "shooter"),
            EntityDefinition::new(BUILDING, "building"),
            EntityDefinition::new(TABLETOP, "tabletop"),
            EntityDefinition::new(SIMPLE, "simple"),
            EntityDefinition::new(ABSORB_ORDER, "absorb-order"),
            EntityDefinition::new(LATE_FAILURE, "late-failure"),
            EntityDefinition::new(MISSING_TRACKS, "missing-tracks"),
            EntityDefinition::new(SCALE_OVERFLOW, "scale-overflow"),
            EntityDefinition::new(FLAT_OVERFLOW, "flat-overflow"),
            EntityDefinition::new(EFFECT_COST, "effect-cost"),
        ],
    )
    .unwrap();

    attach(&mut state, SHOOTER, stats(100));
    attach(
        &mut state,
        SHOOTER,
        tracks(&[("health", 100), ("armor", 20)]),
    );
    attach(
        &mut state,
        SHOOTER,
        intrinsic(&[
            ("armor_instance", "armor"),
            ("vulnerability_instance", "vulnerability"),
        ]),
    );

    attach(&mut state, BUILDING, tracks(&[("durability", 200)]));

    attach(&mut state, TABLETOP, stats(100));
    attach(&mut state, TABLETOP, tracks(&[("health", 100)]));
    attach(
        &mut state,
        TABLETOP,
        ActiveEffectsComponent::new(version(), vec![]).unwrap(),
    );

    attach(&mut state, SIMPLE, stats(100));
    attach(&mut state, SIMPLE, tracks(&[("health", 100)]));

    attach(&mut state, ABSORB_ORDER, stats(100));
    attach(
        &mut state,
        ABSORB_ORDER,
        tracks(&[("health", 100), ("armor", 5), ("reserve", 50)]),
    );
    attach(
        &mut state,
        ABSORB_ORDER,
        intrinsic(&[
            ("first_armor", "armor"),
            ("second_reserve", "reserve_absorber"),
        ]),
    );

    attach(&mut state, LATE_FAILURE, stats(100));
    attach(&mut state, LATE_FAILURE, tracks(&[("health", 100)]));
    attach(
        &mut state,
        LATE_FAILURE,
        intrinsic(&[("missing_protection_instance", "missing_absorber")]),
    );

    attach(&mut state, SCALE_OVERFLOW, stats(100));
    attach(&mut state, SCALE_OVERFLOW, tracks(&[("health", 100)]));
    attach(
        &mut state,
        SCALE_OVERFLOW,
        intrinsic(&[("overflow_instance", "scale_overflow")]),
    );

    attach(&mut state, FLAT_OVERFLOW, stats(100));
    attach(&mut state, FLAT_OVERFLOW, tracks(&[("health", 100)]));
    attach(
        &mut state,
        FLAT_OVERFLOW,
        intrinsic(&[
            ("flat_bound_a_instance", "flat_bound_a"),
            ("flat_bound_b_instance", "flat_bound_b"),
        ]),
    );

    attach(&mut state, EFFECT_COST, stats(100));
    attach(&mut state, EFFECT_COST, tracks(&[("health", 100)]));
    attach(
        &mut state,
        EFFECT_COST,
        ActiveEffectsComponent::new(version(), vec![]).unwrap(),
    );
    state
}

fn damage_request(
    target: EntityId,
    target_track: &str,
    operation_id: &str,
    parts: Vec<DamagePart>,
) -> DamageRequest {
    let operation = operation(operation_id);
    DamageRequest {
        operation: operation.clone(),
        source: request_identity(&operation, "damage_origin"),
        actor: None,
        target,
        target_track: track(target_track),
        parts,
        request_sources: vec![],
        expected_tracks_revision: None,
    }
}

fn tracked_snapshot(
    state: &EntityState,
    entity: EntityId,
) -> (TracksComponent, ComponentRevision, u64) {
    (
        state
            .component::<TracksComponent>(entity)
            .unwrap()
            .unwrap()
            .clone(),
        state.component_revision::<TracksComponent>(entity).unwrap(),
        state.revision(),
    )
}

fn assert_tracked_snapshot(
    state: &EntityState,
    entity: EntityId,
    expected: &(TracksComponent, ComponentRevision, u64),
) {
    assert_eq!(
        state.component::<TracksComponent>(entity).unwrap().unwrap(),
        &expected.0
    );
    assert_eq!(
        state.component_revision::<TracksComponent>(entity).unwrap(),
        expected.1
    );
    assert_eq!(state.revision(), expected.2);
}

#[test]
fn immediate_shooter_hit_returns_the_complete_fixed_pipeline_ledger() {
    let catalog = catalog();
    let mut state = state();
    let mut request = damage_request(
        SHOOTER,
        "health",
        "shooter_hit",
        vec![DamagePart {
            amount: scalar(50),
            kind: kind("impact"),
        }],
    );
    request.actor = Some(ATTACKER);
    let request_source = request.source.clone();
    let receipt = DamageService::apply(&mut state, &catalog, request).unwrap();

    assert_eq!(receipt.operation, operation("shooter_hit"));
    assert_eq!(receipt.source, request_source);
    assert_eq!(receipt.actor, Some(ATTACKER));
    assert_eq!(receipt.target, SHOOTER);
    assert_eq!(receipt.catalog_version, version());
    assert_eq!(receipt.catalog_fingerprint, catalog.fingerprint());
    assert_eq!(receipt.parts.len(), 1);
    let part = &receipt.parts[0];
    assert_eq!(part.index, 0);
    assert_eq!(part.original, scalar(50));
    assert_eq!(part.after_flat, scalar(47));
    assert_eq!(
        (
            part.combined_scale_numerator,
            part.combined_scale_denominator,
        ),
        (3, 4)
    );
    assert_eq!(part.rounding, RoundingPolicy::TowardZero);
    assert_eq!(part.after_scale, scalar(35));
    assert_eq!(part.absorbed, scalar(20));
    assert_eq!(part.applied, scalar(15));
    assert_eq!(part.unapplied, scalar(0));
    assert_eq!(
        receipt.track_changes,
        vec![
            gameplay_mechanics::TrackDamageChange {
                track: track("armor"),
                before: scalar(20),
                after: scalar(0),
            },
            gameplay_mechanics::TrackDamageChange {
                track: track("health"),
                before: scalar(100),
                after: scalar(85),
            },
        ]
    );
    assert_eq!(
        receipt.facts,
        vec![DamageFact::ProtectionTrackDepleted {
            track: track("armor"),
            part_index: 0,
        }]
    );
    assert!(receipt
        .decisions
        .iter()
        .all(|decision| decision.outcome == DecisionOutcome::Applied));
    assert_eq!(receipt.source_cost.intrinsic_entries_visited, 4);
    assert_eq!(receipt.source_cost.equipment_entries_visited, 0);
    assert_eq!(receipt.source_cost.item_components_read, 0);
    assert_eq!(
        receipt.committed_tracks_revision,
        Some(receipt.observed_tracks_revision + 1)
    );
}

#[test]
fn tabletop_preview_is_pure_and_fresh_apply_observes_an_explicit_effect_reaction() {
    let catalog = catalog();
    let mut state = state();
    let preview_request = damage_request(
        TABLETOP,
        "health",
        "tabletop_preview",
        vec![DamagePart {
            amount: scalar(30),
            kind: kind("energy"),
        }],
    );
    let before = tracked_snapshot(&state, TABLETOP);
    let preview = DamageService::preview(&state, &catalog, &preview_request).unwrap();
    assert_eq!(preview.receipt().parts[0].applied, scalar(30));
    assert_eq!(preview.receipt().committed_tracks_revision, None);
    assert_tracked_snapshot(&state, TABLETOP, &before);

    let reaction_operation = operation("apply_reaction");
    EffectService::apply(
        &mut state,
        &catalog,
        EffectApplyRequest {
            operation: reaction_operation.clone(),
            entity: TABLETOP,
            instance: effect_instance("reaction_ward"),
            definition: effect("ward"),
            provenance: request_identity(&reaction_operation, "reaction_owner"),
            stacks: 1,
            expected_revision: None,
        },
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
                TABLETOP,
                "health",
                "unused_template",
                vec![DamagePart {
                    amount: scalar(30),
                    kind: kind("energy"),
                }],
            )
        },
    )
    .unwrap();
    assert!(receipt.parts[0].prevented);
    assert_eq!(receipt.parts[0].applied, scalar(0));
    assert!(receipt.decisions.iter().any(|decision| {
        matches!(
            &decision.source,
            SourceInstanceIdentity::Effect {
                entity,
                effect,
                stack: 1,
                source: source_id,
            } if *entity == TABLETOP
                && effect == &effect_instance("reaction_ward")
                && source_id == &source("invulnerability")
        ) && decision.outcome == DecisionOutcome::Applied
    }));
    assert_eq!(
        state
            .component::<TracksComponent>(TABLETOP)
            .unwrap()
            .unwrap()
            .current(&track("health")),
        Some(scalar(100))
    );
    let encoded = encode_snapshot(&state).unwrap();
    assert!(!encoded.contains("tabletop_apply"));
    assert!(!encoded.contains("DamageReceipt"));
}

#[test]
fn building_damage_repair_and_bounded_over_repair_share_track_authority() {
    let catalog = catalog();
    let mut state = state();
    let damage = DamageService::apply(
        &mut state,
        &catalog,
        damage_request(
            BUILDING,
            "durability",
            "building_damage",
            vec![DamagePart {
                amount: scalar(30),
                kind: kind("energy"),
            }],
        ),
    )
    .unwrap();
    assert_eq!(damage.parts[0].applied, scalar(30));
    assert_eq!(
        damage.track_changes,
        vec![gameplay_mechanics::TrackDamageChange {
            track: track("durability"),
            before: scalar(200),
            after: scalar(170),
        }]
    );

    let repair_operation = operation("building_repair");
    let repair = TrackService::restore(
        &mut state,
        &catalog,
        TrackMutationRequest {
            operation: repair_operation.clone(),
            source: request_identity(&repair_operation, "repair_owner"),
            entity: BUILDING,
            track: track("durability"),
            amount: scalar(25),
            kind: TrackAdjustmentKind::Spend,
            expected_revision: None,
        },
    )
    .unwrap();
    assert_eq!(repair.kind, TrackAdjustmentKind::Restore);
    assert_eq!(repair.requested_amount, scalar(25));
    assert_eq!(repair.applied_amount, scalar(25));
    assert_eq!((repair.before, repair.after), (scalar(170), scalar(195)));

    let finish_operation = operation("finish_repair");
    let finish = TrackService::restore(
        &mut state,
        &catalog,
        TrackMutationRequest {
            operation: finish_operation.clone(),
            source: request_identity(&finish_operation, "repair_owner"),
            entity: BUILDING,
            track: track("durability"),
            amount: scalar(200),
            kind: TrackAdjustmentKind::Restore,
            expected_revision: None,
        },
    )
    .unwrap();
    assert_eq!(finish.requested_amount, scalar(200));
    assert_eq!(finish.applied_amount, scalar(105));
    assert_eq!((finish.before, finish.after), (scalar(195), scalar(300)));
}

#[test]
fn stacking_rounding_and_flat_over_reduction_are_exact_and_preview_only() {
    let catalog = catalog();
    let state = state();

    let mut stacking = damage_request(
        SIMPLE,
        "health",
        "stacking_preview",
        vec![DamagePart {
            amount: scalar(10),
            kind: kind("impact"),
        }],
    );
    stacking.request_sources = vec![
        request_source("low_context", "low_flat"),
        request_source("high_context", "high_flat"),
    ];
    let stacking = DamageService::preview(&state, &catalog, &stacking).unwrap();
    assert_eq!(stacking.receipt().parts[0].after_flat, scalar(5));
    assert!(stacking.receipt().decisions.iter().any(|decision| {
        decision.source_definition == source("high_flat")
            && decision.outcome == DecisionOutcome::Applied
    }));
    assert!(stacking.receipt().decisions.iter().any(|decision| {
        decision.source_definition == source("low_flat")
            && decision.outcome == DecisionOutcome::Suppressed
    }));

    let mut rounding = damage_request(
        SIMPLE,
        "health",
        "rounding_preview",
        vec![DamagePart {
            amount: scalar(1),
            kind: kind("energy"),
        }],
    );
    rounding.request_sources = vec![request_source("half_context", "half_scale")];
    let rounding = DamageService::preview(&state, &catalog, &rounding).unwrap();
    assert_eq!(
        rounding.receipt().parts[0].rounding,
        RoundingPolicy::TowardZero
    );
    assert_eq!(rounding.receipt().parts[0].after_scale, scalar(0));
    assert_eq!(rounding.receipt().parts[0].applied, scalar(0));

    let flat = damage_request(
        FLAT_OVERFLOW,
        "health",
        "flat_bound_preview",
        vec![DamagePart {
            amount: scalar(10),
            kind: kind("energy"),
        }],
    );
    let flat = DamageService::preview(&state, &catalog, &flat).unwrap();
    assert_eq!(flat.receipt().parts[0].after_flat, scalar(0));
    assert_eq!(
        flat.receipt()
            .decisions
            .iter()
            .filter(|decision| decision.outcome == DecisionOutcome::Applied)
            .count(),
        2
    );
    assert_eq!(
        state
            .component::<TracksComponent>(FLAT_OVERFLOW)
            .unwrap()
            .unwrap()
            .current(&track("health")),
        Some(scalar(100))
    );
}

#[test]
fn canonical_absorption_stops_after_exhaustion_and_skips_depleted_protection() {
    let catalog = catalog();
    let mut state = state();
    let request = damage_request(
        ABSORB_ORDER,
        "health",
        "absorption_preview",
        vec![DamagePart {
            amount: scalar(10),
            kind: kind("impact"),
        }],
    );
    let preview = DamageService::preview(&state, &catalog, &request).unwrap();
    assert_eq!(preview.receipt().parts[0].after_scale, scalar(3));
    assert_eq!(preview.receipt().parts[0].absorbed, scalar(3));
    assert!(preview.receipt().decisions.iter().any(|decision| {
        matches!(
            &decision.kind,
            ResponseDecisionKind::Absorb { track: response_track }
                if response_track == &track("reserve")
        ) && decision.outcome == DecisionOutcome::Inapplicable
    }));
    assert!(!preview
        .receipt()
        .track_changes
        .iter()
        .any(|change| change.track == track("reserve")));

    let spend_operation = operation("deplete_first_protection");
    TrackService::spend(
        &mut state,
        &catalog,
        TrackMutationRequest {
            operation: spend_operation.clone(),
            source: request_identity(&spend_operation, "test_owner"),
            entity: ABSORB_ORDER,
            track: track("armor"),
            amount: scalar(5),
            kind: TrackAdjustmentKind::Spend,
            expected_revision: None,
        },
    )
    .unwrap();
    let preview = DamageService::preview(&state, &catalog, &request).unwrap();
    assert!(preview.receipt().decisions.iter().any(|decision| {
        matches!(
            &decision.kind,
            ResponseDecisionKind::Absorb { track: response_track }
                if response_track == &track("armor")
        ) && decision.outcome == DecisionOutcome::Inapplicable
    }));
    assert!(preview.receipt().decisions.iter().any(|decision| {
        matches!(
            &decision.kind,
            ResponseDecisionKind::Absorb { track: response_track }
                if response_track == &track("reserve")
        ) && decision.outcome == DecisionOutcome::Applied
    }));
    assert!(!preview
        .receipt()
        .track_changes
        .iter()
        .any(|change| change.track == track("armor")));
    assert!(preview.receipt().track_changes.iter().any(|change| {
        change.track == track("reserve")
            && change.before == scalar(50)
            && change.after == scalar(47)
    }));
}

#[test]
fn multipart_damage_reports_protection_and_target_depletion_by_part() {
    let catalog = catalog();
    let mut state = state();
    let receipt = DamageService::apply(
        &mut state,
        &catalog,
        damage_request(
            SHOOTER,
            "health",
            "multipart_depletion",
            vec![
                DamagePart {
                    amount: scalar(50),
                    kind: kind("impact"),
                },
                DamagePart {
                    amount: scalar(200),
                    kind: kind("energy"),
                },
            ],
        ),
    )
    .unwrap();
    assert_eq!(
        receipt
            .parts
            .iter()
            .map(|part| part.index)
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
    assert_eq!(receipt.parts[0].absorbed, scalar(20));
    assert_eq!(receipt.parts[0].applied, scalar(15));
    assert_eq!(receipt.parts[1].applied, scalar(85));
    assert_eq!(receipt.parts[1].unapplied, scalar(115));
    assert_eq!(
        receipt.facts,
        vec![
            DamageFact::ProtectionTrackDepleted {
                track: track("armor"),
                part_index: 0,
            },
            DamageFact::TargetTrackDepleted {
                track: track("health"),
                part_index: 1,
            },
        ]
    );
    assert!(receipt
        .decisions
        .iter()
        .any(|decision| decision.part_index == 1
            && decision.outcome == DecisionOutcome::Inapplicable));
}

#[test]
fn stale_duplicate_negative_missing_overflow_and_late_failures_do_not_mutate() {
    let catalog = catalog();
    let mut state = state();

    let stale_revision = state.component_revision::<TracksComponent>(SIMPLE).unwrap();
    let spend_operation = operation("advance_simple_track");
    TrackService::spend(
        &mut state,
        &catalog,
        TrackMutationRequest {
            operation: spend_operation.clone(),
            source: request_identity(&spend_operation, "advance_owner"),
            entity: SIMPLE,
            track: track("health"),
            amount: scalar(1),
            kind: TrackAdjustmentKind::Spend,
            expected_revision: None,
        },
    )
    .unwrap();
    let stale_before = tracked_snapshot(&state, SIMPLE);
    let mut stale = damage_request(
        SIMPLE,
        "health",
        "stale_damage",
        vec![DamagePart {
            amount: scalar(10),
            kind: kind("energy"),
        }],
    );
    stale.expected_tracks_revision = Some(stale_revision);
    assert!(matches!(
        DamageService::apply(&mut state, &catalog, stale),
        Err(MechanicsError::StaleComponentRevision { .. })
    ));
    assert_tracked_snapshot(&state, SIMPLE, &stale_before);

    let duplicate_before = tracked_snapshot(&state, SIMPLE);
    let mut duplicate = damage_request(
        SIMPLE,
        "health",
        "duplicate_damage_source",
        vec![DamagePart {
            amount: scalar(10),
            kind: kind("impact"),
        }],
    );
    duplicate.request_sources = vec![
        request_source("same_context", "low_flat"),
        request_source("same_context", "high_flat"),
    ];
    assert!(matches!(
        DamageService::apply(&mut state, &catalog, duplicate),
        Err(MechanicsError::DuplicateSource { .. })
    ));
    assert_tracked_snapshot(&state, SIMPLE, &duplicate_before);

    let negative_before = tracked_snapshot(&state, SIMPLE);
    assert!(matches!(
        DamageService::apply(
            &mut state,
            &catalog,
            damage_request(
                SIMPLE,
                "health",
                "negative_damage",
                vec![DamagePart {
                    amount: scalar(-1),
                    kind: kind("energy"),
                }],
            ),
        ),
        Err(MechanicsError::Arithmetic(
            MechanicsArithmeticError::NegativeAmount { value: -1 }
        ))
    ));
    assert_tracked_snapshot(&state, SIMPLE, &negative_before);

    let overflow_before = tracked_snapshot(&state, SCALE_OVERFLOW);
    assert!(matches!(
        DamageService::apply(
            &mut state,
            &catalog,
            damage_request(
                SCALE_OVERFLOW,
                "health",
                "scale_overflow",
                vec![DamagePart {
                    amount: scalar(1),
                    kind: kind("energy"),
                }],
            ),
        ),
        Err(MechanicsError::Arithmetic(
            MechanicsArithmeticError::Overflow
        ))
    ));
    assert_tracked_snapshot(&state, SCALE_OVERFLOW, &overflow_before);

    let late_before = tracked_snapshot(&state, LATE_FAILURE);
    assert!(matches!(
        DamageService::apply(
            &mut state,
            &catalog,
            damage_request(
                LATE_FAILURE,
                "health",
                "late_failure",
                vec![
                    DamagePart {
                        amount: scalar(10),
                        kind: kind("energy"),
                    },
                    DamagePart {
                        amount: scalar(10),
                        kind: kind("impact"),
                    },
                ],
            ),
        ),
        Err(MechanicsError::MissingTrack { track: missing, .. })
            if missing == track("missing_protection")
    ));
    assert_tracked_snapshot(&state, LATE_FAILURE, &late_before);

    let missing_before = state.revision();
    assert!(matches!(
        DamageService::apply(
            &mut state,
            &catalog,
            damage_request(
                MISSING_TRACKS,
                "health",
                "missing_component",
                vec![DamagePart {
                    amount: scalar(10),
                    kind: kind("energy"),
                }],
            ),
        ),
        Err(MechanicsError::MissingComponent { .. })
    ));
    assert_eq!(state.revision(), missing_before);
}

#[test]
fn source_costs_cover_effect_bound_evaluation_while_the_simple_path_stays_empty() {
    let catalog = catalog();
    let mut state = state();

    let simple = DamageService::apply(
        &mut state,
        &catalog,
        damage_request(
            SIMPLE,
            "health",
            "simple_damage",
            vec![DamagePart {
                amount: scalar(10),
                kind: kind("energy"),
            }],
        ),
    )
    .unwrap();
    assert!(simple.decisions.is_empty());
    assert_eq!(simple.source_cost.intrinsic_entries_visited, 0);
    assert_eq!(simple.source_cost.effect_entries_visited, 0);
    assert_eq!(simple.source_cost.effect_source_activations_visited, 0);
    assert_eq!(simple.source_cost.equipment_entries_visited, 0);
    assert_eq!(simple.source_cost.item_components_read, 0);
    assert_eq!(simple.source_cost.request_entries_visited, 0);

    let effect_operation = operation("apply_fortify");
    EffectService::apply(
        &mut state,
        &catalog,
        EffectApplyRequest {
            operation: effect_operation.clone(),
            entity: EFFECT_COST,
            instance: effect_instance("fortify_instance"),
            definition: effect("fortify"),
            provenance: request_identity(&effect_operation, "fortify_owner"),
            stacks: 1,
            expected_revision: None,
        },
    )
    .unwrap();
    let costed = DamageService::apply(
        &mut state,
        &catalog,
        damage_request(
            EFFECT_COST,
            "health",
            "costed_damage",
            vec![DamagePart {
                amount: scalar(10),
                kind: kind("energy"),
            }],
        ),
    )
    .unwrap();
    assert_eq!(costed.source_cost.effect_entries_visited, 2);
    assert_eq!(costed.source_cost.effect_source_activations_visited, 2);
    assert_eq!(costed.decisions.len(), 1);
    assert_eq!(costed.decisions[0].outcome, DecisionOutcome::Inapplicable);

    let encoded = encode_snapshot(&state).unwrap();
    assert!(!encoded.contains("costed_damage"));
    let mut restored = decode_snapshot_with_catalog(&encoded, &catalog).unwrap();
    let restore_operation = operation("restore_after_reopen");
    let restored_receipt = TrackService::restore(
        &mut restored,
        &catalog,
        TrackMutationRequest {
            operation: restore_operation.clone(),
            source: request_identity(&restore_operation, "restore_owner"),
            entity: EFFECT_COST,
            track: track("health"),
            amount: scalar(10),
            kind: TrackAdjustmentKind::Restore,
            expected_revision: None,
        },
    )
    .unwrap();
    assert_eq!(restored_receipt.after, scalar(100));
}

#[test]
fn set_policy_remains_a_distinct_non_damage_mutation_lane() {
    let catalog = catalog();
    let mut state = state();
    let set_operation = operation("explicit_track_set");
    let receipt = TrackService::set_under_policy(
        &mut state,
        &catalog,
        TrackSetRequest {
            operation: set_operation.clone(),
            source: request_identity(&set_operation, "set_owner"),
            entity: SIMPLE,
            track: track("health"),
            value: scalar(150),
            policy: TrackSetPolicy::ClampToBounds,
            expected_revision: None,
        },
    )
    .unwrap();
    assert_eq!(receipt.requested, scalar(150));
    assert_eq!(receipt.after, scalar(100));
}
