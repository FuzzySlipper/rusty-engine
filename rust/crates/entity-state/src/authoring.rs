use std::any::{Any, TypeId};

use core_ids::{EntityId, TagId};

use crate::activation::{
    ActivatableComponentKind, ComponentActivation, ComponentActivationError,
    ComponentActivationReceipt,
};
use crate::component::{ComponentRevision, ComponentTypeId, EntityComponent};
use crate::model::{
    ControllerComponent, EntityDefinition, EntityDefinitionError, EntityLifecycle, EntityState,
    KinematicComponent, TransformComponent,
};
use crate::relationship::{reroot_transform_children, RelationshipError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntityAuthoringFact {
    Created {
        entity: EntityId,
    },
    Destroyed {
        entity: EntityId,
    },
    LifecycleChanged {
        entity: EntityId,
        before: EntityLifecycle,
        after: EntityLifecycle,
    },
    LabelAdded {
        entity: EntityId,
        label: TagId,
    },
    LabelRemoved {
        entity: EntityId,
        label: TagId,
    },
    ComponentAttached {
        entity: EntityId,
        component: ComponentTypeId,
    },
    ComponentReplaced {
        entity: EntityId,
        component: ComponentTypeId,
    },
    ComponentDetached {
        entity: EntityId,
        component: ComponentTypeId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityAuthoringReceipt {
    pub revision_before: u64,
    pub revision_after: u64,
    pub facts: Vec<EntityAuthoringFact>,
}

#[derive(Debug)]
pub enum EntityAuthoringError {
    StaleRevision {
        expected: u64,
        actual: u64,
    },
    ComponentRevisionScopeMismatch {
        entity: EntityId,
        component: ComponentTypeId,
        guard_entity: EntityId,
        guard_component: ComponentTypeId,
    },
    StaleComponentRevision {
        entity: EntityId,
        component: ComponentTypeId,
        expected: u64,
        actual: u64,
    },
    InvalidDefinition(EntityDefinitionError),
    Relationship(RelationshipError),
    Activation(ComponentActivationError),
    UnknownEntity {
        entity: EntityId,
    },
    DuplicateEntity {
        entity: EntityId,
    },
    TombstonedEntity {
        entity: EntityId,
    },
    InvalidLifecycleTransition {
        entity: EntityId,
        from: EntityLifecycle,
        to: EntityLifecycle,
    },
    DuplicateLabel {
        entity: EntityId,
        label: TagId,
    },
    MissingLabel {
        entity: EntityId,
        label: TagId,
    },
    UnregisteredComponent {
        rust_type: &'static str,
    },
    ComponentAlreadyPresent {
        entity: EntityId,
        component: ComponentTypeId,
    },
    ComponentAbsent {
        entity: EntityId,
        component: ComponentTypeId,
    },
    InvalidComponent {
        entity: EntityId,
        component: ComponentTypeId,
        reason: String,
    },
    ComponentInUse {
        entity: EntityId,
        component: ComponentTypeId,
        reason: &'static str,
    },
}

impl std::fmt::Display for EntityAuthoringError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "entity authoring rejected: {self:?}")
    }
}

impl std::error::Error for EntityAuthoringError {}

impl From<EntityDefinitionError> for EntityAuthoringError {
    fn from(value: EntityDefinitionError) -> Self {
        Self::InvalidDefinition(value)
    }
}

impl From<RelationshipError> for EntityAuthoringError {
    fn from(value: RelationshipError) -> Self {
        Self::Relationship(value)
    }
}

