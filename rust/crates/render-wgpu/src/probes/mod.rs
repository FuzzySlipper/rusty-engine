//! Indirect light: an irradiance probe volume the Engine bakes from the
//! renderer's own triangles and light rows, and the standard shader samples
//! as the ambient term where the volume covers a fragment (#9596, #9597).
//!
//! A product asks for one volume (`SetIndirectLight`: a box, the spacing
//! between probes, the bounces, and whether the ambient light is the sky or a
//! floor). Probes sit on a world lattice `spacing` apart over the box, so a
//! volume that moves keeps the probes it still covers. Each probe gathers
//! the radiance arriving from a Fibonacci sphere of rays, baked on worker
//! threads, never the render thread: a ray that leaves the scene sees the
//! sky (the ambient and hemisphere rows and, when the sky's light is on, its
//! irradiance harmonics); a ray that hits a surface sees its emission, its
//! albedo times the direct light rows reaching it through shadow rays, and
//! from the second pass its albedo times the probe irradiance there (one more
//! bounce per pass). Each probe keeps its irradiance as L1 spherical
//! harmonics (four RGB coefficients, cosine convolved), and a probe whose
//! rays mostly hit back faces sits inside geometry and is filled from its
//! neighbours, so the shader's trilinear sample never reads a dark hole.
//!
//! The volume is kept in 16 m bricks (`bricks.rs` layout in `Bricks`): each
//! brick's triangles have their own BVH under the brick grid a ray walks, so
//! a change rebuilds one brick's BVH and re-traces the probes of the bricks
//! it can reach (its own and one around, or a light's range), each relaxing
//! against its neighbours' current probes and uploading alone while the old
//! probes keep drawing. The volume is one RGBA16F 3D texture, the three
//! colour channels stacked along its depth (on a software adapter one
//! slab: the ambient coefficient in colour and the vertical coefficient's
//! luminance, so a fragment reads one texel instead of three), that the
//! shader samples trilinearly at the
//! surface offset along its normal, fading over one cell past the box. A change starts a bake once the scene has been still for
//! `DEBOUNCE` (or every `MAX_WAIT` while something inside keeps moving).

mod bake;
pub(crate) mod thumb;
mod trace;

use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::time::{Duration, Instant};

use glam::{IVec3, Mat4, Vec3};
use render_model::{IndirectAmbient, IndirectLightDescriptor, RenderLayer};

use crate::tables::{Aabb, MaterialRef, PartId, Topology};
use crate::{Gpu, Renderer};
use bake::{bake_brick, cells_in, dilate_region, ms, pack_region, BrickBaked, Field};
pub(crate) use thumb::{material_mean, Thumb};
use trace::{Bvh, Scene, Sky, Triangle};

/// Rays per probe: L1 is low-frequency, so few are enough.
const RAYS: u32 = 64;
/// How long the scene stays unchanged before a bake starts.
pub(crate) const DEBOUNCE: Duration = Duration::from_millis(250);
/// A scene that never stays still (something moving inside the box every
/// frame) bakes this often.
pub(crate) const MAX_WAIT: Duration = Duration::from_secs(2);
/// A probe with more back-face hits than this sits inside geometry.
const BACKFACE_LIMIT: f32 = 0.25;
/// Rays and surface lookups start this far off a surface.
const SURFACE_OFFSET: f32 = 0.01;
/// The shader's normal offset, in probe spacings (`lighting.wgsl`).
const NORMAL_OFFSET: f32 = 0.3;
/// A probe whose open rays' mean direction is at least this long (a
/// hemisphere's is 0.5) is open to one side; its fill comes from there.
const OPEN_SIDE: f32 = 0.25;
/// Triangles per BVH leaf.
const LEAF: usize = 4;
/// Light rows are 16 floats (`frame.rs`).
const LIGHT_ROW: usize = 16;
/// A brick's side in metres: the unit of rebake.
pub(crate) const BRICK: f32 = 16.0;

/// What the volume reports through `engine.renderer`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct IndirectLightReadout {
    /// A volume is requested.
    pub enabled: bool,
    /// Probes per axis of the volume (0 before the first bake).
    pub dims: [u32; 3],
    pub probes: u32,
    /// Probes inside geometry, filled from their neighbours.
    pub invalid: u32,
    /// Triangles in the bricks' and the outside's BVHs.
    pub triangles: u32,
    /// The last batch's wall time, collection included.
    pub bake_ms: f64,
    /// Batches of bricks baked since the renderer was made.
    pub bakes: u32,
    /// A change is waiting for the debounce, or bricks are dirty or baking.
    pub pending: bool,
    /// GPU bytes the texture holds.
    pub bytes: u64,
    /// 16 m bricks the volume is kept in.
    pub bricks: u32,
    /// Bricks dirty or baking.
    pub bricks_pending: u32,
    /// The last brick's bake wall milliseconds.
    pub brick_ms: f64,
    /// The slowest brick since the renderer was made.
    pub brick_ms_max: f64,
    /// Bricks the last batch baked.
    pub last_batch_bricks: u32,
    /// Bytes the last frame that uploaded bricks wrote to the texture.
    pub upload_bytes: u64,
}

/// The volume's grid as the frame uniform carries it: probes on the world
/// lattice `offset + k × spacing`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Grid {
    pub min: Vec3,
    pub spacing: f32,
    pub dims: [u32; 3],
    pub ambient: IndirectAmbient,
}

impl Grid {
    /// The lattice's offset from the world origin: half a spacing up to half
    /// a metre, so probes of common spacings sit clear of 1 m voxel faces.
    fn offset(spacing: f32) -> f32 {
        0.5 * spacing.min(1.0)
    }

    pub fn of(descriptor: &IndirectLightDescriptor) -> Self {
        let spacing = descriptor.spacing;
        let offset = Self::offset(spacing);
        let mut min = Vec3::ZERO;
        let mut dims = [0; 3];
        for axis in 0..3 {
            let lo = f64::from(descriptor.center[axis] - descriptor.extent[axis]);
            let hi = f64::from(descriptor.center[axis] + descriptor.extent[axis]);
            let k0 = ((lo - f64::from(offset)) / f64::from(spacing)).ceil() as i64;
            let k1 = (((hi - f64::from(offset)) / f64::from(spacing)).floor() as i64).max(k0 + 1);
            dims[axis] = (k1 - k0 + 1) as u32;
            min[axis] = (f64::from(offset) + k0 as f64 * f64::from(spacing)) as f32;
        }
        Self {
            min,
            spacing,
            dims,
            ambient: descriptor.ambient,
        }
    }

