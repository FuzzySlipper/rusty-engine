use core_ids::EntityId;
use core_math::Vec3;
use engine_spatial::{
    RigidBodyAction, RigidBodyService, RigidBodyStepError, RigidBodyStepRequest, StaticMeshAssetId,
    StaticMeshColliderAsset, StaticMeshColliderInstance, StaticMeshInstanceId, StaticMeshTransform,
    VoxelCollisionScene,
};
use entity_state::{
    decode_snapshot, encode_snapshot, EntityAuthoringService, EntityDefinition, EntityState,
    EntityTransform, RelationshipCommand, RigidBodyComponent, RigidBodyShape,
    RigidBodyStatePublicationError, TransformComponent, TransformParentMode,
};

fn body_state(
    definitions: impl IntoIterator<Item = (EntityId, Vec3, RigidBodyComponent)>,
) -> EntityState {
    let definitions = definitions.into_iter().collect::<Vec<_>>();
    let mut state = EntityState::from_definitions(
        definitions
            .iter()
            .map(|(entity, translation, _)| {
                EntityDefinition::new(*entity, format!("body-{}", entity.raw()))
                    .with_transform(*translation)
            })
            .collect::<Vec<_>>(),
    )
    .unwrap();
    for (entity, _, body) in definitions {
        let revision = state
            .component_revision::<RigidBodyComponent>(entity)
            .unwrap();
        EntityAuthoringService
            .attach_component(&mut state, revision, entity, body)
            .unwrap();
    }
    state
}

fn empty_scene() -> VoxelCollisionScene {
    VoxelCollisionScene::from_solid_voxels(1.0, 8, std::iter::empty::<[i64; 3]>())
        .expect("empty canonical scene")
}

fn no_gravity() -> RigidBodyStepRequest {
    RigidBodyStepRequest {
        step_seconds: 1.0 / 60.0,
        steps: 1,
        gravity: Vec3::ZERO,
        actions: Vec::new(),
    }
}

#[test]
fn caller_driven_step_integrates_gravity_impulse_and_rotation() {
    let entity = EntityId::new(1);
    let body = RigidBodyComponent::dynamic(RigidBodyShape::Sphere { radius: 0.5 }, 2.0);
    let mut state = body_state([(entity, Vec3::new(0.0, 4.0, 0.0), body)]);
    let scene = empty_scene();
    let mut service = RigidBodyService::default();

    let falling = service
        .step(&mut state, &scene, RigidBodyStepRequest::single(1.0 / 60.0))
        .unwrap();
    assert!(falling.facts[0].transform_after.translation.y < 4.0);
    assert!(falling.facts[0].linear_velocity_after.y < 0.0);

    let mut request = no_gravity();
    request.actions.push(RigidBodyAction {
        entity,
        force: Vec3::ZERO,
        torque: Vec3::ZERO,
        impulse: Vec3::new(2.0, 0.0, 0.0),
        torque_impulse: Vec3::new(0.0, 0.0, 1.0),
        wake: true,
    });
    let driven = service.step(&mut state, &scene, request).unwrap();
    assert!(driven.facts[0].linear_velocity_after.x > 0.9);
    assert!(driven.facts[0].angular_velocity_after.z > 0.0);
    assert_ne!(
        driven.facts[0].transform_after.rotation,
        driven.facts[0].transform_before.rotation
    );

    let velocity_before_force = state.rigid_body(entity).unwrap().linear_velocity.x;
    let forced = service
        .step(
            &mut state,
            &scene,
            RigidBodyStepRequest {
                step_seconds: 1.0 / 60.0,
                steps: 4,
                gravity: Vec3::ZERO,
                actions: vec![RigidBodyAction::force(entity, Vec3::new(2.0, 0.0, 0.0))],
            },
        )
        .unwrap();
    let force_delta = forced.facts[0].linear_velocity_after.x - velocity_before_force;
    assert!((force_delta - 4.0 / 60.0).abs() < 0.002, "{force_delta}");
}

