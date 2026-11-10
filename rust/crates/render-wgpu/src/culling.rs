//! GPU visibility and indirect drawing (`cull.wgsl`), a compute candidate
//! (#5912) on the compute pass foundation.
//!
//! With the host's `RendererOptions::gpu_culling`, a view layer's opaque
//! batches are built once per regroup without a frustum test (the candidate
//! list, in `batch.rs`), and each view pass dispatches one thread per
//! candidate to test the part's world bounds against its frustum, then one
//! per batch to write its visible ids, in the list's order, to its run in
//! the instance buffer and their count to its `DrawIndexedIndirectArgs`. The
//! order is the CPU list's, so surfaces drawn at one depth resolve as they
//! do there, every frame. The pass then draws the arguments with
//! `draw_indexed_indirect` (or `multi_draw_indexed_indirect` over runs of
//! batches sharing a mesh and material). Blended parts still sort back to
//! front on the CPU, so they keep the CPU list. The CPU keeps every part's
//! world bounds in a storage buffer beside its row.
//!
//! With `RendererOptions::occlusion_culling`, the world layer's opaque parts
//! are candidates too: without GPU culling, those of the CPU's frustum list
//! (rebuilt as it is, when the drawn set changes), so a pass encodes only
//! what is in view. Its cull also drops parts wholly behind the view's
//! farthest-depth pyramid (`occlusion.rs`). Before the cull, a first
//! dispatch selects the candidates the last cull found visible (a flag per
//! part slot, `shown`) whose materials keep every sample they cover; the
//! view draws them depth-only at this frame's camera and reduces that depth
//! to the pyramid. A dropped part is therefore hidden behind geometry the
//! frame draws, so nothing goes missing when the camera turns or moves or
//! parts regroup; the parts that come into view are tested against what
//! was in view, and draw. Until a cull has marked any part, every
//! candidate in the frustum is an occluder.
//!
//! A device without compute shaders, indirect execution or indirect first
//! instance keeps the CPU draw list and the readout says why.

use std::sync::mpsc::{Receiver, TryRecvError};

use glam::Mat4;

use crate::batch::{Batch, DrawList, Frustum};
use crate::frame::ViewLayer;
use crate::occlusion::Occluders;
use crate::tables::Aabb;
use crate::timing::{GpuPassTiming, PassTimer};
use crate::Gpu;

const WORKGROUP: u32 = 64;
/// `Bounds` in cull.wgsl: min and max, each padded to a vec4.
pub(crate) const BOUNDS_ROW_BYTES: u64 = 32;
/// `CullParams`: six planes, the counts, the camera and the occlusion test.
const PARAMS_BYTES: u64 = 6 * 16 + 16 + 64 + 16;
/// The cull's and the occluder selection's params, each at its own dynamic
/// offset: the most any device asks uniform offsets to align to.
const PARAMS_STRIDE: u64 = 256;
/// A candidate's batch with this bit set never occludes (`cull.wgsl`).
const NO_OCCLUDER: u32 = 1 << 31;
/// `wgpu::util::DrawIndexedIndirectArgs`.
const ARGS_BYTES: u64 = 20;
const STATS_BYTES: u64 = 16;
/// `shown`: one flag per part slot.
const SHOWN_BYTES: u64 = 4;
const PASS: &str = "gpu-cull";
/// The selection of the occluders, before the occlusion's depth pass.
const SELECT_PASS: &str = "occluder-select";
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
    /// The last pass tested its candidates against an occlusion pyramid.
    pub occlusion: bool,
    /// Instances inside the frustum the last read-back pass found wholly
    /// behind the pyramid, and so did not draw.
    pub occluded: u32,
    /// Instances its occluder depth drew: those the cull before found
    /// visible, inside the frustum, with materials that may occlude.
    pub occluders: u32,
    /// Runs of batches drawn with one multi-draw each; 0 without the feature.
    pub multi_draws: u32,
    /// The device has `multi_draw_indexed_indirect`.
    pub multi_draw: bool,
}

