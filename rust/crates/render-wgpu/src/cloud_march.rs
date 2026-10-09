//! The volumetric clouds at reduced resolution, filtered over frames
//! (`sky.wgsl` `fs_clouds_march`, `fs_clouds_composite`). Each world view
//! (by camera and viewport) keeps a target half its size each way and a
//! history of it: every frame the clouds are marched into the target with a
//! step offset that moves each frame, blended with each pixel's cloud as
//! the previous frame saw it (reprojected by the camera's move and the
//! clouds' drift), copied out as the next history, and drawn over the
//! view's background, upscaled. A view seen for the first time, a new size,
//! or a camera that jumped starts afresh: that frame marches with no
//! history.

use glam::{Mat4, Vec3};
use render_model::VolumetricCloudsQuality;

use crate::camera::CameraMatrices;
use crate::frame::PixelRect;
use crate::Gpu;

pub(crate) const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
/// The reduced target's size: the view's divided by this, each way.
const REDUCTION: u32 = 2;
/// `CloudMarchParams`: a matrix and two vectors.
const PARAMS_BYTES: u64 = 64 + 16 + 16;
/// The share of a pixel's new march blended into its history each frame,
/// by quality.
const WEIGHT_LOW: f32 = 0.2;
const WEIGHT_HIGH: f32 = 0.1;
/// A camera that moves farther than this in one frame has cut, metres.
const CUT_DISTANCE: f32 = 50.0;
/// A view's history unused for this many cloudy views is dropped.
const HISTORY_KEPT: u64 = 240;
/// The step phase's increment each frame (the golden ratio's fraction), so
/// successive offsets spread evenly.
const PHASE_STEP: f32 = 0.618_034;

/// One view's reduced clouds and their history.
struct History {
    camera: Option<String>,
    viewport: PixelRect,
    size: [u32; 2],
    current: wgpu::Texture,
    current_view: wgpu::TextureView,
    history: wgpu::Texture,
    params: wgpu::Buffer,
    march_group: wgpu::BindGroup,
    composite_group: wgpu::BindGroup,
    view_proj: Mat4,
    eye: Vec3,
    time: f64,
    frames: u32,
    used: u64,
}

pub(crate) struct CloudMarch {
    sampler: wgpu::Sampler,
    histories: Vec<History>,
    views_drawn: u64,
}

/// What a view's composite needs: its reduced target's bind group.
pub(crate) struct CloudFrame {
    pub index: usize,
}

impl CloudMarch {
    pub fn new(gpu: &Gpu) -> Self {
        Self {
            sampler: gpu.device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("render-wgpu cloud march"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                ..Default::default()
            }),
            histories: Vec::new(),
            views_drawn: 0,
        }
    }

    /// Ready the view's reduced target and write its march parameters;
    /// the march and composite passes then draw with `target_view` and
    /// the bind groups.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare(
        &mut self,
        gpu: &Gpu,
        march_layout: &wgpu::BindGroupLayout,
        composite_layout: &wgpu::BindGroupLayout,
        camera: Option<&str>,
        viewport: PixelRect,
        matrices: &CameraMatrices,
        quality: VolumetricCloudsQuality,
        time: f64,
    ) -> CloudFrame {
        self.views_drawn += 1;
        let drawn = self.views_drawn;
        self.histories
            .retain(|history| drawn - history.used <= HISTORY_KEPT);
        let size = [
            viewport.width.div_ceil(REDUCTION).max(1),
            viewport.height.div_ceil(REDUCTION).max(1),
        ];
        let found = self.histories.iter().position(|history| {
            history.camera.as_deref() == camera && history.viewport == viewport
        });
        let index = match found {
            Some(index) if self.histories[index].size == size => index,
            other => {
                if let Some(index) = other {
                    self.histories.remove(index);
                }
                let history =
                    self.make(gpu, march_layout, composite_layout, camera, viewport, size);
                self.histories.push(history);
                self.histories.len() - 1
            }
        };
        let history = &mut self.histories[index];
        history.used = drawn;
        if history.eye.distance(matrices.eye) > CUT_DISTANCE || time < history.time {
            history.frames = 0;
        }
        let weight = match (history.frames, quality) {
            (0, _) => 0.0,
            (_, VolumetricCloudsQuality::High) => WEIGHT_HIGH,
            _ => WEIGHT_LOW,
        };
        let phase = if history.frames == 0 {
            0.0
        } else {
            (history.frames as f32 * PHASE_STEP).fract()
        };
        let elapsed = (time - history.time).max(0.0) as f32;
        let values: Vec<f32> = history
            .view_proj
            .to_cols_array()
            .into_iter()
            .chain([history.eye.x, history.eye.y, history.eye.z, elapsed])
            .chain([weight, phase, size[0] as f32, size[1] as f32])
            .collect();
        gpu.queue
            .write_buffer(&history.params, 0, bytemuck::cast_slice(&values));
        CloudFrame { index }
    }

    fn make(
        &self,
        gpu: &Gpu,
        march_layout: &wgpu::BindGroupLayout,
        composite_layout: &wgpu::BindGroupLayout,
        camera: Option<&str>,
        viewport: PixelRect,
        size: [u32; 2],
    ) -> History {
        let device = &gpu.device;
        let texture = |label, usage| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: size[0],
                    height: size[1],
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: FORMAT,
                usage,
                view_formats: &[],
            })
        };
        let current = texture(
            "render-wgpu cloud march current",
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
        );
        let history = texture(
            "render-wgpu cloud march history",
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        );
        let current_view = current.create_view(&wgpu::TextureViewDescriptor::default());
        let history_view = history.create_view(&wgpu::TextureViewDescriptor::default());
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("render-wgpu cloud march params"),
            size: PARAMS_BYTES,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let march_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("render-wgpu cloud march"),
            layout: march_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&history_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        let composite_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("render-wgpu cloud composite"),
            layout: composite_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&current_view),
                },
            ],
        });
        History {
            camera: camera.map(str::to_owned),
            viewport,
            size,
            current,
            current_view,
            history,
            params,
            march_group,
            composite_group,
            view_proj: Mat4::IDENTITY,
            eye: Vec3::ZERO,
            time: 0.0,
            frames: 0,
            used: 0,
        }
    }

    pub fn target_view(&self, frame: &CloudFrame) -> &wgpu::TextureView {
        &self.histories[frame.index].current_view
    }

    pub fn march_group(&self, frame: &CloudFrame) -> &wgpu::BindGroup {
        &self.histories[frame.index].march_group
    }

    pub fn composite_group(&self, frame: &CloudFrame) -> &wgpu::BindGroup {
        &self.histories[frame.index].composite_group
    }

    /// After the march, in its encoder: keep this frame's clouds as the
    /// next history, and where the camera stood.
    pub fn keep(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        frame: &CloudFrame,
        matrices: &CameraMatrices,
        time: f64,
    ) {
        let history = &mut self.histories[frame.index];
        encoder.copy_texture_to_texture(
            history.current.as_image_copy(),
            history.history.as_image_copy(),
            wgpu::Extent3d {
                width: history.size[0],
                height: history.size[1],
                depth_or_array_layers: 1,
            },
        );
        history.view_proj = matrices.view_proj;
        history.eye = matrices.eye;
        history.time = time;
        history.frames = history.frames.saturating_add(1);
    }
}
