//! Fog regions (`fog_region.wgsl`): the cells the volumetric fog reads for
//! each fog region, a flow and a density factor apiece, and the compute step
//! that moves them by their flow, settles them toward rest and lets the
//! stirrers drag and clear them. The cells live in the fog's row buffer
//! after the volume and region rows (`volumetric_fog.rs`), so the light pass
//! reads them with no binding of their own.
//!
//! The cells are the Engine's realization of the scene's regions and
//! stirrers, as the fog's history is of its medium: they move once per
//! presentation time an output frame draws, by the time since the last
//! (at most `MAX_STEP`). A reading draws them as they are. A region placed
//! anew, or moved, turned, resized or recelled, starts at rest (the current
//! and density 1), written when it is laid out whatever frame sees it first,
//! so what output frames draw does not depend on readings between them.
//!
//! The cells are not in the committed scene, so a renderer built afresh from
//! it (a rebaseline, a snapshot drawn offline) would start every region at
//! rest. Instead the cells cross with the scene as a [`FogField`]: read from
//! the renderer that evolved them and seeded into the fresh one, whose
//! regions that match one in the field start from its cells.

use std::collections::BTreeMap;

use render_model::{FogRegionDescriptor, FogStirrerDescriptor};

use crate::timing::{GpuPassTiming, PassTimer};
use crate::Gpu;

const PASS: &str = "fog-regions";
pub(crate) const WORKGROUP: [u32; 3] = [64, 1, 1];
/// One cell: its flow and density factor, four floats.
pub(crate) const CELL_BYTES: u64 = 16;
/// The step's uniform: its counts and its seconds.
const PARAMS_BYTES: u64 = 32;
/// `Region` in `fog_region.wgsl`: four vectors.
const REGION_BYTES: u64 = 64;
/// `Stirrer`: three vectors.
const STIRRER_BYTES: u64 = 48;
/// The longest step the cells take, seconds: a slow frame or a pause moves
/// them no further than this.
const MAX_STEP: f64 = 1.0 / 20.0;

/// A region as its cells are laid out.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Placed {
    pub id: u32,
    pub region: FogRegionDescriptor,
    pub cells: [u32; 3],
    /// Its first cell among every region's.
    pub first: u32,
}

impl Placed {
    fn count(&self) -> u32 {
        self.cells.iter().product()
    }

    /// Whether its cells still mean the same places as `other`'s.
    fn same_cells(&self, other: &Placed) -> bool {
        self.id == other.id
            && self.first == other.first
            && self.cells == other.cells
            && self.region.center == other.region.center
            && self.region.half_extents == other.region.half_extents
            && self.region.yaw_degrees == other.region.yaw_degrees
    }
}

/// The fog regions' cells as a renderer evolved them
/// ([`crate::Renderer::read_fog_field`]), to seed a fresh renderer with
/// ([`crate::Renderer::seed_fog_field`]).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FogField {
    pub regions: Vec<FogFieldRegion>,
}

/// One region's cells: its id and the place and cells they mean, and each
/// cell's flow (world metres per second) and density factor.
#[derive(Debug, Clone, PartialEq)]
pub struct FogFieldRegion {
    pub id: u32,
    pub center: [f32; 3],
    pub half_extents: [f32; 3],
    pub yaw_degrees: f32,
    pub cells: [u32; 3],
    pub data: Vec<[f32; 4]>,
}

impl FogFieldRegion {
    /// Whether these cells mean the places `placed`'s do.
    fn fits(&self, placed: &Placed) -> bool {
        self.id == placed.id
            && self.cells == placed.cells
            && self.center == placed.region.center
            && self.half_extents == placed.region.half_extents
            && self.yaw_degrees == placed.region.yaw_degrees
            && self.data.len() == placed.count() as usize
    }
}

/// The words of a field's region header: id, cells (3), centre (3), half
/// extents (3), yaw.
const FIELD_HEADER_WORDS: usize = 11;

impl FogField {
    pub fn is_empty(&self) -> bool {
        self.regions.is_empty()
    }

