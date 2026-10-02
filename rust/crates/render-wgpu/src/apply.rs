//! Delta routing: each `RenderDiff` updates its table and marks what changed.
//! A delta the backend cannot realize is reported and skipped; the retained
//! model already validated every op, so nothing here re-validates it.

use std::collections::BTreeMap;

use glam::{Mat4, Vec3};
use render_model::{
    Geometry, LightDescriptor, MaterialAlphaModeDescriptor, RenderDiff, RenderHandle, RenderLayer,
    RenderMaterialDescriptor, StaticMeshAsset, TextureColorSpace, TextureDescriptor, TextureFilter,
    TextureWrap, VoxelSurfaceAlphaModeDescriptor,
};
use wgpu::util::DeviceExt;

use crate::batch::Pass;
use crate::glb::UvTransform;
use crate::pipelines::VERTEX_FLOATS;
use crate::resources::{self, ResourceSource};
use crate::shaders::Features;
use crate::tables::{
    Aabb, Builtin, Environment, GpuMesh, GpuTexture, MaterialRef, MaterialRow, MeshRef, NodeKind,
    NodeRow, Part, PartClass, PartRow, Topology,
};
use crate::voxel::VoxelSurfaceUniform;
use crate::Renderer;

/// An op the backend skipped, with the reason. Rendering continues without it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyIssue {
    pub op: &'static str,
    pub detail: String,
}

/// `MaterialUniform` size (`rusty::types`): roughness, cutoff, metalness,
/// normal scale; the voxel surface's tile scale, tile origin and sample rect;
/// the base, emissive, normal and occlusion uv transforms (two rows each); the
/// occlusion strength and triplanar sharpness; each slot's uv set; a
/// product shader's 16 parameters.
const MATERIAL_UNIFORM_BYTES: usize = 272;
/// Anisotropic filtering of mipmapped material textures.
const MATERIAL_ANISOTROPY: u16 = 16;
/// Payload groups without a voxel material are fully rough.
const FALLBACK_ROUGHNESS: f32 = 1.0;
/// Prefix of the retained materials payload mesh groups bind by slot.
const PAYLOAD_SLOT_MATERIAL_PREFIX: &str = "voxel-material/";

impl Renderer {
    /// Apply one delta from `PresentationWorld` (a fresh backend applies the
    /// world's snapshot frame first). Returns the ops it could not realize.
    pub fn apply(
        &mut self,
        frame: &render_model::RenderFrameDiff,
        resources: &dyn ResourceSource,
    ) -> Vec<ApplyIssue> {
        let mut issues = Vec::new();
        if !frame.ops.is_empty() {
            self.scene_generation += 1;
        }
        for op in &frame.ops {
            if let Err(detail) = self.apply_op(op, resources) {
                issues.push(ApplyIssue {
                    op: op_name(op),
                    detail,
                });
            }
        }
        issues
    }

