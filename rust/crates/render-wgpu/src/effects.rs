//! Sprites and particles.
//!
//! Sprites are retained nodes (`NodeKind::Sprite`); their quads depend on the
//! camera (billboard orientation, pixel size, viewport placement), so each
//! view pass builds one instance row per visible sprite from the node's
//! propagated world matrix. There is no parent walk. Frames come from the
//! retained descriptor, and `UpdateSprite` carries playback, which the Engine
//! advances on update time.
//!
//! Particles are `PresentationOp::Particle` emitters simulated in
//! [`crate::particles`] on Engine update time only.
//!
//! In a view pass, sprites and particles draw after the world's opaque parts:
//! solid sprites, then the world's blended parts, then blended sprites back to
//! front, then particle cubes and billboards. A soft billboard (a positive
//! `softness_metres`) fades out as it nears the world behind it, which needs
//! the world's depth as a texture, so soft billboards draw last in their own
//! pass over the world's HDR target (`frame.rs`, "render-wgpu particles"),
//! testing and fading against that depth in their fragment shader.

use std::collections::HashMap;

use glam::{Mat3, Mat4, Quat, Vec3, Vec4};
use render_model::{
    BillboardMode, RenderLayer, SpriteAlphaMode, SpriteBlendMode, SpriteDepthPolicy,
    SpriteInstanceDescriptor, SpriteLightingMode, SpriteSizeMode, SpriteViewportFit,
};
use render_presentation::{
    ParticleBlendMode, ParticleSizeMode, ParticleSpriteRef, ParticleVisual, PresentationFrameDiff,
    PresentationOp,
};

use crate::camera::CameraMatrices;
use crate::frame::{PixelRect, ViewLayer, ViewPass};
use crate::particles::{EntityPositions, ParticleIssue};
use crate::pipelines::VERTEX_FLOATS;
use crate::resources::{self, ResourceSource};
use crate::tables::{GpuMesh, GpuTexture, NodeKind, SpriteRow};
use crate::target::{ColorTarget, DEPTH_FORMAT};
use crate::{srgb_to_linear, ApplyIssue, Gpu, Renderer};

/// Particle billboards are drawn `size × 24` pixels across.
const PARTICLE_PIXELS_PER_UNIT: f32 = 24.0;
const SPRITE_ROW_FLOATS: usize = 36;
const PARTICLE_ROW_FLOATS: usize = 16;
const CUBE_ROW_FLOATS: usize = 8;
/// Viewport-placed sprites sit mid-depth (GL clip z 0).
const PLACEMENT_DEPTH: f32 = 0.5;

/// Pipeline state a sprite needs, from its depth policy and alpha mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct SpriteState {
    depth_test: bool,
    depth_write: bool,
    blend: bool,
    /// A blended sprite added to the frame rather than drawn over it.
    additive: bool,
    /// A blended sprite fading against the world's depth: drawn in the
    /// particle pass after the world, by its own depth test.
    soft: bool,
}

/// Colour texture id and detail (normal or height) texture id; `None` is the
/// 1×1 white texture, or the colour texture again for the detail slot.
/// (color, detail) texture name ids (`Tables::names`).
type SpriteTextures = (Option<u32>, Option<u32>);

struct SpriteDraw {
    state: SpriteState,
    textures: SpriteTextures,
    render_order: i32,
    depth: f32,
    row: [f32; SPRITE_ROW_FLOATS],
}

/// What one view pass draws of this family, prepared before the pass.
#[derive(Default)]
pub(crate) struct EffectsPass {
    /// Sprites by their instance in the sprite rows: solid sprites, then
    /// blended ones in back-to-front order (the soft ones among them drawn
    /// after the world instead), then soft sprites.
    solid: Vec<SpriteInstance>,
    blended: Vec<BlendedSprite>,
    soft_sprites: Vec<SpriteInstance>,
    cubes: u32,
    /// Runs into the particle rows: hard billboards, then soft ones.
    billboards: Vec<BillboardRun>,
    /// Sprite nodes examined for this pass.
    pub sprite_candidates: u32,
}

/// Billboard particles drawn by one call: they share a texture, a blend and
/// a depth path (hard in the world pass, soft in the particle pass after it).
#[derive(Clone, Copy)]
struct BillboardRun {
    texture: u32,
    blend: ParticleBlendMode,
    soft: bool,
    first: u32,
    count: u32,
}

/// A blended sprite with the keys the world pass merges it by: transparent
/// objects sort by render order, then back to front.
struct BlendedSprite {
    state: SpriteState,
    textures: SpriteTextures,
    render_order: i32,
    /// Squared distance from the view's eye.
    depth: f32,
    instance: u32,
}

/// One sprite's draw: its pipeline state, textures and row.
#[derive(Clone, Copy)]
struct SpriteInstance {
    state: SpriteState,
    textures: SpriteTextures,
    instance: u32,
}

impl EffectsPass {
    /// Sort key of the `index`th blended sprite: (render order, squared
    /// distance from the eye), drawn in ascending order, farther first.
    pub fn blended_key(&self, index: usize) -> Option<(i32, f32)> {
        self.blended
            .get(index)
            .map(|sprite| (sprite.render_order, sprite.depth))
    }

    pub fn draws(&self) -> u32 {
        (self.solid.len() + self.blended.len() + self.soft_sprites.len() + self.billboards.len())
            as u32
            + u32::from(self.cubes > 0)
    }

    /// Whether any sprite or billboard needs the particle pass after the world's.
    pub fn soft(&self) -> bool {
        !self.soft_sprites.is_empty() || self.billboards.iter().any(|run| run.soft)
    }
}

/// An instance buffer that grows by doubling and is rewritten per view pass.
struct Rows {
    label: &'static str,
    buffer: wgpu::Buffer,
}

impl Rows {
    fn new(device: &wgpu::Device, label: &'static str) -> Self {
        Self {
            label,
            buffer: rows_buffer(device, label, 4096),
        }
    }

