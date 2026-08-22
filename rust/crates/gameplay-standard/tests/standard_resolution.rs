use core_ids::EntityId;
use entity_state::{EntityAuthoringService, EntityComponent, EntityDefinition, EntityState};
use gameplay_mechanics::{
    ActiveEffectInstance, ActiveEffectsComponent, CapacityMetricDefinition, CapacityMetricId,
    CatalogVersion, DamageKindDefinition, DamageKindId, EffectDefinition, EffectDefinitionId,
    EffectMutationKind, EffectStackingPolicy, IntrinsicSourceBinding, IntrinsicSourcesComponent,
    InventoryCapacityLimit, InventoryComponent, InventoryService, ItemCapacityCost, ItemComponent,
    ItemDefinition, ItemDefinitionId, ItemKind, ItemStack, MechanicsCatalog,
    MechanicsCatalogDefinition, MechanicsError, MechanicsScalar, OperationId, RequestSource,
    SourceDefinition, SourceDefinitionId, SourceInstanceId, SourceInstanceIdentity,
    StackingGroupId, StackingPolicy, StatContribution, StatContributionDefinition, StatDefinition,
    StatId, StatValue, StatsComponent, TrackDefinition, TrackId, TrackMaximum, TrackValue,
    TracksComponent, MAX_DAMAGE_PARTS, MAX_DAMAGE_REQUEST_SOURCES,
};
use gameplay_resolution::{
    AttemptStatus, CommitStatus, CorrelationId, PolicyFailure, PolicyResult, Program, ResolutionId,
    ResolutionIdentity, ResolutionMode, ResolutionPlan, ResolutionPolicy, ResolutionRequest,
    ResolutionTraceSink, ResolutionTransaction, StandardResolver,
};
use gameplay_rules::{
    RuleDomainId, RulePackageId, RulePackageSchemaVersion, RuleProvenance, RuleSource,
    RuleSourceId, RuleSubjectId, RuleVersion,
};
use gameplay_standard::{
    CapabilityRequirementId, CapabilityRoleBinding, CapabilityRoleBindings, CapabilityRoleId,
    ComposedOperation, ComposedPredicate, ExactComparison, ExactExpr, ExactInputBundle,
    ExactInputReference, InputId, StandardMechanicsReceiptProjection, StandardOperation,
    StandardOperationContext, StandardPlanningError, StandardPredicate,
    MAX_CAPABILITY_ROLE_BINDINGS, STANDARD_DAMAGE_CAPABILITY, STANDARD_EFFECT_CAPABILITY,
    STANDARD_TRACK_CAPABILITY,
};

const CASTER: EntityId = EntityId::new(7);
const TARGET: EntityId = EntityId::new(8);

fn scalar(value: i64) -> MechanicsScalar {
    MechanicsScalar::new(value).unwrap()
}
fn role(value: &str) -> CapabilityRoleId {
    CapabilityRoleId::parse(value).unwrap()
}
fn capability(value: &str) -> CapabilityRequirementId {
    CapabilityRequirementId::parse(value).unwrap()
}
fn track(value: &str) -> TrackId {
    TrackId::parse(value).unwrap()
}
fn operation(value: &str) -> OperationId {
    OperationId::parse(value).unwrap()
}

fn request_source(index: usize) -> RequestSource {
    RequestSource {
        instance: SourceInstanceId::parse(format!("request_source_{index}")).unwrap(),
        definition: SourceDefinitionId::parse("ward_source").unwrap(),
    }
}

fn catalog() -> MechanicsCatalog {
    catalog_with_version("standard.v1")
}

fn catalog_with_version(version: &str) -> MechanicsCatalog {
    MechanicsCatalog::admit(MechanicsCatalogDefinition {
        version: CatalogVersion::parse(version).unwrap(),
        stats: vec![],
        tracks: vec![TrackDefinition {
            id: track("vitality"),
            minimum: scalar(0),
            maximum: TrackMaximum::Fixed { value: scalar(20) },
        }],
        sources: vec![SourceDefinition {
            id: SourceDefinitionId::parse("ward_source").unwrap(),
            priority: 0,
            stat_contributions: vec![],
            damage_responses: vec![],
        }],
        damage_kinds: vec![DamageKindDefinition {
            id: DamageKindId::parse("impact").unwrap(),
        }],
        effects: vec![EffectDefinition {
            id: EffectDefinitionId::parse("ward").unwrap(),
            stacking_group: StackingGroupId::parse("ward").unwrap(),
            stacking: EffectStackingPolicy::IndependentByProvenance {
                maximum_instances: 2,
            },
            maximum_stacks: 1,
            sources: vec![SourceDefinitionId::parse("ward_source").unwrap()],
        }],
        capacity_metrics: vec![],
        items: vec![],
        equipment_slots: vec![],
    })
    .unwrap()
}

fn effect_catalog() -> MechanicsCatalog {
    MechanicsCatalog::admit(MechanicsCatalogDefinition {
        version: CatalogVersion::parse("standard.v1").unwrap(),
        stats: vec![StatDefinition {
            id: StatId::parse("maximum").unwrap(),
            minimum: scalar(1),
            maximum: scalar(100),
        }],
        tracks: vec![TrackDefinition {
            id: track("vitality"),
            minimum: scalar(0),
            maximum: TrackMaximum::Stat {
                stat: StatId::parse("maximum").unwrap(),
            },
        }],
        sources: vec![
            SourceDefinition {
                id: SourceDefinitionId::parse("boost_source").unwrap(),
                priority: 0,
                stat_contributions: vec![StatContributionDefinition {
                    stat: StatId::parse("maximum").unwrap(),
                    contribution: StatContribution::Add { amount: scalar(10) },
                    stacking_group: StackingGroupId::parse("boost").unwrap(),
                    stacking: StackingPolicy::Sum,
                }],
                damage_responses: vec![],
            },
            SourceDefinition {
                id: SourceDefinitionId::parse("empty_source").unwrap(),
                priority: 0,
                stat_contributions: vec![],
                damage_responses: vec![],
            },
        ],
        damage_kinds: vec![],
        effects: vec![
            EffectDefinition {
                id: EffectDefinitionId::parse("refreshing").unwrap(),
                stacking_group: StackingGroupId::parse("refresh").unwrap(),
                stacking: EffectStackingPolicy::Refresh,
                maximum_stacks: 2,
                sources: vec![SourceDefinitionId::parse("empty_source").unwrap()],
            },
            EffectDefinition {
                id: EffectDefinitionId::parse("armored").unwrap(),
                stacking_group: StackingGroupId::parse("armor").unwrap(),
                stacking: EffectStackingPolicy::Replace,
                maximum_stacks: 1,
                sources: vec![SourceDefinitionId::parse("boost_source").unwrap()],
            },
            EffectDefinition {
                id: EffectDefinitionId::parse("unarmored").unwrap(),
                stacking_group: StackingGroupId::parse("armor").unwrap(),
                stacking: EffectStackingPolicy::Replace,
                maximum_stacks: 1,
                sources: vec![SourceDefinitionId::parse("empty_source").unwrap()],
            },
            EffectDefinition {
                id: EffectDefinitionId::parse("other").unwrap(),
                stacking_group: StackingGroupId::parse("other").unwrap(),
                stacking: EffectStackingPolicy::Replace,
                maximum_stacks: 1,
                sources: vec![SourceDefinitionId::parse("empty_source").unwrap()],
            },
            EffectDefinition {
                id: EffectDefinitionId::parse("independent").unwrap(),
                stacking_group: StackingGroupId::parse("independent").unwrap(),
                stacking: EffectStackingPolicy::IndependentByProvenance {
                    maximum_instances: 2,
                },
                maximum_stacks: 1,
                sources: vec![SourceDefinitionId::parse("empty_source").unwrap()],
            },
        ],
        capacity_metrics: vec![],
        items: vec![],
        equipment_slots: vec![],
    })
    .unwrap()
}

fn attach<T: EntityComponent>(state: &mut EntityState, entity: EntityId, value: T) {
    let revision = state.component_revision::<T>(entity).unwrap();
    EntityAuthoringService
        .attach_component(state, revision, entity, value)
        .unwrap();
}

fn state() -> EntityState {
    let mut state = EntityState::from_definitions_with_registry(
        gameplay_mechanics::gameplay_component_registry().unwrap(),
        [
            EntityDefinition::new(CASTER, "caster"),
            EntityDefinition::new(TARGET, "target"),
        ],
    )
    .unwrap();
    for entity in [CASTER, TARGET] {
        attach(
            &mut state,
            entity,
            TracksComponent::new(
                CatalogVersion::parse("standard.v1").unwrap(),
                vec![TrackValue::new(track("vitality"), scalar(10))],
            )
            .unwrap(),
        );
        attach(
            &mut state,
            entity,
            ActiveEffectsComponent::new(CatalogVersion::parse("standard.v1").unwrap(), vec![])
                .unwrap(),
        );
    }
    state
}

fn effect_state(effects: Vec<ActiveEffectInstance>, vitality: i64) -> EntityState {
    let mut state = state();
    for entity in [CASTER, TARGET] {
        attach(
            &mut state,
            entity,
            StatsComponent::new(
                CatalogVersion::parse("standard.v1").unwrap(),
                vec![StatValue::new(
                    StatId::parse("maximum").unwrap(),
                    scalar(10),
                )],
            )
            .unwrap(),
        );
    }
    let tracks_revision = state.component_revision::<TracksComponent>(CASTER).unwrap();
    EntityAuthoringService
        .replace_component(
            &mut state,
            tracks_revision,
            CASTER,
            TracksComponent::new(
                CatalogVersion::parse("standard.v1").unwrap(),
                vec![TrackValue::new(track("vitality"), scalar(vitality))],
            )
            .unwrap(),
        )
        .unwrap();
    let effects_revision = state
        .component_revision::<ActiveEffectsComponent>(CASTER)
        .unwrap();
    EntityAuthoringService
        .replace_component(
            &mut state,
            effects_revision,
            CASTER,
            ActiveEffectsComponent::new(CatalogVersion::parse("standard.v1").unwrap(), effects)
                .unwrap(),
        )
        .unwrap();
    state
}

fn effect_bindings(operation: &StandardOperation) -> CapabilityRoleBindings {
    CapabilityRoleBindings::admit(
        &operation.requirements(),
        vec![CapabilityRoleBinding::new(
            role("caster"),
            CASTER,
            vec![capability(STANDARD_EFFECT_CAPABILITY)],
        )
        .unwrap()],
    )
    .unwrap()
}

fn context() -> StandardOperationContext {
    let operation = operation("standard_attempt");
    StandardOperationContext::new(
        operation.clone(),
        SourceInstanceIdentity::Request {
            operation: operation.clone(),
            instance: SourceInstanceId::parse("standard_leaf").unwrap(),
        },
    )
    .unwrap()
}

fn item(value: &str) -> ItemDefinitionId {
    ItemDefinitionId::parse(value).unwrap()
}

fn inventory_catalog() -> MechanicsCatalog {
    MechanicsCatalog::admit(MechanicsCatalogDefinition {
        version: CatalogVersion::parse("standard.v1").unwrap(),
        stats: vec![],
        tracks: vec![TrackDefinition {
            id: track("vitality"),
            minimum: scalar(0),
            maximum: TrackMaximum::Fixed { value: scalar(20) },
        }],
        sources: vec![],
        damage_kinds: vec![],
        effects: vec![],
        capacity_metrics: vec![CapacityMetricDefinition {
            id: CapacityMetricId::parse("weight").unwrap(),
        }],
        items: vec![
            ItemDefinition {
                id: item("cells"),
                kind: ItemKind::Fungible,
                maximum_quantity: 10,
                classifications: vec![],
                capacity_costs: vec![ItemCapacityCost {
                    metric: CapacityMetricId::parse("weight").unwrap(),
                    units: 2,
                }],
                equipment: None,
                sources: vec![],
            },
            ItemDefinition {
                id: item("unique-key"),
                kind: ItemKind::Unique,
                maximum_quantity: 1,
                classifications: vec![],
                capacity_costs: vec![],
                equipment: None,
                sources: vec![],
            },
        ],
        equipment_slots: vec![],
    })
    .unwrap()
}

