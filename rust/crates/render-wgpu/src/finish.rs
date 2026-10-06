//! The world's HDR target and the finish pass (`finish_pass.wgsl`).
//!
//! A view draws its background (clear colour or sky) into its target, then
//! its world, sprites and particles into an `Rgba16Float` target of the same
//! size and sample count, cleared to transparent: opaque draws cover their
//! pixel, blended ones accumulate premultiplied colour and coverage. The
//! finish pass reads that colour and the target's depth (every sample of a
//! multisampled pair, finished one by one and averaged), finishes each
//! covered pixel and composites it over the background with premultiplied
//! blending. With bloom or auto exposure (`post.rs`) the world pass also
//! resolves the HDR colour to one sample for them to read.
//!
//! The first world view of a frame times its world pass, its bloom and
//! exposure passes together, and its finish pass (`timing.rs`).

use render_model::{AutoExposureDescriptor, BloomDescriptor};

use crate::gpu::Gpu;
use crate::pipelines::standard;
use crate::post::{Chain, Post};
use crate::shaders::{Entry, Features, Shaders};
use crate::target::{ColorTarget, TargetView, DEPTH_FORMAT};
use crate::timing::{untimed, GpuPassTiming, PassTimer};

/// The world's linear HDR colour.
pub(crate) const HDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// HDR targets unused for this many frames are dropped.
const TARGET_FRAMES_KEPT: u64 = 120;

/// Bytes of `FinishParams`: viewport, output, post.
const PARAMS_BYTES: u64 = 48;

const WORLD: &str = "world";
const POST: &str = "bloom-exposure";
const FINISH: &str = "finish";

/// One size and sample count of HDR target: the colour the world draws
/// into and the finish pass reads, and what bloom and auto exposure need of
/// it once either is on.
struct HdrTarget {
    width: u32,
    height: u32,
    samples: u32,
    color: wgpu::TextureView,
    /// A multisampled colour's single-sample resolve.
    resolve: Option<wgpu::TextureView>,
    bloom: Option<Chain>,
    luminance: Option<Chain>,
    used: u64,
}

impl HdrTarget {
    /// The single-sample world bloom and auto exposure read.
    fn resolved(&self) -> &wgpu::TextureView {
        self.resolve.as_ref().unwrap_or(&self.color)
    }
}

/// What the world pass draws into: the HDR colour and, when bloom or auto
/// exposure reads it, the single-sample resolve.
pub(crate) struct HdrViews {
    pub color: wgpu::TextureView,
    pub resolve: Option<wgpu::TextureView>,
}

/// What a view's finish adds and scales by.
#[derive(Clone, Copy, Default)]
pub(crate) struct FinishPost {
    /// Bloom's intensity; 0 for none.
    pub bloom: f32,
    /// Scale the exposure by auto exposure's adapted value.
    pub auto_exposure: bool,
}

pub(crate) struct Finish {
    shader: wgpu::ShaderModule,
    frame_layout: wgpu::BindGroupLayout,
    single_layout: wgpu::BindGroupLayout,
    multisampled_layout: wgpu::BindGroupLayout,
    single_pipeline: wgpu::PipelineLayout,
    multisampled_pipeline: wgpu::PipelineLayout,
    params: wgpu::Buffer,
    bloom_sampler: wgpu::Sampler,
    /// Finish pipelines by the target drawn into.
    pipelines: Vec<(ColorTarget, wgpu::RenderPipeline)>,
    /// Transparent viewport clears by HDR target sample count.
    clears: Vec<(u32, wgpu::RenderPipeline)>,
    targets: Vec<HdrTarget>,
    pub post: Post,
    frame: u64,
    world_timer: Option<PassTimer>,
    post_timer: Option<PassTimer>,
    finish_timer: Option<PassTimer>,
    /// The bloom and adaptation the post timer last timed.
    timed_post: (bool, bool),
}

