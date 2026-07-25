use std::collections::{BTreeMap, BTreeSet};

use core_assets::AssetReference;
use core_ids::{
    EntityId, PrefabId, PrefabInstanceId, PrefabPartId, ProcessId, SceneId, SceneNodeId, SubjectId,
    TagId,
};
use core_math::Vec3;

pub use crate::capability::{
    AssetBindingCapability, BoundsCapability, CollisionCapability, ContainmentCapability,
    ControllerCapability, KinematicCapability, RenderableCapability, TransformCapability,
};
use crate::command::{BatchReceipt, BatchRejection, EntityCommandBatch};
pub(crate) use crate::definition::{
    bounds_are_valid, half_extents_are_valid, transform_is_valid, translation_is_valid,
    validate_definition, velocity_is_valid,
};
pub use crate::definition::{
    EntityDefinition, EntityDefinitionError, MAX_ABS_TRANSLATION, MAX_ABS_VELOCITY,
};
pub use crate::value::{EntityTransform, Quat};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityLifecycle {
    Active,
    Disabled,
    Tombstoned,
}

impl EntityLifecycle {
    pub const fn is_alive(self) -> bool {
        !matches!(self, Self::Tombstoned)
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Disabled => "disabled",
            Self::Tombstoned => "tombstoned",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntitySource {
    AuthoredScene {
        scene: SceneId,
        node: SceneNodeId,
    },
    RuntimeCreated {
        by: Option<ProcessId>,
    },
    Imported {
        asset: AssetReference,
    },
    PrefabInstance {
        prefab: PrefabId,
        instance: PrefabInstanceId,
        part: PrefabPartId,
        role: Option<String>,
    },
    DiagnosticTooling,
    PolicyProposed {
        by: SubjectId,
    },
}

impl EntitySource {
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::AuthoredScene { .. } => "authoredScene",
            Self::RuntimeCreated { .. } => "runtimeCreated",
            Self::Imported { .. } => "imported",
            Self::PrefabInstance { .. } => "prefabInstance",
            Self::DiagnosticTooling => "diagnosticTooling",
            Self::PolicyProposed { .. } => "policyProposed",
        }
    }

    pub const fn is_save_excluded_by_default(&self) -> bool {
        matches!(self, Self::DiagnosticTooling)
    }

    pub const fn scene_node(&self) -> Option<SceneNodeId> {
        match self {
            Self::AuthoredScene { node, .. } => Some(*node),
            _ => None,
        }
    }
}

