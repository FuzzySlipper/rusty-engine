//! NVIDIA DLSS super resolution as a temporal upscale stage
//! (`UpscaleMethod::Dlss`, `Antialiasing::Dlss`), through `dlss_wgpu`.
//!
//! Built only with the `dlss` cargo feature: its build needs NVIDIA's DLSS
//! SDK (docs/lighting-and-sky.md#dlss). A request DLSS cannot draw is
//! refused with its reason (`Renderer::dlss_refusal`, the detail in
//! `GpuReadout::upscale`) and TAAU draws in its place:
//! - a build without the feature: `SettingRefusal::NotBuilt`;
//! - a software adapter: `SoftwareAdapter`, where TAAU refuses too;
//! - an adapter without DLSS (not Vulkan, not NVIDIA, without NGX's Vulkan
//!   extensions, or one NGX says cannot run it, as on every card before
//!   RTX): `UnsupportedAdapter`;
//! - NGX failing to start, or to make or evaluate the feature:
//!   `InitializationFailed`.
//!
//! The device is made with NGX's Vulkan instance and device extensions on an
//! NVIDIA adapter (`gpu.rs`). NGX starts the first time DLSS is asked for,
//! once per device: every renderer on the device shares it (`Gpu::ngx`), as
//! NGX shuts down a whole device's features when one of its starts ends.
//!
//! It reads what TAAU reads (`motion.rs`): the picture, its depth, the
//! motion target, the jitter (the Halton(2, 3) sequence DLSS suggests) and
//! the history reset. The viewmodel draws against a depth of its own, which
//! TAAU keys as nearest; DLSS takes one depth, so a pass writes the world's
//! depth with the viewmodel's over it, scaled before every world surface
//! (`dlss_inputs.wgsl`). The motion target holds how far each point moved
//! in render pixels; DLSS takes the way back, so its scale is -1, and reads
//! only its first two channels. Its reactive mask (`motion.rs`) goes to
//! DLSS as the bias toward the current colour, drawn out into a target of
//! its own; where it says the picture was drawn unjittered (labels, overlay
//! lines), the pass that draws DLSS's output into the frame takes the
//! picture as it stands, as TAAU does. `NO_MOTION` (a viewport's clear)
//! reads to DLSS as a point come from far off screen, so there it takes the
//! current frame, as TAAU leans on it. The picture is finished and
//! tone-mapped, so DLSS runs on low dynamic range with no exposure. It writes a storage texture at output size (a surface
//! cannot be one), which is then drawn into the output; the `upscale` pass
//! times both.
//!
//! **Ray Reconstruction** (`UpscaleMethod::DlssRr`) is a second feature on
//! the same NGX, for a frame that asks for it. It denoises and upscales the
//! world's linear HDR before finish, from the guide targets (`motion.rs`), so
//! it runs inside the world view (`frame.rs`), not as the output stage:
//! bloom, exposure, finish, overlay lines, labels and the viewmodel draw
//! after it at output size, unjittered. The background is not in the HDR
//! target: the world's alpha carries through RR (`AlphaUpscaling`) and
//! finish blends it over a background drawn at output size. Blended parts are
//! in the colour and marked by the reactive mask, as the responsivity mask.
//! RR takes no exposure. It is refused as DLSS is, and `UnsupportedAdapter`
//! where NGX offers no RR; the frame then keeps super resolution. A frame
//! whose RR fails as it evaluates scales the world bilinearly, once.
//!
//! The render scale picks the mode, as for TAAU: the named mode nearest it
//! (`render_model::TEMPORAL_MODES`, 1 is DLAA). The feature is made for that
//! mode at the output's size before the frame draws, and the frame draws at
//! the render scale held within the input sizes the mode takes
//! (`Dlss::render_size`): ultra performance takes only a third of the
//! output, which 0.33 rounds below. It is made again when the output's size
//! or the mode changes.

use crate::SettingRefusal;

/// Why DLSS does not draw, with the detail a readout shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Refused {
    pub refusal: SettingRefusal,
    pub detail: String,
}

/// The Engine's NGX project id, which NGX takes with the custom engine
/// type.
#[cfg(feature = "dlss")]
pub(crate) const PROJECT_ID: uuid::Uuid =
    uuid::Uuid::from_u128(0x5b0e_6c21_93a4_4d8e_b7f1_2c6a_0d94_e317);

/// NVIDIA's PCI vendor id.
#[cfg(feature = "dlss")]
pub(crate) const NVIDIA: u32 = 0x10de;

/// What Ray Reconstruction reads and writes in a world view.
#[cfg_attr(
    not(feature = "dlss"),
    allow(dead_code, reason = "read only by the built stage")
)]
pub(crate) struct Reconstruction<'a> {
    /// The world's HDR and its depth, at render resolution.
    pub color: &'a wgpu::TextureView,
    pub depth: &'a wgpu::TextureView,
    pub render: (u32, u32),
    pub inputs: &'a crate::motion::TemporalInputs<'a>,
    /// What global illumination's specular rays give it, once they trace.
    pub specular: Option<SpecularRays<'a>>,
    /// The HDR target at output resolution, which it fills.
    pub output: &'a wgpu::TextureView,
    pub output_size: (u32, u32),
    pub render_scale: f32,
}