/// One layer's opaque candidates and their indirect arguments, rebuilt when
/// the layer's parts regroup.
pub(crate) struct CandidateList {
    pub list: DrawList,
    /// Per batch: its material keeps every sample it covers, so it may
    /// draw into the occluder depth.
    pub occludes: Vec<bool>,
    /// Runs of batches that draw with one pipeline, material and buffers,
    /// found once when the list is built, so a pass binds and draws per run
    /// rather than looking each batch up.
    pub runs: Vec<std::ops::Range<usize>>,
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

/// The frustum cull, the occluder selection and the cull with occlusion
/// (`cull.wgsl`).
struct Pipelines {
    cull: wgpu::ComputePipeline,
    occluders: wgpu::ComputePipeline,
    occluded: wgpu::ComputePipeline,
    compact: wgpu::ComputePipeline,
}

pub(crate) struct GpuCulling {
    pipelines: Option<Pipelines>,
    refused: Option<String>,
    layout: wgpu::BindGroupLayout,
    pyramid_layout: wgpu::BindGroupLayout,
    params: wgpu::Buffer,
    /// Every part slot's world bounds.
    pub bounds: wgpu::Buffer,
    /// Every part slot's flag: the last cull of its layer found it visible.
    shown: wgpu::Buffer,
    /// A cull has written `shown` since it was made.
    shown_marked: bool,
    /// This view's occluders were selected, and their count is in the stats.
    occluders_selected: bool,
    /// The world layer's and the viewmodel layer's uploads (`ViewLayer`).
    layers: [LayerBuffers; 2],
    stats: wgpu::Buffer,
    stats_readback: wgpu::Buffer,
    pending_stats: Option<Receiver<Result<(), wgpu::BufferAsyncError>>>,
    timer: Option<PassTimer>,
    select_timer: Option<PassTimer>,
    multi_draw: bool,
    /// Every part slot's bounds are in the buffer; false while the host
    /// culls on the CPU, so turning GPU culling on uploads them all.
    bounds_complete: bool,
    enabled: bool,
    last_candidates: u32,
    last_batches: u32,
    last_multi_draws: u32,
    last_occlusion: bool,
    visible: u32,
    occluded: u32,
    occluders: u32,
}

impl CandidateList {
    /// Arguments for `list`'s batches over `meshes`: each batch's index
    /// range, the base vertex its mesh starts at in the buffers it draws
    /// from, and its visible run.
    /// `run_key`: batches with equal keys next to each other draw as one
    /// run.
    pub fn new<K: PartialEq>(
        list: DrawList,
        visible_base: u32,
        index_range: impl Fn(&Batch) -> (u32, u32, i32),
        occludes: impl Fn(&Batch) -> bool,
        run_key: impl Fn(&Batch) -> K,
    ) -> Self {
        let mut template = Vec::with_capacity(list.batches.len() * ARGS_BYTES as usize);
        for batch in &list.batches {
            let (first_index, index_count, base_vertex) = index_range(batch);
            template.extend_from_slice(
                wgpu::util::DrawIndexedIndirectArgs {
                    index_count,
                    instance_count: 0,
                    first_index,
                    base_vertex,
                    first_instance: visible_base + (batch.first_instance - list_base(&list)),
                }
                .as_bytes(),
            );
        }
        let occludes = list.batches.iter().map(occludes).collect();
        let mut runs: Vec<std::ops::Range<usize>> = Vec::new();
        let mut previous: Option<K> = None;
        for (index, batch) in list.batches.iter().enumerate() {
            let key = run_key(batch);
            match runs.last_mut() {
                Some(run) if previous.as_ref() == Some(&key) => run.end = index + 1,
                _ => runs.push(index..index + 1),
            }
            previous = Some(key);
        }
        Self {
            list,
            occludes,
            runs,
            template,
        }
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
                        has_dynamic_offset: true,
                        min_binding_size: wgpu::BufferSize::new(PARAMS_BYTES),
                    },
                    count: None,
                },
                storage(1, true),
                storage(2, true),
                storage(3, true),
                storage(4, false),
                storage(5, false),
                storage(6, false),
                storage(7, false),
            ],
        });
        let pyramid_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("render-wgpu gpu cull occluders"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
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
        let pipelines = refused.is_none().then(|| {
            let pipeline = |layouts: &[Option<&wgpu::BindGroupLayout>], entry| {
                let pipeline_layout =
                    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("render-wgpu gpu cull"),
                        bind_group_layouts: layouts,
                        immediate_size: 0,
                    });
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("render-wgpu gpu cull"),
                    layout: Some(&pipeline_layout),
                    module: &shader,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    cache: None,
                })
            };
            Pipelines {
                cull: pipeline(&[Some(&layout)], "cs_cull"),
                occluders: pipeline(&[Some(&layout)], "cs_occluders"),
                occluded: pipeline(&[Some(&layout), Some(&pyramid_layout)], "cs_cull_occluded"),
                compact: pipeline(&[Some(&layout)], "cs_compact"),
            }
        });
        let buffer = |label, size, usage| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage,
                mapped_at_creation: false,
            })
        };
        let multi_draw = pipelines.is_some();
        Self {
            pipelines,
            refused,
            layout,
            pyramid_layout,
            params: buffer(
                "render-wgpu gpu cull params",
                PARAMS_STRIDE * 2,
                wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            ),
            bounds: buffer(
                "render-wgpu part bounds",
                INITIAL_CANDIDATES * BOUNDS_ROW_BYTES,
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            ),
            shown: buffer(
                "render-wgpu part shown",
                INITIAL_CANDIDATES * SHOWN_BYTES,
                wgpu::BufferUsages::STORAGE,
            ),
            shown_marked: false,
            occluders_selected: false,
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
            select_timer: PassTimer::new(gpu, SELECT_PASS),
            // wgpu 30 draws several indirect arguments in one call wherever
            // it executes indirect draws at all.
            multi_draw,
            bounds_complete: false,
            enabled: false,
            last_candidates: 0,
            last_batches: 0,
            last_multi_draws: 0,
            last_occlusion: false,
            visible: 0,
            occluded: 0,
            occluders: 0,
        }
    }

    pub fn available(&self) -> bool {
        self.pipelines.is_some()
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
        let size = needed.next_power_of_two();
        self.bounds = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("render-wgpu part bounds"),
            size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        // A slot's flag sits beside its bounds row; the new flags start
        // unmarked.
        self.shown = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("render-wgpu part shown"),
            size: size / BOUNDS_ROW_BYTES * SHOWN_BYTES,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        self.shown_marked = false;
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
            for (index, (batch, occludes)) in candidates
                .list
                .batches
                .iter()
                .zip(&candidates.occludes)
                .enumerate()
            {
                let entry = if *occludes {
                    index as u32
                } else {
                    index as u32 | NO_OCCLUDER
                };
                batch_of.extend(std::iter::repeat_n(entry, batch.instances as usize));
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

    /// Record that a pass drew from the CPU list; a world view's leaves the
    /// cull's passes reporting no time.
    pub fn skipped(&mut self, layer: ViewLayer) {
        self.enabled = false;
        if layer == ViewLayer::World {
            for timer in [&mut self.timer, &mut self.select_timer]
                .into_iter()
                .flatten()
            {
                timer.restart();
            }
        }
    }

    /// The layer's bind group, made after its buffers or the shared ones
    /// were replaced.
    fn bind_group(&mut self, device: &wgpu::Device, instances: &wgpu::Buffer, layer: ViewLayer) {
        let (layout, params, bounds, stats, shown) = (
            &self.layout,
            &self.params,
            &self.bounds,
            &self.stats,
            &self.shown,
        );
        let buffers = &mut self.layers[layer as usize];
        buffers.bind_group.get_or_insert_with(|| {
            fn entry(binding: u32, buffer: &wgpu::Buffer) -> wgpu::BindGroupEntry<'_> {
                wgpu::BindGroupEntry {
                    binding,
                    resource: buffer.as_entire_binding(),
                }
            }
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("render-wgpu gpu cull"),
                layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: params,
                            offset: 0,
                            size: wgpu::BufferSize::new(PARAMS_BYTES),
                        }),
                    },
                    entry(1, bounds),
                    entry(2, &buffers.candidates),
                    entry(3, &buffers.candidate_batches),
                    entry(4, &buffers.args),
                    entry(5, instances),
                    entry(6, stats),
                    entry(7, shown),
                ],
            })
        });
    }

    /// The params at `offset`: the frustum of `view_proj`, the counts and,
    /// with `occluders`, the occlusion test.
    fn write_params(
        &self,
        gpu: &Gpu,
        offset: u64,
        candidates: &CandidateList,
        view_proj: &Mat4,
        every_occluder: bool,
        occluders: Option<&Occluders<'_>>,
    ) {
        let mut params = Vec::with_capacity(PARAMS_BYTES as usize);
        for plane in Frustum::new(view_proj).planes() {
            params.extend_from_slice(bytemuck::cast_slice(&plane.to_array()));
        }
        let counts = [
            candidates.list.ids.len() as u32,
            candidates.list.batches.len() as u32,
            u32::from(every_occluder),
            0,
        ];
        for value in counts {
            params.extend_from_slice(&value.to_le_bytes());
        }
        params.extend_from_slice(bytemuck::cast_slice(&view_proj.to_cols_array()));
        let occlusion = occluders.map_or([0; 4], |occluders| {
            [
                occluders.region[0],
                occluders.region[1],
                occluders.levels,
                0,
            ]
        });
        for value in occlusion {
            params.extend_from_slice(&value.to_le_bytes());
        }
        gpu.queue.write_buffer(&self.params, offset, &params);
    }

    /// Select `layer`'s occluders for `view_proj` into `args(layer)` before
    /// the view's occluder depth pass draws them (`occlusion.rs`): the
    /// candidates the last cull found visible, inside the frustum, whose
    /// batches occlude. Timed as `occluder-select`.
    pub fn encode_occluders(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        instances: &wgpu::Buffer,
        layer: ViewLayer,
        candidates: &CandidateList,
        view_proj: &Mat4,
    ) {
        if self.pipelines.is_none() {
            return;
        }
        if let Some(timer) = &mut self.select_timer {
            timer.collect(gpu);
        }
        self.bind_group(&gpu.device, instances, layer);
        self.write_params(
            gpu,
            PARAMS_STRIDE,
            candidates,
            view_proj,
            !self.shown_marked,
            None,
        );
        let buffers = &self.layers[layer as usize];
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
        encoder.clear_buffer(&self.stats, 0, None);
        let count = candidates.list.ids.len() as u32;
        let pipelines = self.pipelines.as_ref().expect("checked above");
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some(SELECT_PASS),
            timestamp_writes: self
                .select_timer
                .as_ref()
                .and_then(PassTimer::compute_writes),
        });
        if count > 0 {
            pass.set_pipeline(&pipelines.occluders);
            pass.set_bind_group(
                0,
                buffers.bind_group.as_ref().expect("bound above"),
                &[PARAMS_STRIDE as u32],
            );
            pass.dispatch_workgroups(count.div_ceil(WORKGROUP), 1, 1);
        }
        drop(pass);
        if let Some(timer) = &mut self.select_timer {
            timer.resolve(encoder);
        }
        self.occluders_selected = true;
    }

    /// Cull `layer`'s `candidates` for `view_proj` in `encoder` before the
    /// view's render pass, against `occluders` when given; the pass then
    /// draws from `args(layer)` and `instances`.
    #[allow(clippy::too_many_arguments, reason = "one dispatch's inputs")]
    pub fn encode(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        instances: &wgpu::Buffer,
        layer: ViewLayer,
        candidates: &CandidateList,
        view_proj: &Mat4,
        occluders: Option<Occluders<'_>>,
    ) {
        self.collect(gpu);
        if let Some(timer) = &mut self.timer {
            timer.collect(gpu);
        }
        if self.pipelines.is_none() {
            return;
        }
        let device = &gpu.device;
        self.bind_group(device, instances, layer);
        self.write_params(gpu, 0, candidates, view_proj, false, occluders.as_ref());
        let pyramid = occluders.as_ref().map(|occluders| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("render-wgpu gpu cull occluders"),
                layout: &self.pyramid_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(occluders.pyramid),
                }],
            })
        });
        let buffers = &self.layers[layer as usize];
        let count = candidates.list.ids.len() as u32;
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
        // The occluder selection cleared the stats and counted into them.
        if !std::mem::take(&mut self.occluders_selected) {
            encoder.clear_buffer(&self.stats, 0, None);
        }
        if count > 0 {
            let pipelines = self.pipelines.as_ref().expect("checked above");
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some(PASS),
                timestamp_writes: self.timer.as_ref().and_then(PassTimer::compute_writes),
            });
            pass.set_bind_group(0, buffers.bind_group.as_ref().expect("bound above"), &[0]);
            match &pyramid {
                Some(pyramid) => {
                    pass.set_pipeline(&pipelines.occluded);
                    pass.set_bind_group(1, pyramid, &[]);
                }
                None => pass.set_pipeline(&pipelines.cull),
            }
            pass.dispatch_workgroups(count.div_ceil(WORKGROUP), 1, 1);
            pass.set_pipeline(&pipelines.compact);
            pass.dispatch_workgroups(
                (candidates.list.batches.len() as u32).div_ceil(WORKGROUP),
                1,
                1,
            );
            drop(pass);
            if let Some(timer) = &mut self.timer {
                timer.resolve(encoder);
            }
        }
        if self.pending_stats.is_none() {
            encoder.copy_buffer_to_buffer(&self.stats, 0, &self.stats_readback, 0, None);
        }
        if occluders.is_none() {
            // No occluders were selected: that pass reports no time.
            if let Some(timer) = &mut self.select_timer {
                timer.restart();
            }
        }
        self.shown_marked = true;
        self.enabled = true;
        self.last_candidates = count;
        self.last_batches = candidates.list.batches.len() as u32;
        self.last_occlusion = occluders.is_some();
    }

    /// Note how many multi-draws the pass issued.
    pub fn drew(&mut self, multi_draws: u32) {
        self.last_multi_draws = multi_draws;
    }

    /// After the view's encoder was submitted: read the stats and timing.
    pub fn submitted(&mut self) {
        for timer in [&mut self.timer, &mut self.select_timer]
            .into_iter()
            .flatten()
        {
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
                    let stat = |index: usize| {
                        u32::from_le_bytes(mapped[index * 4..index * 4 + 4].try_into().unwrap())
                    };
                    (self.visible, self.occluded, self.occluders) = (stat(0), stat(1), stat(2));
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

    pub fn selection_timing(&self) -> GpuPassTiming {
        self.select_timer
            .as_ref()
            .map_or_else(|| crate::timing::untimed(SELECT_PASS), PassTimer::readout)
    }

    pub fn readout(&self) -> GpuCullingReadout {
        GpuCullingReadout {
            enabled: self.enabled,
            refused: self.refused.clone(),
            candidates: self.last_candidates,
            batches: self.last_batches,
            visible: self.visible,
            occlusion: self.last_occlusion,
            occluded: self.occluded,
            occluders: self.occluders,
            multi_draws: self.last_multi_draws,
            multi_draw: self.multi_draw,
        }
    }
}
