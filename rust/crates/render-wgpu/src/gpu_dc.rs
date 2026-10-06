//! EXPLORE #9513: dual contouring of coarse voxel lattices on the GPU, to
//! measure what a drawn-only level-of-detail mesher would cost against the
//! CPU one. Not a renderer feature: the vertices and indices stay in storage
//! buffers and nothing draws them.
//!
//! Two passes over a batch of chunks, one dispatch per chunk through a
//! dynamic uniform offset: `cs_cells` places a vertex in every cell an edge
//! crossing touches (the crossings' mass point, the summed trilinear
//! gradient as its normal, the majority inside material), `cs_edges` emits
//! two triangles for every crossing edge whose inside sample lies in the
//! chunk's own region, winding by the crossing's direction and splitting the
//! quad on its shorter diagonal, appending indices through an atomic counter.
//! No QEF, jitter, skirts, texture tiles, terrain layers or cube faces.

use std::time::Instant;

use crate::timing::PassTimer;
use crate::Gpu;

/// A coarse lattice as `svc-mesh` dumps it: samples with a two-voxel halo
/// in coarse units, and the chunk's own region in the lattice's coordinates.
pub struct Lattice {
    pub dims: [u32; 3],
    /// The chunk's own cells, as sample index ranges `[min, max)`.
    pub owner_min: [i32; 3],
    pub owner_max: [i32; 3],
    pub values: Vec<f32>,
    pub materials: Vec<u32>,
    /// What the CPU mesher made of it, before skirts.
    pub cpu_triangles: u32,
    pub cpu_vertices: u32,
}

impl Lattice {
    /// Read a `.lat` dump: dims (3 × u32), origin (3 × i64), owner min and
    /// max (6 × i64), values (f32), materials (u16), then the CPU triangle
    /// and vertex counts (2 × u32).
    pub fn read(bytes: &[u8]) -> Option<Self> {
        let mut at = 0;
        let mut take = |n: usize| {
            let slice = bytes.get(at..at + n)?;
            at += n;
            Some(slice)
        };
        let u32_at = |s: &[u8]| u32::from_le_bytes(s.try_into().unwrap());
        let i64_at = |s: &[u8]| i64::from_le_bytes(s.try_into().unwrap());
        let dims = [u32_at(take(4)?), u32_at(take(4)?), u32_at(take(4)?)];
        let origin = [i64_at(take(8)?), i64_at(take(8)?), i64_at(take(8)?)];
        let owner_min = [i64_at(take(8)?), i64_at(take(8)?), i64_at(take(8)?)];
        let owner_max = [i64_at(take(8)?), i64_at(take(8)?), i64_at(take(8)?)];
        let count = (dims[0] * dims[1] * dims[2]) as usize;
        let values = take(count * 4)?
            .chunks_exact(4)
            .map(|v| f32::from_le_bytes(v.try_into().unwrap()))
            .collect();
        let materials = take(count * 2)?
            .chunks_exact(2)
            .map(|v| u32::from(u16::from_le_bytes(v.try_into().unwrap())))
            .collect();
        let cpu_triangles = u32_at(take(4)?);
        let cpu_vertices = u32_at(take(4)?);
        Some(Self {
            dims,
            owner_min: [0, 1, 2].map(|a| (owner_min[a] - origin[a]) as i32),
            owner_max: [0, 1, 2].map(|a| (owner_max[a] - origin[a]) as i32),
            values,
            materials,
            cpu_triangles,
            cpu_vertices,
        })
    }

    fn cells(&self) -> u32 {
        (self.dims[0] - 1) * (self.dims[1] - 1) * (self.dims[2] - 1)
    }
}

/// One batch's outcome.
#[derive(Debug, Clone, Default)]
pub struct Report {
    pub chunks: usize,
    pub upload_bytes: u64,
    /// Lattice bytes written to the device, as the renderer would each
    /// rebuild.
    pub cells_gpu_ms: f64,
    pub edges_gpu_ms: f64,
    /// Wall time of upload, dispatch, submit and the count readback.
    pub wall_ms: f64,
    pub gpu_vertices: u64,
    pub gpu_triangles: u64,
    pub cpu_vertices: u64,
    pub cpu_triangles: u64,
    /// Chunks whose index region overflowed.
    pub overflowed: u32,
}

