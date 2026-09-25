use core_ids::EntityId;
use core_math::{Vec2, Vec3};
use engine_spatial::{
    CharacterControllerCommand, CharacterControllerConfig, CharacterControllerError,
    CharacterControllerService, CharacterTetherRequest, VoxelCollisionScene,
};
use entity_state::{CharacterMotionComponent, EntityDefinition, EntityState};

fn scene() -> VoxelCollisionScene {
    VoxelCollisionScene::from_solid_voxels(1.0, 8, []).unwrap()
}
fn character(position: Vec3, velocity: Vec3) -> EntityState {
    let mut motion = CharacterMotionComponent::at_rest(position.y);
    motion.external_velocity = velocity;
    EntityState::from_definitions([EntityDefinition::new(EntityId::new(1), "character")
        .with_transform(position)
        .with_character_motion(motion)])
    .unwrap()
}
fn config() -> CharacterControllerConfig {
    let mut config = CharacterControllerConfig::default();
    config.vertical.gravity = 9.81;
    config.external_motion.external_decay_per_second = 0.0;
    config.external_motion.maximum_external_speed = 100.0;
    config.external_motion.authored_mass = 80.0;
    config.solver.maximum_displacement_per_step = 2.0;
    config
}
fn command(sequence: u64, tether: Option<CharacterTetherRequest>) -> CharacterControllerCommand {
    CharacterControllerCommand {
        tether,
        ..CharacterControllerCommand::idle(1.0 / 60.0, sequence)
    }
}

#[test]
fn fixed_anchor_swing_repeats_without_energy_growth_and_release_retains_momentum() {
    let run = || {
        let start = Vec3::new(2.4, -1.8, 0.0);
        let mut entities = character(start, Vec3::ZERO);
        let mut service = CharacterControllerService::default();
        let scene = scene();
        let config = config();
        let rope = CharacterTetherRequest::fixed(1, Vec3::ZERO, 3.0);
        let mut crossed = false;
        let mut caught = false;
        let mut last_velocity = Vec3::ZERO;
        for tick in 1..=240 {
            let receipt = service
                .step(
                    &mut entities,
                    &scene,
                    EntityId::new(1),
                    &config,
                    command(tick, Some(rope)),
                )
                .unwrap();
            last_velocity =
                receipt.motion_after.controlled_velocity + receipt.motion_after.external_velocity;
            let energy = 0.5 * last_velocity.length_squared()
                + config.vertical.gravity * receipt.transform_after.translation.y;
            assert!(
                energy <= config.vertical.gravity * start.y + 0.02,
                "energy {energy}"
            );
            assert!(receipt.tether.distance <= 3.0011, "{:?}", receipt.tether);
            assert!(!receipt.tether.unresolved);
            crossed |= receipt.transform_after.translation.x < -1.0;
            caught |= receipt.tether.caught;
        }
        assert!(crossed && caught);
        let released = service
            .step(
                &mut entities,
                &scene,
                EntityId::new(1),
                &config,
                command(241, None),
            )
            .unwrap();
        assert!(released.tether.released);
        let after =
            released.motion_after.controlled_velocity + released.motion_after.external_velocity;
        assert!((after.x - last_velocity.x).abs() < 1e-5);
        assert!((after.y - (last_velocity.y - config.vertical.gravity / 60.0)).abs() < 1e-5);
        (released.transform_after, released.motion_after)
    };
    assert_eq!(run(), run());
}

#[test]
fn high_speed_catch_is_inelastic_and_planar_input_preserves_inertial_tangent() {
    let mut entities = character(Vec3::new(0.0, -2.9, 0.0), Vec3::new(15.0, -25.0, 0.0));
    let mut service = CharacterControllerService::default();
    let scene = scene();
    let mut config = config();
    config.vertical.gravity = 0.0;
    let mut input = command(1, Some(CharacterTetherRequest::fixed(1, Vec3::ZERO, 3.0)));
    input.planar_intent = Vec2::new(1.0, 0.0);
    let result = service
        .step(&mut entities, &scene, EntityId::new(1), &config, input)
        .unwrap();
    assert!(result.tether.caught);
    assert!(result.tether.distance <= 3.001);
    assert!(result.tether.tangential_velocity.length() > 10.0);
    assert!(
        (result.motion_after.controlled_velocity + result.motion_after.external_velocity)
            .length_squared()
            <= 15.0 * 15.0 + 25.0 * 25.0 + 1.0
    );
    input.sequence = 2;
    input.planar_intent = Vec2::ZERO;
    let second = service
        .step(&mut entities, &scene, EntityId::new(1), &config, input)
        .unwrap();
    assert!(second.tether.tangential_velocity.length() > 10.0);
}

