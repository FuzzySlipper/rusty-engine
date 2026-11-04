//! Ray-traced global illumination (`RendererOptions::global_illumination`):
//! a port of Bevy's Solari (`bevy_solari`, read at 84e2d2cc5; see
//! `ray_gi.wgsl`) without its ReSTIR, for a forward renderer.
//!
//! A world view's opaque pipelines draw direct light alone (`RAY_GI`). The
//! world pass then splits, as it does for water, and between its halves:
//!
//! - **Light tiles:** 128 tiles of 1024 samples of the light rows and the
//!   emissive parts ([`Source`]), which every path's light sampling reads.
//! - **World cache:** a hash grid of irradiance over the surfaces paths
//!   reach. Cells a path queried live on; each frame a share of the active
//!   ones samples the lights and one bounce into the cache.
//! - **Paths:** one per pixel from the view's depth and guides, in one of
//!   the surface's lobes, ending in the cache where the lobe is wide. The
//!   primary surface takes no light row: the forward pass drew those.
//! - **Composite:** each lobe's light times its guide's albedo, added to
//!   the HDR picture with its alpha untouched. Water's scene colour and the
//!   blended parts, which keep the probes and the sky's light, draw over it.
//!
//! Paths stop at the opaque parts that cast shadows
//! (`ray_scene::MASK_PATH`): grass that casts none would darken the ground
//! under it past anything the forward pass shows. The pool holds those parts'
//! vertices and each row their albedo, roughness and emission
//! (`RayScene::pool_all`, `ray_shadows.rs`). A texture past the slots reads
//! as its material's mean. Every ray tests masks.
//!
//! One sample a pixel is noise: Ray Reconstruction denoises it, so global
//! illumination draws only where it runs (`Renderer::ray_gi_refusal`).
//! Paths write what it reads ([`RayGi::guides`]): on a smooth surface the
//! motion of what it reflects, and on a mirror the surface its chain ends on,
//! in place of the mirror, in textures of their own: the composite keeps the
//! world's guides, and blended parts draw into the world's motion after the
//! paths. `resolve_guides` merges them once those have drawn.

use glam::{Mat4, Vec3};

use crate::frame::PixelRect;
use crate::shaders::Entry;
use crate::timing::{untimed, GpuPassTiming, PassTimer};
use crate::{Renderer, SettingRefusal};

const CACHE_CELLS: u64 = 1 << 20;
/// `WorldCache` in `ray_gi.wgsl`: the cells' arrays (checksum, life,
/// radiance, geometry, luminance delta, new radiance, a, active index), then
/// `b` before the active index, and the count.
const CACHE_CELL_BYTES: u64 = 4 + 4 + 16 + 32 + 4 + 16 + 4 + 4;
const CACHE_BLOCKS: u32 = 1024;
const CACHE_COUNT_OFFSET: u64 = CACHE_CELLS * CACHE_CELL_BYTES + CACHE_BLOCKS as u64 * 4;
const CACHE_BYTES: u64 = CACHE_COUNT_OFFSET + 16;
const LIGHT_TILES: u32 = 128;
const TILE_SAMPLES: u64 = 1024;
/// `GiLight` in `ray_gi.wgsl`: four vectors.
const LIGHT_BYTES: u64 = 64;
/// `GiSource` in `ray_gi.wgsl`: four vectors.
const SOURCE_WORDS: usize = 16;
const SOURCE_BYTES: u64 = (SOURCE_WORDS * 4) as u64;
/// `GiParams` in `ray_gi.wgsl`.
const PARAMS_BYTES: u64 = 64 * 2 + 16 * 6;
const PIXEL_WORKGROUP: u32 = 8;
/// The cache's dispatches and the scale of the frame's random seed.
const CELL_WORKGROUP: u32 = 1024;
const FRAME_SEED: u32 = 5_782_582;
const IRRADIANCE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
const GUIDE_RECORD_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;
const ALBEDO_GUIDE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const NORMAL_GUIDE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
const MOTION_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R32Float;
const TILES_PASS: &str = "gi-light-tiles";
const CACHE_PASS: &str = "gi-world-cache";
const PATHS_PASS: &str = "gi-paths";

