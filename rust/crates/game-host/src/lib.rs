//! Explicit Rust service-owned game runtime over [`world_kernel`].
//!
//! Game components remain mostly data; named services own live behavior;
//! TypeScript-authored content is admitted before the session starts; and the
//! runtime owns event order, scheduling, projection, persistence, and lifecycle.

#![forbid(unsafe_code)]

mod content;
mod model;
mod runtime;
mod scheduler;
mod services;
mod snapshot;

pub use content::{decode_project_content, ProjectContentError, PROJECT_CONTENT_SCHEMA_VERSION};
pub use model::{
    security_door_definitions, DoorComponent, DoorConfig, DoorState, DoorView, EncounterComponent,
    EncounterConfig, EncounterState, EncounterView, EnemyComponent, EnemyState, EnemyView,
    GameEntityDefinition, GameEntityDefinitionError, GameEvent, GameSession, JournalEntry,
    RuntimeReadout, RuntimeReceipt, SecurityDoorIds, SwitchComponent, SwitchView,
};
pub use runtime::{GameRuntime, RuntimeError, MAX_EVENT_WAVE, MAX_TICK_ADVANCE};
pub use scheduler::{ScheduledIntent, ScheduledIntentKind, Scheduler};
pub use snapshot::{
    decode_game_snapshot, encode_game_snapshot, EncounterSnapshot, EnemySnapshot, GameSnapshot,
    GameSnapshotError, SnapshotEncounterState, SnapshotEnemyState, GAME_SNAPSHOT_SCHEMA_VERSION,
};