    fn apply_op(&mut self, op: &RenderDiff, resources: &dyn ResourceSource) -> Result<(), String> {
        match op {
            RenderDiff::DefineTexture { texture } => self.define_texture(texture, resources)?,
            RenderDiff::ReleaseTexture { id } => {
                self.tables.textures.remove(id);
                if let Some(id) = self.tables.names.get(id) {
                    self.effects.forget_texture(id);
                }
            }
            RenderDiff::DefineMaterial { material } => self.define_material(material.clone())?,
            RenderDiff::DefineShader { shader } => self.define_shader(shader)?,
            RenderDiff::ReleaseShader { id } => {
                self.tables.shaders.remove(id);
            }
            RenderDiff::ReleaseMaterial { id } => {
                if let Some(id) = self.tables.names.get(id) {
                    self.tables.materials.remove(id);
                }
            }
            RenderDiff::DefineStaticMesh { asset } => self.define_static_mesh(asset, resources)?,
            RenderDiff::ReleaseStaticMesh { asset } => {
                if let Some(id) = self.tables.names.get(asset) {
                    self.tables.static_meshes.remove(id);
                }
            }
            RenderDiff::DefineSpriteAtlas { atlas } => {
                let id = self.tables.names.id(&atlas.id);
                let texture = self.tables.names.id(&atlas.texture);
                self.tables.atlases.insert(
                    id,
                    crate::tables::AtlasRow {
                        descriptor: atlas.clone(),
                        texture,
                    },
                );
            }
            RenderDiff::ReleaseSpriteAtlas { id } => {
                if let Some(id) = self.tables.names.get(id) {
                    self.tables.atlases.remove(id);
                }
            }
            RenderDiff::SetBackgroundColor { color } => {
                self.tables.environment = Environment::Color(*color);
                self.tables.environment_dirty = true;
            }
            RenderDiff::SetFog { fog } => self.tables.fog = *fog,
            RenderDiff::SetToneMapping { tone_mapping } => {
                self.tables.tone_mapping = *tone_mapping;
            }
            RenderDiff::SetSkyBackground { background } => {
                self.tables.environment = match background {
                    Some(sky) => Environment::Sky(sky.clone()),
                    None => Environment::Default,
                };
                self.tables.environment_dirty = true;
            }
            RenderDiff::Create {
                handle,
                parent,
                node,
            } => {
                let kind = match node.geometry {
                    Geometry::Group => NodeKind::Group,
                    geometry => NodeKind::Primitive {
                        geometry,
                        material: node.material,
                        has_payload: false,
                    },
                };
                self.insert_node(
                    *handle,
                    *parent,
                    crate::convert::transform_matrix(&node.transform),
                    node.visible,
                    node.layer,
                    kind,
                );
            }
            RenderDiff::CreateStaticMeshInstance {
                handle,
                parent,
                instance,
            } => self.insert_node(
                *handle,
                *parent,
                crate::convert::transform_matrix(&instance.transform),
                instance.visible,
                instance.layer,
                NodeKind::StaticMesh {
                    asset: instance.asset.clone(),
                    overrides: instance
                        .material_overrides
                        .iter()
                        .map(|slot| (slot.slot, slot.material.clone()))
                        .collect(),
                    parameters: BTreeMap::new(),
                },
            ),
            RenderDiff::CreateAnimatedMeshInstance {
                handle,
                parent,
                instance,
            } => {
                self.insert_node(
                    *handle,
                    *parent,
                    crate::convert::transform_matrix(&instance.transform),
                    instance.visible,
                    instance.layer,
                    NodeKind::AnimatedMesh(Box::new(instance.clone())),
                );
                self.create_animated_instance(*handle);
            }
            RenderDiff::CreateVoxelObjectInstance {
                handle,
                parent,
                instance,
            } => self.insert_node(
                *handle,
                *parent,
                crate::convert::transform_matrix(&instance.transform),
                instance.visible,
                RenderLayer::Scene,
                NodeKind::VoxelObject(Box::new(instance.clone())),
            ),
            RenderDiff::CreateSprite {
                handle,
                parent,
                sprite,
            } => {
                let row = crate::tables::SpriteRow {
                    atlas: self.tables.names.id(&sprite.asset),
                    detail: crate::effects::sprite_detail_texture(sprite)
                        .map(|name| self.tables.names.id(name)),
                    descriptor: sprite.clone(),
                };
                self.insert_node(
                    *handle,
                    *parent,
                    crate::convert::transform_matrix(&sprite.transform),
                    sprite.visible,
                    sprite.layer,
                    NodeKind::Sprite(Box::new(row)),
                )
            }
            RenderDiff::CreateLight {
                handle,
                parent,
                light,
            } => {
                self.insert_node(
                    *handle,
                    *parent,
                    Mat4::IDENTITY,
                    true,
                    RenderLayer::Scene,
                    NodeKind::Light(light.clone()),
                );
                self.tables.lights.insert(*handle);
                self.tables.lights_dirty = true;
            }
            RenderDiff::UpdateLight { handle, light } => {
                let node = self.node_mut(*handle)?;
                node.kind = NodeKind::Light(light.clone());
                self.tables.sprites.remove(handle);
                self.tables.lights_dirty = true;
            }
            RenderDiff::Update {
                handle,
                transform,
                material,
                visible,
                ..
            } => {
                let node = self.node_mut(*handle)?;
                if let Some(transform) = transform {
                    node.local = crate::convert::transform_matrix(transform);
                }
                if let Some(visible) = visible {
                    node.visible = *visible;
                }
                // A view material applies to primitive nodes, including their
                // replaced payloads. No Engine producer sends one for another
                // kind (the appearance projector recreates on material
                // changes), so it is reported rather than guessed at.
                let mut rebuild = false;
                let mut unrealized_material = false;
                match (material, &mut node.kind) {
                    (
                        Some(material),
                        NodeKind::Primitive {
                            material: current, ..
                        },
                    ) => {
                        *current = *material;
                        rebuild = true;
                    }
                    (Some(_), NodeKind::Group) | (None, _) => {}
                    (Some(_), _) => unrealized_material = true,
                }
                if transform.is_some() || visible.is_some() {
                    self.tables.dirty_nodes.insert(*handle);
                }
                if rebuild {
                    self.rebuild_parts(*handle);
                }
                if unrealized_material {
                    return Err(
                        "a view material applies only to primitive nodes; the rest of the update applied"
                            .to_owned(),
                    );
                }
            }
            RenderDiff::Destroy { handle } => self.destroy_node(*handle),
            RenderDiff::SetParentJoint { handle, joint } => {
                // The child follows the named joint of its animated parent.
                self.node_mut(*handle)?.parent_joint = joint.clone();
                let parent = self.tables.nodes[handle].parent;
                let resolved = parent
                    .zip(joint.as_deref())
                    .and_then(|(parent, joint)| self.joint_node(parent, joint));
                self.node_mut(*handle)?.parent_joint_node = resolved;
                self.tables.dirty_nodes.insert(*handle);
                if let Some(joint) = joint {
                    let parent = self.tables.nodes[handle].parent;
                    if parent
                        .and_then(|parent| self.joint_pose(parent, joint))
                        .is_none()
                    {
                        return Err(format!(
                            "joint {joint} is not a unique joint of the parent's animated mesh"
                        ));
                    }
                }
            }
            RenderDiff::ReplaceMeshPayload { handle, payload } => {
                let mut streams = resources::mesh_streams(payload, resources)?;
                // Uploaded payloads draw without vertex colours.
                streams.colors = None;
                let mut mesh = self.upload_mesh(
                    &format!("payload {}", handle.raw()),
                    &streams,
                    Topology::Triangles,
                    payload
                        .groups
                        .iter()
                        .map(|group| (group.material_slot, group.start, group.count))
                        .collect(),
                    BTreeMap::new(),
                );
                mesh.texture_space = payload.texture_space;
                self.tables.payload_meshes.insert(*handle, mesh);
                if let NodeKind::Primitive { has_payload, .. } = &mut self.node_mut(*handle)?.kind {
                    *has_payload = true;
                }
                self.rebuild_parts(*handle);
            }
            RenderDiff::SetMaterialInstanceParameters {
                handle,
                slot,
                parameters,
            } => {
                if let NodeKind::StaticMesh {
                    parameters: table, ..
                } = &mut self.node_mut(*handle)?.kind
                {
                    match parameters {
                        Some(parameters) => table.insert(*slot, *parameters),
                        None => table.remove(slot),
                    };
                }
                self.rebuild_parts(*handle);
            }
            RenderDiff::UpdateSprite {
                handle,
                frame,
                tint,
                render_order,
                visible,
            } => {
                let node = self.node_mut(*handle)?;
                if let NodeKind::Sprite(row) = &mut node.kind {
                    let sprite = &mut row.descriptor;
                    if let Some(frame) = frame {
                        sprite.frame = *frame;
                    }
                    if let Some(tint) = tint {
                        sprite.tint = *tint;
                    }
                    if let Some(render_order) = render_order {
                        sprite.render_order = *render_order;
                    }
                }
                if let Some(visible) = visible {
                    node.visible = *visible;
                    self.tables.dirty_nodes.insert(*handle);
                }
            }
            RenderDiff::SetVoxelObjectFrame { handle, frame } => {
                self.set_voxel_object_frame(*handle, *frame)?
            }
            RenderDiff::DefineVoxelObject { asset } => {
                self.define_voxel_object(asset, resources)?
            }
            RenderDiff::ReleaseVoxelObject { asset } => self.release_voxel_object(asset),
            RenderDiff::DefineAnimatedMesh { asset } => {
                self.define_animated_mesh(asset, resources)?
            }
            RenderDiff::ReleaseAnimatedMesh { asset } => self.release_animated_mesh(asset),
            RenderDiff::SetAnimatedMeshInspection { handle, inspection } => {
                if let NodeKind::AnimatedMesh(descriptor) = &mut self.node_mut(*handle)?.kind {
                    descriptor.inspection = inspection.clone();
                }
                self.set_animated_inspection(*handle, inspection)?
            }
            RenderDiff::SetAnimatedMeshPlayback { handle, playback } => {
                self.set_animated_playback(*handle, playback)?
            }
        }
        Ok(())
    }

    fn node_mut(&mut self, handle: RenderHandle) -> Result<&mut NodeRow, String> {
        self.tables
            .nodes
            .get_mut(&handle)
            .ok_or_else(|| format!("unknown node {}", handle.raw()))
    }

    fn insert_node(
        &mut self,
        handle: RenderHandle,
        parent: Option<RenderHandle>,
        local: Mat4,
        visible: bool,
        layer: RenderLayer,
        kind: NodeKind,
    ) {
        if self.tables.nodes.contains_key(&handle) {
            self.destroy_node(handle);
        }
        if let NodeKind::Primitive {
            geometry: Geometry::Line { a, b },
            ..
        } = &kind
        {
            let line = crate::primitives::line(*a, *b);
            let mesh = self.upload_vertices(
                &format!("line {}", handle.raw()),
                &line.vertices,
                &line.indices,
                Topology::Lines,
                vec![(0, 0, 2)],
                BTreeMap::new(),
            );
            self.tables.payload_meshes.insert(handle, mesh);
        }
        if let Some(parent) = parent.and_then(|parent| self.tables.nodes.get_mut(&parent)) {
            parent.children.push(handle);
        }
        if matches!(kind, NodeKind::Sprite(_)) {
            self.tables.sprites.insert(handle);
        } else {
            self.tables.sprites.remove(&handle);
        }
        self.tables.nodes.insert(
            handle,
            NodeRow {
                parent,
                parent_joint: None,
                parent_joint_node: None,
                children: Vec::new(),
                local,
                world: local,
                visible,
                world_visible: visible,
                layer,
                world_layer: layer,
                kind,
                parts: Vec::new(),
            },
        );
        self.tables.dirty_nodes.insert(handle);
        self.rebuild_parts(handle);
    }

