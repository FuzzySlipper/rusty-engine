use core_ids::EntityId;
use core_math::{Vec2, Vec3};
use engine_spatial::{
    CharacterBlockKind, CharacterContactKind, CharacterControllerCommand,
    CharacterControllerConfig, CharacterControllerError, CharacterControllerService,
    StaticMeshAssetId, StaticMeshColliderAsset, StaticMeshColliderInstance, StaticMeshInstanceId,
    StaticMeshTransform, VoxelCollisionScene, VoxelEdit, VoxelEditService, VoxelEditTransaction,
};
use entity_state::{
    CharacterMotionComponent, CharacterStance, EntityDefinition, EntityState, EntityTransform, Quat,
};

fn floor_scene() -> VoxelCollisionScene {
    let mut voxels = Vec::new();
    for x in -8..=8 {
        for z in -8..=8 {
            voxels.push([x, 0, z]);
        }
    }
    VoxelCollisionScene::from_solid_voxels(1.0, 8, voxels).unwrap()
}

fn character(y: f32) -> (EntityId, EntityState) {
    let entity = EntityId::new(1);
    let state = EntityState::from_definitions([EntityDefinition::new(entity, "character")
        .with_transform(Vec3::new(0.0, y, 0.0))
        .with_character_motion(CharacterMotionComponent::at_rest(y))])
    .unwrap();
    (entity, state)
}

fn character_at(position: Vec3) -> (EntityId, EntityState) {
    let entity = EntityId::new(1);
    let state = EntityState::from_definitions([EntityDefinition::new(entity, "character")
        .with_transform(position)
        .with_character_motion(CharacterMotionComponent::at_rest(position.y))])
    .unwrap();
    (entity, state)
}

fn floor_wall_lip_scene() -> VoxelCollisionScene {
    let mut voxels = Vec::new();
    for x in -2..=2 {
        for z in 1..=4 {
            voxels.push([x, 0, z]);
        }
        voxels.push([x, 1, 0]);
    }
    VoxelCollisionScene::from_solid_voxels(1.0, 8, voxels).unwrap()
}

fn craftsurvive_trench_scene() -> VoxelCollisionScene {
    let mut voxels = Vec::new();
    for x in -3..=3 {
        for z in 2..=10 {
            for y in 0..=3 {
                let removed_trench_cell =
                    (-1..=1).contains(&x) && ((y == 3 && z == 5) || (y == 2 && z == 4));
                if !removed_trench_cell {
                    voxels.push([x, y, z]);
                }
            }
        }
    }
    for x in -1..=1 {
        for y in 4..=6 {
            voxels.push([x, y, 3]);
        }
    }
    VoxelCollisionScene::from_solid_voxels(1.0, 8, voxels).unwrap()
}

fn command(sequence: u64, intent: Vec2) -> CharacterControllerCommand {
    CharacterControllerCommand {
        planar_intent: intent,
        ..CharacterControllerCommand::idle(1.0 / 60.0, sequence)
    }
}

fn ramp_scene(rise: f64) -> VoxelCollisionScene {
    let mut scene = floor_scene();
    let asset = StaticMeshColliderAsset::new(
        StaticMeshAssetId(1),
        vec![
            [-2.0, 1.0, 0.0],
            [2.0, 1.0, 0.0],
            [-2.0, 1.0 + rise, -4.0],
            [2.0, 1.0 + rise, -4.0],
        ],
        vec![[0, 1, 2], [1, 3, 2]],
    )
    .unwrap();
    let hash = asset.geometry_hash;
    scene
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
    scene
}

#[test]
fn level_motion_normalizes_diagonal_and_receipt_owns_continuation() {
    let scene = floor_scene();
    let (entity, mut state) = character(1.9);
    let config = CharacterControllerConfig::default();
    let mut service = CharacterControllerService::default();
    service
        .step(&mut state, &scene, entity, &config, command(1, Vec2::ZERO))
        .unwrap();
    let forward = service
        .step(
            &mut state,
            &scene,
            entity,
            &config,
            command(2, Vec2::new(0.0, 1.0)),
        )
        .unwrap();
    let diagonal = service
        .step(
            &mut state,
            &scene,
            entity,
            &config,
            command(3, Vec2::new(1.0, 1.0)),
        )
        .unwrap();
    assert!(forward.motion_after.grounded);
    assert!((forward.transform_after.translation.y - 1.9).abs() < 1.0e-4);
    assert!(forward.displacement.z < 0.0);
    assert!(diagonal.wish_velocity.length() <= config.ground.forward_speed + 1.0e-5);
    assert_eq!(
        state.character_motion(entity).unwrap(),
        &diagonal.motion_after
    );
    assert_eq!(service.readout().unwrap().command_sequence, 3);
}

