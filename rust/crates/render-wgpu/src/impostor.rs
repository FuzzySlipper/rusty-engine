//! Impostor levels of detail (#10017, `impostor_bake.wgsl`).
//!
//! A scatter patch's impostor level shows its mesh as a card: the renderer
//! draws the mesh once from each of a hemi-octahedral grid of directions
//! around its bounding sphere into two atlases (base colour with coverage,
//! and normals in the mesh's own space), each mesh group with its own
//! material, and keeps them as a material of their own: its base texture the
//! colour atlas, its normal map the normal atlas, its parameters the sphere
//! and the grid. A copy at that level draws a quad (IMPOSTOR) facing the
//! camera and showing the view nearest the camera's direction in the copy's
//! own space, lit by the baked normals turned with the copy. A mesh and its
//! materials are baked once; what the atlases hold is derived from them
//! alone (`impostor/<mesh>` and its materials), so the bake is reproducible
//! and holds no authored art of its own.

use std::collections::HashMap;
use std::time::Instant;

use glam::{Mat4, Vec2, Vec3};
use render_model::{
    MaterialAlphaModeDescriptor, MaterialNormalMapDescriptor, RenderMaterialDescriptor,
};

use crate::apply::MaterialParams;
use crate::shaders::{Entry, Features};
use crate::tables::{GpuTexture, MeshRef};
use crate::Renderer;

/// Views a side of an impostor's grid.
pub(crate) const VIEWS: u32 = 8;
/// Texels a side of one view: an impostor stands far enough off that a
/// view rarely covers more of the screen (2 MiB an impostor).
const VIEW_TEXELS: u32 = 64;
/// `Bake` in impostor_bake.wgsl, at a uniform offset each.
const BAKE_BYTES: u64 = 96;
const ALBEDO_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const NORMAL_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// The impostors baked, for diagnostics.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ImpostorReadout {
    pub impostors: u32,
    /// The atlases' bytes.
    pub bytes: u64,
    /// The last bake's wall milliseconds.
    pub bake_ms: f64,
}

pub(crate) struct Impostors {
    layout: wgpu::BindGroupLayout,
    pipeline: Option<wgpu::RenderPipeline>,
    /// Each bake by key (the material it draws with): the mesh and the
    /// materials it was baked from.
    baked: HashMap<String, Vec<String>>,
    readout: ImpostorReadout,
}

impl Impostors {
    /// Forget the bakes made from `source` (a mesh or material defined anew
    /// or released): the parts drawing them bake again when rebuilt.
    pub fn forget(&mut self, source: &str) {
        let before = self.baked.len();
        self.baked
            .retain(|_, sources| !sources.iter().any(|from| from == source));
        let forgotten = (before - self.baked.len()) as u32;
        let side = u64::from(VIEWS * VIEW_TEXELS);
        self.readout.impostors -= forgotten;
        self.readout.bytes -= u64::from(forgotten) * side * side * 8;
    }

    pub fn new(device: &wgpu::Device) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("render-wgpu impostor bake"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: wgpu::BufferSize::new(BAKE_BYTES),
                },
                count: None,
            }],
        });
        Self {
            layout,
            pipeline: None,
            baked: HashMap::new(),
            readout: ImpostorReadout::default(),
        }
    }

    pub fn readout(&self) -> ImpostorReadout {
        self.readout
    }
}

/// The direction of a grid point (0 to 1 each way), at or above the
/// horizon of the mesh's own space (`rusty::view::hemi_octahedral_direction`).
fn hemi_octahedral_direction(uv: Vec2) -> Vec3 {
    let f = uv * 2.0 - Vec2::ONE;
    let p = Vec2::new(f.x + f.y, f.x - f.y) * 0.5;
    Vec3::new(p.x, 1.0 - p.x.abs() - p.y.abs(), p.y).normalize()
}

/// Up for a view looking back along `view` (`rusty::view::impostor_right`).
fn view_up(view: Vec3) -> Vec3 {
    if view.y.abs() > 0.999 {
        Vec3::Z
    } else {
        Vec3::Y
    }
}