/// An emissive part a path's light sampling may land on: its id, world
/// transform by rows and triangles.
#[derive(Clone, PartialEq)]
pub(crate) struct EmissivePart {
    pub part: crate::tables::PartId,
    pub rows: [[f32; 4]; 3],
    pub triangles: u32,
}

/// What the light tiles and a miss read of the world's light rows.
#[derive(Default)]
struct LightRows {
    rows: Vec<u32>,
    /// The ambient rows' radiance and the hemisphere rows' above and below.
    ambient: [f32; 3],
    sky: [f32; 3],
    ground: [f32; 3],
}

/// A kind of light row (`ray_gi.wgsl` `sample_source`, `rusty::lighting`).
const LIGHT_AMBIENT: u32 = 0;
const LIGHT_HEMISPHERE: u32 = 1;

fn light_rows(rows: &[f32], floats: usize) -> LightRows {
    let mut lights = LightRows::default();
    for (index, row) in rows.chunks_exact(floats).enumerate() {
        let colour = [row[0], row[1], row[2]].map(|c| c / std::f32::consts::PI);
        match row[3] as u32 {
            LIGHT_AMBIENT => {
                lights.ambient = std::array::from_fn(|c| lights.ambient[c] + colour[c])
            }
            LIGHT_HEMISPHERE => {
                lights.sky = std::array::from_fn(|c| lights.sky[c] + colour[c]);
                lights.ground =
                    std::array::from_fn(|c| lights.ground[c] + row[12 + c] / std::f32::consts::PI);
            }
            _ => lights.rows.push(index as u32),
        }
    }
    lights
}

struct Targets {
    size: (u32, u32),
    irradiance: wgpu::TextureView,
    specular: wgpu::TextureView,
    /// What each path said of its pixel for `resolve_guides`.
    record: wgpu::TextureView,
    /// What Ray Reconstruction reads in place of the world's. The specular
    /// motion lasts: a pixel keeps it through frames its lobe says nothing.
    diffuse_guide: wgpu::TextureView,
    specular_guide: wgpu::TextureView,
    normal_guide: wgpu::TextureView,
    motion: wgpu::TextureView,
    depth: wgpu::TextureView,
    specular_motion: wgpu::TextureView,
}

/// What a world view drew for paths to start from.
pub(crate) struct ViewSurfaces<'a> {
    pub depth: &'a wgpu::TextureView,
    pub guides: [&'a wgpu::TextureView; 3],
    pub motion: &'a wgpu::TextureView,
}

/// What Ray Reconstruction reads of a frame global illumination drew.
#[cfg_attr(
    not(feature = "dlss"),
    allow(dead_code, reason = "read only by the built stage")
)]
pub(crate) struct RayGuides<'a> {
    pub guides: [&'a wgpu::TextureView; 3],
    pub motion: &'a wgpu::TextureView,
    pub depth: &'a wgpu::TextureView,
    pub specular_motion: &'a wgpu::TextureView,
}

struct Pipelines {
    presample: wgpu::ComputePipeline,
    decay: wgpu::ComputePipeline,
    compact_block: wgpu::ComputePipeline,
    compact_blocks: wgpu::ComputePipeline,
    write_active: wgpu::ComputePipeline,
    sample_di: wgpu::ComputePipeline,
    sample_gi: wgpu::ComputePipeline,
    blend: wgpu::ComputePipeline,
    paths: wgpu::ComputePipeline,
    resolve: wgpu::ComputePipeline,
    composite: wgpu::RenderPipeline,
}