    fn write(&mut self, gpu: &Gpu, rows: &[f32]) {
        let bytes = (rows.len() * 4) as u64;
        if bytes > self.buffer.size() {
            self.buffer = rows_buffer(&gpu.device, self.label, bytes.next_power_of_two());
        }
        if !rows.is_empty() {
            gpu.queue
                .write_buffer(&self.buffer, 0, bytemuck::cast_slice(rows));
        }
    }
}

fn rows_buffer(device: &wgpu::Device, label: &str, size: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// Billboard pipelines by blend mode (`blend_index`).
type BlendPipelines = [wgpu::RenderPipeline; 2];

fn blend_index(blend: ParticleBlendMode) -> usize {
    match blend {
        ParticleBlendMode::Alpha => 0,
        ParticleBlendMode::Additive => 1,
    }
}

struct FormatPipelines {
    format: ColorTarget,
    sprites: HashMap<SpriteState, wgpu::RenderPipeline>,
    /// Hard billboards, depth-tested by the world pass.
    billboard: BlendPipelines,
    /// Soft billboards, testing the world's depth themselves; made at the
    /// first soft emitter.
    soft: Option<BlendPipelines>,
    cube: wgpu::RenderPipeline,
}

/// The soft billboard pass's layouts for one depth sample count: the world's
/// depth bound in group 2 (`effects.wgsl` `scene_depth*`).
struct SoftLayouts {
    depth: wgpu::BindGroupLayout,
    pipeline: wgpu::PipelineLayout,
}

/// Which `SoftLayouts` a view's depth takes: single-sample or multisampled.
fn soft_index(samples: u32) -> usize {
    usize::from(samples > 1)
}

pub(crate) struct Effects {
    shader: wgpu::ShaderModule,
    sprite_layout: wgpu::BindGroupLayout,
    particle_layout: wgpu::BindGroupLayout,
    sprite_pipeline_layout: wgpu::PipelineLayout,
    /// Soft sprites: the sprite layout with the world's depth in group 2.
    soft_sprite_layouts: [wgpu::PipelineLayout; 2],
    particle_pipeline_layout: wgpu::PipelineLayout,
    soft_layouts: [SoftLayouts; 2],
    cube_pipeline_layout: wgpu::PipelineLayout,
    corners: wgpu::Buffer,
    sprite_rows: Rows,
    particle_rows: Rows,
    cube_rows: Rows,
    formats: Vec<FormatPipelines>,
    sprite_bind_groups: HashMap<SpriteTextures, wgpu::BindGroup>,
    /// Particle sprite textures by slot, and each content hash's slot. A slot
    /// lives while an emitter or live particle holds it (`Particles`), then
    /// its binding is dropped and the slot reused.
    particle_textures: Vec<Option<ParticleTexture>>,
    particle_texture_ids: HashMap<String, u32>,
    free_particle_slots: Vec<u32>,
    nearest: wgpu::Sampler,
    // Scratch reused across view passes: no per-particle allocation per frame.
    sprite_scratch: Vec<SpriteDraw>,
    row_scratch: Vec<f32>,
    cube_scratch: Vec<f32>,
    /// (group: 0 cube, 1 + the billboard's blend and depth path; texture
    /// slot; particle index).
    order_scratch: Vec<(u32, u32, u32)>,
}

fn texture_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn sampler_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    }
}

