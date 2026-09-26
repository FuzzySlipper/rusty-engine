//! Canonical, host-neutral retained graphics intent. Projectors produce typed
//! changes into this owner; transports and renderer attachments read snapshots.
//! No browser objects, product callbacks, or incremental history are retained.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use crate::PresentationFrameDiff;
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
    parent_joint: Option<String>,
    parent: Option<RenderHandle>,
    kind: NodeKind,
    mesh_payload: Option<MeshPayloadDescriptor>,
    material_override: Option<Material>,
    material_parameters: BTreeMap<u16, MaterialInstanceParameters>,
    playback: AnimatedMeshPlaybackTimeline,
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
    elapsed_seconds: f64,
    effects: Arc<Vec<PresentationFrameDiff>>,
    retained: SharedGraphics,
}

/// Shared graphics collections; cloned only when a call first mutates them.
#[derive(Debug, Clone, Default, PartialEq)]
struct RetainedGraphics {
    ghost_captures: BTreeMap<crate::GhostPlateHandle, Arc<RenderFrameDiff>>,
    ghost_sources: BTreeMap<crate::GhostPlateHandle, RenderHandle>,
    nodes: BTreeMap<RenderHandle, Arc<PresentationNode>>,
    textures: BTreeMap<String, Arc<TextureDescriptor>>,
    materials: BTreeMap<String, Arc<RenderMaterialDescriptor>>,
    atlases: BTreeMap<String, Arc<SpriteAtlasDescriptor>>,
    static_meshes: BTreeMap<String, Arc<StaticMeshAsset>>,
    animated_meshes: BTreeMap<String, Arc<AnimatedMeshAsset>>,
    voxel_objects: BTreeMap<String, Arc<VoxelObjectRenderAsset>>,
    sky: Option<SkyBackgroundDescriptor>,
    background_color: Option<[f32; 4]>,
    controllers: BTreeMap<crate::AnimationProjectionHandle, crate::AnimationProjectionDescriptor>,
}

