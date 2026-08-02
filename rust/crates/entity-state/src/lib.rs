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
mod rigid_body;
mod rigid_body_publication;
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
    RenderableComponent, RigidBodyComponent, RigidBodyInertiaPolicy, RigidBodyMode, RigidBodyShape,
    TransformComponent, ViewError, MAX_ABS_TRANSLATION, MAX_ABS_VELOCITY,
};
pub use relationship::{
    apply_relationship, preview_relationship, RelationshipCommand, RelationshipError,
    RelationshipKind, RelationshipPreview, RelationshipReadout, RelationshipReceipt,
    TransformParentMode,
};
pub use rigid_body::{
    validate_rigid_body, RigidBodyValidationError, MAX_RIGID_BODY_DAMPING, MAX_RIGID_BODY_FRICTION,
    MAX_RIGID_BODY_GRAVITY_SCALE, MAX_RIGID_BODY_MASS, MAX_RIGID_BODY_RESTITUTION,
    MAX_RIGID_BODY_SHAPE_EXTENT, MAX_RIGID_BODY_SPEED, RIGID_BODY_CODEC_ID,
    RIGID_BODY_CODEC_VERSION, RIGID_BODY_COMPONENT_TYPE_ID,
};
pub use rigid_body_publication::{
    replace_rigid_body_states, RigidBodyStatePublicationError, RigidBodyStateReceipt,
    RigidBodyStateReplacement, MAX_RIGID_BODY_STATE_REPLACEMENTS,
};
pub use snapshot::{
    decode_snapshot, decode_snapshot_with_registry, encode_durable_snapshot, encode_snapshot,
    AssetReferenceSnapshot, AssetVersionSnapshot, BoundsSnapshot, CollisionSnapshot,
    ControllerSnapshot, EntitySnapshot, EntitySourceSnapshot, EntityStateSnapshot,
    EntityStateSnapshotError, KinematicSnapshot, RenderableSnapshot, SnapshotLifecycle,
    TransformSnapshot, ENTITY_STATE_SNAPSHOT_SCHEMA_VERSION,
};
pub use transform::{TransformCommand, TransformError, TransformReceipt, TransformService};