impl Effects {
    pub fn new(
        device: &wgpu::Device,
        frame_layout: &wgpu::BindGroupLayout,
        shader: wgpu::ShaderModule,
    ) -> Self {
        let sprite_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("render-wgpu sprite"),
            entries: &[
                texture_entry(10),
                sampler_entry(11),
                texture_entry(12),
                sampler_entry(13),
            ],
        });
        let particle_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("render-wgpu particle"),
            entries: &[texture_entry(20), sampler_entry(21)],
        });
        let pipeline_layout = |label, groups: &[Option<&wgpu::BindGroupLayout>]| {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: groups,
                immediate_size: 0,
            })
        };
        let soft_layouts = [false, true].map(|multisampled| {
            let depth = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("render-wgpu particle scene depth"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 30 + u32::from(multisampled),
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled,
                    },
                    count: None,
                }],
            });
            SoftLayouts {
                pipeline: pipeline_layout(
                    "render-wgpu soft particle",
                    &[Some(frame_layout), Some(&particle_layout), Some(&depth)],
                ),
                depth,
            }
        });
        let soft_sprite_layouts = [0, 1].map(|index| {
            pipeline_layout(
                "render-wgpu soft sprite",
                &[
                    Some(frame_layout),
                    Some(&sprite_layout),
                    Some(&soft_layouts[index].depth),
                ],
            )
        });
        use wgpu::util::DeviceExt;
        let corners: [f32; 8] = [0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 1.0];
        Self {
            shader,
            sprite_pipeline_layout: pipeline_layout(
                "render-wgpu sprite",
                &[Some(frame_layout), Some(&sprite_layout)],
            ),
            soft_sprite_layouts,
            particle_pipeline_layout: pipeline_layout(
                "render-wgpu particle",
                &[Some(frame_layout), Some(&particle_layout)],
            ),
            soft_layouts,
            cube_pipeline_layout: pipeline_layout(
                "render-wgpu particle cube",
                &[Some(frame_layout)],
            ),
            sprite_layout,
            particle_layout,
            corners: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("render-wgpu quad corners"),
                contents: bytemuck::cast_slice(&corners),
                usage: wgpu::BufferUsages::VERTEX,
            }),
            sprite_rows: Rows::new(device, "render-wgpu sprite rows"),
            particle_rows: Rows::new(device, "render-wgpu particle rows"),
            cube_rows: Rows::new(device, "render-wgpu particle cube rows"),
            formats: Vec::new(),
            sprite_bind_groups: HashMap::new(),
            particle_textures: Vec::new(),
            particle_texture_ids: HashMap::new(),
            free_particle_slots: Vec::new(),
            nearest: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("render-wgpu particle"),
                ..Default::default()
            }),
            sprite_scratch: Vec::new(),
            row_scratch: Vec::new(),
            cube_scratch: Vec::new(),
            order_scratch: Vec::new(),
        }
    }

    /// Drop cached sprite bindings of a texture that was redefined or released.
    pub fn forget_texture(&mut self, id: u32) {
        self.sprite_bind_groups
            .retain(|(color, detail), _| *color != Some(id) && *detail != Some(id));
    }

    fn format_index(&mut self, device: &wgpu::Device, format: ColorTarget) -> usize {
        if let Some(index) = self.formats.iter().position(|set| set.format == format) {
            return index;
        }
        let billboard = [ParticleBlendMode::Alpha, ParticleBlendMode::Additive]
            .map(|blend| self.billboard_pipeline(device, format, blend, None));
        let cube_vertex = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2];
        let cube_instance = wgpu::vertex_attr_array![3 => Float32x4, 4 => Float32x4];
        let cube = self.pipeline(
            device,
            "render-wgpu particle cube",
            &self.cube_pipeline_layout,
            ("vs_particle_cube", "fs_particle_cube"),
            &[
                Some(wgpu::VertexBufferLayout {
                    array_stride: (VERTEX_FLOATS * 4) as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &cube_vertex,
                }),
                Some(wgpu::VertexBufferLayout {
                    array_stride: (CUBE_ROW_FLOATS * 4) as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &cube_instance,
                }),
            ],
            wgpu::PrimitiveTopology::TriangleList,
            Some(wgpu::Face::Back),
            format,
            // Cube particles are transparent but write depth.
            Some(depth_state(true, true)),
            Some(wgpu::BlendState::ALPHA_BLENDING),
        );
        self.formats.push(FormatPipelines {
            format,
            sprites: HashMap::new(),
            billboard,
            soft: None,
            cube,
        });
        self.formats.len() - 1
    }

    /// A billboard pipeline: depth-tested by the world pass, or, for the
    /// particle pass over a depth of `soft` samples, by its own shader.
    fn billboard_pipeline(
        &self,
        device: &wgpu::Device,
        format: ColorTarget,
        blend: ParticleBlendMode,
        soft: Option<u32>,
    ) -> wgpu::RenderPipeline {
        let corner = wgpu::VertexBufferLayout {
            array_stride: 8,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x2],
        };
        let particle_attributes = wgpu::vertex_attr_array![
            1 => Float32x4, 2 => Float32x4, 3 => Float32x4, 4 => Float32x4
        ];
        let (layout, fragment, depth) = match soft {
            None => (
                &self.particle_pipeline_layout,
                "fs_particle",
                Some(depth_state(true, false)),
            ),
            Some(1) => (&self.soft_layouts[0].pipeline, "fs_particle_soft", None),
            Some(_) => (
                &self.soft_layouts[1].pipeline,
                "fs_particle_soft_multisampled",
                None,
            ),
        };
        self.pipeline(
            device,
            "render-wgpu particle billboard",
            layout,
            ("vs_particle", fragment),
            &[
                Some(corner),
                Some(wgpu::VertexBufferLayout {
                    array_stride: (PARTICLE_ROW_FLOATS * 4) as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &particle_attributes,
                }),
            ],
            wgpu::PrimitiveTopology::TriangleStrip,
            None,
            format,
            depth,
            Some(blend_state(blend)),
        )
    }

    /// The soft billboard pipelines of this format, made at the first soft
    /// emitter. The view's depth has the format's sample count.
    fn ensure_soft_pipelines(&mut self, device: &wgpu::Device, format: ColorTarget) {
        let index = self.format_index(device, format);
        if self.formats[index].soft.is_some() {
            return;
        }
        let soft = [ParticleBlendMode::Alpha, ParticleBlendMode::Additive]
            .map(|blend| self.billboard_pipeline(device, format, blend, Some(format.samples)));
        self.formats[index].soft = Some(soft);
    }

    /// The world's depth bound for the particle pass of a view whose depth
    /// has `samples` per pixel.
    pub fn scene_depth_bind_group(
        &self,
        device: &wgpu::Device,
        samples: u32,
        depth: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        let index = soft_index(samples);
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("render-wgpu particle scene depth"),
            layout: &self.soft_layouts[index].depth,
            entries: &[wgpu::BindGroupEntry {
                binding: 30 + index as u32,
                resource: wgpu::BindingResource::TextureView(depth),
            }],
        })
    }

    fn ensure_sprite_pipeline(
        &mut self,
        device: &wgpu::Device,
        format: ColorTarget,
        state: SpriteState,
    ) {
        let index = self.format_index(device, format);
        if self.formats[index].sprites.contains_key(&state) {
            return;
        }
        let corner = wgpu::VertexBufferLayout {
            array_stride: 8,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x2],
        };
        let instance = wgpu::vertex_attr_array![
            1 => Float32x4, 2 => Float32x4, 3 => Float32x4, 4 => Float32x4,
            5 => Float32x4, 6 => Float32x4, 7 => Float32x4, 8 => Float32x4,
            9 => Float32x4
        ];
        // Solid sprites cover their pixel (`finish.rs`); soft ones test the
        // world's depth themselves in the particle pass.
        let (layout, fragment, depth) = if state.soft {
            let soft = soft_index(format.samples);
            (
                &self.soft_sprite_layouts[soft],
                if soft == 0 {
                    "fs_sprite_soft"
                } else {
                    "fs_sprite_soft_multisampled"
                },
                None,
            )
        } else if state.blend {
            (
                &self.sprite_pipeline_layout,
                "fs_sprite",
                Some(depth_state(state.depth_test, state.depth_write)),
            )
        } else {
            (
                &self.sprite_pipeline_layout,
                "fs_sprite_opaque",
                Some(depth_state(state.depth_test, state.depth_write)),
            )
        };
        let blend = state.blend.then(|| {
            blend_state(if state.additive {
                ParticleBlendMode::Additive
            } else {
                ParticleBlendMode::Alpha
            })
        });
        let pipeline = self.pipeline(
            device,
            "render-wgpu sprite",
            layout,
            ("vs_sprite", fragment),
            &[
                Some(corner),
                Some(wgpu::VertexBufferLayout {
                    array_stride: (SPRITE_ROW_FLOATS * 4) as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &instance,
                }),
            ],
            wgpu::PrimitiveTopology::TriangleStrip,
            None,
            format,
            depth,
            blend,
        );
        self.formats[index].sprites.insert(state, pipeline);
    }

    #[allow(clippy::too_many_arguments, reason = "one pipeline description")]
    fn pipeline(
        &self,
        device: &wgpu::Device,
        label: &str,
        layout: &wgpu::PipelineLayout,
        (vertex, fragment): (&str, &str),
        buffers: &[Option<wgpu::VertexBufferLayout<'_>>],
        topology: wgpu::PrimitiveTopology,
        cull_mode: Option<wgpu::Face>,
        format: ColorTarget,
        depth_stencil: Option<wgpu::DepthStencilState>,
        blend: Option<wgpu::BlendState>,
    ) -> wgpu::RenderPipeline {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: &self.shader,
                entry_point: Some(vertex),
                compilation_options: Default::default(),
                buffers,
            },
            primitive: wgpu::PrimitiveState {
                topology,
                cull_mode,
                ..Default::default()
            },
            depth_stencil,
            multisample: format.multisample(),
            fragment: Some(wgpu::FragmentState {
                module: &self.shader,
                entry_point: Some(fragment),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: format.format,
                    blend,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        })
    }

    /// Draw the solid sprites, between the world's opaque and blended parts.
    pub fn draw_solid_sprites(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        format: ColorTarget,
        effects: &EffectsPass,
    ) {
        for sprite in &effects.solid {
            self.draw_sprite(
                pass,
                format,
                sprite.state,
                &sprite.textures,
                sprite.instance,
            );
        }
    }

    /// Draw one blended sprite. The world pass interleaves these with its
    /// blended parts, so each call binds its own pipeline and buffers.
    pub fn draw_blended_sprite(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        format: ColorTarget,
        effects: &EffectsPass,
        index: usize,
    ) {
        if let Some(sprite) = effects.blended.get(index) {
            self.draw_sprite(
                pass,
                format,
                sprite.state,
                &sprite.textures,
                sprite.instance,
            );
        }
    }

    /// Draw the soft sprites in the particle pass, back to front, the
    /// world's depth bound in group 2.
    pub fn draw_soft_sprites(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        format: ColorTarget,
        effects: &EffectsPass,
    ) {
        for sprite in &effects.soft_sprites {
            self.draw_sprite(
                pass,
                format,
                sprite.state,
                &sprite.textures,
                sprite.instance,
            );
        }
    }

    fn draw_sprite(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        format: ColorTarget,
        state: SpriteState,
        textures: &SpriteTextures,
        instance: u32,
    ) {
        let Some(set) = self.formats.iter().find(|set| set.format == format) else {
            return;
        };
        let (Some(pipeline), Some(bind_group)) = (
            set.sprites.get(&state),
            self.sprite_bind_groups.get(textures),
        ) else {
            return;
        };
        pass.set_pipeline(pipeline);
        pass.set_vertex_buffer(0, self.corners.slice(..));
        pass.set_vertex_buffer(1, self.sprite_rows.buffer.slice(..));
        pass.set_bind_group(1, bind_group, &[]);
        pass.draw(0..4, instance..instance + 1);
    }

    pub fn draw_particles(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        format: ColorTarget,
        effects: &EffectsPass,
        cube: Option<&GpuMesh>,
    ) {
        let Some(set) = self.formats.iter().find(|set| set.format == format) else {
            return;
        };
        if let (true, Some(cube)) = (effects.cubes > 0, cube) {
            pass.set_pipeline(&set.cube);
            pass.set_vertex_buffer(0, cube.vertices.slice(..));
            pass.set_vertex_buffer(1, self.cube_rows.buffer.slice(..));
            pass.set_index_buffer(cube.indices.slice(..), wgpu::IndexFormat::Uint32);
            let count = cube.groups.first().map_or(0, |group| group.2);
            pass.draw_indexed(0..count, 0, 0..effects.cubes);
        }
        self.draw_billboards(pass, effects, &set.billboard, false);
    }

    /// Draw the soft billboards in the particle pass, the world's depth
    /// bound in group 2 (`scene_depth_bind_group`).
    pub fn draw_soft_particles(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        format: ColorTarget,
        effects: &EffectsPass,
    ) {
        if let Some(pipelines) = self
            .formats
            .iter()
            .find(|set| set.format == format)
            .and_then(|set| set.soft.as_ref())
        {
            self.draw_billboards(pass, effects, pipelines, true);
        }
    }

    fn draw_billboards(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        effects: &EffectsPass,
        pipelines: &BlendPipelines,
        soft: bool,
    ) {
        let mut bound = None;
        for run in effects.billboards.iter().filter(|run| run.soft == soft) {
            let Some(Some(texture)) = self.particle_textures.get(run.texture as usize) else {
                continue;
            };
            if bound.is_none() {
                pass.set_vertex_buffer(0, self.corners.slice(..));
                pass.set_vertex_buffer(1, self.particle_rows.buffer.slice(..));
            }
            if bound != Some(run.blend) {
                pass.set_pipeline(&pipelines[blend_index(run.blend)]);
                bound = Some(run.blend);
            }
            pass.set_bind_group(1, &texture.bind_group, &[]);
            pass.draw(0..4, run.first..run.first + run.count);
        }
    }
}

