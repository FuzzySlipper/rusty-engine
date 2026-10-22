//! Render targets: the offscreen colour and depth target with RGBA readback,
//! and the view the pass pipeline encodes into.

use crate::Gpu;

pub(crate) const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// Offscreen colour is sRGB-encoded RGBA8, the byte layout readback returns.
pub(crate) const OFFSCREEN_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// Samples per pixel of the world and its depth at the primary destination
/// (the offscreen primary target and the window surface) until the renderer
/// says otherwise (`Renderer::samples`). Offscreen render targets and
/// captures are single-sample.
pub(crate) const PRIMARY_SAMPLES: u32 = 4;

/// What a pipeline must match to draw into a target: colour format, sample
/// count, and whether a motion target follows the colour (`motion.rs`).
/// Pipeline caches are keyed by it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ColorTarget {
    pub format: wgpu::TextureFormat,
    pub samples: u32,
    pub motion: bool,
}

impl ColorTarget {
    /// A target of `format` at `samples`, without a motion target.
    pub fn new(format: wgpu::TextureFormat, samples: u32) -> Self {
        Self {
            format,
            samples,
            motion: false,
        }
    }

    pub fn multisample(self) -> wgpu::MultisampleState {
        wgpu::MultisampleState {
            count: self.samples,
            ..Default::default()
        }
    }

    /// The colour targets a pipeline drawing here declares: `color`, then
    /// the motion target written as `motion` says, when there is one.
    pub fn targets(
        self,
        color: wgpu::ColorTargetState,
        motion: crate::motion::MotionWrite,
    ) -> Vec<Option<wgpu::ColorTargetState>> {
        let mut targets = vec![Some(color)];
        if self.motion {
            targets.push(Some(motion.target_state()));
        }
        targets
    }
}

/// What one render call draws into: the single-sample image every pass
/// writes, and the depth the world draws with, `samples` per pixel (as its
/// HDR world target has). A world view's finish pass averages the samples
/// into `color`.
#[derive(Clone, Copy)]
pub(crate) struct TargetView<'a> {
    pub color: &'a wgpu::TextureView,
    pub depth: &'a wgpu::TextureView,
    pub format: wgpu::TextureFormat,
    pub samples: u32,
    pub width: u32,
    pub height: u32,
}

impl TargetView<'_> {
    /// What a pipeline drawing into `color` must match.
    pub fn key(&self) -> ColorTarget {
        ColorTarget::new(self.format, 1)
    }
}

pub(crate) fn depth_texture(gpu: &Gpu, width: u32, height: u32) -> wgpu::TextureView {
    multisampled_depth(gpu, width, height, 1)
}

