//! The renderer's compute pass (`compute.wgsl`): one dispatch after the row
//! uploads and before the view passes, with its bind group, storage buffers
//! and GPU timing. It is the foundation the compute candidates (#9496) build
//! on, so each reports its cost the same way; its own workload is a proof
//! that reads the part rows and writes each part's world position, which
//! `tests/compute.rs` reads back.
//!
//! The pass refuses, with a diagnostic, on an adapter without compute
//! shaders or with workgroup limits below its own; the frame draws without
//! it. Timing uses the device's timestamp queries when it has them: each
//! timed frame resolves the pass's begin and end stamps into a mapped
//! buffer, read without blocking on a later frame, and the readout gives the
//! median over the recent timed frames.

use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, TryRecvError};

use crate::Gpu;

/// `@workgroup_size` in compute.wgsl.
pub(crate) const WORKGROUP_SIZE: u32 = 64;
/// `ComputeParams` in compute.wgsl.
const PARAMS_BYTES: u64 = 16;
/// One `vec4<f32>` per part row.
const OUTPUT_ROW_BYTES: u64 = 16;
/// Begin and end of the pass.
const TIMESTAMPS: u32 = 2;
const TIMESTAMP_BYTES: u64 = 8 * TIMESTAMPS as u64;
/// Timed frames the median is taken over.
const TIMING_WINDOW: usize = 120;

/// The adapter's compute limits. The device takes wgpu's default limits
/// (`gpu.rs`); a candidate needing more raises them there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComputeLimits {
    pub workgroup_size: [u32; 3],
    pub invocations_per_workgroup: u32,
    pub workgroups_per_dimension: u32,
    pub workgroup_storage_bytes: u32,
    pub storage_buffer_binding_bytes: u64,
}

/// What the compute pass reports for diagnostics and evidence.
#[derive(Debug, Clone, PartialEq)]
pub struct ComputeReadout {
    /// Why the pass does not run on this adapter; `None` while it runs.
    pub refused: Option<String>,
    /// The device has timestamp queries, so the pass is timed.
    pub timestamps: bool,
    /// Workgroups the last frame dispatched.
    pub workgroups: u32,
    /// Recent frames whose GPU time was read back.
    pub timed_frames: usize,
    /// Median GPU milliseconds of the pass over those frames; 0 with none.
    pub median_gpu_ms: f64,
    pub limits: ComputeLimits,
}

pub(crate) struct ComputePass {
    pipeline: wgpu::ComputePipeline,
    layout: wgpu::BindGroupLayout,
    params: wgpu::Buffer,
    output: wgpu::Buffer,
    /// Rebuilt when the output buffer grows or the parts buffer is replaced
    /// (`invalidate`).
    bind_group: Option<wgpu::BindGroup>,
    timing: Option<Timing>,
    workgroups: u32,
    limits: ComputeLimits,
}

struct Timing {
    queries: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    readback: wgpu::Buffer,
    /// Nanoseconds per timestamp tick.
    period: f32,
    /// The readback of a timed frame is mapping; its result arrives here.
    pending: Option<Receiver<Result<(), wgpu::BufferAsyncError>>>,
    recent: VecDeque<f64>,
}

