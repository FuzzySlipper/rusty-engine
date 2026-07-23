use core_ids::EntityId;
use game_host::{encode_game_snapshot, GameRuntime};

const PROJECT: &str = include_str!("../../../../../content/generated/encounter-gate.project.json");

fn main() {
    let mut runtime = GameRuntime::from_project_content(PROJECT).expect("admit encounter project");
    let first = runtime
        .defeat_enemy(EntityId::new(1), EntityId::new(4))
        .expect("defeat first enemy");
    let second = runtime
        .defeat_enemy(EntityId::new(1), EntityId::new(5))
        .expect("defeat second enemy");

    println!(
        "first_events={} clearing_events={} final_revision={}",
        first.events.len(),
        second.events.len(),
        runtime.session().entities().revision()
    );
    println!(
        "{}",
        encode_game_snapshot(&runtime).expect("encode final snapshot")
    );
}