pub(crate) struct RayGi {
    cache: wgpu::Buffer,
    tiles: wgpu::Buffer,
    sources: wgpu::Buffer,
    /// The dispatch of the cache's active cells, which the compaction
    /// writes: bound alone, as a dispatch may not read what it binds.
    dispatch: wgpu::Buffer,
    params: wgpu::Buffer,
    view_layout: wgpu::BindGroupLayout,
    scene_layout: wgpu::BindGroupLayout,
    dispatch_layout: wgpu::BindGroupLayout,
    composite_layout: wgpu::BindGroupLayout,
    resolve_layout: wgpu::BindGroupLayout,
    pipelines: Option<Pipelines>,
    targets: Option<Targets>,
    /// This view's groups, as `prepare_ray_gi` made them.
    view_group: Option<wgpu::BindGroup>,
    composite_group: Option<wgpu::BindGroup>,
    resolve_group: Option<wgpu::BindGroup>,
    /// Whether the last view drew: it holds what Ray Reconstruction reads.
    drew: bool,
    scene_group: wgpu::BindGroup,
    dispatch_group: wgpu::BindGroup,
    region: PixelRect,
    frames: u32,
    /// The next frame starts the cache's history again.
    reset: bool,
    /// Sources the tiles sample, and the cells a readback last counted.
    sources_count: u32,
    active_cells: u32,
    count_readback: wgpu::Buffer,
    pending_count: Option<std::sync::mpsc::Receiver<Result<(), wgpu::BufferAsyncError>>>,
    timers: [Option<PassTimer>; 3],
}

/// What ray-traced global illumination reports while it draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RayGiReadout {
    /// World cache cells alive at the last count.
    pub world_cache_cells: u32,
    /// The tiles of samples, and the lights they sample.
    pub light_tiles: u32,
    pub light_sources: u32,
}