impl ComputePass {
    /// The pass on `gpu`, or why this adapter cannot run it.
    pub fn new(gpu: &Gpu, shader: wgpu::ShaderModule) -> Result<Self, String> {
        let downlevel = gpu.adapter.get_downlevel_capabilities();
        if !downlevel
            .flags
            .contains(wgpu::DownlevelFlags::COMPUTE_SHADERS)
        {
            return Err("the adapter has no compute shaders".to_owned());
        }
        let limits = ComputeLimits::of(&gpu.adapter.limits());
        let granted = ComputeLimits::of(&gpu.device.limits());
        if granted.workgroup_size[0] < WORKGROUP_SIZE
            || granted.invocations_per_workgroup < WORKGROUP_SIZE
        {
            return Err(format!(
                "the device allows {} invocations per workgroup ({} along x); the pass needs {WORKGROUP_SIZE}",
                granted.invocations_per_workgroup, granted.workgroup_size[0]
            ));
        }
        let device = &gpu.device;
        let buffer = |binding, read_only| wgpu::BindGroupLayoutEntry {
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
            label: Some("render-wgpu compute"),
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
                buffer(1, true),
                buffer(2, false),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("render-wgpu compute"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("render-wgpu compute"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("cs_parts"),
            compilation_options: Default::default(),
            cache: None,
        });
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("render-wgpu compute params"),
            size: PARAMS_BYTES,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let timing = device
            .features()
            .contains(wgpu::Features::TIMESTAMP_QUERY)
            .then(|| Timing {
                queries: device.create_query_set(&wgpu::QuerySetDescriptor {
                    label: Some("render-wgpu compute timestamps"),
                    ty: wgpu::QueryType::Timestamp,
                    count: TIMESTAMPS,
                }),
                resolve: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("render-wgpu compute timestamps"),
                    size: TIMESTAMP_BYTES,
                    usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                    mapped_at_creation: false,
                }),
                readback: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("render-wgpu compute timestamp readback"),
                    size: TIMESTAMP_BYTES,
                    usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
                period: gpu.queue.get_timestamp_period(),
                pending: None,
                recent: VecDeque::with_capacity(TIMING_WINDOW),
            });
        Ok(Self {
            pipeline,
            layout,
            params,
            output: output_buffer(device, OUTPUT_ROW_BYTES),
            bind_group: None,
            timing,
            workgroups: 0,
            limits,
        })
    }

    /// The parts buffer was replaced: bind again before the next dispatch.
    pub fn invalidate(&mut self) {
        self.bind_group = None;
    }

    /// Dispatch over `count` part rows of `parts` and submit. Collects the
    /// GPU time of the last timed frame first, if its readback finished.
    pub fn encode(&mut self, gpu: &Gpu, parts: &wgpu::Buffer, count: u32) {
        self.collect(gpu);
        let device = &gpu.device;
        let needed = u64::from(count.max(1)) * OUTPUT_ROW_BYTES;
        if needed > self.output.size() {
            self.output = output_buffer(device, needed.next_power_of_two());
            self.bind_group = None;
        }
        let bind_group = self.bind_group.get_or_insert_with(|| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("render-wgpu compute"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.params.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: parts.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: self.output.as_entire_binding(),
                    },
                ],
            })
        });
        let mut params = [0u8; PARAMS_BYTES as usize];
        params[..4].copy_from_slice(&count.to_le_bytes());
        gpu.queue.write_buffer(&self.params, 0, &params);
        self.workgroups = count.div_ceil(WORKGROUP_SIZE).max(1);

        // A frame is timed while no earlier frame's readback is still
        // mapping: one readback buffer, one timed frame in flight.
        let timed = self
            .timing
            .as_ref()
            .filter(|timing| timing.pending.is_none());
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("render-wgpu compute"),
        });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("render-wgpu compute"),
                timestamp_writes: timed.map(|timing| wgpu::ComputePassTimestampWrites {
                    query_set: &timing.queries,
                    beginning_of_pass_write_index: Some(0),
                    end_of_pass_write_index: Some(1),
                }),
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &*bind_group, &[]);
            pass.dispatch_workgroups(self.workgroups, 1, 1);
        }
        if let Some(timing) = timed {
            encoder.resolve_query_set(&timing.queries, 0..TIMESTAMPS, &timing.resolve, 0);
            encoder.copy_buffer_to_buffer(&timing.resolve, 0, &timing.readback, 0, None);
        }
        gpu.queue.submit([encoder.finish()]);
        if let Some(timing) = self
            .timing
            .as_mut()
            .filter(|timing| timing.pending.is_none())
        {
            let (sender, receiver) = std::sync::mpsc::channel();
            timing
                .readback
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |result| {
                    let _ = sender.send(result);
                });
            timing.pending = Some(receiver);
        }
    }

    /// Take the GPU time of the pending timed frame if its readback has
    /// mapped, without waiting for it.
    fn collect(&mut self, gpu: &Gpu) {
        let Some(timing) = self.timing.as_mut() else {
            return;
        };
        let Some(receiver) = timing.pending.as_ref() else {
            return;
        };
        // Mapping completes in a poll; this one never blocks.
        let _ = gpu.device.poll(wgpu::PollType::Poll);
        match receiver.try_recv() {
            Err(TryRecvError::Empty) => return,
            Ok(Ok(())) => {
                if let Ok(mapped) = timing.readback.slice(..).get_mapped_range() {
                    let stamp = |index: usize| {
                        u64::from_le_bytes(mapped[index * 8..index * 8 + 8].try_into().unwrap())
                    };
                    let ticks = stamp(1).saturating_sub(stamp(0));
                    let ms = ticks as f64 * f64::from(timing.period) / 1_000_000.0;
                    if timing.recent.len() == TIMING_WINDOW {
                        timing.recent.pop_front();
                    }
                    timing.recent.push_back(ms);
                }
                timing.readback.unmap();
            }
            // A failed or abandoned map leaves nothing mapped; the next
            // frame times again.
            Ok(Err(_)) | Err(TryRecvError::Disconnected) => {}
        }
        timing.pending = None;
    }

    pub fn readout(&self) -> ComputeReadout {
        let (timed_frames, median_gpu_ms) = match &self.timing {
            Some(timing) if !timing.recent.is_empty() => {
                let mut values: Vec<f64> = timing.recent.iter().copied().collect();
                values.sort_by(f64::total_cmp);
                (values.len(), values[values.len() / 2])
            }
            _ => (0, 0.0),
        };
        ComputeReadout {
            refused: None,
            timestamps: self.timing.is_some(),
            workgroups: self.workgroups,
            timed_frames,
            median_gpu_ms,
            limits: self.limits,
        }
    }

    /// The last dispatch's output rows: each part row's world position and
    /// a 1 where the row was processed. Blocks until the GPU finishes; for
    /// tests and tools, never a frame.
    pub fn read_output(&self, gpu: &Gpu, rows: u32) -> Vec<[f32; 4]> {
        let size = u64::from(rows) * OUTPUT_ROW_BYTES;
        if size == 0 || size > self.output.size() {
            return Vec::new();
        }
        let staging = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("render-wgpu compute readback"),
            size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render-wgpu compute readback"),
            });
        encoder.copy_buffer_to_buffer(&self.output, 0, &staging, 0, Some(size));
        gpu.queue.submit([encoder.finish()]);
        let slice = staging.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
        if !matches!(receiver.recv(), Ok(Ok(()))) {
            return Vec::new();
        }
        let rows = match slice.get_mapped_range() {
            Ok(mapped) => bytemuck::cast_slice::<u8, [f32; 4]>(&mapped).to_vec(),
            Err(_) => Vec::new(),
        };
        staging.unmap();
        rows
    }
}

impl ComputeLimits {
    fn of(limits: &wgpu::Limits) -> Self {
        Self {
            workgroup_size: [
                limits.max_compute_workgroup_size_x,
                limits.max_compute_workgroup_size_y,
                limits.max_compute_workgroup_size_z,
            ],
            invocations_per_workgroup: limits.max_compute_invocations_per_workgroup,
            workgroups_per_dimension: limits.max_compute_workgroups_per_dimension,
            workgroup_storage_bytes: limits.max_compute_workgroup_storage_size,
            storage_buffer_binding_bytes: limits.max_storage_buffer_binding_size,
        }
    }
}

/// The readout of an adapter the pass refused.
pub(crate) fn refused_readout(gpu: &Gpu, reason: &str) -> ComputeReadout {
    ComputeReadout {
        refused: Some(reason.to_owned()),
        timestamps: gpu
            .device
            .features()
            .contains(wgpu::Features::TIMESTAMP_QUERY),
        workgroups: 0,
        timed_frames: 0,
        median_gpu_ms: 0.0,
        limits: ComputeLimits::of(&gpu.adapter.limits()),
    }
}

fn output_buffer(device: &wgpu::Device, size: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("render-wgpu compute output"),
        size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    })
}