fn depth_state(test: bool, write: bool) -> wgpu::DepthStencilState {
    wgpu::DepthStencilState {
        format: DEPTH_FORMAT,
        depth_write_enabled: Some(write),
        depth_compare: Some(if test {
            wgpu::CompareFunction::LessEqual
        } else {
            wgpu::CompareFunction::Always
        }),
        stencil: Default::default(),
        bias: Default::default(),
    }
}

/// How a billboard's colour reaches the frame. Additive colour is
/// premultiplied by its alpha in the shader, so a faded particle adds
/// nothing; the frame's alpha keeps its coverage either way.
fn blend_state(blend: ParticleBlendMode) -> wgpu::BlendState {
    match blend {
        ParticleBlendMode::Alpha => wgpu::BlendState::ALPHA_BLENDING,
        ParticleBlendMode::Additive => wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendState::ALPHA_BLENDING.alpha,
        },
    }
}

struct ParticleTexture {
    content_hash: String,
    bind_group: wgpu::BindGroup,
}

/// A sprite material's alpha and depth state.
fn sprite_state(sprite: &SpriteInstanceDescriptor) -> (SpriteState, f32) {
    let (transparent, cutoff, alpha_write) = match sprite.material.alpha {
        SpriteAlphaMode::Opaque => (sprite.tint[3] < 1.0, 0.0, true),
        SpriteAlphaMode::Mask { cutoff } => (sprite.tint[3] < 1.0, cutoff, true),
        SpriteAlphaMode::Blend => (true, 0.0, false),
    };
    let material = &sprite.material;
    (
        SpriteState {
            depth_test: sprite.depth != SpriteDepthPolicy::DepthTestOff,
            depth_write: sprite.depth != SpriteDepthPolicy::DepthWriteOff && alpha_write,
            blend: transparent,
            additive: transparent && material.blend == SpriteBlendMode::Additive,
            soft: transparent
                && material.softness_metres > 0.0
                && sprite.depth != SpriteDepthPolicy::DepthTestOff,
        },
        cutoff,
    )
}