    /// The lattice index of the first probe on each axis.
    pub fn lattice(&self) -> [i64; 3] {
        let offset = Self::offset(self.spacing);
        [0, 1, 2].map(|axis| ((self.min[axis] - offset) / self.spacing).round() as i64)
    }

    /// The frame uniform's two rows: origin and spacing; dims and mode (0
    /// off, 1 the ambient light is the sky, 2 a floor; 3 and 4 the same
    /// with the one-slab encoding).
    pub fn uniform(grid: Option<&Grid>, compact: bool) -> [f32; 8] {
        match grid {
            None => [0.0; 8],
            Some(grid) => [
                grid.min.x,
                grid.min.y,
                grid.min.z,
                grid.spacing,
                grid.dims[0] as f32,
                grid.dims[1] as f32,
                grid.dims[2] as f32,
                match grid.ambient {
                    IndirectAmbient::Sky => 1.0,
                    IndirectAmbient::Floor => 2.0,
                } + if compact { 2.0 } else { 0.0 },
            ],
        }
    }

    pub fn probes(&self) -> usize {
        (self.dims[0] * self.dims[1] * self.dims[2]) as usize
    }

    pub fn index(&self, [x, y, z]: [u32; 3]) -> usize {
        ((z * self.dims[1] + y) * self.dims[0] + x) as usize
    }

    pub fn coords(&self, index: usize) -> [u32; 3] {
        let index = index as u32;
        let x = index % self.dims[0];
        let y = index / self.dims[0] % self.dims[1];
        let z = index / (self.dims[0] * self.dims[1]);
        [x, y, z]
    }

    pub fn position(&self, [x, y, z]: [u32; 3]) -> Vec3 {
        self.min + Vec3::new(x as f32, y as f32, z as f32) * self.spacing
    }

    #[cfg(test)]
    pub fn max(&self) -> Vec3 {
        self.position([self.dims[0] - 1, self.dims[1] - 1, self.dims[2] - 1])
    }
}

/// The volume's 16 m bricks: world-aligned cells over the grid, each with
/// the probe indices it holds.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Bricks {
    /// The first cell on each axis, in bricks from the world origin.
    pub b0: [i64; 3],
    /// Cells per axis.
    pub nb: [u32; 3],
    /// Each brick's inclusive probe index range per axis.
    pub ranges: Vec<[[u32; 2]; 3]>,
}

impl Bricks {
    pub fn of(grid: &Grid) -> Self {
        let mut b0 = [0; 3];
        let mut nb = [0; 3];
        let mut axis_ranges: [Vec<[u32; 2]>; 3] = Default::default();
        for axis in 0..3 {
            let cell_of = |i: u32| {
                let position = f64::from(grid.min[axis]) + f64::from(i) * f64::from(grid.spacing);
                (position / f64::from(BRICK)).floor() as i64
            };
            let first = cell_of(0);
            let last = cell_of(grid.dims[axis] - 1);
            b0[axis] = first;
            nb[axis] = (last - first + 1) as u32;
            let mut ranges = vec![[u32::MAX, 0]; nb[axis] as usize];
            for i in 0..grid.dims[axis] {
                let cell = (cell_of(i) - first) as usize;
                ranges[cell][0] = ranges[cell][0].min(i);
                ranges[cell][1] = ranges[cell][1].max(i);
            }
            axis_ranges[axis] = ranges;
        }
        let mut ranges = Vec::with_capacity((nb[0] * nb[1] * nb[2]) as usize);
        for bz in 0..nb[2] {
            for by in 0..nb[1] {
                for bx in 0..nb[0] {
                    ranges.push([
                        axis_ranges[0][bx as usize],
                        axis_ranges[1][by as usize],
                        axis_ranges[2][bz as usize],
                    ]);
                }
            }
        }
        Self { b0, nb, ranges }
    }

    pub fn len(&self) -> usize {
        self.ranges.len()
    }

    /// A brick by its cell coordinates relative to the first cell.
    pub fn index_of(&self, cell: IVec3) -> usize {
        ((cell.z as u32 * self.nb[1] + cell.y as u32) * self.nb[0] + cell.x as u32) as usize
    }

    pub fn coords(&self, index: usize) -> [u32; 3] {
        let index = index as u32;
        [
            index % self.nb[0],
            index / self.nb[0] % self.nb[1],
            index / (self.nb[0] * self.nb[1]),
        ]
    }

    /// A brick's world cell.
    pub fn cell(&self, index: usize) -> [i64; 3] {
        let c = self.coords(index);
        [0, 1, 2].map(|axis| self.b0[axis] + i64::from(c[axis]))
    }

    pub fn cell_box(&self, index: usize) -> (Vec3, Vec3) {
        let cell = self.cell(index);
        let min = Vec3::new(cell[0] as f32, cell[1] as f32, cell[2] as f32) * BRICK;
        (min, min + Vec3::splat(BRICK))
    }

    /// The box every brick together covers.
    pub fn world_bounds(&self) -> (Vec3, Vec3) {
        let min = Vec3::new(self.b0[0] as f32, self.b0[1] as f32, self.b0[2] as f32) * BRICK;
        (
            min,
            min + Vec3::new(self.nb[0] as f32, self.nb[1] as f32, self.nb[2] as f32) * BRICK,
        )
    }

    /// The bricks whose cells overlap `min..=max`.
    pub fn touching(&self, min: Vec3, max: Vec3) -> Vec<usize> {
        let mut lo = [0; 3];
        let mut hi = [0; 3];
        for axis in 0..3 {
            let last = self.b0[axis] + i64::from(self.nb[axis]) - 1;
            let first = ((min[axis] / BRICK).floor() as i64).clamp(self.b0[axis], last);
            let end = ((max[axis] / BRICK).floor() as i64).clamp(self.b0[axis], last);
            if max[axis] < self.b0[axis] as f32 * BRICK || min[axis] >= (last + 1) as f32 * BRICK {
                return Vec::new();
            }
            lo[axis] = (first - self.b0[axis]) as u32;
            hi[axis] = (end - self.b0[axis]) as u32;
        }
        let mut out = Vec::new();
        for bz in lo[2]..=hi[2] {
            for by in lo[1]..=hi[1] {
                for bx in lo[0]..=hi[0] {
                    out.push(self.index_of(IVec3::new(bx as i32, by as i32, bz as i32)));
                }
            }
        }
        out
    }
}

