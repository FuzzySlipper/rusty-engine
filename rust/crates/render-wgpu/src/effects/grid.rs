//! A parts rig drawn on a screen-pixel grid
//! (`PartsRigInstanceDescriptor::pixel_grid`).
//!
//! Each grid rig a view draws is first drawn smooth and lit, as it would be
//! in the world, into its own region of a scratch image the size of the view
//! (`encode_grid`, before the world pass): its colour premultiplied by its
//! coverage, at the world pass's samples, resolved to one sample a pixel.
//! Its region is its screen rectangle on its grid, which is anchored at the
//! projection of the rig's origin, so a block holds its place on the figure
//! however the camera or the rig moves. In the world pass the rig is one quad
//! on its own plane (raster depth is the plane's, so what stands in front
//! still hides it pixel by pixel), whose fragments average their block of
//! the scratch image and draw where it is covered enough
//! (`effects.wgsl` `fs_parts_rig_grid`). A rig whose rectangle does not fit
//! in what the view's earlier grid rigs left of the scratch image, or that
//! stands across the eye's plane, draws smooth, and the readout counts it.
//! The scratch image lives while grid rigs draw, and is dropped after a
//! frame that draws none.

use super::*;
use crate::finish::HDR_FORMAT;
use crate::timing::PassTimer;

/// A block quad's row: the rig's plane (model), the quad on it in rig units,
/// the grid (anchor in target pixels, block side, coverage cutoff), the
/// screen rectangle it covers in target pixels, and the shift from screen to
/// scratch image and its palette (first and count).
pub(super) const BLOCK_ROW_FLOATS: usize = 32;
/// Floats a palette colour takes in the palette buffer: its linear colour,
/// then its sRGB values, which blocks are matched by.
const PALETTE_COLOR_FLOATS: usize = 8;
/// The block quad is the rig's bounds widened by this many blocks on each
/// side, plus a pixel, in rig units at its size on screen, so its projection
/// covers every block a figure's edge reaches.
const QUAD_MARGIN_BLOCKS: f32 = 2.0;
/// A rig whose bounds reach this close to the eye's plane (clip w) draws
/// smooth: its projection is unbounded.
const NEAREST_W: f32 = 1e-4;

/// Where a grid rig draws in one view.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct GridPlacement {
    /// Block side in target pixels.
    pub side: u32,
    /// A block corner: the rig origin's projection, rounded to a pixel
    /// corner, in target pixels.
    pub anchor: [i32; 2],
    pub cutoff: f32,
    /// The screen rectangle its blocks cover, inside the viewport: x0, y0,
    /// x1, y1 in target pixels (x1, y1 exclusive).
    pub rect: [i32; 4],
    /// The rig's plane and the quad on it, in its (mirrored) rig units.
    pub model: Mat4,
    pub quad: [f32; 4],
    /// Its palette in the pass's palette buffer: first colour and count.
    pub palette: [u32; 2],
}

/// How a grid rig places in one view.
pub(super) enum Placed {
    /// Wholly outside the viewport: nothing to draw.
    Off,
    /// It stands across the eye's plane: drawn smooth.
    Smooth,
    Grid {
        anchor: [i32; 2],
        rect: [i32; 4],
        quad: [f32; 4],
    },
}

/// A rig of `bounds` (its pose's, in its mirrored rig units) on the plane
/// `model` places, as `camera` sees it in `viewport`, on a grid of blocks of
/// `side` target pixels.
pub(super) fn place(
    model: &Mat4,
    bounds: [f32; 4],
    camera: &CameraMatrices,
    viewport: PixelRect,
    side: u32,
) -> Placed {
    let size = glam::Vec2::new(viewport.width as f32, viewport.height as f32);
    let corner = glam::Vec2::new(viewport.x as f32, viewport.y as f32);
    let pixel = |point: Vec3| {
        let clip = camera.view_proj * point.extend(1.0);
        (clip.w > NEAREST_W).then(|| {
            let ndc = clip.truncate().truncate() / clip.w;
            corner + glam::Vec2::new(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5) * size
        })
    };
    let [x0, y0, x1, y1] = bounds;
    let corners = [[x0, y0], [x1, y0], [x1, y1], [x0, y1]]
        .map(|[x, y]| pixel(model.transform_point3(Vec3::new(x, y, 0.0))));
    let (Some(origin), [Some(a), Some(b), Some(c), Some(d)]) =
        (pixel(model.w_axis.truncate()), corners)
    else {
        return Placed::Smooth;
    };
    let low = a.min(b).min(c).min(d);
    let high = a.max(b).max(c).max(d);
    let anchor = origin.round();
    let step = side as f32;
    // Out to whole blocks, a pixel past the figure's edge, then in to the
    // viewport.
    let snap = |value: f32, anchor: f32, round: fn(f32) -> f32| {
        anchor + round((value - anchor) / step) * step
    };
    let rect = [
        snap(low.x - 1.0, anchor.x, f32::floor).max(corner.x),
        snap(low.y - 1.0, anchor.y, f32::floor).max(corner.y),
        snap(high.x + 1.0, anchor.x, f32::ceil).min(corner.x + size.x),
        snap(high.y + 1.0, anchor.y, f32::ceil).min(corner.y + size.y),
    ];
    if rect[2] <= rect[0] || rect[3] <= rect[1] {
        return Placed::Off;
    }
    // Rig units per pixel along each of its axes, at its size on screen.
    let across = (high - low).max(glam::Vec2::ONE);
    let per_pixel = ((x1 - x0) / across.x).max((y1 - y0) / across.y);
    let margin = (QUAD_MARGIN_BLOCKS * step + 1.0) * per_pixel;
    Placed::Grid {
        anchor: [anchor.x as i32, anchor.y as i32],
        rect: rect.map(|value| value as i32),
        quad: [x0 - margin, y0 - margin, x1 + margin, y1 + margin],
    }
}

