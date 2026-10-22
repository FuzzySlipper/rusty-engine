//! The upscale stage: the step from the primary picture, drawn at render
//! resolution, to the output at output resolution
//! (`Renderer::draw_primary`). Timed as `upscale`.
//!
//! A primary frame has two resolutions ([`FrameResolution`]): the output's
//! (the streamed frame or the window) and the render resolution the primary
//! views draw at. Where each pass runs:
//! - **Render resolution:** everything a primary view draws. That is the
//!   world with its sky, clouds, ambient occlusion, water copy and
//!   reflections, volumetric fog, bloom, exposure and finish; the overlay
//!   lines, drawn inside the world and viewmodel passes against their
//!   depth; the labels; the viewmodel; offscreen targets presented into the
//!   primary; and inspection views.
//! - **Output resolution:** this stage, or a product image effect in its
//!   place (`image_effect.rs`), which samples the picture into the output
//!   itself. Then a playing video and the hosts' UI.
//!
//! Offscreen composition targets draw at their own size either way.
//!
//! Labels and overlay lines stay at render resolution. Depth-layer labels
//! and overlay lines test the world's depth, which exists only at render
//! resolution and only until the viewmodel pass clears it. A later primary
//! view also covers an earlier view's labels, which drawing them over the
//! output after a whole-picture stage would undo.
//!
//! The stage reads the picture's colour and depth at render resolution, and
//! a temporal method also its motion target, jitter and history reset
//! (`motion.rs`), which the frame draws whenever such a method is selected.
//! At render scale 1 with no method selected and no image effect, there is
//! no picture and no stage: the views draw straight into the output.
//!
//! Three methods draw:
//! - **Bilinear:** each output pixel samples the picture. A render scale
//!   below 1 selects it.
//! - **TAAU** (`upscale.wgsl`): the temporal anti-aliasing and upscaling
//!   stage. `Antialiasing::Temporal` selects it at any render scale: at 1
//!   it is native-resolution TAA, and below it upscales (the named modes,
//!   `render_model::TEMPORAL_MODES`). The views jitter, and each output
//!   pixel gathers their frames into a history at output resolution: the
//!   previous history reprojected by the motion target (dilated to the
//!   nearest depth), clipped to this frame's neighbourhood in YCoCg, and
//!   blended by how many frames it holds and how near this frame's samples
//!   fall. Pixels with no motion of their own (water, `motion.rs`) lean
//!   on the current frame. Below render scale 1 the history is lightly
//!   sharpened into the output. The stage reads the finished picture,
//!   after tone mapping, so it needs no exposure.
//!
//!   A software adapter refuses it (`Renderer::temporal_refusal`) and
//!   draws bilinear at the render scale, without anti-aliasing. A product
//!   image effect replaces the stage: while one is set, TAAU is refused
//!   (`ImageEffect`) and the views do not jitter. Its history starts again
//!   whenever the frame says it no longer holds (`TemporalInputs`), the
//!   output's size changes, or a frame draws without it.
//! - **DLSS** (`dlss.rs`): NVIDIA's temporal upscaler, from the same inputs.
//!   `Antialiasing::Dlss` selects it, in builds with it, on adapters that
//!   run it; where it is refused, TAAU draws.

use render_shaders::{Entry, Features};

use crate::compose::Compose;
use crate::motion::TemporalInputs;
use crate::shaders::Shaders;
use crate::target::{RenderPicture, TargetView};
use crate::timing::{untimed, GpuPassTiming, PassTimer};
use crate::{Gpu, RendererOptions};

const PASS: &str = "upscale";

/// The history's format: colour and its frames.
const HISTORY_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// `Params` in `upscale.wgsl`: sizes, then jitter, reset and sharpening.
const UNIFORM_FLOATS: usize = 8;

/// TAAU's sharpening at a render scale (`fs_sharpen`, 0 to 1): none at
/// native resolution, rising to `MOST_SHARPENING` at half resolution and
/// below, where the history's resampling softens most.
const MOST_SHARPENING: f32 = 0.5;

fn sharpening(render_scale: f32) -> f32 {
    (MOST_SHARPENING * (1.0 - render_scale) / 0.5).clamp(0.0, MOST_SHARPENING)
}

/// The two sizes of a primary frame, in pixels: the output's and the render
/// resolution its views drew at. They are equal when nothing upscales.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FrameResolution {
    pub render: (u32, u32),
    pub output: (u32, u32),
}

impl FrameResolution {
    /// `scale` of the output's size, rounded, at least one pixel a side.
    pub(crate) fn scaled(output: (u32, u32), scale: f32) -> Self {
        let side = |side: u32| ((side as f32 * scale).round() as u32).max(1);
        Self {
            render: (side(output.0), side(output.1)),
            output,
        }
    }
}

