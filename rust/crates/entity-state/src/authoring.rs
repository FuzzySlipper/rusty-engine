use core_ids::{EntityId, TagId};

use crate::activation::{
    ActivatableCapabilityKind, CapabilityActivation, CapabilityActivationError,
    CapabilityActivationReceipt,
};
use crate::model::{
    bounds_are_valid, half_extents_are_valid, transform_is_valid, validate_definition,
    velocity_is_valid, AssetBindingCapability, BoundsCapability, CollisionCapability,
    ControllerCapability, EntityDefinition, EntityDefinitionError, EntityLifecycle, EntityState,
    KinematicCapability, RenderableCapability, TransformCapability,
};
use crate::relationship::{reroot_transform_children, RelationshipError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityCapabilityKind {
    Transform,
    Bounds,
    Collision,
    Renderable,
    Kinematic,
    Controller,
    AssetBinding,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EntityCapability {
    Transform(TransformCapability),
    Bounds(BoundsCapability),
    Collision(CollisionCapability),
    Renderable(RenderableCapability),
    Kinematic(KinematicCapability),
    Controller(ControllerCapability),
    AssetBinding(AssetBindingCapability),
}

impl EntityCapability {
    pub const fn kind(&self) -> EntityCapabilityKind {
        match self {
            Self::Transform(_) => EntityCapabilityKind::Transform,
            Self::Bounds(_) => EntityCapabilityKind::Bounds,
            Self::Collision(_) => EntityCapabilityKind::Collision,
            Self::Renderable(_) => EntityCapabilityKind::Renderable,
            Self::Kinematic(_) => EntityCapabilityKind::Kinematic,
            Self::Controller(_) => EntityCapabilityKind::Controller,
            Self::AssetBinding(_) => EntityCapabilityKind::AssetBinding,
        }
    }
}

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
    CapabilityAttached {
        entity: EntityId,
        capability: EntityCapabilityKind,
    },
    CapabilityDetached {
        entity: EntityId,
        capability: EntityCapabilityKind,
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
    InvalidDefinition(EntityDefinitionError),
    Relationship(RelationshipError),
    Activation(CapabilityActivationError),
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
    CapabilityAlreadyPresent {
        entity: EntityId,
        capability: EntityCapabilityKind,
    },
    CapabilityAbsent {
        entity: EntityId,
        capability: EntityCapabilityKind,
    },
    InvalidCapability {
        entity: EntityId,
        capability: EntityCapabilityKind,
        reason: &'static str,
    },
    CapabilityInUse {
        entity: EntityId,
        capability: EntityCapabilityKind,
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

impl From<CapabilityActivationError> for EntityAuthoringError {
    fn from(value: CapabilityActivationError) -> Self {
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
            validate_definition(definition)?;
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
        state.remove_capabilities_and_relations(entity);
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

    pub fn attach_capability(
        self,
        state: &mut EntityState,
        expected_revision: u64,
        entity: EntityId,
        capability: EntityCapability,
    ) -> Result<EntityAuthoringReceipt, EntityAuthoringError> {
        ensure_revision(state, expected_revision)?;
        ensure_alive(state, entity)?;
        validate_capability(state, entity, &capability)?;
        let kind = capability.kind();
        if capability_present(state, entity, kind) {
            return Err(EntityAuthoringError::CapabilityAlreadyPresent {
                entity,
                capability: kind,
            });
        }
        match capability {
            EntityCapability::Transform(value) => {
                state.transforms.insert(entity, value);
            }
            EntityCapability::Bounds(value) => {
                state.bounds.insert(entity, value);
            }
            EntityCapability::Collision(value) => {
                state.collisions.insert(entity, value);
            }
            EntityCapability::Renderable(value) => {
                state.renderables.insert(entity, value);
            }
            EntityCapability::Kinematic(value) => {
                state.kinematics.insert(entity, value);
            }
            EntityCapability::Controller(value) => {
                state.controllers.insert(entity, value);
            }
            EntityCapability::AssetBinding(value) => {
                state.asset_bindings.insert(entity, value);
            }
        }
        bump_with_fact(
            state,
            EntityAuthoringFact::CapabilityAttached {
                entity,
                capability: kind,
            },
        )
    }

    pub fn detach_capability(
        self,
        state: &mut EntityState,
        expected_revision: u64,
        entity: EntityId,
        capability: EntityCapabilityKind,
    ) -> Result<EntityAuthoringReceipt, EntityAuthoringError> {
        ensure_revision(state, expected_revision)?;
        ensure_alive(state, entity)?;
        if !capability_present(state, entity, capability) {
            return Err(EntityAuthoringError::CapabilityAbsent { entity, capability });
        }
        if capability == EntityCapabilityKind::Transform && state.kinematics.contains_key(&entity) {
            return Err(EntityAuthoringError::CapabilityInUse {
                entity,
                capability,
                reason: "kinematic capability requires transform",
            });
        }
        match capability {
            EntityCapabilityKind::Transform => {
                reroot_transform_children(state, entity);
                state.transform_parents.remove(&entity);
                state.transforms.remove(&entity);
            }
            EntityCapabilityKind::Bounds => {
                state.bounds.remove(&entity);
            }
            EntityCapabilityKind::Collision => {
                state.collisions.remove(&entity);
            }
            EntityCapabilityKind::Renderable => {
                state.renderables.remove(&entity);
            }
            EntityCapabilityKind::Kinematic => {
                state.kinematics.remove(&entity);
            }
            EntityCapabilityKind::Controller => {
                state.controllers.remove(&entity);
                state.inactive_controllers.remove(&entity);
            }
            EntityCapabilityKind::AssetBinding => {
                state.asset_bindings.remove(&entity);
            }
        }
        bump_with_fact(
            state,
            EntityAuthoringFact::CapabilityDetached { entity, capability },
        )
    }

    pub fn set_capability_activation(
        self,
        state: &mut EntityState,
        expected_revision: u64,
        entity: EntityId,
        capability: ActivatableCapabilityKind,
        activation: CapabilityActivation,
    ) -> Result<CapabilityActivationReceipt, EntityAuthoringError> {
        Ok(crate::activation::set_capability_activation(
            state,
            expected_revision,
            entity,
            capability,
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
            return Ok(EntityAuthoringReceipt {
                revision_before: state.revision,
                revision_after: state.revision,
                facts: Vec::new(),
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

fn capability_present(
    state: &EntityState,
    entity: EntityId,
    capability: EntityCapabilityKind,
) -> bool {
    match capability {
        EntityCapabilityKind::Transform => state.transforms.contains_key(&entity),
        EntityCapabilityKind::Bounds => state.bounds.contains_key(&entity),
        EntityCapabilityKind::Collision => state.collisions.contains_key(&entity),
        EntityCapabilityKind::Renderable => state.renderables.contains_key(&entity),
        EntityCapabilityKind::Kinematic => state.kinematics.contains_key(&entity),
        EntityCapabilityKind::Controller => state.controllers.contains_key(&entity),
        EntityCapabilityKind::AssetBinding => state.asset_bindings.contains_key(&entity),
    }
}

fn validate_capability(
    state: &EntityState,
    entity: EntityId,
    capability: &EntityCapability,
) -> Result<(), EntityAuthoringError> {
    let invalid = |capability, reason| EntityAuthoringError::InvalidCapability {
        entity,
        capability,
        reason,
    };
    match capability {
        EntityCapability::Transform(value) if !transform_is_valid(value.transform()) => Err(
            invalid(EntityCapabilityKind::Transform, "invalid transform"),
        ),
        EntityCapability::Bounds(value) if !bounds_are_valid(*value) => {
            Err(invalid(EntityCapabilityKind::Bounds, "invalid bounds"))
        }
        EntityCapability::Renderable(value) if value.asset.trim().is_empty() => Err(invalid(
            EntityCapabilityKind::Renderable,
            "render asset is empty",
        )),
        EntityCapability::Kinematic(value) if !state.transforms.contains_key(&entity) => Err(
            invalid(EntityCapabilityKind::Kinematic, "transform is absent"),
        ),
        EntityCapability::Kinematic(value)
            if !half_extents_are_valid(value.half_extents)
                || !velocity_is_valid(value.velocity) =>
        {
            Err(invalid(
                EntityCapabilityKind::Kinematic,
                "invalid half extents or velocity",
            ))
        }
        _ => Ok(()),
    }
}