    /// Destroy a node and its whole subtree, as `PresentationWorld` does.
    fn destroy_node(&mut self, handle: RenderHandle) {
        let Some(node) = self.tables.nodes.remove(&handle) else {
            return;
        };
        if let Some(parent) = node
            .parent
            .and_then(|parent| self.tables.nodes.get_mut(&parent))
        {
            parent.children.retain(|child| *child != handle);
        }
        let mut stack = vec![node];
        let mut removed = vec![handle];
        while let Some(node) = stack.pop() {
            for part in node.parts {
                self.tables.parts.remove(part);
            }
            if matches!(node.kind, NodeKind::Light(_)) {
                self.tables.lights_dirty = true;
            }
            for child in node.children {
                if let Some(child_node) = self.tables.nodes.remove(&child) {
                    stack.push(child_node);
                    removed.push(child);
                }
            }
        }
        for handle in removed {
            self.remove_animated_instance(handle);
            self.tables.payload_meshes.remove(&handle);
            self.tables.lights.remove(&handle);
            self.tables.sprites.remove(&handle);
            self.tables.dirty_nodes.remove(&handle);
        }
    }

    /// Re-derive a node's drawable parts from its kind and bindings.
    pub(crate) fn rebuild_parts(&mut self, handle: RenderHandle) {
        let animated = self
            .tables
            .nodes
            .get(&handle)
            .is_some_and(|node| matches!(node.kind, NodeKind::AnimatedMesh(_)));
        let animated_parts = if animated {
            self.animated_parts(handle)
        } else {
            Vec::new()
        };
        let Some(node) = self.tables.nodes.get_mut(&handle) else {
            return;
        };
        for part in std::mem::take(&mut node.parts) {
            self.tables.parts.remove(part);
        }
        let mut parts: Vec<(Part, PartRow)> = Vec::new();
        match &node.kind {
            NodeKind::Primitive {
                geometry,
                material,
                has_payload,
            } => {
                // `wireframe` draws a primitive's triangle edges, unlit;
                // lines and points ignore it.
                let unlit = |mesh, first_index, index_count, wireframe| {
                    (
                        Part {
                            node: handle,
                            mesh,
                            first_index,
                            index_count,
                            material: MaterialRef::Unlit,
                            wireframe,
                        },
                        PartRow::new(material.color, [0.0; 3]),
                    )
                };
                if *has_payload {
                    if let Some(mesh) = self.tables.payload_meshes.get(&handle) {
                        for (slot, start, count) in &mesh.groups {
                            let slot_material = format!("{PAYLOAD_SLOT_MATERIAL_PREFIX}{slot}");
                            let (material_ref, color, emission) = match crate::tables::named(
                                &self.tables.names,
                                &self.tables.materials,
                                &slot_material,
                            ) {
                                Some((id, row)) => (
                                    MaterialRef::Retained(id),
                                    mul(row.descriptor.color, row.descriptor.texture_tint),
                                    emission(
                                        row.descriptor.emission_color,
                                        row.descriptor.emission_intensity,
                                    ),
                                ),
                                None => (MaterialRef::LitFallback, slot_color(*slot), [0.0; 3]),
                            };
                            parts.push((
                                Part {
                                    node: handle,
                                    mesh: MeshRef::Payload(handle),
                                    first_index: *start,
                                    index_count: *count,
                                    material: material_ref,
                                    // The node's view material, wireframe
                                    // included, applies to uploaded meshes.
                                    wireframe: material.wireframe,
                                },
                                PartRow::new(mul(color, material.color), emission),
                            ));
                        }
                    }
                } else {
                    let wireframe = material.wireframe;
                    match geometry {
                        Geometry::Group => {}
                        Geometry::Line { .. } => {
                            parts.push(unlit(MeshRef::Payload(handle), 0, 2, false))
                        }
                        Geometry::Cube => {
                            parts.push(unlit(MeshRef::Builtin(Builtin::Cube), 0, 0, wireframe))
                        }
                        Geometry::Sphere => {
                            parts.push(unlit(MeshRef::Builtin(Builtin::Sphere), 0, 0, wireframe))
                        }
                        Geometry::Quad => {
                            parts.push(unlit(MeshRef::Builtin(Builtin::Quad), 0, 0, wireframe))
                        }
                        Geometry::Point => {
                            parts.push(unlit(MeshRef::Builtin(Builtin::Point), 0, 0, false))
                        }
                    }
                }
            }
            NodeKind::StaticMesh {
                asset,
                overrides,
                parameters,
            } => {
                if let Some((mesh_id, mesh)) =
                    crate::tables::named(&self.tables.names, &self.tables.static_meshes, asset)
                {
                    for (slot, start, count) in &mesh.groups {
                        let material_id = overrides.get(slot).or_else(|| mesh.slots.get(slot));
                        let (material_ref, color, emission) = match material_id.and_then(|id| {
                            crate::tables::named(&self.tables.names, &self.tables.materials, id)
                        }) {
                            Some((id, row)) => {
                                let descriptor = &row.descriptor;
                                let override_ = parameters.get(slot);
                                let tint =
                                    override_.map_or(descriptor.texture_tint, |p| p.texture_tint);
                                let (emission_color, intensity) = override_.map_or(
                                    (descriptor.emission_color, descriptor.emission_intensity),
                                    |p| (p.emission_color, p.emission_intensity),
                                );
                                (
                                    MaterialRef::Retained(id),
                                    mul(descriptor.color, tint),
                                    emission(emission_color, intensity),
                                )
                            }
                            None => (MaterialRef::LitFallback, slot_color(*slot), [0.0; 3]),
                        };
                        parts.push((
                            Part {
                                node: handle,
                                mesh: MeshRef::Static(mesh_id),
                                first_index: *start,
                                index_count: *count,
                                material: material_ref,
                                wireframe: false,
                            },
                            PartRow::new(color, emission),
                        ));
                    }
                }
            }
            NodeKind::VoxelObject(instance) => {
                if let Some((object_id, object)) = crate::tables::named(
                    &self.tables.names,
                    &self.tables.voxel_objects,
                    &instance.asset,
                ) {
                    let index = object.frame_mesh(instance.frame);
                    if let Some(mesh) = object.meshes.get(index as usize) {
                        for (slot, start, count) in &mesh.groups {
                            let material_id = instance
                                .material_overrides
                                .iter()
                                .find(|binding| binding.slot == *slot)
                                .map(|binding| &binding.material)
                                .or_else(|| object.slots.get(slot));
                            let (material_ref, color, emission) = match material_id.and_then(|id| {
                                crate::tables::named(&self.tables.names, &self.tables.materials, id)
                            }) {
                                Some((id, row)) => (
                                    MaterialRef::Retained(id),
                                    mul(row.descriptor.color, row.descriptor.texture_tint),
                                    emission(
                                        row.descriptor.emission_color,
                                        row.descriptor.emission_intensity,
                                    ),
                                ),
                                None => (MaterialRef::LitFallback, slot_color(*slot), [0.0; 3]),
                            };
                            parts.push((
                                Part {
                                    node: handle,
                                    mesh: MeshRef::Voxel(object_id, index),
                                    first_index: *start,
                                    index_count: *count,
                                    material: material_ref,
                                    wireframe: false,
                                },
                                PartRow::new(color, emission),
                            ));
                        }
                    }
                }
            }
            NodeKind::AnimatedMesh(_) => {
                // Inspection draws the instance as a wireframe.
                let wireframe = self
                    .tables
                    .animated
                    .get(&handle)
                    .is_some_and(|instance| instance.inspection.wireframe);
                for (mesh, index_count, material) in animated_parts {
                    let (color, emission) = match &material {
                        MaterialRef::Retained(id) => match self.tables.materials.get(*id) {
                            Some(row) => (
                                mul(row.descriptor.color, row.descriptor.texture_tint),
                                emission(
                                    row.descriptor.emission_color,
                                    row.descriptor.emission_intensity,
                                ),
                            ),
                            None => ([1.0; 4], [0.0; 3]),
                        },
                        _ => ([1.0; 4], [0.0; 3]),
                    };
                    parts.push((
                        Part {
                            node: handle,
                            mesh,
                            first_index: 0,
                            index_count,
                            material,
                            wireframe,
                        },
                        PartRow::new(color, emission),
                    ));
                }
            }
            NodeKind::Group | NodeKind::Light(_) | NodeKind::Sprite(_) => {}
        }
        let (world, shown, layer) = (node.world, node.world_visible, node.world_layer);
        let mut ids = Vec::with_capacity(parts.len());
        for (mut part, mut row) in parts {
            if let MeshRef::Builtin(kind) = part.mesh {
                part.index_count = self.builtins[&kind].groups[0].2;
            }
            let mesh = self.mesh(&part.mesh);
            if let Some(space) = mesh.and_then(|mesh| mesh.texture_space) {
                row.texture_space = [
                    space.origin[0],
                    space.origin[1],
                    space.origin[2],
                    1.0 / space.cell_size,
                ];
            }
            let bounds = mesh.map_or(Aabb::EMPTY, |mesh| mesh.bounds);
            if let (true, Some(mesh)) = (part.wireframe, mesh) {
                mesh.edges
                    .get_or_init(|| edge_buffer(&self.gpu.device, &mesh.cpu.indices));
            }
            let (descriptor, features) = match &part.material {
                MaterialRef::Retained(id) => {
                    let row = self.tables.materials.get(*id);
                    (
                        row.map(|row| &row.descriptor),
                        row.map_or(Features::default(), |row| row.features),
                    )
                }
                MaterialRef::Unlit => (None, Features::UNLIT),
                MaterialRef::LitFallback => (None, Features::default()),
            };
            let features = features | mesh.map_or(Features::default(), |mesh| mesh.features());
            let class = PartClass {
                blend: row.color[3] < 1.0 || descriptor.is_some_and(blends),
                double_sided: descriptor.is_some_and(|descriptor| descriptor.double_sided),
                lines: part.wireframe || mesh.is_some_and(|mesh| mesh.topology == Topology::Lines),
                features,
            };
            let id = self.tables.parts.insert(part, row, bounds, class);
            self.tables.parts.write(id, &world, shown, layer);
            ids.push(id);
        }
        if let Some(node) = self.tables.nodes.get_mut(&handle) {
            node.parts = ids;
        }
        if animated {
            // Rigid GLB nodes draw at their pose under the instance.
            self.write_animated_parts(handle);
        }
    }

