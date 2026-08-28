use std::collections::{BTreeMap, BTreeSet};

use core_assets::{AssetHash, AssetId, AssetReference, AssetVersionReq};
use core_ids::{EntityId, PrefabId, SceneId, SceneNodeId};
use entity_state::{
    EntityAuthoringError, EntityAuthoringReceipt, EntityAuthoringService, EntityDefinition,
    EntitySource, EntityState,
};

use crate::{
    composed_world_transforms, validate_scene, FlatSceneDocument, SceneBootstrapBindings,
    SceneEntityReference, SceneLight, SceneNodeKind, SceneTransform, SceneValidationReport,
};

pub const DEFAULT_BASE_ENTITY_ID: EntityId = EntityId::new(1);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvailableSceneAsset {
    pub version: u32,
    pub hash: Option<AssetHash>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SceneResolutionContext {
    pub available_assets: BTreeMap<AssetId, AvailableSceneAsset>,
    pub entity_definition_ids: BTreeSet<String>,
    pub prefab_ids: BTreeSet<PrefabId>,
    pub generator_presets: BTreeSet<(String, String)>,
    pub catalog_ids: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SceneReferenceError {
    UnknownAsset {
        asset: String,
    },
    AssetVersionMismatch {
        asset: String,
        required: AssetVersionReq,
        available: u32,
    },
    AssetHashMismatch {
        asset: String,
    },
    UnknownEntityDefinition {
        node: SceneNodeId,
        stable_id: String,
    },
    UnknownPrefab {
        node: SceneNodeId,
        prefab_id: PrefabId,
    },
    UnknownGeneratorPreset {
        node: SceneNodeId,
        provider_id: String,
        preset_id: String,
    },
    UnknownCatalog {
        node: SceneNodeId,
        binding_id: String,
        catalog_id: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlannedSceneEntity {
    pub node: SceneNodeId,
    pub entity: EntityId,
    pub parent_entity: Option<EntityId>,
    pub local_transform: SceneTransform,
    pub world_transform: SceneTransform,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedSceneInstance {
    pub node: SceneNodeId,
    pub entity: EntityId,
    pub instance_id: String,
    pub reference: SceneEntityReference,
    pub spawn_marker_id: Option<String>,
    pub local_transform: SceneTransform,
    pub world_transform: SceneTransform,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlannedSceneLight {
    pub node: SceneNodeId,
    pub entity: EntityId,
    pub light: SceneLight,
    pub world_transform: SceneTransform,
}

/// One asset-bearing node planned by the scene owner. The application/world
/// transform and presentation-only renderable-local transform intentionally
/// remain distinct facts.
#[derive(Debug, Clone, PartialEq)]
pub struct PlannedSceneRenderable {
    pub node: SceneNodeId,
    pub entity: EntityId,
    pub asset: AssetReference,
    pub world_transform: SceneTransform,
    pub renderable_local_transform: SceneTransform,
}

#[derive(Debug)]
pub enum SceneAdmissionError {
    InvalidScene(SceneValidationReport),
    UnresolvedReferences {
        errors: Vec<SceneReferenceError>,
    },
    EntityAllocationOverflow {
        base_entity: EntityId,
        node: SceneNodeId,
    },
    EntityAuthoring(EntityAuthoringError),
}

impl std::fmt::Display for SceneAdmissionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "scene admission rejected: {self:?}")
    }
}

impl std::error::Error for SceneAdmissionError {}

impl From<EntityAuthoringError> for SceneAdmissionError {
    fn from(error: EntityAuthoringError) -> Self {
        Self::EntityAuthoring(error)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SceneAdmissionPlan {
    scene_id: SceneId,
    scene_revision: u64,
    allocations: Vec<PlannedSceneEntity>,
    definitions: Vec<EntityDefinition>,
    resolved_instances: Vec<ResolvedSceneInstance>,
    lights: Vec<PlannedSceneLight>,
    renderables: Vec<PlannedSceneRenderable>,
    bootstrap_bindings: Option<SceneBootstrapBindings>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SceneAdmissionReceipt {
    pub scene_id: SceneId,
    pub scene_revision: u64,
    pub authoring: EntityAuthoringReceipt,
    pub allocations: Vec<PlannedSceneEntity>,
    pub resolved_instances: Vec<ResolvedSceneInstance>,
    pub lights: Vec<PlannedSceneLight>,
    pub bootstrap_bindings: Option<SceneBootstrapBindings>,
}

impl SceneAdmissionPlan {
    pub fn prepare(
        document: &FlatSceneDocument,
        resolution: &SceneResolutionContext,
    ) -> Result<Self, SceneAdmissionError> {
        Self::prepare_with_base(document, DEFAULT_BASE_ENTITY_ID, resolution)
    }

    pub fn prepare_with_base(
        document: &FlatSceneDocument,
        base_entity: EntityId,
        resolution: &SceneResolutionContext,
    ) -> Result<Self, SceneAdmissionError> {
        let report = validate_scene(document);
        if !report.is_valid() {
            return Err(SceneAdmissionError::InvalidScene(report));
        }
        let document = document.canonical();
        let reference_errors = resolve_references(&document, resolution);
        if !reference_errors.is_empty() {
            return Err(SceneAdmissionError::UnresolvedReferences {
                errors: reference_errors,
            });
        }

        let node_entities = document
            .nodes
            .iter()
            .map(|node| {
                let entity = base_entity
                    .raw()
                    .checked_sub(1)
                    .and_then(|base| base.checked_add(node.id.raw()))
                    .map(EntityId::new)
                    .ok_or(SceneAdmissionError::EntityAllocationOverflow {
                        base_entity,
                        node: node.id,
                    })?;
                Ok::<_, SceneAdmissionError>((node.id, entity))
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        let authored_world = composed_world_transforms(&document);
        let marker_world = document
            .nodes
            .iter()
            .filter_map(|node| match &node.kind {
                SceneNodeKind::Marker(marker) => {
                    Some((marker.marker_id.as_str(), authored_world[&node.id]))
                }
                _ => None,
            })
            .collect::<BTreeMap<_, _>>();

        let mut allocations = Vec::with_capacity(document.nodes.len());
        let mut definitions = Vec::with_capacity(document.nodes.len());
        let mut resolved_instances = Vec::new();
        let mut lights = Vec::new();
        let mut renderables = Vec::new();
        for node in &document.nodes {
            let entity = node_entities[&node.id];
            let marker_spawn = match &node.kind {
                SceneNodeKind::EntityInstance(instance) => instance
                    .spawn_marker_id
                    .as_deref()
                    .map(|marker| marker_world[marker].compose(node.transform)),
                _ => None,
            };
            let (local_transform, world_transform, parent_entity) =
                if let Some(marker_transform) = marker_spawn {
                    (marker_transform, marker_transform, None)
                } else {
                    (
                        node.transform,
                        authored_world[&node.id],
                        node.parent.map(|parent| node_entities[&parent]),
                    )
                };
            allocations.push(PlannedSceneEntity {
                node: node.id,
                entity,
                parent_entity,
                local_transform,
                world_transform,
            });

            let name = node
                .metadata
                .label
                .clone()
                .unwrap_or_else(|| format!("scene-node-{}", node.id.raw()));
            let mut definition = EntityDefinition::new(entity, name)
                .with_source(EntitySource::AuthoredScene {
                    scene: document.id,
                    node: node.id,
                })
                .with_full_transform(local_transform);
            if let Some(parent) = parent_entity {
                definition = definition.with_transform_parent(parent);
            }
            if let Some(asset) = node.kind.asset() {
                definition = definition
                    .with_renderable(asset.id().as_str(), true)
                    .with_renderable_local_transform(node.renderable_transform)
                    .with_asset_binding(asset.clone());
                renderables.push(PlannedSceneRenderable {
                    node: node.id,
                    entity,
                    asset: asset.clone(),
                    world_transform,
                    renderable_local_transform: node.renderable_transform,
                });
            }
            definitions.push(definition);

            if let SceneNodeKind::EntityInstance(instance) = &node.kind {
                resolved_instances.push(ResolvedSceneInstance {
                    node: node.id,
                    entity,
                    instance_id: instance.instance_id.clone(),
                    reference: instance.reference.clone(),
                    spawn_marker_id: instance.spawn_marker_id.clone(),
                    local_transform: node.transform,
                    world_transform,
                });
            }
            if let SceneNodeKind::Light(light) = &node.kind {
                lights.push(PlannedSceneLight {
                    node: node.id,
                    entity,
                    light: light.clone(),
                    world_transform,
                });
            }
        }
        let bootstrap_bindings = document.nodes.iter().find_map(|node| match &node.kind {
            SceneNodeKind::Bootstrap(bindings) => Some(bindings.clone()),
            _ => None,
        });
        Ok(Self {
            scene_id: document.id,
            scene_revision: document.revision,
            allocations,
            definitions,
            resolved_instances,
            lights,
            renderables,
            bootstrap_bindings,
        })
    }

    pub fn scene_id(&self) -> SceneId {
        self.scene_id
    }

    pub fn scene_revision(&self) -> u64 {
        self.scene_revision
    }

    pub fn allocations(&self) -> &[PlannedSceneEntity] {
        &self.allocations
    }

    pub fn resolved_instances(&self) -> &[ResolvedSceneInstance] {
        &self.resolved_instances
    }

    pub fn lights(&self) -> &[PlannedSceneLight] {
        &self.lights
    }

    pub fn renderables(&self) -> &[PlannedSceneRenderable] {
        &self.renderables
    }

    pub fn bootstrap_bindings(&self) -> Option<&SceneBootstrapBindings> {
        self.bootstrap_bindings.as_ref()
    }

    pub fn apply(
        &self,
        state: &mut EntityState,
        expected_state_revision: u64,
    ) -> Result<SceneAdmissionReceipt, SceneAdmissionError> {
        let authoring = EntityAuthoringService.admit(
            state,
            expected_state_revision,
            self.definitions.clone(),
        )?;
        Ok(SceneAdmissionReceipt {
            scene_id: self.scene_id,
            scene_revision: self.scene_revision,
            authoring,
            allocations: self.allocations.clone(),
            resolved_instances: self.resolved_instances.clone(),
            lights: self.lights.clone(),
            bootstrap_bindings: self.bootstrap_bindings.clone(),
        })
    }
}

fn resolve_references(
    document: &FlatSceneDocument,
    context: &SceneResolutionContext,
) -> Vec<SceneReferenceError> {
    let mut errors = Vec::new();
    for reference in &document.dependencies {
        resolve_asset(reference, context, &mut errors);
    }
    for node in &document.nodes {
        match &node.kind {
            SceneNodeKind::EntityInstance(instance) => match &instance.reference {
                SceneEntityReference::EntityDefinition { stable_id } => {
                    if !context.entity_definition_ids.contains(stable_id) {
                        errors.push(SceneReferenceError::UnknownEntityDefinition {
                            node: node.id,
                            stable_id: stable_id.clone(),
                        });
                    }
                }
                SceneEntityReference::Prefab { prefab_id, .. } => {
                    if !context.prefab_ids.contains(prefab_id) {
                        errors.push(SceneReferenceError::UnknownPrefab {
                            node: node.id,
                            prefab_id: *prefab_id,
                        });
                    }
                }
            },
            SceneNodeKind::Bootstrap(bindings) => {
                if let Some(generator) = &bindings.generator {
                    let key = (generator.provider_id.clone(), generator.preset_id.clone());
                    if !context.generator_presets.contains(&key) {
                        errors.push(SceneReferenceError::UnknownGeneratorPreset {
                            node: node.id,
                            provider_id: generator.provider_id.clone(),
                            preset_id: generator.preset_id.clone(),
                        });
                    }
                }
                for catalog in &bindings.catalogs {
                    if !context.catalog_ids.contains(&catalog.catalog_id) {
                        errors.push(SceneReferenceError::UnknownCatalog {
                            node: node.id,
                            binding_id: catalog.binding_id.clone(),
                            catalog_id: catalog.catalog_id.clone(),
                        });
                    }
                }
            }
            SceneNodeKind::EmptyGroup
            | SceneNodeKind::StaticMesh(_)
            | SceneNodeKind::AnimatedMesh(_)
            | SceneNodeKind::Sprite(_)
            | SceneNodeKind::VoxelVolume(_)
            | SceneNodeKind::Light(_)
            | SceneNodeKind::Marker(_) => {}
        }
    }
    errors
}

fn resolve_asset(
    reference: &AssetReference,
    context: &SceneResolutionContext,
    errors: &mut Vec<SceneReferenceError>,
) {
    let Some(available) = context.available_assets.get(reference.id()) else {
        errors.push(SceneReferenceError::UnknownAsset {
            asset: reference.id().as_str().to_string(),
        });
        return;
    };
    let version_matches = match reference.version() {
        AssetVersionReq::Any => true,
        AssetVersionReq::Exact(required) => available.version == required,
        AssetVersionReq::AtLeast(required) => available.version >= required,
    };
    if !version_matches {
        errors.push(SceneReferenceError::AssetVersionMismatch {
            asset: reference.id().as_str().to_string(),
            required: reference.version(),
            available: available.version,
        });
    }
    if reference
        .hash()
        .is_some_and(|required| available.hash.as_ref() != Some(required))
    {
        errors.push(SceneReferenceError::AssetHashMismatch {
            asset: reference.id().as_str().to_string(),
        });
    }
}
