//! Host-neutral entity/capability kernel for the Rusty Engine experiment.
//!
//! This crate owns reusable world invariants and one atomic mutation boundary.
//! It contains no door, encounter, TypeScript-host, render-host, or Asha runtime
//! topology.

#![forbid(unsafe_code)]

mod command;
mod model;
mod snapshot;

pub use command::{
    BatchReceipt, BatchRejection, WorldCommand, WorldCommandBatch, WorldCommandError, WorldFact,
};
pub use model::{
    CollisionCapability, EntityCore, EntityDefinition, EntityDefinitionError, EntityLifecycle,
    EntityView, ProjectionNode, RenderableCapability, TransformCapability, ViewError, WorldKernel,
    MAX_ABS_TRANSLATION,
};
pub use snapshot::{
    decode_snapshot, encode_snapshot, EntitySnapshot, WorldSnapshot, WorldSnapshotError,
    WORLD_SNAPSHOT_SCHEMA_VERSION,
};
