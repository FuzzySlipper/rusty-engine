use std::collections::BTreeMap;

use core_ids::EntityId;
use core_math::Vec3;

use crate::model::{translation_is_valid, velocity_is_valid, EntityLifecycle, EntityState};

#[derive(Debug, Clone, PartialEq)]
pub enum EntityCommand {
    SetTranslation { entity: EntityId, translation: Vec3 },
    SetCollisionEnabled { entity: EntityId, enabled: bool },
    SetVisible { entity: EntityId, visible: bool },
    SetKinematicVelocity { entity: EntityId, velocity: Vec3 },
}

impl EntityCommand {
    fn entity(&self) -> EntityId {
        match self {
            Self::SetTranslation { entity, .. }
            | Self::SetCollisionEnabled { entity, .. }
            | Self::SetVisible { entity, .. }
            | Self::SetKinematicVelocity { entity, .. } => *entity,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct EntityCommandBatch {
    pub commands: Vec<EntityCommand>,
}

impl EntityCommandBatch {
    pub fn new(commands: impl IntoIterator<Item = EntityCommand>) -> Self {
        Self {
            commands: commands.into_iter().collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum EntityFact {
    TranslationChanged {
        entity: EntityId,
        before: Vec3,
        after: Vec3,
        revision: u64,
    },
    CollisionChanged {
        entity: EntityId,
        before: bool,
        after: bool,
        revision: u64,
    },
    VisibilityChanged {
        entity: EntityId,
        before: bool,
        after: bool,
        revision: u64,
    },
    KinematicVelocityChanged {
        entity: EntityId,
        before: Vec3,
        after: Vec3,
        revision: u64,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct BatchReceipt {
    pub revision_before: u64,
    pub revision_after: u64,
    pub facts: Vec<EntityFact>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntityCommandError {
    UnknownEntity { entity: EntityId },
    EntityDisabled { entity: EntityId },
    MissingTransform { entity: EntityId },
    MissingCollision { entity: EntityId },
    MissingRenderable { entity: EntityId },
    MissingKinematic { entity: EntityId },
    InvalidTranslation { entity: EntityId },
    InvalidKinematicVelocity { entity: EntityId },
    StaticColliderMovement { entity: EntityId },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchRejection {
    pub revision: u64,
    pub command_index: Option<usize>,
    pub reason: EntityCommandError,
}

impl std::fmt::Display for BatchRejection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "entity command batch rejected: {:?}",
            self.reason
        )
    }
}

impl std::error::Error for BatchRejection {}

#[derive(Debug, Clone, Copy)]
struct ProjectedEntity {
    original_translation: Option<Vec3>,
    next_translation: Option<Vec3>,
    original_collision_enabled: Option<bool>,
    next_collision_enabled: Option<bool>,
    static_collider: bool,
    original_visible: Option<bool>,
    next_visible: Option<bool>,
    original_velocity: Option<Vec3>,
    next_velocity: Option<Vec3>,
}

impl ProjectedEntity {
    fn from_state(entities: &EntityState, entity: EntityId) -> Self {
        let transform = entities
            .transforms
            .get(&entity)
            .map(|value| value.translation);
        let collision = entities.collisions.get(&entity);
        let visible = entities.renderables.get(&entity).map(|value| value.visible);
        let velocity = entities.kinematics.get(&entity).map(|value| value.velocity);
        Self {
            original_translation: transform,
            next_translation: transform,
            original_collision_enabled: collision.map(|value| value.enabled),
            next_collision_enabled: collision.map(|value| value.enabled),
            static_collider: collision.is_some_and(|value| value.static_collider),
            original_visible: visible,
            next_visible: visible,
            original_velocity: velocity,
            next_velocity: velocity,
        }
    }
}

pub(crate) fn apply_batch(
    entities: &mut EntityState,
    batch: EntityCommandBatch,
) -> Result<BatchReceipt, BatchRejection> {
    let revision_before = entities.revision;
    let mut projected = BTreeMap::<EntityId, ProjectedEntity>::new();

    for (command_index, command) in batch.commands.iter().enumerate() {
        let entity = command.entity();
        let Some(core) = entities.entities.get(&entity) else {
            return Err(reject(
                entities,
                Some(command_index),
                EntityCommandError::UnknownEntity { entity },
            ));
        };
        if core.lifecycle != EntityLifecycle::Active {
            return Err(reject(
                entities,
                Some(command_index),
                EntityCommandError::EntityDisabled { entity },
            ));
        }
        let state = projected
            .entry(entity)
            .or_insert_with(|| ProjectedEntity::from_state(entities, entity));

        match command {
            EntityCommand::SetTranslation { translation, .. } => {
                if state.next_translation.is_none() {
                    return Err(reject(
                        entities,
                        Some(command_index),
                        EntityCommandError::MissingTransform { entity },
                    ));
                }
                if !translation_is_valid(*translation) {
                    return Err(reject(
                        entities,
                        Some(command_index),
                        EntityCommandError::InvalidTranslation { entity },
                    ));
                }
                state.next_translation = Some(*translation);
            }
            EntityCommand::SetCollisionEnabled { enabled, .. } => {
                if state.next_collision_enabled.is_none() {
                    return Err(reject(
                        entities,
                        Some(command_index),
                        EntityCommandError::MissingCollision { entity },
                    ));
                }
                state.next_collision_enabled = Some(*enabled);
            }
            EntityCommand::SetVisible { visible, .. } => {
                if state.next_visible.is_none() {
                    return Err(reject(
                        entities,
                        Some(command_index),
                        EntityCommandError::MissingRenderable { entity },
                    ));
                }
                state.next_visible = Some(*visible);
            }
            EntityCommand::SetKinematicVelocity { velocity, .. } => {
                if state.next_velocity.is_none() {
                    return Err(reject(
                        entities,
                        Some(command_index),
                        EntityCommandError::MissingKinematic { entity },
                    ));
                }
                if !velocity_is_valid(*velocity) {
                    return Err(reject(
                        entities,
                        Some(command_index),
                        EntityCommandError::InvalidKinematicVelocity { entity },
                    ));
                }
                state.next_velocity = Some(*velocity);
            }
        }
    }

    for (entity, state) in &projected {
        if state.static_collider
            && state.original_translation != state.next_translation
            && state.original_collision_enabled == Some(true)
            && state.next_collision_enabled == Some(true)
        {
            return Err(reject(
                entities,
                None,
                EntityCommandError::StaticColliderMovement { entity: *entity },
            ));
        }
    }

    let changed = projected.values().any(|state| {
        state.original_translation != state.next_translation
            || state.original_collision_enabled != state.next_collision_enabled
            || state.original_visible != state.next_visible
            || state.original_velocity != state.next_velocity
    });
    let revision_after = if changed {
        revision_before.saturating_add(1)
    } else {
        revision_before
    };
    let mut facts = Vec::new();

    for (entity, state) in projected {
        if state.original_translation != state.next_translation {
            let before = state
                .original_translation
                .expect("validated transform presence");
            let after = state
                .next_translation
                .expect("validated transform presence");
            entities
                .transforms
                .get_mut(&entity)
                .expect("validated transform presence")
                .translation = after;
            facts.push(EntityFact::TranslationChanged {
                entity,
                before,
                after,
                revision: revision_after,
            });
        }
        if state.original_collision_enabled != state.next_collision_enabled {
            let before = state
                .original_collision_enabled
                .expect("validated collision presence");
            let after = state
                .next_collision_enabled
                .expect("validated collision presence");
            entities
                .collisions
                .get_mut(&entity)
                .expect("validated collision presence")
                .enabled = after;
            facts.push(EntityFact::CollisionChanged {
                entity,
                before,
                after,
                revision: revision_after,
            });
        }
        if state.original_visible != state.next_visible {
            let before = state
                .original_visible
                .expect("validated renderable presence");
            let after = state.next_visible.expect("validated renderable presence");
            entities
                .renderables
                .get_mut(&entity)
                .expect("validated renderable presence")
                .visible = after;
            facts.push(EntityFact::VisibilityChanged {
                entity,
                before,
                after,
                revision: revision_after,
            });
        }
        if state.original_velocity != state.next_velocity {
            let before = state
                .original_velocity
                .expect("validated kinematic presence");
            let after = state.next_velocity.expect("validated kinematic presence");
            entities
                .kinematics
                .get_mut(&entity)
                .expect("validated kinematic presence")
                .velocity = after;
            facts.push(EntityFact::KinematicVelocityChanged {
                entity,
                before,
                after,
                revision: revision_after,
            });
        }
    }

    entities.revision = revision_after;
    Ok(BatchReceipt {
        revision_before,
        revision_after,
        facts,
    })
}

fn reject(
    entities: &EntityState,
    command_index: Option<usize>,
    reason: EntityCommandError,
) -> BatchRejection {
    BatchRejection {
        revision: entities.revision,
        command_index,
        reason,
    }
}
