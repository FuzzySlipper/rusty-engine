use core_ids::EntityId;
use core_math::Vec3;
use engine_spatial::{
    GeneratedRoomConfig, KinematicMotionSystem, MotionAxis, MotionFact, VoxelCollisionScene,
};
use entity_state::{EntityDefinition, EntityState};

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
    let mut entities = EntityState::from_definitions([EntityDefinition::new(id, "runner")
        .with_transform(Vec3::new(0.5, 1.5, 0.5))
        .with_kinematic(Vec3::new(0.25, 0.25, 0.25), Vec3::new(8.0, 1.0, 0.0))])
    .expect("valid entity state");

    let receipt = KinematicMotionSystem::run(&mut entities, &scene, 0.5).expect("motion phase");

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
    let view = entities.view(id).expect("runner");
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
    let mut entities = EntityState::from_definitions(definitions).expect("valid movers");

    let receipt = KinematicMotionSystem::run(&mut entities, &scene, 0.25).expect("motion phase");

    assert_eq!(receipt.bodies_considered, 32);
    assert_eq!(receipt.moved_bodies, 32);
    assert_eq!(receipt.revision_after, 1);
    assert_eq!(entities.revision(), 1);
    assert_eq!(receipt.entity_facts.len(), 32);
}

#[test]
fn scene_admission_bounds_dense_chunk_allocation() {
    assert!(matches!(
        VoxelCollisionScene::from_solid_voxels(1.0, 65, []),
        Err(engine_spatial::CollisionSceneError::InvalidChunkSize)
    ));
}

#[test]
fn generated_room_is_deterministic_and_seed_changes_canonical_voxels_and_mesh() {
    let config = room_config(4);
    let first = VoxelCollisionScene::from_generated_room(config).unwrap();
    let repeated = VoxelCollisionScene::from_generated_room(config).unwrap();
    let variation = VoxelCollisionScene::from_generated_room(room_config(9)).unwrap();

    assert_eq!(first.generated_room(), repeated.generated_room());
    assert_eq!(first.material_voxels(), repeated.material_voxels());
    assert_eq!(first.mesh_chunks(), repeated.mesh_chunks());
    assert_ne!(
        first.generated_room().unwrap().1.pillar_voxel,
        variation.generated_room().unwrap().1.pillar_voxel,
    );
    assert_ne!(first.material_voxels(), variation.material_voxels());
    assert_ne!(
        first.mesh_chunks()[0].content_hash,
        variation.mesh_chunks()[0].content_hash,
    );
}

#[test]
fn generated_pillar_drives_collision_navigation_and_visible_mesh_from_one_world() {
    let scene = VoxelCollisionScene::from_generated_room(room_config(4)).unwrap();
    let record = scene.generated_room().unwrap().1;
    let [x, y, z] = record.pillar_voxel;

    assert!(scene
        .material_voxels()
        .iter()
        .any(|voxel| voxel.address == record.pillar_voxel && voxel.material_slot == 3));
    assert!(scene.contains_point([x as f64 + 0.5, y as f64 + 0.5, z as f64 + 0.5]));
    let navigation = scene
        .navigation_step(
            Vec3::new(1.5, 1.5, 6.5),
            Vec3::new(7.5, 1.5, 6.5),
            Vec3::ZERO,
            0.1,
            512,
        )
        .unwrap();
    assert!(
        navigation.path_len > 7,
        "route must detour around the pillar"
    );
    let mesh = &scene.mesh_chunks()[0];
    assert!(mesh.vertices > 0);
    assert!(mesh.quads > 0);
    assert!(mesh.faces_culled > 0);
    assert!(mesh.groups.iter().any(|group| group.material_slot == 3));
}

#[test]
fn generated_exit_aperture_is_canonical_collision_navigation_and_mesh_empty_space() {
    let scene = VoxelCollisionScene::from_generated_room(room_config(4)).unwrap();
    let record = scene.generated_room().unwrap().1;

    assert_eq!(record.exit_aperture_min, [3, 1, 11]);
    assert_eq!(record.exit_aperture_max_exclusive, [6, 3, 12]);
    for x in 3..6 {
        for y in 1..3 {
            assert!(!scene.contains_point([x as f64 + 0.5, y as f64 + 0.5, 11.5]));
            assert!(!scene
                .material_voxels()
                .iter()
                .any(|voxel| voxel.address == [x, y, 11]));
        }
    }
    assert!(scene.contains_point([2.5, 1.5, 11.5]));
    assert!(scene.contains_point([6.5, 1.5, 11.5]));
    assert!(scene
        .navigation_step(
            Vec3::new(4.5, 1.5, 10.5),
            Vec3::new(4.5, 1.5, 12.5),
            Vec3::ZERO,
            0.4,
            64,
        )
        .is_ok());
}

#[test]
fn bounded_room_fixture_stays_one_chunk_with_reviewable_mesh_counts() {
    let scene = VoxelCollisionScene::from_generated_room(GeneratedRoomConfig {
        seed: 41,
        voxel_size: 1.0,
        chunk_size: 32,
        width: 15,
        height: 6,
        length: 20,
    })
    .unwrap();
    let mesh = &scene.mesh_chunks()[0];

    assert_eq!(scene.resident_chunk_count(), 1);
    assert!(scene.solid_voxel_count() < 2_000);
    assert!(mesh.vertices < 20_000);
}

fn room_config(seed: u64) -> GeneratedRoomConfig {
    GeneratedRoomConfig {
        seed,
        voxel_size: 1.0,
        chunk_size: 16,
        width: 7,
        height: 4,
        length: 10,
    }
}