impl Default for EntitySource {
    fn default() -> Self {
        Self::RuntimeCreated { by: None }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityCore {
    pub id: EntityId,
    pub name: String,
    pub lifecycle: EntityLifecycle,
    pub source: EntitySource,
    pub labels: Vec<TagId>,
}

impl EntityCore {
    pub fn has_label(&self, label: TagId) -> bool {
        self.labels.binary_search(&label).is_ok()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KinematicBodyView {
    pub entity: EntityId,
    pub translation: Vec3,
    pub half_extents: Vec3,
    pub velocity: Vec3,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EntityView {
    pub id: EntityId,
    pub name: String,
    pub lifecycle: EntityLifecycle,
    pub source: EntitySource,
    pub labels: Vec<TagId>,
    pub transform: Option<TransformCapability>,
    pub world_transform: Option<EntityTransform>,
    pub bounds: Option<BoundsCapability>,
    pub collision: Option<CollisionCapability>,
    pub renderable: Option<RenderableCapability>,
    pub kinematic: Option<KinematicCapability>,
    pub controller: Option<ControllerCapability>,
    pub controller_active: bool,
    pub asset_binding: Option<AssetBindingCapability>,
    pub transform_parent: Option<EntityId>,
    pub contained_in: Option<EntityId>,
    pub derived_from: Option<EntityId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewError {
    pub entity: EntityId,
}

impl std::fmt::Display for ViewError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "unknown entity {}", self.entity)
    }
}

impl std::error::Error for ViewError {}

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectionNode {
    pub entity: EntityId,
    pub name: String,
    pub asset: String,
    pub translation: Option<Vec3>,
    pub transform: Option<EntityTransform>,
    pub visible: bool,
}

#[derive(Debug, Clone, Default)]
pub struct EntityState {
    pub(crate) revision: u64,
    pub(crate) entities: BTreeMap<EntityId, EntityCore>,
    pub(crate) transforms: BTreeMap<EntityId, TransformCapability>,
    pub(crate) bounds: BTreeMap<EntityId, BoundsCapability>,
    pub(crate) collisions: BTreeMap<EntityId, CollisionCapability>,
    pub(crate) renderables: BTreeMap<EntityId, RenderableCapability>,
    pub(crate) kinematics: BTreeMap<EntityId, KinematicCapability>,
    pub(crate) controllers: BTreeMap<EntityId, ControllerCapability>,
    pub(crate) inactive_controllers: BTreeSet<EntityId>,
    pub(crate) asset_bindings: BTreeMap<EntityId, AssetBindingCapability>,
    pub(crate) transform_parents: BTreeMap<EntityId, EntityId>,
    pub(crate) containment: BTreeMap<EntityId, ContainmentCapability>,
    pub(crate) derived_from: BTreeMap<EntityId, EntityId>,
}

impl EntityState {
    pub fn from_definitions(
        definitions: impl IntoIterator<Item = EntityDefinition>,
    ) -> Result<Self, EntityDefinitionError> {
        let definitions: Vec<_> = definitions.into_iter().collect();
        let mut ids = BTreeSet::new();
        for definition in &definitions {
            if !ids.insert(definition.id) {
                return Err(EntityDefinitionError::DuplicateEntity {
                    entity: definition.id,
                });
            }
            validate_definition(definition)?;
        }

        let mut state = Self::default();
        for definition in &definitions {
            state.insert_definition(definition.clone());
        }
        for definition in &definitions {
            crate::relationship::apply_definition_relationships(&mut state, definition).map_err(
                |error| EntityDefinitionError::InvalidRelationship {
                    entity: definition.id,
                    reason: error.to_string(),
                },
            )?;
        }
        Ok(state)
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn contains(&self, entity: EntityId) -> bool {
        self.entities.contains_key(&entity)
    }

    pub fn is_alive(&self, entity: EntityId) -> bool {
        self.entities
            .get(&entity)
            .is_some_and(|core| core.lifecycle.is_alive())
    }

    pub fn core(&self, entity: EntityId) -> Option<&EntityCore> {
        self.entities.get(&entity)
    }

    pub fn lifecycle(&self, entity: EntityId) -> Option<EntityLifecycle> {
        self.entities.get(&entity).map(|core| core.lifecycle)
    }

    pub fn transform(&self, entity: EntityId) -> Option<&TransformCapability> {
        self.transforms.get(&entity)
    }

    pub fn bounds(&self, entity: EntityId) -> Option<&BoundsCapability> {
        self.bounds.get(&entity)
    }

    pub fn collision(&self, entity: EntityId) -> Option<&CollisionCapability> {
        self.collisions.get(&entity)
    }

    pub fn active_collision(&self, entity: EntityId) -> Option<&CollisionCapability> {
        (self.lifecycle(entity) == Some(EntityLifecycle::Active))
            .then(|| self.collisions.get(&entity))
            .flatten()
            .filter(|collision| collision.enabled)
    }

    pub fn renderable(&self, entity: EntityId) -> Option<&RenderableCapability> {
        self.renderables.get(&entity)
    }

    pub fn kinematic(&self, entity: EntityId) -> Option<&KinematicCapability> {
        self.kinematics.get(&entity)
    }

    pub fn controller(&self, entity: EntityId) -> Option<&ControllerCapability> {
        self.controllers.get(&entity)
    }

    pub fn active_controller(&self, entity: EntityId) -> Option<&ControllerCapability> {
        (self.lifecycle(entity) == Some(EntityLifecycle::Active)
            && !self.inactive_controllers.contains(&entity))
        .then(|| self.controllers.get(&entity))
        .flatten()
    }

    pub fn asset_binding(&self, entity: EntityId) -> Option<&AssetBindingCapability> {
        self.asset_bindings.get(&entity)
    }

    pub fn transform_parent(&self, entity: EntityId) -> Option<EntityId> {
        self.transform_parents.get(&entity).copied()
    }

    pub fn contained_in(&self, entity: EntityId) -> Option<EntityId> {
        self.containment.get(&entity).map(|value| value.container)
    }

    pub fn derived_from(&self, entity: EntityId) -> Option<EntityId> {
        self.derived_from.get(&entity).copied()
    }

    pub fn total_count(&self) -> usize {
        self.entities.len()
    }

    pub fn alive_count(&self) -> usize {
        self.entities
            .values()
            .filter(|core| core.lifecycle.is_alive())
            .count()
    }

    pub fn entities(&self) -> impl Iterator<Item = &EntityCore> {
        self.entities.values()
    }

    pub fn view(&self, entity: EntityId) -> Result<EntityView, ViewError> {
        let core = self.entities.get(&entity).ok_or(ViewError { entity })?;
        Ok(EntityView {
            id: entity,
            name: core.name.clone(),
            lifecycle: core.lifecycle,
            source: core.source.clone(),
            labels: core.labels.clone(),
            transform: self.transforms.get(&entity).copied(),
            world_transform: self.world_transform(entity),
            bounds: self.bounds.get(&entity).copied(),
            collision: self.collisions.get(&entity).copied(),
            renderable: self.renderables.get(&entity).cloned(),
            kinematic: self.kinematics.get(&entity).copied(),
            controller: self.controllers.get(&entity).copied(),
            controller_active: self.controllers.contains_key(&entity)
                && !self.inactive_controllers.contains(&entity),
            asset_binding: self.asset_bindings.get(&entity).cloned(),
            transform_parent: self.transform_parents.get(&entity).copied(),
            contained_in: self.containment.get(&entity).map(|value| value.container),
            derived_from: self.derived_from.get(&entity).copied(),
        })
    }

    pub fn kinematic_bodies(&self) -> impl Iterator<Item = KinematicBodyView> + '_ {
        self.kinematics.iter().filter_map(|(entity, kinematic)| {
            if self.entities.get(entity)?.lifecycle != EntityLifecycle::Active {
                return None;
            }
            let translation = self.transforms.get(entity)?.translation;
            Some(KinematicBodyView {
                entity: *entity,
                translation,
                half_extents: kinematic.half_extents,
                velocity: kinematic.velocity,
            })
        })
    }

    pub fn projection(&self) -> Vec<ProjectionNode> {
        self.renderables
            .iter()
            .filter_map(|(entity, renderable)| {
                let core = self.entities.get(entity)?;
                let transform = self.world_transform(*entity);
                Some(ProjectionNode {
                    entity: *entity,
                    name: core.name.clone(),
                    asset: renderable.asset.clone(),
                    translation: transform.map(|transform| transform.translation),
                    transform,
                    visible: core.lifecycle == EntityLifecycle::Active && renderable.visible,
                })
            })
            .collect()
    }

    pub fn apply_batch(
        &mut self,
        batch: EntityCommandBatch,
    ) -> Result<BatchReceipt, BatchRejection> {
        crate::command::apply_batch(self, batch)
    }

    pub fn world_transform(&self, entity: EntityId) -> Option<EntityTransform> {
        let mut chain = Vec::new();
        let mut cursor = Some(entity);
        let mut seen = BTreeSet::new();
        while let Some(current) = cursor {
            if !seen.insert(current) {
                return None;
            }
            chain.push(self.transforms.get(&current)?.transform());
            cursor = self.transform_parents.get(&current).copied();
        }
        chain.into_iter().rev().reduce(EntityTransform::compose)
    }

    pub(crate) fn insert_definition(&mut self, mut definition: EntityDefinition) {
        definition.labels.sort_unstable();
        definition.labels.dedup();
        let id = definition.id;
        self.entities.insert(
            id,
            EntityCore {
                id,
                name: definition.name,
                lifecycle: EntityLifecycle::Active,
                source: definition.source,
                labels: definition.labels,
            },
        );
        if let Some(value) = definition.transform {
            self.transforms.insert(id, value);
        }
        if let Some(value) = definition.bounds {
            self.bounds.insert(id, value);
        }
        if let Some(value) = definition.collision {
            self.collisions.insert(id, value);
        }
        if let Some(value) = definition.renderable {
            self.renderables.insert(id, value);
        }
        if let Some(value) = definition.kinematic {
            self.kinematics.insert(id, value);
        }
        if let Some(value) = definition.controller {
            self.controllers.insert(id, value);
        }
        if let Some(value) = definition.asset_binding {
            self.asset_bindings.insert(id, value);
        }
    }

    pub(crate) fn remove_capabilities_and_relations(&mut self, entity: EntityId) {
        self.transforms.remove(&entity);
        self.bounds.remove(&entity);
        self.collisions.remove(&entity);
        self.renderables.remove(&entity);
        self.kinematics.remove(&entity);
        self.controllers.remove(&entity);
        self.inactive_controllers.remove(&entity);
        self.asset_bindings.remove(&entity);
        self.transform_parents.remove(&entity);
        self.transform_parents.retain(|_, parent| *parent != entity);
        self.containment.remove(&entity);
        self.containment
            .retain(|_, value| value.container != entity);
        self.derived_from.remove(&entity);
    }
}