/// How far each pixel's specular ray went (`ray_gi.rs`), and the camera it
/// left, each matrix by rows.
#[cfg_attr(
    not(feature = "dlss"),
    allow(dead_code, reason = "read only by the built stage")
)]
pub(crate) struct SpecularRays<'a> {
    pub hit_distance: &'a wgpu::TextureView,
    pub world_to_view: [f32; 16],
    pub view_to_clip: [f32; 16],
}

#[cfg(not(feature = "dlss"))]
pub(crate) use unbuilt::Dlss;

#[cfg(not(feature = "dlss"))]
mod unbuilt {
    use super::{Reconstruction, Refused};
    use crate::motion::TemporalInputs;
    use crate::target::{RenderPicture, TargetView};
    use crate::timing::PassTimer;
    use crate::{Gpu, SettingRefusal};

    #[derive(Default)]
    pub(crate) struct Dlss {
        /// Tests only: DLSS takes the frame and fails as it evaluates, with
        /// this refusal, which stays from then on.
        #[cfg(test)]
        pub failing: Option<Refused>,
        #[cfg(test)]
        failed: Option<Refused>,
    }

    impl Dlss {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn prepare(&mut self, _gpu: &Gpu) {}

        pub fn refusal(&self) -> Option<Refused> {
            #[cfg(test)]
            if self.failing.is_some() {
                return self.failed.clone();
            }
            Some(Refused {
                refusal: SettingRefusal::NotBuilt,
                detail: "this build has no DLSS (render-wgpu's `dlss` feature)".to_owned(),
            })
        }

        pub fn forget_history(&mut self) {}

        pub fn ray_reconstruction_refusal(&self) -> Option<Refused> {
            self.refusal()
        }

        /// Tests only: under `failing`, RR takes the frame at `scaled`.
        pub fn ray_reconstruction_size(
            &mut self,
            _gpu: &Gpu,
            _output: (u32, u32),
            scaled: (u32, u32),
            _render_scale: f32,
        ) -> Result<(u32, u32), Refused> {
            #[cfg(test)]
            if self.failing.is_some() {
                return Ok(scaled);
            }
            let _ = scaled;
            Err(self.refusal().expect("refused without the feature"))
        }

        pub fn reconstruct(
            &mut self,
            _gpu: &Gpu,
            _timer: Option<&mut PassTimer>,
            encoder: wgpu::CommandEncoder,
            _work: &Reconstruction<'_>,
        ) -> Result<(), (Refused, wgpu::CommandEncoder)> {
            #[cfg(test)]
            if let Some(failing) = &self.failing {
                self.failed = Some(failing.clone());
                return Err((failing.clone(), encoder));
            }
            Err((
                self.refusal().expect("refused without the feature"),
                encoder,
            ))
        }

        pub fn render_size(
            &mut self,
            _gpu: &Gpu,
            _output: (u32, u32),
            scaled: (u32, u32),
            _render_scale: f32,
        ) -> Result<(u32, u32), Refused> {
            #[cfg(test)]
            if self.failing.is_some() {
                return Ok(scaled);
            }
            let _ = scaled;
            Err(self.refusal().expect("refused without the feature"))
        }

        #[allow(clippy::too_many_arguments, reason = "one stage's inputs")]
        pub fn run(
            &mut self,
            _gpu: &Gpu,
            _sampler: &wgpu::Sampler,
            _timer: Option<&mut PassTimer>,
            _picture: &RenderPicture,
            _inputs: &TemporalInputs<'_>,
            _render_scale: f32,
            _output: &TargetView<'_>,
        ) -> Result<(), Refused> {
            #[cfg(test)]
            if let Some(failing) = &self.failing {
                self.failed = Some(failing.clone());
                return Err(failing.clone());
            }
            Err(self.refusal().expect("refused without the feature"))
        }
    }
}

#[cfg(feature = "dlss")]
pub(crate) use built::{Dlss, Ngx};

#[cfg(feature = "dlss")]
mod built {
    use std::sync::{Arc, Mutex, OnceLock};

    use dlss_wgpu::ray_reconstruction::{
        DlssRayReconstruction, DlssRayReconstructionDepthMode,
        DlssRayReconstructionRenderParameters, DlssRayReconstructionRoughnessMode,
        DlssRayReconstructionSpecularGuide,
    };
    use dlss_wgpu::super_resolution::{
        DlssSuperResolution, DlssSuperResolutionExposure, DlssSuperResolutionRenderParameters,
    };
    use dlss_wgpu::{DlssError, DlssFeatureFlags, DlssPerfQualityMode, DlssSdk};

    use super::{Reconstruction, Refused, NVIDIA, PROJECT_ID};
    use crate::motion::TemporalInputs;
    use crate::target::{RenderPicture, TargetView};
    use crate::timing::PassTimer;
    use crate::{Gpu, SettingRefusal};

    /// What DLSS writes: colour at output size, which a storage binding
    /// takes.
    const OUTPUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

    /// The DLSS mode of each of `render_model::TEMPORAL_MODES`, in order.
    const MODES: [DlssPerfQualityMode; render_model::TEMPORAL_MODES.len()] = [
        DlssPerfQualityMode::Dlaa,
        DlssPerfQualityMode::Quality,
        DlssPerfQualityMode::Balanced,
        DlssPerfQualityMode::Performance,
        DlssPerfQualityMode::UltraPerformance,
    ];

    /// The motion target holds how far a point moved; DLSS takes where it
    /// was.
    const MOTION_SCALE: [f32; 2] = [-1.0, -1.0];

