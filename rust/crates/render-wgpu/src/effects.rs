//! Sprites and particles.
//!
//! Sprites are retained nodes (`NodeKind::Sprite`); their quads depend on the
//! camera (billboard orientation, pixel size, viewport placement), so each
//! view pass builds one instance row per visible sprite from the node's
//! propagated world matrix. There is no parent walk. Frames come from the
//! retained descriptor, and `UpdateSprite` carries playback, which the Engine
//! advances on update time.
//!
//! Parts rigs (`NodeKind::PartsRig`, `parts_rigs.rs`) draw as sprites do:
//! each view places a rig's plane by a sprite row, and the rig's pieces,
//! built into its own buffers once per pose, draw on it in runs.
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
//!
//! Beside a motion target (`motion.rs`), particles write their motion from
//! where they were drawn in the previous frame; sprites write none.

use std::collections::HashMap;

use glam::{Mat3, Mat4, Quat, Vec3, Vec4};
use render_model::{
    BillboardMode, PartsRigBlend, PartsRigInstanceDescriptor, RenderHandle, SpriteAlphaMode,
    SpriteBlendMode, SpriteDepthPolicy, SpriteInstanceDescriptor, SpriteLightingMode,
    SpriteMaterialDescriptor, SpriteSizeMode, SpriteViewportFit,
};
use render_presentation::{
    ParticleBlendMode, ParticleSizeMode, ParticleSpriteRef, ParticleVisual, PresentationFrameDiff,
    PresentationOp,
};

use crate::camera::CameraMatrices;
use crate::frame::{PixelRect, ViewLayer, ViewPass};
use crate::motion::MotionWrite;
use crate::particles::{EntityPositions, ParticleIssue};
use crate::parts_rigs::{RigGeometry, RigRun, RIG_VERTEX_BYTES};
use crate::pipelines::VERTEX_FLOATS;
use crate::resources::{self, ResourceSource};
use crate::tables::{GpuMesh, GpuTexture, NodeKind, SpriteRow};
use crate::target::{ColorTarget, DEPTH_FORMAT};
use crate::{ApplyIssue, Gpu, Renderer};

/// Particle billboards are drawn `size × 24` pixels across.
const PARTICLE_PIXELS_PER_UNIT: f32 = 24.0;
const SPRITE_ROW_FLOATS: usize = 40;
const PARTICLE_ROW_FLOATS: usize = 16;
const CUBE_ROW_FLOATS: usize = 8;
/// Beside a motion target, each particle row and cube row also holds where
/// the particle was drawn in the previous frame (`motion.rs`).
const PREVIOUS_FLOATS: usize = 4;
/// A sprite batch instance's row: world centre, atlas frame, quad.
const BATCH_ROW_FLOATS: usize = 12;
/// A sprite batch's corner vertex: corner, then what every instance shares
/// (tint, lighting, softness and facing, the node's axes, emission).
const BATCH_CORNER_FLOATS: usize = 26;
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
    /// A sprite batch: its instances' rows are its own buffer, drawn at once.
    batch: bool,
}

/// Pipeline state of one run of a parts rig's pieces.
///
/// A rig's pieces lie on one plane, but each triangle's depth is its own
/// interpolation of it: on a plane turned from the view, a later piece can
/// land a little behind an earlier one. So the pieces' colours are drawn
/// without writing depth, in the pose's order, and a solid rig's depth is
/// written after them by a pass of its normal pieces (`depth_write`) that
/// leaves the colour as it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct RigState {
    depth_test: bool,
    /// The depth pass.
    depth_write: bool,
    /// Drawn as solid sprites are: normal pieces cover their pixels.
    solid: bool,
    blend: PartsRigBlend,
}

/// Colour, detail (normal or height), tint mask and emission map texture
/// name ids (`Tables::names`); `None` is the 1×1 white texture, or the colour
/// texture again for the detail slot. The white mask tints the whole sprite,
/// and the white map emits over all of it.
type SpriteTextures = (Option<u32>, Option<u32>, Option<u32>, Option<u32>);

struct SpriteDraw {
    state: SpriteState,
    textures: SpriteTextures,
    render_order: i32,
    depth: f32,
    /// Unused by a batch, which draws its own rows.
    row: [f32; SPRITE_ROW_FLOATS],
    batch: Option<RenderHandle>,
    /// A parts rig's view, placed by the row: its pieces draw from its
    /// buffers.
    rig: Option<RigKey>,
    /// A solid rig whose pose has additive, multiply or screen pieces: they
    /// blend over what is behind them, so it draws with the blended draws,
    /// back to front after everything solid, still solid itself.
    blends_over: bool,
}

impl SpriteDraw {
    /// Drawn with the blended draws, back to front.
    fn late(&self) -> bool {
        self.state.blend || self.blends_over
    }
}

/// One view of a parts rig node: the node and the view's index.
type RigKey = (RenderHandle, u32);

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
    /// Sprites the pass's batches draw.
    pub batch_instances: u32,
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
    batch: Option<RenderHandle>,
    rig: Option<RigKey>,
}

/// One sprite's draw: its pipeline state, textures and row, or the batch
/// whose rows it draws.
#[derive(Clone, Copy)]
struct SpriteInstance {
    state: SpriteState,
    textures: SpriteTextures,
    instance: u32,
    batch: Option<RenderHandle>,
    rig: Option<RigKey>,
}

/// A parts rig's buffers, built from its pose and drawing when either
/// changes, not per view, and dropped with its node.
struct RigBuffers {
    /// The pose and drawing generation they were built from.
    pose: u64,
    generation: u64,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    runs: Vec<RigRun>,
    /// Its vertices' bounds in rig units: min x, min y, max x, max y.
    bounds: [f32; 4],
    /// Its lighting reads its pages' detail maps (authored normal or depth).
    detail: bool,
}

