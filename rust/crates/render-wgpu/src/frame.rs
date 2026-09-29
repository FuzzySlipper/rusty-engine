//! The fixed pass pipeline, in order: propagate dirty transforms, prepare GPU
//! rows, then per view pass build the draw list for its layer and encode sky
//! and world (or viewmodel) draws. `composition.rs` orders the view passes.

use glam::Vec3;
use render_host_contracts::RendererCompositionCamera;
use render_model::{MaterialAlphaModeDescriptor, RenderHandle, RenderLayer};

use crate::apply::light_row;
use crate::camera::CameraMatrices;
use crate::tables::{Environment, MaterialRef, MeshRef, NodeKind, Topology, PART_ROW_FLOATS};
use crate::target::TargetView;
use crate::{
    srgb_to_linear, OffscreenTarget, PresentSkip, Renderer, WindowSurface, DEFAULT_CLEAR_SRGB,
    NEUTRAL_GROUND_SRGB, NEUTRAL_HEMISPHERE_INTENSITY, NEUTRAL_KEY_INTENSITY, NEUTRAL_KEY_POSITION,
    NEUTRAL_VIEWMODEL_KEY_POSITION,
};

const LIGHT_ROW_FLOATS: usize = 16;
/// Frame uniform: two matrices, camera position, light count and first light.
const FRAME_UNIFORM_BYTES: u64 = (16 + 16 + 4 + 4) * 4;

/// Per-frame counts for diagnostics.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FrameStats {
    pub draws: u32,
    pub parts_uploaded: u32,
    pub lights: u32,
    /// Offscreen composition views drawn this frame; a target that is not
    /// stale is presented as it is.
    pub offscreen_views: u32,
}

/// Which retained layers a view pass draws.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ViewLayer {
    /// Every layer but the viewmodel.
    World,
    /// Camera-relative content, after a depth break, with its own lights.
    Viewmodel,
}

/// Rows `first..first + count` of the lights buffer.
#[derive(Clone, Copy, Default)]
pub(crate) struct LightRange {
    pub first: u32,
    pub count: u32,
}

/// World lights, then viewmodel lights, in one buffer.
#[derive(Clone, Copy, Default)]
pub(crate) struct LightRanges {
    pub world: LightRange,
    pub viewmodel: LightRange,
}

/// A pixel rectangle with its origin at the top left.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PixelRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl PixelRect {
    pub fn whole(width: u32, height: u32) -> Self {
        Self {
            x: 0,
            y: 0,
            width: width.max(1),
            height: height.max(1),
        }
    }

    pub fn aspect(&self) -> f32 {
        self.width as f32 / self.height as f32
    }
}

/// How a view pass begins on its target.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum PassStart {
    /// Clear the whole target: colour and depth for a world pass, depth only
    /// for a viewmodel pass.
    Target,
    /// Clear only the viewport, keeping what earlier passes drew elsewhere.
    Viewport,
}

pub(crate) struct ViewPass<'a> {
    pub target: TargetView<'a>,
    pub viewport: PixelRect,
    pub camera: CameraMatrices,
    pub layer: ViewLayer,
    pub start: PassStart,
    /// Linear background colour a world pass clears to.
    pub clear: [f32; 4],
    /// Draw the retained sky behind a world pass.
    pub sky: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Pass {
    Opaque,
    OpaqueDoubleSided,
    Lines,
    Blend,
    BlendDoubleSided,
}

struct Draw {
    pass: Pass,
    part: u32,
    depth: f32,
}

impl Renderer {
    /// Render the retained scene from `camera` into the whole offscreen
    /// target: the world, then the viewmodel layer. A composition renders
    /// through [`Renderer::render_view_composition`] instead.
    pub fn render_offscreen(
        &mut self,
        camera: &RendererCompositionCamera,
        target: &OffscreenTarget,
    ) -> FrameStats {
        let uploaded = self.prepare();
        let mut stats = self.render_camera(camera, target.view());
        stats.parts_uploaded = uploaded;
        stats
    }