    /// The uploaded mesh a part draws, if it is (still) defined.
    pub(crate) fn mesh(&self, mesh: &MeshRef) -> Option<&GpuMesh> {
        match mesh {
            MeshRef::Static(asset) => self.tables.static_meshes.get(*asset),
            MeshRef::Payload(handle) => self.tables.payload_meshes.get(handle),
            MeshRef::Builtin(kind) => self.builtins.get(kind),
            MeshRef::Voxel(asset, index) => self
                .tables
                .voxel_objects
                .get(*asset)
                .and_then(|object| object.meshes.get(*index as usize)),
            MeshRef::AnimatedRigid(..) | MeshRef::AnimatedSkinned(..) => self.animated_mesh(mesh),
        }
    }

    fn define_texture(
        &mut self,
        texture: &TextureDescriptor,
        resources: &dyn ResourceSource,
    ) -> Result<(), String> {
        let image = resources::texture_image(texture, resources)?;
        let uploaded = self.upload_texture(texture, image.as_ref());
        self.tables.textures.insert(texture.id.clone(), uploaded);
        if let Some(id) = self.tables.names.get(&texture.id) {
            self.effects.forget_texture(id);
        }
        // Materials sampling this texture and a sky showing it bind the new view.
        let dependents: Vec<RenderMaterialDescriptor> = self
            .tables
            .materials
            .values()
            .filter(|row| row.descriptor.textures().any(|used| *used == texture.id))
            .map(|row| row.descriptor.clone())
            .collect();
        for descriptor in dependents {
            // A product shader's error was reported when it was defined.
            let _ = self.define_material(descriptor);
        }
        self.tables.environment_dirty = true;
        match image {
            Some(_) => Ok(()),
            None => Err(format!(
                "{} has no payload; drawn with the colour fallback",
                texture.id
            )),
        }
    }

    /// Upload decoded pixels with the descriptor's colour space and sampler
    /// modes; a metadata-only descriptor becomes a 1×1 white texture.
    fn upload_texture(
        &self,
        descriptor: &TextureDescriptor,
        image: Option<&resources::DecodedImage>,
    ) -> GpuTexture {
        let label = descriptor.id.as_str();
        let srgb = descriptor
            .payload
            .as_ref()
            .is_none_or(|payload| payload.color_space == TextureColorSpace::Srgb);
        let (width, height, pixels) = match image {
            Some(image) => (image.width, image.height, image.rgba.as_slice()),
            None => (1, 1, &[255u8; 4][..]),
        };
        upload_rgba_texture(
            &self.gpu,
            label,
            width,
            height,
            pixels,
            srgb,
            descriptor.filter == TextureFilter::Nearest,
            descriptor.wrap == TextureWrap::Repeat,
            descriptor.filter == TextureFilter::Linear,
        )
    }

    fn define_material(&mut self, descriptor: RenderMaterialDescriptor) -> Result<(), String> {
        let texture = descriptor
            .texture
            .as_ref()
            .and_then(|id| self.tables.textures.get(id));
        let params = MaterialParams::of(&descriptor, texture.map(|texture| texture.size));
        self.insert_material(descriptor, &params)
    }