/// How the stage brings the picture to output resolution
/// (`FrameStats::upscale`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpscaleMethod {
    /// Each output pixel samples the picture bilinearly.
    Bilinear,
    /// Temporal anti-aliasing and upscaling: jittered frames gathered into
    /// a history at output resolution.
    Taau,
    /// NVIDIA DLSS super resolution, from the same jittered frames.
    Dlss,
}

impl UpscaleMethod {
    /// The method `options` select, or `None` to draw straight into the
    /// output. `temporal` is whether the device and frame can run a
    /// temporal stage (no refusal and no image effect), and `dlss` whether
    /// DLSS can (no refusal).
    ///
    /// `Antialiasing::Temporal` selects TAAU at any render scale, and
    /// `Antialiasing::Dlss` DLSS, or TAAU where DLSS is refused; otherwise
    /// a render scale below 1 selects bilinear. One setting holds MSAA and
    /// the temporal stages, so they exclude each other by its shape.
    pub(crate) fn selected(options: &RendererOptions, temporal: bool, dlss: bool) -> Option<Self> {
        if options.temporal && temporal {
            Some(if options.dlss && dlss {
                Self::Dlss
            } else {
                Self::Taau
            })
        } else {
            (options.render_scale < 1.0).then_some(Self::Bilinear)
        }
    }

    /// Whether the method accumulates frames: the primary views then jitter
    /// and draw their motion for it (`motion.rs`).
    pub fn temporal(self) -> bool {
        match self {
            Self::Bilinear => false,
            Self::Taau | Self::Dlss => true,
        }
    }

    /// Its name in readouts: `bilinear`, `taau` or `dlss`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Bilinear => "bilinear",
            Self::Taau => "taau",
            Self::Dlss => "dlss",
        }
    }
}

/// TAAU's history: two textures at output resolution, the one the last
/// frame wrote and the one this frame writes.
struct History {
    size: (u32, u32),
    textures: [wgpu::TextureView; 2],
    /// Which texture the last frame wrote.
    last: usize,
}

impl History {
    fn new(gpu: &Gpu, (width, height): (u32, u32)) -> Self {
        let texture = || {
            gpu.device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("render-wgpu upscale history"),
                    size: crate::target::extent(width, height),
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: HISTORY_FORMAT,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        Self {
            size: (width, height),
            textures: [texture(), texture()],
            last: 0,
        }
    }
}

/// TAAU's shader, layout and pipelines, made when it first runs.
struct Taau {
    module: wgpu::ShaderModule,
    layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    /// The history alone, for the sharpening pass to read.
    resolve: wgpu::RenderPipeline,
    /// The history and the output at once, by output format.
    resolve_output: Vec<(wgpu::TextureFormat, wgpu::RenderPipeline)>,
    /// The sharpening pass, by output format.
    sharpen: Vec<(wgpu::TextureFormat, wgpu::RenderPipeline)>,
    uniform: wgpu::Buffer,
    sampler: wgpu::Sampler,
    history: Option<History>,
}

impl Taau {
    fn new(gpu: &Gpu, shaders: &mut Shaders) -> Self {
        let device = &gpu.device;
        let module = shaders
            .module(device, Entry::Upscale, Features::default())
            .expect("the upscale shader composes");
        let texture = |binding, sample_type| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type,
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("render-wgpu upscale taau"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new((UNIFORM_FLOATS * 4) as u64),
                    },
                    count: None,
                },
                texture(1, wgpu::TextureSampleType::Float { filterable: true }),
                texture(2, wgpu::TextureSampleType::Depth),
                texture(3, wgpu::TextureSampleType::Float { filterable: false }),
                texture(4, wgpu::TextureSampleType::Float { filterable: true }),
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                texture(6, wgpu::TextureSampleType::Depth),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("render-wgpu upscale taau"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let resolve = pipeline(
            device,
            &pipeline_layout,
            &module,
            "fs_resolve",
            &[Some(HISTORY_FORMAT.into())],
        );
        Self {
            module,
            layout,
            pipeline_layout,
            resolve,
            resolve_output: Vec::new(),
            sharpen: Vec::new(),
            uniform: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("render-wgpu upscale taau"),
                size: (UNIFORM_FLOATS * 4) as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("render-wgpu upscale history"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
            history: None,
        }
    }

    /// The pipeline `entry` makes for `format` among `made`, made once.
    fn by_format<'a>(
        made: &'a mut Vec<(wgpu::TextureFormat, wgpu::RenderPipeline)>,
        device: &wgpu::Device,
        layout: &wgpu::PipelineLayout,
        module: &wgpu::ShaderModule,
        entry: &str,
        format: wgpu::TextureFormat,
    ) -> &'a wgpu::RenderPipeline {
        let index = match made.iter().position(|(known, _)| *known == format) {
            Some(index) => index,
            None => {
                let targets: &[Option<wgpu::ColorTargetState>] = if entry == "fs_resolve_output" {
                    &[Some(HISTORY_FORMAT.into()), Some(format.into())]
                } else {
                    &[Some(format.into())]
                };
                made.push((format, pipeline(device, layout, module, entry, targets)));
                made.len() - 1
            }
        };
        &made[index].1
    }
}

