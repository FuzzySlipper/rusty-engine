//! Gaussian splats: animated-mesh primitives drawn as Gaussians
//! (`docs/splat-characters.md`, `splats.wgsl`).
//!
//! **Resources.** A GLB's splat primitive (`glb::GlbSplats`) is uploaded once
//! per asset as a [`SplatSet`]: 32 bytes per Gaussian (centre, display colour
//! and opacity, rotation, standard deviations) in a storage buffer every
//! instance of the asset reads.
//!
//! **Runs.** A view's splat instances take their places among the blended
//! draws by the squared distance of their bounds' centre, as blended parts
//! do (`frame.rs` `draw_blended`). Instances that follow each other there are
//! one run. Each run's Gaussians are projected for the view and sorted back
//! to front together on the GPU, every frame (`cs_project`, the radix sort),
//! so overlapping instances interleave Gaussian by Gaussian.
//!
//! **Layers.** A run is drawn into a layer of its own, an RGBA16F image the
//! size of the view's target, tested against the world's opaque depth and
//! writing none. Splat colours are display-referred and were trained by
//! blending those values, so the layer blends them as they are: blended in
//! linear light, as the HDR target would, a figure comes out lighter. The
//! layer is composited into the HDR target at the run's place among the
//! blended draws, decoded to linear light there.
//!
//! **Cost.** Projecting and sorting run on the GPU for every view each frame
//! with a splat run; nothing is copied to or from the CPU per frame. The
//! readout counts the frame's runs, instances and Gaussians, and the
//! `splats` pass timer covers the projection, the sort and the layers of the
//! frame's first view with a run.

use std::collections::HashMap;

use glam::{DMat4, Mat4, Vec3};

use crate::glb::GlbSplats;
use crate::motion::MotionWrite;
use crate::tables::Aabb;
use crate::target::{ColorTarget, DEPTH_FORMAT};
use crate::timing::PassTimer;
use crate::Gpu;

/// Bytes of one resident Gaussian (`splats.wgsl` `Gaussian`).
const GAUSSIAN_BYTES: u64 = 32;
/// Bytes of one projected Gaussian (`splats.wgsl` `Projected`).
const PROJECTED_BYTES: u64 = 32;
/// One dynamic uniform slot of the parameters buffer.
const PARAMS_SLOT: u64 = 256;
const WORKGROUP: u32 = 256;
/// Keys a sort block takes (`splats.wgsl` `TILE`).
const TILE: u32 = 1024;
const DIGITS: u32 = 256;
/// Workgroup storage the sort's scatter uses: two tiles of keys and values
/// and three digit tables.
const SORT_SHARED_BYTES: u32 = 2 * TILE * 4 + 3 * DIGITS * 4;
/// The most Gaussians one run sorts: the digit counts are scanned by groups
/// of `TILE` whose totals one workgroup scans, so at most `TILE` groups.
pub(crate) const MAX_RUN_GAUSSIANS: u32 = TILE * TILE * TILE / DIGITS;
/// The sort keys' digit shifts: 16-bit keys, two passes of eight bits.
const SORT_SHIFTS: [u32; 2] = [0, 8];
/// How far past a Gaussian's centre its bounds reach, in standard
/// deviations: its quad's reach, 2·sqrt(2)σ, rounded up.
const BOUNDS_SIGMAS: f32 = 3.0;
const LAYER_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// One splat primitive of an animated asset, resident on the GPU.
pub(crate) struct SplatSet {
    /// Its Gaussians.
    pub count: u32,
    /// The Gaussians to `BOUNDS_SIGMAS`, in the primitive's space.
    pub bounds: Aabb,
    /// Whether the primitive is skinned (its centres are in bind space).
    pub skinned: bool,
    group: wgpu::BindGroup,
}

/// What the splats drew over the last frame, summed over its views.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SplatReadout {
    /// Runs drawn: each a layer, its own projection and sort.
    pub runs: u32,
    /// Splat instances drawn.
    pub instances: u32,
    /// Gaussians projected and sorted.
    pub gaussians: u64,
    /// Splat instances that had no room in their run
    /// ([`MAX_RUN_GAUSSIANS`]) and drew nothing.
    pub refused_instances: u32,
    /// Why splats draw nothing on this device, if they do not.
    pub refused: Option<String>,
}

/// One instance's sets as one view draws them.
pub(crate) struct SplatInstance {
    /// Its node: the order between instances at one distance.
    pub handle: render_model::RenderHandle,
    /// Each set and the matrix taking its space to the world.
    pub sets: Vec<(SetRef, Mat4)>,
    /// The sets' world bounds.
    pub bounds: Aabb,
    /// Squared distance from the eye to the bounds' centre: its place among
    /// the blended draws.
    pub depth: f32,
}