impl RayGi {
    pub fn new(gpu: &crate::Gpu) -> Self {
        let device = &gpu.device;
        let buffer = |label, size, usage| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage,
                mapped_at_creation: false,
            })
        };
        let storage = wgpu::BufferUsages::STORAGE;
        let cache = buffer(
            "render-wgpu ray gi world cache",
            CACHE_BYTES,
            storage | wgpu::BufferUsages::COPY_SRC,
        );
        let tiles = buffer(
            "render-wgpu ray gi light tiles",
            u64::from(LIGHT_TILES) * TILE_SAMPLES * LIGHT_BYTES,
            storage,
        );
        let sources = sources_buffer(device, 1);
        let dispatch = buffer(
            "render-wgpu ray gi dispatch",
            16,
            storage | wgpu::BufferUsages::INDIRECT,
        );
        let scene_layout = scene_layout(device);
        let dispatch_layout = dispatch_layout(device);
        let scene_group = scene_group(device, &scene_layout, &cache, &tiles, &sources);
        let dispatch_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("render-wgpu ray gi dispatch"),
            layout: &dispatch_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: dispatch.as_entire_binding(),
            }],
        });
        Self {
            cache,
            tiles,
            sources,
            dispatch,
            params: buffer(
                "render-wgpu ray gi params",
                PARAMS_BYTES,
                wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            ),
            view_layout: view_layout(device),
            scene_layout,
            dispatch_layout,
            composite_layout: composite_layout(device),
            resolve_layout: resolve_layout(device),
            pipelines: None,
            targets: None,
            view_group: None,
            composite_group: None,
            resolve_group: None,
            drew: false,
            scene_group,
            dispatch_group,
            region: PixelRect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
            frames: 0,
            reset: true,
            sources_count: 0,
            active_cells: 0,
            count_readback: buffer(
                "render-wgpu ray gi cells readback",
                4,
                wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            ),
            pending_count: None,
            timers: [TILES_PASS, CACHE_PASS, PATHS_PASS].map(|pass| PassTimer::new(gpu, pass)),
        }
    }

    pub fn readout(&self) -> RayGiReadout {
        RayGiReadout {
            world_cache_cells: self.active_cells,
            light_tiles: LIGHT_TILES,
            light_sources: self.sources_count,
        }
    }

    pub fn timings(&self) -> [GpuPassTiming; 3] {
        let timing = |at: usize, pass| {
            self.timers[at]
                .as_ref()
                .map_or_else(|| untimed(pass), PassTimer::readout)
        };
        [
            timing(0, TILES_PASS),
            timing(1, CACHE_PASS),
            timing(2, PATHS_PASS),
        ]
    }

    /// What Ray Reconstruction reads of the frame, when rays drew it.
    pub fn guides(&self) -> Option<RayGuides<'_>> {
        self.targets
            .as_ref()
            .filter(|_| self.drew)
            .map(|targets| RayGuides {
                guides: [
                    &targets.diffuse_guide,
                    &targets.specular_guide,
                    &targets.normal_guide,
                ],
                motion: &targets.motion,
                depth: &targets.depth,
                specular_motion: &targets.specular_motion,
            })
    }

    /// A view that does not draw leaves no guides for the frame.
    pub fn skipped(&mut self) {
        self.drew = false;
    }

    /// Trace the frame's light, in the view `prepare_ray_gi` readied, with
    /// the frame's group 0 and the rays' group 4.
    pub fn encode(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        frame: &wgpu::BindGroup,
        rays: &wgpu::BindGroup,
    ) {
        let (Some(pipelines), Some(view)) = (&self.pipelines, &self.view_group) else {
            return;
        };
        let groups = |pass: &mut wgpu::ComputePass<'_>| {
            pass.set_bind_group(0, frame, &[]);
            pass.set_bind_group(1, view, &[]);
            pass.set_bind_group(2, &self.scene_group, &[]);
            pass.set_bind_group(4, rays, &[]);
        };
        let cells = (CACHE_CELLS / u64::from(CELL_WORKGROUP)) as u32;
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("render-wgpu ray gi light tiles"),
                timestamp_writes: self.timers[0].as_ref().and_then(PassTimer::compute_writes),
            });
            groups(&mut pass);
            pass.set_bind_group(3, &self.dispatch_group, &[]);
            pass.set_pipeline(&pipelines.presample);
            pass.dispatch_workgroups(LIGHT_TILES, 1, 1);
            pass.set_pipeline(&pipelines.decay);
            pass.dispatch_workgroups(cells, 1, 1);
            pass.set_pipeline(&pipelines.compact_block);
            pass.dispatch_workgroups(cells, 1, 1);
            pass.set_pipeline(&pipelines.compact_blocks);
            pass.dispatch_workgroups(1, 1, 1);
            pass.set_pipeline(&pipelines.write_active);
            pass.dispatch_workgroups(cells, 1, 1);
        }
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("render-wgpu ray gi world cache"),
                timestamp_writes: self.timers[1].as_ref().and_then(PassTimer::compute_writes),
            });
            groups(&mut pass);
            for pipeline in [&pipelines.sample_di, &pipelines.sample_gi, &pipelines.blend] {
                pass.set_pipeline(pipeline);
                pass.dispatch_workgroups_indirect(&self.dispatch, 0);
            }
        }
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("render-wgpu ray gi paths"),
                timestamp_writes: self.timers[2].as_ref().and_then(PassTimer::compute_writes),
            });
            groups(&mut pass);
            pass.set_pipeline(&pipelines.paths);
            pass.dispatch_workgroups(
                self.region.width.div_ceil(PIXEL_WORKGROUP),
                self.region.height.div_ceil(PIXEL_WORKGROUP),
                1,
            );
        }
        for timer in self.timers.iter_mut().flatten() {
            timer.resolve(encoder);
        }
        if self.pending_count.is_none() {
            encoder.copy_buffer_to_buffer(
                &self.cache,
                CACHE_COUNT_OFFSET,
                &self.count_readback,
                0,
                4,
            );
        }
    }

    /// Add the frame's light to `hdr`, single-sample, within the view.
    pub fn composite(&self, encoder: &mut wgpu::CommandEncoder, hdr: &wgpu::TextureView) {
        let (Some(pipelines), Some(group)) = (&self.pipelines, &self.composite_group) else {
            return;
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("render-wgpu ray gi composite"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: hdr,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        let region = self.region;
        pass.set_scissor_rect(region.x, region.y, region.width, region.height);
        pass.set_pipeline(&pipelines.composite);
        pass.set_bind_group(0, group, &[]);
        pass.draw(0..3, 0..1);
    }

    /// Merge what Ray Reconstruction reads, once the world's blended parts
    /// have drawn.
    pub fn resolve(&self, encoder: &mut wgpu::CommandEncoder) {
        let (Some(pipelines), Some(group)) = (&self.pipelines, &self.resolve_group) else {
            return;
        };
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("render-wgpu ray gi guides"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipelines.resolve);
        pass.set_bind_group(3, group, &[]);
        pass.dispatch_workgroups(
            self.region.width.div_ceil(PIXEL_WORKGROUP),
            self.region.height.div_ceil(PIXEL_WORKGROUP),
            1,
        );
    }

    /// After the view's encoder is submitted.
    pub fn submitted(&mut self) {
        for timer in self.timers.iter_mut().flatten() {
            timer.submitted();
        }
        if self.pending_count.is_none() {
            let (sender, receiver) = std::sync::mpsc::channel();
            self.count_readback
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |result| {
                    let _ = sender.send(result);
                });
            self.pending_count = Some(receiver);
        }
    }

    /// The cells the last readback counted, and the timers' results.
    fn collect(&mut self, gpu: &crate::Gpu) {
        for timer in self.timers.iter_mut().flatten() {
            timer.collect(gpu);
        }
        let Some(receiver) = &self.pending_count else {
            return;
        };
        match receiver.try_recv() {
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            Err(std::sync::mpsc::TryRecvError::Disconnected) | Ok(Err(_)) => {
                self.pending_count = None;
            }
            Ok(Ok(())) => {
                self.pending_count = None;
                if let Ok(mapped) = self.count_readback.slice(..).get_mapped_range() {
                    self.active_cells = bytemuck::cast_slice::<u8, u32>(&mapped)[0];
                }
                self.count_readback.unmap();
            }
        }
    }

    /// What the tiles sample now: the world's other light rows, then the
    /// emissive parts.
    fn write_sources(&mut self, gpu: &crate::Gpu, rows: &[u32], emissive: &[EmissivePart]) {
        let mut words: Vec<u32> = Vec::with_capacity((rows.len() + emissive.len()) * SOURCE_WORDS);
        for row in rows {
            words.extend([*row, 0, 0, 0]);
            words.extend([0; SOURCE_WORDS - 4]);
        }
        for part in emissive {
            words.extend([part.part, part.triangles, 0, 0]);
            words.extend(part.rows.iter().flatten().map(|value| value.to_bits()));
        }
        self.sources_count = (rows.len() + emissive.len()) as u32;
        let needed = words.len() as u64 * 4;
        if needed > self.sources.size() {
            self.sources = sources_buffer(&gpu.device, (needed / SOURCE_BYTES).next_power_of_two());
            self.scene_group = scene_group(
                &gpu.device,
                &self.scene_layout,
                &self.cache,
                &self.tiles,
                &self.sources,
            );
        }
        if !words.is_empty() {
            gpu.queue
                .write_buffer(&self.sources, 0, bytemuck::cast_slice(&words));
        }
    }

    /// The targets of a view of `size`.
    fn targets(&mut self, device: &wgpu::Device, size: (u32, u32)) {
        if self.targets.as_ref().is_none_or(|made| made.size != size) {
            let target = |label, format| {
                device
                    .create_texture(&wgpu::TextureDescriptor {
                        label: Some(label),
                        size: wgpu::Extent3d {
                            width: size.0,
                            height: size.1,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format,
                        usage: wgpu::TextureUsages::STORAGE_BINDING
                            | wgpu::TextureUsages::TEXTURE_BINDING,
                        view_formats: &[],
                    })
                    .create_view(&Default::default())
            };
            self.targets = Some(Targets {
                size,
                irradiance: target("render-wgpu ray gi irradiance", IRRADIANCE_FORMAT),
                specular: target("render-wgpu ray gi specular", IRRADIANCE_FORMAT),
                record: target("render-wgpu ray gi guide record", GUIDE_RECORD_FORMAT),
                diffuse_guide: target("render-wgpu ray gi diffuse guide", ALBEDO_GUIDE_FORMAT),
                specular_guide: target("render-wgpu ray gi specular guide", ALBEDO_GUIDE_FORMAT),
                normal_guide: target("render-wgpu ray gi normal guide", NORMAL_GUIDE_FORMAT),
                motion: target("render-wgpu ray gi motion", MOTION_FORMAT),
                depth: target("render-wgpu ray gi depth", DEPTH_FORMAT),
                specular_motion: target("render-wgpu ray gi specular motion", MOTION_FORMAT),
            });
        }
    }
}

fn sources_buffer(device: &wgpu::Device, sources: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("render-wgpu ray gi sources"),
        size: sources.max(1) * SOURCE_BYTES,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn scene_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    cache: &wgpu::Buffer,
    tiles: &wgpu::Buffer,
    sources: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("render-wgpu ray gi scene"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: cache.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: tiles.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: sources.as_entire_binding(),
            },
        ],
    })
}