    /// Hold a product shader, and redefine the materials it shades so they
    /// compile it. Its error, if it does not compose, names its file and
    /// line; those materials draw with the standard shade stage.
    fn define_shader(&mut self, shader: &render_model::ShaderDescriptor) -> Result<(), String> {
        let product = self.layouts.shaders.product(crate::shaders::ProductShader {
            path: shader.path.clone(),
            source: shader.source.clone(),
            keywords: shader.keywords.clone(),
        });
        self.tables
            .shaders
            .insert(shader.id.clone(), (shader.clone(), product));
        let dependents: Vec<RenderMaterialDescriptor> = self
            .tables
            .materials
            .values()
            .filter(|row| {
                row.descriptor
                    .shader
                    .as_ref()
                    .is_some_and(|used| used.shader == shader.id)
            })
            .map(|row| row.descriptor.clone())
            .collect();
        let mut errors: Vec<String> = Vec::new();
        for descriptor in dependents {
            if let Err(error) = self.define_material(descriptor) {
                if !errors.contains(&error) {
                    errors.push(error);
                }
            }
        }
        match errors.is_empty() {
            true => Ok(()),
            false => Err(errors.join("\n")),
        }
    }

    /// Store a material row with `params` and re-derive the parts drawing it.
    /// A product shader that is not defined, or does not compose, is an
    /// error; the material is stored and draws with the standard shade stage.
    pub(crate) fn insert_material(
        &mut self,
        descriptor: RenderMaterialDescriptor,
        params: &MaterialParams,
    ) -> Result<(), String> {
        let texture = descriptor
            .texture
            .as_ref()
            .and_then(|id| self.tables.textures.get(id));
        let (bind_group, features) =
            self.material_bind_group(&descriptor.id, params, texture.unwrap_or(&self.white));
        let mut error = None;
        let product = match &descriptor.shader {
            Some(used) => match self.tables.shaders.get(&used.shader) {
                Some((_, product)) => *product,
                None => {
                    error = Some(format!("{} is not defined", used.shader));
                    0
                }
            },
            None => 0,
        };
        let mut features = features.with_product(product);
        self.prepare_material(features, blends(&descriptor), descriptor.double_sided);
        let composed = self.layouts.shader_errors.drain(..).next();
        if let Some(composed) = composed {
            error = Some(composed);
            features = features.with_product(0);
            self.prepare_material(features, blends(&descriptor), descriptor.double_sided);
        }
        let id = self.tables.names.id(&descriptor.id);
        self.tables.materials.insert(
            id,
            MaterialRow {
                descriptor,
                bind_group,
                features,
                maps: params.maps.clone(),
            },
        );
        // Parts bound to this material carry its colour and emission in their
        // rows; re-derive the nodes that draw it.
        let dependents: Vec<RenderHandle> = self
            .tables
            .parts
            .meta
            .iter()
            .flatten()
            .filter(|part| {
                part.material == MaterialRef::Retained(id)
                    || matches!(part.material, MaterialRef::LitFallback)
            })
            .map(|part| part.node)
            .collect();
        for handle in dedup(dependents) {
            self.rebuild_parts(handle);
        }
        error.map_or(Ok(()), Err)
    }

    /// Make a material's pipelines on every target so far and for the shadow
    /// casters, so its first draw does not compile them.
    pub(crate) fn prepare_material(&mut self, features: Features, blend: bool, double_sided: bool) {
        let passes: &[Pass] = match (blend, double_sided) {
            (false, false) => &[Pass::Opaque, Pass::OpaqueMirrored],
            (false, true) => &[Pass::OpaqueDoubleSided],
            (true, false) => &[Pass::Blend, Pass::BlendMirrored],
            (true, true) => &[Pass::BlendDoubleSided],
        };
        let device = &self.gpu.device;
        // Compiled even before any target exists, so a product shader's
        // error is known when its material is defined.
        self.layouts.world_shader(device, features);
        for &pass in passes {
            for pipelines in &mut self.pipelines {
                self.layouts.prepare(device, pipelines, features, pass);
            }
            if self.options.shadows {
                self.layouts.prepare_caster(device, features, pass);
            }
        }
    }

    /// A material's bind group and the features its variant compiles, its
    /// maps resolved to retained textures. A map whose texture is not
    /// retained is left out: it binds white and its feature is off.
    pub(crate) fn material_bind_group(
        &self,
        label: &str,
        params: &MaterialParams,
        texture: &GpuTexture,
    ) -> (wgpu::BindGroup, Features) {
        let retained = |map: &MapSlot| self.tables.textures.contains_key(&map.texture);
        let params = MaterialParams {
            maps: MaterialMaps {
                base: params.maps.base,
                base_tex_coord: params.maps.base_tex_coord,
                emissive: params.maps.emissive.clone().filter(retained),
                normal: params.maps.normal.clone().filter(|(map, _)| retained(map)),
                occlusion: params
                    .maps
                    .occlusion
                    .clone()
                    .filter(|(map, _)| retained(map)),
            },
            voxel_surface: params.voxel_surface,
            product_textures: params.product_textures.clone(),
            ..*params
        };
        let lookup = |map: Option<&MapSlot>| {
            map.and_then(|map| self.tables.textures.get(&map.texture))
                .unwrap_or(&self.white)
        };
        let bind_group = material_bind_group(
            &self.gpu.device,
            &self.layouts.material,
            label,
            &params,
            texture,
            &MapTextures {
                emissive: lookup(params.maps.emissive.as_ref()),
                normal: lookup(params.maps.normal.as_ref().map(|(map, _)| map)),
                occlusion: lookup(params.maps.occlusion.as_ref().map(|(map, _)| map)),
                product: params.product_textures.each_ref().map(|texture| {
                    texture
                        .as_ref()
                        .and_then(|id| self.tables.textures.get(id))
                        .unwrap_or(&self.white)
                }),
            },
        );
        (bind_group, params.features())
    }

    fn define_static_mesh(
        &mut self,
        asset: &StaticMeshAsset,
        resources: &dyn ResourceSource,
    ) -> Result<(), String> {
        let streams = resources::mesh_streams(&asset.payload, resources)?;
        let mesh = self.upload_mesh(
            &asset.asset,
            &streams,
            Topology::Triangles,
            asset
                .payload
                .groups
                .iter()
                .map(|group| (group.material_slot, group.start, group.count))
                .collect(),
            asset
                .material_slots
                .iter()
                .map(|slot| (slot.slot, slot.material.clone()))
                .collect(),
        );
        let id = self.tables.names.id(&asset.asset);
        let redefined = self.tables.static_meshes.insert(id, mesh).is_some();
        if redefined {
            let instances: Vec<RenderHandle> = self
                .tables
                .nodes
                .iter()
                .filter(|(_, node)| matches!(&node.kind, NodeKind::StaticMesh { asset: used, .. } if *used == asset.asset))
                .map(|(handle, _)| *handle)
                .collect();
            for handle in instances {
                self.rebuild_parts(handle);
            }
        }
        Ok(())
    }

