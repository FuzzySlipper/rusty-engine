use core_ids::EntityId;
use core_math::Vec3;

use crate::model::{transform_is_valid, EntityLifecycle, EntityState, EntityTransform};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransformCommand {
    Set {
        entity: EntityId,
        transform: EntityTransform,
    },
    Translate {
        entity: EntityId,
        delta: Vec3,
    },
}

impl TransformCommand {
    pub const fn entity(self) -> EntityId {
        match self {
            Self::Set { entity, .. } | Self::Translate { entity, .. } => entity,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransformReceipt {
    pub revision_before: u64,
    pub revision_after: u64,
    pub entity: EntityId,
    pub before: EntityTransform,
    pub after: EntityTransform,
    pub projection_changed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformError {
    StaleRevision { expected: u64, actual: u64 },
    UnknownEntity { entity: EntityId },
    Tombstoned { entity: EntityId },
    Disabled { entity: EntityId },
    MissingTransform { entity: EntityId },
    Immovable { entity: EntityId },
    InvalidTransform { entity: EntityId },
}

impl std::fmt::Display for TransformError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "transform rejected: {self:?}")
    }
}

impl std::error::Error for TransformError {}

#[derive(Debug, Default, Clone, Copy)]
pub struct TransformService;

impl TransformService {
    pub fn eligible(self, state: &EntityState, entity: EntityId) -> Result<(), TransformError> {
        let core = state
            .entities
            .get(&entity)
            .ok_or(TransformError::UnknownEntity { entity })?;
        match core.lifecycle {
            EntityLifecycle::Active => {}
            EntityLifecycle::Disabled => return Err(TransformError::Disabled { entity }),
            EntityLifecycle::Tombstoned => return Err(TransformError::Tombstoned { entity }),
        }
        if state.transform(entity).is_none() {
            return Err(TransformError::MissingTransform { entity });
        }
        if state
            .collision(entity)
            .is_some_and(|collision| collision.enabled && collision.static_collider)
        {
            return Err(TransformError::Immovable { entity });
        }
        Ok(())
    }

    pub fn apply(
        self,
        state: &mut EntityState,
        expected_revision: u64,
        command: TransformCommand,
    ) -> Result<TransformReceipt, TransformError> {
        if state.revision != expected_revision {
            return Err(TransformError::StaleRevision {
                expected: expected_revision,
                actual: state.revision,
            });
        }
        let entity = command.entity();
        self.eligible(state, entity)?;
        let before = state
            .transform(entity)
            .expect("eligibility checked")
            .transform();
        let after = match command {
            TransformCommand::Set { transform, .. } => transform,
            TransformCommand::Translate { delta, .. } => EntityTransform {
                translation: before.translation + delta,
                ..before
            },
        };
        if !transform_is_valid(after) {
            return Err(TransformError::InvalidTransform { entity });
        }
        let revision_before = state.revision;
        if before != after {
            state.components.insert_unchecked(
                entity,
                crate::model::TransformComponent::from_transform(after),
            );
            state.revision = state.revision.saturating_add(1);
        }
        Ok(TransformReceipt {
            revision_before,
            revision_after: state.revision,
            entity,
            before,
            after,
            projection_changed: before != after
                && state
                    .renderable(entity)
                    .is_some_and(|renderable| renderable.visible),
        })
    }
}

impl EntityState {
    pub fn apply_transform(
        &mut self,
        expected_revision: u64,
        command: TransformCommand,
    ) -> Result<TransformReceipt, TransformError> {
        TransformService.apply(self, expected_revision, command)
    }
}
