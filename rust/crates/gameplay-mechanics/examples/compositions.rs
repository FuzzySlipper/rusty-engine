//! Three direct downstream compositions over the same mechanics services.
//!
//! Run with:
//!
//! `cargo run -p gameplay-mechanics --example compositions`

use core_ids::EntityId;
use entity_state::{
    encode_snapshot, EntityAuthoringService, EntityComponent, EntityDefinition, EntityState,
};
use gameplay_mechanics::{
    decode_snapshot_with_catalog, ActiveEffectsComponent, CatalogVersion, DamageKindDefinition,
    DamageKindId, DamageKindSelector, DamagePart, DamageRequest, DamageResponseDefinition,
    DamageService, EffectApplyRequest, EffectDefinition, EffectDefinitionId, EffectInstanceId,
    EffectService, EffectStackingPolicy, MechanicsCatalog, MechanicsCatalogDefinition,
    MechanicsScalar, OperationId, SourceDefinition, SourceDefinitionId, SourceInstanceId,
    SourceInstanceIdentity, StackingGroupId, StackingPolicy, StatContribution,
    StatContributionDefinition, StatDefinition, StatId, StatService, StatValue, StatsComponent,
    TrackAdjustmentKind, TrackDefinition, TrackId, TrackMaximum, TrackMutationRequest,
    TrackService, TrackValue, TracksComponent,
};

const SHOOTER: EntityId = EntityId::new(1);
const INFRASTRUCTURE: EntityId = EntityId::new(2);
const D20_ACTOR: EntityId = EntityId::new(3);

fn scalar(value: i64) -> MechanicsScalar {
    MechanicsScalar::new(value).expect("example values stay inside the mechanics scalar bound")
}

fn stat(value: &str) -> StatId {
    StatId::parse(value).expect("example stat identities are valid")
}

fn track(value: &str) -> TrackId {
    TrackId::parse(value).expect("example track identities are valid")
}

fn source(value: &str) -> SourceDefinitionId {
    SourceDefinitionId::parse(value).expect("example source identities are valid")
}

fn version() -> CatalogVersion {
    CatalogVersion::parse("example.v1").expect("example catalog version is valid")
}

fn operation(value: &str) -> OperationId {
    OperationId::parse(value).expect("example operation identity is valid")
}

fn request_source(operation: &OperationId, value: &str) -> SourceInstanceIdentity {
    SourceInstanceIdentity::Request {
        operation: operation.clone(),
        instance: SourceInstanceId::parse(value).expect("example source instance is valid"),
    }
}

fn catalog() -> MechanicsCatalog {
    MechanicsCatalog::admit(MechanicsCatalogDefinition {
        version: version(),
        stats: vec![
            StatDefinition {
                id: stat("accuracy"),
                minimum: scalar(0),
                maximum: scalar(100),
            },
            StatDefinition {
                id: stat("production"),
                minimum: scalar(0),
                maximum: scalar(1_000),
            },
        ],
        tracks: vec![
            TrackDefinition {
                id: track("health"),
                minimum: scalar(0),
                maximum: TrackMaximum::Fixed { value: scalar(100) },
            },
            TrackDefinition {
                id: track("integrity"),
                minimum: scalar(0),
                maximum: TrackMaximum::Fixed { value: scalar(500) },
            },
            TrackDefinition {
                id: track("vitality"),
                minimum: scalar(0),
                maximum: TrackMaximum::Fixed { value: scalar(80) },
            },
        ],
        sources: vec![
            SourceDefinition {
                id: source("infrastructure_upgrade"),
                priority: 0,
                stat_contributions: vec![StatContributionDefinition {
                    stat: stat("production"),
                    contribution: StatContribution::Add { amount: scalar(25) },
                    stacking_group: StackingGroupId::parse("production_upgrade")
                        .expect("example stacking group is valid"),
                    stacking: StackingPolicy::Sum,
                }],
                damage_responses: vec![],
            },
            SourceDefinition {
                id: source("reaction_ward"),
                priority: 0,
                stat_contributions: vec![],
                damage_responses: vec![DamageResponseDefinition::Prevent {
                    selector: DamageKindSelector::Any,
                    stacking_group: StackingGroupId::parse("reaction_prevention")
                        .expect("example stacking group is valid"),
                    stacking: StackingPolicy::UniqueBySource,
                }],
            },
        ],
        damage_kinds: vec![DamageKindDefinition {
            id: DamageKindId::parse("kinetic").expect("example damage kind is valid"),
        }],
        effects: vec![
            EffectDefinition {
                id: EffectDefinitionId::parse("infrastructure_upgrade")
                    .expect("example effect identity is valid"),
                stacking_group: StackingGroupId::parse("infrastructure_upgrade")
                    .expect("example stacking group is valid"),
                stacking: EffectStackingPolicy::Refresh,
                maximum_stacks: 1,
                sources: vec![source("infrastructure_upgrade")],
            },
            EffectDefinition {
                id: EffectDefinitionId::parse("reaction_ward")
                    .expect("example effect identity is valid"),
                stacking_group: StackingGroupId::parse("reaction_ward")
                    .expect("example stacking group is valid"),
                stacking: EffectStackingPolicy::Refresh,
                maximum_stacks: 1,
                sources: vec![source("reaction_ward")],
            },
        ],
        capacity_metrics: vec![],
        items: vec![],
        equipment_slots: vec![],
    })
    .expect("example catalog is admitted")
}

