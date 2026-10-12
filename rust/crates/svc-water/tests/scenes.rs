//! The kernel's headless test scenes (#9882): a dam break, rain pooling in
//! a hollow, steady flow over a tilted plane, a channel that overbanks onto its
//! floodplain in a storm, and a dam built mid-run.

use svc_water::{Boundary, FlowParams, Side, StepReport, WaterGrid, DEFAULT_COURANT};

const THREADS: usize = 4;

/// Advances `grid` by `seconds` in stable steps, calling `each` after every
/// step.
fn run(grid: &mut WaterGrid, seconds: f64, mut each: impl FnMut(&WaterGrid, &StepReport)) {
    let end = grid.time() + seconds;
    while grid.time() < end {
        let dt = grid
            .stable_step(DEFAULT_COURANT)
            .min((end - grid.time()) as f32);
        let report = grid.step(dt, THREADS);
        each(grid, &report);
    }
}

/// Discharge through `side` per second averaged over the last `seconds`.
fn mean_outflow(grid: &mut WaterGrid, side: Side, seconds: f64) -> f64 {
    let mut volume = 0.0;
    run(grid, seconds, |_, report| volume += report.outflow(side));
    volume / seconds
}

fn bed_from(width: usize, length: usize, f: impl Fn(usize, usize) -> f32) -> Vec<f32> {
    (0..length)
        .flat_map(|z| (0..width).map(move |x| (x, z)))
        .map(|(x, z)| f(x, z))
        .collect()
}

/// A frictionless dam break on a dry flat bed: the front runs downstream at a
/// plausible speed and no depth ever goes negative.
#[test]
fn dam_break_front_advances_without_negative_depth() {
    const CELLS: usize = 400;
    const CELL: f32 = 0.25;
    const STILL_DEPTH: f32 = 1.0;
    const DAM: usize = CELLS / 4;
    const SECONDS: f64 = 4.0;
    const FRONT_DEPTH: f32 = 1.0e-3;

    let mut grid = WaterGrid::new(CELLS, 3, CELL, vec![0.0; CELLS * 3]);
    grid.set_params(FlowParams {
        manning_n: 0.0,
        ..FlowParams::default()
    });
    for z in 0..3 {
        for x in 0..DAM {
            let i = grid.index(x, z);
            grid.depth_mut()[i] = STILL_DEPTH;
        }
    }
    let start_volume = grid.total_volume();
    let mut lowest = f32::MAX;
    run(&mut grid, SECONDS, |grid, _| {
        lowest = lowest.min(grid.depth().iter().copied().fold(f32::MAX, f32::min));
    });
    assert!(lowest >= 0.0, "a depth went negative: {lowest}");

    let front = (0..CELLS)
        .rev()
        .find(|&x| grid.depth()[grid.index(x, 1)] > FRONT_DEPTH)
        .expect("water left the reservoir");
    let travelled = (front + 1 - DAM) as f64 * f64::from(CELL);
    let speed = travelled / SECONDS;
    // Ritter's frictionless dry-bed front runs at 2 * sqrt(g * h0). Pipes
    // carry no momentum advection, so each newly wet face starts from rest
    // and the tip runs at about a third of that at any resolution (2.1 m/s
    // here from 1 m cells down to 0.125 m cells).
    let ritter = 2.0 * (9.81 * f64::from(STILL_DEPTH)).sqrt();
    eprintln!("dam break front {speed:.2} m/s, Ritter {ritter:.2} m/s");
    assert!(
        speed > 0.25 * ritter && speed < ritter,
        "front speed {speed:.2} m/s against Ritter's {ritter:.2} m/s"
    );
    let drift = (grid.total_volume() - start_volume).abs() / start_volume;
    assert!(drift < 1.0e-5, "walled volume drifted by {drift:e}");
}

/// Rain over a bowl runs down into it and settles to one flat surface holding
/// exactly what fell.
#[test]
fn rain_pools_flat_in_a_hollow() {
    const CELLS: usize = 48;
    const CELL: f32 = 1.0;
    const CURVATURE: f32 = 0.004;
    const RAIN_VOLUME: f64 = 60.0;
    const RAIN_SECONDS: f64 = 60.0;
    const SETTLE_SECONDS: f64 = 600.0;
    const FLAT: f32 = 2.0e-3;
    const WET: f32 = 0.01;

    let centre = (CELLS as f32 - 1.0) / 2.0;
    let bed = bed_from(CELLS, CELLS, |x, z| {
        let (dx, dz) = (x as f32 - centre, z as f32 - centre);
        CURVATURE * (dx * dx + dz * dz)
    });
    let mut grid = WaterGrid::new(CELLS, CELLS, CELL, bed);
    let area = f64::from(CELL * CELL) * grid.cell_count() as f64;
    let rate = (RAIN_VOLUME / RAIN_SECONDS / area) as f32;
    grid.sources_mut().fill(rate);
    let mut fallen = 0.0;
    run(&mut grid, RAIN_SECONDS, |_, report| {
        fallen += report.source_volume
    });
    grid.sources_mut().fill(0.0);
    run(&mut grid, SETTLE_SECONDS, |_, _| {});

    let error = (fallen - RAIN_VOLUME).abs() / RAIN_VOLUME;
    assert!(
        error < 1.0e-4,
        "{fallen} m³ fell, expected {RAIN_VOLUME} m³"
    );
    let held = (grid.total_volume() - fallen).abs() / fallen;
    assert!(
        held < 1.0e-4,
        "hollow holds {} m³ of {fallen} m³",
        grid.total_volume()
    );

    let wet: Vec<f32> = (0..grid.cell_count())
        .filter(|&i| grid.depth()[i] > WET)
        .map(|i| grid.surface(i))
        .collect();
    let (low, high) = wet
        .iter()
        .fold((f32::MAX, f32::MIN), |(lo, hi), &s| (lo.min(s), hi.max(s)));
    eprintln!(
        "hollow: {} wet cells, surface {low:.4}..{high:.4}",
        wet.len()
    );
    assert!(wet.len() > 200, "only {} wet cells", wet.len());
    assert!(high - low < FLAT, "surface spread {} m", high - low);
}

