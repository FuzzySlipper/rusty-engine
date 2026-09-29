//! Window surface presentation for the desktop shell. The shell owns the
//! window; this module owns the wgpu surface configuration and depth buffer.

use crate::{target, Gpu, GpuError};

/// A configured presentation surface for one window.
pub struct WindowSurface {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
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
            view_formats: Vec::new(),
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        surface.configure(&gpu.device, &config);
        Ok(Self {
            depth_view: target::depth_texture(gpu, config.width, config.height),
            surface,
            config,
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
        self.depth_view = target::depth_texture(gpu, self.config.width, self.config.height);
    }

    /// Acquire the next swapchain image, let `draw` encode into it, and
    /// present it.
    pub(crate) fn present_with(
        &mut self,
        gpu: &Gpu,
        draw: impl FnOnce(target::TargetView<'_>),
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
        let view = frame.texture.create_view(&Default::default());
        draw(target::TargetView {
            color: &view,
            depth: &self.depth_view,
            format: self.config.format,
            width: self.config.width,
            height: self.config.height,
        });
        gpu.queue.present(frame);
        Ok(())
    }
}
