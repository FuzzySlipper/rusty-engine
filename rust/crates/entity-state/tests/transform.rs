use core_ids::EntityId;
use core_math::Vec3;
use entity_state::{
    EntityDefinition, EntityState, EntityTransform, Quat, TransformCommand, TransformError,
    TransformService,
};

#[test]
fn full_transform_mutation_is_validated_and_revisioned() {
    let id = EntityId::new(1);
    let mut state = EntityState::from_definitions([EntityDefinition::new(id, "visible")
        .with_transform(Vec3::ZERO)
        .with_renderable("mesh/visible", true)])
    .expect("fixture");
    let next = EntityTransform {
        translation: Vec3::new(2.0, 3.0, 4.0),
        rotation: Quat::new(0.0, 1.0, 0.0, 0.0),
        scale: Vec3::splat(2.0),
    };
    let receipt = TransformService
        .apply(
            &mut state,
            0,
            TransformCommand::Set {
                entity: id,
                transform: next,
            },
        )
        .expect("full transform");
    assert_eq!(receipt.revision_after, 1);
    assert!(receipt.projection_changed);
    assert_eq!(state.transform(id).unwrap().transform(), next);

    let no_op = TransformService
        .apply(
            &mut state,
            1,
            TransformCommand::Set {
                entity: id,
                transform: next,
            },
        )
        .expect("idempotent set");
    assert_eq!(no_op.revision_after, 1);
    assert!(!no_op.projection_changed);
}

#[test]
fn active_static_collision_is_immovable_and_failure_is_atomic() {
    let id = EntityId::new(2);
    let mut state = EntityState::from_definitions([EntityDefinition::new(id, "wall")
        .with_transform(Vec3::ONE)
        .with_collision(true, true)])
    .expect("fixture");
    let error = TransformService
        .apply(
            &mut state,
            0,
            TransformCommand::Translate {
                entity: id,
                delta: Vec3::ONE,
            },
        )
        .expect_err("static wall");
    assert_eq!(error, TransformError::Immovable { entity: id });
    assert_eq!(state.revision(), 0);
    assert_eq!(state.transform(id).unwrap().translation, Vec3::ONE);
}

#[test]
fn invalid_quaternion_and_stale_revision_fail_before_mutation() {
    let id = EntityId::new(3);
    let mut state = EntityState::from_definitions([
        EntityDefinition::new(id, "actor").with_transform(Vec3::ZERO)
    ])
    .expect("fixture");
    let invalid = EntityTransform {
        rotation: Quat::new(0.0, 0.0, 0.0, 2.0),
        ..EntityTransform::IDENTITY
    };
    assert_eq!(
        TransformService
            .apply(
                &mut state,
                0,
                TransformCommand::Set {
                    entity: id,
                    transform: invalid,
                },
            )
            .unwrap_err(),
        TransformError::InvalidTransform { entity: id }
    );
    assert_eq!(
        TransformService
            .apply(
                &mut state,
                9,
                TransformCommand::Translate {
                    entity: id,
                    delta: Vec3::ONE,
                },
            )
            .unwrap_err(),
        TransformError::StaleRevision {
            expected: 9,
            actual: 0,
        }
    );
    assert_eq!(state.transform(id).unwrap().translation, Vec3::ZERO);
}
