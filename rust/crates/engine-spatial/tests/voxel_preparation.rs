use engine_spatial::{
    VoxelChunkIdentity, VoxelChunkLeaseRegistry, VoxelChunkPayload, VoxelChunkResidencyOperation,
    VoxelChunkResidencyService, VoxelChunkResidencyTransaction, VoxelCollisionScene,
    VoxelPreparationPoll, VoxelResidencyPreparation,
};
use std::{
    sync::Arc,
    thread,
    time::{Duration, Instant},
};
fn operation(x: i64) -> VoxelChunkResidencyOperation {
    VoxelChunkResidencyOperation::Admit {
        chunk: VoxelChunkIdentity::new(x, 0, 0),
        payload: VoxelChunkPayload::new([8; 3], vec![1; 512]),
    }
}
fn finish(mut worker: VoxelResidencyPreparation) -> engine_spatial::PreparedVoxelChunkResidency {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match worker.poll() {
            VoxelPreparationPoll::Ready(result) => return (*result).unwrap(),
            VoxelPreparationPoll::WorkerFailed => panic!("worker failed"),
            VoxelPreparationPoll::Pending => {
                assert!(Instant::now() < deadline);
                thread::yield_now();
            }
        }
    }
}
#[test]
fn background_preparation_leaves_source_unchanged_and_commits_atomically() {
    let mut scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, []).unwrap();
    let leases = VoxelChunkLeaseRegistry::default();
    let revision = scene.source_revision();
    let worker = VoxelResidencyPreparation::start(
        Arc::new(scene.clone()),
        leases.clone(),
        revision,
        vec![operation(0)],
    )
    .unwrap();
    assert_eq!(scene.solid_voxel_count(), 0);
    let prepared = finish(worker);
    assert_eq!(scene.solid_voxel_count(), 0);
    VoxelChunkResidencyService::commit(&mut scene, &leases, prepared).unwrap();
    assert_eq!(scene.solid_voxel_count(), 512);
    assert_ne!(scene.source_revision(), revision);
}
#[test]
fn stale_background_candidate_cannot_overwrite_newer_scene() {
    let mut scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, []).unwrap();
    let leases = VoxelChunkLeaseRegistry::default();
    let revision = scene.source_revision();
    let worker = VoxelResidencyPreparation::start(
        Arc::new(scene.clone()),
        leases.clone(),
        revision,
        vec![operation(0)],
    )
    .unwrap();
    VoxelChunkResidencyService::apply(
        &mut scene,
        &leases,
        VoxelChunkResidencyTransaction {
            expected_scene_source_revision: revision,
            operations: &[operation(1)],
        },
    )
    .unwrap();
    let current = scene.source_revision();
    assert!(VoxelChunkResidencyService::commit(&mut scene, &leases, finish(worker)).is_err());
    assert_eq!(scene.source_revision(), current);
    assert!(
        VoxelChunkResidencyService::resident_chunk(&scene, VoxelChunkIdentity::ORIGIN).is_none()
    );
}
#[test]
fn dropping_preparation_discards_result_without_publication() {
    let scene = Arc::new(VoxelCollisionScene::from_solid_voxels(1.0, 8, []).unwrap());
    let worker = VoxelResidencyPreparation::start(
        Arc::clone(&scene),
        VoxelChunkLeaseRegistry::default(),
        scene.source_revision(),
        vec![operation(0)],
    )
    .unwrap();
    drop(worker);
    assert_eq!(scene.solid_voxel_count(), 0);
    assert_eq!(Arc::strong_count(&scene), 1);
}

#[test]
fn rebase_during_preparation_rejects_old_local_projections() {
    use engine_spatial::{
        VoxelChunkResidencyApplyError, WorldOriginRebaseRequest, WorldOriginRebaseService,
        WorldOriginState,
    };
    let mut scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, []).unwrap();
    let leases = VoxelChunkLeaseRegistry::default();
    let prepared = finish(
        VoxelResidencyPreparation::start(
            Arc::new(scene.clone()),
            leases.clone(),
            scene.source_revision(),
            vec![operation(0)],
        )
        .unwrap(),
    );
    let mut origin = WorldOriginState::default();
    let mut entities = entity_state::EntityState::from_definitions([]).unwrap();
    let request = WorldOriginRebaseRequest {
        expected_origin_revision: origin.revision(),
        expected_entity_revision: entities.revision(),
        expected_voxel_source_revision: scene.source_revision().raw(),
        expected_static_mesh_revision: scene.static_mesh_collision_revision(),
        target_origin: core_space::WorldOrigin::new([8, 0, 0]),
        entities: vec![],
    };
    WorldOriginRebaseService
        .apply(&mut origin, &mut entities, &mut scene, request)
        .unwrap();
    assert!(matches!(
        VoxelChunkResidencyService::commit(&mut scene, &leases, prepared),
        Err(VoxelChunkResidencyApplyError::PreparedOriginChanged { .. })
    ));
    assert_eq!(scene.world_origin(), origin.origin());
    assert_eq!(scene.solid_voxel_count(), 0);
}
