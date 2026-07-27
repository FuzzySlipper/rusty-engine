use core_ids::{EntityId, ProcessId, TagId};
use core_math::Vec3;
use entity_state::{
    encode_snapshot, ActivatableComponentKind, CollisionComponent, ComponentActivation,
    ComponentActivationState, ControllerComponent, EntityAuthoringError, EntityAuthoringService,
    EntityDefinition, EntityLifecycle, EntityState, KinematicComponent, TransformComponent,
    KINEMATIC_COMPONENT_TYPE_ID,
};

#[test]
fn admission_is_atomic_and_revision_guarded() {
    let service = EntityAuthoringService;
    let mut state = EntityState::default();
    let invalid =
        EntityDefinition::new(EntityId::new(2), "invalid").with_kinematic(Vec3::ONE, Vec3::ZERO);
    let result = service.admit(
        &mut state,
        0,
        [EntityDefinition::new(EntityId::new(1), "valid"), invalid],
    );
    assert!(matches!(
        result,
        Err(EntityAuthoringError::InvalidDefinition(_))
    ));
    assert_eq!(state.revision(), 0);
    assert!(!state.contains(EntityId::new(1)));

    let receipt = service
        .admit(
            &mut state,
            0,
            [
                EntityDefinition::new(EntityId::new(1), "one"),
                EntityDefinition::new(EntityId::new(2), "two"),
            ],
        )
        .expect("valid definitions commit together");
    assert_eq!(receipt.revision_after, 1);
    assert_eq!(receipt.facts.len(), 2);
    assert!(matches!(
        service.admit(
            &mut state,
            0,
            [EntityDefinition::new(EntityId::new(3), "stale")]
        ),
        Err(EntityAuthoringError::StaleRevision {
            expected: 0,
            actual: 1
        })
    ));
    assert!(!state.contains(EntityId::new(3)));
}

#[test]
fn lifecycle_labels_and_tombstones_have_one_clear_owner() {
    let service = EntityAuthoringService;
    let id = EntityId::new(10);
    let mut state = EntityState::from_definitions([EntityDefinition::new(id, "crate")
        .with_transform(Vec3::new(2.0, 0.0, 0.0))
        .with_labels([TagId::new(7)])])
    .expect("fixture");

    service
        .add_label(&mut state, 0, id, TagId::new(3))
        .expect("label added");
    service.disable(&mut state, 1, id).expect("disabled");
    assert_eq!(
        state.view(id).expect("view").lifecycle,
        EntityLifecycle::Disabled
    );
    service.enable(&mut state, 2, id).expect("enabled");
    service.destroy(&mut state, 3, id).expect("destroyed");

    let view = state.view(id).expect("tombstone remains queryable");
    assert_eq!(view.lifecycle, EntityLifecycle::Tombstoned);
    assert_eq!(view.labels, vec![TagId::new(3), TagId::new(7)]);
    assert!(view.transform.is_none());
    assert!(!state.is_alive(id));
    assert!(matches!(
        service.enable(&mut state, 4, id),
        Err(EntityAuthoringError::InvalidLifecycleTransition { .. })
    ));
}

#[test]
fn repeated_lifecycle_and_activation_transitions_are_rejected_without_mutation() {
    let service = EntityAuthoringService;
    let id = EntityId::new(11);
    let mut state = EntityState::from_definitions([EntityDefinition::new(id, "actor")
        .with_transform(Vec3::ZERO)
        .with_collision(true, false)
        .with_controller(ControllerComponent::Process(ProcessId::new(5)))])
    .expect("fixture");

    let before = encode_snapshot(&state).unwrap();
    assert!(matches!(
        service.enable(&mut state, 0, id),
        Err(EntityAuthoringError::InvalidLifecycleTransition {
            from: EntityLifecycle::Active,
            to: EntityLifecycle::Active,
            ..
        })
    ));
    assert_eq!(encode_snapshot(&state).unwrap(), before);

    assert!(matches!(
        service.set_component_activation(
            &mut state,
            0,
            id,
            ActivatableComponentKind::Controller,
            ComponentActivation::Active,
        ),
        Err(EntityAuthoringError::Activation(
            entity_state::ComponentActivationError::AlreadyInState {
                state: ComponentActivationState::Active,
                ..
            }
        ))
    ));
    assert_eq!(encode_snapshot(&state).unwrap(), before);
}

