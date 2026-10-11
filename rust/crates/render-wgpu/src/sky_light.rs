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
//!
//! While the cloud layer draws or a backdrop shows anything, the light is
//! built instead from a capture of what the first primary view's
//! background and backdrop draw around its eye (`frame.rs` `capture_sky`),
//! wherever the view looks: six 128² faces, two a frame
//! once a first capture exists, never while a build is under way, then
//! their mips. A capture begins again once the last one is built if the eye,
//! the scene or (for drifting clouds and backdrop particles) the time moved.

use std::ops::Range;

use glam::Vec3;

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
/// The capture's faces, as `sky_light.wgsl` looks them up: +X, -X, +Y, -Y,
/// +Z, -Z, each a 90° square view along its axis with its up.
pub(crate) const CAPTURE_FACES: [(Vec3, Vec3); 6] = [
    (Vec3::X, Vec3::Y),
    (Vec3::NEG_X, Vec3::Y),
    (Vec3::Y, Vec3::NEG_Z),
    (Vec3::NEG_Y, Vec3::Z),
    (Vec3::Z, Vec3::Y),
    (Vec3::NEG_Z, Vec3::Y),
];
/// Each face's view, for the cloud march's history (`cloud_march.rs`).
pub(crate) const CAPTURE_VIEWS: [&str; 6] = [
    "sky-light +x",
    "sky-light -x",
    "sky-light +y",
    "sky-light -y",
    "sky-light +z",
    "sky-light -z",
];
/// A capture face's side in texels and its format.
pub(crate) const CAPTURE_SIZE: u32 = SKY_CUBE_SIZE;
pub(crate) const CAPTURE_FORMAT: wgpu::TextureFormat = FORMAT;
/// Faces a capture renders a frame once a first one exists.
const FACES_PER_FRAME: u32 = 2;

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
    /// The capture of what the background, the clouds and the backdrop
    /// draw around the camera, by its count of finished captures.
    Captured { generation: u64 },
}

/// What the scene was when a capture began: a capture begins again only
/// once this moves.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CaptureState {
    /// The world view's eye.
    pub eye: [f32; 3],
    /// The renderer's count of applied changes (`scene_generation`).
    pub scene: u64,
    /// The presentation time, while what the capture sees moves with it.
    pub time: Option<f64>,
}

/// The six faces (`CAPTURE_FACES`) as the layers of one array with mips,
/// rendered a face at a time, which a build reads in place of the
/// panorama.
struct Capture {
    /// Each face's base level, a colour attachment.
    faces: Vec<wgpu::TextureView>,
    /// Each mip level of every face.
    levels: Vec<wgpu::TextureView>,
    /// Every level of every face, sampled.
    whole: wgpu::TextureView,
    depth: wgpu::TextureView,
    /// The next face to render.
    next: u32,
    /// Captures finished.
    generation: u64,
    /// The scene when the capture under way, or the last, began.
    began: Option<CaptureState>,
}

