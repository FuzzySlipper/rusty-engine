use core_ids::EntityId;
use core_math::Vec3;
use entity_state::{
    decode_snapshot, decode_snapshot_with_registry, encode_snapshot, ComponentCodec,
    ComponentPersistence, ComponentRegistration, ComponentRegistrationError, ComponentRegistry,
    ComponentReplacement, ComponentTypeId, EntityAuthoringError, EntityAuthoringService,
    EntityComponent, EntityDefinition, EntityState, EntityStateSnapshotError,
    RegisteredComponentSnapshotError, TransformComponent, MAX_COMPONENT_INSPECTION_ENTITIES,
    MAX_COMPONENT_TYPE_ID_BYTES, TRANSFORM_COMPONENT_TYPE_ID,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const RUNTIME_TYPE_ID: &str = "fixture.runtime-power";
const DURABLE_TYPE_ID: &str = "fixture.durable-power";
const DURABLE_CODEC_ID: &str = "fixture.durable-power-json";

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimePower {
    strength: u16,
}

impl EntityComponent for RuntimePower {}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OtherPower;

impl EntityComponent for OtherPower {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DurablePower {
    strength: u16,
    enabled: bool,
}

impl EntityComponent for DurablePower {}

fn component_id(value: &str) -> ComponentTypeId {
    ComponentTypeId::parse(value).expect("fixture component identity")
}

fn runtime_registration() -> ComponentRegistration<RuntimePower> {
    ComponentRegistration::runtime_only(component_id(RUNTIME_TYPE_ID)).with_validator(|value| {
        (value.strength <= 100)
            .then_some(())
            .ok_or_else(|| "strength exceeds 100".to_string())
    })
}

fn durable_registration() -> ComponentRegistration<DurablePower> {
    let codec = ComponentCodec::new(
        DURABLE_CODEC_ID,
        1,
        |value| serde_json::to_value(value).expect("fixture codec is infallible"),
        |value| serde_json::from_value(value).map_err(|error| error.to_string()),
    )
    .expect("fixture codec");
    ComponentRegistration::durable(component_id(DURABLE_TYPE_ID), codec).with_validator(|value| {
        (value.strength <= 100)
            .then_some(())
            .ok_or_else(|| "strength exceeds 100".to_string())
    })
}

#[test]
fn built_in_and_downstream_components_share_typed_store_and_deterministic_iteration() {
    let first = EntityId::new(1);
    let second = EntityId::new(2);
    let mut state = EntityState::from_definitions([
        EntityDefinition::new(second, "second").with_transform(Vec3::ONE),
        EntityDefinition::new(first, "first").with_transform(Vec3::ZERO),
    ])
    .expect("fixture");
    state
        .register_component(runtime_registration())
        .expect("external registration");

    assert_eq!(
        state
            .component_type_id::<TransformComponent>()
            .unwrap()
            .as_str(),
        TRANSFORM_COMPONENT_TYPE_ID
    );
    assert_eq!(
        state.component::<TransformComponent>(first).unwrap(),
        state.transform(first)
    );
    assert!(state.has_component::<TransformComponent>(first).unwrap());
    assert_eq!(
        state
            .components::<TransformComponent>()
            .unwrap()
            .map(|(entity, _)| entity)
            .collect::<Vec<_>>(),
        vec![first, second]
    );

    let service = EntityAuthoringService;
    let second_revision = state.component_revision::<RuntimePower>(second).unwrap();
    let first_revision = state.component_revision::<RuntimePower>(first).unwrap();
    service
        .attach_component(
            &mut state,
            second_revision,
            second,
            RuntimePower { strength: 20 },
        )
        .expect("second external component");
    assert_eq!(
        state.component_revision::<RuntimePower>(first).unwrap(),
        first_revision
    );
    service
        .attach_component(
            &mut state,
            first_revision,
            first,
            RuntimePower { strength: 10 },
        )
        .expect("first external component");
    assert_eq!(
        state
            .components::<RuntimePower>()
            .unwrap()
            .map(|(entity, value)| (entity, value.strength))
            .collect::<Vec<_>>(),
        vec![(first, 10), (second, 20)]
    );

    let inspection = state.component_inspection();
    let external = inspection
        .kinds
        .iter()
        .find(|kind| kind.type_id.as_str() == RUNTIME_TYPE_ID)
        .expect("external kind is inspectable");
    assert_eq!(external.persistence, ComponentPersistence::RuntimeOnly);
    assert_eq!(external.count, 2);
    assert_eq!(external.entity_sample, vec![first, second]);
    assert!(!external.entity_sample_truncated);
}

#[test]
fn typed_mutation_is_revision_guarded_validated_and_cleans_up_on_destroy() {
    let entity = EntityId::new(10);
    let unknown = EntityId::new(99);
    let mut state =
        EntityState::from_definitions([EntityDefinition::new(entity, "actor")]).expect("fixture");
    state.register_component(runtime_registration()).unwrap();
    let service = EntityAuthoringService;

    let unknown_revision = state.component_revision::<RuntimePower>(unknown).unwrap();
    assert!(matches!(
        service.attach_component(
            &mut state,
            unknown_revision,
            unknown,
            RuntimePower { strength: 1 }
        ),
        Err(EntityAuthoringError::UnknownEntity { entity: value }) if value == unknown
    ));
    assert_eq!(state.revision(), 0);
    assert!(!state.has_component::<RuntimePower>(entity).unwrap());

    let initial_revision = state.component_revision::<RuntimePower>(entity).unwrap();
    service
        .attach_component(
            &mut state,
            initial_revision.clone(),
            entity,
            RuntimePower { strength: 40 },
        )
        .expect("attach");
    assert!(matches!(
        service.replace_component(
            &mut state,
            initial_revision,
            entity,
            RuntimePower { strength: 50 }
        ),
        Err(EntityAuthoringError::StaleComponentRevision {
            expected: 0,
            actual: 1,
            ..
        })
    ));
    let attached_revision = state.component_revision::<RuntimePower>(entity).unwrap();
    assert!(matches!(
        service.replace_component(
            &mut state,
            attached_revision.clone(),
            entity,
            RuntimePower { strength: 101 }
        ),
        Err(EntityAuthoringError::InvalidComponent { .. })
    ));
    assert_eq!(state.revision(), 1);
    assert_eq!(
        state.component::<RuntimePower>(entity).unwrap(),
        Some(&RuntimePower { strength: 40 })
    );

    service
        .replace_component(
            &mut state,
            attached_revision,
            entity,
            RuntimePower { strength: 50 },
        )
        .expect("replace");
    assert_eq!(state.revision(), 2);
    let replaced_revision = state.component_revision::<RuntimePower>(entity).unwrap();
    service
        .detach_component::<RuntimePower>(&mut state, replaced_revision, entity)
        .expect("remove");
    assert_eq!(state.revision(), 3);
    assert!(!state.has_component::<RuntimePower>(entity).unwrap());

    let detached_revision = state.component_revision::<RuntimePower>(entity).unwrap();
    service
        .attach_component(
            &mut state,
            detached_revision,
            entity,
            RuntimePower { strength: 60 },
        )
        .expect("reattach");
    let live_revision = state.component_revision::<RuntimePower>(entity).unwrap();
    service.destroy(&mut state, 4, entity).expect("destroy");
    assert_eq!(state.components::<RuntimePower>().unwrap().count(), 0);
    assert!(matches!(
        service.attach_component(
            &mut state,
            live_revision,
            entity,
            RuntimePower { strength: 1 }
        ),
        Err(EntityAuthoringError::TombstonedEntity { .. })
    ));
    assert_eq!(state.revision(), 5);
}

#[test]
fn homogeneous_component_replacements_validate_all_slots_before_one_publication() {
    let first = EntityId::new(20);
    let second = EntityId::new(21);
    let mut state = EntityState::from_definitions([
        EntityDefinition::new(first, "first"),
        EntityDefinition::new(second, "second"),
    ])
    .unwrap();
    state.register_component(runtime_registration()).unwrap();
    let service = EntityAuthoringService;
    for (entity, strength) in [(first, 10), (second, 20)] {
        let revision = state.component_revision::<RuntimePower>(entity).unwrap();
        service
            .attach_component(&mut state, revision, entity, RuntimePower { strength })
            .unwrap();
    }

    let before_revision = state.revision();
    let first_revision = state.component_revision::<RuntimePower>(first).unwrap();
    let second_revision = state.component_revision::<RuntimePower>(second).unwrap();
    assert!(matches!(
        service.replace_components(
            &mut state,
            vec![
                ComponentReplacement {
                    expected_revision: first_revision.clone(),
                    entity: first,
                    component: RuntimePower { strength: 30 },
                },
                ComponentReplacement {
                    expected_revision: second_revision.clone(),
                    entity: second,
                    component: RuntimePower { strength: 101 },
                },
            ],
        ),
        Err(EntityAuthoringError::InvalidComponent { entity, .. }) if entity == second
    ));
    assert_eq!(state.revision(), before_revision);
    assert_eq!(
        state.component::<RuntimePower>(first).unwrap(),
        Some(&RuntimePower { strength: 10 })
    );
    assert_eq!(
        state.component::<RuntimePower>(second).unwrap(),
        Some(&RuntimePower { strength: 20 })
    );

    let receipt = service
        .replace_components(
            &mut state,
            vec![
                ComponentReplacement {
                    expected_revision: second_revision,
                    entity: second,
                    component: RuntimePower { strength: 40 },
                },
                ComponentReplacement {
                    expected_revision: first_revision,
                    entity: first,
                    component: RuntimePower { strength: 30 },
                },
            ],
        )
        .unwrap();
    assert_eq!(receipt.revision_before, before_revision);
    assert_eq!(receipt.revision_after, before_revision + 1);
    assert_eq!(
        receipt
            .facts
            .iter()
            .map(|fact| match fact {
                entity_state::EntityAuthoringFact::ComponentReplaced { entity, .. } => *entity,
                other => panic!("unexpected fact {other:?}"),
            })
            .collect::<Vec<_>>(),
        vec![first, second]
    );
    assert_eq!(
        state.component::<RuntimePower>(first).unwrap(),
        Some(&RuntimePower { strength: 30 })
    );
    assert_eq!(
        state.component::<RuntimePower>(second).unwrap(),
        Some(&RuntimePower { strength: 40 })
    );
}

#[test]
fn registration_conflicts_fail_before_existing_state_changes() {
    let entity = EntityId::new(20);
    let mut state =
        EntityState::from_definitions([EntityDefinition::new(entity, "actor")]).expect("fixture");
    state.register_component(runtime_registration()).unwrap();
    let revision = state.component_revision::<RuntimePower>(entity).unwrap();
    EntityAuthoringService
        .attach_component(&mut state, revision, entity, RuntimePower { strength: 12 })
        .unwrap();
    let before_inspection = state.component_inspection();

    assert!(matches!(
        state.register_component(runtime_registration()),
        Err(ComponentRegistrationError::DuplicateStableId { .. })
    ));
    assert!(matches!(
        state.register_component(ComponentRegistration::<OtherPower>::runtime_only(
            component_id(RUNTIME_TYPE_ID)
        )),
        Err(ComponentRegistrationError::StableIdConflict { .. })
    ));
    assert!(matches!(
        state.register_component(ComponentRegistration::<RuntimePower>::runtime_only(
            component_id("fixture.same-rust-type-other-id")
        )),
        Err(ComponentRegistrationError::RustTypeConflict { .. })
    ));
    let incompatible_codec = ComponentCodec::new(
        "fixture.runtime-now-durable",
        1,
        |value: &RuntimePower| json!({ "strength": value.strength }),
        |value: Value| {
            value
                .get("strength")
                .and_then(Value::as_u64)
                .and_then(|value| u16::try_from(value).ok())
                .map(|strength| RuntimePower { strength })
                .ok_or_else(|| "invalid runtime power".to_string())
        },
    )
    .unwrap();
    assert!(matches!(
        state.register_component(ComponentRegistration::durable(
            component_id(RUNTIME_TYPE_ID),
            incompatible_codec
        )),
        Err(ComponentRegistrationError::IncompatibleCodec { .. })
    ));

    assert_eq!(state.revision(), 1);
    assert_eq!(state.component_inspection(), before_inspection);
    assert_eq!(
        state.component::<RuntimePower>(entity).unwrap(),
        Some(&RuntimePower { strength: 12 })
    );
}

#[test]
fn durable_external_component_round_trips_only_with_matching_explicit_registry() {
    let entity = EntityId::new(30);
    let mut registry = ComponentRegistry::new();
    registry.register(durable_registration()).unwrap();
    let mut state = EntityState::from_definitions_with_registry(
        registry.clone(),
        [EntityDefinition::new(entity, "durable")],
    )
    .unwrap();
    let revision = state.component_revision::<DurablePower>(entity).unwrap();
    EntityAuthoringService
        .attach_component(
            &mut state,
            revision,
            entity,
            DurablePower {
                strength: 77,
                enabled: true,
            },
        )
        .unwrap();

    let encoded = encode_snapshot(&state).unwrap();
    assert!(encoded.contains(DURABLE_TYPE_ID));
    assert!(encoded.contains(DURABLE_CODEC_ID));
    assert!(!encoded.contains(std::any::type_name::<DurablePower>()));
    assert!(matches!(
        decode_snapshot(&encoded),
        Err(EntityStateSnapshotError::RegisteredComponent(
            RegisteredComponentSnapshotError::UnknownRequiredType { .. }
        ))
    ));

    let restored = decode_snapshot_with_registry(&encoded, registry).unwrap();
    assert_eq!(restored.revision(), 1);
    assert_eq!(
        restored.component::<DurablePower>(entity).unwrap(),
        Some(&DurablePower {
            strength: 77,
            enabled: true,
        })
    );
}

#[test]
fn durable_component_snapshot_rejects_duplicates_codec_drift_and_bad_values() {
    let entity = EntityId::new(40);
    let mut registry = ComponentRegistry::new();
    registry.register(durable_registration()).unwrap();
    let mut state = EntityState::from_definitions_with_registry(
        registry.clone(),
        [EntityDefinition::new(entity, "strict")],
    )
    .unwrap();
    let revision = state.component_revision::<DurablePower>(entity).unwrap();
    EntityAuthoringService
        .attach_component(
            &mut state,
            revision,
            entity,
            DurablePower {
                strength: 10,
                enabled: false,
            },
        )
        .unwrap();
    let encoded = encode_snapshot(&state).unwrap();
    let source: Value = serde_json::from_str(&encoded).unwrap();

    let mut duplicate_type = source.clone();
    let records = duplicate_type["registeredComponents"]
        .as_array_mut()
        .unwrap();
    records.push(records[0].clone());
    assert!(matches!(
        decode_snapshot_with_registry(&duplicate_type.to_string(), registry.clone()),
        Err(EntityStateSnapshotError::RegisteredComponent(
            RegisteredComponentSnapshotError::DuplicateType { .. }
        ))
    ));

    let mut duplicate_value = source.clone();
    let values = duplicate_value["registeredComponents"][0]["values"]
        .as_array_mut()
        .unwrap();
    values.push(values[0].clone());
    assert!(matches!(
        decode_snapshot_with_registry(&duplicate_value.to_string(), registry.clone()),
        Err(EntityStateSnapshotError::RegisteredComponent(
            RegisteredComponentSnapshotError::DuplicateEntityValue { .. }
        ))
    ));

    let mut codec_drift = source.clone();
    codec_drift["registeredComponents"][0]["version"] = json!(2);
    assert!(matches!(
        decode_snapshot_with_registry(&codec_drift.to_string(), registry.clone()),
        Err(EntityStateSnapshotError::RegisteredComponent(
            RegisteredComponentSnapshotError::CodecMismatch { .. }
        ))
    ));

    let mut unknown_entity = source.clone();
    unknown_entity["registeredComponents"][0]["values"][0]["entity"] = json!(999);
    assert!(matches!(
        decode_snapshot_with_registry(&unknown_entity.to_string(), registry.clone()),
        Err(EntityStateSnapshotError::RegisteredComponent(
            RegisteredComponentSnapshotError::UnknownEntity { .. }
        ))
    ));

    let mut tombstone_value = source.clone();
    tombstone_value["entities"][0]["lifecycle"] = json!("tombstoned");
    assert!(matches!(
        decode_snapshot_with_registry(&tombstone_value.to_string(), registry.clone()),
        Err(EntityStateSnapshotError::RegisteredComponent(
            RegisteredComponentSnapshotError::TombstonedEntity { .. }
        ))
    ));

    let mut malformed_identity = source.clone();
    malformed_identity["registeredComponents"][0]["typeId"] = json!("Fixture.Invalid");
    assert!(matches!(
        decode_snapshot_with_registry(&malformed_identity.to_string(), registry.clone()),
        Err(EntityStateSnapshotError::RegisteredComponent(
            RegisteredComponentSnapshotError::InvalidTypeId { .. }
        ))
    ));

    let mut bad_value = source;
    bad_value["registeredComponents"][0]["values"][0]["value"]["strength"] = json!(101);
    assert!(matches!(
        decode_snapshot_with_registry(&bad_value.to_string(), registry),
        Err(EntityStateSnapshotError::RegisteredComponent(
            RegisteredComponentSnapshotError::InvalidValue { .. }
        ))
    ));
}

#[test]
fn runtime_only_snapshot_policy_and_inspection_bounds_are_explicit() {
    let entities: Vec<_> = (0..(MAX_COMPONENT_INSPECTION_ENTITIES + 6))
        .map(|index| {
            EntityDefinition::new(
                EntityId::new(1_000 + index as u64),
                format!("entity-{index}"),
            )
        })
        .collect();
    let mut state = EntityState::from_definitions(entities).unwrap();
    state.register_component(runtime_registration()).unwrap();
    let service = EntityAuthoringService;
    for index in 0..(MAX_COMPONENT_INSPECTION_ENTITIES + 6) {
        let entity = EntityId::new(1_000 + index as u64);
        let revision = state.component_revision::<RuntimePower>(entity).unwrap();
        service
            .attach_component(&mut state, revision, entity, RuntimePower { strength: 1 })
            .unwrap();
    }

    let kind = state
        .component_inspection()
        .kinds
        .into_iter()
        .find(|kind| kind.type_id.as_str() == RUNTIME_TYPE_ID)
        .unwrap();
    assert_eq!(kind.count, MAX_COMPONENT_INSPECTION_ENTITIES + 6);
    assert_eq!(kind.entity_sample.len(), MAX_COMPONENT_INSPECTION_ENTITIES);
    assert!(kind.entity_sample_truncated);

    let encoded = encode_snapshot(&state).unwrap();
    assert!(!encoded.contains(RUNTIME_TYPE_ID));
    let restored = decode_snapshot(&encoded).unwrap();
    assert_eq!(restored.revision(), state.revision());
    assert_eq!(restored.total_count(), state.total_count());
}

#[test]
fn stable_component_and_codec_identities_are_bounded_and_versioned() {
    let at_limit = format!("a{}", "b".repeat(MAX_COMPONENT_TYPE_ID_BYTES - 1));
    assert_eq!(
        ComponentTypeId::parse(&at_limit).unwrap().as_str(),
        at_limit
    );
    let over_limit = format!("a{}", "b".repeat(MAX_COMPONENT_TYPE_ID_BYTES));
    assert!(ComponentTypeId::parse(over_limit).is_err());
    assert!(ComponentTypeId::parse("Rust.Type").is_err());
    assert!(ComponentCodec::<DurablePower>::new(
        DURABLE_CODEC_ID,
        0,
        |value| serde_json::to_value(value).unwrap(),
        |value| serde_json::from_value(value).map_err(|error| error.to_string()),
    )
    .is_err());
}

#[test]
fn generic_replacement_cannot_bypass_static_transform_invariant() {
    let entity = EntityId::new(2_000);
    let mut state = EntityState::from_definitions([EntityDefinition::new(entity, "wall")
        .with_transform(Vec3::ZERO)
        .with_collision(true, true)])
    .unwrap();
    let requested = TransformComponent::from_transform(entity_state::EntityTransform::at(
        Vec3::new(1.0, 0.0, 0.0),
    ));
    let revision = state
        .component_revision::<TransformComponent>(entity)
        .unwrap();
    assert!(matches!(
        EntityAuthoringService.replace_component(&mut state, revision, entity, requested),
        Err(EntityAuthoringError::ComponentInUse { .. })
    ));
    assert_eq!(state.revision(), 0);
    assert_eq!(state.transform(entity).unwrap().translation, Vec3::ZERO);
}