/// One brick's bake state.
#[derive(Clone, Default)]
struct BrickState {
    /// The BVH over the triangles in the brick's cell; `None` when empty or
    /// not built yet.
    blas: Option<Arc<Bvh>>,
    blas_dirty: bool,
    probes_dirty: bool,
    /// Baked at least once since the volume was made or scrolled over it.
    baked: bool,
    /// Bricks of this cell being baked.
    in_flight: bool,
    ms: f64,
}

/// The requested volume on the CPU: its grid, bricks and probes, and what
/// is dirty.
pub(crate) struct Volume {
    request: IndirectLightDescriptor,
    grid: Grid,
    bricks: Bricks,
    states: Vec<BrickState>,
    field: Field,
    /// The BVH over triangles beyond every brick.
    outside: Option<Arc<Bvh>>,
    outside_dirty: bool,
    /// Every brick has been baked at least once: the shader may read the
    /// volume. A scroll keeps it, drawing the new bricks' placeholders.
    complete: bool,
}

/// What a batch has to do, taken from the dirty flags.
pub(crate) struct Work {
    rebuild: Vec<usize>,
    bake: Vec<usize>,
    outside: bool,
}

impl Volume {
    fn new(request: IndirectLightDescriptor) -> Self {
        let grid = Grid::of(&request);
        let bricks = Bricks::of(&grid);
        let states = vec![
            BrickState {
                blas_dirty: true,
                probes_dirty: true,
                ..BrickState::default()
            };
            bricks.len()
        ];
        Self {
            request,
            grid,
            bricks,
            states,
            field: Field::new(grid),
            outside: None,
            outside_dirty: true,
            complete: false,
        }
    }

    /// Whether `request` keeps this volume's lattice: only its centre moved.
    fn same_lattice(&self, request: &IndirectLightDescriptor) -> bool {
        self.request.spacing == request.spacing
            && self.request.extent == request.extent
            && self.request.bounces == request.bounces
            && self.request.ambient == request.ambient
    }

    /// The volume moved to `request`: probes keep their world positions and
    /// values, bricks their BVHs, and a probe new to the volume takes the
    /// nearest old probe's value until its brick bakes.
    fn scrolled(&self, request: IndirectLightDescriptor) -> Self {
        let mut next = Self::new(request);
        let old = &self.grid;
        let new = &next.grid;
        let (old_k, new_k) = (old.lattice(), new.lattice());
        let delta = [0, 1, 2].map(|axis| new_k[axis] - old_k[axis]);
        let mut carried = vec![false; new.probes()];
        for (index, carried) in carried.iter_mut().enumerate() {
            let coords = new.coords(index);
            let mut old_coords = [0; 3];
            let mut inside = true;
            for axis in 0..3 {
                let k = i64::from(coords[axis]) + delta[axis];
                inside &= (0..i64::from(old.dims[axis])).contains(&k);
                old_coords[axis] = k.clamp(0, i64::from(old.dims[axis]) - 1) as u32;
            }
            let from = old.index(old_coords);
            next.field.sh[index] = self.field.sh[from];
            if inside {
                next.field.raw_valid[index] = self.field.raw_valid[from];
                next.field.filled[index] = self.field.filled[from];
                next.field.opens[index] = self.field.opens[from];
                next.field.exits[index] = self.field.exits[from];
                *carried = true;
            } else {
                next.field.filled[index] = self.field.filled[from];
            }
        }
        let by_cell: HashMap<[i64; 3], &BrickState> = (0..self.bricks.len())
            .map(|index| (self.bricks.cell(index), &self.states[index]))
            .collect();
        for index in 0..next.bricks.len() {
            let range = next.bricks.ranges[index];
            let all_carried = cells_in(range)
                .into_iter()
                .all(|cell| carried[new.index(cell)]);
            let state = &mut next.states[index];
            if let Some(old) = by_cell.get(&next.bricks.cell(index)) {
                state.blas = old.blas.clone();
                state.blas_dirty = old.blas_dirty || (old.blas.is_none() && !old.baked);
                state.baked = old.baked && all_carried;
                state.probes_dirty = !state.baked || old.probes_dirty;
            }
        }
        next.outside = self.outside.clone();
        next.outside_dirty = true;
        next.complete = self.complete;
        next
    }

    /// A change within `min..=max`: the bricks it touches rebuild their
    /// BVHs, those plus one around rebake, and the outside rebuilds when
    /// the box reaches past the bricks.
    fn mark_bounds(&mut self, min: Vec3, max: Vec3) -> bool {
        let mut marked = false;
        for brick in self.bricks.touching(min, max) {
            self.states[brick].blas_dirty = true;
            marked = true;
        }
        for brick in self
            .bricks
            .touching(min - Vec3::splat(BRICK), max + Vec3::splat(BRICK))
        {
            self.states[brick].probes_dirty = true;
            marked = true;
        }
        let (lo, hi) = self.bricks.world_bounds();
        if min.cmplt(lo).any() || max.cmpgt(hi).any() {
            self.outside_dirty = true;
            marked = true;
        }
        marked
    }

    /// A light changed: the bricks within its range rebake, or every brick
    /// for an unbounded or global light.
    fn mark_light(&mut self, row: &[f32; LIGHT_ROW]) {
        let kind = row[3] as u32;
        let range = row[7];
        if matches!(kind, 3 | 4) && range > 0.0 {
            let position = Vec3::new(row[4], row[5], row[6]);
            let reach = Vec3::splat(range + BRICK);
            for brick in self.bricks.touching(position - reach, position + reach) {
                self.states[brick].probes_dirty = true;
            }
        } else {
            for state in &mut self.states {
                state.probes_dirty = true;
            }
        }
    }

    fn mark_all(&mut self) {
        for state in &mut self.states {
            state.blas_dirty = true;
            state.probes_dirty = true;
        }
        self.outside_dirty = true;
    }

    fn pending_bricks(&self) -> u32 {
        self.states
            .iter()
            .filter(|state| state.probes_dirty || state.in_flight)
            .count() as u32
    }

    /// Take what is dirty, clearing the flags; `None` when nothing is.
    fn take_work(&mut self) -> Option<Work> {
        let mut work = Work {
            rebuild: Vec::new(),
            bake: Vec::new(),
            outside: std::mem::take(&mut self.outside_dirty),
        };
        for (index, state) in self.states.iter_mut().enumerate() {
            if std::mem::take(&mut state.blas_dirty) {
                work.rebuild.push(index);
            }
            if std::mem::take(&mut state.probes_dirty) {
                state.in_flight = true;
                work.bake.push(index);
            }
        }
        (!work.rebuild.is_empty() || !work.bake.is_empty() || work.outside).then_some(work)
    }
}