    /// Render the retained scene from `camera` into the window and present it.
    pub fn render_to_surface(
        &mut self,
        camera: &RendererCompositionCamera,
        surface: &mut WindowSurface,
    ) -> Result<FrameStats, PresentSkip> {
        let uploaded = self.prepare();
        let gpu = self.gpu.clone();
        let mut stats = FrameStats::default();
        surface.present_with(&gpu, |view| stats = self.render_camera(camera, view))?;
        stats.parts_uploaded = uploaded;
        Ok(stats)
    }

    /// Bring GPU rows up to date with the tables. Returns rows uploaded.
    pub(crate) fn prepare(&mut self) -> u32 {
        self.propagate_transforms();
        if self.tables.lights_dirty {
            self.upload_lights();
            self.tables.lights_dirty = false;
        }
        if self.tables.environment_dirty {
            self.rebuild_sky();
            self.tables.environment_dirty = false;
        }
        self.upload_parts()
    }

    /// Recompute world transform, visibility and layer for each dirty subtree.
    fn propagate_transforms(&mut self) {
        let dirty: Vec<RenderHandle> = self.tables.dirty_nodes.drain().collect();
        let dirty_set: std::collections::HashSet<RenderHandle> = dirty.iter().copied().collect();
        for root in dirty {
            // An ancestor that is also dirty recomputes this subtree anyway.
            let mut ancestor = self.tables.nodes.get(&root).and_then(|node| node.parent);
            let mut covered = false;
            while let Some(handle) = ancestor {
                if dirty_set.contains(&handle) {
                    covered = true;
                    break;
                }
                ancestor = self.tables.nodes.get(&handle).and_then(|node| node.parent);
            }
            if !covered {
                self.update_subtree(root);
            }
        }
    }

    fn update_subtree(&mut self, root: RenderHandle) {
        let mut stack = vec![root];
        while let Some(handle) = stack.pop() {
            let parent_state = self
                .tables
                .nodes
                .get(&handle)
                .and_then(|node| node.parent)
                .and_then(|parent| self.tables.nodes.get(&parent))
                .map(|parent| (parent.world, parent.world_visible, parent.world_layer));
            let Some(node) = self.tables.nodes.get_mut(&handle) else {
                continue;
            };
            let (world, visible, layer) = match parent_state {
                Some((world, visible, layer)) => {
                    (world * node.local, visible && node.visible, layer)
                }
                None => (node.local, node.visible, node.layer),
            };
            node.world = world;
            node.world_visible = visible;
            node.world_layer = layer;
            if matches!(node.kind, NodeKind::Light(_)) {
                self.tables.lights_dirty = true;
            }
            let parts = node.parts.clone();
            stack.extend(node.children.iter().copied());
            for part in parts {
                self.tables.parts.write(part, &world);
            }
        }
    }

    /// World lights (the world rig and retained lights outside the viewmodel
    /// layer), then viewmodel lights (the viewmodel rig and retained lights in
    /// the viewmodel layer, in camera-local coordinates).
    fn upload_lights(&mut self) {
        let mut rows: Vec<f32> = Vec::new();
        if self.options.default_world_lights {
            neutral_rig(&mut rows, NEUTRAL_KEY_POSITION);
        }
        self.retained_light_rows(&mut rows, ViewLayer::World);
        let world_count = (rows.len() / LIGHT_ROW_FLOATS) as u32;
        if self.options.default_viewmodel_lights {
            neutral_rig(&mut rows, NEUTRAL_VIEWMODEL_KEY_POSITION);
        }
        self.retained_light_rows(&mut rows, ViewLayer::Viewmodel);
        let total = (rows.len() / LIGHT_ROW_FLOATS) as u32;
        self.lights = LightRanges {
            world: LightRange {
                first: 0,
                count: world_count,
            },
            viewmodel: LightRange {
                first: world_count,
                count: total - world_count,
            },
        };
        let needed = rows.len().max(LIGHT_ROW_FLOATS) * 4;
        if needed as u64 > self.lights_buffer.size() {
            self.lights_buffer = storage_buffer(
                &self.gpu.device,
                "render-wgpu lights",
                (needed as u64).next_power_of_two(),
            );
            self.rebind_frame();
        }
        if !rows.is_empty() {
            self.gpu
                .queue
                .write_buffer(&self.lights_buffer, 0, bytemuck::cast_slice(&rows));
        }
    }