impl Renderer {
    /// The material an impostor of static mesh `asset` draws with, its
    /// slots bound as `overrides` say (its own bindings otherwise): baked
    /// now if it was not. None while the mesh or a material it draws is
    /// not defined; a part asks again when they are.
    pub(crate) fn impostor_material(
        &mut self,
        asset: &str,
        overrides: &[(u16, String)],
    ) -> Option<String> {
        let mut key = format!("impostor/{asset}");
        for (slot, material) in overrides {
            key.push_str(&format!("/{slot}={material}"));
        }
        if self.impostors.baked.contains_key(&key) {
            return Some(key);
        }
        let mut sources = vec![asset.to_owned()];
        let started = Instant::now();
        let mesh_id = self.tables.names.get(asset)?;
        let mesh = self.mesh(&MeshRef::Static(mesh_id))?;
        let (vertices, indices, bounds) =
            (mesh.vertices.clone(), mesh.indices.clone(), mesh.bounds);
        // Each group's material: its bind group, colour and alpha cutoff.
        let mut groups = Vec::new();
        for (slot, first, count) in &mesh.groups {
            let id = overrides
                .iter()
                .rfind(|(bound, _)| bound == slot)
                .map(|(_, material)| material)
                .or_else(|| mesh.slots.get(slot))?;
            sources.push(id.clone());
            let row = self.tables.materials.get(self.tables.names.get(id)?)?;
            let color = row.descriptor.color;
            let tint = row.descriptor.texture_tint;
            let cutoff = match row.descriptor.alpha_mode {
                MaterialAlphaModeDescriptor::Mask { cutoff } => cutoff,
                _ => 0.0,
            };
            groups.push((
                row.bind_group.clone(),
                *first,
                *count,
                [
                    color[0] * tint[0],
                    color[1] * tint[1],
                    color[2] * tint[2],
                    1.0,
                ],
                cutoff,
            ));
        }
        let device = self.gpu.device.clone();
        let device = &device;
        let center = (bounds.min + bounds.max) * 0.5;
        let radius = ((bounds.max - bounds.min) * 0.5).length().max(1e-3);
        let side = VIEWS * VIEW_TEXELS;
        let texture = |label, format, usage| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: side,
                    height: side,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let sampled = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
        let albedo = texture("render-wgpu impostor colour", ALBEDO_FORMAT, sampled);
        let normal = texture("render-wgpu impostor normals", NORMAL_FORMAT, sampled);
        let depth = texture(
            "render-wgpu impostor depth",
            DEPTH_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        );
        let pipeline = self.impostors.pipeline.get_or_insert_with(|| {
            let shader = crate::pipelines::standard(self.layouts.shaders.module(
                device,
                Entry::ImpostorBake,
                Features::default(),
            ));
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("render-wgpu impostor bake"),
                bind_group_layouts: &[Some(&self.impostors.layout), Some(&self.layouts.material)],
                immediate_size: 0,
            });
            let buffers = crate::pipelines::mesh_buffers(Features::default(), false);
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("render-wgpu impostor bake"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_bake"),
                    compilation_options: Default::default(),
                    buffers: &buffers,
                },
                primitive: wgpu::PrimitiveState {
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::Less),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_bake"),
                    compilation_options: Default::default(),
                    targets: &[Some(ALBEDO_FORMAT.into()), Some(NORMAL_FORMAT.into())],
                }),
                multiview_mask: None,
                cache: None,
            })
        });
        // Each view's and group's uniform, at aligned offsets.
        let stride = u64::from(device.limits().min_uniform_buffer_offset_alignment).max(BAKE_BYTES);
        let draws = (VIEWS * VIEWS) as usize * groups.len();
        let mut uniforms = vec![0u8; (stride as usize) * draws.max(1)];
        let mut at = 0usize;
        for view in 0..VIEWS * VIEWS {
            let cell = Vec2::new((view % VIEWS) as f32, (view / VIEWS) as f32);
            let direction = hemi_octahedral_direction((cell + 0.5) / VIEWS as f32);
            let eye = center + direction * radius * 2.0;
            let view_proj =
                Mat4::orthographic_rh(-radius, radius, -radius, radius, 0.0, radius * 4.0)
                    * Mat4::look_at_rh(eye, center, view_up(direction));
            for group in &groups {
                let mut bytes = Vec::with_capacity(BAKE_BYTES as usize);
                bytes.extend_from_slice(bytemuck::cast_slice(&view_proj.to_cols_array()));
                bytes.extend_from_slice(bytemuck::cast_slice(&group.3));
                bytes.extend_from_slice(bytemuck::cast_slice(&[group.4, 0.0, 0.0, 0.0]));
                uniforms[at..at + bytes.len()].copy_from_slice(&bytes);
                at += stride as usize;
            }
        }
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("render-wgpu impostor bake views"),
            size: uniforms.len() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.gpu.queue.write_buffer(&uniform, 0, &uniforms);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("render-wgpu impostor bake views"),
            layout: &self.impostors.layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &uniform,
                    offset: 0,
                    size: wgpu::BufferSize::new(BAKE_BYTES),
                }),
            }],
        });
        let albedo_view = albedo.create_view(&Default::default());
        let normal_view = normal.create_view(&Default::default());
        let depth_view = depth.create_view(&Default::default());
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("render-wgpu impostor bake"),
        });
        {
            let clear = |view| {
                Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })
            };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render-wgpu impostor bake"),
                color_attachments: &[clear(&albedo_view), clear(&normal_view)],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
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
            pass.set_pipeline(pipeline);
            pass.set_vertex_buffer(0, vertices.slice(..));
            pass.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint32);
            let mut offset = 0u64;
            for view in 0..VIEWS * VIEWS {
                let (x, y) = (view % VIEWS, view / VIEWS);
                pass.set_viewport(
                    (x * VIEW_TEXELS) as f32,
                    (y * VIEW_TEXELS) as f32,
                    VIEW_TEXELS as f32,
                    VIEW_TEXELS as f32,
                    0.0,
                    1.0,
                );
                pass.set_scissor_rect(x * VIEW_TEXELS, y * VIEW_TEXELS, VIEW_TEXELS, VIEW_TEXELS);
                for group in &groups {
                    pass.set_bind_group(0, &bind_group, &[offset as u32]);
                    pass.set_bind_group(1, &group.0, &[]);
                    pass.draw_indexed(group.1..group.1 + group.2, 0, 0..1);
                    offset += stride;
                }
            }
        }
        self.gpu.queue.submit([encoder.finish()]);
        // The atlases as textures, and the material that draws them.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("render-wgpu impostor"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let albedo_id = format!("{key}/colour");
        let normal_id = format!("{key}/normals");
        for (id, view) in [(&albedo_id, albedo_view), (&normal_id, normal_view)] {
            self.tables.textures.insert(
                id.clone(),
                GpuTexture {
                    view,
                    sampler: sampler.clone(),
                    size: (side, side),
                    mipped: None,
                },
            );
        }
        let descriptor = RenderMaterialDescriptor {
            texture_transform: None,
            stochastic_tiling: None,
            terrain_layers: None,
            shader: None,
            id: key.clone(),
            color: [1.0; 4],
            texture: Some(albedo_id),
            roughness: 0.9,
            metalness: 0.0,
            texture_tint: [1.0; 4],
            emission_color: [0.0; 3],
            emission_intensity: 0.0,
            uv_strategy: render_model::MaterialUvStrategy::Flat,
            alpha_mode: MaterialAlphaModeDescriptor::Mask { cutoff: 0.5 },
            double_sided: true,
            voxel_surface: None,
            normal_map: Some(MaterialNormalMapDescriptor {
                texture: normal_id,
                scale: 1.0,
            }),
            triplanar: None,
            emission_map: Default::default(),
            occlusion_map: Default::default(),
            unlit: false,
            flat_shading: false,
            keep_dry: false,
            wind: None,
            water: None,
            translucent_shadow: false,
        };
        let mut params = MaterialParams::of(&descriptor, Some((side, side)));
        params.parameters[0] = [center.x, center.y, center.z, radius];
        params.parameters[1] = [VIEWS as f32, 0.0, 0.0, 0.0];
        // Recorded first: the parts drawing an earlier bake of it are rebuilt
        // as its material is inserted, and ask for it again.
        self.impostors.baked.insert(key.clone(), sources);
        if self.insert_material(descriptor, &params).is_err() {
            self.impostors.baked.remove(&key);
            return None;
        }
        // The normal atlas is the card's normals (IMPOSTOR), not a tangent
        // space map.
        let id = self.tables.names.id(&key);
        if let Some(row) = self.tables.materials.get_mut(id) {
            row.features = row.features.without(Features::NORMAL_MAP);
        }
        let readout = &mut self.impostors.readout;
        readout.impostors += 1;
        readout.bytes += u64::from(side * side) * 8;
        readout.bake_ms = started.elapsed().as_secs_f64() * 1e3;
        Some(key)
    }
}
