//! Reusable live entity and capability state for the Rusty Engine experiment.
//!
//! This crate owns reusable entity invariants and one atomic mutation boundary.
//! It contains no door, encounter, render-host, or Asha runtime topology.

#![forbid(unsafe_code)]

mod command;
mod model;
mod snapshot;

pub use command::{
    BatchReceipt, BatchRejection, EntityCommand, EntityCommandBatch, EntityCommandError, EntityFact,
};
pub use model::{
    CollisionCapability, EntityCore, EntityDefinition, EntityDefinitionError, EntityLifecycle,
    EntityState, EntityView, KinematicBodyView, KinematicCapability, ProjectionNode,
    RenderableCapability, TransformCapability, ViewError, MAX_ABS_TRANSLATION, MAX_ABS_VELOCITY,
};
pub use snapshot::{
    decode_snapshot, encode_snapshot, EntitySnapshot, EntityStateSnapshot,
    EntityStateSnapshotError, KinematicSnapshot, ENTITY_STATE_SNAPSHOT_SCHEMA_VERSION,
};
