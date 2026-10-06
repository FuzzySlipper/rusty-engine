//! GPU visibility and indirect drawing (`cull.wgsl`), a compute candidate
//! (#5912) on the compute pass foundation.
//!
//! With the host's `RendererOptions::gpu_culling`, a view layer's opaque
//! batches are built once per regroup without a frustum test (the candidate
//! list, in `batch.rs`), and each view pass dispatches one thread per
//! candidate to test the part's world bounds against its frustum. Visible
//! ids are appended to their batch's run in the instance buffer and counted
//! in the batch's `DrawIndexedIndirectArgs`, which the pass then draws with
//! `draw_indexed_indirect` (or `multi_draw_indexed_indirect` over runs of
//! batches sharing a mesh and material). Blended parts still sort back to
//! front on the CPU, so they keep the CPU list. The CPU keeps every part's
//! world bounds in a storage buffer beside its row.
//!
//! A device without compute shaders, indirect execution or indirect first
//! instance keeps the CPU draw list and the readout says why.

use std::sync::mpsc::{Receiver, TryRecvError};

use glam::Mat4;

use crate::batch::{Batch, DrawList, Frustum};
use crate::frame::ViewLayer;
use crate::tables::Aabb;
use crate::timing::{GpuPassTiming, PassTimer};
use crate::Gpu;

const WORKGROUP: u32 = 64;
/// `Bounds` in cull.wgsl: min and max, each padded to a vec4.
pub(crate) const BOUNDS_ROW_BYTES: u64 = 32;
/// `CullParams`: six planes and the counts.
const PARAMS_BYTES: u64 = 6 * 16 + 16;
/// `wgpu::util::DrawIndexedIndirectArgs`.
const ARGS_BYTES: u64 = 20;
const STATS_BYTES: u64 = 16;
const PASS: &str = "gpu-cull";
const INITIAL_CANDIDATES: u64 = 1024;
const INITIAL_BATCHES: u64 = 256;

/// The GPU culling of the last view pass, for diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuCullingReadout {
    /// The last view pass drew its opaque batches from GPU-culled runs.
    pub enabled: bool,
    /// Why this device cannot cull on the GPU; `None` while it can.
    pub refused: Option<String>,
    /// Opaque candidates the last pass tested.
    pub candidates: u32,
    /// Opaque batches the last pass drew indirectly.
    pub batches: u32,
    /// Instances the last read-back pass found visible.
    pub visible: u32,
    /// Runs of batches drawn with one multi-draw each; 0 without the feature.
    pub multi_draws: u32,
    /// The device has `multi_draw_indexed_indirect`.
    pub multi_draw: bool,
}

/// One layer's opaque candidates and their indirect arguments, rebuilt when
/// the layer's parts regroup.
pub(crate) struct CandidateList {
    pub list: DrawList,
    /// The arguments with `instance_count` 0, copied over `args` each pass.
    /// Each batch's run of visible ids starts at the visible base plus its
    /// offset within the candidates, so the visible region mirrors them.
    template: Vec<u8>,
}

/// One view layer's upload: its candidate ids, each one's batch, the
/// argument template and the arguments the pass draws from. The world and
/// viewmodel lists are culled in turn every frame, so each keeps its own.
struct LayerBuffers {
    candidates: wgpu::Buffer,
    candidate_batches: wgpu::Buffer,
    args: wgpu::Buffer,
    args_template: wgpu::Buffer,
    bind_group: Option<wgpu::BindGroup>,
}

impl LayerBuffers {
    fn new(device: &wgpu::Device) -> Self {
        let buffer = |label, size, usage| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage,
                mapped_at_creation: false,
            })
        };
        let storage = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST;
        Self {
            candidates: buffer(
                "render-wgpu gpu cull candidates",
                INITIAL_CANDIDATES * 4,
                storage,
            ),
            candidate_batches: buffer(
                "render-wgpu gpu cull candidate batches",
                INITIAL_CANDIDATES * 4,
                storage,
            ),
            args: buffer(
                "render-wgpu gpu cull draws",
                INITIAL_BATCHES * ARGS_BYTES,
                wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::INDIRECT
                    | wgpu::BufferUsages::COPY_DST,
            ),
            args_template: buffer(
                "render-wgpu gpu cull draw template",
                INITIAL_BATCHES * ARGS_BYTES,
                wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
            ),
            bind_group: None,
        }
    }
}

