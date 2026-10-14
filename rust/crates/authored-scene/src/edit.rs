use std::collections::{btree_map::Entry, BTreeMap, BTreeSet};

use core_assets::AssetReference;
use core_ids::SceneNodeId;

use crate::validation::{
    missing_dependency, valid_stable_id, validate_node_fields, voxel_world_errors,
};
use crate::{
    FlatSceneDocument, SceneLight, SceneNodeKind, SceneNodeRecord, SceneTransform,
    SceneValidationError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneObjectRecord {
    pub id: SceneNodeId,
    pub parent: Option<SceneNodeId>,
    pub child_order: u32,
    pub label: Option<String>,
    pub kind: &'static str,
    pub has_renderable_asset: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneObjectSnapshot {
    pub scene_revision: u64,
    pub objects: Vec<SceneObjectRecord>,
}

impl SceneObjectSnapshot {
    /// Lists the document's nodes in identity order.
    pub fn from_document(document: &FlatSceneDocument) -> Self {
        let mut objects = document
            .nodes
            .iter()
            .map(|node| SceneObjectRecord {
                id: node.id,
                parent: node.parent,
                child_order: node.child_order,
                label: node.metadata.label.clone(),
                kind: node.kind.tag(),
                has_renderable_asset: node.kind.asset().is_some(),
            })
            .collect::<Vec<_>>();
        objects.sort_by_key(|object| object.id.raw());
        Self {
            scene_revision: document.revision,
            objects,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SceneEditCommand {
    Create {
        record: SceneNodeRecord,
    },
    Delete {
        id: SceneNodeId,
    },
    Rename {
        id: SceneNodeId,
        label: Option<String>,
    },
    Reparent {
        id: SceneNodeId,
        parent: Option<SceneNodeId>,
        child_order: u32,
    },
    UpdateLight {
        id: SceneNodeId,
        light: SceneLight,
    },
    SetTransform {
        id: SceneNodeId,
        transform: SceneTransform,
    },
    SetRenderableTransform {
        id: SceneNodeId,
        transform: SceneTransform,
    },
    SetKind {
        id: SceneNodeId,
        kind: SceneNodeKind,
    },
    RetargetVoxelAsset {
        id: SceneNodeId,
        asset: AssetReference,
        tags: Vec<String>,
    },
    Select {
        id: Option<SceneNodeId>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneEditReceipt {
    pub revision_before: u64,
    pub revision_after: u64,
    pub selected: Option<SceneNodeId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SceneEditError {
    StaleRevision {
        expected: u64,
        actual: u64,
    },
    RevisionOverflow,
    InvalidAfter {
        errors: Vec<SceneValidationError>,
    },
    MissingObject {
        id: SceneNodeId,
    },
    DuplicateObject {
        id: SceneNodeId,
    },
    MissingParent {
        id: SceneNodeId,
        parent: SceneNodeId,
    },
    SelfParent {
        id: SceneNodeId,
    },
    BlankLabel {
        id: SceneNodeId,
    },
    WrongObjectKind {
        id: SceneNodeId,
    },
}

impl SceneEditError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::StaleRevision { .. } => "stale-scene-revision",
            Self::RevisionOverflow => "scene-revision-overflow",
            Self::InvalidAfter { .. } => "invalid-scene-after-edit",
            Self::MissingObject { .. } => "missing-scene-object",
            Self::DuplicateObject { .. } => "duplicate-scene-object",
            Self::MissingParent { .. } => "missing-scene-object-parent",
            Self::SelfParent { .. } => "scene-object-self-parent",
            Self::BlankLabel { .. } => "blank-scene-object-label",
            Self::WrongObjectKind { .. } => "wrong-scene-object-kind",
        }
    }
}

impl std::fmt::Display for SceneEditError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "scene edit rejected: {self:?}")
    }
}

impl std::error::Error for SceneEditError {}

#[derive(Debug, Default, Clone, Copy)]
pub struct SceneEditService;

impl SceneEditService {
    /// Applies one typed edit to a valid scene document, such as one
    /// [`crate::decode_scene`] returned or this service produced.
    ///
    /// The edit checks the node it writes and only the relationships that edit
    /// can change: identity and parent for created nodes, parent cycles for a
    /// reparent, unique marker, instance and bootstrap identities, spawn
    /// markers that a removed or changed marker leaves referenced, composed
    /// voxel placement below a node whose frame changes, and the dependency
    /// list derived from node assets. A refused edit leaves the document
    /// unchanged.
    pub fn apply(
        self,
        document: &mut FlatSceneDocument,
        expected_revision: u64,
        command: SceneEditCommand,
    ) -> Result<SceneEditReceipt, SceneEditError> {
        if document.revision != expected_revision {
            return Err(SceneEditError::StaleRevision {
                expected: expected_revision,
                actual: document.revision,
            });
        }
        let revision_before = document.revision;
        if let SceneEditCommand::Select { id } = command {
            if let Some(id) = id {
                index_of(document, id)?;
            }
            return Ok(SceneEditReceipt {
                revision_before,
                revision_after: revision_before,
                selected: id,
            });
        }
        let revision_after = revision_before
            .checked_add(1)
            .ok_or(SceneEditError::RevisionOverflow)?;

        let selected = match command {
            SceneEditCommand::Create { record } => {
                create(document, record)?;
                None
            }
            SceneEditCommand::Delete { id } => {
                delete(document, id)?;
                None
            }
            SceneEditCommand::Rename { id, label } => {
                require_non_blank_label(id, label.as_deref())?;
                let index = index_of(document, id)?;
                let mut candidate = document.nodes[index].clone();
                candidate.metadata.label = label;
                replace_node(document, index, candidate, |_, _, _| None)?;
                Some(id)
            }
            SceneEditCommand::Reparent {
                id,
                parent,
                child_order,
            } => {
                validate_parent(document, id, parent)?;
                let index = index_of(document, id)?;
                let mut candidate = document.nodes[index].clone();
                candidate.parent = parent;
                candidate.child_order = child_order;
                replace_node(document, index, candidate, |document, _, errors| {
                    match parent.and_then(|parent| cycle_through(document, id, parent)) {
                        Some(path) => errors.push(SceneValidationError::Cycle { path }),
                        None => voxel_errors_below(document, id, errors),
                    }
                    None
                })?;
                Some(id)
            }
            SceneEditCommand::UpdateLight { id, light } => {
                let index = index_of(document, id)?;
                if !matches!(document.nodes[index].kind, SceneNodeKind::Light(_)) {
                    return Err(SceneEditError::WrongObjectKind { id });
                }
                let mut candidate = document.nodes[index].clone();
                candidate.kind = SceneNodeKind::Light(light);
                replace_node(document, index, candidate, |_, _, _| None)?;
                Some(id)
            }
            SceneEditCommand::SetTransform { id, transform } => {
                let index = index_of(document, id)?;
                let mut candidate = document.nodes[index].clone();
                candidate.transform = transform;
                replace_node(document, index, candidate, |document, previous, errors| {
                    // A composed world rotation or scale depends only on the
                    // rotations and scales along the chain.
                    if errors.is_empty()
                        && (previous.transform.rotation != transform.rotation
                            || previous.transform.scale != transform.scale)
                    {
                        voxel_errors_below(document, id, errors);
                    }
                    None
                })?;
                Some(id)
            }
            SceneEditCommand::SetRenderableTransform { id, transform } => {
                let index = index_of(document, id)?;
                if document.nodes[index].kind.asset().is_none() {
                    return Err(SceneEditError::WrongObjectKind { id });
                }
                let mut candidate = document.nodes[index].clone();
                candidate.renderable_transform = transform;
                replace_node(document, index, candidate, |_, _, _| None)?;
                Some(id)
            }
            SceneEditCommand::SetKind { id, kind } => {
                let index = index_of(document, id)?;
                let mut candidate = document.nodes[index].clone();
                candidate.kind = kind;
                if candidate.kind.asset().is_none() {
                    candidate.renderable_transform = SceneTransform::IDENTITY;
                }
                replace_node(document, index, candidate, |document, previous, errors| {
                    let node = &document.nodes[index];
                    validate_kind_relations(document, node, errors);
                    if let SceneNodeKind::Marker(marker) = &previous.kind {
                        let kept = matches!(&node.kind, SceneNodeKind::Marker(current)
                            if current.marker_id == marker.marker_id);
                        if !kept {
                            spawn_references(
                                document,
                                &BTreeSet::from([marker.marker_id.as_str()]),
                                errors,
                            );
                        }
                    }
                    if matches!(node.kind, SceneNodeKind::VoxelVolume(_)) {
                        if let Some(world) = world_transform(document, id) {
                            voxel_world_errors(id, world, errors);
                        }
                    }
                    (previous.kind.asset().is_some() || node.kind.asset().is_some())
                        .then(|| reconciled_dependencies(document.nodes.iter(), errors))
                })?;
                Some(id)
            }
            SceneEditCommand::RetargetVoxelAsset { id, asset, tags } => {
                let index = index_of(document, id)?;
                if !matches!(document.nodes[index].kind, SceneNodeKind::VoxelVolume(_)) {
                    return Err(SceneEditError::WrongObjectKind { id });
                }
                let mut candidate = document.nodes[index].clone();
                candidate.kind = SceneNodeKind::VoxelVolume(asset);
                candidate.metadata.tags = tags;
                candidate.canonicalize();
                replace_node(document, index, candidate, |document, _, errors| {
                    Some(reconciled_dependencies(document.nodes.iter(), errors))
                })?;
                Some(id)
            }
            SceneEditCommand::Select { .. } => unreachable!("selection returned above"),
        };
        document.revision = revision_after;
        Ok(SceneEditReceipt {
            revision_before,
            revision_after,
            selected,
        })
    }
}

fn find(document: &FlatSceneDocument, id: SceneNodeId) -> Option<&SceneNodeRecord> {
    document.nodes.iter().find(|node| node.id == id)
}

fn index_of(document: &FlatSceneDocument, id: SceneNodeId) -> Result<usize, SceneEditError> {
    document
        .nodes
        .iter()
        .position(|node| node.id == id)
        .ok_or(SceneEditError::MissingObject { id })
}

/// The node and its ancestors, nearest first. The walk is bounded by the node
/// count, so it ends on any document.
fn ancestry(
    document: &FlatSceneDocument,
    id: SceneNodeId,
) -> impl Iterator<Item = &SceneNodeRecord> {
    std::iter::successors(find(document, id), |node| {
        node.parent.and_then(|parent| find(document, parent))
    })
    .take(document.nodes.len())
}

fn world_transform(document: &FlatSceneDocument, id: SceneNodeId) -> Option<SceneTransform> {
    let chain = ancestry(document, id)
        .map(|node| node.transform)
        .collect::<Vec<_>>();
    chain.into_iter().rev().reduce(SceneTransform::compose)
}

/// Writes `candidate` over the node at `index` after checking its own fields
/// and, through `relations`, what the change can break elsewhere. `relations`
/// sees the document with the candidate in place and the previous node, and
/// returns the dependency list to install when node assets changed. A refusal
/// puts the previous node back.
fn replace_node(
    document: &mut FlatSceneDocument,
    index: usize,
    candidate: SceneNodeRecord,
    relations: impl FnOnce(
        &FlatSceneDocument,
        &SceneNodeRecord,
        &mut Vec<SceneValidationError>,
    ) -> Option<Vec<AssetReference>>,
) -> Result<(), SceneEditError> {
    let mut errors = Vec::new();
    validate_node_fields(&candidate, &mut errors);
    let previous = std::mem::replace(&mut document.nodes[index], candidate);
    let dependencies = relations(document, &previous, &mut errors);
    if !errors.is_empty() {
        document.nodes[index] = previous;
        return Err(SceneEditError::InvalidAfter { errors });
    }
    if let Some(dependencies) = dependencies {
        document.dependencies = dependencies;
    }
    Ok(())
}

fn create(
    document: &mut FlatSceneDocument,
    mut record: SceneNodeRecord,
) -> Result<(), SceneEditError> {
    if find(document, record.id).is_some() {
        return Err(SceneEditError::DuplicateObject { id: record.id });
    }
    validate_parent(document, record.id, record.parent)?;
    require_non_blank_label(record.id, record.metadata.label.as_deref())?;
    record.canonicalize();
    let mut errors = Vec::new();
    validate_node_fields(&record, &mut errors);
    validate_kind_relations(document, &record, &mut errors);
    if matches!(record.kind, SceneNodeKind::VoxelVolume(_)) {
        let parent = record
            .parent
            .and_then(|parent| world_transform(document, parent))
            .unwrap_or(SceneTransform::IDENTITY);
        voxel_world_errors(record.id, parent.compose(record.transform), &mut errors);
    }
    let dependencies = record.kind.asset().is_some().then(|| {
        reconciled_dependencies(
            document.nodes.iter().chain(std::iter::once(&record)),
            &mut errors,
        )
    });
    if !errors.is_empty() {
        return Err(SceneEditError::InvalidAfter { errors });
    }
    if let Some(dependencies) = dependencies {
        document.dependencies = dependencies;
    }
    let index = document
        .nodes
        .partition_point(|node| node.id.raw() < record.id.raw());
    document.nodes.insert(index, record);
    Ok(())
}

fn delete(document: &mut FlatSceneDocument, id: SceneNodeId) -> Result<(), SceneEditError> {
    index_of(document, id)?;
    let doomed = subtree(document, id);
    let removed_markers = document
        .nodes
        .iter()
        .filter(|node| doomed.contains(&node.id))
        .filter_map(|node| match &node.kind {
            SceneNodeKind::Marker(marker) => Some(marker.marker_id.as_str()),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let removes_asset = document
        .nodes
        .iter()
        .any(|node| doomed.contains(&node.id) && node.kind.asset().is_some());
    let mut errors = Vec::new();
    let survivors = || {
        document
            .nodes
            .iter()
            .filter(|node| !doomed.contains(&node.id))
    };
    if !removed_markers.is_empty() {
        for node in survivors() {
            spawn_reference(node, &removed_markers, &mut errors);
        }
    }
    let dependencies = removes_asset.then(|| reconciled_dependencies(survivors(), &mut errors));
    if !errors.is_empty() {
        return Err(SceneEditError::InvalidAfter { errors });
    }
    document.nodes.retain(|node| !doomed.contains(&node.id));
    if let Some(dependencies) = dependencies {
        document.dependencies = dependencies;
    }
    Ok(())
}

/// The node and every descendant, found through one child lookup.
fn subtree(document: &FlatSceneDocument, root: SceneNodeId) -> BTreeSet<SceneNodeId> {
    let mut children = BTreeMap::<SceneNodeId, Vec<SceneNodeId>>::new();
    for node in &document.nodes {
        if let Some(parent) = node.parent {
            children.entry(parent).or_default().push(node.id);
        }
    }
    let mut doomed = BTreeSet::new();
    let mut pending = vec![root];
    while let Some(id) = pending.pop() {
        if doomed.insert(id) {
            pending.extend(children.get(&id).into_iter().flatten().copied());
        }
    }
    doomed
}

/// The path of the parent cycle `id -> parent -> ... -> id` that setting
/// `parent` closes, if it closes one.
fn cycle_through(
    document: &FlatSceneDocument,
    id: SceneNodeId,
    parent: SceneNodeId,
) -> Option<Vec<SceneNodeId>> {
    let mut path = vec![id];
    for node in ancestry(document, parent) {
        if node.id == id {
            return Some(path);
        }
        path.push(node.id);
    }
    None
}

/// Composed placement of every voxel volume at or below `root`.
fn voxel_errors_below(
    document: &FlatSceneDocument,
    root: SceneNodeId,
    errors: &mut Vec<SceneValidationError>,
) {
    for node in &document.nodes {
        if matches!(node.kind, SceneNodeKind::VoxelVolume(_))
            && ancestry(document, node.id).any(|ancestor| ancestor.id == root)
        {
            if let Some(world) = world_transform(document, node.id) {
                voxel_world_errors(node.id, world, errors);
            }
        }
    }
}

/// Identities `node`'s kind must keep unique, and the spawn marker it names.
fn validate_kind_relations(
    document: &FlatSceneDocument,
    node: &SceneNodeRecord,
    errors: &mut Vec<SceneValidationError>,
) {
    let others = || document.nodes.iter().filter(|other| other.id != node.id);
    match &node.kind {
        SceneNodeKind::Marker(marker) => {
            if others().any(|other| {
                matches!(&other.kind, SceneNodeKind::Marker(existing)
                    if existing.marker_id == marker.marker_id)
            }) {
                errors.push(SceneValidationError::DuplicateMarkerId {
                    node: node.id,
                    marker_id: marker.marker_id.clone(),
                });
            }
        }
        SceneNodeKind::EntityInstance(instance) => {
            if others().any(|other| {
                matches!(&other.kind, SceneNodeKind::EntityInstance(existing)
                    if existing.instance_id == instance.instance_id)
            }) {
                errors.push(SceneValidationError::DuplicateEntityInstanceId {
                    node: node.id,
                    instance_id: instance.instance_id.clone(),
                });
            }
            if let Some(marker_id) = instance.spawn_marker_id.as_deref() {
                if valid_stable_id(marker_id)
                    && !others().any(|other| {
                        matches!(&other.kind, SceneNodeKind::Marker(marker)
                            if marker.marker_id == marker_id)
                    })
                {
                    errors.push(SceneValidationError::UnknownSpawnMarker {
                        node: node.id,
                        marker_id: marker_id.to_string(),
                    });
                }
            }
        }
        SceneNodeKind::Bootstrap(_) => {
            if others().any(|other| matches!(other.kind, SceneNodeKind::Bootstrap(_))) {
                errors.push(SceneValidationError::DuplicateBootstrapNode { node: node.id });
            }
        }
        SceneNodeKind::EmptyGroup
        | SceneNodeKind::StaticMesh(_)
        | SceneNodeKind::AnimatedMesh(_)
        | SceneNodeKind::Sprite(_)
        | SceneNodeKind::VoxelVolume(_)
        | SceneNodeKind::Light(_) => {}
    }
}

/// Entity instances that would still spawn at one of `removed` markers.
fn spawn_references(
    document: &FlatSceneDocument,
    removed: &BTreeSet<&str>,
    errors: &mut Vec<SceneValidationError>,
) {
    for node in &document.nodes {
        spawn_reference(node, removed, errors);
    }
}

fn spawn_reference(
    node: &SceneNodeRecord,
    removed: &BTreeSet<&str>,
    errors: &mut Vec<SceneValidationError>,
) {
    if let SceneNodeKind::EntityInstance(instance) = &node.kind {
        if let Some(marker_id) = instance
            .spawn_marker_id
            .as_deref()
            .filter(|marker_id| removed.contains(marker_id))
        {
            errors.push(SceneValidationError::UnknownSpawnMarker {
                node: node.id,
                marker_id: marker_id.to_string(),
            });
        }
    }
}

fn validate_parent(
    document: &FlatSceneDocument,
    id: SceneNodeId,
    parent: Option<SceneNodeId>,
) -> Result<(), SceneEditError> {
    if parent == Some(id) {
        return Err(SceneEditError::SelfParent { id });
    }
    if let Some(parent) = parent {
        if find(document, parent).is_none() {
            return Err(SceneEditError::MissingParent { id, parent });
        }
    }
    Ok(())
}

fn require_non_blank_label(id: SceneNodeId, label: Option<&str>) -> Result<(), SceneEditError> {
    if label.is_some_and(|label| label.trim().is_empty()) {
        return Err(SceneEditError::BlankLabel { id });
    }
    Ok(())
}

/// The dependency list derived from node assets, one reference per asset id.
/// A node naming an id another node already pins differently has no declared
/// dependency.
fn reconciled_dependencies<'a>(
    nodes: impl Iterator<Item = &'a SceneNodeRecord>,
    errors: &mut Vec<SceneValidationError>,
) -> Vec<AssetReference> {
    let mut references = BTreeMap::<&str, &AssetReference>::new();
    for node in nodes {
        let Some(asset) = node.kind.asset() else {
            continue;
        };
        match references.entry(asset.id().as_str()) {
            Entry::Vacant(entry) => {
                entry.insert(asset);
            }
            Entry::Occupied(entry) if *entry.get() != asset => {
                errors.push(missing_dependency(node.id, asset));
            }
            Entry::Occupied(_) => {}
        }
    }
    references.into_values().cloned().collect()
}
