//! Captures: a frozen retained frame rendered off the live view.
//!
//! The capture interface, shared by output images (`RenderOutput.CaptureImage`)
//! and ghost plate captures (#8788):
//! - [`Renderer::isolated`] builds a renderer over its own tables from a
//!   complete frozen frame, sharing the device and host options. The live
//!   tables, composition and GPU rows are untouched.
//! - [`Renderer::capture`] renders that renderer's tables from one camera into
//!   new colour and depth textures of any size and colour format. Both are
//!   sampleable, so a family pass (a linear-depth resolve, coverage, a
//!   normal pass) can read them after.
//!
//! [`Renderer::capture_image`] is the output job: capture into linear half
//! float, then resolve, un-premultiply, expose or tone map, encode sRGB, read
//! back and write PNG. GLB export stays with the browser output executor.

use render_host_contracts::{RenderOutputJob, RenderOutputOperation, RendererCameraProjection};
use render_model::RenderFrameDiff;

use crate::camera::{self, CameraPose};
use crate::compose::Conversion;
use crate::frame::{PassStart, PixelRect, ViewLayer, ViewPass};
use crate::tables::Environment;
use crate::target::{TargetView, DEPTH_FORMAT};
use crate::{encode_png, ApplyIssue, OffscreenTarget, Renderer, ResourceSource};

/// Linear capture format: exposure and tone mapping run after the capture.
const LINEAR_CAPTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// What a capture clears to.
#[derive(Clone, Copy)]
pub(crate) enum CaptureBackground {
    /// A straight-alpha linear RGBA colour; alpha 0 leaves uncovered pixels
    /// transparent.
    Clear([f32; 4]),
    /// The retained background colour or sky.
    Environment,
}

pub(crate) struct CaptureRequest {
    pub pose: CameraPose,
    pub projection: RendererCameraProjection,
    pub width: u32,
    pub height: u32,
    pub format: wgpu::TextureFormat,
    pub background: CaptureBackground,
    /// Draw the viewmodel layer after a depth break, as a primary view does.
    pub viewmodel: bool,
}

pub(crate) struct Capture {
    pub color: wgpu::Texture,
    /// Depth32Float, cleared to 1 where nothing drew.
    #[allow(dead_code, reason = "read by ghost plate captures (#8788)")]
    pub depth: wgpu::Texture,
}

impl Renderer {
    /// A renderer holding only `frame`, a complete frozen frame, with this
    /// renderer's device and options. Returns the ops it could not realize.
    pub(crate) fn isolated(
        &self,
        frame: &RenderFrameDiff,
        resources: &dyn ResourceSource,
    ) -> (Renderer, Vec<ApplyIssue>) {
        let mut isolated = Renderer::new(&self.gpu, self.options);
        let issues = isolated.apply(frame, resources);
        (isolated, issues)
    }

    /// Render the tables from the request's camera into new textures.
    pub(crate) fn capture(&mut self, request: &CaptureRequest) -> Capture {
        let texture = |label, format, usage| {
            self.gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: crate::target::extent(request.width, request.height),
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | usage,
                view_formats: &[],
            })
        };
        let color = texture(
            "render-wgpu capture colour",
            request.format,
            wgpu::TextureUsages::COPY_SRC,
        );
        let depth = texture(
            "render-wgpu capture depth",
            DEPTH_FORMAT,
            wgpu::TextureUsages::empty(),
        );
        let color_view = color.create_view(&Default::default());
        let depth_view = depth.create_view(&Default::default());
        let target = TargetView {
            color: &color_view,
            depth: &depth_view,
            format: request.format,
            width: request.width,
            height: request.height,
        };
        self.prepare();
        let area = PixelRect::whole(request.width, request.height);
        // The capture holds premultiplied colour: blending over the clear then
        // yields premultiplied pixels, which the conversion un-premultiplies.
        let (clear, sky) = match request.background {
            CaptureBackground::Clear([r, g, b, a]) => ([r * a, g * a, b * a, a], false),
            CaptureBackground::Environment => (self.environment_clear(), true),
        };
        self.encode_view(ViewPass {
            target,
            viewport: area,
            camera: camera::camera_matrices(request.pose, &request.projection, area.aspect()),
            layer: ViewLayer::World,
            start: PassStart::Target,
            clear,
            sky,
        });
        if request.viewmodel {
            self.encode_view(ViewPass {
                target,
                viewport: area,
                camera: camera::viewmodel_matrices(&request.projection, area.aspect()),
                layer: ViewLayer::Viewmodel,
                start: PassStart::Target,
                clear,
                sky: false,
            });
        }
        Capture { color, depth }
    }

    /// Execute an image output job: a top-to-bottom, straight-alpha sRGB RGBA8
    /// PNG, or the diagnostic that failed the job. The job's frame is the
    /// frozen source subtree; nothing live changes.
    ///
    /// A sample count above 1 is realized as ordered-grid supersampling
    /// (`ceil(sqrt(samples))`² texels per pixel, resolved in linear light).
    pub fn capture_image(
        &self,
        job: &RenderOutputJob,
        resources: &dyn ResourceSource,
    ) -> Result<Vec<u8>, String> {
        let RenderOutputOperation::Image {
            camera,
            width,
            height,
            background,
            use_camera_background,
            exposure,
            aces_filmic,
            samples,
            pose,
        } = &job.operation
        else {
            return Err("capture: GLB export stays with the browser output executor".to_owned());
        };
        let factor = (f64::from((*samples).max(1))).sqrt().ceil() as u32;
        let limit = self.gpu.device.limits().max_texture_dimension_2d;
        let (capture_width, capture_height) = (
            width.checked_mul(factor).unwrap_or(u32::MAX),
            height.checked_mul(factor).unwrap_or(u32::MAX),
        );
        if *width == 0 || *height == 0 || capture_width > limit || capture_height > limit {
            return Err(format!(
                "capture: {width}x{height} with {samples} samples ({factor}x{factor} supersampling) exceeds the device texture limit {limit}"
            ));
        }
        let (mut isolated, issues) = self.isolated(&job.frame, resources);
        if let Some(issue) = issues.first() {
            return Err(format!(
                "capture: the frozen scene's {} op is not realized: {}",
                issue.op, issue.detail
            ));
        }
        // A pose job samples one animated instance of the frozen scene.
        if let Some(pose) = pose {
            isolated
                .set_animated_playback(
                    pose.handle,
                    &render_model::AnimatedMeshPlaybackCommand::Sample {
                        clip: pose.clip.clone(),
                        normalized_time: pose.normalized_time as f32,
                    },
                )
                .map_err(|detail| format!("capture: pose sample: {detail}"))?;
        }
        let use_environment =
            *use_camera_background && !matches!(isolated.tables.environment, Environment::Default);
        let capture = isolated.capture(&CaptureRequest {
            pose: camera::descriptor_pose(camera),
            projection: camera.projection,
            width: capture_width,
            height: capture_height,
            format: LINEAR_CAPTURE_FORMAT,
            background: if use_environment {
                CaptureBackground::Environment
            } else {
                CaptureBackground::Clear(*background)
            },
            viewmodel: true,
        });
        let output = OffscreenTarget::new(&self.gpu, *width, *height);
        let source = capture.color.create_view(&Default::default());
        isolated.compose.convert(
            &self.gpu,
            &source,
            output.view().color,
            Conversion {
                factor,
                exposure: *exposure,
                aces: *aces_filmic,
            },
        );
        encode_png(*width, *height, &output.read_rgba(&self.gpu))
    }
}