pub(crate) struct GpuCulling {
    pipeline: Option<wgpu::ComputePipeline>,
    refused: Option<String>,
    layout: wgpu::BindGroupLayout,
    params: wgpu::Buffer,
    /// Every part slot's world bounds.
    pub bounds: wgpu::Buffer,
    /// The world layer's and the viewmodel layer's uploads (`ViewLayer`).
    layers: [LayerBuffers; 2],
    stats: wgpu::Buffer,
    stats_readback: wgpu::Buffer,
    pending_stats: Option<Receiver<Result<(), wgpu::BufferAsyncError>>>,
    timer: Option<PassTimer>,
    multi_draw: bool,
    /// Every part slot's bounds are in the buffer; false while the host
    /// culls on the CPU, so turning GPU culling on uploads them all.
    bounds_complete: bool,
    enabled: bool,
    last_candidates: u32,
    last_batches: u32,
    last_multi_draws: u32,
    visible: u32,
}

impl CandidateList {
    /// Arguments for `list`'s batches over `meshes`: each batch's index
    /// range and its visible run.
    pub fn new(
        list: DrawList,
        visible_base: u32,
        index_range: impl Fn(&Batch) -> Option<(u32, u32)>,
    ) -> Self {
        let mut template = Vec::with_capacity(list.batches.len() * ARGS_BYTES as usize);
        for batch in &list.batches {
            let (first_index, index_count) = index_range(batch).unwrap_or((0, 0));
            template.extend_from_slice(
                wgpu::util::DrawIndexedIndirectArgs {
                    index_count,
                    instance_count: 0,
                    first_index,
                    base_vertex: 0,
                    first_instance: visible_base + (batch.first_instance - list_base(&list)),
                }
                .as_bytes(),
            );
        }
        Self { list, template }
    }
}

/// The first instance of a list (its base in the instance buffer).
fn list_base(list: &DrawList) -> u32 {
    list.batches.first().map_or(0, |batch| batch.first_instance)
}

