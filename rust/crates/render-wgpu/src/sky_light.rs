//! The sky's light (`sky_light.wgsl`): the background (a sky panorama, two
//! blended, or the clear colour) as an environment for the standard shader.
//!
//! While the product's sky light is on, a change of background (its
//! selection, blend amount or colour) builds a 128² cube whose mips hold its
//! radiance prefiltered for rising roughness, and nine irradiance
//! harmonics, into the back of two copies. The first build finishes in its
//! frame; later ones take a few mip levels a frame and then swap in, so a
//! blend that moves every tick costs a share of a build per frame. The
//! compute pipelines are made on first use.

use crate::gpu::Gpu;
use crate::pipelines::standard;
use crate::shaders::{Entry, Features, Shaders};
use crate::timing::{untimed, GpuPassTiming, PassTimer};

/// The cube's base size in texels and its mip levels (down to 1×1).
pub(crate) const SKY_CUBE_SIZE: u32 = 128;
pub(crate) const SKY_LEVELS: u32 = SKY_CUBE_SIZE.trailing_zeros() + 1;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
/// GGX samples per texel of each prefiltered level.
const SAMPLES: f32 = 64.0;
/// Levels a rebuild takes per frame once a first build exists.
const LEVELS_PER_FRAME: u32 = 3;
/// One uniform slot per level and one for the harmonics, 256 bytes apart.
const PARAMS_STRIDE: u64 = 256;
const PARAMS_BYTES: u64 = 48;
const PASS: &str = "sky-light";

/// What the sky's light is built from.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SkySource {
    /// A uniform environment of a linear colour.
    Color([f32; 3]),
    /// A panorama, or two blended by `amount`, by texture id.
    Panorama {
        first: String,
        second: String,
        amount: f32,
    },
}

/// One copy of the light: the cube (sampled whole, written a level at a
/// time) and the harmonics, and what they were built from.
struct Copy {
    cube: wgpu::TextureView,
    levels: Vec<wgpu::TextureView>,
    irradiance: wgpu::Buffer,
    source: Option<SkySource>,
}

impl Copy {
    fn new(device: &wgpu::Device) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("render-wgpu sky light"),
            size: wgpu::Extent3d {
                width: SKY_CUBE_SIZE,
                height: SKY_CUBE_SIZE,
                depth_or_array_layers: 6,
            },
            mip_level_count: SKY_LEVELS,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        });
        let cube = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("render-wgpu sky light"),
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });
        let levels = (0..SKY_LEVELS)
            .map(|level| {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some("render-wgpu sky light level"),
                    dimension: Some(wgpu::TextureViewDimension::D2Array),
                    base_mip_level: level,
                    mip_level_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();
        let irradiance = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("render-wgpu sky irradiance"),
            size: 9 * 16,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        Self {
            cube,
            levels,
            irradiance,
            source: None,
        }
    }
}

struct Pipelines {
    layout: wgpu::BindGroupLayout,
    prefilter: wgpu::ComputePipeline,
    irradiance: wgpu::ComputePipeline,
    params: wgpu::Buffer,
    /// Panoramas wrap around and clamp at the poles.
    panorama_sampler: wgpu::Sampler,
    /// A 1×1 texture standing in for the panoramas of a colour.
    blank: wgpu::TextureView,
}

pub(crate) struct SkyLight {
    pipelines: Option<Pipelines>,
    copies: [Copy; 2],
    /// The copy the shader samples.
    front: usize,
    /// The source the back copy is being built from, and its next level.
    building: Option<(SkySource, u32)>,
    pub sampler: wgpu::Sampler,
    timer: Option<PassTimer>,
}