/// The light rows whose count differs between `old` and `new`: a row added,
/// removed or changed, counted with its duplicates, since two lights alike in
/// every value are still two lights.
fn changed_rows(old: &[f32], new: &[f32]) -> Vec<[f32; LIGHT_ROW]> {
    let mut counts: HashMap<[u32; LIGHT_ROW], i32> = HashMap::new();
    for row in old.as_chunks::<LIGHT_ROW>().0 {
        *counts.entry(row.map(f32::to_bits)).or_default() -= 1;
    }
    for row in new.as_chunks::<LIGHT_ROW>().0 {
        *counts.entry(row.map(f32::to_bits)).or_default() += 1;
    }
    counts
        .into_iter()
        .filter(|(_, count)| *count != 0)
        .map(|(row, _)| row.map(f32::from_bits))
        .collect()
}

/// When the next bake starts.
#[derive(Default)]
struct Schedule {
    /// When changes inside the box were first and last seen since the last
    /// bake started.
    dirty: Option<(Instant, Instant)>,
}

impl Schedule {
    fn touch(&mut self, now: Instant) {
        let first = self.dirty.map_or(now, |(first, _)| first);
        self.dirty = Some((first, now));
    }

    /// The scene has been still for `DEBOUNCE`, or has kept changing for
    /// `MAX_WAIT`.
    fn due(&self, now: Instant) -> bool {
        self.dirty.is_some_and(|(first, last)| {
            now.duration_since(last) >= DEBOUNCE || now.duration_since(first) >= MAX_WAIT
        })
    }
}

/// Everything a batch of bricks needs, collected on the render thread.
pub(crate) struct Batch {
    /// The renderer's scene generation the batch reads.
    generation: u64,
    grid: Grid,
    bricks: Bricks,
    bounces: u32,
    /// Bricks whose BVH to rebuild, with their cells' triangles.
    rebuild: Vec<(usize, Vec<Triangle>)>,
    /// Every brick's current BVH.
    blas: Vec<Option<Arc<Bvh>>>,
    outside: Option<Arc<Bvh>>,
    /// The outside's triangles when it rebuilds.
    outside_rebuild: Option<Vec<Triangle>>,
    /// Bricks to bake.
    bake: Vec<usize>,
    /// The world's light rows (`frame.rs` `light_row`).
    rows: Vec<f32>,
    sky: Sky,
    /// The volume's probes as the batch starts: what bounces read beyond
    /// the brick being baked.
    snapshot: Arc<Field>,
    collect_ms: f64,
}

/// What a batch sends back, brick by brick.
enum BrickResult {
    Blas {
        brick: usize,
        blas: Option<Arc<Bvh>>,
    },
    Outside {
        blas: Option<Arc<Bvh>>,
    },
    Baked {
        brick: usize,
        baked: BrickBaked,
    },
    Done {
        ms: f64,
    },
}

/// Run a batch: rebuild the BVHs, then bake its bricks one after another.
fn run_batch(batch: Batch, emit: &mut dyn FnMut(BrickResult)) {
    let started = Instant::now();
    let mut blas = batch.blas;
    for (brick, triangles) in batch.rebuild {
        let built = (!triangles.is_empty()).then(|| Arc::new(Bvh::build(triangles)));
        blas[brick] = built.clone();
        emit(BrickResult::Blas { brick, blas: built });
    }
    let mut outside = batch.outside;
    if let Some(triangles) = batch.outside_rebuild {
        outside = (!triangles.is_empty()).then(|| Arc::new(Bvh::build(triangles)));
        emit(BrickResult::Outside {
            blas: outside.clone(),
        });
    }
    let scene = Scene {
        bricks: batch.bricks,
        blas,
        outside,
    };
    for brick in batch.bake {
        let baked = bake_brick(
            &scene,
            &batch.grid,
            scene.bricks.ranges[brick],
            batch.bounces,
            &batch.rows,
            &batch.sky,
            &batch.snapshot,
        );
        emit(BrickResult::Baked { brick, baked });
    }
    emit(BrickResult::Done {
        ms: batch.collect_ms + ms(started),
    });
}

/// The volume on the GPU, and the bakes that keep it current.
pub(crate) struct ProbeVolume {
    texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    texture_dims: [u32; 3],
    /// One slab instead of three: on a software adapter, where each
    /// filtered read costs a share of the frame.
    compact: bool,
    volume: Option<Volume>,
    schedule: Schedule,
    /// The scene generation the last batch started from.
    baked_generation: u64,
    /// A batch running on its worker thread.
    job: Option<Receiver<BrickResult>>,
    /// Bricks the running batch has baked so far.
    batch_bricks: u32,
    /// The world light rows the last marking compared against.
    last_rows: Vec<f32>,
    /// Parts whose move the last batch already read, with the bounds it
    /// read, so the frame that drains them does not mark their bricks again
    /// unless they moved since.
    covered_moved: HashMap<PartId, Aabb>,
    readout: IndirectLightReadout,
    /// Bytes uploaded since the frame began.
    frame_upload: u64,
}

impl ProbeVolume {
    pub fn new(gpu: &Gpu) -> Self {
        let compact = gpu.adapter.get_info().device_type == wgpu::DeviceType::Cpu;
        let (texture, view) = texture(gpu, [1, 1, 1], slabs(compact));
        Self {
            texture,
            view,
            compact,
            sampler: gpu.device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("render-wgpu probes"),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
            texture_dims: [1, 1, 1],
            volume: None,
            schedule: Schedule::default(),
            baked_generation: 0,
            job: None,
            batch_bricks: 0,
            last_rows: Vec::new(),
            covered_moved: HashMap::new(),
            readout: IndirectLightReadout::default(),
            frame_upload: 0,
        }
    }

    pub fn readout(&self) -> IndirectLightReadout {
        let pending_bricks = self.volume.as_ref().map_or(0, Volume::pending_bricks);
        IndirectLightReadout {
            pending: self.schedule.dirty.is_some() || self.job.is_some() || pending_bricks > 0,
            bricks_pending: pending_bricks,
            ..self.readout
        }
    }

    /// The frame uniform's rows: the grid once every brick has baked, and
    /// its encoding.
    pub fn uniform(&self) -> [f32; 8] {
        let grid = self
            .volume
            .as_ref()
            .filter(|volume| volume.complete)
            .map(|volume| &volume.grid);
        Grid::uniform(grid, self.compact)
    }

    /// A change inside the volume (or to the request) was seen now.
    pub fn touch(&mut self, now: Instant) {
        self.schedule.touch(now);
    }

