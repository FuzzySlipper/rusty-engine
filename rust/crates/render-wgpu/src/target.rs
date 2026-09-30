//! Render targets: the offscreen colour and depth target with RGBA readback,
//! and the view the pass pipeline encodes into.

use crate::Gpu;

pub(crate) const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// Offscreen colour is sRGB-encoded RGBA8, the byte layout readback returns.
pub(crate) const OFFSCREEN_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// Samples per pixel of the primary destination (the offscreen primary
/// target and the window surface). Offscreen render targets and captures
/// are single-sample.
pub(crate) const PRIMARY_SAMPLES: u32 = 4;

/// What a pipeline must match to draw into a target: colour format and
/// sample count. Pipeline caches are keyed by it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ColorTarget {
    pub format: wgpu::TextureFormat,
    pub samples: u32,
}

impl ColorTarget {
    pub fn multisample(self) -> wgpu::MultisampleState {
        wgpu::MultisampleState {
            count: self.samples,
            ..Default::default()
        }
    }
}

/// What one render call draws into. With `samples > 1`, `color` and `depth`
/// are multisampled and every pass resolves `color` into `resolve`.
#[derive(Clone, Copy)]
pub(crate) struct TargetView<'a> {
    pub color: &'a wgpu::TextureView,
    pub resolve: Option<&'a wgpu::TextureView>,
    pub depth: &'a wgpu::TextureView,
    pub format: wgpu::TextureFormat,
    pub samples: u32,
    pub width: u32,
    pub height: u32,
}

impl TargetView<'_> {
    pub fn key(&self) -> ColorTarget {
        ColorTarget {
            format: self.format,
            samples: self.samples,
        }
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

/// A multisampled colour attachment that passes resolve from.
pub(crate) fn multisampled_color(
    gpu: &Gpu,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
) -> wgpu::TextureView {
    gpu.device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("render-wgpu multisampled colour"),
            size: extent(width, height),
            mip_level_count: 1,
            sample_count: PRIMARY_SAMPLES,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
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

/// An offscreen colour and depth target of one size, with a readback buffer.
/// It is a primary destination: passes draw multisampled and resolve into
/// the single-sample colour that readback copies.
pub struct OffscreenTarget {
    color: wgpu::Texture,
    color_view: wgpu::TextureView,
    multisampled: wgpu::TextureView,
    depth_view: wgpu::TextureView,
    readback: wgpu::Buffer,
    padded_row: u32,
    width: u32,
    height: u32,
}

impl OffscreenTarget {
    pub fn new(gpu: &Gpu, width: u32, height: u32) -> Self {
        let (width, height) = (width.max(1), height.max(1));
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
        let padded_row = (width * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("render-wgpu readback"),
            size: u64::from(padded_row) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        Self {
            color_view: color.create_view(&Default::default()),
            multisampled: multisampled_color(gpu, width, height, OFFSCREEN_FORMAT),
            depth_view: multisampled_depth(gpu, width, height, PRIMARY_SAMPLES),
            color,
            readback,
            padded_row,
            width,
            height,
        }
    }

    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Reallocate for a new size. The next render fills the new target.
    pub fn resize(&mut self, gpu: &Gpu, width: u32, height: u32) {
        if (width.max(1), height.max(1)) != (self.width, self.height) {
            *self = Self::new(gpu, width, height);
        }
    }

    pub(crate) fn view(&self) -> TargetView<'_> {
        let (color, resolve) = if PRIMARY_SAMPLES > 1 {
            (&self.multisampled, Some(&self.color_view))
        } else {
            (&self.color_view, None)
        };
        TargetView {
            color,
            resolve,
            depth: &self.depth_view,
            format: OFFSCREEN_FORMAT,
            samples: PRIMARY_SAMPLES,
            width: self.width,
            height: self.height,
        }
    }

    /// The single-sample colour readback copies: the resolve target of
    /// primary passes, or a plain output a conversion pass writes directly.
    pub(crate) fn resolved_color(&self) -> &wgpu::TextureView {
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
