use core_ids::{EntityId, ProcessId, TagId};
use core_math::Vec3;
use entity_state::{
    ActivatableCapabilityKind, CapabilityActivation, CapabilityActivationState,
    CollisionCapability, ControllerCapability, EntityAuthoringError, EntityAuthoringService,
    EntityCapability, EntityCapabilityKind, EntityDefinition, EntityLifecycle, EntityState,
    KinematicCapability, TransformCapability,
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
fn capabilities_are_typed_and_activation_is_distinct_from_entity_lifecycle() {
    let service = EntityAuthoringService;
    let id = EntityId::new(20);
    let mut state = EntityState::from_definitions([EntityDefinition::new(id, "actor")
        .with_transform(Vec3::ZERO)
        .with_collision(true, false)
        .with_controller(ControllerCapability::Process(ProcessId::new(4)))])
    .expect("fixture");

    let readout = state
        .capability_activation(id, ActivatableCapabilityKind::Collision)
        .expect("collision activation");
    assert_eq!(readout.state, CapabilityActivationState::Active);
    assert!(readout.effective);

    service.disable(&mut state, 0, id).expect("disable entity");
    let readout = state
        .capability_activation(id, ActivatableCapabilityKind::Collision)
        .expect("collision activation");
    assert_eq!(readout.state, CapabilityActivationState::Active);
    assert!(!readout.effective);

    service
        .set_capability_activation(
            &mut state,
            1,
            id,
            ActivatableCapabilityKind::Controller,
            CapabilityActivation::Inactive,
        )
        .expect("controller deactivated");
    assert_eq!(
        state
            .capability_activation(id, ActivatableCapabilityKind::Controller)
            .expect("controller readout")
            .state,
        CapabilityActivationState::Inactive
    );
}

#[test]
fn attachment_validation_prevents_orphan_kinematics_and_transform_removal() {
    let service = EntityAuthoringService;
    let id = EntityId::new(30);
    let mut state =
        EntityState::from_definitions([EntityDefinition::new(id, "moving")]).expect("fixture");
    assert!(matches!(
        service.attach_capability(
            &mut state,
            0,
            id,
            EntityCapability::Kinematic(KinematicCapability {
                half_extents: Vec3::ONE,
                velocity: Vec3::ZERO,
            })
        ),
        Err(EntityAuthoringError::InvalidCapability {
            capability: EntityCapabilityKind::Kinematic,
            ..
        })
    ));
    service
        .attach_capability(
            &mut state,
            0,
            id,
            EntityCapability::Transform(TransformCapability::from_transform(
                entity_state::EntityTransform::at(Vec3::ZERO),
            )),
        )
        .expect("transform attached");
    service
        .attach_capability(
            &mut state,
            1,
            id,
            EntityCapability::Kinematic(KinematicCapability {
                half_extents: Vec3::ONE,
                velocity: Vec3::ZERO,
            }),
        )
        .expect("kinematic attached");
    assert!(matches!(
        service.detach_capability(&mut state, 2, id, EntityCapabilityKind::Transform),
        Err(EntityAuthoringError::CapabilityInUse { .. })
    ));
    assert_eq!(state.revision(), 2);

    service
        .attach_capability(
            &mut state,
            2,
            id,
            EntityCapability::Collision(CollisionCapability {
                enabled: false,
                static_collider: false,
            }),
        )
        .expect("collision attached");
}
