//! A product image effect over each primary view's finished picture
//! (`RenderDiff::SetImageEffect`, `image_effect.wgsl`). While one is set,
//! the view draws into a picture of its own (the render scale's, when there
//! is one), and a full-screen pass hands every output pixel to the
//! product's WGSL `fn image_effect(pixel: ImagePixel) -> vec4<f32>`, which
//! may sample the picture anywhere and reads its depth, its own parameters
//! and textures, and the presentation time. Timed as `image-effect`. With
//! none set, views draw as before.

use render_shaders::{Entry, Features};

use crate::gpu::Gpu;
use crate::shaders::Shaders;
use crate::tables::GpuTexture;
use crate::target::{ScaledPrimary, TargetView};
use crate::timing::{untimed, GpuPassTiming, PassTimer};

/// Bytes of `ImageEffectParams` in `image.wgsl`: four parameter rows and
/// the picture row.
const UNIFORM_BYTES: u64 = 5 * 16;
const PASS: &str = "image-effect";

pub(crate) struct ImageEffect {
    /// Single-sample and multisampled picture depth.
    layouts: [wgpu::BindGroupLayout; 2],
    pipeline_layouts: [wgpu::PipelineLayout; 2],
    sampler: wgpu::Sampler,
    uniform: wgpu::Buffer,
    /// The effect's shader module, composed with the product's WGSL, and
    /// the pipelines made from it by output format and depth samples.
    module: Option<wgpu::ShaderModule>,
    /// The product shader the module was composed from.
    composed: Option<u32>,
    /// Modules composed and pipelines made so far, for diagnostics.
    builds: (u64, u64),
    pipelines: Vec<((wgpu::TextureFormat, bool), wgpu::RenderPipeline)>,
    timer: Option<PassTimer>,
}

fn layout_entries(multisampled: bool) -> Vec<wgpu::BindGroupLayoutEntry> {
    let texture = |binding| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    };
    let sampler = |binding| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    };
    vec![
        wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: wgpu::BufferSize::new(UNIFORM_BYTES),
            },
            count: None,
        },
        texture(1),
        sampler(2),
        wgpu::BindGroupLayoutEntry {
            binding: 3 + u32::from(multisampled),
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Depth,
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled,
            },
            count: None,
        },
        texture(5),
        sampler(6),
        texture(7),
        sampler(8),
    ]
}