    fn retained_light_rows(&self, rows: &mut Vec<f32>, layer: ViewLayer) {
        let mut handles: Vec<&RenderHandle> = self.tables.lights.iter().collect();
        handles.sort();
        for handle in handles {
            if let Some(node) = self.tables.nodes.get(handle) {
                let in_layer =
                    (node.world_layer == RenderLayer::Viewmodel) == (layer == ViewLayer::Viewmodel);
                if let (NodeKind::Light(light), true, true) =
                    (&node.kind, node.world_visible, in_layer)
                {
                    if let Some(row) = light_row(light, &node.world) {
                        rows.extend_from_slice(&row);
                    }
                }
            }
        }
    }

    fn upload_parts(&mut self) -> u32 {
        let parts = &mut self.tables.parts;
        let row_bytes = (PART_ROW_FLOATS * 4) as u64;
        let needed = (parts.meta.len().max(1) as u64) * row_bytes;
        if parts.grown && needed > self.parts_buffer.size() {
            self.parts_buffer = storage_buffer(
                &self.gpu.device,
                "render-wgpu parts",
                needed.next_power_of_two(),
            );
            // A new buffer starts empty: upload every row once.
            parts.dirty.extend(0..parts.meta.len() as u32);
            parts.grown = false;
            self.rebind_frame();
        }
        let parts = &mut self.tables.parts;
        parts.grown = false;
        let mut dirty: Vec<u32> = parts.dirty.drain().collect();
        dirty.sort_unstable();
        let uploaded = dirty.len() as u32;
        // Coalesce consecutive rows into one write each.
        let mut index = 0;
        while index < dirty.len() {
            let start = dirty[index];
            let mut end = start;
            while index + 1 < dirty.len() && dirty[index + 1] == end + 1 {
                index += 1;
                end += 1;
            }
            let floats =
                &parts.gpu[start as usize * PART_ROW_FLOATS..(end as usize + 1) * PART_ROW_FLOATS];
            self.gpu.queue.write_buffer(
                &self.parts_buffer,
                u64::from(start) * row_bytes,
                bytemuck::cast_slice(floats),
            );
            index += 1;
        }
        uploaded
    }

    fn rebind_frame(&mut self) {
        self.frame_bind_group = frame_bind_group(
            &self.gpu.device,
            &self.layouts.frame,
            &self.frame_buffer,
            &self.parts_buffer,
            &self.lights_buffer,
        );
    }