/// Steady inflow at the top of a tilted plane reaches a steady state that
/// leaves through the open foot at the inflow rate.
#[test]
fn tilted_plane_reaches_steady_outflow() {
    const WIDTH: usize = 64;
    const LENGTH: usize = 16;
    const CELL: f32 = 1.0;
    const SLOPE: f32 = 0.01;
    const INFLOW: f32 = 2.0;
    const SETTLE_SECONDS: f64 = 900.0;
    const MEASURE_SECONDS: f64 = 60.0;

    let bed = bed_from(WIDTH, LENGTH, |x, _| SLOPE * (WIDTH - x) as f32 * CELL);
    let mut grid = WaterGrid::new(WIDTH, LENGTH, CELL, bed);
    grid.set_boundary(Side::XMin, 0..LENGTH, Boundary::Inflow(INFLOW));
    grid.set_boundary(Side::XMax, 0..LENGTH, Boundary::Open);
    run(&mut grid, SETTLE_SECONDS, |_, _| {});
    let before = grid.total_volume();
    let out = mean_outflow(&mut grid, Side::XMax, MEASURE_SECONDS);
    let storage_change = (grid.total_volume() - before).abs() / before;
    eprintln!("plane: outflow {out:.4} m³/s, storage {before:.2} m³");
    assert!(
        (out - f64::from(INFLOW)).abs() < 0.01 * f64::from(INFLOW),
        "steady outflow {out} m³/s, inflow {INFLOW} m³/s"
    );
    assert!(
        storage_change < 1.0e-3,
        "storage still changing by {storage_change:e}"
    );
}

/// Channel scene: a straight channel with banks and a floodplain sloping
/// back to it along a valley. The storm scene a product reuses.
struct Valley {
    grid: WaterGrid,
    channel: std::ops::Range<usize>,
}

impl Valley {
    const WIDTH: usize = 96;
    const LENGTH: usize = 48;
    const CELL: f32 = 2.0;
    const VALLEY_SLOPE: f32 = 0.002;
    const CHANNEL_DEPTH: f32 = 1.5;
    const CROSS_SLOPE: f32 = 0.01;
    const CHANNEL_CELLS: usize = 4;

    fn new() -> Self {
        let first = (Self::LENGTH - Self::CHANNEL_CELLS) / 2;
        let channel = first..first + Self::CHANNEL_CELLS;
        let banks = channel.clone();
        let bed = bed_from(Self::WIDTH, Self::LENGTH, move |x, z| {
            let valley = Self::VALLEY_SLOPE * (Self::WIDTH - x) as f32 * Self::CELL;
            if banks.contains(&z) {
                return valley;
            }
            let from_bank = if z < banks.start {
                banks.start - z
            } else {
                z + 1 - banks.end
            };
            valley + Self::CHANNEL_DEPTH + Self::CROSS_SLOPE * from_bank as f32 * Self::CELL
        });
        let mut grid = WaterGrid::new(Self::WIDTH, Self::LENGTH, Self::CELL, bed);
        grid.set_boundary(Side::XMax, 0..Self::LENGTH, Boundary::Open);
        Self { grid, channel }
    }

    fn set_inflow(&mut self, discharge: f32) {
        self.grid.set_boundary(
            Side::XMin,
            self.channel.clone(),
            Boundary::Inflow(discharge),
        );
    }

    /// Water standing on the floodplain, m³.
    fn floodplain_volume(&self) -> f64 {
        let area = f64::from(Self::CELL * Self::CELL);
        (0..self.grid.cell_count())
            .filter(|&i| !self.channel.contains(&(i / Self::WIDTH)))
            .map(|i| f64::from(self.grid.depth()[i]) * area)
            .sum()
    }

    fn floodplain_wet_cells(&self, depth: f32) -> usize {
        (0..self.grid.cell_count())
            .filter(|&i| !self.channel.contains(&(i / Self::WIDTH)))
            .filter(|&i| self.grid.depth()[i] > depth)
            .count()
    }
}