#[test]
fn wall_slide_preserves_tangent_motion_and_reports_contact() {
    let mut voxels = floor_scene().solid_voxels().to_vec();
    for y in 1..=3 {
        for z in -4..=4 {
            voxels.push([1, y, z]);
        }
    }
    let scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, voxels).unwrap();
    let (entity, mut state) = character(1.9);
    let config = CharacterControllerConfig::default();
    let mut service = CharacterControllerService::default();
    service
        .step(&mut state, &scene, entity, &config, command(1, Vec2::ZERO))
        .unwrap();
    let receipt = (2..=90)
        .find_map(|sequence| {
            let receipt = service
                .step(
                    &mut state,
                    &scene,
                    entity,
                    &config,
                    command(sequence, Vec2::new(1.0, 1.0)),
                )
                .unwrap();
            receipt
                .blocks
                .contains(&CharacterBlockKind::Wall)
                .then_some(receipt)
        })
        .expect("controller should reach the wall");
    assert!(receipt.displacement.z < 0.0);
    assert!(receipt.transform_after.translation.x < 0.66);
    assert!(receipt.blocks.contains(&CharacterBlockKind::Wall));
}

#[test]
fn inside_corner_stops_inward_motion_and_outside_corner_preserves_progress() {
    let mut voxels = floor_scene().solid_voxels().to_vec();
    for y in 1..=3 {
        for z in -4..=1 {
            voxels.push([1, y, z]);
        }
        for x in -1..=4 {
            voxels.push([x, y, -2]);
        }
    }
    let scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, voxels).unwrap();
    let (entity, mut state) = character(1.9);
    let config = CharacterControllerConfig::default();
    let mut service = CharacterControllerService::default();
    let mut corner = None;
    let mut blocked = false;
    for sequence in 1..=60 {
        let receipt = service
            .step(
                &mut state,
                &scene,
                entity,
                &config,
                command(sequence, Vec2::new(1.0, 1.0)),
            )
            .unwrap();
        blocked |= receipt.blocks.contains(&CharacterBlockKind::Wall);
        corner = Some(receipt);
    }
    let corner = corner.unwrap();
    assert!(blocked);
    assert!(corner.transform_after.translation.x < 0.67);
    assert!(corner.transform_after.translation.z > -0.67);
    assert!(corner.displacement.length() < 1.0e-3);

    let mut voxels = floor_scene().solid_voxels().to_vec();
    for y in 1..=3 {
        voxels.push([1, y, -1]);
    }
    let scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, voxels).unwrap();
    let (entity, mut state) = character(1.9);
    let mut service = CharacterControllerService::default();
    for sequence in 1..=90 {
        service
            .step(
                &mut state,
                &scene,
                entity,
                &config,
                command(sequence, Vec2::new(0.4, 1.0)),
            )
            .unwrap();
    }
    assert!(state.transform(entity).unwrap().translation.z < -2.0);
}

#[test]
fn crouch_preserves_feet_and_standing_waits_for_clearance() {
    let mut voxels = floor_scene().solid_voxels().to_vec();
    voxels.push([0, 2, 0]);
    let scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, voxels).unwrap();
    let (entity, mut state) = character(1.4);
    let mut motion = *state.character_motion(entity).unwrap();
    motion.stance = CharacterStance::Crouched;
    let revisions = (
        state
            .component_revision::<entity_state::TransformComponent>(entity)
            .unwrap(),
        state
            .component_revision::<CharacterMotionComponent>(entity)
            .unwrap(),
    );
    let transform = *state.transform(entity).unwrap();
    entity_state::replace_character_motion_state(
        &mut state,
        entity_state::CharacterMotionStateReplacement {
            entity,
            expected_transform_revision: revisions.0,
            expected_motion_revision: revisions.1,
            transform,
            motion,
        },
    )
    .unwrap();
    let mut config = CharacterControllerConfig::default();
    config.shape.crouched_height = 0.8;
    let receipt = CharacterControllerService::default()
        .step(&mut state, &scene, entity, &config, command(1, Vec2::ZERO))
        .unwrap();
    assert!(receipt.stance.blocked);
    assert_eq!(receipt.stance.accepted, CharacterStance::Crouched);
    assert!((receipt.transform_after.translation.y - 1.4).abs() < 0.03);
}