    fn rebuild_sky(&mut self) {
        self.sky_bind_group = None;
        let Environment::Sky(sky) = &self.tables.environment else {
            return;
        };
        let Some(first) = self.tables.textures.get(&sky.texture) else {
            return;
        };
        let (second, amount) = match &sky.blend {
            Some(blend) => match self.tables.textures.get(&blend.texture) {
                Some(second) => (second, blend.amount),
                None => (first, 0.0),
            },
            None => (first, 0.0),
        };
        let mut uniform = [0u8; 16];
        uniform[0..4].copy_from_slice(&amount.to_le_bytes());
        use wgpu::util::DeviceExt;
        let buffer = self
            .gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("render-wgpu sky"),
                contents: &uniform,
                usage: wgpu::BufferUsages::UNIFORM,
            });
        self.sky_bind_group = Some(
            self.gpu
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("render-wgpu sky"),
                    layout: &self.layouts.sky,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(&first.view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::TextureView(&second.view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: wgpu::BindingResource::Sampler(&first.sampler),
                        },
                        wgpu::BindGroupEntry {
                            binding: 4,
                            resource: wgpu::BindingResource::Sampler(&second.sampler),
                        },
                    ],
                }),
        );
    }

    /// The linear colour a world pass clears to: the retained background
    /// colour, or the Engine default behind a sky or with nothing selected.
    pub(crate) fn environment_clear(&self) -> [f32; 4] {
        match &self.tables.environment {
            Environment::Color(color) => *color,
            _ => {
                let rgb = DEFAULT_CLEAR_SRGB.map(srgb_to_linear);
                [rgb[0], rgb[1], rgb[2], 1.0]
            }
        }
    }

    /// Encode and submit one view pass. Each pass writes the frame uniform
    /// and submits on its own, so passes with different cameras never share
    /// one uniform write. Returns the draws issued.
    pub(crate) fn encode_view(&mut self, view: ViewPass<'_>) -> u32 {
        let world_layer = view.layer == ViewLayer::World;
        let lights = if world_layer {
            self.lights.world
        } else {
            self.lights.viewmodel
        };
        let view_proj = view.camera.view_proj;
        let eye = view.camera.eye;
        let draws = self.draw_list(eye, view.layer);
        if !world_layer && draws.is_empty() {
            return 0;
        }
        let mut uniform = Vec::with_capacity((FRAME_UNIFORM_BYTES / 4) as usize);
        uniform.extend_from_slice(&view_proj.to_cols_array());
        uniform.extend_from_slice(&view_proj.inverse().to_cols_array());
        uniform.extend_from_slice(&[eye.x, eye.y, eye.z, 1.0]);
        let mut bytes: Vec<u8> = bytemuck::cast_slice(&uniform).to_vec();
        for count in [lights.count, lights.first, 0, 0] {
            bytes.extend_from_slice(&count.to_le_bytes());
        }
        self.gpu.queue.write_buffer(&self.frame_buffer, 0, &bytes);

        let format = view.target.format;
        let format_index = match self.pipelines.iter().position(|set| set.format == format) {
            Some(index) => index,
            None => {
                self.pipelines
                    .push(self.layouts.pipelines(&self.gpu.device, format));
                self.pipelines.len() - 1
            }
        };
        let whole = view.start == PassStart::Target;
        if !whole {
            self.compose.prepare_clear(&self.gpu, format, view.clear);
        }
        let pipelines = &self.pipelines[format_index];
        let color_load = if whole && world_layer {
            let [r, g, b, a] = view.clear.map(f64::from);
            wgpu::LoadOp::Clear(wgpu::Color { r, g, b, a })
        } else {
            wgpu::LoadOp::Load
        };
        let depth_load = if whole {
            wgpu::LoadOp::Clear(1.0)
        } else {
            wgpu::LoadOp::Load
        };
        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render-wgpu view"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(if world_layer {
                    "render-wgpu world"
                } else {
                    "render-wgpu viewmodel"
                }),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: view.target.color,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: color_load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: view.target.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: depth_load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            let area = view.viewport;
            pass.set_viewport(
                area.x as f32,
                area.y as f32,
                area.width as f32,
                area.height as f32,
                0.0,
                1.0,
            );
            pass.set_scissor_rect(area.x, area.y, area.width, area.height);
            if !whole {
                self.compose.clear_viewport(&mut pass, format, world_layer);
            }
            pass.set_bind_group(0, &self.frame_bind_group, &[]);
            if let (true, true, Some(sky)) = (world_layer, view.sky, &self.sky_bind_group) {
                pass.set_pipeline(&pipelines.sky);
                pass.set_bind_group(1, sky, &[]);
                pass.draw(0..3, 0..1);
            }
            let mut current: Option<Pass> = None;
            for draw in &draws {
                let Some(part) = self.tables.parts.meta[draw.part as usize].as_ref() else {
                    continue;
                };
                let mesh = match &part.mesh {
                    MeshRef::Static(asset) => self.tables.static_meshes.get(asset),
                    MeshRef::Payload(handle) => self.tables.payload_meshes.get(handle),
                    MeshRef::Builtin(kind) => self.builtins.get(kind),
                };
                let Some(mesh) = mesh else { continue };
                if current != Some(draw.pass) {
                    pass.set_pipeline(match draw.pass {
                        Pass::Opaque => &pipelines.opaque,
                        Pass::OpaqueDoubleSided => &pipelines.opaque_double_sided,
                        Pass::Lines => &pipelines.lines,
                        Pass::Blend => &pipelines.blend,
                        Pass::BlendDoubleSided => &pipelines.blend_double_sided,
                    });
                    current = Some(draw.pass);
                }
                let material = match &part.material {
                    MaterialRef::Retained(id) => self
                        .tables
                        .materials
                        .get(id)
                        .map_or(&self.lit_fallback_material, |row| &row.bind_group),
                    MaterialRef::Unlit => &self.unlit_material,
                    MaterialRef::LitFallback => &self.lit_fallback_material,
                };
                pass.set_bind_group(1, material, &[]);
                pass.set_vertex_buffer(0, mesh.vertices.slice(..));
                pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(
                    part.first_index..part.first_index + part.index_count,
                    0,
                    draw.part..draw.part + 1,
                );
            }
        }
        self.gpu.queue.submit([encoder.finish()]);
        draws.len() as u32
    }

    /// Every visible part in `layer`, opaque first, blended parts back to
    /// front.
    fn draw_list(&self, eye: Vec3, layer: ViewLayer) -> Vec<Draw> {
        let mut draws = Vec::new();
        for (id, part) in self.tables.parts.meta.iter().enumerate() {
            let Some(part) = part else { continue };
            let Some(node) = self.tables.nodes.get(&part.node) else {
                continue;
            };
            // The viewmodel layer draws in its own camera pass.
            let viewmodel = node.world_layer == RenderLayer::Viewmodel;
            if !node.world_visible || viewmodel != (layer == ViewLayer::Viewmodel) {
                continue;
            }
            let row = &self.tables.parts.rows[id];
            let descriptor = match &part.material {
                MaterialRef::Retained(material) => self
                    .tables
                    .materials
                    .get(material)
                    .map(|row| &row.descriptor),
                _ => None,
            };
            let double_sided = descriptor.is_some_and(|descriptor| descriptor.double_sided);
            let blend = row.color[3] < 1.0
                || descriptor.is_some_and(|descriptor| {
                    descriptor.alpha_mode == MaterialAlphaModeDescriptor::Blend
                });
            let is_line = matches!(&part.mesh, MeshRef::Payload(handle)
                if self.tables.payload_meshes.get(handle).is_some_and(|mesh| mesh.topology == Topology::Lines));
            let pass = match (is_line, blend, double_sided) {
                (true, _, _) => Pass::Lines,
                (false, false, false) => Pass::Opaque,
                (false, false, true) => Pass::OpaqueDoubleSided,
                (false, true, false) => Pass::Blend,
                (false, true, true) => Pass::BlendDoubleSided,
            };
            let depth = if blend {
                -node.world.w_axis.truncate().distance_squared(eye)
            } else {
                0.0
            };
            draws.push(Draw {
                pass,
                part: id as u32,
                depth,
            });
        }
        draws.sort_by(|a, b| {
            a.pass
                .cmp(&b.pass)
                .then(a.depth.total_cmp(&b.depth))
                .then(a.part.cmp(&b.part))
        });
        draws
    }
}

