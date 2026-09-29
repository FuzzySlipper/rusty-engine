//! Authored scene documents and prefab registries: validation, editing,
//! codecs, and resolved plan facts.
//! Products apply these facts to their own state through named Engine services.

#![forbid(unsafe_code)]

mod admission;
mod codec;
mod edit;
mod light;
mod model;
mod prefab;
mod prefab_codec;
mod prefab_resolution;
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
pub use prefab::{
    validate_prefab_registry, PrefabDefinition, PrefabDiagnostic, PrefabDiagnosticCode,
    PrefabInstanceRecord, PrefabOverride, PrefabOverrideValue, PrefabPart, PrefabPartReference,
    PrefabPartRoleBinding, PrefabPartSource, PrefabRegistry, PrefabRegistryValidationContext,
    PrefabTransform, PrefabValidationReport, PrefabVariantDelta, ValidatedPrefabRegistry,
    PREFAB_DEFINITION_SCHEMA_VERSION, PREFAB_REGISTRY_SCHEMA_VERSION,
};
pub use prefab_codec::{decode_prefab_registry, encode_prefab_registry, PrefabCodecError};
pub use prefab_resolution::{
    resolve_prefab, PrefabResolutionError, ResolvedPrefab, ResolvedPrefabPart,
};
pub use validation::{
    composed_world_transforms, validate_scene, SceneDiagnostic, SceneValidationError,
    SceneValidationReport, TransformInvalid,
};

pub use entity_state::{EntityTransform as SceneTransform, Quat};