    /// Whether a batch should start now and none is running.
    pub fn due(&self, now: Instant) -> bool {
        self.job.is_none() && self.schedule.due(now)
    }

    /// Whether the scene has changed since the last batch started.
    pub fn stale(&self, generation: u64) -> bool {
        self.baked_generation != generation
    }

    /// The frame began: the frame before is the last one, uploads or none.
    pub fn begin_frame(&mut self) {
        self.readout.upload_bytes = self.frame_upload;
        self.frame_upload = 0;
    }

    /// Request a volume, move the one there is, or with `None` drop it.
    /// Returns whether the frame bind group must be rebuilt (the texture
    /// changed).
    pub fn request(&mut self, gpu: &Gpu, descriptor: Option<IndirectLightDescriptor>) -> bool {
        let Some(descriptor) = descriptor else {
            return self.clear(gpu);
        };
        if self
            .volume
            .as_ref()
            .is_some_and(|volume| volume.request == descriptor)
        {
            return false;
        }
        let volume = match &self.volume {
            Some(old) if old.same_lattice(&descriptor) => old.scrolled(descriptor),
            _ => Volume::new(descriptor),
        };
        // A running batch belongs to the old grid.
        self.job = None;
        self.batch_bricks = 0;
        let rebind = self.ensure_texture(gpu, volume.grid.dims);
        let complete = volume.complete;
        self.volume = Some(volume);
        if complete {
            // The moved volume keeps drawing from what it carried.
            let dims = self.texture_dims;
            self.upload_region(gpu, [0, 0, 0], [dims[0] - 1, dims[1] - 1, dims[2] - 1]);
        }
        self.readout.enabled = true;
        self.readout.dims = self.texture_dims;
        self.readout.probes = self.volume.as_ref().map_or(0, |v| v.grid.probes() as u32);
        self.readout.bricks = self.volume.as_ref().map_or(0, |v| v.bricks.len() as u32);
        self.readout.bytes = texture_bytes(self.texture_dims, slabs(self.compact));
        self.schedule.touch(Instant::now());
        rebind
    }

    fn ensure_texture(&mut self, gpu: &Gpu, dims: [u32; 3]) -> bool {
        if self.texture_dims == dims {
            return false;
        }
        let (texture, view) = texture(gpu, dims, slabs(self.compact));
        self.texture = texture;
        self.view = view;
        self.texture_dims = dims;
        true
    }

    /// Whether the last batch already read this part where it is now; each
    /// part is answered once.
    pub fn covered_move(&mut self, part: PartId, bounds: &Aabb) -> bool {
        self.covered_moved.remove(&part).as_ref() == Some(bounds)
    }

    /// The frame's moves have been looked at.
    pub fn moves_seen(&mut self) {
        self.covered_moved.clear();
    }

    /// A retained change within `min..=max`.
    pub fn mark_bounds(&mut self, min: Vec3, max: Vec3) -> bool {
        self.volume
            .as_mut()
            .is_some_and(|volume| volume.mark_bounds(min, max))
    }

    /// The world's light rows now: the bricks within reach of every row that
    /// changed since the last marking rebake.
    pub fn mark_lights(&mut self, rows: &[f32]) -> bool {
        let Some(volume) = self.volume.as_mut() else {
            return false;
        };
        let mut marked = false;
        for row in changed_rows(&self.last_rows, rows) {
            volume.mark_light(&row);
            marked = true;
        }
        self.last_rows = rows.to_vec();
        marked
    }

    /// Materials, textures or the sky changed: everything rebakes.
    pub fn mark_all(&mut self) -> bool {
        match self.volume.as_mut() {
            Some(volume) => {
                volume.mark_all();
                true
            }
            None => false,
        }
    }

    /// Start `batch` on a worker thread.
    pub fn start(&mut self, batch: Batch) {
        self.schedule.dirty = None;
        self.baked_generation = batch.generation;
        self.batch_bricks = 0;
        let (sender, receiver) = mpsc::channel();
        self.job = Some(receiver);
        let spawned = std::thread::Builder::new()
            .name("rusty-probe-bake".to_owned())
            .spawn(move || {
                run_batch(batch, &mut |result| {
                    let _ = sender.send(result);
                });
            });
        if spawned.is_err() {
            self.job = None;
        }
    }

    /// Nothing was dirty after all: the debounce is over.
    pub fn settle(&mut self) {
        self.schedule.dirty = None;
    }

    /// Take the finished bricks of the running batch and upload them.
    pub fn poll(&mut self, gpu: &Gpu) {
        loop {
            let result = match self.job.as_ref().map(Receiver::try_recv) {
                Some(Ok(result)) => result,
                Some(Err(mpsc::TryRecvError::Disconnected)) => {
                    self.job = None;
                    return;
                }
                _ => return,
            };
            self.absorb(gpu, result);
        }
    }

    /// Run `batch` inline, on this thread's workers, and upload it: for
    /// tools and tests that want the volume before the next frame.
    pub fn bake_now(&mut self, gpu: &Gpu, batch: Batch) {
        self.schedule.dirty = None;
        self.baked_generation = batch.generation;
        self.job = None;
        self.batch_bricks = 0;
        let mut results = Vec::new();
        run_batch(batch, &mut |result| results.push(result));
        for result in results {
            self.absorb(gpu, result);
        }
    }

    fn absorb(&mut self, gpu: &Gpu, result: BrickResult) {
        let Some(volume) = self.volume.as_mut() else {
            return;
        };
        match result {
            BrickResult::Blas { brick, blas } => volume.states[brick].blas = blas,
            BrickResult::Outside { blas } => volume.outside = blas,
            BrickResult::Baked { brick, baked } => {
                let range = volume.bricks.ranges[brick];
                for (local, cell) in cells_in(range).into_iter().enumerate() {
                    let index = volume.grid.index(cell);
                    volume.field.sh[index] = baked.sh[local];
                    volume.field.raw_valid[index] = baked.raw_valid[local];
                    volume.field.filled[index] = baked.raw_valid[local];
                    volume.field.opens[index] = baked.opens[local];
                    volume.field.exits[index] = baked.exits[local];
                }
                let dims = volume.grid.dims;
                let lo = [0, 1, 2].map(|axis| range[axis][0].saturating_sub(1));
                let hi = [0, 1, 2].map(|axis| (range[axis][1] + 1).min(dims[axis] - 1));
                dilate_region(&mut volume.field, lo, hi);
                let state = &mut volume.states[brick];
                state.baked = true;
                state.in_flight = false;
                state.ms = baked.ms;
                self.readout.brick_ms = baked.ms;
                self.readout.brick_ms_max = self.readout.brick_ms_max.max(baked.ms);
                self.batch_bricks += 1;
                self.upload_region(gpu, lo, hi);
            }
            BrickResult::Done { ms } => {
                self.job = None;
                let volume = self.volume.as_mut().expect("checked above");
                volume.complete = volume.states.iter().all(|state| state.baked);
                self.readout.bake_ms = ms;
                self.readout.bakes += 1;
                self.readout.last_batch_bricks = self.batch_bricks;
                self.readout.invalid = volume
                    .field
                    .raw_valid
                    .iter()
                    .filter(|valid| !**valid)
                    .count() as u32;
                self.readout.triangles = volume
                    .states
                    .iter()
                    .filter_map(|state| state.blas.as_ref())
                    .map(|blas| blas.len())
                    .sum::<usize>() as u32
                    + volume.outside.as_ref().map_or(0, |blas| blas.len() as u32);
            }
        }
    }

