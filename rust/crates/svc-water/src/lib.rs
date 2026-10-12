//! Column-water depth-and-flow kernel over a bed height grid.
//!
//! # Lane
//!
//! `rust-service` — the generic solver behind the Engine's local water fields.
//! It knows a regular grid of cells with a bed height and a water depth each,
//! and nothing about where the grid sits in a world, what the water means, how
//! it is drawn, or when it is advanced. One surface per cell: stacked water is
//! outside the model.
//!
//! # Model
//!
//! Virtual pipes (Mei, Decaudin and Hu 2007; Šťava et al. 2008). Each cell keeps
//! a non-negative outflow discharge through each of its four faces. A step
//! accelerates every face flow by the surface difference across it, damps it
//! with Manning friction, scales a cell's outflows down so they never take more
//! water than the cell holds, then moves the water. Two choices differ from
//! the papers' constant pipe area:
//!
//! - The pipe cross-section is the face width times the water depth above the
//!   higher of the two beds (the sill). Gravity waves then travel at
//!   `sqrt(g * depth)` as in shallow water, and water in a hollow only leaves
//!   over the rim by the depth that stands above it.
//! - Friction is Manning's, applied semi-implicitly so thin sheets on slopes
//!   stay stable and slow, and deep channels run faster than floodplains.
//!
//! Wet and dry need no special case: a face with less than
//! [`FlowParams::dry_depth`] of water above its sill carries nothing, and the
//! outflow scaling keeps every depth non-negative.
//!
//! # Determinism
//!
//! A step reads only the state the previous step left and writes each cell
//! from its own neighbourhood, in two phases (face flows, then depths). The
//! parallel split hands each thread a fixed band of whole rows; neighbouring
//! rows are read from the shared previous phase, never written across bands.
//! Totals are summed per row and then across rows in row order, so the result
//! is bit-identical for any thread count.
//!
//! # Units
//!
//! Metres, seconds, cubic metres. Grid `x` runs along a row and `z` across rows;
//! cell `(x, z)` is at index `z * width + x`. Sources are depth rates in m/s
//! (rain is positive, a drain negative); boundary inflows are discharges in m³/s.

#![forbid(unsafe_code)]

use std::ops::Range;

/// Standard gravity, m/s².
pub const STANDARD_GRAVITY: f32 = 9.81;

/// Manning roughness of a natural earth channel, s/m^(1/3).
pub const NATURAL_CHANNEL_MANNING_N: f32 = 0.03;

/// Water thinner than this over a face's sill does not flow through it, and a
/// cell holding less reads as dry with zero velocity.
pub const DEFAULT_DRY_DEPTH: f32 = 1.0e-4;

/// Courant number [`WaterGrid::stable_step`] callers normally use.
pub const DEFAULT_COURANT: f32 = 0.5;

/// Flow constants for a grid.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlowParams {
    /// Gravity, m/s².
    pub gravity: f32,
    /// Manning roughness, s/m^(1/3). Zero is frictionless.
    pub manning_n: f32,
    /// See [`DEFAULT_DRY_DEPTH`].
    pub dry_depth: f32,
}

impl Default for FlowParams {
    fn default() -> Self {
        Self {
            gravity: STANDARD_GRAVITY,
            manning_n: NATURAL_CHANNEL_MANNING_N,
            dry_depth: DEFAULT_DRY_DEPTH,
        }
    }
}

/// One edge of the grid. The discriminant is the face index in a cell's
/// outflow array: the face a cell on that edge uses to leave the grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    /// The `x = width - 1` edge.
    XMax = 0,
    /// The `x = 0` edge.
    XMin = 1,
    /// The `z = length - 1` edge.
    ZMax = 2,
    /// The `z = 0` edge.
    ZMin = 3,
}

impl Side {
    /// All sides in face-index order, the order of per-side report arrays.
    pub const ALL: [Side; 4] = [Side::XMax, Side::XMin, Side::ZMax, Side::ZMin];

    /// Index into per-side arrays.
    pub const fn index(self) -> usize {
        self as usize
    }
}

