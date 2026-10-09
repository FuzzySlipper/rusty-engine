//! The world's HDR target and the finish pass (`finish_pass.wgsl`).
//!
//! A view draws its background (clear colour or sky) into its target's
//! single-sample image, then its world, sprites and particles into an
//! `Rgba16Float` target of the same size and the depth's sample count,
//! cleared to transparent: opaque draws cover their pixel, blended ones
//! accumulate premultiplied colour and coverage. The finish pass reads that
//! colour and the target's depth (every sample of a multisampled pair,
//! finished one by one and averaged), finishes each covered pixel and
//! composites it over the background in the image with premultiplied
//! blending. With bloom or auto exposure (`post.rs`) the world pass also
//! resolves the HDR colour to one sample for them to read.
//!
//! The first world view of a frame times its world pass, its bloom and
//! exposure passes together, and its finish pass (`timing.rs`).

use render_model::{AutoExposureDescriptor, BloomDescriptor};

use crate::frame::ViewLayer;
use crate::gpu::Gpu;
use crate::pipelines::standard;
use crate::post::{Chain, Post, ShaftTargets};
use crate::shaders::{Entry, Features, Shaders};
use crate::target::{ColorTarget, TargetView, DEPTH_FORMAT};
use crate::timing::{untimed, GpuPassTiming, PassTimer};

/// The world's linear HDR colour.
pub(crate) const HDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// HDR targets unused for this many frames are dropped.
const TARGET_FRAMES_KEPT: u64 = 120;

/// Bytes of `FinishParams`: viewport, output, post, fog.
const PARAMS_BYTES: u64 = 64;