    /// Write the probes of `lo..=hi` to the texture.
    fn upload_region(&mut self, gpu: &Gpu, lo: [u32; 3], hi: [u32; 3]) {
        let Some(volume) = self.volume.as_ref() else {
            return;
        };
        let texels = pack_region(&volume.field, lo, hi, self.compact);
        let extent = [0, 1, 2].map(|axis| hi[axis] - lo[axis] + 1);
        let per_channel = (extent[0] * extent[1] * extent[2] * 4) as usize;
        let depth = self.texture_dims[2];
        for channel in 0..slabs(self.compact) {
            let block =
                &texels[channel as usize * per_channel..(channel as usize + 1) * per_channel];
            gpu.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: lo[0],
                        y: lo[1],
                        z: channel * depth + lo[2],
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                bytemuck::cast_slice(block),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(extent[0] * 8),
                    rows_per_image: Some(extent[1]),
                },
                wgpu::Extent3d {
                    width: extent[0],
                    height: extent[1],
                    depth_or_array_layers: extent[2],
                },
            );
        }
        self.frame_upload += (texels.len() * 2) as u64;
    }

    /// Drop the volume (the request went away). Returns whether the frame
    /// bind group must be rebuilt.
    pub fn clear(&mut self, gpu: &Gpu) -> bool {
        self.schedule.dirty = None;
        self.job = None;
        let had = self.volume.take().is_some();
        let rebind = self.ensure_texture(gpu, [1, 1, 1]);
        self.last_rows.clear();
        self.readout = IndirectLightReadout {
            bakes: self.readout.bakes,
            brick_ms_max: self.readout.brick_ms_max,
            ..IndirectLightReadout::default()
        };
        had && rebind
    }
}

/// Texels a probe takes: one per colour channel, or one in the compact
/// encoding.
fn slabs(compact: bool) -> u32 {
    if compact {
        1
    } else {
        3
    }
}

fn texture_bytes(dims: [u32; 3], slabs: u32) -> u64 {
    u64::from(dims[0]) * u64::from(dims[1]) * u64::from(dims[2]) * 8 * u64::from(slabs)
}

/// One RGBA16F 3D texture of `dims` probes, `slabs` texels a probe stacked
/// along its depth.
fn texture(gpu: &Gpu, dims: [u32; 3], slabs: u32) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("render-wgpu probes"),
        size: wgpu::Extent3d {
            width: dims[0],
            height: dims[1],
            depth_or_array_layers: dims[2] * slabs,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D3,
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    (texture, view)
}

impl Renderer {
    /// The next batch of probe work from the retained scene as it stands:
    /// the triangles of the bricks whose BVHs rebuild, the light rows, the
    /// sky, and the probes as they are; `None` when nothing is dirty.
    pub(crate) fn probe_batch(&mut self) -> Option<Batch> {
        let descriptor = self.tables.indirect_light?;
        let started = Instant::now();
        let work = self.probes.volume.as_mut()?.take_work()?;
        let volume = self.probes.volume.as_ref()?;
        let (grid, bricks) = (volume.grid, volume.bricks.clone());
        let rebuild = work
            .rebuild
            .iter()
            .map(|brick| {
                let (min, max) = bricks.cell_box(*brick);
                (*brick, self.probe_triangles(|t| t.touches(min, max)))
            })
            .collect();
        let outside_rebuild = work.outside.then(|| {
            let (min, max) = bricks.world_bounds();
            self.probe_triangles(|t| !t.touches(min, max))
        });
        let mut rows: Vec<f32> = Vec::new();
        self.world_light_rows(&mut rows);
        let pi = std::f32::consts::PI;
        let mut sky = Sky::NONE;
        for row in rows.as_chunks::<LIGHT_ROW>().0 {
            let color = Vec3::new(row[0], row[1], row[2]);
            match row[3] as u32 {
                // Uniform radiance L gives irradiance πL; a floor keeps its
                // ambient rows in the shader instead.
                0 if descriptor.ambient == IndirectAmbient::Sky => sky.uniform += color / pi,
                1 => {
                    // E(n) = mix(ground, sky, n.y/2 + 1/2) from L = a + b·y.
                    let ground = Vec3::new(row[12], row[13], row[14]);
                    sky.a += (ground + color) / (2.0 * pi);
                    sky.b += (color - ground) * (3.0 / (4.0 * pi));
                }
                _ => {}
            }
        }
        if let Some(sky_light) = self.tables.sky_light.filter(|light| light.intensity > 0.0) {
            if let Some(coefficients) = self.sky_light.read_irradiance(&self.gpu) {
                // Irradiance coefficients become radiance through the band
                // factors (π, 2π/3, π/4), scaled by the light's intensity.
                let bands = [
                    pi,
                    2.0 * pi / 3.0,
                    2.0 * pi / 3.0,
                    2.0 * pi / 3.0,
                    pi / 4.0,
                    pi / 4.0,
                    pi / 4.0,
                    pi / 4.0,
                    pi / 4.0,
                ];
                let mut harmonics = [Vec3::ZERO; 9];
                for (index, c) in coefficients.iter().enumerate() {
                    harmonics[index] =
                        Vec3::new(c[0], c[1], c[2]) * (sky_light.intensity / bands[index]);
                }
                sky.harmonics = Some(harmonics);
            }
        }
        // What this batch reads is not a change for the next frame.
        self.probes.last_rows = rows.clone();
        self.probes.covered_moved = self
            .tables
            .parts
            .moved
            .iter()
            .map(|id| (*id, self.tables.parts.state[*id as usize].world_bounds))
            .collect();
        self.tables.parts.former_bounds.clear();
        let volume = self.probes.volume.as_ref()?;
        Some(Batch {
            generation: self.scene_generation,
            grid,
            bricks,
            bounces: descriptor.bounces.max(1),
            rebuild,
            blas: volume
                .states
                .iter()
                .map(|state| state.blas.clone())
                .collect(),
            outside: volume.outside.clone(),
            outside_rebuild,
            bake: work.bake,
            rows,
            sky,
            snapshot: Arc::new(volume.field.clone()),
            collect_ms: ms(started),
        })
    }