#[test]
fn canonical_solver_enforces_planar_axis_locks_without_rejecting_locked_influences() {
    let entity = EntityId::new(2);
    let mut body = RigidBodyComponent::dynamic(RigidBodyShape::Sphere { radius: 0.5 }, 1.0);
    body.locked_translation_axes = [false, true, false];
    body.locked_rotation_axes = [true, false, true];
    let mut state = body_state([(entity, Vec3::ZERO, body)]);
    let scene = empty_scene();
    let mut service = RigidBodyService::default();

    let receipt = service
        .step(
            &mut state,
            &scene,
            RigidBodyStepRequest {
                step_seconds: 1.0 / 60.0,
                steps: 1,
                // This would exceed the discrete motion limit if the locked Y
                // contribution were still included by preflight.
                gravity: Vec3::new(0.0, -50_000.0, 0.0),
                actions: vec![RigidBodyAction {
                    entity,
                    force: Vec3::new(2.0, 1_000_000.0, 3.0),
                    torque: Vec3::new(100.0, 4.0, 100.0),
                    impulse: Vec3::new(1.0, 1_000_000.0, 1.5),
                    torque_impulse: Vec3::new(100.0, 2.0, 100.0),
                    wake: true,
                }],
            },
        )
        .expect("solver, not downstream correction, suppresses locked axes");
    let fact = receipt.facts[0];

    assert!(fact.transform_after.translation.x > 0.0);
    assert!(fact.transform_after.translation.z > 0.0);
    assert_eq!(fact.transform_after.translation.y, 0.0);
    assert_eq!(fact.linear_velocity_after.y, 0.0);
    assert!(fact.angular_velocity_after.y > 0.0);
    assert_eq!(fact.angular_velocity_after.x, 0.0);
    assert_eq!(fact.angular_velocity_after.z, 0.0);

    // Assert the solved orientation, not merely the angular-velocity fact.
    let orientation = fact.transform_after.rotation;
    assert!(
        orientation.y.abs() > 1.0e-4,
        "yaw did not integrate: {orientation:?}"
    );
    assert!(
        orientation.x.abs() < 1.0e-6,
        "pitch leaked: {orientation:?}"
    );
    assert!(orientation.z.abs() < 1.0e-6, "roll leaked: {orientation:?}");
}

#[test]
fn all_axis_locks_are_valid_and_stationary() {
    let entity = EntityId::new(3);
    let mut body = RigidBodyComponent::dynamic(RigidBodyShape::Sphere { radius: 0.5 }, 1.0);
    body.locked_translation_axes = [true; 3];
    body.locked_rotation_axes = [true; 3];
    let mut state = body_state([(entity, Vec3::new(1.0, 2.0, 3.0), body)]);
    let scene = empty_scene();
    let mut service = RigidBodyService::default();

    let receipt = service
        .step(
            &mut state,
            &scene,
            RigidBodyStepRequest {
                step_seconds: 1.0 / 60.0,
                steps: 1,
                gravity: Vec3::new(0.0, -9.81, 0.0),
                actions: vec![RigidBodyAction {
                    entity,
                    force: Vec3::splat(1_000_000.0),
                    torque: Vec3::splat(1_000_000.0),
                    impulse: Vec3::splat(1_000_000.0),
                    torque_impulse: Vec3::splat(1_000_000.0),
                    wake: true,
                }],
            },
        )
        .expect("all locked axes are a valid canonical body");
    let fact = receipt.facts[0];
    assert_eq!(fact.transform_before, fact.transform_after);
    assert_eq!(fact.linear_velocity_after, Vec3::ZERO);
    assert_eq!(fact.angular_velocity_after, Vec3::ZERO);
}

