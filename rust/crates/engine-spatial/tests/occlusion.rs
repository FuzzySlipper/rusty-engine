use core_ids::EntityId;
use engine_spatial::{
    SpatialOcclusionCollider, SpatialOcclusionError, SpatialOcclusionHit, SpatialOcclusionQuery,
    SpatialOcclusionService, VoxelCollisionScene,
};

const MOVER: EntityId = EntityId::new(1);
const DOOR: EntityId = EntityId::new(9);

fn cube(entity: EntityId, center: [f64; 3]) -> SpatialOcclusionCollider {
    SpatialOcclusionCollider {
        entity,
        min: center.map(|value| value - 0.5),
        max: center.map(|value| value + 0.5),
    }
}

#[test]
fn supplied_door_box_blocks_until_omitted_or_moved_away() {
    let scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, []).unwrap();
    let ignored = [MOVER];
    let query = SpatialOcclusionQuery {
        origin: [0.0, 0.0, 0.0],
        direction: [1.0, 0.0, 0.0],
        max_distance: 10.0,
        ignored_entities: &ignored,
    };
    let mover = cube(MOVER, [0.0, 0.0, 0.0]);

    assert_eq!(
        SpatialOcclusionService::cast_ray(&scene, query, [mover, cube(DOOR, [2.0, 0.0, 0.0])])
            .unwrap(),
        Some(SpatialOcclusionHit::Entity {
            entity: DOOR,
            point: [1.5, 0.0, 0.0],
            distance: 1.5,
        })
    );
    assert_eq!(
        SpatialOcclusionService::cast_ray(&scene, query, [mover]).unwrap(),
        None
    );
    assert_eq!(
        SpatialOcclusionService::cast_ray(&scene, query, [mover, cube(DOOR, [20.0, 0.0, 0.0])])
            .unwrap(),
        None
    );
}

#[test]
fn strict_nearest_order_selects_entity_then_voxel_when_entity_is_ignored() {
    let scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, [[4, 0, 0]]).unwrap();
    let front = EntityId::new(3);
    let behind = EntityId::new(7);
    let colliders = [cube(front, [2.0, 0.5, 0.5]), cube(behind, [6.0, 0.5, 0.5])];
    let base = SpatialOcclusionQuery {
        origin: [0.5, 0.5, 0.5],
        direction: [1.0, 0.0, 0.0],
        max_distance: 10.0,
        ignored_entities: &[],
    };

    assert!(matches!(
        SpatialOcclusionService::cast_ray(&scene, base, colliders).unwrap(),
        Some(SpatialOcclusionHit::Entity {
            entity,
            distance: 1.0,
            ..
        }) if entity == front
    ));
    assert!(matches!(
        SpatialOcclusionService::cast_ray(
            &scene,
            SpatialOcclusionQuery {
                ignored_entities: &[front],
                ..base
            },
            colliders,
        )
        .unwrap(),
        Some(SpatialOcclusionHit::Voxel(hit))
            if hit.voxel == [4, 0, 0] && hit.distance == 3.5
    ));
}

#[test]
fn exact_ties_prefer_lowest_entity_identity_before_voxel() {
    let scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, [[2, 0, 0]]).unwrap();
    let lower = EntityId::new(3);
    let higher = EntityId::new(9);
    let colliders = [cube(higher, [2.5, 0.5, 0.5]), cube(lower, [2.5, 0.5, 0.5])];
    let base = SpatialOcclusionQuery {
        origin: [0.5, 0.5, 0.5],
        direction: [5.0, 0.0, 0.0],
        max_distance: 10.0,
        ignored_entities: &[],
    };

    assert_eq!(
        SpatialOcclusionService::cast_ray(&scene, base, colliders).unwrap(),
        Some(SpatialOcclusionHit::Entity {
            entity: lower,
            point: [2.0, 0.5, 0.5],
            distance: 1.5,
        })
    );
    assert!(matches!(
        SpatialOcclusionService::cast_ray(
            &scene,
            SpatialOcclusionQuery {
                ignored_entities: &[lower, higher],
                ..base
            },
            colliders,
        )
        .unwrap(),
        Some(SpatialOcclusionHit::Voxel(hit))
            if hit.voxel == [2, 0, 0] && hit.distance == 1.5
    ));
}

#[test]
fn large_queries_run_and_invalid_queries_are_typed() {
    // 5,000 boxes and 16 ignored identities: past the former 4,096 and 8 caps.
    let scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, [[2, 0, 0]]).unwrap();
    let colliders: Vec<_> = (1..=5_000)
        .map(|raw| cube(EntityId::new(raw), [0.5, 100.0 + raw as f64, 0.5]))
        .collect();
    let ignored: Vec<EntityId> = (1..=16).map(EntityId::new).collect();
    let query = SpatialOcclusionQuery {
        origin: [0.5, 0.5, 0.5],
        direction: [1.0, 0.0, 0.0],
        max_distance: 10.0,
        ignored_entities: &ignored,
    };

    assert!(matches!(
        SpatialOcclusionService::cast_ray(&scene, query, colliders.iter().copied()).unwrap(),
        Some(SpatialOcclusionHit::Voxel(hit)) if hit.voxel == [2, 0, 0]
    ));
    assert_eq!(
        SpatialOcclusionService::cast_ray(
            &scene,
            SpatialOcclusionQuery {
                direction: [0.0, 0.0, 0.0],
                ..query
            },
            colliders.iter().copied(),
        ),
        Err(SpatialOcclusionError::InvalidDirection)
    );
}