fn storage_entry(binding: u32, read_only: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn texture_entry(binding: u32, stage: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: stage,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: false },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn written_entry(binding: u32, format: wgpu::TextureFormat) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::StorageTexture {
            access: wgpu::StorageTextureAccess::WriteOnly,
            format,
            view_dimension: wgpu::TextureViewDimension::D2,
        },
        count: None,
    }
}

/// Group 1 of `ray_gi.wgsl`.
fn view_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    let compute = wgpu::ShaderStages::COMPUTE;
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("render-wgpu ray gi view"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: compute,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Depth,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            texture_entry(1, compute),
            texture_entry(2, compute),
            texture_entry(3, compute),
            written_entry(4, IRRADIANCE_FORMAT),
            written_entry(5, IRRADIANCE_FORMAT),
            written_entry(6, GUIDE_RECORD_FORMAT),
            params_entry(7),
            written_entry(8, ALBEDO_GUIDE_FORMAT),
            written_entry(9, ALBEDO_GUIDE_FORMAT),
            written_entry(10, NORMAL_GUIDE_FORMAT),
            storage_entry(11, true),
        ],
    })
}

fn params_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

/// `resolve_guides`'s group 3 in `ray_gi.wgsl`.
fn resolve_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    let compute = wgpu::ShaderStages::COMPUTE;
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("render-wgpu ray gi guides"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: compute,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Depth,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            texture_entry(2, compute),
            texture_entry(3, compute),
            texture_entry(4, compute),
            written_entry(5, MOTION_FORMAT),
            written_entry(6, DEPTH_FORMAT),
            written_entry(7, MOTION_FORMAT),
            params_entry(8),
        ],
    })
}

