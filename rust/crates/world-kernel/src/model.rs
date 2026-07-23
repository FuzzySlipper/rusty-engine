use std::collections::{BTreeMap, BTreeSet};

use core_ids::EntityId;
use core_math::Vec3;

use crate::command::{BatchReceipt, BatchRejection, WorldCommandBatch};

pub const MAX_ABS_TRANSLATION: f32 = 1_000_000.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityLifecycle {
    Active,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityCore {
    pub id: EntityId,
    pub name: String,
    pub lifecycle: EntityLifecycle,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransformCapability {
    pub translation: Vec3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollisionCapability {
    pub enabled: bool,
    pub static_collider: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderableCapability {
    pub visible: bool,
    pub asset: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EntityDefinition {
    pub id: EntityId,
    pub name: String,
    pub transform: Option<TransformCapability>,
    pub collision: Option<CollisionCapability>,
    pub renderable: Option<RenderableCapability>,
}

impl EntityDefinition {
    pub fn new(id: EntityId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            transform: None,
            collision: None,
            renderable: None,
        }
    }

    pub fn with_transform(mut self, translation: Vec3) -> Self {
        self.transform = Some(TransformCapability { translation });
        self
    }

    pub fn with_collision(mut self, enabled: bool, static_collider: bool) -> Self {
        self.collision = Some(CollisionCapability {
            enabled,
            static_collider,
        });
        self
    }

    pub fn with_renderable(mut self, asset: impl Into<String>, visible: bool) -> Self {
        self.renderable = Some(RenderableCapability {
            visible,
            asset: asset.into(),
        });
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntityDefinitionError {
    DuplicateEntity { entity: EntityId },
    EmptyName { entity: EntityId },
    InvalidTranslation { entity: EntityId },
    EmptyAsset { entity: EntityId },
}

impl std::fmt::Display for EntityDefinitionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for EntityDefinitionError {}

#[derive(Debug, Clone, PartialEq)]
pub struct EntityView {
    pub id: EntityId,
    pub name: String,
    pub lifecycle: EntityLifecycle,
    pub transform: Option<TransformCapability>,
    pub collision: Option<CollisionCapability>,
    pub renderable: Option<RenderableCapability>,
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
    pub visible: bool,
}

#[derive(Debug, Default)]
pub struct WorldKernel {
    pub(crate) revision: u64,
    pub(crate) entities: BTreeMap<EntityId, EntityCore>,
    pub(crate) transforms: BTreeMap<EntityId, TransformCapability>,
    pub(crate) collisions: BTreeMap<EntityId, CollisionCapability>,
    pub(crate) renderables: BTreeMap<EntityId, RenderableCapability>,
}

impl WorldKernel {
    pub fn from_definitions(
        definitions: impl IntoIterator<Item = EntityDefinition>,
    ) -> Result<Self, EntityDefinitionError> {
        let definitions: Vec<EntityDefinition> = definitions.into_iter().collect();
        let mut ids = BTreeSet::new();

        for definition in &definitions {
            if !ids.insert(definition.id) {
                return Err(EntityDefinitionError::DuplicateEntity {
                    entity: definition.id,
                });
            }
            validate_definition(definition)?;
        }

        let mut world = Self::default();
        for definition in definitions {
            let id = definition.id;
            world.entities.insert(
                id,
                EntityCore {
                    id,
                    name: definition.name,
                    lifecycle: EntityLifecycle::Active,
                },
            );
            if let Some(transform) = definition.transform {
                world.transforms.insert(id, transform);
            }
            if let Some(collision) = definition.collision {
                world.collisions.insert(id, collision);
            }
            if let Some(renderable) = definition.renderable {
                world.renderables.insert(id, renderable);
            }
        }
        Ok(world)
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn contains(&self, entity: EntityId) -> bool {
        self.entities.contains_key(&entity)
    }

    pub fn view(&self, entity: EntityId) -> Result<EntityView, ViewError> {
        let core = self.entities.get(&entity).ok_or(ViewError { entity })?;
        Ok(EntityView {
            id: entity,
            name: core.name.clone(),
            lifecycle: core.lifecycle,
            transform: self.transforms.get(&entity).copied(),
            collision: self.collisions.get(&entity).copied(),
            renderable: self.renderables.get(&entity).cloned(),
        })
    }

    pub fn projection(&self) -> Vec<ProjectionNode> {
        self.renderables
            .iter()
            .filter_map(|(entity, renderable)| {
                let core = self.entities.get(entity)?;
                Some(ProjectionNode {
                    entity: *entity,
                    name: core.name.clone(),
                    asset: renderable.asset.clone(),
                    translation: self.transforms.get(entity).map(|value| value.translation),
                    visible: core.lifecycle == EntityLifecycle::Active && renderable.visible,
                })
            })
            .collect()
    }

    pub fn apply_batch(
        &mut self,
        batch: WorldCommandBatch,
    ) -> Result<BatchReceipt, BatchRejection> {
        crate::command::apply_batch(self, batch)
    }
}

pub(crate) fn translation_is_valid(value: Vec3) -> bool {
    value.x.is_finite()
        && value.y.is_finite()
        && value.z.is_finite()
        && value.x.abs() <= MAX_ABS_TRANSLATION
        && value.y.abs() <= MAX_ABS_TRANSLATION
        && value.z.abs() <= MAX_ABS_TRANSLATION
}

fn validate_definition(definition: &EntityDefinition) -> Result<(), EntityDefinitionError> {
    if definition.name.trim().is_empty() {
        return Err(EntityDefinitionError::EmptyName {
            entity: definition.id,
        });
    }
    if definition
        .transform
        .is_some_and(|value| !translation_is_valid(value.translation))
    {
        return Err(EntityDefinitionError::InvalidTranslation {
            entity: definition.id,
        });
    }
    if definition
        .renderable
        .as_ref()
        .is_some_and(|value| value.asset.trim().is_empty())
    {
        return Err(EntityDefinitionError::EmptyAsset {
            entity: definition.id,
        });
    }
    Ok(())
}
