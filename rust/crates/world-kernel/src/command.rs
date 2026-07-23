use std::collections::BTreeMap;

use core_ids::EntityId;
use core_math::Vec3;

use crate::model::{translation_is_valid, velocity_is_valid, EntityLifecycle, WorldKernel};

#[derive(Debug, Clone, PartialEq)]
pub enum WorldCommand {
    SetTranslation { entity: EntityId, translation: Vec3 },
    SetCollisionEnabled { entity: EntityId, enabled: bool },
    SetVisible { entity: EntityId, visible: bool },
    SetKinematicVelocity { entity: EntityId, velocity: Vec3 },
}

impl WorldCommand {
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
pub struct WorldCommandBatch {
    pub commands: Vec<WorldCommand>,
}

impl WorldCommandBatch {
    pub fn new(commands: impl IntoIterator<Item = WorldCommand>) -> Self {
        Self {
            commands: commands.into_iter().collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum WorldFact {
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
    pub facts: Vec<WorldFact>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorldCommandError {
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
    pub reason: WorldCommandError,
}

impl std::fmt::Display for BatchRejection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "world command batch rejected: {:?}", self.reason)
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
    fn from_world(world: &WorldKernel, entity: EntityId) -> Self {
        let transform = world.transforms.get(&entity).map(|value| value.translation);
        let collision = world.collisions.get(&entity);
        let visible = world.renderables.get(&entity).map(|value| value.visible);
        let velocity = world.kinematics.get(&entity).map(|value| value.velocity);
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
    world: &mut WorldKernel,
    batch: WorldCommandBatch,
) -> Result<BatchReceipt, BatchRejection> {
    let revision_before = world.revision;
    let mut projected = BTreeMap::<EntityId, ProjectedEntity>::new();

    for (command_index, command) in batch.commands.iter().enumerate() {
        let entity = command.entity();
        let Some(core) = world.entities.get(&entity) else {
            return Err(reject(
                world,
                Some(command_index),
                WorldCommandError::UnknownEntity { entity },
            ));
        };
        if core.lifecycle != EntityLifecycle::Active {
            return Err(reject(
                world,
                Some(command_index),
                WorldCommandError::EntityDisabled { entity },
            ));
        }
        let state = projected
            .entry(entity)
            .or_insert_with(|| ProjectedEntity::from_world(world, entity));

        match command {
            WorldCommand::SetTranslation { translation, .. } => {
                if state.next_translation.is_none() {
                    return Err(reject(
                        world,
                        Some(command_index),
                        WorldCommandError::MissingTransform { entity },
                    ));
                }
                if !translation_is_valid(*translation) {
                    return Err(reject(
                        world,
                        Some(command_index),
                        WorldCommandError::InvalidTranslation { entity },
                    ));
                }
                state.next_translation = Some(*translation);
            }
            WorldCommand::SetCollisionEnabled { enabled, .. } => {
                if state.next_collision_enabled.is_none() {
                    return Err(reject(
                        world,
                        Some(command_index),
                        WorldCommandError::MissingCollision { entity },
                    ));
                }
                state.next_collision_enabled = Some(*enabled);
            }
            WorldCommand::SetVisible { visible, .. } => {
                if state.next_visible.is_none() {
                    return Err(reject(
                        world,
                        Some(command_index),
                        WorldCommandError::MissingRenderable { entity },
                    ));
                }
                state.next_visible = Some(*visible);
            }
            WorldCommand::SetKinematicVelocity { velocity, .. } => {
                if state.next_velocity.is_none() {
                    return Err(reject(
                        world,
                        Some(command_index),
                        WorldCommandError::MissingKinematic { entity },
                    ));
                }
                if !velocity_is_valid(*velocity) {
                    return Err(reject(
                        world,
                        Some(command_index),
                        WorldCommandError::InvalidKinematicVelocity { entity },
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
                world,
                None,
                WorldCommandError::StaticColliderMovement { entity: *entity },
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
            world
                .transforms
                .get_mut(&entity)
                .expect("validated transform presence")
                .translation = after;
            facts.push(WorldFact::TranslationChanged {
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
            world
                .collisions
                .get_mut(&entity)
                .expect("validated collision presence")
                .enabled = after;
            facts.push(WorldFact::CollisionChanged {
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
            world
                .renderables
                .get_mut(&entity)
                .expect("validated renderable presence")
                .visible = after;
            facts.push(WorldFact::VisibilityChanged {
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
            world
                .kinematics
                .get_mut(&entity)
                .expect("validated kinematic presence")
                .velocity = after;
            facts.push(WorldFact::KinematicVelocityChanged {
                entity,
                before,
                after,
                revision: revision_after,
            });
        }
    }

    world.revision = revision_after;
    Ok(BatchReceipt {
        revision_before,
        revision_after,
        facts,
    })
}

fn reject(
    world: &WorldKernel,
    command_index: Option<usize>,
    reason: WorldCommandError,
) -> BatchRejection {
    BatchRejection {
        revision: world.revision,
        command_index,
        reason,
    }
}