impl Capture {
    fn new(gpu: &Gpu) -> Self {
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("render-wgpu sky capture"),
            size: wgpu::Extent3d {
                width: CAPTURE_SIZE,
                height: CAPTURE_SIZE,
                depth_or_array_layers: 6,
            },
            mip_level_count: SKY_LEVELS,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: CAPTURE_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        });
        let faces = (0..6)
            .map(|face| {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some("render-wgpu sky capture face"),
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    base_mip_level: 0,
                    mip_level_count: Some(1),
                    base_array_layer: face,
                    array_layer_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();
        let levels = (0..SKY_LEVELS)
            .map(|level| {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some("render-wgpu sky capture level"),
                    dimension: Some(wgpu::TextureViewDimension::D2Array),
                    base_mip_level: level,
                    mip_level_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();
        let whole = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("render-wgpu sky capture"),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        Self {
            faces,
            levels,
            whole,
            depth: crate::target::depth_texture(gpu, CAPTURE_SIZE, CAPTURE_SIZE),
            next: 0,
            generation: 0,
            began: None,
        }
    }
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
    downsample: wgpu::ComputePipeline,
    /// A 1×1 texture standing in for the panoramas of a colour.
    blank: wgpu::TextureView,
    /// A 1×1 array standing in for the capture of a background.
    blank_capture: wgpu::TextureView,
    /// The capture's faces clamp at their edges.
    capture_sampler: wgpu::Sampler,
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
    /// Made when first wanted, kept after.
    capture: Option<Capture>,
    /// The scene wanted a capture when last asked (`capture_faces`).
    capturing: bool,
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
            capture: None,
            capturing: false,
        }
    }

    /// The finished capture a build reads, while the scene wants one.
    pub fn captured(&self) -> Option<SkySource> {
        let capture = self.capture.as_ref().filter(|_| self.capturing)?;
        (capture.generation > 0).then_some(SkySource::Captured {
            generation: capture.generation,
        })
    }

    /// The faces to capture this frame while the scene wants a capture
    /// (`wanted`; none otherwise, and the light goes back to the
    /// background): all six for the first, then two a frame of one begun
    /// when `state` differs from the last one's, never while a build is
    /// under way. Each renders into `capture_target`; `captured_faces`
    /// follows.
    pub fn capture_faces(&mut self, gpu: &Gpu, wanted: bool, state: CaptureState) -> Range<u32> {
        self.capturing = wanted;
        if !wanted || self.building.is_some() {
            return 0..0;
        }
        let capture = self.capture.get_or_insert_with(|| Capture::new(gpu));
        if capture.next == 0 {
            if capture.generation > 0 && capture.began == Some(state) {
                return 0..0;
            }
            capture.began = Some(state);
        }
        let count = if capture.generation == 0 {
            CAPTURE_FACES.len() as u32
        } else {
            FACES_PER_FRAME
        };
        capture.next..(capture.next + count).min(CAPTURE_FACES.len() as u32)
    }

    /// A capture face's colour target and the capture's depth.
    pub fn capture_target(&self, face: u32) -> (wgpu::TextureView, wgpu::TextureView) {
        let capture = self.capture.as_ref().expect("a capture is under way");
        (capture.faces[face as usize].clone(), capture.depth.clone())
    }

    /// Before a frame's capture faces: the `sky-light` timer's first stamp.
    pub fn begin_capture(&mut self, gpu: &Gpu) {
        let Some(timer) = &self.timer else {
            return;
        };
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render-wgpu sky capture"),
            });
        drop(encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("render-wgpu sky capture begins"),
            timestamp_writes: timer.compute_writes_between(true, false),
        }));
        gpu.queue.submit([encoder.finish()]);
    }

    /// After the faces up to `end` rendered: once all six have, their mips,
    /// and the capture is finished. The timer's last stamp follows.
    pub fn captured_faces(&mut self, gpu: &Gpu, shaders: &mut Shaders, end: u32) {
        let finished = end == CAPTURE_FACES.len() as u32;
        let pipelines = self
            .pipelines
            .get_or_insert_with(|| Pipelines::new(gpu, shaders));
        let capture = self.capture.as_mut().expect("a capture is under way");
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render-wgpu sky capture"),
            });
        if finished {
            for level in 1..SKY_LEVELS as usize {
                let group = pipelines.bind_group(
                    gpu,
                    &capture.levels[level],
                    &self.copies[0].irradiance,
                    &capture.levels[level - 1],
                    None,
                );
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("render-wgpu sky capture mips"),
                    timestamp_writes: None,
                });
                pass.set_bind_group(0, &group, &[0]);
                pass.set_pipeline(&pipelines.downsample);
                let size = (CAPTURE_SIZE >> level).max(1);
                pass.dispatch_workgroups(size.div_ceil(8), size.div_ceil(8), 6);
            }
            capture.next = 0;
            capture.generation += 1;
        } else {
            capture.next = end;
        }
        if let Some(timer) = &mut self.timer {
            drop(encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("render-wgpu sky capture ends"),
                timestamp_writes: timer.compute_writes_between(false, true),
            }));
            timer.resolve(&mut encoder);
        }
        gpu.queue.submit([encoder.finish()]);
        if let Some(timer) = &mut self.timer {
            timer.submitted();
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
            // A capture that went cannot be built from.
            SkySource::Captured { .. } if self.capture.is_none() => {
                self.building = None;
                return false;
            }
            SkySource::Captured { .. } => None,
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
        let capture = match &source {
            SkySource::Captured { .. } => self.capture.as_ref().map(|capture| &capture.whole),
            _ => None,
        };
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
            let group = pipelines.bind_group(
                gpu,
                output,
                &self.copies[back].irradiance,
                capture.unwrap_or(&pipelines.blank_capture),
                Some((first, second)),
            );
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
        let sampler = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
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
                sampler(3),
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
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                sampler(7),
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
        let blank_capture = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("render-wgpu sky capture blank"),
                size: crate::target::extent(1, 1),
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: CAPTURE_FORMAT,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                ..Default::default()
            });
        Self {
            prefilter: pipeline("cs_prefilter"),
            irradiance: pipeline("cs_irradiance"),
            downsample: pipeline("cs_downsample"),
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
            blank_capture,
            capture_sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("render-wgpu sky capture"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::MipmapFilterMode::Linear,
                ..Default::default()
            }),
        }
    }

    /// A step's bindings: its parameters, the panoramas (blank without),
    /// the level it writes, the harmonics, and the capture (or the one
    /// level of it a mip is made from).
    fn bind_group(
        &self,
        gpu: &Gpu,
        output: &wgpu::TextureView,
        irradiance: &wgpu::Buffer,
        capture: &wgpu::TextureView,
        panoramas: Option<(&wgpu::TextureView, &wgpu::TextureView)>,
    ) -> wgpu::BindGroup {
        let (first, second) = panoramas.unwrap_or((&self.blank, &self.blank));
        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("render-wgpu sky light"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &self.params,
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
                    resource: wgpu::BindingResource::Sampler(&self.panorama_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(output),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: irradiance.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(capture),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::Sampler(&self.capture_sampler),
                },
            ],
        })
    }
}

/// Each level's parameters (its roughness and size) and the harmonics',
/// for a build from `source`.
fn write_params(gpu: &Gpu, buffer: &wgpu::Buffer, source: &SkySource) {
    let (mode, amount, color) = match source {
        SkySource::Color(color) => (0.0, 0.0, *color),
        SkySource::Panorama { amount, .. } => (1.0, *amount, [0.0; 3]),
        SkySource::Captured { .. } => (2.0, 0.0, [0.0; 3]),
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