    /// The named mode nearest `render_scale`.
    fn mode(render_scale: f32) -> DlssPerfQualityMode {
        let nearest = render_model::TEMPORAL_MODES
            .iter()
            .enumerate()
            .min_by(|(_, (_, a)), (_, (_, b))| {
                (a - render_scale)
                    .abs()
                    .total_cmp(&(b - render_scale).abs())
            })
            .map_or(0, |(index, _)| index);
        MODES[nearest]
    }

    fn failed(what: &str, error: DlssError) -> Refused {
        Refused {
            refusal: SettingRefusal::InitializationFailed,
            detail: format!("{what}: {error}"),
        }
    }

    /// NGX on one device, from the first time a renderer there asks for
    /// DLSS, or why it refused.
    pub(crate) type Ngx = Arc<OnceLock<Result<Arc<Mutex<DlssSdk>>, Refused>>>;

    /// One of DLSS's inputs drawn from the stage's own at render resolution
    /// (`dlss_inputs.wgsl`): the world's and the viewmodel's depth in one,
    /// or the reactive mask out of the motion target.
    struct Input {
        layout: wgpu::BindGroupLayout,
        pipeline: wgpu::RenderPipeline,
        format: wgpu::TextureFormat,
        /// The textures it reads, at these bindings.
        first_binding: u32,
        /// The last render size's target.
        target: Option<((u32, u32), wgpu::TextureView)>,
    }

    impl Input {
        /// The depth DLSS reads: `fs_depth` over the world's depth (binding
        /// 0) and the viewmodel's (1).
        fn depth(device: &wgpu::Device) -> Self {
            Self::new(
                device,
                "fs_depth",
                wgpu::TextureFormat::R32Float,
                0,
                &[wgpu::TextureSampleType::Depth; 2],
            )
        }

        /// DLSS's bias toward the current colour: `fs_bias` over the motion
        /// target (binding 2).
        fn bias(device: &wgpu::Device) -> Self {
            Self::new(
                device,
                "fs_bias",
                wgpu::TextureFormat::R8Unorm,
                2,
                &[wgpu::TextureSampleType::Float { filterable: false }],
            )
        }

