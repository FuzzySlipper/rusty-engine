use std::collections::BTreeSet;

use core_ids::EntityId;

use crate::{
    validate_rigid_body, ComponentRevision, EntityLifecycle, EntityState, KinematicComponent,
    RigidBodyComponent, RigidBodyValidationError, TransformComponent,
};

pub const MAX_RIGID_BODY_STATE_REPLACEMENTS: usize = 1_024;

#[derive(Debug, Clone, PartialEq)]
pub struct RigidBodyStateReplacement {
    pub entity: EntityId,
    pub expected_transform_revision: ComponentRevision,
    pub expected_rigid_body_revision: ComponentRevision,
    pub transform: TransformComponent,
    pub rigid_body: RigidBodyComponent,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RigidBodyStateReceipt {
    pub revision_before: u64,
    pub revision_after: u64,
    pub entities_considered: usize,
    pub entities_changed: Vec<EntityId>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RigidBodyStatePublicationError {
    TooManyReplacements {
        actual: usize,
        maximum: usize,
    },
    DuplicateEntity {
        entity: EntityId,
    },
    UnknownEntity {
        entity: EntityId,
    },
    EntityNotActive {
        entity: EntityId,
    },
    MissingTransform {
        entity: EntityId,
    },
    MissingRigidBody {
        entity: EntityId,
    },
    KinematicConflict {
        entity: EntityId,
    },
    NonUnitScale {
        entity: EntityId,
    },
    InvalidTransform {
        entity: EntityId,
    },
    InvalidRigidBody {
        entity: EntityId,
        reason: RigidBodyValidationError,
    },
    RevisionScopeMismatch {
        entity: EntityId,
    },
    StaleTransform {
        entity: EntityId,
        expected: u64,
        actual: u64,
    },
    StaleRigidBody {
        entity: EntityId,
        expected: u64,
        actual: u64,
    },
    RevisionExhausted,
}

impl RigidBodyStatePublicationError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::TooManyReplacements { .. } => "rigid-body-publication-quota-exceeded",
            Self::DuplicateEntity { .. } => "duplicate-rigid-body-publication-entity",
            Self::UnknownEntity { .. } => "unknown-rigid-body-publication-entity",
            Self::EntityNotActive { .. } => "inactive-rigid-body-publication-entity",
            Self::MissingTransform { .. } => "missing-rigid-body-transform",
            Self::MissingRigidBody { .. } => "missing-rigid-body-component",
            Self::KinematicConflict { .. } => "kinematic-rigid-body-conflict",
            Self::NonUnitScale { .. } => "scaled-rigid-body-transform",
            Self::InvalidTransform { .. } => "invalid-rigid-body-transform",
            Self::InvalidRigidBody { .. } => "invalid-rigid-body-publication-state",
            Self::RevisionScopeMismatch { .. } => "rigid-body-revision-scope-mismatch",
            Self::StaleTransform { .. } => "stale-rigid-body-transform",
            Self::StaleRigidBody { .. } => "stale-rigid-body-component",
            Self::RevisionExhausted => "rigid-body-publication-revision-exhausted",
        }
    }
}

impl std::fmt::Display for RigidBodyStatePublicationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {self:?}", self.code())
    }
}

impl std::error::Error for RigidBodyStatePublicationError {}