    /// The triangles of every shown, opaque, triangle part of the world in
    /// world space that `keep` admits, with their albedo (the part colour
    /// times its texture's mean) and emission.
    fn probe_triangles(&self, keep: impl Fn(&Triangle) -> bool) -> Vec<Triangle> {
        let parts = &self.tables.parts;
        let mut triangles = Vec::new();
        for (id, part) in parts.meta.iter().enumerate() {
            let Some(part) = part else { continue };
            let state = &parts.state[id];
            // Scattered copies are clutter, not the scene light bounces in.
            if !state.shown
                || state.leader.is_some()
                || state.layer != RenderLayer::Scene
                || state.class.blend
                || state.class.lines
                || part.wireframe
            {
                continue;
            }
            let Some(mesh) = self.mesh(&part.mesh) else {
                continue;
            };
            if mesh.topology != Topology::Triangles {
                continue;
            }
            let row = crate::tables::PART_ROW_FLOATS * id;
            let gpu = &parts.gpu[row..row + crate::tables::PART_ROW_FLOATS];
            let model = Mat4::from_cols_slice(&gpu[..16]);
            let color = Vec3::new(gpu[28], gpu[29], gpu[30]);
            let emission = Vec3::new(gpu[32], gpu[33], gpu[34]);
            let (albedo, emission) = match &part.material {
                MaterialRef::Unlit => (Vec3::ZERO, color),
                MaterialRef::LitFallback => (color, emission),
                MaterialRef::Retained(material) => {
                    let mean = self
                        .tables
                        .materials
                        .get(*material)
                        .and_then(|row| self.tables.material_means.get(&row.descriptor.id))
                        .copied()
                        .unwrap_or([1.0; 3]);
                    (color * Vec3::from(mean), emission)
                }
            };
            let cpu = &mesh.cpu;
            let first = part.first_index as usize;
            let end = (first + part.index_count as usize).min(cpu.indices.len());
            for corner in cpu.indices[first..end].as_chunks::<3>().0 {
                let [a, b, c] =
                    [0, 1, 2].map(|k| model.transform_point3(cpu.positions[corner[k] as usize]));
                // Mirrored parts wind the other way.
                let (b, c) = if state.mirrored { (c, b) } else { (b, c) };
                let e1 = b - a;
                let e2 = c - a;
                let normal = e1.cross(e2);
                if normal.length_squared() <= 0.0 {
                    continue;
                }
                let triangle = Triangle {
                    v0: a,
                    e1,
                    e2,
                    normal: normal.normalize(),
                    albedo: albedo.min(Vec3::ONE),
                    emission,
                    two_sided: state.class.double_sided,
                };
                if keep(&triangle) {
                    triangles.push(triangle);
                }
            }
        }
        triangles
    }

