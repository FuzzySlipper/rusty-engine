use core_ids::EntityId;
use core_math::Vec3;
use core_space::{GlobalPosition, WorldOrigin};
use engine_spatial::{
    KinematicTriggerDefinition, MaterialVoxel, PreparedWorldOriginRebase, SpatialCollisionHit,
    StaticMeshAssetId, StaticMeshColliderAsset, StaticMeshColliderInstance, StaticMeshInstanceId,
    StaticMeshTransform, TriggerCollider, TriggerReconcileCause, TriggerVolumeSystem,
    VoxelCollisionScene, VoxelEdit, VoxelEditService, WorldOriginAffectedTransform,
    WorldOriginEntity, WorldOriginRebaseError, WorldOriginRebaseReceipt, WorldOriginRebaseRequest,
    WorldOriginRebaseService, WorldOriginState,
};
use entity_state::{EntityTransform, Quat};

const TRIGGER: EntityId = EntityId::new(3);
const SUBJECT: EntityId = EntityId::new(4);
const FAR_X: i64 = 100_000;

fn global(x: f64, y: f64, z: f64) -> GlobalPosition {
    GlobalPosition::from_world([x, y, z]).unwrap()
}

fn fixture() -> (WorldOriginState, VoxelCollisionScene) {
    let floor = (-2..=6).map(|offset| MaterialVoxel {
        state: 0,
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
    scene
        .replace_static_mesh_colliders(
            [asset],
            [StaticMeshColliderInstance {
                id: StaticMeshInstanceId(12),
                asset: StaticMeshAssetId(9),
                transform: StaticMeshTransform {
                    translation: [FAR_X as f64 + 2.0, 2.0, 2.0],
                    ..StaticMeshTransform::IDENTITY
                },
            }],
        )
        .unwrap();
    (WorldOriginState::default(), scene)
}

fn root(entity: EntityId, x: f64, y: f64) -> WorldOriginEntity {
    WorldOriginEntity {
        entity,
        transform: EntityTransform {
            rotation: Quat::new(0.0, 0.6, 0.0, 0.8),
            scale: Vec3::splat(2.0),
            ..EntityTransform::at(Vec3::new(x as f32, y as f32, 0.0))
        },
        global_position: global(x, y, 0.0),
    }
}

fn bindings() -> Vec<WorldOriginEntity> {
    vec![
        root(TRIGGER, FAR_X as f64 + 4.0, 1.0),
        root(SUBJECT, FAR_X as f64 + 4.25, 1.0),
    ]
}

fn request(
    target_origin: WorldOrigin,
    entities: Vec<WorldOriginEntity>,
) -> WorldOriginRebaseRequest {
    WorldOriginRebaseRequest {
        target_origin,
        entities,
    }
}

fn commit(
    origin: &mut WorldOriginState,
    scene: &mut VoxelCollisionScene,
    prepared: &PreparedWorldOriginRebase,
) -> WorldOriginRebaseReceipt {
    let (rebased, receipt) = WorldOriginRebaseService
        .commit(origin, scene, prepared)
        .unwrap();
    *scene = rebased;
    receipt
}

fn rebase(
    origin: &mut WorldOriginState,
    scene: &mut VoxelCollisionScene,
    target_origin: WorldOrigin,
    entities: Vec<WorldOriginEntity>,
) -> Vec<WorldOriginAffectedTransform> {
    let prepared = WorldOriginRebaseService
        .prepare(origin, request(target_origin, entities))
        .unwrap();
    commit(origin, scene, &prepared);
    prepared.affected_transforms().to_vec()
}

fn collider(entity: EntityId, center: Vec3, half: f32) -> TriggerCollider {
    TriggerCollider {
        entity,
        min: center - Vec3::splat(half),
        max: center + Vec3::splat(half),
        collision_enabled: true,
    }
}

#[test]
fn rebase_keeps_voxel_nav_trigger_and_static_mesh_continuous() {
    let (mut origin, mut scene) = fixture();
    let source_revision = scene.source_revision();
    let authority_hash = scene.authority_hash();
    let mut triggers = TriggerVolumeSystem::new([KinematicTriggerDefinition::new(
        TRIGGER,
        "test.zone",
        ["test"],
    )])
    .unwrap();
    let entered = triggers.reconcile(
        [
            collider(TRIGGER, Vec3::new(FAR_X as f32 + 4.0, 1.0, 0.0), 0.5),
            collider(SUBJECT, Vec3::new(FAR_X as f32 + 4.25, 1.0, 0.0), 0.25),
        ],
        1,
        TriggerReconcileCause::Spawn,
    );
    assert_eq!(entered.active_overlaps.len(), 1);

    let prepared = WorldOriginRebaseService
        .prepare(
            &origin,
            request(WorldOrigin::new([FAR_X, 0, 0]), bindings()),
        )
        .unwrap();
    assert_eq!(prepared.target_origin(), WorldOrigin::new([FAR_X, 0, 0]));
    let receipt = commit(&mut origin, &mut scene, &prepared);

    assert_eq!(receipt.origin_after, WorldOrigin::new([FAR_X, 0, 0]));
    assert_eq!(receipt.entity_count, 2);
    assert_eq!(scene.world_origin(), receipt.origin_after);
    assert_eq!(scene.rebase_revision(), receipt.revision_after);
    assert_eq!(scene.source_revision(), source_revision);
    assert_eq!(scene.authority_hash(), authority_hash);
    let affected = prepared.affected_transforms();
    assert_eq!(affected[0].entity, TRIGGER);
    assert_eq!(affected[0].transform.translation, Vec3::new(4.0, 1.0, 0.0));
    assert_eq!(
        affected[0].transform.rotation,
        Quat::new(0.0, 0.6, 0.0, 0.8)
    );
    assert_eq!(affected[0].transform.scale, Vec3::splat(2.0));
    assert_eq!(affected[1].transform.translation, Vec3::new(4.25, 1.0, 0.0));

    let continued = triggers.reconcile(
        [
            collider(TRIGGER, affected[0].transform.translation, 0.5),
            collider(SUBJECT, affected[1].transform.translation, 0.25),
        ],
        2,
        TriggerReconcileCause::Movement,
    );
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
    let edit = VoxelEditService::apply(&mut scene, &clear).unwrap();
    assert_eq!(edit.fact.changed_min, [FAR_X + 2, 0, 0]);
    assert_eq!(edit.fact.changed_max_inclusive, [FAR_X + 2, 0, 0]);
    assert_eq!(scene.world_origin(), origin.origin());
    assert_eq!(scene.rebase_revision(), origin.revision());
    assert!(scene
        .raycast([2.5, 3.0, 0.5], [0.0, -1.0, 0.0], 4.0)
        .is_none());

    rebase(
        &mut origin,
        &mut scene,
        WorldOrigin::new([FAR_X + 1, 0, 0]),
        bindings(),
    );
    assert_eq!(origin.revision(), 2);
    assert_eq!(scene.world_origin(), origin.origin());
}

#[test]
fn repeated_positive_and_negative_rebases_do_not_accumulate_or_alias() {
    for far_x in [900_000_i64, -900_000_i64] {
        let entity = EntityId::new(51);
        let global_position = global(far_x as f64 + 0.375, 2.5, -0.625);
        let mut origin = WorldOriginState::default();
        let mut transform = EntityTransform::at(Vec3::new(far_x as f32, 2.5, -0.625));
        let mut scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, [[far_x, 0, -1]]).unwrap();

        for target in [far_x, far_x - 5_000, far_x + 5_000, far_x] {
            let affected = rebase(
                &mut origin,
                &mut scene,
                WorldOrigin::new([target, 0, 0]),
                vec![WorldOriginEntity {
                    entity,
                    transform,
                    global_position,
                }],
            );
            transform = affected[0].transform;
            assert_eq!(
                GlobalPosition::from_local(origin.origin(), transform.translation.to_array()),
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
fn failed_prepare_publishes_nothing_and_edits_after_prepare_survive() {
    let (mut origin, mut scene) = fixture();
    let origin_before = origin.readout();
    let scene_origin_before = scene.world_origin();

    // A root beyond the local envelope of the target origin cannot be placed.
    let unreachable = request(
        WorldOrigin::new([0, 0, 0]),
        vec![root(SUBJECT, FAR_X as f64, 1.0)],
    );
    assert!(matches!(
        WorldOriginRebaseService.prepare(&origin, unreachable),
        Err(WorldOriginRebaseError::Position {
            entity: SUBJECT,
            ..
        })
    ));
    assert_eq!(origin.readout(), origin_before);
    assert_eq!(scene.world_origin(), scene_origin_before);

    // Commit rebases the live scene, so an edit made after prepare is kept.
    let prepared = WorldOriginRebaseService
        .prepare(
            &origin,
            request(WorldOrigin::new([FAR_X, 0, 0]), bindings()),
        )
        .unwrap();
    VoxelEditService::apply(
        &mut scene,
        &[VoxelEdit::Clear {
            address: [FAR_X + 2, 0, 0],
        }],
    )
    .unwrap();
    let edited_source = scene.source_revision();
    let receipt = commit(&mut origin, &mut scene, &prepared);
    assert_eq!(receipt.origin_after, WorldOrigin::new([FAR_X, 0, 0]));
    assert_eq!(scene.source_revision(), edited_source);
    assert!(scene
        .raycast([2.5, 3.0, 0.5], [0.0, -1.0, 0.0], 4.0)
        .is_none());
    assert!(scene
        .raycast([1.5, 3.0, 0.5], [0.0, -1.0, 0.0], 4.0)
        .is_some());
}