#[test]
fn axis_locks_do_not_change_disabled_body_behavior() {
    let entity = EntityId::new(4);
    let mut body = RigidBodyComponent::dynamic(RigidBodyShape::Sphere { radius: 0.5 }, 1.0);
    body.enabled = false;
    body.locked_translation_axes = [false, true, false];
    body.locked_rotation_axes = [true, false, true];
    let mut state = body_state([(entity, Vec3::new(1.0, 2.0, 3.0), body)]);
    let scene = empty_scene();
    let mut service = RigidBodyService::default();

    let receipt = service
        .step(
            &mut state,
            &scene,
            RigidBodyStepRequest {
                step_seconds: 1.0 / 60.0,
                steps: 1,
                gravity: Vec3::new(0.0, -9.81, 0.0),
                actions: vec![RigidBodyAction::impulse(entity, Vec3::new(0.5, 0.5, 0.5))],
            },
        )
        .expect("disabled bodies remain admissible with lock facts");
    let fact = receipt.facts[0];
    assert_eq!(fact.transform_before, fact.transform_after);
    assert!(fact.linear_velocity_after.x > 0.0);
    assert_eq!(fact.linear_velocity_after.y, 0.0);
    assert!(fact.linear_velocity_after.z > 0.0);
    assert_eq!(fact.angular_velocity_after, Vec3::ZERO);
}

#[test]
fn dynamic_bodies_contact_voxel_and_static_mesh_environment() {
    let body = RigidBodyComponent::dynamic(RigidBodyShape::Sphere { radius: 0.45 }, 1.0);
    let voxel_entity = EntityId::new(10);
    let mut voxel_state = body_state([(voxel_entity, Vec3::new(0.5, 3.0, 0.5), body)]);
    let voxel_scene = VoxelCollisionScene::from_solid_voxels(
        1.0,
        8,
        (-2..=2).flat_map(|x| (-2..=2).map(move |z| [x, 0, z])),
    )
    .unwrap();
    let mut service = RigidBodyService::default();
    let mut saw_voxel_contact = false;
    for _ in 0..120 {
        let receipt = service
            .step(
                &mut voxel_state,
                &voxel_scene,
                RigidBodyStepRequest::single(1.0 / 60.0),
            )
            .unwrap();
        saw_voxel_contact |= receipt
            .contacts
            .iter()
            .any(|contact| contact.first == voxel_entity && contact.second.is_none());
    }
    assert!(saw_voxel_contact);
    assert!(voxel_state.transform(voxel_entity).unwrap().translation.y >= 1.43);

    let mesh_entity = EntityId::new(11);
    let mut mesh_state = body_state([(mesh_entity, Vec3::new(0.0, 2.0, 0.0), body)]);
    let mut mesh_scene = empty_scene();
    let asset = StaticMeshColliderAsset::new(
        StaticMeshAssetId(1),
        vec![
            [-4.0, 0.0, -4.0],
            [4.0, 0.0, -4.0],
            [4.0, 0.0, 4.0],
            [-4.0, 0.0, 4.0],
        ],
        vec![[0, 2, 1], [0, 3, 2]],
    )
    .unwrap();
    let hash = asset.geometry_hash;
    mesh_scene
        .replace_static_mesh_colliders(
            0,
            [asset],
            [StaticMeshColliderInstance {
                id: StaticMeshInstanceId(1),
                asset: StaticMeshAssetId(1),
                expected_geometry_hash: hash,
                transform: StaticMeshTransform::IDENTITY,
            }],
        )
        .unwrap();
    let mut mesh_service = RigidBodyService::default();
    let mut saw_mesh_contact = false;
    for _ in 0..120 {
        let receipt = mesh_service
            .step(
                &mut mesh_state,
                &mesh_scene,
                RigidBodyStepRequest::single(1.0 / 60.0),
            )
            .unwrap();
        saw_mesh_contact |= receipt
            .contacts
            .iter()
            .any(|contact| contact.first == mesh_entity && contact.second.is_none());
    }
    assert!(saw_mesh_contact);
    assert!(mesh_state.transform(mesh_entity).unwrap().translation.y >= 0.43);
}