fn attach<T: EntityComponent>(state: &mut EntityState, entity: EntityId, value: T) {
    let revision = state
        .component_revision::<T>(entity)
        .expect("example component type is registered");
    EntityAuthoringService
        .attach_component(state, revision, entity, value)
        .expect("example component attachment is valid");
}

fn state() -> EntityState {
    let registry =
        gameplay_mechanics::gameplay_component_registry().expect("fixed registrations are valid");
    let mut state = EntityState::from_definitions_with_registry(
        registry,
        [
            EntityDefinition::new(SHOOTER, "shooter"),
            EntityDefinition::new(INFRASTRUCTURE, "power-plant"),
            EntityDefinition::new(D20_ACTOR, "tabletop-actor"),
        ],
    )
    .expect("example entities are valid");
    attach(
        &mut state,
        SHOOTER,
        StatsComponent::new(
            version(),
            vec![StatValue::new(stat("accuracy"), scalar(70))],
        )
        .expect("shooter stats are valid"),
    );
    attach(
        &mut state,
        SHOOTER,
        TracksComponent::new(
            version(),
            vec![TrackValue::new(track("health"), scalar(100))],
        )
        .expect("shooter tracks are valid"),
    );
    attach(
        &mut state,
        INFRASTRUCTURE,
        StatsComponent::new(
            version(),
            vec![StatValue::new(stat("production"), scalar(100))],
        )
        .expect("infrastructure stats are valid"),
    );
    attach(
        &mut state,
        INFRASTRUCTURE,
        TracksComponent::new(
            version(),
            vec![TrackValue::new(track("integrity"), scalar(420))],
        )
        .expect("infrastructure tracks are valid"),
    );
    attach(
        &mut state,
        INFRASTRUCTURE,
        ActiveEffectsComponent::new(version(), vec![]).expect("infrastructure effects are valid"),
    );
    attach(
        &mut state,
        D20_ACTOR,
        TracksComponent::new(
            version(),
            vec![TrackValue::new(track("vitality"), scalar(80))],
        )
        .expect("tabletop tracks are valid"),
    );
    attach(
        &mut state,
        D20_ACTOR,
        ActiveEffectsComponent::new(version(), vec![]).expect("tabletop effects are valid"),
    );
    state
}

fn damage_request(
    entity: EntityId,
    target_track: &str,
    operation_name: &str,
    amount: i64,
) -> DamageRequest {
    let operation = operation(operation_name);
    DamageRequest {
        source: request_source(&operation, "direct_damage"),
        operation,
        actor: None,
        target: entity,
        target_track: track(target_track),
        parts: vec![DamagePart {
            amount: scalar(amount),
            kind: DamageKindId::parse("kinetic").expect("example damage kind is valid"),
        }],
        request_sources: vec![],
        expected_tracks_revision: None,
    }
}