/// A sprite batch's buffers, built when the batch, its node's place or its
/// shared values change, not per view, and dropped with its node.
struct BatchBuffers {
    /// The instances, node matrix and atlas the rows were built from.
    instances: std::sync::Arc<[render_model::SpriteBatchInstance]>,
    world: Mat4,
    rows: wgpu::Buffer,
    count: u32,
    corner_values: [f32; BATCH_CORNER_FLOATS * 4],
    corners: wgpu::Buffer,
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
    rigs: HashMap<RigState, wgpu::RenderPipeline>,
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
    /// The same with MOTION, for particles beside a motion target, made
    /// when one first draws (`prepare_effects`).
    motion_shader: Option<wgpu::ShaderModule>,
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
    /// Sprite batches' buffers, by node.
    batches: HashMap<RenderHandle, BatchBuffers>,
    /// Parts rigs' buffers, by node and view.
    rigs: HashMap<RigKey, RigBuffers>,
    rig_scratch: RigGeometry,
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
                texture_entry(14),
                sampler_entry(15),
                texture_entry(16),
                sampler_entry(17),
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
        let corners: [f32; 8] = [0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 1.0];
        Self {
            shader,
            motion_shader: None,
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
            corners: crate::gpu::buffer_init(
                device,
                &wgpu::util::BufferInitDescriptor {
                    label: Some("render-wgpu quad corners"),
                    contents: bytemuck::cast_slice(&corners),
                    usage: wgpu::BufferUsages::VERTEX,
                },
            ),
            sprite_rows: Rows::new(device, "render-wgpu sprite rows"),
            particle_rows: Rows::new(device, "render-wgpu particle rows"),
            cube_rows: Rows::new(device, "render-wgpu particle cube rows"),
            formats: Vec::new(),
            sprite_bind_groups: HashMap::new(),
            batches: HashMap::new(),
            rigs: HashMap::new(),
            rig_scratch: RigGeometry::default(),
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

    /// Rebuild every sprite batch's rows at its next draw: an atlas, and so
    /// its frames, changed.
    pub fn forget_sprite_batches(&mut self) {
        self.batches.clear();
    }

    /// Drop a destroyed or replaced node's sprite batch or parts rig
    /// buffers.
    pub fn forget_sprite_batch(&mut self, node: RenderHandle) {
        self.batches.remove(&node);
        self.rigs.retain(|(handle, _), _| *handle != node);
    }

    /// Drop cached sprite bindings of a texture that was redefined or released.
    pub fn forget_texture(&mut self, id: u32) {
        self.sprite_bind_groups.retain(|textures, _| {
            ![textures.0, textures.1, textures.2, textures.3].contains(&Some(id))
        });
    }

    /// A parts rig's draw in this view: the rig view nearest the bearing
    /// the camera sees it from, its buffers made ready for its pose, and the
    /// row placing its plane. None when that view has no pose, drawing or
    /// piece to draw, or no size this view.
    fn prepare_rig(
        &mut self,
        gpu: &Gpu,
        handle: RenderHandle,
        rig: &crate::parts_rigs::PartsRigRow,
        assets: &crate::tables::Slots<crate::parts_rigs::PartsRigAssetRow>,
        world: &Mat4,
        (camera, viewport, pixel_ratio): (&CameraMatrices, PixelRect, f32),
    ) -> Option<SpriteDraw> {
        let descriptor = &rig.descriptor;
        let index = descriptor.view_seen(&world.to_cols_array_2d(), &rig_camera(camera));
        let (view, row_view) = (&descriptor.views[index], rig.views.get(index)?);
        let key = (handle, index as u32);
        let asset = assets.get(row_view.asset)?;
        let pose = row_view.pose.as_ref()?;
        let stale = self.rigs.get(&key).is_none_or(|buffers| {
            buffers.pose != row_view.pose_revision || buffers.generation != asset.generation
        });
        if stale {
            let geometry = &mut self.rig_scratch;
            crate::parts_rigs::build(asset, pose, geometry);
            let vertices: &[u8] = bytemuck::cast_slice(&geometry.vertices);
            let indices: &[u8] = bytemuck::cast_slice(&geometry.indices);
            let fits = self.rigs.get(&key).is_some_and(|buffers| {
                buffers.vertices.size() >= vertices.len() as u64
                    && buffers.indices.size() >= indices.len() as u64
            });
            if !fits {
                let buffer = |label, bytes: usize, usage| {
                    gpu.device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some(label),
                        size: (bytes.max(256) as u64).next_power_of_two(),
                        usage: usage | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    })
                };
                self.rigs.insert(
                    key,
                    RigBuffers {
                        pose: 0,
                        generation: 0,
                        vertices: buffer(
                            "render-wgpu parts rig vertices",
                            vertices.len(),
                            wgpu::BufferUsages::VERTEX,
                        ),
                        indices: buffer(
                            "render-wgpu parts rig indices",
                            indices.len(),
                            wgpu::BufferUsages::INDEX,
                        ),
                        runs: Vec::new(),
                        bounds: [0.0; 4],
                        detail: false,
                    },
                );
            }
            let buffers = self.rigs.get_mut(&key)?;
            if !vertices.is_empty() {
                gpu.queue.write_buffer(&buffers.vertices, 0, vertices);
                gpu.queue.write_buffer(&buffers.indices, 0, indices);
            }
            buffers.pose = row_view.pose_revision;
            buffers.generation = asset.generation;
            buffers.runs.clone_from(&geometry.runs);
            buffers.bounds = geometry.bounds;
        }
        let buffers = self.rigs.get_mut(&key)?;
        buffers.detail = matches!(
            descriptor.material.lighting,
            SpriteLightingMode::AuthoredNormal | SpriteLightingMode::AuthoredDepth
        );
        let buffers = &*buffers;
        let first = *buffers.runs.first()?;
        let mirror = if view.mirror { -1.0 } else { 1.0 };
        let model = match &descriptor.viewport_placement {
            Some(placement) => {
                rig_viewport_model(placement, mirror, descriptor.texel_snap, camera, viewport)
            }
            None => {
                let pixels = (descriptor.size_mode == SpriteSizeMode::Pixel)
                    .then_some([descriptor.scale; 2]);
                let placed = match pixels {
                    Some(_) => *world,
                    None => *world * Mat4::from_scale(Vec3::splat(descriptor.scale)),
                };
                facing_model(
                    &placed,
                    descriptor.billboard,
                    pixels,
                    descriptor.material.authored_facing,
                    camera,
                    viewport,
                    pixel_ratio,
                )?
            }
        };
        let (state, cutoff) = parts_rig_state(descriptor);
        let material = &descriptor.material;
        let ([mode, strength, bias], _) = material_lighting(material, first.detail, None);
        let mut row = [0.0; SPRITE_ROW_FLOATS];
        row[..16].copy_from_slice(&model.to_cols_array());
        row[16..18].copy_from_slice(&[descriptor.texel_snap, mirror]);
        // The frame it draws inside, in its plane's (mirrored) units.
        row[24..28].copy_from_slice(&match &descriptor.viewport_placement {
            Some(placement) => {
                let [x0, y0, x1, y1] = placement.frame;
                if view.mirror {
                    [-x1, y0, -x0, y1]
                } else {
                    [x0, y0, x1, y1]
                }
            }
            None => [f32::MIN, f32::MIN, f32::MAX, f32::MAX],
        });
        row[20..24].copy_from_slice(&descriptor.tint);
        row[28..32].copy_from_slice(&[mode, cutoff, strength, bias]);
        row[36..39].copy_from_slice(
            &(Vec3::from(material.emission_color) * material.emission_intensity).to_array(),
        );
        let [x0, y0, x1, y1] = buffers.bounds;
        let center =
            model.transform_point3(Vec3::new((x0 + x1) * 0.5 * mirror, (y0 + y1) * 0.5, 0.0));
        Some(SpriteDraw {
            state,
            textures: rig_textures(&first, buffers.detail),
            render_order: descriptor.render_order,
            depth: center.distance_squared(camera.eye),
            row,
            batch: None,
            rig: Some(key),
            blends_over: buffers
                .runs
                .iter()
                .any(|run| run.blend != PartsRigBlend::Normal),
        })
    }

    fn format_index(&mut self, device: &wgpu::Device, format: ColorTarget) -> usize {
        if let Some(index) = self.formats.iter().position(|set| set.format == format) {
            return index;
        }
        let billboard = [ParticleBlendMode::Alpha, ParticleBlendMode::Additive]
            .map(|blend| self.billboard_pipeline(device, format, blend, None));
        let cube_vertex = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2];
        let cube_instance =
            wgpu::vertex_attr_array![3 => Float32x4, 4 => Float32x4, 5 => Float32x4];
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
                    array_stride: (cube_row_floats(format) * 4) as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &cube_instance[..2 + usize::from(format.motion)],
                }),
            ],
            wgpu::PrimitiveTopology::TriangleList,
            Some(wgpu::Face::Back),
            format,
            // Cube particles are transparent but write depth.
            Some(depth_state(true, true)),
            Some(wgpu::BlendState::ALPHA_BLENDING),
            MotionWrite::Blend,
        );
        self.formats.push(FormatPipelines {
            format,
            sprites: HashMap::new(),
            rigs: HashMap::new(),
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
            1 => Float32x4, 2 => Float32x4, 3 => Float32x4, 4 => Float32x4, 5 => Float32x4
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
                    array_stride: (particle_row_floats(format) * 4) as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &particle_attributes[..4 + usize::from(format.motion)],
                }),
            ],
            wgpu::PrimitiveTopology::TriangleStrip,
            None,
            format,
            depth,
            Some(blend_state(blend)),
            MotionWrite::Blend,
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

    /// The layout of the world's depth for the particle pass, single-sample
    /// (0) and multisampled (1): the precipitation pass binds the same.
    pub fn scene_depth_layouts(&self) -> [&wgpu::BindGroupLayout; 2] {
        [&self.soft_layouts[0].depth, &self.soft_layouts[1].depth]
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
            9 => Float32x4, 10 => Float32x4
        ];
        let batch_corner = wgpu::vertex_attr_array![
            0 => Float32x2, 1 => Float32x4, 2 => Float32x4, 3 => Float32x4,
            4 => Float32x4, 5 => Float32x4, 9 => Float32x4
        ];
        let batch_instance =
            wgpu::vertex_attr_array![6 => Float32x4, 7 => Float32x4, 8 => Float32x4];
        let buffers = if state.batch {
            [
                wgpu::VertexBufferLayout {
                    array_stride: (BATCH_CORNER_FLOATS * 4) as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &batch_corner,
                },
                wgpu::VertexBufferLayout {
                    array_stride: (BATCH_ROW_FLOATS * 4) as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &batch_instance,
                },
            ]
        } else {
            [
                corner,
                wgpu::VertexBufferLayout {
                    array_stride: (SPRITE_ROW_FLOATS * 4) as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &instance,
                },
            ]
        };
        let [first, second] = buffers;
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
            (
                if state.batch {
                    "vs_sprite_batch"
                } else {
                    "vs_sprite"
                },
                fragment,
            ),
            &[Some(first), Some(second)],
            wgpu::PrimitiveTopology::TriangleStrip,
            None,
            format,
            depth,
            blend,
            MotionWrite::None,
        );
        self.formats[index].sprites.insert(state, pipeline);
    }

    fn ensure_rig_pipeline(&mut self, device: &wgpu::Device, format: ColorTarget, state: RigState) {
        let index = self.format_index(device, format);
        if self.formats[index].rigs.contains_key(&state) {
            return;
        }
        let vertex = wgpu::vertex_attr_array![
            0 => Float32x2, 11 => Float32x2, 12 => Unorm8x4, 13 => Unorm8x4, 14 => Unorm16x4
        ];
        let instance = wgpu::vertex_attr_array![
            1 => Float32x4, 2 => Float32x4, 3 => Float32x4, 4 => Float32x4,
            5 => Float32x4, 6 => Float32x4, 7 => Float32x4, 8 => Float32x4,
            9 => Float32x4, 10 => Float32x4
        ];
        let opaque = state.solid && state.blend == PartsRigBlend::Normal;
        // The depth pass keeps the colour the pieces drew.
        let keep = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::Zero,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        };
        let blend = if state.depth_write {
            Some(wgpu::BlendState {
                color: keep,
                alpha: keep,
            })
        } else {
            (!opaque).then(|| rig_blend_state(state.blend))
        };
        let pipeline = self.pipeline(
            device,
            "render-wgpu parts rig",
            &self.sprite_pipeline_layout,
            (
                "vs_parts_rig",
                if opaque {
                    "fs_parts_rig_opaque"
                } else {
                    "fs_parts_rig"
                },
            ),
            &[
                Some(wgpu::VertexBufferLayout {
                    array_stride: RIG_VERTEX_BYTES,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &vertex,
                }),
                Some(wgpu::VertexBufferLayout {
                    array_stride: (SPRITE_ROW_FLOATS * 4) as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &instance,
                }),
            ],
            wgpu::PrimitiveTopology::TriangleList,
            None,
            format,
            Some(depth_state(state.depth_test, state.depth_write)),
            blend,
            MotionWrite::None,
        );
        self.formats[index].rigs.insert(state, pipeline);
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
        motion: MotionWrite,
    ) -> wgpu::RenderPipeline {
        // Only a pipeline writing motion needs the shader that writes it.
        let shader = match &self.motion_shader {
            Some(shader) if format.motion && motion != MotionWrite::None => shader,
            _ => &self.shader,
        };
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: shader,
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
                module: shader,
                entry_point: Some(fragment),
                compilation_options: Default::default(),
                targets: &format.targets(
                    wgpu::ColorTargetState {
                        format: format.format,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    },
                    motion,
                ),
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
            self.draw_sprite(pass, format, sprite);
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
                &SpriteInstance {
                    state: sprite.state,
                    textures: sprite.textures,
                    instance: sprite.instance,
                    batch: sprite.batch,
                    rig: sprite.rig,
                },
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
            self.draw_sprite(pass, format, sprite);
        }
    }

    fn draw_sprite(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        format: ColorTarget,
        sprite: &SpriteInstance,
    ) {
        let Some(set) = self.formats.iter().find(|set| set.format == format) else {
            return;
        };
        if let Some(key) = sprite.rig {
            self.draw_rig(pass, set, sprite, key);
            return;
        }
        let (Some(pipeline), Some(bind_group)) = (
            set.sprites.get(&sprite.state),
            self.sprite_bind_groups.get(&sprite.textures),
        ) else {
            return;
        };
        pass.set_pipeline(pipeline);
        pass.set_bind_group(1, bind_group, &[]);
        match sprite.batch {
            Some(handle) => {
                let Some(batch) = self.batches.get(&handle) else {
                    return;
                };
                pass.set_vertex_buffer(0, batch.corners.slice(..));
                pass.set_vertex_buffer(1, batch.rows.slice(..));
                pass.draw(0..4, 0..batch.count);
            }
            None => {
                pass.set_vertex_buffer(0, self.corners.slice(..));
                pass.set_vertex_buffer(1, self.sprite_rows.buffer.slice(..));
                pass.draw(0..4, sprite.instance..sprite.instance + 1);
            }
        }
    }

    /// Draw a parts rig's runs in order, each with its page and blend mode,
    /// on the plane its sprite row places.
    fn draw_rig(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        set: &FormatPipelines,
        sprite: &SpriteInstance,
        key: RigKey,
    ) {
        let Some(rig) = self.rigs.get(&key) else {
            return;
        };
        pass.set_vertex_buffer(0, rig.vertices.slice(..));
        pass.set_vertex_buffer(1, self.sprite_rows.buffer.slice(..));
        pass.set_index_buffer(rig.indices.slice(..), wgpu::IndexFormat::Uint32);
        for run in &rig.runs {
            let (Some(pipeline), Some(bind_group)) = (
                set.rigs.get(&rig_state(sprite.state, run.blend)),
                self.sprite_bind_groups.get(&rig_textures(run, rig.detail)),
            ) else {
                continue;
            };
            pass.set_pipeline(pipeline);
            pass.set_bind_group(1, bind_group, &[]);
            pass.draw_indexed(
                run.first..run.first + run.count,
                0,
                sprite.instance..sprite.instance + 1,
            );
        }
        let Some(depth) = rig_depth_state(sprite.state).and_then(|state| set.rigs.get(&state))
        else {
            return;
        };
        pass.set_pipeline(depth);
        for run in rig
            .runs
            .iter()
            .filter(|run| run.blend == PartsRigBlend::Normal)
        {
            if let Some(bind_group) = self.sprite_bind_groups.get(&rig_textures(run, rig.detail)) {
                pass.set_bind_group(1, bind_group, &[]);
                pass.draw_indexed(
                    run.first..run.first + run.count,
                    0,
                    sprite.instance..sprite.instance + 1,
                );
            }
        }
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

/// Floats in one particle row, and one cube row, drawn into `format`.
fn particle_row_floats(format: ColorTarget) -> usize {
    PARTICLE_ROW_FLOATS + if format.motion { PREVIOUS_FLOATS } else { 0 }
}

fn cube_row_floats(format: ColorTarget) -> usize {
    CUBE_ROW_FLOATS + if format.motion { PREVIOUS_FLOATS } else { 0 }
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

/// How a run of a rig's premultiplied pieces reaches the frame, by its slot
/// blend mode; the frame's alpha keeps its coverage either way.
fn rig_blend_state(blend: PartsRigBlend) -> wgpu::BlendState {
    use wgpu::BlendFactor::{Dst, One, OneMinusSrc, OneMinusSrcAlpha};
    let (src_factor, dst_factor) = match blend {
        PartsRigBlend::Normal => (One, OneMinusSrcAlpha),
        PartsRigBlend::Additive => (One, One),
        PartsRigBlend::Multiply => (Dst, OneMinusSrcAlpha),
        PartsRigBlend::Screen => (One, OneMinusSrc),
    };
    wgpu::BlendState {
        color: wgpu::BlendComponent {
            src_factor,
            dst_factor,
            operation: wgpu::BlendOperation::Add,
        },
        alpha: wgpu::BlendComponent {
            src_factor: One,
            dst_factor: OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        },
    }
}

/// A run's colour pipeline state, from its rig's sprite state: solid when
/// the rig is.
fn rig_state(state: SpriteState, blend: PartsRigBlend) -> RigState {
    RigState {
        depth_test: state.depth_test,
        depth_write: false,
        solid: !state.blend,
        blend,
    }
}

/// The depth pass of a rig whose sprite state writes depth: a solid one.
fn rig_depth_state(state: SpriteState) -> Option<RigState> {
    (state.depth_write && !state.blend).then_some(RigState {
        depth_test: state.depth_test,
        depth_write: true,
        solid: true,
        blend: PartsRigBlend::Normal,
    })
}

/// A run's sprite bindings: its page as the colour texture, and its detail
/// map when the rig's lighting reads one; otherwise the page again, which
/// derived-gradient lighting reads.
fn rig_textures(run: &RigRun, detail: bool) -> SpriteTextures {
    (Some(run.texture), run.detail.filter(|_| detail), None, None)
}

/// A parts rig's alpha and depth state, as `sprite_state` gives a sprite's.
/// A rig placed in the viewport draws over its layer without depth, so the
/// finish pass, which fogs by depth, leaves it as drawn.
fn parts_rig_state(rig: &PartsRigInstanceDescriptor) -> (SpriteState, f32) {
    let (cutoff, alpha_write) = match rig.material.alpha {
        SpriteAlphaMode::Opaque => (0.0, true),
        SpriteAlphaMode::Mask { cutoff } => (cutoff, true),
        SpriteAlphaMode::Blend => (0.0, false),
    };
    let placed = rig.viewport_placement.is_some();
    (
        SpriteState {
            depth_test: rig.depth != SpriteDepthPolicy::DepthTestOff && !placed,
            depth_write: rig.depth != SpriteDepthPolicy::DepthWriteOff && alpha_write && !placed,
            blend: rig.blended(),
            additive: false,
            soft: false,
            batch: false,
        },
        cutoff,
    )
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
            batch: false,
        },
        cutoff,
    )
}