impl SkyLight {
    pub fn new(gpu: &Gpu) -> Self {
        let device = &gpu.device;
        Self {
            pipelines: None,
            copies: [Copy::new(device), Copy::new(device)],
            front: 0,
            building: None,
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("render-wgpu sky light"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::MipmapFilterMode::Linear,
                ..Default::default()
            }),
            timer: PassTimer::new(gpu, PASS),
        }
    }

    /// The cube and harmonics the shader samples.
    pub fn cube(&self) -> &wgpu::TextureView {
        &self.copies[self.front].cube
    }

    pub fn irradiance(&self) -> &wgpu::Buffer {
        &self.copies[self.front].irradiance
    }

    /// Whether the shader's copy holds a build.
    pub fn built(&self) -> bool {
        self.copies[self.front].source.is_some()
    }

    /// The shader's copy's nine irradiance coefficients, read back now (144
    /// bytes, a short wait on the device); `None` before a build. The probe
    /// bake takes them as the sky a missed ray sees.
    pub fn read_irradiance(&self, gpu: &Gpu) -> Option<[[f32; 4]; 9]> {
        if !self.built() {
            return None;
        }
        let staging = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("render-wgpu sky irradiance readback"),
            size: 9 * 16,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render-wgpu sky irradiance readback"),
            });
        encoder.copy_buffer_to_buffer(self.irradiance(), 0, &staging, 0, 9 * 16);
        gpu.queue.submit([encoder.finish()]);
        let slice = staging.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
        let mapped = slice.get_mapped_range().ok()?;
        let floats: &[f32] = bytemuck::cast_slice(&mapped);
        let mut coefficients = [[0.0; 4]; 9];
        for (row, values) in coefficients.iter_mut().zip(floats.as_chunks::<4>().0) {
            *row = *values;
        }
        drop(mapped);
        staging.unmap();
        Some(coefficients)
    }

    pub fn timing(&self) -> GpuPassTiming {
        self.timer
            .as_ref()
            .map_or_else(|| untimed(PASS), PassTimer::readout)
    }

    /// Move toward the light of `wanted` (none while the sky light is off):
    /// start a build when it differs from the shader's copy, and encode the
    /// levels this frame takes, finding panoramas by id with `panorama`.
    /// Returns whether the shader's copy changed (the frame bind group must
    /// be rebuilt).
    pub fn progress<'a>(
        &mut self,
        gpu: &Gpu,
        shaders: &mut Shaders,
        wanted: Option<SkySource>,
        panorama: impl Fn(&str) -> Option<&'a wgpu::TextureView>,
    ) -> bool {
        if let Some(timer) = &mut self.timer {
            timer.collect(gpu);
        }
        let Some(wanted) = wanted else {
            self.building = None;
            return false;
        };
        if self.building.is_none() && self.copies[self.front].source.as_ref() != Some(&wanted) {
            self.building = Some((wanted, 0));
        }
        let Some((source, next)) = self.building.clone() else {
            return false;
        };
        let textures = match &source {
            SkySource::Color(_) => None,
            SkySource::Panorama { first, second, .. } => {
                match (panorama(first), panorama(second)) {
                    (Some(first), Some(second)) => Some((first, second)),
                    // A texture went: this build cannot finish.
                    _ => {
                        self.building = None;
                        return false;
                    }
                }
            }
        };
        let built = self.built();
        let pipelines = self
            .pipelines
            .get_or_insert_with(|| Pipelines::new(gpu, shaders));
        if next == 0 {
            write_params(gpu, &pipelines.params, &source);
        }
        let (first, second) = textures.unwrap_or((&pipelines.blank, &pipelines.blank));
        // The first build finishes now; later ones take a few levels a
        // frame, the harmonics with the last (step `SKY_LEVELS`).
        let end = if built {
            (next + LEVELS_PER_FRAME).min(SKY_LEVELS)
        } else {
            SKY_LEVELS
        };
        let finishing = end == SKY_LEVELS;
        let steps: Vec<u32> = (next..end).chain(finishing.then_some(SKY_LEVELS)).collect();
        let back = 1 - self.front;
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render-wgpu sky light"),
            });
        for (index, &step) in steps.iter().enumerate() {
            let harmonics = step == SKY_LEVELS;
            let output = &self.copies[back].levels[step.min(SKY_LEVELS - 1) as usize];
            let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("render-wgpu sky light"),
                layout: &pipelines.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &pipelines.params,
                            offset: 0,
                            size: wgpu::BufferSize::new(PARAMS_BYTES),
                        }),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(first),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(second),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::Sampler(&pipelines.panorama_sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::TextureView(output),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: self.copies[back].irradiance.as_entire_binding(),
                    },
                ],
            });
            let timer = self.timer.as_ref().and_then(|timer| {
                timer.compute_writes_between(index == 0, index + 1 == steps.len())
            });
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("render-wgpu sky light"),
                timestamp_writes: timer,
            });
            pass.set_bind_group(0, &group, &[(u64::from(step) * PARAMS_STRIDE) as u32]);
            if harmonics {
                pass.set_pipeline(&pipelines.irradiance);
                pass.dispatch_workgroups(1, 1, 1);
            } else {
                let size = (SKY_CUBE_SIZE >> step).max(1);
                pass.set_pipeline(&pipelines.prefilter);
                pass.dispatch_workgroups(size.div_ceil(8), size.div_ceil(8), 6);
            }
        }
        if let Some(timer) = &mut self.timer {
            timer.resolve(&mut encoder);
        }
        gpu.queue.submit([encoder.finish()]);
        if let Some(timer) = &mut self.timer {
            timer.submitted();
        }
        if finishing {
            self.copies[back].source = Some(source);
            self.front = back;
            self.building = None;
            true
        } else {
            self.building = Some((source, end));
            false
        }
    }
}