#[test]
fn dynamic_contacts_respect_filtering_and_sleep_wake_state() {
    let first = EntityId::new(20);
    let second = EntityId::new(21);
    let mut first_body = RigidBodyComponent::dynamic(RigidBodyShape::Sphere { radius: 0.5 }, 1.0);
    first_body.linear_velocity = Vec3::new(4.0, 0.0, 0.0);
    let mut second_body = first_body;
    second_body.linear_velocity = Vec3::new(-4.0, 0.0, 0.0);
    let mut state = body_state([
        (first, Vec3::new(-1.5, 0.0, 0.0), first_body),
        (second, Vec3::new(1.5, 0.0, 0.0), second_body),
    ]);
    let scene = empty_scene();
    let mut service = RigidBodyService::default();
    let mut saw_pair = false;
    for _ in 0..30 {
        let receipt = service.step(&mut state, &scene, no_gravity()).unwrap();
        saw_pair |= receipt
            .contacts
            .iter()
            .any(|contact| contact.second.is_some());
    }
    assert!(saw_pair);

    let mut filtered_first = first_body;
    filtered_first.collision_groups = 1;
    filtered_first.collision_mask = 1;
    let mut filtered_second = second_body;
    filtered_second.collision_groups = 2;
    filtered_second.collision_mask = 2;
    let mut filtered = body_state([
        (first, Vec3::new(-1.5, 0.0, 0.0), filtered_first),
        (second, Vec3::new(1.5, 0.0, 0.0), filtered_second),
    ]);
    let mut filtered_service = RigidBodyService::default();
    for _ in 0..30 {
        assert!(filtered_service
            .step(&mut filtered, &scene, no_gravity())
            .unwrap()
            .contacts
            .is_empty());
    }
    assert!(filtered.transform(first).unwrap().translation.x > 0.0);
    assert!(filtered.transform(second).unwrap().translation.x < 0.0);

    let sleeper = EntityId::new(22);
    let mut sleeping_body =
        RigidBodyComponent::dynamic(RigidBodyShape::Sphere { radius: 0.5 }, 1.0);
    sleeping_body.sleeping = true;
    let mut sleeping = body_state([(sleeper, Vec3::ZERO, sleeping_body)]);
    let mut sleep_service = RigidBodyService::default();
    let idle = sleep_service
        .step(&mut sleeping, &scene, no_gravity())
        .unwrap();
    assert!(idle.facts[0].sleeping_after);
    let mut wake = no_gravity();
    wake.actions
        .push(RigidBodyAction::impulse(sleeper, Vec3::new(1.0, 0.0, 0.0)));
    let woke = sleep_service.step(&mut sleeping, &scene, wake).unwrap();
    assert_eq!(woke.woken_bodies, 1);
    assert!(!woke.facts[0].sleeping_after);
}

#[test]
fn prepared_steps_reject_stale_slots_without_publishing_derived_state() {
    let entity = EntityId::new(30);
    let body = RigidBodyComponent::dynamic(RigidBodyShape::Sphere { radius: 0.5 }, 1.0);
    let mut state = body_state([(entity, Vec3::new(0.0, 2.0, 0.0), body)]);
    let scene = empty_scene();
    let mut service = RigidBodyService::default();
    let prepared = service
        .prepare(&state, &scene, RigidBodyStepRequest::single(1.0 / 60.0))
        .unwrap();
    let slot = state
        .component_revision::<TransformComponent>(entity)
        .unwrap();
    EntityAuthoringService
        .replace_component(
            &mut state,
            slot,
            entity,
            TransformComponent::from_transform(EntityTransform::at(Vec3::new(5.0, 2.0, 0.0))),
        )
        .unwrap();
    let bytes_before = encode_snapshot(&state).unwrap();

    assert!(matches!(
        service.commit(&mut state, &scene, prepared),
        Err(RigidBodyStepError::Publication(
            RigidBodyStatePublicationError::StaleTransform { entity: stale, .. }
        )) if stale == entity
    ));
    assert_eq!(encode_snapshot(&state).unwrap(), bytes_before);
    assert!(service.readout().is_none());
}

