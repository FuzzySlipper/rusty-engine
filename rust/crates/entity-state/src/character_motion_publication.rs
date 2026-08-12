use core_ids::EntityId;

use crate::{
    validate_character_motion, CharacterMotionComponent, CharacterMotionValidationError,
    ComponentRevision, EntityLifecycle, EntityState, KinematicComponent, RigidBodyComponent,
    TransformComponent,
};

#[derive(Debug, Clone, PartialEq)]
pub struct CharacterMotionStateReplacement {
    pub entity: EntityId,
    pub expected_transform_revision: ComponentRevision,
    pub expected_motion_revision: ComponentRevision,
    pub transform: TransformComponent,
    pub motion: CharacterMotionComponent,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CharacterMotionStateReceipt {
    pub revision_before: u64,
    pub revision_after: u64,
    pub entity: EntityId,
    pub changed: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CharacterMotionPublicationError {
    UnknownEntity {
        entity: EntityId,
    },
    EntityNotActive {
        entity: EntityId,
    },
    MissingTransform {
        entity: EntityId,
    },
    MissingCharacterMotion {
        entity: EntityId,
    },
    KinematicConflict {
        entity: EntityId,
    },
    RigidBodyConflict {
        entity: EntityId,
    },
    ParentedTransform {
        entity: EntityId,
    },
    NonUnitScale {
        entity: EntityId,
    },
    InvalidTransform {
        entity: EntityId,
    },
    InvalidMotion {
        entity: EntityId,
        reason: CharacterMotionValidationError,
    },
    RevisionScopeMismatch {
        entity: EntityId,
    },
    StaleTransform {
        entity: EntityId,
        expected: u64,
        actual: u64,
    },
    StaleMotion {
        entity: EntityId,
        expected: u64,
        actual: u64,
    },
    RevisionExhausted,
}

impl CharacterMotionPublicationError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::UnknownEntity { .. } => "unknown-character-motion-entity",
            Self::EntityNotActive { .. } => "inactive-character-motion-entity",
            Self::MissingTransform { .. } => "missing-character-motion-transform",
            Self::MissingCharacterMotion { .. } => "missing-character-motion-component",
            Self::KinematicConflict { .. } => "legacy-kinematic-character-motion-conflict",
            Self::RigidBodyConflict { .. } => "rigid-body-character-motion-conflict",
            Self::ParentedTransform { .. } => "parented-character-motion-transform",
            Self::NonUnitScale { .. } => "scaled-character-motion-transform",
            Self::InvalidTransform { .. } => "invalid-character-motion-transform",
            Self::InvalidMotion { .. } => "invalid-character-motion-publication-state",
            Self::RevisionScopeMismatch { .. } => "character-motion-revision-scope-mismatch",
            Self::StaleTransform { .. } => "stale-character-motion-transform",
            Self::StaleMotion { .. } => "stale-character-motion-component",
            Self::RevisionExhausted => "character-motion-publication-revision-exhausted",
        }
    }
}

impl std::fmt::Display for CharacterMotionPublicationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {self:?}", self.code())
    }
}

impl std::error::Error for CharacterMotionPublicationError {}

