use core_ids::EntityId;
use core_math::Vec3;
use entity_state::{
    decode_snapshot, encode_snapshot, CharacterMotionComponent, EntityAuthoringService,
    EntityDefinition, EntityState, EntityStateSnapshotError, KinematicSnapshot, RigidBodyComponent,
    RigidBodyShape,
};

#[test]
fn valid_character_motion_snapshot_round_trips() {
    let entity = EntityId::new(91);
    let state = EntityState::from_definitions([EntityDefinition::new(entity, "character")
        .with_transform(Vec3::new(0.0, 1.9, 0.0))
        .with_character_motion(CharacterMotionComponent::at_rest(1.9))])
    .unwrap();
    let restored = decode_snapshot(&encode_snapshot(&state).unwrap()).unwrap();
    assert_eq!(restored.transform(entity), state.transform(entity));
    assert_eq!(
        restored.character_motion(entity),
        state.character_motion(entity)
    );
}

#[test]
fn snapshot_restore_rejects_character_motion_with_kinematic() {
    let entity = EntityId::new(92);
    let state = EntityState::from_definitions([EntityDefinition::new(entity, "character")
        .with_transform(Vec3::new(0.0, 1.9, 0.0))
        .with_character_motion(CharacterMotionComponent::at_rest(1.9))])
    .unwrap();
    let mut forged = state.snapshot();
    forged.entities[0].kinematic = Some(KinematicSnapshot {
        half_extents: [0.35, 0.9, 0.35],
        velocity: [0.0; 3],
    });
    let bytes = serde_json::to_string_pretty(&forged).unwrap();
    assert!(matches!(
        decode_snapshot(&bytes),
        Err(EntityStateSnapshotError::ConflictingComponents {
            entity: 92,
            first: "character-motion",
            second: "kinematic",
        })
    ));
}

#[test]
fn snapshot_restore_rejects_character_motion_with_rigid_body() {
    let entity = EntityId::new(93);
    let character = EntityState::from_definitions([EntityDefinition::new(entity, "character")
        .with_transform(Vec3::new(0.0, 1.9, 0.0))
        .with_character_motion(CharacterMotionComponent::at_rest(1.9))])
    .unwrap();

    let mut rigid = EntityState::from_definitions([
        EntityDefinition::new(entity, "rigid").with_transform(Vec3::new(0.0, 1.9, 0.0))
    ])
    .unwrap();
    let body = RigidBodyComponent::dynamic(RigidBodyShape::Sphere { radius: 0.5 }, 1.0);
    let slot = rigid
        .component_revision::<RigidBodyComponent>(entity)
        .unwrap();
    EntityAuthoringService
        .attach_component(&mut rigid, slot, entity, body)
        .unwrap();

    let mut forged = character.snapshot();
    let rigid_record = rigid
        .snapshot()
        .registered_components
        .into_iter()
        .find(|record| record.type_id == entity_state::RIGID_BODY_COMPONENT_TYPE_ID)
        .unwrap();
    forged.registered_components.push(rigid_record);
    let bytes = serde_json::to_string_pretty(&forged).unwrap();
    assert!(matches!(
        decode_snapshot(&bytes),
        Err(EntityStateSnapshotError::ConflictingComponents {
            entity: 93,
            first: "character-motion",
            second: "rigid-body",
        })
    ));
}
