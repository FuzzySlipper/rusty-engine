use core_ids::EntityId;
use entity_state::{
    encode_snapshot, EntityAuthoringService, EntityComponent, EntityDefinition, EntityState,
};
use gameplay_continuous_mechanics::*;

const ENTITY: EntityId = EntityId::new(7);

fn value(value: f64) -> ContinuousValue {
    ContinuousValue::new(value).unwrap()
}
fn id<T>(
    parse: impl FnOnce(String) -> Result<T, ContinuousMechanicsIdentityError>,
    value: &str,
) -> T {
    parse(value.to_string()).unwrap()
}

fn catalog() -> ContinuousMechanicsCatalog {
    let stat = id(ContinuousStatId::parse, "focus");
    let track = id(ContinuousTrackId::parse, "energy");
    let source = id(ContinuousSourceDefinitionId::parse, "focus_boost");
    ContinuousMechanicsCatalog::admit(ContinuousMechanicsCatalogDefinition {
        version: id(ContinuousCatalogVersion::parse, "v1"),
        stats: vec![ContinuousStatDefinition::new(stat.clone(), value(0.0), value(10.0)).unwrap()],
        tracks: vec![ContinuousTrackDefinition::new(
            track,
            value(0.0),
            ContinuousTrackMaximum::Stat { stat: stat.clone() },
        )
        .unwrap()],
        sources: vec![
            ContinuousSourceDefinition {
                id: source.clone(),
                priority: 0,
                stat_contributions: vec![ContinuousStatContributionDefinition {
                    stat: stat.clone(),
                    contribution: ContinuousStatContribution::add(value(0.25)),
                    stacking_group: id(ContinuousStackingGroupId::parse, "focus_add"),
                    stacking: ContinuousStackingPolicy::Sum,
                }],
            },
            ContinuousSourceDefinition {
                id: id(ContinuousSourceDefinitionId::parse, "focus_drain"),
                priority: 0,
                stat_contributions: vec![ContinuousStatContributionDefinition {
                    stat,
                    contribution: ContinuousStatContribution::add(value(-2.0)),
                    stacking_group: id(ContinuousStackingGroupId::parse, "focus_drain"),
                    stacking: ContinuousStackingPolicy::Sum,
                }],
            },
        ],
        effects: vec![
            ContinuousEffectDefinition {
                id: id(ContinuousEffectDefinitionId::parse, "boost_effect"),
                sources: vec![source],
            },
            ContinuousEffectDefinition {
                id: id(ContinuousEffectDefinitionId::parse, "drain_effect"),
                sources: vec![id(ContinuousSourceDefinitionId::parse, "focus_drain")],
            },
        ],
    })
    .unwrap()
}

fn attach<T: EntityComponent>(state: &mut EntityState, value: T) {
    let revision = state.component_revision::<T>(ENTITY).unwrap();
    EntityAuthoringService
        .attach_component(state, revision, ENTITY, value)
        .unwrap();
}

fn state(catalog: &ContinuousMechanicsCatalog) -> EntityState {
    let mut state = EntityState::from_definitions_with_registry(
        continuous_mechanics_component_registry().unwrap(),
        [EntityDefinition::new(ENTITY, "actor")],
    )
    .unwrap();
    attach(
        &mut state,
        ContinuousStatsComponent::new(
            catalog.version().clone(),
            vec![ContinuousStatValue::new(
                id(ContinuousStatId::parse, "focus"),
                value(2.5),
            )],
        )
        .unwrap(),
    );
    attach(
        &mut state,
        ContinuousTracksComponent::new(
            catalog.version().clone(),
            vec![ContinuousTrackValue::new(
                id(ContinuousTrackId::parse, "energy"),
                value(1.25),
            )],
        )
        .unwrap(),
    );
    attach(
        &mut state,
        ContinuousIntrinsicSourcesComponent::new(
            catalog.version().clone(),
            vec![ContinuousIntrinsicSourceBinding::new(
                id(ContinuousSourceInstanceId::parse, "boost_a"),
                id(ContinuousSourceDefinitionId::parse, "focus_boost"),
            )],
        )
        .unwrap(),
    );
    attach(
        &mut state,
        ContinuousActiveEffectsComponent::new(catalog.version().clone(), vec![]).unwrap(),
    );
    state
}