/// Lighting mode, strength and bias as `effects.wgsl` reads them, and the
/// detail texture it samples.
fn sprite_lighting(sprite: &SpriteRow, color: Option<u32>) -> ([f32; 3], Option<u32>) {
    material_lighting(&sprite.descriptor.material, sprite.detail, color)
}

fn material_lighting(
    material: &SpriteMaterialDescriptor,
    detail: Option<u32>,
    color: Option<u32>,
) -> ([f32; 3], Option<u32>) {
    let effective = material.normal_strength * (1.0 - material.normal_bias * 0.5).clamp(0.5, 1.5);
    match material.lighting {
        SpriteLightingMode::Unlit => ([0.0, 0.0, 0.0], None),
        SpriteLightingMode::Synthetic => {
            ([1.0, material.normal_strength, material.normal_bias], None)
        }
        SpriteLightingMode::AuthoredNormal => ([2.0, effective, 0.0], detail),
        SpriteLightingMode::AuthoredDepth => ([3.0, effective, 0.0], detail),
        SpriteLightingMode::DerivedGradient => ([3.0, effective, 0.0], color),
    }
}

/// A sprite's emitted light before its map and its wind bend:
/// `effects.wgsl`'s `emission`. A sprite placed in the viewport has no world
/// to lean in.
fn sprite_emission(sprite: &SpriteInstanceDescriptor) -> [f32; 4] {
    let material = &sprite.material;
    let bend = if sprite.viewport_placement.is_some() {
        0.0
    } else {
        material.wind_bend
    };
    (Vec3::from(material.emission_color) * material.emission_intensity)
        .extend(bend)
        .to_array()
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

/// A sprite batch's corner vertices: each corner with what the instances
/// share (`effects.wgsl` `SpriteBatchIn`).
fn batch_corners(
    sprite: &SpriteInstanceDescriptor,
    world: &Mat4,
    params: [f32; 4],
    additive: bool,
) -> [f32; BATCH_CORNER_FLOATS * 4] {
    let facing = match sprite.billboard {
        BillboardMode::None => 0.0,
        BillboardMode::Spherical => 1.0,
        BillboardMode::Cylindrical => 2.0,
        BillboardMode::CylindricalView => 3.0,
    };
    let right = world.x_axis.truncate().try_normalize().unwrap_or(Vec3::X);
    let up = world.y_axis.truncate().try_normalize().unwrap_or(Vec3::Y);
    let mut values = [0.0; BATCH_CORNER_FLOATS * 4];
    for (index, corner) in [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]]
        .into_iter()
        .enumerate()
    {
        let vertex = &mut values[index * BATCH_CORNER_FLOATS..][..BATCH_CORNER_FLOATS];
        vertex[..2].copy_from_slice(&corner);
        vertex[2..6].copy_from_slice(&sprite.tint);
        vertex[6..10].copy_from_slice(&params);
        vertex[10..14].copy_from_slice(&[
            sprite.material.softness_metres,
            f32::from(additive),
            facing,
            f32::from(sprite.material.wind_whole_texels),
        ]);
        vertex[14..18].copy_from_slice(&right.extend(sprite.material.authored_facing).to_array());
        vertex[18..22].copy_from_slice(&up.extend(0.0).to_array());
        vertex[22..26].copy_from_slice(&sprite_emission(sprite));
    }
    values
}

