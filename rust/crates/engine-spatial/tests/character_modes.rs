use core_ids::EntityId;
use core_math::{Vec2, Vec3};
use engine_spatial::{
    CharacterControllerCommand, CharacterControllerConfig, CharacterControllerService,
    CharacterMovementMode, CharacterMovementRequest, VoxelCollisionScene,
};
use entity_state::{CharacterMotionComponent, EntityDefinition, EntityState};

fn state(position: Vec3) -> (EntityId, EntityState) {
    let id = EntityId::new(1);
    (
        id,
        EntityState::from_definitions([EntityDefinition::new(id, "swimmer")
            .with_transform(position)
            .with_character_motion(CharacterMotionComponent::at_rest(position.y))])
        .unwrap(),
    )
}
fn water() -> CharacterMovementRequest {
    CharacterMovementRequest {
        mode: CharacterMovementMode::Swimming,
        minimum: Vec3::new(-5.0, -5.0, -5.0),
        maximum: Vec3::new(5.0, 3.0, 5.0),
        speed: 3.0,
        acceleration: 10.0,
        drag: 2.0,
        gravity_scale: 1.0,
        buoyancy: 1.0,
        ..Default::default()
    }
}

#[test]
fn submerged_character_floats_swims_and_reports_breathing_facts() {
    let scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, []).unwrap();
    let (id, mut entities) = state(Vec3::ZERO);
    let mut service = CharacterControllerService::default();
    for seq in 1..=120 {
        let cmd = CharacterControllerCommand {
            movement: water(),
            ..CharacterControllerCommand::idle(1.0 / 60.0, seq)
        };
        let receipt = service
            .step(
                &mut entities,
                &scene,
                id,
                &CharacterControllerConfig::default(),
                cmd,
            )
            .unwrap();
        assert!(receipt.movement.head_submerged);
        assert!(receipt.transform_after.translation.y.abs() < 0.001);
    }
    let cmd = CharacterControllerCommand {
        movement: CharacterMovementRequest {
            vertical_intent: 1.0,
            ..water()
        },
        planar_intent: Vec2::new(1.0, 0.0),
        ..CharacterControllerCommand::idle(1.0 / 60.0, 121)
    };
    let receipt = service
        .step(
            &mut entities,
            &scene,
            id,
            &CharacterControllerConfig::default(),
            cmd,
        )
        .unwrap();
    assert!(receipt.displacement.y > 0.0);
    assert!(receipt.displacement.x > 0.0);
}

#[test]
fn water_exit_returns_to_air_gravity() {
    let scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, []).unwrap();
    let (id, mut entities) = state(Vec3::new(8.0, 0.0, 0.0));
    let r = CharacterControllerService::default()
        .step(
            &mut entities,
            &scene,
            id,
            &CharacterControllerConfig::default(),
            CharacterControllerCommand {
                movement: water(),
                ..CharacterControllerCommand::idle(1.0 / 60.0, 1)
            },
        )
        .unwrap();
    assert_eq!(r.movement.mode, CharacterMovementMode::Walking);
    assert!(!r.movement.head_submerged);
    assert!(r.displacement.y < 0.0);
}

#[test]
fn climb_rail_is_bounded_and_release_falls() {
    let scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, []).unwrap();
    let (id, mut entities) = state(Vec3::new(0.0, 1.0, 0.0));
    let mut service = CharacterControllerService::default();
    let climb = CharacterMovementRequest {
        mode: CharacterMovementMode::Climbing,
        minimum: Vec3::ZERO,
        maximum: Vec3::new(0.0, 3.0, 0.0),
        vertical_intent: 1.0,
        speed: 3.0,
        climb_reach: 0.5,
        ..Default::default()
    };
    for seq in 1..=120 {
        let r = service
            .step(
                &mut entities,
                &scene,
                id,
                &CharacterControllerConfig::default(),
                CharacterControllerCommand {
                    movement: climb,
                    ..CharacterControllerCommand::idle(1.0 / 60.0, seq)
                },
            )
            .unwrap();
        assert!(r.movement.climb_attached);
        assert!(r.transform_after.translation.y <= 3.001);
    }
    let r = service
        .step(
            &mut entities,
            &scene,
            id,
            &CharacterControllerConfig::default(),
            CharacterControllerCommand::idle(1.0 / 60.0, 121),
        )
        .unwrap();
    assert!(r.displacement.y < 0.0);
}

