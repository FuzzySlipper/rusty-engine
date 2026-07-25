use std::collections::BTreeSet;

use core_assets::AssetReference;
use core_ids::SceneNodeId;

use crate::{
    validate_scene, FlatSceneDocument, SceneLight, SceneNodeKind, SceneNodeRecord, SceneTransform,
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
    pub fn from_document(document: &FlatSceneDocument) -> Self {
        let canonical = document.canonical();
        Self {
            scene_revision: canonical.revision,
            objects: canonical
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
                .collect(),
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
    pub snapshot: SceneObjectSnapshot,
    pub selected: Option<SceneNodeId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SceneEditError {
    StaleRevision {
        expected: u64,
        actual: u64,
    },
    RevisionOverflow,
    InvalidBefore {
        errors: Vec<crate::SceneValidationError>,
    },
    InvalidAfter {
        errors: Vec<crate::SceneValidationError>,
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
            Self::InvalidBefore { .. } => "invalid-scene-before-edit",
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
        let before = validate_scene(document);
        if !before.is_valid() {
            return Err(SceneEditError::InvalidBefore {
                errors: before.errors,
            });
        }
        if let SceneEditCommand::Select { id } = &command {
            if id.is_some_and(|id| !contains_node(document, id)) {
                return Err(SceneEditError::MissingObject { id: id.unwrap() });
            }
            return Ok(SceneEditReceipt {
                revision_before: document.revision,
                revision_after: document.revision,
                snapshot: SceneObjectSnapshot::from_document(document),
                selected: *id,
            });
        }

        let revision_before = document.revision;
        let mut next = document.canonical();
        let mut reconcile_assets = false;
        let selected = match &command {
            SceneEditCommand::Create { record } => {
                if contains_node(&next, record.id) {
                    return Err(SceneEditError::DuplicateObject { id: record.id });
                }
                validate_parent(&next, record.id, record.parent)?;
                require_non_blank_label(record.id, record.metadata.label.as_deref())?;
                upgrade_schema_for_kind(&mut next, &record.kind);
                next.nodes.push(record.clone());
                reconcile_assets = true;
                None
            }
            SceneEditCommand::Delete { id } => {
                if !contains_node(&next, *id) {
                    return Err(SceneEditError::MissingObject { id: *id });
                }
                delete_subtree(&mut next, *id);
                reconcile_assets = true;
                None
            }
            SceneEditCommand::Rename { id, label } => {
                require_non_blank_label(*id, label.as_deref())?;
                find_node_mut(&mut next, *id)
                    .ok_or(SceneEditError::MissingObject { id: *id })?
                    .metadata
                    .label = label.clone();
                Some(*id)
            }
            SceneEditCommand::Reparent {
                id,
                parent,
                child_order,
            } => {
                validate_parent(&next, *id, *parent)?;
                let node = find_node_mut(&mut next, *id)
                    .ok_or(SceneEditError::MissingObject { id: *id })?;
                node.parent = *parent;
                node.child_order = *child_order;
                Some(*id)
            }
            SceneEditCommand::UpdateLight { id, light } => {
                let node = find_node_mut(&mut next, *id)
                    .ok_or(SceneEditError::MissingObject { id: *id })?;
                if !matches!(node.kind, SceneNodeKind::Light(_)) {
                    return Err(SceneEditError::WrongObjectKind { id: *id });
                }
                node.kind = SceneNodeKind::Light(light.clone());
                upgrade_schema(&mut next, 2);
                Some(*id)
            }
            SceneEditCommand::SetTransform { id, transform } => {
                find_node_mut(&mut next, *id)
                    .ok_or(SceneEditError::MissingObject { id: *id })?
                    .transform = *transform;
                Some(*id)
            }
            SceneEditCommand::SetKind { id, kind } => {
                let node = find_node_mut(&mut next, *id)
                    .ok_or(SceneEditError::MissingObject { id: *id })?;
                node.kind = kind.clone();
                upgrade_schema_for_kind(&mut next, kind);
                reconcile_assets = true;
                Some(*id)
            }
            SceneEditCommand::RetargetVoxelAsset { id, asset, tags } => {
                let node = find_node_mut(&mut next, *id)
                    .ok_or(SceneEditError::MissingObject { id: *id })?;
                if !matches!(node.kind, SceneNodeKind::VoxelVolume(_)) {
                    return Err(SceneEditError::WrongObjectKind { id: *id });
                }
                node.kind = SceneNodeKind::VoxelVolume(asset.clone());
                node.metadata.tags = tags.clone();
                reconcile_assets = true;
                Some(*id)
            }
            SceneEditCommand::Select { .. } => unreachable!("selection returned above"),
        };

        if reconcile_assets {
            reconcile_dependencies(&mut next);
        }
        next.revision = next
            .revision
            .checked_add(1)
            .ok_or(SceneEditError::RevisionOverflow)?;
        next.canonicalize();
        let after = validate_scene(&next);
        if !after.is_valid() {
            return Err(SceneEditError::InvalidAfter {
                errors: after.errors,
            });
        }
        let receipt = SceneEditReceipt {
            revision_before,
            revision_after: next.revision,
            snapshot: SceneObjectSnapshot::from_document(&next),
            selected,
        };
        *document = next;
        Ok(receipt)
    }
}

fn contains_node(document: &FlatSceneDocument, id: SceneNodeId) -> bool {
    document.nodes.iter().any(|node| node.id == id)
}

fn find_node_mut(
    document: &mut FlatSceneDocument,
    id: SceneNodeId,
) -> Option<&mut SceneNodeRecord> {
    document.nodes.iter_mut().find(|node| node.id == id)
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
        if !contains_node(document, parent) {
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

fn delete_subtree(document: &mut FlatSceneDocument, root: SceneNodeId) {
    let mut doomed = BTreeSet::from([root]);
    loop {
        let before = doomed.len();
        for node in &document.nodes {
            if node.parent.is_some_and(|parent| doomed.contains(&parent)) {
                doomed.insert(node.id);
            }
        }
        if doomed.len() == before {
            break;
        }
    }
    document.nodes.retain(|node| !doomed.contains(&node.id));
}

fn reconcile_dependencies(document: &mut FlatSceneDocument) {
    let mut references = document
        .nodes
        .iter()
        .filter_map(|node| node.kind.asset().cloned())
        .collect::<Vec<_>>();
    references.sort_by(|left, right| left.id().as_str().cmp(right.id().as_str()));
    references.dedup_by(|left, right| left.id() == right.id());
    document.dependencies = references;
}

fn upgrade_schema_for_kind(document: &mut FlatSceneDocument, kind: &SceneNodeKind) {
    let version = match kind {
        SceneNodeKind::Light(_) => 2,
        SceneNodeKind::EntityInstance(crate::SceneEntityInstance {
            reference: crate::SceneEntityReference::Prefab { .. },
            ..
        })
        | SceneNodeKind::Marker(_) => 4,
        SceneNodeKind::EntityInstance(_) | SceneNodeKind::Bootstrap(_) => 3,
        SceneNodeKind::EmptyGroup
        | SceneNodeKind::StaticMesh(_)
        | SceneNodeKind::Sprite(_)
        | SceneNodeKind::VoxelVolume(_) => 1,
    };
    upgrade_schema(document, version);
}

fn upgrade_schema(document: &mut FlatSceneDocument, version: u32) {
    document.schema_version = document.schema_version.max(version);
    document.metadata.authoring_format_version =
        document.metadata.authoring_format_version.max(version);
}