const WORLD: &str = "world";
const POST: &str = "bloom-exposure";
const FINISH: &str = "finish";
const PARTICLES: &str = "particles";
const CLOUDS: &str = "clouds";
const BACKDROP: &str = "backdrop";
const PRECIPITATION: &str = "precipitation";

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
    /// Bloom chains and sun shaft targets by viewport size, so each view
    /// treats its own viewport.
    bloom: Vec<Chain>,
    shafts: Vec<ShaftTargets>,
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
    /// Sun shafts' strength (their intensity, faded as the sun leaves the
    /// view or nears the horizon); 0 for none.
    pub shafts: f32,
    /// Scale the exposure by auto exposure's adapted value.
    pub auto_exposure: bool,
    /// The view's volumetric fog, when it drew some.
    pub fog: Option<crate::volumetric_fog::FogLookup>,
    /// Where the analytic fog begins, in metres along each ray: the reach of
    /// the volumetric fog that stands in for it nearer (0: from the eye).
    pub analytic_start: f32,
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
    /// Finish pipelines by the image's format and the world's samples.
    pipelines: Vec<(ColorTarget, wgpu::RenderPipeline)>,
    /// Transparent viewport clears by HDR target sample count.
    clears: Vec<(u32, wgpu::RenderPipeline)>,
    targets: Vec<HdrTarget>,
    pub post: Post,
    frame: u64,
    world_timer: Option<PassTimer>,
    /// The soft-particle pass, timed only in the frames that draw it.
    particles_timer: Option<PassTimer>,
    /// The sky's cloud layer, timed in the frames that draw it.
    clouds_timer: Option<PassTimer>,
    /// The backdrop's pass, timed in the frames that draw it.
    backdrop_timer: Option<PassTimer>,
    /// The precipitation pass, timed in the frames that draw it.
    precipitation_timer: Option<PassTimer>,
    post_timer: Option<PassTimer>,
    finish_timer: Option<PassTimer>,
    /// The bloom and adaptation the post timer last timed.
    timed_post: (bool, bool, bool),
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
                    texture(
                        8,
                        wgpu::TextureSampleType::Float { filterable: true },
                        false,
                    ),
                    // The view's volumetric fog (`volumetric_fog.rs`): light
                    // scattered toward the camera and transmittance by
                    // distance, and its sampler.
                    wgpu::BindGroupLayoutEntry {
                        binding: 9,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D3,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 10,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
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
            particles_timer: PassTimer::new(gpu, PARTICLES),
            clouds_timer: PassTimer::new(gpu, CLOUDS),
            backdrop_timer: PassTimer::new(gpu, BACKDROP),
            precipitation_timer: PassTimer::new(gpu, PRECIPITATION),
            post_timer: PassTimer::new(gpu, POST),
            finish_timer: PassTimer::new(gpu, FINISH),
            timed_post: (false, false, false),
        }
    }

    /// A new frame: targets unused for a while are dropped and the timers
    /// take any times read back.
    pub fn begin_frame(&mut self, gpu: &Gpu) {
        self.frame += 1;
        self.post.begin_frame();
        for timer in [
            &mut self.world_timer,
            &mut self.particles_timer,
            &mut self.clouds_timer,
            &mut self.backdrop_timer,
            &mut self.precipitation_timer,
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
        for target in &mut self.targets {
            target
                .bloom
                .retain(|chain| frame - chain.used <= TARGET_FRAMES_KEPT);
            target
                .shafts
                .retain(|shafts| frame - shafts.used <= TARGET_FRAMES_KEPT);
        }
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
                    bloom: Vec::new(),
                    shafts: Vec::new(),
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

    /// Bloom, auto exposure and sun shafts (the sun's place in the view's
    /// uv and how far the rays reach) from `viewport` (x, y, width, height
    /// in pixels) of the resolved world of the HDR target of this size and
    /// sample count, as of presentation time `now`; `timed` for a world
    /// view.
    #[allow(clippy::too_many_arguments)]
    pub fn encode_post(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        (width, height, samples): (u32, u32, u32),
        viewport: [u32; 4],
        bloom: Option<BloomDescriptor>,
        auto_exposure: Option<AutoExposureDescriptor>,
        shafts: Option<([f32; 2], f32)>,
        now: f64,
        timed: bool,
    ) {
        let Some(index) = self.index(width, height, samples) else {
            return;
        };
        let frame = self.frame;
        let [x, y, view_width, view_height] = viewport;
        let target = &mut self.targets[index];
        let chain = bloom.map(|_| {
            match target
                .bloom
                .iter()
                .position(|chain| chain.key == (view_width, view_height))
            {
                Some(chain) => chain,
                None => {
                    target
                        .bloom
                        .push(Chain::bloom(gpu, view_width, view_height));
                    target.bloom.len() - 1
                }
            }
        });
        if let Some(chain) = chain {
            target.bloom[chain].used = frame;
        }
        let shaft_targets = shafts.map(|_| {
            match target
                .shafts
                .iter()
                .position(|shafts| shafts.key == (view_width, view_height))
            {
                Some(index) => index,
                None => {
                    target
                        .shafts
                        .push(ShaftTargets::new(gpu, view_width, view_height));
                    target.shafts.len() - 1
                }
            }
        });
        if let Some(index) = shaft_targets {
            target.shafts[index].used = frame;
        }
        if auto_exposure.is_some() && target.luminance.is_none() {
            target.luminance = Some(Chain::luminance(gpu));
        }
        let target = &self.targets[index];
        let source = target.resolved().clone();
        let region = [
            x as f32 / width as f32,
            y as f32 / height as f32,
            view_width as f32 / width as f32,
            view_height as f32 / height as f32,
        ];
        // A median never mixes frames that did different work.
        let work = (bloom.is_some(), auto_exposure.is_some(), shafts.is_some());
        if timed && work != self.timed_post {
            self.timed_post = work;
            if let Some(timer) = &mut self.post_timer {
                timer.restart();
            }
        }
        let timer = self.post_timer.as_ref().filter(|_| timed);
        // The stamps open on the first pass and close on the last.
        let last_is_bloom = auto_exposure.is_none() && shafts.is_none();
        if let (Some(bloom), Some(chain)) = (bloom, chain) {
            let stamps = timer.map(|timer| (timer, true, last_is_bloom));
            self.post.bloom(
                gpu,
                encoder,
                (&source, region),
                (width, height),
                &target.bloom[chain],
                bloom,
                stamps,
            );
        }
        if let (Some(auto), Some(chain)) = (auto_exposure, &target.luminance) {
            let stamps = timer.map(|timer| (timer, bloom.is_none(), shafts.is_none()));
            self.post.adapt(
                gpu,
                encoder,
                (&source, region),
                (width, height),
                chain,
                auto,
                now,
                stamps,
            );
        }
        if let (Some((sun, length)), Some(index)) = (shafts, shaft_targets) {
            let first = bloom.is_none() && auto_exposure.is_none();
            let stamps = timer.map(|timer| (timer, first, true));
            self.post.shafts(
                gpu,
                encoder,
                (&source, region),
                (width, height),
                &target.shafts[index],
                sun,
                length,
                stamps,
            );
        }
        if let Some(timer) = self.post_timer.as_mut().filter(|_| timed) {
            timer.resolve(encoder);
        }
    }

    /// The timer of a view layer's pass: the world's or the backdrop's (the
    /// viewmodel's is untimed).
    fn layer_timer(&self, layer: ViewLayer) -> Option<&PassTimer> {
        match layer {
            ViewLayer::World => self.world_timer.as_ref(),
            ViewLayer::Backdrop => self.backdrop_timer.as_ref(),
            ViewLayer::Viewmodel => None,
        }
    }

    /// A world or backdrop pass's stamps: `begin` on its first pass, `end`
    /// on its last (it splits around the water depth copy).
    pub fn layer_writes_between(
        &self,
        layer: ViewLayer,
        begin: bool,
        end: bool,
    ) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
        self.layer_timer(layer)
            .and_then(|timer| timer.render_writes_between(begin, end))
    }

    /// After a timed world or backdrop pass, in its encoder.
    pub fn resolve_layer(&mut self, layer: ViewLayer, encoder: &mut wgpu::CommandEncoder) {
        let timer = match layer {
            ViewLayer::World => &mut self.world_timer,
            ViewLayer::Backdrop => &mut self.backdrop_timer,
            ViewLayer::Viewmodel => return,
        };
        if let Some(timer) = timer {
            timer.resolve(encoder);
        }
    }

    /// The soft-particle pass's stamps, for a world view.
    pub fn particles_writes(&self, timed: bool) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
        self.particles_timer
            .as_ref()
            .filter(|_| timed)
            .and_then(PassTimer::render_writes)
    }

    /// After a timed soft-particle pass, in its encoder.
    pub fn resolve_particles(&mut self, encoder: &mut wgpu::CommandEncoder) {
        if let Some(timer) = &mut self.particles_timer {
            timer.resolve(encoder);
        }
    }

    /// The volumetric clouds' passes' stamps: `begin` on the march, `end` on
    /// the composite.
    pub fn clouds_writes_between(
        &self,
        begin: bool,
        end: bool,
    ) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
        self.clouds_timer
            .as_ref()
            .and_then(|timer| timer.render_writes_between(begin, end))
    }

    /// After a cloud layer pass, in its encoder.
    pub fn resolve_clouds(&mut self, encoder: &mut wgpu::CommandEncoder) {
        if let Some(timer) = &mut self.clouds_timer {
            timer.resolve(encoder);
        }
    }

    /// The precipitation pass's stamps.
    pub fn precipitation_writes(&self) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
        self.precipitation_timer
            .as_ref()
            .and_then(PassTimer::render_writes)
    }

    /// After a precipitation pass, in its encoder.
    pub fn resolve_precipitation(&mut self, encoder: &mut wgpu::CommandEncoder) {
        if let Some(timer) = &mut self.precipitation_timer {
            timer.resolve(encoder);
        }
    }

    /// After a view's encoder was submitted: read its timed passes back.
    pub fn submitted(&mut self) {
        for timer in [
            &mut self.world_timer,
            &mut self.particles_timer,
            &mut self.clouds_timer,
            &mut self.backdrop_timer,
            &mut self.precipitation_timer,
            &mut self.post_timer,
            &mut self.finish_timer,
        ]
        .into_iter()
        .flatten()
        {
            timer.submitted();
        }
    }

    /// The clouds, backdrop, world, bloom and exposure, and finish timings,
    /// in frame order.
    pub fn timings(&self) -> Vec<GpuPassTiming> {
        [
            (&self.clouds_timer, CLOUDS),
            (&self.backdrop_timer, BACKDROP),
            (&self.world_timer, WORLD),
            (&self.particles_timer, PARTICLES),
            (&self.precipitation_timer, PRECIPITATION),
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
        fog_view: &wgpu::TextureView,
        fog_sampler: &wgpu::Sampler,
        timed: bool,
    ) {
        // The pipeline draws the image single-sample, averaging every sample
        // of a multisampled world.
        let key = ColorTarget {
            format: target.format,
            samples: target.samples,
        };
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
                    multisample: Default::default(),
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
        let bloom = hdr
            .bloom
            .iter()
            .find(|chain| chain.key == (viewport[2], viewport[3]))
            .filter(|_| post.bloom > 0.0);
        let shafts = hdr
            .shafts
            .iter()
            .find(|shafts| shafts.key == (viewport[2], viewport[3]))
            .filter(|_| post.shafts > 0.0);
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
            if shafts.is_some() { post.shafts } else { 0.0 },
            0.0,
            if post.fog.is_some() { 1.0 } else { 0.0 },
            post.fog.map_or(1.0, |fog| fog.distance),
            post.fog.map_or(1.0, |fog| fog.slices as f32),
            post.analytic_start,
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
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::TextureView(
                        shafts.map_or(&self.post.black, ShaftTargets::top),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: wgpu::BindingResource::TextureView(fog_view),
                },
                wgpu::BindGroupEntry {
                    binding: 10,
                    resource: wgpu::BindingResource::Sampler(fog_sampler),
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
                resolve_target: None,
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
