use std::collections::{BTreeMap, BTreeSet};

use core_assets::{AssetKind, AssetReference, AssetVersionReq};
use core_ids::{PrefabId, SceneId, SceneNodeId};

use crate::{SceneLight, SceneTransform};

pub const CURRENT_SCENE_SCHEMA_VERSION: u32 = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SceneEntityReference {
    EntityDefinition {
        stable_id: String,
    },
    Prefab {
        prefab_id: PrefabId,
        variant_id: Option<String>,
        instantiation_seed: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneMarker {
    pub marker_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneEntityInstance {
    pub instance_id: String,
    pub reference: SceneEntityReference,
    pub spawn_marker_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneGeneratorBinding {
    pub provider_id: String,
    pub preset_id: String,
    pub seed: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneCatalogBinding {
    pub binding_id: String,
    pub catalog_id: String,
    pub source_path: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SceneBootstrapBindings {
    pub generator: Option<SceneGeneratorBinding>,
    pub catalogs: Vec<SceneCatalogBinding>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SceneNodeKind {
    EmptyGroup,
    StaticMesh(AssetReference),
    AnimatedMesh(AssetReference),
    Sprite(AssetReference),
    VoxelVolume(AssetReference),
    Light(SceneLight),
    Marker(SceneMarker),
    EntityInstance(SceneEntityInstance),
    Bootstrap(SceneBootstrapBindings),
}

impl SceneNodeKind {
    pub const fn expected_asset_kind(&self) -> Option<AssetKind> {
        match self {
            Self::StaticMesh(_) => Some(AssetKind::StaticMesh),
            Self::AnimatedMesh(_) => Some(AssetKind::AnimatedMesh),
            Self::Sprite(_) => Some(AssetKind::Sprite),
            Self::VoxelVolume(_) => Some(AssetKind::VoxelVolume),
            Self::EmptyGroup
            | Self::Light(_)
            | Self::Marker(_)
            | Self::EntityInstance(_)
            | Self::Bootstrap(_) => None,
        }
    }

    pub const fn asset(&self) -> Option<&AssetReference> {
        match self {
            Self::StaticMesh(asset)
            | Self::AnimatedMesh(asset)
            | Self::Sprite(asset)
            | Self::VoxelVolume(asset) => Some(asset),
            Self::EmptyGroup
            | Self::Light(_)
            | Self::Marker(_)
            | Self::EntityInstance(_)
            | Self::Bootstrap(_) => None,
        }
    }

    pub const fn tag(&self) -> &'static str {
        match self {
            Self::EmptyGroup => "emptyGroup",
            Self::StaticMesh(_) => "staticMesh",
            Self::AnimatedMesh(_) => "animatedMesh",
            Self::Sprite(_) => "sprite",
            Self::VoxelVolume(_) => "voxelVolume",
            Self::Light(_) => "light",
            Self::Marker(_) => "marker",
            Self::EntityInstance(_) => "entityInstance",
            Self::Bootstrap(_) => "bootstrap",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NodeMetadata {
    pub label: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SceneMetadata {
    pub name: Option<String>,
    pub authoring_format_version: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SceneNode {
    pub id: SceneNodeId,
    pub transform: SceneTransform,
    pub kind: SceneNodeKind,
    pub metadata: NodeMetadata,
    pub children: Vec<SceneNode>,
}

impl SceneNode {
    pub fn leaf(id: SceneNodeId, kind: SceneNodeKind) -> Self {
        Self {
            id,
            transform: SceneTransform::IDENTITY,
            kind,
            metadata: NodeMetadata::default(),
            children: Vec::new(),
        }
    }

    pub fn with_children(mut self, children: Vec<SceneNode>) -> Self {
        self.children = children;
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SceneTree {
    pub id: SceneId,
    pub revision: u64,
    pub schema_version: u32,
    pub metadata: SceneMetadata,
    pub dependencies: Vec<AssetReference>,
    pub roots: Vec<SceneNode>,
}

impl SceneTree {
    pub fn to_flat(&self) -> FlatSceneDocument {
        let mut nodes = Vec::new();
        for (order, root) in self.roots.iter().enumerate() {
            flatten_into(&mut nodes, root, None, order as u32);
        }
        let mut document = FlatSceneDocument {
            id: self.id,
            revision: self.revision,
            schema_version: self.schema_version,
            metadata: self.metadata.clone(),
            dependencies: self.dependencies.clone(),
            nodes,
        };
        document.canonicalize();
        document
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SceneNodeRecord {
    pub id: SceneNodeId,
    pub parent: Option<SceneNodeId>,
    pub child_order: u32,
    pub transform: SceneTransform,
    pub kind: SceneNodeKind,
    pub metadata: NodeMetadata,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FlatSceneDocument {
    pub id: SceneId,
    pub revision: u64,
    pub schema_version: u32,
    pub metadata: SceneMetadata,
    pub dependencies: Vec<AssetReference>,
    pub nodes: Vec<SceneNodeRecord>,
}

impl FlatSceneDocument {
    pub fn new(id: SceneId) -> Self {
        Self {
            id,
            revision: 0,
            schema_version: CURRENT_SCENE_SCHEMA_VERSION,
            metadata: SceneMetadata {
                name: None,
                authoring_format_version: CURRENT_SCENE_SCHEMA_VERSION,
            },
            dependencies: Vec::new(),
            nodes: Vec::new(),
        }
    }

    pub fn canonicalize(&mut self) {
        self.nodes.sort_by_key(|node| node.id.raw());
        self.dependencies.sort_by(|left, right| {
            left.id()
                .as_str()
                .cmp(right.id().as_str())
                .then_with(|| version_key(left.version()).cmp(&version_key(right.version())))
                .then_with(|| {
                    left.hash()
                        .map(|hash| hash.as_str())
                        .cmp(&right.hash().map(|hash| hash.as_str()))
                })
        });
        for node in &mut self.nodes {
            node.metadata.tags.sort();
            node.metadata.tags.dedup();
            if let SceneNodeKind::Bootstrap(bindings) = &mut node.kind {
                bindings.catalogs.sort_by(|left, right| {
                    left.binding_id
                        .cmp(&right.binding_id)
                        .then_with(|| left.catalog_id.cmp(&right.catalog_id))
                        .then_with(|| left.source_path.cmp(&right.source_path))
                });
            }
        }
    }

    pub fn canonical(&self) -> Self {
        let mut document = self.clone();
        document.canonicalize();
        document
    }

    pub fn to_tree(&self) -> Option<SceneTree> {
        let known: BTreeSet<_> = self.nodes.iter().map(|node| node.id).collect();
        if known.len() != self.nodes.len()
            || self
                .nodes
                .iter()
                .any(|node| node.parent.is_some_and(|parent| !known.contains(&parent)))
        {
            return None;
        }
        let mut children: BTreeMap<Option<SceneNodeId>, Vec<usize>> = BTreeMap::new();
        for (index, node) in self.nodes.iter().enumerate() {
            children.entry(node.parent).or_default().push(index);
        }
        let mut visiting = BTreeSet::new();
        let roots = self.build_children(None, &children, &mut visiting)?;
        if count_nodes(&roots) != self.nodes.len() {
            return None;
        }
        Some(SceneTree {
            id: self.id,
            revision: self.revision,
            schema_version: self.schema_version,
            metadata: self.metadata.clone(),
            dependencies: self.dependencies.clone(),
            roots,
        })
    }

    fn build_children(
        &self,
        parent: Option<SceneNodeId>,
        children: &BTreeMap<Option<SceneNodeId>, Vec<usize>>,
        visiting: &mut BTreeSet<SceneNodeId>,
    ) -> Option<Vec<SceneNode>> {
        let Some(indices) = children.get(&parent) else {
            return Some(Vec::new());
        };
        let mut ordered = indices.clone();
        ordered.sort_by_key(|index| {
            let node = &self.nodes[*index];
            (node.child_order, node.id)
        });
        let mut nodes = Vec::with_capacity(ordered.len());
        for index in ordered {
            let record = &self.nodes[index];
            if !visiting.insert(record.id) {
                return None;
            }
            let descendants = self.build_children(Some(record.id), children, visiting)?;
            visiting.remove(&record.id);
            nodes.push(SceneNode {
                id: record.id,
                transform: record.transform,
                kind: record.kind.clone(),
                metadata: record.metadata.clone(),
                children: descendants,
            });
        }
        Some(nodes)
    }
}

fn flatten_into(
    output: &mut Vec<SceneNodeRecord>,
    node: &SceneNode,
    parent: Option<SceneNodeId>,
    child_order: u32,
) {
    output.push(SceneNodeRecord {
        id: node.id,
        parent,
        child_order,
        transform: node.transform,
        kind: node.kind.clone(),
        metadata: node.metadata.clone(),
    });
    for (order, child) in node.children.iter().enumerate() {
        flatten_into(output, child, Some(node.id), order as u32);
    }
}

fn count_nodes(nodes: &[SceneNode]) -> usize {
    nodes
        .iter()
        .map(|node| 1 + count_nodes(&node.children))
        .sum()
}

fn version_key(requirement: AssetVersionReq) -> (u8, u32) {
    match requirement {
        AssetVersionReq::Any => (0, 0),
        AssetVersionReq::Exact(version) => (1, version),
        AssetVersionReq::AtLeast(version) => (2, version),
    }
}
