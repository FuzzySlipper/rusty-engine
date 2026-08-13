use core_ids::{EntityId, SceneId, SceneNodeId};
use core_math::{Vec2, Vec3};
use core_space::{GlobalPosition, WorldOrigin};
use engine_spatial::{
    decode_world_origin_state, encode_world_origin_state, CharacterControllerCommand,
    CharacterControllerConfig, CharacterControllerService, KinematicTriggerDefinition,
    MaterialVoxel, SpatialCollisionHit, StaticMeshAssetId, StaticMeshColliderAsset,
    StaticMeshColliderInstance, StaticMeshInstanceId, StaticMeshTransform, TriggerReconcileCause,
    TriggerVolumeSystem, VoxelCollisionScene, VoxelEdit, VoxelEditService, VoxelEditTransaction,
    WorldOriginEntity, WorldOriginRebaseError, WorldOriginRebaseRequest, WorldOriginRebaseService,
    WorldOriginState,
};
use entity_state::{
    CharacterMotionComponent, EntityCommand, EntityCommandBatch, EntityDefinition, EntitySource,
    EntityState, TransformCommand,
};

const PLAYER: EntityId = EntityId::new(1);
const PLATFORM: EntityId = EntityId::new(2);
const TRIGGER: EntityId = EntityId::new(3);
const SUBJECT: EntityId = EntityId::new(4);
const FAR_X: i64 = 100_000;

fn global(x: f64, y: f64, z: f64) -> GlobalPosition {
    GlobalPosition::from_world([x, y, z]).unwrap()
}

fn fixture() -> (WorldOriginState, EntityState, VoxelCollisionScene) {
    let mut motion = CharacterMotionComponent::at_rest(1.9);
    motion.grounded = true;
    motion.support_entity = Some(PLATFORM);
    motion.support_previous_translation = Vec3::new(FAR_X as f32, 1.0, 0.0);

    let entities = EntityState::from_definitions([
        EntityDefinition::new(PLAYER, "player")
            .with_transform(Vec3::new(FAR_X as f32, 1.9, 0.0))
            .with_bounds(Vec3::new(-0.3, -0.9, -0.3), Vec3::new(0.3, 0.9, 0.3))
            .with_collision(true, false)
            .with_character_motion(motion),
        EntityDefinition::new(PLATFORM, "platform")
            .with_source(EntitySource::AuthoredScene {
                scene: SceneId::new(7),
                node: SceneNodeId::new(11),
            })
            .with_transform(Vec3::new(FAR_X as f32, 1.0, 0.0))
            .with_bounds(Vec3::new(-1.0, -0.1, -1.0), Vec3::new(1.0, 0.1, 1.0))
            .with_collision(true, true),
        EntityDefinition::new(TRIGGER, "trigger")
            .with_transform(Vec3::new(FAR_X as f32 + 4.0, 1.0, 0.0))
            .with_bounds(Vec3::splat(-0.5), Vec3::splat(0.5))
            .with_collision(true, true),
        EntityDefinition::new(SUBJECT, "subject")
            .with_transform(Vec3::new(FAR_X as f32 + 4.25, 1.0, 0.0))
            .with_bounds(Vec3::splat(-0.25), Vec3::splat(0.25))
            .with_collision(true, false),
    ])
    .unwrap();

    let floor = (-2..=6).map(|offset| MaterialVoxel {
        address: [FAR_X + offset, 0, 0],
        material_slot: 1,
    });
    let mut scene = VoxelCollisionScene::from_material_voxels(1.0, 8, floor).unwrap();
    let asset = StaticMeshColliderAsset::new(
        StaticMeshAssetId(9),
        vec![
            [-0.5, -0.5, 0.0],
            [0.5, -0.5, 0.0],
            [-0.5, 0.5, 0.0],
            [0.5, 0.5, 0.0],
        ],
        vec![[0, 1, 2], [1, 3, 2]],
    )
    .unwrap();
    let geometry_hash = asset.geometry_hash;
    scene
        .replace_static_mesh_colliders(
            0,
            [asset],
            [StaticMeshColliderInstance {
                id: StaticMeshInstanceId(12),
                asset: StaticMeshAssetId(9),
                expected_geometry_hash: geometry_hash,
                transform: StaticMeshTransform {
                    translation: [FAR_X as f64 + 2.0, 2.0, 2.0],
                    ..StaticMeshTransform::IDENTITY
                },
            }],
        )
        .unwrap();
    (WorldOriginState::default(), entities, scene)
}