#[test]
fn flight_and_climbing_use_collision_sweep() {
    let scene = VoxelCollisionScene::from_solid_voxels(
        1.0,
        8,
        (-2..=2).flat_map(|x| (-2..=2).map(move |z| [x, 3, z])),
    )
    .unwrap();
    for mode in [
        CharacterMovementMode::Flying,
        CharacterMovementMode::Climbing,
    ] {
        let (id, mut entities) = state(Vec3::new(0.0, 1.0, 0.0));
        let mut service = CharacterControllerService::default();
        for seq in 1..=120 {
            let r = service
                .step(
                    &mut entities,
                    &scene,
                    id,
                    &CharacterControllerConfig::default(),
                    CharacterControllerCommand {
                        movement: CharacterMovementRequest {
                            mode,
                            minimum: Vec3::ZERO,
                            maximum: Vec3::new(0.0, 8.0, 0.0),
                            vertical_intent: 1.0,
                            speed: 3.0,
                            acceleration: 20.0,
                            climb_reach: 0.5,
                            ..Default::default()
                        },
                        ..CharacterControllerCommand::idle(1.0 / 60.0, seq)
                    },
                )
                .unwrap();
            assert!(r.transform_after.translation.y < 2.2);
        }
    }
}

#[test]
fn buoyancy_settles_at_surface_without_floor_snap() {
    let scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, []).unwrap();
    let (id, mut entities) = state(Vec3::new(0.0, 1.0, 0.0));
    let mut service = CharacterControllerService::default();
    let movement = CharacterMovementRequest {
        acceleration: 0.0,
        buoyancy: 2.0,
        drag: 4.0,
        ..water()
    };
    for sequence in 1..=900 {
        let receipt = service
            .step(
                &mut entities,
                &scene,
                id,
                &CharacterControllerConfig::default(),
                CharacterControllerCommand {
                    movement,
                    ..CharacterControllerCommand::idle(1.0 / 60.0, sequence)
                },
            )
            .unwrap();
        assert!(receipt.transform_after.translation.y.is_finite());
        if sequence > 800 {
            assert!((receipt.transform_after.translation.y - 3.0).abs() < 0.08);
            assert!(!receipt.movement.head_submerged);
            assert!(!receipt.motion_after.grounded);
        }
    }
}

#[test]
fn modes_reject_invalid_environment_before_moving() {
    let config = CharacterControllerConfig::default();
    for movement in [
        CharacterMovementRequest {
            maximum: Vec3::ZERO,
            minimum: Vec3::ZERO,
            ..water()
        },
        CharacterMovementRequest {
            mode: CharacterMovementMode::Climbing,
            minimum: Vec3::ZERO,
            maximum: Vec3::new(1.0, 4.0, 0.0),
            climb_reach: 1.0,
            ..Default::default()
        },
        CharacterMovementRequest {
            speed: f32::NAN,
            ..water()
        },
    ] {
        assert!(CharacterControllerCommand {
            movement,
            ..CharacterControllerCommand::idle(1.0 / 60.0, 1)
        }
        .validate_against(&config)
        .is_err());
    }
}

#[test]
fn crossing_water_boundary_activates_swimming_on_accepted_position() {
    let scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, []).unwrap();
    let (id, mut entities) = state(Vec3::new(-5.001, 0.0, 0.0));
    let mut service = CharacterControllerService::default();
    let config = CharacterControllerConfig::default();
    let entered = service
        .step(
            &mut entities,
            &scene,
            id,
            &config,
            CharacterControllerCommand {
                movement: water(),
                planar_intent: Vec2::new(1.0, 0.0),
                ..CharacterControllerCommand::idle(1.0 / 60.0, 1)
            },
        )
        .unwrap();
    assert_eq!(entered.movement.mode, CharacterMovementMode::Swimming);
    assert!(entered.movement.head_submerged);
    for seq in 2..=61 {
        let receipt = service
            .step(
                &mut entities,
                &scene,
                id,
                &config,
                CharacterControllerCommand {
                    movement: water(),
                    ..CharacterControllerCommand::idle(1.0 / 60.0, seq)
                },
            )
            .unwrap();
        assert_eq!(receipt.movement.mode, CharacterMovementMode::Swimming);
        assert!(receipt.transform_after.translation.y > -1.0);
    }
}