/// Group 2 of `ray_gi.wgsl`.
fn scene_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("render-wgpu ray gi scene"),
        entries: &[
            storage_entry(0, false),
            storage_entry(1, false),
            storage_entry(2, true),
        ],
    })
}

/// Group 3 of `ray_gi.wgsl`, which the compaction alone binds.
fn dispatch_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("render-wgpu ray gi dispatch"),
        entries: &[storage_entry(0, false)],
    })
}

/// Group 0 of `ray_gi_composite.wgsl`.
fn composite_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    let fragment = wgpu::ShaderStages::FRAGMENT;
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("render-wgpu ray gi composite"),
        entries: &[
            texture_entry(0, fragment),
            texture_entry(1, fragment),
            texture_entry(2, fragment),
            texture_entry(3, fragment),
        ],
    })
}

/// `GiParams` in `ray_gi.wgsl`.
fn params_words(
    view_proj: &Mat4,
    eye: Vec3,
    region: PixelRect,
    state: [u32; 4],
    lights: &LightRows,
) -> [u32; (PARAMS_BYTES / 4) as usize] {
    let mut words = [0u32; (PARAMS_BYTES / 4) as usize];
    words[..16].copy_from_slice(&view_proj.inverse().to_cols_array().map(f32::to_bits));
    words[16..32].copy_from_slice(&view_proj.to_cols_array().map(f32::to_bits));
    words[32..35].copy_from_slice(&eye.to_array().map(f32::to_bits));
    words[36..40].copy_from_slice(&[region.x, region.y, region.width, region.height]);
    words[40..44].copy_from_slice(&state);
    words[44..47].copy_from_slice(&lights.ambient.map(f32::to_bits));
    words[48..51].copy_from_slice(&lights.sky.map(f32::to_bits));
    words[52..55].copy_from_slice(&lights.ground.map(f32::to_bits));
    words
}