const CHUNK_UNIFORM_STRIDE: u64 = 256;
/// Indices a cell may emit at most: three edges, six indices each.
const INDICES_PER_CELL: u32 = 18;
const WORKGROUP: u32 = 64;

pub struct GpuDualContouring {
    cells: wgpu::ComputePipeline,
    edges: wgpu::ComputePipeline,
    layout: wgpu::BindGroupLayout,
}

impl GpuDualContouring {
    pub fn new(gpu: &Gpu) -> Self {
        let device = &gpu.device;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("explore gpu dual contouring"),
            source: wgpu::ShaderSource::Wgsl(include_str!("gpu_dc.wgsl").into()),
        });
        let storage = |binding: u32, read_only: bool| wgpu::BindGroupLayoutEntry {
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
            label: Some("explore gpu dual contouring"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: None,
                    },
                    count: None,
                },
                storage(1, true),
                storage(2, true),
                storage(3, false),
                storage(4, false),
                storage(5, false),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("explore gpu dual contouring"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = |entry: &str| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry),
                layout: Some(&pipeline_layout),
                module: &shader,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        Self {
            cells: pipeline("cs_cells"),
            edges: pipeline("cs_edges"),
            layout,
        }
    }

    /// Mesh every lattice once, as one batch: upload, two dispatches a chunk,
    /// a readback of the counts.
    pub fn mesh(&self, gpu: &Gpu, lattices: &[Lattice]) -> Report {
        let device = &gpu.device;
        let started = Instant::now();
        // Concatenate the lattices; each chunk's uniform names its offsets.
        let mut values: Vec<f32> = Vec::new();
        let mut materials: Vec<u32> = Vec::new();
        let mut uniforms: Vec<u8> = Vec::new();
        let mut vertex_total = 0_u32;
        let mut index_total = 0_u32;
        let mut cpu_vertices = 0_u64;
        let mut cpu_triangles = 0_u64;
        for (chunk_index, lattice) in lattices.iter().enumerate() {
            let cells = lattice.cells();
            let mut uniform = Vec::with_capacity(CHUNK_UNIFORM_STRIDE as usize);
            for v in [
                lattice.dims[0],
                lattice.dims[1],
                lattice.dims[2],
                values.len() as u32,
                vertex_total,
                index_total,
                cells * INDICES_PER_CELL,
                chunk_index as u32,
            ] {
                uniform.extend_from_slice(&v.to_le_bytes());
            }
            for v in lattice.owner_min.iter().chain([&0]) {
                uniform.extend_from_slice(&v.to_le_bytes());
            }
            for v in lattice.owner_max.iter().chain([&0]) {
                uniform.extend_from_slice(&v.to_le_bytes());
            }
            uniform.resize(CHUNK_UNIFORM_STRIDE as usize, 0);
            uniforms.extend_from_slice(&uniform);
            values.extend_from_slice(&lattice.values);
            materials.extend_from_slice(&lattice.materials);
            vertex_total += cells;
            index_total += cells * INDICES_PER_CELL;
            cpu_vertices += u64::from(lattice.cpu_vertices);
            cpu_triangles += u64::from(lattice.cpu_triangles);
        }
        let upload_bytes = (values.len() * 4 + materials.len() * 4 + uniforms.len()) as u64;
        let buffer = |label: &str, size: u64, usage: wgpu::BufferUsages| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: size.max(16),
                usage,
                mapped_at_creation: false,
            })
        };
        let uniform_buffer = buffer(
            "chunks",
            uniforms.len() as u64,
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        );
        let values_buffer = buffer(
            "values",
            (values.len() * 4) as u64,
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        );
        let materials_buffer = buffer(
            "materials",
            (materials.len() * 4) as u64,
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        );
        // Vertex: position (3), material, normal (3), active flag: 32 bytes.
        let vertices_buffer = buffer(
            "vertices",
            u64::from(vertex_total) * 32,
            wgpu::BufferUsages::STORAGE,
        );
        let indices_buffer = buffer(
            "indices",
            u64::from(index_total) * 4,
            wgpu::BufferUsages::STORAGE,
        );
        // Per chunk: index count, vertex count, overflow flag, pad.
        let counters_buffer = buffer(
            "counters",
            lattices.len() as u64 * 16,
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
        );
        let readback = buffer(
            "counters readback",
            lattices.len() as u64 * 16,
            wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        );
        gpu.queue.write_buffer(&uniform_buffer, 0, &uniforms);
        gpu.queue.write_buffer(&values_buffer, 0, bytemuck::cast_slice(&values));
        gpu.queue.write_buffer(&materials_buffer, 0, bytemuck::cast_slice(&materials));
        gpu.queue.write_buffer(&counters_buffer, 0, &vec![0_u8; lattices.len() * 16]);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("explore gpu dual contouring"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &uniform_buffer,
                        offset: 0,
                        size: wgpu::BufferSize::new(CHUNK_UNIFORM_STRIDE),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: values_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: materials_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: vertices_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: indices_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: counters_buffer.as_entire_binding(),
                },
            ],
        });
        let mut cells_timer = PassTimer::new(gpu, "explore-dc-cells");
        let mut edges_timer = PassTimer::new(gpu, "explore-dc-edges");
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("explore gpu dual contouring"),
        });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("cells"),
                timestamp_writes: cells_timer.as_ref().and_then(PassTimer::compute_writes),
            });
            pass.set_pipeline(&self.cells);
            for (index, lattice) in lattices.iter().enumerate() {
                pass.set_bind_group(0, &bind_group, &[index as u32 * CHUNK_UNIFORM_STRIDE as u32]);
                pass.dispatch_workgroups(lattice.cells().div_ceil(WORKGROUP), 1, 1);
            }
        }
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("edges"),
                timestamp_writes: edges_timer.as_ref().and_then(PassTimer::compute_writes),
            });
            pass.set_pipeline(&self.edges);
            for (index, lattice) in lattices.iter().enumerate() {
                pass.set_bind_group(0, &bind_group, &[index as u32 * CHUNK_UNIFORM_STRIDE as u32]);
                // One thread per sample and axis.
                let samples = lattice.dims[0] * lattice.dims[1] * lattice.dims[2];
                pass.dispatch_workgroups((samples * 3).div_ceil(WORKGROUP), 1, 1);
            }
        }
        if let Some(timer) = cells_timer.as_mut() {
            timer.resolve(&mut encoder);
        }
        if let Some(timer) = edges_timer.as_mut() {
            timer.resolve(&mut encoder);
        }
        encoder.copy_buffer_to_buffer(&counters_buffer, 0, &readback, 0, lattices.len() as u64 * 16);
        gpu.queue.submit([encoder.finish()]);
        if let Some(timer) = cells_timer.as_mut() {
            timer.submitted();
        }
        if let Some(timer) = edges_timer.as_mut() {
            timer.submitted();
        }
        let slice = readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        let mut report = Report {
            chunks: lattices.len(),
            upload_bytes,
            cpu_vertices,
            cpu_triangles,
            ..Report::default()
        };
        if let Ok(mapped) = slice.get_mapped_range() {
            let counts: &[u32] = bytemuck::cast_slice(&mapped);
            for chunk in counts.chunks_exact(4) {
                report.gpu_triangles += u64::from(chunk[0] / 3);
                report.gpu_vertices += u64::from(chunk[1]);
                report.overflowed += chunk[2].min(1);
            }
        }
        report.wall_ms = started.elapsed().as_secs_f64() * 1000.0;
        for (timer, slot) in [
            (cells_timer.as_mut(), &mut report.cells_gpu_ms),
            (edges_timer.as_mut(), &mut report.edges_gpu_ms),
        ] {
            if let Some(timer) = timer {
                for _ in 0..100 {
                    timer.collect(gpu);
                    let readout = timer.readout();
                    if readout.timed_frames > 0 {
                        *slot = readout.median_gpu_ms;
                        break;
                    }
                    let _ = device.poll(wgpu::PollType::wait_indefinitely());
                }
            }
        }
        report
    }
}
