//! Skinning on the GPU (`skin.wgsl`): each animated instance's skinned
//! primitive is posed from its joint palette by a compute pass, into the
//! instance's own vertex buffer, which the draw, shadow and ray paths read
//! as they read any mesh.
//!
//! **When.** `animated.rs` queues each skinned primitive whose instance has
//! a new pose to show; [`Skinning::flush`] writes the frame's palettes and
//! job table and poses them all in one pass, submitted before anything else
//! the frame encodes (BLAS builds, culling, views, captures). A held pose
//! costs nothing.
//!
//! **Where.** Skinned meshes' vertices, extra streams and motion history
//! are ranges of shared arena buffers, which the pass writes in place and
//! draws, BLAS builds and the ray pool read at the range's offset. So the
//! primitives skinned from the same rest data into the same arena buffer
//! are one dispatch: a dispatch per mesh ran one after another, about 3 µs
//! each on RADV and 7 µs on an RTX 3080 however small, and copying a staging
//! buffer into each mesh's own cost about 2 µs a mesh on the 3080.
//!
//! **What stays on the CPU.** Per pose: each joint's palette matrix, the
//! posed bounds from each joint's box at rest (`SkinSource::joints`), and
//! a job in the table. Posed positions on the CPU are made only when a reading
//! asks for them (`Renderer::posed_geometry`): a probe bake, a ghost plate's
//! bounds, a product's bounds request. A device without compute shaders,
//! or `RendererOptions::cpu_skinning`, skins on the CPU as before.

use std::sync::{Arc, Mutex};

use glam::Mat4;

use crate::glb::GlbPrimitive;
use crate::pipelines::{EXTRA_VERTEX_FLOATS, VERTEX_FLOATS};
use crate::tables::{Aabb, VertexStream};
use crate::timing::PassTimer;
use crate::Gpu;

/// Threads per workgroup (`skin.wgsl` `cs_skin`).
const WORKGROUP: u32 = 64;
/// Workgroups per row of a dispatch; a larger primitive takes more rows.
const ROW_GROUPS: u32 = 65_535;
/// Words per vertex in the skin stream: two of packed joints, four weights.
const SKIN_WORDS: usize = 6;
/// Words per job in the frame's table (`skin.wgsl` `JOB_WORDS`).
const JOB_WORDS: usize = 9;
const VERTEX_BYTES: u64 = (VERTEX_FLOATS * 4) as u64;
const EXTRA_BYTES: u64 = (EXTRA_VERTEX_FLOATS * 4) as u64;
/// `skin.wgsl` `SkinParams`.
const PARAMS_BYTES: u64 = 32;
/// An arena's first buffer, and the largest it makes.
const CHUNK_BYTES: u64 = 4 << 20;
const MAX_CHUNK_BYTES: u64 = 64 << 20;

/// A skinned primitive at rest, uploaded once per asset and read by every
/// instance's dispatch.
pub(crate) struct SkinSource {
    /// The primitive in the renderer's vertex layout, its `COLOR_0` colours.
    pub vertices: wgpu::Buffer,
    /// The same with a splat core's lit colours (`GlbCore::albedo`).
    pub lit: Option<wgpu::Buffer>,
    /// Joints and weights (`SKIN_WORDS` per vertex).
    pub skin: wgpu::Buffer,
    /// The extra stream at rest (tangent, uv1), when the primitive draws one.
    pub extra: Option<wgpu::Buffer>,
    /// Whether the extra stream's tangents are skinned (the primitive has
    /// tangents) rather than the constant +X.
    pub tangents: bool,
    /// Each skin joint's box over the vertices it weights, at rest: a
    /// skinned vertex is a weighted mean of its joints' placements of it, so
    /// it lies within the union of these boxes as the palette places them.
    pub joints: Vec<Aabb>,
}