#[test]
fn components_are_typed_and_activation_is_distinct_from_entity_lifecycle() {
    let service = EntityAuthoringService;
    let id = EntityId::new(20);
    let mut state = EntityState::from_definitions([EntityDefinition::new(id, "actor")
        .with_transform(Vec3::ZERO)
        .with_collision(true, false)
        .with_controller(ControllerComponent::Process(ProcessId::new(4)))])
    .expect("fixture");

    let readout = state
        .component_activation(id, ActivatableComponentKind::Collision)
        .expect("collision activation");
    assert_eq!(readout.state, ComponentActivationState::Active);
    assert!(readout.effective);

    service.disable(&mut state, 0, id).expect("disable entity");
    let readout = state
        .component_activation(id, ActivatableComponentKind::Collision)
        .expect("collision activation");
    assert_eq!(readout.state, ComponentActivationState::Active);
    assert!(!readout.effective);

    service
        .set_component_activation(
            &mut state,
            1,
            id,
            ActivatableComponentKind::Controller,
            ComponentActivation::Inactive,
        )
        .expect("controller deactivated");
    assert_eq!(
        state
            .component_activation(id, ActivatableComponentKind::Controller)
            .expect("controller readout")
            .state,
        ComponentActivationState::Inactive
    );
}

#[test]
fn attachment_validation_prevents_orphan_kinematics_and_transform_removal() {
    let service = EntityAuthoringService;
    let id = EntityId::new(30);
    let mut state =
        EntityState::from_definitions([EntityDefinition::new(id, "moving")]).expect("fixture");
    assert_eq!(
        state
            .component_activation(id, ActivatableComponentKind::Collision)
            .unwrap()
            .state,
        ComponentActivationState::Absent
    );
    let kinematic_revision = state.component_revision::<KinematicComponent>(id).unwrap();
    assert!(matches!(
        service.attach_component(
            &mut state,
            kinematic_revision.clone(),
            id,
            KinematicComponent {
                half_extents: Vec3::ONE,
                velocity: Vec3::ZERO,
            }
        ),
        Err(EntityAuthoringError::InvalidComponent {
            component,
            ..
        }) if component.as_str() == KINEMATIC_COMPONENT_TYPE_ID
    ));
    let transform_revision = state.component_revision::<TransformComponent>(id).unwrap();
    service
        .attach_component(
            &mut state,
            transform_revision,
            id,
            TransformComponent::from_transform(entity_state::EntityTransform::at(Vec3::ZERO)),
        )
        .expect("transform attached");
    service
        .attach_component(
            &mut state,
            kinematic_revision,
            id,
            KinematicComponent {
                half_extents: Vec3::ONE,
                velocity: Vec3::ZERO,
            },
        )
        .expect("kinematic attached");
    let transform_revision = state.component_revision::<TransformComponent>(id).unwrap();
    assert!(matches!(
        service.detach_component::<TransformComponent>(&mut state, transform_revision, id),
        Err(EntityAuthoringError::ComponentInUse { .. })
    ));
    assert_eq!(state.revision(), 2);

    let collision_revision = state.component_revision::<CollisionComponent>(id).unwrap();
    service
        .attach_component(
            &mut state,
            collision_revision,
            id,
            CollisionComponent {
                enabled: false,
                static_collider: false,
            },
        )
        .expect("collision attached");
    let collision = state
        .component_activation(id, ActivatableComponentKind::Collision)
        .expect("collision readout");
    assert_eq!(collision.state, ComponentActivationState::Inactive);
    assert!(!collision.effective);
    service
        .set_component_activation(
            &mut state,
            3,
            id,
            ActivatableComponentKind::Collision,
            ComponentActivation::Active,
        )
        .expect("collision activated");
    assert!(state.active_collision(id).is_some());
    service.disable(&mut state, 4, id).expect("entity disabled");
    let collision = state
        .component_activation(id, ActivatableComponentKind::Collision)
        .expect("collision readout");
    assert_eq!(collision.state, ComponentActivationState::Active);
    assert!(!collision.effective);
}