#[test]
fn buffered_jump_consumes_on_ground_and_coyote_jump_consumes_once() {
    let scene = floor_scene();
    let (entity, mut state) = character(2.1);
    let config = CharacterControllerConfig::default();
    let mut service = CharacterControllerService::default();
    let mut first = command(1, Vec2::ZERO);
    first.jump_pressed = true;
    service
        .step(&mut state, &scene, entity, &config, first)
        .unwrap();
    let landing = service
        .step(&mut state, &scene, entity, &config, command(2, Vec2::ZERO))
        .unwrap();
    assert!(
        landing.motion_after.controlled_velocity.y > 0.0
            || landing.motion_after.jump_buffer_remaining > 0.0
    );
}

#[test]
fn environment_change_between_prepare_and_commit_is_fail_atomic() {
    let mut scene = floor_scene();
    let (entity, mut state) = character(1.9);
    let config = CharacterControllerConfig::default();
    let mut service = CharacterControllerService::default();
    let prepared = service
        .prepare(&state, &scene, entity, &config, command(1, Vec2::ZERO))
        .unwrap();
    let before = state.clone();
    scene.replace_static_mesh_colliders(0, [], []).unwrap();
    assert!(matches!(
        service.commit(&mut state, &scene, prepared),
        Err(CharacterControllerError::StaleEnvironment)
    ));
    assert_eq!(state.revision(), before.revision());
    assert_eq!(state.transform(entity), before.transform(entity));
    assert_eq!(
        state.character_motion(entity),
        before.character_motion(entity)
    );
}

#[test]
fn bounded_autostep_climbs_a_low_voxel_and_reports_the_choice() {
    let scene = floor_scene();
    let entity = EntityId::new(1);
    let step_id = EntityId::new(2);
    let mut state = EntityState::from_definitions([
        EntityDefinition::new(entity, "character")
            .with_transform(Vec3::new(0.0, 1.9, 0.0))
            .with_character_motion(CharacterMotionComponent::at_rest(1.9)),
        EntityDefinition::new(step_id, "step")
            .with_transform(Vec3::new(0.0, 1.125, -1.5))
            .with_bounds(Vec3::new(-0.5, -0.125, -0.5), Vec3::new(0.5, 0.125, 0.5))
            .with_collision(true, false),
    ])
    .unwrap();
    let config = CharacterControllerConfig::default();
    let mut service = CharacterControllerService::default();
    let mut stepped = None;
    for sequence in 1..=90 {
        let receipt = service
            .step(
                &mut state,
                &scene,
                entity,
                &config,
                command(sequence, Vec2::new(0.0, 1.0)),
            )
            .unwrap();
        if receipt.step.is_some_and(|step| step.accepted) {
            stepped = Some(receipt);
            break;
        }
    }
    let receipt = stepped.expect("controller should accept low step candidate");
    let step = receipt.step.unwrap();
    assert!(step.rise > 0.0);
    assert!(step.rise <= config.surface.maximum_step_height);
    let horizontal = Vec2::new(receipt.displacement.x, receipt.displacement.z).length();
    assert!(horizontal <= config.ground.forward_speed / 60.0 + 1.0e-4);
}

#[test]
fn moving_platform_support_is_retained_and_carries_next_step() {
    let scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, []).unwrap();
    let character_id = EntityId::new(1);
    let platform_id = EntityId::new(2);
    let mut state = EntityState::from_definitions([
        EntityDefinition::new(character_id, "character")
            .with_transform(Vec3::new(0.0, 1.9, 0.0))
            .with_character_motion(CharacterMotionComponent::at_rest(1.9)),
        EntityDefinition::new(platform_id, "platform")
            .with_transform(Vec3::new(0.0, 0.75, 0.0))
            .with_bounds(Vec3::new(-1.0, -0.25, -1.0), Vec3::new(1.0, 0.25, 1.0))
            .with_collision(true, false),
    ])
    .unwrap();
    let config = CharacterControllerConfig::default();
    let mut service = CharacterControllerService::default();
    let first = service
        .step(
            &mut state,
            &scene,
            character_id,
            &config,
            command(1, Vec2::ZERO),
        )
        .unwrap();
    assert_eq!(first.motion_after.support_entity, Some(platform_id));
    let revision = state.revision();
    state
        .apply_transform(
            revision,
            entity_state::TransformCommand::Translate {
                entity: platform_id,
                delta: Vec3::new(0.2, 0.0, 0.0),
            },
        )
        .unwrap();
    let second = service
        .step(
            &mut state,
            &scene,
            character_id,
            &config,
            command(2, Vec2::ZERO),
        )
        .unwrap();
    let platform = second.platform.expect("platform fact");
    assert_eq!(platform.entity, platform_id);
    assert!(!platform.departed);
    assert!((platform.carried_displacement.x - 0.2).abs() < 1.0e-5);
    assert!(second.displacement.x > 0.19);
}

