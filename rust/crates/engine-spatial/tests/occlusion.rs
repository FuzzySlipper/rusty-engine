use core_ids::EntityId;
use core_math::Vec3;
use engine_spatial::{
    EntityMotionCommand, EntityMotionOutcome, EntityMotionService, SpatialOcclusionError,
    SpatialOcclusionHit, SpatialOcclusionQuery, SpatialOcclusionService, VoxelCollisionScene,
    MAX_OCCLUSION_IGNORED_ENTITIES, MAX_OCCLUSION_QUERY_ENTITIES,
};
use entity_state::{EntityCommand, EntityCommandBatch, EntityDefinition, EntityState};

const MOVER: EntityId = EntityId::new(1);
const DOOR: EntityId = EntityId::new(9);

#[test]
fn hidden_active_door_matches_motion_until_disabled_or_moved_open() {
    let scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, []).unwrap();
    let mut entities = motion_fixture();
    let ignored = [MOVER];
    let query = || SpatialOcclusionQuery {
        origin: [0.0, 0.0, 0.0],
        direction: [1.0, 0.0, 0.0],
        max_distance: 10.0,
        ignored_entities: &ignored,
    };

    let hit = SpatialOcclusionService
        .cast_ray(&scene, &entities, query())
        .unwrap()
        .unwrap();
    assert_eq!(
        hit,
        SpatialOcclusionHit::Entity {
            entity: DOOR,
            point: [1.5, 0.0, 0.0],
            distance: 1.5,
        }
    );
    let blocked = EntityMotionService
        .resolve(
            &entities,
            EntityMotionCommand {
                entity: MOVER,
                delta: Vec3::new(2.0, 0.0, 0.0),
            },
        )
        .unwrap();
    assert_eq!(blocked.hit, Some(DOOR));
    assert_eq!(
        blocked.outcome,
        EntityMotionOutcome::Blocked { at: Vec3::ZERO }
    );

    entities
        .apply_batch(EntityCommandBatch::new([
            EntityCommand::SetCollisionEnabled {
                entity: DOOR,
                enabled: false,
            },
        ]))
        .unwrap();
    assert_eq!(
        SpatialOcclusionService
            .cast_ray(&scene, &entities, query())
            .unwrap(),
        None
    );
    assert!(matches!(
        EntityMotionService
            .resolve(
                &entities,
                EntityMotionCommand {
                    entity: MOVER,
                    delta: Vec3::new(2.0, 0.0, 0.0),
                },
            )
            .unwrap()
            .outcome,
        EntityMotionOutcome::Moved { .. }
    ));

    entities
        .apply_batch(EntityCommandBatch::new([
            EntityCommand::SetTranslation {
                entity: DOOR,
                translation: Vec3::new(20.0, 0.0, 0.0),
            },
            EntityCommand::SetCollisionEnabled {
                entity: DOOR,
                enabled: true,
            },
        ]))
        .unwrap();
    assert_eq!(
        SpatialOcclusionService
            .cast_ray(&scene, &entities, query())
            .unwrap(),
        None
    );
}

