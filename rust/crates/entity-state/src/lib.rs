//! Reusable live entity and typed component state for Rusty Engine.
//!
//! This crate owns reusable entity invariants and one atomic mutation boundary.
//! It contains no door, encounter, render-host, or legacy runtime topology.

#![forbid(unsafe_code)]

mod activation;
mod authoring;
mod command;
mod component;
mod components;
mod definition;
mod model;
mod relationship;
mod snapshot;
mod transform;
mod value;

pub use activation::{
    component_activation, set_component_activation, ActivatableComponentKind, ComponentActivation,
    ComponentActivationError, ComponentActivationReadout, ComponentActivationReceipt,
    ComponentActivationState,
};
pub use authoring::{
    ComponentReplacement, EntityAuthoringError, EntityAuthoringFact, EntityAuthoringReceipt,
    EntityAuthoringService, MAX_COMPONENT_REPLACEMENTS,
};
pub use component::{
    ComponentAccessError, ComponentCodec, ComponentCodecError, ComponentIdentityError,
    ComponentIter, ComponentKindInspection, ComponentPersistence, ComponentRegistration,
    ComponentRegistrationError, ComponentRegistry, ComponentRevision, ComponentStoreInspection,
    ComponentTypeId, ComponentValueSnapshot, EntityComponent, RegisteredComponentSnapshot,
    RegisteredComponentSnapshotError, ASSET_BINDING_COMPONENT_TYPE_ID, BOUNDS_COMPONENT_TYPE_ID,
    COLLISION_COMPONENT_TYPE_ID, CONTROLLER_COMPONENT_TYPE_ID, KINEMATIC_COMPONENT_TYPE_ID,
    MAX_COMPONENT_CODEC_ID_BYTES, MAX_COMPONENT_INSPECTION_ENTITIES, MAX_COMPONENT_TYPE_ID_BYTES,
    MAX_REGISTERED_COMPONENT_TYPES, RENDERABLE_COMPONENT_TYPE_ID, TRANSFORM_COMPONENT_TYPE_ID,
};

pub use command::{
    BatchReceipt, BatchRejection, EntityCommand, EntityCommandBatch, EntityCommandError, EntityFact,
};
pub use model::{
    AssetBindingComponent, BoundsComponent, CollisionComponent, ControllerComponent, EntityCore,
    EntityDefinition, EntityDefinitionError, EntityLifecycle, EntitySource, EntityState,
    EntityTransform, EntityView, KinematicBodyView, KinematicComponent, ProjectionNode, Quat,
    RenderableComponent, TransformComponent, ViewError, MAX_ABS_TRANSLATION, MAX_ABS_VELOCITY,
};
pub use relationship::{
    apply_relationship, preview_relationship, RelationshipCommand, RelationshipError,
    RelationshipKind, RelationshipPreview, RelationshipReadout, RelationshipReceipt,
    TransformParentMode,
};
pub use snapshot::{
    decode_snapshot, decode_snapshot_with_registry, encode_durable_snapshot, encode_snapshot,
    AssetReferenceSnapshot, AssetVersionSnapshot, BoundsSnapshot, CollisionSnapshot,
    ControllerSnapshot, EntitySnapshot, EntitySourceSnapshot, EntityStateSnapshot,
    EntityStateSnapshotError, KinematicSnapshot, RenderableSnapshot, SnapshotLifecycle,
    TransformSnapshot, ENTITY_STATE_SNAPSHOT_SCHEMA_VERSION,
};
pub use transform::{TransformCommand, TransformError, TransformReceipt, TransformService};