fn bindings() -> Vec<WorldOriginEntity> {
    vec![
        WorldOriginEntity {
            entity: PLAYER,
            global_position: global(FAR_X as f64, 1.9, 0.0),
        },
        WorldOriginEntity {
            entity: PLATFORM,
            global_position: global(FAR_X as f64, 1.0, 0.0),
        },
        WorldOriginEntity {
            entity: TRIGGER,
            global_position: global(FAR_X as f64 + 4.0, 1.0, 0.0),
        },
        WorldOriginEntity {
            entity: SUBJECT,
            global_position: global(FAR_X as f64 + 4.25, 1.0, 0.0),
        },
    ]
}

fn request(
    origin: &WorldOriginState,
    entities: &EntityState,
    scene: &VoxelCollisionScene,
    target_origin: WorldOrigin,
    entities_at_global_positions: Vec<WorldOriginEntity>,
) -> WorldOriginRebaseRequest {
    WorldOriginRebaseRequest {
        expected_origin_revision: origin.revision(),
        expected_entity_revision: entities.revision(),
        expected_voxel_source_revision: scene.source_revision().raw(),
        expected_static_mesh_revision: scene.static_mesh_collision_revision(),
        target_origin,
        entities: entities_at_global_positions,
    }
}

#[test]
fn rebase_keeps_controller_support_voxel_nav_trigger_and_static_mesh_continuous() {
    let (mut origin, mut entities, mut scene) = fixture();
    let source_revision = scene.source_revision();
    let authority_hash = scene.authority_hash();
    let mut triggers = TriggerVolumeSystem::new([KinematicTriggerDefinition::new(
        TRIGGER,
        "test.zone",
        ["test"],
    )])
    .unwrap();
    let entered = triggers
        .reconcile(&entities, 1, TriggerReconcileCause::Spawn)
        .unwrap();
    assert_eq!(entered.active_overlaps.len(), 1);

    let rebase = request(
        &origin,
        &entities,
        &scene,
        WorldOrigin::new([FAR_X, 0, 0]),
        bindings(),
    );
    let receipt = WorldOriginRebaseService
        .apply(&mut origin, &mut entities, &mut scene, rebase)
        .unwrap();

    assert_eq!(receipt.origin_after, WorldOrigin::new([FAR_X, 0, 0]));
    assert_eq!(scene.world_origin(), receipt.origin_after);
    assert_eq!(scene.rebase_revision(), receipt.revision_after);
    assert_eq!(scene.source_revision(), source_revision);
    assert_eq!(scene.authority_hash(), authority_hash);
    assert_eq!(entities.transform(PLAYER).unwrap().translation.x, 0.0);
    assert_eq!(entities.transform(PLATFORM).unwrap().translation.x, 0.0);
    let motion = entities.character_motion(PLAYER).unwrap();
    assert!(motion.grounded);
    assert_eq!(motion.support_entity, Some(PLATFORM));
    assert_eq!(motion.support_previous_translation.x, 0.0);
    assert_ne!(motion.collision_world_hash, 0);

    let continued = triggers
        .reconcile(&entities, 2, TriggerReconcileCause::Movement)
        .unwrap();
    assert!(continued.facts.is_empty());
    assert_eq!(continued.continued, entered.active_overlaps);

    let voxel_hit = scene
        .raycast([2.5, 3.0, 0.5], [0.0, -1.0, 0.0], 4.0)
        .unwrap();
    assert_eq!(voxel_hit.voxel, [FAR_X + 2, 0, 0]);
    let nav = scene
        .navigation_step(
            Vec3::new(0.5, 1.5, 0.5),
            Vec3::new(4.5, 1.5, 0.5),
            Vec3::ZERO,
            1.0,
            64,
        )
        .unwrap();
    assert!(nav.next_waypoint.x.abs() < 16_384.0);
    assert!(matches!(
        scene.raycast_world([2.0, 2.0, 0.0], [0.0, 0.0, 1.0], 4.0),
        Some(SpatialCollisionHit::StaticMesh(hit))
            if hit.instance == StaticMeshInstanceId(12)
    ));

    let clear = [VoxelEdit::Clear {
        address: [FAR_X + 2, 0, 0],
    }];
    let edit = VoxelEditService::apply(
        &mut scene,
        VoxelEditTransaction {
            expected_revision: source_revision,
            edits: &clear,
        },
    )
    .unwrap();
    assert_eq!(edit.fact.changed_min, [FAR_X + 2, 0, 0]);
    assert_eq!(edit.fact.changed_max_inclusive, [FAR_X + 2, 0, 0]);
    assert_eq!(scene.world_origin(), origin.origin());
    assert_eq!(scene.rebase_revision(), origin.revision());
    assert!(scene
        .raycast([2.5, 3.0, 0.5], [0.0, -1.0, 0.0], 4.0)
        .is_none());

    let after_edit_rebase = request(
        &origin,
        &entities,
        &scene,
        WorldOrigin::new([FAR_X + 1, 0, 0]),
        bindings(),
    );
    WorldOriginRebaseService
        .apply(&mut origin, &mut entities, &mut scene, after_edit_rebase)
        .unwrap();
    assert_eq!(origin.revision(), 2);
    assert_eq!(scene.world_origin(), origin.origin());
}