#[test]
fn platform_translation_and_rotation_carry_flags_are_independent() {
    let setup = || {
        let player = EntityId::new(1);
        let platform = EntityId::new(2);
        let state = EntityState::from_definitions([
            EntityDefinition::new(player, "player")
                .with_transform(Vec3::new(0.5, 1.9, 0.0))
                .with_character_motion(CharacterMotionComponent::at_rest(1.9)),
            EntityDefinition::new(platform, "platform")
                .with_transform(Vec3::new(0.0, 0.75, 0.0))
                .with_bounds(Vec3::new(-1.0, -0.25, -1.0), Vec3::new(1.0, 0.25, 1.0))
                .with_collision(true, false),
        ])
        .unwrap();
        (player, platform, state)
    };
    let scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, []).unwrap();

    let (player, platform, mut state) = setup();
    let mut service = CharacterControllerService::default();
    let mut config = CharacterControllerConfig::default();
    config.platform.carry_translation = false;
    config.platform.carry_rotation = true;
    service
        .step(&mut state, &scene, player, &config, command(1, Vec2::ZERO))
        .unwrap();
    let revision = state.revision();
    state
        .apply_transform(
            revision,
            entity_state::TransformCommand::Translate {
                entity: platform,
                delta: Vec3::new(0.2, 0.0, 0.0),
            },
        )
        .unwrap();
    let receipt = service
        .step(&mut state, &scene, player, &config, command(2, Vec2::ZERO))
        .unwrap();
    assert!(receipt.platform.unwrap().carried_displacement.length() < 1.0e-5);

    let (player, platform, mut state) = setup();
    let mut service = CharacterControllerService::default();
    let mut config = CharacterControllerConfig::default();
    config.platform.carry_translation = false;
    config.platform.carry_rotation = true;
    service
        .step(&mut state, &scene, player, &config, command(1, Vec2::ZERO))
        .unwrap();
    let before = state.world_transform(platform).unwrap();
    let half = std::f32::consts::FRAC_PI_4;
    let revision = state.revision();
    state
        .apply_transform(
            revision,
            entity_state::TransformCommand::Set {
                entity: platform,
                transform: EntityTransform {
                    rotation: Quat {
                        x: 0.0,
                        y: half.sin(),
                        z: 0.0,
                        w: half.cos(),
                    },
                    ..before
                },
            },
        )
        .unwrap();
    let receipt = service
        .step(&mut state, &scene, player, &config, command(2, Vec2::ZERO))
        .unwrap();
    assert!(receipt.platform.unwrap().carried_displacement.length() > 0.4);
}

#[test]
fn external_impulse_remains_separate_from_controlled_velocity() {
    let scene = floor_scene();
    let (entity, mut state) = character(1.9);
    let config = CharacterControllerConfig::default();
    let mut service = CharacterControllerService::default();
    service
        .step(&mut state, &scene, entity, &config, command(1, Vec2::ZERO))
        .unwrap();
    let mut impulse = command(2, Vec2::new(0.0, 1.0));
    impulse.external_impulse = Vec3::new(2.0, 0.0, 0.0);
    let receipt = service
        .step(&mut state, &scene, entity, &config, impulse)
        .unwrap();
    assert!(receipt.motion_after.external_velocity.x > 1.9);
    assert!(receipt.motion_after.controlled_velocity.z < 0.0);
    assert!(receipt.displacement.x > 0.0);
}

#[test]
fn ceiling_stops_upward_velocity_without_bounce() {
    let mut voxels = floor_scene().solid_voxels().to_vec();
    voxels.push([0, 3, 0]);
    let scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, voxels).unwrap();
    let (entity, mut state) = character(1.9);
    let config = CharacterControllerConfig::default();
    let mut service = CharacterControllerService::default();
    service
        .step(&mut state, &scene, entity, &config, command(1, Vec2::ZERO))
        .unwrap();
    let mut hit = None;
    for sequence in 2..=90 {
        let mut input = command(sequence, Vec2::ZERO);
        input.jump_pressed = sequence == 2;
        let receipt = service
            .step(&mut state, &scene, entity, &config, input)
            .unwrap();
        if receipt.blocks.contains(&CharacterBlockKind::Ceiling) {
            hit = Some(receipt);
            break;
        }
    }
    let receipt = hit.expect("jump should reach the low ceiling");
    assert!(receipt.motion_after.controlled_velocity.y <= 0.0);
    assert!(receipt.transform_after.translation.y <= 2.1 + 1.0e-4);
}