impl ImageEffect {
    pub fn new(gpu: &Gpu) -> Self {
        let device = &gpu.device;
        let layouts = [false, true].map(|multisampled| {
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("render-wgpu image effect"),
                entries: &layout_entries(multisampled),
            })
        });
        let pipeline_layouts = [0, 1].map(|index| {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("render-wgpu image effect"),
                bind_group_layouts: &[Some(&layouts[index])],
                immediate_size: 0,
            })
        });
        Self {
            layouts,
            pipeline_layouts,
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("render-wgpu image effect picture"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
            uniform: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("render-wgpu image effect"),
                size: UNIFORM_BYTES,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            module: None,
            composed: None,
            builds: (0, 0),
            pipelines: Vec::new(),
            timer: PassTimer::new(gpu, PASS),
        }
    }

    /// Compose the effect from the product shader `product` (its id from
    /// `Shaders::product`), or clear it with `None`. The same shader as
    /// before keeps its module and pipelines, so changing only the effect's
    /// parameters or textures compiles nothing, unless `recompose` (its
    /// source was redefined). A shader that does not compose is an error
    /// naming its file and line; the effect is then off.
    pub fn set(
        &mut self,
        device: &wgpu::Device,
        shaders: &mut Shaders,
        product: Option<u32>,
        recompose: bool,
    ) -> Result<(), String> {
        if !recompose && product.is_some() && product == self.composed && self.module.is_some() {
            return Ok(());
        }
        self.pipelines.clear();
        self.module = None;
        self.composed = None;
        let Some(product) = product else {
            return Ok(());
        };
        let features = Features::default().with_product(product);
        self.module = Some(shaders.module(device, Entry::ImageEffect, features)?);
        self.composed = Some(product);
        self.builds.0 += 1;
        Ok(())
    }

    /// Shader modules composed and pipelines made so far.
    pub fn builds(&self) -> (u64, u64) {
        self.builds
    }

    pub fn active(&self) -> bool {
        self.module.is_some()
    }

    /// Run the effect over `picture` into `output`.
    #[allow(clippy::too_many_arguments, reason = "one pass description")]
    pub fn apply(
        &mut self,
        gpu: &Gpu,
        picture: &ScaledPrimary,
        output: &TargetView<'_>,
        parameters: &[[f32; 4]; 4],
        textures: [&GpuTexture; 2],
        time: f64,
    ) {
        let Some(module) = &self.module else {
            return;
        };
        let multisampled = picture.samples() > 1;
        let key = (output.format, multisampled);
        let index = match self.pipelines.iter().position(|(known, _)| *known == key) {
            Some(index) => index,
            None => {
                let pipeline = gpu
                    .device
                    .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                        label: Some("render-wgpu image effect"),
                        layout: Some(&self.pipeline_layouts[usize::from(multisampled)]),
                        vertex: wgpu::VertexState {
                            module,
                            entry_point: Some("vs_image"),
                            compilation_options: Default::default(),
                            buffers: &[],
                        },
                        primitive: Default::default(),
                        depth_stencil: None,
                        multisample: Default::default(),
                        fragment: Some(wgpu::FragmentState {
                            module,
                            entry_point: Some(if multisampled {
                                "fs_image_multisampled"
                            } else {
                                "fs_image"
                            }),
                            compilation_options: Default::default(),
                            targets: &[Some(wgpu::ColorTargetState {
                                format: output.format,
                                blend: None,
                                write_mask: wgpu::ColorWrites::ALL,
                            })],
                        }),
                        multiview_mask: None,
                        cache: None,
                    });
                self.pipelines.push((key, pipeline));
                self.builds.1 += 1;
                self.pipelines.len() - 1
            }
        };
        let (width, height) = picture.size();
        let mut values = [0.0f32; 20];
        for (row, parameter) in parameters.iter().enumerate() {
            values[row * 4..row * 4 + 4].copy_from_slice(parameter);
        }
        values[16] = width as f32;
        values[17] = height as f32;
        // Wrapped to a day so the time keeps its precision in a long session.
        values[18] = time.rem_euclid(TIME_WRAP_SECONDS) as f32;
        gpu.queue
            .write_buffer(&self.uniform, 0, bytemuck::cast_slice(&values));
        let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("render-wgpu image effect"),
            layout: &self.layouts[usize::from(multisampled)],
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(picture.image()),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3 + u32::from(multisampled),
                    resource: wgpu::BindingResource::TextureView(picture.depth()),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(&textures[0].view),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::Sampler(&textures[0].sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::TextureView(&textures[1].view),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::Sampler(&textures[1].sampler),
                },
            ],
        });
        if let Some(timer) = &mut self.timer {
            timer.collect(gpu);
        }
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render-wgpu image effect"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render-wgpu image effect"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: output.color,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: self.timer.as_ref().and_then(PassTimer::render_writes),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipelines[index].1);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        if let Some(timer) = &mut self.timer {
            timer.resolve(&mut encoder);
        }
        gpu.queue.submit([encoder.finish()]);
        if let Some(timer) = &mut self.timer {
            timer.submitted();
        }
    }

    /// The effect pass's timing, for `gpu.passes`.
    pub fn timing(&self) -> GpuPassTiming {
        self.timer
            .as_ref()
            .map_or_else(|| untimed(PASS), PassTimer::readout)
    }
}

/// The presentation time is handed to the effect modulo a day.
const TIME_WRAP_SECONDS: f64 = 86_400.0;