impl Renderer {
    /// Why global illumination does not draw: the rays' refusal, then Ray
    /// Reconstruction's, which denoises it, then an antialiasing that does
    /// not run it.
    pub(crate) fn ray_gi_refusal(&self) -> Option<SettingRefusal> {
        if let Some((refusal, _)) = crate::ray_scene::refusal(&self.gpu) {
            return Some(refusal);
        }
        if self.ray_gi_without_denoiser {
            return None;
        }
        if !self.options.dlss {
            return Some(SettingRefusal::NeedsDlss);
        }
        self.upscale
            .ray_reconstruction_refusal()
            .map(|refused| refused.refusal)
            .or_else(|| self.gpu.guides_refusal().map(|_| SettingRefusal::NeedsDlss))
    }

    /// Whether the setting is on and nothing refuses it.
    pub(crate) fn ray_gi_wanted(&self) -> bool {
        self.options.global_illumination == render_model::GlobalIllumination::RayTraced
            && self.ray_gi_refusal().is_none()
    }

    /// Whether this frame's world takes rays' light: where Ray Reconstruction
    /// denoises it, on a single-sample target.
    pub(crate) fn ray_gi_draws(&self, samples: u32) -> bool {
        self.ray_gi.is_some()
            && samples == 1
            && (self.reconstructing || self.ray_gi_without_denoiser)
    }