/// What the grid rigs drew over the last frame.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PartsRigGridReadout {
    /// Grid rigs drawn as blocks, summed over the frame's views.
    pub drawn: u32,
    /// Grid rigs drawn smooth: their rectangle did not fit what the view's
    /// earlier grid rigs left of the scratch image, or they stood across the
    /// eye's plane.
    pub smooth: u32,
}

/// The scratch image grid rigs are drawn into: colour at the world pass's
/// samples, resolved, and a depth the rigs' pipelines draw with.
struct Scratch {
    samples: u32,
    width: u32,
    height: u32,
    color: Option<wgpu::TextureView>,
    resolve: wgpu::TextureView,
    depth: wgpu::TextureView,
    /// Its resolved image and the palette buffer, for the block quads.
    bind_group: wgpu::BindGroup,
    palette_bytes: u64,
}

pub(super) struct Grid {
    layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    scratch: Option<Scratch>,
    rows: Rows,
    palette: wgpu::Buffer,
    /// This pass's palettes: each colour's linear value then its sRGB
    /// values, padded to vec4s.
    pub palette_values: Vec<f32>,
    pub row_values: Vec<f32>,
    /// The scratch image's shelf packing in this pass: the next region's
    /// corner and the current shelf's height, in viewport pixels.
    shelf: (u32, u32, u32),
    viewport: PixelRect,
    timer: Option<PassTimer>,
    used: bool,
    counts: PartsRigGridReadout,
    pub last: PartsRigGridReadout,
}