#[test]
fn storm_overbanks_onto_the_floodplain_and_drains_back() {
    const BASE_FLOW: f32 = 5.0;
    const STORM_FLOW: f32 = 80.0;
    const BASE_SECONDS: f64 = 600.0;
    const STORM_SECONDS: f64 = 600.0;
    const RECESSION_SECONDS: f64 = 1800.0;
    const WET: f32 = 0.05;
    const DAMP: f32 = 0.01;

    let mut valley = Valley::new();
    valley.set_inflow(BASE_FLOW);
    run(&mut valley.grid, BASE_SECONDS, |_, _| {});
    let base_plain = valley.floodplain_wet_cells(DAMP);
    let base_out = mean_outflow(&mut valley.grid, Side::XMax, 60.0);
    eprintln!("base flow: {base_plain} floodplain cells damp, outflow {base_out:.2} m³/s");
    assert_eq!(base_plain, 0, "base flow left the channel");

    valley.set_inflow(STORM_FLOW);
    run(&mut valley.grid, STORM_SECONDS, |_, _| {});
    let storm_plain = valley.floodplain_wet_cells(WET);
    let storm_volume = valley.floodplain_volume();
    eprintln!("storm: {storm_plain} floodplain cells wet, {storm_volume:.0} m³ on the plain");
    assert!(
        storm_plain > Valley::WIDTH * 4,
        "storm wetted {storm_plain} floodplain cells"
    );

    valley.set_inflow(BASE_FLOW);
    run(&mut valley.grid, RECESSION_SECONDS, |_, _| {});
    let left_plain = valley.floodplain_wet_cells(WET);
    let left_volume = valley.floodplain_volume();
    eprintln!("recession: {left_plain} floodplain cells wet, {left_volume:.1} m³ on the plain");
    assert!(
        left_plain == 0,
        "{left_plain} floodplain cells still wet after the storm"
    );
    assert!(
        left_volume < 0.05 * storm_volume,
        "plain kept {left_volume:.1} m³"
    );
}

/// Raising a row of cells into a dam stops the flow below it and backs water
/// up behind it; the step after the edit runs on the new bed version.
#[test]
fn a_dam_built_mid_run_stops_the_flow_and_backs_water_up() {
    const WIDTH: usize = 64;
    const LENGTH: usize = 16;
    const CELL: f32 = 1.0;
    const SLOPE: f32 = 0.01;
    const INFLOW: f32 = 2.0;
    const DAM_X: usize = 40;
    const DAM_HEIGHT: f32 = 3.0;
    const BACKUP_SECONDS: f64 = 300.0;

    let bed = bed_from(WIDTH, LENGTH, |x, _| SLOPE * (WIDTH - x) as f32 * CELL);
    let mut grid = WaterGrid::new(WIDTH, LENGTH, CELL, bed);
    grid.set_boundary(Side::XMin, 0..LENGTH, Boundary::Inflow(INFLOW));
    grid.set_boundary(Side::XMax, 0..LENGTH, Boundary::Open);
    run(&mut grid, 900.0, |_, _| {});
    let behind = grid.index(DAM_X - 1, LENGTH / 2);
    let surface_before = grid.surface(behind);
    let version_before = grid.bed_version();

    let dam: Vec<(usize, f32)> = (0..LENGTH)
        .map(|z| {
            let i = grid.index(DAM_X, z);
            (i, grid.bed()[i] + DAM_HEIGHT)
        })
        .collect();
    let version = grid.edit_bed(dam);
    assert_eq!(version, version_before + 1);
    let dt = grid.stable_step(DEFAULT_COURANT);
    let report = grid.step(dt, THREADS);
    assert_eq!(report.bed_version, version);

    run(&mut grid, BACKUP_SECONDS, |_, _| {});
    let out = mean_outflow(&mut grid, Side::XMax, 60.0);
    let rise = grid.surface(behind) - surface_before;
    eprintln!("dam: outflow {out:.4} m³/s, surface behind rose {rise:.3} m");
    assert!(
        out < 0.01 * f64::from(INFLOW),
        "{out} m³/s still passes the dam"
    );
    let expected_rise = INFLOW * BACKUP_SECONDS as f32 / (DAM_X as f32 * LENGTH as f32);
    assert!(
        rise > 0.5 * expected_rise,
        "surface behind the dam rose {rise} m"
    );
    let over = grid.surface(behind) < grid.bed()[grid.index(DAM_X, LENGTH / 2)];
    assert!(over, "water topped the dam");
}

/// The same inputs give bit-identical results on any thread count.
#[test]
fn results_do_not_depend_on_thread_count() {
    let mut runs = Vec::new();
    for threads in [1, 3, 8] {
        let mut valley = Valley::new();
        valley.set_inflow(40.0);
        valley.grid.sources_mut()[17] = 0.01;
        let mut reports = Vec::new();
        for _ in 0..400 {
            let dt = valley.grid.stable_step(DEFAULT_COURANT);
            reports.push(valley.grid.step(dt, threads));
        }
        runs.push((
            valley.grid.depth().to_vec(),
            valley.grid.velocity().to_vec(),
            reports,
        ));
    }
    for other in &runs[1..] {
        assert!(runs[0] == *other, "thread count changed the result");
    }
}