        fn new(
            device: &wgpu::Device,
            entry: &str,
            format: wgpu::TextureFormat,
            first_binding: u32,
            reads: &[wgpu::TextureSampleType],
        ) -> Self {
            let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("render-wgpu dlss inputs"),
                source: wgpu::ShaderSource::Wgsl(include_str!("dlss_inputs.wgsl").into()),
            });
            let entries: Vec<wgpu::BindGroupLayoutEntry> = reads
                .iter()
                .zip(first_binding..)
                .map(|(sample_type, binding)| wgpu::BindGroupLayoutEntry {
                    binding,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: *sample_type,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                })
                .collect();
            let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("render-wgpu dlss inputs"),
                entries: &entries,
            });
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("render-wgpu dlss inputs"),
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 0,
            });
            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("render-wgpu dlss inputs"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs_fullscreen"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    targets: &[Some(format.into())],
                }),
                multiview_mask: None,
                cache: None,
            });
            Self {
                layout,
                pipeline,
                format,
                first_binding,
                target: None,
            }
        }

        /// Draw from `reads` into the target for `size`, in `encoder`.
        fn draw(
            &mut self,
            device: &wgpu::Device,
            encoder: &mut wgpu::CommandEncoder,
            size: (u32, u32),
            reads: &[&wgpu::TextureView],
        ) -> &wgpu::TextureView {
            if self.target.as_ref().is_none_or(|(made, _)| *made != size) {
                let view = device
                    .create_texture(&wgpu::TextureDescriptor {
                        label: Some("render-wgpu dlss input"),
                        size: crate::target::extent(size.0, size.1),
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: self.format,
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                            | wgpu::TextureUsages::TEXTURE_BINDING,
                        view_formats: &[],
                    })
                    .create_view(&Default::default());
                self.target = Some((size, view));
            }
            let target = &self.target.as_ref().expect("made above").1;
            let entries: Vec<wgpu::BindGroupEntry<'_>> = reads
                .iter()
                .zip(self.first_binding..)
                .map(|(view, binding)| wgpu::BindGroupEntry {
                    binding,
                    resource: wgpu::BindingResource::TextureView(view),
                })
                .collect();
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("render-wgpu dlss inputs"),
                layout: &self.layout,
                entries: &entries,
            });
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render-wgpu dlss inputs"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // The triangle covers it all.
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.draw(0..3, 0..1);
            drop(pass);
            target
        }
    }

    /// The pass that draws DLSS's output into the frame (`dlss_inputs.wgsl`
    /// `fs_output`): where the reactive mask says the picture was drawn
    /// unjittered (labels and overlay lines), the picture as it stands, as
    /// TAAU takes it; DLSS would reconstruct those pixels as jittered.
    struct Output {
        layout: wgpu::BindGroupLayout,
        pipeline_layout: wgpu::PipelineLayout,
        module: wgpu::ShaderModule,
        /// By the frame's format.
        pipelines: Vec<(wgpu::TextureFormat, wgpu::RenderPipeline)>,
    }

    impl Output {
        fn new(device: &wgpu::Device) -> Self {
            let texture = |binding, filterable| wgpu::BindGroupLayoutEntry {
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
                label: Some("render-wgpu dlss output"),
                entries: &[
                    texture(2, false),
                    texture(3, true),
                    texture(4, true),
                    wgpu::BindGroupLayoutEntry {
                        binding: 5,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("render-wgpu dlss output"),
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 0,
            });
            Self {
                module: device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("render-wgpu dlss output"),
                    source: wgpu::ShaderSource::Wgsl(include_str!("dlss_inputs.wgsl").into()),
                }),
                layout,
                pipeline_layout,
                pipelines: Vec::new(),
            }
        }

        fn bind_group(
            &self,
            device: &wgpu::Device,
            sampler: &wgpu::Sampler,
            upscaled: &wgpu::TextureView,
            picture: &wgpu::TextureView,
            motion: &wgpu::TextureView,
        ) -> wgpu::BindGroup {
            let texture = |binding, view| wgpu::BindGroupEntry {
                binding,
                resource: wgpu::BindingResource::TextureView(view),
            };
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("render-wgpu dlss output"),
                layout: &self.layout,
                entries: &[
                    texture(2, motion),
                    texture(3, upscaled),
                    texture(4, picture),
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: wgpu::BindingResource::Sampler(sampler),
                    },
                ],
            })
        }

        /// The pipeline for a frame of `format`, made once.
        fn pipeline(
            &mut self,
            device: &wgpu::Device,
            format: wgpu::TextureFormat,
        ) -> &wgpu::RenderPipeline {
            let index = match self.pipelines.iter().position(|(made, _)| *made == format) {
                Some(index) => index,
                None => {
                    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                        label: Some("render-wgpu dlss output"),
                        layout: Some(&self.pipeline_layout),
                        vertex: wgpu::VertexState {
                            module: &self.module,
                            entry_point: Some("vs_fullscreen"),
                            compilation_options: Default::default(),
                            buffers: &[],
                        },
                        primitive: Default::default(),
                        depth_stencil: None,
                        multisample: Default::default(),
                        fragment: Some(wgpu::FragmentState {
                            module: &self.module,
                            entry_point: Some("fs_output"),
                            compilation_options: Default::default(),
                            targets: &[Some(format.into())],
                        }),
                        multiview_mask: None,
                        cache: None,
                    });
                    self.pipelines.push((format, pipeline));
                    self.pipelines.len() - 1
                }
            };
            &self.pipelines[index].1
        }
    }

    /// The feature for one output size and mode, and what it writes.
    struct Feature {
        size: (u32, u32),
        mode: DlssPerfQualityMode,
        dlss: DlssSuperResolution,
        output: wgpu::TextureView,
    }

    /// Ray Reconstruction for one output size and mode, and what it writes.
    struct Reconstructor {
        size: (u32, u32),
        mode: DlssPerfQualityMode,
        feature: DlssRayReconstruction,
        output: wgpu::TextureView,
    }

    pub(crate) struct Dlss {
        /// The device's NGX, from the first time DLSS is asked for, or why
        /// it refused.
        sdk: Option<Result<Arc<Mutex<DlssSdk>>, Refused>>,
        /// Why the last feature could not be made or evaluated.
        failed: Option<Refused>,
        feature: Option<Feature>,
        /// Ray Reconstruction, and why it could not be made or evaluated.
        reconstructor: Option<Reconstructor>,
        reconstruction_failed: Option<Refused>,
        /// The next frame starts RR's history again.
        reconstruction_fresh: bool,
        /// The depth pass, from the first frame with a viewmodel depth, and
        /// the reactive mask's, from the first frame.
        depth: Option<Input>,
        bias: Option<Input>,
        /// The pass that draws DLSS's output into the frame, from the first
        /// frame.
        output: Option<Output>,
        /// The next frame starts the history again.
        fresh: bool,
    }

    impl Dlss {
        pub fn new() -> Self {
            Self {
                sdk: None,
                failed: None,
                feature: None,
                reconstructor: None,
                reconstruction_failed: None,
                reconstruction_fresh: true,
                depth: None,
                bias: None,
                output: None,
                fresh: true,
            }
        }

        /// NGX on `gpu`, started once per device.
        pub fn prepare(&mut self, gpu: &Gpu) {
            if self.sdk.is_none() {
                self.sdk = Some(gpu.ngx.get_or_init(|| start(gpu)).clone());
            }
        }

        pub fn refusal(&self) -> Option<Refused> {
            match &self.sdk {
                Some(Err(refused)) => Some(refused.clone()),
                _ => self.failed.clone(),
            }
        }

        pub fn forget_history(&mut self) {
            self.fresh = true;
            self.reconstruction_fresh = true;
        }

        /// Why Ray Reconstruction does not draw, if it cannot.
        pub fn ray_reconstruction_refusal(&self) -> Option<Refused> {
            match &self.sdk {
                Some(Err(refused)) => Some(refused.clone()),
                _ => self.reconstruction_failed.clone(),
            }
        }

        /// The render resolution RR draws from for `output` in the mode
        /// nearest `render_scale`. Makes the feature; on failure the refusal
        /// stays.
        pub fn ray_reconstruction_size(
            &mut self,
            gpu: &Gpu,
            output: (u32, u32),
            _scaled: (u32, u32),
            render_scale: f32,
        ) -> Result<(u32, u32), Refused> {
            let result = self.reconstructor(gpu, output, render_scale).map(|made| {
                let [width, height] = made.feature.render_resolution();
                (width, height)
            });
            if let Err(refused) = &result {
                self.reconstruction_failed = Some(refused.clone());
                self.reconstructor = None;
            }
            result
        }

        /// RR for an output of `size` in the mode `render_scale` picks, made
        /// again when either changes.
        fn reconstructor(
            &mut self,
            gpu: &Gpu,
            size: (u32, u32),
            render_scale: f32,
        ) -> Result<&mut Reconstructor, Refused> {
            let sdk = match &self.sdk {
                Some(Ok(sdk)) => Arc::clone(sdk),
                Some(Err(refused)) => return Err(refused.clone()),
                None => unreachable!("DLSS runs only once prepared"),
            };
            if !gpu.ray_reconstruction {
                let info = gpu.adapter.get_info();
                return Err(Refused {
                    refusal: SettingRefusal::UnsupportedAdapter,
                    detail: format!(
                        "{} ({}) has no DLSS Ray Reconstruction",
                        info.name, info.driver
                    ),
                });
            }
            let mode = mode(render_scale);
            if self
                .reconstructor
                .as_ref()
                .is_none_or(|made| made.size != size || made.mode != mode)
            {
                // The old feature goes first: NGX releases it on an idle
                // device.
                self.reconstructor = None;
                let feature = DlssRayReconstruction::new(
                    [size.0, size.1],
                    mode,
                    DlssFeatureFlags::HighDynamicRange
                        | DlssFeatureFlags::LowResolutionMotionVectors
                        | DlssFeatureFlags::AlphaUpscaling,
                    DlssRayReconstructionRoughnessMode::Packed,
                    DlssRayReconstructionDepthMode::Hardware,
                    sdk,
                    &gpu.device,
                    &gpu.queue,
                )
                .map_err(|error| failed("Ray Reconstruction could not be made", error))?;
                let output = gpu
                    .device
                    .create_texture(&wgpu::TextureDescriptor {
                        label: Some("render-wgpu ray reconstruction"),
                        size: crate::target::extent(size.0, size.1),
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: crate::finish::HDR_FORMAT,
                        usage: wgpu::TextureUsages::STORAGE_BINDING
                            | wgpu::TextureUsages::TEXTURE_BINDING
                            | wgpu::TextureUsages::COPY_SRC,
                        view_formats: &[],
                    })
                    .create_view(&Default::default());
                self.reconstructor = Some(Reconstructor {
                    size,
                    mode,
                    feature,
                    output,
                });
                self.reconstruction_fresh = true;
            }
            Ok(self.reconstructor.as_mut().expect("made above"))
        }

        /// Denoise and upscale the world's HDR into `work.output`, finishing
        /// and submitting `encoder` with it. On failure nothing is submitted
        /// and the encoder comes back, the refusal staying.
        pub fn reconstruct(
            &mut self,
            gpu: &Gpu,
            timer: Option<&mut PassTimer>,
            mut encoder: wgpu::CommandEncoder,
            work: &Reconstruction<'_>,
        ) -> Result<(), (Refused, wgpu::CommandEncoder)> {
            if let Err(refused) = self.reconstructor(gpu, work.output_size, work.render_scale) {
                self.reconstruction_failed = Some(refused.clone());
                self.reconstructor = None;
                return Err((refused, encoder));
            }
            let Some(guides) = work.inputs.guides else {
                let refused = Refused {
                    refusal: SettingRefusal::InitializationFailed,
                    detail: "Ray Reconstruction ran without its guides".to_owned(),
                };
                return Err((refused, encoder));
            };
            let fresh = std::mem::replace(&mut self.reconstruction_fresh, false);
            // The stage's first stamp, before RR's own commands.
            if let Some(writes) = timer
                .as_deref()
                .and_then(|timer| timer.compute_writes_between(true, false))
            {
                encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("render-wgpu ray reconstruction begins"),
                    timestamp_writes: Some(writes),
                });
            }
            let responsivity = self
                .bias
                .get_or_insert_with(|| Input::bias(&gpu.device))
                .draw(
                    &gpu.device,
                    &mut encoder,
                    work.render,
                    &[work.inputs.motion],
                );
            let made = self.reconstructor.as_mut().expect("made above");
            let rendered = made.feature.render(
                DlssRayReconstructionRenderParameters {
                    diffuse_albedo: guides[0],
                    specular_albedo: guides[1],
                    normals: guides[2],
                    roughness: None,
                    color: work.color,
                    depth: work.depth,
                    motion_vectors: work.inputs.motion,
                    // Until rays trace, reflections move as their surfaces do.
                    specular_guide: match &work.specular {
                        Some(rays) => DlssRayReconstructionSpecularGuide::SpecularHitDistance {
                            texture_view: rays.hit_distance,
                            world_to_view_rows_array: rays.world_to_view,
                            view_to_clip_rows_array: rays.view_to_clip,
                        },
                        None => DlssRayReconstructionSpecularGuide::SpecularMotionVectors(
                            work.inputs.motion,
                        ),
                    },
                    transparency_overlay: None,
                    color_before_transparency: None,
                    screen_space_subsurface_scattering_guide: None,
                    depth_of_field_guide: None,
                    responsivity_mask: Some(responsivity),
                    alpha: None,
                    dlss_output_alpha: None,
                    dlss_output: &made.output,
                    reset: work.inputs.history_reset || fresh,
                    jitter_offset: work.inputs.jitter.into(),
                    partial_texture_size: Some([work.render.0, work.render.1]),
                    motion_vector_scale: Some(MOTION_SCALE),
                },
                &mut encoder,
                &gpu.adapter,
            );
            let commands = match rendered {
                Ok(commands) => commands,
                Err(error) => {
                    let refused = failed("Ray Reconstruction could not evaluate", error);
                    self.reconstruction_failed = Some(refused.clone());
                    self.reconstructor = None;
                    return Err((refused, encoder));
                }
            };
            let mut after = gpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("render-wgpu ray reconstruction output"),
                });
            after.copy_texture_to_texture(
                made.output.texture().as_image_copy(),
                work.output.texture().as_image_copy(),
                crate::target::extent(work.output_size.0, work.output_size.1),
            );
            if let Some(writes) = timer
                .as_deref()
                .and_then(|timer| timer.compute_writes_between(false, true))
            {
                after.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("render-wgpu ray reconstruction ends"),
                    timestamp_writes: Some(writes),
                });
            }
            if let Some(timer) = timer {
                timer.resolve(&mut after);
                gpu.queue
                    .submit([encoder.finish(), commands, after.finish()]);
                timer.submitted();
            } else {
                gpu.queue
                    .submit([encoder.finish(), commands, after.finish()]);
            }
            Ok(())
        }

        /// Evaluate DLSS over `picture` into `output`; on failure, nothing
        /// is drawn and the refusal stays.
        #[allow(clippy::too_many_arguments, reason = "one stage's inputs")]
        pub fn run(
            &mut self,
            gpu: &Gpu,
            sampler: &wgpu::Sampler,
            timer: Option<&mut PassTimer>,
            picture: &RenderPicture,
            inputs: &TemporalInputs<'_>,
            render_scale: f32,
            output: &TargetView<'_>,
        ) -> Result<(), Refused> {
            let result = self.evaluate(gpu, sampler, timer, picture, inputs, render_scale, output);
            if let Err(refused) = &result {
                self.failed = Some(refused.clone());
                self.feature = None;
            }
            result
        }

        /// The render resolution DLSS draws from for `output` at
        /// `render_scale`: `scaled`, held within the input sizes the named
        /// mode nearest the scale takes. Makes the feature for that output
        /// and mode; on failure the refusal stays.
        pub fn render_size(
            &mut self,
            gpu: &Gpu,
            output: (u32, u32),
            scaled: (u32, u32),
            render_scale: f32,
        ) -> Result<(u32, u32), Refused> {
            let result = self.feature(gpu, output, render_scale).map(|feature| {
                let range = feature.dlss.render_resolution_range();
                let (least, most) = (range.start(), range.end());
                (
                    scaled.0.clamp(least[0], most[0]),
                    scaled.1.clamp(least[1], most[1]),
                )
            });
            if let Err(refused) = &result {
                self.failed = Some(refused.clone());
                self.feature = None;
            }
            result
        }

        /// The feature for an output of `size` in the mode `render_scale`
        /// picks, made again when either changes.
        fn feature(
            &mut self,
            gpu: &Gpu,
            size: (u32, u32),
            render_scale: f32,
        ) -> Result<&mut Feature, Refused> {
            let sdk = match &self.sdk {
                Some(Ok(sdk)) => Arc::clone(sdk),
                Some(Err(refused)) => return Err(refused.clone()),
                None => unreachable!("DLSS runs only once prepared"),
            };
            let mode = mode(render_scale);
            if self
                .feature
                .as_ref()
                .is_none_or(|feature| feature.size != size || feature.mode != mode)
            {
                // The old feature goes first: NGX releases it on an idle
                // device.
                self.feature = None;
                let dlss = DlssSuperResolution::new(
                    [size.0, size.1],
                    mode,
                    DlssFeatureFlags::LowResolutionMotionVectors,
                    sdk,
                    &gpu.device,
                    &gpu.queue,
                )
                .map_err(|error| failed("DLSS could not be made", error))?;
                let output = gpu
                    .device
                    .create_texture(&wgpu::TextureDescriptor {
                        label: Some("render-wgpu upscale dlss"),
                        size: crate::target::extent(size.0, size.1),
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: OUTPUT_FORMAT,
                        usage: wgpu::TextureUsages::STORAGE_BINDING
                            | wgpu::TextureUsages::TEXTURE_BINDING,
                        view_formats: &[],
                    })
                    .create_view(&Default::default());
                self.feature = Some(Feature {
                    size,
                    mode,
                    dlss,
                    output,
                });
                self.fresh = true;
            }
            Ok(self.feature.as_mut().expect("made above"))
        }

        #[allow(clippy::too_many_arguments, reason = "one stage's inputs")]
        fn evaluate(
            &mut self,
            gpu: &Gpu,
            sampler: &wgpu::Sampler,
            timer: Option<&mut PassTimer>,
            picture: &RenderPicture,
            inputs: &TemporalInputs<'_>,
            render_scale: f32,
            output: &TargetView<'_>,
        ) -> Result<(), Refused> {
            self.feature(gpu, (output.width, output.height), render_scale)?;
            let fresh = std::mem::replace(&mut self.fresh, false);
            let feature = self.feature.as_mut().expect("made above");
            let render = picture.size();
            let mut encoder = gpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("render-wgpu upscale dlss"),
                });
            // The stage's first stamp, before DLSS's own commands.
            if let Some(writes) = timer
                .as_deref()
                .and_then(|timer| timer.compute_writes_between(true, false))
            {
                encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("render-wgpu upscale dlss begins"),
                    timestamp_writes: Some(writes),
                });
            }
            let depth = match picture.viewmodel_depth() {
                Some(viewmodel) => self
                    .depth
                    .get_or_insert_with(|| Input::depth(&gpu.device))
                    .draw(
                        &gpu.device,
                        &mut encoder,
                        render,
                        &[picture.depth(), viewmodel],
                    ),
                None => picture.depth(),
            };
            let bias = self
                .bias
                .get_or_insert_with(|| Input::bias(&gpu.device))
                .draw(&gpu.device, &mut encoder, render, &[inputs.motion]);
            let commands = feature
                .dlss
                .render(
                    DlssSuperResolutionRenderParameters {
                        color: picture.image(),
                        depth,
                        motion_vectors: inputs.motion,
                        exposure: DlssSuperResolutionExposure::Automatic,
                        bias: Some(bias),
                        dlss_output: &feature.output,
                        reset: inputs.history_reset || fresh,
                        // In render pixels, x right and y down, as the views
                        // took it.
                        jitter_offset: inputs.jitter.into(),
                        partial_texture_size: Some([render.0, render.1]),
                        motion_vector_scale: Some(MOTION_SCALE),
                    },
                    &mut encoder,
                    &gpu.adapter,
                )
                .map_err(|error| failed("DLSS could not evaluate", error))?;
            let drawing = self.output.get_or_insert_with(|| Output::new(&gpu.device));
            let group = drawing.bind_group(
                &gpu.device,
                sampler,
                &feature.output,
                picture.image(),
                inputs.motion,
            );
            let pipeline = drawing.pipeline(&gpu.device, output.format);
            let mut drawn = gpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("render-wgpu upscale dlss output"),
                });
            {
                let mut pass = drawn.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("render-wgpu upscale dlss output"),
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
                    timestamp_writes: timer
                        .as_deref()
                        .and_then(|timer| timer.render_writes_between(false, true)),
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_pipeline(pipeline);
                pass.set_bind_group(0, &group, &[]);
                pass.draw(0..3, 0..1);
            }
            if let Some(timer) = timer {
                timer.resolve(&mut drawn);
            }
            // DLSS's commands follow the encoder that moved its inputs into
            // place, in the same submission (`DlssSuperResolution::render`).
            gpu.queue
                .submit([encoder.finish(), commands, drawn.finish()]);
            Ok(())
        }
    }

    /// NGX on `gpu`, or why DLSS cannot draw there.
    fn start(gpu: &Gpu) -> Result<Arc<Mutex<DlssSdk>>, Refused> {
        let info = gpu.adapter.get_info();
        let unsupported = |detail: String| Refused {
            refusal: SettingRefusal::UnsupportedAdapter,
            detail,
        };
        if gpu.is_software() {
            return Err(Refused {
                refusal: SettingRefusal::SoftwareAdapter,
                detail: format!("{} is a software adapter", info.name),
            });
        }
        if info.backend != wgpu::Backend::Vulkan {
            return Err(unsupported(format!(
                "DLSS draws with Vulkan; {} draws with {:?}",
                info.name, info.backend
            )));
        }
        if info.vendor != NVIDIA {
            return Err(unsupported(format!("{} is not an NVIDIA GPU", info.name)));
        }
        if !gpu.dlss {
            return Err(unsupported(format!(
                "{} ({}) does not have DLSS's Vulkan extensions",
                info.name, info.driver
            )));
        }
        DlssSdk::new(PROJECT_ID, gpu.device.clone()).map_err(|error| match error {
            DlssError::FeatureNotSupported => unsupported(format!(
                "{} ({}) cannot run DLSS: it needs an NVIDIA RTX GPU",
                info.name, info.driver
            )),
            error => failed("NGX did not start", error),
        })
    }
}

