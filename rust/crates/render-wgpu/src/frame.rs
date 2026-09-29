//! The fixed pass pipeline, in order: propagate dirty transforms, prepare GPU
//! rows, build the draw list for the view, encode sky and world passes.

use glam::{Mat4, Vec3};
use render_host_contracts::{RendererCameraProjection, RendererCompositionCamera};
use render_model::{MaterialAlphaModeDescriptor, RenderHandle, RenderLayer};

use crate::apply::light_row;
use crate::tables::{Environment, MaterialRef, MeshRef, NodeKind, Topology, PART_ROW_FLOATS};
use crate::target::TargetView;
use crate::{
    srgb_to_linear, OffscreenTarget, PresentSkip, Renderer, WindowSurface, DEFAULT_CLEAR_SRGB,
    NEUTRAL_GROUND_SRGB, NEUTRAL_HEMISPHERE_INTENSITY, NEUTRAL_KEY_INTENSITY, NEUTRAL_KEY_POSITION,
};

const LIGHT_ROW_FLOATS: usize = 16;
/// Frame uniform: two matrices, camera position, counts.
const FRAME_UNIFORM_BYTES: u64 = (16 + 16 + 4 + 4) * 4;

/// Per-frame counts for diagnostics.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FrameStats {
    pub draws: u32,
    pub parts_uploaded: u32,
    pub lights: u32,
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
    /// Render the retained scene from `camera` into the offscreen target.
    pub fn render_offscreen(
        &mut self,
        camera: &RendererCompositionCamera,
        target: &OffscreenTarget,
    ) -> FrameStats {
        let uploaded = self.prepare();
        let mut stats = self.encode(camera, target.view());
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
        surface.present_with(&gpu, |view| stats = self.encode(camera, view))?;
        stats.parts_uploaded = uploaded;
        Ok(stats)
    }

    /// Bring GPU rows up to date with the tables. Returns rows uploaded.
    fn prepare(&mut self) -> u32 {
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

    fn upload_lights(&mut self) {
        let mut rows: Vec<f32> = Vec::new();
        if self.options.default_world_lights {
            let sky = [NEUTRAL_HEMISPHERE_INTENSITY; 3];
            let ground =
                NEUTRAL_GROUND_SRGB.map(|c| srgb_to_linear(c) * NEUTRAL_HEMISPHERE_INTENSITY);
            rows.extend_from_slice(&[
                sky[0], sky[1], sky[2], 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, ground[0],
                ground[1], ground[2], 0.0,
            ]);
            // The key light shines from its position toward the origin.
            let travel = -Vec3::from(NEUTRAL_KEY_POSITION).normalize();
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
        let mut handles: Vec<&RenderHandle> = self.tables.lights.iter().collect();
        handles.sort();
        for handle in handles {
            if let Some(node) = self.tables.nodes.get(handle) {
                if let (NodeKind::Light(light), true) = (&node.kind, node.world_visible) {
                    if let Some(row) = light_row(light, &node.world) {
                        rows.extend_from_slice(&row);
                    }
                }
            }
        }
        self.light_count = (rows.len() / LIGHT_ROW_FLOATS) as u32;
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

    fn encode(&mut self, camera: &RendererCompositionCamera, view: TargetView<'_>) -> FrameStats {
        let (view_proj, eye) = camera_matrices(camera, view.width, view.height);
        let mut uniform = Vec::with_capacity((FRAME_UNIFORM_BYTES / 4) as usize);
        uniform.extend_from_slice(&view_proj.to_cols_array());
        uniform.extend_from_slice(&view_proj.inverse().to_cols_array());
        uniform.extend_from_slice(&[eye.x, eye.y, eye.z, 1.0]);
        let mut bytes: Vec<u8> = bytemuck::cast_slice(&uniform).to_vec();
        bytes.extend_from_slice(&self.light_count.to_le_bytes());
        bytes.extend_from_slice(&[0u8; 12]);
        self.gpu.queue.write_buffer(&self.frame_buffer, 0, &bytes);

        let draws = self.draw_list(eye);
        let format_index = match self
            .pipelines
            .iter()
            .position(|set| set.format == view.format)
        {
            Some(index) => index,
            None => {
                self.pipelines
                    .push(self.layouts.pipelines(&self.gpu.device, view.format));
                self.pipelines.len() - 1
            }
        };
        let pipelines = &self.pipelines[format_index];
        let clear = match &self.tables.environment {
            Environment::Color(color) => [color[0], color[1], color[2], color[3]],
            _ => {
                let rgb = DEFAULT_CLEAR_SRGB.map(srgb_to_linear);
                [rgb[0], rgb[1], rgb[2], 1.0]
            }
        };
        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render-wgpu frame"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render-wgpu world"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: view.color,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: f64::from(clear[0]),
                            g: f64::from(clear[1]),
                            b: f64::from(clear[2]),
                            a: f64::from(clear[3]),
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: view.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &self.frame_bind_group, &[]);
            if let Some(sky) = &self.sky_bind_group {
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
        FrameStats {
            draws: draws.len() as u32,
            parts_uploaded: 0,
            lights: self.light_count,
        }
    }

    /// Every visible part of the world view, opaque first, blended parts
    /// back to front.
    fn draw_list(&self, eye: Vec3) -> Vec<Draw> {
        let mut draws = Vec::new();
        for (id, part) in self.tables.parts.meta.iter().enumerate() {
            let Some(part) = part else { continue };
            let Some(node) = self.tables.nodes.get(&part.node) else {
                continue;
            };
            // The viewmodel layer draws in its own camera pass (#8785).
            if !node.world_visible || node.world_layer == RenderLayer::Viewmodel {
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

/// View-projection matrix and eye position for one composition camera.
/// Engine yaw zero faces -Z and positive yaw turns toward +X.
pub(crate) fn camera_matrices(
    camera: &RendererCompositionCamera,
    width: u32,
    height: u32,
) -> (Mat4, Vec3) {
    let eye = Vec3::new(
        camera.pose.position[0] as f32,
        camera.pose.position[1] as f32,
        camera.pose.position[2] as f32,
    );
    let (forward, up) = match &camera.basis {
        Some(basis) => (
            Vec3::new(
                basis.forward[0] as f32,
                basis.forward[1] as f32,
                basis.forward[2] as f32,
            ),
            Vec3::new(basis.up[0] as f32, basis.up[1] as f32, basis.up[2] as f32),
        ),
        None => {
            let yaw = (camera.pose.yaw_degrees as f32).to_radians();
            let pitch = (camera.pose.pitch_degrees as f32).to_radians();
            (
                Vec3::new(
                    yaw.sin() * pitch.cos(),
                    pitch.sin(),
                    -yaw.cos() * pitch.cos(),
                ),
                Vec3::new(
                    -yaw.sin() * pitch.sin(),
                    pitch.cos(),
                    yaw.cos() * pitch.sin(),
                ),
            )
        }
    };
    let view = Mat4::look_to_rh(eye, forward, up);
    let aspect = width.max(1) as f32 / height.max(1) as f32;
    let projection = match camera.projection {
        RendererCameraProjection::Perspective {
            fov_y_degrees,
            near,
            far,
        } => Mat4::perspective_rh(
            (fov_y_degrees as f32).to_radians(),
            aspect,
            near as f32,
            far as f32,
        ),
        RendererCameraProjection::Orthographic {
            vertical_size,
            near,
            far,
        } => {
            let half_height = vertical_size as f32 * 0.5;
            let half_width = half_height * aspect;
            Mat4::orthographic_rh(
                -half_width,
                half_width,
                -half_height,
                half_height,
                near as f32,
                far as f32,
            )
        }
    };
    (projection * view, eye)
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
