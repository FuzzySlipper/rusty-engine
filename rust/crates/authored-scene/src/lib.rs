//! Authored scene documents, validation, editing, and atomic entity admission.
//!
//! The scene is durable authoring data. Runtime state begins only when an
//! explicit admission plan is applied to `entity-state`; there is no runtime
//! session facade, replay record, or project-bundle routing in this crate.

#![forbid(unsafe_code)]

mod admission;
mod codec;
mod edit;
mod light;
mod model;
mod validation;

pub use admission::{
    AvailableSceneAsset, PlannedSceneEntity, PlannedSceneLight, ResolvedSceneInstance,
    SceneAdmissionError, SceneAdmissionPlan, SceneAdmissionReceipt, SceneReferenceError,
    SceneResolutionContext, DEFAULT_BASE_ENTITY_ID,
};
pub use codec::{decode_scene, decode_scene_unvalidated, encode_scene, SceneCodecError};
pub use edit::{
    SceneEditCommand, SceneEditError, SceneEditReceipt, SceneEditService, SceneObjectRecord,
    SceneObjectSnapshot,
};
pub use light::{SceneLight, SceneLightInvalid, SceneLightShadowIntent};
pub use model::{
    FlatSceneDocument, NodeMetadata, SceneBootstrapBindings, SceneCatalogBinding,
    SceneEntityInstance, SceneEntityReference, SceneGeneratorBinding, SceneMarker, SceneMetadata,
    SceneNode, SceneNodeKind, SceneNodeRecord, SceneTree, CURRENT_SCENE_SCHEMA_VERSION,
};
pub use validation::{
    composed_world_transforms, validate_scene, SceneDiagnostic, SceneValidationError,
    SceneValidationReport, TransformInvalid,
};

pub use entity_state::{EntityTransform as SceneTransform, Quat};