    /// Ready a world view's light: `region` of a target of `size`, read from
    /// its `surfaces`. Whether it draws.
    pub(crate) fn prepare_ray_gi(
        &mut self,
        size: (u32, u32),
        view_proj: &Mat4,
        eye: Vec3,
        region: PixelRect,
        surfaces: ViewSurfaces<'_>,
    ) -> bool {
        let Some(mut gi) = self.ray_gi.take() else {
            return false;
        };
        let ViewSurfaces {
            depth,
            guides,
            motion,
        } = surfaces;
        let traceable = self.refresh_ray_trace_group();
        if traceable {
            gi.collect(&self.gpu);
            let mut rows = Vec::new();
            self.world_light_rows(&mut rows);
            let lights = light_rows(&rows, crate::frame::LIGHT_ROW_FLOATS);
            let emissive = self
                .ray_shadows
                .as_ref()
                .map_or(&[][..], crate::ray_shadows::RayShadows::emissive);
            let emissive_count = emissive.len() as u32;
            gi.write_sources(&self.gpu, &lights.rows, emissive);
            gi.frames = gi.frames.wrapping_add(1);
            let state = [
                gi.frames.wrapping_mul(FRAME_SEED),
                gi.sources_count,
                u32::from(std::mem::take(&mut gi.reset)),
                emissive_count,
            ];
            let params = params_words(view_proj, eye, region, state, &lights);
            self.gpu
                .queue
                .write_buffer(&gi.params, 0, bytemuck::cast_slice(&params));
            gi.region = region;
            let device = &self.gpu.device;
            gi.targets(device, size);
            let targets = gi.targets.as_ref().expect("made above");
            let view_entry = |binding, view| wgpu::BindGroupEntry {
                binding,
                resource: wgpu::BindingResource::TextureView(view),
            };
            let view_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("render-wgpu ray gi view"),
                layout: &gi.view_layout,
                entries: &[
                    view_entry(0, depth),
                    view_entry(1, guides[0]),
                    view_entry(2, guides[1]),
                    view_entry(3, guides[2]),
                    view_entry(4, &targets.irradiance),
                    view_entry(5, &targets.specular),
                    view_entry(6, &targets.record),
                    wgpu::BindGroupEntry {
                        binding: 7,
                        resource: gi.params.as_entire_binding(),
                    },
                    view_entry(8, &targets.diffuse_guide),
                    view_entry(9, &targets.specular_guide),
                    view_entry(10, &targets.normal_guide),
                    wgpu::BindGroupEntry {
                        binding: 11,
                        resource: self.temporal.previous_models().as_entire_binding(),
                    },
                ],
            });
            let resolve_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("render-wgpu ray gi guides"),
                layout: &gi.resolve_layout,
                entries: &[
                    view_entry(1, depth),
                    view_entry(2, motion),
                    view_entry(3, &targets.record),
                    view_entry(4, guides[2]),
                    view_entry(5, &targets.motion),
                    view_entry(6, &targets.depth),
                    view_entry(7, &targets.specular_motion),
                    wgpu::BindGroupEntry {
                        binding: 8,
                        resource: gi.params.as_entire_binding(),
                    },
                ],
            });
            let composite_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("render-wgpu ray gi composite"),
                layout: &gi.composite_layout,
                entries: &[
                    view_entry(0, guides[0]),
                    view_entry(1, guides[1]),
                    view_entry(2, &targets.irradiance),
                    view_entry(3, &targets.specular),
                ],
            });
            gi.view_group = Some(view_group);
            gi.composite_group = Some(composite_group);
            gi.resolve_group = Some(resolve_group);
            self.make_ray_gi_pipelines(&mut gi);
        }
        gi.drew = traceable;
        self.ray_gi = Some(gi);
        traceable
    }

    fn make_ray_gi_pipelines(&mut self, gi: &mut RayGi) {
        if gi.pipelines.is_some() {
            return;
        }
        let device = &self.gpu.device;
        let module = self
            .layouts
            .shaders
            .module(device, Entry::RayGi, Default::default())
            .expect("ray-traced global illumination composes");
        let layout = |dispatch: bool| {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("render-wgpu ray gi"),
                bind_group_layouts: &[
                    Some(&self.layouts.frame),
                    Some(&gi.view_layout),
                    Some(&gi.scene_layout),
                    dispatch.then_some(&gi.dispatch_layout),
                    self.layouts.ray.as_ref(),
                ],
                immediate_size: 0,
            })
        };
        let plain = layout(false);
        let compacting = layout(true);
        let resolving = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("render-wgpu ray gi guides"),
            bind_group_layouts: &[None, None, None, Some(&gi.resolve_layout)],
            immediate_size: 0,
        });
        let pipeline = |layout: &wgpu::PipelineLayout, entry: &str| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("render-wgpu ray gi"),
                layout: Some(layout),
                module: &module,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let composite_module = self
            .layouts
            .shaders
            .module(device, Entry::RayGiComposite, Default::default())
            .expect("the composite composes");
        let composite_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("render-wgpu ray gi composite"),
            bind_group_layouts: &[Some(&gi.composite_layout)],
            immediate_size: 0,
        });
        let composite = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("render-wgpu ray gi composite"),
            layout: Some(&composite_layout),
            vertex: wgpu::VertexState {
                module: &composite_module,
                entry_point: Some("vs_composite"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &composite_module,
                entry_point: Some("fs_composite"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: crate::finish::HDR_FORMAT,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::Zero,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        gi.pipelines = Some(Pipelines {
            presample: pipeline(&plain, "presample_light_tiles"),
            decay: pipeline(&plain, "decay_world_cache"),
            compact_block: pipeline(&plain, "compact_world_cache_single_block"),
            compact_blocks: pipeline(&plain, "compact_world_cache_blocks"),
            write_active: pipeline(&compacting, "compact_world_cache_write_active_cells"),
            sample_di: pipeline(&plain, "sample_di"),
            sample_gi: pipeline(&plain, "sample_gi"),
            blend: pipeline(&plain, "blend_new_samples"),
            paths: pipeline(&plain, "trace_pixels"),
            resolve: pipeline(&resolving, "resolve_guides"),
            composite,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ambient_and_hemisphere_rows_are_what_a_miss_meets_and_the_others_are_sampled() {
        let mut rows = vec![0.0; 16 * 3];
        rows[..4].copy_from_slice(&[std::f32::consts::PI, 0.0, 0.0, 0.0]);
        rows[16..20].copy_from_slice(&[std::f32::consts::PI * 2.0, 0.0, 0.0, 1.0]);
        rows[28] = std::f32::consts::PI * 4.0;
        rows[32..36].copy_from_slice(&[1.0, 1.0, 1.0, 2.0]);
        let lights = light_rows(&rows, 16);
        assert_eq!(lights.rows, vec![2]);
        assert!((lights.ambient[0] - 1.0).abs() < 1e-5);
        assert!((lights.sky[0] - 2.0).abs() < 1e-5);
        assert!((lights.ground[0] - 4.0).abs() < 1e-5);
    }
}
