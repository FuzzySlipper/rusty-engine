//! Bloom and auto exposure (`post.wgsl`): what the finish pass adds to and
//! scales a view's HDR world by. Both read the world resolved to one sample,
//! only within the view's viewport: views sharing a target never see each
//! other.
//!
//! Bloom: the world's light above the threshold, downsampled into a half
//! resolution mip chain, then upsampled back with each level added onto the
//! next larger. Auto exposure: coverage-weighted log luminance at 128²,
//! averaged down to one texel, and an adaptation that moves a persistent
//! exposure toward the one bringing it to middle grey over presentation time.

use render_model::{AutoExposureDescriptor, BloomDescriptor};

use crate::gpu::Gpu;
use crate::pipelines::standard;
use crate::shaders::{Entry, Features, Shaders};
use crate::timing::PassTimer;

pub(crate) const BLOOM_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
const LUMINANCE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg16Float;
const EXPOSURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R32Float;
/// Bloom mips below the half-resolution one, at most.
const BLOOM_LEVELS: u32 = 6;
/// The source region of a pass that reads a whole texture.
const WHOLE: [f32; 4] = [0.0, 0.0, 1.0, 1.0];
/// The luminance chain's first level: 128², averaged down to 1².
const LUMINANCE_SIZE: u32 = 128;
/// The soft knee below the bloom threshold, as a fraction of it.
const BLOOM_KNEE: f32 = 0.5;
/// Uniform slots a frame's post passes start with, each 256 bytes apart;
/// a frame that needs more doubles them.
const PARAM_SLOTS: u64 = 64;
const PARAM_STRIDE: u64 = 256;
/// Bytes of `PostParams`: texel, adaptation, source region.
const PARAMS_BYTES: u64 = 48;

/// A mip chain: the texture's per-level views and sizes, the viewport size
/// it was made for and the frame it was last used in.
pub(crate) struct Chain {
    views: Vec<wgpu::TextureView>,
    sizes: Vec<(u32, u32)>,
    pub key: (u32, u32),
    pub used: u64,
}

