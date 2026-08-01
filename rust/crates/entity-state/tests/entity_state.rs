use core_ids::EntityId;
use core_math::Vec3;
use entity_state::{
    decode_snapshot, encode_snapshot, EntityCommand, EntityCommandBatch, EntityCommandError,
    EntityDefinition, EntityDefinitionError, EntityFact, EntityState, EntityTransform,
};

fn door_fixture() -> EntityState {
    EntityState::from_definitions([
        EntityDefinition::new(EntityId::new(9), "room"),
        EntityDefinition::new(EntityId::new(10), "security-door")
            .with_transform(Vec3::ZERO)
            .with_collision(true, true)
            .with_renderable("mesh/security-door", true)
            .with_containment(EntityId::new(9)),
    ])
    .expect("valid fixture")
}

#[test]
fn atomic_batch_applies_related_component_changes_once() {
    let mut entities = door_fixture();
    let receipt = entities
        .apply_batch(EntityCommandBatch::new([
            EntityCommand::SetTranslation {
                entity: EntityId::new(10),
                translation: Vec3::new(0.0, 3.0, 0.0),
            },
            EntityCommand::SetCollisionEnabled {
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
        EntityFact::TranslationChanged { .. }
    ));
    assert!(matches!(
        receipt.facts[1],
        EntityFact::CollisionChanged { .. }
    ));
    let view = entities.view(EntityId::new(10)).expect("door view");
    assert_eq!(view.transform.expect("transform").translation.y, 3.0);
    assert!(!view.collision.expect("collision").enabled);
}

#[test]
fn rejected_batch_leaves_every_component_unchanged() {
    let mut entities = door_fixture();
    let rejection = entities
        .apply_batch(EntityCommandBatch::new([EntityCommand::SetTranslation {
            entity: EntityId::new(10),
            translation: Vec3::new(0.0, 3.0, 0.0),
        }]))
        .expect_err("enabled static collider cannot move");

    assert_eq!(
        rejection.reason,
        EntityCommandError::StaticColliderMovement {
            entity: EntityId::new(10)
        }
    );
    assert_eq!(entities.revision(), 0);
    assert_eq!(
        entities
            .relationships(EntityId::new(10))
            .unwrap()
            .contained_in,
        Some(EntityId::new(9))
    );
    let view = entities.view(EntityId::new(10)).expect("door view");
    assert_eq!(view.transform.expect("transform").translation, Vec3::ZERO);
    assert!(view.collision.expect("collision").enabled);
}

#[test]
fn snapshot_round_trip_preserves_entity_state_and_projection() {
    let mut entities = door_fixture();
    entities
        .apply_batch(EntityCommandBatch::new([
            EntityCommand::SetCollisionEnabled {
                entity: EntityId::new(10),
                enabled: false,
            },
            EntityCommand::SetTranslation {
                entity: EntityId::new(10),
                translation: Vec3::new(0.0, 3.0, 0.0),
            },
        ]))
        .expect("open door");

    let encoded = encode_snapshot(&entities).expect("encode");
    let restored = decode_snapshot(&encoded).expect("decode");
    assert_eq!(restored.revision(), 1);
    assert_eq!(
        restored.view(EntityId::new(10)),
        entities.view(EntityId::new(10))
    );
    assert_eq!(restored.projection(), entities.projection());
}

#[test]
fn renderable_local_transform_round_trips_without_moving_entity_authority() {
    let id = EntityId::new(11);
    let world = EntityTransform::at(Vec3::new(4.0, 3.0, 2.0));
    let visual_local = EntityTransform::at(Vec3::new(0.0, -1.25, 0.0));
    let entities = EntityState::from_definitions([EntityDefinition::new(id, "offset-mesh")
        .with_full_transform(world)
        .with_renderable("mesh/offset", true)
        .with_renderable_local_transform(visual_local)])
    .expect("valid renderable-local transform");

    assert_eq!(entities.world_transform(id), Some(world));
    assert_eq!(
        entities
            .view(id)
            .unwrap()
            .renderable
            .unwrap()
            .local_transform,
        visual_local
    );
    let encoded = encode_snapshot(&entities).expect("encode");
    assert!(encoded.contains("localTransform"));
    let restored = decode_snapshot(&encoded).expect("decode");
    assert_eq!(restored.world_transform(id), Some(world));
    assert_eq!(restored.projection(), entities.projection());
}

#[test]
fn identity_renderable_transform_keeps_legacy_snapshot_shape() {
    let encoded = encode_snapshot(&door_fixture()).expect("encode");
    assert!(!encoded.contains("localTransform"));
    let restored = decode_snapshot(&encoded).expect("decode legacy-compatible shape");
    assert_eq!(
        restored
            .view(EntityId::new(10))
            .unwrap()
            .renderable
            .unwrap()
            .local_transform,
        EntityTransform::IDENTITY
    );
}

#[test]
fn invalid_renderable_local_transform_is_typed_and_rejected() {
    let id = EntityId::new(12);
    let invalid = EntityState::from_definitions([EntityDefinition::new(id, "invalid-offset")
        .with_renderable("mesh/offset", true)
        .with_renderable_local_transform(EntityTransform::at(Vec3::new(0.0, f32::NAN, 0.0)))])
    .expect_err("non-finite visual offsets must reject");
    assert_eq!(
        invalid,
        EntityDefinitionError::InvalidRenderableTransform { entity: id }
    );
}

#[test]
fn snapshot_rejects_unknown_fields() {
    let encoded = encode_snapshot(&door_fixture()).expect("encode");
    let invalid = encoded.replacen("\"revision\": 0", "\"revision\": 0, \"mystery\": true", 1);
    assert!(decode_snapshot(&invalid).is_err());
}

#[test]
fn kinematic_component_round_trips_and_changes_atomically_with_position() {
    let id = EntityId::new(20);
    let mut entities =
        EntityState::from_definitions([EntityDefinition::new(id, "moving-platform")
            .with_transform(Vec3::new(1.0, 2.0, 3.0))
            .with_kinematic(Vec3::new(0.5, 0.25, 1.0), Vec3::new(4.0, 0.0, -2.0))])
        .expect("valid kinematic body");

    let receipt = entities
        .apply_batch(EntityCommandBatch::new([
            EntityCommand::SetTranslation {
                entity: id,
                translation: Vec3::new(5.0, 2.0, 1.0),
            },
            EntityCommand::SetKinematicVelocity {
                entity: id,
                velocity: Vec3::ZERO,
            },
        ]))
        .expect("position and velocity should commit together");

    assert_eq!(receipt.revision_after, 1);
    assert_eq!(receipt.facts.len(), 2);
    let restored = decode_snapshot(&encode_snapshot(&entities).expect("encode")).expect("decode");
    assert_eq!(restored.view(id), entities.view(id));
    assert_eq!(restored.kinematic_bodies().count(), 1);
}

#[test]
fn kinematic_component_requires_transform_and_positive_bounds() {
    let id = EntityId::new(21);
    let missing_transform =
        EntityState::from_definitions([EntityDefinition::new(id, "orphan-motion")
            .with_kinematic(Vec3::new(0.5, 0.5, 0.5), Vec3::ZERO)])
        .expect_err("kinematics without a transform must be rejected");
    assert_eq!(
        missing_transform,
        EntityDefinitionError::KinematicMissingTransform { entity: id }
    );

    let invalid_bounds = EntityState::from_definitions([EntityDefinition::new(id, "flat-motion")
        .with_transform(Vec3::ZERO)
        .with_kinematic(Vec3::new(0.5, 0.0, 0.5), Vec3::ZERO)])
    .expect_err("zero half extent must be rejected");
    assert_eq!(
        invalid_bounds,
        EntityDefinitionError::InvalidKinematicHalfExtents { entity: id }
    );
}
