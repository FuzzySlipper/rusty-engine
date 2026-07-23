use core_ids::EntityId;
use core_math::Vec3;
use world_kernel::{
    decode_snapshot, encode_snapshot, EntityDefinition, EntityDefinitionError, WorldCommand,
    WorldCommandBatch, WorldCommandError, WorldFact, WorldKernel,
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
        .apply_batch(WorldCommandBatch::new([
            WorldCommand::SetTranslation {
                entity: EntityId::new(10),
                translation: Vec3::new(0.0, 3.0, 0.0),
            },
            WorldCommand::SetCollisionEnabled {
                entity: EntityId::new(10),
                enabled: false,
            },
        ]))
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

#[test]
fn kinematic_capability_round_trips_and_changes_atomically_with_position() {
    let id = EntityId::new(20);
    let mut world = WorldKernel::from_definitions([EntityDefinition::new(id, "moving-platform")
        .with_transform(Vec3::new(1.0, 2.0, 3.0))
        .with_kinematic(Vec3::new(0.5, 0.25, 1.0), Vec3::new(4.0, 0.0, -2.0))])
    .expect("valid kinematic body");

    let receipt = world
        .apply_batch(WorldCommandBatch::new([
            WorldCommand::SetTranslation {
                entity: id,
                translation: Vec3::new(5.0, 2.0, 1.0),
            },
            WorldCommand::SetKinematicVelocity {
                entity: id,
                velocity: Vec3::ZERO,
            },
        ]))
        .expect("position and velocity should commit together");

    assert_eq!(receipt.revision_after, 1);
    assert_eq!(receipt.facts.len(), 2);
    let restored = decode_snapshot(&encode_snapshot(&world).expect("encode")).expect("decode");
    assert_eq!(restored.view(id), world.view(id));
    assert_eq!(restored.kinematic_bodies().count(), 1);
}

#[test]
fn kinematic_capability_requires_transform_and_positive_bounds() {
    let id = EntityId::new(21);
    let missing_transform =
        WorldKernel::from_definitions([EntityDefinition::new(id, "orphan-motion")
            .with_kinematic(Vec3::new(0.5, 0.5, 0.5), Vec3::ZERO)])
        .expect_err("kinematics without a transform must be rejected");
    assert_eq!(
        missing_transform,
        EntityDefinitionError::KinematicMissingTransform { entity: id }
    );

    let invalid_bounds = WorldKernel::from_definitions([EntityDefinition::new(id, "flat-motion")
        .with_transform(Vec3::ZERO)
        .with_kinematic(Vec3::new(0.5, 0.0, 0.5), Vec3::ZERO)])
    .expect_err("zero half extent must be rejected");
    assert_eq!(
        invalid_bounds,
        EntityDefinitionError::InvalidKinematicHalfExtents { entity: id }
    );
}