/// What lies beyond a run of edge cells.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Boundary {
    /// Nothing leaves or enters. Every edge starts as a wall.
    Wall,
    /// A reservoir held at this surface height outside each edge cell (a sea,
    /// a lake, or a river stage from the product's network). Water flows in or
    /// out depending on which surface is higher.
    Stage(f32),
    /// This total discharge, m³/s, enters spread evenly over the run of cells.
    /// Nothing leaves through an inflow edge.
    Inflow(f32),
    /// The valley goes on: the ground beyond continues the slope of the last
    /// two cells and carries the edge cell's depth, so water leaves at the
    /// rate that slope drives (normal flow) and a region cut out of a larger
    /// landscape drains as if it were not cut. Nothing enters.
    Open,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Edge {
    Wall,
    Stage(f32),
    /// Discharge through this one cell's face, m³/s.
    Inflow(f32),
    Open,
}

/// The water that a step moved, and the state it left.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StepReport {
    /// Length of the step, s.
    pub dt: f32,
    /// Simulated time after the step, s.
    pub time: f64,
    /// Bed version the step ran on.
    pub bed_version: u64,
    /// Water in the grid after the step, m³.
    pub total_volume: f64,
    /// Water positive sources added, m³.
    pub source_volume: f64,
    /// Water negative sources removed, m³ (never more than was there).
    pub sink_volume: f64,
    /// Water that entered through each side, m³, indexed by [`Side::index`].
    pub boundary_inflow: [f64; 4],
    /// Water that left through each side, m³, indexed by [`Side::index`].
    pub boundary_outflow: [f64; 4],
    /// Cells deeper than the dry depth after the step.
    pub wet_cells: usize,
    /// Fastest wave speed plus flow speed after the step, Stage reservoirs
    /// included, m/s.
    pub max_celerity: f32,
}

impl StepReport {
    pub fn inflow(&self, side: Side) -> f64 {
        self.boundary_inflow[side.index()]
    }