impl Pipelines {
    fn new(gpu: &Gpu, shaders: &mut Shaders) -> Self {
        let device = &gpu.device;
        let shader = standard(shaders.module(device, Entry::SkyLight, Features::default()));
        let texture = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("render-wgpu sky light"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: wgpu::BufferSize::new(PARAMS_BYTES),
                    },
                    count: None,
                },
                texture(1),
                texture(2),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: FORMAT,
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("render-wgpu sky light"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = |entry| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("render-wgpu sky light"),
                layout: Some(&pipeline_layout),
                module: &shader,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        use wgpu::util::DeviceExt;
        let blank = device
            .create_texture_with_data(
                &gpu.queue,
                &wgpu::TextureDescriptor {
                    label: Some("render-wgpu sky light blank"),
                    size: crate::target::extent(1, 1),
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                },
                wgpu::util::TextureDataOrder::LayerMajor,
                &[0; 4],
            )
            .create_view(&Default::default());
        Self {
            prefilter: pipeline("cs_prefilter"),
            irradiance: pipeline("cs_irradiance"),
            layout,
            params: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("render-wgpu sky light params"),
                size: u64::from(SKY_LEVELS + 1) * PARAMS_STRIDE,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            panorama_sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("render-wgpu sky light panorama"),
                address_mode_u: wgpu::AddressMode::Repeat,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::MipmapFilterMode::Linear,
                ..Default::default()
            }),
            blank,
        }
    }
}

/// Each level's parameters (its roughness and size) and the harmonics',
/// for a build from `source`.
fn write_params(gpu: &Gpu, buffer: &wgpu::Buffer, source: &SkySource) {
    let (mode, amount, color) = match source {
        SkySource::Color(color) => (0.0, 0.0, *color),
        SkySource::Panorama { amount, .. } => (1.0, *amount, [0.0; 3]),
    };
    let mut bytes = vec![0u8; ((SKY_LEVELS + 1) as u64 * PARAMS_STRIDE) as usize];
    for level in 0..=SKY_LEVELS {
        let roughness = level.min(SKY_LEVELS - 1) as f32 / (SKY_LEVELS - 1) as f32;
        let size = (SKY_CUBE_SIZE >> level.min(SKY_LEVELS - 1)).max(1) as f32;
        let values = [
            mode, amount, roughness, SAMPLES, color[0], color[1], color[2], 0.0, size, 0.0, 0.0,
            0.0,
        ];
        let at = (u64::from(level) * PARAMS_STRIDE) as usize;
        bytes[at..at + 48].copy_from_slice(bytemuck::cast_slice(&values));
    }
    gpu.queue.write_buffer(buffer, 0, &bytes);
}