impl Finish {
    pub fn new(gpu: &Gpu, shaders: &mut Shaders) -> Self {
        let device = &gpu.device;
        let shader = standard(shaders.module(device, Entry::Finish, Features::default()));
        let uniform = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let frame_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("render-wgpu finish frame"),
            entries: &[uniform(0)],
        });
        let layout = |multisampled: bool| {
            let texture = |binding, sample_type, multisampled| wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled,
                },
                count: None,
            };
            let float = wgpu::TextureSampleType::Float { filterable: false };
            let depth = wgpu::TextureSampleType::Depth;
            let (color, scene_depth) = if multisampled {
                (texture(4, float, true), texture(3, depth, true))
            } else {
                (texture(1, float, false), texture(2, depth, false))
            };
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("render-wgpu finish"),
                entries: &[
                    uniform(0),
                    color,
                    scene_depth,
                    texture(
                        5,
                        wgpu::TextureSampleType::Float { filterable: true },
                        false,
                    ),
                    wgpu::BindGroupLayoutEntry {
                        binding: 6,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    texture(7, float, false),
                ],
            })
        };
        let (single_layout, multisampled_layout) = (layout(false), layout(true));
        let pipeline_layout = |group: &wgpu::BindGroupLayout| {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("render-wgpu finish"),
                bind_group_layouts: &[Some(&frame_layout), Some(group)],
                immediate_size: 0,
            })
        };
        let single_pipeline = pipeline_layout(&single_layout);
        let multisampled_pipeline = pipeline_layout(&multisampled_layout);
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("render-wgpu finish params"),
            size: PARAMS_BYTES,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            shader,
            frame_layout,
            single_layout,
            multisampled_layout,
            single_pipeline,
            multisampled_pipeline,
            params,
            bloom_sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("render-wgpu bloom"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
            pipelines: Vec::new(),
            clears: Vec::new(),
            targets: Vec::new(),
            post: Post::new(gpu, shaders),
            frame: 0,
            world_timer: PassTimer::new(gpu, WORLD),
            post_timer: PassTimer::new(gpu, POST),
            finish_timer: PassTimer::new(gpu, FINISH),
            timed_post: (false, false),
        }
    }

    /// A new frame: targets unused for a while are dropped and the timers
    /// take any times read back.
    pub fn begin_frame(&mut self, gpu: &Gpu) {
        self.frame += 1;
        self.post.begin_frame();
        for timer in [
            &mut self.world_timer,
            &mut self.post_timer,
            &mut self.finish_timer,
        ]
        .into_iter()
        .flatten()
        {
            timer.collect(gpu);
        }
        let frame = self.frame;
        self.targets
            .retain(|target| frame - target.used <= TARGET_FRAMES_KEPT);
    }

    fn index(&self, width: u32, height: u32, samples: u32) -> Option<usize> {
        self.targets.iter().position(|target| {
            (target.width, target.height, target.samples) == (width, height, samples)
        })
    }

    /// The HDR target for a view target of this size and sample count, made
    /// if new; with `resolved`, its single-sample resolve too.
    pub fn target(
        &mut self,
        gpu: &Gpu,
        width: u32,
        height: u32,
        samples: u32,
        resolved: bool,
    ) -> HdrViews {
        let texture = |samples| {
            gpu.device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("render-wgpu world HDR"),
                    size: crate::target::extent(width, height),
                    mip_level_count: 1,
                    sample_count: samples,
                    dimension: wgpu::TextureDimension::D2,
                    format: HDR_FORMAT,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        let index = match self.index(width, height, samples) {
            Some(index) => index,
            None => {
                self.targets.push(HdrTarget {
                    width,
                    height,
                    samples,
                    color: texture(samples),
                    resolve: None,
                    bloom: None,
                    luminance: None,
                    used: self.frame,
                });
                self.targets.len() - 1
            }
        };
        let target = &mut self.targets[index];
        target.used = self.frame;
        if resolved && samples > 1 && target.resolve.is_none() {
            target.resolve = Some(texture(1));
        }
        HdrViews {
            color: target.color.clone(),
            resolve: if resolved && samples > 1 {
                target.resolve.clone()
            } else {
                None
            },
        }
    }

    /// Bloom and auto exposure from the resolved world of the HDR target of
    /// this size and sample count, as of presentation time `now`; `timed`
    /// for a world view.
    #[allow(clippy::too_many_arguments)]
    pub fn encode_post(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        (width, height, samples): (u32, u32, u32),
        bloom: Option<BloomDescriptor>,
        auto_exposure: Option<AutoExposureDescriptor>,
        now: f64,
        timed: bool,
    ) {
        let Some(index) = self.index(width, height, samples) else {
            return;
        };
        let target = &mut self.targets[index];
        if bloom.is_some() && target.bloom.is_none() {
            target.bloom = Some(Chain::bloom(gpu, width, height));
        }
        if auto_exposure.is_some() && target.luminance.is_none() {
            target.luminance = Some(Chain::luminance(gpu));
        }
        let target = &self.targets[index];
        let source = target.resolved().clone();
        // A median never mixes frames that did different work.
        let work = (bloom.is_some(), auto_exposure.is_some());
        if timed && work != self.timed_post {
            self.timed_post = work;
            if let Some(timer) = &mut self.post_timer {
                timer.restart();
            }
        }
        let timer = self.post_timer.as_ref().filter(|_| timed);
        if let (Some(bloom), Some(chain)) = (bloom, &target.bloom) {
            // The stamps open on the first pass and close on the last.
            let stamps = timer.map(|timer| (timer, true, auto_exposure.is_none()));
            self.post
                .bloom(gpu, encoder, &source, (width, height), chain, bloom, stamps);
        }
        if let (Some(auto), Some(chain)) = (auto_exposure, &target.luminance) {
            let stamps = timer.map(|timer| (timer, bloom.is_none(), true));
            self.post
                .adapt(gpu, encoder, &source, chain, auto, now, stamps);
        }
        if let Some(timer) = self.post_timer.as_mut().filter(|_| timed) {
            timer.resolve(encoder);
        }
    }

    /// The world pass's stamps, for a world view.
    pub fn world_writes(&self, timed: bool) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
        self.world_timer
            .as_ref()
            .filter(|_| timed)
            .and_then(PassTimer::render_writes)
    }

    /// After a timed world pass, in its encoder.
    pub fn resolve_world(&mut self, encoder: &mut wgpu::CommandEncoder) {
        if let Some(timer) = &mut self.world_timer {
            timer.resolve(encoder);
        }
    }

    /// After a view's encoder was submitted: read its timed passes back.
    pub fn submitted(&mut self) {
        for timer in [
            &mut self.world_timer,
            &mut self.post_timer,
            &mut self.finish_timer,
        ]
        .into_iter()
        .flatten()
        {
            timer.submitted();
        }
    }

    /// The world, bloom and exposure, and finish timings, in frame order.
    pub fn timings(&self) -> Vec<GpuPassTiming> {
        [
            (&self.world_timer, WORLD),
            (&self.post_timer, POST),
            (&self.finish_timer, FINISH),
        ]
        .into_iter()
        .map(|(timer, pass)| {
            timer
                .as_ref()
                .map_or_else(|| untimed(pass), PassTimer::readout)
        })
        .collect()
    }

    /// Clear the viewport of a multisampled or single-sample HDR target to
    /// transparent and its depth to the far plane.
    pub fn clear_viewport(
        &mut self,
        device: &wgpu::Device,
        pass: &mut wgpu::RenderPass<'_>,
        samples: u32,
    ) {
        let index = match self.clears.iter().position(|(key, _)| *key == samples) {
            Some(index) => index,
            None => {
                let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("render-wgpu world HDR clear"),
                    layout: Some(
                        &device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                            label: Some("render-wgpu world HDR clear"),
                            bind_group_layouts: &[],
                            immediate_size: 0,
                        }),
                    ),
                    vertex: wgpu::VertexState {
                        module: &self.shader,
                        entry_point: Some("vs_clear"),
                        compilation_options: Default::default(),
                        buffers: &[],
                    },
                    primitive: Default::default(),
                    depth_stencil: Some(wgpu::DepthStencilState {
                        format: DEPTH_FORMAT,
                        depth_write_enabled: Some(true),
                        depth_compare: Some(wgpu::CompareFunction::Always),
                        stencil: Default::default(),
                        bias: Default::default(),
                    }),
                    multisample: wgpu::MultisampleState {
                        count: samples,
                        ..Default::default()
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &self.shader,
                        entry_point: Some("fs_clear"),
                        compilation_options: Default::default(),
                        targets: &[Some(HDR_FORMAT.into())],
                    }),
                    multiview_mask: None,
                    cache: None,
                });
                self.clears.push((samples, pipeline));
                self.clears.len() - 1
            }
        };
        pass.set_pipeline(&self.clears[index].1);
        pass.draw(0..3, 0..1);
    }

    /// Encode the finish pass of a view drawn into `target` within
    /// `viewport` (x, y, width, height), its world in the HDR target of the
    /// same size, finished with `frame` (the view's frame uniform); `timed`
    /// for a world view.
    #[allow(clippy::too_many_arguments)]
    pub fn encode(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        target: &TargetView<'_>,
        viewport: [u32; 4],
        frame: &wgpu::Buffer,
        post: FinishPost,
        timed: bool,
    ) {
        let key = target.key();
        let multisampled = key.samples > 1;
        if !self.pipelines.iter().any(|(format, _)| *format == key) {
            let pipeline = gpu
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("render-wgpu finish"),
                    layout: Some(if multisampled {
                        &self.multisampled_pipeline
                    } else {
                        &self.single_pipeline
                    }),
                    vertex: wgpu::VertexState {
                        module: &self.shader,
                        entry_point: Some("vs_finish"),
                        compilation_options: Default::default(),
                        buffers: &[],
                    },
                    primitive: Default::default(),
                    depth_stencil: None,
                    multisample: key.multisample(),
                    fragment: Some(wgpu::FragmentState {
                        module: &self.shader,
                        entry_point: Some(if multisampled {
                            "fs_finish_multisampled"
                        } else {
                            "fs_finish"
                        }),
                        compilation_options: Default::default(),
                        targets: &[Some(wgpu::ColorTargetState {
                            format: key.format,
                            blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    multiview_mask: None,
                    cache: None,
                });
            self.pipelines.push((key, pipeline));
        }
        let Some(index) = self.index(target.width, target.height, key.samples) else {
            return;
        };
        let hdr = &self.targets[index];
        // A float target keeps what the finish makes; a normalized one
        // clamps each sample to 1.
        let ceiling = match key.format {
            wgpu::TextureFormat::Rgba16Float
            | wgpu::TextureFormat::Rgba32Float
            | wgpu::TextureFormat::Rg11b10Ufloat => f32::MAX,
            _ => 1.0,
        };
        let bloom = hdr.bloom.as_ref().filter(|_| post.bloom > 0.0);
        let [x, y, width, height] = viewport.map(|value| value as f32);
        let params = [
            x,
            y,
            width,
            height,
            ceiling,
            target.width as f32,
            target.height as f32,
            0.0,
            if bloom.is_some() { post.bloom } else { 0.0 },
            if post.auto_exposure { 1.0 } else { 0.0 },
            0.0,
            0.0,
        ];
        gpu.queue
            .write_buffer(&self.params, 0, bytemuck::cast_slice(&params));
        let frame_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("render-wgpu finish frame"),
            layout: &self.frame_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: frame.as_entire_binding(),
            }],
        });
        let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("render-wgpu finish"),
            layout: if multisampled {
                &self.multisampled_layout
            } else {
                &self.single_layout
            },
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: if multisampled { 4 } else { 1 },
                    resource: wgpu::BindingResource::TextureView(&hdr.color),
                },
                wgpu::BindGroupEntry {
                    binding: if multisampled { 3 } else { 2 },
                    resource: wgpu::BindingResource::TextureView(target.depth),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(
                        bloom.map_or(&self.post.black, Chain::top),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::Sampler(&self.bloom_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::TextureView(if post.auto_exposure {
                        self.post.exposure()
                    } else {
                        &self.post.unit
                    }),
                },
            ],
        });
        let pipeline = &self
            .pipelines
            .iter()
            .find(|(format, _)| *format == key)
            .expect("finish pipeline made")
            .1;
        let timer = self.finish_timer.as_ref().filter(|_| timed);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("render-wgpu finish"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target.color,
                depth_slice: None,
                resolve_target: target.resolve,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: timer.and_then(PassTimer::render_writes),
            occlusion_query_set: None,
            multiview_mask: None,
        });
        let [x, y, width, height] = viewport;
        pass.set_viewport(x as f32, y as f32, width as f32, height as f32, 0.0, 1.0);
        pass.set_scissor_rect(x, y, width, height);
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &frame_group, &[]);
        pass.set_bind_group(1, &group, &[]);
        pass.draw(0..3, 0..1);
        drop(pass);
        if let Some(timer) = self.finish_timer.as_mut().filter(|_| timed) {
            timer.resolve(encoder);
        }
    }
}