impl GpuCulling {
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
            label: Some("render-wgpu gpu cull"),
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
                storage(2, true),
                storage(3, true),
                storage(4, false),
                storage(5, false),
                storage(6, false),
            ],
        });
        let mut refused = gpu.compute_refusal([WORKGROUP, 1, 1], 0);
        if refused.is_none()
            && !gpu
                .adapter
                .get_downlevel_capabilities()
                .flags
                .contains(wgpu::DownlevelFlags::INDIRECT_EXECUTION)
        {
            refused = Some("the adapter has no indirect execution".to_owned());
        }
        if refused.is_none()
            && !device
                .features()
                .contains(wgpu::Features::INDIRECT_FIRST_INSTANCE)
        {
            refused = Some("the device has no indirect first instance".to_owned());
        }
        let pipeline = refused.is_none().then(|| {
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("render-wgpu gpu cull"),
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 0,
            });
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("render-wgpu gpu cull"),
                layout: Some(&pipeline_layout),
                module: &shader,
                entry_point: Some("cs_cull"),
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
        let multi_draw = pipeline.is_some();
        Self {
            pipeline,
            refused,
            layout,
            params: buffer(
                "render-wgpu gpu cull params",
                PARAMS_BYTES,
                wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            ),
            bounds: buffer(
                "render-wgpu part bounds",
                INITIAL_CANDIDATES * BOUNDS_ROW_BYTES,
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            ),
            layers: [LayerBuffers::new(device), LayerBuffers::new(device)],
            stats: buffer(
                "render-wgpu gpu cull stats",
                STATS_BYTES,
                wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_SRC
                    | wgpu::BufferUsages::COPY_DST,
            ),
            stats_readback: buffer(
                "render-wgpu gpu cull stats readback",
                STATS_BYTES,
                wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            ),
            pending_stats: None,
            timer: PassTimer::new(gpu, PASS),
            // wgpu 30 draws several indirect arguments in one call wherever
            // it executes indirect draws at all.
            multi_draw,
            bounds_complete: false,
            enabled: false,
            last_candidates: 0,
            last_batches: 0,
            last_multi_draws: 0,
            visible: 0,
        }
    }

    pub fn available(&self) -> bool {
        self.pipeline.is_some()
    }

    pub fn multi_draw(&self) -> bool {
        self.multi_draw
    }

    /// The CPU is culling: the bounds rows fall behind until GPU culling
    /// resumes.
    pub fn bounds_stale(&mut self) {
        self.bounds_complete = false;
    }

    /// Room for `slots` part bounds rows. Returns whether every row must
    /// upload: the buffer was replaced, or the rows fell behind.
    pub fn reserve_bounds(&mut self, device: &wgpu::Device, slots: u32) -> bool {
        let needed = u64::from(slots.max(1)) * BOUNDS_ROW_BYTES;
        if needed <= self.bounds.size() {
            return !std::mem::replace(&mut self.bounds_complete, true);
        }
        self.bounds_complete = true;
        self.bounds = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("render-wgpu part bounds"),
            size: needed.next_power_of_two(),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.invalidate();
        true
    }

    /// The arguments `layer`'s last cull wrote, which its pass draws from.
    pub fn args(&self, layer: ViewLayer) -> &wgpu::Buffer {
        &self.layers[layer as usize].args
    }

    /// Write one part slot's world bounds.
    pub fn write_bounds(&self, queue: &wgpu::Queue, slot: u32, bounds: &Aabb) {
        let row: [f32; 8] = [
            bounds.min.x,
            bounds.min.y,
            bounds.min.z,
            0.0,
            bounds.max.x,
            bounds.max.y,
            bounds.max.z,
            0.0,
        ];
        queue.write_buffer(
            &self.bounds,
            u64::from(slot) * BOUNDS_ROW_BYTES,
            bytemuck::cast_slice(&row),
        );
    }

    /// Upload a layer's candidate list: ids, each one's batch, and the
    /// argument template.
    pub fn upload_candidates(&mut self, gpu: &Gpu, layer: ViewLayer, candidates: &CandidateList) {
        let device = &gpu.device;
        let buffers = &mut self.layers[layer as usize];
        let count = candidates.list.ids.len().max(1) as u64;
        let batches = candidates.list.batches.len().max(1) as u64;
        let grow = |buffer: &mut wgpu::Buffer, label: &str, needed: u64, usage| {
            if needed > buffer.size() {
                *buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(label),
                    size: needed.next_power_of_two(),
                    usage,
                    mapped_at_creation: false,
                });
                true
            } else {
                false
            }
        };
        let storage = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST;
        let mut replaced = grow(
            &mut buffers.candidates,
            "render-wgpu gpu cull candidates",
            count * 4,
            storage,
        );
        replaced |= grow(
            &mut buffers.candidate_batches,
            "render-wgpu gpu cull candidate batches",
            count * 4,
            storage,
        );
        replaced |= grow(
            &mut buffers.args,
            "render-wgpu gpu cull draws",
            batches * ARGS_BYTES,
            wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::INDIRECT
                | wgpu::BufferUsages::COPY_DST,
        );
        replaced |= grow(
            &mut buffers.args_template,
            "render-wgpu gpu cull draw template",
            batches * ARGS_BYTES,
            wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
        );
        if replaced {
            buffers.bind_group = None;
        }
        if !candidates.list.ids.is_empty() {
            gpu.queue.write_buffer(
                &buffers.candidates,
                0,
                bytemuck::cast_slice(&candidates.list.ids),
            );
            let mut batch_of: Vec<u32> = Vec::with_capacity(candidates.list.ids.len());
            for (index, batch) in candidates.list.batches.iter().enumerate() {
                batch_of.extend(std::iter::repeat_n(index as u32, batch.instances as usize));
            }
            gpu.queue.write_buffer(
                &buffers.candidate_batches,
                0,
                bytemuck::cast_slice(&batch_of),
            );
            gpu.queue
                .write_buffer(&buffers.args_template, 0, &candidates.template);
        }
    }

    /// The instance or bounds buffer was replaced: every layer binds again
    /// before its next dispatch.
    pub fn invalidate(&mut self) {
        for layer in &mut self.layers {
            layer.bind_group = None;
        }
    }

    /// Record that a pass drew from the CPU list.
    pub fn skipped(&mut self) {
        self.enabled = false;
    }

    /// Cull `layer`'s `candidates` for `view_proj` in `encoder` before the
    /// view's render pass; the pass then draws from `args(layer)` and
    /// `instances`.
    pub fn encode(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        instances: &wgpu::Buffer,
        layer: ViewLayer,
        candidates: &CandidateList,
        view_proj: &Mat4,
    ) {
        self.collect(gpu);
        if let Some(timer) = &mut self.timer {
            timer.collect(gpu);
        }
        let Some(pipeline) = &self.pipeline else {
            return;
        };
        let device = &gpu.device;
        let (layout, params_buffer, bounds, stats) =
            (&self.layout, &self.params, &self.bounds, &self.stats);
        let buffers = &mut self.layers[layer as usize];
        let bind_group = buffers.bind_group.get_or_insert_with(|| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("render-wgpu gpu cull"),
                layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: params_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: bounds.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: buffers.candidates.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: buffers.candidate_batches.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: buffers.args.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: instances.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 6,
                        resource: stats.as_entire_binding(),
                    },
                ],
            })
        });
        let count = candidates.list.ids.len() as u32;
        let batches = candidates.list.batches.len() as u32;
        let mut params = Vec::with_capacity(PARAMS_BYTES as usize);
        for plane in Frustum::new(view_proj).planes() {
            params.extend_from_slice(bytemuck::cast_slice(&plane.to_array()));
        }
        for value in [count, batches, 0, 0] {
            params.extend_from_slice(&value.to_le_bytes());
        }
        gpu.queue.write_buffer(params_buffer, 0, &params);
        let template_bytes = candidates.template.len() as u64;
        if template_bytes > 0 {
            encoder.copy_buffer_to_buffer(
                &buffers.args_template,
                0,
                &buffers.args,
                0,
                Some(template_bytes),
            );
        }
        encoder.clear_buffer(stats, 0, None);
        if count > 0 {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some(PASS),
                timestamp_writes: self.timer.as_ref().and_then(PassTimer::compute_writes),
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &*bind_group, &[]);
            pass.dispatch_workgroups(count.div_ceil(WORKGROUP), 1, 1);
            drop(pass);
            if let Some(timer) = &mut self.timer {
                timer.resolve(encoder);
            }
        }
        if self.pending_stats.is_none() {
            encoder.copy_buffer_to_buffer(&self.stats, 0, &self.stats_readback, 0, None);
        }
        self.enabled = true;
        self.last_candidates = count;
        self.last_batches = batches;
    }

    /// Note how many multi-draws the pass issued.
    pub fn drew(&mut self, multi_draws: u32) {
        self.last_multi_draws = multi_draws;
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

    fn collect(&mut self, gpu: &Gpu) {
        let Some(receiver) = self.pending_stats.as_ref() else {
            return;
        };
        let _ = gpu.device.poll(wgpu::PollType::Poll);
        match receiver.try_recv() {
            Err(TryRecvError::Empty) => return,
            Ok(Ok(())) => {
                if let Ok(mapped) = self.stats_readback.slice(..).get_mapped_range() {
                    self.visible = u32::from_le_bytes(mapped[0..4].try_into().unwrap());
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

    pub fn readout(&self) -> GpuCullingReadout {
        GpuCullingReadout {
            enabled: self.enabled,
            refused: self.refused.clone(),
            candidates: self.last_candidates,
            batches: self.last_batches,
            visible: self.visible,
            multi_draws: self.last_multi_draws,
            multi_draw: self.multi_draw,
        }
    }
}
