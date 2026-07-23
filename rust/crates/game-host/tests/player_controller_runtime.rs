use core_ids::EntityId;
use game_host::{
    decode_game_snapshot, encode_game_snapshot, GameEntityDefinitionError, GameRuntime,
    PlayerControlFact, ProjectContentError, ResolvedPlayerAction, RuntimeError,
};
use serde_json::{json, Value};

const PROJECT: &str = include_str!("../../../../content/generated/encounter-gate.project.json");
const PLAYER: EntityId = EntityId::new(1);

#[test]
fn semantic_move_actions_use_the_collision_aware_kinematic_path() {
    let mut runtime = GameRuntime::from_project_content(PROJECT).expect("admit player project");
    let before = player_position(&runtime);
    let mut moved = false;
    let mut blocked = false;

    for _ in 0..12 {
        let receipt = runtime
            .apply_player_action(
                PLAYER,
                ResolvedPlayerAction::Move {
                    forward: 1.0,
                    right: 0.0,
                },
            )
            .expect("move action");
        moved |= receipt
            .facts
            .iter()
            .any(|fact| matches!(fact, PlayerControlFact::Moved { .. }));
        blocked |= receipt
            .facts
            .iter()
            .any(|fact| matches!(fact, PlayerControlFact::Blocked { .. }));
    }

    let after = player_position(&runtime);
    assert!(moved, "the player should advance before reaching the wall");
    assert!(blocked, "the authored voxel wall should stop the player");
    assert!((after.x - before.x).abs() < 0.000_01);
    assert!(after.z > before.z);
    assert!(after.z < 3.0);
    assert_eq!(
        runtime
            .session()
            .entity(PLAYER)
            .unwrap()
            .kinematic
            .unwrap()
            .velocity,
        core_math::Vec3::ZERO,
        "an action cannot leave polling-style velocity behind",
    );
}

#[test]
fn semantic_look_action_updates_durable_controller_state_without_moving_the_entity() {
    let mut runtime = GameRuntime::from_project_content(PROJECT).unwrap();
    let before_position = player_position(&runtime);
    let before = runtime.session().player_controller(PLAYER).unwrap().state;

    let receipt = runtime
        .apply_player_action(
            PLAYER,
            ResolvedPlayerAction::Look {
                yaw_delta: 0.5,
                pitch_delta: -0.25,
            },
        )
        .unwrap();

    let after = runtime.session().player_controller(PLAYER).unwrap().state;
    assert_eq!(after.yaw_degrees, -174.0);
    assert_eq!(after.pitch_degrees, -13.0);
    assert_eq!(player_position(&runtime), before_position);
    assert!(receipt.motion.is_none());
    assert!(receipt.facts.iter().any(|fact| matches!(
        fact,
        PlayerControlFact::LookChanged {
            before_yaw_degrees,
            after_yaw_degrees,
            ..
        } if *before_yaw_degrees == before.yaw_degrees && *after_yaw_degrees == after.yaw_degrees
    )));
}

#[test]
fn malformed_or_unresolved_action_values_fail_closed() {
    let mut runtime = GameRuntime::from_project_content(PROJECT).unwrap();

    let error = runtime
        .apply_player_action(
            PLAYER,
            ResolvedPlayerAction::Move {
                forward: 1.01,
                right: 0.0,
            },
        )
        .unwrap_err();

    assert!(matches!(error, RuntimeError::InvalidPlayerAction { .. }));
}

#[test]
fn duplicate_authored_keyboard_controls_are_rejected_at_admission() {
    let mut project: Value = serde_json::from_str(PROJECT).unwrap();
    let player = entity_mut(&mut project, PLAYER.raw());
    player["playerController"]["bindings"]["moveBackward"] = json!("KeyW");

    let error = GameRuntime::from_project_content(&project.to_string()).unwrap_err();

    assert!(matches!(
        error,
        RuntimeError::Content(ProjectContentError::Definition(
            GameEntityDefinitionError::InvalidPlayerControllerConfig { entity }
        )) if entity == PLAYER
    ));
}

#[test]
fn snapshot_reopen_preserves_player_pose_and_controller_but_derives_no_camera_state() {
    let mut runtime = GameRuntime::from_project_content(PROJECT).unwrap();
    runtime
        .apply_player_action(
            PLAYER,
            ResolvedPlayerAction::Move {
                forward: 1.0,
                right: 0.0,
            },
        )
        .unwrap();
    runtime
        .apply_player_action(
            PLAYER,
            ResolvedPlayerAction::Look {
                yaw_delta: -0.25,
                pitch_delta: 0.5,
            },
        )
        .unwrap();
    let encoded = encode_game_snapshot(&runtime).unwrap();

    assert!(!encoded.contains("camera"));
    let reopened = decode_game_snapshot(&encoded).unwrap();

    assert_eq!(player_position(&runtime), player_position(&reopened));
    assert_eq!(
        runtime.session().player_controller(PLAYER),
        reopened.session().player_controller(PLAYER),
    );
}

fn player_position(runtime: &GameRuntime) -> core_math::Vec3 {
    runtime
        .session()
        .player_controller(PLAYER)
        .unwrap()
        .entity_view
        .transform
        .unwrap()
        .translation
}

fn entity_mut(project: &mut Value, id: u64) -> &mut Value {
    project["entities"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|entity| entity["id"] == id)
        .unwrap()
}