#[test]
fn fractional_resource_derived_stat_snapshot_and_receipt_continue_by_bits() {
    let catalog = catalog();
    let mut original = state(&catalog);
    let evaluated = ContinuousStatService::evaluate(
        &original,
        &catalog,
        ENTITY,
        &id(ContinuousStatId::parse, "focus"),
    )
    .unwrap();
    assert_eq!(evaluated.value.bits(), value(2.75).bits());
    let first = ContinuousTrackService::restore(
        &mut original,
        &catalog,
        ContinuousTrackAdjustmentRequest {
            operation: id(ContinuousOperationId::parse, "restore_one"),
            entity: ENTITY,
            track: id(ContinuousTrackId::parse, "energy"),
            amount: value(0.5),
            kind: ContinuousTrackAdjustmentKind::Restore,
            expected_revision: None,
        },
    )
    .unwrap();
    assert_eq!(first.after.bits(), value(1.75).bits());
    let snapshot = encode_snapshot(&original).unwrap();
    let mut reopened = decode_snapshot_with_continuous_catalog(&snapshot, &catalog).unwrap();
    let request = ContinuousTrackAdjustmentRequest {
        operation: id(ContinuousOperationId::parse, "restore_two"),
        entity: ENTITY,
        track: id(ContinuousTrackId::parse, "energy"),
        amount: value(0.25),
        kind: ContinuousTrackAdjustmentKind::Restore,
        expected_revision: None,
    };
    let original_receipt =
        ContinuousTrackService::restore(&mut original, &catalog, request.clone()).unwrap();
    let reopened_receipt =
        ContinuousTrackService::restore(&mut reopened, &catalog, request).unwrap();
    assert!(original_receipt.same_durable_result(&reopened_receipt));
}

#[test]
fn codecs_use_hex_bits_and_reject_invalid_persisted_values() {
    let catalog = catalog();
    let component = ContinuousStatsComponent::new(
        catalog.version().clone(),
        vec![ContinuousStatValue::new(
            id(ContinuousStatId::parse, "focus"),
            value(1.5),
        )],
    )
    .unwrap();
    let encoded = serde_json::to_string(&component).unwrap();
    assert!(encoded.contains("3ff8000000000000"));
    let invalid = encoded.replace("3ff8000000000000", "8000000000000000");
    assert!(serde_json::from_str::<ContinuousStatsComponent>(&invalid).is_err());
}

#[test]
fn persisted_continuous_bits_are_strict_and_catalog_admission_is_infallible_after_decode() {
    fn catalog_json(bits: &str) -> String {
        format!(
            r#"{{"version":"v1","stats":[{{"id":"focus","minimum":"{bits}","maximum":"3ff0000000000000"}}],"tracks":[],"sources":[],"effects":[]}}"#
        )
    }
    for bits in [
        "7ff8000000000001", // NaN payload
        "7ff0000000000000", // +infinity
        "fff0000000000000", // -infinity
        "8000000000000000", // persisted negative zero
        "3FF0000000000000", // uppercase
        "3ff000000000000",  // wrong width
        "not-hex-value!!!!",
    ] {
        assert!(
            serde_json::from_str::<ContinuousMechanicsCatalogDefinition>(&catalog_json(bits))
                .is_err(),
            "{bits}"
        );
    }
    let subnormal: ContinuousMechanicsCatalogDefinition =
        serde_json::from_str(&catalog_json("0000000000000001")).unwrap();
    ContinuousMechanicsCatalog::admit(subnormal).unwrap();
    let maximum: ContinuousMechanicsCatalogDefinition = serde_json::from_str(
        &catalog_json("0000000000000000").replace("3ff0000000000000", "7fefffffffffffff"),
    )
    .unwrap();
    ContinuousMechanicsCatalog::admit(maximum).unwrap();

    let component =
        r#"{"catalogVersion":"v1","values":[{"stat":"focus","base":"0000000000000001"}]}"#;
    assert!(serde_json::from_str::<ContinuousStatsComponent>(component).is_ok());
    let unknown = r#"{"catalogVersion":"v1","values":[],"unexpected":true}"#;
    assert!(serde_json::from_str::<ContinuousStatsComponent>(unknown).is_err());
}