/// A set by its asset's slot and its index among the asset's sets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SetRef {
    pub slot: u32,
    pub set: usize,
}

/// The view a run is drawn for.
pub(crate) struct SplatTarget<'a> {
    pub depth: &'a wgpu::TextureView,
    pub width: u32,
    pub height: u32,
    pub samples: u32,
    pub viewport: crate::frame::PixelRect,
    /// The camera the pass draws with (jittered when it is).
    pub camera: crate::camera::CameraMatrices,
}

struct Pipelines {
    project_layout: wgpu::BindGroupLayout,
    gaussian_layout: wgpu::BindGroupLayout,
    sort_layout: wgpu::BindGroupLayout,
    draw_layout: wgpu::BindGroupLayout,
    composite_layout: wgpu::BindGroupLayout,
    draw_pipeline_layout: wgpu::PipelineLayout,
    composite_pipeline_layout: wgpu::PipelineLayout,
    module: wgpu::ShaderModule,
    project: wgpu::ComputePipeline,
    sort_count: wgpu::ComputePipeline,
    scan_local: wgpu::ComputePipeline,
    scan_sums: wgpu::ComputePipeline,
    scan_add: wgpu::ComputePipeline,
    sort_scatter: wgpu::ComputePipeline,
}

/// The buffers a run is projected and sorted in, reused by every run.
struct RunBuffers {
    capacity: u32,
    /// The sorted keys and values, which a test reads back: two passes
    /// sort them back into the projection's pair.
    #[cfg(test)]
    keys: wgpu::Buffer,
    #[cfg(test)]
    values: wgpu::Buffer,
    args: wgpu::Buffer,
    project_group: wgpu::BindGroup,
    /// Pass 0 sorts the projection's keys into the second pair, pass 1 back.
    sort_groups: [wgpu::BindGroup; 2],
    draw_group: wgpu::BindGroup,
}

/// A run's layer: colour at the view's samples, resolved when multisampled,
/// and the bind group the composite reads it by.
struct Layer {
    size: (u32, u32, u32),
    color: wgpu::TextureView,
    resolve: Option<wgpu::TextureView>,
    group: wgpu::BindGroup,
}

pub(crate) struct Splats {
    pipelines: Option<Pipelines>,
    refusal: Option<String>,
    run: Option<RunBuffers>,
    params: wgpu::Buffer,
    params_slots: u64,
    /// Each view's runs draw into the first of these; a view submits its
    /// work before the next view reuses them.
    layers: Vec<Layer>,
    /// The most layers a view drew into this frame; the rest are dropped at
    /// the next.
    layers_used: usize,
    draws: HashMap<u32, wgpu::RenderPipeline>,
    composites: HashMap<ColorTarget, wgpu::RenderPipeline>,
    timer: Option<PassTimer>,
    timed: bool,
    counts: SplatReadout,
    last: SplatReadout,
}

impl Splats {
    pub fn new(
        gpu: &Gpu,
        module: wgpu::ShaderModule,
        frame_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let device = &gpu.device;
        let refusal = gpu
            .compute_refusal([WORKGROUP, 1, 1], SORT_SHARED_BYTES)
            .or_else(|| {
                let flags = gpu.adapter.get_downlevel_capabilities().flags;
                (!flags.contains(wgpu::DownlevelFlags::VERTEX_STORAGE))
                    .then(|| "the adapter cannot read storage buffers in vertex shaders".to_owned())
                    .or_else(|| {
                        (!flags.contains(wgpu::DownlevelFlags::INDIRECT_EXECUTION)).then(|| {
                            "the adapter cannot draw from GPU-written indirect arguments".to_owned()
                        })
                    })
            });
        let pipelines = refusal
            .is_none()
            .then(|| Pipelines::new(device, module, frame_layout));
        Self {
            pipelines,
            refusal,
            run: None,
            params: params_buffer(device, 16),
            params_slots: 16,
            layers: Vec::new(),
            layers_used: 0,
            draws: HashMap::new(),
            composites: HashMap::new(),
            timer: PassTimer::new(gpu, "splats"),
            timed: false,
            counts: SplatReadout::default(),
            last: SplatReadout::default(),
        }
    }

    /// Whether this device draws splats.
    pub fn draws(&self) -> bool {
        self.pipelines.is_some()
    }

