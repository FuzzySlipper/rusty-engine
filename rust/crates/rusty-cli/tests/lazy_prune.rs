//! Every `rusty` invocation clears dead dev session records, even one that
//! has nothing to do with dev sessions.

use std::{fs, process::Command};

#[test]
fn an_unrelated_command_prunes_a_dead_session_record() {
    let cache = std::env::temp_dir().join(format!("rusty-lazy-prune-{}", std::process::id()));
    let sessions = cache.join("rusty-engine/sessions");
    // A session whose `rusty dev` was killed: its lock is no longer held.
    let dead = sessions.join("game-4242");
    fs::create_dir_all(&dead).unwrap();
    fs::write(dead.join("lock"), "").unwrap();
    fs::write(
        dead.join("session.json"),
        r#"{"id":"game-4242","state":"serving","startedAt":1}"#,
    )
    .unwrap();
    // Not a session directory: left alone.
    fs::write(sessions.join("notes.txt"), "kept").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rusty"))
        .arg("--help")
        .env("XDG_CACHE_HOME", &cache)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(!dead.exists(), "the dead record is pruned");
    assert!(sessions.join("notes.txt").exists());

    // Without a registry at all the command still succeeds.
    fs::remove_dir_all(&cache).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rusty"))
        .arg("--help")
        .env("XDG_CACHE_HOME", &cache)
        .output()
        .unwrap();
    assert!(output.status.success());
    let _ = fs::remove_dir_all(&cache);
}