#[test]
fn repeated_positive_and_negative_rebases_do_not_accumulate_or_alias() {
    for far_x in [900_000_i64, -900_000_i64] {
        let entity = EntityId::new(51);
        let global_position = global(far_x as f64 + 0.375, 2.5, -0.625);
        let mut origin = WorldOriginState::default();
        let mut entities =
            EntityState::from_definitions([EntityDefinition::new(entity, "traveler")
                .with_transform(Vec3::new(far_x as f32, 2.5, -0.625))])
            .unwrap();
        let mut scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, [[far_x, 0, -1]]).unwrap();

        for target in [far_x, far_x - 5_000, far_x + 5_000, far_x] {
            let rebase = request(
                &origin,
                &entities,
                &scene,
                WorldOrigin::new([target, 0, 0]),
                vec![WorldOriginEntity {
                    entity,
                    global_position,
                }],
            );
            WorldOriginRebaseService
                .apply(&mut origin, &mut entities, &mut scene, rebase)
                .unwrap();
            assert_eq!(
                origin
                    .global_from_local(entities.transform(entity).unwrap().translation.to_array()),
                Ok(global_position)
            );
            let local_voxel_x = (far_x - target) as f64 + 0.5;
            assert_eq!(
                scene
                    .raycast([local_voxel_x, 2.0, -0.5], [0.0, -1.0, 0.0], 3.0)
                    .unwrap()
                    .voxel,
                [far_x, 0, -1]
            );
        }
    }
}