    /// Upload a splat primitive's Gaussians, or `None` on a device that
    /// draws no splats.
    pub fn upload(&self, gpu: &Gpu, label: &str, splats: &GlbSplats) -> Option<SplatSet> {
        let pipelines = self.pipelines.as_ref()?;
        let (words, bounds) = gaussian_words(splats);
        let count = splats.centres.len() as u32;
        // An empty buffer cannot be bound: keep one Gaussian's room.
        let mut bytes: Vec<u8> = words.iter().flat_map(|word| word.to_le_bytes()).collect();
        bytes.resize(bytes.len().max(GAUSSIAN_BYTES as usize), 0);
        let buffer = crate::gpu::buffer_init(
            &gpu.device,
            &wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: &bytes,
                usage: wgpu::BufferUsages::STORAGE,
            },
        );
        let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(label),
            layout: &pipelines.gaussian_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });
        Some(SplatSet {
            count,
            bounds,
            skinned: splats.joints.is_some(),
            group,
        })
    }

    /// At the start of a frame: take the last frame's counts and timing,
    /// and drop the layers the last frame did not draw into.
    pub fn begin_frame(&mut self, gpu: &Gpu) {
        if let Some(timer) = &mut self.timer {
            timer.collect(gpu);
        }
        self.timed = false;
        self.last = std::mem::take(&mut self.counts);
        self.layers.truncate(std::mem::take(&mut self.layers_used));
    }

    pub fn readout(&self) -> SplatReadout {
        SplatReadout {
            refused: self.refusal.clone(),
            ..self.last.clone()
        }
    }

    pub fn timing(&self) -> crate::timing::GpuPassTiming {
        self.timer
            .as_ref()
            .map_or_else(|| crate::timing::untimed("splats"), PassTimer::readout)
    }

    /// Instances refused for want of room in a run.
    pub fn refused_instances(&mut self, count: u32) {
        self.counts.refused_instances += count;
    }

    /// Make the composite's pipeline for a pass drawing into `format`
    /// before the pass borrows the renderer.
    pub fn prepare_target(&mut self, device: &wgpu::Device, format: ColorTarget) {
        let Some(pipelines) = &self.pipelines else {
            return;
        };
        self.composites.entry(format).or_insert_with(|| {
            let reactive = format.motion;
            let blend = wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING;
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("render-wgpu splat composite"),
                layout: Some(&pipelines.composite_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &pipelines.module,
                    entry_point: Some("vs_composite"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: wgpu::PrimitiveState::default(),
                // Inside the blended pass, whose depth it neither tests nor
                // writes: the layer was tested against it.
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(false),
                    depth_compare: Some(wgpu::CompareFunction::Always),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: format.multisample(),
                fragment: Some(wgpu::FragmentState {
                    module: &pipelines.module,
                    entry_point: Some(if reactive {
                        "fs_composite_reactive"
                    } else {
                        "fs_composite"
                    }),
                    compilation_options: Default::default(),
                    targets: &format.targets(
                        wgpu::ColorTargetState {
                            format: format.format,
                            blend: Some(blend),
                            write_mask: wgpu::ColorWrites::ALL,
                        },
                        if reactive {
                            MotionWrite::Reactive
                        } else {
                            MotionWrite::None
                        },
                    ),
                }),
                multiview_mask: None,
                cache: None,
            })
        });
    }

    /// Project, sort and draw each run into a layer of its own, in order:
    /// `runs[i]` is composited by [`Self::draw_composite`]`(.., i)`.
    /// `depth_ranges[i]` is run `i`'s nearest and farthest view depth, which
    /// its sort keys span.
    pub fn encode_runs(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        runs: &[Vec<(&SplatSet, Mat4)>],
        depth_ranges: &[(f32, f32)],
        target: &SplatTarget<'_>,
    ) {
        if self.pipelines.is_none() || runs.is_empty() {
            return;
        }
        let largest = runs
            .iter()
            .map(|run| run.iter().map(|(set, _)| set.count).sum::<u32>())
            .max()
            .unwrap_or(0);
        let slots = runs.iter().map(|run| run.len() as u64 + 2).sum::<u64>();
        self.ensure_params(&gpu.device, slots);
        self.ensure_run(&gpu.device, largest.max(1));
        self.ensure_draw(&gpu.device, target.samples);
        for index in 0..runs.len() {
            self.layer(&gpu.device, index, target);
        }
        self.layers_used = self.layers_used.max(runs.len());

        // Every slot this view's dispatches read, written at once: for each
        // run its two sort passes, then its sets.
        let viewport = target.viewport;
        let (width, height) = (viewport.width as f32, viewport.height as f32);
        let projection = target.camera.projection;
        let ortho = projection.z_axis.w == 0.0;
        let lens = [
            projection.x_axis.x * 0.5 * width,
            projection.y_axis.y * 0.5 * height,
            width,
            height,
        ];
        let view = target.camera.view.as_dmat4();
        let view_proj = target.camera.view_proj.as_dmat4();
        let mut bytes = Vec::new();
        let mut offsets = Vec::with_capacity(runs.len());
        for (run, &(near, far)) in runs.iter().zip(depth_ranges) {
            let total: u32 = run.iter().map(|(set, _)| set.count).sum();
            let blocks = total.div_ceil(TILE);
            let sort = bytes.len() as u32;
            for shift in SORT_SHIFTS {
                let mut slot = Vec::with_capacity(PARAMS_SLOT as usize);
                for value in [total, blocks, shift, 0] {
                    slot.extend_from_slice(&value.to_le_bytes());
                }
                slot.resize(PARAMS_SLOT as usize, 0);
                bytes.extend_from_slice(&slot);
            }
            let mut sets = Vec::with_capacity(run.len());
            let mut first = 0u32;
            for (index, (set, model)) in run.iter().enumerate() {
                let model = model.as_dmat4();
                let mut slot = Vec::with_capacity(PARAMS_SLOT as usize);
                push_matrix(&mut slot, &(view * model));
                push_matrix(&mut slot, &(view_proj * model));
                for value in lens {
                    slot.extend_from_slice(&value.to_le_bytes());
                }
                let span = (far - near).max(1e-3);
                for value in [near, 1.0 / span, if ortho { 1.0 } else { 0.0 }, 0.0] {
                    slot.extend_from_slice(&value.to_le_bytes());
                }
                for value in [first, set.count, u32::from(index == 0), 0] {
                    slot.extend_from_slice(&value.to_le_bytes());
                }
                slot.resize(PARAMS_SLOT as usize, 0);
                sets.push(bytes.len() as u32);
                bytes.extend_from_slice(&slot);
                first += set.count;
            }
            offsets.push((sort, sets, total, blocks));
        }
        gpu.queue.write_buffer(&self.params, 0, &bytes);

        let pipelines = self.pipelines.as_ref().expect("checked above");
        let buffers = self.run.as_ref().expect("ensured above");
        let draw = &self.draws[&target.samples];
        let timer = self.timer.as_ref().filter(|_| !self.timed);
        for (index, (run, (sort, sets, total, blocks))) in runs.iter().zip(&offsets).enumerate() {
            let groups = (DIGITS * blocks).div_ceil(TILE);
            encoder.clear_buffer(&buffers.args, 0, None);
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("render-wgpu splats project and sort"),
                    timestamp_writes: timer
                        .filter(|_| index == 0)
                        .and_then(|timer| timer.compute_writes_between(true, false)),
                });
                pass.set_pipeline(&pipelines.project);
                for ((set, _), offset) in run.iter().zip(sets) {
                    pass.set_bind_group(0, &buffers.project_group, &[*offset]);
                    pass.set_bind_group(1, &set.group, &[]);
                    pass.dispatch_workgroups(set.count.div_ceil(WORKGROUP).max(1), 1, 1);
                }
                for (step, group) in buffers.sort_groups.iter().enumerate() {
                    pass.set_bind_group(0, group, &[sort + step as u32 * PARAMS_SLOT as u32]);
                    pass.set_pipeline(&pipelines.sort_count);
                    pass.dispatch_workgroups(*blocks, 1, 1);
                    pass.set_pipeline(&pipelines.scan_local);
                    pass.dispatch_workgroups(groups, 1, 1);
                    pass.set_pipeline(&pipelines.scan_sums);
                    pass.dispatch_workgroups(1, 1, 1);
                    pass.set_pipeline(&pipelines.scan_add);
                    pass.dispatch_workgroups(groups, 1, 1);
                    pass.set_pipeline(&pipelines.sort_scatter);
                    pass.dispatch_workgroups(*blocks, 1, 1);
                }
            }
            let layer = &self.layers[index];
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render-wgpu splat layer"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &layer.color,
                    depth_slice: None,
                    resolve_target: layer.resolve.as_ref(),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: target.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: timer
                    .filter(|_| index + 1 == runs.len())
                    .and_then(|timer| timer.render_writes_between(false, true)),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_viewport(
                viewport.x as f32,
                viewport.y as f32,
                width,
                height,
                0.0,
                1.0,
            );
            pass.set_pipeline(draw);
            pass.set_bind_group(0, &buffers.draw_group, &[]);
            pass.draw_indirect(&buffers.args, 0);
            drop(pass);
            self.counts.runs += 1;
            self.counts.gaussians += u64::from(*total);
        }
        if !self.timed {
            if let Some(timer) = &mut self.timer {
                timer.resolve(encoder);
            }
            self.timed = true;
        }
    }

    /// Instances a view drew in its runs.
    pub fn drew_instances(&mut self, count: u32) {
        self.counts.instances += count;
    }

    /// After the encoder holding a view's runs was submitted.
    pub fn submitted(&mut self) {
        if let Some(timer) = &mut self.timer {
            timer.submitted();
        }
    }

    /// Composite run `run` of the view's last [`Self::encode_runs`] into a
    /// pass drawing into `format` (the frame's bind group in group 0).
    pub fn draw_composite(&self, pass: &mut wgpu::RenderPass<'_>, format: ColorTarget, run: usize) {
        let (Some(pipeline), Some(layer)) = (self.composites.get(&format), self.layers.get(run))
        else {
            return;
        };
        pass.set_pipeline(pipeline);
        pass.set_bind_group(1, &layer.group, &[]);
        pass.draw(0..3, 0..1);
    }

    fn ensure_params(&mut self, device: &wgpu::Device, slots: u64) {
        if slots <= self.params_slots {
            return;
        }
        self.params_slots = slots.next_power_of_two();
        self.params = params_buffer(device, self.params_slots);
        // The run's bind groups name the old buffer.
        self.run = None;
    }

    fn ensure_run(&mut self, device: &wgpu::Device, gaussians: u32) {
        if self
            .run
            .as_ref()
            .is_some_and(|run| run.capacity >= gaussians)
        {
            return;
        }
        let pipelines = self.pipelines.as_ref().expect("a device that draws splats");
        let capacity = gaussians.next_power_of_two().max(TILE);
        let blocks = capacity.div_ceil(TILE);
        let storage = |label: &str, bytes: u64, usage: wgpu::BufferUsages| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: bytes,
                usage: wgpu::BufferUsages::STORAGE | usage,
                mapped_at_creation: false,
            })
        };
        let words = u64::from(capacity) * 4;
        // Readable, so a test can compare the sort with the CPU's.
        let readable = wgpu::BufferUsages::COPY_SRC;
        let projected = storage(
            "render-wgpu splats projected",
            u64::from(capacity) * PROJECTED_BYTES,
            wgpu::BufferUsages::empty(),
        );
        let keys = [
            storage("render-wgpu splat keys", words, readable),
            storage("render-wgpu splat keys sorted", words, readable),
        ];
        let values = [
            storage("render-wgpu splat values", words, readable),
            storage("render-wgpu splat values sorted", words, readable),
        ];
        let counts = storage(
            "render-wgpu splat digit counts",
            u64::from(DIGITS * blocks) * 4,
            wgpu::BufferUsages::empty(),
        );
        let sums = storage(
            "render-wgpu splat digit sums",
            u64::from(TILE) * 4,
            wgpu::BufferUsages::empty(),
        );
        let args = storage(
            "render-wgpu splat draw",
            16,
            wgpu::BufferUsages::INDIRECT | wgpu::BufferUsages::COPY_DST | readable,
        );
        let params = wgpu::BindingResource::Buffer(wgpu::BufferBinding {
            buffer: &self.params,
            offset: 0,
            size: wgpu::BufferSize::new(PARAMS_SLOT),
        });
        fn entry(binding: u32, resource: wgpu::BindingResource<'_>) -> wgpu::BindGroupEntry<'_> {
            wgpu::BindGroupEntry { binding, resource }
        }
        let project_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("render-wgpu splats project"),
            layout: &pipelines.project_layout,
            entries: &[
                entry(0, params.clone()),
                entry(1, projected.as_entire_binding()),
                entry(2, keys[0].as_entire_binding()),
                entry(3, values[0].as_entire_binding()),
                entry(4, args.as_entire_binding()),
            ],
        });
        let sort_group = |from: usize, to: usize| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("render-wgpu splats sort"),
                layout: &pipelines.sort_layout,
                entries: &[
                    entry(10, params.clone()),
                    entry(11, keys[from].as_entire_binding()),
                    entry(12, values[from].as_entire_binding()),
                    entry(13, keys[to].as_entire_binding()),
                    entry(14, values[to].as_entire_binding()),
                    entry(15, counts.as_entire_binding()),
                    entry(16, sums.as_entire_binding()),
                ],
            })
        };
        let sort_groups = [sort_group(0, 1), sort_group(1, 0)];
        let draw_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("render-wgpu splats draw"),
            layout: &pipelines.draw_layout,
            entries: &[
                entry(20, projected.as_entire_binding()),
                // Two passes sort the keys back into the first pair.
                entry(21, values[0].as_entire_binding()),
            ],
        });
        self.run = Some(RunBuffers {
            capacity,
            #[cfg(test)]
            keys: keys[0].clone(),
            #[cfg(test)]
            values: values[0].clone(),
            args,
            project_group,
            sort_groups,
            draw_group,
        });
    }

    fn ensure_draw(&mut self, device: &wgpu::Device, samples: u32) {
        let pipelines = self.pipelines.as_ref().expect("a device that draws splats");
        self.draws.entry(samples).or_insert_with(|| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("render-wgpu splat layer"),
                layout: Some(&pipelines.draw_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &pipelines.module,
                    entry_point: Some("vs_splat"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleStrip,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(false),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: samples,
                    ..Default::default()
                },
                fragment: Some(wgpu::FragmentState {
                    module: &pipelines.module,
                    entry_point: Some("fs_splat"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: LAYER_FORMAT,
                        blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        });
    }

    /// Layer `index`, made or remade at the target's size.
    fn layer(&mut self, device: &wgpu::Device, index: usize, target: &SplatTarget<'_>) {
        let size = (target.width, target.height, target.samples);
        if self
            .layers
            .get(index)
            .is_some_and(|layer| layer.size == size)
        {
            return;
        }
        let pipelines = self.pipelines.as_ref().expect("a device that draws splats");
        let texture = |samples: u32, label: &str| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width: size.0,
                        height: size.1,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: samples,
                    dimension: wgpu::TextureDimension::D2,
                    format: LAYER_FORMAT,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        let color = texture(size.2, "render-wgpu splat layer");
        let resolve = (size.2 > 1).then(|| texture(1, "render-wgpu splat layer resolved"));
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("render-wgpu splat layer"),
            layout: &pipelines.composite_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 30,
                resource: wgpu::BindingResource::TextureView(resolve.as_ref().unwrap_or(&color)),
            }],
        });
        let layer = Layer {
            size,
            color,
            resolve,
            group,
        };
        if index < self.layers.len() {
            self.layers[index] = layer;
        } else {
            self.layers.push(layer);
        }
    }
}

