use core_ids::EntityId;
use engine_spatial::{
    SpatialPerceptionError, SpatialPerceptionObserver, SpatialPerceptionPairKind,
    SpatialPerceptionQuery, SpatialPerceptionService, SpatialPerceptionTarget, VoxelCollisionScene,
};
use entity_state::EntityState;

fn observer(entity: u64, origin: [f64; 3], evidence: f64) -> SpatialPerceptionObserver {
    SpatialPerceptionObserver {
        entity: EntityId::new(entity),
        origin,
        forward: [1.0, 0.0, 0.0],
        maximum_distance: 10.0,
        minimum_facing_cosine: 0.5,
        evidence,
    }
}

fn target(entity: u64, center: [f64; 3]) -> SpatialPerceptionTarget {
    SpatialPerceptionTarget {
        entity: EntityId::new(entity),
        center,
    }
}

fn query<'a>(
    scene: &'a VoxelCollisionScene,
    entities: &'a EntityState,
    observers: &'a [SpatialPerceptionObserver],
    targets: &'a [SpatialPerceptionTarget],
) -> SpatialPerceptionQuery<'a> {
    SpatialPerceptionQuery {
        scene,
        entities,
        observers,
        targets,
    }
}

#[test]
fn evaluates_distance_facing_and_deterministic_visible_reduction() {
    let scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, []).unwrap();
    let entities = EntityState::default();
    let observers = [
        observer(9, [0.0, 0.0, 0.0], 0.25),
        observer(3, [0.0, 0.0, 0.0], 0.75),
    ];
    let targets = [target(22, [4.0, 0.0, 0.0]), target(11, [4.0, 0.0, 0.0])];
    let readout = SpatialPerceptionService
        .evaluate(query(&scene, &entities, &observers, &targets))
        .unwrap();

    assert_eq!(readout.selected_observers, 2);
    assert_eq!(readout.selected_targets, 2);
    assert_eq!(readout.selection_comparisons, 4);
    assert_eq!(readout.distance_rejects, 0);
    assert_eq!(readout.facing_rejects, 0);
    assert_eq!(readout.visibility_casts, 4);
    assert_eq!(readout.occlusion_rejects, 0);
    assert_eq!(readout.pairs[0].observer, EntityId::new(3));
    assert_eq!(readout.pairs[0].target, EntityId::new(11));
    assert_eq!(readout.pairs[0].kind, SpatialPerceptionPairKind::Visible);
    assert_eq!(readout.aggregates[0].target, EntityId::new(11));
    assert_eq!(readout.aggregates[0].visible_observer_count, 2);
    assert_eq!(readout.aggregates[0].evidence_total, 1.0);
    assert_eq!(readout.aggregates[1].target, EntityId::new(22));
}

#[test]
fn reports_facing_rejection_and_voxel_occlusion_as_typed_pair_facts() {
    let scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, [[2, 0, 0]]).unwrap();
    let entities = EntityState::default();
    let mut behind_observer = observer(1, [0.0, 0.0, 0.0], 1.0);
    behind_observer.forward = [-1.0, 0.0, 0.0];
    let observers = [behind_observer, observer(2, [0.0, 2.0, 0.0], 2.0)];
    let targets = [target(7, [4.0, 0.0, 0.0])];
    let readout = SpatialPerceptionService
        .evaluate(query(&scene, &entities, &observers, &targets))
        .unwrap();

    assert_eq!(readout.facing_rejects, 1);
    assert_eq!(readout.visibility_casts, 1);
    assert_eq!(readout.occlusion_rejects, 1);
    assert!(readout.aggregates.is_empty());
    assert_eq!(
        readout.pairs[0].kind,
        SpatialPerceptionPairKind::FacingRejected
    );
    assert_eq!(readout.pairs[1].kind, SpatialPerceptionPairKind::Occluded);
}

#[test]
fn distance_rejected_pairs_are_not_retained_and_duplicate_ids_are_rejected() {
    let scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, []).unwrap();
    let entities = EntityState::default();
    let observers = [observer(1, [0.0, 0.0, 0.0], 1.0)];
    let targets = [target(2, [100.0, 0.0, 0.0])];
    let readout = SpatialPerceptionService
        .evaluate(query(&scene, &entities, &observers, &targets))
        .unwrap();
    assert_eq!(readout.distance_rejects, 1);
    assert!(readout.pairs.is_empty());

    let duplicate_observers = [
        observer(1, [0.0, 0.0, 0.0], 1.0),
        observer(1, [1.0, 0.0, 0.0], 1.0),
    ];
    assert_eq!(
        SpatialPerceptionService.evaluate(query(&scene, &entities, &duplicate_observers, &targets)),
        Err(SpatialPerceptionError::DuplicateObserver(EntityId::new(1)))
    );
}