#[test]
fn combined_registry_keeps_exact_and_continuous_component_families_separate() {
    let registry = combined_gameplay_component_registry().unwrap();
    let state = EntityState::from_definitions_with_registry(
        registry,
        [EntityDefinition::new(ENTITY, "actor")],
    )
    .unwrap();
    assert_eq!(
        state
            .component_type_id::<gameplay_mechanics::StatsComponent>()
            .unwrap()
            .as_str(),
        gameplay_mechanics::STATS_COMPONENT_TYPE_ID
    );
    assert_eq!(
        state
            .component_type_id::<ContinuousStatsComponent>()
            .unwrap()
            .as_str(),
        CONTINUOUS_STATS_COMPONENT_TYPE_ID
    );
}

#[test]
fn continuous_fingerprint_is_canonical_and_bit_sensitive() {
    let first = catalog();
    let mut definition = first.definition().clone();
    definition.sources.reverse();
    definition.effects.reverse();
    let reordered = ContinuousMechanicsCatalog::admit(definition).unwrap();
    assert_eq!(first.fingerprint(), reordered.fingerprint());
    assert!(first.fingerprint().starts_with("sha256:"));
    assert_eq!(first.fingerprint().len(), "sha256:".len() + 64);

    let mut changed = first.definition().clone();
    changed.sources[0].stat_contributions[0].contribution =
        ContinuousStatContribution::add(value(f64::from_bits(value(0.25).bits() + 1)));
    let changed = ContinuousMechanicsCatalog::admit(changed).unwrap();
    assert_ne!(first.fingerprint(), changed.fingerprint());
}

#[test]
fn exact_component_family_registration_sentinel_remains_frozen() {
    use gameplay_mechanics::MechanicsComponentKind;
    assert_eq!(MechanicsComponentKind::ALL.len(), 7);
    let observed: Vec<_> = MechanicsComponentKind::ALL
        .into_iter()
        .map(|kind| (kind.type_id(), kind.codec_id(), kind.codec_version()))
        .collect();
    assert_eq!(
        observed,
        vec![
            ("rusty.mechanics.stats", "rusty.mechanics.stats-json", 1),
            ("rusty.mechanics.tracks", "rusty.mechanics.tracks-json", 1),
            (
                "rusty.mechanics.intrinsic-sources",
                "rusty.mechanics.intrinsic-sources-json",
                1
            ),
            (
                "rusty.mechanics.active-effects",
                "rusty.mechanics.active-effects-json",
                2
            ),
            (
                "rusty.mechanics.inventory",
                "rusty.mechanics.inventory-json",
                2
            ),
            ("rusty.mechanics.item", "rusty.mechanics.item-json", 1),
            (
                "rusty.mechanics.equipment",
                "rusty.mechanics.equipment-json",
                1
            ),
        ]
    );
}