fn palette_buffer(device: &wgpu::Device, bytes: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("render-wgpu parts rig palettes"),
        size: bytes,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

impl Grid {
    pub fn new(gpu: &Gpu, frame_layout: &wgpu::BindGroupLayout) -> Self {
        let device = &gpu.device;
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("render-wgpu parts rig grid"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 40,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 41,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("render-wgpu parts rig grid"),
            bind_group_layouts: &[Some(frame_layout), Some(&layout)],
            immediate_size: 0,
        });
        Self {
            layout,
            pipeline_layout,
            scratch: None,
            rows: Rows::new(device, "render-wgpu parts rig grid rows"),
            palette: palette_buffer(device, 256),
            palette_values: Vec::new(),
            row_values: Vec::new(),
            shelf: (0, 0, 0),
            viewport: PixelRect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
            timer: PassTimer::new(gpu, "parts-rig-grid"),
            used: false,
            counts: PartsRigGridReadout::default(),
            last: PartsRigGridReadout::default(),
        }
    }

    /// At the start of a frame: take the last frame's counts and timing,
    /// and drop the scratch image when the last frame drew no grid rig.
    pub fn begin_frame(&mut self, gpu: &Gpu) {
        if let Some(timer) = &mut self.timer {
            timer.collect(gpu);
        }
        self.last = std::mem::take(&mut self.counts);
        if !std::mem::take(&mut self.used) {
            self.scratch = None;
        }
    }

    /// At the start of a view pass's preparation.
    pub fn begin_pass(&mut self, viewport: PixelRect) {
        self.palette_values.clear();
        self.row_values.clear();
        self.shelf = (0, 0, 0);
        self.viewport = viewport;
    }

    pub fn smooth(&mut self) {
        self.counts.smooth += 1;
    }

    /// Keep `palette` for a rig this pass: its first colour and count.
    pub fn add_palette(&mut self, palette: &[[f32; 3]]) -> [u32; 2] {
        let first = (self.palette_values.len() / PALETTE_COLOR_FLOATS) as u32;
        for color in palette {
            let encoded = color.map(|value| linear_srgb(value.min(1.0)));
            self.palette_values.extend_from_slice(&[
                color[0], color[1], color[2], 0.0, encoded[0], encoded[1], encoded[2], 0.0,
            ]);
        }
        [first, palette.len() as u32]
    }

    /// A region of the scratch image for a rectangle `width` × `height`
    /// pixels: its corner in target pixels, or None when it does not fit
    /// what this pass's earlier rigs left.
    fn pack(&mut self, width: u32, height: u32) -> Option<[u32; 2]> {
        let (mut x, mut y, mut shelf) = self.shelf;
        if x + width > self.viewport.width {
            (x, y, shelf) = (0, y + shelf, 0);
        }
        if x + width > self.viewport.width || y + height > self.viewport.height {
            return None;
        }
        self.shelf = (x + width, y, shelf.max(height));
        Some([self.viewport.x + x, self.viewport.y + y])
    }

    /// Place `grid` in the scratch image: the row of its block quad, and the
    /// scratch region (x, y, width, height in target pixels) its pieces draw
    /// into. None when it does not fit; it then draws smooth.
    pub fn admit(&mut self, grid: &GridPlacement) -> Option<(u32, [u32; 4])> {
        let [x0, y0, x1, y1] = grid.rect;
        let (width, height) = ((x1 - x0) as u32, (y1 - y0) as u32);
        let Some([x, y]) = self.pack(width, height) else {
            self.counts.smooth += 1;
            return None;
        };
        self.counts.drawn += 1;
        self.used = true;
        let index = (self.row_values.len() / BLOCK_ROW_FLOATS) as u32;
        self.row_values
            .extend_from_slice(&grid.model.to_cols_array());
        self.row_values.extend_from_slice(&grid.quad);
        self.row_values.extend_from_slice(&[
            grid.anchor[0] as f32,
            grid.anchor[1] as f32,
            grid.side as f32,
            grid.cutoff,
        ]);
        self.row_values
            .extend_from_slice(&grid.rect.map(|value| value as f32));
        self.row_values.extend_from_slice(&[
            x as f32 - x0 as f32,
            y as f32 - y0 as f32,
            grid.palette[0] as f32,
            grid.palette[1] as f32,
        ]);
        Some((index, [x, y, width, height]))
    }

    /// Make the scratch image ready for a target of `width` × `height` at
    /// `samples`, and write this pass's rows and palettes.
    pub fn upload(&mut self, gpu: &Gpu, width: u32, height: u32, samples: u32) {
        self.rows.write(gpu, &self.row_values);
        let palette_bytes = (self.palette_values.len() * 4) as u64;
        let grown = palette_bytes > self.palette.size();
        if grown {
            self.palette = palette_buffer(&gpu.device, palette_bytes.next_power_of_two());
        }
        if !self.palette_values.is_empty() {
            gpu.queue
                .write_buffer(&self.palette, 0, bytemuck::cast_slice(&self.palette_values));
        }
        let fits = self.scratch.as_ref().is_some_and(|scratch| {
            scratch.samples == samples
                && scratch.width >= width
                && scratch.height >= height
                && scratch.palette_bytes == self.palette.size()
        });
        if fits {
            return;
        }
        let (width, height) = match &self.scratch {
            Some(scratch) if scratch.samples == samples => {
                (scratch.width.max(width), scratch.height.max(height))
            }
            _ => (width, height),
        };
        let device = &gpu.device;
        let texture = |label, format, samples, usage| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: samples,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        let resolve = texture(
            "render-wgpu parts rig grid",
            HDR_FORMAT,
            1,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let color = (samples > 1).then(|| {
            texture(
                "render-wgpu parts rig grid samples",
                HDR_FORMAT,
                samples,
                wgpu::TextureUsages::RENDER_ATTACHMENT,
            )
        });
        let depth = texture(
            "render-wgpu parts rig grid depth",
            DEPTH_FORMAT,
            samples,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        );
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("render-wgpu parts rig grid"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 40,
                    resource: wgpu::BindingResource::TextureView(&resolve),
                },
                wgpu::BindGroupEntry {
                    binding: 41,
                    resource: self.palette.as_entire_binding(),
                },
            ],
        });
        self.scratch = Some(Scratch {
            samples,
            width,
            height,
            color,
            resolve,
            depth,
            bind_group,
            palette_bytes: self.palette.size(),
        });
    }
}