#[cfg(all(test, not(feature = "dlss")))]
mod tests {
    use render_host_contracts::{
        RendererCameraPose, RendererCameraProjection, RendererCompositionCamera,
    };
    use render_model::{Antialiasing, RendererSettingsDescriptor};

    use super::Refused;
    use crate::{Gpu, OffscreenTarget, Renderer, RendererOptions, SettingRefusal, UpscaleMethod};

    /// The scene a Ray Reconstruction frame is compared on: a sky-coloured
    /// background behind a box.
    fn renderer(gpu: &Gpu, settings: RendererSettingsDescriptor) -> Renderer {
        let mut renderer = Renderer::new(gpu, RendererOptions::default().with_settings(&settings));
        let mut world = render_presentation::PresentationWorld::default();
        let delta = world
            .apply(render_model::RenderFrameDiff {
                publication: None,
                ops: vec![
                    render_model::RenderDiff::SetBackgroundColor {
                        color: [0.1, 0.3, 0.7, 1.0],
                    },
                    render_model::RenderDiff::Create {
                        handle: render_model::RenderHandle::new(1),
                        parent: None,
                        node: render_model::RenderNode::new(render_model::Geometry::Cube),
                    },
                ],
            })
            .expect("the fixture is admitted");
        let issues = renderer.apply(&delta, &crate::NoResources);
        assert!(issues.is_empty(), "{issues:?}");
        renderer
    }