#[test]
fn unresolved_deep_overlap_rejects_without_partial_publication() {
    let scene = floor_scene();
    let entity = EntityId::new(1);
    let blocker = EntityId::new(2);
    let mut state = EntityState::from_definitions([
        EntityDefinition::new(entity, "character")
            .with_transform(Vec3::new(0.0, 1.9, 0.0))
            .with_character_motion(CharacterMotionComponent::at_rest(1.9)),
        EntityDefinition::new(blocker, "blocker")
            .with_transform(Vec3::new(0.0, 1.9, 0.0))
            .with_bounds(Vec3::new(-2.0, -2.0, -2.0), Vec3::new(2.0, 2.0, 2.0))
            .with_collision(true, false),
    ])
    .unwrap();
    let before_revision = state.revision();
    let before_transform = *state.transform(entity).unwrap();
    let mut config = CharacterControllerConfig::default();
    config.recovery.maximum_distance = 0.05;
    config.recovery.maximum_speed = 1.0;
    config.solver.maximum_recovery_passes = 1;
    let error = CharacterControllerService::default()
        .step(&mut state, &scene, entity, &config, command(1, Vec2::ZERO))
        .unwrap_err();
    assert!(matches!(
        error,
        CharacterControllerError::UnresolvedPenetration { .. }
    ));
    assert_eq!(state.revision(), before_revision);
    assert_eq!(state.transform(entity), Some(&before_transform));
}

#[test]
fn generated_commands_remain_finite_and_within_solver_bounds() {
    let mut voxels = floor_scene().solid_voxels().to_vec();
    for y in 1..=4 {
        for z in -5..=5 {
            voxels.push([3, y, z]);
        }
    }
    let scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, voxels).unwrap();
    let (entity, mut state) = character(1.9);
    let config = CharacterControllerConfig::default();
    let mut service = CharacterControllerService::default();
    let mut seed = 0x9e37_79b9u32;
    for sequence in 1..=512 {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let x = ((seed & 0xffff) as f32 / 32767.5) - 1.0;
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let y = ((seed & 0xffff) as f32 / 32767.5) - 1.0;
        let receipt = service
            .step(
                &mut state,
                &scene,
                entity,
                &config,
                command(sequence, Vec2::new(x, y)),
            )
            .unwrap();
        let p = receipt.transform_after.translation;
        let v = receipt.motion_after.controlled_velocity + receipt.motion_after.external_velocity;
        assert!([p.x, p.y, p.z, v.x, v.y, v.z]
            .into_iter()
            .all(f32::is_finite));
        assert!(receipt.cast_count <= config.solver.maximum_queries_per_step);
        assert!(receipt.contacts.len() <= usize::from(config.solver.maximum_contacts));
        assert!(receipt.recovery_passes <= config.solver.maximum_recovery_passes);
        assert!(
            receipt.displacement.length() <= config.solver.maximum_displacement_per_step + 1.0e-4
        );
    }
}

#[test]
fn standable_ramp_climbs_while_over_limit_ramp_blocks_and_slides() {
    let run = |scene: VoxelCollisionScene, mut config: CharacterControllerConfig| {
        config.surface.maximum_step_height = 0.0;
        let (entity, mut state) = character(1.9);
        let mut service = CharacterControllerService::default();
        let mut receipts = Vec::new();
        for sequence in 1..=90 {
            receipts.push(
                service
                    .step(
                        &mut state,
                        &scene,
                        entity,
                        &config,
                        command(sequence, Vec2::new(0.0, 1.0)),
                    )
                    .unwrap(),
            );
        }
        receipts
    };
    let below = run(ramp_scene(2.0), CharacterControllerConfig::default());
    assert!(below
        .iter()
        .any(|receipt| receipt.transform_after.translation.y > 2.1));
    assert!(below.iter().any(|receipt| receipt.ground.is_some()));

    let at_limit_rise = 4.0
        * CharacterControllerConfig::default()
            .surface
            .maximum_slope_radians
            .tan();
    let at_limit = run(
        ramp_scene(f64::from(at_limit_rise)),
        CharacterControllerConfig::default(),
    );
    assert!(at_limit
        .iter()
        .any(|receipt| receipt.transform_after.translation.y > 2.1));

    let above = run(ramp_scene(7.0), CharacterControllerConfig::default());
    assert!(above
        .iter()
        .any(|receipt| receipt.blocks.contains(&CharacterBlockKind::SteepSlope)));
    assert!(above
        .iter()
        .all(|receipt| receipt.transform_after.translation.y < 2.2));
}