/// A sprite batch's instance rows: world centre, atlas frame and the quad
/// around the centre, the node's scale and the instance's applied.
fn batch_rows(
    sprite: &SpriteInstanceDescriptor,
    rows: &crate::tables::SpriteBatchRows,
    world: &Mat4,
    atlas: Option<&crate::tables::AtlasRow>,
) -> Vec<f32> {
    let scale = [
        world.x_axis.truncate().length(),
        world.y_axis.truncate().length(),
    ];
    let mut values = Vec::with_capacity(rows.instances.len() * BATCH_ROW_FLOATS);
    for instance in rows.instances.iter() {
        let rect = atlas.and_then(|atlas| atlas.descriptor.frame_rect(instance.frame));
        let uv = rect.map_or([0.0, 0.0, 1.0, 1.0], |rect| {
            [
                rect.uv_min[0],
                rect.uv_min[1],
                rect.uv_max[0],
                rect.uv_max[1],
            ]
        });
        let [w, h] = rect.and_then(|rect| rect.size).unwrap_or(sprite.size);
        let (w, h) = (w * instance.scale * scale[0], h * instance.scale * scale[1]);
        let center = world.transform_point3(Vec3::from(instance.position));
        values.extend_from_slice(&center.extend(1.0).to_array());
        values.extend_from_slice(&uv);
        values.extend_from_slice(&[
            -sprite.pivot[0] * w,
            -sprite.pivot[1] * h,
            (1.0 - sprite.pivot[0]) * w,
            (1.0 - sprite.pivot[1]) * h,
        ]);
    }
    values
}

