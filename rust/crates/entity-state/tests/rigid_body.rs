use core_ids::EntityId;
use core_math::Vec3;
use entity_state::{
    decode_snapshot, encode_snapshot, validate_rigid_body, ComponentPersistence,
    EntityAuthoringError, EntityAuthoringService, EntityDefinition, EntityState,
    EntityStateSnapshotError, KinematicComponent, KinematicSnapshot, RigidBodyComponent,
    RigidBodyShape, RigidBodyStatePublicationError, RigidBodyStateReplacement,
    RigidBodyValidationError, TransformComponent, MAX_RIGID_BODY_MASS, RIGID_BODY_CODEC_VERSION,
    RIGID_BODY_COMPONENT_TYPE_ID,
};

#[test]
fn rigid_body_is_a_durable_revision_guarded_component() {
    let entity = EntityId::new(71);
    let mut state = EntityState::from_definitions([
        EntityDefinition::new(entity, "dynamic crate").with_transform(Vec3::new(1.0, 3.0, 5.0))
    ])
    .expect("entity fixture");
    let body = RigidBodyComponent::dynamic(
        RigidBodyShape::Cuboid {
            half_extents: Vec3::new(0.5, 1.0, 1.5),
        },
        4.0,
    );
    let initial = state
        .component_revision::<RigidBodyComponent>(entity)
        .expect("built-in registration");
    EntityAuthoringService
        .attach_component(&mut state, initial.clone(), entity, body)
        .expect("attach rigid body");

    assert_eq!(state.rigid_body(entity), Some(&body));
    let kind = state
        .component_inspection()
        .kinds
        .into_iter()
        .find(|kind| kind.type_id.as_str() == RIGID_BODY_COMPONENT_TYPE_ID)
        .expect("rigid-body inspection");
    assert_eq!(
        kind.persistence,
        ComponentPersistence::Durable {
            version: RIGID_BODY_CODEC_VERSION,
        }
    );

    let bytes = encode_snapshot(&state).expect("encode");
    let reopened = decode_snapshot(&bytes).expect("reopen with default built-ins");
    assert_eq!(reopened.rigid_body(entity), Some(&body));
    assert_eq!(
        reopened
            .component_revision::<RigidBodyComponent>(entity)
            .unwrap()
            .revision(),
        0,
        "slot guards are reacquired rather than persisted"
    );

    let mut changed = body;
    changed.mass = 8.0;
    assert!(matches!(
        EntityAuthoringService.replace_component(&mut state, initial, entity, changed),
        Err(EntityAuthoringError::StaleComponentRevision { .. })
    ));
    assert_eq!(state.rigid_body(entity), Some(&body));
}

#[test]
fn invalid_rigid_body_rejects_without_mutation() {
    let entity = EntityId::new(72);
    let mut state = EntityState::from_definitions([
        EntityDefinition::new(entity, "invalid crate").with_transform(Vec3::ZERO)
    ])
    .expect("entity fixture");
    let revision_before = state.revision();
    let slot = state
        .component_revision::<RigidBodyComponent>(entity)
        .unwrap();
    let invalid = RigidBodyComponent::dynamic(RigidBodyShape::Sphere { radius: 1.0 }, f32::NAN);

    assert!(matches!(
        EntityAuthoringService.attach_component(&mut state, slot, entity, invalid),
        Err(EntityAuthoringError::InvalidComponent { reason, .. })
            if reason == RigidBodyValidationError::InvalidMass.code()
    ));
    assert_eq!(state.revision(), revision_before);
    assert_eq!(state.rigid_body(entity), None);
}

#[test]
fn rigid_body_mass_is_strictly_positive_and_bounded() {
    let shape = RigidBodyShape::Sphere { radius: 1.0 };
    for mass in [0.0, -1.0, f32::INFINITY, f32::NAN] {
        assert_eq!(
            validate_rigid_body(&RigidBodyComponent::dynamic(shape, mass)),
            Err(RigidBodyValidationError::InvalidMass)
        );
    }
    assert!(validate_rigid_body(&RigidBodyComponent::dynamic(shape, MAX_RIGID_BODY_MASS)).is_ok());
    assert_eq!(
        validate_rigid_body(&RigidBodyComponent::dynamic(
            shape,
            MAX_RIGID_BODY_MASS + 1.0
        )),
        Err(RigidBodyValidationError::InvalidMass)
    );

    let entity = EntityId::new(73);
    let mut state =
        EntityState::from_definitions([
            EntityDefinition::new(entity, "massless crate").with_transform(Vec3::ZERO)
        ])
        .unwrap();
    let bytes_before = encode_snapshot(&state).unwrap();
    let slot = state
        .component_revision::<RigidBodyComponent>(entity)
        .unwrap();
    assert!(matches!(
        EntityAuthoringService.attach_component(
            &mut state,
            slot,
            entity,
            RigidBodyComponent::dynamic(shape, 0.0),
        ),
        Err(EntityAuthoringError::InvalidComponent { reason, .. })
            if reason == RigidBodyValidationError::InvalidMass.code()
    ));
    assert_eq!(encode_snapshot(&state).unwrap(), bytes_before);
}