/// Lighting mode, strength and bias as `effects.wgsl` reads them, and the
/// detail texture it samples.
fn sprite_lighting(sprite: &SpriteRow, color: Option<u32>) -> ([f32; 3], Option<u32>) {
    let material = &sprite.descriptor.material;
    let effective = material.normal_strength * (1.0 - material.normal_bias * 0.5).clamp(0.5, 1.5);
    match material.lighting {
        SpriteLightingMode::Unlit => ([0.0, 0.0, 0.0], None),
        SpriteLightingMode::Synthetic => {
            ([1.0, material.normal_strength, material.normal_bias], None)
        }
        SpriteLightingMode::AuthoredNormal => ([2.0, effective, 0.0], sprite.detail),
        SpriteLightingMode::AuthoredDepth => ([3.0, effective, 0.0], sprite.detail),
        SpriteLightingMode::DerivedGradient => ([3.0, effective, 0.0], color),
    }
}

/// The authored detail texture a sprite's lighting mode samples, resolved to
/// a name id when the sprite is created.
pub(crate) fn sprite_detail_texture(sprite: &SpriteInstanceDescriptor) -> Option<&str> {
    let material = &sprite.material;
    match material.lighting {
        SpriteLightingMode::AuthoredNormal => material.normal_texture.as_deref(),
        SpriteLightingMode::AuthoredDepth => material.depth_texture.as_deref(),
        _ => None,
    }
}

/// The sprite's world quad for this camera: model matrix and plane rectangle.
fn sprite_quad(
    sprite: &SpriteInstanceDescriptor,
    world: &Mat4,
    frame_size: [f32; 2],
    camera: &CameraMatrices,
    viewport: PixelRect,
    pixel_ratio: f32,
) -> Option<(Mat4, [f32; 4])> {
    let (width, height) = (viewport.width as f32, viewport.height as f32);
    if let Some(placement) = sprite.viewport_placement {
        // The renderer owns the final screen rectangle.
        let target_width = width * placement.size[0];
        let target_height = height * placement.size[1];
        let frame_aspect = frame_size[0] / frame_size[1];
        let target_aspect = target_width / target_height;
        let stretch = placement.fit == SpriteViewportFit::Stretch;
        let quad_width = if stretch || frame_aspect >= target_aspect {
            target_width
        } else {
            target_height * frame_aspect
        };
        let quad_height = if stretch || frame_aspect <= target_aspect {
            target_height
        } else {
            target_width / frame_aspect
        };
        let left =
            width * placement.minimum[0] + (target_width - quad_width) * placement.alignment[0];
        let bottom =
            height * placement.minimum[1] + (target_height - quad_height) * placement.alignment[1];
        let ndc = Mat4::from_translation(Vec3::new(
            (left + quad_width * 0.5) / width * 2.0 - 1.0,
            (bottom + quad_height * 0.5) / height * 2.0 - 1.0,
            PLACEMENT_DEPTH,
        )) * Mat4::from_scale(Vec3::new(
            quad_width / width * 2.0,
            quad_height / height * 2.0,
            1.0,
        ));
        return Some((camera.view_proj.inverse() * ndc, [-0.5, -0.5, 0.5, 0.5]));
    }
    let [w, h] = frame_size;
    let quad = [
        -sprite.pivot[0] * w,
        -sprite.pivot[1] * h,
        (1.0 - sprite.pivot[0]) * w,
        (1.0 - sprite.pivot[1]) * h,
    ];
    if sprite.billboard == BillboardMode::None && sprite.size_mode == SpriteSizeMode::World {
        return Some((*world, quad));
    }
    let (mut scale, authored, position) = world.to_scale_rotation_translation();
    let camera_world = camera.view.inverse();
    let orthographic = camera.projection.w_axis.w == 1.0;
    if sprite.size_mode == SpriteSizeMode::Pixel {
        // World units per target pixel at the sprite's depth; the size is in
        // CSS pixels.
        let depth = if orthographic {
            1.0
        } else {
            let view_z = camera.view.transform_point3(position).z;
            if view_z >= 0.0 {
                // Behind the camera plane: no finite size this pass.
                return None;
            }
            -view_z
        };
        let per_pixel_x = 2.0 * depth / (camera.projection.x_axis.x * width);
        let per_pixel_y = 2.0 * depth / (camera.projection.y_axis.y * height);
        scale.x *= sprite.size[0] * pixel_ratio * per_pixel_x / w;
        scale.y *= sprite.size[1] * pixel_ratio * per_pixel_y / h;
    }
    let rotation = match sprite.billboard {
        BillboardMode::None => authored,
        BillboardMode::Spherical => Quat::from_mat4(&camera_world),
        BillboardMode::Cylindrical => {
            let mut forward = if orthographic {
                camera_world.z_axis.truncate()
            } else {
                camera.eye - position
            };
            forward.y = 0.0;
            if forward.length_squared() <= f32::EPSILON {
                forward = authored * Vec3::Z;
                forward.y = 0.0;
                if forward.length_squared() <= f32::EPSILON {
                    forward = Vec3::Z;
                }
            }
            let forward = forward.normalize();
            let right = Vec3::Y.cross(forward).normalize();
            Quat::from_mat3(&Mat3::from_cols(right, Vec3::Y, forward))
        }
    };
    Some((
        Mat4::from_scale_rotation_translation(scale, rotation, position),
        quad,
    ))
}