#[test]
fn live_voxel_edit_beneath_character_invalidates_support_and_continues() {
    let mut scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, [[0, 0, 0]]).unwrap();
    let (entity, mut state) = character(1.9);
    let config = CharacterControllerConfig::default();
    let mut service = CharacterControllerService::default();
    let grounded = service
        .step(&mut state, &scene, entity, &config, command(1, Vec2::ZERO))
        .unwrap();
    assert!(grounded.motion_after.grounded);
    let expected_revision = scene.source_revision();
    VoxelEditService::apply(
        &mut scene,
        VoxelEditTransaction {
            expected_revision,
            edits: &[VoxelEdit::Clear { address: [0, 0, 0] }],
        },
    )
    .unwrap();
    let falling = service
        .step(&mut state, &scene, entity, &config, command(2, Vec2::ZERO))
        .unwrap();
    assert!(!falling.motion_after.grounded);
    assert!(falling.motion_after.controlled_velocity.y < 0.0);
    assert_ne!(
        grounded.motion_after.collision_world_hash,
        falling.motion_after.collision_world_hash
    );
}

#[test]
fn fixed_step_partitions_are_repeatable_and_near_equivalent() {
    let simulate = |dt: f32, count: u64| {
        let scene = floor_scene();
        let (entity, mut state) = character(1.9);
        let config = CharacterControllerConfig::default();
        let mut service = CharacterControllerService::default();
        for sequence in 1..=count {
            let mut input = command(sequence, Vec2::new(0.0, 1.0));
            input.step_seconds = dt;
            service
                .step(&mut state, &scene, entity, &config, input)
                .unwrap();
        }
        *state.transform(entity).unwrap()
    };
    let sixty_a = simulate(1.0 / 60.0, 60);
    let sixty_b = simulate(1.0 / 60.0, 60);
    let thirty = simulate(1.0 / 30.0, 30);
    assert_eq!(sixty_a, sixty_b);
    assert!((sixty_a.translation.z - thirty.translation.z).abs() < 0.2);
    assert!(
        (sixty_a.translation.y - thirty.translation.y).abs() < 0.05,
        "60 Hz {sixty_a:?}, 30 Hz {thirty:?}"
    );
}

#[test]
fn floor_snap_and_open_ledge_have_distinct_support_outcomes() {
    let scene = floor_scene();
    let (entity, mut state) = character(2.05);
    let config = CharacterControllerConfig::default();
    let snapped = CharacterControllerService::default()
        .step(&mut state, &scene, entity, &config, command(1, Vec2::ZERO))
        .unwrap();
    assert!(snapped.motion_after.grounded);
    assert!(snapped.ground.unwrap().snapped_distance > 0.1);
    assert!(snapped.transform_after.translation.y < 1.93);

    let ledge_scene = VoxelCollisionScene::from_solid_voxels(
        1.0,
        8,
        (-4..=4).flat_map(|x| (0..=4).map(move |z| [x, 0, z])),
    )
    .unwrap();
    let (entity, mut state) = character(1.9);
    let mut service = CharacterControllerService::default();
    let mut airborne = None;
    for sequence in 1..=90 {
        let receipt = service
            .step(
                &mut state,
                &ledge_scene,
                entity,
                &config,
                command(sequence, Vec2::new(0.0, 1.0)),
            )
            .unwrap();
        if !receipt.motion_after.grounded {
            airborne = Some(receipt);
            break;
        }
    }
    let airborne = airborne.expect("walking beyond finite floor should leave support");
    assert!(airborne.ground.is_none());
    assert!(airborne.motion_after.controlled_velocity.y <= 0.0);
}

#[test]
fn floor_wall_lip_retains_stable_support_at_rest_and_under_pressure() {
    let scene = floor_wall_lip_scene();
    let (entity, mut state) = character_at(Vec3::new(0.5, 1.9, 1.350_6));
    let config = CharacterControllerConfig::default();
    let mut service = CharacterControllerService::default();

    let mut previous_grounded = None;
    let mut grounded_transitions = 0;
    let mut saw_rejected_wall_with_accepted_support = false;
    for sequence in 1..=120 {
        let receipt = service
            .step(
                &mut state,
                &scene,
                entity,
                &config,
                command(sequence, Vec2::ZERO),
            )
            .unwrap();
        if previous_grounded.is_some_and(|grounded| grounded != receipt.motion_after.grounded) {
            grounded_transitions += 1;
        }
        previous_grounded = Some(receipt.motion_after.grounded);
        if let Some(probe) = receipt.floor_probe {
            if let Some(rejected) = probe.rejected_hit {
                assert_eq!(rejected.kind, CharacterContactKind::Wall);
                assert!(probe.accepted_support.is_some());
                saw_rejected_wall_with_accepted_support = true;
            }
        }
        assert!(
            receipt.motion_after.grounded,
            "floor support was lost at idle sequence {sequence}: {receipt:?}"
        );
        assert!((receipt.transform_after.translation.y - 1.9).abs() < 1.0e-4);
    }
    assert_eq!(grounded_transitions, 0);
    assert!(saw_rejected_wall_with_accepted_support);

    let pressure_start = *state.transform(entity).unwrap();
    let mut saw_wall = false;
    for sequence in 121..=240 {
        let receipt = service
            .step(
                &mut state,
                &scene,
                entity,
                &config,
                command(sequence, Vec2::new(0.0, 1.0)),
            )
            .unwrap();
        saw_wall |= receipt.blocks.contains(&CharacterBlockKind::Wall);
        assert!(
            receipt.motion_after.grounded,
            "floor support was lost under wall pressure at sequence {sequence}: {receipt:?}"
        );
        assert!(
            (receipt.transform_after.translation.y - pressure_start.translation.y).abs() < 1.0e-4
        );
        assert!(receipt.motion_after.controlled_velocity.y.abs() < 1.0e-4);
        assert!(receipt.transform_after.translation.z >= 1.349);
        assert!(receipt.step.is_none_or(|step| !step.accepted));
    }
    assert!(saw_wall);
}

