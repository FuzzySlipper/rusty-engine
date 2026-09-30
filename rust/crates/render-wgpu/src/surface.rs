//! Window surface presentation for the desktop shell. The shell owns the
//! window; this module owns the wgpu surface configuration and depth buffer.

use std::sync::Arc;

use crate::{target, Gpu, GpuError};

/// A configured presentation surface for one window.
pub struct WindowSurface {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    /// The format the scene is drawn in: the surface format when it is sRGB,
    /// else its sRGB view (a WebGPU canvas offers no sRGB format, #8874).
    scene_format: wgpu::TextureFormat,
    /// The window is a primary destination: passes draw multisampled and
    /// resolve into the swapchain image.
    multisampled: wgpu::TextureView,
    depth_view: wgpu::TextureView,
}

/// Why a frame was not presented. The caller renders again next tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentSkip {
    /// The surface was lost or outdated; it has been reconfigured.
    Reconfigured,
    /// The swapchain timed out or the window is occluded.
    Unavailable,
}

impl WindowSurface {
    /// A surface for `window` on a device made by [`Gpu::for_display`] for
    /// the window's display.
    pub fn create<W>(gpu: &Gpu, window: Arc<W>, width: u32, height: u32) -> Result<Self, GpuError>
    where
        W: wgpu::rwh::HasWindowHandle
            + wgpu::rwh::HasDisplayHandle
            + std::fmt::Debug
            + Send
            + Sync
            + 'static,
    {
        let surface = gpu
            .instance
            .create_surface(window)
            .map_err(|error| GpuError::Surface(error.to_string()))?;
        Self::new(gpu, surface, width, height)
    }

    pub(crate) fn new(
        gpu: &Gpu,
        surface: wgpu::Surface<'static>,
        width: u32,
        height: u32,
    ) -> Result<Self, GpuError> {
        let capabilities = surface.get_capabilities(&gpu.adapter);
        // Prefer an sRGB format so shading output is encoded as offscreen.
        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .or_else(|| capabilities.formats.first().copied())
            .ok_or_else(|| GpuError::Surface("surface reports no formats".to_owned()))?;
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: width.max(1),
            height: height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode: capabilities
                .alpha_modes
                .first()
                .copied()
                .unwrap_or(wgpu::CompositeAlphaMode::Auto),
            // Overlays blend in gamma space through the non-sRGB view, as a
            // browser composites its page. A non-sRGB surface draws its scene
            // through the sRGB view instead.
            view_formats: if format.is_srgb() {
                vec![format.remove_srgb_suffix()]
            } else if format.add_srgb_suffix() != format {
                vec![format.add_srgb_suffix()]
            } else {
                Vec::new()
            },
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        surface.configure(&gpu.device, &config);
        let scene_format = format.add_srgb_suffix();
        Ok(Self {
            multisampled: target::multisampled_color(
                gpu,
                config.width,
                config.height,
                scene_format,
            ),
            depth_view: target::multisampled_depth(
                gpu,
                config.width,
                config.height,
                target::PRIMARY_SAMPLES,
            ),
            surface,
            config,
            scene_format,
        })
    }

    pub fn size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    pub fn resize(&mut self, gpu: &Gpu, width: u32, height: u32) {
        if (width.max(1), height.max(1)) == self.size() {
            return;
        }
        self.config.width = width.max(1);
        self.config.height = height.max(1);
        self.reconfigure(gpu);
    }

    fn reconfigure(&mut self, gpu: &Gpu) {
        self.surface.configure(&gpu.device, &self.config);
        let (width, height) = (self.config.width, self.config.height);
        self.multisampled = target::multisampled_color(gpu, width, height, self.scene_format);
        self.depth_view = target::multisampled_depth(gpu, width, height, target::PRIMARY_SAMPLES);
    }

    /// Acquire the next swapchain image, let `draw` encode into it, and
    /// present it.
    pub(crate) fn present_with(
        &mut self,
        gpu: &Gpu,
        draw: impl FnOnce(target::TargetView<'_>),
    ) -> Result<(), PresentSkip> {
        self.present_layers(gpu, |view, _| draw(view))
    }

    /// As [`Self::present_with`], and `draw` also gets the finished image as
    /// a single-sample, non-sRGB view, to encode overlays over it in gamma
    /// space after the scene is resolved.
    pub(crate) fn present_layers(
        &mut self,
        gpu: &Gpu,
        draw: impl FnOnce(target::TargetView<'_>, target::TargetView<'_>),
    ) -> Result<(), PresentSkip> {
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.reconfigure(gpu);
                return Err(PresentSkip::Reconfigured);
            }
            _ => return Err(PresentSkip::Unavailable),
        };
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(self.scene_format),
            ..Default::default()
        });
        let (color, resolve) = if target::PRIMARY_SAMPLES > 1 {
            (&self.multisampled, Some(&view))
        } else {
            (&view, None)
        };
        let gamma = self.config.format.remove_srgb_suffix();
        let gamma_view = frame.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(gamma),
            ..Default::default()
        });
        draw(
            target::TargetView {
                color,
                resolve,
                depth: &self.depth_view,
                format: self.scene_format,
                samples: target::PRIMARY_SAMPLES,
                width: self.config.width,
                height: self.config.height,
            },
            target::TargetView {
                color: &gamma_view,
                resolve: None,
                depth: &self.depth_view,
                format: gamma,
                samples: 1,
                width: self.config.width,
                height: self.config.height,
            },
        );
        gpu.queue.present(frame);
        Ok(())
    }
}