pub(crate) fn multisampled_depth(
    gpu: &Gpu,
    width: u32,
    height: u32,
    samples: u32,
) -> wgpu::TextureView {
    gpu.device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("render-wgpu depth"),
            size: extent(width, height),
            mip_level_count: 1,
            sample_count: samples,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            // Labels sample the scene's depth at their anchors.
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

pub(crate) fn extent(width: u32, height: u32) -> wgpu::Extent3d {
    wgpu::Extent3d {
        width: width.max(1),
        height: height.max(1),
        depth_or_array_layers: 1,
    }
}

/// Bytes in one readback row: copies align rows to 256 bytes.
fn padded_row(width: u32) -> u32 {
    (width * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT
}

pub(crate) fn fit_to_device(gpu: &Gpu, width: u32, height: u32) -> (u32, u32) {
    let limits = gpu.device.limits();
    fit_to_limits(
        width,
        height,
        limits.max_texture_dimension_2d,
        limits.max_buffer_size,
    )
}

/// The largest size of this aspect within the device's limits: each side
/// within its texture limit and the padded readback within its buffer limit.
/// A size that fits is unchanged.
fn fit_to_limits(width: u32, height: u32, max_side: u32, max_buffer: u64) -> (u32, u32) {
    let (width, height) = (width.max(1), height.max(1));
    let tight_bytes = 4.0 * f64::from(width) * f64::from(height);
    let scale = (f64::from(max_side) / f64::from(width.max(height)))
        .min((max_buffer as f64 / tight_bytes).sqrt())
        .min(1.0);
    let fitted = |side: u32| ((f64::from(side) * scale) as u32).clamp(1, max_side);
    let width = fitted(width);
    // Row padding is outside the tight estimate; the height gives it room.
    let rows = u32::try_from(max_buffer / u64::from(padded_row(width))).unwrap_or(u32::MAX);
    (width, fitted(height).min(rows.max(1)))
}

fn fitted_pixel_ratio(ratio: f32, requested_width: u32, fitted_width: u32) -> f32 {
    let requested_width = requested_width.max(1);
    if fitted_width == requested_width {
        ratio
    } else {
        (f64::from(ratio) * f64::from(fitted_width) / f64::from(requested_width)) as f32
    }
}

/// An offscreen colour and depth target of one size, with a readback buffer.
/// It is a primary destination: the world draws with the depth's samples
/// and finishes into the single-sample colour that readback copies.
pub struct OffscreenTarget {
    color: wgpu::Texture,
    color_view: wgpu::TextureView,
    depth_view: wgpu::TextureView,
    readback: wgpu::Buffer,
    padded_row: u32,
    width: u32,
    height: u32,
    samples: u32,
}

impl OffscreenTarget {
    /// A target of this size with `samples` per pixel
    /// (`Renderer::samples`), made smaller, keeping its aspect, when the
    /// device cannot draw or read back that size ([`Self::size`] says).
    pub fn new(gpu: &Gpu, width: u32, height: u32, samples: u32) -> Self {
        let (width, height) = fit_to_device(gpu, width, height);
        let color = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("render-wgpu offscreen colour"),
            size: extent(width, height),
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: OFFSCREEN_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let padded_row = padded_row(width);
        let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("render-wgpu readback"),
            size: u64::from(padded_row) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        Self {
            color_view: color.create_view(&Default::default()),
            depth_view: multisampled_depth(gpu, width, height, samples),
            color,
            readback,
            padded_row,
            width,
            height,
            samples,
        }
    }

    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Target pixels per CSS pixel for a view of `requested_width` pixels
    /// at `ratio` (`Renderer::set_pixel_ratio`) drawn into this target:
    /// `ratio` itself, unless the target was made smaller, when it shrinks
    /// with it. Authored CSS sizes then keep their share of the frame, and
    /// their size when the viewer shows it at the size it asked for.
    pub fn fitted_pixel_ratio(&self, requested_width: u32, ratio: f32) -> f32 {
        fitted_pixel_ratio(ratio, requested_width, self.width)
    }

    /// Reallocate for a new size or sample count. The next render fills the
    /// new target.
    pub fn resize(&mut self, gpu: &Gpu, width: u32, height: u32, samples: u32) {
        let (width, height) = fit_to_device(gpu, width, height);
        if (width, height, samples) != (self.width, self.height, self.samples) {
            *self = Self::new(gpu, width, height, samples);
        }
    }

    pub(crate) fn view(&self) -> TargetView<'_> {
        TargetView {
            color: &self.color_view,
            depth: &self.depth_view,
            format: OFFSCREEN_FORMAT,
            samples: self.samples,
            width: self.width,
            height: self.height,
        }
    }

    /// The colour readback copies.
    pub(crate) fn color(&self) -> &wgpu::TextureView {
        &self.color_view
    }

    /// Copy the last rendered frame to CPU memory as tightly packed,
    /// sRGB-encoded RGBA8 rows, top row first. Blocks until the GPU finishes.
    pub fn read_rgba(&self, gpu: &Gpu) -> Vec<u8> {
        let mut pixels = Vec::new();
        self.read_rgba_into(gpu, &mut pixels);
        pixels
    }

    /// As [`Self::read_rgba`], reusing the caller's buffer across frames.
    pub fn read_rgba_into(&self, gpu: &Gpu, pixels: &mut Vec<u8>) {
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render-wgpu readback"),
            });
        encoder.copy_texture_to_buffer(
            self.color.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &self.readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.padded_row),
                    rows_per_image: Some(self.height),
                },
            },
            self.color.size(),
        );
        gpu.queue.submit([encoder.finish()]);
        let slice = self.readback.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        // A lost device or failed map leaves the previous contents; readback
        // has no partial result to report.
        let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
        pixels.clear();
        if !matches!(receiver.recv(), Ok(Ok(()))) {
            return;
        }
        let row = (self.width * 4) as usize;
        pixels.reserve(row * self.height as usize);
        if let Ok(mapped) = slice.get_mapped_range() {
            for padded in mapped.chunks(self.padded_row as usize) {
                pixels.extend_from_slice(&padded[..row]);
            }
        }
        self.readback.unmap();
    }
}