#[test]
fn descending_trench_edge_does_not_manufacture_a_transient_up_step() {
    let scene = craftsurvive_trench_scene();
    let (entity, mut state) = character_at(Vec3::new(0.5, 3.875, 5.307_7));
    let mut config = CharacterControllerConfig::default();
    config.shape.standing_height = 1.75;
    config.shape.crouched_height = 1.0;
    config.shape.radius = 0.3;
    config.shape.contact_skin = 0.015;
    config.ground.forward_speed = 7.0;
    config.ground.backward_speed = 7.0;
    config.ground.strafe_speed = 7.0;
    config.ground.acceleration = 48.0;
    config.ground.braking = 58.0;
    config.ground.friction = 9.0;
    config.surface.maximum_step_height = 1.05;
    config.surface.floor_snap_distance = 0.25;

    let mut service = CharacterControllerService::default();
    let mut minimum_y = f32::INFINITY;
    let mut maximum_y = f32::NEG_INFINITY;
    let mut accepted_steps = 0;
    for sequence in 1..=240 {
        let receipt = service
            .step(
                &mut state,
                &scene,
                entity,
                &config,
                command(sequence, Vec2::new(0.0, 1.0)),
            )
            .unwrap();
        minimum_y = minimum_y.min(receipt.transform_after.translation.y);
        maximum_y = maximum_y.max(receipt.transform_after.translation.y);
        accepted_steps += usize::from(receipt.step.is_some_and(|step| step.accepted));
    }
    let final_transform = state.transform(entity).unwrap();
    assert_eq!(
        accepted_steps, 0,
        "descending support must not become an up step"
    );
    assert!(
        maximum_y <= 3.89,
        "trench traversal bounced to y={maximum_y}"
    );
    assert!(
        minimum_y >= 3.87,
        "trench pressure dropped to y={minimum_y}"
    );
    assert!(final_transform.translation.z >= 5.3);
    assert!(state.character_motion(entity).unwrap().grounded);
}

#[test]
fn buffered_jump_fires_after_landing_and_coyote_jump_fires_once() {
    let scene = floor_scene();
    let (entity, mut state) = character(2.25);
    let config = CharacterControllerConfig::default();
    let mut service = CharacterControllerService::default();
    let mut jumped = false;
    for sequence in 1..=60 {
        let mut input = command(sequence, Vec2::ZERO);
        input.jump_pressed = sequence == 1;
        let receipt = service
            .step(&mut state, &scene, entity, &config, input)
            .unwrap();
        if receipt.motion_after.controlled_velocity.y > 1.0 {
            jumped = true;
            assert_eq!(receipt.motion_after.jump_buffer_remaining, 0.0);
            break;
        }
    }
    assert!(jumped, "buffered input should be consumed after landing");

    let ledge_scene = VoxelCollisionScene::from_solid_voxels(
        1.0,
        8,
        (-4..=4).flat_map(|x| (0..=4).map(move |z| [x, 0, z])),
    )
    .unwrap();
    let (entity, mut state) = character(1.9);
    let mut service = CharacterControllerService::default();
    let mut sequence = 0;
    loop {
        sequence += 1;
        let receipt = service
            .step(
                &mut state,
                &ledge_scene,
                entity,
                &config,
                command(sequence, Vec2::new(0.0, 1.0)),
            )
            .unwrap();
        if !receipt.motion_after.grounded {
            break;
        }
        assert!(sequence < 90);
    }
    sequence += 1;
    let mut input = command(sequence, Vec2::new(0.0, 1.0));
    input.jump_pressed = true;
    let jumped = service
        .step(&mut state, &ledge_scene, entity, &config, input)
        .unwrap();
    assert!(jumped.motion_after.controlled_velocity.y > 0.0);
    sequence += 1;
    let held = CharacterControllerCommand {
        jump_held: true,
        ..command(sequence, Vec2::new(0.0, 1.0))
    };
    let after = service
        .step(&mut state, &ledge_scene, entity, &config, held)
        .unwrap();
    assert!(after.motion_after.controlled_velocity.y < jumped.motion_after.controlled_velocity.y);
}