#[test]
fn restore_caps_before_a_huge_finite_addition_can_overflow() {
    let catalog = catalog();
    let mut state = state(&catalog);
    let receipt = ContinuousTrackService::restore(
        &mut state,
        &catalog,
        ContinuousTrackAdjustmentRequest {
            operation: id(ContinuousOperationId::parse, "huge_restore"),
            entity: ENTITY,
            track: id(ContinuousTrackId::parse, "energy"),
            amount: value(f64::MAX),
            kind: ContinuousTrackAdjustmentKind::Restore,
            expected_revision: None,
        },
    )
    .unwrap();
    assert_eq!(receipt.after, receipt.maximum);
    assert_eq!(
        receipt.applied_amount,
        receipt.maximum.checked_sub(receipt.before).unwrap()
    );
}

#[test]
fn wrong_component_revision_scope_is_not_reported_as_a_stale_number() {
    let catalog = catalog();
    let mut state = state(&catalog);
    let wrong_scope = state
        .component_revision::<ContinuousTracksComponent>(ENTITY)
        .unwrap();
    let before = state
        .component_revision::<ContinuousStatsComponent>(ENTITY)
        .unwrap();
    let error = ContinuousStatService::set_base(
        &mut state,
        &catalog,
        ContinuousStatBaseMutationRequest {
            operation: id(ContinuousOperationId::parse, "wrong_scope"),
            entity: ENTITY,
            stat: id(ContinuousStatId::parse, "focus"),
            base: value(2.0),
            expected_revision: Some(wrong_scope),
        },
    )
    .unwrap_err();
    assert!(matches!(
        error,
        ContinuousMechanicsError::RevisionScopeMismatch { .. }
    ));
    assert_eq!(
        state
            .component_revision::<ContinuousStatsComponent>(ENTITY)
            .unwrap(),
        before
    );
}

#[test]
fn stale_same_scope_revision_is_distinct_and_nonmutating() {
    let catalog = catalog();
    let mut state = state(&catalog);
    let stale = state
        .component_revision::<ContinuousStatsComponent>(ENTITY)
        .unwrap();
    ContinuousStatService::set_base(
        &mut state,
        &catalog,
        ContinuousStatBaseMutationRequest {
            operation: id(ContinuousOperationId::parse, "fresh"),
            entity: ENTITY,
            stat: id(ContinuousStatId::parse, "focus"),
            base: value(3.0),
            expected_revision: Some(stale.clone()),
        },
    )
    .unwrap();
    let before = state
        .component_revision::<ContinuousStatsComponent>(ENTITY)
        .unwrap();
    let error = ContinuousStatService::set_base(
        &mut state,
        &catalog,
        ContinuousStatBaseMutationRequest {
            operation: id(ContinuousOperationId::parse, "stale"),
            entity: ENTITY,
            stat: id(ContinuousStatId::parse, "focus"),
            base: value(4.0),
            expected_revision: Some(stale),
        },
    )
    .unwrap_err();
    assert!(matches!(
        error,
        ContinuousMechanicsError::StaleRevision { .. }
    ));
    assert_eq!(
        state
            .component_revision::<ContinuousStatsComponent>(ENTITY)
            .unwrap(),
        before
    );
}

#[test]
fn mechanics_scalar_bridge_widens_then_quantizes_with_typed_provenance() {
    use gameplay_mechanics::MechanicsScalar;
    use gameplay_standard::{
        quantize_continuous_to_mechanics, widen_mechanics_scalar_to_continuous, CapabilityRoleId,
        ContinuousInputReference, ContinuousQuantizationMode, ContinuousQuantizationSource,
        InputId,
    };
    let scalar = MechanicsScalar::new(7).unwrap();
    let widened = widen_mechanics_scalar_to_continuous(scalar);
    let component = ContinuousStatsComponent::new(
        id(ContinuousCatalogVersion::parse, "v1"),
        vec![ContinuousStatValue::new(
            id(ContinuousStatId::parse, "focus"),
            widened,
        )],
    )
    .unwrap();
    assert_eq!(
        component
            .base(&id(ContinuousStatId::parse, "focus"))
            .unwrap()
            .bits(),
        widened.bits()
    );
    let source = ContinuousQuantizationSource::DirectInput {
        input: ContinuousInputReference::Fact {
            role: CapabilityRoleId::parse("bridge_role").unwrap(),
            id: InputId::parse("bridge_id").unwrap(),
        },
    };
    let receipt = quantize_continuous_to_mechanics(
        value(7.75),
        ContinuousQuantizationMode::TowardZero,
        source.clone(),
    )
    .unwrap();
    assert_eq!(receipt.source(), &source);
    assert_eq!(receipt.source_bits(), value(7.75).bits());
    assert_eq!(receipt.mode(), ContinuousQuantizationMode::TowardZero);
    assert_eq!(receipt.result(), Some(scalar));
    assert_eq!(receipt.remainder().unwrap().bits(), value(0.75).bits());
}