fn pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    module: &wgpu::ShaderModule,
    entry: &str,
    targets: &[Option<wgpu::ColorTargetState>],
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("render-wgpu upscale taau"),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module,
            entry_point: Some("vs_fullscreen"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        fragment: Some(wgpu::FragmentState {
            module,
            entry_point: Some(entry),
            compilation_options: Default::default(),
            targets,
        }),
        multiview_mask: None,
        cache: None,
    })
}

pub(crate) struct Upscale {
    /// Bilinear's sampler.
    sampler: wgpu::Sampler,
    /// TAAU, from the first frame it runs.
    taau: Option<Taau>,
    /// DLSS, which starts the first time it is asked for.
    pub(crate) dlss: crate::dlss::Dlss,
    /// The temporal method the last frame ran: each keeps its own
    /// history, which a frame of the other breaks.
    last_temporal: Option<UpscaleMethod>,
    timer: Option<PassTimer>,
}

impl Upscale {
    pub fn new(gpu: &Gpu) -> Self {
        Self {
            sampler: gpu.device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("render-wgpu upscale"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
            taau: None,
            dlss: crate::dlss::Dlss::new(),
            last_temporal: None,
            timer: PassTimer::new(gpu, PASS),
        }
    }

    /// A primary frame drew without a temporal stage: their histories no
    /// longer hold, and go.
    pub fn forget_history(&mut self) {
        if let Some(taau) = &mut self.taau {
            taau.history = None;
        }
        self.dlss.forget_history();
        self.last_temporal = None;
    }

    /// Start DLSS on this device, once: the settings ask for it.
    pub fn prepare_dlss(&mut self, gpu: &Gpu) {
        self.dlss.prepare(gpu);
    }

    /// The render resolution DLSS draws from in a frame of `resolution`
    /// at `render_scale` (`dlss.rs`), or None if it refused as it started.
    pub fn dlss_render_size(
        &mut self,
        gpu: &Gpu,
        resolution: FrameResolution,
        render_scale: f32,
    ) -> Option<(u32, u32)> {
        self.dlss
            .render_size(gpu, resolution.output, resolution.render, render_scale)
            .ok()
    }

    /// Why DLSS does not draw, if it cannot.
    pub fn dlss_refusal(&self) -> Option<crate::DlssRefusal> {
        self.dlss.refusal().map(|refused| crate::DlssRefusal {
            refusal: refused.refusal,
            detail: refused.detail,
        })
    }

    /// Bring `picture` to `output` by `method`, over the whole output, and
    /// say which method drew: TAAU where DLSS fails as it runs, bilinear
    /// where a temporal method has no inputs. A temporal method reads
    /// `temporal`, which the frame drew for it, over a single-sample
    /// picture.
    #[allow(clippy::too_many_arguments, reason = "one stage's inputs")]
    pub fn run(
        &mut self,
        gpu: &Gpu,
        shaders: &mut Shaders,
        compose: &mut Compose,
        method: UpscaleMethod,
        picture: &RenderPicture,
        temporal: Option<TemporalInputs<'_>>,
        render_scale: f32,
        output: &TargetView<'_>,
    ) -> UpscaleMethod {
        if let Some(timer) = &mut self.timer {
            timer.collect(gpu);
        }
        if method.temporal() && self.last_temporal != Some(method) {
            self.forget_history();
        }
        self.last_temporal = method.temporal().then_some(method);
        let drawn = match (method, temporal) {
            (UpscaleMethod::Taau, Some(inputs)) => {
                self.taau(gpu, shaders, picture, &inputs, render_scale, output);
                UpscaleMethod::Taau
            }
            (UpscaleMethod::Dlss, Some(inputs)) => {
                let drawn = self.dlss.run(
                    gpu,
                    compose,
                    &self.sampler,
                    self.timer.as_mut(),
                    picture,
                    &inputs,
                    render_scale,
                    output,
                );
                // Refused as it ran: TAAU draws this frame, and the next
                // selects it.
                if drawn.is_ok() {
                    UpscaleMethod::Dlss
                } else {
                    self.last_temporal = Some(UpscaleMethod::Taau);
                    self.taau(gpu, shaders, picture, &inputs, render_scale, output);
                    UpscaleMethod::Taau
                }
            }
            _ => {
                self.forget_history();
                self.bilinear(gpu, compose, picture, output);
                UpscaleMethod::Bilinear
            }
        };
        if let Some(timer) = &mut self.timer {
            timer.submitted();
        }
        drawn
    }

    fn taau(
        &mut self,
        gpu: &Gpu,
        shaders: &mut Shaders,
        picture: &RenderPicture,
        inputs: &TemporalInputs<'_>,
        render_scale: f32,
        output: &TargetView<'_>,
    ) {
        let taau = self.taau.get_or_insert_with(|| Taau::new(gpu, shaders));
        let size = (output.width, output.height);
        let mut reset = inputs.history_reset;
        if taau
            .history
            .as_ref()
            .is_none_or(|history| history.size != size)
        {
            taau.history = Some(History::new(gpu, size));
            reset = true;
        }
        let (width, height) = picture.size();
        let sharpening = sharpening(render_scale);
        let values: [f32; UNIFORM_FLOATS] = [
            width as f32,
            height as f32,
            output.width as f32,
            output.height as f32,
            inputs.jitter.x,
            inputs.jitter.y,
            if reset { 1.0 } else { 0.0 },
            sharpening,
        ];
        gpu.queue
            .write_buffer(&taau.uniform, 0, bytemuck::cast_slice(&values));
        let history = taau.history.as_mut().expect("made above");
        let (read, write) = (history.last, 1 - history.last);
        history.last = write;
        let bind_group = |source: &wgpu::TextureView| {
            gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("render-wgpu upscale taau"),
                layout: &taau.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: taau.uniform.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(picture.image()),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(picture.depth()),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(inputs.motion),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::TextureView(source),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: wgpu::BindingResource::Sampler(&taau.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 6,
                        resource: wgpu::BindingResource::TextureView(
                            picture.viewmodel_depth().unwrap_or(picture.depth()),
                        ),
                    },
                ],
            })
        };
        let resolving = bind_group(&history.textures[read]);
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render-wgpu upscale taau"),
            });
        let attachment = |view| {
            Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })
        };
        let sharpened = sharpening > 0.0;
        {
            let resolve = if sharpened {
                &taau.resolve
            } else {
                Taau::by_format(
                    &mut taau.resolve_output,
                    &gpu.device,
                    &taau.pipeline_layout,
                    &taau.module,
                    "fs_resolve_output",
                    output.format,
                )
            };
            let attachments = [
                attachment(&history.textures[write]),
                attachment(output.color),
            ];
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render-wgpu upscale taau"),
                color_attachments: &attachments[..if sharpened { 1 } else { 2 }],
                depth_stencil_attachment: None,
                timestamp_writes: self
                    .timer
                    .as_ref()
                    .and_then(|timer| timer.render_writes_between(true, !sharpened)),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(resolve);
            pass.set_bind_group(0, &resolving, &[]);
            pass.draw(0..3, 0..1);
        }
        if sharpened {
            let sharpening = bind_group(&history.textures[write]);
            let sharpen = Taau::by_format(
                &mut taau.sharpen,
                &gpu.device,
                &taau.pipeline_layout,
                &taau.module,
                "fs_sharpen",
                output.format,
            );
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render-wgpu upscale sharpen"),
                color_attachments: &[attachment(output.color)],
                depth_stencil_attachment: None,
                timestamp_writes: self
                    .timer
                    .as_ref()
                    .and_then(|timer| timer.render_writes_between(false, true)),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(sharpen);
            pass.set_bind_group(0, &sharpening, &[]);
            pass.draw(0..3, 0..1);
        }
        if let Some(timer) = &mut self.timer {
            timer.resolve(&mut encoder);
        }
        gpu.queue.submit([encoder.finish()]);
    }

    fn bilinear(
        &mut self,
        gpu: &Gpu,
        compose: &mut Compose,
        picture: &RenderPicture,
        output: &TargetView<'_>,
    ) {
        compose.prepare_blit(&gpu.device, output.format);
        let group = compose.blit_bind_group(&gpu.device, picture.image(), &self.sampler);
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render-wgpu upscale"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render-wgpu upscale"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: output.color,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: self.timer.as_ref().and_then(PassTimer::render_writes),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            compose.blit(&mut pass, output.format, &group);
        }
        if let Some(timer) = &mut self.timer {
            timer.resolve(&mut encoder);
        }
        gpu.queue.submit([encoder.finish()]);
    }

    /// The stage's timing, for `gpu.passes`.
    pub fn timing(&self) -> GpuPassTiming {
        self.timer
            .as_ref()
            .map_or_else(|| untimed(PASS), PassTimer::readout)
    }
}
