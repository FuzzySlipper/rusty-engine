//! Volumetric fog (`volumetric_fog.wgsl`): before a world view's finish, two
//! compute dispatches light a froxel grid over the view (the medium and the
//! fog volumes, lit by the pass's lights through their shadows, the cloud
//! layer and the sky) and integrate it front to back; the finish pass reads
//! the scattered light and transmittance at each pixel's distance
//! (`finish_pass.wgsl`). The grid's resolution follows the
//! `volumetric_fog` setting; a device without compute shaders, or a
//! software one, refuses it and draws the analytic fog alone.
//!
//! Temporal filtering: each world view (by camera and viewport) keeps a
//! history grid. Each frame the light pass samples every cell at a jittered
//! point within it (a Halton sequence) and blends it with the cell's centre
//! reprojected into the previous frame's grid, by the quality's weight;
//! the result becomes the next history. A view seen for the first time, a
//! grid or reach that changed, or a camera that jumped resets it: that frame
//! samples cell centres with no history. A reading beside the output's
//! frames (a tool's capture, `FogView::reading`) lights against a matching
//! view's history but keeps nothing.
//!
//! Fog regions (`fog_regions.rs`) add to the medium like fog volumes; the
//! light pass reads their cells from the same row buffer as the volumes,
//! after the volume and region rows, and an output frame moves the cells on
//! before it lights them.

use std::collections::BTreeMap;

use render_model::{
    FogRegionDescriptor, FogStirrerDescriptor, FogVolumeDescriptor, FogVolumeShape,
    VolumetricFogDescriptor, VolumetricFogQuality,
};

use glam::{Mat4, Vec3};

use crate::camera::CameraMatrices;
use crate::fog_regions::{FogRegions, CELL_BYTES};
use crate::timing::{GpuPassTiming, PassTimer};
use crate::Gpu;

const PASS: &str = "volumetric-fog";
const LIGHT_WORKGROUP: [u32; 3] = [4, 4, 4];
const INTEGRATE_WORKGROUP: [u32; 3] = [8, 8, 1];
/// `FogParams`: four vectors, the previous frame's view-projection, its eye
/// and the temporal row.
const PARAMS_BYTES: u64 = 64 + 64 + 16 + 16;
/// The share of a cell's new sample blended into its history each frame, by
/// quality: High remembers longer for smoother shafts.
const TEMPORAL_WEIGHT_LOW: f32 = 0.25;
const TEMPORAL_WEIGHT_HIGH: f32 = 0.1;
/// A camera that moves farther than this share of the grid's reach in one
/// frame has cut: its history is dropped.
const CUT_SHARE: f32 = 0.1;
/// Jitter positions cycle through this many Halton points.
const JITTER_PERIOD: u32 = 16;
/// A view's history unused for this many fogged views is dropped.
const HISTORY_KEPT: u64 = 240;
/// `FogVolume`, and `FogRegion`: six vectors.
const ROW_BYTES: u64 = 96;
/// The volume rows, then the region rows; the regions' cells follow (a
/// multiple of 256 bytes, so the cells bind at that offset).
const REGION_ROWS_BYTE: u64 = ROW_BYTES * FogVolumeDescriptor::MAX_VOLUMES as u64;
const FIRST_CELL_BYTE: u64 =
    ROW_BYTES * (FogVolumeDescriptor::MAX_VOLUMES + FogRegionDescriptor::MAX_REGIONS) as u64;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// Cells across, down and deep at a quality.
pub(crate) fn grid(quality: VolumetricFogQuality) -> Option<[u32; 3]> {
    match quality {
        VolumetricFogQuality::Off => None,
        VolumetricFogQuality::Low => Some([96, 54, 48]),
        VolumetricFogQuality::High => Some([160, 90, 64]),
    }
}

/// The world view a fog grid is lit for: its camera and viewport name its
/// temporal history.
pub(crate) struct FogView<'a> {
    pub camera: Option<&'a str>,
    pub viewport: [u32; 4],
    pub matrices: &'a CameraMatrices,
    /// A reading (`Renderer::reading`): it reads the view's history and
    /// leaves it as it was.
    pub reading: bool,
}

