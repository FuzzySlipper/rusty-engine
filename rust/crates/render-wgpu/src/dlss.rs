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
//! the history reset. The motion target holds how far each point moved in
//! render pixels; DLSS takes the way back, so its scale is -1. `NO_MOTION`
//! (the sky, clouds and water) reads to DLSS as a point come from far off
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

    pub(crate) struct Dlss;

    impl Dlss {
        pub fn new() -> Self {
            Self
        }

        pub fn prepare(&mut self, _gpu: &Gpu) {}

        pub fn refusal(&self) -> Option<Refused> {
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
            _scaled: (u32, u32),
            _render_scale: f32,
        ) -> Result<(u32, u32), Refused> {
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
        /// The next frame starts the history again.
        fresh: bool,
    }

    impl Dlss {
        pub fn new() -> Self {
            Self {
                sdk: None,
                failed: None,
                feature: None,
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
            let commands = feature
                .dlss
                .render(
                    DlssSuperResolutionRenderParameters {
                        color: picture.image(),
                        depth: picture.depth(),
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