/// sRGB-encoded value of a linear one in [0, 1].
pub(crate) fn linear_srgb(value: f32) -> f32 {
    if value <= 0.003_130_8 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    }
}

impl Effects {
    /// A block quad pipeline drawing into `format`, by the rig's depth
    /// policy.
    pub(super) fn ensure_grid_pipeline(
        &mut self,
        device: &wgpu::Device,
        format: ColorTarget,
        depth: (bool, bool),
    ) {
        let index = self.format_index(device, format);
        if self.formats[index].grids.contains_key(&depth) {
            return;
        }
        let corner = wgpu::VertexBufferLayout {
            array_stride: 8,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x2],
        };
        let instance = wgpu::vertex_attr_array![
            1 => Float32x4, 2 => Float32x4, 3 => Float32x4, 4 => Float32x4,
            5 => Float32x4, 6 => Float32x4, 7 => Float32x4, 8 => Float32x4
        ];
        let pipeline = self.pipeline(
            device,
            "render-wgpu parts rig grid",
            &self.grid.pipeline_layout,
            (
                "vs_parts_rig_grid",
                if format.motion {
                    "fs_parts_rig_grid_reactive"
                } else {
                    "fs_parts_rig_grid"
                },
            ),
            &[
                Some(corner),
                Some(wgpu::VertexBufferLayout {
                    array_stride: (BLOCK_ROW_FLOATS * 4) as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &instance,
                }),
            ],
            wgpu::PrimitiveTopology::TriangleStrip,
            None,
            format,
            Some(depth_state(depth.0, depth.1)),
            None,
            MotionWrite::Reactive,
        );
        self.formats[index].grids.insert(depth, pipeline);
    }

    /// Draw a grid rig's block quad (`index` its row) in the world pass.
    pub(super) fn draw_grid(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        set: &FormatPipelines,
        state: SpriteState,
        index: u32,
    ) {
        let (Some(pipeline), Some(scratch)) = (
            set.grids.get(&(state.depth_test, state.depth_write)),
            self.grid.scratch.as_ref(),
        ) else {
            return;
        };
        pass.set_pipeline(pipeline);
        pass.set_bind_group(1, &scratch.bind_group, &[]);
        pass.set_vertex_buffer(0, self.corners.slice(..));
        pass.set_vertex_buffer(1, self.grid.rows.buffer.slice(..));
        pass.draw(0..4, index..index + 1);
    }

    /// Draw this pass's grid rigs smooth into their regions of the scratch
    /// image, before the world pass: `frame` is the view's frame binding,
    /// `viewport` its viewport.
    pub fn encode_grid(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        frame: &wgpu::BindGroup,
        effects: &EffectsPass,
        viewport: PixelRect,
    ) {
        let Some(scratch) = self.grid.scratch.as_ref() else {
            return;
        };
        if effects.grid.is_empty() {
            return;
        }
        let format = ColorTarget::new(HDR_FORMAT, scratch.samples);
        let Some(set) = self.formats.iter().find(|set| set.format == format) else {
            return;
        };
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render-wgpu parts rig grid"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: scratch.color.as_ref().unwrap_or(&scratch.resolve),
                    depth_slice: None,
                    resolve_target: scratch.color.as_ref().map(|_| &scratch.resolve),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: if scratch.color.is_some() {
                            wgpu::StoreOp::Discard
                        } else {
                            wgpu::StoreOp::Store
                        },
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &scratch.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: self.grid.timer.as_ref().and_then(PassTimer::render_writes),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            // The projection the world pass draws with: each rig shifted
            // into its region by its row, and kept there.
            pass.set_viewport(
                viewport.x as f32,
                viewport.y as f32,
                viewport.width as f32,
                viewport.height as f32,
                0.0,
                1.0,
            );
            pass.set_bind_group(0, frame, &[]);
            for (sprite, [x, y, width, height]) in &effects.grid {
                pass.set_scissor_rect(*x, *y, *width, *height);
                if let Some(key) = sprite.rig {
                    self.draw_rig(&mut pass, set, sprite, key, false);
                }
            }
        }
        if let Some(timer) = &mut self.grid.timer {
            timer.resolve(encoder);
        }
    }

    /// After the encoder holding `encode_grid`'s pass was submitted.
    pub fn grid_submitted(&mut self) {
        if let Some(timer) = &mut self.grid.timer {
            timer.submitted();
        }
    }

    pub fn grid_timing(&self) -> crate::timing::GpuPassTiming {
        self.grid.timer.as_ref().map_or_else(
            || crate::timing::untimed("parts-rig-grid"),
            PassTimer::readout,
        )
    }
}