impl Pipelines {
    fn new(
        device: &wgpu::Device,
        module: wgpu::ShaderModule,
        frame_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let storage = |binding: u32, read_only: bool, visibility: wgpu::ShaderStages| {
            wgpu::BindGroupLayoutEntry {
                binding,
                visibility,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }
        };
        let params = |binding: u32| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: true,
                min_binding_size: wgpu::BufferSize::new(PARAMS_SLOT),
            },
            count: None,
        };
        let compute = wgpu::ShaderStages::COMPUTE;
        let layout = |label: &str, entries: &[wgpu::BindGroupLayoutEntry]| {
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some(label),
                entries,
            })
        };
        let project_layout = layout(
            "render-wgpu splats project",
            &[
                params(0),
                storage(1, false, compute),
                storage(2, false, compute),
                storage(3, false, compute),
                storage(4, false, compute),
            ],
        );
        let gaussian_layout = layout("render-wgpu splat gaussians", &[storage(0, true, compute)]);
        let sort_layout = layout(
            "render-wgpu splats sort",
            &[
                params(10),
                storage(11, true, compute),
                storage(12, true, compute),
                storage(13, false, compute),
                storage(14, false, compute),
                storage(15, false, compute),
                storage(16, false, compute),
            ],
        );
        let draw_layout = layout(
            "render-wgpu splats draw",
            &[
                storage(20, true, wgpu::ShaderStages::VERTEX),
                storage(21, true, wgpu::ShaderStages::VERTEX),
            ],
        );
        let composite_layout = layout(
            "render-wgpu splat composite",
            &[wgpu::BindGroupLayoutEntry {
                binding: 30,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        );
        let pipeline_layout = |label: &str, groups: &[Option<&wgpu::BindGroupLayout>]| {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: groups,
                immediate_size: 0,
            })
        };
        let project_pipeline_layout = pipeline_layout(
            "render-wgpu splats project",
            &[Some(&project_layout), Some(&gaussian_layout)],
        );
        let sort_pipeline_layout =
            pipeline_layout("render-wgpu splats sort", &[Some(&sort_layout)]);
        let draw_pipeline_layout =
            pipeline_layout("render-wgpu splats draw", &[Some(&draw_layout)]);
        let composite_pipeline_layout = pipeline_layout(
            "render-wgpu splat composite",
            &[Some(frame_layout), Some(&composite_layout)],
        );
        let kernel = |layout: &wgpu::PipelineLayout, entry: &str| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry),
                layout: Some(layout),
                module: &module,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let project = kernel(&project_pipeline_layout, "cs_project");
        let sort_count = kernel(&sort_pipeline_layout, "cs_sort_count");
        let scan_local = kernel(&sort_pipeline_layout, "cs_scan_local");
        let scan_sums = kernel(&sort_pipeline_layout, "cs_scan_sums");
        let scan_add = kernel(&sort_pipeline_layout, "cs_scan_add");
        let sort_scatter = kernel(&sort_pipeline_layout, "cs_sort_scatter");
        Self {
            project_layout,
            gaussian_layout,
            sort_layout,
            draw_layout,
            composite_layout,
            draw_pipeline_layout,
            composite_pipeline_layout,
            module,
            project,
            sort_count,
            scan_local,
            scan_sums,
            scan_add,
            sort_scatter,
        }
    }
}