impl Renderer {
    /// Apply the renderer ops of one presentation delta: particles, ghost
    /// plates, animation controllers and video. `entities` resolves entity-attached
    /// anchors (the runtime passes `PresentationWorld::entity_world_position`).
    /// Billboard labels are drawn by the primary views; telemetry overlays
    /// (DOM UI) and audio are not renderer ops. Video playbacks end in facts
    /// ([`Renderer::take_video_facts`]), not issues.
    pub fn apply_presentation(
        &mut self,
        frame: &PresentationFrameDiff,
        resources: &dyn ResourceSource,
        entities: EntityPositions<'_>,
    ) -> Vec<ApplyIssue> {
        let mut issues = Vec::new();
        for op in &frame.ops {
            let issue = match op {
                PresentationOp::Particle { op, .. } => {
                    let visual = match op {
                        render_presentation::ParticleProjectionOp::Emit { descriptor, .. }
                        | render_presentation::ParticleProjectionOp::Create {
                            descriptor, ..
                        } => Some(&descriptor.visual),
                        render_presentation::ParticleProjectionOp::Update { patch, .. } => {
                            patch.visual.as_ref()
                        }
                        render_presentation::ParticleProjectionOp::Destroy { .. } => None,
                    };
                    let loaded = match visual {
                        Some(ParticleVisual::Billboard { sprite }) => {
                            self.particle_texture(sprite, resources).map(Some)
                        }
                        _ => Ok(None),
                    };
                    let result = loaded.and_then(|texture| {
                        let applied = self
                            .particles
                            .apply(op, texture, entities)
                            .map_err(|issue| format!("{issue:?}"));
                        // A refused op leaves a freshly loaded slot unheld.
                        if let Some(slot) = texture {
                            if !self.particles.holds_texture(slot) {
                                self.drop_particle_texture(slot);
                            }
                        }
                        applied
                    });
                    self.drop_released_particle_textures();
                    match result {
                        Ok(()) => None,
                        Err(detail) => Some(("particle", detail)),
                    }
                }
                PresentationOp::Billboard { op, .. } => self
                    .labels
                    .apply(op, resources, entities)
                    .err()
                    .map(|detail| ("billboard", detail)),
                PresentationOp::Animation { op, .. } => self
                    .apply_animation_op(op)
                    .err()
                    .map(|detail| ("animation", detail)),
                PresentationOp::GhostPlate { op, .. } => self
                    .apply_ghost_op(op, resources)
                    .err()
                    .map(|detail| ("ghostPlate", detail)),
                PresentationOp::Video { op, .. } => {
                    self.apply_video_op(op, resources);
                    None
                }
                // Audio has its own realizer; it is not a renderer op.
                PresentationOp::Audio { .. } => None,
            };
            if let Some((op, detail)) = issue {
                issues.push(ApplyIssue { op, detail });
            }
        }
        if !frame.ops.is_empty() {
            self.scene_generation += 1;
        }
        issues
    }

    /// Advance particles by `seconds` of Engine update time, and move entity
    /// anchored labels to their entities. The backend reads no clock: a held
    /// simulation passes no time and every particle holds.
    pub fn advance_effects(
        &mut self,
        seconds: f64,
        entities: EntityPositions<'_>,
    ) -> Vec<ApplyIssue> {
        self.labels.refresh_anchors(entities);
        if self.particles.is_empty() || seconds <= 0.0 {
            return Vec::new();
        }
        self.scene_generation += 1;
        let issues = self.particles.advance(seconds as f32, entities);
        self.drop_released_particle_textures();
        issues
            .into_iter()
            .map(|issue| ApplyIssue {
                op: "particle",
                detail: match issue {
                    ParticleIssue::Dropped(count) => {
                        format!("the emitter's max_particles dropped {count} particles")
                    }
                    other => format!("{other:?}"),
                },
            })
            .collect()
    }

    /// Live particles and retained emitters, for diagnostics and tests.
    pub fn particle_counts(&self) -> (usize, usize) {
        (
            self.particles.particles.len(),
            self.particles.emitter_count(),
        )
    }

    /// Drop the bindings of texture slots no emitter or particle holds.
    fn drop_released_particle_textures(&mut self) {
        for slot in self.particles.take_released_textures() {
            self.drop_particle_texture(slot);
        }
    }

    fn drop_particle_texture(&mut self, slot: u32) {
        if let Some(texture) = self
            .effects
            .particle_textures
            .get_mut(slot as usize)
            .and_then(Option::take)
        {
            self.effects
                .particle_texture_ids
                .remove(&texture.content_hash);
            self.effects.free_particle_slots.push(slot);
        }
    }

    /// The live particle texture bindings (a readout for lifecycle checks).
    pub fn particle_texture_count(&self) -> usize {
        self.effects.particle_texture_ids.len()
    }