#[test]
fn strict_nearest_order_selects_entity_then_voxel_when_entity_is_ignored() {
    let scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, [[4, 0, 0]]).unwrap();
    let front = EntityId::new(3);
    let behind = EntityId::new(7);
    let entities = EntityState::from_definitions([
        collider(front, Vec3::new(2.0, 0.5, 0.5)),
        collider(behind, Vec3::new(6.0, 0.5, 0.5)),
    ])
    .unwrap();
    let base = SpatialOcclusionQuery {
        origin: [0.5, 0.5, 0.5],
        direction: [1.0, 0.0, 0.0],
        max_distance: 10.0,
        ignored_entities: &[],
    };

    assert!(matches!(
        SpatialOcclusionService
            .cast_ray(&scene, &entities, base)
            .unwrap(),
        Some(SpatialOcclusionHit::Entity {
            entity,
            distance: 1.0,
            ..
        }) if entity == front
    ));
    assert!(matches!(
        SpatialOcclusionService
            .cast_ray(
                &scene,
                &entities,
                SpatialOcclusionQuery {
                    ignored_entities: &[front],
                    ..base
                },
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
    let entities = EntityState::from_definitions([
        collider(higher, Vec3::new(2.5, 0.5, 0.5)),
        collider(lower, Vec3::new(2.5, 0.5, 0.5)),
    ])
    .unwrap();
    let base = SpatialOcclusionQuery {
        origin: [0.5, 0.5, 0.5],
        direction: [5.0, 0.0, 0.0],
        max_distance: 10.0,
        ignored_entities: &[],
    };

    assert_eq!(
        SpatialOcclusionService
            .cast_ray(&scene, &entities, base)
            .unwrap(),
        Some(SpatialOcclusionHit::Entity {
            entity: lower,
            point: [2.0, 0.5, 0.5],
            distance: 1.5,
        })
    );
    assert!(matches!(
        SpatialOcclusionService
            .cast_ray(
                &scene,
                &entities,
                SpatialOcclusionQuery {
                    ignored_entities: &[lower, higher],
                    ..base
                },
            )
            .unwrap(),
        Some(SpatialOcclusionHit::Voxel(hit))
            if hit.voxel == [2, 0, 0] && hit.distance == 1.5
    ));
}

#[test]
fn invalid_and_over_quota_queries_are_typed_and_leave_authority_unchanged() {
    let scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, [[2, 0, 0]]).unwrap();
    let definitions = (1..=MAX_OCCLUSION_QUERY_ENTITIES + 1)
        .map(|raw| EntityDefinition::new(EntityId::new(raw as u64), format!("quota-entity-{raw}")));
    let entities = EntityState::from_definitions(definitions).unwrap();
    let entity_revision = entities.revision();
    let scene_hash = scene.authority_hash();
    let query = SpatialOcclusionQuery {
        origin: [0.5, 0.5, 0.5],
        direction: [1.0, 0.0, 0.0],
        max_distance: 10.0,
        ignored_entities: &[],
    };

    assert_eq!(
        SpatialOcclusionService.cast_ray(&scene, &entities, query),
        Err(SpatialOcclusionError::TooManyEntities {
            actual: MAX_OCCLUSION_QUERY_ENTITIES + 1,
            limit: MAX_OCCLUSION_QUERY_ENTITIES,
        })
    );
    let ignored = vec![EntityId::new(1); MAX_OCCLUSION_IGNORED_ENTITIES + 1];
    assert_eq!(
        SpatialOcclusionService.cast_ray(
            &scene,
            &EntityState::from_definitions(Vec::<EntityDefinition>::new()).unwrap(),
            SpatialOcclusionQuery {
                ignored_entities: &ignored,
                ..query
            },
        ),
        Err(SpatialOcclusionError::TooManyIgnoredEntities {
            actual: MAX_OCCLUSION_IGNORED_ENTITIES + 1,
            limit: MAX_OCCLUSION_IGNORED_ENTITIES,
        })
    );
    assert_eq!(
        SpatialOcclusionService.cast_ray(
            &scene,
            &EntityState::from_definitions(Vec::<EntityDefinition>::new()).unwrap(),
            SpatialOcclusionQuery {
                direction: [0.0, 0.0, 0.0],
                ..query
            },
        ),
        Err(SpatialOcclusionError::InvalidDirection)
    );
    assert_eq!(entities.revision(), entity_revision);
    assert_eq!(entities.total_count(), MAX_OCCLUSION_QUERY_ENTITIES + 1);
    assert_eq!(scene.authority_hash(), scene_hash);
}

fn motion_fixture() -> EntityState {
    EntityState::from_definitions([
        EntityDefinition::new(MOVER, "mover")
            .with_transform(Vec3::ZERO)
            .with_bounds(Vec3::splat(-0.5), Vec3::splat(0.5))
            .with_collision(true, false),
        EntityDefinition::new(DOOR, "hidden-door")
            .with_transform(Vec3::new(2.0, 0.0, 0.0))
            .with_bounds(Vec3::splat(-0.5), Vec3::splat(0.5))
            .with_collision(true, true)
            .with_renderable("mesh/door", false),
    ])
    .unwrap()
}

fn collider(entity: EntityId, translation: Vec3) -> EntityDefinition {
    EntityDefinition::new(entity, format!("collider-{}", entity.raw()))
        .with_transform(translation)
        .with_bounds(Vec3::splat(-0.5), Vec3::splat(0.5))
        .with_collision(true, true)
}
