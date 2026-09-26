//! Authored scene documents, validation, editing, and resolved plan facts.
//! Products apply these facts to their own state through named Engine services.

#![forbid(unsafe_code)]

mod admission;
mod codec;
mod edit;
mod light;
mod model;
mod validation;

pub use admission::{
    AvailableSceneAsset, PlannedSceneEntity, PlannedSceneLight, PlannedSceneRenderable,
    ResolvedSceneInstance, SceneAdmissionError, SceneAdmissionPlan, SceneReferenceError,
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