/// Atomically replace one character's transform and continuation facts.
pub fn replace_character_motion_state(
    state: &mut EntityState,
    replacement: CharacterMotionStateReplacement,
) -> Result<CharacterMotionStateReceipt, CharacterMotionPublicationError> {
    let entity = replacement.entity;
    let Some(core) = state.core(entity) else {
        return Err(CharacterMotionPublicationError::UnknownEntity { entity });
    };
    if core.lifecycle != EntityLifecycle::Active {
        return Err(CharacterMotionPublicationError::EntityNotActive { entity });
    }
    if state.transform(entity).is_none() {
        return Err(CharacterMotionPublicationError::MissingTransform { entity });
    }
    if state.character_motion(entity).is_none() {
        return Err(CharacterMotionPublicationError::MissingCharacterMotion { entity });
    }
    if state
        .has_component::<KinematicComponent>(entity)
        .expect("built-in registration")
    {
        return Err(CharacterMotionPublicationError::KinematicConflict { entity });
    }
    if state
        .has_component::<RigidBodyComponent>(entity)
        .expect("built-in registration")
    {
        return Err(CharacterMotionPublicationError::RigidBodyConflict { entity });
    }
    if state.transform_parent(entity).is_some() {
        return Err(CharacterMotionPublicationError::ParentedTransform { entity });
    }
    if replacement.transform.scale != core_math::Vec3::ONE {
        return Err(CharacterMotionPublicationError::NonUnitScale { entity });
    }
    if !crate::definition::transform_is_valid(replacement.transform.transform()) {
        return Err(CharacterMotionPublicationError::InvalidTransform { entity });
    }
    validate_character_motion(&replacement.motion)
        .map_err(|reason| CharacterMotionPublicationError::InvalidMotion { entity, reason })?;

    let transform_type = state
        .component_type_id::<TransformComponent>()
        .expect("built-in transform registration");
    let motion_type = state
        .component_type_id::<CharacterMotionComponent>()
        .expect("built-in character-motion registration");
    if replacement.expected_transform_revision.entity() != entity
        || replacement.expected_transform_revision.component() != transform_type
        || replacement.expected_motion_revision.entity() != entity
        || replacement.expected_motion_revision.component() != motion_type
    {
        return Err(CharacterMotionPublicationError::RevisionScopeMismatch { entity });
    }
    let actual_transform = state
        .component_revision::<TransformComponent>(entity)
        .expect("built-in transform registration")
        .revision();
    if actual_transform != replacement.expected_transform_revision.revision() {
        return Err(CharacterMotionPublicationError::StaleTransform {
            entity,
            expected: replacement.expected_transform_revision.revision(),
            actual: actual_transform,
        });
    }
    let actual_motion = state
        .component_revision::<CharacterMotionComponent>(entity)
        .expect("built-in character-motion registration")
        .revision();
    if actual_motion != replacement.expected_motion_revision.revision() {
        return Err(CharacterMotionPublicationError::StaleMotion {
            entity,
            expected: replacement.expected_motion_revision.revision(),
            actual: actual_motion,
        });
    }

    let changed = state.transform(entity) != Some(&replacement.transform)
        || state.character_motion(entity) != Some(&replacement.motion);
    let revision_before = state.revision;
    let revision_after = if changed {
        revision_before
            .checked_add(1)
            .ok_or(CharacterMotionPublicationError::RevisionExhausted)?
    } else {
        revision_before
    };
    if state.transform(entity) != Some(&replacement.transform) {
        state
            .components
            .insert_unchecked(entity, replacement.transform);
    }
    if state.character_motion(entity) != Some(&replacement.motion) {
        state
            .components
            .insert_unchecked(entity, replacement.motion);
    }
    state.revision = revision_after;
    Ok(CharacterMotionStateReceipt {
        revision_before,
        revision_after,
        entity,
        changed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EntityDefinition, EntityState};

    #[test]
    fn exact_slots_publish_together_and_stale_guard_changes_nothing() {
        let entity = EntityId::new(7);
        let motion = CharacterMotionComponent::at_rest(2.0);
        let mut state = EntityState::from_definitions([EntityDefinition::new(entity, "character")
            .with_transform(core_math::Vec3::new(0.0, 2.0, 0.0))
            .with_character_motion(motion)])
        .unwrap();
        let transform_revision = state
            .component_revision::<TransformComponent>(entity)
            .unwrap();
        let motion_revision = state
            .component_revision::<CharacterMotionComponent>(entity)
            .unwrap();
        let next_transform = TransformComponent::from_transform(crate::EntityTransform::at(
            core_math::Vec3::new(1.0, 2.0, 0.0),
        ));
        replace_character_motion_state(
            &mut state,
            CharacterMotionStateReplacement {
                entity,
                expected_transform_revision: transform_revision.clone(),
                expected_motion_revision: motion_revision.clone(),
                transform: next_transform,
                motion,
            },
        )
        .unwrap();
        let before = state.clone();
        assert!(matches!(
            replace_character_motion_state(
                &mut state,
                CharacterMotionStateReplacement {
                    entity,
                    expected_transform_revision: transform_revision,
                    expected_motion_revision: motion_revision,
                    transform: next_transform,
                    motion,
                }
            ),
            Err(CharacterMotionPublicationError::StaleTransform { .. })
        ));
        assert_eq!(state.revision(), before.revision());
        assert_eq!(state.transform(entity), before.transform(entity));
        assert_eq!(
            state.character_motion(entity),
            before.character_motion(entity)
        );
    }
}