#[test]
fn prepared_steps_reject_changed_lock_facts_without_publishing_derived_state() {
    let entity = EntityId::new(31);
    let body = RigidBodyComponent::dynamic(RigidBodyShape::Sphere { radius: 0.5 }, 1.0);
    let mut state = body_state([(entity, Vec3::new(0.0, 2.0, 0.0), body)]);
    let scene = empty_scene();
    let mut service = RigidBodyService::default();
    let prepared = service
        .prepare(&state, &scene, RigidBodyStepRequest::single(1.0 / 60.0))
        .unwrap();
    let slot = state
        .component_revision::<RigidBodyComponent>(entity)
        .unwrap();
    let mut changed = *state.rigid_body(entity).unwrap();
    changed.locked_translation_axes = [false, true, false];
    EntityAuthoringService
        .replace_component(&mut state, slot, entity, changed)
        .unwrap();
    let bytes_before = encode_snapshot(&state).unwrap();

    assert!(matches!(
        service.commit(&mut state, &scene, prepared),
        Err(RigidBodyStepError::Publication(
            RigidBodyStatePublicationError::StaleRigidBody { entity: stale, .. }
        )) if stale == entity
    ));
    assert_eq!(encode_snapshot(&state).unwrap(), bytes_before);
    assert!(service.readout().is_none());
}

#[test]
fn prepared_steps_bind_relationship_and_body_set_authority() {
    let body_entity = EntityId::new(32);
    let parent_entity = EntityId::new(33);
    let body = RigidBodyComponent::dynamic(RigidBodyShape::Sphere { radius: 0.5 }, 1.0);
    let mut state = EntityState::from_definitions([
        EntityDefinition::new(body_entity, "body").with_transform(Vec3::new(0.0, 2.0, 0.0)),
        EntityDefinition::new(parent_entity, "parent").with_transform(Vec3::ZERO),
    ])
    .unwrap();
    let body_slot = state
        .component_revision::<RigidBodyComponent>(body_entity)
        .unwrap();
    EntityAuthoringService
        .attach_component(&mut state, body_slot, body_entity, body)
        .unwrap();
    let scene = empty_scene();
    let mut service = RigidBodyService::default();
    let prepared = service
        .prepare(&state, &scene, RigidBodyStepRequest::single(1.0 / 60.0))
        .unwrap();
    state
        .apply_relationship(
            state.revision(),
            RelationshipCommand::SetTransformParent {
                child: body_entity,
                parent: parent_entity,
                mode: TransformParentMode::KeepLocal,
            },
        )
        .unwrap();
    let bytes_before = encode_snapshot(&state).unwrap();

    assert!(matches!(
        service.commit(&mut state, &scene, prepared),
        Err(RigidBodyStepError::StaleRelationship { entity }) if entity == body_entity
    ));
    assert_eq!(encode_snapshot(&state).unwrap(), bytes_before);
    assert!(service.readout().is_none());

    let first = EntityId::new(34);
    let added = EntityId::new(35);
    let mut body_set = EntityState::from_definitions([
        EntityDefinition::new(first, "first body").with_transform(Vec3::new(0.0, 2.0, 0.0)),
        EntityDefinition::new(added, "later body").with_transform(Vec3::new(2.0, 2.0, 0.0)),
    ])
    .unwrap();
    let first_slot = body_set
        .component_revision::<RigidBodyComponent>(first)
        .unwrap();
    EntityAuthoringService
        .attach_component(&mut body_set, first_slot, first, body)
        .unwrap();
    let prepared = service
        .prepare(&body_set, &scene, RigidBodyStepRequest::single(1.0 / 60.0))
        .unwrap();
    let added_slot = body_set
        .component_revision::<RigidBodyComponent>(added)
        .unwrap();
    EntityAuthoringService
        .attach_component(&mut body_set, added_slot, added, body)
        .unwrap();
    let bytes_before = encode_snapshot(&body_set).unwrap();

    assert!(matches!(
        service.commit(&mut body_set, &scene, prepared),
        Err(RigidBodyStepError::StaleBodySet)
    ));
    assert_eq!(encode_snapshot(&body_set).unwrap(), bytes_before);
    assert!(service.readout().is_none());
}

