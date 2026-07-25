use std::collections::{BTreeMap, BTreeSet};

use core_assets::{AssetId, AssetKind};
use core_ids::{PrefabId, PrefabPartId};

use crate::{
    PrefabOverride, PrefabOverrideValue, PrefabPartSource, PrefabRegistryValidationContext,
    PrefabTransform, ValidatedPrefabRegistry,
};

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedPrefabPart {
    pub id: PrefabPartId,
    pub namespace: String,
    pub display_name: String,
    pub parent: Option<PrefabPartId>,
    pub transform: PrefabTransform,
    pub source: PrefabPartSource,
    pub roles: Vec<String>,
    pub material: Option<String>,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedPrefab {
    pub requested: PrefabId,
    pub base: PrefabId,
    pub variant_id: Option<String>,
    pub parts: Vec<ResolvedPrefabPart>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrefabResolutionError {
    MissingPrefab(PrefabId),
    MissingBase(PrefabId),
    UnknownRole(String),
    RemovedRole(String),
    WrongOverrideKind(String),
    InvalidOverrideValue(String),
    UnknownOverrideAsset(String),
    UnknownOverrideEntityDefinition(String),
    DuplicateOverride { role: String, field: &'static str },
}

impl std::fmt::Display for PrefabResolutionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "could not resolve prefab: {self:?}")
    }
}

impl std::error::Error for PrefabResolutionError {}

/// Resolves the authored base/variant plus instance-local overrides into one
/// explicit composition. It does not allocate entities or mutate live state.
pub fn resolve_prefab(
    registry: &ValidatedPrefabRegistry,
    prefab: PrefabId,
    instance_overrides: &[PrefabOverride],
) -> Result<ResolvedPrefab, PrefabResolutionError> {
    let requested = registry
        .as_registry()
        .definitions
        .iter()
        .find(|definition| definition.id == prefab)
        .ok_or(PrefabResolutionError::MissingPrefab(prefab))?;
    let (base, variant_id, removed_roles, variant_overrides) = match &requested.variant {
        Some(variant) => (
            registry
                .as_registry()
                .definitions
                .iter()
                .find(|definition| definition.id == variant.base)
                .ok_or(PrefabResolutionError::MissingBase(variant.base))?,
            Some(variant.variant_id.clone()),
            variant.removed_roles.as_slice(),
            variant.overrides.as_slice(),
        ),
        None => (requested, None, &[][..], &[][..]),
    };
    let roles: BTreeMap<_, _> = base
        .part_roles
        .iter()
        .map(|binding| (binding.role.as_str(), binding.part))
        .collect();
    let removed_parts: BTreeSet<_> = removed_roles
        .iter()
        .filter_map(|role| roles.get(role.as_str()).copied())
        .collect();
    let mut parts: BTreeMap<_, _> = base
        .parts
        .iter()
        .filter(|part| !removed_parts.contains(&part.id))
        .map(|part| {
            let mut part_roles: Vec<_> = base
                .part_roles
                .iter()
                .filter(|binding| binding.part == part.id)
                .map(|binding| binding.role.clone())
                .collect();
            part_roles.sort();
            (
                part.id,
                ResolvedPrefabPart {
                    id: part.id,
                    namespace: part.namespace.clone(),
                    display_name: part.display_name.clone(),
                    parent: part.parent,
                    transform: part.transform,
                    source: part.source.clone(),
                    roles: part_roles,
                    material: None,
                    active: true,
                },
            )
        })
        .collect();
    let mut instance_fields = BTreeSet::new();
    for item in instance_overrides {
        if !instance_fields.insert((item.target_role.as_str(), item.value.field())) {
            return Err(PrefabResolutionError::DuplicateOverride {
                role: item.target_role.clone(),
                field: item.value.field(),
            });
        }
    }
    for item in variant_overrides.iter().chain(instance_overrides) {
        let part_id = roles
            .get(item.target_role.as_str())
            .copied()
            .ok_or_else(|| PrefabResolutionError::UnknownRole(item.target_role.clone()))?;
        let part = parts
            .get_mut(&part_id)
            .ok_or_else(|| PrefabResolutionError::RemovedRole(item.target_role.clone()))?;
        apply_override(part, item, registry.validation_context())?;
    }
    Ok(ResolvedPrefab {
        requested: prefab,
        base: base.id,
        variant_id,
        parts: parts.into_values().collect(),
    })
}

