//! Clustered forward shading (`light_clusters.wgsl`), a compute candidate
//! (#5938) on the compute pass foundation: before a world view pass, a
//! dispatch bins the pass's light rows into a view-frustum cluster grid, and
//! `rusty::lighting` reads a fragment's cluster list (and the global list of
//! unbounded lights) instead of looping over every light of the pass.
//!
//! The host turns it on (`RendererOptions::clustered_lighting`); a device
//! without compute shaders keeps the loop, and so does the viewmodel pass,
//! whose lights are few and camera-local. The grid is 16×9 tiles by 24 depth
//! slices; a cluster past its capacity keeps the first lights and the readout
//! counts the overflow.

use std::sync::mpsc::{Receiver, TryRecvError};

use glam::Mat4;

use crate::camera::CameraMatrices;
use crate::frame::LightRange;
use crate::timing::{GpuPassTiming, PassTimer};
use crate::Gpu;

/// Tiles across, tiles down, depth slices.
pub(crate) const GRID: [u32; 3] = [16, 9, 24];
/// Words per cluster: the count, then light row indices.
pub(crate) const CLUSTER_STRIDE: u32 = 64;
/// Lights a cluster, or the global list, can name.
pub(crate) const CLUSTER_CAPACITY: u32 = CLUSTER_STRIDE - 1;
const CLUSTERS: u32 = GRID[0] * GRID[1] * GRID[2];
/// The clusters, then the global list.
const ENTRIES: u32 = CLUSTERS + 1;
const WORKGROUP: u32 = 64;
/// `ClusterParams`: two matrices and three vectors.
const PARAMS_BYTES: u64 = 64 + 64 + 16 + 16 + 16;
/// `stats`: overflowed clusters, binned lights, global lights, spare.
const STATS_BYTES: u64 = 16;
const PASS: &str = "light-clusters";

/// The light clustering of the last world view, for diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LightClusterReadout {
    /// The last world view's lighting read its clusters.
    pub enabled: bool,
    /// Why this device cannot cluster; `None` while it can.
    pub refused: Option<String>,
    /// Why the last world view looped although clustering is on: more
    /// unbounded lights than the global list names. `None` while it
    /// clustered or clustering is off.
    pub fallback: Option<String>,
    /// Tiles across, tiles down, depth slices.
    pub grid: [u32; 3],
    /// Words each cluster holds: the count and up to `stride - 1` lights.
    pub cluster_stride: u32,
    /// Light rows the last binning placed in clusters, counted per cluster.
    pub binned_lights: u32,
    /// Lights in the global list: ambient, hemisphere, directional, and
    /// point or spot lights without a range.
    pub global_lights: u32,
    /// Clusters that had more lights than they hold.
    pub overflowed_clusters: u32,
}

pub(crate) struct LightClusters {
    pipeline: Option<wgpu::ComputePipeline>,
    refused: Option<String>,
    bind_group: Option<wgpu::BindGroup>,
    layout: wgpu::BindGroupLayout,
    params: wgpu::Buffer,
    /// Bound at `rusty::view` binding 7 for the lighting module.
    pub clusters: wgpu::Buffer,
    stats: wgpu::Buffer,
    stats_readback: wgpu::Buffer,
    pending_stats: Option<Receiver<Result<(), wgpu::BufferAsyncError>>>,
    timer: Option<PassTimer>,
    enabled: bool,
    fallback: Option<String>,
    binned_lights: u32,
    global_lights: u32,
    overflowed_clusters: u32,
}

/// What a world view's frame uniform says about its clusters: the grid and
/// a flag, then near, far, ln(far / near) and an orthographic flag.
pub(crate) struct ClusterUniform {
    pub grid: [u32; 4],
    pub depth: [f32; 4],
}

impl ClusterUniform {
    /// A pass whose lighting loops over its light rows.
    pub const LOOP: Self = Self {
        grid: [0; 4],
        depth: [0.0; 4],
    };
}