/// The primary picture at render resolution (`upscale.rs`), which the
/// primary views draw into while an upscale stage or an image effect runs:
/// the single-sample image every pass writes and the depth at the primary's
/// sample count. Under a temporal stage the viewmodel passes draw against a
/// depth of their own, so the world's depth survives the frame for the
/// stage's motion dilation.
pub(crate) struct RenderPicture {
    image: wgpu::TextureView,
    depth: wgpu::TextureView,
    viewmodel_depth: Option<wgpu::TextureView>,
    width: u32,
    height: u32,
    samples: u32,
}

impl RenderPicture {
    pub fn new(gpu: &Gpu, width: u32, height: u32, samples: u32, temporal: bool) -> Self {
        let (width, height) = (width.max(1), height.max(1));
        let image = gpu
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("render-wgpu render picture"),
                size: extent(width, height),
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: OFFSCREEN_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&Default::default());
        Self {
            depth: multisampled_depth(gpu, width, height, samples),
            viewmodel_depth: temporal.then(|| multisampled_depth(gpu, width, height, samples)),
            image,
            width,
            height,
            samples,
        }
    }

    pub fn matches(&self, width: u32, height: u32, samples: u32, temporal: bool) -> bool {
        (self.width, self.height, self.samples) == (width.max(1), height.max(1), samples)
            && self.viewmodel_depth.is_some() == temporal
    }

    /// The picture and its depth, for the upscale stage or an image effect.
    pub fn image(&self) -> &wgpu::TextureView {
        &self.image
    }

    pub fn depth(&self) -> &wgpu::TextureView {
        &self.depth
    }

    /// The viewmodel passes' own depth, under a temporal stage.
    pub fn viewmodel_depth(&self) -> Option<&wgpu::TextureView> {
        self.viewmodel_depth.as_ref()
    }

    pub fn samples(&self) -> u32 {
        self.samples
    }

    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    pub fn view(&self) -> TargetView<'_> {
        TargetView {
            color: &self.image,
            depth: &self.depth,
            format: OFFSCREEN_FORMAT,
            samples: self.samples,
            width: self.width,
            height: self.height,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_size_beyond_the_device_keeps_its_aspect_within_both_limits() {
        // wgpu's default limits: 8192 per side, a 256 MiB buffer.
        let (side, buffer) = (8192, 256 << 20);
        assert_eq!(fit_to_limits(5120, 2880, side, buffer), (5120, 2880));
        assert_eq!(fit_to_limits(10240, 5760, side, buffer), (8192, 4608));
        assert_eq!(fit_to_limits(0, 0, side, buffer), (1, 1));
        // A 16384 texture limit leaves the readback buffer as the bound.
        let (width, height) = fit_to_limits(16384, 16384, 16384, buffer);
        assert!(u64::from(padded_row(width)) * u64::from(height) <= buffer);
        assert!(
            width.abs_diff(height) <= 1 && width > 8000,
            "{width}x{height}"
        );
        let fitted = fit_to_limits(16384, 16384, 16384, buffer);
        assert_eq!(fit_to_limits(fitted.0, fitted.1, 16384, buffer), fitted);
    }

    #[test]
    fn a_fitted_target_draws_authored_css_sizes_at_the_viewers_size() {
        // A 5K display at device pixel ratio 2 on an 8192-side device.
        let (side, buffer) = (8192, 256 << 20);
        let (css_width, ratio) = (5120.0_f32, 2.0_f32);
        let (width, _) = fit_to_limits(10240, 5760, side, buffer);
        assert_eq!(width, 8192);
        let fitted = fitted_pixel_ratio(ratio, 10240, width);
        assert_eq!(fitted, 1.6);
        // A 20 CSS pixel sprite covers 32 target pixels, which the viewer
        // stretches from 8192 to its 5120 CSS pixels: 20 again.
        let sprite = 20.0 * fitted;
        assert_eq!(sprite * css_width / width as f32, 20.0);
        // A size that fits keeps the viewer's exact ratio.
        for ratio in [1.0, 1.25, 1.5, 2.0, 2.625] {
            assert_eq!(fitted_pixel_ratio(ratio, 3000, 3000), ratio);
        }
    }
}