pub(crate) fn storage_buffer(device: &wgpu::Device, label: &str, size: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

pub(crate) fn frame_uniform_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("render-wgpu frame"),
        size: FRAME_UNIFORM_BYTES,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

pub(crate) fn frame_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    frame: &wgpu::Buffer,
    parts: &wgpu::Buffer,
    lights: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("render-wgpu frame"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: frame.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: parts.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: lights.as_entire_binding(),
            },
        ],
    })
}

/// The neutral rig: a hemisphere light and a key light shining from
/// `key_position` toward the origin.
fn neutral_rig(rows: &mut Vec<f32>, key_position: [f32; 3]) {
    let sky = [NEUTRAL_HEMISPHERE_INTENSITY; 3];
    let ground = NEUTRAL_GROUND_SRGB.map(|c| srgb_to_linear(c) * NEUTRAL_HEMISPHERE_INTENSITY);
    rows.extend_from_slice(&[
        sky[0], sky[1], sky[2], 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, ground[0], ground[1],
        ground[2], 0.0,
    ]);
    let travel = -Vec3::from(key_position).normalize();
    rows.extend_from_slice(&[
        NEUTRAL_KEY_INTENSITY,
        NEUTRAL_KEY_INTENSITY,
        NEUTRAL_KEY_INTENSITY,
        2.0,
        0.0,
        0.0,
        0.0,
        0.0,
        travel.x,
        travel.y,
        travel.z,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
    ]);
}
