//! Reusable live entity and capability state for the Rusty Engine successor.
//!
//! This crate owns reusable entity invariants and one atomic mutation boundary.
//! It contains no door, encounter, render-host, or legacy runtime topology.

#![forbid(unsafe_code)]

mod activation;
mod authoring;
mod capability;
mod command;
mod definition;
mod model;
mod relationship;
mod snapshot;
mod transform;
mod value;

pub use activation::{
    capability_activation, set_capability_activation, ActivatableCapabilityKind,
    CapabilityActivation, CapabilityActivationError, CapabilityActivationReadout,
    CapabilityActivationReceipt, CapabilityActivationState,
};
pub use authoring::{
    EntityAuthoringError, EntityAuthoringFact, EntityAuthoringReceipt, EntityAuthoringService,
    EntityCapability, EntityCapabilityKind,
};

pub use command::{
    BatchReceipt, BatchRejection, EntityCommand, EntityCommandBatch, EntityCommandError, EntityFact,
};
pub use model::{
    AssetBindingCapability, BoundsCapability, CollisionCapability, ContainmentCapability,
    ControllerCapability, EntityCore, EntityDefinition, EntityDefinitionError, EntityLifecycle,
    EntitySource, EntityState, EntityTransform, EntityView, KinematicBodyView, KinematicCapability,
    ProjectionNode, Quat, RenderableCapability, TransformCapability, ViewError,
    MAX_ABS_TRANSLATION, MAX_ABS_VELOCITY,
};
pub use relationship::{
    apply_relationship, preview_relationship, RelationshipCommand, RelationshipError,
    RelationshipKind, RelationshipPreview, RelationshipReadout, RelationshipReceipt,
    TransformParentMode,
};
pub use snapshot::{
    decode_snapshot, encode_durable_snapshot, encode_snapshot, AssetReferenceSnapshot,
    AssetVersionSnapshot, BoundsSnapshot, CollisionSnapshot, ControllerSnapshot, EntitySnapshot,
    EntitySourceSnapshot, EntityStateSnapshot, EntityStateSnapshotError, KinematicSnapshot,
    RenderableSnapshot, SnapshotLifecycle, TransformSnapshot, ENTITY_STATE_SNAPSHOT_SCHEMA_VERSION,
};
pub use transform::{TransformCommand, TransformError, TransformReceipt, TransformService};
