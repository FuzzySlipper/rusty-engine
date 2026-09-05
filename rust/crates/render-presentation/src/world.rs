//! Canonical, host-neutral retained graphics intent. Projectors produce typed
//! changes into this owner; transports and renderer attachments read snapshots.
//! No browser objects, product callbacks, or incremental history are retained.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use render_model::*;

pub const PRESENTATION_WORLD_STREAM: &str = "presentation-world";

#[derive(Debug, Clone, PartialEq)]
enum NodeKind {
    Primitive(RenderNode),
    StaticMesh(StaticMeshInstanceDescriptor),
    AnimatedMesh(AnimatedMeshInstanceDescriptor),
    VoxelObject(VoxelObjectInstanceDescriptor),
    Sprite(SpriteInstanceDescriptor),
    Light(LightDescriptor),
}

#[derive(Debug, Clone, PartialEq)]
struct PresentationNode {
    parent: Option<RenderHandle>,
    kind: NodeKind,
    mesh_payload: Option<MeshPayloadDescriptor>,
    material_override: Option<Material>,
    material_parameters: BTreeMap<u16, MaterialInstanceParameters>,
}

/// A complete graphics baseline and the exact continuation point it represents.
/// Runtime binding and attachment epochs are owned by the enclosing session.
#[derive(Debug, Clone, PartialEq)]
pub struct PresentationSnapshot {
    pub revision: u64,
    pub frame: RenderFrameDiff,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PresentationWorld {
    revision: u64,
    nodes: BTreeMap<RenderHandle, Arc<PresentationNode>>,
    textures: BTreeMap<String, Arc<TextureDescriptor>>,
    materials: BTreeMap<String, Arc<RenderMaterialDescriptor>>,
    atlases: BTreeMap<String, Arc<SpriteAtlasDescriptor>>,
    static_meshes: BTreeMap<String, Arc<StaticMeshAsset>>,
    animated_meshes: BTreeMap<String, Arc<AnimatedMeshAsset>>,
    voxel_objects: BTreeMap<String, Arc<VoxelObjectRenderAsset>>,
    sky: Option<SkyBackgroundDescriptor>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PresentationWorldError {
    Frame(RenderFrameError),
    RevisionExhausted,
    UnknownNode(RenderHandle),
    DuplicateNode(RenderHandle),
    WrongNodeKind(RenderHandle),
    ReferencedResource(String),
}

impl std::fmt::Display for PresentationWorldError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for PresentationWorldError {}

impl PresentationWorld {
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Admit a complete named graphics change atomically. Upstream projector
    /// frontiers are internal: all consumers continue this world's revision.
    /// Call owners may stage a clone and install it with their other state.
    pub fn apply(
        &mut self,
        frame: &RenderFrameDiff,
    ) -> Result<RenderFrameDiff, PresentationWorldError> {
        frame.validate().map_err(PresentationWorldError::Frame)?;
        if frame.ops.is_empty() {
            return Ok(RenderFrameDiff::new());
        }
        let revision = self
            .revision
            .checked_add(1)
            .filter(|revision| *revision <= JSON_SAFE_U64_MAX)
            .ok_or(PresentationWorldError::RevisionExhausted)?;
        let mut candidate = self.clone();
        let mut operations = Vec::with_capacity(frame.ops.len());
        for op in &frame.ops {
            // Named services may select the same immutable texture repeatedly
            // (for example, the active sky). Retain that fact once rather than
            // publishing a stale texture-version update to the realization.
            if matches!(op, RenderDiff::DefineTexture { texture }
                if candidate.textures.get(&texture.id).is_some_and(|current| current.as_ref() == texture))
                || matches!(op, RenderDiff::SetSkyBackground { background } if &candidate.sky == background)
            {
                continue;
            }
            candidate.apply_operation(op)?;
            operations.push(op.clone());
        }
        if operations.is_empty() {
            return Ok(RenderFrameDiff::new());
        }
        let delta = RenderFrameDiff::try_from_published_ops(
            PRESENTATION_WORLD_STREAM,
            self.revision,
            revision,
            operations,
        )
        .map_err(PresentationWorldError::Frame)?;
        candidate.revision = revision;
        *self = candidate;
        Ok(delta)
    }

    /// Resource definitions precede uses; parents precede children. Handles
    /// retain their active identity, even after many creates and destroys.
    pub fn snapshot(&self) -> PresentationSnapshot {
        let mut ops = Vec::new();
        ops.extend(
            self.textures
                .values()
                .map(|value| value.as_ref().clone())
                .map(|texture| RenderDiff::DefineTexture { texture }),
        );
        ops.extend(
            self.materials
                .values()
                .map(|value| value.as_ref().clone())
                .map(|material| RenderDiff::DefineMaterial { material }),
        );
        ops.extend(
            self.atlases
                .values()
                .map(|value| value.as_ref().clone())
                .map(|atlas| RenderDiff::DefineSpriteAtlas { atlas }),
        );
        ops.extend(
            self.static_meshes
                .values()
                .map(|value| value.as_ref().clone())
                .map(|asset| RenderDiff::DefineStaticMesh { asset }),
        );
        ops.extend(
            self.animated_meshes
                .values()
                .map(|value| value.as_ref().clone())
                .map(|asset| RenderDiff::DefineAnimatedMesh { asset }),
        );
        ops.extend(
            self.voxel_objects
                .values()
                .map(|value| value.as_ref().clone())
                .map(|asset| RenderDiff::DefineVoxelObject { asset }),
        );
        ops.push(RenderDiff::SetSkyBackground {
            background: self.sky.clone(),
        });
        // Creation requires an existing parent and parents cannot be changed,
        // so this traversal is acyclic by construction.
        let mut emitted = BTreeSet::new();
        for handle in self.nodes.keys() {
            self.snapshot_node(*handle, &mut emitted, &mut ops);
        }
        PresentationSnapshot {
            revision: self.revision,
            frame: RenderFrameDiff {
                ops,
                ..RenderFrameDiff::new()
            },
        }
    }

    fn snapshot_node(
        &self,
        handle: RenderHandle,
        emitted: &mut BTreeSet<RenderHandle>,
        ops: &mut Vec<RenderDiff>,
    ) {
        if emitted.contains(&handle) {
            return;
        }
        let node = &self.nodes[&handle];
        if let Some(parent) = node.parent {
            self.snapshot_node(parent, emitted, ops);
        }
        let parent = node.parent;
        ops.push(match &node.kind {
            NodeKind::Primitive(node) => RenderDiff::Create {
                handle,
                parent,
                node: node.clone(),
            },
            NodeKind::StaticMesh(instance) => RenderDiff::CreateStaticMeshInstance {
                handle,
                parent,
                instance: instance.clone(),
            },
            NodeKind::AnimatedMesh(instance) => RenderDiff::CreateAnimatedMeshInstance {
                handle,
                parent,
                instance: instance.clone(),
            },
            NodeKind::VoxelObject(instance) => RenderDiff::CreateVoxelObjectInstance {
                handle,
                parent,
                instance: instance.clone(),
            },
            NodeKind::Sprite(sprite) => RenderDiff::CreateSprite {
                handle,
                parent,
                sprite: sprite.clone(),
            },
            NodeKind::Light(light) => RenderDiff::CreateLight {
                handle,
                parent,
                light: light.clone(),
            },
        });
        if let Some(material) = node.material_override {
            ops.push(RenderDiff::Update {
                handle,
                material: Some(material),
                transform: None,
                visible: None,
                metadata: None,
            });
        }
        if let Some(payload) = &node.mesh_payload {
            ops.push(RenderDiff::ReplaceMeshPayload {
                handle,
                payload: payload.clone(),
            });
        }
        for (slot, parameters) in &node.material_parameters {
            ops.push(RenderDiff::SetMaterialInstanceParameters {
                handle,
                slot: *slot,
                parameters: Some(*parameters),
            });
        }
        emitted.insert(handle);
    }

    fn insert(
        &mut self,
        handle: RenderHandle,
        parent: Option<RenderHandle>,
        kind: NodeKind,
    ) -> Result<(), PresentationWorldError> {
        if self.nodes.contains_key(&handle) {
            return Err(PresentationWorldError::DuplicateNode(handle));
        }
        if let Some(parent) = parent {
            if !self.nodes.contains_key(&parent) {
                return Err(PresentationWorldError::UnknownNode(parent));
            }
        }
        self.nodes.insert(
            handle,
            Arc::new(PresentationNode {
                parent,
                kind,
                mesh_payload: None,
                material_override: None,
                material_parameters: BTreeMap::new(),
            }),
        );
        Ok(())
    }

    fn node_mut(
        &mut self,
        handle: RenderHandle,
    ) -> Result<&mut PresentationNode, PresentationWorldError> {
        self.nodes
            .get_mut(&handle)
            .map(Arc::make_mut)
            .ok_or(PresentationWorldError::UnknownNode(handle))
    }

    fn apply_operation(&mut self, op: &RenderDiff) -> Result<(), PresentationWorldError> {
        match op {
            RenderDiff::Create {
                handle,
                parent,
                node,
            } => self.insert(*handle, *parent, NodeKind::Primitive(node.clone()))?,
            RenderDiff::CreateStaticMeshInstance {
                handle,
                parent,
                instance,
            } => self.insert(*handle, *parent, NodeKind::StaticMesh(instance.clone()))?,
            RenderDiff::CreateAnimatedMeshInstance {
                handle,
                parent,
                instance,
            } => self.insert(*handle, *parent, NodeKind::AnimatedMesh(instance.clone()))?,
            RenderDiff::CreateVoxelObjectInstance {
                handle,
                parent,
                instance,
            } => self.insert(*handle, *parent, NodeKind::VoxelObject(instance.clone()))?,
            RenderDiff::CreateSprite {
                handle,
                parent,
                sprite,
            } => self.insert(*handle, *parent, NodeKind::Sprite(sprite.clone()))?,
            RenderDiff::CreateLight {
                handle,
                parent,
                light,
            } => self.insert(*handle, *parent, NodeKind::Light(light.clone()))?,
            RenderDiff::Destroy { handle } => {
                if !self.nodes.contains_key(handle) {
                    return Err(PresentationWorldError::UnknownNode(*handle));
                }
                let mut removed = BTreeSet::from([*handle]);
                loop {
                    let before = removed.len();
                    for (handle, node) in &self.nodes {
                        if node.parent.is_some_and(|parent| removed.contains(&parent)) {
                            removed.insert(*handle);
                        }
                    }
                    if before == removed.len() {
                        break;
                    }
                }
                self.nodes.retain(|handle, _| !removed.contains(handle));
            }
            RenderDiff::Update {
                handle,
                transform,
                material,
                visible,
                metadata,
            } => {
                let node = self.node_mut(*handle)?;
                if material.is_some() {
                    node.material_override = *material;
                }
                macro_rules! update {
                    ($value:expr) => {{
                        if let Some(transform) = transform {
                            $value.transform = *transform;
                        }
                        if let Some(visible) = visible {
                            $value.visible = *visible;
                        }
                        if let Some(metadata) = metadata {
                            $value.metadata = metadata.clone();
                        }
                    }};
                }
                match &mut node.kind {
                    NodeKind::Primitive(value) => {
                        update!(value);
                        if let Some(material) = material {
                            value.material = *material;
                        }
                    }
                    NodeKind::StaticMesh(value) => {
                        update!(value);
                    }
                    NodeKind::AnimatedMesh(value) => {
                        update!(value);
                    }
                    NodeKind::VoxelObject(value) => {
                        update!(value);
                    }
                    NodeKind::Sprite(value) => {
                        update!(value);
                    }
                    NodeKind::Light(_) => {
                        return Err(PresentationWorldError::WrongNodeKind(*handle))
                    }
                }
            }
            RenderDiff::ReplaceMeshPayload { handle, payload } => {
                let node = self.node_mut(*handle)?;
                if !matches!(&node.kind, NodeKind::Primitive(node) if node.geometry != Geometry::Group)
                {
                    return Err(PresentationWorldError::WrongNodeKind(*handle));
                }
                node.mesh_payload = Some(payload.clone());
            }
            RenderDiff::UpdateLight { handle, light } => match &mut self.node_mut(*handle)?.kind {
                NodeKind::Light(value) => *value = light.clone(),
                _ => return Err(PresentationWorldError::WrongNodeKind(*handle)),
            },
            RenderDiff::SetMaterialInstanceParameters {
                handle,
                slot,
                parameters,
            } => {
                let node = self.node_mut(*handle)?;
                if let Some(parameters) = parameters {
                    node.material_parameters.insert(*slot, *parameters);
                } else {
                    node.material_parameters.remove(slot);
                }
            }
            RenderDiff::SetAnimatedMeshPlayback { handle, playback } => {
                match &mut self.node_mut(*handle)?.kind {
                    NodeKind::AnimatedMesh(value) => value.playback = Some(playback.clone()),
                    _ => return Err(PresentationWorldError::WrongNodeKind(*handle)),
                }
            }
            RenderDiff::SetVoxelObjectFrame { handle, frame } => {
                match &mut self.node_mut(*handle)?.kind {
                    NodeKind::VoxelObject(value) => value.frame = *frame,
                    _ => return Err(PresentationWorldError::WrongNodeKind(*handle)),
                }
            }
            RenderDiff::UpdateSprite {
                handle,
                frame,
                tint,
                render_order,
                visible,
            } => match &mut self.node_mut(*handle)?.kind {
                NodeKind::Sprite(value) => {
                    if let Some(frame) = frame {
                        value.frame = *frame;
                    }
                    if let Some(tint) = tint {
                        value.tint = *tint;
                    }
                    if let Some(render_order) = render_order {
                        value.render_order = *render_order;
                    }
                    if let Some(visible) = visible {
                        value.visible = *visible;
                    }
                }
                _ => return Err(PresentationWorldError::WrongNodeKind(*handle)),
            },
            RenderDiff::DefineTexture { texture } => {
                self.textures
                    .insert(texture.id.clone(), Arc::new(texture.clone()));
            }
            RenderDiff::DefineMaterial { material } => {
                self.materials
                    .insert(material.id.clone(), Arc::new(material.clone()));
            }
            RenderDiff::DefineSpriteAtlas { atlas } => {
                self.atlases
                    .insert(atlas.id.clone(), Arc::new(atlas.clone()));
            }
            RenderDiff::DefineStaticMesh { asset } => {
                self.static_meshes
                    .insert(asset.asset.clone(), Arc::new(asset.clone()));
            }
            RenderDiff::DefineAnimatedMesh { asset } => {
                self.animated_meshes
                    .insert(asset.asset.clone(), Arc::new(asset.clone()));
            }
            RenderDiff::DefineVoxelObject { asset } => {
                self.voxel_objects
                    .insert(asset.asset.clone(), Arc::new(asset.clone()));
            }
            RenderDiff::ReleaseVoxelObject { asset } => {
                if self.nodes.values().any(|node| matches!(&node.kind, NodeKind::VoxelObject(instance) if &instance.asset == asset)) {
                    return Err(PresentationWorldError::ReferencedResource(asset.clone()));
                }
                self.voxel_objects.remove(asset);
            }
            RenderDiff::SetSkyBackground { background } => self.sky = background.clone(),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(ops: Vec<RenderDiff>) -> RenderFrameDiff {
        RenderFrameDiff::try_from_ops(ops).unwrap()
    }

    #[test]
    fn baseline_keeps_current_identity_and_hierarchy_after_churn() {
        let mut world = PresentationWorld::default();
        let parent = RenderHandle::new(90);
        let child = RenderHandle::new(4);
        world
            .apply(&frame(vec![
                RenderDiff::Create {
                    handle: parent,
                    parent: None,
                    node: RenderNode::new(Geometry::Group),
                },
                RenderDiff::Create {
                    handle: child,
                    parent: Some(parent),
                    node: RenderNode::new(Geometry::Cube),
                },
            ]))
            .unwrap();
        let transform = Transform {
            translation: [3.0, 2.0, 1.0],
            ..Transform::IDENTITY
        };
        world
            .apply(&frame(vec![RenderDiff::Update {
                handle: child,
                transform: Some(transform),
                visible: Some(false),
                material: None,
                metadata: None,
            }]))
            .unwrap();
        let before = world.clone();
        let snapshot = world.snapshot();
        assert_eq!(world, before, "snapshot cannot advance active state");
        assert_eq!(snapshot.revision, 2);
        assert!(snapshot.frame.publication.is_none());
        assert!(
            matches!(&snapshot.frame.ops[1], RenderDiff::Create { handle, .. } if *handle == parent)
        );
        assert!(
            matches!(&snapshot.frame.ops[2], RenderDiff::Create { handle, parent: Some(p), node }
            if *handle == child && *p == parent && node.transform == transform && !node.visible)
        );
        let mut reconstructed = PresentationWorld::default();
        reconstructed.apply(&snapshot.frame).unwrap();
        assert_eq!(reconstructed.snapshot().frame, snapshot.frame);

        let delta = world
            .apply(&frame(vec![RenderDiff::Destroy { handle: parent }]))
            .unwrap();
        assert_eq!(delta.publication.unwrap().base_revision, snapshot.revision);
        reconstructed
            .apply(&frame(vec![RenderDiff::Destroy { handle: parent }]))
            .unwrap();
        assert_eq!(reconstructed.snapshot().frame, world.snapshot().frame);
        assert!(
            world.nodes.is_empty(),
            "parent destruction removes retained descendants"
        );
    }

    #[test]
    fn failed_change_preserves_world_and_revision() {
        let mut world = PresentationWorld::default();
        let before = world.snapshot();
        let result = world.apply(&frame(vec![
            RenderDiff::Create {
                handle: RenderHandle::new(1),
                parent: None,
                node: RenderNode::new(Geometry::Cube),
            },
            RenderDiff::Destroy {
                handle: RenderHandle::new(999),
            },
        ]));
        assert!(matches!(
            result,
            Err(PresentationWorldError::UnknownNode(_))
        ));
        assert_eq!(world.snapshot(), before);
    }
}