    /// Little-endian words: the region count, then per region its header
    /// and its cells, four floats each.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut words: Vec<u32> = vec![self.regions.len() as u32];
        for region in &self.regions {
            words.push(region.id);
            words.extend(region.cells);
            words.extend(region.center.map(f32::to_bits));
            words.extend(region.half_extents.map(f32::to_bits));
            words.push(region.yaw_degrees.to_bits());
            words.extend(region.data.iter().flatten().map(|value| value.to_bits()));
        }
        words.iter().flat_map(|word| word.to_le_bytes()).collect()
    }

    /// A field from [`Self::to_bytes`]; `None` when the bytes do not hold
    /// one whole.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let (words, tail) = bytes.as_chunks::<4>();
        if !tail.is_empty() {
            return None;
        }
        let words: Vec<u32> = words.iter().map(|word| u32::from_le_bytes(*word)).collect();
        let (&count, mut rest) = words.split_first()?;
        let float = f32::from_bits;
        let mut regions = Vec::new();
        for _ in 0..count {
            let header = rest.get(..FIELD_HEADER_WORDS)?;
            let cells = [header[1], header[2], header[3]];
            let count = cells
                .iter()
                .try_fold(1usize, |all, &n| all.checked_mul(n as usize))?;
            let data = rest.get(FIELD_HEADER_WORDS..FIELD_HEADER_WORDS + 4 * count)?;
            regions.push(FogFieldRegion {
                id: header[0],
                cells,
                center: [float(header[4]), float(header[5]), float(header[6])],
                half_extents: [float(header[7]), float(header[8]), float(header[9])],
                yaw_degrees: float(header[10]),
                data: data
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|cell| cell.map(float))
                    .collect(),
            });
            rest = &rest[FIELD_HEADER_WORDS + 4 * count..];
        }
        rest.is_empty().then_some(Self { regions })
    }
}

/// The regions laid out in id order, each while the scene's cells still
/// hold it.
pub(crate) fn lay_out(regions: &BTreeMap<u32, FogRegionDescriptor>) -> Vec<Placed> {
    let mut first = 0;
    let mut placed = Vec::new();
    for (&id, region) in regions.iter().take(FogRegionDescriptor::MAX_REGIONS) {
        let cells = region.cells();
        let count: u32 = cells.iter().product();
        if first + count > FogRegionDescriptor::MAX_SCENE_CELLS {
            continue;
        }
        placed.push(Placed {
            id,
            region: *region,
            cells,
            first,
        });
        first += count;
    }
    placed
}

pub(crate) struct FogRegions {
    pipeline: Option<wgpu::ComputePipeline>,
    layout: wgpu::BindGroupLayout,
    params: wgpu::Buffer,
    regions: wgpu::Buffer,
    stirrers: wgpu::Buffer,
    /// The cells as the last step left them, which the next reads; as many
    /// as the row buffer holds.
    previous: Option<wgpu::Buffer>,
    /// Where the cells start in the row buffer.
    first_cell_byte: u64,
    group: Option<wgpu::BindGroup>,
    placed: Vec<Placed>,
    /// The presentation time the cells were last moved to.
    moved_to: Option<f64>,
    /// Cells a fresh renderer starts the matching regions from, when they
    /// are first laid out.
    seed: FogField,
    timer: Option<PassTimer>,
}