impl SkinSource {
    /// `primitive`'s posed bounds under `palette`.
    pub fn bounds(&self, palette: &[Mat4]) -> Aabb {
        let mut bounds = Aabb::EMPTY;
        for (joint, matrix) in self.joints.iter().zip(palette) {
            bounds = bounds.union(&joint.transformed(matrix));
        }
        bounds
    }
}

/// What a skinned primitive's instance keeps for its dispatches: its
/// copies' source vertices and palette sets, and the palettes it was posed
/// with.
#[derive(Default)]
pub(crate) struct SkinTarget {
    /// Per copy after the primitive's own vertices: its source vertex and
    /// palette set; none without cuts.
    pub copies: Option<Arc<[u32]>>,
    /// The palettes its current vertices were skinned with.
    pub pose: Option<Arc<SkinPose>>,
    /// The palettes of its previous vertices (`SkinnedHistory::previous`),
    /// while it has them.
    pub previous_pose: Option<Arc<SkinPose>>,
}

/// The palette sets a skinned primitive was posed with: the instance's,
/// then each severed piece's, `joints` matrices each.
pub(crate) struct SkinPose {
    pub palettes: Vec<Mat4>,
    pub joints: usize,
}

/// A range of an arena buffer, held by the streams in it and returned to
/// the arena when the last of them drops.
pub(crate) struct Slot {
    chunks: Arc<Mutex<Vec<Chunk>>>,
    chunk: usize,
    offset: u64,
    size: u64,
}

impl Drop for Slot {
    fn drop(&mut self) {
        let mut chunks = self
            .chunks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(chunk) = chunks.get_mut(self.chunk) {
            chunk.release(self.offset, self.size);
        }
    }
}

/// One buffer of an arena and its free ranges, in offset order.
struct Chunk {
    buffer: wgpu::Buffer,
    free: Vec<(u64, u64)>,
}

impl Chunk {
    /// The first free range of `size` bytes, taken.
    fn take(&mut self, size: u64) -> Option<u64> {
        let at = self.free.iter().position(|(_, free)| *free >= size)?;
        let (offset, free) = self.free[at];
        if free == size {
            self.free.remove(at);
        } else {
            self.free[at] = (offset + size, free - size);
        }
        Some(offset)
    }

    /// A range given back, joined to its free neighbours.
    fn release(&mut self, offset: u64, size: u64) {
        let at = self.free.partition_point(|(free, _)| *free < offset);
        self.free.insert(at, (offset, size));
        if at + 1 < self.free.len() && offset + size == self.free[at + 1].0 {
            self.free[at].1 += self.free[at + 1].1;
            self.free.remove(at + 1);
        }
        if at > 0 && self.free[at - 1].0 + self.free[at - 1].1 == offset {
            self.free[at - 1].1 += self.free[at].1;
            self.free.remove(at);
        }
    }
}

/// Buffers that skinned meshes' streams share, so one dispatch writes many
/// meshes in place: each allocation a whole number of `unit` bytes (one
/// vertex), so a range's first vertex is an index into its buffer.
struct Arena {
    label: &'static str,
    unit: u64,
    chunks: Arc<Mutex<Vec<Chunk>>>,
}

impl Arena {
    fn new(label: &'static str, unit: u64) -> Self {
        Self {
            label,
            unit,
            chunks: Arc::default(),
        }
    }

