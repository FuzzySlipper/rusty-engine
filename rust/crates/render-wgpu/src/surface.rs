//! Window surface presentation for the desktop shell. The shell owns the
//! window; this module owns the wgpu surface configuration and depth buffer.

use std::sync::Arc;

use crate::{target, Gpu, GpuError};

/// A configured presentation surface for one window.
pub struct WindowSurface {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    /// The window is a primary destination: the world draws with this
    /// depth's samples and finishes into the swapchain image.
    depth_view: wgpu::TextureView,
    samples: u32,
    /// The sample count and vsync the renderer asked for, applied by the
    /// next acquire (a surface is not reconfigured under an acquired image).
    wanted: Option<(u32, bool)>,
    /// The present modes the surface supports, which say whether a request
    /// for no vsync is realized ([`WindowSurface::vsync_only`]).
    present_modes: Vec<wgpu::PresentMode>,
}

/// A swapchain image acquired for one frame: acquire it, draw into it
/// ([`crate::Renderer::render_view_composition_to_frame`]), then present it.
pub struct SurfaceFrame {
    texture: wgpu::SurfaceTexture,
    view: wgpu::TextureView,
    /// The same image as a non-sRGB view, for overlays blended in gamma
    /// space.
    gamma_view: wgpu::TextureView,
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
            // browser composites its page.
            view_formats: if format.is_srgb() {
                vec![format.remove_srgb_suffix()]
            } else {
                Vec::new()
            },
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        surface.configure(&gpu.device, &config);
        let samples = target::PRIMARY_SAMPLES;
        Ok(Self {
            depth_view: target::multisampled_depth(gpu, config.width, config.height, samples),
            surface,
            config,
            samples,
            wanted: None,
            present_modes: capabilities.present_modes,
        })
    }

    /// What the renderer wants of the window (`Renderer::samples`,
    /// `Renderer::vsync`); the next acquire configures it.
    pub(crate) fn request(&mut self, samples: u32, vsync: bool) {
        if samples != self.samples || vsync != self.vsync() {
            self.wanted = Some((samples, vsync));
        }
    }

    fn vsync(&self) -> bool {
        !matches!(
            self.config.present_mode,
            wgpu::PresentMode::AutoNoVsync
                | wgpu::PresentMode::Immediate
                | wgpu::PresentMode::Mailbox
        )
    }

    /// The display can present only in step with its refresh: the surface
    /// has no immediate or mailbox mode, so `AutoNoVsync` falls back to FIFO
    /// and a request for no vsync changes nothing.
    pub fn vsync_only(&self) -> bool {
        Self::vsync_only_among(&self.present_modes)
    }

    /// Whether `modes`, a surface's supported present modes, hold none that
    /// presents without waiting for the display. wgpu resolves `AutoNoVsync`
    /// through `Immediate`, then `Mailbox`, then `Fifo`.
    pub fn vsync_only_among(modes: &[wgpu::PresentMode]) -> bool {
        !modes.iter().any(|mode| {
            matches!(
                mode,
                wgpu::PresentMode::Immediate | wgpu::PresentMode::Mailbox
            )
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
        self.depth_view = target::multisampled_depth(gpu, width, height, self.samples);
    }

    /// Acquire the next swapchain image. Under vsync this waits until the
    /// display frees one, so take it before anything a frame shares with
    /// other threads.
    pub fn acquire(&mut self, gpu: &Gpu) -> Result<SurfaceFrame, PresentSkip> {
        if let Some((samples, vsync)) = self.wanted.take() {
            self.config.present_mode = if vsync {
                wgpu::PresentMode::AutoVsync
            } else {
                wgpu::PresentMode::AutoNoVsync
            };
            self.samples = samples;
            self.reconfigure(gpu);
        }
        let texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture)
            | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.reconfigure(gpu);
                return Err(PresentSkip::Reconfigured);
            }
            _ => return Err(PresentSkip::Unavailable),
        };
        let view = texture.texture.create_view(&Default::default());
        let gamma_view = texture.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(self.config.format.remove_srgb_suffix()),
            ..Default::default()
        });
        Ok(SurfaceFrame {
            texture,
            view,
            gamma_view,
        })
    }

    /// Present a frame drawn into `frame`.
    pub fn present(&self, gpu: &Gpu, frame: SurfaceFrame) {
        gpu.queue.present(frame.texture);
    }

    /// The views to draw `frame` through: the scene's (the image, with the
    /// world's multisampled depth), and the image as a non-sRGB view, to
    /// encode overlays over the finished scene in gamma space.
    pub(crate) fn views<'a>(
        &'a self,
        frame: &'a SurfaceFrame,
    ) -> (target::TargetView<'a>, target::TargetView<'a>) {
        (
            target::TargetView {
                color: &frame.view,
                depth: &self.depth_view,
                format: self.config.format,
                samples: self.samples,
                width: self.config.width,
                height: self.config.height,
            },
            target::TargetView {
                color: &frame.gamma_view,
                depth: &self.depth_view,
                format: self.config.format.remove_srgb_suffix(),
                samples: self.samples,
                width: self.config.width,
                height: self.config.height,
            },
        )
    }

    /// Acquire the next swapchain image, let `draw` encode into it, and
    /// present it.
    pub(crate) fn present_with(
        &mut self,
        gpu: &Gpu,
        draw: impl FnOnce(target::TargetView<'_>),
    ) -> Result<(), PresentSkip> {
        let frame = self.acquire(gpu)?;
        draw(self.views(&frame).0);
        self.present(gpu, frame);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::WindowSurface;
    use wgpu::PresentMode;

    #[test]
    fn a_surface_with_only_fifo_modes_cannot_turn_vsync_off() {
        assert!(WindowSurface::vsync_only_among(&[PresentMode::Fifo]));
        assert!(WindowSurface::vsync_only_among(&[
            PresentMode::Fifo,
            PresentMode::FifoRelaxed
        ]));
        assert!(!WindowSurface::vsync_only_among(&[
            PresentMode::Fifo,
            PresentMode::Mailbox
        ]));
        assert!(!WindowSurface::vsync_only_among(&[
            PresentMode::Immediate,
            PresentMode::Fifo
        ]));
        assert!(WindowSurface::vsync_only_among(&[]));
    }
}