#[test]
fn dynamic_anchor_saturates_both_sides_and_reports_unresolved_distance() {
    let mut entities = character(Vec3::new(0.0, -3.0, 0.0), Vec3::new(0.0, -20.0, 0.0));
    let mut config = config();
    config.vertical.gravity = 0.0;
    config.external_motion.maximum_dynamic_impulse = 80.0;
    let mut rope = CharacterTetherRequest::fixed(1, Vec3::ZERO, 3.0);
    rope.anchor_id = 7;
    rope.anchor_velocity = Vec3::new(1.0, 0.0, 0.0);
    let result = CharacterControllerService::default()
        .step(
            &mut entities,
            &scene(),
            EntityId::new(1),
            &config,
            command(1, Some(rope)),
        )
        .unwrap();
    assert!(result.tether.saturated && result.tether.unresolved);
    assert!(result.tether.reaction_impulse.length() <= 80.001);
    let velocity_change = result.motion_after.external_velocity - Vec3::new(0.0, -20.0, 0.0);
    assert!((velocity_change * 80.0 + result.tether.reaction_impulse).length() < 0.001);
}

#[test]
fn invalid_attachment_and_stale_step_do_not_mutate_character() {
    let mut entities = character(Vec3::new(0.0, -4.0, 0.0), Vec3::ZERO);
    let mut service = CharacterControllerService::default();
    let scene = scene();
    let config = config();
    let before = entities.revision();
    assert!(matches!(
        service.step(
            &mut entities,
            &scene,
            EntityId::new(1),
            &config,
            command(1, Some(CharacterTetherRequest::fixed(1, Vec3::ZERO, 3.0)))
        ),
        Err(CharacterControllerError::TetherOutOfReach)
    ));
    assert_eq!(before, entities.revision());
    let rope = CharacterTetherRequest::fixed(1, Vec3::ZERO, 4.0);
    let prepared = service
        .prepare(
            &entities,
            &scene,
            EntityId::new(1),
            &config,
            command(1, Some(rope)),
        )
        .unwrap();
    service
        .step(
            &mut entities,
            &scene,
            EntityId::new(1),
            &config,
            command(1, Some(rope)),
        )
        .unwrap();
    let committed = entities.revision();
    assert!(service.commit(&mut entities, &scene, prepared).is_err());
    assert_eq!(committed, entities.revision());
    let invalid = CharacterTetherRequest {
        anchor_valid: false,
        ..rope
    };
    let result = service
        .step(
            &mut entities,
            &scene,
            EntityId::new(1),
            &config,
            command(2, Some(invalid)),
        )
        .unwrap();
    assert!(result.tether.invalidated && !result.motion_after.tether_attached);
}

#[test]
fn loaded_reeling_is_rate_limited_and_snapshot_restores_attachment() {
    let mut entities = character(Vec3::new(0.0, -3.0, 0.0), Vec3::ZERO);
    let mut service = CharacterControllerService::default();
    let scene = scene();
    let config = config();
    let mut rope = CharacterTetherRequest::fixed(1, Vec3::ZERO, 3.0);
    rope.target_length = 2.0;
    rope.reel_speed = 0.25;
    for tick in 1..=120 {
        let receipt = service
            .step(
                &mut entities,
                &scene,
                EntityId::new(1),
                &config,
                command(tick, Some(rope)),
            )
            .unwrap();
        assert!((receipt.tether.maximum_length - (3.0 - tick as f32 * 0.25 / 60.0)).abs() < 1e-4);
        assert!(!receipt.tether.unresolved);
        assert!(receipt.motion_after.external_velocity.length() < 1.0);
    }
    let mut reopened =
        entity_state::decode_snapshot(&entity_state::encode_snapshot(&entities).unwrap()).unwrap();
    rope.target_length = 4.0;
    let next = command(121, Some(rope));
    let continuous = service
        .step(&mut entities, &scene, EntityId::new(1), &config, next)
        .unwrap();
    let restored = CharacterControllerService::default()
        .step(&mut reopened, &scene, EntityId::new(1), &config, next)
        .unwrap();
    assert_eq!(continuous.tether, restored.tether);
    assert_eq!(continuous.motion_after, restored.motion_after);
    assert!(continuous.tether.maximum_length > 2.5);
}