fn inventory_state(caster_quantity: u64, target_quantity: u64) -> EntityState {
    let mut state = state();
    for (owner, quantity) in [(CASTER, caster_quantity), (TARGET, target_quantity)] {
        let stacks = (quantity != 0)
            .then(|| ItemStack {
                definition: item("cells"),
                quantity,
            })
            .into_iter()
            .collect();
        attach(
            &mut state,
            owner,
            InventoryComponent::with_capacity_limits(
                CatalogVersion::parse("standard.v1").unwrap(),
                stacks,
                vec![InventoryCapacityLimit::new(
                    CapacityMetricId::parse("weight").unwrap(),
                    10,
                )],
            )
            .unwrap(),
        );
    }
    state
}

const CONTAINED_ITEM: EntityId = EntityId::new(9);

fn attach_contained_unique_item(state: &mut EntityState) {
    let revision = state.revision();
    EntityAuthoringService
        .admit(
            state,
            revision,
            [EntityDefinition::new(CONTAINED_ITEM, "contained-item").with_containment(CASTER)],
        )
        .unwrap();
    attach(
        state,
        CONTAINED_ITEM,
        ItemComponent::new(
            CatalogVersion::parse("standard.v1").unwrap(),
            item("unique-key"),
        ),
    );
}

fn inventory_bindings(operation: &StandardOperation) -> CapabilityRoleBindings {
    CapabilityRoleBindings::admit(
        &operation.requirements(),
        vec![
            CapabilityRoleBinding::new(
                role("from"),
                CASTER,
                vec![capability(gameplay_standard::STANDARD_INVENTORY_CAPABILITY)],
            )
            .unwrap(),
            CapabilityRoleBinding::new(
                role("to"),
                TARGET,
                vec![capability(gameplay_standard::STANDARD_INVENTORY_CAPABILITY)],
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

fn stack_quantity(state: &EntityState, catalog: &MechanicsCatalog, owner: EntityId) -> u64 {
    InventoryService::view(state, catalog, owner)
        .unwrap()
        .stacks()
        .iter()
        .find(|stack| stack.definition == item("cells"))
        .map_or(0, |stack| stack.quantity)
}

#[test]
fn typed_roles_predicates_and_mechanics_plans_remain_explicit() {
    let state = state();
    let spend = StandardOperation::SpendTrack {
        role: role("caster"),
        track: track("vitality"),
        amount: ExactExpr::Literal(scalar(3)).into(),
    };
    let requirements = spend.requirements();
    let roles = CapabilityRoleBindings::admit(
        &requirements,
        vec![CapabilityRoleBinding::new(
            role("caster"),
            CASTER,
            vec![capability(STANDARD_TRACK_CAPABILITY)],
        )
        .unwrap()],
    )
    .unwrap();
    let plan = spend
        .plan(
            &roles,
            &ExactInputBundle::empty(),
            &state,
            &catalog(),
            &context(),
        )
        .unwrap();
    assert_eq!(
        plan.observed_revisions().len(),
        gameplay_mechanics::MechanicsComponentKind::ALL.len()
    );
    assert_eq!(plan.catalog().version().as_str(), "standard.v1");
    assert_eq!(plan.exact_evaluations().len(), 1);
    assert_eq!(
        plan.exact_evaluations()[0].semantics_version(),
        gameplay_standard::EXACT_EVALUATOR_SEMANTICS_VERSION
    );
    assert_eq!(plan.exact_evaluations()[0].result(), scalar(3));
    assert_eq!(
        plan.exact_evaluations()[0].expression(),
        &ExactExpr::Literal(scalar(3))
    );
    assert_eq!(plan.observed_revisions()[0].entity(), CASTER);
    assert!(StandardPredicate::Exact(ExactComparison::GreaterThan(
        ExactExpr::Literal(scalar(2)),
        ExactExpr::Literal(scalar(1))
    ))
    .evaluate(&ExactInputBundle::empty())
    .unwrap());
    assert!(CapabilityRoleBindings::admit(
        &requirements,
        vec![CapabilityRoleBinding::new(role("caster"), CASTER, vec![]).unwrap()]
    )
    .is_err());
}

#[test]
fn role_binding_admission_has_a_total_bound() {
    let bindings = (0..MAX_CAPABILITY_ROLE_BINDINGS)
        .map(|index| {
            CapabilityRoleBinding::new(
                role(&format!("role{index}")),
                CASTER,
                vec![capability(STANDARD_TRACK_CAPABILITY)],
            )
            .unwrap()
        })
        .collect();
    assert!(CapabilityRoleBindings::admit(&[], bindings).is_ok());

    let one_over = (0..=MAX_CAPABILITY_ROLE_BINDINGS)
        .map(|index| {
            CapabilityRoleBinding::new(
                role(&format!("role{index}")),
                CASTER,
                vec![capability(STANDARD_TRACK_CAPABILITY)],
            )
            .unwrap()
        })
        .collect();
    assert!(matches!(
        CapabilityRoleBindings::admit(&[], one_over),
        Err(gameplay_standard::StandardRoleAdmissionError::RoleBindingQuotaExceeded {
            actual,
            maximum,
        }) if actual == MAX_CAPABILITY_ROLE_BINDINGS + 1
            && maximum == MAX_CAPABILITY_ROLE_BINDINGS
    ));
}

#[test]
fn candidate_execution_rebases_private_revisions_and_never_mutates_the_planning_state() {
    let catalog = catalog();
    let authoritative = state();
    let spend = StandardOperation::SpendTrack {
        role: role("caster"),
        track: track("vitality"),
        amount: ExactExpr::Literal(scalar(3)).into(),
    };
    let restore = StandardOperation::RestoreTrack {
        role: role("caster"),
        track: track("vitality"),
        amount: ExactExpr::Literal(scalar(1)).into(),
    };
    let roles = CapabilityRoleBindings::admit(
        &spend.requirements(),
        vec![CapabilityRoleBinding::new(
            role("caster"),
            CASTER,
            vec![
                capability(STANDARD_TRACK_CAPABILITY),
                capability(STANDARD_EFFECT_CAPABILITY),
            ],
        )
        .unwrap()],
    )
    .unwrap();
    let spend = spend
        .plan(
            &roles,
            &ExactInputBundle::empty(),
            &authoritative,
            &catalog,
            &context(),
        )
        .unwrap();
    let restore = restore
        .plan(
            &roles,
            &ExactInputBundle::empty(),
            &authoritative,
            &catalog,
            &context(),
        )
        .unwrap();
    let apply = StandardOperation::ApplyEffect {
        role: role("caster"),
        instance: gameplay_mechanics::EffectInstanceId::parse("ward_one").unwrap(),
        definition: EffectDefinitionId::parse("ward").unwrap(),
        stacks: 1,
    }
    .plan(
        &roles,
        &ExactInputBundle::empty(),
        &authoritative,
        &catalog,
        &context(),
    )
    .unwrap();
    let remove = StandardOperation::RemoveEffect {
        role: role("caster"),
        instance: gameplay_mechanics::EffectInstanceId::parse("ward_one").unwrap(),
    }
    .plan(
        &roles,
        &ExactInputBundle::empty(),
        &authoritative,
        &catalog,
        &context(),
    )
    .unwrap();
    let mut candidate = gameplay_mechanics::decode_snapshot_with_catalog(
        &entity_state::encode_snapshot(&authoritative).unwrap(),
        &catalog,
    )
    .unwrap();
    restore
        .effect()
        .apply_to_candidate(&mut candidate, &catalog)
        .unwrap();
    let receipt = spend
        .effect()
        .apply_to_candidate(&mut candidate, &catalog)
        .unwrap();
    match receipt {
        gameplay_standard::StandardMechanicsReceipt::Track(receipt) => {
            assert_eq!(receipt.operation, *context().operation());
            assert_eq!(receipt.source, *context().source());
        }
        _ => panic!("spend keeps its typed track receipt"),
    }
    apply
        .effect()
        .apply_to_candidate(&mut candidate, &catalog)
        .unwrap();
    remove
        .effect()
        .apply_to_candidate(&mut candidate, &catalog)
        .unwrap();
    assert_eq!(
        authoritative
            .component::<TracksComponent>(CASTER)
            .unwrap()
            .unwrap()
            .current(&track("vitality")),
        Some(scalar(10))
    );
    assert_eq!(
        candidate
            .component::<TracksComponent>(CASTER)
            .unwrap()
            .unwrap()
            .current(&track("vitality")),
        Some(scalar(8))
    );
    assert!(candidate
        .component::<ActiveEffectsComponent>(CASTER)
        .unwrap()
        .unwrap()
        .effects()
        .is_empty());
}

#[test]
fn remove_existing_then_apply_same_effect_plans_and_executes_on_one_candidate() {
    let catalog = catalog();
    let mut authoritative = state();
    let existing = ActiveEffectsComponent::new(
        CatalogVersion::parse("standard.v1").unwrap(),
        vec![ActiveEffectInstance::new(
            gameplay_mechanics::EffectInstanceId::parse("ward_existing").unwrap(),
            EffectDefinitionId::parse("ward").unwrap(),
            context().source().clone(),
            1,
        )
        .unwrap()],
    )
    .unwrap();
    let effects_revision = authoritative
        .component_revision::<ActiveEffectsComponent>(CASTER)
        .unwrap();
    EntityAuthoringService
        .replace_component(&mut authoritative, effects_revision, CASTER, existing)
        .unwrap();
    let remove = StandardOperation::RemoveEffect {
        role: role("caster"),
        instance: gameplay_mechanics::EffectInstanceId::parse("ward_existing").unwrap(),
    };
    let apply = StandardOperation::ApplyEffect {
        role: role("caster"),
        instance: gameplay_mechanics::EffectInstanceId::parse("ward_existing").unwrap(),
        definition: EffectDefinitionId::parse("ward").unwrap(),
        stacks: 1,
    };
    let roles = CapabilityRoleBindings::admit(
        &remove.requirements(),
        vec![CapabilityRoleBinding::new(
            role("caster"),
            CASTER,
            vec![capability(STANDARD_EFFECT_CAPABILITY)],
        )
        .unwrap()],
    )
    .unwrap();
    let remove = remove
        .plan(
            &roles,
            &ExactInputBundle::empty(),
            &authoritative,
            &catalog,
            &context(),
        )
        .unwrap();
    let apply = apply
        .plan(
            &roles,
            &ExactInputBundle::empty(),
            &authoritative,
            &catalog,
            &context(),
        )
        .unwrap();
    let mut candidate = gameplay_mechanics::decode_snapshot_with_catalog(
        &entity_state::encode_snapshot(&authoritative).unwrap(),
        &catalog,
    )
    .unwrap();
    remove
        .effect()
        .apply_to_candidate(&mut candidate, &catalog)
        .unwrap();
    apply
        .effect()
        .apply_to_candidate(&mut candidate, &catalog)
        .unwrap();
    assert_eq!(
        candidate
            .component::<ActiveEffectsComponent>(CASTER)
            .unwrap()
            .unwrap()
            .effects()
            .len(),
        1
    );
}

#[test]
fn refresh_and_replace_effect_leaves_keep_policy_context_and_typed_receipts() {
    let catalog = effect_catalog();
    let refresh_instance = gameplay_mechanics::EffectInstanceId::parse("refresh_one").unwrap();
    let refresh = StandardOperation::RefreshEffect {
        role: role("caster"),
        instance: refresh_instance.clone(),
        stacks: 2,
    };
    let authoritative = effect_state(
        vec![ActiveEffectInstance::new(
            refresh_instance.clone(),
            EffectDefinitionId::parse("refreshing").unwrap(),
            SourceInstanceIdentity::Request {
                operation: operation("before_refresh"),
                instance: SourceInstanceId::parse("before_refresh_source").unwrap(),
            },
            1,
        )
        .unwrap()],
        10,
    );
    let plan = refresh
        .plan(
            &effect_bindings(&refresh),
            &ExactInputBundle::new(vec![]),
            &authoritative,
            &catalog,
            &context(),
        )
        .unwrap();
    assert!(plan.observed_revisions().iter().any(|observed| {
        observed.entity() == CASTER
            && observed.component() == gameplay_mechanics::MechanicsComponentKind::ActiveEffects
    }));
    let mut candidate = gameplay_mechanics::decode_snapshot_with_catalog(
        &entity_state::encode_snapshot(&authoritative).unwrap(),
        &catalog,
    )
    .unwrap();
    let receipt = plan
        .effect()
        .apply_to_candidate(&mut candidate, &catalog)
        .unwrap();
    assert_eq!(
        StandardMechanicsReceiptProjection(&receipt)
            .effect()
            .unwrap()
            .kind,
        EffectMutationKind::Refresh,
        "the existing borrowed receipt projection exposes refresh evidence without a DTO"
    );
    let gameplay_standard::StandardMechanicsReceipt::Effect(receipt) = receipt else {
        panic!("refresh preserves the native typed effect receipt");
    };
    assert_eq!(receipt.kind, EffectMutationKind::Refresh);
    assert_eq!(receipt.operation, *context().operation());
    assert_eq!(
        receipt.current.as_ref().unwrap().provenance(),
        context().source()
    );
    assert_eq!(receipt.current.as_ref().unwrap().stacks(), 2);
    assert_eq!(
        authoritative
            .component::<ActiveEffectsComponent>(CASTER)
            .unwrap()
            .unwrap()
            .effects()[0]
            .stacks(),
        1,
        "planning and candidate execution never publish to the authority source"
    );
    let stale_revision = authoritative
        .component_revision::<ActiveEffectsComponent>(CASTER)
        .unwrap();
    let stale_component = ActiveEffectsComponent::new(
        CatalogVersion::parse("standard.v1").unwrap(),
        vec![ActiveEffectInstance::new(
            refresh_instance,
            EffectDefinitionId::parse("refreshing").unwrap(),
            context().source().clone(),
            2,
        )
        .unwrap()],
    )
    .unwrap();
    let mut stale_source = authoritative;
    EntityAuthoringService
        .replace_component(&mut stale_source, stale_revision, CASTER, stale_component)
        .unwrap();
    assert!(matches!(
        plan.validate_source_state(&stale_source, &catalog),
        Err(gameplay_standard::StandardPlanValidationError::StaleComponentRevision { .. })
    ));
    assert!(matches!(
        plan.validate_source_state(&stale_source, &catalog_with_version("standard.v2")),
        Err(gameplay_standard::StandardPlanValidationError::CatalogChanged { .. })
    ));

    let replacement_instance = gameplay_mechanics::EffectInstanceId::parse("armor").unwrap();
    let replace = StandardOperation::ReplaceEffect {
        role: role("caster"),
        instance: replacement_instance.clone(),
        definition: EffectDefinitionId::parse("unarmored").unwrap(),
        stacks: 1,
    };
    let authoritative = effect_state(
        vec![ActiveEffectInstance::new(
            replacement_instance.clone(),
            EffectDefinitionId::parse("armored").unwrap(),
            context().source().clone(),
            1,
        )
        .unwrap()],
        10,
    );
    let plan = replace
        .plan(
            &effect_bindings(&replace),
            &ExactInputBundle::new(vec![]),
            &authoritative,
            &catalog,
            &context(),
        )
        .unwrap();
    let mut candidate = gameplay_mechanics::decode_snapshot_with_catalog(
        &entity_state::encode_snapshot(&authoritative).unwrap(),
        &catalog,
    )
    .unwrap();
    let receipt = plan
        .effect()
        .apply_to_candidate(&mut candidate, &catalog)
        .unwrap();
    let gameplay_standard::StandardMechanicsReceipt::Effect(receipt) = receipt else {
        panic!("replace preserves the native typed effect receipt");
    };
    assert_eq!(receipt.kind, EffectMutationKind::Replace);
    assert_eq!(receipt.removed.len(), 1);
    assert_eq!(
        receipt.removed[0].definition(),
        &EffectDefinitionId::parse("armored").unwrap()
    );
    assert_eq!(
        receipt.current.as_ref().unwrap().instance(),
        &replacement_instance
    );
    assert_eq!(
        receipt.current.as_ref().unwrap().definition(),
        &EffectDefinitionId::parse("unarmored").unwrap()
    );
}

#[test]
fn effect_leaf_planning_rejects_missing_policy_stack_and_unrelated_identity_conflicts() {
    let catalog = effect_catalog();
    let state = effect_state(vec![], 10);
    let missing = StandardOperation::RefreshEffect {
        role: role("caster"),
        instance: gameplay_mechanics::EffectInstanceId::parse("missing").unwrap(),
        stacks: 1,
    };
    assert!(matches!(
        missing.plan(
            &effect_bindings(&missing),
            &ExactInputBundle::new(vec![]),
            &state,
            &catalog,
            &context(),
        ),
        Err(StandardPlanningError::MissingEffectInstance { .. })
    ));

    let independent_instance = gameplay_mechanics::EffectInstanceId::parse("independent").unwrap();
    let independent_state = effect_state(
        vec![ActiveEffectInstance::new(
            independent_instance.clone(),
            EffectDefinitionId::parse("independent").unwrap(),
            context().source().clone(),
            1,
        )
        .unwrap()],
        10,
    );
    let wrong_policy = StandardOperation::RefreshEffect {
        role: role("caster"),
        instance: independent_instance,
        stacks: 1,
    };
    assert!(matches!(
        wrong_policy.plan(
            &effect_bindings(&wrong_policy),
            &ExactInputBundle::new(vec![]),
            &independent_state,
            &catalog,
            &context(),
        ),
        Err(StandardPlanningError::EffectPolicyMismatch {
            expected: "refresh",
            ..
        })
    ));

    let invalid_stacks = StandardOperation::ReplaceEffect {
        role: role("caster"),
        instance: gameplay_mechanics::EffectInstanceId::parse("armor").unwrap(),
        definition: EffectDefinitionId::parse("unarmored").unwrap(),
        stacks: 0,
    };
    assert!(matches!(
        invalid_stacks.plan(
            &effect_bindings(&invalid_stacks),
            &ExactInputBundle::new(vec![]),
            &state,
            &catalog,
            &context(),
        ),
        Err(StandardPlanningError::EffectStacks { actual: 0, .. })
    ));
    let over_stacks = StandardOperation::ReplaceEffect {
        role: role("caster"),
        instance: gameplay_mechanics::EffectInstanceId::parse("armor").unwrap(),
        definition: EffectDefinitionId::parse("unarmored").unwrap(),
        stacks: 2,
    };
    assert!(matches!(
        over_stacks.plan(
            &effect_bindings(&over_stacks),
            &ExactInputBundle::new(vec![]),
            &state,
            &catalog,
            &context(),
        ),
        Err(StandardPlanningError::EffectStacks {
            actual: 2,
            maximum: 1,
        })
    ));

    let conflicting_instance = gameplay_mechanics::EffectInstanceId::parse("shared").unwrap();
    let conflict_state = effect_state(
        vec![ActiveEffectInstance::new(
            conflicting_instance.clone(),
            EffectDefinitionId::parse("other").unwrap(),
            context().source().clone(),
            1,
        )
        .unwrap()],
        10,
    );
    let conflict = StandardOperation::ReplaceEffect {
        role: role("caster"),
        instance: conflicting_instance,
        definition: EffectDefinitionId::parse("unarmored").unwrap(),
        stacks: 1,
    };
    assert!(matches!(
        conflict.plan(
            &effect_bindings(&conflict),
            &ExactInputBundle::new(vec![]),
            &conflict_state,
            &catalog,
            &context(),
        ),
        Err(StandardPlanningError::EffectInstanceConflict { .. })
    ));
}

#[test]
fn effect_leaves_rebase_sequential_candidate_revisions_without_publishing() {
    let catalog = effect_catalog();
    let refresh_instance = gameplay_mechanics::EffectInstanceId::parse("refresh_one").unwrap();
    let armor_instance = gameplay_mechanics::EffectInstanceId::parse("armor").unwrap();
    let authoritative = effect_state(
        vec![
            ActiveEffectInstance::new(
                refresh_instance.clone(),
                EffectDefinitionId::parse("refreshing").unwrap(),
                context().source().clone(),
                1,
            )
            .unwrap(),
            ActiveEffectInstance::new(
                armor_instance.clone(),
                EffectDefinitionId::parse("armored").unwrap(),
                context().source().clone(),
                1,
            )
            .unwrap(),
        ],
        10,
    );
    let refresh = StandardOperation::RefreshEffect {
        role: role("caster"),
        instance: refresh_instance,
        stacks: 2,
    };
    let replace = StandardOperation::ReplaceEffect {
        role: role("caster"),
        instance: armor_instance,
        definition: EffectDefinitionId::parse("unarmored").unwrap(),
        stacks: 1,
    };
    let refresh_plan = refresh
        .plan(
            &effect_bindings(&refresh),
            &ExactInputBundle::new(vec![]),
            &authoritative,
            &catalog,
            &context(),
        )
        .unwrap();
    let replace_plan = replace
        .plan(
            &effect_bindings(&replace),
            &ExactInputBundle::new(vec![]),
            &authoritative,
            &catalog,
            &context(),
        )
        .unwrap();
    let mut candidate = gameplay_mechanics::decode_snapshot_with_catalog(
        &entity_state::encode_snapshot(&authoritative).unwrap(),
        &catalog,
    )
    .unwrap();
    refresh_plan
        .effect()
        .apply_to_candidate(&mut candidate, &catalog)
        .unwrap();
    replace_plan
        .effect()
        .apply_to_candidate(&mut candidate, &catalog)
        .unwrap();
    let effects = candidate
        .component::<ActiveEffectsComponent>(CASTER)
        .unwrap()
        .unwrap()
        .effects();
    assert_eq!(effects.len(), 2);
    assert_eq!(
        effects
            .iter()
            .find(|effect| effect.definition() == &EffectDefinitionId::parse("refreshing").unwrap())
            .unwrap()
            .stacks(),
        2
    );
    assert!(effects
        .iter()
        .any(|effect| effect.definition() == &EffectDefinitionId::parse("unarmored").unwrap()));
    assert!(authoritative
        .component::<ActiveEffectsComponent>(CASTER)
        .unwrap()
        .unwrap()
        .effects()
        .iter()
        .any(|effect| effect.definition() == &EffectDefinitionId::parse("armored").unwrap()));
}

#[test]
fn effect_replace_rebases_private_candidate_and_is_atomic_when_it_would_strand_a_track() {
    let catalog = effect_catalog();
    let instance = gameplay_mechanics::EffectInstanceId::parse("armor").unwrap();
    let authoritative = effect_state(
        vec![ActiveEffectInstance::new(
            instance.clone(),
            EffectDefinitionId::parse("armored").unwrap(),
            context().source().clone(),
            1,
        )
        .unwrap()],
        20,
    );
    let replace = StandardOperation::ReplaceEffect {
        role: role("caster"),
        instance,
        definition: EffectDefinitionId::parse("unarmored").unwrap(),
        stacks: 1,
    };
    let plan = replace
        .plan(
            &effect_bindings(&replace),
            &ExactInputBundle::new(vec![]),
            &authoritative,
            &catalog,
            &context(),
        )
        .unwrap();
    let mut candidate = gameplay_mechanics::decode_snapshot_with_catalog(
        &entity_state::encode_snapshot(&authoritative).unwrap(),
        &catalog,
    )
    .unwrap();
    let candidate_effects_before = candidate
        .component::<ActiveEffectsComponent>(CASTER)
        .unwrap()
        .unwrap()
        .clone();
    let candidate_effects_revision_before = candidate
        .component_revision::<ActiveEffectsComponent>(CASTER)
        .unwrap();
    let candidate_tracks_revision_before = candidate
        .component_revision::<TracksComponent>(CASTER)
        .unwrap();
    let candidate_state_revision_before = candidate.revision();
    assert!(matches!(
        plan.effect().apply_to_candidate(&mut candidate, &catalog),
        Err(MechanicsError::EffectWouldInvalidateTrack {
            current: 20,
            prospective_maximum: 10,
            ..
        })
    ));
    assert_eq!(
        candidate
            .component::<ActiveEffectsComponent>(CASTER)
            .unwrap()
            .unwrap(),
        &candidate_effects_before
    );
    assert_eq!(
        candidate
            .component_revision::<ActiveEffectsComponent>(CASTER)
            .unwrap(),
        candidate_effects_revision_before
    );
    assert_eq!(
        candidate
            .component_revision::<TracksComponent>(CASTER)
            .unwrap(),
        candidate_tracks_revision_before
    );
    assert_eq!(candidate.revision(), candidate_state_revision_before);
    assert_eq!(
        authoritative
            .component::<ActiveEffectsComponent>(CASTER)
            .unwrap()
            .unwrap()
            .effects()[0]
            .definition(),
        &EffectDefinitionId::parse("armored").unwrap(),
        "candidate failure never reaches the authority source"
    );
}

#[test]
fn planning_rejects_oversized_requests_and_role_capability_retention() {
    let catalog = catalog();
    let state = state();
    let target = role("target");
    let damage = StandardOperation::SubmitDamage {
        actor: None,
        target: target.clone(),
        target_track: track("vitality"),
        parts: vec![],
        request_sources: vec![],
    };
    let bindings = CapabilityRoleBindings::admit(
        &damage.requirements(),
        vec![CapabilityRoleBinding::new(
            target,
            TARGET,
            vec![capability(STANDARD_DAMAGE_CAPABILITY)],
        )
        .unwrap()],
    )
    .unwrap();
    assert!(matches!(
        damage.plan(
            &bindings,
            &ExactInputBundle::empty(),
            &state,
            &catalog,
            &context()
        ),
        Err(StandardPlanningError::DamageParts { actual: 0, .. })
    ));
    let too_many = vec![
        capability(STANDARD_TRACK_CAPABILITY);
        gameplay_standard::MAX_CAPABILITY_REQUIREMENTS_PER_ROLE + 1
    ];
    assert!(CapabilityRoleBinding::new(role("caster"), CASTER, too_many).is_err());
    let invalid_effect = StandardOperation::ApplyEffect {
        role: role("caster"),
        instance: gameplay_mechanics::EffectInstanceId::parse("invalid").unwrap(),
        definition: EffectDefinitionId::parse("ward").unwrap(),
        stacks: 0,
    };
    let effect_bindings = CapabilityRoleBindings::admit(
        &invalid_effect.requirements(),
        vec![CapabilityRoleBinding::new(
            role("caster"),
            CASTER,
            vec![capability(STANDARD_EFFECT_CAPABILITY)],
        )
        .unwrap()],
    )
    .unwrap();
    assert!(matches!(
        invalid_effect.plan(
            &effect_bindings,
            &ExactInputBundle::empty(),
            &state,
            &catalog,
            &context()
        ),
        Err(StandardPlanningError::EffectStacks { actual: 0, .. })
    ));
}

#[test]
fn damage_planning_accepts_exact_quotas_and_rejects_one_over_before_expression_evaluation() {
    let catalog = catalog();
    let state = state();
    let target = role("target");
    let bindings = CapabilityRoleBindings::admit(
        &StandardOperation::SubmitDamage {
            actor: None,
            target: target.clone(),
            target_track: track("vitality"),
            parts: vec![(
                ExactExpr::Literal(scalar(1)).into(),
                DamageKindId::parse("impact").unwrap(),
            )],
            request_sources: vec![],
        }
        .requirements(),
        vec![CapabilityRoleBinding::new(
            target.clone(),
            TARGET,
            vec![capability(STANDARD_DAMAGE_CAPABILITY)],
        )
        .unwrap()],
    )
    .unwrap();
    let impact = DamageKindId::parse("impact").unwrap();

    let exact_parts = StandardOperation::SubmitDamage {
        actor: None,
        target: target.clone(),
        target_track: track("vitality"),
        parts: vec![(ExactExpr::Literal(scalar(1)).into(), impact.clone()); MAX_DAMAGE_PARTS],
        request_sources: vec![],
    };
    let exact_parts_plan = exact_parts
        .plan(
            &bindings,
            &ExactInputBundle::empty(),
            &state,
            &catalog,
            &context(),
        )
        .unwrap();
    assert_eq!(exact_parts_plan.exact_evaluations().len(), MAX_DAMAGE_PARTS);

    let exact_sources = StandardOperation::SubmitDamage {
        actor: None,
        target: target.clone(),
        target_track: track("vitality"),
        parts: vec![(ExactExpr::Literal(scalar(1)).into(), impact.clone())],
        request_sources: (0..MAX_DAMAGE_REQUEST_SOURCES)
            .map(request_source)
            .collect(),
    };
    exact_sources
        .plan(
            &bindings,
            &ExactInputBundle::empty(),
            &state,
            &catalog,
            &context(),
        )
        .unwrap();

    let missing_input = ExactExpr::Input(ExactInputReference::Parameter {
        role: target.clone(),
        id: InputId::parse("missing").unwrap(),
    });
    let too_many_parts = StandardOperation::SubmitDamage {
        actor: None,
        target: target.clone(),
        target_track: track("vitality"),
        parts: vec![(missing_input.clone().into(), impact.clone()); MAX_DAMAGE_PARTS + 1],
        request_sources: vec![],
    };
    assert!(matches!(
        too_many_parts.plan(
            &bindings,
            &ExactInputBundle::empty(),
            &state,
            &catalog,
            &context(),
        ),
        Err(StandardPlanningError::DamageParts { actual, maximum })
            if actual == MAX_DAMAGE_PARTS + 1 && maximum == MAX_DAMAGE_PARTS
    ));

    let too_many_sources = StandardOperation::SubmitDamage {
        actor: None,
        target,
        target_track: track("vitality"),
        parts: vec![(missing_input.into(), impact)],
        request_sources: (0..MAX_DAMAGE_REQUEST_SOURCES + 1)
            .map(request_source)
            .collect(),
    };
    assert!(matches!(
        too_many_sources.plan(
            &bindings,
            &ExactInputBundle::empty(),
            &state,
            &catalog,
            &context(),
        ),
        Err(StandardPlanningError::DamageRequestSources { actual, maximum })
            if actual == MAX_DAMAGE_REQUEST_SOURCES + 1 && maximum == MAX_DAMAGE_REQUEST_SOURCES
    ));
}

#[test]
fn plan_source_validation_rejects_a_changed_authoritative_component() {
    let catalog = catalog();
    let mut source = state();
    let operation = StandardOperation::SpendTrack {
        role: role("caster"),
        track: track("vitality"),
        amount: ExactExpr::Literal(scalar(1)).into(),
    };
    let bindings = CapabilityRoleBindings::admit(
        &operation.requirements(),
        vec![CapabilityRoleBinding::new(
            role("caster"),
            CASTER,
            vec![capability(STANDARD_TRACK_CAPABILITY)],
        )
        .unwrap()],
    )
    .unwrap();
    let plan = operation
        .plan(
            &bindings,
            &ExactInputBundle::empty(),
            &source,
            &catalog,
            &context(),
        )
        .unwrap();
    assert!(matches!(
        plan.validate_source_state(&source, &catalog_with_version("standard.v2")),
        Err(gameplay_standard::StandardPlanValidationError::CatalogChanged { .. })
    ));
    let revision = source
        .component_revision::<TracksComponent>(CASTER)
        .unwrap();
    let replacement = TracksComponent::new(
        CatalogVersion::parse("standard.v1").unwrap(),
        vec![TrackValue::new(track("vitality"), scalar(9))],
    )
    .unwrap();
    EntityAuthoringService
        .replace_component(&mut source, revision, CASTER, replacement)
        .unwrap();
    let validation = plan.validate_source_state(&source, &catalog);
    assert!(matches!(
        validation,
        Err(gameplay_standard::StandardPlanValidationError::StaleComponentRevision { .. })
    ));
}

#[test]
fn damage_plan_rejects_stale_secondary_effect_source_even_when_tracks_are_unchanged() {
    let catalog = catalog();
    let mut source = state();
    let operation = StandardOperation::SubmitDamage {
        actor: Some(role("caster")),
        target: role("target"),
        target_track: track("vitality"),
        parts: vec![(
            ExactExpr::Literal(scalar(1)).into(),
            DamageKindId::parse("impact").unwrap(),
        )],
        request_sources: vec![],
    };
    let bindings = CapabilityRoleBindings::admit(
        &operation.requirements(),
        vec![
            CapabilityRoleBinding::new(
                role("caster"),
                CASTER,
                vec![capability(STANDARD_DAMAGE_CAPABILITY)],
            )
            .unwrap(),
            CapabilityRoleBinding::new(
                role("target"),
                TARGET,
                vec![capability(STANDARD_DAMAGE_CAPABILITY)],
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let plan = operation
        .plan(
            &bindings,
            &ExactInputBundle::empty(),
            &source,
            &catalog,
            &context(),
        )
        .unwrap();
    assert!(plan
        .observed_revisions()
        .iter()
        .any(|observed| observed.entity() == TARGET
            && observed.component() == gameplay_mechanics::MechanicsComponentKind::ActiveEffects));

    let expected = source
        .component_revision::<ActiveEffectsComponent>(TARGET)
        .unwrap();
    let replacement = ActiveEffectsComponent::new(
        CatalogVersion::parse("standard.v1").unwrap(),
        vec![ActiveEffectInstance::new(
            gameplay_mechanics::EffectInstanceId::parse("secondary_ward").unwrap(),
            EffectDefinitionId::parse("ward").unwrap(),
            SourceInstanceIdentity::Request {
                operation: OperationId::parse("secondary_effect").unwrap(),
                instance: SourceInstanceId::parse("secondary_effect_source").unwrap(),
            },
            1,
        )
        .unwrap()],
    )
    .unwrap();
    EntityAuthoringService
        .replace_component(&mut source, expected, TARGET, replacement)
        .unwrap();
    let validation = plan.validate_source_state(&source, &catalog);
    assert!(
        matches!(
            validation,
            Err(gameplay_standard::StandardPlanValidationError::StaleComponentRevision {
                ref expected,
                ref actual,
            }) if expected.entity() == TARGET
                && expected.component() == gameplay_mechanics::MechanicsComponentKind::ActiveEffects
                && actual.revision() > expected.revision()
        ),
        "unexpected validation outcome: {validation:?}"
    );
}

#[test]
fn damage_plan_rejects_stale_intrinsic_source_even_when_target_tracks_are_unchanged() {
    let catalog = catalog();
    let mut source = state();
    attach(
        &mut source,
        TARGET,
        IntrinsicSourcesComponent::new(
            CatalogVersion::parse("standard.v1").unwrap(),
            vec![IntrinsicSourceBinding::new(
                SourceInstanceId::parse("source_before_plan").unwrap(),
                SourceDefinitionId::parse("ward_source").unwrap(),
            )],
        )
        .unwrap(),
    );
    let operation = StandardOperation::SubmitDamage {
        actor: None,
        target: role("target"),
        target_track: track("vitality"),
        parts: vec![(
            (ExactExpr::Literal(scalar(1))).into(),
            DamageKindId::parse("impact").unwrap(),
        )],
        request_sources: vec![],
    };
    let bindings = CapabilityRoleBindings::admit(
        &operation.requirements(),
        vec![CapabilityRoleBinding::new(
            role("target"),
            TARGET,
            vec![capability(STANDARD_DAMAGE_CAPABILITY)],
        )
        .unwrap()],
    )
    .unwrap();
    let plan = operation
        .plan(
            &bindings,
            &ExactInputBundle::empty(),
            &source,
            &catalog,
            &context(),
        )
        .unwrap();
    assert!(plan
        .observed_revisions()
        .iter()
        .any(|observed| observed.entity() == TARGET
            && observed.component()
                == gameplay_mechanics::MechanicsComponentKind::IntrinsicSources));

    let expected = source
        .component_revision::<IntrinsicSourcesComponent>(TARGET)
        .unwrap();
    let replacement = IntrinsicSourcesComponent::new(
        CatalogVersion::parse("standard.v1").unwrap(),
        vec![IntrinsicSourceBinding::new(
            SourceInstanceId::parse("source_after_plan").unwrap(),
            SourceDefinitionId::parse("ward_source").unwrap(),
        )],
    )
    .unwrap();
    EntityAuthoringService
        .replace_component(&mut source, expected, TARGET, replacement)
        .unwrap();
    assert!(matches!(
        plan.validate_source_state(&source, &catalog),
        Err(gameplay_standard::StandardPlanValidationError::StaleComponentRevision {
            ref expected,
            ..
        }) if expected.entity() == TARGET
            && expected.component() == gameplay_mechanics::MechanicsComponentKind::IntrinsicSources
    ));
}

#[test]
fn conservative_slot_guards_reject_absent_to_present_and_present_to_absent_changes() {
    let catalog = catalog();
    let operation = StandardOperation::SpendTrack {
        role: role("caster"),
        track: track("vitality"),
        amount: ExactExpr::Literal(scalar(1)).into(),
    };
    let bindings = CapabilityRoleBindings::admit(
        &operation.requirements(),
        vec![CapabilityRoleBinding::new(
            role("caster"),
            CASTER,
            vec![capability(STANDARD_TRACK_CAPABILITY)],
        )
        .unwrap()],
    )
    .unwrap();

    let mut absent_to_present = state();
    let absent_plan = operation
        .plan(
            &bindings,
            &ExactInputBundle::empty(),
            &absent_to_present,
            &catalog,
            &context(),
        )
        .unwrap();
    attach(
        &mut absent_to_present,
        CASTER,
        StatsComponent::new(CatalogVersion::parse("standard.v1").unwrap(), vec![]).unwrap(),
    );
    assert!(matches!(
        absent_plan.validate_source_state(&absent_to_present, &catalog),
        Err(gameplay_standard::StandardPlanValidationError::StaleComponentRevision {
            ref expected,
            ..
        }) if expected.component() == gameplay_mechanics::MechanicsComponentKind::Stats
    ));

    let mut present_to_absent = state();
    let present_plan = operation
        .plan(
            &bindings,
            &ExactInputBundle::empty(),
            &present_to_absent,
            &catalog,
            &context(),
        )
        .unwrap();
    let effects_revision = present_to_absent
        .component_revision::<ActiveEffectsComponent>(CASTER)
        .unwrap();
    EntityAuthoringService
        .detach_component::<ActiveEffectsComponent>(
            &mut present_to_absent,
            effects_revision,
            CASTER,
        )
        .unwrap();
    assert!(matches!(
        present_plan.validate_source_state(&present_to_absent, &catalog),
        Err(gameplay_standard::StandardPlanValidationError::StaleComponentRevision {
            ref expected,
            ..
        }) if expected.component() == gameplay_mechanics::MechanicsComponentKind::ActiveEffects
    ));
}

#[test]
fn admitted_operands_require_their_canonical_role_capabilities() {
    let subject = RuleSubjectId::parse("admitted_amount").unwrap();
    let source = RuleSourceId::parse("rules").unwrap();
    let package_context = gameplay_standard::StandardPackageContext::new(
        RulePackageSchemaVersion::IntegerOnlyV1,
        RuleDomainId::parse("game").unwrap(),
        RulePackageId::parse("standard").unwrap(),
        RuleVersion::new(1).unwrap(),
        vec![],
        vec![RuleSource::new(source.clone(), "rules.json").unwrap()],
        vec![RuleProvenance::new(subject.clone(), source.clone(), None, None).unwrap()],
    );
    let input = ExactInputReference::Parameter {
        role: role("target"),
        id: InputId::parse("amount").unwrap(),
    };
    let admitted = gameplay_standard::admit_exact_definition(
        &package_context,
        gameplay_standard::ExactDefinition::new(
            subject,
            source,
            ExactExpr::Input(input.clone()),
            vec![
                gameplay_standard::RoleRequirement::new(
                    role("caster"),
                    vec![capability("rule.magic")],
                )
                .unwrap(),
                // The referenced input role has no declared capability. Before this regression,
                // merely finding this bound role allowed the unrelated caster requirement to vanish.
                gameplay_standard::RoleRequirement::new(role("target"), vec![]).unwrap(),
            ],
        )
        .unwrap(),
    )
    .unwrap();
    let operation = StandardOperation::SpendTrack {
        role: role("caster"),
        track: track("vitality"),
        amount: gameplay_standard::StandardExactOperand::from_admitted(&admitted),
    };
    let bindings = CapabilityRoleBindings::admit(
        &operation.requirements(),
        vec![
            CapabilityRoleBinding::new(
                role("caster"),
                CASTER,
                vec![capability(STANDARD_TRACK_CAPABILITY)],
            )
            .unwrap(),
            CapabilityRoleBinding::new(role("target"), TARGET, vec![]).unwrap(),
        ],
    )
    .unwrap();
    assert!(matches!(
        operation.plan(
            &bindings,
            &ExactInputBundle::new(vec![(input, scalar(1))])
                .expect("single input evidence is valid"),
            &state(),
            &catalog(),
            &context(),
        ),
        Err(StandardPlanningError::Roles(
            gameplay_standard::StandardRoleBindingsError::MissingCapability { .. }
        ))
    ));
}

#[test]
fn request_source_must_match_the_context_operation() {
    let context = operation("context");
    let claimed = operation("claimed");
    assert!(StandardOperationContext::new(
        context,
        SourceInstanceIdentity::Request {
            operation: claimed,
            instance: SourceInstanceId::parse("leaf").unwrap()
        }
    )
    .is_err());
}

#[test]
fn fungible_inventory_leaves_are_typed_candidate_operations() {
    let grant = StandardOperation::GrantStack {
        role: role("to"),
        item: item("cells"),
        quantity: 2,
    };
    let consume = StandardOperation::ConsumeStack {
        role: role("from"),
        item: item("cells"),
        quantity: 2,
    };
    let transfer = StandardOperation::TransferStack {
        from: role("from"),
        to: role("to"),
        item: item("cells"),
        quantity: 1,
    };
    assert!(matches!(
        CapabilityRoleBindings::admit(&grant.requirements(), vec![]),
        Err(gameplay_standard::StandardRoleAdmissionError::MissingRole { .. })
    ));
    assert!(matches!(
        CapabilityRoleBindings::admit(
            &grant.requirements(),
            vec![CapabilityRoleBinding::new(role("to"), TARGET, vec![]).unwrap()],
        ),
        Err(gameplay_standard::StandardRoleAdmissionError::MissingCapability { .. })
    ));
    for operation in [&grant, &consume, &transfer] {
        assert!(operation
            .requirements()
            .iter()
            .all(
                |requirement| requirement.capabilities().contains(&capability(
                    gameplay_standard::STANDARD_INVENTORY_CAPABILITY
                ))
            ));
    }

    let source = inventory_state(3, 0);
    let catalog = inventory_catalog();
    let bindings = inventory_bindings(&transfer);
    let grant_plan = grant
        .plan(
            &bindings,
            &ExactInputBundle::empty(),
            &source,
            &catalog,
            &context(),
        )
        .unwrap();
    let consume_plan = consume
        .plan(
            &bindings,
            &ExactInputBundle::empty(),
            &source,
            &catalog,
            &context(),
        )
        .unwrap();
    let transfer_plan = transfer
        .plan(
            &bindings,
            &ExactInputBundle::empty(),
            &source,
            &catalog,
            &context(),
        )
        .unwrap();
    assert_eq!(
        grant_plan.observed_state_revision(),
        Some(source.revision())
    );
    assert!(grant_plan.observed_revisions().iter().any(|observed| {
        observed.entity() == TARGET
            && observed.component() == gameplay_mechanics::MechanicsComponentKind::Inventory
    }));

    let mut candidate = gameplay_mechanics::decode_snapshot_with_catalog(
        &entity_state::encode_snapshot(&source).unwrap(),
        &catalog,
    )
    .unwrap();
    assert!(matches!(
        grant_plan.effect().apply_to_candidate(&mut candidate, &catalog),
        Ok(gameplay_standard::StandardMechanicsReceipt::Inventory(receipt))
            if receipt.kind == gameplay_mechanics::InventoryMutationKind::Grant
    ));
    assert!(matches!(
        consume_plan.effect().apply_to_candidate(&mut candidate, &catalog),
        Ok(gameplay_standard::StandardMechanicsReceipt::Inventory(receipt))
            if receipt.kind == gameplay_mechanics::InventoryMutationKind::Consume
    ));
    assert!(matches!(
        transfer_plan.effect().apply_to_candidate(&mut candidate, &catalog),
        Ok(gameplay_standard::StandardMechanicsReceipt::InventoryTransfer(receipt))
            if receipt.from_owner == CASTER && receipt.to_owner == TARGET
    ));
    assert_eq!(stack_quantity(&candidate, &catalog, CASTER), 0);
    assert_eq!(stack_quantity(&candidate, &catalog, TARGET), 3);
    assert_eq!(stack_quantity(&source, &catalog, CASTER), 3);
}

#[test]
fn inventory_planning_rejects_invalid_authoring_and_guards_source_facts() {
    let source = inventory_state(3, 0);
    let catalog = inventory_catalog();
    let zero = StandardOperation::GrantStack {
        role: role("to"),
        item: item("cells"),
        quantity: 0,
    };
    assert!(matches!(
        zero.plan(
            &inventory_bindings(&zero),
            &ExactInputBundle::empty(),
            &source,
            &catalog,
            &context(),
        ),
        Err(StandardPlanningError::InventoryQuantity { .. })
    ));
    let above_maximum = StandardOperation::GrantStack {
        role: role("to"),
        item: item("cells"),
        quantity: 11,
    };
    assert!(matches!(
        above_maximum.plan(
            &inventory_bindings(&above_maximum),
            &ExactInputBundle::empty(),
            &source,
            &catalog,
            &context(),
        ),
        Err(StandardPlanningError::InventoryQuantity { .. })
    ));
    let unique = StandardOperation::GrantStack {
        role: role("to"),
        item: item("unique-key"),
        quantity: 1,
    };
    assert!(matches!(
        unique.plan(
            &inventory_bindings(&unique),
            &ExactInputBundle::empty(),
            &source,
            &catalog,
            &context(),
        ),
        Err(StandardPlanningError::InventoryItemKind { .. })
    ));
    let same_owner = StandardOperation::TransferStack {
        from: role("from"),
        to: role("from"),
        item: item("cells"),
        quantity: 1,
    };
    assert!(matches!(
        same_owner.plan(
            &inventory_bindings(&same_owner),
            &ExactInputBundle::empty(),
            &source,
            &catalog,
            &context(),
        ),
        Err(StandardPlanningError::InventoryOwnerConflict { .. })
    ));

    let operation = StandardOperation::ConsumeStack {
        role: role("from"),
        item: item("cells"),
        quantity: 1,
    };
    let plan = operation
        .plan(
            &inventory_bindings(&operation),
            &ExactInputBundle::empty(),
            &source,
            &catalog,
            &context(),
        )
        .unwrap();
    let mut changed = source;
    attach(
        &mut changed,
        CASTER,
        ItemComponent::new(
            CatalogVersion::parse("standard.v1").unwrap(),
            item("unique-key"),
        ),
    );
    assert!(matches!(
        plan.validate_source_state(&changed, &catalog),
        Err(gameplay_standard::StandardPlanValidationError::StaleStateRevision { .. })
    ));
}

#[test]
fn contained_item_slots_guard_only_inventory_capacity_plans() {
    let mut non_inventory_source = state();
    attach_contained_unique_item(&mut non_inventory_source);
    let catalog = catalog();
    let spend = StandardOperation::SpendTrack {
        role: role("caster"),
        track: track("vitality"),
        amount: ExactExpr::Literal(scalar(1)).into(),
    };
    let bindings = CapabilityRoleBindings::admit(
        &spend.requirements(),
        vec![CapabilityRoleBinding::new(
            role("caster"),
            CASTER,
            vec![capability(STANDARD_TRACK_CAPABILITY)],
        )
        .unwrap()],
    )
    .unwrap();
    let non_inventory_plan = spend
        .plan(
            &bindings,
            &ExactInputBundle::empty(),
            &non_inventory_source,
            &catalog,
            &context(),
        )
        .unwrap();
    assert!(!non_inventory_plan
        .observed_revisions()
        .iter()
        .any(|observed| {
            observed.entity() == CONTAINED_ITEM
                && observed.component() == gameplay_mechanics::MechanicsComponentKind::Item
        }));
    let item_revision = non_inventory_source
        .component_revision::<ItemComponent>(CONTAINED_ITEM)
        .unwrap();
    EntityAuthoringService
        .detach_component::<ItemComponent>(&mut non_inventory_source, item_revision, CONTAINED_ITEM)
        .unwrap();
    assert!(non_inventory_plan
        .validate_source_state(&non_inventory_source, &catalog)
        .is_ok());

    let mut inventory_source = inventory_state(3, 0);
    attach_contained_unique_item(&mut inventory_source);
    let inventory_catalog = inventory_catalog();
    let consume = StandardOperation::ConsumeStack {
        role: role("from"),
        item: item("cells"),
        quantity: 1,
    };
    let inventory_plan = consume
        .plan(
            &inventory_bindings(&consume),
            &ExactInputBundle::empty(),
            &inventory_source,
            &inventory_catalog,
            &context(),
        )
        .unwrap();
    assert!(inventory_plan.observed_revisions().iter().any(|observed| {
        observed.entity() == CONTAINED_ITEM
            && observed.component() == gameplay_mechanics::MechanicsComponentKind::Item
    }));
    let item_revision = inventory_source
        .component_revision::<ItemComponent>(CONTAINED_ITEM)
        .unwrap();
    EntityAuthoringService
        .detach_component::<ItemComponent>(&mut inventory_source, item_revision, CONTAINED_ITEM)
        .unwrap();
    assert!(matches!(
        inventory_plan.validate_source_state(&inventory_source, &inventory_catalog),
        Err(gameplay_standard::StandardPlanValidationError::StaleStateRevision { .. })
    ));
}

#[test]
fn inventory_candidate_reports_underflow_and_capacity_without_mutating_the_plan_source() {
    let underflow = StandardOperation::ConsumeStack {
        role: role("from"),
        item: item("cells"),
        quantity: 4,
    };
    let full = inventory_state(5, 0);
    let catalog = inventory_catalog();
    let underflow_plan = underflow
        .plan(
            &inventory_bindings(&underflow),
            &ExactInputBundle::empty(),
            &inventory_state(3, 0),
            &catalog,
            &context(),
        )
        .unwrap();
    let mut underflow_candidate = inventory_state(3, 0);
    assert!(matches!(
        underflow_plan
            .effect()
            .apply_to_candidate(&mut underflow_candidate, &catalog),
        Err(gameplay_mechanics::MechanicsError::InventoryInsufficientQuantity { .. })
    ));
    let grant = StandardOperation::GrantStack {
        role: role("from"),
        item: item("cells"),
        quantity: 6,
    };
    let full_plan = grant
        .plan(
            &inventory_bindings(&grant),
            &ExactInputBundle::empty(),
            &full,
            &catalog,
            &context(),
        )
        .unwrap();
    let mut full_candidate = full;
    assert!(matches!(
        full_plan
            .effect()
            .apply_to_candidate(&mut full_candidate, &catalog),
        Err(gameplay_mechanics::MechanicsError::InventoryQuantityLimitExceeded { .. })
    ));
    let capacity = StandardOperation::GrantStack {
        role: role("from"),
        item: item("cells"),
        quantity: 3,
    };
    let capacity_source = inventory_state(3, 0);
    let capacity_plan = capacity
        .plan(
            &inventory_bindings(&capacity),
            &ExactInputBundle::empty(),
            &capacity_source,
            &catalog,
            &context(),
        )
        .unwrap();
    let mut capacity_candidate = capacity_source;
    assert!(matches!(
        capacity_plan
            .effect()
            .apply_to_candidate(&mut capacity_candidate, &catalog),
        Err(gameplay_mechanics::MechanicsError::InventoryCapacityExceeded { .. })
    ));
}

#[test]
fn dagger_and_demo_shapes_remain_typed_and_product_meaning_stays_outside_standard() {
    let dagger_damage = StandardOperation::SubmitDamage {
        actor: Some(role("weapon-user")),
        target: role("armor-owner"),
        target_track: track("vitality"),
        parts: vec![(
            ExactExpr::Literal(scalar(4)).into(),
            DamageKindId::parse("impact").unwrap(),
        )],
        request_sources: vec![],
    };
    let demo = StandardOperation::ApplyEffect {
        role: role("target"),
        instance: gameplay_mechanics::EffectInstanceId::parse("ward_one").unwrap(),
        definition: EffectDefinitionId::parse("ward").unwrap(),
        stacks: 1,
    };
    let dagger_spend = StandardOperation::SpendTrack {
        role: role("weapon-user"),
        track: track("vitality"),
        amount: ExactExpr::Literal(scalar(1)).into(),
    };
    // Downstream policy selects this ordinary fungible loot result; standard neither allocates
    // an item entity nor learns Dagger's material/progression rules.
    let dagger_loot = StandardOperation::GrantStack {
        role: role("loot-recipient"),
        item: item("cells"),
        quantity: 2,
    };
    // A Demo pickup policy chooses participants and quantity before using the same closed leaf.
    let demo_pickup = StandardOperation::TransferStack {
        from: role("pickup-source"),
        to: role("player"),
        item: item("cells"),
        quantity: 1,
    };
    assert!(dagger_loot.requirements()[0]
        .capabilities()
        .contains(&capability(
            gameplay_standard::STANDARD_INVENTORY_CAPABILITY
        )));
    assert_eq!(demo_pickup.requirements().len(), 2);
    let dagger_program: Program<StandardPredicate, StandardOperation> = Program::Sequence {
        steps: vec![
            Program::Operation(dagger_spend),
            Program::Operation(dagger_damage.clone()),
        ],
    };
    assert!(matches!(dagger_program, Program::Sequence { ref steps } if steps.len() == 2));
    let dagger_requirements = dagger_damage.requirements();
    assert_eq!(dagger_requirements.len(), 2);
    assert!(dagger_requirements.iter().all(|requirement| requirement
        .capabilities()
        .contains(&capability(STANDARD_DAMAGE_CAPABILITY))));
    assert_eq!(
        demo.requirements()[0].capabilities(),
        &[capability(STANDARD_EFFECT_CAPABILITY)]
    );

    #[derive(Debug)]
    enum DemoExtension {
        TriggerClassicAlarm,
    }
    let demo_program: Program<ComposedPredicate<()>, ComposedOperation<DemoExtension>> =
        Program::When {
            predicate: ComposedPredicate::Standard(StandardPredicate::Exact(
                ExactComparison::GreaterThan(
                    ExactExpr::Literal(scalar(1)),
                    ExactExpr::Literal(scalar(0)),
                ),
            )),
            then_program: Box::new(Program::Sequence {
                steps: vec![
                    Program::Operation(ComposedOperation::Standard(dagger_damage)),
                    Program::Operation(ComposedOperation::Standard(demo)),
                    Program::Operation(ComposedOperation::Product(
                        DemoExtension::TriggerClassicAlarm,
                    )),
                ],
            }),
            otherwise_program: None,
        };
    assert!(matches!(demo_program, Program::When { .. }));
}

#[derive(Clone)]
struct ProductIntent {
    late_failure: bool,
}

struct ProductPolicy<'a> {
    planning_state: &'a EntityState,
    catalog: &'a MechanicsCatalog,
    bindings: CapabilityRoleBindings,
    context: StandardOperationContext,
}

impl ResolutionPolicy for ProductPolicy<'_> {
    type RawIntent = ProductIntent;
    type Intent = ProductIntent;
    type Facts = ExactInputBundle;
    type Predicate = StandardPredicate;
    type Operation = StandardOperation;
    type Effect = gameplay_standard::StandardOperationPlan;
    type Event = ();
    type Evidence = ();
    type Interceptor = ();
    type TraceDetail = ();
    type Rejection = &'static str;
    type Fault = gameplay_standard::StandardPlanningError;
    type Suspension = ();

    fn admit(
        &mut self,
        intent: &Self::RawIntent,
        _: &[()],
        _: &mut dyn ResolutionTraceSink<()>,
    ) -> PolicyResult<Self::Intent, Self::Rejection, Self::Fault, Self::Suspension> {
        Ok(intent.clone())
    }
    fn gather(
        &mut self,
        _: &Self::Intent,
        _: &[()],
        _: &mut dyn ResolutionTraceSink<()>,
    ) -> PolicyResult<Self::Facts, Self::Rejection, Self::Fault, Self::Suspension> {
        Ok(ExactInputBundle::empty())
    }
    fn check(
        &mut self,
        _: &Self::Intent,
        _: &Self::Facts,
        _: &[()],
        _: &mut dyn ResolutionTraceSink<()>,
    ) -> PolicyResult<(), Self::Rejection, Self::Fault, Self::Suspension> {
        Ok(())
    }
    fn plan(
        &mut self,
        intent: &Self::Intent,
        _: &Self::Facts,
        _: &[()],
        _: &mut dyn ResolutionTraceSink<()>,
    ) -> PolicyResult<
        Program<Self::Predicate, Self::Operation>,
        Self::Rejection,
        Self::Fault,
        Self::Suspension,
    > {
        let spend = |amount| StandardOperation::SpendTrack {
            role: role("caster"),
            track: track("vitality"),
            amount: ExactExpr::Literal(scalar(amount)).into(),
        };
        Ok(Program::Sequence {
            steps: if intent.late_failure {
                vec![Program::Operation(spend(3)), Program::Operation(spend(50))]
            } else {
                vec![Program::Operation(spend(3))]
            },
        })
    }
    fn evaluate_predicate(
        &mut self,
        predicate: &Self::Predicate,
        _: &Self::Intent,
        facts: &Self::Facts,
        _: &[()],
        _: &mut dyn ResolutionTraceSink<()>,
    ) -> PolicyResult<bool, Self::Rejection, Self::Fault, Self::Suspension> {
        predicate
            .evaluate(facts)
            .map_err(|_| gameplay_resolution::PolicyFailure::Rejected("predicate failed"))
    }
    fn plan_operation(
        &mut self,
        operation: &Self::Operation,
        _: &Self::Intent,
        facts: &Self::Facts,
        _: &[()],
        _: &mut dyn ResolutionTraceSink<()>,
    ) -> PolicyResult<
        ResolutionPlan<Self::Effect, Self::Event, Self::RawIntent, Self::Evidence>,
        Self::Rejection,
        Self::Fault,
        Self::Suspension,
    > {
        let mut plan = ResolutionPlan::new();
        plan.push_effect(
            operation
                .plan(
                    &self.bindings,
                    facts,
                    self.planning_state,
                    self.catalog,
                    &self.context,
                )
                .map_err(gameplay_resolution::PolicyFailure::Fault)?,
        );
        Ok(plan)
    }
}

struct ProductSession {
    authority: EntityState,
    catalog: MechanicsCatalog,
    candidate: Option<EntityState>,
    expected_revision: u64,
    revision: u64,
    stale: bool,
    commits: usize,
    mechanics_receipts: Vec<gameplay_standard::StandardMechanicsReceipt>,
}

#[allow(dead_code)]
#[derive(Debug)]
enum ProductSessionError {
    Snapshot(entity_state::EntityStateSnapshotError),
    MechanicsSnapshot(gameplay_mechanics::MechanicsSnapshotError),
    SourceValidation(gameplay_standard::StandardPlanValidationError),
    Mechanics(gameplay_mechanics::MechanicsError),
    StaleProductRevision,
    MissingCandidate,
}

impl ProductSession {
    fn new(authority: EntityState, catalog: MechanicsCatalog) -> Self {
        let revision = authority.revision();
        Self {
            authority,
            catalog,
            candidate: None,
            expected_revision: revision,
            revision,
            stale: false,
            commits: 0,
            mechanics_receipts: Vec::new(),
        }
    }
}

impl ResolutionTransaction for ProductSession {
    type Effect = gameplay_standard::StandardOperationPlan;
    type Error = ProductSessionError;

    fn stage(&mut self, effect: &Self::Effect) -> Result<(), Self::Error> {
        effect
            .validate_source_state(&self.authority, &self.catalog)
            .map_err(ProductSessionError::SourceValidation)?;
        if self.candidate.is_none() {
            self.candidate = Some(
                gameplay_mechanics::decode_snapshot_with_catalog(
                    &entity_state::encode_snapshot(&self.authority)
                        .map_err(ProductSessionError::Snapshot)?,
                    &self.catalog,
                )
                .map_err(ProductSessionError::MechanicsSnapshot)?,
            );
        }
        let receipt = effect
            .effect()
            .apply_to_candidate(self.candidate.as_mut().unwrap(), &self.catalog)
            .map_err(ProductSessionError::Mechanics)?;
        self.mechanics_receipts.push(receipt);
        Ok(())
    }
    fn commit(&mut self) -> Result<(), Self::Error> {
        if self.stale || self.revision != self.expected_revision {
            return Err(ProductSessionError::StaleProductRevision);
        }
        self.authority = self
            .candidate
            .take()
            .ok_or(ProductSessionError::MissingCandidate)?;
        self.revision += 1;
        self.commits += 1;
        Ok(())
    }
    fn abort(&mut self) {
        self.candidate = None;
        self.mechanics_receipts.clear();
    }
}

#[test]
fn inventory_product_session_rebases_sequential_candidates_and_publishes_once() {
    let source = inventory_state(3, 0);
    let catalog = inventory_catalog();
    let grant = StandardOperation::GrantStack {
        role: role("to"),
        item: item("cells"),
        quantity: 2,
    };
    let transfer = StandardOperation::TransferStack {
        from: role("from"),
        to: role("to"),
        item: item("cells"),
        quantity: 1,
    };
    let bindings = inventory_bindings(&transfer);
    let grant_plan = grant
        .plan(
            &bindings,
            &ExactInputBundle::empty(),
            &source,
            &catalog,
            &context(),
        )
        .unwrap();
    let transfer_plan = transfer
        .plan(
            &bindings,
            &ExactInputBundle::empty(),
            &source,
            &catalog,
            &context(),
        )
        .unwrap();
    let mut session = ProductSession::new(source, catalog);
    session.stage(&grant_plan).unwrap();
    session.stage(&transfer_plan).unwrap();
    session.commit().unwrap();
    assert_eq!(session.commits, 1);
    assert_eq!(
        stack_quantity(&session.authority, &session.catalog, CASTER),
        2
    );
    assert_eq!(
        stack_quantity(&session.authority, &session.catalog, TARGET),
        3
    );
    assert!(matches!(
        session.mechanics_receipts.as_slice(),
        [
            gameplay_standard::StandardMechanicsReceipt::Inventory(_),
            gameplay_standard::StandardMechanicsReceipt::InventoryTransfer(_)
        ]
    ));

    let source = inventory_state(3, 0);
    let catalog = inventory_catalog();
    let late_consume = StandardOperation::ConsumeStack {
        role: role("from"),
        item: item("cells"),
        quantity: 4,
    };
    let grant_plan = grant
        .plan(
            &bindings,
            &ExactInputBundle::empty(),
            &source,
            &catalog,
            &context(),
        )
        .unwrap();
    let late_plan = late_consume
        .plan(
            &inventory_bindings(&late_consume),
            &ExactInputBundle::empty(),
            &source,
            &catalog,
            &context(),
        )
        .unwrap();
    let mut failed = ProductSession::new(source, catalog);
    failed.stage(&grant_plan).unwrap();
    assert!(matches!(
        failed.stage(&late_plan),
        Err(ProductSessionError::Mechanics(
            gameplay_mechanics::MechanicsError::InventoryInsufficientQuantity { .. }
        ))
    ));
    failed.abort();
    assert_eq!(failed.commits, 0);
    assert_eq!(
        stack_quantity(&failed.authority, &failed.catalog, CASTER),
        3
    );
    assert_eq!(
        stack_quantity(&failed.authority, &failed.catalog, TARGET),
        0
    );
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DemoPredicate {
    FeatureEnabled(bool),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DemoExtension {
    RecordClassicAlarm,
}

#[derive(Debug, Clone)]
enum ExecutableEffect {
    Standard(Box<gameplay_standard::StandardOperationPlan>),
    Product(DemoExtension),
}

struct ExecutablePolicy<'a> {
    planning_state: &'a EntityState,
    catalog: &'a MechanicsCatalog,
    bindings: CapabilityRoleBindings,
    context: StandardOperationContext,
    program: Program<ComposedPredicate<DemoPredicate>, ComposedOperation<DemoExtension>>,
}

impl ResolutionPolicy for ExecutablePolicy<'_> {
    type RawIntent = ();
    type Intent = ();
    type Facts = ExactInputBundle;
    type Predicate = ComposedPredicate<DemoPredicate>;
    type Operation = ComposedOperation<DemoExtension>;
    type Effect = ExecutableEffect;
    type Event = ();
    type Evidence = ();
    type Interceptor = ();
    type TraceDetail = ();
    type Rejection = &'static str;
    type Fault = StandardPlanningError;
    type Suspension = ();

    fn admit(
        &mut self,
        _: &Self::RawIntent,
        _: &[()],
        _: &mut dyn ResolutionTraceSink<()>,
    ) -> PolicyResult<Self::Intent, Self::Rejection, Self::Fault, Self::Suspension> {
        Ok(())
    }
    fn gather(
        &mut self,
        _: &Self::Intent,
        _: &[()],
        _: &mut dyn ResolutionTraceSink<()>,
    ) -> PolicyResult<Self::Facts, Self::Rejection, Self::Fault, Self::Suspension> {
        Ok(ExactInputBundle::empty())
    }
    fn check(
        &mut self,
        _: &Self::Intent,
        _: &Self::Facts,
        _: &[()],
        _: &mut dyn ResolutionTraceSink<()>,
    ) -> PolicyResult<(), Self::Rejection, Self::Fault, Self::Suspension> {
        Ok(())
    }
    fn plan(
        &mut self,
        _: &Self::Intent,
        _: &Self::Facts,
        _: &[()],
        _: &mut dyn ResolutionTraceSink<()>,
    ) -> PolicyResult<
        Program<Self::Predicate, Self::Operation>,
        Self::Rejection,
        Self::Fault,
        Self::Suspension,
    > {
        Ok(self.program.clone())
    }
    fn evaluate_predicate(
        &mut self,
        predicate: &Self::Predicate,
        _: &Self::Intent,
        facts: &Self::Facts,
        _: &[()],
        _: &mut dyn ResolutionTraceSink<()>,
    ) -> PolicyResult<bool, Self::Rejection, Self::Fault, Self::Suspension> {
        match predicate {
            ComposedPredicate::Standard(predicate) => predicate
                .evaluate(facts)
                .map_err(|_| PolicyFailure::Rejected("standard predicate failed")),
            ComposedPredicate::Product(DemoPredicate::FeatureEnabled(value)) => Ok(*value),
        }
    }
    fn plan_operation(
        &mut self,
        operation: &Self::Operation,
        _: &Self::Intent,
        facts: &Self::Facts,
        _: &[()],
        _: &mut dyn ResolutionTraceSink<()>,
    ) -> PolicyResult<
        ResolutionPlan<Self::Effect, Self::Event, Self::RawIntent, Self::Evidence>,
        Self::Rejection,
        Self::Fault,
        Self::Suspension,
    > {
        let mut plan = ResolutionPlan::new();
        match operation {
            ComposedOperation::Standard(operation) => plan.push_effect(
                operation
                    .plan(
                        &self.bindings,
                        facts,
                        self.planning_state,
                        self.catalog,
                        &self.context,
                    )
                    .map(Box::new)
                    .map(ExecutableEffect::Standard)
                    .map_err(PolicyFailure::Fault)?,
            ),
            ComposedOperation::Product(operation) => {
                plan.push_effect(ExecutableEffect::Product(operation.clone()));
            }
        }
        Ok(plan)
    }
}

struct ExecutableSession {
    mechanics: ProductSession,
    product_operations: Vec<DemoExtension>,
}

impl ExecutableSession {
    fn new(authority: EntityState, catalog: MechanicsCatalog) -> Self {
        Self {
            mechanics: ProductSession::new(authority, catalog),
            product_operations: Vec::new(),
        }
    }
}

impl ResolutionTransaction for ExecutableSession {
    type Effect = ExecutableEffect;
    type Error = ProductSessionError;

    fn stage(&mut self, effect: &Self::Effect) -> Result<(), Self::Error> {
        match effect {
            ExecutableEffect::Standard(effect) => self.mechanics.stage(effect),
            ExecutableEffect::Product(operation) => {
                self.product_operations.push(operation.clone());
                Ok(())
            }
        }
    }

    fn commit(&mut self) -> Result<(), Self::Error> {
        self.mechanics.commit()
    }

    fn abort(&mut self) {
        self.mechanics.abort();
        self.product_operations.clear();
    }
}

fn executable_bindings() -> CapabilityRoleBindings {
    CapabilityRoleBindings::admit(
        &[],
        vec![
            CapabilityRoleBinding::new(
                role("caster"),
                CASTER,
                vec![
                    capability(STANDARD_TRACK_CAPABILITY),
                    capability(STANDARD_DAMAGE_CAPABILITY),
                ],
            )
            .unwrap(),
            CapabilityRoleBinding::new(
                role("target"),
                TARGET,
                vec![
                    capability(STANDARD_DAMAGE_CAPABILITY),
                    capability(STANDARD_EFFECT_CAPABILITY),
                ],
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

#[test]
fn resolver_executes_dagger_and_conditional_demo_programs_with_one_product_publication() {
    let catalog = catalog();
    let resolver = StandardResolver::default();
    let dagger = Program::Sequence {
        steps: vec![
            Program::Operation(ComposedOperation::Standard(StandardOperation::SpendTrack {
                role: role("caster"),
                track: track("vitality"),
                amount: ExactExpr::Literal(scalar(2)).into(),
            })),
            Program::Operation(ComposedOperation::Standard(
                StandardOperation::SubmitDamage {
                    actor: Some(role("caster")),
                    target: role("target"),
                    target_track: track("vitality"),
                    parts: vec![(
                        ExactExpr::Literal(scalar(3)).into(),
                        DamageKindId::parse("impact").unwrap(),
                    )],
                    request_sources: vec![],
                },
            )),
        ],
    };
    let planning_state = state();
    let mut dagger_policy = ExecutablePolicy {
        planning_state: &planning_state,
        catalog: &catalog,
        bindings: executable_bindings(),
        context: context(),
        program: dagger,
    };
    let mut dagger_session = ExecutableSession::new(state(), catalog.clone());
    let dagger_receipt = resolver.resolve(
        &mut dagger_policy,
        &mut dagger_session,
        ResolutionRequest::new(
            ResolutionIdentity::root(
                ResolutionId::new(30).unwrap(),
                CorrelationId::new(40).unwrap(),
            ),
            ResolutionMode::Apply,
            (),
            vec![],
        ),
    );
    assert!(dagger_receipt.succeeded());
    assert_eq!(dagger_session.mechanics.commits, 1);
    assert!(matches!(
        dagger_session.mechanics.mechanics_receipts.as_slice(),
        [
            gameplay_standard::StandardMechanicsReceipt::Track(_),
            gameplay_standard::StandardMechanicsReceipt::Damage(_)
        ]
    ));
    assert_eq!(
        dagger_session
            .mechanics
            .authority
            .component::<TracksComponent>(TARGET)
            .unwrap()
            .unwrap()
            .current(&track("vitality")),
        Some(scalar(7))
    );

    for (enabled, expected_receipt) in [(true, "effect"), (false, "damage")] {
        let demo_program = Program::When {
            predicate: ComposedPredicate::Product(DemoPredicate::FeatureEnabled(enabled)),
            then_program: Box::new(Program::Sequence {
                steps: vec![
                    Program::Operation(ComposedOperation::Standard(
                        StandardOperation::ApplyEffect {
                            role: role("target"),
                            instance: gameplay_mechanics::EffectInstanceId::parse("demo_ward")
                                .unwrap(),
                            definition: EffectDefinitionId::parse("ward").unwrap(),
                            stacks: 1,
                        },
                    )),
                    Program::Operation(ComposedOperation::Product(
                        DemoExtension::RecordClassicAlarm,
                    )),
                ],
            }),
            otherwise_program: Some(Box::new(Program::Sequence {
                steps: vec![
                    Program::Operation(ComposedOperation::Standard(
                        StandardOperation::SubmitDamage {
                            actor: None,
                            target: role("target"),
                            target_track: track("vitality"),
                            parts: vec![(
                                ExactExpr::Literal(scalar(1)).into(),
                                DamageKindId::parse("impact").unwrap(),
                            )],
                            request_sources: vec![],
                        },
                    )),
                    Program::Operation(ComposedOperation::Product(
                        DemoExtension::RecordClassicAlarm,
                    )),
                ],
            })),
        };
        let mut demo_policy = ExecutablePolicy {
            planning_state: &planning_state,
            catalog: &catalog,
            bindings: executable_bindings(),
            context: context(),
            program: demo_program,
        };
        let mut demo_session = ExecutableSession::new(state(), catalog.clone());
        let receipt = resolver.resolve(
            &mut demo_policy,
            &mut demo_session,
            ResolutionRequest::new(
                ResolutionIdentity::root(
                    ResolutionId::new(if enabled { 31 } else { 32 }).unwrap(),
                    CorrelationId::new(41).unwrap(),
                ),
                ResolutionMode::Apply,
                (),
                vec![],
            ),
        );
        assert!(receipt.succeeded());
        assert_eq!(demo_session.mechanics.commits, 1);
        assert_eq!(
            demo_session.product_operations,
            vec![DemoExtension::RecordClassicAlarm]
        );
        match (
            expected_receipt,
            demo_session.mechanics.mechanics_receipts.as_slice(),
        ) {
            ("effect", [gameplay_standard::StandardMechanicsReceipt::Effect(_)])
            | ("damage", [gameplay_standard::StandardMechanicsReceipt::Damage(_)]) => {}
            (_, receipts) => panic!("unexpected demo mechanics receipts: {receipts:?}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutcomeMode {
    Rejected,
    Faulted,
    Suspended,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutcomeRejection {
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutcomeFault {
    Invalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutcomeSuspension {
    AwaitingInput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutcomeMarker {
    Empty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutcomeTransactionError {
    UnexpectedEffect,
}

#[derive(Default)]
struct OutcomePolicy;

impl ResolutionPolicy for OutcomePolicy {
    type RawIntent = OutcomeMode;
    type Intent = OutcomeMode;
    type Facts = ExactInputBundle;
    type Predicate = StandardPredicate;
    type Operation = StandardOperation;
    type Effect = gameplay_standard::StandardOperationPlan;
    type Event = OutcomeMarker;
    type Evidence = OutcomeMarker;
    type Interceptor = OutcomeMarker;
    type TraceDetail = OutcomeMarker;
    type Rejection = OutcomeRejection;
    type Fault = OutcomeFault;
    type Suspension = OutcomeSuspension;

    fn admit(
        &mut self,
        intent: &Self::RawIntent,
        _: &[Self::Evidence],
        _: &mut dyn ResolutionTraceSink<Self::TraceDetail>,
    ) -> PolicyResult<Self::Intent, Self::Rejection, Self::Fault, Self::Suspension> {
        match intent {
            OutcomeMode::Rejected => Err(PolicyFailure::Rejected(OutcomeRejection::Blocked)),
            OutcomeMode::Faulted => Err(PolicyFailure::Fault(OutcomeFault::Invalid)),
            OutcomeMode::Suspended => {
                Err(PolicyFailure::Suspended(OutcomeSuspension::AwaitingInput))
            }
        }
    }

    fn gather(
        &mut self,
        _: &Self::Intent,
        _: &[Self::Evidence],
        _: &mut dyn ResolutionTraceSink<Self::TraceDetail>,
    ) -> PolicyResult<Self::Facts, Self::Rejection, Self::Fault, Self::Suspension> {
        Ok(ExactInputBundle::empty())
    }

    fn check(
        &mut self,
        _: &Self::Intent,
        _: &Self::Facts,
        _: &[Self::Evidence],
        _: &mut dyn ResolutionTraceSink<Self::TraceDetail>,
    ) -> PolicyResult<(), Self::Rejection, Self::Fault, Self::Suspension> {
        Ok(())
    }

    fn plan(
        &mut self,
        _: &Self::Intent,
        _: &Self::Facts,
        _: &[Self::Evidence],
        _: &mut dyn ResolutionTraceSink<Self::TraceDetail>,
    ) -> PolicyResult<
        Program<Self::Predicate, Self::Operation>,
        Self::Rejection,
        Self::Fault,
        Self::Suspension,
    > {
        Ok(Program::Sequence { steps: vec![] })
    }

    fn evaluate_predicate(
        &mut self,
        _: &Self::Predicate,
        _: &Self::Intent,
        _: &Self::Facts,
        _: &[Self::Evidence],
        _: &mut dyn ResolutionTraceSink<Self::TraceDetail>,
    ) -> PolicyResult<bool, Self::Rejection, Self::Fault, Self::Suspension> {
        Ok(true)
    }

    fn plan_operation(
        &mut self,
        _: &Self::Operation,
        _: &Self::Intent,
        _: &Self::Facts,
        _: &[Self::Evidence],
        _: &mut dyn ResolutionTraceSink<Self::TraceDetail>,
    ) -> PolicyResult<
        ResolutionPlan<Self::Effect, Self::Event, Self::RawIntent, Self::Evidence>,
        Self::Rejection,
        Self::Fault,
        Self::Suspension,
    > {
        let mut plan = ResolutionPlan::new();
        plan.push_event(OutcomeMarker::Empty);
        Ok(plan)
    }
}

#[derive(Default)]
struct OutcomeTransaction;

impl ResolutionTransaction for OutcomeTransaction {
    type Effect = gameplay_standard::StandardOperationPlan;
    type Error = OutcomeTransactionError;

    fn stage(&mut self, _: &Self::Effect) -> Result<(), Self::Error> {
        Err(OutcomeTransactionError::UnexpectedEffect)
    }

    fn commit(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn abort(&mut self) {}
}

fn outcome_request(
    mode: OutcomeMode,
    resolution: u64,
    correlation: u64,
) -> ResolutionRequest<OutcomeMode, OutcomeMarker> {
    ResolutionRequest::new(
        ResolutionIdentity::root(
            ResolutionId::new(resolution).unwrap(),
            CorrelationId::new(correlation).unwrap(),
        ),
        ResolutionMode::Apply,
        mode,
        vec![],
    )
}

fn resolution_request(
    mode: ResolutionMode,
    late_failure: bool,
) -> ResolutionRequest<ProductIntent, ()> {
    ResolutionRequest::new(
        ResolutionIdentity::root(
            ResolutionId::new(1).unwrap(),
            CorrelationId::new(2).unwrap(),
        ),
        mode,
        ProductIntent { late_failure },
        vec![],
    )
}

#[test]
fn product_session_proves_preview_apply_stale_and_late_failure_are_fail_atomic() {
    let catalog = catalog();
    let planning_state = state();
    let bindings = CapabilityRoleBindings::admit(
        &StandardOperation::SpendTrack {
            role: role("caster"),
            track: track("vitality"),
            amount: ExactExpr::Literal(scalar(3)).into(),
        }
        .requirements(),
        vec![CapabilityRoleBinding::new(
            role("caster"),
            CASTER,
            vec![capability(STANDARD_TRACK_CAPABILITY)],
        )
        .unwrap()],
    )
    .unwrap();
    let resolver = StandardResolver::default();
    let mut preview_session = ProductSession::new(state(), catalog.clone());
    let mut preview_policy = ProductPolicy {
        planning_state: &planning_state,
        catalog: &catalog,
        bindings: bindings.clone(),
        context: context(),
    };
    let preview = resolver.resolve(
        &mut preview_policy,
        &mut preview_session,
        resolution_request(ResolutionMode::Preview, false),
    );
    assert!(preview.succeeded());
    assert_eq!(preview_session.commits, 0);
    assert_eq!(
        preview_session
            .authority
            .component::<TracksComponent>(CASTER)
            .unwrap()
            .unwrap()
            .current(&track("vitality")),
        Some(scalar(10))
    );

    let mut applied_session = ProductSession::new(state(), catalog.clone());
    let mut applied_policy = ProductPolicy {
        planning_state: &planning_state,
        catalog: &catalog,
        bindings: bindings.clone(),
        context: context(),
    };
    let applied = resolver.resolve(
        &mut applied_policy,
        &mut applied_session,
        resolution_request(ResolutionMode::Apply, false),
    );
    assert!(applied.succeeded());
    assert_eq!(applied.effects().len(), preview.effects().len());
    assert_eq!(applied_session.commits, 1);
    assert!(matches!(
        applied_session.mechanics_receipts.as_slice(),
        [gameplay_standard::StandardMechanicsReceipt::Track(receipt)]
            if receipt.operation == operation("standard_attempt")
                && receipt.source == *context().source()
    ));
    assert_eq!(
        applied_session
            .authority
            .component::<TracksComponent>(CASTER)
            .unwrap()
            .unwrap()
            .current(&track("vitality")),
        Some(scalar(7))
    );

    let mut failed_session = ProductSession::new(state(), catalog.clone());
    let mut failed_policy = ProductPolicy {
        planning_state: &planning_state,
        catalog: &catalog,
        bindings: bindings.clone(),
        context: context(),
    };
    let failed = resolver.resolve(
        &mut failed_policy,
        &mut failed_session,
        resolution_request(ResolutionMode::Apply, true),
    );
    assert!(!failed.succeeded());
    assert_eq!(failed_session.commits, 0);
    assert!(failed_session.mechanics_receipts.is_empty());
    assert_eq!(
        failed_session
            .authority
            .component::<TracksComponent>(CASTER)
            .unwrap()
            .unwrap()
            .current(&track("vitality")),
        Some(scalar(10))
    );

    let mut stale_session = ProductSession::new(state(), catalog.clone());
    stale_session.stale = true;
    let mut stale_policy = ProductPolicy {
        planning_state: &planning_state,
        catalog: &catalog,
        bindings,
        context: context(),
    };
    let stale = resolver.resolve(
        &mut stale_policy,
        &mut stale_session,
        resolution_request(ResolutionMode::Apply, false),
    );
    assert!(!stale.succeeded());
    assert!(stale_session.mechanics_receipts.is_empty());
    assert!(matches!(
        stale.commit(),
        CommitStatus::Failed(ProductSessionError::StaleProductRevision)
    ));
    assert_eq!(
        stale_session
            .authority
            .component::<TracksComponent>(CASTER)
            .unwrap()
            .unwrap()
            .current(&track("vitality")),
        Some(scalar(10))
    );
}

#[test]
fn standard_resolver_preserves_typed_policy_outcomes_and_request_identity() {
    let resolver = StandardResolver::default();
    for (index, mode) in [
        OutcomeMode::Rejected,
        OutcomeMode::Faulted,
        OutcomeMode::Suspended,
    ]
    .into_iter()
    .enumerate()
    {
        let request = outcome_request(mode, index as u64 + 10, index as u64 + 20);
        let identity = request.identity();
        let mut policy = OutcomePolicy;
        let mut transaction = OutcomeTransaction;
        let receipt = resolver.resolve(&mut policy, &mut transaction, request);

        assert_eq!(receipt.attempt().identity(), identity);
        assert_eq!(
            receipt.attempt().identity().correlation(),
            identity.correlation()
        );
        assert_eq!(receipt.commit(), &CommitStatus::NotAttempted);
        match mode {
            OutcomeMode::Rejected => assert_eq!(
                receipt.attempt().status(),
                &AttemptStatus::Rejected(OutcomeRejection::Blocked)
            ),
            OutcomeMode::Faulted => assert_eq!(
                receipt.attempt().status(),
                &AttemptStatus::Faulted(OutcomeFault::Invalid)
            ),
            OutcomeMode::Suspended => assert_eq!(
                receipt.attempt().status(),
                &AttemptStatus::Suspended(OutcomeSuspension::AwaitingInput)
            ),
        }
    }
}