#[test]
fn stacking_is_deterministic_by_group_definition_and_preserves_inapplicable_provenance() {
    let stat = id(ContinuousStatId::parse, "focus");
    let other = id(ContinuousStatId::parse, "other");
    let make = |name: &str, amount: f64, group: &str, stacking| ContinuousSourceDefinition {
        id: id(ContinuousSourceDefinitionId::parse, name),
        priority: 0,
        stat_contributions: vec![ContinuousStatContributionDefinition {
            stat: stat.clone(),
            contribution: ContinuousStatContribution::add(value(amount)),
            stacking_group: id(ContinuousStackingGroupId::parse, group),
            stacking,
        }],
    };
    let mut sources = vec![
        make("sum_a", 1.0, "sum", ContinuousStackingPolicy::Sum),
        make("sum_b", 2.0, "sum", ContinuousStackingPolicy::Sum),
        make("high_a", 3.0, "high", ContinuousStackingPolicy::Highest),
        make("high_b", 3.0, "high", ContinuousStackingPolicy::Highest),
        make("low_a", -1.0, "low", ContinuousStackingPolicy::Lowest),
        make("low_b", -1.0, "low", ContinuousStackingPolicy::Lowest),
        make(
            "unique_a",
            1.0,
            "unique",
            ContinuousStackingPolicy::UniqueBySource,
        ),
        make(
            "unique_b",
            2.0,
            "unique",
            ContinuousStackingPolicy::UniqueBySource,
        ),
    ];
    sources[0]
        .stat_contributions
        .push(ContinuousStatContributionDefinition {
            stat: other,
            contribution: ContinuousStatContribution::add(value(99.0)),
            stacking_group: id(ContinuousStackingGroupId::parse, "other"),
            stacking: ContinuousStackingPolicy::Sum,
        });
    let catalog = ContinuousMechanicsCatalog::admit(ContinuousMechanicsCatalogDefinition {
        version: id(ContinuousCatalogVersion::parse, "v1"),
        stats: vec![
            ContinuousStatDefinition::new(stat.clone(), value(-100.0), value(100.0)).unwrap(),
            ContinuousStatDefinition::new(
                id(ContinuousStatId::parse, "other"),
                value(-100.0),
                value(100.0),
            )
            .unwrap(),
        ],
        tracks: vec![],
        sources: sources.clone(),
        effects: vec![],
    })
    .unwrap();
    let mut state = EntityState::from_definitions_with_registry(
        continuous_mechanics_component_registry().unwrap(),
        [EntityDefinition::new(ENTITY, "actor")],
    )
    .unwrap();
    attach(
        &mut state,
        ContinuousStatsComponent::new(
            catalog.version().clone(),
            vec![ContinuousStatValue::new(stat.clone(), value(0.0))],
        )
        .unwrap(),
    );
    let bindings = [
        "sum_a", "sum_b", "high_a", "high_b", "low_a", "low_b", "unique_a", "unique_a", "unique_b",
    ]
    .into_iter()
    .enumerate()
    .map(|(n, definition)| {
        ContinuousIntrinsicSourceBinding::new(
            id(ContinuousSourceInstanceId::parse, &format!("i{n}")),
            id(ContinuousSourceDefinitionId::parse, definition),
        )
    })
    .collect();
    attach(
        &mut state,
        ContinuousIntrinsicSourcesComponent::new(catalog.version().clone(), bindings).unwrap(),
    );
    let eval = ContinuousStatService::evaluate(&state, &catalog, ENTITY, &stat).unwrap();
    assert_eq!(eval.value, value(8.0)); // 1+2 + highest 3 + lowest -1 + unique 1+2
    assert!(eval
        .decisions
        .iter()
        .any(|d| d.outcome == ContinuousDecisionOutcome::Inapplicable));
    assert_eq!(
        eval.decisions
            .iter()
            .filter(|d| d.outcome == ContinuousDecisionOutcome::Applied
                && d.source_definition.as_str() == "high_a")
            .count(),
        1
    );
    let mut permuted = sources;
    permuted.reverse();
    let permuted = ContinuousMechanicsCatalog::admit(ContinuousMechanicsCatalogDefinition {
        version: id(ContinuousCatalogVersion::parse, "v1"),
        stats: catalog.definition().stats.clone(),
        tracks: vec![],
        sources: permuted,
        effects: vec![],
    })
    .unwrap();
    let again = ContinuousStatService::evaluate(&state, &permuted, ENTITY, &stat).unwrap();
    assert_eq!(catalog.fingerprint(), permuted.fingerprint());
    assert_eq!(eval.value, again.value);
    assert_eq!(eval.decisions, again.decisions);
}