    pub(crate) fn upload_mesh(
        &self,
        label: &str,
        streams: &resources::MeshStreams,
        topology: Topology,
        groups: Vec<(u16, u32, u32)>,
        slots: BTreeMap<u16, String>,
    ) -> GpuMesh {
        let vertex_count = streams.positions.len() / 3;
        let mut vertices = Vec::with_capacity(vertex_count * VERTEX_FLOATS);
        for vertex in 0..vertex_count {
            vertices.extend_from_slice(&streams.positions[vertex * 3..vertex * 3 + 3]);
            match streams.normals.get(vertex * 3..vertex * 3 + 3) {
                Some(normal) => vertices.extend_from_slice(normal),
                None => vertices.extend_from_slice(&[0.0, 1.0, 0.0]),
            }
            match streams
                .uvs
                .as_ref()
                .and_then(|uvs| uvs.get(vertex * 2..vertex * 2 + 2))
            {
                Some(uv) => vertices.extend_from_slice(uv),
                None => vertices.extend_from_slice(&[0.0, 0.0]),
            }
            match streams
                .colors
                .as_ref()
                .and_then(|colors| colors.get(vertex * 4..vertex * 4 + 4))
            {
                Some(color) => vertices.extend_from_slice(color),
                None => vertices.extend_from_slice(&[1.0; 4]),
            }
        }
        self.upload_vertices(label, &vertices, &streams.indices, topology, groups, slots)
    }

    pub(crate) fn upload_vertices(
        &self,
        label: &str,
        vertices: &[f32],
        indices: &[u32],
        topology: Topology,
        groups: Vec<(u16, u32, u32)>,
        slots: BTreeMap<u16, String>,
    ) -> GpuMesh {
        let device = &self.gpu.device;
        let mut bounds = Aabb::EMPTY;
        let mut positions = Vec::with_capacity(vertices.len() / VERTEX_FLOATS);
        for vertex in vertices.as_chunks::<VERTEX_FLOATS>().0 {
            let position = Vec3::new(vertex[0], vertex[1], vertex[2]);
            bounds.include(position);
            positions.push(position);
        }
        GpuMesh {
            bounds,
            cpu: std::sync::Arc::new(crate::tables::CpuGeometry {
                positions,
                indices: indices.to_vec(),
            }),
            edges: Default::default(),
            extra: None,
            texture_space: None,
            vertices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: bytemuck::cast_slice(vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }),
            indices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: bytemuck::cast_slice(indices),
                usage: wgpu::BufferUsages::INDEX,
            }),
            topology,
            groups,
            slots,
        }
    }
}

/// Upload RGBA8 pixels (sRGB or linear) with nearest or linear filtering and
/// repeat or clamp wrapping. With `mipmaps`, the full mip chain is built for
/// material sampling (trilinear); every other user samples the base level.
#[allow(clippy::too_many_arguments)]
pub(crate) fn upload_rgba_texture(
    gpu: &crate::Gpu,
    label: &str,
    width: u32,
    height: u32,
    pixels: &[u8],
    srgb: bool,
    nearest: bool,
    repeat: bool,
    mipmaps: bool,
) -> GpuTexture {
    let chain;
    let (levels, pixels) = if mipmaps && width.max(height) > 1 {
        chain = resources::mip_chain(width, height, pixels, srgb);
        (chain.0, chain.1.as_slice())
    } else {
        (1, pixels)
    };
    let texture = gpu.device.create_texture_with_data(
        &gpu.queue,
        &wgpu::TextureDescriptor {
            label: Some(label),
            size: crate::target::extent(width, height),
            mip_level_count: levels,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: if srgb {
                wgpu::TextureFormat::Rgba8UnormSrgb
            } else {
                wgpu::TextureFormat::Rgba8Unorm
            },
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        },
        wgpu::util::TextureDataOrder::LayerMajor,
        pixels,
    );
    let filter = if nearest {
        wgpu::FilterMode::Nearest
    } else {
        wgpu::FilterMode::Linear
    };
    let address = if repeat {
        wgpu::AddressMode::Repeat
    } else {
        wgpu::AddressMode::ClampToEdge
    };
    let sampler = |mipmap_filter, anisotropy_clamp| {
        gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some(label),
            address_mode_u: address,
            address_mode_v: address,
            mag_filter: filter,
            min_filter: filter,
            mipmap_filter,
            anisotropy_clamp,
            ..Default::default()
        })
    };
    GpuTexture {
        size: (width, height),
        view: texture.create_view(&wgpu::TextureViewDescriptor {
            mip_level_count: Some(1),
            ..Default::default()
        }),
        sampler: sampler(wgpu::MipmapFilterMode::Nearest, 1),
        mipped: (levels > 1).then(|| {
            (
                texture.create_view(&Default::default()),
                // Mipmapped textures are linear-filtered, which anisotropic
                // filtering requires; it keeps grazing surfaces sharp.
                sampler(wgpu::MipmapFilterMode::Linear, MATERIAL_ANISOTROPY),
            )
        }),
    }
}

/// What a material bind group's uniform carries.
pub(crate) struct MaterialParams {
    pub roughness: f32,
    /// Alpha-masked below this cutoff.
    pub alpha_cutoff: Option<f32>,
    pub unlit: bool,
    pub metalness: f32,
    pub voxel_surface: Option<VoxelSurfaceUniform>,
    /// Triplanar blend sharpness.
    pub triplanar: Option<f32>,
    /// A product shader's parameters (`material.parameters`).
    pub parameters: [[f32; 4]; 4],
    /// A product shader's own textures (`product_map_a`, `product_map_b`).
    pub product_textures: [Option<String>; 2],
    pub maps: MaterialMaps,
}

/// A GLB material's texture maps beyond its base colour, and each slot's uv
/// transform. Engine materials have none.
#[derive(Clone, Default)]
pub(crate) struct MaterialMaps {
    pub base: Option<UvTransform>,
    /// The uv set (0 or 1) the base colour texture reads.
    pub base_tex_coord: u32,
    /// Multiplies the part's emission.
    pub emissive: Option<MapSlot>,
    /// With the normal scale.
    pub normal: Option<(MapSlot, f32)>,
    /// With the occlusion strength.
    pub occlusion: Option<(MapSlot, f32)>,
}

/// A map's retained texture (linear for normal and occlusion data) and uv
/// transform.
#[derive(Clone)]
pub(crate) struct MapSlot {
    pub texture: String,
    pub transform: UvTransform,
    /// The uv set (0 or 1) it reads.
    pub tex_coord: u32,
}

impl MaterialParams {
    /// A retained material's uniform. A voxel surface's own alpha policy
    /// applies to it.
    pub(crate) fn of(
        descriptor: &RenderMaterialDescriptor,
        texture_size: Option<(u32, u32)>,
    ) -> Self {
        let cutoff = match (&descriptor.voxel_surface, descriptor.alpha_mode) {
            (Some(surface), _) => match surface.alpha_mode {
                VoxelSurfaceAlphaModeDescriptor::Mask { cutoff } => Some(cutoff),
                _ => None,
            },
            (None, MaterialAlphaModeDescriptor::Mask { cutoff }) => Some(cutoff),
            (None, _) => None,
        };
        let voxel_surface = descriptor
            .voxel_surface
            .as_ref()
            .map(|surface| VoxelSurfaceUniform::resolve(surface, texture_size));
        Self {
            roughness: descriptor.roughness,
            alpha_cutoff: cutoff,
            unlit: false,
            metalness: descriptor.metalness,
            voxel_surface,
            triplanar: descriptor.triplanar.map(|triplanar| triplanar.sharpness),
            parameters: descriptor
                .shader
                .as_ref()
                .map_or([[0.0; 4]; 4], |shader| shader.parameters),
            product_textures: std::array::from_fn(|slot| {
                descriptor
                    .shader
                    .as_ref()
                    .and_then(|shader| shader.textures.get(slot).cloned())
            }),
            maps: MaterialMaps {
                normal: descriptor.normal_map.as_ref().map(|map| {
                    (
                        MapSlot {
                            texture: map.texture.clone(),
                            transform: UvTransform::IDENTITY,
                            tex_coord: 0,
                        },
                        map.scale,
                    )
                }),
                ..MaterialMaps::default()
            },
        }
    }

