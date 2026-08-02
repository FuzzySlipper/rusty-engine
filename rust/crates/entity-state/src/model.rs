use std::collections::{BTreeMap, BTreeSet};

use core_assets::AssetReference;
use core_ids::{
    EntityId, PrefabId, PrefabInstanceId, PrefabPartId, ProcessId, SceneId, SceneNodeId, SubjectId,
    TagId,
};
use core_math::Vec3;

use crate::command::{BatchReceipt, BatchRejection, EntityCommandBatch};
use crate::component::{
    ComponentAccessError, ComponentIter, ComponentRegistration, ComponentRegistrationError,
    ComponentRegistry, ComponentRevision, ComponentStore, ComponentStoreInspection,
    ComponentTypeId, EntityComponent,
};
pub use crate::components::{
    AssetBindingComponent, BoundsComponent, CollisionComponent, ControllerComponent,
    KinematicComponent, RenderableComponent, RigidBodyComponent, RigidBodyInertiaPolicy,
    RigidBodyMode, RigidBodyShape, TransformComponent,
};
pub(crate) use crate::definition::{
    transform_is_valid, translation_is_valid, validate_definition, velocity_is_valid,
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
    pub transform: Option<TransformComponent>,
    pub world_transform: Option<EntityTransform>,
    pub bounds: Option<BoundsComponent>,
    pub collision: Option<CollisionComponent>,
    pub renderable: Option<RenderableComponent>,
    pub kinematic: Option<KinematicComponent>,
    pub rigid_body: Option<RigidBodyComponent>,
    pub controller: Option<ControllerComponent>,
    pub controller_active: bool,
    pub asset_binding: Option<AssetBindingComponent>,
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
    pub renderable_local_transform: EntityTransform,
    pub visible: bool,
}

#[derive(Debug, Clone)]
pub struct EntityState {
    pub(crate) revision: u64,
    pub(crate) entities: BTreeMap<EntityId, EntityCore>,
    pub(crate) components: ComponentStore,
    pub(crate) inactive_controllers: BTreeSet<EntityId>,
    pub(crate) transform_parents: BTreeMap<EntityId, EntityId>,
    pub(crate) containment: BTreeMap<EntityId, EntityId>,
    pub(crate) containment_children: BTreeMap<EntityId, BTreeSet<EntityId>>,
    pub(crate) derived_from: BTreeMap<EntityId, EntityId>,
}

impl Default for EntityState {
    fn default() -> Self {
        Self::with_registry(ComponentRegistry::default())
    }
}

impl EntityState {
    pub fn with_registry(registry: ComponentRegistry) -> Self {
        Self {
            revision: 0,
            entities: BTreeMap::new(),
            components: ComponentStore::from_registry(&registry),
            inactive_controllers: BTreeSet::new(),
            transform_parents: BTreeMap::new(),
            containment: BTreeMap::new(),
            containment_children: BTreeMap::new(),
            derived_from: BTreeMap::new(),
        }
    }

    pub fn from_definitions(
        definitions: impl IntoIterator<Item = EntityDefinition>,
    ) -> Result<Self, EntityDefinitionError> {
        Self::from_definitions_with_registry(ComponentRegistry::default(), definitions)
    }

    pub fn from_definitions_with_registry(
        registry: ComponentRegistry,
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

        let mut state = Self::with_registry(registry);
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
        state.components.reset_revisions();
        Ok(state)
    }

    /// Adds a component type to this state instance without changing live state or its revision.
    pub fn register_component<T: EntityComponent>(
        &mut self,
        registration: ComponentRegistration<T>,
    ) -> Result<(), ComponentRegistrationError> {
        self.components.register(registration)
    }

    pub fn component<T: EntityComponent>(
        &self,
        entity: EntityId,
    ) -> Result<Option<&T>, ComponentAccessError> {
        self.components.get::<T>(entity)
    }

    pub fn has_component<T: EntityComponent>(
        &self,
        entity: EntityId,
    ) -> Result<bool, ComponentAccessError> {
        self.components.has::<T>(entity)
    }