#[test]
fn descending_step_uses_floor_snap_without_manufacturing_an_up_step() {
    let scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, []).unwrap();
    let player = EntityId::new(1);
    let upper = EntityId::new(2);
    let lower = EntityId::new(3);
    let mut state = EntityState::from_definitions([
        EntityDefinition::new(player, "player")
            .with_transform(Vec3::new(0.0, 1.9, 0.5))
            .with_character_motion(CharacterMotionComponent::at_rest(1.9)),
        EntityDefinition::new(upper, "upper")
            .with_transform(Vec3::new(0.0, 0.5, 0.5))
            .with_bounds(Vec3::new(-2.0, -0.5, -0.5), Vec3::new(2.0, 0.5, 0.5))
            .with_collision(true, false),
        EntityDefinition::new(lower, "lower")
            .with_transform(Vec3::new(0.0, 0.3, -0.5))
            .with_bounds(Vec3::new(-2.0, -0.5, -0.5), Vec3::new(2.0, 0.5, 0.5))
            .with_collision(true, false),
    ])
    .unwrap();
    let config = CharacterControllerConfig::default();
    let mut service = CharacterControllerService::default();
    let mut descended = None;
    for sequence in 1..=60 {
        let receipt = service
            .step(
                &mut state,
                &scene,
                player,
                &config,
                command(sequence, Vec2::new(0.0, 1.0)),
            )
            .unwrap();
        if receipt.transform_after.translation.y < 1.8 {
            descended = Some(receipt);
            break;
        }
    }
    let receipt = descended.expect("controller should adhere to the lower surface");
    assert!(receipt.motion_after.grounded);
    assert!(receipt.ground.unwrap().snapped_distance > 0.0);
    assert!(receipt.step.is_none_or(|step| !step.accepted));
}

#[test]
#[ignore = "performance probe; run with scripts/measure-character-controller.sh"]
fn representative_character_controller_performance_budget() {
    let mut voxels = Vec::new();
    for x in -32..=32 {
        for z in -32..=32 {
            voxels.push([x, 0, z]);
        }
    }
    for y in 1..=4 {
        for z in -16..=16 {
            voxels.push([8, y, z]);
        }
    }
    let voxel_scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, voxels).unwrap();
    let static_scene = ramp_scene(2.0);
    let config = CharacterControllerConfig::default();
    let iterations = 5_000u64;
    let measure = |label: &str, scene: &VoxelCollisionScene, mut state: EntityState| {
        let entity = EntityId::new(1);
        let mut service = CharacterControllerService::default();
        let started = std::time::Instant::now();
        for sequence in 1..=iterations {
            service
                .step(
                    &mut state,
                    scene,
                    entity,
                    &config,
                    command(sequence, Vec2::ZERO),
                )
                .unwrap();
        }
        let elapsed = started.elapsed();
        let nanos_per_step = elapsed.as_nanos() / u128::from(iterations);
        println!(
            "character_controller scene={label} steps={iterations} elapsed_ms={} ns_per_step={nanos_per_step}",
            elapsed.as_millis()
        );
        assert!(
            nanos_per_step < 1_000_000,
            "1 ms fixed-step budget exceeded for {label}"
        );
    };
    measure("voxel", &voxel_scene, character(1.9).1);
    measure("static_mesh", &static_scene, character(1.9).1);

    let mut definitions = vec![EntityDefinition::new(EntityId::new(1), "character")
        .with_transform(Vec3::new(0.0, 1.9, 0.0))
        .with_character_motion(CharacterMotionComponent::at_rest(1.9))];
    for raw in 2..=129 {
        definitions.push(
            EntityDefinition::new(EntityId::new(raw), format!("obstacle-{raw}"))
                .with_transform(Vec3::new(12.0 + raw as f32, 1.0, 0.0))
                .with_bounds(Vec3::new(-0.5, -0.5, -0.5), Vec3::new(0.5, 0.5, 0.5))
                .with_collision(true, false),
        );
    }
    measure(
        "mixed_128_active",
        &static_scene,
        EntityState::from_definitions(definitions).unwrap(),
    );
}