#[test]
fn prepared_steps_reject_static_environment_replacement_without_entity_mutation() {
    let entity = EntityId::new(31);
    let body = RigidBodyComponent::dynamic(RigidBodyShape::Sphere { radius: 0.5 }, 1.0);
    let mut state = body_state([(entity, Vec3::new(0.0, 2.0, 0.0), body)]);
    let mut scene = empty_scene();
    let mut service = RigidBodyService::default();
    let prepared = service
        .prepare(&state, &scene, RigidBodyStepRequest::single(1.0 / 60.0))
        .unwrap();
    let asset = StaticMeshColliderAsset::new(
        StaticMeshAssetId(99),
        vec![[-1.0, 0.0, -1.0], [1.0, 0.0, -1.0], [0.0, 0.0, 1.0]],
        vec![[0, 2, 1]],
    )
    .unwrap();
    let hash = asset.geometry_hash;
    scene
        .replace_static_mesh_colliders(
            0,
            [asset],
            [StaticMeshColliderInstance {
                id: StaticMeshInstanceId(99),
                asset: StaticMeshAssetId(99),
                expected_geometry_hash: hash,
                transform: StaticMeshTransform::IDENTITY,
            }],
        )
        .unwrap();
    let bytes_before = encode_snapshot(&state).unwrap();

    assert!(matches!(
        service.commit(&mut state, &scene, prepared),
        Err(RigidBodyStepError::StaleEnvironment)
    ));
    assert_eq!(encode_snapshot(&state).unwrap(), bytes_before);
    assert!(service.readout().is_none());
}

#[test]
fn snapshot_reopen_continues_with_rebuilt_derived_world_deterministically() {
    let entity = EntityId::new(40);
    let mut body = RigidBodyComponent::dynamic(
        RigidBodyShape::Cuboid {
            half_extents: Vec3::splat(0.5),
        },
        2.0,
    );
    body.linear_velocity = Vec3::new(0.5, 0.0, 0.25);
    body.locked_translation_axes = [false, true, false];
    body.locked_rotation_axes = [true, false, true];
    let mut original = body_state([(entity, Vec3::new(0.0, 4.0, 0.0), body)]);
    let scene = empty_scene();
    let mut first_service = RigidBodyService::default();
    for _ in 0..4 {
        first_service
            .step(
                &mut original,
                &scene,
                RigidBodyStepRequest::single(1.0 / 60.0),
            )
            .unwrap();
    }
    let bytes = encode_snapshot(&original).unwrap();
    let mut reopened = decode_snapshot(&bytes).unwrap();
    let mut rebuilt_service = RigidBodyService::default();

    let original_receipt = first_service
        .step(
            &mut original,
            &scene,
            RigidBodyStepRequest::single(1.0 / 60.0),
        )
        .unwrap();
    let reopened_receipt = rebuilt_service
        .step(
            &mut reopened,
            &scene,
            RigidBodyStepRequest::single(1.0 / 60.0),
        )
        .unwrap();
    assert_eq!(
        encode_snapshot(&original).unwrap(),
        encode_snapshot(&reopened).unwrap()
    );
    assert_eq!(original_receipt.facts, reopened_receipt.facts);
    assert_eq!(original_receipt.contacts, reopened_receipt.contacts);
}

#[test]
fn bounded_step_and_motion_admission_reject_before_mutation() {
    let entity = EntityId::new(50);
    let mut body = RigidBodyComponent::dynamic(RigidBodyShape::Sphere { radius: 0.5 }, 1.0);
    body.linear_velocity = Vec3::new(100.0, 0.0, 0.0);
    let mut state = body_state([(entity, Vec3::ZERO, body)]);
    let scene = empty_scene();
    let mut service = RigidBodyService::default();
    let bytes_before = encode_snapshot(&state).unwrap();
    let excessive = RigidBodyStepRequest {
        step_seconds: 1.0 / 15.0,
        steps: 1,
        gravity: Vec3::ZERO,
        actions: Vec::new(),
    };
    assert!(matches!(
        service.step(&mut state, &scene, excessive),
        Err(RigidBodyStepError::Backend(
            svc_collision::DynamicsError::MotionLimitExceeded { .. }
        ))
    ));
    assert_eq!(encode_snapshot(&state).unwrap(), bytes_before);

    let too_many_steps = RigidBodyStepRequest {
        step_seconds: 1.0 / 60.0,
        steps: svc_collision::MAX_DYNAMICS_STEPS + 1,
        gravity: Vec3::ZERO,
        actions: Vec::new(),
    };
    assert!(matches!(
        service.step(&mut state, &scene, too_many_steps),
        Err(RigidBodyStepError::Backend(
            svc_collision::DynamicsError::InvalidStepCount { .. }
        ))
    ));
    assert_eq!(encode_snapshot(&state).unwrap(), bytes_before);

    let mut ccd = body;
    ccd.continuous_collision = true;
    let body_slot = state
        .component_revision::<RigidBodyComponent>(entity)
        .unwrap();
    EntityAuthoringService
        .replace_component(&mut state, body_slot, entity, ccd)
        .unwrap();
    service
        .step(
            &mut state,
            &scene,
            RigidBodyStepRequest {
                step_seconds: 1.0 / 15.0,
                steps: 1,
                gravity: Vec3::ZERO,
                actions: Vec::new(),
            },
        )
        .expect("bounded high-speed CCD movement is explicitly admitted");
}