    fn camera() -> RendererCompositionCamera {
        RendererCompositionCamera {
            id: "camera".to_owned(),
            pose: RendererCameraPose {
                position: [0.0, 0.0, 3.0],
                pitch_degrees: 0.0,
                yaw_degrees: 0.0,
            },
            basis: None,
            projection: RendererCameraProjection::Perspective {
                fov_y_degrees: 60.0,
                near: 0.1,
                far: 100.0,
            },
            motion: None,
            viewmodel_fov_y_degrees: 0.0,
        }
    }

    /// A frame whose Ray Reconstruction fails as it evaluates takes its whole
    /// path but RR itself: the world drawn at render resolution, the
    /// background at output size, the HDR scaled, the depth carried, finish
    /// at output size. It looks as a plain frame does, at either size, says
    /// it scaled, and the refusal stays.
    #[test]
    fn a_ray_reconstruction_frame_refused_as_it_runs_draws_the_world_scaled() {
        const SIZE: (u32, u32) = (128, 72);
        let gpu = Gpu::headless_with_ray_queries().expect("a wgpu adapter");
        if gpu.is_software() {
            return;
        }
        let plain = {
            let mut renderer = renderer(&gpu, RendererSettingsDescriptor::DEFAULT);
            let target = OffscreenTarget::new(&gpu, SIZE.0, SIZE.1, renderer.samples());
            renderer.render_offscreen(&camera(), &target);
            target.read_rgba(&gpu)
        };
        let distinct: std::collections::HashSet<&[u8]> = plain.chunks(4).collect();
        assert!(distinct.len() > 2, "the box stands out of the background");
        for render_scale in [1.0, 0.67] {
            let mut renderer = renderer(
                &gpu,
                RendererSettingsDescriptor {
                    antialiasing: Antialiasing::Dlss,
                    global_illumination: render_model::GlobalIllumination::RayTraced,
                    render_scale,
                    ..RendererSettingsDescriptor::DEFAULT
                },
            );
            let refused = Refused {
                refusal: SettingRefusal::InitializationFailed,
                detail: "injected".to_owned(),
            };
            renderer.upscale.dlss.failing = Some(refused.clone());
            let target = OffscreenTarget::new(&gpu, SIZE.0, SIZE.1, renderer.samples());
            // The refused frame scales the world; the next draws the world
            // as any TAAU frame does, its render picture left behind.
            for expected in [UpscaleMethod::Bilinear, UpscaleMethod::Taau] {
                let stats = renderer.render_offscreen(&camera(), &target);
                assert_eq!(stats.upscale, Some(expected), "{render_scale}");
                let picture = target.read_rgba(&gpu);
                let difference: u64 = picture
                    .iter()
                    .zip(&plain)
                    .map(|(a, b)| u64::from(a.abs_diff(*b)))
                    .sum();
                let mean = difference as f64 / picture.len() as f64;
                assert!(
                    mean < 4.0,
                    "scale {render_scale}, {expected:?}: differs by {mean}"
                );
            }
            assert_eq!(
                renderer
                    .gpu_readout()
                    .ray_reconstruction
                    .map(|readout| readout.refusal),
                Some(refused.refusal)
            );
            // A frame with no stage at all draws straight into its target.
            renderer.set_options(RendererOptions::default());
            let direct = OffscreenTarget::new(&gpu, SIZE.0, SIZE.1, renderer.samples());
            renderer.render_offscreen(&camera(), &direct);
            assert_eq!(direct.read_rgba(&gpu), plain, "scale {render_scale}");
        }
    }