fn params_buffer(device: &wgpu::Device, slots: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("render-wgpu splat parameters"),
        size: slots * PARAMS_SLOT,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn push_matrix(bytes: &mut Vec<u8>, matrix: &DMat4) {
    for value in matrix.as_mat4().to_cols_array() {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
}

/// The GPU words of a splat primitive's Gaussians (`splats.wgsl`
/// `Gaussian`) and their bounds to `BOUNDS_SIGMAS`.
pub(crate) fn gaussian_words(splats: &GlbSplats) -> (Vec<u32>, Aabb) {
    let mut words = Vec::with_capacity(splats.centres.len() * 8);
    let mut bounds = Aabb::EMPTY;
    for (((centre, rotation), scale), color) in splats
        .centres
        .iter()
        .zip(&splats.rotations)
        .zip(&splats.scales)
        .zip(&splats.colors)
    {
        let unorm = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u32;
        let packed_color =
            unorm(color[0]) | unorm(color[1]) << 8 | unorm(color[2]) << 16 | unorm(color[3]) << 24;
        let length = rotation.iter().map(|c| c * c).sum::<f32>().sqrt();
        let rotation = if length > 1e-8 {
            rotation.map(|c| c / length)
        } else {
            [0.0, 0.0, 0.0, 1.0]
        };
        let snorm = |value: f32| ((value.clamp(-1.0, 1.0) * 32767.0).round() as i16) as u16 as u32;
        words.extend_from_slice(&[
            centre[0].to_bits(),
            centre[1].to_bits(),
            centre[2].to_bits(),
            packed_color,
            snorm(rotation[0]) | snorm(rotation[1]) << 16,
            snorm(rotation[2]) | snorm(rotation[3]) << 16,
            u32::from(half_bits(scale[0])) | u32::from(half_bits(scale[1])) << 16,
            u32::from(half_bits(scale[2])),
        ]);
        let reach = BOUNDS_SIGMAS * scale.iter().fold(0.0f32, |a, b| a.max(b.abs()));
        let centre = Vec3::from(*centre);
        bounds.include(centre - Vec3::splat(reach));
        bounds.include(centre + Vec3::splat(reach));
    }
    (words, bounds)
}

/// The IEEE half nearest a non-negative `value` (a standard deviation):
/// tiny values flush toward zero, large ones stop at the largest half.
fn half_bits(value: f32) -> u16 {
    let value = value.abs();
    if !value.is_finite() || value >= 65504.0 {
        return 0x7bff;
    }
    let bits = value.to_bits();
    let exponent = ((bits >> 23) & 0xff) as i32 - 127 + 15;
    let mantissa = bits & 0x7f_ffff;
    if exponent <= 0 {
        // Subnormal half: the value in units of 2^-24.
        return (value * 16_777_216.0).round().min(1023.0) as u16;
    }
    // Round the 23-bit mantissa to 10 bits, carrying into the exponent.
    let rounded = (((exponent as u32) << 10) | (mantissa >> 13)) + ((mantissa >> 12) & 1);
    rounded.min(0x7bff) as u16
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::PixelRect;

    fn read_words(gpu: &Gpu, buffer: &wgpu::Buffer, words: u64) -> Vec<u32> {
        let staging = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("splat test readback"),
            size: words * 4,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        encoder.copy_buffer_to_buffer(buffer, 0, &staging, 0, Some(words * 4));
        gpu.queue.submit([encoder.finish()]);
        staging.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
        let bytes = staging
            .slice(..)
            .get_mapped_range()
            .expect("mapped")
            .to_vec();
        let (words, _) = bytes.as_chunks::<4>();
        words.iter().map(|word| u32::from_le_bytes(*word)).collect()
    }

    /// Gaussians at pseudo-random depths in front of a camera at the origin
    /// looking down −z, a few behind it and a few invisible.
    fn scattered(count: usize, seed: u32) -> GlbSplats {
        let mut state = seed;
        let mut next = || {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (state >> 8) as f32 / (1u32 << 24) as f32
        };
        let mut splats = GlbSplats {
            centres: Vec::new(),
            rotations: Vec::new(),
            scales: Vec::new(),
            colors: Vec::new(),
            joints: None,
            weights: None,
        };
        for index in 0..count {
            // Every 97th behind the camera; every 89th invisible.
            let depth = if index % 97 == 0 {
                -1.0
            } else {
                // Coarse depths, so equal keys show the sort is stable.
                1.0 + (next() * 400.0).floor() * 0.05
            };
            splats
                .centres
                .push([next() * 0.2 - 0.1, next() * 0.2 - 0.1, -depth]);
            splats.rotations.push([0.0, 0.0, 0.0, 1.0]);
            splats.scales.push([0.001; 3]);
            let opacity = if index % 89 == 0 { 0.0 } else { 1.0 };
            splats.colors.push([1.0, 0.5, 0.25, opacity]);
        }
        splats
    }

    /// The GPU's radix sort orders a run of two sets as a stable sort by
    /// key does, the culled Gaussians last, and the draw counts the rest.
    #[test]
    fn a_run_sorts_back_to_front_as_a_stable_cpu_sort_does() {
        let gpu = Gpu::headless().expect("a wgpu adapter");
        let device = &gpu.device;
        let module = crate::pipelines::standard(crate::shaders::Shaders::new().module(
            device,
            render_shaders::Entry::Splats,
            render_shaders::Features::default(),
        ));
        let frame_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[],
        });
        let mut splats = Splats::new(&gpu, module, &frame_layout);
        if let Some(refusal) = &splats.refusal {
            panic!("splats refused on this adapter: {refusal}");
        }
        let first = scattered(40_000, 7);
        let second = scattered(33_123, 11);
        let sets = [
            splats.upload(&gpu, "first", &first).expect("uploaded"),
            splats.upload(&gpu, "second", &second).expect("uploaded"),
        ];
        let depth = device
            .create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width: 64,
                    height: 64,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: DEPTH_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
            .create_view(&Default::default());
        let projection = glam::camera::rh::proj::directx::perspective(1.0, 1.0, 0.1, 100.0);
        let camera = crate::camera::CameraMatrices {
            view_proj: projection,
            view: Mat4::IDENTITY,
            projection,
            eye: Vec3::ZERO,
        };
        let shift = Mat4::from_translation(Vec3::new(0.0, 0.0, -0.025));
        let run = vec![(&sets[0], Mat4::IDENTITY), (&sets[1], shift)];
        let (near, far) = (0.5, 25.0);
        let mut encoder = device.create_command_encoder(&Default::default());
        splats.encode_runs(
            &gpu,
            &mut encoder,
            &[run],
            &[(near, far)],
            &SplatTarget {
                depth: &depth,
                width: 64,
                height: 64,
                samples: 1,
                viewport: PixelRect::whole(64, 64),
                camera,
            },
        );
        gpu.queue.submit([encoder.finish()]);

        let buffers = splats.run.as_ref().expect("a run's buffers");
        let total = first.centres.len() + second.centres.len();
        let keys = read_words(&gpu, &buffers.keys, total as u64);
        let values = read_words(&gpu, &buffers.values, total as u64);
        let drawn = read_words(&gpu, &buffers.args, 4);

        // The CPU's keys: every Gaussian's view depth across the run's span.
        let mut expected: Vec<(u32, u32)> = Vec::with_capacity(total);
        for (slot, (centre, color)) in first
            .centres
            .iter()
            .zip(&first.colors)
            .map(|(c, color)| (*c, color[3]))
            .chain(
                second
                    .centres
                    .iter()
                    .zip(&second.colors)
                    .map(|(c, color)| ([c[0], c[1], c[2] - 0.025], color[3])),
            )
            .enumerate()
        {
            let depth = -centre[2];
            let key = if depth <= 0.0 || color == 0.0 {
                0xffff
            } else {
                let along = ((depth - near) / (far - near)).clamp(0.0, 1.0);
                ((1.0 - along) * 65534.0).round() as u32
            };
            expected.push((key, slot as u32));
        }
        let visible = expected.iter().filter(|(key, _)| *key != 0xffff).count();
        assert_eq!(drawn[0], 4, "four vertices a Gaussian");
        assert_eq!(drawn[1] as usize, visible, "the draw counts the visible");
        expected.sort_by_key(|&(key, _)| key);
        let gpu_order: Vec<(u32, u32)> = keys.iter().copied().zip(values.iter().copied()).collect();
        // Keys can differ from the CPU's by a rounding step; the order must
        // be ascending, stable and hold every slot once.
        for pair in gpu_order.windows(2) {
            let ((key_a, value_a), (key_b, value_b)) = (pair[0], pair[1]);
            assert!(
                key_a < key_b || (key_a == key_b && value_a < value_b),
                "sorted stably: {:?} then {:?}",
                pair[0],
                pair[1]
            );
        }
        let mut seen = vec![false; total];
        for &(_, value) in &gpu_order {
            assert!(
                !std::mem::replace(&mut seen[value as usize], true),
                "slot {value} once"
            );
        }
        let mut gpu_keys = vec![0u32; total];
        for &(key, value) in &gpu_order {
            gpu_keys[value as usize] = key;
        }
        for &(key, slot) in &expected {
            let gpu_key = gpu_keys[slot as usize];
            assert!(
                gpu_key.abs_diff(key) <= 1,
                "slot {slot}: {gpu_key} against {key}"
            );
        }
    }
}
