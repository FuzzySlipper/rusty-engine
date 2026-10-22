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
//! (`dlss_depth.wgsl`). The motion target holds how far each point moved in
//! render pixels; DLSS takes the way back, so its scale is -1. `NO_MOTION`
//! (water) reads to DLSS as a point come from far off
//! screen, so there it takes the current frame, as TAAU leans on it. The
//! picture is finished and tone-mapped, so DLSS runs on low dynamic range
//! with no exposure. It writes a storage texture at output size (a surface
//! cannot be one), which is then drawn into the output; the `upscale` pass
//! times both.
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

#[cfg(not(feature = "dlss"))]
pub(crate) use unbuilt::Dlss;

#[cfg(not(feature = "dlss"))]
mod unbuilt {
    use super::Refused;
    use crate::compose::Compose;
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
            _compose: &mut Compose,
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

    use dlss_wgpu::super_resolution::{
        DlssSuperResolution, DlssSuperResolutionExposure, DlssSuperResolutionRenderParameters,
    };
    use dlss_wgpu::{DlssError, DlssFeatureFlags, DlssPerfQualityMode, DlssSdk};

    use super::{Refused, NVIDIA, PROJECT_ID};
    use crate::compose::Compose;
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

    /// DLSS's depth: the world's and the viewmodel's in one
    /// (`dlss_depth.wgsl`), at render resolution.
    struct Depth {
        layout: wgpu::BindGroupLayout,
        pipeline: wgpu::RenderPipeline,
        /// The last render size's target.
        target: Option<((u32, u32), wgpu::TextureView)>,
    }

    impl Depth {
        const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R32Float;

        fn new(device: &wgpu::Device) -> Self {
            let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("render-wgpu dlss depth"),
                source: wgpu::ShaderSource::Wgsl(include_str!("dlss_depth.wgsl").into()),
            });
            let depth = |binding| wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Depth,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            };
            let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("render-wgpu dlss depth"),
                entries: &[depth(0), depth(1)],
            });
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("render-wgpu dlss depth"),
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 0,
            });
            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("render-wgpu dlss depth"),
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
                    entry_point: Some("fs_depth"),
                    compilation_options: Default::default(),
                    targets: &[Some(Self::FORMAT.into())],
                }),
                multiview_mask: None,
                cache: None,
            });
            Self {
                layout,
                pipeline,
                target: None,
            }
        }

        /// Write `world` with `viewmodel` over it into the target for
        /// `size`, in `encoder`: the depth DLSS reads.
        fn combine(
            &mut self,
            device: &wgpu::Device,
            encoder: &mut wgpu::CommandEncoder,
            size: (u32, u32),
            world: &wgpu::TextureView,
            viewmodel: &wgpu::TextureView,
        ) -> &wgpu::TextureView {
            if self.target.as_ref().is_none_or(|(made, _)| *made != size) {
                let view = device
                    .create_texture(&wgpu::TextureDescriptor {
                        label: Some("render-wgpu dlss depth"),
                        size: crate::target::extent(size.0, size.1),
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: Self::FORMAT,
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                            | wgpu::TextureUsages::TEXTURE_BINDING,
                        view_formats: &[],
                    })
                    .create_view(&Default::default());
                self.target = Some((size, view));
            }
            let target = &self.target.as_ref().expect("made above").1;
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("render-wgpu dlss depth"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(world),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(viewmodel),
                    },
                ],
            });
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render-wgpu dlss depth"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
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

    /// The feature for one output size and mode, and what it writes.
    struct Feature {
        size: (u32, u32),
        mode: DlssPerfQualityMode,
        dlss: DlssSuperResolution,
        output: wgpu::TextureView,
    }

    pub(crate) struct Dlss {
        /// The device's NGX, from the first time DLSS is asked for, or why
        /// it refused.
        sdk: Option<Result<Arc<Mutex<DlssSdk>>, Refused>>,
        /// Why the last feature could not be made or evaluated.
        failed: Option<Refused>,
        feature: Option<Feature>,
        /// The depth pass, from the first frame with a viewmodel depth.
        depth: Option<Depth>,
        /// The next frame starts the history again.
        fresh: bool,
    }

    impl Dlss {
        pub fn new() -> Self {
            Self {
                sdk: None,
                failed: None,
                feature: None,
                depth: None,
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
        }

        /// Evaluate DLSS over `picture` into `output`; on failure, nothing
        /// is drawn and the refusal stays.
        #[allow(clippy::too_many_arguments, reason = "one stage's inputs")]
        pub fn run(
            &mut self,
            gpu: &Gpu,
            compose: &mut Compose,
            sampler: &wgpu::Sampler,
            timer: Option<&mut PassTimer>,
            picture: &RenderPicture,
            inputs: &TemporalInputs<'_>,
            render_scale: f32,
            output: &TargetView<'_>,
        ) -> Result<(), Refused> {
            let result = self.evaluate(
                gpu,
                compose,
                sampler,
                timer,
                picture,
                inputs,
                render_scale,
                output,
            );
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
            compose: &mut Compose,
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
                    .get_or_insert_with(|| Depth::new(&gpu.device))
                    .combine(
                        &gpu.device,
                        &mut encoder,
                        render,
                        picture.depth(),
                        viewmodel,
                    ),
                None => picture.depth(),
            };
            let commands = feature
                .dlss
                .render(
                    DlssSuperResolutionRenderParameters {
                        color: picture.image(),
                        depth,
                        motion_vectors: inputs.motion,
                        exposure: DlssSuperResolutionExposure::Automatic,
                        bias: None,
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
            compose.prepare_blit(&gpu.device, output.format);
            let group = compose.blit_bind_group(&gpu.device, &feature.output, sampler);
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
                compose.blit(&mut pass, output.format, &group);
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
