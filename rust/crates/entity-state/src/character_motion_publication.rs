use core_ids::EntityId;

use crate::{
    CharacterMotionComponent, ComponentRevision, EntityLifecycle, EntityState, TransformComponent,
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
///
/// The values come from the character controller, which solved them from this
/// entity's transform and motion at the guarded revisions; the revisions are
/// the only check that the slots are still the ones it read. Removing either
/// component bumps its revision, so a removed slot is reported as stale.
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