#[test]
fn body_and_action_quotas_reject_before_entity_mutation() {
    let body = RigidBodyComponent::dynamic(RigidBodyShape::Sphere { radius: 0.25 }, 1.0);
    let definitions = (0..=svc_collision::MAX_DYNAMICS_BODIES).map(|index| {
        (
            EntityId::new(index as u64 + 1),
            Vec3::new(index as f32, 0.0, 0.0),
            body,
        )
    });
    let mut state = body_state(definitions);
    let before = encode_snapshot(&state).unwrap();
    let mut service = RigidBodyService::default();
    assert!(matches!(
        service.step(&mut state, &empty_scene(), no_gravity()),
        Err(RigidBodyStepError::Backend(
            svc_collision::DynamicsError::TooManyBodies { .. }
        ))
    ));
    assert_eq!(encode_snapshot(&state).unwrap(), before);
    assert!(service.readout().is_none());

    let entity = EntityId::new(2_000);
    let mut one = body_state([(entity, Vec3::ZERO, body)]);
    let one_before = encode_snapshot(&one).unwrap();
    let mut request = no_gravity();
    request.actions =
        vec![RigidBodyAction::force(entity, Vec3::ZERO); svc_collision::MAX_DYNAMICS_ACTIONS + 1];
    assert!(matches!(
        service.step(&mut one, &empty_scene(), request),
        Err(RigidBodyStepError::Backend(
            svc_collision::DynamicsError::TooManyActions { .. }
        ))
    ));
    assert_eq!(encode_snapshot(&one).unwrap(), one_before);
    assert!(service.readout().is_none());
}

#[test]
fn bounded_stack_settles_without_interpenetrating_the_floor() {
    let body = RigidBodyComponent::dynamic(
        RigidBodyShape::Cuboid {
            half_extents: Vec3::splat(0.45),
        },
        1.0,
    );
    let entities = [EntityId::new(60), EntityId::new(61), EntityId::new(62)];
    let mut state = body_state(
        entities
            .into_iter()
            .enumerate()
            .map(|(index, entity)| (entity, Vec3::new(0.5, 1.55 + index as f32, 0.5), body)),
    );
    let scene = VoxelCollisionScene::from_solid_voxels(
        1.0,
        8,
        (-2..=2).flat_map(|x| (-2..=2).map(move |z| [x, 0, z])),
    )
    .unwrap();
    let mut service = RigidBodyService::default();
    for _ in 0..240 {
        service
            .step(&mut state, &scene, RigidBodyStepRequest::single(1.0 / 60.0))
            .unwrap();
    }
    let mut heights = entities
        .map(|entity| state.transform(entity).unwrap().translation.y)
        .to_vec();
    heights.sort_by(f32::total_cmp);
    assert!(
        heights[0] >= 1.43,
        "bottom body crossed voxel floor: {heights:?}"
    );
    assert!(
        heights.windows(2).all(|pair| pair[1] - pair[0] >= 0.84),
        "settled bodies substantially overlap: {heights:?}"
    );
}