fn apply_override(
    part: &mut ResolvedPrefabPart,
    item: &PrefabOverride,
    context: &PrefabRegistryValidationContext,
) -> Result<(), PrefabResolutionError> {
    match &item.value {
        PrefabOverrideValue::Transform { transform } => {
            if !transform.is_valid() {
                return Err(PrefabResolutionError::InvalidOverrideValue(
                    item.target_role.clone(),
                ));
            }
            part.transform = *transform;
        }
        PrefabOverrideValue::EntityDefinition { stable_id } => match &mut part.source {
            PrefabPartSource::EntityDefinition { stable_id: value }
                if context.entity_definition_ids.contains(stable_id) =>
            {
                *value = stable_id.clone()
            }
            PrefabPartSource::EntityDefinition { .. } if !stable_id.is_empty() => {
                return Err(PrefabResolutionError::UnknownOverrideEntityDefinition(
                    stable_id.clone(),
                ));
            }
            PrefabPartSource::EntityDefinition { .. } => {
                return Err(PrefabResolutionError::InvalidOverrideValue(
                    item.target_role.clone(),
                ))
            }
            _ => {
                return Err(PrefabResolutionError::WrongOverrideKind(
                    item.target_role.clone(),
                ))
            }
        },
        PrefabOverrideValue::Asset { asset } => match &mut part.source {
            PrefabPartSource::Scene { asset: value }
                if AssetId::parse(asset).is_ok_and(|id| id.kind() == AssetKind::Scene)
                    && context.asset_ids.contains(asset) =>
            {
                *value = asset.clone()
            }
            PrefabPartSource::VoxelObject { asset: value }
                if AssetId::parse(asset).is_ok_and(|id| id.kind() == AssetKind::VoxelObject)
                    && context.asset_ids.contains(asset) =>
            {
                *value = asset.clone()
            }
            PrefabPartSource::Scene { .. } => match AssetId::parse(asset) {
                Ok(id) if id.kind() == AssetKind::Scene => {
                    return Err(PrefabResolutionError::UnknownOverrideAsset(asset.clone()));
                }
                _ => {
                    return Err(PrefabResolutionError::InvalidOverrideValue(
                        item.target_role.clone(),
                    ));
                }
            },
            PrefabPartSource::VoxelObject { .. } => match AssetId::parse(asset) {
                Ok(id) if id.kind() == AssetKind::VoxelObject => {
                    return Err(PrefabResolutionError::UnknownOverrideAsset(asset.clone()));
                }
                _ => {
                    return Err(PrefabResolutionError::InvalidOverrideValue(
                        item.target_role.clone(),
                    ));
                }
            },
            PrefabPartSource::EntityDefinition { .. } => {
                return Err(PrefabResolutionError::WrongOverrideKind(
                    item.target_role.clone(),
                ));
            }
        },
        PrefabOverrideValue::Material { asset } => {
            if matches!(part.source, PrefabPartSource::EntityDefinition { .. })
                || !AssetId::parse(asset).is_ok_and(|id| id.kind() == AssetKind::Material)
            {
                return Err(PrefabResolutionError::WrongOverrideKind(
                    item.target_role.clone(),
                ));
            }
            if !context.asset_ids.contains(asset) {
                return Err(PrefabResolutionError::UnknownOverrideAsset(asset.clone()));
            }
            part.material = Some(asset.clone());
        }
        PrefabOverrideValue::Activation { active } => part.active = *active,
    }
    Ok(())
}