#[test]
fn moving_support_carry_continues_after_far_origin_rebase() {
    let player = EntityId::new(41);
    let platform = EntityId::new(42);
    let mut entities = EntityState::from_definitions([
        EntityDefinition::new(player, "character")
            .with_transform(Vec3::new(FAR_X as f32, 1.9, 0.0))
            .with_character_motion(CharacterMotionComponent::at_rest(1.9)),
        EntityDefinition::new(platform, "moving platform")
            .with_transform(Vec3::new(FAR_X as f32, 0.75, 0.0))
            .with_bounds(Vec3::new(-1.0, -0.25, -1.0), Vec3::new(1.0, 0.25, 1.0))
            .with_collision(true, false),
    ])
    .unwrap();
    let mut scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, []).unwrap();
    let mut origin = WorldOriginState::default();
    let config = CharacterControllerConfig::default();
    let mut controller = CharacterControllerService::default();
    let command = |sequence| CharacterControllerCommand {
        planar_intent: Vec2::ZERO,
        ..CharacterControllerCommand::idle(1.0 / 60.0, sequence)
    };
    let settled = controller
        .step(&mut entities, &scene, player, &config, command(1))
        .unwrap();
    assert_eq!(settled.motion_after.support_entity, Some(platform));

    let rebase = request(
        &origin,
        &entities,
        &scene,
        WorldOrigin::new([FAR_X, 0, 0]),
        vec![
            WorldOriginEntity {
                entity: player,
                global_position: global(
                    FAR_X as f64,
                    settled.transform_after.translation.y as f64,
                    0.0,
                ),
            },
            WorldOriginEntity {
                entity: platform,
                global_position: global(FAR_X as f64, 0.75, 0.0),
            },
        ],
    );
    WorldOriginRebaseService
        .apply(&mut origin, &mut entities, &mut scene, rebase)
        .unwrap();
    let revision = entities.revision();
    entities
        .apply_transform(
            revision,
            TransformCommand::Translate {
                entity: platform,
                delta: Vec3::new(0.2, 0.0, 0.0),
            },
        )
        .unwrap();
    let carried = controller
        .step(&mut entities, &scene, player, &config, command(2))
        .unwrap();
    let platform_fact = carried.platform.expect("support carry fact");
    assert_eq!(platform_fact.entity, platform);
    assert!((platform_fact.carried_displacement.x - 0.2).abs() < 1.0e-5);
    assert!(carried.displacement.x > 0.19);
}

#[test]
fn failed_prepare_and_stale_commit_publish_nothing_and_snapshots_are_typed() {
    let (mut origin, mut entities, mut scene) = fixture();
    let origin_before = origin.readout();
    let scene_origin_before = scene.world_origin();
    let rebase_revision_before = scene.rebase_revision();
    let player_before = *entities.transform(PLAYER).unwrap();

    let mut incomplete = bindings();
    incomplete.pop();
    let invalid = request(
        &origin,
        &entities,
        &scene,
        WorldOrigin::new([FAR_X, 0, 0]),
        incomplete,
    );
    assert!(matches!(
        WorldOriginRebaseService.prepare(&origin, &entities, &scene, invalid),
        Err(WorldOriginRebaseError::MissingRootEntity { entity: SUBJECT })
    ));
    assert_eq!(origin.readout(), origin_before);
    assert_eq!(*entities.transform(PLAYER).unwrap(), player_before);
    assert_eq!(scene.world_origin(), scene_origin_before);

    let valid = request(
        &origin,
        &entities,
        &scene,
        WorldOrigin::new([FAR_X, 0, 0]),
        bindings(),
    );
    let prepared = WorldOriginRebaseService
        .prepare(&origin, &entities, &scene, valid)
        .unwrap();
    entities
        .apply_batch(EntityCommandBatch::new([EntityCommand::SetTranslation {
            entity: SUBJECT,
            translation: Vec3::new(FAR_X as f32 + 5.0, 1.0, 0.0),
        }]))
        .unwrap();
    let scene_source_before = scene.source_revision();
    assert!(matches!(
        WorldOriginRebaseService.commit(&mut origin, &mut entities, &mut scene, prepared),
        Err(WorldOriginRebaseError::StaleEntityState { .. })
    ));
    assert_eq!(origin.readout(), origin_before);
    assert_eq!(scene.world_origin(), scene_origin_before);
    assert_eq!(scene.rebase_revision(), rebase_revision_before);
    assert_eq!(scene.source_revision(), scene_source_before);

    let encoded = encode_world_origin_state(origin).unwrap();
    assert_eq!(decode_world_origin_state(&encoded).unwrap(), origin);
    assert!(matches!(
        decode_world_origin_state(
            br#"{"schemaVersion":2,"origin":[0,0,0],"revision":0,"localEnvelope":16384.0}"#
        ),
        Err(WorldOriginRebaseError::UnsupportedSnapshotSchema { actual: 2 })
    ));
    assert!(matches!(
        decode_world_origin_state(b"not json"),
        Err(WorldOriginRebaseError::SnapshotDecode)
    ));
}