#[test]
fn lowering_a_derived_track_maximum_rejects_before_stat_publication() {
    let catalog = catalog();
    let mut state = state(&catalog);
    let revision = state
        .component_revision::<ContinuousTracksComponent>(ENTITY)
        .unwrap();
    EntityAuthoringService
        .replace_component(
            &mut state,
            revision,
            ENTITY,
            ContinuousTracksComponent::new(
                catalog.version().clone(),
                vec![ContinuousTrackValue::new(
                    id(ContinuousTrackId::parse, "energy"),
                    value(2.0),
                )],
            )
            .unwrap(),
        )
        .unwrap();
    let before = state
        .component_revision::<ContinuousStatsComponent>(ENTITY)
        .unwrap();
    let error = ContinuousStatService::set_base(
        &mut state,
        &catalog,
        ContinuousStatBaseMutationRequest {
            operation: id(ContinuousOperationId::parse, "lower_focus"),
            entity: ENTITY,
            stat: id(ContinuousStatId::parse, "focus"),
            base: value(1.0),
            expected_revision: Some(before.clone()),
        },
    )
    .unwrap_err();
    assert!(matches!(
        error,
        ContinuousMechanicsError::WouldInvalidateTrack { .. }
    ));
    assert_eq!(
        state
            .component_revision::<ContinuousStatsComponent>(ENTITY)
            .unwrap(),
        before
    );
}

#[test]
fn applying_a_lowering_effect_rejects_without_publishing_the_effect() {
    let catalog = catalog();
    let mut state = state(&catalog);
    let before = state
        .component_revision::<ContinuousActiveEffectsComponent>(ENTITY)
        .unwrap();
    let error = ContinuousEffectService::apply(
        &mut state,
        &catalog,
        ContinuousEffectApplyRequest {
            operation: id(ContinuousOperationId::parse, "apply_drain"),
            entity: ENTITY,
            effect: ContinuousActiveEffectInstance::new(
                id(ContinuousEffectInstanceId::parse, "drain_one"),
                id(ContinuousEffectDefinitionId::parse, "drain_effect"),
            ),
            expected_revision: Some(before.clone()),
        },
    )
    .unwrap_err();
    assert!(matches!(
        error,
        ContinuousMechanicsError::WouldInvalidateTrack { .. }
    ));
    assert_eq!(
        state
            .component_revision::<ContinuousActiveEffectsComponent>(ENTITY)
            .unwrap(),
        before
    );
}