#[test]
fn kinematic_and_dynamic_rigid_body_components_conflict_without_mutation() {
    let kinematic_entity = EntityId::new(74);
    let mut kinematic_state =
        EntityState::from_definitions([EntityDefinition::new(kinematic_entity, "kinematic mover")
            .with_transform(Vec3::ZERO)
            .with_kinematic(Vec3::new(0.5, 0.5, 0.5), Vec3::ZERO)])
        .unwrap();
    let bytes_before = encode_snapshot(&kinematic_state).unwrap();
    let body_slot = kinematic_state
        .component_revision::<RigidBodyComponent>(kinematic_entity)
        .unwrap();
    assert!(matches!(
        EntityAuthoringService.attach_component(
            &mut kinematic_state,
            body_slot,
            kinematic_entity,
            RigidBodyComponent::dynamic(RigidBodyShape::Sphere { radius: 0.5 }, 1.0),
        ),
        Err(EntityAuthoringError::ComponentInUse { reason, .. })
            if reason == "kinematic component conflicts with dynamic rigid-body motion"
    ));
    assert_eq!(encode_snapshot(&kinematic_state).unwrap(), bytes_before);
    assert!(kinematic_state.rigid_body(kinematic_entity).is_none());

    let dynamic_entity = EntityId::new(75);
    let body = RigidBodyComponent::dynamic(RigidBodyShape::Sphere { radius: 0.5 }, 1.0);
    let mut dynamic_state =
        EntityState::from_definitions([
            EntityDefinition::new(dynamic_entity, "dynamic mover").with_transform(Vec3::ZERO)
        ])
        .unwrap();
    let body_slot = dynamic_state
        .component_revision::<RigidBodyComponent>(dynamic_entity)
        .unwrap();
    EntityAuthoringService
        .attach_component(&mut dynamic_state, body_slot, dynamic_entity, body)
        .unwrap();
    let bytes_before = encode_snapshot(&dynamic_state).unwrap();
    let kinematic_slot = dynamic_state
        .component_revision::<KinematicComponent>(dynamic_entity)
        .unwrap();
    assert!(matches!(
        EntityAuthoringService.attach_component(
            &mut dynamic_state,
            kinematic_slot,
            dynamic_entity,
            KinematicComponent {
                half_extents: Vec3::new(0.5, 0.5, 0.5),
                velocity: Vec3::ZERO,
            },
        ),
        Err(EntityAuthoringError::ComponentInUse { reason, .. })
            if reason == "dynamic rigid-body component conflicts with kinematic motion"
    ));
    assert_eq!(encode_snapshot(&dynamic_state).unwrap(), bytes_before);
    assert!(dynamic_state.kinematic(dynamic_entity).is_none());
}

#[test]
fn snapshot_restore_rejects_kinematic_and_dynamic_rigid_body_conflict() {
    let entity = EntityId::new(76);
    let body = RigidBodyComponent::dynamic(RigidBodyShape::Sphere { radius: 0.5 }, 1.0);
    let mut source = EntityState::from_definitions([
        EntityDefinition::new(entity, "forged mover").with_transform(Vec3::ZERO)
    ])
    .unwrap();
    let body_slot = source
        .component_revision::<RigidBodyComponent>(entity)
        .unwrap();
    EntityAuthoringService
        .attach_component(&mut source, body_slot, entity, body)
        .unwrap();
    let source_bytes = encode_snapshot(&source).unwrap();

    let mut forged = source.snapshot();
    forged.entities[0].kinematic = Some(KinematicSnapshot {
        half_extents: [0.5, 0.5, 0.5],
        velocity: [0.0, 0.0, 0.0],
    });
    let forged_bytes = serde_json::to_string_pretty(&forged).unwrap();

    assert!(matches!(
        decode_snapshot(&forged_bytes),
        Err(EntityStateSnapshotError::ConflictingComponents {
            entity: 76,
            first: "kinematic",
            second: "rigid-body",
        })
    ));
    assert_eq!(encode_snapshot(&source).unwrap(), source_bytes);
    assert!(source.kinematic(entity).is_none());
    assert_eq!(source.rigid_body(entity), Some(&body));
}