    /// A stream of `units` vertices; none when the device has no room.
    fn allocate(&self, device: &wgpu::Device, units: u64) -> Option<VertexStream> {
        let size = units.max(1) * self.unit;
        let mut chunks = self
            .chunks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let found = chunks
            .iter_mut()
            .enumerate()
            .find_map(|(index, chunk)| Some((index, chunk.take(size)?)));
        let (chunk, offset) = match found {
            Some(found) => found,
            None => {
                // Each new buffer twice the last, from 4 MiB to 64 MiB, or
                // the one range when it is larger.
                let limits = device.limits();
                let largest = limits
                    .max_buffer_size
                    .min(limits.max_storage_buffer_binding_size);
                let grown = (CHUNK_BYTES << chunks.len().min(4)).min(MAX_CHUNK_BYTES);
                let bytes = (grown.max(size).min(largest) / self.unit) * self.unit;
                if bytes < size {
                    return None;
                }
                let buffer = crate::gpu::allocating(device, || {
                    device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some(self.label),
                        size: bytes,
                        usage: wgpu::BufferUsages::VERTEX
                            | wgpu::BufferUsages::STORAGE
                            | wgpu::BufferUsages::COPY_SRC
                            | crate::ray_scene::mesh_input(device),
                        mapped_at_creation: false,
                    })
                })
                .ok()?;
                let mut chunk = Chunk {
                    buffer,
                    free: vec![(0, bytes)],
                };
                let offset = chunk.take(size)?;
                chunks.push(chunk);
                (chunks.len() - 1, offset)
            }
        };
        Some(VertexStream {
            buffer: chunks[chunk].buffer.clone(),
            offset,
            size,
            _slot: Some(Arc::new(Slot {
                chunks: self.chunks.clone(),
                chunk,
                offset,
                size,
            })),
        })
    }
}

struct Kernel {
    pipeline: wgpu::ComputePipeline,
    frame_layout: wgpu::BindGroupLayout,
    source_layout: wgpu::BindGroupLayout,
    /// Bound where a primitive has no extra stream.
    empty: wgpu::Buffer,
    /// Bound where a dispatch writes no extra stream.
    empty_output: wgpu::Buffer,
    /// Bytes between params entries (the uniform offset alignment).
    stride: u64,
}

/// A queued primitive: what it is posed from and with, and where it goes.
struct Job {
    vertices: wgpu::Buffer,
    skin: wgpu::Buffer,
    source_extra: Option<wgpu::Buffer>,
    own: u32,
    palette_first: u32,
    joints: u32,
    copies: Option<Arc<[u32]>>,
    tangents: bool,
    output: VertexStream,
    output_extra: Option<VertexStream>,
}

impl Job {
    fn outputs(&self) -> u32 {
        (self.output.size / VERTEX_BYTES) as u32
    }

    /// What it shares a dispatch with others by: its rest data and the
    /// arena buffers it writes.
    fn dispatch_key(&self) -> (&wgpu::Buffer, &wgpu::Buffer, Option<&wgpu::Buffer>) {
        (
            &self.vertices,
            &self.output.buffer,
            self.output_extra.as_ref().map(|extra| &extra.buffer),
        )
    }
}

/// The frame's buffers the pass reads, grown as needed.
struct FrameBuffers {
    palettes: wgpu::Buffer,
    table: wgpu::Buffer,
    params: wgpu::Buffer,
    group: Option<wgpu::BindGroup>,
}

pub(crate) struct Skinning {
    /// None on a device without compute shaders, which skins on the CPU.
    kernel: Option<Kernel>,
    jobs: Vec<Job>,
    /// The queued primitives' palettes, three rows per joint.
    palettes: Vec<[f32; 4]>,
    frame: FrameBuffers,
    vertices: Arena,
    extra: Arena,
    timer: Option<PassTimer>,
}

