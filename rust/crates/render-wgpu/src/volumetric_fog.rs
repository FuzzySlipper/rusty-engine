//! Volumetric fog (`volumetric_fog.wgsl`): before a world view's finish, two
//! compute dispatches light a froxel grid over the view (the medium and the
//! fog volumes, lit by the pass's lights through their shadows, the cloud
//! layer and the sky) and integrate it front to back; the finish pass reads
//! the scattered light and transmittance at each pixel's distance
//! (`finish_pass.wgsl`). The grid's resolution follows the
//! `volumetric_fog` setting; a device without compute shaders, or a
//! software one, refuses it and draws the analytic fog alone.

use render_model::{
    FogVolumeDescriptor, FogVolumeShape, VolumetricFogDescriptor, VolumetricFogQuality,
};

use crate::timing::{GpuPassTiming, PassTimer};
use crate::Gpu;

const PASS: &str = "volumetric-fog";
const LIGHT_WORKGROUP: [u32; 3] = [4, 4, 4];
const INTEGRATE_WORKGROUP: [u32; 3] = [8, 8, 1];
/// `FogParams`: four vectors.
const PARAMS_BYTES: u64 = 64;
/// `FogVolume`: six vectors.
const VOLUME_BYTES: u64 = 96;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// Cells across, down and deep at a quality.
pub(crate) fn grid(quality: VolumetricFogQuality) -> Option<[u32; 3]> {
    match quality {
        VolumetricFogQuality::Off => None,
        VolumetricFogQuality::Low => Some([96, 54, 48]),
        VolumetricFogQuality::High => Some([160, 90, 64]),
    }
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
}

/// A grid's views keep its textures.
struct Grid {
    size: [u32; 3],
    integrated_view: wgpu::TextureView,
    light_group: wgpu::BindGroup,
    integrate_group: wgpu::BindGroup,
}

pub(crate) struct VolumetricFog {
    pipelines: Option<(wgpu::ComputePipeline, wgpu::ComputePipeline)>,
    refused: Option<String>,
    light_layout: wgpu::BindGroupLayout,
    integrate_layout: wgpu::BindGroupLayout,
    params: wgpu::Buffer,
    volumes: wgpu::Buffer,
    grid: Option<Grid>,
    /// What the finish pass binds when a view draws no fog: no light, all
    /// transmitted.
    clear: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    timer: Option<PassTimer>,
    last_grid: Option<[u32; 3]>,
    last_volumes: u32,
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
    pub fn new(
        gpu: &Gpu,
        shader: wgpu::ShaderModule,
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
            volumes: buffer(
                "render-wgpu fog volumes",
                VOLUME_BYTES * FogVolumeDescriptor::MAX_VOLUMES as u64,
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            ),
            grid: None,
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
        }
    }

    pub fn refused(&self) -> Option<&str> {
        self.refused.as_deref()
    }

    fn grid_for(&mut self, gpu: &Gpu, size: [u32; 3]) -> &Grid {
        if self.grid.as_ref().is_none_or(|grid| grid.size != size) {
            let device = &gpu.device;
            let usage = wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING;
            let scatter = texture_3d(device, "render-wgpu volumetric fog scatter", size, usage);
            let integrated =
                texture_3d(device, "render-wgpu volumetric fog integrated", size, usage);
            let scatter_view = scatter.create_view(&wgpu::TextureViewDescriptor::default());
            let integrated_view = integrated.create_view(&wgpu::TextureViewDescriptor::default());
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
                        resource: self.volumes.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&scatter_view),
                    },
                ],
            });
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
                integrated_view,
                light_group,
                integrate_group,
            });
        }
        self.grid.as_ref().expect("made above")
    }

    /// Lights and integrates a world view's fog into the grid before its
    /// finish, with the view's frame bind group, and returns what the finish
    /// pass needs to read it; `None` when the view draws none (off, refused,
    /// or no medium and no volumes).
    #[allow(clippy::too_many_arguments)]
    pub fn encode<'a>(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        frame_group: &wgpu::BindGroup,
        quality: VolumetricFogQuality,
        medium: &VolumetricFogDescriptor,
        volumes: impl Iterator<Item = &'a FogVolumeDescriptor>,
        time_seconds: f64,
        timed: bool,
    ) -> Option<FogLookup> {
        self.last_grid = None;
        self.last_volumes = 0;
        let size = grid(quality)?;
        self.pipelines.as_ref()?;
        let rows: Vec<[f32; 24]> = volumes
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
        if medium.density <= 0.0 && rows.is_empty() {
            return None;
        }
        if let Some(timer) = &mut self.timer {
            timer.collect(gpu);
        }
        let mut params = [0u32; 16];
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
            0.0,
            0.0,
        ];
        for (slot, value) in params[4..].iter_mut().zip(floats) {
            *slot = value.to_bits();
        }
        gpu.queue
            .write_buffer(&self.params, 0, bytemuck::cast_slice(&params));
        if !rows.is_empty() {
            gpu.queue
                .write_buffer(&self.volumes, 0, bytemuck::cast_slice(&rows));
        }
        let (light, integrate) = self.pipelines.as_ref().expect("checked above");
        let (light, integrate) = (light.clone(), integrate.clone());
        let grid = self.grid_for(gpu, size);
        let (light_group, integrate_group) =
            (grid.light_group.clone(), grid.integrate_group.clone());
        {
            let writes = self
                .timer
                .as_ref()
                .filter(|_| timed)
                .and_then(PassTimer::compute_writes);
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("render-wgpu volumetric fog"),
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
            pass.set_pipeline(&integrate);
            pass.set_bind_group(1, &integrate_group, &[]);
            pass.dispatch_workgroups(
                size[0].div_ceil(INTEGRATE_WORKGROUP[0]),
                size[1].div_ceil(INTEGRATE_WORKGROUP[1]),
                1,
            );
        }
        if timed {
            if let Some(timer) = &mut self.timer {
                timer.resolve(encoder);
            }
        }
        self.last_grid = Some(size);
        self.last_volumes = rows.len() as u32;
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
    }

    pub fn timing(&self) -> GpuPassTiming {
        self.timer
            .as_ref()
            .map_or_else(|| crate::timing::untimed(PASS), PassTimer::readout)
    }

    pub fn readout(&self) -> VolumetricFogReadout {
        VolumetricFogReadout {
            refused: self.refused.clone(),
            grid: self.last_grid,
            volumes: self.last_volumes,
        }
    }
}