    /// The standard shader features the material compiles in. An unlit
    /// material reads no maps beyond its base colour.
    pub(crate) fn features(&self) -> Features {
        let base = Features::default()
            .with(Features::MASK, self.alpha_cutoff.is_some())
            .with(Features::VOXEL_SURFACE, self.voxel_surface.is_some())
            .with(Features::TRIPLANAR, self.triplanar.is_some());
        if self.unlit {
            return base | Features::UNLIT;
        }
        base.with(Features::NORMAL_MAP, self.maps.normal.is_some())
            .with(Features::EMISSIVE_MAP, self.maps.emissive.is_some())
            .with(Features::OCCLUSION_MAP, self.maps.occlusion.is_some())
    }
}

/// Whether a retained material draws alpha-blended.
pub(crate) fn blends(descriptor: &RenderMaterialDescriptor) -> bool {
    match &descriptor.voxel_surface {
        Some(surface) => surface.alpha_mode == VoxelSurfaceAlphaModeDescriptor::Blend,
        None => descriptor.alpha_mode == MaterialAlphaModeDescriptor::Blend,
    }
}

/// The textures a material binds after its albedo: emissive, normal and
/// occlusion maps, and a product shader's two maps (white where the material
/// has none).
pub(crate) struct MapTextures<'a> {
    pub emissive: &'a GpuTexture,
    pub normal: &'a GpuTexture,
    pub occlusion: &'a GpuTexture,
    pub product: [&'a GpuTexture; 2],
}

pub(crate) fn material_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    label: &str,
    params: &MaterialParams,
    texture: &GpuTexture,
    maps: &MapTextures<'_>,
) -> wgpu::BindGroup {
    let mut floats = [0f32; MATERIAL_UNIFORM_BYTES / 4];
    floats[0] = params.roughness;
    floats[1] = params.alpha_cutoff.unwrap_or(0.0);
    floats[2] = params.metalness;
    floats[3] = params.maps.normal.as_ref().map_or(1.0, |(_, scale)| *scale);
    if let Some(surface) = &params.voxel_surface {
        floats[4..6].copy_from_slice(&surface.tile_scale);
        floats[6..8].copy_from_slice(&surface.tile_origin);
        floats[8..10].copy_from_slice(&surface.uv_min);
        floats[10..12].copy_from_slice(&surface.uv_max);
    }
    let transforms = [
        params.maps.base,
        params.maps.emissive.as_ref().map(|map| map.transform),
        params.maps.normal.as_ref().map(|(map, _)| map.transform),
        params.maps.occlusion.as_ref().map(|(map, _)| map.transform),
    ];
    for (slot, transform) in transforms.into_iter().enumerate() {
        let [a, b, c, d, e, f] = transform.unwrap_or(UvTransform::IDENTITY).0;
        let start = 12 + slot * 8;
        floats[start..start + 3].copy_from_slice(&[a, b, c]);
        floats[start + 4..start + 7].copy_from_slice(&[d, e, f]);
    }
    let sets = [
        params.maps.base_tex_coord,
        params.maps.emissive.as_ref().map_or(0, |map| map.tex_coord),
        params
            .maps
            .normal
            .as_ref()
            .map_or(0, |(map, _)| map.tex_coord),
        params
            .maps
            .occlusion
            .as_ref()
            .map_or(0, |(map, _)| map.tex_coord),
    ];
    for (slot, set) in sets.into_iter().enumerate() {
        floats[48 + slot] = f32::from_bits(set);
    }
    floats[44] = params
        .maps
        .occlusion
        .as_ref()
        .map_or(0.0, |(_, strength)| *strength);
    floats[45] = params.triplanar.unwrap_or(1.0);
    for (row, values) in params.parameters.iter().enumerate() {
        floats[52 + row * 4..56 + row * 4].copy_from_slice(values);
    }
    let uniform: &[u8] = bytemuck::cast_slice(&floats);
    let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: uniform,
        usage: wgpu::BufferUsages::UNIFORM,
    });
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(texture.material_binding().0),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(texture.material_binding().1),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(maps.emissive.material_binding().0),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::Sampler(maps.emissive.material_binding().1),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: wgpu::BindingResource::TextureView(maps.normal.material_binding().0),
            },
            wgpu::BindGroupEntry {
                binding: 6,
                resource: wgpu::BindingResource::Sampler(maps.normal.material_binding().1),
            },
            wgpu::BindGroupEntry {
                binding: 7,
                resource: wgpu::BindingResource::TextureView(maps.occlusion.material_binding().0),
            },
            wgpu::BindGroupEntry {
                binding: 8,
                resource: wgpu::BindingResource::Sampler(maps.occlusion.material_binding().1),
            },
            wgpu::BindGroupEntry {
                binding: 9,
                resource: wgpu::BindingResource::TextureView(maps.product[0].material_binding().0),
            },
            wgpu::BindGroupEntry {
                binding: 10,
                resource: wgpu::BindingResource::Sampler(maps.product[0].material_binding().1),
            },
            wgpu::BindGroupEntry {
                binding: 11,
                resource: wgpu::BindingResource::TextureView(maps.product[1].material_binding().0),
            },
            wgpu::BindGroupEntry {
                binding: 12,
                resource: wgpu::BindingResource::Sampler(maps.product[1].material_binding().1),
            },
        ],
    })
}

/// The unlit (primitive) and lit fallback material bind groups.
pub(crate) fn builtin_materials(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    white: &GpuTexture,
) -> (wgpu::BindGroup, wgpu::BindGroup) {
    (
        material_bind_group(
            device,
            layout,
            "render-wgpu unlit",
            &MaterialParams {
                roughness: 1.0,
                alpha_cutoff: None,
                unlit: true,
                metalness: 0.0,
                voxel_surface: None,
                triplanar: None,
                parameters: [[0.0; 4]; 4],
                product_textures: [None, None],
                maps: MaterialMaps::default(),
            },
            white,
            &MapTextures {
                emissive: white,
                normal: white,
                occlusion: white,
                product: [white; 2],
            },
        ),
        material_bind_group(
            device,
            layout,
            "render-wgpu lit fallback",
            &MaterialParams {
                roughness: FALLBACK_ROUGHNESS,
                alpha_cutoff: None,
                unlit: false,
                metalness: 0.0,
                voxel_surface: None,
                triplanar: None,
                parameters: [[0.0; 4]; 4],
                product_textures: [None, None],
                maps: MaterialMaps::default(),
            },
            white,
            &MapTextures {
                emissive: white,
                normal: white,
                occlusion: white,
                product: [white; 2],
            },
        ),
    )
}