impl Skinning {
    pub fn new(gpu: &Gpu, module: impl FnOnce() -> wgpu::ShaderModule) -> Self {
        let device = &gpu.device;
        let refusal = gpu.compute_refusal([WORKGROUP, 1, 1], 0).or_else(|| {
            (device.limits().max_storage_buffers_per_shader_stage < 7)
                .then(|| "the device binds fewer than 7 storage buffers to a kernel".to_owned())
        });
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
        let kernel = refusal.is_none().then(|| {
            let frame_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("render-wgpu skin frame"),
                entries: &[
                    storage(0, true),
                    storage(1, true),
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: true,
                            min_binding_size: wgpu::BufferSize::new(PARAMS_BYTES),
                        },
                        count: None,
                    },
                ],
            });
            let source_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("render-wgpu skin source"),
                entries: &[
                    storage(0, true),
                    storage(1, true),
                    storage(2, true),
                    storage(3, false),
                    storage(4, false),
                ],
            });
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("render-wgpu skin"),
                bind_group_layouts: &[Some(&frame_layout), Some(&source_layout)],
                immediate_size: 0,
            });
            let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("render-wgpu skin"),
                layout: Some(&layout),
                module: &module(),
                entry_point: Some("cs_skin"),
                compilation_options: Default::default(),
                cache: None,
            });
            let empty = |label| {
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(label),
                    size: 16,
                    usage: wgpu::BufferUsages::STORAGE,
                    mapped_at_creation: false,
                })
            };
            Kernel {
                pipeline,
                frame_layout,
                source_layout,
                empty: empty("render-wgpu skin empty"),
                empty_output: empty("render-wgpu skin empty output"),
                stride: u64::from(device.limits().min_uniform_buffer_offset_alignment)
                    .max(PARAMS_BYTES),
            }
        });
        Self {
            frame: FrameBuffers {
                palettes: storage_buffer(device, "render-wgpu skin palettes", 4096),
                table: storage_buffer(device, "render-wgpu skin table", 4096),
                params: uniform_buffer(device, 4096),
                group: None,
            },
            timer: kernel
                .as_ref()
                .and_then(|_| PassTimer::new(gpu, "skinning")),
            kernel,
            jobs: Vec::new(),
            palettes: Vec::new(),
            vertices: Arena::new("render-wgpu skinned vertices", VERTEX_BYTES),
            extra: Arena::new("render-wgpu skinned extra", EXTRA_BYTES),
        }
    }

    /// Whether this device skins on the GPU.
    pub fn available(&self) -> bool {
        self.kernel.is_some()
    }

    pub fn timing(&self) -> crate::timing::GpuPassTiming {
        self.timer
            .as_ref()
            .map_or_else(|| crate::timing::untimed("skinning"), PassTimer::readout)
    }

    /// Room in the arena for `count` skinned vertices; none when the device
    /// has none.
    pub fn vertices(&self, device: &wgpu::Device, count: usize) -> Option<VertexStream> {
        self.vertices.allocate(device, count as u64)
    }

    /// Room in the arena for `count` vertices' extra stream.
    pub fn extra(&self, device: &wgpu::Device, count: usize) -> Option<VertexStream> {
        self.extra.allocate(device, count as u64)
    }

    /// Upload `primitive` at rest for skinning on the GPU, with the colours
    /// a lit splat core draws when it has them. Refused when the device has
    /// no room; the primitive is then skinned on the CPU.
    pub fn source(
        &self,
        gpu: &Gpu,
        label: &str,
        primitive: &GlbPrimitive,
        rest: &[f32],
        lit: Option<&[f32]>,
        extra: Option<&[f32]>,
    ) -> Option<SkinSource> {
        self.kernel.as_ref()?;
        let (joints, weights) = (primitive.joints.as_ref()?, primitive.weights.as_ref()?);
        let mut skin = Vec::with_capacity(joints.len() * SKIN_WORDS);
        for (joints, weights) in joints.iter().zip(weights) {
            skin.push(u32::from(joints[0]) | u32::from(joints[1]) << 16);
            skin.push(u32::from(joints[2]) | u32::from(joints[3]) << 16);
            skin.extend(weights.map(f32::to_bits));
        }
        let boxes = joint_boxes(primitive);
        let device = &gpu.device;
        let upload = |data: &[u8]| {
            crate::gpu::buffer_init(
                device,
                &wgpu::util::BufferInitDescriptor {
                    label: Some(label),
                    contents: data,
                    usage: wgpu::BufferUsages::STORAGE,
                },
            )
        };
        crate::gpu::allocating(device, || SkinSource {
            vertices: upload(bytemuck::cast_slice(rest)),
            lit: lit.map(|lit| upload(bytemuck::cast_slice(lit))),
            skin: upload(bytemuck::cast_slice(&skin)),
            extra: extra.map(|extra| upload(bytemuck::cast_slice(extra))),
            tangents: primitive.tangents.is_some(),
            joints: boxes,
        })
        .ok()
    }

    /// Queue `vertices` (and `extra`, when given) of a skinned primitive,
    /// arena streams, to be posed from `source`, with its lit colours when
    /// `lit`, by `pose`'s palette sets: its own `own` vertices by the first,
    /// each copy by the set `copies` names.
    #[allow(clippy::too_many_arguments)]
    pub fn queue(
        &mut self,
        source: &SkinSource,
        lit: bool,
        copies: Option<&Arc<[u32]>>,
        pose: &SkinPose,
        own: u32,
        vertices: &VertexStream,
        extra: Option<&VertexStream>,
    ) {
        if self.kernel.is_none() {
            return;
        }
        let palette_first = (self.palettes.len() / 3) as u32;
        for matrix in &pose.palettes {
            self.palettes
                .extend([matrix.row(0), matrix.row(1), matrix.row(2)].map(|row| row.to_array()));
        }
        let output_extra = extra.filter(|_| source.extra.is_some());
        // A newer pose for the same vertices replaces one queued before it.
        self.jobs.retain(|job| !job.output.same(vertices));
        self.jobs.push(Job {
            vertices: match (&source.lit, lit) {
                (Some(lit), true) => lit.clone(),
                _ => source.vertices.clone(),
            },
            skin: source.skin.clone(),
            source_extra: output_extra.and(source.extra.clone()),
            own,
            palette_first,
            joints: pose.joints as u32,
            copies: copies.cloned(),
            tangents: source.tangents,
            output: vertices.clone(),
            output_extra: output_extra.cloned(),
        });
    }

    /// Pose every queued primitive in place: one dispatch per rest data
    /// and arena buffers they share, all in one pass, submitted at once.
    pub fn flush(&mut self, gpu: &Gpu) {
        let Some(kernel) = &self.kernel else {
            return;
        };
        if self.jobs.is_empty() {
            return;
        }
        let device = &gpu.device;
        if let Some(timer) = &mut self.timer {
            timer.collect(gpu);
        }
        self.jobs
            .sort_by(|a, b| a.dispatch_key().cmp(&b.dispatch_key()));
        let mut records = Vec::with_capacity(self.jobs.len() * JOB_WORDS);
        let mut workgroup_jobs = Vec::new();
        let mut copies: Vec<u32> = Vec::new();
        let mut job_copies = Vec::with_capacity(self.jobs.len());
        // Each dispatch: its first job, first workgroup and workgroups.
        let mut dispatches: Vec<(usize, u32, u32)> = Vec::new();
        for (index, job) in self.jobs.iter().enumerate() {
            let first_workgroup = workgroup_jobs.len() as u32;
            let workgroups = job.outputs().div_ceil(WORKGROUP);
            workgroup_jobs.extend(std::iter::repeat_n(index as u32, workgroups as usize));
            match dispatches.last_mut() {
                Some((first, _, count))
                    if self.jobs[*first].dispatch_key() == job.dispatch_key() =>
                {
                    *count += workgroups;
                }
                _ => dispatches.push((index, first_workgroup, workgroups)),
            }
            job_copies.push(copies.len() as u32);
            if let Some(job_copies) = &job.copies {
                copies.extend_from_slice(job_copies);
            }
            let flags = u32::from(job.output_extra.is_some()) | u32::from(job.tangents) << 1;
            records.extend([
                job.outputs(),
                job.own,
                job.palette_first,
                job.joints,
                (job.output.offset / VERTEX_BYTES) as u32,
                job.output_extra
                    .as_ref()
                    .map_or(0, |extra| (extra.offset / EXTRA_BYTES) as u32),
                0,
                flags,
                first_workgroup,
            ]);
        }
        // Each job's first copy, after the jobs and the workgroups' jobs.
        let groups_at = records.len() as u32;
        let copies_start = groups_at + workgroup_jobs.len() as u32;
        for (index, first) in job_copies.into_iter().enumerate() {
            records[index * JOB_WORDS + 6] = copies_start + first;
        }
        let mut table = records;
        table.extend_from_slice(&workgroup_jobs);
        table.extend_from_slice(&copies);

        let frame = &mut self.frame;
        let grow = |buffer: &mut wgpu::Buffer, bytes: u64, make: &dyn Fn(u64) -> wgpu::Buffer| {
            if bytes > buffer.size() {
                *buffer = make(bytes.next_power_of_two());
                true
            } else {
                false
            }
        };
        let mut grown = grow(
            &mut frame.palettes,
            self.palettes.len() as u64 * 16,
            &|size| storage_buffer(device, "render-wgpu skin palettes", size),
        );
        grown |= grow(&mut frame.table, table.len() as u64 * 4, &|size| {
            storage_buffer(device, "render-wgpu skin table", size)
        });
        grown |= grow(
            &mut frame.params,
            dispatches.len() as u64 * kernel.stride,
            &|size| uniform_buffer(device, size),
        );
        if grown {
            frame.group = None;
        }
        let frame_group = frame.group.get_or_insert_with(|| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("render-wgpu skin frame"),
                layout: &kernel.frame_layout,
                entries: &[
                    entry(0, &frame.palettes),
                    entry(1, &frame.table),
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &frame.params,
                            offset: 0,
                            size: wgpu::BufferSize::new(PARAMS_BYTES),
                        }),
                    },
                ],
            })
        });
        let mut params = vec![0u8; dispatches.len() * kernel.stride as usize];
        for (index, (_, first_workgroup, workgroups)) in dispatches.iter().enumerate() {
            let words: [u32; 8] = [
                *first_workgroup,
                (*workgroups).min(ROW_GROUPS) * WORKGROUP,
                0,
                groups_at,
                *workgroups,
                0,
                0,
                0,
            ];
            let at = index * kernel.stride as usize;
            params[at..at + PARAMS_BYTES as usize].copy_from_slice(bytemuck::cast_slice(&words));
        }
        gpu.queue
            .write_buffer(&frame.palettes, 0, bytemuck::cast_slice(&self.palettes));
        gpu.queue
            .write_buffer(&frame.table, 0, bytemuck::cast_slice(&table));
        gpu.queue.write_buffer(&frame.params, 0, &params);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("render-wgpu skin"),
        });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("render-wgpu skin"),
                timestamp_writes: self.timer.as_ref().and_then(PassTimer::compute_writes),
            });
            pass.set_pipeline(&kernel.pipeline);
            for (index, (first, _, workgroups)) in dispatches.iter().enumerate() {
                let job = &self.jobs[*first];
                let source = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("render-wgpu skin source"),
                    layout: &kernel.source_layout,
                    entries: &[
                        entry(0, &job.vertices),
                        entry(1, &job.skin),
                        entry(2, job.source_extra.as_ref().unwrap_or(&kernel.empty)),
                        entry(3, &job.output.buffer),
                        entry(
                            4,
                            job.output_extra
                                .as_ref()
                                .map_or(&kernel.empty_output, |extra| &extra.buffer),
                        ),
                    ],
                });
                pass.set_bind_group(0, &*frame_group, &[(index as u64 * kernel.stride) as u32]);
                pass.set_bind_group(1, &source, &[]);
                pass.dispatch_workgroups(
                    (*workgroups).min(ROW_GROUPS),
                    workgroups.div_ceil(ROW_GROUPS),
                    1,
                );
            }
        }
        if let Some(timer) = &mut self.timer {
            timer.resolve(&mut encoder);
        }
        gpu.queue.submit([encoder.finish()]);
        if let Some(timer) = &mut self.timer {
            timer.submitted();
        }
        self.jobs.clear();
        self.palettes.clear();
    }
}