#[test]
fn infeasible_rope_correction_stops_at_terrain_instead_of_teleporting() {
    let wall = (-8..=8).flat_map(|y| (-2..=2).map(move |z| [0, y, z]));
    let scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, wall).unwrap();
    let mut entities = character(Vec3::new(1.5, 2.0, 0.0), Vec3::ZERO);
    let mut service = CharacterControllerService::default();
    let config = config();
    let mut rope = CharacterTetherRequest::fixed(1, Vec3::new(-2.0, 2.0, 0.0), 4.0);
    rope.target_length = 2.0;
    rope.reel_speed = 0.25;
    let mut unresolved = false;
    for tick in 1..=300 {
        let result = service
            .step(
                &mut entities,
                &scene,
                EntityId::new(1),
                &config,
                command(tick, Some(rope)),
            )
            .unwrap();
        assert!(
            result.transform_after.translation.x >= 1.0 + config.shape.radius - 0.002,
            "{:?}",
            result.transform_after
        );
        assert!(result.displacement.length() <= config.solver.maximum_displacement_per_step);
        assert!(result.cast_count <= config.solver.maximum_queries_per_step);
        unresolved |= result.tether.unresolved;
    }
    assert!(unresolved);
}

#[test]
fn slack_ground_controls_jump_and_ledge_departure_keep_the_same_controller() {
    let ground = (-2..=2).flat_map(|x| (-2..=2).map(move |z| [x, 0, z]));
    let scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, ground).unwrap();
    let config = config();
    let mut entities = character(Vec3::new(0.0, 2.0, 0.0), Vec3::ZERO);
    let mut service = CharacterControllerService::default();
    let rope = CharacterTetherRequest::fixed(1, Vec3::new(0.0, 5.0, 0.0), 6.0);
    let mut grounded = false;
    let mut airborne = false;
    let mut caught = false;
    for tick in 1..=180 {
        let mut command = command(tick, Some(rope));
        if tick <= 60 {
            command.planar_intent = Vec2::new(1.0, 0.0);
        }
        let result = service
            .step(&mut entities, &scene, EntityId::new(1), &config, command)
            .unwrap();
        if result.motion_after.grounded {
            grounded = true;
            assert!(
                result.motion_after.controlled_velocity.x <= config.ground.strafe_speed + 0.001
            );
        } else if grounded {
            airborne = true;
        }
        caught |= result.tether.caught;
        assert!(result.tether.distance <= 6.0011);
    }
    assert!(grounded && airborne && caught);

    let mut entities = character(Vec3::new(0.0, 2.0, 0.0), Vec3::ZERO);
    let mut service = CharacterControllerService::default();
    let settled = service
        .step(
            &mut entities,
            &scene,
            EntityId::new(1),
            &config,
            command(1, Some(rope)),
        )
        .unwrap();
    assert!(settled.motion_after.grounded);
    let mut jump = command(2, Some(rope));
    jump.jump_pressed = true;
    let jumped = service
        .step(&mut entities, &scene, EntityId::new(1), &config, jump)
        .unwrap();
    assert!(!jumped.motion_after.grounded);
    assert!(jumped.motion_after.external_velocity.y > 0.0);
}

#[test]
fn repeated_attach_and_release_never_adds_unrequested_energy() {
    let mut entities = character(Vec3::new(0.0, -3.0, 0.0), Vec3::new(5.0, 0.0, 0.0));
    let mut service = CharacterControllerService::default();
    let mut config = config();
    config.vertical.gravity = 0.0;
    let scene = scene();
    for sequence in 1..=40 {
        let position = entities.transform(EntityId::new(1)).unwrap().translation;
        let rope = (sequence % 2 == 1)
            .then(|| CharacterTetherRequest::fixed(sequence, Vec3::ZERO, position.length()));
        let result = service
            .step(
                &mut entities,
                &scene,
                EntityId::new(1),
                &config,
                command(sequence, rope),
            )
            .unwrap();
        let velocity =
            result.motion_after.controlled_velocity + result.motion_after.external_velocity;
        assert!(velocity.length_squared() <= 25.001);
    }
}