fn shooter_path(state: &mut EntityState, catalog: &MechanicsCatalog) {
    let receipt = DamageService::apply(
        state,
        catalog,
        damage_request(SHOOTER, "health", "shooter_hit", 12),
    )
    .expect("direct shooter damage resolves");
    assert_eq!(receipt.parts[0].applied, scalar(12));
}

fn infrastructure_path(state: &mut EntityState, catalog: &MechanicsCatalog) {
    EffectService::apply(
        state,
        catalog,
        EffectApplyRequest {
            operation: operation("start_upgrade"),
            entity: INFRASTRUCTURE,
            instance: EffectInstanceId::parse("upgrade_one").expect("effect instance is valid"),
            definition: EffectDefinitionId::parse("infrastructure_upgrade")
                .expect("effect definition is valid"),
            provenance: request_source(&operation("start_upgrade"), "city_phase"),
            stacks: 1,
            expected_revision: None,
        },
    )
    .expect("caller-owned city phase applies the effect");
    let production = StatService::evaluate(
        state,
        catalog,
        INFRASTRUCTURE,
        &stat("production"),
        &operation("inspect_production"),
        &[],
    )
    .expect("production evaluates");
    assert_eq!(production.value, scalar(125));

    DamageService::apply(
        state,
        catalog,
        damage_request(INFRASTRUCTURE, "integrity", "infrastructure_damage", 30),
    )
    .expect("infrastructure damage resolves");
    TrackService::restore(
        state,
        catalog,
        TrackMutationRequest {
            operation: operation("repair_shift"),
            source: request_source(&operation("repair_shift"), "maintenance"),
            entity: INFRASTRUCTURE,
            track: track("integrity"),
            amount: scalar(10),
            kind: TrackAdjustmentKind::Restore,
            expected_revision: None,
        },
    )
    .expect("repair uses the ordinary track service");
}

fn d20_shaped_path(state: &mut EntityState, catalog: &MechanicsCatalog) {
    let request = damage_request(D20_ACTOR, "vitality", "tabletop_strike", 20);
    let preview =
        DamageService::preview(state, catalog, &request).expect("downstream can preview a strike");
    assert_eq!(preview.receipt().parts[0].applied, scalar(20));

    // A downstream reaction owner decides to apply the ward, then asks Engine
    // for a fresh resolution. Engine stores no turn or reaction session.
    EffectService::apply(
        state,
        catalog,
        EffectApplyRequest {
            operation: operation("reaction_ward"),
            entity: D20_ACTOR,
            instance: EffectInstanceId::parse("ward_one").expect("effect instance is valid"),
            definition: EffectDefinitionId::parse("reaction_ward")
                .expect("effect definition is valid"),
            provenance: request_source(&operation("reaction_ward"), "reaction_owner"),
            stacks: 1,
            expected_revision: None,
        },
    )
    .expect("the downstream reaction applies an explicit effect");
    let receipt = DamageService::apply(state, catalog, request)
        .expect("fresh apply observes the completed reaction");
    assert!(receipt.parts[0].prevented);
    assert_eq!(receipt.parts[0].applied, scalar(0));
}

fn main() {
    let catalog = catalog();
    let mut state = state();
    shooter_path(&mut state, &catalog);
    infrastructure_path(&mut state, &catalog);
    d20_shaped_path(&mut state, &catalog);

    let snapshot = encode_snapshot(&state).expect("authoritative state snapshots");
    let mut restored =
        decode_snapshot_with_catalog(&snapshot, &catalog).expect("snapshot strictly reconstructs");
    let continued = TrackService::restore(
        &mut restored,
        &catalog,
        TrackMutationRequest {
            operation: operation("post_restore_repair"),
            source: request_source(&operation("post_restore_repair"), "maintenance"),
            entity: INFRASTRUCTURE,
            track: track("integrity"),
            amount: scalar(5),
            kind: TrackAdjustmentKind::Restore,
            expected_revision: None,
        },
    )
    .expect("restored state continues through the same named services");
    assert_eq!(continued.applied_amount, scalar(5));
}