    pub fn outflow(&self, side: Side) -> f64 {
        self.boundary_outflow[side.index()]
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct RowTotals {
    volume: f64,
    source: f64,
    sink: f64,
    inflow: [f64; 4],
    outflow: [f64; 4],
    wet: usize,
    max_celerity: f32,
}

/// Face offsets in face-index order: +x, -x, +z, -z.
const FACE_DX: [isize; 4] = [1, -1, 0, 0];
const FACE_DZ: [isize; 4] = [0, 0, 1, -1];
/// The face of the neighbour that points back at this cell.
const OPPOSITE: [usize; 4] = [1, 0, 3, 2];

/// A grid of column water over a bed.
#[derive(Debug, Clone)]
pub struct WaterGrid {
    width: usize,
    length: usize,
    cell_size: f32,
    params: FlowParams,
    bed: Vec<f32>,
    depth: Vec<f32>,
    source: Vec<f32>,
    /// Outflow discharge through each face, m³/s, in face-index order.
    outflow: Vec<[f32; 4]>,
    /// Depth-averaged velocity (x, z), m/s.
    velocity: Vec<[f32; 2]>,
    /// Per side, one entry per edge cell along it.
    edges: [Vec<Edge>; 4],
    /// Discharge entering each edge cell from beyond its side, m³/s.
    edge_inflow: [Vec<f32>; 4],
    row_totals: Vec<RowTotals>,
    bed_version: u64,
    time: f64,
    max_celerity: Option<f32>,
}

impl WaterGrid {
    /// A dry grid of `width` by `length` cells of `cell_size` metres, on the
    /// given bed heights (row-major, `width * length` of them), walled on
    /// every side.
    ///
    /// # Panics
    /// If a dimension is zero, the cell size is not positive, or `bed` has the
    /// wrong length.
    pub fn new(width: usize, length: usize, cell_size: f32, bed: Vec<f32>) -> Self {
        assert!(
            width > 0 && length > 0,
            "water grid needs at least one cell"
        );
        assert!(cell_size > 0.0, "water grid cell size must be positive");
        assert_eq!(bed.len(), width * length, "bed heights must cover the grid");
        let cells = width * length;
        Self {
            width,
            length,
            cell_size,
            params: FlowParams::default(),
            bed,
            depth: vec![0.0; cells],
            source: vec![0.0; cells],
            outflow: vec![[0.0; 4]; cells],
            velocity: vec![[0.0; 2]; cells],
            edges: [
                vec![Edge::Wall; length],
                vec![Edge::Wall; length],
                vec![Edge::Wall; width],
                vec![Edge::Wall; width],
            ],
            edge_inflow: [
                vec![0.0; length],
                vec![0.0; length],
                vec![0.0; width],
                vec![0.0; width],
            ],
            row_totals: vec![RowTotals::default(); length],
            bed_version: 0,
            time: 0.0,
            max_celerity: None,
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn length(&self) -> usize {
        self.length
    }

    pub fn cell_count(&self) -> usize {
        self.width * self.length
    }

    pub fn cell_size(&self) -> f32 {
        self.cell_size
    }

    pub fn index(&self, x: usize, z: usize) -> usize {
        debug_assert!(x < self.width && z < self.length);
        z * self.width + x
    }

    /// Cells along one side.
    pub fn side_len(&self, side: Side) -> usize {
        match side {
            Side::XMax | Side::XMin => self.length,
            Side::ZMax | Side::ZMin => self.width,
        }
    }

    pub fn params(&self) -> FlowParams {
        self.params
    }

    pub fn set_params(&mut self, params: FlowParams) {
        self.params = params;
        self.max_celerity = None;
    }

    pub fn bed(&self) -> &[f32] {
        &self.bed
    }

    pub fn depth(&self) -> &[f32] {
        &self.depth
    }

    /// Velocity per cell, (x, z) in m/s, as of the last step.
    pub fn velocity(&self) -> &[[f32; 2]] {
        &self.velocity
    }

    pub fn surface(&self, index: usize) -> f32 {
        self.bed[index] + self.depth[index]
    }

    pub fn is_wet(&self, index: usize) -> bool {
        self.depth[index] > self.params.dry_depth
    }

    /// Changes whenever the bed does; a step reports the version it ran on.
    pub fn bed_version(&self) -> u64 {
        self.bed_version
    }

    /// Simulated seconds advanced so far.
    pub fn time(&self) -> f64 {
        self.time
    }

    pub fn total_volume(&self) -> f64 {
        let area = f64::from(self.cell_size * self.cell_size);
        self.depth.iter().map(|&d| f64::from(d)).sum::<f64>() * area
    }

    /// Water depths, for placing or removing water directly.
    pub fn depth_mut(&mut self) -> &mut [f32] {
        self.max_celerity = None;
        &mut self.depth
    }

    /// Fills every cell whose bed is below `surface` up to it.
    pub fn fill_to_surface(&mut self, surface: f32) {
        for (depth, &bed) in self.depth.iter_mut().zip(&self.bed) {
            *depth = depth.max(surface - bed);
        }
        self.max_celerity = None;
    }

    /// Per-cell depth rates, m/s: positive adds water, negative drains it.
    /// They stay set until changed.
    pub fn sources_mut(&mut self) -> &mut [f32] {
        &mut self.source
    }

    /// Sets new heights for some cells, given as `(index, height)`, and bumps
    /// the bed version once if any height changed. Water depth is kept, so a
    /// raised cell lifts its water and it runs off. Returns the bed version.
    pub fn edit_bed(&mut self, edits: impl IntoIterator<Item = (usize, f32)>) -> u64 {
        let mut changed = false;
        for (index, height) in edits {
            if self.bed[index] != height {
                self.bed[index] = height;
                changed = true;
            }
        }
        if changed {
            self.bed_version += 1;
            self.max_celerity = None;
        }
        self.bed_version
    }

    /// Replaces every bed height. Returns the bumped bed version.
    ///
    /// # Panics
    /// If `bed` does not cover the grid.
    pub fn replace_bed(&mut self, bed: &[f32]) -> u64 {
        self.bed.copy_from_slice(bed);
        self.bed_version += 1;
        self.max_celerity = None;
        self.bed_version
    }

    /// Sets the condition beyond the edge cells `cells` of `side` (indices
    /// along the side: `z` for the x sides, `x` for the z sides).
    ///
    /// # Panics
    /// If the range runs past the side.
    pub fn set_boundary(&mut self, side: Side, cells: Range<usize>, boundary: Boundary) {
        let edge = match boundary {
            Boundary::Wall => Edge::Wall,
            Boundary::Stage(surface) => Edge::Stage(surface),
            Boundary::Inflow(discharge) => Edge::Inflow(discharge / cells.len().max(1) as f32),
            Boundary::Open => Edge::Open,
        };
        self.edges[side.index()][cells].fill(edge);
        self.max_celerity = None;
    }

    /// The longest step a Courant number of `courant` allows, s. A dry grid
    /// assumes a wave as deep as one cell is wide, so water arriving through
    /// a boundary is not met by one huge step.
    pub fn stable_step(&mut self, courant: f32) -> f32 {
        let floor = (self.params.gravity * self.cell_size).sqrt();
        let celerity = match self.max_celerity {
            Some(c) => c,
            None => {
                let c = self.measure_celerity();
                self.max_celerity = Some(c);
                c
            }
        };
        courant * self.cell_size / celerity.max(floor)
    }

    fn measure_celerity(&self) -> f32 {
        let g = self.params.gravity;
        let mut max = self.stage_celerity();
        for (&d, v) in self.depth.iter().zip(&self.velocity) {
            max = max.max(celerity(g, d, *v));
        }
        max
    }

    /// The fastest wave a Stage reservoir sends in: as deep as its surface
    /// stands over the edge cell's bed.
    fn stage_celerity(&self) -> f32 {
        let g = self.params.gravity;
        let mut max = 0.0f32;
        for side in Side::ALL {
            for (k, edge) in self.edges[side.index()].iter().enumerate() {
                if let Edge::Stage(surface) = *edge {
                    let bed = self.bed[edge_cell(self.width, self.length, side, k)];
                    max = max.max((g * (surface - bed).max(0.0)).sqrt());
                }
            }
        }
        max
    }

    /// Advances the water by `dt` seconds on up to `threads` threads. The
    /// caller keeps `dt` within [`WaterGrid::stable_step`].
    pub fn step(&mut self, dt: f32, threads: usize) -> StepReport {
        let width = self.width;
        let bands = band_rows(self.length, threads);
        let band_cells = bands * width;
        let ctx = FaceContext {
            width,
            length: self.length,
            area: self.cell_size * self.cell_size,
            params: self.params,
            dt,
            bed: &self.bed,
            edges: &self.edges,
        };

        // Phase 1: face flows from the current surfaces.
        {
            let depth = &self.depth;
            let mut work: Vec<(usize, &mut [[f32; 4]])> = self
                .outflow
                .chunks_mut(band_cells)
                .enumerate()
                .map(|(band, cells)| (band * band_cells, cells))
                .collect();
            run_bands(&mut work, |(first, cells)| {
                ctx.update_faces(*first, cells, depth)
            });
        }

        // Inflow from beyond each side: Stage reservoirs accelerate like any
        // face, Inflow edges deliver their discharge.
        for side in Side::ALL {
            let inflows = &mut self.edge_inflow[side.index()];
            for (k, inflow) in inflows.iter_mut().enumerate() {
                *inflow = match self.edges[side.index()][k] {
                    Edge::Stage(outside) => {
                        let cell = edge_cell(width, self.length, side, k);
                        let bed = self.bed[cell];
                        let surface = bed + self.depth[cell];
                        pipe(&ctx, *inflow, outside, surface, bed, bed)
                    }
                    Edge::Inflow(discharge) => discharge.max(0.0),
                    Edge::Wall | Edge::Open => 0.0,
                };
            }
        }

        // Phase 2: move the water and read velocities.
        {
            let outflow = &self.outflow;
            let edge_inflow = &self.edge_inflow;
            let source = &self.source;
            let ctx = &ctx;
            let mut work: Vec<_> = self
                .depth
                .chunks_mut(band_cells)
                .zip(self.velocity.chunks_mut(band_cells))
                .zip(self.row_totals.chunks_mut(bands))
                .enumerate()
                .map(|(band, ((depth, velocity), totals))| (band * bands, depth, velocity, totals))
                .collect();
            run_bands(&mut work, |(first_row, depth, velocity, totals)| {
                ctx.move_water(
                    *first_row,
                    depth,
                    velocity,
                    totals,
                    outflow,
                    edge_inflow,
                    source,
                )
            });
        }

        let mut sum = RowTotals::default();
        for row in &self.row_totals {
            sum.volume += row.volume;
            sum.source += row.source;
            sum.sink += row.sink;
            for side in 0..4 {
                sum.inflow[side] += row.inflow[side];
                sum.outflow[side] += row.outflow[side];
            }
            sum.wet += row.wet;
            sum.max_celerity = sum.max_celerity.max(row.max_celerity);
        }
        self.time += f64::from(dt);
        sum.max_celerity = sum.max_celerity.max(self.stage_celerity());
        self.max_celerity = Some(sum.max_celerity);
        StepReport {
            dt,
            time: self.time,
            bed_version: self.bed_version,
            total_volume: sum.volume,
            source_volume: sum.source,
            sink_volume: sum.sink,
            boundary_inflow: sum.inflow,
            boundary_outflow: sum.outflow,
            wet_cells: sum.wet,
            max_celerity: sum.max_celerity,
        }
    }
}

/// The index of the `k`th cell along `side`.
fn edge_cell(width: usize, length: usize, side: Side, k: usize) -> usize {
    match side {
        Side::XMax => k * width + width - 1,
        Side::XMin => k * width,
        Side::ZMax => (length - 1) * width + k,
        Side::ZMin => k,
    }
}

/// Rows per band for `threads` threads.
fn band_rows(rows: usize, threads: usize) -> usize {
    rows.div_ceil(threads.clamp(1, rows))
}

fn run_bands<T: Send>(work: &mut [T], task: impl Fn(&mut T) + Sync) {
    if let [only] = work {
        task(only);
        return;
    }
    std::thread::scope(|scope| {
        let task = &task;
        let (first, rest) = work.split_first_mut().expect("at least one band");
        for item in rest {
            scope.spawn(move || task(item));
        }
        task(first);
    });
}

fn celerity(gravity: f32, depth: f32, velocity: [f32; 2]) -> f32 {
    let speed = (velocity[0] * velocity[0] + velocity[1] * velocity[1]).sqrt();
    speed + (gravity * depth.max(0.0)).sqrt()
}

/// Read-only state shared by every band of a step.
struct FaceContext<'a> {
    width: usize,
    length: usize,
    area: f32,
    params: FlowParams,
    dt: f32,
    bed: &'a [f32],
    edges: &'a [Vec<Edge>; 4],
}

/// The new outflow through one face from a cell with surface `from` and bed
/// `from_bed` towards one with surface `to` and bed `to_bed`.
fn pipe(ctx: &FaceContext<'_>, flow: f32, from: f32, to: f32, from_bed: f32, to_bed: f32) -> f32 {
    let sill_depth = from.max(to) - from_bed.max(to_bed);
    if sill_depth <= ctx.params.dry_depth {
        return 0.0;
    }
    let g = ctx.params.gravity;
    let accelerated = flow + ctx.dt * g * sill_depth * (from - to);
    if accelerated <= 0.0 {
        return 0.0;
    }
    let n = ctx.params.manning_n;
    if n == 0.0 {
        return accelerated;
    }
    let face_width = ctx.area.sqrt();
    let speed = flow / (face_width * sill_depth);
    let friction = g * n * n * speed / sill_depth.powf(4.0 / 3.0);
    accelerated / (1.0 + ctx.dt * friction)
}

impl FaceContext<'_> {
    /// The cell's neighbour through `face` as (surface, bed), or `None` when
    /// nothing can leave that way.
    fn neighbour(&self, depth: &[f32], x: usize, z: usize, face: usize) -> Option<(f32, f32)> {
        let nx = x as isize + FACE_DX[face];
        let nz = z as isize + FACE_DZ[face];
        if nx >= 0 && nz >= 0 && (nx as usize) < self.width && (nz as usize) < self.length {
            let j = nz as usize * self.width + nx as usize;
            return Some((self.bed[j] + depth[j], self.bed[j]));
        }
        let along = if face < 2 { z } else { x };
        let i = z * self.width + x;
        let bed = self.bed[i];
        match self.edges[face][along] {
            Edge::Stage(surface) => Some((surface, bed)),
            Edge::Open => {
                let inner = self
                    .inner_neighbour(x, z, face)
                    .map_or(bed, |j| self.bed[j]);
                let beyond = bed + (bed - inner);
                Some((beyond + depth[i], beyond))
            }
            Edge::Wall | Edge::Inflow(_) => None,
        }
    }

    /// The cell on the opposite side from `face`, if the grid has one.
    fn inner_neighbour(&self, x: usize, z: usize, face: usize) -> Option<usize> {
        let nx = x as isize - FACE_DX[face];
        let nz = z as isize - FACE_DZ[face];
        let inside =
            nx >= 0 && nz >= 0 && (nx as usize) < self.width && (nz as usize) < self.length;
        inside.then(|| nz as usize * self.width + nx as usize)
    }

    fn update_faces(&self, first: usize, cells: &mut [[f32; 4]], depth: &[f32]) {
        for (offset, faces) in cells.iter_mut().enumerate() {
            let i = first + offset;
            let (x, z) = (i % self.width, i / self.width);
            let bed = self.bed[i];
            let surface = bed + depth[i];
            let mut total = 0.0;
            for (face, flow) in faces.iter_mut().enumerate() {
                *flow = match self.neighbour(depth, x, z, face) {
                    Some((to, to_bed)) => pipe(self, *flow, surface, to, bed, to_bed),
                    None => 0.0,
                };
                total += *flow;
            }
            let available = depth[i] * self.area;
            let leaving = total * self.dt;
            if leaving > available {
                let scale = available / leaving;
                for flow in faces.iter_mut() {
                    *flow *= scale;
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn move_water(
        &self,
        first_row: usize,
        depth: &mut [f32],
        velocity: &mut [[f32; 2]],
        totals: &mut [RowTotals],
        outflow: &[[f32; 4]],
        edge_inflow: &[Vec<f32>; 4],
        source: &[f32],
    ) {
        let w = self.width;
        let dt = self.dt;
        let face_width = self.area.sqrt();
        let dt64 = f64::from(dt);
        let area64 = f64::from(self.area);
        for (row, total) in totals.iter_mut().enumerate() {
            let z = first_row + row;
            *total = RowTotals::default();
            for x in 0..w {
                let i = z * w + x;
                let local = row * w + x;
                let out = outflow[i];
                // Discharge arriving through each face, in face-index order.
                let mut arriving = [0.0f32; 4];
                for face in 0..4 {
                    let nx = x as isize + FACE_DX[face];
                    let nz = z as isize + FACE_DZ[face];
                    arriving[face] =
                        if nx >= 0 && nz >= 0 && (nx as usize) < w && (nz as usize) < self.length {
                            outflow[nz as usize * w + nx as usize][OPPOSITE[face]]
                        } else {
                            let along = if face < 2 { z } else { x };
                            let edge_in = edge_inflow[face][along];
                            total.inflow[face] += f64::from(edge_in) * dt64;
                            total.outflow[face] += f64::from(out[face]) * dt64;
                            edge_in
                        };
                }
                let net: f32 = arriving.iter().sum::<f32>() - out.iter().sum::<f32>();
                let before = depth[local];
                let mut after = (before + net * dt / self.area).max(0.0);
                let rate = source[i];
                if rate != 0.0 {
                    let added = rate * dt;
                    if after + added < 0.0 {
                        total.sink += f64::from(after) * area64;
                        after = 0.0;
                    } else {
                        if added > 0.0 {
                            total.source += f64::from(added) * area64;
                        } else {
                            total.sink += f64::from(-added) * area64;
                        }
                        after += added;
                    }
                }
                depth[local] = after;

                let mean = 0.5 * (before + after);
                velocity[local] = if mean > self.params.dry_depth {
                    // Eastward discharge through the -x and +x faces, averaged.
                    let qx = 0.5 * ((arriving[1] - out[1]) + (out[0] - arriving[0]));
                    let qz = 0.5 * ((arriving[3] - out[3]) + (out[2] - arriving[2]));
                    let section = face_width * mean;
                    [qx / section, qz / section]
                } else {
                    [0.0, 0.0]
                };
                total.volume += f64::from(after) * area64;
                if after > self.params.dry_depth {
                    total.wet += 1;
                }
                total.max_celerity =
                    total
                        .max_celerity
                        .max(celerity(self.params.gravity, after, velocity[local]));
            }
        }
    }
}