/// World-space light rows for the shader, in `rusty::types`' `Light` layout.
pub(crate) fn light_row(light: &LightDescriptor, world: &Mat4) -> Option<[f32; 16]> {
    let scaled = |color: [f32; 3], intensity: f32| color.map(|c| c * intensity);
    let rotate = |direction: [f32; 3]| {
        world
            .transform_vector3(crate::convert::vec3(direction))
            .normalize_or_zero()
    };
    let place = |position: [f32; 3]| world.transform_point3(crate::convert::vec3(position));
    let row = |kind: f32,
               color: [f32; 3],
               position: Vec3,
               range: f32,
               direction: Vec3,
               decay: f32,
               extra: [f32; 4]| {
        [
            color[0],
            color[1],
            color[2],
            kind,
            position.x,
            position.y,
            position.z,
            range,
            direction.x,
            direction.y,
            direction.z,
            decay,
            extra[0],
            extra[1],
            extra[2],
            extra[3],
        ]
    };
    match light {
        LightDescriptor::Ambient { enabled: false, .. }
        | LightDescriptor::Directional { enabled: false, .. }
        | LightDescriptor::Point { enabled: false, .. }
        | LightDescriptor::Spot { enabled: false, .. } => None,
        LightDescriptor::Ambient {
            color, intensity, ..
        } => Some(row(
            0.0,
            scaled(*color, *intensity),
            Vec3::ZERO,
            0.0,
            Vec3::ZERO,
            0.0,
            [0.0; 4],
        )),
        LightDescriptor::Directional {
            color,
            intensity,
            direction,
            ..
        } => Some(row(
            2.0,
            scaled(*color, *intensity),
            Vec3::ZERO,
            0.0,
            rotate(*direction),
            0.0,
            [0.0; 4],
        )),
        LightDescriptor::Point {
            color,
            intensity,
            position,
            range,
            decay,
            ..
        } => Some(row(
            3.0,
            scaled(*color, *intensity),
            place(*position),
            range.unwrap_or(0.0),
            Vec3::ZERO,
            *decay,
            [0.0; 4],
        )),
        LightDescriptor::Spot {
            color,
            intensity,
            position,
            direction,
            range,
            decay,
            outer_angle_radians,
            penumbra,
            ..
        } => Some(row(
            4.0,
            scaled(*color, *intensity),
            place(*position),
            range.unwrap_or(0.0),
            rotate(*direction),
            *decay,
            [
                outer_angle_radians.cos(),
                (outer_angle_radians * (1.0 - penumbra)).cos(),
                0.0,
                0.0,
            ],
        )),
    }
}

/// A line-list index buffer with each triangle's three edges (a-b, b-c,
/// c-a). Edge `i` of a triangle range `[start, start + count)` sits at
/// `[start * 2, (start + count) * 2)`.
pub(crate) fn edge_buffer(device: &wgpu::Device, indices: &[u32]) -> wgpu::Buffer {
    let mut edges = Vec::with_capacity(indices.len() * 2);
    for triangle in indices.as_chunks::<3>().0 {
        edges.extend_from_slice(&[
            triangle[0],
            triangle[1],
            triangle[1],
            triangle[2],
            triangle[2],
            triangle[0],
        ]);
    }
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("render-wgpu wireframe edges"),
        contents: bytemuck::cast_slice(&edges),
        usage: wgpu::BufferUsages::INDEX,
    })
}

fn mul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    std::array::from_fn(|index| a[index] * b[index])
}

fn emission(color: [f32; 3], intensity: f32) -> [f32; 3] {
    color.map(|component| component * intensity)
}

/// The deterministic fallback hue for an unbound slot (golden angle,
/// HSL saturation 0.7, lightness 0.5), in linear RGB.
pub fn slot_color(slot: u16) -> [f32; 4] {
    let hue = (f32::from(slot) * 0.618_034) % 1.0;
    let (s, l) = (0.7_f32, 0.5_f32);
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let channel = |t: f32| {
        let t = t.rem_euclid(1.0);
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };
    let srgb = [
        channel(hue + 1.0 / 3.0),
        channel(hue),
        channel(hue - 1.0 / 3.0),
    ];
    let linear = srgb.map(crate::srgb_to_linear);
    [linear[0], linear[1], linear[2], 1.0]
}

fn dedup(mut handles: Vec<RenderHandle>) -> Vec<RenderHandle> {
    handles.sort();
    handles.dedup();
    handles
}

fn op_name(op: &RenderDiff) -> &'static str {
    match op {
        RenderDiff::SetParentJoint { .. } => "setParentJoint",
        RenderDiff::Create { .. } => "create",
        RenderDiff::Update { .. } => "update",
        RenderDiff::Destroy { .. } => "destroy",
        RenderDiff::ReplaceMeshPayload { .. } => "replaceMeshPayload",
        RenderDiff::CreateLight { .. } => "createLight",
        RenderDiff::UpdateLight { .. } => "updateLight",
        RenderDiff::DefineMaterial { .. } => "defineMaterial",
        RenderDiff::ReleaseMaterial { .. } => "releaseMaterial",
        RenderDiff::SetMaterialInstanceParameters { .. } => "setMaterialInstanceParameters",
        RenderDiff::DefineTexture { .. } => "defineTexture",
        RenderDiff::DefineShader { .. } => "defineShader",
        RenderDiff::ReleaseShader { .. } => "releaseShader",
        RenderDiff::ReleaseTexture { .. } => "releaseTexture",
        RenderDiff::SetSkyBackground { .. } => "setSkyBackground",
        RenderDiff::SetBackgroundColor { .. } => "setBackgroundColor",
        RenderDiff::SetFog { .. } => "setFog",
        RenderDiff::SetToneMapping { .. } => "setToneMapping",
        RenderDiff::DefineSpriteAtlas { .. } => "defineSpriteAtlas",
        RenderDiff::ReleaseSpriteAtlas { .. } => "releaseSpriteAtlas",
        RenderDiff::DefineStaticMesh { .. } => "defineStaticMesh",
        RenderDiff::ReleaseStaticMesh { .. } => "releaseStaticMesh",
        RenderDiff::DefineAnimatedMesh { .. } => "defineAnimatedMesh",
        RenderDiff::ReleaseAnimatedMesh { .. } => "releaseAnimatedMesh",
        RenderDiff::DefineVoxelObject { .. } => "defineVoxelObject",
        RenderDiff::ReleaseVoxelObject { .. } => "releaseVoxelObject",
        RenderDiff::CreateStaticMeshInstance { .. } => "createStaticMeshInstance",
        RenderDiff::CreateAnimatedMeshInstance { .. } => "createAnimatedMeshInstance",
        RenderDiff::SetAnimatedMeshInspection { .. } => "setAnimatedMeshInspection",
        RenderDiff::SetAnimatedMeshPlayback { .. } => "setAnimatedMeshPlayback",
        RenderDiff::CreateVoxelObjectInstance { .. } => "createVoxelObjectInstance",
        RenderDiff::SetVoxelObjectFrame { .. } => "setVoxelObjectFrame",
        RenderDiff::CreateSprite { .. } => "createSprite",
        RenderDiff::UpdateSprite { .. } => "updateSprite",
    }
}
