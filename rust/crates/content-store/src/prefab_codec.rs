use core_ids::{PrefabId, PrefabPartId};
use serde::{Deserialize, Serialize};

use crate::{
    PrefabDefinition, PrefabOverride, PrefabOverrideValue, PrefabPart, PrefabPartRoleBinding,
    PrefabPartSource, PrefabRegistry, PrefabRegistryValidationContext, PrefabTransform,
    PrefabVariantDelta, ValidatedPrefabRegistry,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrefabCodecError {
    pub path: String,
    pub message: String,
}

impl PrefabCodecError {
    fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for PrefabCodecError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.path, self.message)
    }
}

impl std::error::Error for PrefabCodecError {}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct StoredRegistry {
    schema_version: u32,
    definitions: Vec<StoredDefinition>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct StoredDefinition {
    id: u64,
    schema_version: u32,
    display_name: String,
    parts: Vec<StoredPart>,
    part_roles: Vec<StoredRole>,
    variant: Option<StoredVariant>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct StoredPart {
    id: u64,
    namespace: String,
    display_name: String,
    parent: Option<u64>,
    transform: StoredTransform,
    source: StoredSource,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct StoredRole {
    role: String,
    part: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct StoredVariant {
    variant_id: String,
    base: u64,
    removed_roles: Vec<String>,
    overrides: Vec<StoredOverride>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct StoredOverride {
    target_role: String,
    value: StoredOverrideValue,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(
    deny_unknown_fields,
    tag = "field",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum StoredOverrideValue {
    Transform { transform: StoredTransform },
    EntityDefinition { stable_id: String },
    Asset { asset: String },
    Material { asset: String },
    Activation { active: bool },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(
    deny_unknown_fields,
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum StoredSource {
    Scene { asset: String },
    EntityDefinition { stable_id: String },
    VoxelObject { asset: String },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct StoredTransform {
    translation: [f32; 3],
    rotation: [f32; 4],
    scale: [f32; 3],
}

pub fn encode_prefab_registry(
    registry: &ValidatedPrefabRegistry,
) -> Result<String, PrefabCodecError> {
    let stored = StoredRegistry::from(registry.as_registry());
    let mut encoded = serde_json::to_string_pretty(&stored)
        .map_err(|error| PrefabCodecError::new("$", error.to_string()))?;
    encoded.push('\n');
    Ok(encoded)
}

pub fn decode_prefab_registry(
    input: &str,
    context: &PrefabRegistryValidationContext,
) -> Result<ValidatedPrefabRegistry, PrefabCodecError> {
    let mut deserializer = serde_json::Deserializer::from_str(input);
    let stored: StoredRegistry =
        serde_path_to_error::deserialize(&mut deserializer).map_err(|error| {
            let path = error.path().to_string();
            PrefabCodecError::new(
                if path.is_empty() { "$" } else { path.as_str() },
                error.inner().to_string(),
            )
        })?;
    deserializer
        .end()
        .map_err(|error| PrefabCodecError::new("$", error.to_string()))?;
    ValidatedPrefabRegistry::new(stored.into(), context).map_err(|report| {
        let first = report.diagnostics.first();
        PrefabCodecError::new(
            first.map_or("$", |diagnostic| diagnostic.path.as_str()),
            format!(
                "prefab registry has {} diagnostic(s): {}",
                report.diagnostics.len(),
                first.map_or("validation failed", |diagnostic| diagnostic
                    .message
                    .as_str())
            ),
        )
    })
}

impl From<&PrefabRegistry> for StoredRegistry {
    fn from(registry: &PrefabRegistry) -> Self {
        let canonical = registry.canonical();
        Self {
            schema_version: canonical.schema_version,
            definitions: canonical
                .definitions
                .iter()
                .map(StoredDefinition::from)
                .collect(),
        }
    }
}

impl From<StoredRegistry> for PrefabRegistry {
    fn from(registry: StoredRegistry) -> Self {
        Self {
            schema_version: registry.schema_version,
            definitions: registry.definitions.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<&PrefabDefinition> for StoredDefinition {
    fn from(definition: &PrefabDefinition) -> Self {
        Self {
            id: definition.id.raw(),
            schema_version: definition.schema_version,
            display_name: definition.display_name.clone(),
            parts: definition.parts.iter().map(StoredPart::from).collect(),
            part_roles: definition.part_roles.iter().map(StoredRole::from).collect(),
            variant: definition.variant.as_ref().map(StoredVariant::from),
        }
    }
}

impl From<StoredDefinition> for PrefabDefinition {
    fn from(definition: StoredDefinition) -> Self {
        Self {
            id: PrefabId::new(definition.id),
            schema_version: definition.schema_version,
            display_name: definition.display_name,
            parts: definition.parts.into_iter().map(Into::into).collect(),
            part_roles: definition.part_roles.into_iter().map(Into::into).collect(),
            variant: definition.variant.map(Into::into),
        }
    }
}

impl From<&PrefabPart> for StoredPart {
    fn from(part: &PrefabPart) -> Self {
        Self {
            id: part.id.raw(),
            namespace: part.namespace.clone(),
            display_name: part.display_name.clone(),
            parent: part.parent.map(PrefabPartId::raw),
            transform: part.transform.into(),
            source: (&part.source).into(),
        }
    }
}

impl From<StoredPart> for PrefabPart {
    fn from(part: StoredPart) -> Self {
        Self {
            id: PrefabPartId::new(part.id),
            namespace: part.namespace,
            display_name: part.display_name,
            parent: part.parent.map(PrefabPartId::new),
            transform: part.transform.into(),
            source: part.source.into(),
        }
    }
}

impl From<&PrefabPartRoleBinding> for StoredRole {
    fn from(binding: &PrefabPartRoleBinding) -> Self {
        Self {
            role: binding.role.clone(),
            part: binding.part.raw(),
        }
    }
}

impl From<StoredRole> for PrefabPartRoleBinding {
    fn from(binding: StoredRole) -> Self {
        Self {
            role: binding.role,
            part: PrefabPartId::new(binding.part),
        }
    }
}

impl From<&PrefabVariantDelta> for StoredVariant {
    fn from(variant: &PrefabVariantDelta) -> Self {
        Self {
            variant_id: variant.variant_id.clone(),
            base: variant.base.raw(),
            removed_roles: variant.removed_roles.clone(),
            overrides: variant.overrides.iter().map(StoredOverride::from).collect(),
        }
    }
}

impl From<StoredVariant> for PrefabVariantDelta {
    fn from(variant: StoredVariant) -> Self {
        Self {
            variant_id: variant.variant_id,
            base: PrefabId::new(variant.base),
            removed_roles: variant.removed_roles,
            overrides: variant.overrides.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<&PrefabOverride> for StoredOverride {
    fn from(item: &PrefabOverride) -> Self {
        Self {
            target_role: item.target_role.clone(),
            value: (&item.value).into(),
        }
    }
}

impl From<StoredOverride> for PrefabOverride {
    fn from(item: StoredOverride) -> Self {
        Self {
            target_role: item.target_role,
            value: item.value.into(),
        }
    }
}

impl From<&PrefabOverrideValue> for StoredOverrideValue {
    fn from(value: &PrefabOverrideValue) -> Self {
        match value {
            PrefabOverrideValue::Transform { transform } => Self::Transform {
                transform: (*transform).into(),
            },
            PrefabOverrideValue::EntityDefinition { stable_id } => Self::EntityDefinition {
                stable_id: stable_id.clone(),
            },
            PrefabOverrideValue::Asset { asset } => Self::Asset {
                asset: asset.clone(),
            },
            PrefabOverrideValue::Material { asset } => Self::Material {
                asset: asset.clone(),
            },
            PrefabOverrideValue::Activation { active } => Self::Activation { active: *active },
        }
    }
}

impl From<StoredOverrideValue> for PrefabOverrideValue {
    fn from(value: StoredOverrideValue) -> Self {
        match value {
            StoredOverrideValue::Transform { transform } => Self::Transform {
                transform: transform.into(),
            },
            StoredOverrideValue::EntityDefinition { stable_id } => {
                Self::EntityDefinition { stable_id }
            }
            StoredOverrideValue::Asset { asset } => Self::Asset { asset },
            StoredOverrideValue::Material { asset } => Self::Material { asset },
            StoredOverrideValue::Activation { active } => Self::Activation { active },
        }
    }
}

impl From<&PrefabPartSource> for StoredSource {
    fn from(source: &PrefabPartSource) -> Self {
        match source {
            PrefabPartSource::Scene { asset } => Self::Scene {
                asset: asset.clone(),
            },
            PrefabPartSource::EntityDefinition { stable_id } => Self::EntityDefinition {
                stable_id: stable_id.clone(),
            },
            PrefabPartSource::VoxelObject { asset } => Self::VoxelObject {
                asset: asset.clone(),
            },
        }
    }
}

impl From<StoredSource> for PrefabPartSource {
    fn from(source: StoredSource) -> Self {
        match source {
            StoredSource::Scene { asset } => Self::Scene { asset },
            StoredSource::EntityDefinition { stable_id } => Self::EntityDefinition { stable_id },
            StoredSource::VoxelObject { asset } => Self::VoxelObject { asset },
        }
    }
}

impl From<PrefabTransform> for StoredTransform {
    fn from(transform: PrefabTransform) -> Self {
        Self {
            translation: transform.translation,
            rotation: transform.rotation,
            scale: transform.scale,
        }
    }
}

impl From<StoredTransform> for PrefabTransform {
    fn from(transform: StoredTransform) -> Self {
        Self {
            translation: transform.translation,
            rotation: transform.rotation,
            scale: transform.scale,
        }
    }
}