    pub fn components<T: EntityComponent>(
        &self,
    ) -> Result<ComponentIter<'_, T>, ComponentAccessError> {
        self.components.iter::<T>()
    }

    pub fn component_type_id<T: EntityComponent>(
        &self,
    ) -> Result<&ComponentTypeId, ComponentAccessError> {
        self.components.type_id_for::<T>()
    }

    /// Captures the instance-local revision for one entity/component slot.
    ///
    /// The slot may currently be absent. The returned guard is used by typed component mutation
    /// and is unaffected by changes to other entities or component types.
    pub fn component_revision<T: EntityComponent>(
        &self,
        entity: EntityId,
    ) -> Result<ComponentRevision, ComponentAccessError> {
        let component = self.components.type_id_for::<T>()?.clone();
        let revision = self.components.revision::<T>(entity)?;
        Ok(ComponentRevision {
            entity,
            component,
            revision,
        })
    }

    pub fn component_inspection(&self) -> ComponentStoreInspection {
        self.components.inspection()
    }

    pub fn component_types_for_entity(&self, entity: EntityId) -> Vec<ComponentTypeId> {
        self.components.types_for_entity(entity)
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

    pub fn transform(&self, entity: EntityId) -> Option<&TransformComponent> {
        self.components
            .get::<TransformComponent>(entity)
            .expect("built-in transform component is registered")
    }

    pub fn bounds(&self, entity: EntityId) -> Option<&BoundsComponent> {
        self.components
            .get::<BoundsComponent>(entity)
            .expect("built-in bounds component is registered")
    }

    pub fn collision(&self, entity: EntityId) -> Option<&CollisionComponent> {
        self.components
            .get::<CollisionComponent>(entity)
            .expect("built-in collision component is registered")
    }

    pub fn active_collision(&self, entity: EntityId) -> Option<&CollisionComponent> {
        (self.lifecycle(entity) == Some(EntityLifecycle::Active))
            .then(|| self.collision(entity))
            .flatten()
            .filter(|collision| collision.enabled)
    }

    pub fn renderable(&self, entity: EntityId) -> Option<&RenderableComponent> {
        self.components
            .get::<RenderableComponent>(entity)
            .expect("built-in renderable component is registered")
    }

    pub fn kinematic(&self, entity: EntityId) -> Option<&KinematicComponent> {
        self.components
            .get::<KinematicComponent>(entity)
            .expect("built-in kinematic component is registered")
    }

    pub fn rigid_body(&self, entity: EntityId) -> Option<&RigidBodyComponent> {
        self.components
            .get::<RigidBodyComponent>(entity)
            .expect("built-in rigid-body component is registered")
    }

    pub fn rigid_bodies(&self) -> ComponentIter<'_, RigidBodyComponent> {
        self.components
            .iter::<RigidBodyComponent>()
            .expect("built-in rigid-body component is registered")
    }

    pub fn controller(&self, entity: EntityId) -> Option<&ControllerComponent> {
        self.components
            .get::<ControllerComponent>(entity)
            .expect("built-in controller component is registered")
    }

    pub fn active_controller(&self, entity: EntityId) -> Option<&ControllerComponent> {
        (self.lifecycle(entity) == Some(EntityLifecycle::Active)
            && !self.inactive_controllers.contains(&entity))
        .then(|| self.controller(entity))
        .flatten()
    }

    pub fn asset_binding(&self, entity: EntityId) -> Option<&AssetBindingComponent> {
        self.components
            .get::<AssetBindingComponent>(entity)
            .expect("built-in asset binding component is registered")
    }

    pub fn transform_parent(&self, entity: EntityId) -> Option<EntityId> {
        self.transform_parents.get(&entity).copied()
    }

    pub fn contained_in(&self, entity: EntityId) -> Option<EntityId> {
        self.containment.get(&entity).copied()
    }

    /// Iterates the canonical direct containment children of one entity in identity order.
    ///
    /// This reads the maintained reverse relationship index and does not scan the entity
    /// population or unrelated containment owners.
    pub fn contained_entities(&self, container: EntityId) -> impl Iterator<Item = EntityId> + '_ {
        self.containment_children
            .get(&container)
            .into_iter()
            .flat_map(|children| children.iter().copied())
    }

    pub fn contained_entity_count(&self, container: EntityId) -> usize {
        self.containment_children
            .get(&container)
            .map_or(0, BTreeSet::len)
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
            transform: self.transform(entity).copied(),
            world_transform: self.world_transform(entity),
            bounds: self.bounds(entity).copied(),
            collision: self.collision(entity).copied(),
            renderable: self.renderable(entity).cloned(),
            kinematic: self.kinematic(entity).copied(),
            rigid_body: self.rigid_body(entity).copied(),
            controller: self.controller(entity).copied(),
            controller_active: self.controller(entity).is_some()
                && !self.inactive_controllers.contains(&entity),
            asset_binding: self.asset_binding(entity).cloned(),
            transform_parent: self.transform_parents.get(&entity).copied(),
            contained_in: self.containment.get(&entity).copied(),
            derived_from: self.derived_from.get(&entity).copied(),
        })
    }

    pub fn kinematic_bodies(&self) -> impl Iterator<Item = KinematicBodyView> + '_ {
        self.components
            .iter::<KinematicComponent>()
            .expect("built-in kinematic component is registered")
            .filter_map(|(entity, kinematic)| {
                if self.entities.get(&entity)?.lifecycle != EntityLifecycle::Active {
                    return None;
                }
                let translation = self.transform(entity)?.translation;
                Some(KinematicBodyView {
                    entity,
                    translation,
                    half_extents: kinematic.half_extents,
                    velocity: kinematic.velocity,
                })
            })
    }

    pub fn projection(&self) -> Vec<ProjectionNode> {
        self.components
            .iter::<RenderableComponent>()
            .expect("built-in renderable component is registered")
            .filter_map(|(entity, renderable)| {
                let core = self.entities.get(&entity)?;
                let transform = self.world_transform(entity);
                Some(ProjectionNode {
                    entity,
                    name: core.name.clone(),
                    asset: renderable.asset.clone(),
                    translation: transform.map(|transform| transform.translation),
                    transform,
                    renderable_local_transform: renderable.local_transform,
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
            chain.push(self.transform(current)?.transform());
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
            self.components.insert_unchecked(id, value);
        }
        if let Some(value) = definition.bounds {
            self.components.insert_unchecked(id, value);
        }
        if let Some(value) = definition.collision {
            self.components.insert_unchecked(id, value);
        }
        if let Some(value) = definition.renderable {
            self.components.insert_unchecked(id, value);
        }
        if let Some(value) = definition.kinematic {
            self.components.insert_unchecked(id, value);
        }
        if let Some(value) = definition.controller {
            self.components.insert_unchecked(id, value);
        }
        if let Some(value) = definition.asset_binding {
            self.components.insert_unchecked(id, value);
        }
    }

    pub(crate) fn remove_components_and_relations(&mut self, entity: EntityId) {
        self.components.remove_entity(entity);
        self.inactive_controllers.remove(&entity);
        self.transform_parents.remove(&entity);
        self.transform_parents.retain(|_, parent| *parent != entity);
        if let Some(container) = self.containment.remove(&entity) {
            remove_containment_child(&mut self.containment_children, container, entity);
        }
        if let Some(children) = self.containment_children.remove(&entity) {
            for child in children {
                self.containment.remove(&child);
            }
        }
        self.derived_from.remove(&entity);
    }
}

fn remove_containment_child(
    index: &mut BTreeMap<EntityId, BTreeSet<EntityId>>,
    container: EntityId,
    child: EntityId,
) {
    let remove_container = index.get_mut(&container).is_some_and(|children| {
        children.remove(&child);
        children.is_empty()
    });
    if remove_container {
        index.remove(&container);
    }
}
