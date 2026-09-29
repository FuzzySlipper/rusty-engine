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

use crate::pipelines::VERTEX_FLOATS;
use crate::resources::{self, ResourceSource};
use crate::tables::{
    transform_matrix, Aabb, Builtin, Environment, GpuMesh, GpuTexture, MaterialRef, MaterialRow,
    MeshRef, NodeKind, NodeRow, Part, PartClass, PartRow, Topology,
};
use crate::voxel::VoxelSurfaceUniform;
use crate::Renderer;

/// An op the backend skipped, with the reason. Rendering continues without it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyIssue {
    pub op: &'static str,
    pub detail: String,
}

/// Material flags in `MaterialUniform.flags` (world.wgsl).
const FLAG_UNLIT: u32 = 1;
const FLAG_MASK: u32 = 2;
const FLAG_VOXEL_SURFACE: u32 = 4;
/// `MaterialUniform` size: roughness, cutoff, flags, pad, then the voxel
/// surface's tile scale, tile origin, and sample rect.
const MATERIAL_UNIFORM_BYTES: usize = 48;
/// Payload groups without a voxel material use Three's fallback roughness.
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
                self.effects.forget_texture(id);
            }
            RenderDiff::DefineMaterial { material } => self.define_material(material.clone()),
            RenderDiff::ReleaseMaterial { id } => {
                self.tables.materials.remove(id);
            }
            RenderDiff::DefineStaticMesh { asset } => self.define_static_mesh(asset, resources)?,
            RenderDiff::ReleaseStaticMesh { asset } => {
                self.tables.static_meshes.remove(asset);
            }
            RenderDiff::DefineSpriteAtlas { atlas } => {
                self.tables.atlases.insert(atlas.id.clone(), atlas.clone());
            }
            RenderDiff::ReleaseSpriteAtlas { id } => {
                self.tables.atlases.remove(id);
            }
            RenderDiff::SetBackgroundColor { color } => {
                self.tables.environment = Environment::Color(*color);
                self.tables.environment_dirty = true;
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
                    transform_matrix(&node.transform),
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
                transform_matrix(&instance.transform),
                instance.visible,
                RenderLayer::Scene,
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
            } => self.insert_node(
                *handle,
                *parent,
                transform_matrix(&instance.transform),
                instance.visible,
                RenderLayer::Scene,
                NodeKind::AnimatedMesh(Box::new(instance.clone())),
            ),
            RenderDiff::CreateVoxelObjectInstance {
                handle,
                parent,
                instance,
            } => self.insert_node(
                *handle,
                *parent,
                transform_matrix(&instance.transform),
                instance.visible,
                RenderLayer::Scene,
                NodeKind::VoxelObject(Box::new(instance.clone())),
            ),
            RenderDiff::CreateSprite {
                handle,
                parent,
                sprite,
            } => self.insert_node(
                *handle,
                *parent,
                transform_matrix(&sprite.transform),
                sprite.visible,
                sprite.layer,
                NodeKind::Sprite(Box::new(sprite.clone())),
            ),
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
                    node.local = transform_matrix(transform);
                }
                if let Some(visible) = visible {
                    node.visible = *visible;
                }
                let mut rebuild = false;
                if let (
                    Some(material),
                    NodeKind::Primitive {
                        material: current, ..
                    },
                ) = (material, &mut node.kind)
                {
                    *current = *material;
                    rebuild = true;
                }
                if transform.is_some() || visible.is_some() {
                    self.tables.dirty_nodes.insert(*handle);
                }
                if rebuild {
                    self.rebuild_parts(*handle);
                }
            }
            RenderDiff::Destroy { handle } => self.destroy_node(*handle),
            RenderDiff::SetParentJoint { handle, joint } => {
                // Joint attachment is realized with animated meshes (#8788).
                self.node_mut(*handle)?.parent_joint = joint.clone();
            }
            RenderDiff::ReplaceMeshPayload { handle, payload } => {
                let mut streams = resources::mesh_streams(payload, resources)?;
                // Uploaded payloads draw without vertex colours, as in Three.
                streams.colors = None;
                let mesh = self.upload_mesh(
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
                if let NodeKind::Sprite(sprite) = &mut node.kind {
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
            RenderDiff::DefineAnimatedMesh { .. }
            | RenderDiff::ReleaseAnimatedMesh { .. }
            | RenderDiff::SetAnimatedMeshInspection { .. }
            | RenderDiff::SetAnimatedMeshPlayback { .. } => {
                return Err("realized by a later family (#8788)".to_owned());
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
        self.tables.nodes.insert(
            handle,
            NodeRow {
                parent,
                parent_joint: None,
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
            self.tables.payload_meshes.remove(&handle);
            self.tables.lights.remove(&handle);
            self.tables.dirty_nodes.remove(&handle);
        }
    }

    /// Re-derive a node's drawable parts from its kind and bindings.
    pub(crate) fn rebuild_parts(&mut self, handle: RenderHandle) {
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
                let unlit = |mesh, first_index, index_count| {
                    (
                        Part {
                            node: handle,
                            mesh,
                            first_index,
                            index_count,
                            material: MaterialRef::Unlit,
                        },
                        PartRow {
                            color: material.color,
                            emission: [0.0; 3],
                        },
                    )
                };
                if *has_payload {
                    if let Some(mesh) = self.tables.payload_meshes.get(&handle) {
                        for (slot, start, count) in &mesh.groups {
                            let slot_material = format!("{PAYLOAD_SLOT_MATERIAL_PREFIX}{slot}");
                            let (material_ref, color, emission) =
                                match self.tables.materials.get(&slot_material) {
                                    Some(row) => (
                                        MaterialRef::Retained(slot_material),
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
                                },
                                PartRow {
                                    color: mul(color, material.color),
                                    emission,
                                },
                            ));
                        }
                    }
                } else {
                    match geometry {
                        Geometry::Group => {}
                        Geometry::Line { .. } => parts.push(unlit(MeshRef::Payload(handle), 0, 2)),
                        Geometry::Cube => parts.push(unlit(MeshRef::Builtin(Builtin::Cube), 0, 0)),
                        Geometry::Sphere => {
                            parts.push(unlit(MeshRef::Builtin(Builtin::Sphere), 0, 0))
                        }
                        Geometry::Quad => parts.push(unlit(MeshRef::Builtin(Builtin::Quad), 0, 0)),
                        Geometry::Point => {
                            parts.push(unlit(MeshRef::Builtin(Builtin::Point), 0, 0))
                        }
                    }
                }
            }
            NodeKind::StaticMesh {
                asset,
                overrides,
                parameters,
            } => {
                if let Some(mesh) = self.tables.static_meshes.get(asset) {
                    for (slot, start, count) in &mesh.groups {
                        let material_id = overrides.get(slot).or_else(|| mesh.slots.get(slot));
                        let row = material_id.and_then(|id| self.tables.materials.get(id));
                        let (material_ref, color, emission) = match (material_id, row) {
                            (Some(id), Some(row)) => {
                                let descriptor = &row.descriptor;
                                let override_ = parameters.get(slot);
                                let tint =
                                    override_.map_or(descriptor.texture_tint, |p| p.texture_tint);
                                let (emission_color, intensity) = override_.map_or(
                                    (descriptor.emission_color, descriptor.emission_intensity),
                                    |p| (p.emission_color, p.emission_intensity),
                                );
                                (
                                    MaterialRef::Retained(id.clone()),
                                    mul(descriptor.color, tint),
                                    emission(emission_color, intensity),
                                )
                            }
                            _ => (MaterialRef::LitFallback, slot_color(*slot), [0.0; 3]),
                        };
                        parts.push((
                            Part {
                                node: handle,
                                mesh: MeshRef::Static(asset.clone()),
                                first_index: *start,
                                index_count: *count,
                                material: material_ref,
                            },
                            PartRow { color, emission },
                        ));
                    }
                }
            }
            NodeKind::VoxelObject(instance) => {
                if let Some(object) = self.tables.voxel_objects.get(&instance.asset) {
                    let index = object.frame_mesh(instance.frame);
                    if let Some(mesh) = object.meshes.get(index as usize) {
                        for (slot, start, count) in &mesh.groups {
                            let material_id = instance
                                .material_overrides
                                .iter()
                                .find(|binding| binding.slot == *slot)
                                .map(|binding| &binding.material)
                                .or_else(|| object.slots.get(slot));
                            let (material_ref, color, emission) = match material_id
                                .and_then(|id| Some((id, self.tables.materials.get(id)?)))
                            {
                                Some((id, row)) => (
                                    MaterialRef::Retained(id.clone()),
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
                                    mesh: MeshRef::Voxel(instance.asset.clone(), index),
                                    first_index: *start,
                                    index_count: *count,
                                    material: material_ref,
                                },
                                PartRow { color, emission },
                            ));
                        }
                    }
                }
            }
            NodeKind::Group
            | NodeKind::Light(_)
            | NodeKind::AnimatedMesh(_)
            | NodeKind::Sprite(_) => {}
        }
        let (world, shown, layer) = (node.world, node.world_visible, node.world_layer);
        let mut ids = Vec::with_capacity(parts.len());
        for (mut part, row) in parts {
            if let MeshRef::Builtin(kind) = part.mesh {
                part.index_count = self.builtins[&kind].groups[0].2;
            }
            let mesh = self.mesh(&part.mesh);
            let bounds = mesh.map_or(Aabb::EMPTY, |mesh| mesh.bounds);
            let descriptor = match &part.material {
                MaterialRef::Retained(id) => {
                    self.tables.materials.get(id).map(|row| &row.descriptor)
                }
                _ => None,
            };
            let class = PartClass {
                blend: row.color[3] < 1.0 || descriptor.is_some_and(blends),
                double_sided: descriptor.is_some_and(|descriptor| descriptor.double_sided),
                lines: mesh.is_some_and(|mesh| mesh.topology == Topology::Lines),
            };
            let id = self.tables.parts.insert(part, row, bounds, class);
            self.tables.parts.write(id, &world, shown, layer);
            ids.push(id);
        }
        if let Some(node) = self.tables.nodes.get_mut(&handle) {
            node.parts = ids;
        }
    }

    /// The uploaded mesh a part draws, if it is (still) defined.
    pub(crate) fn mesh(&self, mesh: &MeshRef) -> Option<&GpuMesh> {
        match mesh {
            MeshRef::Static(asset) => self.tables.static_meshes.get(asset),
            MeshRef::Payload(handle) => self.tables.payload_meshes.get(handle),
            MeshRef::Builtin(kind) => self.builtins.get(kind),
            MeshRef::Voxel(asset, index) => self
                .tables
                .voxel_objects
                .get(asset)
                .and_then(|object| object.meshes.get(*index as usize)),
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
        self.effects.forget_texture(&texture.id);
        // Materials sampling this texture and a sky showing it bind the new view.
        let dependents: Vec<RenderMaterialDescriptor> = self
            .tables
            .materials
            .values()
            .filter(|row| row.descriptor.texture.as_deref() == Some(texture.id.as_str()))
            .map(|row| row.descriptor.clone())
            .collect();
        for descriptor in dependents {
            self.define_material(descriptor);
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
        let texture = self.gpu.device.create_texture_with_data(
            &self.gpu.queue,
            &wgpu::TextureDescriptor {
                label: Some(label),
                size: crate::target::extent(width, height),
                mip_level_count: 1,
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
        let filter = match descriptor.filter {
            TextureFilter::Nearest => wgpu::FilterMode::Nearest,
            TextureFilter::Linear => wgpu::FilterMode::Linear,
        };
        let address = match descriptor.wrap {
            TextureWrap::Repeat => wgpu::AddressMode::Repeat,
            TextureWrap::Clamp => wgpu::AddressMode::ClampToEdge,
        };
        GpuTexture {
            size: (width, height),
            view: texture.create_view(&Default::default()),
            sampler: self.gpu.device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some(label),
                address_mode_u: address,
                address_mode_v: address,
                mag_filter: filter,
                min_filter: filter,
                ..Default::default()
            }),
        }
    }

    fn define_material(&mut self, descriptor: RenderMaterialDescriptor) {
        let texture = descriptor
            .texture
            .as_ref()
            .and_then(|id| self.tables.textures.get(id));
        let params = MaterialParams::of(&descriptor, texture.map(|texture| texture.size));
        let bind_group =
            self.material_bind_group(&descriptor.id, &params, texture.unwrap_or(&self.white));
        let id = descriptor.id.clone();
        self.tables.materials.insert(
            id.clone(),
            MaterialRow {
                descriptor,
                bind_group,
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
                part.material == MaterialRef::Retained(id.clone())
                    || matches!(part.material, MaterialRef::LitFallback)
            })
            .map(|part| part.node)
            .collect();
        for handle in dedup(dependents) {
            self.rebuild_parts(handle);
        }
    }

    pub(crate) fn material_bind_group(
        &self,
        label: &str,
        params: &MaterialParams,
        texture: &GpuTexture,
    ) -> wgpu::BindGroup {
        material_bind_group(
            &self.gpu.device,
            &self.layouts.material,
            label,
            params,
            texture,
        )
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
        let redefined = self
            .tables
            .static_meshes
            .insert(asset.asset.clone(), mesh)
            .is_some();
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
        for vertex in vertices.chunks_exact(VERTEX_FLOATS) {
            bounds.include(Vec3::new(vertex[0], vertex[1], vertex[2]));
        }
        GpuMesh {
            bounds,
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

/// What a material bind group's uniform carries.
pub(crate) struct MaterialParams {
    pub roughness: f32,
    pub alpha_cutoff: f32,
    pub flags: u32,
    pub voxel_surface: Option<VoxelSurfaceUniform>,
}

impl MaterialParams {
    /// A retained material's uniform. A voxel surface's own alpha policy
    /// applies to it, as the Three lane's voxel surface specialization did.
    fn of(descriptor: &RenderMaterialDescriptor, texture_size: Option<(u32, u32)>) -> Self {
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
            alpha_cutoff: cutoff.unwrap_or(0.0),
            flags: cutoff.map_or(0, |_| FLAG_MASK)
                | voxel_surface.map_or(0, |_| FLAG_VOXEL_SURFACE),
            voxel_surface,
        }
    }
}

/// Whether a retained material draws alpha-blended.
pub(crate) fn blends(descriptor: &RenderMaterialDescriptor) -> bool {
    match &descriptor.voxel_surface {
        Some(surface) => surface.alpha_mode == VoxelSurfaceAlphaModeDescriptor::Blend,
        None => descriptor.alpha_mode == MaterialAlphaModeDescriptor::Blend,
    }
}

pub(crate) fn material_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    label: &str,
    params: &MaterialParams,
    texture: &GpuTexture,
) -> wgpu::BindGroup {
    let mut floats = [0f32; MATERIAL_UNIFORM_BYTES / 4];
    floats[0] = params.roughness;
    floats[1] = params.alpha_cutoff;
    floats[2] = f32::from_bits(params.flags);
    if let Some(surface) = &params.voxel_surface {
        floats[4..6].copy_from_slice(&surface.tile_scale);
        floats[6..8].copy_from_slice(&surface.tile_origin);
        floats[8..10].copy_from_slice(&surface.uv_min);
        floats[10..12].copy_from_slice(&surface.uv_max);
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
                resource: wgpu::BindingResource::TextureView(&texture.view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(&texture.sampler),
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
                alpha_cutoff: 0.0,
                flags: FLAG_UNLIT,
                voxel_surface: None,
            },
            white,
        ),
        material_bind_group(
            device,
            layout,
            "render-wgpu lit fallback",
            &MaterialParams {
                roughness: FALLBACK_ROUGHNESS,
                alpha_cutoff: 0.0,
                flags: 0,
                voxel_surface: None,
            },
            white,
        ),
    )
}

/// World-space light rows for the shader, in `world.wgsl`'s `Light` layout.
pub(crate) fn light_row(light: &LightDescriptor, world: &Mat4) -> Option<[f32; 16]> {
    let scaled = |color: [f32; 3], intensity: f32| color.map(|c| c * intensity);
    let rotate = |direction: [f32; 3]| {
        world
            .transform_vector3(Vec3::from(direction))
            .normalize_or_zero()
    };
    let place = |position: [f32; 3]| world.transform_point3(Vec3::from(position));
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

fn mul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    std::array::from_fn(|index| a[index] * b[index])
}

fn emission(color: [f32; 3], intensity: f32) -> [f32; 3] {
    color.map(|component| component * intensity)
}

/// Three's deterministic fallback hue for an unbound slot (golden angle,
/// HSL saturation 0.7, lightness 0.5), in linear RGB.
fn slot_color(slot: u16) -> [f32; 4] {
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
        RenderDiff::ReleaseTexture { .. } => "releaseTexture",
        RenderDiff::SetSkyBackground { .. } => "setSkyBackground",
        RenderDiff::SetBackgroundColor { .. } => "setBackgroundColor",
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
