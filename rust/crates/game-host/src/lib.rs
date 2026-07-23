//! Explicit Rust service-owned game runtime over [`entity_state`].
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

pub use content::{
    decode_project_content, AdmittedProject, ProjectContentError, PROJECT_CONTENT_SCHEMA_VERSION,
};
pub use engine_spatial::{MotionAxis, MotionFact, MotionPhaseReceipt};
pub use model::{
    security_door_definitions, DoorComponent, DoorConfig, DoorState, DoorView, EncounterComponent,
    EncounterConfig, EncounterState, EncounterView, EnemyComponent, EnemyState, EnemyView,
    GameEntityDefinition, GameEntityDefinitionError, GameEvent, GameSession, JournalEntry,
    NavigationComponent, NavigationConfig, NavigationFact, NavigationFailure,
    NavigationPhaseReceipt, NavigationState, NavigationView, RuntimeReadout, RuntimeReceipt,
    SecurityDoorIds, SwitchComponent, SwitchView, MAX_NAVIGATION_QUERY_BUDGET,
    MAX_NAVIGATION_SPEED_UNITS_PER_SECOND,
};
pub use runtime::{GameRuntime, RuntimeError, MAX_EVENT_WAVE, MAX_TICK_ADVANCE};
pub use scheduler::{ScheduledIntent, ScheduledIntentKind, Scheduler};
pub use snapshot::{
    decode_game_snapshot, encode_game_snapshot, EncounterSnapshot, EnemySnapshot, GameSnapshot,
    GameSnapshotError, NavigationSnapshot, SnapshotEncounterState, SnapshotEnemyState,
    SnapshotNavigationState, VoxelCollisionSnapshot, GAME_SNAPSHOT_SCHEMA_VERSION,
};