#[test]
fn applying_a_minimum_raising_effect_rejects_without_publishing_the_effect() {
    let mut definition = catalog().definition().clone();
    let source = definition
        .sources
        .iter_mut()
        .find(|source| source.id.as_str() == "focus_boost")
        .unwrap();
    source.stat_contributions[0].contribution = ContinuousStatContribution::minimum(value(2.0));
    let catalog = ContinuousMechanicsCatalog::admit(definition).unwrap();
    let mut state = state(&catalog);
    let intrinsic_revision = state
        .component_revision::<ContinuousIntrinsicSourcesComponent>(ENTITY)
        .unwrap();
    EntityAuthoringService
        .replace_component(
            &mut state,
            intrinsic_revision,
            ENTITY,
            ContinuousIntrinsicSourcesComponent::new(catalog.version().clone(), vec![]).unwrap(),
        )
        .unwrap();
    let before = state
        .component_revision::<ContinuousActiveEffectsComponent>(ENTITY)
        .unwrap();
    let error = ContinuousEffectService::apply(
        &mut state,
        &catalog,
        ContinuousEffectApplyRequest {
            operation: id(ContinuousOperationId::parse, "raise_minimum"),
            entity: ENTITY,
            effect: ContinuousActiveEffectInstance::new(
                id(ContinuousEffectInstanceId::parse, "raise_minimum_one"),
                id(ContinuousEffectDefinitionId::parse, "boost_effect"),
            ),
            expected_revision: Some(before.clone()),
        },
    )
    .unwrap_err();
    assert!(matches!(
        error,
        ContinuousMechanicsError::WouldInvalidateTrackMinimum { .. }
    ));
    assert_eq!(
        state
            .component_revision::<ContinuousActiveEffectsComponent>(ENTITY)
            .unwrap(),
        before
    );
}

#[test]
fn removing_a_supporting_effect_rejects_without_publishing_the_effect() {
    let catalog = catalog();
    let mut state = state(&catalog);
    let effect_revision = state
        .component_revision::<ContinuousActiveEffectsComponent>(ENTITY)
        .unwrap();
    EntityAuthoringService
        .replace_component(
            &mut state,
            effect_revision,
            ENTITY,
            ContinuousActiveEffectsComponent::new(
                catalog.version().clone(),
                vec![ContinuousActiveEffectInstance::new(
                    id(ContinuousEffectInstanceId::parse, "boost_one"),
                    id(ContinuousEffectDefinitionId::parse, "boost_effect"),
                )],
            )
            .unwrap(),
        )
        .unwrap();
    let track_revision = state
        .component_revision::<ContinuousTracksComponent>(ENTITY)
        .unwrap();
    EntityAuthoringService
        .replace_component(
            &mut state,
            track_revision,
            ENTITY,
            ContinuousTracksComponent::new(
                catalog.version().clone(),
                vec![ContinuousTrackValue::new(
                    id(ContinuousTrackId::parse, "energy"),
                    value(2.9),
                )],
            )
            .unwrap(),
        )
        .unwrap();
    let before = state
        .component_revision::<ContinuousActiveEffectsComponent>(ENTITY)
        .unwrap();
    let error = ContinuousEffectService::remove(
        &mut state,
        &catalog,
        ContinuousEffectRemoveRequest {
            operation: id(ContinuousOperationId::parse, "remove_boost"),
            entity: ENTITY,
            instance: id(ContinuousEffectInstanceId::parse, "boost_one"),
            expected_revision: Some(before.clone()),
        },
    )
    .unwrap_err();
    assert!(matches!(
        error,
        ContinuousMechanicsError::WouldInvalidateTrack { .. }
    ));
    assert_eq!(
        state
            .component_revision::<ContinuousActiveEffectsComponent>(ENTITY)
            .unwrap(),
        before
    );
}