    /// Load a particle sprite once per content hash, from the admitted
    /// texture resource or the retained texture of the same id, and return
    /// its slot.
    fn particle_texture(
        &mut self,
        sprite: &ParticleSpriteRef,
        resources: &dyn ResourceSource,
    ) -> Result<u32, String> {
        if let Some(slot) = self.effects.particle_texture_ids.get(&sprite.content_hash) {
            return Ok(*slot);
        }
        let identity = format!(
            "texture-resource/{}",
            sprite
                .content_hash
                .strip_prefix("sha256:")
                .unwrap_or(&sprite.content_hash)
        );
        let view = match resources.bytes(&identity) {
            Some(bytes) => {
                let image = resources::decode_png(&bytes)?;
                self.upload_rgba("render-wgpu particle sprite", &image)
            }
            None => match self.tables.textures.get(&sprite.asset) {
                Some(texture) => texture.view.clone(),
                None => return Err(format!("particle sprite {} is unavailable", sprite.asset)),
            },
        };
        let bind_group = self
            .gpu
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("render-wgpu particle sprite"),
                layout: &self.effects.particle_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 20,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 21,
                        resource: wgpu::BindingResource::Sampler(&self.effects.nearest),
                    },
                ],
            });
        let texture = ParticleTexture {
            content_hash: sprite.content_hash.clone(),
            bind_group,
        };
        let slot = match self.effects.free_particle_slots.pop() {
            Some(slot) => {
                self.effects.particle_textures[slot as usize] = Some(texture);
                slot
            }
            None => {
                self.effects.particle_textures.push(Some(texture));
                (self.effects.particle_textures.len() - 1) as u32
            }
        };
        self.effects
            .particle_texture_ids
            .insert(sprite.content_hash.clone(), slot);
        Ok(slot)
    }

    fn upload_rgba(&self, label: &str, image: &resources::DecodedImage) -> wgpu::TextureView {
        use wgpu::util::DeviceExt;
        self.gpu
            .device
            .create_texture_with_data(
                &self.gpu.queue,
                &wgpu::TextureDescriptor {
                    label: Some(label),
                    size: crate::target::extent(image.width, image.height),
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                },
                wgpu::util::TextureDataOrder::LayerMajor,
                &image.rgba,
            )
            .create_view(&Default::default())
    }

    /// A sprite texture by name id. Only a bind-group cache miss asks, so the
    /// name lookup stays off the per-sprite path.
    fn sprite_texture(&self, id: Option<u32>) -> &GpuTexture {
        id.and_then(|id| self.tables.textures.get(self.tables.names.name(id)))
            .unwrap_or(&self.white)
    }

    /// Build this view pass's sprite and particle rows and make their
    /// pipelines and bindings ready. Runs before the pass is encoded.
    /// `format` is the world's HDR target the view draws effects into.
    pub(crate) fn prepare_effects(
        &mut self,
        view: &ViewPass<'_>,
        format: ColorTarget,
    ) -> EffectsPass {
        let mut pass = EffectsPass::default();
        let viewmodel = view.layer == ViewLayer::Viewmodel;
        let pixel_ratio = self.pixel_ratio();
        let mut draws = std::mem::take(&mut self.effects.sprite_scratch);
        draws.clear();
        for handle in &self.tables.sprites {
            pass.sprite_candidates += 1;
            let Some(node) = self.tables.nodes.get(handle) else {
                continue;
            };
            let NodeKind::Sprite(resolved) = &node.kind else {
                continue;
            };
            if !node.world_visible || (node.world_layer == RenderLayer::Viewmodel) != viewmodel {
                continue;
            }
            let sprite = &resolved.descriptor;
            let atlas = self.tables.atlases.get(resolved.atlas);
            let rect = atlas.and_then(|atlas| atlas.descriptor.frame_rect(sprite.frame));
            let uv = rect.map_or([0.0, 0.0, 1.0, 1.0], |rect| {
                [
                    rect.uv_min[0],
                    rect.uv_min[1],
                    rect.uv_max[0],
                    rect.uv_max[1],
                ]
            });
            let frame_size = rect.and_then(|rect| rect.size).unwrap_or(sprite.size);
            let Some((model, quad)) = sprite_quad(
                sprite,
                &node.world,
                frame_size,
                &view.camera,
                view.viewport,
                pixel_ratio,
            ) else {
                continue;
            };
            let color = atlas.map(|atlas| atlas.texture);
            let ([mode, strength, bias], detail) = sprite_lighting(resolved, color);
            let (state, cutoff) = sprite_state(sprite);
            let mut row = [0.0; SPRITE_ROW_FLOATS];
            row[..16].copy_from_slice(&model.to_cols_array());
            row[16..20].copy_from_slice(&uv);
            row[20..24].copy_from_slice(&sprite.tint);
            row[24..28].copy_from_slice(&quad);
            row[28..32].copy_from_slice(&[mode, cutoff, strength, bias]);
            row[32..36].copy_from_slice(&[
                sprite.material.softness_metres,
                f32::from(state.additive),
                0.0,
                0.0,
            ]);
            draws.push(SpriteDraw {
                state,
                textures: (color, detail),
                render_order: sprite.render_order,
                depth: model.w_axis.truncate().distance_squared(view.camera.eye),
                row,
            });
        }
        // Solid front to back, blended back to front, each by render order.
        draws.sort_by(|a, b| {
            a.state
                .blend
                .cmp(&b.state.blend)
                .then(a.render_order.cmp(&b.render_order))
                .then(if a.state.blend {
                    b.depth.total_cmp(&a.depth)
                } else {
                    a.depth.total_cmp(&b.depth)
                })
        });
        let mut rows = std::mem::take(&mut self.effects.row_scratch);
        rows.clear();
        for draw in &draws {
            self.effects
                .ensure_sprite_pipeline(&self.gpu.device, format, draw.state);
            if !self.effects.sprite_bind_groups.contains_key(&draw.textures) {
                let color = self.sprite_texture(draw.textures.0);
                let detail = match draw.textures.1 {
                    Some(_) => self.sprite_texture(draw.textures.1),
                    None => color,
                };
                let bind_group = self
                    .gpu
                    .device
                    .create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("render-wgpu sprite"),
                        layout: &self.effects.sprite_layout,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 10,
                                resource: wgpu::BindingResource::TextureView(&color.view),
                            },
                            wgpu::BindGroupEntry {
                                binding: 11,
                                resource: wgpu::BindingResource::Sampler(&color.sampler),
                            },
                            wgpu::BindGroupEntry {
                                binding: 12,
                                resource: wgpu::BindingResource::TextureView(&detail.view),
                            },
                            wgpu::BindGroupEntry {
                                binding: 13,
                                resource: wgpu::BindingResource::Sampler(&detail.sampler),
                            },
                        ],
                    });
                self.effects
                    .sprite_bind_groups
                    .insert(draw.textures, bind_group);
            }
            let instance = (rows.len() / SPRITE_ROW_FLOATS) as u32;
            rows.extend_from_slice(&draw.row);
            if draw.state.soft {
                pass.soft_sprites.push(SpriteInstance {
                    state: draw.state,
                    textures: draw.textures,
                    instance,
                });
            } else if draw.state.blend {
                pass.blended.push(BlendedSprite {
                    state: draw.state,
                    textures: draw.textures,
                    render_order: draw.render_order,
                    depth: draw.depth,
                    instance,
                });
            } else {
                pass.solid.push(SpriteInstance {
                    state: draw.state,
                    textures: draw.textures,
                    instance,
                });
            }
        }
        self.effects.sprite_rows.write(&self.gpu, &rows);
        self.effects.sprite_scratch = draws;

        // Particles draw in world passes only.
        if !viewmodel && !self.particles.particles.is_empty() {
            self.effects.format_index(&self.gpu.device, format);
            let points = PARTICLE_PIXELS_PER_UNIT * pixel_ratio;
            let half = Vec4::new(
                points / view.viewport.width as f32,
                points / view.viewport.height as f32,
                0.0,
                0.0,
            );
            let mut order = std::mem::take(&mut self.effects.order_scratch);
            order.clear();
            let mut soft = false;
            for (index, particle) in self.particles.particles.iter().enumerate() {
                let descriptor = &particle.descriptor;
                let group = match &descriptor.visual {
                    ParticleVisual::Cube => Some((0, 0)),
                    // Hard billboards by blend then texture, then soft ones
                    // the same way: each run is one draw.
                    ParticleVisual::Billboard { .. } => particle.texture.map(|slot| {
                        let path = u32::from(descriptor.softness_metres > 0.0) << 1
                            | u32::from(descriptor.blend == ParticleBlendMode::Additive);
                        (1 + path, slot)
                    }),
                };
                if let Some((group, texture)) = group {
                    soft |= group >= 3;
                    order.push((group, texture, index as u32));
                }
            }
            if soft {
                self.effects.ensure_soft_pipelines(&self.gpu.device, format);
            }
            order.sort_unstable();
            rows.clear();
            let mut cubes = std::mem::take(&mut self.effects.cube_scratch);
            cubes.clear();
            for (group, texture, index) in &order {
                let particle = self.particles.particles[*index as usize].view();
                let color = [
                    srgb_to_linear(particle.color[0]),
                    srgb_to_linear(particle.color[1]),
                    srgb_to_linear(particle.color[2]),
                    particle.color[3],
                ];
                let p = particle.position;
                if *group == 0 {
                    cubes.extend_from_slice(&[p.x, p.y, p.z, particle.size.max(0.0)]);
                    cubes.extend_from_slice(&color);
                } else {
                    let descriptor = &self.particles.particles[*index as usize].descriptor;
                    let first = (rows.len() / PARTICLE_ROW_FLOATS) as u32;
                    match pass.billboards.last_mut() {
                        Some(run)
                            if run.texture == *texture
                                && run.blend == descriptor.blend
                                && run.soft == (*group >= 3) =>
                        {
                            run.count += 1;
                        }
                        _ => pass.billboards.push(BillboardRun {
                            texture: *texture,
                            blend: descriptor.blend,
                            soft: *group >= 3,
                            first,
                            count: 1,
                        }),
                    }
                    let frames = match &descriptor.visual {
                        ParticleVisual::Billboard { sprite } => {
                            f32::from(sprite.frame_count.max(1))
                        }
                        ParticleVisual::Cube => 1.0,
                    };
                    let size = particle.size.max(0.0);
                    // Screen size is a fixed clip offset scaled by depth in
                    // the shader; world size is a projected half edge.
                    let (half_x, half_y, world) = match descriptor.size_mode {
                        ParticleSizeMode::Screen => {
                            let size = size.max(1.0 / PARTICLE_PIXELS_PER_UNIT);
                            (half.x * size, half.y * size, 0.0)
                        }
                        ParticleSizeMode::World => (
                            0.5 * size * view.camera.projection.x_axis.x,
                            0.5 * size * view.camera.projection.y_axis.y,
                            1.0,
                        ),
                    };
                    let additive = f32::from(descriptor.blend == ParticleBlendMode::Additive);
                    rows.extend_from_slice(&[p.x, p.y, p.z, half_x]);
                    rows.extend_from_slice(&color);
                    rows.extend_from_slice(&[half_y, particle.frame as f32, frames, world]);
                    rows.extend_from_slice(&[
                        descriptor.softness_metres.max(0.0),
                        additive,
                        0.0,
                        0.0,
                    ]);
                }
            }
            pass.cubes = (cubes.len() / CUBE_ROW_FLOATS) as u32;
            self.effects.particle_rows.write(&self.gpu, &rows);
            self.effects.cube_rows.write(&self.gpu, &cubes);
            self.effects.cube_scratch = cubes;
            self.effects.order_scratch = order;
        }
        self.effects.row_scratch = rows;
        pass
    }
}