/// The scene's fog: its medium, fog volumes, fog regions and the stirrers
/// moving them.
pub(crate) struct FogScene<'a> {
    pub medium: &'a VolumetricFogDescriptor,
    pub volumes: &'a BTreeMap<u32, FogVolumeDescriptor>,
    pub regions: &'a BTreeMap<u32, FogRegionDescriptor>,
    pub stirrers: &'a BTreeMap<u32, FogStirrerDescriptor>,
}

/// What the finish pass needs to read a view's fog.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FogLookup {
    /// The grid's reach, metres.
    pub distance: f32,
    /// Its depth in cells.
    pub slices: u32,
}

/// The volumetric fog the last world view drew, for diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolumetricFogReadout {
    /// Why this device does not draw it; `None` while it can.
    pub refused: Option<String>,
    /// The grid the last view lit, or `None` when it drew none.
    pub grid: Option<[u32; 3]>,
    /// The fog volumes it took.
    pub volumes: u32,
    /// The fog regions it took, their cells together, and the stirrers
    /// moving them.
    pub regions: u32,
    pub region_cells: u32,
    pub stirrers: u32,
}

/// A grid's views keep its textures.
struct Grid {
    size: [u32; 3],
    scatter: wgpu::Texture,
    scatter_view: wgpu::TextureView,
    integrated_view: wgpu::TextureView,
    integrate_group: wgpu::BindGroup,
}

/// One view's temporal history: last frame's filtered grid and where its
/// camera stood.
struct History {
    camera: Option<String>,
    viewport: [u32; 4],
    size: [u32; 3],
    reach: f32,
    texture: wgpu::Texture,
    light_group: wgpu::BindGroup,
    view_proj: Mat4,
    eye: Vec3,
    /// Frames accumulated since the last reset (0: none yet).
    frames: u32,
    used: u64,
}

/// The `n`th point of the Halton sequence in `base`, in [0, 1): the fog's
/// froxel offsets, and the primary views' jitter (`motion.rs`).
pub(crate) fn halton(mut n: u32, base: u32) -> f32 {
    let (mut result, mut fraction) = (0.0, 1.0);
    while n > 0 {
        fraction /= base as f32;
        result += fraction * (n % base) as f32;
        n /= base;
    }
    result
}

pub(crate) struct VolumetricFog {
    pipelines: Option<(wgpu::ComputePipeline, wgpu::ComputePipeline)>,
    refused: Option<String>,
    light_layout: wgpu::BindGroupLayout,
    integrate_layout: wgpu::BindGroupLayout,
    params: wgpu::Buffer,
    /// The volume and region rows, then the regions' cells: as many as
    /// `cell_capacity`.
    rows: wgpu::Buffer,
    cell_capacity: u64,
    regions: FogRegions,
    grid: Option<Grid>,
    histories: Vec<History>,
    /// Fogged views drawn, for dropping unused histories.
    views_drawn: u64,
    /// What the finish pass binds when a view draws no fog: no light, all
    /// transmitted.
    clear: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    timer: Option<PassTimer>,
    last_grid: Option<[u32; 3]>,
    last_volumes: u32,
    last_regions: (u32, u32, u32),
}

fn rows_buffer(device: &wgpu::Device, cells: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("render-wgpu fog rows"),
        size: FIRST_CELL_BYTE + cells * CELL_BYTES,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    })
}

fn texture_3d(
    device: &wgpu::Device,
    label: &str,
    size: [u32; 3],
    usage: wgpu::TextureUsages,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: size[2],
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D3,
        format: FORMAT,
        usage,
        view_formats: &[],
    })
}