/// Atomically replace transform and rigid-body facts for a bounded body set.
///
/// This is a narrow publication primitive for the named spatial service, not a
/// generic heterogeneous command transaction. Every exact slot guard and every
/// candidate is checked before the first component is written.
pub fn replace_rigid_body_states(
    state: &mut EntityState,
    mut replacements: Vec<RigidBodyStateReplacement>,
) -> Result<RigidBodyStateReceipt, RigidBodyStatePublicationError> {
    if replacements.len() > MAX_RIGID_BODY_STATE_REPLACEMENTS {
        return Err(RigidBodyStatePublicationError::TooManyReplacements {
            actual: replacements.len(),
            maximum: MAX_RIGID_BODY_STATE_REPLACEMENTS,
        });
    }
    replacements.sort_by_key(|replacement| replacement.entity);
    let mut seen = BTreeSet::new();
    let transform_type = state
        .component_type_id::<TransformComponent>()
        .expect("built-in transform registration")
        .clone();
    let rigid_body_type = state
        .component_type_id::<RigidBodyComponent>()
        .expect("built-in rigid-body registration")
        .clone();

    for replacement in &replacements {
        let entity = replacement.entity;
        if !seen.insert(entity) {
            return Err(RigidBodyStatePublicationError::DuplicateEntity { entity });
        }
        let Some(core) = state.core(entity) else {
            return Err(RigidBodyStatePublicationError::UnknownEntity { entity });
        };
        if core.lifecycle != EntityLifecycle::Active {
            return Err(RigidBodyStatePublicationError::EntityNotActive { entity });
        }
        if state.transform(entity).is_none() {
            return Err(RigidBodyStatePublicationError::MissingTransform { entity });
        }
        if state.rigid_body(entity).is_none() {
            return Err(RigidBodyStatePublicationError::MissingRigidBody { entity });
        }
        if state
            .has_component::<KinematicComponent>(entity)
            .expect("built-in kinematic registration")
        {
            return Err(RigidBodyStatePublicationError::KinematicConflict { entity });
        }
        if replacement.transform.scale != core_math::Vec3::ONE {
            return Err(RigidBodyStatePublicationError::NonUnitScale { entity });
        }
        if !crate::definition::transform_is_valid(replacement.transform.transform()) {
            return Err(RigidBodyStatePublicationError::InvalidTransform { entity });
        }
        validate_rigid_body(&replacement.rigid_body).map_err(|reason| {
            RigidBodyStatePublicationError::InvalidRigidBody { entity, reason }
        })?;
        if replacement.expected_transform_revision.entity() != entity
            || replacement.expected_transform_revision.component() != &transform_type
            || replacement.expected_rigid_body_revision.entity() != entity
            || replacement.expected_rigid_body_revision.component() != &rigid_body_type
        {
            return Err(RigidBodyStatePublicationError::RevisionScopeMismatch { entity });
        }
        let actual_transform = state
            .component_revision::<TransformComponent>(entity)
            .expect("built-in transform registration")
            .revision();
        if actual_transform != replacement.expected_transform_revision.revision() {
            return Err(RigidBodyStatePublicationError::StaleTransform {
                entity,
                expected: replacement.expected_transform_revision.revision(),
                actual: actual_transform,
            });
        }
        let actual_body = state
            .component_revision::<RigidBodyComponent>(entity)
            .expect("built-in rigid-body registration")
            .revision();
        if actual_body != replacement.expected_rigid_body_revision.revision() {
            return Err(RigidBodyStatePublicationError::StaleRigidBody {
                entity,
                expected: replacement.expected_rigid_body_revision.revision(),
                actual: actual_body,
            });
        }
    }

    let changed: Vec<_> = replacements
        .iter()
        .filter(|replacement| {
            state.transform(replacement.entity) != Some(&replacement.transform)
                || state.rigid_body(replacement.entity) != Some(&replacement.rigid_body)
        })
        .map(|replacement| replacement.entity)
        .collect();
    let revision_before = state.revision;
    let revision_after = if changed.is_empty() {
        revision_before
    } else {
        revision_before
            .checked_add(1)
            .ok_or(RigidBodyStatePublicationError::RevisionExhausted)?
    };
    for replacement in replacements {
        if changed.binary_search(&replacement.entity).is_ok() {
            if state.transform(replacement.entity) != Some(&replacement.transform) {
                state
                    .components
                    .insert_unchecked(replacement.entity, replacement.transform);
            }
            if state.rigid_body(replacement.entity) != Some(&replacement.rigid_body) {
                state
                    .components
                    .insert_unchecked(replacement.entity, replacement.rigid_body);
            }
        }
    }
    state.revision = revision_after;
    Ok(RigidBodyStateReceipt {
        revision_before,
        revision_after,
        entities_considered: seen.len(),
        entities_changed: changed,
    })
}