impl Chain {
    fn new(
        gpu: &Gpu,
        label: &str,
        format: wgpu::TextureFormat,
        size: (u32, u32),
        levels: u32,
    ) -> Self {
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: crate::target::extent(size.0, size.1),
            mip_level_count: levels,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let views = (0..levels)
            .map(|level| {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some(label),
                    base_mip_level: level,
                    mip_level_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();
        let sizes = (0..levels)
            .map(|level| ((size.0 >> level).max(1), (size.1 >> level).max(1)))
            .collect();
        Self {
            views,
            sizes,
            key: size,
            used: 0,
        }
    }

    /// The bloom chain of a `width`×`height` viewport: half resolution and
    /// smaller, down to a few texels.
    pub fn bloom(gpu: &Gpu, width: u32, height: u32) -> Self {
        let size = ((width / 2).max(1), (height / 2).max(1));
        let levels = (1..=BLOOM_LEVELS)
            .take_while(|&level| size.0.min(size.1) >> (level - 1) >= 4)
            .count()
            .max(1) as u32;
        Self {
            key: (width, height),
            ..Self::new(gpu, "render-wgpu bloom", BLOOM_FORMAT, size, levels)
        }
    }

    pub fn luminance(gpu: &Gpu) -> Self {
        let levels = LUMINANCE_SIZE.trailing_zeros() + 1;
        Self::new(
            gpu,
            "render-wgpu luminance",
            LUMINANCE_FORMAT,
            (LUMINANCE_SIZE, LUMINANCE_SIZE),
            levels,
        )
    }

    /// The finished bloom the finish pass samples.
    pub fn top(&self) -> &wgpu::TextureView {
        &self.views[0]
    }
}

/// A post timer, and whether a sequence of passes opens (begin stamp) and
/// closes (end stamp) its time.
pub(crate) type Stamps<'a> = Option<(&'a PassTimer, bool, bool)>;

/// The stamps of a pass that is a sequence's `first` and/or `last`.
fn at(stamps: Stamps<'_>, first: bool, last: bool) -> Stamps<'_> {
    stamps.and_then(|(timer, begin, end)| {
        let (begin, end) = (begin && first, end && last);
        (begin || end).then_some((timer, begin, end))
    })
}

pub(crate) struct Post {
    shader: wgpu::ShaderModule,
    layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    sampler: wgpu::Sampler,
    params: wgpu::Buffer,
    /// The uniform slots `params` holds, and the next free one this frame.
    slots: u64,
    slot: u64,
    pipelines: Vec<(
        (&'static str, wgpu::TextureFormat, bool),
        wgpu::RenderPipeline,
    )>,
    /// A 1×1 transparent black texture: no bloom.
    pub black: wgpu::TextureView,
    /// Auto exposure's two 1×1 values, the current one first, and the
    /// presentation time it was adapted at.
    exposure: [wgpu::TextureView; 2],
    current: usize,
    adapted_at: Option<f64>,
    /// A 1×1 exposure of 1: no auto exposure.
    pub unit: wgpu::TextureView,
}

impl Post {
    pub fn new(gpu: &Gpu, shaders: &mut Shaders) -> Self {
        let device = &gpu.device;
        let shader = standard(shaders.module(device, Entry::Post, Features::default()));
        let texture_entry = |binding, filterable| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("render-wgpu post"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: wgpu::BufferSize::new(PARAMS_BYTES),
                    },
                    count: None,
                },
                texture_entry(1, true),
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                texture_entry(3, false),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("render-wgpu post"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pixel = |label, format, bytes: &[u8]| {
            use wgpu::util::DeviceExt;
            gpu.device
                .create_texture_with_data(
                    &gpu.queue,
                    &wgpu::TextureDescriptor {
                        label: Some(label),
                        size: crate::target::extent(1, 1),
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING
                            | wgpu::TextureUsages::RENDER_ATTACHMENT,
                        view_formats: &[],
                    },
                    wgpu::util::TextureDataOrder::LayerMajor,
                    bytes,
                )
                .create_view(&Default::default())
        };
        let one = 1.0f32.to_le_bytes();
        Self {
            shader,
            layout,
            pipeline_layout,
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("render-wgpu post"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
            params: params_buffer(device, PARAM_SLOTS),
            slots: PARAM_SLOTS,
            slot: 0,
            pipelines: Vec::new(),
            black: pixel("render-wgpu no bloom", BLOOM_FORMAT, &[0; 8]),
            exposure: [
                pixel("render-wgpu exposure", EXPOSURE_FORMAT, &one),
                pixel("render-wgpu exposure", EXPOSURE_FORMAT, &one),
            ],
            current: 0,
            adapted_at: None,
            unit: pixel("render-wgpu unit exposure", EXPOSURE_FORMAT, &one),
        }
    }

    /// A new frame's passes start at the first uniform slot.
    pub fn begin_frame(&mut self) {
        self.slot = 0;
    }

    /// The exposure auto exposure has adapted to.
    pub fn exposure(&self) -> &wgpu::TextureView {
        &self.exposure[self.current]
    }

    /// Auto exposure was turned off: the next adaptation starts afresh.
    pub fn reset_exposure(&mut self) {
        self.adapted_at = None;
    }

    fn pipeline(
        &mut self,
        device: &wgpu::Device,
        fragment: &'static str,
        format: wgpu::TextureFormat,
        additive: bool,
    ) -> usize {
        if let Some(index) = self
            .pipelines
            .iter()
            .position(|(key, _)| *key == (fragment, format, additive))
        {
            return index;
        }
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("render-wgpu post"),
            layout: Some(&self.pipeline_layout),
            vertex: wgpu::VertexState {
                module: &self.shader,
                entry_point: Some("vs_post"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &self.shader,
                entry_point: Some(fragment),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: additive.then_some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent::REPLACE,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        self.pipelines
            .push(((fragment, format, additive), pipeline));
        self.pipelines.len() - 1
    }

    /// One full-target pass of `fragment` from `region` of `source` (x, y,
    /// width, height in its uv) into `target`, with the timer's begin and
    /// end stamps as `stamps` asks.
    #[allow(clippy::too_many_arguments)]
    fn pass(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        fragment: &'static str,
        (target, format): (&wgpu::TextureView, wgpu::TextureFormat),
        additive: bool,
        (source, region): (&wgpu::TextureView, [f32; 4]),
        previous: Option<&wgpu::TextureView>,
        params: [f32; 8],
        stamps: Stamps<'_>,
    ) {
        if self.slot == self.slots {
            // Earlier passes this frame keep the full buffer they bound.
            self.slots *= 2;
            self.params = params_buffer(&gpu.device, self.slots);
            self.slot = 0;
        }
        let offset = self.slot * PARAM_STRIDE;
        self.slot += 1;
        let mut values = [0.0f32; 12];
        values[..8].copy_from_slice(&params);
        values[8..].copy_from_slice(&region);
        gpu.queue
            .write_buffer(&self.params, offset, bytemuck::cast_slice(&values));
        let pipeline = self.pipeline(&gpu.device, fragment, format, additive);
        let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("render-wgpu post"),
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
                    resource: wgpu::BindingResource::TextureView(source),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(previous.unwrap_or(&self.unit)),
                },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(fragment),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: if additive {
                        wgpu::LoadOp::Load
                    } else {
                        wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)
                    },
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: stamps
                .and_then(|(timer, begin, end)| timer.render_writes_between(begin, end)),
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipelines[pipeline].1);
        pass.set_bind_group(0, &group, &[offset as u32]);
        pass.draw(0..3, 0..1);
    }

    /// Bloom from `region` (x, y, width, height in uv) of the resolved world
    /// (`source`, `size` texels) into `chain`; `stamps` opens on its first
    /// pass and closes on its last as asked.
    #[allow(clippy::too_many_arguments)]
    pub fn bloom(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        (source, region): (&wgpu::TextureView, [f32; 4]),
        size: (u32, u32),
        chain: &Chain,
        bloom: BloomDescriptor,
        stamps: Stamps<'_>,
    ) {
        let texel = |(width, height): (u32, u32)| [1.0 / width as f32, 1.0 / height as f32];
        let [x, y] = texel(size);
        self.pass(
            gpu,
            encoder,
            "fs_bloom_prefilter",
            (&chain.views[0], BLOOM_FORMAT),
            false,
            (source, region),
            None,
            [
                x,
                y,
                bloom.threshold,
                bloom.threshold * BLOOM_KNEE,
                0.0,
                0.0,
                0.0,
                0.0,
            ],
            at(stamps, true, chain.views.len() == 1),
        );
        for level in 1..chain.views.len() {
            let [x, y] = texel(chain.sizes[level - 1]);
            self.pass(
                gpu,
                encoder,
                "fs_bloom_down",
                (&chain.views[level], BLOOM_FORMAT),
                false,
                (&chain.views[level - 1], WHOLE),
                None,
                [x, y, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                None,
            );
        }
        for level in (0..chain.views.len() - 1).rev() {
            let [x, y] = texel(chain.sizes[level + 1]);
            self.pass(
                gpu,
                encoder,
                "fs_bloom_up",
                (&chain.views[level], BLOOM_FORMAT),
                true,
                (&chain.views[level + 1], WHOLE),
                None,
                [x, y, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                at(stamps, false, level == 0),
            );
        }
    }

    /// Measure the luminance of `region` of the resolved world (`size`
    /// texels) into `chain` and adapt the exposure toward it as of
    /// presentation time `now`; `stamps` as for `bloom`.
    #[allow(clippy::too_many_arguments)]
    pub fn adapt(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        (source, region): (&wgpu::TextureView, [f32; 4]),
        size: (u32, u32),
        chain: &Chain,
        auto: AutoExposureDescriptor,
        now: f64,
        stamps: Stamps<'_>,
    ) {
        self.pass(
            gpu,
            encoder,
            "fs_luminance",
            (&chain.views[0], LUMINANCE_FORMAT),
            false,
            (source, region),
            None,
            [
                1.0 / size.0 as f32,
                1.0 / size.1 as f32,
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
            ],
            at(stamps, true, false),
        );
        for level in 1..chain.views.len() {
            self.pass(
                gpu,
                encoder,
                "fs_luminance_down",
                (&chain.views[level], LUMINANCE_FORMAT),
                false,
                (&chain.views[level - 1], WHOLE),
                None,
                [0.0; 8],
                None,
            );
        }
        let seconds = self
            .adapted_at
            .map_or(-1.0, |at| (now - at).max(0.0) as f32);
        self.adapted_at = Some(now);
        let next = 1 - self.current;
        let [previous, target] = if next == 1 {
            let [first, second] = &self.exposure;
            [first.clone(), second.clone()]
        } else {
            let [first, second] = &self.exposure;
            [second.clone(), first.clone()]
        };
        let last = chain
            .views
            .last()
            .expect("a luminance chain has levels")
            .clone();
        self.pass(
            gpu,
            encoder,
            "fs_adapt",
            (&target, EXPOSURE_FORMAT),
            false,
            (&last, WHOLE),
            Some(&previous),
            [
                0.0,
                0.0,
                0.0,
                0.0,
                auto.speed,
                auto.min_exposure,
                auto.max_exposure,
                seconds,
            ],
            at(stamps, false, true),
        );
        self.current = next;
    }
}

/// The post passes' uniform buffer of `slots` slots.
fn params_buffer(device: &wgpu::Device, slots: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("render-wgpu post params"),
        size: slots * PARAM_STRIDE,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}
