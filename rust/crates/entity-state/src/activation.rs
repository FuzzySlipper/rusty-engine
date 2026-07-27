use core_ids::EntityId;

use crate::model::{EntityLifecycle, EntityState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivatableComponentKind {
    Collision,
    Controller,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentActivation {
    Inactive,
    Active,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentActivationState {
    Absent,
    Inactive,
    Active,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentActivationReadout {
    pub entity: EntityId,
    pub component: ActivatableComponentKind,
    pub state: ComponentActivationState,
    pub effective: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentActivationReceipt {
    pub revision_before: u64,
    pub revision_after: u64,
    pub entity: EntityId,
    pub component: ActivatableComponentKind,
    pub before: ComponentActivationState,
    pub after: ComponentActivationState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentActivationError {
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
    ComponentAbsent {
        entity: EntityId,
        component: ActivatableComponentKind,
    },
    AlreadyInState {
        entity: EntityId,
        component: ActivatableComponentKind,
        state: ComponentActivationState,
    },
}

impl std::fmt::Display for ComponentActivationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "component activation rejected: {self:?}")
    }
}

impl std::error::Error for ComponentActivationError {}

impl EntityState {
    pub fn component_activation(
        &self,
        entity: EntityId,
        component: ActivatableComponentKind,
    ) -> Result<ComponentActivationReadout, ComponentActivationError> {
        component_activation(self, entity, component)
    }

    pub fn set_component_activation(
        &mut self,
        expected_revision: u64,
        entity: EntityId,
        component: ActivatableComponentKind,
        activation: ComponentActivation,
    ) -> Result<ComponentActivationReceipt, ComponentActivationError> {
        set_component_activation(self, expected_revision, entity, component, activation)
    }
}

pub fn component_activation(
    state: &EntityState,
    entity: EntityId,
    component: ActivatableComponentKind,
) -> Result<ComponentActivationReadout, ComponentActivationError> {
    let core = state
        .entities
        .get(&entity)
        .ok_or(ComponentActivationError::UnknownEntity { entity })?;
    if core.lifecycle == EntityLifecycle::Tombstoned {
        return Err(ComponentActivationError::TombstonedEntity { entity });
    }
    let component_state = match component {
        ActivatableComponentKind::Collision => match state.collision(entity) {
            None => ComponentActivationState::Absent,
            Some(value) if value.enabled => ComponentActivationState::Active,
            Some(_) => ComponentActivationState::Inactive,
        },
        ActivatableComponentKind::Controller => {
            if state.controller(entity).is_none() {
                ComponentActivationState::Absent
            } else if state.inactive_controllers.contains(&entity) {
                ComponentActivationState::Inactive
            } else {
                ComponentActivationState::Active
            }
        }
    };
    Ok(ComponentActivationReadout {
        entity,
        component,
        state: component_state,
        effective: core.lifecycle == EntityLifecycle::Active
            && component_state == ComponentActivationState::Active,
    })
}

pub fn set_component_activation(
    state: &mut EntityState,
    expected_revision: u64,
    entity: EntityId,
    component: ActivatableComponentKind,
    activation: ComponentActivation,
) -> Result<ComponentActivationReceipt, ComponentActivationError> {
    if state.revision != expected_revision {
        return Err(ComponentActivationError::StaleRevision {
            expected: expected_revision,
            actual: state.revision,
        });
    }
    let before = component_activation(state, entity, component)?.state;
    if before == ComponentActivationState::Absent {
        return Err(ComponentActivationError::ComponentAbsent { entity, component });
    }
    let after = match activation {
        ComponentActivation::Inactive => ComponentActivationState::Inactive,
        ComponentActivation::Active => ComponentActivationState::Active,
    };
    if before == after {
        return Err(ComponentActivationError::AlreadyInState {
            entity,
            component,
            state: before,
        });
    }
    let revision_before = state.revision;
    match component {
        ActivatableComponentKind::Collision => {
            let mut collision = *state.collision(entity).expect("presence checked");
            collision.enabled = activation == ComponentActivation::Active;
            state.components.insert_unchecked(entity, collision);
        }
        ActivatableComponentKind::Controller => match activation {
            ComponentActivation::Inactive => {
                state.inactive_controllers.insert(entity);
            }
            ComponentActivation::Active => {
                state.inactive_controllers.remove(&entity);
            }
        },
    }
    state.revision = state.revision.saturating_add(1);
    Ok(ComponentActivationReceipt {
        revision_before,
        revision_after: state.revision,
        entity,
        component,
        before,
        after,
    })
}
