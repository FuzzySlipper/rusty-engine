//! Explicit Rust service-owned game runtime over [`world_kernel`].
//!
//! This crate is the familiar baseline in the authority-host comparison. Game
//! components remain mostly data; named services own behavior; the runtime owns
//! event order, scheduling, projection, and lifecycle.

#![forbid(unsafe_code)]

mod model;
mod runtime;
mod scheduler;
mod snapshot;

pub use model::{
    security_door_definitions, DoorComponent, DoorConfig, DoorState, DoorView,
    GameEntityDefinition, GameEntityDefinitionError, GameEvent, GameSession, JournalEntry,
    RuntimeReadout, RuntimeReceipt, SecurityDoorIds, SwitchComponent, SwitchView,
};
pub use runtime::{GameRuntime, RuntimeError, MAX_EVENT_WAVE, MAX_TICK_ADVANCE};
pub use scheduler::{ScheduledIntent, ScheduledIntentKind, Scheduler};
pub use snapshot::{
    decode_game_snapshot, encode_game_snapshot, GameSnapshot, GameSnapshotError,
    GAME_SNAPSHOT_SCHEMA_VERSION,
};