impl Renderer {
    /// A sprite batch's instances with their reach, for the backdrop's
    /// depth range.
    pub(crate) fn sprite_batch_rows(
        &self,
        batch: &render_model::SpriteBatchDescriptor,
    ) -> crate::tables::SpriteBatchRows {
        let instances = &batch.instances;
        let (min, max) = instances.iter().fold(
            (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
            |(min, max), instance| {
                let position = Vec3::from(instance.position);
                (min.min(position), max.max(position))
            },
        );
        let center = if instances.is_empty() {
            Vec3::ZERO
        } else {
            (min + max) * 0.5
        };
        // The largest frame any instance may show, at its scale.
        let largest_frame = self
            .tables
            .names
            .get(&batch.sprite.asset)
            .and_then(|id| self.tables.atlases.get(id))
            .map_or(0.0, |atlas| {
                atlas
                    .descriptor
                    .frames
                    .iter()
                    .filter_map(|frame| frame.size)
                    .fold(0.0f32, |largest, [w, h]| largest.max(w).max(h))
            })
            .max(batch.sprite.size[0])
            .max(batch.sprite.size[1]);
        let radius = instances.iter().fold(0.0f32, |radius, instance| {
            radius.max(
                Vec3::from(instance.position).distance(center) + largest_frame * instance.scale,
            )
        });
        crate::tables::SpriteBatchRows {
            instances: instances.clone(),
            center,
            radius,
        }
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
        // A rectangle at one depth in the view stays one in the world, so
        // its matrix is affine like any sprite's: lit sprites read its
        // normal and place from it.
        let (x, y) = (
            (left + quad_width * 0.5) / width * 2.0 - 1.0,
            (bottom + quad_height * 0.5) / height * 2.0 - 1.0,
        );
        let (half_x, half_y) = (quad_width / width, quad_height / height);
        let unproject = camera.view_proj.inverse();
        let world = |x, y| unproject.project_point3(Vec3::new(x, y, PLACEMENT_DEPTH));
        let right = world(x + half_x, y) - world(x - half_x, y);
        let up = world(x, y + half_y) - world(x, y - half_y);
        let facing = right.cross(up).try_normalize().unwrap_or(Vec3::Z);
        let model = Mat4::from_cols(
            right.extend(0.0),
            up.extend(0.0),
            facing.extend(0.0),
            world(x, y).extend(1.0),
        );
        return Some((model, [-0.5, -0.5, 0.5, 0.5]));
    }
    let [w, h] = frame_size;
    let quad = [
        -sprite.pivot[0] * w,
        -sprite.pivot[1] * h,
        (1.0 - sprite.pivot[0]) * w,
        (1.0 - sprite.pivot[1]) * h,
    ];
    let pixels = (sprite.size_mode == SpriteSizeMode::Pixel)
        .then(|| [sprite.size[0] / w, sprite.size[1] / h]);
    let model = facing_model(
        world,
        sprite.billboard,
        pixels,
        sprite.material.authored_facing,
        camera,
        viewport,
        pixel_ratio,
    )?;
    Some((model, quad))
}

/// The matrix that draws a viewport-placed rig's plane for `camera`: from
/// rig units (mirrored for a mirrored view, as `effects.wgsl` mirrors them)
/// to the world, a little in front of the eye, so that its frame fits its
/// rectangle of `viewport` (`PartsRigViewportPlacement`). With a texel snap,
/// each texel is a whole number of the viewport's pixels (where at least
/// one fits) and the rig's origin, on which its texel grid lies, is on a
/// pixel corner, so each pixel's centre lies in exactly one texel. It draws
/// only inside its frame (`effects.wgsl` `shaded_parts_rig`).
fn rig_viewport_model(
    placement: &render_model::PartsRigViewportPlacement,
    mirror: f32,
    texel_snap: f32,
    camera: &CameraMatrices,
    viewport: PixelRect,
) -> Mat4 {
    /// How far below a whole number of pixels per texel a fit still counts
    /// as it: a 381-pixel square over 127 texels is 3, not 2.9999998.
    const WHOLE_PIXELS: f32 = 1e-3;
    let (width, height) = (viewport.width as f32, viewport.height as f32);
    let rectangle = placement.rectangle;
    let [x0, y0, x1, y1] = placement.frame;
    let (x0, x1) = if mirror < 0.0 { (-x1, -x0) } else { (x0, x1) };
    let (frame_width, frame_height) = (x1 - x0, y1 - y0);
    let (target_width, target_height) = (width * rectangle.size[0], height * rectangle.size[1]);
    // Pixels per rig unit, across and up.
    let mut scale = match rectangle.fit {
        SpriteViewportFit::Stretch => [target_width / frame_width, target_height / frame_height],
        SpriteViewportFit::Contain => {
            [(target_width / frame_width).min(target_height / frame_height); 2]
        }
    };
    if texel_snap > 0.0 {
        scale = scale.map(|scale| {
            let per_texel = scale * texel_snap;
            if per_texel + WHOLE_PIXELS >= 1.0 {
                (per_texel + WHOLE_PIXELS).floor() / texel_snap
            } else {
                scale
            }
        });
    }
    let (quad_width, quad_height) = (frame_width * scale[0], frame_height * scale[1]);
    let left = width * rectangle.minimum[0] + (target_width - quad_width) * rectangle.alignment[0];
    let bottom =
        height * rectangle.minimum[1] + (target_height - quad_height) * rectangle.alignment[1];
    // The rig's origin in pixels from the viewport's lower left.
    let mut origin = [left - x0 * scale[0], bottom - y0 * scale[1]];
    if texel_snap > 0.0 {
        origin = origin.map(f32::round);
    }
    // A rectangle at one depth in the view stays one in the world, so the
    // matrix is affine like a placed sprite's.
    let unproject = camera.view_proj.inverse();
    let world = |[x, y]: [f32; 2]| {
        unproject.project_point3(Vec3::new(
            x / width * 2.0 - 1.0,
            y / height * 2.0 - 1.0,
            PLACEMENT_DEPTH,
        ))
    };
    let at = world(origin);
    let right = (world([origin[0] + scale[0] * frame_width, origin[1]]) - at) / frame_width;
    let up = (world([origin[0], origin[1] + scale[1] * frame_height]) - at) / frame_height;
    let facing = right.cross(up).try_normalize().unwrap_or(Vec3::Z);
    Mat4::from_cols(
        right.extend(0.0),
        up.extend(0.0),
        facing.extend(0.0),
        at.extend(1.0),
    )
}

/// `camera` as a parts rig's placement reads it.
pub(crate) fn rig_camera(camera: &CameraMatrices) -> render_model::PartsRigCamera {
    let camera_world = camera.view.inverse();
    render_model::PartsRigCamera {
        eye: camera.eye.to_array(),
        right: camera_world.x_axis.truncate().to_array(),
        up: camera_world.y_axis.truncate().to_array(),
        forward: (-camera_world.z_axis.truncate()).to_array(),
        orthographic: camera.projection.w_axis.w == 1.0,
    }
}

/// The matrix that places a plane facing the camera by `billboard`, from
/// its node's world matrix: x right and y up on the plane, z its facing.
/// `pixels`, for a pixel-sized plane, is CSS pixels per plane unit across
/// and up; otherwise the node's matrix sizes it. None when a pixel-sized
/// plane lies behind the camera.
fn facing_model(
    world: &Mat4,
    billboard: BillboardMode,
    pixels: Option<[f32; 2]>,
    authored_facing: f32,
    camera: &CameraMatrices,
    viewport: PixelRect,
    pixel_ratio: f32,
) -> Option<Mat4> {
    let (width, height) = (viewport.width as f32, viewport.height as f32);
    if billboard == BillboardMode::None && pixels.is_none() {
        return Some(*world);
    }
    let (mut scale, authored, position) = world.to_scale_rotation_translation();
    let camera_world = camera.view.inverse();
    let orthographic = camera.projection.w_axis.w == 1.0;
    if let Some([across, up]) = pixels {
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
        scale.x *= across * pixel_ratio * per_pixel_x;
        scale.y *= up * pixel_ratio * per_pixel_y;
    }
    let rotation = match billboard {
        BillboardMode::None => authored,
        BillboardMode::Spherical => Quat::from_mat4(&camera_world),
        BillboardMode::Cylindrical | BillboardMode::CylindricalView => {
            let toward = if orthographic || billboard == BillboardMode::CylindricalView {
                camera_world.z_axis.truncate()
            } else {
                camera.eye - position
            };
            let forward = upright_facing(toward, authored * Vec3::Z, authored_facing);
            let right = Vec3::Y.cross(forward).normalize();
            Quat::from_mat3(&Mat3::from_cols(right, Vec3::Y, forward))
        }
    };
    Some(Mat4::from_scale_rotation_translation(
        scale, rotation, position,
    ))
}

/// [`render_model::upright_facing`] over glam vectors.
pub(crate) fn upright_facing(toward: Vec3, authored: Vec3, keep: f32) -> Vec3 {
    Vec3::from(render_model::upright_facing(
        toward.to_array(),
        authored.to_array(),
        keep,
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
                        // An update of an unknown emitter leaves a freshly
                        // loaded slot unheld.
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
                // Controllers drive the poses the Engine evaluates; the
                // renderer draws those.
                PresentationOp::Animation { .. } => None,
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

    /// The sprite batches holding GPU buffers (a readout for lifecycle
    /// checks).
    pub fn sprite_batch_buffer_count(&self) -> usize {
        self.effects.batches.len()
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
    /// Make the bind group of a sprite's textures, or a rig page's, ready.
    fn ensure_sprite_bindings(&mut self, textures: SpriteTextures) {
        if self.effects.sprite_bind_groups.contains_key(&textures) {
            return;
        }
        let color = self.sprite_texture(textures.0);
        let detail = match textures.1 {
            Some(_) => self.sprite_texture(textures.1),
            None => color,
        };
        let tint_mask = self.sprite_texture(textures.2);
        let emission_map = self.sprite_texture(textures.3);
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
                    wgpu::BindGroupEntry {
                        binding: 14,
                        resource: wgpu::BindingResource::TextureView(&tint_mask.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 15,
                        resource: wgpu::BindingResource::Sampler(&tint_mask.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 16,
                        resource: wgpu::BindingResource::TextureView(&emission_map.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 17,
                        resource: wgpu::BindingResource::Sampler(&emission_map.sampler),
                    },
                ],
            });
        self.effects.sprite_bind_groups.insert(textures, bind_group);
    }

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
        if format.motion && self.effects.motion_shader.is_none() {
            self.effects.motion_shader =
                Some(crate::pipelines::standard(self.layouts.shaders.module(
                    &self.gpu.device,
                    crate::shaders::Entry::Effects,
                    crate::shaders::Features::MOTION,
                )));
        }
        // Sprites draw in their own layer; particles in the world, or in the
        // backdrop when their emitter says so (none in the viewmodel).
        let backdrop = view.layer == ViewLayer::Backdrop;
        let takes_particles = view.layer != ViewLayer::Viewmodel;
        let pixel_ratio = self.pixel_ratio();
        let mut draws = std::mem::take(&mut self.effects.sprite_scratch);
        draws.clear();
        for handle in &self.tables.sprites {
            pass.sprite_candidates += 1;
            let Some(node) = self.tables.nodes.get(handle) else {
                continue;
            };
            if !node.world_visible || ViewLayer::of(node.world_layer) != view.layer {
                continue;
            }
            let resolved = match &node.kind {
                NodeKind::Sprite(resolved) => resolved,
                NodeKind::PartsRig(rig) => {
                    draws.extend(self.effects.prepare_rig(
                        &self.gpu,
                        *handle,
                        rig,
                        &self.tables.parts_rigs,
                        &node.world,
                        (&view.camera, view.viewport, pixel_ratio),
                    ));
                    continue;
                }
                _ => continue,
            };
            let sprite = &resolved.descriptor;
            let atlas = self.tables.atlases.get(resolved.atlas);
            if let Some(rows) = &resolved.batch {
                let color = atlas.map(|atlas| atlas.texture);
                let ([mode, strength, bias], detail) = sprite_lighting(resolved, color);
                let (mut state, cutoff) = sprite_state(sprite);
                state.batch = true;
                let corner_values = batch_corners(
                    sprite,
                    &node.world,
                    [mode, cutoff, strength, bias],
                    state.additive,
                );
                let stale = self.effects.batches.get(handle).is_none_or(|buffers| {
                    !std::sync::Arc::ptr_eq(&buffers.instances, &rows.instances)
                        || buffers.world != node.world
                });
                if stale {
                    let rows_values = batch_rows(sprite, rows, &node.world, atlas);
                    let buffer = |label, contents: &[f32]| {
                        crate::gpu::buffer_init(
                            &self.gpu.device,
                            &wgpu::util::BufferInitDescriptor {
                                label: Some(label),
                                contents: bytemuck::cast_slice(contents),
                                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                            },
                        )
                    };
                    // An empty batch still binds a buffer of one row.
                    let rows_buffer = buffer(
                        "render-wgpu sprite batch rows",
                        if rows_values.is_empty() {
                            &[0.0; BATCH_ROW_FLOATS]
                        } else {
                            &rows_values
                        },
                    );
                    self.effects.batches.insert(
                        *handle,
                        BatchBuffers {
                            instances: rows.instances.clone(),
                            world: node.world,
                            rows: rows_buffer,
                            count: rows.instances.len() as u32,
                            corner_values,
                            corners: buffer("render-wgpu sprite batch corners", &corner_values),
                        },
                    );
                }
                let buffers = self
                    .effects
                    .batches
                    .get_mut(handle)
                    .expect("a drawn batch has buffers");
                if buffers.corner_values != corner_values {
                    // A tint, lighting or emission change: four vertices,
                    // not the rows.
                    self.gpu.queue.write_buffer(
                        &buffers.corners,
                        0,
                        bytemuck::cast_slice(&corner_values),
                    );
                    buffers.corner_values = corner_values;
                }
                if buffers.count == 0 {
                    continue;
                }
                pass.batch_instances += buffers.count;
                draws.push(SpriteDraw {
                    state,
                    textures: (color, detail, resolved.tint_mask, resolved.emission_map),
                    render_order: sprite.render_order,
                    depth: node
                        .world
                        .transform_point3(rows.center)
                        .distance_squared(view.camera.eye),
                    row: [0.0; SPRITE_ROW_FLOATS],
                    batch: Some(*handle),
                    rig: None,
                    blends_over: false,
                });
                continue;
            }
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
                f32::from(sprite.material.wind_whole_texels),
            ]);
            row[36..40].copy_from_slice(&sprite_emission(sprite));
            draws.push(SpriteDraw {
                state,
                textures: (color, detail, resolved.tint_mask, resolved.emission_map),
                render_order: sprite.render_order,
                depth: model.w_axis.truncate().distance_squared(view.camera.eye),
                row,
                batch: None,
                rig: None,
                blends_over: false,
            });
        }
        // Solid front to back, blended back to front, each by render order.
        draws.sort_by(|a, b| {
            a.late()
                .cmp(&b.late())
                .then(a.render_order.cmp(&b.render_order))
                .then(if a.late() {
                    b.depth.total_cmp(&a.depth)
                } else {
                    a.depth.total_cmp(&b.depth)
                })
        });
        let mut rows = std::mem::take(&mut self.effects.row_scratch);
        rows.clear();
        for draw in &draws {
            match draw.rig.and_then(|key| self.effects.rigs.get(&key)) {
                Some(rig) => {
                    let detail = rig.detail;
                    for run in rig.runs.clone() {
                        let state = rig_state(draw.state, run.blend);
                        self.effects
                            .ensure_rig_pipeline(&self.gpu.device, format, state);
                        self.ensure_sprite_bindings(rig_textures(&run, detail));
                    }
                    if let Some(depth) = rig_depth_state(draw.state) {
                        self.effects
                            .ensure_rig_pipeline(&self.gpu.device, format, depth);
                    }
                }
                None => {
                    self.effects
                        .ensure_sprite_pipeline(&self.gpu.device, format, draw.state);
                    self.ensure_sprite_bindings(draw.textures);
                }
            }
            let instance = (rows.len() / SPRITE_ROW_FLOATS) as u32;
            // A batch draws its own rows; a rig is placed by its row.
            if draw.batch.is_none() {
                rows.extend_from_slice(&draw.row);
            }
            if draw.state.soft {
                pass.soft_sprites.push(SpriteInstance {
                    state: draw.state,
                    textures: draw.textures,
                    instance,
                    batch: draw.batch,
                    rig: draw.rig,
                });
            } else if draw.late() {
                pass.blended.push(BlendedSprite {
                    state: draw.state,
                    textures: draw.textures,
                    render_order: draw.render_order,
                    depth: draw.depth,
                    instance,
                    batch: draw.batch,
                    rig: draw.rig,
                });
            } else {
                pass.solid.push(SpriteInstance {
                    state: draw.state,
                    textures: draw.textures,
                    instance,
                    batch: draw.batch,
                    rig: draw.rig,
                });
            }
        }
        self.effects.sprite_rows.write(&self.gpu, &rows);
        self.effects.sprite_scratch = draws;

        // Particles draw in world passes only.
        if takes_particles && !self.particles.particles.is_empty() {
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
                if descriptor.backdrop != backdrop {
                    continue;
                }
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
            let row_floats = particle_row_floats(format);
            for (group, texture, index) in &order {
                let live = &self.particles.particles[*index as usize];
                let particle = live.view();
                // Linear, as every other world colour is.
                let color = particle.color;
                let p = particle.position;
                let previous = live.previous;
                if *group == 0 {
                    cubes.extend_from_slice(&[p.x, p.y, p.z, particle.size.max(0.0)]);
                    cubes.extend_from_slice(&color);
                    if format.motion {
                        cubes.extend_from_slice(&[previous.x, previous.y, previous.z, 0.0]);
                    }
                } else {
                    let descriptor = &live.descriptor;
                    let first = (rows.len() / row_floats) as u32;
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
                    if format.motion {
                        rows.extend_from_slice(&[previous.x, previous.y, previous.z, 0.0]);
                    }
                }
            }
            pass.cubes = (cubes.len() / cube_row_floats(format)) as u32;
            self.effects.particle_rows.write(&self.gpu, &rows);
            self.effects.cube_rows.write(&self.gpu, &cubes);
            self.effects.cube_scratch = cubes;
            self.effects.order_scratch = order;
        }
        self.effects.row_scratch = rows;
        pass
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::{camera_matrices, CameraPose};
    use render_host_contracts::RendererCameraProjection;
    use render_model::{PartsRigInstanceDescriptor, PartsRigView};

    fn rig(billboard: BillboardMode, authored_facing: f32) -> PartsRigInstanceDescriptor {
        let view = |heading: f32, mirror| PartsRigView {
            asset: "parts-rig/a".to_owned(),
            heading,
            mirror,
        };
        PartsRigInstanceDescriptor {
            views: vec![
                view(0.0, false),
                view(-std::f32::consts::FRAC_PI_2, false),
                view(std::f32::consts::PI, false),
                view(std::f32::consts::FRAC_PI_2, true),
            ],
            scale: 0.002,
            size_mode: SpriteSizeMode::World,
            billboard,
            tint: [1.0; 4],
            render_order: 0,
            depth: SpriteDepthPolicy::Default,
            layer: Default::default(),
            material: render_model::SpriteMaterialDescriptor {
                authored_facing,
                ..Default::default()
            },
            texel_snap: 0.0,
            viewport_placement: None,
            visible: true,
            transform: Default::default(),
            metadata: Default::default(),
        }
    }

    /// `place_point`, which the services answer bone reads with, puts a
    /// rig point where the pass draws it: the view `prepare_rig` picks, its
    /// mirror (`effects.wgsl` negates x) and `facing_model`'s plane.
    #[test]
    fn a_placed_rig_point_is_where_the_pass_draws_it() {
        let worlds = [
            Mat4::from_translation(Vec3::new(1.0, 0.5, -2.0)),
            Mat4::from_scale_rotation_translation(
                Vec3::new(1.5, 0.8, 1.2),
                Quat::from_rotation_y(0.7),
                Vec3::new(-3.0, 0.0, 4.0),
            ),
            Mat4::from_scale_rotation_translation(
                Vec3::new(-1.0, 1.0, 1.0),
                Quat::from_rotation_y(-2.0),
                Vec3::new(0.0, 1.0, 0.0),
            ),
        ];
        let projections = [
            RendererCameraProjection::Perspective {
                fov_y_degrees: 50.0,
                near: 0.1,
                far: 100.0,
            },
            RendererCameraProjection::Orthographic {
                vertical_size: 6.0,
                near: 0.1,
                far: 100.0,
            },
        ];
        let viewport = PixelRect {
            x: 0,
            y: 0,
            width: 640,
            height: 360,
        };
        let point = [37.0, 412.0];
        let mut views = std::collections::BTreeSet::new();
        for billboard in [
            BillboardMode::None,
            BillboardMode::Spherical,
            BillboardMode::Cylindrical,
            BillboardMode::CylindricalView,
        ] {
            for keep in [0.0, 0.4] {
                let rig = rig(billboard, keep);
                for world in &worlds {
                    for projection in &projections {
                        for bearing in [0.0f32, 70.0, 135.0, 200.0, 290.0] {
                            let (sin, cos) = bearing.to_radians().sin_cos();
                            let eye =
                                world.w_axis.truncate() + Vec3::new(sin * 6.0, 2.0, cos * 6.0);
                            let look = (world.w_axis.truncate() - eye).normalize();
                            let orientation = Quat::from_rotation_arc(Vec3::NEG_Z, look);
                            let camera = camera_matrices(
                                CameraPose {
                                    position: eye,
                                    orientation,
                                },
                                projection,
                                16.0 / 9.0,
                            );
                            let placed = rig
                                .place_point(&world.to_cols_array_2d(), &rig_camera(&camera), point)
                                .expect("a world-sized rig places its points");
                            let view =
                                rig.view_seen(&world.to_cols_array_2d(), &rig_camera(&camera));
                            assert_eq!(placed.view, view);
                            views.insert(view);
                            let model = facing_model(
                                &(*world * Mat4::from_scale(Vec3::splat(rig.scale))),
                                billboard,
                                None,
                                keep,
                                &camera,
                                viewport,
                                1.0,
                            )
                            .unwrap();
                            let mirror = if rig.views[view].mirror { -1.0 } else { 1.0 };
                            let drawn =
                                model.transform_point3(Vec3::new(point[0] * mirror, point[1], 0.0));
                            let error = (drawn - Vec3::from(placed.world)).length();
                            assert!(
                                error < 1e-4,
                                "{billboard:?} keep {keep} bearing {bearing}: {drawn} vs {:?}",
                                placed.world
                            );
                        }
                    }
                }
            }
        }
        assert_eq!(views.len(), 4, "every view is seen: {views:?}");
    }
}
