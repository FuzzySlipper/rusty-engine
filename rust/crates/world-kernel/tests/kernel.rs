use core_ids::EntityId;
use core_math::Vec3;
use world_kernel::{
    decode_snapshot, encode_snapshot, EntityDefinition, WorldCommand, WorldCommandBatch,
    WorldCommandError, WorldFact, WorldKernel,
};

fn door_world() -> WorldKernel {
    WorldKernel::from_definitions([EntityDefinition::new(EntityId::new(10), "security-door")
        .with_transform(Vec3::ZERO)
        .with_collision(true, true)
        .with_renderable("mesh/security-door", true)])
    .expect("valid fixture")
}

#[test]
fn atomic_batch_applies_related_capability_changes_once() {
    let mut world = door_world();
    let receipt = world
        .apply_batch(
            WorldCommandBatch::new([
                WorldCommand::SetTranslation {
                    entity: EntityId::new(10),
                    translation: Vec3::new(0.0, 3.0, 0.0),
                },
                WorldCommand::SetCollisionEnabled {
                    entity: EntityId::new(10),
                    enabled: false,
                },
            ])
            .expecting(0),
        )
        .expect("batch should be valid regardless of command order");

    assert_eq!(receipt.revision_before, 0);
    assert_eq!(receipt.revision_after, 1);
    assert_eq!(receipt.facts.len(), 2);
    assert!(matches!(
        receipt.facts[0],
        WorldFact::TranslationChanged { .. }
    ));
    assert!(matches!(
        receipt.facts[1],
        WorldFact::CollisionChanged { .. }
    ));
    let view = world.view(EntityId::new(10)).expect("door view");
    assert_eq!(view.transform.expect("transform").translation.y, 3.0);
    assert!(!view.collision.expect("collision").enabled);
}

#[test]
fn rejected_batch_leaves_every_capability_unchanged() {
    let mut world = door_world();
    let rejection = world
        .apply_batch(WorldCommandBatch::new([WorldCommand::SetTranslation {
            entity: EntityId::new(10),
            translation: Vec3::new(0.0, 3.0, 0.0),
        }]))
        .expect_err("enabled static collider cannot move");

    assert_eq!(
        rejection.reason,
        WorldCommandError::StaticColliderMovement {
            entity: EntityId::new(10)
        }
    );
    assert_eq!(world.revision(), 0);
    let view = world.view(EntityId::new(10)).expect("door view");
    assert_eq!(view.transform.expect("transform").translation, Vec3::ZERO);
    assert!(view.collision.expect("collision").enabled);
}

#[test]
fn stale_revision_rejects_before_mutation() {
    let mut world = door_world();
    let rejection = world
        .apply_batch(
            WorldCommandBatch::new([WorldCommand::SetCollisionEnabled {
                entity: EntityId::new(10),
                enabled: false,
            }])
            .expecting(7),
        )
        .expect_err("revision must match");
    assert_eq!(
        rejection.reason,
        WorldCommandError::StaleRevision {
            expected: 7,
            actual: 0
        }
    );
    assert!(
        world
            .view(EntityId::new(10))
            .expect("door")
            .collision
            .expect("collision")
            .enabled
    );
}

#[test]
fn snapshot_round_trip_preserves_world_and_projection() {
    let mut world = door_world();
    world
        .apply_batch(WorldCommandBatch::new([
            WorldCommand::SetCollisionEnabled {
                entity: EntityId::new(10),
                enabled: false,
            },
            WorldCommand::SetTranslation {
                entity: EntityId::new(10),
                translation: Vec3::new(0.0, 3.0, 0.0),
            },
        ]))
        .expect("open door");

    let encoded = encode_snapshot(&world).expect("encode");
    let restored = decode_snapshot(&encoded).expect("decode");
    assert_eq!(restored.revision(), 1);
    assert_eq!(
        restored.view(EntityId::new(10)),
        world.view(EntityId::new(10))
    );
    assert_eq!(restored.projection(), world.projection());
}

#[test]
fn snapshot_rejects_unknown_fields() {
    let encoded = encode_snapshot(&door_world()).expect("encode");
    let invalid = encoded.replacen("\"revision\": 0", "\"revision\": 0, \"mystery\": true", 1);
    assert!(decode_snapshot(&invalid).is_err());
}
