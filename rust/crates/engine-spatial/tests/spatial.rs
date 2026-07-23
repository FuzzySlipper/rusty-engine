use core_ids::EntityId;
use core_math::Vec3;
use engine_spatial::{KinematicMotionSystem, MotionAxis, MotionFact, VoxelCollisionScene};
use world_kernel::{EntityDefinition, WorldKernel};

#[test]
fn donor_collision_queries_cover_chunks_negative_space_and_raycast() {
    let scene = VoxelCollisionScene::from_solid_voxels(1.0, 4, [[2, 1, 0], [-1, 0, 0], [2, 1, 0]])
        .expect("valid scene");

    assert_eq!(scene.solid_voxel_count(), 2);
    assert_eq!(scene.resident_chunk_count(), 2);
    assert!(scene.contains_point([2.5, 1.5, 0.5]));
    assert!(scene.contains_point([-0.5, 0.5, 0.5]));
    assert!(!scene.contains_point([1.5, 1.5, 0.5]));
    let hit = scene
        .raycast([0.5, 1.5, 0.5], [1.0, 0.0, 0.0], 10.0)
        .expect("wall should be hit");
    assert_eq!(hit.voxel, [2, 1, 0]);
    assert_eq!(hit.distance, 1.5);
}

#[test]
fn central_motion_phase_blocks_one_axis_without_tunneling() {
    let scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, [[2, 1, 0]]).expect("valid scene");
    let id = EntityId::new(1);
    let mut world = WorldKernel::from_definitions([EntityDefinition::new(id, "runner")
        .with_transform(Vec3::new(0.5, 1.5, 0.5))
        .with_kinematic(Vec3::new(0.25, 0.25, 0.25), Vec3::new(8.0, 1.0, 0.0))])
    .expect("valid world");

    let receipt = KinematicMotionSystem::run(&mut world, &scene, 0.5).expect("motion phase");

    assert_eq!(receipt.bodies_considered, 1);
    assert_eq!(receipt.moved_bodies, 1);
    assert_eq!(receipt.blocked_axes, 1);
    assert_eq!(receipt.revision_before, 0);
    assert_eq!(receipt.revision_after, 1);
    assert!(receipt.facts.iter().any(|fact| matches!(
        fact,
        MotionFact::Blocked {
            entity,
            axis: MotionAxis::X,
            ..
        } if *entity == id
    )));
    let view = world.view(id).expect("runner");
    assert_eq!(
        view.transform.expect("transform").translation,
        Vec3::new(0.5, 2.0, 0.5)
    );
    assert_eq!(
        view.kinematic.expect("kinematic").velocity,
        Vec3::new(0.0, 1.0, 0.0)
    );
}

#[test]
fn one_motion_phase_commits_many_entities_at_one_revision() {
    let scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, []).expect("empty scene");
    let definitions = (1..=32).map(|raw| {
        EntityDefinition::new(EntityId::new(raw), format!("mover-{raw}"))
            .with_transform(Vec3::new(0.0, raw as f32, 0.0))
            .with_kinematic(Vec3::new(0.2, 0.2, 0.2), Vec3::new(2.0, 0.0, 0.0))
    });
    let mut world = WorldKernel::from_definitions(definitions).expect("valid movers");

    let receipt = KinematicMotionSystem::run(&mut world, &scene, 0.25).expect("motion phase");

    assert_eq!(receipt.bodies_considered, 32);
    assert_eq!(receipt.moved_bodies, 32);
    assert_eq!(receipt.revision_after, 1);
    assert_eq!(world.revision(), 1);
    assert_eq!(receipt.world_facts.len(), 32);
}

#[test]
fn scene_admission_bounds_dense_chunk_allocation() {
    assert!(matches!(
        VoxelCollisionScene::from_solid_voxels(1.0, 65, []),
        Err(engine_spatial::CollisionSceneError::InvalidChunkSize)
    ));
}
