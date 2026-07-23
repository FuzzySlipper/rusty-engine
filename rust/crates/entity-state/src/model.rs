use std::collections::{BTreeMap, BTreeSet};

use core_ids::EntityId;
use core_math::Vec3;

use crate::command::{BatchReceipt, BatchRejection, EntityCommandBatch};

pub const MAX_ABS_TRANSLATION: f32 = 1_000_000.0;
pub const MAX_ABS_VELOCITY: f32 = 10_000.0;

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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KinematicCapability {
    pub half_extents: Vec3,
    pub velocity: Vec3,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KinematicBodyView {
    pub entity: EntityId,
    pub translation: Vec3,
    pub half_extents: Vec3,
    pub velocity: Vec3,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EntityDefinition {
    pub id: EntityId,
    pub name: String,
    pub transform: Option<TransformCapability>,
    pub collision: Option<CollisionCapability>,
    pub renderable: Option<RenderableCapability>,
    pub kinematic: Option<KinematicCapability>,
}

impl EntityDefinition {
    pub fn new(id: EntityId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            transform: None,
            collision: None,
            renderable: None,
            kinematic: None,
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

    pub fn with_kinematic(mut self, half_extents: Vec3, velocity: Vec3) -> Self {
        self.kinematic = Some(KinematicCapability {
            half_extents,
            velocity,
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
    KinematicMissingTransform { entity: EntityId },
    InvalidKinematicHalfExtents { entity: EntityId },
    InvalidKinematicVelocity { entity: EntityId },
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
    pub kinematic: Option<KinematicCapability>,
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
pub struct EntityState {
    pub(crate) revision: u64,
    pub(crate) entities: BTreeMap<EntityId, EntityCore>,
    pub(crate) transforms: BTreeMap<EntityId, TransformCapability>,
    pub(crate) collisions: BTreeMap<EntityId, CollisionCapability>,
    pub(crate) renderables: BTreeMap<EntityId, RenderableCapability>,
    pub(crate) kinematics: BTreeMap<EntityId, KinematicCapability>,
}

impl EntityState {
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

        let mut state = Self::default();
        for definition in definitions {
            let id = definition.id;
            state.entities.insert(
                id,
                EntityCore {
                    id,
                    name: definition.name,
                    lifecycle: EntityLifecycle::Active,
                },
            );
            if let Some(transform) = definition.transform {
                state.transforms.insert(id, transform);
            }
            if let Some(collision) = definition.collision {
                state.collisions.insert(id, collision);
            }
            if let Some(renderable) = definition.renderable {
                state.renderables.insert(id, renderable);
            }
            if let Some(kinematic) = definition.kinematic {
                state.kinematics.insert(id, kinematic);
            }
        }
        Ok(state)
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
            kinematic: self.kinematics.get(&entity).copied(),
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
        batch: EntityCommandBatch,
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
    if let Some(kinematic) = definition.kinematic {
        if definition.transform.is_none() {
            return Err(EntityDefinitionError::KinematicMissingTransform {
                entity: definition.id,
            });
        }
        if !half_extents_are_valid(kinematic.half_extents) {
            return Err(EntityDefinitionError::InvalidKinematicHalfExtents {
                entity: definition.id,
            });
        }
        if !velocity_is_valid(kinematic.velocity) {
            return Err(EntityDefinitionError::InvalidKinematicVelocity {
                entity: definition.id,
            });
        }
    }
    Ok(())
}

pub(crate) fn velocity_is_valid(value: Vec3) -> bool {
    value.x.is_finite()
        && value.y.is_finite()
        && value.z.is_finite()
        && value.x.abs() <= MAX_ABS_VELOCITY
        && value.y.abs() <= MAX_ABS_VELOCITY
        && value.z.abs() <= MAX_ABS_VELOCITY
}

fn half_extents_are_valid(value: Vec3) -> bool {
    translation_is_valid(value) && value.x > 0.0 && value.y > 0.0 && value.z > 0.0
}