impl VolumetricFog {
    /// `shader` lights and integrates the grid; `region_shader` moves the
    /// fog regions' cells.
    pub fn new(
        gpu: &Gpu,
        shader: wgpu::ShaderModule,
        region_shader: wgpu::ShaderModule,
        frame_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let device = &gpu.device;
        let entry = |binding, ty| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty,
            count: None,
        };
        let uniform = wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        };
        let storage_texture = wgpu::BindingType::StorageTexture {
            access: wgpu::StorageTextureAccess::WriteOnly,
            format: FORMAT,
            view_dimension: wgpu::TextureViewDimension::D3,
        };
        let light_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("render-wgpu volumetric fog light"),
            entries: &[
                entry(0, uniform),
                entry(
                    1,
                    wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                ),
                entry(2, storage_texture),
                entry(
                    5,
                    wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D3,
                        multisampled: false,
                    },
                ),
                entry(
                    6,
                    wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                ),
            ],
        });
        let integrate_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("render-wgpu volumetric fog integrate"),
            entries: &[
                entry(0, uniform),
                entry(
                    3,
                    wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D3,
                        multisampled: false,
                    },
                ),
                entry(4, storage_texture),
            ],
        });
        let refused = if gpu.is_software() {
            Some("a software adapter draws the analytic fog alone".to_owned())
        } else {
            gpu.compute_refusal(LIGHT_WORKGROUP, 0)
                .or_else(|| gpu.compute_refusal(INTEGRATE_WORKGROUP, 0))
        };
        let pipelines = refused.is_none().then(|| {
            let pipeline = |label, layout: &wgpu::BindGroupLayout, entry_point| {
                let pipeline_layout =
                    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some(label),
                        bind_group_layouts: &[Some(frame_layout), Some(layout)],
                        immediate_size: 0,
                    });
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some(label),
                    layout: Some(&pipeline_layout),
                    module: &shader,
                    entry_point: Some(entry_point),
                    compilation_options: Default::default(),
                    cache: None,
                })
            };
            (
                pipeline(
                    "render-wgpu volumetric fog light",
                    &light_layout,
                    "cs_light",
                ),
                pipeline(
                    "render-wgpu volumetric fog integrate",
                    &integrate_layout,
                    "cs_integrate",
                ),
            )
        });
        let clear_texture = texture_3d(
            device,
            "render-wgpu volumetric fog clear",
            [1, 1, 1],
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        );
        // Half floats: 0, 0, 0, 1.
        let clear_texel: [u16; 4] = [0, 0, 0, 0x3C00];
        gpu.queue.write_texture(
            clear_texture.as_image_copy(),
            bytemuck::cast_slice(&clear_texel),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(8),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        let regions = FogRegions::new(gpu, region_shader, refused.is_some());
        let buffer = |label, size, usage| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage,
                mapped_at_creation: false,
            })
        };
        Self {
            pipelines,
            refused,
            light_layout,
            integrate_layout,
            params: buffer(
                "render-wgpu volumetric fog params",
                PARAMS_BYTES,
                wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            ),
            rows: rows_buffer(device, 0),
            cell_capacity: 0,
            regions,
            grid: None,
            histories: Vec::new(),
            views_drawn: 0,
            clear: clear_texture.create_view(&wgpu::TextureViewDescriptor::default()),
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("render-wgpu volumetric fog"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                ..Default::default()
            }),
            timer: PassTimer::new(gpu, PASS),
            last_grid: None,
            last_volumes: 0,
            last_regions: (0, 0, 0),
        }
    }

    pub fn refused(&self) -> Option<&str> {
        self.refused.as_deref()
    }

    fn grid_for(&mut self, gpu: &Gpu, size: [u32; 3]) -> &Grid {
        if self.grid.as_ref().is_none_or(|grid| grid.size != size) {
            // The histories bind the old grid's scatter texture.
            self.histories.clear();
            let device = &gpu.device;
            let usage = wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING;
            let scatter = texture_3d(
                device,
                "render-wgpu volumetric fog scatter",
                size,
                usage | wgpu::TextureUsages::COPY_SRC,
            );
            let integrated =
                texture_3d(device, "render-wgpu volumetric fog integrated", size, usage);
            let scatter_view = scatter.create_view(&wgpu::TextureViewDescriptor::default());
            let integrated_view = integrated.create_view(&wgpu::TextureViewDescriptor::default());
            let integrate_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("render-wgpu volumetric fog integrate"),
                layout: &self.integrate_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.params.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(&scatter_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::TextureView(&integrated_view),
                    },
                ],
            });
            self.grid = Some(Grid {
                size,
                scatter,
                scatter_view,
                integrated_view,
                integrate_group,
            });
        }
        self.grid.as_ref().expect("made above")
    }

    /// The history of the view drawn by `camera` in `viewport`, made (empty)
    /// if new, and the frames it holds to reproject: none after a new grid
    /// or reach, or a camera that jumped. A `reading` leaves it as it was.
    #[allow(clippy::too_many_arguments)]
    fn history_for(
        &mut self,
        gpu: &Gpu,
        camera: Option<&str>,
        viewport: [u32; 4],
        size: [u32; 3],
        reach: f32,
        eye: Vec3,
        reading: bool,
    ) -> (usize, u32) {
        self.views_drawn += 1;
        let drawn = self.views_drawn;
        self.histories
            .retain(|history| drawn - history.used <= HISTORY_KEPT);
        let found = self.histories.iter().position(|history| {
            history.camera.as_deref() == camera && history.viewport == viewport
        });
        let index = match found {
            Some(index) if self.histories[index].size == size => index,
            other => {
                if let Some(index) = other {
                    self.histories.remove(index);
                }
                let device = &gpu.device;
                let texture = texture_3d(
                    device,
                    "render-wgpu volumetric fog history",
                    size,
                    wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                );
                let history_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
                let grid = self.grid.as_ref().expect("the grid is made first");
                let light_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("render-wgpu volumetric fog light"),
                    layout: &self.light_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: self.params.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: self.rows.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::TextureView(&grid.scatter_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 5,
                            resource: wgpu::BindingResource::TextureView(&history_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 6,
                            resource: wgpu::BindingResource::Sampler(&self.sampler),
                        },
                    ],
                });
                self.histories.push(History {
                    camera: camera.map(str::to_owned),
                    viewport,
                    size,
                    reach,
                    texture,
                    light_group,
                    view_proj: Mat4::IDENTITY,
                    eye,
                    frames: 0,
                    used: drawn,
                });
                self.histories.len() - 1
            }
        };
        let history = &mut self.histories[index];
        history.used = drawn;
        let cut = history.reach != reach || history.eye.distance(eye) > reach * CUT_SHARE;
        let frames = if cut { 0 } else { history.frames };
        if !reading {
            history.frames = frames;
            history.reach = reach;
        }
        (index, frames)
    }

    /// Whether a view would draw fog at `quality` with this scene's fog:
    /// the device takes it and there is fog to light.
    pub fn would_draw(&self, quality: VolumetricFogQuality, scene: &FogScene<'_>) -> bool {
        grid(quality).is_some()
            && self.pipelines.is_some()
            && (scene.medium.density > 0.0
                || !scene.volumes.is_empty()
                || !scene.regions.is_empty())
    }

    /// Lays the scene's regions out in the row buffer, growing it (and
    /// starting the histories, which bind it, over) when they need more
    /// cells than it holds; returns their rows.
    fn lay_out_regions(
        &mut self,
        gpu: &Gpu,
        regions: &BTreeMap<u32, FogRegionDescriptor>,
    ) -> Vec<[f32; 24]> {
        if !self.regions.available() {
            return Vec::new();
        }
        let placed = crate::fog_regions::lay_out(regions);
        if placed.is_empty() {
            self.regions.clear();
            return Vec::new();
        }
        let cells: u64 = placed
            .iter()
            .map(|placed| u64::from(placed.cells.iter().product::<u32>()))
            .sum();
        let grown = cells > self.cell_capacity;
        if grown {
            // Room for the scene's limit at once past a first small region,
            // so growing happens at most twice.
            self.cell_capacity = if self.cell_capacity == 0 {
                cells
            } else {
                u64::from(FogRegionDescriptor::MAX_SCENE_CELLS)
            };
            self.rows = rows_buffer(&gpu.device, self.cell_capacity);
            self.histories.clear();
        }
        self.regions.lay_out(
            gpu,
            &self.rows,
            grown,
            FIRST_CELL_BYTE,
            self.cell_capacity,
            placed,
        );
        self.regions
            .placed()
            .iter()
            .map(|placed| {
                let region = &placed.region;
                let [cx, cy, cz] = region.center;
                let [hx, hy, hz] = region.half_extents;
                let [ar, ag, ab] = region.albedo;
                let [er, eg, eb] = region.emission;
                let [vx, vy, vz] = region.current;
                let [nx, ny, nz] = placed.cells.map(|n| n as f32);
                [
                    cx,
                    cy,
                    cz,
                    region.yaw_degrees.to_radians(),
                    hx,
                    hy,
                    hz,
                    placed.first as f32,
                    region.density,
                    region.edge,
                    region.noise_scale,
                    region.noise_strength,
                    ar,
                    ag,
                    ab,
                    nx,
                    er,
                    eg,
                    eb,
                    ny,
                    vx,
                    vy,
                    vz,
                    nz,
                ]
            })
            .collect()
    }

    /// Lights and integrates a world view's fog into the grid before its
    /// finish, with the view's frame bind group, and returns what the finish
    /// pass needs to read it; `None` when the view draws none (off, refused,
    /// or no medium and no volumes).
    #[allow(clippy::too_many_arguments)]
    pub fn encode(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        frame_group: &wgpu::BindGroup,
        view: FogView<'_>,
        quality: VolumetricFogQuality,
        scene: FogScene<'_>,
        time_seconds: f64,
        timed: bool,
    ) -> Option<FogLookup> {
        self.last_grid = None;
        self.last_volumes = 0;
        self.last_regions = (0, 0, 0);
        let size = grid(quality)?;
        self.pipelines.as_ref()?;
        let medium = scene.medium;
        let rows: Vec<[f32; 24]> = scene
            .volumes
            .values()
            .take(FogVolumeDescriptor::MAX_VOLUMES)
            .map(|volume| {
                let [cx, cy, cz] = volume.center;
                let [hx, hy, hz] = volume.half_extents;
                let [ar, ag, ab] = volume.albedo;
                let [er, eg, eb] = volume.emission;
                let [vx, vy, vz] = volume.noise_velocity;
                let shape = match volume.shape {
                    FogVolumeShape::Box => 0.0,
                    FogVolumeShape::Ellipsoid => 1.0,
                };
                [
                    cx,
                    cy,
                    cz,
                    shape,
                    hx,
                    hy,
                    hz,
                    volume.yaw_degrees.to_radians(),
                    volume.density,
                    volume.edge,
                    volume.noise_scale,
                    volume.noise_strength,
                    ar,
                    ag,
                    ab,
                    0.0,
                    er,
                    eg,
                    eb,
                    0.0,
                    vx,
                    vy,
                    vz,
                    0.0,
                ]
            })
            .collect();
        let region_rows = self.lay_out_regions(gpu, scene.regions);
        if medium.density <= 0.0 && rows.is_empty() && region_rows.is_empty() {
            return None;
        }
        if let Some(timer) = &mut self.timer {
            timer.collect(gpu);
        }
        let (light, integrate) = self.pipelines.as_ref().expect("checked above");
        let (light, integrate) = (light.clone(), integrate.clone());
        self.grid_for(gpu, size);
        let (index, frames) = self.history_for(
            gpu,
            view.camera,
            view.viewport,
            size,
            medium.distance,
            view.matrices.eye,
            view.reading,
        );
        // The temporal row: the history's weight (0: none to reproject) and
        // the frame's jitter within each cell (the centre without history).
        let history = &self.histories[index];
        let (weight, jitter) = if frames == 0 {
            (0.0, [0.5; 3])
        } else {
            let point = frames % JITTER_PERIOD + 1;
            let weight = match quality {
                VolumetricFogQuality::High => TEMPORAL_WEIGHT_HIGH,
                _ => TEMPORAL_WEIGHT_LOW,
            };
            (
                weight,
                [halton(point, 2), halton(point, 3), halton(point, 5)],
            )
        };
        let mut params = [0u32; (PARAMS_BYTES / 4) as usize];
        params[..4].copy_from_slice(&[size[0], size[1], size[2], rows.len() as u32]);
        let floats = [
            medium.density,
            medium.base_height,
            medium.falloff_height,
            medium.distance,
            medium.albedo[0],
            medium.albedo[1],
            medium.albedo[2],
            medium.anisotropy,
            medium.ambient,
            // Presentation time wraps hourly, so noise keeps its precision.
            (time_seconds % 3600.0) as f32,
            region_rows.len() as f32,
            0.0,
        ];
        let temporal = history
            .view_proj
            .to_cols_array()
            .into_iter()
            .chain([history.eye.x, history.eye.y, history.eye.z, 0.0])
            .chain([weight, jitter[0], jitter[1], jitter[2]]);
        for (slot, value) in params[4..]
            .iter_mut()
            .zip(floats.into_iter().chain(temporal))
        {
            *slot = value.to_bits();
        }
        gpu.queue
            .write_buffer(&self.params, 0, bytemuck::cast_slice(&params));
        if !rows.is_empty() {
            gpu.queue
                .write_buffer(&self.rows, 0, bytemuck::cast_slice(&rows));
        }
        if !region_rows.is_empty() {
            gpu.queue.write_buffer(
                &self.rows,
                REGION_ROWS_BYTE,
                bytemuck::cast_slice(&region_rows),
            );
            // An output frame moves the cells on before lighting them; a
            // reading draws them as they are.
            if !view.reading {
                self.regions.step(
                    gpu,
                    encoder,
                    &self.rows,
                    scene.stirrers.values(),
                    time_seconds,
                    timed,
                );
            }
        }
        let light_group = self.histories[index].light_group.clone();
        let grid = self.grid.as_ref().expect("made above");
        let integrate_group = grid.integrate_group.clone();
        {
            let writes = self
                .timer
                .as_ref()
                .filter(|_| timed)
                .and_then(|timer| timer.compute_writes_between(true, false));
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("render-wgpu volumetric fog light"),
                timestamp_writes: writes,
            });
            pass.set_bind_group(0, frame_group, &[]);
            pass.set_pipeline(&light);
            pass.set_bind_group(1, &light_group, &[]);
            pass.dispatch_workgroups(
                size[0].div_ceil(LIGHT_WORKGROUP[0]),
                size[1].div_ceil(LIGHT_WORKGROUP[1]),
                size[2].div_ceil(LIGHT_WORKGROUP[2]),
            );
        }
        // This frame's filtered grid is the view's next history, unless it
        // is a reading's.
        if !view.reading {
            encoder.copy_texture_to_texture(
                grid.scatter.as_image_copy(),
                self.histories[index].texture.as_image_copy(),
                wgpu::Extent3d {
                    width: size[0],
                    height: size[1],
                    depth_or_array_layers: size[2],
                },
            );
        }
        {
            let writes = self
                .timer
                .as_ref()
                .filter(|_| timed)
                .and_then(|timer| timer.compute_writes_between(false, true));
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("render-wgpu volumetric fog integrate"),
                timestamp_writes: writes,
            });
            pass.set_bind_group(0, frame_group, &[]);
            pass.set_pipeline(&integrate);
            pass.set_bind_group(1, &integrate_group, &[]);
            pass.dispatch_workgroups(
                size[0].div_ceil(INTEGRATE_WORKGROUP[0]),
                size[1].div_ceil(INTEGRATE_WORKGROUP[1]),
                1,
            );
        }
        if !view.reading {
            let history = &mut self.histories[index];
            history.view_proj = view.matrices.view_proj;
            history.eye = view.matrices.eye;
            history.frames = history.frames.saturating_add(1);
        }
        if timed {
            if let Some(timer) = &mut self.timer {
                timer.resolve(encoder);
            }
        }
        self.last_grid = Some(size);
        self.last_volumes = rows.len() as u32;
        self.last_regions = (
            region_rows.len() as u32,
            self.regions
                .placed()
                .iter()
                .map(|placed| placed.cells.iter().product::<u32>())
                .sum(),
            if region_rows.is_empty() {
                0
            } else {
                scene.stirrers.len().min(FogStirrerDescriptor::MAX_STIRRERS) as u32
            },
        );
        Some(FogLookup {
            distance: medium.distance,
            slices: size[2],
        })
    }

    /// What the finish pass binds: the integrated grid while a view drew
    /// fog, else no fog at all.
    pub fn lookup_view(&self, drawn: bool) -> &wgpu::TextureView {
        match (&self.grid, drawn) {
            (Some(grid), true) => &grid.integrated_view,
            _ => &self.clear,
        }
    }

    /// After the frame that drew fog is submitted.
    pub fn submitted(&mut self) {
        if let Some(timer) = &mut self.timer {
            timer.submitted();
        }
        self.regions.submitted();
    }

    /// The light and integrate passes', and the fog regions' step.
    pub fn timings(&self) -> [GpuPassTiming; 2] {
        [
            self.timer
                .as_ref()
                .map_or_else(|| crate::timing::untimed(PASS), PassTimer::readout),
            self.regions.timing(),
        ]
    }

    pub fn readout(&self) -> VolumetricFogReadout {
        VolumetricFogReadout {
            refused: self.refused.clone(),
            grid: self.last_grid,
            volumes: self.last_volumes,
            regions: self.last_regions.0,
            region_cells: self.last_regions.1,
            stirrers: self.last_regions.2,
        }
    }
}
