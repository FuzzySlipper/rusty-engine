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
//! The stage reads the picture's colour and depth at render resolution. One
//! method draws today, bilinear. At render scale 1 with no method selected
//! and no image effect, there is no picture and no stage: the views draw
//! straight into the output.

use crate::compose::Compose;
use crate::target::{RenderPicture, TargetView};
use crate::timing::{untimed, GpuPassTiming, PassTimer};
use crate::{Gpu, RendererOptions};

const PASS: &str = "upscale";

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

/// How the stage brings the picture to output resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UpscaleMethod {
    /// Each output pixel samples the picture bilinearly.
    Bilinear,
}

impl UpscaleMethod {
    /// The method `options` select, or `None` to draw straight into the
    /// output.
    ///
    /// A render scale below 1 selects bilinear. A temporal method (TAAU,
    /// DLSS) will be a further value of the `antialiasing` setting beside
    /// its MSAA sample counts, selected at any render scale. One setting
    /// holding both keeps a temporal method exclusive with MSAA by its
    /// shape, with no refusal between two settings.
    pub fn selected(options: &RendererOptions) -> Option<Self> {
        (options.render_scale < 1.0).then_some(Self::Bilinear)
    }
}

pub(crate) struct Upscale {
    /// Bilinear's sampler.
    sampler: wgpu::Sampler,
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
            timer: PassTimer::new(gpu, PASS),
        }
    }

    /// Bring `picture` to `output` by `method`, over the whole output.
    pub fn run(
        &mut self,
        gpu: &Gpu,
        compose: &mut Compose,
        method: UpscaleMethod,
        picture: &RenderPicture,
        output: &TargetView<'_>,
    ) {
        // Bilinear is the only method so far.
        let UpscaleMethod::Bilinear = method;
        compose.prepare_blit(&gpu.device, output.format);
        let group = compose.blit_bind_group(&gpu.device, picture.image(), &self.sampler);
        if let Some(timer) = &mut self.timer {
            timer.collect(gpu);
        }
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
        if let Some(timer) = &mut self.timer {
            timer.submitted();
        }
    }

    /// The stage's timing, for `gpu.passes`.
    pub fn timing(&self) -> GpuPassTiming {
        self.timer
            .as_ref()
            .map_or_else(|| untimed(PASS), PassTimer::readout)
    }
}