#[test]
fn slack_tether_preserves_platform_carry_and_jump_departure_velocity() {
    let scene = scene();
    let entity = EntityId::new(1);
    let platform = EntityId::new(2);
    let mut entities = EntityState::from_definitions([
        EntityDefinition::new(entity, "character")
            .with_transform(Vec3::new(0.0, 1.9, 0.0))
            .with_character_motion(CharacterMotionComponent::at_rest(1.9)),
        EntityDefinition::new(platform, "platform")
            .with_transform(Vec3::new(0.0, 0.75, 0.0))
            .with_bounds(Vec3::new(-1.0, -0.25, -1.0), Vec3::new(1.0, 0.25, 1.0))
            .with_collision(true, false),
    ])
    .unwrap();
    let config = config();
    let mut service = CharacterControllerService::default();
    let rope = CharacterTetherRequest::fixed(1, Vec3::new(0.0, 5.0, 0.0), 6.0);
    let first = service
        .step(
            &mut entities,
            &scene,
            entity,
            &config,
            command(1, Some(rope)),
        )
        .unwrap();
    assert_eq!(first.motion_after.support_entity, Some(platform));
    entities
        .apply_transform(
            entities.revision(),
            entity_state::TransformCommand::Translate {
                entity: platform,
                delta: Vec3::new(0.2, 0.0, 0.0),
            },
        )
        .unwrap();
    let carried = service
        .step(
            &mut entities,
            &scene,
            entity,
            &config,
            command(2, Some(rope)),
        )
        .unwrap();
    assert!(carried.displacement.x > 0.19);
    let support_speed = carried.platform.unwrap().point_velocity.x;
    assert!(support_speed > 0.0);
    entities
        .apply_transform(
            entities.revision(),
            entity_state::TransformCommand::Translate {
                entity: platform,
                delta: Vec3::new(0.2, 0.0, 0.0),
            },
        )
        .unwrap();
    let mut jump = command(3, Some(rope));
    jump.jump_pressed = true;
    let departed = service
        .step(&mut entities, &scene, entity, &config, jump)
        .unwrap();
    assert!(!departed.motion_after.grounded);
    assert!(departed.platform.unwrap().departed);
    assert!(departed.motion_after.external_velocity.x >= support_speed - 0.001);
}

#[test]
fn local_anchor_edit_readmits_reach_and_release_keeps_endpoint_identity() {
    let mut entities = character(Vec3::new(0.0, -3.0, 0.0), Vec3::ZERO);
    let mut service = CharacterControllerService::default();
    let mut config = config();
    config.vertical.gravity = 0.0;
    let scene = scene();
    let mut rope = CharacterTetherRequest::fixed(1, Vec3::new(0.0, 1.0, 0.0), 3.0);
    rope.local_anchor.y = 1.0;
    let attached = service
        .step(
            &mut entities,
            &scene,
            EntityId::new(1),
            &config,
            command(1, Some(rope)),
        )
        .unwrap();
    let revision = entities.revision();
    let moved_anchor = CharacterTetherRequest {
        local_anchor: Vec3::new(0.0, -2.0, 0.0),
        ..rope
    };
    assert!(matches!(
        service.step(
            &mut entities,
            &scene,
            EntityId::new(1),
            &config,
            command(2, Some(moved_anchor))
        ),
        Err(CharacterControllerError::TetherOutOfReach)
    ));
    assert_eq!(entities.revision(), revision);
    let released = service
        .step(
            &mut entities,
            &scene,
            EntityId::new(1),
            &config,
            command(2, None),
        )
        .unwrap();
    assert!(released.tether.released);
    assert_eq!(
        released.tether.character_point,
        attached.tether.character_point
    );
    assert_eq!(released.tether.anchor_point, attached.tether.anchor_point);
    assert_eq!(
        released.tether.maximum_length,
        attached.tether.maximum_length
    );
}

#[test]
fn floor_snap_does_not_launch_a_reeling_character() {
    let mut entities = character(Vec3::new(0.0, 3.911, -7.0), Vec3::ZERO);
    let mut service = CharacterControllerService::default();
    let scene = VoxelCollisionScene::from_solid_voxels(
        1.0,
        8,
        (-12..=12).flat_map(|x| (-12..=12).map(move |z| [x, 2, z])),
    )
    .unwrap();
    let mut config = config();
    config.vertical.gravity = 24.0;
    config.shape.standing_height = 1.75;
    config.shape.radius = 0.3;
    config.shape.contact_skin = 0.015;
    config.surface.floor_snap_distance = 0.25;
    config.surface.floor_snap_speed_limit = 10.0;
    config.external_motion.maximum_external_speed = 20.0;
    let mut rope = CharacterTetherRequest::fixed(1, Vec3::new(0.0, 11.0, -2.0), 8.975);
    rope.target_length = 4.0;
    rope.reel_speed = 0.25;
    let mut catches = 0;
    let mut max_speed = 0.0_f32;
    for tick in 1..=2400 {
        let mut input = command(tick, Some(rope));
        input.step_seconds = 1.0 / 120.0;
        let receipt = service
            .step(&mut entities, &scene, EntityId::new(1), &config, input)
            .unwrap();
        max_speed = max_speed.max(
            (receipt.motion_after.controlled_velocity + receipt.motion_after.external_velocity)
                .length(),
        );
        if tick > 1200 {
            catches += u32::from(receipt.tether.caught);
        }
    }
    assert!(
        max_speed < 5.0,
        "floor adhesion injected a launch: {max_speed}"
    );
    assert_eq!(
        catches, 0,
        "a loaded pendulum should not repeatedly lose taut state"
    );
}