impl FogRegions {
    /// `refused`: the volumetric fog's device refusal; regions draw only
    /// with it.
    pub fn new(gpu: &Gpu, shader: wgpu::ShaderModule, refused: bool) -> Self {
        let device = &gpu.device;
        let entry = |binding, ty| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let read = wgpu::BufferBindingType::Storage { read_only: true };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("render-wgpu fog regions"),
            entries: &[
                entry(0, wgpu::BufferBindingType::Uniform),
                entry(1, read),
                entry(2, read),
                entry(3, read),
                entry(4, wgpu::BufferBindingType::Storage { read_only: false }),
            ],
        });
        let pipeline = (!refused && gpu.compute_refusal(WORKGROUP, 0).is_none()).then(|| {
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("render-wgpu fog regions"),
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 0,
            });
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("render-wgpu fog regions"),
                layout: Some(&pipeline_layout),
                module: &shader,
                entry_point: Some("cs_stir"),
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
        let storage = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST;
        Self {
            pipeline,
            layout,
            params: buffer(
                "render-wgpu fog regions params",
                PARAMS_BYTES,
                wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            ),
            regions: buffer(
                "render-wgpu fog regions",
                REGION_BYTES * FogRegionDescriptor::MAX_REGIONS as u64,
                storage,
            ),
            stirrers: buffer(
                "render-wgpu fog stirrers",
                STIRRER_BYTES * FogStirrerDescriptor::MAX_STIRRERS as u64,
                storage,
            ),
            previous: None,
            first_cell_byte: 0,
            group: None,
            placed: Vec::new(),
            moved_to: None,
            seed: FogField::default(),
            timer: PassTimer::new(gpu, PASS),
        }
    }

    pub fn available(&self) -> bool {
        self.pipeline.is_some()
    }

    /// The regions as laid out.
    pub fn placed(&self) -> &[Placed] {
        &self.placed
    }

    /// No regions: the next laid out start at rest.
    pub fn clear(&mut self) {
        self.placed.clear();
    }

    /// Takes `placed` (some regions) as the regions' layout in `rows`, whose
    /// cells start at `first_cell_byte` and hold `capacity` cells (some): a
    /// region new to its cells, or every region when `rows` is new, starts
    /// from the seed's cells that fit it, or else at rest.
    pub fn lay_out(
        &mut self,
        gpu: &Gpu,
        rows: &wgpu::Buffer,
        rows_new: bool,
        first_cell_byte: u64,
        capacity: u64,
        placed: Vec<Placed>,
    ) {
        if rows_new || self.group.is_none() {
            self.placed.clear();
            let previous = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("render-wgpu fog region cells"),
                size: capacity * CELL_BYTES,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.group = Some(gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("render-wgpu fog regions"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.params.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: self.regions.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: self.stirrers.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: previous.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: rows,
                            offset: first_cell_byte,
                            size: wgpu::BufferSize::new(capacity * CELL_BYTES),
                        }),
                    },
                ],
            }));
            self.previous = Some(previous);
            self.first_cell_byte = first_cell_byte;
        }
        for region in &placed {
            if self.placed.iter().any(|known| known.same_cells(region)) {
                continue;
            }
            let offset = first_cell_byte + u64::from(region.first) * CELL_BYTES;
            if let Some(seeded) = self.seed.regions.iter().find(|seeded| seeded.fits(region)) {
                gpu.queue
                    .write_buffer(rows, offset, bytemuck::cast_slice(&seeded.data));
                continue;
            }
            let [x, y, z] = region.region.current;
            let rest: Vec<[f32; 4]> = vec![[x, y, z, 1.0]; region.count() as usize];
            gpu.queue
                .write_buffer(rows, offset, bytemuck::cast_slice(&rest));
        }
        self.seed.regions.clear();
        self.placed = placed;
    }

    /// Starts the regions `field` fits, when they are next laid out, from
    /// its cells.
    pub fn seed(&mut self, field: FogField) {
        self.seed = field;
    }

    /// The laid-out regions' cells in `rows` as the GPU holds them now,
    /// waiting for them; or, before they are laid out, the seed still
    /// waiting to be.
    pub fn read(&self, gpu: &Gpu, rows: &wgpu::Buffer) -> FogField {
        let cells: u32 = self.placed.iter().map(Placed::count).sum();
        if cells == 0 {
            return self.seed.clone();
        }
        let bytes = u64::from(cells) * CELL_BYTES;
        let staging = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("render-wgpu fog field readback"),
            size: bytes,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render-wgpu fog field readback"),
            });
        encoder.copy_buffer_to_buffer(rows, self.first_cell_byte, &staging, 0, bytes);
        gpu.queue.submit([encoder.finish()]);
        let slice = staging.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
        if !matches!(receiver.recv(), Ok(Ok(()))) {
            return FogField::default();
        }
        let Ok(mapped) = slice.get_mapped_range() else {
            return FogField::default();
        };
        let all: &[[f32; 4]] = bytemuck::cast_slice(&mapped);
        let regions = self
            .placed
            .iter()
            .map(|placed| {
                let first = placed.first as usize;
                FogFieldRegion {
                    id: placed.id,
                    center: placed.region.center,
                    half_extents: placed.region.half_extents,
                    yaw_degrees: placed.region.yaw_degrees,
                    cells: placed.cells,
                    data: all[first..first + placed.count() as usize].to_vec(),
                }
            })
            .collect();
        drop(mapped);
        staging.unmap();
        FogField { regions }
    }

    /// Moves the cells on to `time` by its distance from the last time
    /// they moved, with `stirrers` stirring them; nothing at a time they
    /// already reached. The cells of `rows` (as laid out) are copied aside
    /// first, for the step to read.
    pub fn step<'a>(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        rows: &wgpu::Buffer,
        stirrers: impl Iterator<Item = &'a FogStirrerDescriptor>,
        time: f64,
        timed: bool,
    ) {
        let Some(pipeline) = &self.pipeline else {
            return;
        };
        let Some(previous) = &self.previous else {
            return;
        };
        let Some(group) = &self.group else {
            return;
        };
        let seconds = match self.moved_to {
            Some(last) if time > last => (time - last).min(MAX_STEP) as f32,
            Some(last) if time == last => return,
            _ => 0.0,
        };
        self.moved_to = Some(time);
        let cells: u32 = self.placed.iter().map(Placed::count).sum();
        if cells == 0 || seconds <= 0.0 {
            return;
        }
        let regions: Vec<[f32; 16]> = self
            .placed
            .iter()
            .map(|placed| {
                let region = &placed.region;
                let [cx, cy, cz] = region.center;
                let [hx, hy, hz] = region.half_extents;
                let [vx, vy, vz] = region.current;
                let [nx, ny, nz] = placed.cells;
                [
                    cx,
                    cy,
                    cz,
                    region.yaw_degrees.to_radians(),
                    hx,
                    hy,
                    hz,
                    region.settle_seconds,
                    vx,
                    vy,
                    vz,
                    0.0,
                    f32::from_bits(nx),
                    f32::from_bits(ny),
                    f32::from_bits(nz),
                    f32::from_bits(placed.first),
                ]
            })
            .collect();
        let stirrers: Vec<[f32; 12]> = stirrers
            .take(FogStirrerDescriptor::MAX_STIRRERS)
            .map(|stirrer| {
                let [px, py, pz] = stirrer.position;
                let [vx, vy, vz] = stirrer.velocity;
                [
                    px,
                    py,
                    pz,
                    stirrer.radius,
                    vx,
                    vy,
                    vz,
                    stirrer.push,
                    stirrer.clear,
                    0.0,
                    0.0,
                    0.0,
                ]
            })
            .collect();
        let params: [u32; 8] = [
            regions.len() as u32,
            stirrers.len() as u32,
            cells,
            0,
            seconds.to_bits(),
            0,
            0,
            0,
        ];
        gpu.queue
            .write_buffer(&self.params, 0, bytemuck::cast_slice(&params));
        gpu.queue
            .write_buffer(&self.regions, 0, bytemuck::cast_slice(&regions));
        if !stirrers.is_empty() {
            gpu.queue
                .write_buffer(&self.stirrers, 0, bytemuck::cast_slice(&stirrers));
        }
        if let Some(timer) = &mut self.timer {
            timer.collect(gpu);
        }
        encoder.copy_buffer_to_buffer(
            rows,
            self.first_cell_byte,
            previous,
            0,
            u64::from(cells) * CELL_BYTES,
        );
        {
            let writes = self
                .timer
                .as_ref()
                .filter(|_| timed)
                .and_then(PassTimer::compute_writes);
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("render-wgpu fog regions"),
                timestamp_writes: writes,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, group, &[]);
            pass.dispatch_workgroups(cells.div_ceil(WORKGROUP[0]), 1, 1);
        }
        if timed {
            if let Some(timer) = &mut self.timer {
                timer.resolve(encoder);
            }
        }
    }

    /// After the frame that moved the cells is submitted.
    pub fn submitted(&mut self) {
        if let Some(timer) = &mut self.timer {
            timer.submitted();
        }
    }

    pub fn timing(&self) -> GpuPassTiming {
        self.timer
            .as_ref()
            .map_or_else(|| crate::timing::untimed(PASS), PassTimer::readout)
    }
}