#[derive(Debug, Clone, Default, PartialEq)]
struct SharedGraphics(Arc<RetainedGraphics>);
impl std::ops::Deref for SharedGraphics {
    type Target = RetainedGraphics;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for SharedGraphics {
    fn deref_mut(&mut self) -> &mut Self::Target {
        Arc::make_mut(&mut self.0)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum PresentationWorldError {
    Frame(RenderFrameError),
    Presentation(crate::PresentationFrameError),
    RevisionExhausted,
    InvalidPlayback,
    UnknownNode(RenderHandle),
    DuplicateNode(RenderHandle),
    WrongNodeKind(RenderHandle),
    InvalidParentJoint { handle: RenderHandle, joint: String },
    UndefinedStaticMesh(String),
    UndefinedResource(String),
    ReferencedResource(String),
}

impl std::fmt::Display for PresentationWorldError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for PresentationWorldError {}

impl PresentationWorld {
    pub fn advance_elapsed(&mut self, seconds: f64) {
        if seconds.is_finite() && seconds >= 0.0 {
            self.elapsed_seconds += seconds;
        }
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Apply admitted graphics in order to the caller-owned candidate.
    /// Failure invalidates the candidate; only the call owner commits it.
    /// Upstream projector frontiers are internal to this world.
    pub fn apply(
        &mut self,
        frame: RenderFrameDiff,
    ) -> Result<RenderFrameDiff, PresentationWorldError> {
        if frame.ops.is_empty() {
            return Ok(RenderFrameDiff::new());
        }
        let revision = self
            .revision
            .checked_add(1)
            .filter(|revision| *revision <= JSON_SAFE_U64_MAX)
            .ok_or(PresentationWorldError::RevisionExhausted)?;
        let mut operations = Vec::with_capacity(frame.ops.len());
        for op in frame.ops {
            // Named services may select the same immutable texture repeatedly
            // (for example, the active sky). Retain that fact once rather than
            // publishing a stale texture-version update to the realization.
            if matches!(&op, RenderDiff::SetParentJoint { handle, joint } if self.retained.nodes.get(handle).is_some_and(|node| &node.parent_joint == joint))
                || matches!(&op, RenderDiff::DefineTexture { texture }
                if self.retained.textures.get(&texture.id).is_some_and(|current| current.as_ref() == texture))
                || matches!(&op, RenderDiff::SetSkyBackground { background } if &self.retained.sky == background && self.retained.background_color.is_none())
                || matches!(&op, RenderDiff::SetBackgroundColor { color } if self.retained.background_color == Some(*color) && self.retained.sky.is_none())
            {
                continue;
            }
            self.apply_operation(&op)?;
            operations.push(op);
        }
        if operations.is_empty() {
            return Ok(RenderFrameDiff::new());
        }
        // Every operation was validated on entry and copied without modification.
        // Only the publication metadata is new; do not rescan the mesh bodies.
        let operation_count = u32::try_from(operations.len()).map_err(|_| {
            PresentationWorldError::Frame(RenderFrameError::PublicationOperationCount {
                expected: u32::MAX,
                actual: operations.len(),
            })
        })?;
        let delta = RenderFrameDiff {
            schema_version: RENDER_FRAME_SCHEMA_VERSION,
            publication: Some(RenderFramePublication {
                stream: PRESENTATION_WORLD_STREAM.to_owned(),
                base_revision: self.revision,
                revision,
                operation_count,
            }),
            ops: operations,
        };
        self.revision = revision;
        Ok(delta)
    }

    /// Named mechanisms supply their complete retained state, never historical signals.
    /// The common world commits this alongside graphics and owns its publication frontier.
    pub fn retain_effects(&mut self, mut frames: Vec<PresentationFrameDiff>) {
        for frame in &mut frames {
            for op in &mut frame.ops {
                if let crate::PresentationOp::GhostPlate {
                    op: crate::GhostPlateProjectionOp::Create { handle, descriptor },
                    ..
                } = op
                {
                    descriptor.captured_scene = self.retained.ghost_captures.get(handle).cloned();
                }
            }
        }
        self.effects = Arc::new(frames);
    }

    pub fn effects_snapshot(&self) -> Vec<PresentationFrameDiff> {
        self.effects.as_ref().clone()
    }

    pub fn apply_presentation(
        &mut self,
        frame: PresentationFrameDiff,
    ) -> Result<PresentationFrameDiff, PresentationWorldError> {
        if frame.ops.is_empty() {
            return Ok(frame);
        }
        let revision = self
            .revision
            .checked_add(1)
            .filter(|revision| *revision <= JSON_SAFE_U64_MAX)
            .ok_or(PresentationWorldError::RevisionExhausted)?;
        let mut output = frame;
        for operation in &mut output.ops {
            if let crate::PresentationOp::Animation { op, .. } = operation {
                match op {
                    crate::AnimationProjectionOp::Create { handle, descriptor } => {
                        self.retained
                            .controllers
                            .insert(*handle, descriptor.clone());
                    }
                    crate::AnimationProjectionOp::Update { handle, controller } => {
                        if let Some(descriptor) = self.retained.controllers.get_mut(handle) {
                            descriptor.controller = controller.clone();
                        }
                    }
                    crate::AnimationProjectionOp::Destroy { handle } => {
                        self.retained.controllers.remove(handle);
                    }
                }
            }
            if let crate::PresentationOp::GhostPlate { op, .. } = operation {
                match op {
                    crate::GhostPlateProjectionOp::Create { handle, descriptor } => {
                        let capture = Arc::new(self.capture_scene(descriptor.source)?);
                        descriptor.captured_scene = Some(capture.clone());
                        self.retained
                            .ghost_sources
                            .insert(*handle, descriptor.source);
                        self.retained.ghost_captures.insert(*handle, capture);
                    }
                    crate::GhostPlateProjectionOp::Recapture {
                        handle,
                        captured_scene,
                        ..
                    } => {
                        let source = *self
                            .retained
                            .ghost_sources
                            .get(handle)
                            .ok_or(PresentationWorldError::InvalidPlayback)?;
                        let capture = Arc::new(self.capture_scene(source)?);
                        *captured_scene = Some(capture.clone());
                        self.retained.ghost_captures.insert(*handle, capture);
                    }
                    crate::GhostPlateProjectionOp::Destroy { handle } => {
                        self.retained.ghost_captures.remove(handle);
                        self.retained.ghost_sources.remove(handle);
                    }
                    crate::GhostPlateProjectionOp::Update { .. } => {}
                }
            }
        }
        output.publication = Some(RenderFramePublication {
            stream: PRESENTATION_WORLD_STREAM.to_owned(),
            base_revision: self.revision,
            revision,
            operation_count: output
                .ops
                .len()
                .try_into()
                .map_err(|_| PresentationWorldError::RevisionExhausted)?,
        });
        self.revision = revision;
        Ok(output)
    }

    /// Capture only the retained source hierarchy and scene lights. Resource
    /// definitions are immutable and shared by the stored capture frame.
    fn capture_scene(
        &self,
        source: RenderHandle,
    ) -> Result<RenderFrameDiff, PresentationWorldError> {
        self.capture_output_scene(source, false)
    }

    pub fn capture_output_scene(
        &self,
        source: RenderHandle,
        retain_background: bool,
    ) -> Result<RenderFrameDiff, PresentationWorldError> {
        if !self.retained.nodes.contains_key(&source) {
            return Err(PresentationWorldError::UnknownNode(source));
        }
        let mut selected = BTreeSet::new();
        selected.insert(source);
        loop {
            let before = selected.len();
            for (handle, node) in &self.retained.nodes {
                if node.parent.is_some_and(|parent| selected.contains(&parent)) {
                    selected.insert(*handle);
                }
            }
            if before == selected.len() {
                break;
            }
        }
        let mut renderable = selected.clone();
        // Ancestors preserve the capture's original world-space transform.
        let mut parent = self.retained.nodes[&source].parent;
        while let Some(handle) = parent {
            selected.insert(handle);
            parent = self.retained.nodes[&handle].parent;
        }
        for (handle, node) in &self.retained.nodes {
            if matches!(node.kind, NodeKind::Light(_)) {
                renderable.insert(*handle);
                selected.insert(*handle);
                let mut parent = node.parent;
                while let Some(handle) = parent {
                    selected.insert(handle);
                    parent = self.retained.nodes[&handle].parent;
                }
            }
        }
        let mut captured = self.clone();
        captured
            .retained
            .nodes
            .retain(|handle, _| selected.contains(handle));
        for (handle, node) in &mut captured.retained.nodes {
            if renderable.contains(handle) {
                continue;
            }
            let node = Arc::make_mut(node);
            macro_rules! group {
                ($value:expr, $layer:expr) => {
                    RenderNode {
                        geometry: Geometry::Group,
                        material: Material::DEFAULT,
                        transform: $value.transform,
                        visible: $value.visible,
                        layer: $layer,
                        metadata: $value.metadata.clone(),
                    }
                };
            }
            let group = match &node.kind {
                NodeKind::Primitive(value) => group!(value, value.layer),
                NodeKind::StaticMesh(value) => group!(value, RenderLayer::Scene),
                NodeKind::AnimatedMesh(value) => group!(value, RenderLayer::Scene),
                NodeKind::VoxelObject(value) => group!(value, RenderLayer::Scene),
                NodeKind::Sprite(value) => group!(value, value.layer),
                NodeKind::Light(_) => continue,
            };
            node.kind = NodeKind::Primitive(group);
            node.mesh_payload = None;
            node.material_override = None;
            node.material_parameters.clear();
        }

        if !retain_background {
            captured.retained.sky = None;
            captured.retained.background_color = None;
        }
        // Capture dependencies are a subset of the live world. In particular,
        // one small plate must not copy or realize every unrelated mesh.
        let mut meshes = BTreeSet::new();
        let mut animated = BTreeSet::new();
        let mut voxels = BTreeSet::new();
        let mut atlases = BTreeSet::new();
        let mut materials = BTreeSet::new();
        let mut textures = BTreeSet::new();
        for node in captured.retained.nodes.values() {
            match &node.kind {
                NodeKind::StaticMesh(instance) => {
                    meshes.insert(instance.asset.clone());
                    materials.extend(
                        instance
                            .material_overrides
                            .iter()
                            .map(|slot| slot.material.clone()),
                    );
                }
                NodeKind::AnimatedMesh(instance) => {
                    animated.insert(instance.asset.clone());
                    materials.extend(
                        instance
                            .material_overrides
                            .iter()
                            .map(|slot| slot.material.clone()),
                    );
                }
                NodeKind::VoxelObject(instance) => {
                    voxels.insert(instance.asset.clone());
                    materials.extend(
                        instance
                            .material_overrides
                            .iter()
                            .map(|slot| slot.material.clone()),
                    );
                }
                NodeKind::Sprite(sprite) => {
                    atlases.insert(sprite.asset.clone());
                    textures.extend(sprite.material.normal_texture.iter().cloned());
                    textures.extend(sprite.material.depth_texture.iter().cloned());
                }
                _ => {}
            }
        }
        captured
            .retained
            .static_meshes
            .retain(|id, _| meshes.contains(id));
        captured
            .retained
            .animated_meshes
            .retain(|id, _| animated.contains(id));
        captured
            .retained
            .voxel_objects
            .retain(|id, _| voxels.contains(id));
        captured
            .retained
            .atlases
            .retain(|id, _| atlases.contains(id));
        for asset in captured.retained.static_meshes.values() {
            materials.extend(
                asset
                    .material_slots
                    .iter()
                    .map(|slot| slot.material.clone()),
            );
        }
        for asset in captured.retained.animated_meshes.values() {
            materials.extend(
                asset
                    .material_slots
                    .iter()
                    .map(|slot| slot.material.clone()),
            );
        }
        for asset in captured.retained.voxel_objects.values() {
            materials.extend(
                asset
                    .material_slots
                    .iter()
                    .map(|slot| slot.material.clone()),
            );
        }
        captured
            .retained
            .materials
            .retain(|id, _| materials.contains(id));
        for material in captured.retained.materials.values() {
            textures.extend(material.texture.iter().cloned());
            if let Some(surface) = &material.voxel_surface {
                match &surface.mapping {
                    VoxelSurfaceMappingDescriptor::Repeat { texture, .. }
                    | VoxelSurfaceMappingDescriptor::Atlas { texture, .. } => {
                        textures.insert(texture.clone());
                    }
                }
            }
        }
        for atlas in captured.retained.atlases.values() {
            textures.insert(atlas.texture.clone());
        }
        if let Some(sky) = &captured.retained.sky {
            textures.insert(sky.texture.clone());
            if let Some(blend) = &sky.blend {
                textures.insert(blend.texture.clone());
            }
        }
        captured
            .retained
            .textures
            .retain(|id, _| textures.contains(id));
        let mut frame = captured.snapshot().frame;
        for controller in self
            .retained
            .controllers
            .values()
            .filter(|controller| renderable.contains(&controller.target))
        {
            frame
                .ops
                .push(controller.frozen_pose_operation(controller.controller.phase_seconds));
        }
        Ok(frame)
    }

    /// Resource definitions precede uses; parents precede children. Handles
    /// retain their active identity, even after many creates and destroys.
    pub fn snapshot(&self) -> PresentationSnapshot {
        let mut ops = Vec::new();
        ops.extend(
            self.retained
                .textures
                .values()
                .map(|value| value.as_ref().clone())
                .map(|texture| RenderDiff::DefineTexture { texture }),
        );
        ops.extend(
            self.retained
                .materials
                .values()
                .map(|value| value.as_ref().clone())
                .map(|material| RenderDiff::DefineMaterial { material }),
        );
        ops.extend(
            self.retained
                .atlases
                .values()
                .map(|value| value.as_ref().clone())
                .map(|atlas| RenderDiff::DefineSpriteAtlas { atlas }),
        );
        ops.extend(
            self.retained
                .static_meshes
                .values()
                .map(|value| value.as_ref().clone())
                .map(|asset| RenderDiff::DefineStaticMesh { asset }),
        );
        ops.extend(
            self.retained
                .animated_meshes
                .values()
                .map(|value| value.as_ref().clone())
                .map(|asset| RenderDiff::DefineAnimatedMesh { asset }),
        );
        ops.extend(
            self.retained
                .voxel_objects
                .values()
                .map(|value| value.as_ref().clone())
                .map(|asset| RenderDiff::DefineVoxelObject { asset }),
        );
        if let Some(color) = self.retained.background_color {
            ops.push(RenderDiff::SetBackgroundColor { color });
        } else {
            ops.push(RenderDiff::SetSkyBackground {
                background: self.retained.sky.clone(),
            });
        }
        // Creation requires an existing parent and parents cannot be changed,
        // so this traversal is acyclic by construction.
        let mut emitted = BTreeSet::new();
        for handle in self.retained.nodes.keys() {
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
        let node = &self.retained.nodes[&handle];
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
                instance: {
                    let mut instance = instance.clone();
                    instance.playback = node
                        .playback
                        .snapshot_command(self.elapsed_seconds)
                        .expect("admitted playback timeline");
                    instance
                },
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
        if let Some(joint) = &node.parent_joint {
            ops.push(RenderDiff::SetParentJoint {
                handle,
                joint: Some(joint.clone()),
            });
        }
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
        if self.retained.nodes.contains_key(&handle) {
            return Err(PresentationWorldError::DuplicateNode(handle));
        }
        if let Some(parent) = parent {
            if !self.retained.nodes.contains_key(&parent) {
                return Err(PresentationWorldError::UnknownNode(parent));
            }
        }
        let mut playback = AnimatedMeshPlaybackTimeline::default();
        if let NodeKind::AnimatedMesh(instance) = &kind {
            if let Some(command) = &instance.playback {
                playback
                    .apply_command(command, self.elapsed_seconds)
                    .map_err(|_| PresentationWorldError::InvalidPlayback)?;
            }
        }
        self.retained.nodes.insert(
            handle,
            Arc::new(PresentationNode {
                parent_joint: None,
                playback,
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
        self.retained
            .nodes
            .get_mut(&handle)
            .map(Arc::make_mut)
            .ok_or(PresentationWorldError::UnknownNode(handle))
    }

    fn apply_operation(&mut self, op: &RenderDiff) -> Result<(), PresentationWorldError> {
        match op {
            RenderDiff::SetParentJoint { handle, joint } => {
                if let Some(joint) = joint {
                    let node = self
                        .retained
                        .nodes
                        .get(handle)
                        .ok_or(PresentationWorldError::UnknownNode(*handle))?;
                    let parent = node
                        .parent
                        .and_then(|parent| self.retained.nodes.get(&parent));
                    let valid = match parent.map(|parent| &parent.kind) {
                        Some(NodeKind::AnimatedMesh(instance)) => self
                            .retained
                            .animated_meshes
                            .get(&instance.asset)
                            .and_then(|asset| asset.rig.as_ref())
                            .is_some_and(|rig| {
                                rig.joints
                                    .iter()
                                    .filter(|candidate| candidate.id == *joint)
                                    .count()
                                    == 1
                            }),
                        _ => false,
                    };
                    if !valid {
                        return Err(PresentationWorldError::InvalidParentJoint {
                            handle: *handle,
                            joint: joint.clone(),
                        });
                    }
                }
                self.node_mut(*handle)?.parent_joint = joint.clone();
            }
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
                if !self.retained.nodes.contains_key(handle) {
                    return Err(PresentationWorldError::UnknownNode(*handle));
                }
                let mut removed = BTreeSet::from([*handle]);
                loop {
                    let before = removed.len();
                    for (handle, node) in &self.retained.nodes {
                        if node.parent.is_some_and(|parent| removed.contains(&parent)) {
                            removed.insert(*handle);
                        }
                    }
                    if before == removed.len() {
                        break;
                    }
                }
                self.retained
                    .nodes
                    .retain(|handle, _| !removed.contains(handle));
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
            RenderDiff::SetAnimatedMeshInspection { handle, inspection } => {
                match &mut self.node_mut(*handle)?.kind {
                    NodeKind::AnimatedMesh(value) => value.inspection = inspection.clone(),
                    _ => return Err(PresentationWorldError::WrongNodeKind(*handle)),
                }
            }
            RenderDiff::SetAnimatedMeshPlayback { handle, playback } => {
                let now = self.elapsed_seconds;
                let node = self.node_mut(*handle)?;
                node.playback
                    .apply_command(playback, now)
                    .map_err(|_| PresentationWorldError::InvalidPlayback)?;
                match &mut node.kind {
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
            RenderDiff::ReleaseMaterial { id } => {
                if !self.retained.materials.contains_key(id) {
                    return Err(PresentationWorldError::UndefinedResource(id.clone()));
                }
                let bound = self.retained.static_meshes.values().any(|asset| asset.material_slots.iter().any(|slot| &slot.material == id))
                    || self.retained.animated_meshes.values().any(|asset| asset.material_slots.iter().any(|slot| &slot.material == id))
                    || self.retained.voxel_objects.values().any(|asset| asset.material_slots.iter().any(|slot| &slot.material == id))
                    || self.retained.nodes.values().any(|node| match &node.kind {
                        NodeKind::StaticMesh(instance) => instance.material_overrides.iter().any(|slot| &slot.material == id),
                        NodeKind::AnimatedMesh(instance) => instance.material_overrides.iter().any(|slot| &slot.material == id),
                        NodeKind::VoxelObject(instance) => instance.material_overrides.iter().any(|slot| &slot.material == id),
                        _ => false,
                    })
                    || self.retained.ghost_captures.values().any(|frame| frame.ops.iter().any(|op| matches!(op, RenderDiff::DefineMaterial { material } if &material.id == id)));
                if bound {
                    return Err(PresentationWorldError::ReferencedResource(id.clone()));
                }
                self.retained.materials.remove(id);
            }
            RenderDiff::ReleaseTexture { id } => {
                if !self.retained.textures.contains_key(id) {
                    return Err(PresentationWorldError::UndefinedResource(id.clone()));
                }
                let bound = self.retained.materials.values().any(|material| material.texture.as_ref() == Some(id)
                    || material.voxel_surface.as_ref().is_some_and(|surface| match &surface.mapping {
                        VoxelSurfaceMappingDescriptor::Repeat { texture, .. } | VoxelSurfaceMappingDescriptor::Atlas { texture, .. } => texture == id,
                    }))
                    || self.retained.atlases.values().any(|atlas| &atlas.texture == id)
                    || self.retained.sky.as_ref().is_some_and(|sky| &sky.texture == id || sky.blend.as_ref().is_some_and(|blend| &blend.texture == id))
                    || self.retained.nodes.values().any(|node| matches!(&node.kind, NodeKind::Sprite(sprite) if sprite.material.normal_texture.as_ref() == Some(id) || sprite.material.depth_texture.as_ref() == Some(id)))
                    || self.retained.ghost_captures.values().any(|frame| frame.ops.iter().any(|op| matches!(op, RenderDiff::DefineTexture { texture } if &texture.id == id)));
                if bound {
                    return Err(PresentationWorldError::ReferencedResource(id.clone()));
                }
                self.retained.textures.remove(id);
            }
            RenderDiff::ReleaseSpriteAtlas { id } => {
                if !self.retained.atlases.contains_key(id) {
                    return Err(PresentationWorldError::UndefinedResource(id.clone()));
                }
                if self.retained.nodes.values().any(|node| matches!(&node.kind, NodeKind::Sprite(sprite) if &sprite.asset == id))
                    || self.retained.ghost_captures.values().any(|frame| frame.ops.iter().any(|op| matches!(op, RenderDiff::DefineSpriteAtlas { atlas } if &atlas.id == id))) {
                    return Err(PresentationWorldError::ReferencedResource(id.clone()));
                }
                self.retained.atlases.remove(id);
            }
            RenderDiff::ReleaseAnimatedMesh { asset } => {
                if !self.retained.animated_meshes.contains_key(asset) {
                    return Err(PresentationWorldError::UndefinedResource(asset.clone()));
                }
                if self.retained.nodes.values().any(|node| matches!(&node.kind, NodeKind::AnimatedMesh(instance) if &instance.asset == asset))
                    || self.retained.ghost_captures.values().any(|frame| frame.ops.iter().any(|op| matches!(op, RenderDiff::DefineAnimatedMesh { asset: definition } if &definition.asset == asset))) {
                    return Err(PresentationWorldError::ReferencedResource(asset.clone()));
                }
                self.retained.animated_meshes.remove(asset);
            }
            RenderDiff::DefineTexture { texture } => {
                self.retained
                    .textures
                    .insert(texture.id.clone(), Arc::new(texture.clone()));
            }
            RenderDiff::DefineMaterial { material } => {
                self.retained
                    .materials
                    .insert(material.id.clone(), Arc::new(material.clone()));
            }
            RenderDiff::DefineSpriteAtlas { atlas } => {
                self.retained
                    .atlases
                    .insert(atlas.id.clone(), Arc::new(atlas.clone()));
            }
            RenderDiff::DefineStaticMesh { asset } => {
                self.retained
                    .static_meshes
                    .insert(asset.asset.clone(), Arc::new(asset.clone()));
            }
            RenderDiff::ReleaseStaticMesh { asset } => {
                if !self.retained.static_meshes.contains_key(asset) {
                    return Err(PresentationWorldError::UndefinedStaticMesh(asset.clone()));
                }
                if self.retained.nodes.values().any(|node| matches!(&node.kind, NodeKind::StaticMesh(instance) if &instance.asset == asset)) {
                    return Err(PresentationWorldError::ReferencedResource(asset.clone()));
                }
                self.retained.static_meshes.remove(asset);
            }
            RenderDiff::DefineAnimatedMesh { asset } => {
                self.retained
                    .animated_meshes
                    .insert(asset.asset.clone(), Arc::new(asset.clone()));
            }
            RenderDiff::DefineVoxelObject { asset } => {
                self.retained
                    .voxel_objects
                    .insert(asset.asset.clone(), Arc::new(asset.clone()));
            }
            RenderDiff::ReleaseVoxelObject { asset } => {
                if self.retained.nodes.values().any(|node| matches!(&node.kind, NodeKind::VoxelObject(instance) if &instance.asset == asset)) {
                    return Err(PresentationWorldError::ReferencedResource(asset.clone()));
                }
                self.retained.voxel_objects.remove(asset);
            }
            RenderDiff::SetSkyBackground { background } => {
                self.retained.sky = background.clone();
                self.retained.background_color = None;
            }
            RenderDiff::SetBackgroundColor { color } => {
                self.retained.sky = None;
                self.retained.background_color = Some(*color);
            }
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

    fn static_mesh(asset: &str) -> StaticMeshAsset {
        StaticMeshAsset {
            asset: asset.to_string(),
            payload: MeshPayloadDescriptor {
                layout: MeshBufferLayout {
                    vertex_count: 3,
                    index_count: 3,
                    index_width: MeshIndexWidth::U32,
                    attributes: vec![
                        MeshAttribute {
                            name: MeshAttributeName::Position,
                            components: 3,
                            kind: MeshAttributeKind::F32,
                        },
                        MeshAttribute {
                            name: MeshAttributeName::Normal,
                            components: 3,
                            kind: MeshAttributeKind::F32,
                        },
                    ],
                },
                groups: vec![MeshGroupDescriptor {
                    material_slot: 0,
                    start: 0,
                    count: 3,
                }],
                bounds: MeshBoundsDescriptor {
                    min: [0.0; 3],
                    max: [1.0, 1.0, 0.0],
                },
                source: MeshPayloadSource::Inline {
                    positions: vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
                    normals: vec![0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0],
                    uvs: None,
                    colors: None,
                    indices: vec![0, 1, 2],
                },
                provenance: MeshProvenance::StaticAsset,
            },
            material_slots: vec![MeshMaterialSlot {
                slot: 0,
                material: "material/plain".to_string(),
            }],
            collision: MeshCollisionPolicy::VisualOnly,
        }
    }

    #[test]
    fn group_capture_includes_every_part_without_unrelated_geometry() {
        let mut world = PresentationWorld::default();
        world
            .apply(frame(vec![
                RenderDiff::Create {
                    handle: RenderHandle::new(1),
                    parent: None,
                    node: RenderNode::new(Geometry::Group),
                },
                RenderDiff::Create {
                    handle: RenderHandle::new(2),
                    parent: Some(RenderHandle::new(1)),
                    node: RenderNode::new(Geometry::Cube),
                },
                RenderDiff::Create {
                    handle: RenderHandle::new(3),
                    parent: Some(RenderHandle::new(1)),
                    node: RenderNode::new(Geometry::Sphere),
                },
                RenderDiff::Create {
                    handle: RenderHandle::new(4),
                    parent: None,
                    node: RenderNode::new(Geometry::Quad),
                },
            ]))
            .unwrap();
        let captured = world
            .capture_output_scene(RenderHandle::new(1), false)
            .unwrap();
        let handles: Vec<_> = captured
            .ops
            .iter()
            .filter_map(|op| match op {
                RenderDiff::Create { handle, .. } => Some(handle.raw()),
                _ => None,
            })
            .collect();
        assert_eq!(handles, vec![1, 2, 3]);
        assert!(captured.ops.iter().any(|op| matches!(op, RenderDiff::Create { handle, node, .. } if handle.raw() == 1 && node.geometry == Geometry::Group)));
        world
            .apply(frame(vec![RenderDiff::Destroy {
                handle: RenderHandle::new(3),
            }]))
            .unwrap();
        assert!(captured
            .ops
            .iter()
            .any(|op| matches!(op, RenderDiff::Create { handle, .. } if handle.raw() == 3)));
    }

    #[test]
    fn selected_child_capture_retains_ancestor_transform_without_ancestor_geometry() {
        let mut world = PresentationWorld::default();
        let mut parent = RenderNode::new(Geometry::Cube);
        parent.transform.translation = [3.0, 0.0, 0.0];
        world
            .apply(frame(vec![
                RenderDiff::Create {
                    handle: RenderHandle::new(1),
                    parent: None,
                    node: parent.clone(),
                },
                RenderDiff::Create {
                    handle: RenderHandle::new(2),
                    parent: Some(RenderHandle::new(1)),
                    node: RenderNode::new(Geometry::Sphere),
                },
            ]))
            .unwrap();
        let captured = world
            .capture_output_scene(RenderHandle::new(2), false)
            .unwrap();
        assert!(captured.ops.iter().any(|op| matches!(op, RenderDiff::Create {handle,node,..} if handle.raw()==1 && node.geometry==Geometry::Group && node.transform==parent.transform)));
        assert!(captured.ops.iter().any(|op| matches!(op, RenderDiff::Create {handle,node,..} if handle.raw()==2 && node.geometry==Geometry::Sphere)));
        assert!(world.snapshot().frame.ops.iter().any(|op| matches!(op, RenderDiff::Create {handle,node,..} if handle.raw()==1 && node.geometry==Geometry::Cube)));
    }

    #[test]
    fn candidate_copies_collections_only_on_first_mutation() {
        let committed = PresentationWorld::default();
        let mut candidate = committed.clone();
        candidate.advance_elapsed(0.1);
        candidate.retain_effects(Vec::new());
        assert!(Arc::ptr_eq(&committed.retained.0, &candidate.retained.0));
        candidate
            .apply(frame(vec![RenderDiff::DefineStaticMesh {
                asset: static_mesh("mesh/one"),
            }]))
            .unwrap();
        assert!(!Arc::ptr_eq(&committed.retained.0, &candidate.retained.0));
        let retained = Arc::as_ptr(&candidate.retained.0);
        candidate
            .apply(frame(vec![RenderDiff::DefineStaticMesh {
                asset: static_mesh("mesh/two"),
            }]))
            .unwrap();
        assert_eq!(retained, Arc::as_ptr(&candidate.retained.0));
        assert!(!committed.retained.static_meshes.contains_key("mesh/one"));
    }

    #[test]
    fn static_mesh_release_requires_an_unused_live_asset_and_preserves_frozen_capture() {
        let mut world = PresentationWorld::default();
        let asset = static_mesh("mesh/frozen-capture");
        let handle = RenderHandle::new(41);
        world
            .apply(frame(vec![
                RenderDiff::DefineStaticMesh {
                    asset: asset.clone(),
                },
                RenderDiff::CreateStaticMeshInstance {
                    handle,
                    parent: None,
                    instance: StaticMeshInstanceDescriptor {
                        asset: asset.asset.clone(),
                        transform: Transform::IDENTITY,
                        visible: true,
                        material_overrides: Vec::new(),
                        metadata: RenderMetadata::default(),
                    },
                },
            ]))
            .unwrap();
        let frozen_capture = world.capture_scene(handle).unwrap();
        assert!(frozen_capture
            .ops
            .iter()
            .any(|operation| matches!(operation, RenderDiff::DefineStaticMesh { asset } if asset.asset == "mesh/frozen-capture")));

        assert!(matches!(
            world.apply(frame(vec![RenderDiff::ReleaseStaticMesh {
                asset: asset.asset.clone(),
            }])),
            Err(PresentationWorldError::ReferencedResource(_))
        ));
        world
            .apply(frame(vec![RenderDiff::Destroy { handle }]))
            .unwrap();
        world
            .apply(frame(vec![RenderDiff::ReleaseStaticMesh {
                asset: asset.asset.clone(),
            }]))
            .unwrap();

        assert!(!world.snapshot().frame.ops.iter().any(
            |operation| matches!(operation, RenderDiff::DefineStaticMesh { asset } if asset.asset == "mesh/frozen-capture"),
        ));
        assert!(frozen_capture
            .ops
            .iter()
            .any(|operation| matches!(operation, RenderDiff::DefineStaticMesh { asset } if asset.asset == "mesh/frozen-capture")));
        assert!(matches!(
            world.apply(frame(vec![RenderDiff::ReleaseStaticMesh {
                asset: asset.asset.clone(),
            }])),
            Err(PresentationWorldError::UndefinedStaticMesh(_))
        ));
    }

    #[test]
    fn baseline_keeps_current_identity_and_hierarchy_after_churn() {
        let mut world = PresentationWorld::default();
        let parent = RenderHandle::new(90);
        let child = RenderHandle::new(4);
        world
            .apply(frame(vec![
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
            .apply(frame(vec![RenderDiff::Update {
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
        reconstructed.apply(snapshot.frame.clone()).unwrap();
        assert_eq!(reconstructed.snapshot().frame, snapshot.frame);

        let delta = world
            .apply(frame(vec![RenderDiff::Destroy { handle: parent }]))
            .unwrap();
        assert_eq!(delta.publication.unwrap().base_revision, snapshot.revision);
        reconstructed
            .apply(frame(vec![RenderDiff::Destroy { handle: parent }]))
            .unwrap();
        assert_eq!(reconstructed.snapshot().frame, world.snapshot().frame);
        assert!(
            world.retained.nodes.is_empty(),
            "parent destruction removes retained descendants"
        );
    }

    #[test]
    fn graphics_and_effects_share_one_reconstructible_frontier() {
        use crate::{
            AudioBus, AudioBusControl, AudioProjectionOp, PresentationOp, PresentationOpMeta,
        };
        let mut world = PresentationWorld::default();
        let graphics = world
            .apply(frame(vec![RenderDiff::Create {
                handle: RenderHandle::new(1),
                parent: None,
                node: RenderNode::new(Geometry::Cube),
            }]))
            .unwrap();
        let retained = PresentationFrameDiff::try_from_ops(vec![PresentationOp::Audio {
            meta: PresentationOpMeta::new(0),
            op: AudioProjectionOp::BusControl {
                bus: AudioBus::Sfx,
                control: AudioBusControl::SetMuted { muted: true },
            },
        }])
        .unwrap();
        let effect_delta = world.apply_presentation(retained.clone()).unwrap();
        assert_eq!(
            effect_delta.publication.as_ref().unwrap().base_revision,
            graphics.publication.unwrap().revision
        );
        world.retain_effects(vec![retained.clone()]);
        assert_eq!(world.effects_snapshot(), vec![retained]);
        assert_eq!(world.snapshot().revision, 2);
        let next = world
            .apply(frame(vec![RenderDiff::Destroy {
                handle: RenderHandle::new(1),
            }]))
            .unwrap();
        assert_eq!(next.publication.unwrap().base_revision, 2);
    }

    #[test]
    fn failed_change_preserves_world_and_revision() {
        let world = PresentationWorld::default();
        let mut candidate = world.clone();
        let before = world.snapshot();
        let result = candidate.apply(frame(vec![
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

    #[test]
    fn background_color_and_sky_clear_replace_one_retained_background() {
        let mut world = PresentationWorld::default();
        world
            .apply(frame(vec![RenderDiff::SetBackgroundColor {
                color: [0.0, 0.0, 0.0, 1.0],
            }]))
            .unwrap();
        assert!(matches!(
            world.snapshot().frame.ops.as_slice(),
            [RenderDiff::SetBackgroundColor { color }] if *color == [0.0, 0.0, 0.0, 1.0]
        ));
        world
            .apply(frame(vec![RenderDiff::SetSkyBackground {
                background: None,
            }]))
            .unwrap();
        assert!(matches!(
            world.snapshot().frame.ops.as_slice(),
            [RenderDiff::SetSkyBackground { background: None }]
        ));
    }
}

#[cfg(test)]
mod joint_attachment_tests {
    use super::*;
    #[test]
    fn joint_relation_survives_baselines_rejects_missing_joint_and_releases_with_parent() {
        let body = RenderHandle::new(8654);
        let child = RenderHandle::new(8655);
        let asset = AnimatedMeshAsset {
            asset: "mesh-animation/body".into(),
            runtime_format: AnimatedMeshRuntimeFormat::Glb,
            content_hash: None,
            clips: vec![],
            clip_packs: vec![],
            default_clip: None,
            embedded_material_slots: vec![],
            material_slots: vec![],
            bounds: MeshBoundsDescriptor {
                min: [0.0; 3],
                max: [1.0; 3],
            },
            rig: Some(AnimationRigSignature {
                joints: vec![AnimationRigJoint {
                    id: "Hand".into(),
                    parent: None,
                }],
                bind_rest_hash: format!("sha256:{}", "a".repeat(64)),
                bind_rest_convention: AnimationBindRestConvention::LocalMatrixV1,
                root_convention: AnimationRootConvention::InPlace,
                root_joint_id: "Hand".into(),
                structural_root_ids: vec!["Hand".into()],
                designated_motion_root_ids: vec![],
                authored_pose_translation_joint_ids: vec![],
            }),
        };
        let mut world = PresentationWorld::default();
        let initial = RenderFrameDiff::try_from_ops(vec![
            RenderDiff::DefineAnimatedMesh {
                asset: asset.clone(),
            },
            RenderDiff::CreateAnimatedMeshInstance {
                handle: body,
                parent: None,
                instance: AnimatedMeshInstanceDescriptor {
                    asset: asset.asset.clone(),
                    transform: Transform::IDENTITY,
                    material_overrides: vec![],
                    playback: None,
                    visible: true,
                    metadata: RenderMetadata::default(),
                    inspection: AnimatedMeshInspection::default(),
                },
            },
            RenderDiff::Create {
                handle: child,
                parent: Some(body),
                node: RenderNode::new(Geometry::Cube),
            },
            RenderDiff::SetParentJoint {
                handle: child,
                joint: Some("Hand".into()),
            },
        ])
        .unwrap();
        world.apply(initial.clone()).unwrap();
        let baseline = world.snapshot();
        assert!(baseline.frame.ops.iter().any(|op| matches!(op, RenderDiff::SetParentJoint { handle, joint: Some(joint) } if *handle == child && joint == "Hand")));
        let mut restored = PresentationWorld::default();
        restored.apply(baseline.frame.clone()).unwrap();
        let revision = world.revision();
        let repeated = RenderFrameDiff::try_from_ops(vec![RenderDiff::SetParentJoint {
            handle: child,
            joint: Some("Hand".into()),
        }])
        .unwrap();
        assert!(world.apply(repeated.clone()).unwrap().ops.is_empty());
        let bad = RenderFrameDiff::try_from_ops(vec![RenderDiff::SetParentJoint {
            handle: child,
            joint: Some("MissingHand".into()),
        }])
        .unwrap();
        assert!(world
            .apply(bad.clone())
            .unwrap_err()
            .to_string()
            .contains("MissingHand"));
        assert_eq!(world.revision(), revision);
        world
            .apply(
                RenderFrameDiff::try_from_ops(vec![
                    RenderDiff::Destroy { handle: body },
                    RenderDiff::ReleaseAnimatedMesh { asset: asset.asset },
                ])
                .unwrap(),
            )
            .unwrap();
        assert!(!world
            .snapshot()
            .frame
            .ops
            .iter()
            .any(|op| matches!(op, RenderDiff::SetParentJoint { .. })));
    }
}