/// The near and far planes of a projection and whether it is orthographic.
fn near_far(projection: &Mat4) -> (f32, f32, bool) {
    let cols = projection.to_cols_array_2d();
    // In f64: the perspective far plane comes from `1 + m22`, which is
    // nearly zero.
    let (m22, m32) = (f64::from(cols[2][2]), f64::from(cols[3][2]));
    if cols[3][3] == 1.0 {
        // glam orthographic_rh: m22 = 1 / (near - far), m32 = near * m22.
        let near = m32 / m22;
        (near as f32, (near - 1.0 / m22) as f32, true)
    } else {
        // glam perspective_rh: m22 = far / (near - far), m32 = near * m22.
        let near = m32 / m22;
        (near as f32, (m22 * near / (m22 + 1.0)) as f32, false)
    }
}

impl LightClusters {
    pub fn new(gpu: &Gpu, shader: wgpu::ShaderModule) -> Self {
        let device = &gpu.device;
        let storage = |binding, read_only| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("render-wgpu light clusters"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                storage(1, true),
                storage(2, false),
                storage(3, false),
            ],
        });
        let refused = gpu.compute_refusal([WORKGROUP, 1, 1], 0);
        let pipeline = refused.is_none().then(|| {
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("render-wgpu light clusters"),
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 0,
            });
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("render-wgpu light clusters"),
                layout: Some(&pipeline_layout),
                module: &shader,
                entry_point: Some("cs_assign"),
                compilation_options: Default::default(),
                cache: None,
            })
        });
        let buffer = |label, size, usage| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage,
                mapped_at_creation: false,
            })
        };
        Self {
            pipeline,
            refused,
            bind_group: None,
            layout,
            params: buffer(
                "render-wgpu light cluster params",
                PARAMS_BYTES,
                wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            ),
            clusters: buffer(
                "render-wgpu light clusters",
                u64::from(ENTRIES * CLUSTER_STRIDE) * 4,
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            ),
            stats: buffer(
                "render-wgpu light cluster stats",
                STATS_BYTES,
                wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_SRC
                    | wgpu::BufferUsages::COPY_DST,
            ),
            stats_readback: buffer(
                "render-wgpu light cluster stats readback",
                STATS_BYTES,
                wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            ),
            pending_stats: None,
            timer: PassTimer::new(gpu, PASS),
            enabled: false,
            fallback: None,
            binned_lights: 0,
            global_lights: 0,
            overflowed_clusters: 0,
        }
    }

    /// The lights buffer was replaced: bind again before the next dispatch.
    pub fn invalidate(&mut self) {
        self.bind_group = None;
    }

    /// Record that a pass drew without clusters.
    pub fn skipped(&mut self) {
        self.enabled = false;
        self.fallback = None;
    }

    /// Bin `lights` for a view of `camera` into the clusters, in `encoder`
    /// before the view's render pass. Returns what the view's frame uniform
    /// says about its clusters.
    pub fn encode(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        lights_buffer: &wgpu::Buffer,
        camera: &CameraMatrices,
        lights: LightRange,
    ) -> ClusterUniform {
        if self.pipeline.is_none() {
            self.enabled = false;
            return ClusterUniform::LOOP;
        }
        self.collect(gpu);
        if let Some(timer) = &mut self.timer {
            timer.collect(gpu);
        }
        let pipeline = self.pipeline.as_ref().expect("checked above");
        let device = &gpu.device;
        let bind_group = self.bind_group.get_or_insert_with(|| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("render-wgpu light clusters"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.params.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: lights_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: self.clusters.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: self.stats.as_entire_binding(),
                    },
                ],
            })
        });
        let (near, far, orthographic) = near_far(&camera.projection);
        let depth = [
            near,
            far,
            (far / near).max(1.0).ln(),
            f32::from(u8::from(orthographic)),
        ];
        let mut params = Vec::with_capacity(PARAMS_BYTES as usize);
        params.extend_from_slice(bytemuck::cast_slice(&camera.view.to_cols_array()));
        params.extend_from_slice(bytemuck::cast_slice(&camera.projection.to_cols_array()));
        for value in [GRID[0], GRID[1], GRID[2], 0] {
            params.extend_from_slice(&value.to_le_bytes());
        }
        for value in depth {
            params.extend_from_slice(&value.to_le_bytes());
        }
        for value in [lights.first, lights.count, 0, 0] {
            params.extend_from_slice(&value.to_le_bytes());
        }
        gpu.queue.write_buffer(&self.params, 0, &params);
        encoder.clear_buffer(&self.stats, 0, None);
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some(PASS),
                timestamp_writes: self.timer.as_ref().and_then(PassTimer::compute_writes),
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &*bind_group, &[]);
            pass.dispatch_workgroups(ENTRIES.div_ceil(WORKGROUP), 1, 1);
        }
        if let Some(timer) = &mut self.timer {
            timer.resolve(encoder);
        }
        if self.pending_stats.is_none() {
            encoder.copy_buffer_to_buffer(&self.stats, 0, &self.stats_readback, 0, None);
        }
        self.enabled = true;
        self.fallback = None;
        ClusterUniform {
            grid: [GRID[0], GRID[1], GRID[2], 1],
            depth,
        }
    }

    /// A world view looped although clustering is on: `global_lights`
    /// unbounded lights do not fit the global list.
    pub fn looped(&mut self, global_lights: u32) {
        self.enabled = false;
        self.fallback = Some(format!(
            "{global_lights} unbounded lights exceed the global list of {CLUSTER_CAPACITY}; the pass looped over its lights"
        ));
    }

    /// After the view's encoder was submitted: read the stats and timing.
    pub fn submitted(&mut self) {
        if let Some(timer) = &mut self.timer {
            timer.submitted();
        }
        if self.pending_stats.is_none() {
            let (sender, receiver) = std::sync::mpsc::channel();
            self.stats_readback
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |result| {
                    let _ = sender.send(result);
                });
            self.pending_stats = Some(receiver);
        }
    }

    /// Take the last readback's stats if it has mapped, without waiting.
    fn collect(&mut self, gpu: &Gpu) {
        let Some(receiver) = self.pending_stats.as_ref() else {
            return;
        };
        let _ = gpu.device.poll(wgpu::PollType::Poll);
        match receiver.try_recv() {
            Err(TryRecvError::Empty) => return,
            Ok(Ok(())) => {
                if let Ok(mapped) = self.stats_readback.slice(..).get_mapped_range() {
                    let word = |index: usize| {
                        u32::from_le_bytes(mapped[index * 4..index * 4 + 4].try_into().unwrap())
                    };
                    self.overflowed_clusters = word(0);
                    self.binned_lights = word(1);
                    self.global_lights = word(2);
                }
                self.stats_readback.unmap();
            }
            Ok(Err(_)) | Err(TryRecvError::Disconnected) => {}
        }
        self.pending_stats = None;
    }

    pub fn timing(&self) -> GpuPassTiming {
        self.timer
            .as_ref()
            .map_or_else(|| crate::timing::untimed(PASS), PassTimer::readout)
    }

    pub fn readout(&self) -> LightClusterReadout {
        LightClusterReadout {
            enabled: self.enabled,
            refused: self.refused.clone(),
            fallback: self.fallback.clone(),
            grid: GRID,
            cluster_stride: CLUSTER_STRIDE,
            binned_lights: self.binned_lights,
            global_lights: self.global_lights,
            overflowed_clusters: self.overflowed_clusters,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn near_and_far_come_back_from_either_projection() {
        let (near, far, orthographic) = near_far(&Mat4::perspective_rh(1.0, 1.5, 0.25, 300.0));
        assert!((near - 0.25).abs() < 1e-4, "{near}");
        // The far plane comes back through a near-cancelling term: a tenth of
        // a percent is more than the depth slices need.
        assert!((far - 300.0).abs() < 0.3, "{far}");
        assert!(!orthographic);
        let (near, far, orthographic) =
            near_far(&Mat4::orthographic_rh(-2.0, 2.0, -1.0, 1.0, 0.5, 40.0));
        assert!((near - 0.5).abs() < 1e-4 && (far - 40.0).abs() < 1e-3);
        assert!(orthographic);
    }
}