#[test]
fn rigid_body_state_publication_is_atomic_across_component_slots() {
    let first = EntityId::new(81);
    let second = EntityId::new(82);
    let body = RigidBodyComponent::dynamic(RigidBodyShape::Sphere { radius: 0.5 }, 1.0);
    let mut state = EntityState::from_definitions([
        EntityDefinition::new(first, "first").with_transform(Vec3::ZERO),
        EntityDefinition::new(second, "second").with_transform(Vec3::new(2.0, 0.0, 0.0)),
    ])
    .unwrap();
    for entity in [first, second] {
        let slot = state
            .component_revision::<RigidBodyComponent>(entity)
            .unwrap();
        EntityAuthoringService
            .attach_component(&mut state, slot, entity, body)
            .unwrap();
    }
    let revision_before = state.revision();
    let replacements = [first, second].map(|entity| RigidBodyStateReplacement {
        entity,
        expected_transform_revision: state
            .component_revision::<TransformComponent>(entity)
            .unwrap(),
        expected_rigid_body_revision: state
            .component_revision::<RigidBodyComponent>(entity)
            .unwrap(),
        transform: TransformComponent::from_transform(entity_state::EntityTransform::at(
            Vec3::new(entity.raw() as f32, 1.0, 0.0),
        )),
        rigid_body: RigidBodyComponent {
            linear_velocity: Vec3::new(1.0, 0.0, 0.0),
            ..body
        },
    });

    let receipt = entity_state::replace_rigid_body_states(&mut state, replacements.to_vec())
        .expect("all slots publish together");
    assert_eq!(receipt.revision_before, revision_before);
    assert_eq!(receipt.revision_after, revision_before + 1);
    assert_eq!(receipt.entities_changed, vec![first, second]);
    assert_eq!(state.transform(first).unwrap().translation.x, 81.0);
    assert_eq!(state.rigid_body(second).unwrap().linear_velocity.x, 1.0);
}

#[test]
fn stale_rigid_body_publication_preserves_every_candidate_entity() {
    let first = EntityId::new(91);
    let second = EntityId::new(92);
    let body = RigidBodyComponent::dynamic(RigidBodyShape::Sphere { radius: 0.5 }, 1.0);
    let mut state = EntityState::from_definitions([
        EntityDefinition::new(first, "first").with_transform(Vec3::ZERO),
        EntityDefinition::new(second, "second").with_transform(Vec3::new(2.0, 0.0, 0.0)),
    ])
    .unwrap();
    for entity in [first, second] {
        let slot = state
            .component_revision::<RigidBodyComponent>(entity)
            .unwrap();
        EntityAuthoringService
            .attach_component(&mut state, slot, entity, body)
            .unwrap();
    }
    let replacements = [first, second].map(|entity| RigidBodyStateReplacement {
        entity,
        expected_transform_revision: state
            .component_revision::<TransformComponent>(entity)
            .unwrap(),
        expected_rigid_body_revision: state
            .component_revision::<RigidBodyComponent>(entity)
            .unwrap(),
        transform: TransformComponent::from_transform(entity_state::EntityTransform::at(
            Vec3::new(9.0, 9.0, 9.0),
        )),
        rigid_body: body,
    });
    let second_slot = state
        .component_revision::<TransformComponent>(second)
        .unwrap();
    EntityAuthoringService
        .replace_component(
            &mut state,
            second_slot,
            second,
            TransformComponent::from_transform(entity_state::EntityTransform::at(Vec3::new(
                3.0, 0.0, 0.0,
            ))),
        )
        .unwrap();
    let bytes_before = encode_snapshot(&state).unwrap();

    assert!(matches!(
        entity_state::replace_rigid_body_states(&mut state, replacements.to_vec()),
        Err(RigidBodyStatePublicationError::StaleTransform { entity, .. }) if entity == second
    ));
    assert_eq!(encode_snapshot(&state).unwrap(), bytes_before);
    assert_eq!(state.transform(first).unwrap().translation, Vec3::ZERO);
    assert_eq!(state.transform(second).unwrap().translation.x, 3.0);
}
