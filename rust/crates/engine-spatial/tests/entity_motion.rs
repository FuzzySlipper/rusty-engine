use core_ids::EntityId;
use core_math::Vec3;
use engine_spatial::{
    EntityMotionCommand, EntityMotionOutcome, EntityMotionService, FirstPersonMotionCommand,
    FirstPersonMotionInput, FirstPersonMotionService,
};
use entity_state::{EntityDefinition, EntityState};

fn collision_fixture() -> EntityState {
    EntityState::from_definitions([
        EntityDefinition::new(EntityId::new(1), "mover")
            .with_transform(Vec3::ZERO)
            .with_bounds(Vec3::splat(-0.5), Vec3::splat(0.5))
            .with_collision(true, false),
        EntityDefinition::new(EntityId::new(2), "wall")
            .with_transform(Vec3::new(2.0, 0.0, 0.0))
            .with_bounds(Vec3::splat(-0.5), Vec3::splat(0.5))
            .with_collision(true, true),
    ])
    .expect("fixture")
}

#[test]
fn entity_aabb_motion_slides_and_commits_once() {
    let mut state = collision_fixture();
    let receipt = EntityMotionService
        .apply(
            &mut state,
            0,
            EntityMotionCommand {
                entity: EntityId::new(1),
                delta: Vec3::new(2.0, 1.0, 0.0),
            },
        )
        .expect("slide");
    assert_eq!(
        receipt.resolution.outcome,
        EntityMotionOutcome::Slid {
            to: Vec3::new(0.0, 1.0, 0.0),
            blocked_axes: [true, false, false],
        }
    );
    assert_eq!(receipt.resolution.hit, Some(EntityId::new(2)));
    assert_eq!(receipt.transform.revision_after, 1);
}

#[test]
fn first_person_motion_updates_pose_and_clamps_pitch() {
    let id = EntityId::new(3);
    let mut state = EntityState::from_definitions([
        EntityDefinition::new(id, "camera").with_transform(Vec3::ZERO)
    ])
    .expect("fixture");
    let receipt = FirstPersonMotionService
        .apply(
            &mut state,
            0,
            FirstPersonMotionCommand {
                entity: id,
                tick: 12,
                input: FirstPersonMotionInput {
                    move_forward: 1.0,
                    move_right: 0.0,
                    move_up: 0.0,
                    yaw_delta_degrees: 90.0,
                    pitch_delta_degrees: 100.0,
                    delta_seconds: 0.5,
                    speed_units_per_second: 4.0,
                },
            },
        )
        .expect("motion");
    assert_eq!(receipt.to.position, Vec3::new(0.0, 0.0, -2.0));
    assert_eq!(receipt.to.yaw_degrees, 90.0);
    assert_eq!(receipt.to.pitch_degrees, 89.0);
    assert_eq!(receipt.transform.revision_after, 1);
}

#[test]
fn first_person_collision_keeps_look_rotation_when_translation_blocks() {
    let mut state = collision_fixture();
    let receipt = FirstPersonMotionService
        .apply_with_entity_collision(
            &mut state,
            0,
            FirstPersonMotionCommand {
                entity: EntityId::new(1),
                tick: 1,
                input: FirstPersonMotionInput {
                    move_forward: 0.0,
                    move_right: 1.0,
                    move_up: 0.0,
                    yaw_delta_degrees: 45.0,
                    pitch_delta_degrees: 0.0,
                    delta_seconds: 1.0,
                    speed_units_per_second: 2.0,
                },
            },
        )
        .expect("look while blocked");
    assert_eq!(receipt.to.position, Vec3::ZERO);
    assert_eq!(receipt.to.yaw_degrees, 45.0);
    assert!(receipt.collision.is_some());
    assert_eq!(receipt.transform.revision_after, 1);
}
