use std::collections::BTreeSet;

use core_ids::EntityId;

use crate::model::{EntityDefinition, EntityLifecycle, EntityState, TransformComponent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformParentMode {
    KeepLocal,
    KeepWorld,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationshipKind {
    TransformParent,
    Containment,
    SourceAncestry,
    RenderGrouping,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationshipCommand {
    SetTransformParent {
        child: EntityId,
        parent: EntityId,
        mode: TransformParentMode,
    },
    ClearTransformParent {
        child: EntityId,
        keep_world: bool,
    },
    SetContainment {
        child: EntityId,
        container: EntityId,
    },
    ClearContainment {
        child: EntityId,
    },
    SetSourceAncestry {
        entity: EntityId,
        source: EntityId,
    },
    ClearSourceAncestry {
        entity: EntityId,
    },
    SetRenderGroup {
        entity: EntityId,
        group: EntityId,
    },
}

impl RelationshipCommand {
    pub const fn kind(self) -> RelationshipKind {
        match self {
            Self::SetTransformParent { .. } | Self::ClearTransformParent { .. } => {
                RelationshipKind::TransformParent
            }
            Self::SetContainment { .. } | Self::ClearContainment { .. } => {
                RelationshipKind::Containment
            }
            Self::SetSourceAncestry { .. } | Self::ClearSourceAncestry { .. } => {
                RelationshipKind::SourceAncestry
            }
            Self::SetRenderGroup { .. } => RelationshipKind::RenderGrouping,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RelationshipReadout {
    pub entity: EntityId,
    pub transform_parent: Option<EntityId>,
    pub contained_in: Option<EntityId>,
    pub derived_from: Option<EntityId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RelationshipPreview {
    pub revision: u64,
    pub command: RelationshipCommand,
    pub before: RelationshipReadout,
    pub after: RelationshipReadout,
    pub changes_state: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RelationshipReceipt {
    pub revision_before: u64,
    pub revision_after: u64,
    pub command: RelationshipCommand,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelationshipError {
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
    SelfRelationship {
        entity: EntityId,
    },
    MissingTransform {
        entity: EntityId,
    },
    Cycle {
        kind: RelationshipKind,
        entity: EntityId,
        target: EntityId,
    },
    ProjectionOnly {
        kind: RelationshipKind,
    },
    NoSuchRelation {
        entity: EntityId,
        kind: RelationshipKind,
    },
}

impl std::fmt::Display for RelationshipError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "relationship rejected: {self:?}")
    }
}

impl std::error::Error for RelationshipError {}

impl EntityState {
    pub fn relationships(
        &self,
        entity: EntityId,
    ) -> Result<RelationshipReadout, RelationshipError> {
        ensure_alive(self, entity)?;
        Ok(readout(self, entity))
    }

    pub fn preview_relationship(
        &self,
        command: RelationshipCommand,
    ) -> Result<RelationshipPreview, RelationshipError> {
        preview_relationship(self, command)
    }

    pub fn apply_relationship(
        &mut self,
        expected_revision: u64,
        command: RelationshipCommand,
    ) -> Result<RelationshipReceipt, RelationshipError> {
        apply_relationship(self, expected_revision, command)
    }
}

pub fn preview_relationship(
    state: &EntityState,
    command: RelationshipCommand,
) -> Result<RelationshipPreview, RelationshipError> {
    validate_command(state, command)?;
    let entity = command_entity(command);
    let before = readout(state, entity);
    let mut after = before;
    match command {
        RelationshipCommand::SetTransformParent { parent, .. } => {
            after.transform_parent = Some(parent);
        }
        RelationshipCommand::ClearTransformParent { .. } => after.transform_parent = None,
        RelationshipCommand::SetContainment { container, .. } => {
            after.contained_in = Some(container);
        }
        RelationshipCommand::ClearContainment { .. } => after.contained_in = None,
        RelationshipCommand::SetSourceAncestry { source, .. } => {
            after.derived_from = Some(source);
        }
        RelationshipCommand::ClearSourceAncestry { .. } => after.derived_from = None,
        RelationshipCommand::SetRenderGroup { .. } => unreachable!("rejected during validation"),
    }
    Ok(RelationshipPreview {
        revision: state.revision,
        command,
        before,
        after,
        changes_state: before != after,
    })
}

pub fn apply_relationship(
    state: &mut EntityState,
    expected_revision: u64,
    command: RelationshipCommand,
) -> Result<RelationshipReceipt, RelationshipError> {
    if state.revision != expected_revision {
        return Err(RelationshipError::StaleRevision {
            expected: expected_revision,
            actual: state.revision,
        });
    }
    let preview = preview_relationship(state, command)?;
    let revision_before = state.revision;
    if preview.changes_state {
        mutate(state, command);
        state.revision = state.revision.saturating_add(1);
    }
    Ok(RelationshipReceipt {
        revision_before,
        revision_after: state.revision,
        command,
    })
}

pub(crate) fn apply_definition_relationships(
    state: &mut EntityState,
    definition: &EntityDefinition,
) -> Result<(), RelationshipError> {
    if let Some(parent) = definition.transform_parent {
        let command = RelationshipCommand::SetTransformParent {
            child: definition.id,
            parent,
            mode: TransformParentMode::KeepLocal,
        };
        validate_command(state, command)?;
        mutate(state, command);
    }
    if let Some(container) = definition.contained_in {
        let command = RelationshipCommand::SetContainment {
            child: definition.id,
            container,
        };
        validate_command(state, command)?;
        mutate(state, command);
    }
    if let Some(source) = definition.derived_from {
        let command = RelationshipCommand::SetSourceAncestry {
            entity: definition.id,
            source,
        };
        validate_command(state, command)?;
        mutate(state, command);
    }
    Ok(())
}

pub(crate) fn reroot_transform_children(state: &mut EntityState, parent: EntityId) {
    let children: Vec<_> = state
        .transform_parents
        .iter()
        .filter_map(|(child, current_parent)| (*current_parent == parent).then_some(*child))
        .collect();
    for child in children {
        let world = state.world_transform(child);
        state.transform_parents.remove(&child);
        if let Some(world) = world {
            state
                .components
                .insert_unchecked(child, TransformComponent::from_transform(world));
        }
    }
}

fn validate_command(
    state: &EntityState,
    command: RelationshipCommand,
) -> Result<(), RelationshipError> {
    match command {
        RelationshipCommand::SetTransformParent { child, parent, .. } => {
            ensure_pair(state, child, parent)?;
            ensure_transform(state, child)?;
            ensure_transform(state, parent)?;
            ensure_acyclic(
                &state.transform_parents,
                RelationshipKind::TransformParent,
                child,
                parent,
            )?;
        }
        RelationshipCommand::ClearTransformParent { child, .. } => {
            ensure_alive(state, child)?;
            ensure_transform(state, child)?;
            ensure_relation_present(
                state.transform_parents.contains_key(&child),
                child,
                RelationshipKind::TransformParent,
            )?;
        }
        RelationshipCommand::SetContainment { child, container } => {
            ensure_pair(state, child, container)?;
            let links = state
                .containment
                .iter()
                .map(|(entity, container)| (*entity, *container))
                .collect();
            ensure_acyclic(&links, RelationshipKind::Containment, child, container)?;
        }
        RelationshipCommand::ClearContainment { child } => {
            ensure_alive(state, child)?;
            ensure_relation_present(
                state.containment.contains_key(&child),
                child,
                RelationshipKind::Containment,
            )?;
        }
        RelationshipCommand::SetSourceAncestry { entity, source } => {
            ensure_alive(state, entity)?;
            ensure_known(state, source)?;
        }
        RelationshipCommand::ClearSourceAncestry { entity } => {
            ensure_alive(state, entity)?;
            ensure_relation_present(
                state.derived_from.contains_key(&entity),
                entity,
                RelationshipKind::SourceAncestry,
            )?;
        }
        RelationshipCommand::SetRenderGroup { .. } => {
            return Err(RelationshipError::ProjectionOnly {
                kind: RelationshipKind::RenderGrouping,
            });
        }
    }
    Ok(())
}

fn mutate(state: &mut EntityState, command: RelationshipCommand) {
    match command {
        RelationshipCommand::SetTransformParent {
            child,
            parent,
            mode,
        } => {
            let world = (mode == TransformParentMode::KeepWorld)
                .then(|| state.world_transform(child))
                .flatten();
            state.transform_parents.insert(child, parent);
            if let (Some(world), Some(parent_world)) = (world, state.world_transform(parent)) {
                state.components.insert_unchecked(
                    child,
                    TransformComponent::from_transform(parent_world.relative_to(world)),
                );
            }
        }
        RelationshipCommand::ClearTransformParent { child, keep_world } => {
            let world = keep_world.then(|| state.world_transform(child)).flatten();
            state.transform_parents.remove(&child);
            if let Some(world) = world {
                state
                    .components
                    .insert_unchecked(child, TransformComponent::from_transform(world));
            }
        }
        RelationshipCommand::SetContainment { child, container } => {
            if let Some(previous) = state.containment.insert(child, container) {
                remove_containment_child(state, previous, child);
            }
            state
                .containment_children
                .entry(container)
                .or_default()
                .insert(child);
        }
        RelationshipCommand::ClearContainment { child } => {
            if let Some(container) = state.containment.remove(&child) {
                remove_containment_child(state, container, child);
            }
        }
        RelationshipCommand::SetSourceAncestry { entity, source } => {
            state.derived_from.insert(entity, source);
        }
        RelationshipCommand::ClearSourceAncestry { entity } => {
            state.derived_from.remove(&entity);
        }
        RelationshipCommand::SetRenderGroup { .. } => unreachable!("validated before mutation"),
    }
}

fn remove_containment_child(state: &mut EntityState, container: EntityId, child: EntityId) {
    let remove_container = state
        .containment_children
        .get_mut(&container)
        .is_some_and(|children| {
            children.remove(&child);
            children.is_empty()
        });
    if remove_container {
        state.containment_children.remove(&container);
    }
}

fn command_entity(command: RelationshipCommand) -> EntityId {
    match command {
        RelationshipCommand::SetTransformParent { child, .. }
        | RelationshipCommand::ClearTransformParent { child, .. }
        | RelationshipCommand::SetContainment { child, .. }
        | RelationshipCommand::ClearContainment { child }
        | RelationshipCommand::SetSourceAncestry { entity: child, .. }
        | RelationshipCommand::ClearSourceAncestry { entity: child }
        | RelationshipCommand::SetRenderGroup { entity: child, .. } => child,
    }
}

fn readout(state: &EntityState, entity: EntityId) -> RelationshipReadout {
    RelationshipReadout {
        entity,
        transform_parent: state.transform_parents.get(&entity).copied(),
        contained_in: state.containment.get(&entity).copied(),
        derived_from: state.derived_from.get(&entity).copied(),
    }
}

fn ensure_alive(state: &EntityState, entity: EntityId) -> Result<(), RelationshipError> {
    match state.entities.get(&entity) {
        None => Err(RelationshipError::UnknownEntity { entity }),
        Some(core) if core.lifecycle == EntityLifecycle::Tombstoned => {
            Err(RelationshipError::TombstonedEntity { entity })
        }
        Some(_) => Ok(()),
    }
}

fn ensure_known(state: &EntityState, entity: EntityId) -> Result<(), RelationshipError> {
    state
        .entities
        .contains_key(&entity)
        .then_some(())
        .ok_or(RelationshipError::UnknownEntity { entity })
}

fn ensure_pair(
    state: &EntityState,
    entity: EntityId,
    target: EntityId,
) -> Result<(), RelationshipError> {
    ensure_alive(state, entity)?;
    ensure_alive(state, target)?;
    if entity == target {
        return Err(RelationshipError::SelfRelationship { entity });
    }
    Ok(())
}

fn ensure_transform(state: &EntityState, entity: EntityId) -> Result<(), RelationshipError> {
    if state.transform(entity).is_some() {
        Ok(())
    } else {
        Err(RelationshipError::MissingTransform { entity })
    }
}

fn ensure_relation_present(
    present: bool,
    entity: EntityId,
    kind: RelationshipKind,
) -> Result<(), RelationshipError> {
    present
        .then_some(())
        .ok_or(RelationshipError::NoSuchRelation { entity, kind })
}

fn ensure_acyclic(
    links: &std::collections::BTreeMap<EntityId, EntityId>,
    kind: RelationshipKind,
    entity: EntityId,
    target: EntityId,
) -> Result<(), RelationshipError> {
    let mut cursor = Some(target);
    let mut seen = BTreeSet::new();
    while let Some(current) = cursor {
        if current == entity || !seen.insert(current) {
            return Err(RelationshipError::Cycle {
                kind,
                entity,
                target,
            });
        }
        cursor = links.get(&current).copied();
    }
    Ok(())
}