    /// A frame DLSS takes and fails as it evaluates draws TAAU, and its
    /// statistics and the GPU readout name TAAU, while the settings readout
    /// refuses DLSS from then on.
    #[test]
    fn a_dlss_frame_refused_as_it_runs_reports_the_taau_that_drew() {
        let gpu = Gpu::headless().expect("a wgpu adapter");
        if gpu.is_software() {
            return;
        }
        let mut renderer = Renderer::new(
            &gpu,
            RendererOptions::default().with_settings(&RendererSettingsDescriptor {
                antialiasing: Antialiasing::Dlss,
                render_scale: 0.67,
                ..RendererSettingsDescriptor::DEFAULT
            }),
        );
        let refused = Refused {
            refusal: SettingRefusal::InitializationFailed,
            detail: "injected".to_owned(),
        };
        renderer.upscale.dlss.failing = Some(refused.clone());
        assert_eq!(renderer.settings_readout().antialiasing, None);
        let camera = RendererCompositionCamera {
            id: "camera".to_owned(),
            pose: RendererCameraPose {
                position: [0.0, 0.0, 3.0],
                pitch_degrees: 0.0,
                yaw_degrees: 0.0,
            },
            basis: None,
            projection: RendererCameraProjection::Perspective {
                fov_y_degrees: 60.0,
                near: 0.1,
                far: 100.0,
            },
            motion: None,
            viewmodel_fov_y_degrees: 0.0,
        };
        let target = OffscreenTarget::new(&gpu, 64, 36, renderer.samples());
        for frame in 0..2 {
            let stats = renderer.render_offscreen(&camera, &target);
            assert_eq!(stats.upscale, Some(UpscaleMethod::Taau), "frame {frame}");
            let readout = renderer.gpu_readout();
            assert_eq!(readout.upscale.method, Some(UpscaleMethod::Taau));
            assert_eq!(
                readout.dlss.map(|dlss| (dlss.refusal, dlss.detail)),
                Some((refused.refusal, refused.detail.clone()))
            );
            let settings = renderer.settings_readout();
            assert_eq!(settings.antialiasing, Some(refused.refusal));
            assert_eq!(settings.effective.antialiasing, Antialiasing::Temporal);
        }
    }
}
