use core_ids::EntityId;
use core_math::Vec3;
use entity_state::{
    decode_snapshot, encode_snapshot, CharacterMotionComponent, EntityDefinition, EntityState,
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
fn far_fast_entities_and_long_timers_are_ordinary_values() {
    // Past the former 1e6 translation, 1e4 speed and 60 s timer bounds.
    let entity = EntityId::new(93);
    let mut motion = CharacterMotionComponent::at_rest(2_000_000.0);
    motion.external_velocity = Vec3::new(20_000.0, 0.0, 0.0);
    motion.coyote_remaining = 90.0;
    let state = EntityState::from_definitions([EntityDefinition::new(entity, "far character")
        .with_transform(Vec3::new(2_000_000.0, 2_000_000.0, 0.0))
        .with_character_motion(motion)])
    .unwrap();
    let restored = decode_snapshot(&encode_snapshot(&state).unwrap()).unwrap();
    assert_eq!(
        restored.character_motion(entity),
        state.character_motion(entity)
    );
}