    /// Whether a part's world bounds are a change to the requested volume.
    pub(crate) fn probe_bounds_changed(&mut self, bounds: &Aabb) -> bool {
        !bounds.is_empty() && self.probes.mark_bounds(bounds.min, bounds.max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor(center: [f32; 3], extent: [f32; 3], spacing: f32) -> IndirectLightDescriptor {
        IndirectLightDescriptor {
            center,
            extent,
            spacing,
            bounces: 1,
            ambient: IndirectAmbient::Sky,
        }
    }

    #[test]
    fn probes_sit_on_a_world_lattice_shared_by_overlapping_volumes() {
        let grid = Grid::of(&descriptor([10.0, 2.0, -4.0], [3.5, 1.5, 3.5], 1.0));
        assert_eq!(grid.dims, [8, 4, 8]);
        assert_eq!(grid.min, Vec3::new(6.5, 0.5, -7.5));
        assert_eq!(grid.max(), Vec3::new(13.5, 3.5, -0.5));
        assert_eq!(grid.lattice(), [6, 0, -8]);
        // A spacing of 2 m puts probes at 0.5 + 2k, clear of 1 m voxel faces.
        let two = Grid::of(&descriptor([0.0, 0.0, 0.0], [4.0, 4.0, 4.0], 2.0));
        assert_eq!(two.min, Vec3::splat(-3.5));
        assert_eq!(two.dims, [4, 4, 4]);
        // Moved 3 m, the box holds the lattice probes 0.5 to 6.5: two cells on.
        let moved = Grid::of(&descriptor([3.0, 0.0, 0.0], [4.0, 4.0, 4.0], 2.0));
        assert_eq!(moved.min, Vec3::new(0.5, -3.5, -3.5));
        assert_eq!(moved.dims, [4, 4, 4]);
        assert_eq!(moved.lattice()[0] - two.lattice()[0], 2);
        for (index, coords) in [(0, [0, 0, 0]), (5, [1, 1, 0]), (63, [3, 3, 3])] {
            assert_eq!(two.coords(index), coords);
            assert_eq!(two.index(coords), index);
        }
        let uniform = Grid::uniform(Some(&grid), false);
        assert_eq!(&uniform[..4], &[6.5, 0.5, -7.5, 1.0]);
        assert_eq!(&uniform[4..], &[8.0, 4.0, 8.0, 1.0]);
        assert_eq!(Grid::uniform(Some(&grid), true)[7], 3.0);
        assert_eq!(Grid::uniform(None, true), [0.0; 8]);
    }

    #[test]
    fn bricks_are_world_cells_that_hold_every_probe_once() {
        let grid = Grid::of(&descriptor([0.0, 2.0, 0.0], [24.0, 4.0, 24.0], 2.0));
        assert_eq!(grid.dims, [24, 4, 24]);
        let bricks = Bricks::of(&grid);
        assert_eq!(bricks.b0, [-2, -1, -2]);
        assert_eq!(bricks.nb, [4, 2, 4]);
        assert_eq!(bricks.len(), 32);
        let mut seen = vec![0; grid.probes()];
        for brick in 0..bricks.len() {
            for cell in cells_in(bricks.ranges[brick]) {
                let (min, max) = bricks.cell_box(brick);
                let p = grid.position(cell);
                assert!(
                    p.cmpge(min).all() && p.cmplt(max).all(),
                    "{p} in {min}..{max}"
                );
                seen[grid.index(cell)] += 1;
            }
        }
        assert!(seen.iter().all(|count| *count == 1));
        assert_eq!(bricks.cell(bricks.index_of(IVec3::new(2, 1, 3))), [0, 0, 1]);
        // A box in one corner touches its cell and, widened by a brick, its
        // neighbours too.
        let touched = bricks.touching(Vec3::new(19.0, 1.0, 19.0), Vec3::new(21.0, 3.0, 21.0));
        assert_eq!(touched, vec![bricks.index_of(IVec3::new(3, 1, 3))]);
        let around = bricks.touching(Vec3::new(3.0, -15.0, 3.0), Vec3::new(37.0, 19.0, 37.0));
        assert_eq!(around.len(), 2 * 2 * 2);
        assert!(bricks
            .touching(Vec3::splat(100.0), Vec3::splat(101.0))
            .is_empty());
    }

    #[test]
    fn a_change_marks_the_bricks_it_reaches_and_a_light_those_in_its_range() {
        let mut volume = Volume::new(descriptor([0.0, 2.0, 0.0], [24.0, 4.0, 24.0], 2.0));
        assert!(volume.take_work().is_some());
        assert!(volume.take_work().is_none());
        assert!(volume.mark_bounds(Vec3::new(19.0, 1.0, 19.0), Vec3::new(21.0, 3.0, 21.0)));
        let work = volume.take_work().expect("marked");
        assert_eq!(work.rebuild.len(), 1);
        assert_eq!(work.bake.len(), 8);
        assert!(!work.outside);
        // A box reaching past the bricks rebuilds the outside too.
        assert!(volume.mark_bounds(Vec3::new(30.0, 0.0, 0.0), Vec3::new(40.0, 1.0, 1.0)));
        assert!(volume.take_work().expect("marked").outside);
        // A ranged point light marks the bricks within its range plus one.
        let mut row = [0.0_f32; LIGHT_ROW];
        row[3] = 3.0;
        row[4] = -20.0;
        row[5] = 2.0;
        row[6] = -20.0;
        row[7] = 6.0;
        volume.mark_light(&row);
        let work = volume.take_work().expect("marked");
        assert_eq!(work.bake.len(), 3 * 2 * 3);
        // An ambient row marks every brick.
        volume.mark_light(&[0.0; LIGHT_ROW]);
        assert_eq!(volume.take_work().expect("marked").bake.len(), 32);
    }

    #[test]
    fn light_rows_are_compared_with_their_duplicates() {
        let mut torch = [0.0_f32; LIGHT_ROW];
        torch[3] = 3.0;
        torch[7] = 6.0;
        let mut ambient = [0.0_f32; LIGHT_ROW];
        ambient[0] = 0.5;
        let two_torches: Vec<f32> = [torch, torch, ambient].concat();
        let one_torch: Vec<f32> = [torch, ambient].concat();
        // Removing one of two identical torches is a change where it stood.
        assert_eq!(changed_rows(&two_torches, &one_torch), vec![torch]);
        assert_eq!(changed_rows(&one_torch, &two_torches), vec![torch]);
        assert!(changed_rows(&one_torch, &one_torch).is_empty());
        let mut moved = torch;
        moved[4] = 10.0;
        let changed = changed_rows(&one_torch, &[moved, ambient].concat());
        assert_eq!(changed.len(), 2, "the old and the new place: {changed:?}");
    }

    #[test]
    fn a_moved_volume_keeps_its_probes_and_bakes_only_the_new_bricks() {
        let mut volume = Volume::new(descriptor([0.0, 2.0, 0.0], [24.0, 4.0, 24.0], 2.0));
        volume.take_work();
        for (index, sh) in volume.field.sh.iter_mut().enumerate() {
            *sh = [Vec3::splat(index as f32); 4];
        }
        volume.field.raw_valid.fill(true);
        volume.field.filled.fill(true);
        for state in &mut volume.states {
            state.baked = true;
            state.blas = Some(Arc::new(Bvh::build(Vec::new())));
        }
        volume.complete = true;
        let moved = volume.scrolled(descriptor([16.0, 2.0, 0.0], [24.0, 4.0, 24.0], 2.0));
        assert!(moved.complete);
        assert_eq!(moved.grid.dims, volume.grid.dims);
        // A probe at the same world position keeps its value.
        let probe = moved.grid.index([0, 1, 5]);
        let world = moved.grid.position([0, 1, 5]);
        let old = volume.grid.index([8, 1, 5]);
        assert_eq!(volume.grid.position([8, 1, 5]), world);
        assert_eq!(moved.field.sh[probe], volume.field.sh[old]);
        assert!(moved.field.raw_valid[probe]);
        // New probes take the nearest old value until their bricks bake.
        let new = moved.grid.index([23, 1, 5]);
        assert_eq!(
            moved.field.sh[new],
            volume.field.sh[volume.grid.index([23, 1, 5])]
        );
        assert!(moved.field.filled[new] && !moved.field.raw_valid[new]);
        // The new column of cells bakes and builds its BVHs; the column the
        // old volume only partly covered bakes again (its new probes hold
        // placeholders) with the BVHs it kept.
        let work = moved.clone_for_test().take_work().expect("new bricks");
        assert_eq!(work.bake.len(), 2 * (2 * 4));
        assert_eq!(work.rebuild.len(), 2 * 4);
        assert!(work.outside);
    }

    #[test]
    fn a_bake_is_due_after_the_scene_is_still_or_has_kept_moving_long_enough() {
        let mut schedule = Schedule::default();
        let start = Instant::now();
        assert!(!schedule.due(start));
        schedule.touch(start);
        assert!(!schedule.due(start + DEBOUNCE / 2));
        assert!(schedule.due(start + DEBOUNCE));
        // Every frame changes something: the first change's age decides.
        let mut now = start;
        while now < start + MAX_WAIT - DEBOUNCE / 2 {
            now += DEBOUNCE / 2;
            schedule.touch(now);
            assert!(!schedule.due(now), "{:?}", now - start);
        }
        assert!(schedule.due(start + MAX_WAIT));
    }

    impl Volume {
        fn clone_for_test(&self) -> Self {
            Self {
                request: self.request,
                grid: self.grid,
                bricks: self.bricks.clone(),
                states: self.states.clone(),
                field: self.field.clone(),
                outside: self.outside.clone(),
                outside_dirty: self.outside_dirty,
                complete: self.complete,
            }
        }
    }
}
