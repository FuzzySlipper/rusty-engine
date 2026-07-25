use core_ids::EntityId;

use crate::model::{EntityLifecycle, EntityState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivatableCapabilityKind {
    Collision,
    Controller,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityActivation {
    Inactive,
    Active,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityActivationState {
    Absent,
    Inactive,
    Active,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityActivationReadout {
    pub entity: EntityId,
    pub capability: ActivatableCapabilityKind,
    pub state: CapabilityActivationState,
    pub effective: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityActivationReceipt {
    pub revision_before: u64,
    pub revision_after: u64,
    pub entity: EntityId,
    pub capability: ActivatableCapabilityKind,
    pub before: CapabilityActivationState,
    pub after: CapabilityActivationState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityActivationError {
    StaleRevision {
        expected: u64,
        actual: u64,
    },
    UnknownEntity {
        entity: EntityId,
    },
    TombstonedEntity {
        entity: EntityId,
    },
    CapabilityAbsent {
        entity: EntityId,
        capability: ActivatableCapabilityKind,
    },
    AlreadyInState {
        entity: EntityId,
        capability: ActivatableCapabilityKind,
        state: CapabilityActivationState,
    },
}

impl std::fmt::Display for CapabilityActivationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "capability activation rejected: {self:?}")
    }
}

impl std::error::Error for CapabilityActivationError {}

impl EntityState {
    pub fn capability_activation(
        &self,
        entity: EntityId,
        capability: ActivatableCapabilityKind,
    ) -> Result<CapabilityActivationReadout, CapabilityActivationError> {
        capability_activation(self, entity, capability)
    }

    pub fn set_capability_activation(
        &mut self,
        expected_revision: u64,
        entity: EntityId,
        capability: ActivatableCapabilityKind,
        activation: CapabilityActivation,
    ) -> Result<CapabilityActivationReceipt, CapabilityActivationError> {
        set_capability_activation(self, expected_revision, entity, capability, activation)
    }
}

pub fn capability_activation(
    state: &EntityState,
    entity: EntityId,
    capability: ActivatableCapabilityKind,
) -> Result<CapabilityActivationReadout, CapabilityActivationError> {
    let core = state
        .entities
        .get(&entity)
        .ok_or(CapabilityActivationError::UnknownEntity { entity })?;
    if core.lifecycle == EntityLifecycle::Tombstoned {
        return Err(CapabilityActivationError::TombstonedEntity { entity });
    }
    let capability_state = match capability {
        ActivatableCapabilityKind::Collision => match state.collisions.get(&entity) {
            None => CapabilityActivationState::Absent,
            Some(value) if value.enabled => CapabilityActivationState::Active,
            Some(_) => CapabilityActivationState::Inactive,
        },
        ActivatableCapabilityKind::Controller => {
            if !state.controllers.contains_key(&entity) {
                CapabilityActivationState::Absent
            } else if state.inactive_controllers.contains(&entity) {
                CapabilityActivationState::Inactive
            } else {
                CapabilityActivationState::Active
            }
        }
    };
    Ok(CapabilityActivationReadout {
        entity,
        capability,
        state: capability_state,
        effective: core.lifecycle == EntityLifecycle::Active
            && capability_state == CapabilityActivationState::Active,
    })
}

pub fn set_capability_activation(
    state: &mut EntityState,
    expected_revision: u64,
    entity: EntityId,
    capability: ActivatableCapabilityKind,
    activation: CapabilityActivation,
) -> Result<CapabilityActivationReceipt, CapabilityActivationError> {
    if state.revision != expected_revision {
        return Err(CapabilityActivationError::StaleRevision {
            expected: expected_revision,
            actual: state.revision,
        });
    }
    let before = capability_activation(state, entity, capability)?.state;
    if before == CapabilityActivationState::Absent {
        return Err(CapabilityActivationError::CapabilityAbsent { entity, capability });
    }
    let after = match activation {
        CapabilityActivation::Inactive => CapabilityActivationState::Inactive,
        CapabilityActivation::Active => CapabilityActivationState::Active,
    };
    if before == after {
        return Err(CapabilityActivationError::AlreadyInState {
            entity,
            capability,
            state: before,
        });
    }
    let revision_before = state.revision;
    match capability {
        ActivatableCapabilityKind::Collision => {
            state
                .collisions
                .get_mut(&entity)
                .expect("presence checked")
                .enabled = activation == CapabilityActivation::Active;
        }
        ActivatableCapabilityKind::Controller => match activation {
            CapabilityActivation::Inactive => {
                state.inactive_controllers.insert(entity);
            }
            CapabilityActivation::Active => {
                state.inactive_controllers.remove(&entity);
            }
        },
    }
    state.revision = state.revision.saturating_add(1);
    Ok(CapabilityActivationReceipt {
        revision_before,
        revision_after: state.revision,
        entity,
        capability,
        before,
        after,
    })
}
