use core_assets::{AssetHash, AssetId, AssetReference, AssetVersionReq};
use core_ids::{PrefabId, SceneId, SceneNodeId};
use core_math::Vec3;
use serde::{Deserialize, Serialize};

use crate::{
    validate_scene, FlatSceneDocument, NodeMetadata, Quat, SceneBootstrapBindings,
    SceneCatalogBinding, SceneEntityInstance, SceneEntityReference, SceneGeneratorBinding,
    SceneLight, SceneLightShadowIntent, SceneMarker, SceneMetadata, SceneNodeKind, SceneNodeRecord,
    SceneTransform,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneCodecError {
    pub path: String,
    pub message: String,
}

impl SceneCodecError {
    fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for SceneCodecError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.path, self.message)
    }
}

impl std::error::Error for SceneCodecError {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct StoredSceneDocument {
    pub schema_version: u32,
    pub id: u64,
    #[serde(default)]
    pub revision: u64,
    pub metadata: StoredSceneMetadata,
    #[serde(default)]
    pub dependencies: Vec<StoredAssetReference>,
    #[serde(default)]
    pub nodes: Vec<StoredSceneNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct StoredSceneMetadata {
    pub name: Option<String>,
    pub authoring_format_version: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct StoredSceneNode {
    pub id: u64,
    pub parent: Option<u64>,
    pub child_order: u32,
    pub label: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub transform: StoredTransform,
    pub kind: StoredNodeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct StoredTransform {
    pub translation: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, tag = "kind", rename_all = "camelCase")]
enum StoredNodeKind {
    EmptyGroup,
    StaticMesh { asset: StoredAssetReference },
    AnimatedMesh { asset: StoredAssetReference },
    Sprite { asset: StoredAssetReference },
    VoxelVolume { asset: StoredAssetReference },
    Light { light: StoredLight },
    Marker { marker_id: String },
    EntityInstance { instance: StoredEntityInstance },
    Bootstrap { bindings: StoredBootstrapBindings },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, tag = "kind", rename_all = "camelCase")]
enum StoredLight {
    Ambient {
        color: [f32; 3],
        intensity: f32,
        enabled: bool,
        shadow_intent: StoredShadowIntent,
    },
    Directional {
        color: [f32; 3],
        intensity: f32,
        enabled: bool,
        shadow_intent: StoredShadowIntent,
    },
    Point {
        color: [f32; 3],
        intensity: f32,
        enabled: bool,
        range: Option<f32>,
        decay: f32,
        shadow_intent: StoredShadowIntent,
    },
    Spot {
        color: [f32; 3],
        intensity: f32,
        enabled: bool,
        range: Option<f32>,
        decay: f32,
        outer_angle_radians: f32,
        penumbra: f32,
        shadow_intent: StoredShadowIntent,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum StoredShadowIntent {
    Disabled,
    Requested,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct StoredEntityInstance {
    pub instance_id: String,
    pub reference: StoredEntityReference,
    pub spawn_marker_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, tag = "kind", rename_all = "camelCase")]
enum StoredEntityReference {
    EntityDefinition {
        stable_id: String,
    },
    Prefab {
        prefab_id: u64,
        variant_id: Option<String>,
        instantiation_seed: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct StoredBootstrapBindings {
    pub generator: Option<StoredGeneratorBinding>,
    #[serde(default)]
    pub catalogs: Vec<StoredCatalogBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct StoredGeneratorBinding {
    pub provider_id: String,
    pub preset_id: String,
    pub seed: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct StoredCatalogBinding {
    pub binding_id: String,
    pub catalog_id: String,
    pub source_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct StoredAssetReference {
    pub id: String,
    #[serde(default)]
    pub version: StoredAssetVersionRequirement,
    pub hash: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, tag = "req", rename_all = "camelCase")]
enum StoredAssetVersionRequirement {
    #[default]
    Any,
    Exact {
        value: u32,
    },
    AtLeast {
        value: u32,
    },
}

pub fn encode_scene(document: &FlatSceneDocument) -> Result<String, SceneCodecError> {
    let report = validate_scene(document);
    if !report.is_valid() {
        return Err(validation_error(report));
    }
    let stored = StoredSceneDocument::from(&document.canonical());
    let mut encoded = serde_json::to_string_pretty(&stored)
        .map_err(|error| SceneCodecError::new("$", error.to_string()))?;
    encoded.push('\n');
    Ok(encoded)
}

pub fn decode_scene(input: &str) -> Result<FlatSceneDocument, SceneCodecError> {
    let mut document = decode_scene_unvalidated(input)?;
    let report = validate_scene(&document);
    if !report.is_valid() {
        return Err(validation_error(report));
    }
    document.canonicalize();
    Ok(document)
}

/// Decode the strict stored shape while preserving semantically invalid values
/// for read-only authoring diagnostics. Runtime admission should use
/// [`decode_scene`], which validates before returning.
pub fn decode_scene_unvalidated(input: &str) -> Result<FlatSceneDocument, SceneCodecError> {
    let mut deserializer = serde_json::Deserializer::from_str(input);
    let stored: StoredSceneDocument =
        serde_path_to_error::deserialize(&mut deserializer).map_err(|error| {
            let path = error.path().to_string();
            SceneCodecError::new(
                if path.is_empty() { "$" } else { path.as_str() },
                error.inner().to_string(),
            )
        })?;
    deserializer
        .end()
        .map_err(|error| SceneCodecError::new("$", error.to_string()))?;
    stored.into_document()
}

fn validation_error(report: crate::SceneValidationReport) -> SceneCodecError {
    SceneCodecError::new(
        "$",
        format!(
            "scene validation failed: {}",
            report
                .errors
                .iter()
                .map(crate::SceneValidationError::code)
                .collect::<Vec<_>>()
                .join(", ")
        ),
    )
}

impl From<&FlatSceneDocument> for StoredSceneDocument {
    fn from(document: &FlatSceneDocument) -> Self {
        Self {
            schema_version: document.schema_version,
            id: document.id.raw(),
            revision: document.revision,
            metadata: StoredSceneMetadata {
                name: document.metadata.name.clone(),
                authoring_format_version: document.metadata.authoring_format_version,
            },
            dependencies: document
                .dependencies
                .iter()
                .map(StoredAssetReference::from)
                .collect(),
            nodes: document.nodes.iter().map(StoredSceneNode::from).collect(),
        }
    }
}

impl StoredSceneDocument {
    fn into_document(self) -> Result<FlatSceneDocument, SceneCodecError> {
        Ok(FlatSceneDocument {
            id: SceneId::new(self.id),
            revision: self.revision,
            schema_version: self.schema_version,
            metadata: SceneMetadata {
                name: self.metadata.name,
                authoring_format_version: self.metadata.authoring_format_version,
            },
            dependencies: self
                .dependencies
                .into_iter()
                .enumerate()
                .map(|(index, reference)| {
                    reference.into_reference(&format!("dependencies[{index}]"))
                })
                .collect::<Result<_, _>>()?,
            nodes: self
                .nodes
                .into_iter()
                .enumerate()
                .map(|(index, node)| node.into_node(index))
                .collect::<Result<_, _>>()?,
        })
    }
}

impl From<&SceneNodeRecord> for StoredSceneNode {
    fn from(node: &SceneNodeRecord) -> Self {
        Self {
            id: node.id.raw(),
            parent: node.parent.map(SceneNodeId::raw),
            child_order: node.child_order,
            label: node.metadata.label.clone(),
            tags: node.metadata.tags.clone(),
            transform: node.transform.into(),
            kind: StoredNodeKind::from(&node.kind),
        }
    }
}

impl StoredSceneNode {
    fn into_node(self, index: usize) -> Result<SceneNodeRecord, SceneCodecError> {
        let path = format!("nodes[{index}]");
        Ok(SceneNodeRecord {
            id: SceneNodeId::new(self.id),
            parent: self.parent.map(SceneNodeId::new),
            child_order: self.child_order,
            transform: self.transform.into(),
            kind: self.kind.into_kind(&format!("{path}.kind"))?,
            metadata: NodeMetadata {
                label: self.label,
                tags: self.tags,
            },
        })
    }
}

impl From<SceneTransform> for StoredTransform {
    fn from(transform: SceneTransform) -> Self {
        Self {
            translation: transform.translation.to_array(),
            rotation: [
                transform.rotation.x,
                transform.rotation.y,
                transform.rotation.z,
                transform.rotation.w,
            ],
            scale: transform.scale.to_array(),
        }
    }
}

impl From<StoredTransform> for SceneTransform {
    fn from(transform: StoredTransform) -> Self {
        Self {
            translation: Vec3::new(
                transform.translation[0],
                transform.translation[1],
                transform.translation[2],
            ),
            rotation: Quat::new(
                transform.rotation[0],
                transform.rotation[1],
                transform.rotation[2],
                transform.rotation[3],
            ),
            scale: Vec3::new(transform.scale[0], transform.scale[1], transform.scale[2]),
        }
    }
}

impl From<&SceneNodeKind> for StoredNodeKind {
    fn from(kind: &SceneNodeKind) -> Self {
        match kind {
            SceneNodeKind::EmptyGroup => Self::EmptyGroup,
            SceneNodeKind::StaticMesh(asset) => Self::StaticMesh {
                asset: asset.into(),
            },
            SceneNodeKind::AnimatedMesh(asset) => Self::AnimatedMesh {
                asset: asset.into(),
            },
            SceneNodeKind::Sprite(asset) => Self::Sprite {
                asset: asset.into(),
            },
            SceneNodeKind::VoxelVolume(asset) => Self::VoxelVolume {
                asset: asset.into(),
            },
            SceneNodeKind::Light(light) => Self::Light {
                light: light.into(),
            },
            SceneNodeKind::Marker(marker) => Self::Marker {
                marker_id: marker.marker_id.clone(),
            },
            SceneNodeKind::EntityInstance(instance) => Self::EntityInstance {
                instance: instance.into(),
            },
            SceneNodeKind::Bootstrap(bindings) => Self::Bootstrap {
                bindings: bindings.into(),
            },
        }
    }
}

impl StoredNodeKind {
    fn into_kind(self, path: &str) -> Result<SceneNodeKind, SceneCodecError> {
        Ok(match self {
            Self::EmptyGroup => SceneNodeKind::EmptyGroup,
            Self::StaticMesh { asset } => {
                SceneNodeKind::StaticMesh(asset.into_reference(&format!("{path}.asset"))?)
            }
            Self::AnimatedMesh { asset } => {
                SceneNodeKind::AnimatedMesh(asset.into_reference(&format!("{path}.asset"))?)
            }
            Self::Sprite { asset } => {
                SceneNodeKind::Sprite(asset.into_reference(&format!("{path}.asset"))?)
            }
            Self::VoxelVolume { asset } => {
                SceneNodeKind::VoxelVolume(asset.into_reference(&format!("{path}.asset"))?)
            }
            Self::Light { light } => SceneNodeKind::Light(light.into()),
            Self::Marker { marker_id } => SceneNodeKind::Marker(SceneMarker { marker_id }),
            Self::EntityInstance { instance } => SceneNodeKind::EntityInstance(instance.into()),
            Self::Bootstrap { bindings } => SceneNodeKind::Bootstrap(bindings.into()),
        })
    }
}

impl From<&SceneLight> for StoredLight {
    fn from(light: &SceneLight) -> Self {
        match light {
            SceneLight::Ambient {
                color,
                intensity,
                enabled,
                shadow_intent,
            } => Self::Ambient {
                color: *color,
                intensity: *intensity,
                enabled: *enabled,
                shadow_intent: (*shadow_intent).into(),
            },
            SceneLight::Directional {
                color,
                intensity,
                enabled,
                shadow_intent,
            } => Self::Directional {
                color: *color,
                intensity: *intensity,
                enabled: *enabled,
                shadow_intent: (*shadow_intent).into(),
            },
            SceneLight::Point {
                color,
                intensity,
                enabled,
                range,
                decay,
                shadow_intent,
            } => Self::Point {
                color: *color,
                intensity: *intensity,
                enabled: *enabled,
                range: *range,
                decay: *decay,
                shadow_intent: (*shadow_intent).into(),
            },
            SceneLight::Spot {
                color,
                intensity,
                enabled,
                range,
                decay,
                outer_angle_radians,
                penumbra,
                shadow_intent,
            } => Self::Spot {
                color: *color,
                intensity: *intensity,
                enabled: *enabled,
                range: *range,
                decay: *decay,
                outer_angle_radians: *outer_angle_radians,
                penumbra: *penumbra,
                shadow_intent: (*shadow_intent).into(),
            },
        }
    }
}

impl From<StoredLight> for SceneLight {
    fn from(light: StoredLight) -> Self {
        match light {
            StoredLight::Ambient {
                color,
                intensity,
                enabled,
                shadow_intent,
            } => Self::Ambient {
                color,
                intensity,
                enabled,
                shadow_intent: shadow_intent.into(),
            },
            StoredLight::Directional {
                color,
                intensity,
                enabled,
                shadow_intent,
            } => Self::Directional {
                color,
                intensity,
                enabled,
                shadow_intent: shadow_intent.into(),
            },
            StoredLight::Point {
                color,
                intensity,
                enabled,
                range,
                decay,
                shadow_intent,
            } => Self::Point {
                color,
                intensity,
                enabled,
                range,
                decay,
                shadow_intent: shadow_intent.into(),
            },
            StoredLight::Spot {
                color,
                intensity,
                enabled,
                range,
                decay,
                outer_angle_radians,
                penumbra,
                shadow_intent,
            } => Self::Spot {
                color,
                intensity,
                enabled,
                range,
                decay,
                outer_angle_radians,
                penumbra,
                shadow_intent: shadow_intent.into(),
            },
        }
    }
}

impl From<SceneLightShadowIntent> for StoredShadowIntent {
    fn from(intent: SceneLightShadowIntent) -> Self {
        match intent {
            SceneLightShadowIntent::Disabled => Self::Disabled,
            SceneLightShadowIntent::Requested => Self::Requested,
        }
    }
}

impl From<StoredShadowIntent> for SceneLightShadowIntent {
    fn from(intent: StoredShadowIntent) -> Self {
        match intent {
            StoredShadowIntent::Disabled => Self::Disabled,
            StoredShadowIntent::Requested => Self::Requested,
        }
    }
}

impl From<&SceneEntityInstance> for StoredEntityInstance {
    fn from(instance: &SceneEntityInstance) -> Self {
        Self {
            instance_id: instance.instance_id.clone(),
            reference: (&instance.reference).into(),
            spawn_marker_id: instance.spawn_marker_id.clone(),
        }
    }
}

impl From<StoredEntityInstance> for SceneEntityInstance {
    fn from(instance: StoredEntityInstance) -> Self {
        Self {
            instance_id: instance.instance_id,
            reference: instance.reference.into(),
            spawn_marker_id: instance.spawn_marker_id,
        }
    }
}

impl From<&SceneEntityReference> for StoredEntityReference {
    fn from(reference: &SceneEntityReference) -> Self {
        match reference {
            SceneEntityReference::EntityDefinition { stable_id } => Self::EntityDefinition {
                stable_id: stable_id.clone(),
            },
            SceneEntityReference::Prefab {
                prefab_id,
                variant_id,
                instantiation_seed,
            } => Self::Prefab {
                prefab_id: prefab_id.raw(),
                variant_id: variant_id.clone(),
                instantiation_seed: *instantiation_seed,
            },
        }
    }
}

impl From<StoredEntityReference> for SceneEntityReference {
    fn from(reference: StoredEntityReference) -> Self {
        match reference {
            StoredEntityReference::EntityDefinition { stable_id } => {
                Self::EntityDefinition { stable_id }
            }
            StoredEntityReference::Prefab {
                prefab_id,
                variant_id,
                instantiation_seed,
            } => Self::Prefab {
                prefab_id: PrefabId::new(prefab_id),
                variant_id,
                instantiation_seed,
            },
        }
    }
}

impl From<&SceneBootstrapBindings> for StoredBootstrapBindings {
    fn from(bindings: &SceneBootstrapBindings) -> Self {
        Self {
            generator: bindings
                .generator
                .as_ref()
                .map(|generator| StoredGeneratorBinding {
                    provider_id: generator.provider_id.clone(),
                    preset_id: generator.preset_id.clone(),
                    seed: generator.seed,
                }),
            catalogs: bindings
                .catalogs
                .iter()
                .map(|catalog| StoredCatalogBinding {
                    binding_id: catalog.binding_id.clone(),
                    catalog_id: catalog.catalog_id.clone(),
                    source_path: catalog.source_path.clone(),
                })
                .collect(),
        }
    }
}

impl From<StoredBootstrapBindings> for SceneBootstrapBindings {
    fn from(bindings: StoredBootstrapBindings) -> Self {
        Self {
            generator: bindings.generator.map(|generator| SceneGeneratorBinding {
                provider_id: generator.provider_id,
                preset_id: generator.preset_id,
                seed: generator.seed,
            }),
            catalogs: bindings
                .catalogs
                .into_iter()
                .map(|catalog| SceneCatalogBinding {
                    binding_id: catalog.binding_id,
                    catalog_id: catalog.catalog_id,
                    source_path: catalog.source_path,
                })
                .collect(),
        }
    }
}

impl From<&AssetReference> for StoredAssetReference {
    fn from(reference: &AssetReference) -> Self {
        Self {
            id: reference.id().as_str().to_string(),
            version: reference.version().into(),
            hash: reference.hash().map(|hash| hash.as_str().to_string()),
        }
    }
}

impl StoredAssetReference {
    fn into_reference(self, path: &str) -> Result<AssetReference, SceneCodecError> {
        let id = AssetId::parse(&self.id)
            .map_err(|error| SceneCodecError::new(format!("{path}.id"), error.to_string()))?;
        let hash = self
            .hash
            .map(|hash| {
                AssetHash::parse(&hash).map_err(|error| {
                    SceneCodecError::new(format!("{path}.hash"), error.to_string())
                })
            })
            .transpose()?;
        Ok(AssetReference::new(id, self.version.into(), hash))
    }
}

impl From<AssetVersionReq> for StoredAssetVersionRequirement {
    fn from(requirement: AssetVersionReq) -> Self {
        match requirement {
            AssetVersionReq::Any => Self::Any,
            AssetVersionReq::Exact(value) => Self::Exact { value },
            AssetVersionReq::AtLeast(value) => Self::AtLeast { value },
        }
    }
}

impl From<StoredAssetVersionRequirement> for AssetVersionReq {
    fn from(requirement: StoredAssetVersionRequirement) -> Self {
        match requirement {
            StoredAssetVersionRequirement::Any => Self::Any,
            StoredAssetVersionRequirement::Exact { value } => Self::Exact(value),
            StoredAssetVersionRequirement::AtLeast { value } => Self::AtLeast(value),
        }
    }
}