/// Each joint's box over the rest positions of the vertices it weights,
/// by skin joint index (`SkinSource::joints`).
fn joint_boxes(primitive: &GlbPrimitive) -> Vec<Aabb> {
    let (Some(joints), Some(weights)) = (&primitive.joints, &primitive.weights) else {
        return Vec::new();
    };
    let count = joints
        .iter()
        .flatten()
        .map(|joint| usize::from(*joint) + 1)
        .max()
        .unwrap_or(0);
    let mut boxes = vec![Aabb::EMPTY; count];
    for ((joints, weights), position) in joints.iter().zip(weights).zip(&primitive.positions) {
        for (joint, weight) in joints.iter().zip(weights) {
            if *weight > 0.0 {
                boxes[usize::from(*joint)].include(crate::convert::vec3(*position));
            }
        }
    }
    boxes
}

fn entry(binding: u32, buffer: &wgpu::Buffer) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry {
        binding,
        resource: buffer.as_entire_binding(),
    }
}

fn storage_buffer(device: &wgpu::Device, label: &str, size: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn uniform_buffer(device: &wgpu::Device, size: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("render-wgpu skin params"),
        size,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

#[cfg(test)]
mod tests {
    use glam::{Quat, Vec3};

    use super::*;

    /// A vertex skinned by any blend of its joints lies within the union of
    /// its joints' boxes as the palette places them: the bounds a pose
    /// culls and fits shadows by hold every vertex the CPU skins.
    #[test]
    fn posed_joint_boxes_hold_every_skinned_vertex() {
        let mut seed = 0x2545_f491_u32;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed as f32 / u32::MAX as f32
        };
        let joints = 6u16;
        let count = 400;
        let mut primitive = GlbPrimitive {
            positions: Vec::new(),
            normals: Vec::new(),
            uvs: None,
            uvs1: None,
            tangents: None,
            colors: None,
            joints: Some(Vec::new()),
            weights: Some(Vec::new()),
            indices: Vec::new(),
            material: None,
            core: None,
        };
        for _ in 0..count {
            primitive
                .positions
                .push([next() * 2.0 - 1.0, next() * 3.0, next() - 0.5]);
            primitive.normals.push([0.0, 1.0, 0.0]);
            let picked = [0, 1, 2, 3].map(|_| (next() * f32::from(joints)) as u16 % joints);
            let raw = [next(), next(), next() * 0.2, 0.0];
            let sum: f32 = raw.iter().sum();
            primitive.joints.as_mut().unwrap().push(picked);
            primitive
                .weights
                .as_mut()
                .unwrap()
                .push(raw.map(|weight| weight / sum));
        }
        let boxes = joint_boxes(&primitive);
        for _ in 0..20 {
            let palette: Vec<Mat4> = (0..joints)
                .map(|_| {
                    Mat4::from_scale_rotation_translation(
                        Vec3::splat(0.5 + next()),
                        Quat::from_euler(
                            glam::EulerRot::YXZ,
                            next() * 6.0,
                            next() * 6.0,
                            next() * 6.0,
                        ),
                        Vec3::new(next() * 4.0 - 2.0, next() * 4.0, next() * 4.0 - 2.0),
                    )
                })
                .collect();
            let mut bounds = Aabb::EMPTY;
            for (joint, matrix) in boxes.iter().zip(&palette) {
                bounds = bounds.union(&joint.transformed(matrix));
            }
            let slack = Vec3::splat(1e-4);
            for vertex in 0..count {
                let joints = primitive.joints.as_ref().unwrap()[vertex];
                let weights = primitive.weights.as_ref().unwrap()[vertex];
                let mut matrix = Mat4::ZERO;
                for influence in 0..4 {
                    if weights[influence] > 0.0 {
                        matrix += palette[usize::from(joints[influence])] * weights[influence];
                    }
                }
                let placed = matrix.transform_point3(Vec3::from(primitive.positions[vertex]));
                assert!(
                    placed.cmpge(bounds.min - slack).all()
                        && placed.cmple(bounds.max + slack).all(),
                    "{placed} outside {bounds:?}"
                );
            }
        }
    }
}