impl From<ComponentActivationError> for EntityAuthoringError {
    fn from(value: ComponentActivationError) -> Self {
        Self::Activation(value)
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct EntityAuthoringService;

impl EntityAuthoringService {
    pub fn admit(
        self,
        state: &mut EntityState,
        expected_revision: u64,
        definitions: impl IntoIterator<Item = EntityDefinition>,
    ) -> Result<EntityAuthoringReceipt, EntityAuthoringError> {
        ensure_revision(state, expected_revision)?;
        let definitions: Vec<_> = definitions.into_iter().collect();
        let mut staged = state.clone();
        let mut facts = Vec::with_capacity(definitions.len());
        for definition in &definitions {
            crate::definition::validate_definition(definition)?;
            if staged.entities.contains_key(&definition.id) {
                return Err(EntityAuthoringError::DuplicateEntity {
                    entity: definition.id,
                });
            }
            staged.insert_definition(definition.clone());
            facts.push(EntityAuthoringFact::Created {
                entity: definition.id,
            });
        }
        for definition in &definitions {
            crate::relationship::apply_definition_relationships(&mut staged, definition)?;
        }
        commit(state, staged, facts)
    }

    pub fn destroy(
        self,
        state: &mut EntityState,
        expected_revision: u64,
        entity: EntityId,
    ) -> Result<EntityAuthoringReceipt, EntityAuthoringError> {
        ensure_revision(state, expected_revision)?;
        ensure_alive(state, entity)?;
        reroot_transform_children(state, entity);
        state.remove_components_and_relations(entity);
        state
            .entities
            .get_mut(&entity)
            .expect("entity presence checked")
            .lifecycle = EntityLifecycle::Tombstoned;
        bump_with_fact(state, EntityAuthoringFact::Destroyed { entity })
    }

    pub fn disable(
        self,
        state: &mut EntityState,
        expected_revision: u64,
        entity: EntityId,
    ) -> Result<EntityAuthoringReceipt, EntityAuthoringError> {
        self.set_lifecycle(state, expected_revision, entity, EntityLifecycle::Disabled)
    }

    pub fn enable(
        self,
        state: &mut EntityState,
        expected_revision: u64,
        entity: EntityId,
    ) -> Result<EntityAuthoringReceipt, EntityAuthoringError> {
        self.set_lifecycle(state, expected_revision, entity, EntityLifecycle::Active)
    }

    pub fn add_label(
        self,
        state: &mut EntityState,
        expected_revision: u64,
        entity: EntityId,
        label: TagId,
    ) -> Result<EntityAuthoringReceipt, EntityAuthoringError> {
        ensure_revision(state, expected_revision)?;
        ensure_alive(state, entity)?;
        let labels = &mut state
            .entities
            .get_mut(&entity)
            .expect("entity presence checked")
            .labels;
        match labels.binary_search(&label) {
            Ok(_) => Err(EntityAuthoringError::DuplicateLabel { entity, label }),
            Err(index) => {
                labels.insert(index, label);
                bump_with_fact(state, EntityAuthoringFact::LabelAdded { entity, label })
            }
        }
    }

    pub fn remove_label(
        self,
        state: &mut EntityState,
        expected_revision: u64,
        entity: EntityId,
        label: TagId,
    ) -> Result<EntityAuthoringReceipt, EntityAuthoringError> {
        ensure_revision(state, expected_revision)?;
        ensure_alive(state, entity)?;
        let labels = &mut state
            .entities
            .get_mut(&entity)
            .expect("entity presence checked")
            .labels;
        let index = labels
            .binary_search(&label)
            .map_err(|_| EntityAuthoringError::MissingLabel { entity, label })?;
        labels.remove(index);
        bump_with_fact(state, EntityAuthoringFact::LabelRemoved { entity, label })
    }

    pub fn attach_component<T: EntityComponent>(
        self,
        state: &mut EntityState,
        expected_revision: ComponentRevision,
        entity: EntityId,
        component: T,
    ) -> Result<EntityAuthoringReceipt, EntityAuthoringError> {
        ensure_alive(state, entity)?;
        let type_id = ensure_component_revision::<T>(state, entity, &expected_revision)?;
        if state
            .components
            .has::<T>(entity)
            .expect("registration checked")
        {
            return Err(EntityAuthoringError::ComponentAlreadyPresent {
                entity,
                component: type_id,
            });
        }
        validate_component(state, entity, &component, &type_id)?;
        state.components.insert_unchecked(entity, component);
        bump_with_fact(
            state,
            EntityAuthoringFact::ComponentAttached {
                entity,
                component: type_id,
            },
        )
    }

    pub fn replace_component<T: EntityComponent + PartialEq>(
        self,
        state: &mut EntityState,
        expected_revision: ComponentRevision,
        entity: EntityId,
        component: T,
    ) -> Result<EntityAuthoringReceipt, EntityAuthoringError> {
        ensure_alive(state, entity)?;
        let type_id = ensure_component_revision::<T>(state, entity, &expected_revision)?;
        let before = state
            .components
            .get::<T>(entity)
            .expect("registration checked")
            .ok_or_else(|| EntityAuthoringError::ComponentAbsent {
                entity,
                component: type_id.clone(),
            })?;
        validate_component(state, entity, &component, &type_id)?;
        if before == &component {
            return Ok(EntityAuthoringReceipt {
                revision_before: state.revision,
                revision_after: state.revision,
                facts: Vec::new(),
            });
        }
        state.components.insert_unchecked(entity, component);
        bump_with_fact(
            state,
            EntityAuthoringFact::ComponentReplaced {
                entity,
                component: type_id,
            },
        )
    }

    pub fn detach_component<T: EntityComponent>(
        self,
        state: &mut EntityState,
        expected_revision: ComponentRevision,
        entity: EntityId,
    ) -> Result<EntityAuthoringReceipt, EntityAuthoringError> {
        ensure_alive(state, entity)?;
        let type_id = ensure_component_revision::<T>(state, entity, &expected_revision)?;
        if !state
            .components
            .has::<T>(entity)
            .expect("registration checked")
        {
            return Err(EntityAuthoringError::ComponentAbsent {
                entity,
                component: type_id,
            });
        }
        validate_detach::<T>(state, entity, &type_id)?;
        if TypeId::of::<T>() == TypeId::of::<TransformComponent>() {
            reroot_transform_children(state, entity);
            state.transform_parents.remove(&entity);
        }
        state.components.remove_unchecked::<T>(entity);
        if TypeId::of::<T>() == TypeId::of::<ControllerComponent>() {
            state.inactive_controllers.remove(&entity);
        }
        bump_with_fact(
            state,
            EntityAuthoringFact::ComponentDetached {
                entity,
                component: type_id,
            },
        )
    }

    pub fn set_component_activation(
        self,
        state: &mut EntityState,
        expected_revision: u64,
        entity: EntityId,
        component: ActivatableComponentKind,
        activation: ComponentActivation,
    ) -> Result<ComponentActivationReceipt, EntityAuthoringError> {
        Ok(crate::activation::set_component_activation(
            state,
            expected_revision,
            entity,
            component,
            activation,
        )?)
    }

    fn set_lifecycle(
        self,
        state: &mut EntityState,
        expected_revision: u64,
        entity: EntityId,
        lifecycle: EntityLifecycle,
    ) -> Result<EntityAuthoringReceipt, EntityAuthoringError> {
        ensure_revision(state, expected_revision)?;
        let core = state
            .entities
            .get_mut(&entity)
            .ok_or(EntityAuthoringError::UnknownEntity { entity })?;
        let before = core.lifecycle;
        if before == EntityLifecycle::Tombstoned || lifecycle == EntityLifecycle::Tombstoned {
            return Err(EntityAuthoringError::InvalidLifecycleTransition {
                entity,
                from: before,
                to: lifecycle,
            });
        }
        if before == lifecycle {
            return Err(EntityAuthoringError::InvalidLifecycleTransition {
                entity,
                from: before,
                to: lifecycle,
            });
        }
        core.lifecycle = lifecycle;
        bump_with_fact(
            state,
            EntityAuthoringFact::LifecycleChanged {
                entity,
                before,
                after: lifecycle,
            },
        )
    }
}

fn registered_type_id<T: EntityComponent>(
    state: &EntityState,
) -> Result<ComponentTypeId, EntityAuthoringError> {
    state.components.type_id_for::<T>().cloned().map_err(|_| {
        EntityAuthoringError::UnregisteredComponent {
            rust_type: std::any::type_name::<T>(),
        }
    })
}

fn ensure_component_revision<T: EntityComponent>(
    state: &EntityState,
    entity: EntityId,
    expected: &ComponentRevision,
) -> Result<ComponentTypeId, EntityAuthoringError> {
    let component = registered_type_id::<T>(state)?;
    if expected.entity != entity || expected.component != component {
        return Err(EntityAuthoringError::ComponentRevisionScopeMismatch {
            entity,
            component,
            guard_entity: expected.entity,
            guard_component: expected.component.clone(),
        });
    }
    let actual = state
        .components
        .revision::<T>(entity)
        .expect("registration checked");
    if expected.revision != actual {
        return Err(EntityAuthoringError::StaleComponentRevision {
            entity,
            component,
            expected: expected.revision,
            actual,
        });
    }
    Ok(component)
}

fn validate_component<T: EntityComponent>(
    state: &EntityState,
    entity: EntityId,
    value: &T,
    type_id: &ComponentTypeId,
) -> Result<(), EntityAuthoringError> {
    state
        .components
        .validate(value)
        .map_err(|error| EntityAuthoringError::InvalidComponent {
            entity,
            component: type_id.clone(),
            reason: error.reason,
        })?;
    if TypeId::of::<T>() == TypeId::of::<KinematicComponent>() && state.transform(entity).is_none()
    {
        return Err(EntityAuthoringError::InvalidComponent {
            entity,
            component: type_id.clone(),
            reason: "transform component is absent".to_string(),
        });
    }
    if TypeId::of::<T>() == TypeId::of::<TransformComponent>()
        && state
            .collision(entity)
            .is_some_and(|collision| collision.enabled && collision.static_collider)
    {
        let requested = (value as &dyn Any)
            .downcast_ref::<TransformComponent>()
            .expect("type identity checked");
        if state
            .transform(entity)
            .is_some_and(|current| current != requested)
        {
            return Err(EntityAuthoringError::ComponentInUse {
                entity,
                component: type_id.clone(),
                reason: "active static collision prevents transform replacement",
            });
        }
    }
    Ok(())
}

fn validate_detach<T: EntityComponent>(
    state: &EntityState,
    entity: EntityId,
    type_id: &ComponentTypeId,
) -> Result<(), EntityAuthoringError> {
    if TypeId::of::<T>() == TypeId::of::<TransformComponent>() && state.kinematic(entity).is_some()
    {
        return Err(EntityAuthoringError::ComponentInUse {
            entity,
            component: type_id.clone(),
            reason: "kinematic component requires transform",
        });
    }
    Ok(())
}

fn ensure_revision(state: &EntityState, expected: u64) -> Result<(), EntityAuthoringError> {
    if state.revision == expected {
        Ok(())
    } else {
        Err(EntityAuthoringError::StaleRevision {
            expected,
            actual: state.revision,
        })
    }
}

fn ensure_alive(state: &EntityState, entity: EntityId) -> Result<(), EntityAuthoringError> {
    match state.entities.get(&entity) {
        None => Err(EntityAuthoringError::UnknownEntity { entity }),
        Some(core) if core.lifecycle == EntityLifecycle::Tombstoned => {
            Err(EntityAuthoringError::TombstonedEntity { entity })
        }
        Some(_) => Ok(()),
    }
}

fn commit(
    state: &mut EntityState,
    mut staged: EntityState,
    facts: Vec<EntityAuthoringFact>,
) -> Result<EntityAuthoringReceipt, EntityAuthoringError> {
    let revision_before = state.revision;
    if !facts.is_empty() {
        staged.revision = revision_before.saturating_add(1);
        *state = staged;
    }
    Ok(EntityAuthoringReceipt {
        revision_before,
        revision_after: state.revision,
        facts,
    })
}

fn bump_with_fact(
    state: &mut EntityState,
    fact: EntityAuthoringFact,
) -> Result<EntityAuthoringReceipt, EntityAuthoringError> {
    let revision_before = state.revision;
    state.revision = state.revision.saturating_add(1);
    Ok(EntityAuthoringReceipt {
        revision_before,
        revision_after: state.revision,
        facts: vec![fact],
    })
}
