//! Milliseconds per kernel step by grid size and thread count (#9882).
//!
//! `cargo run --release -p svc-water --example water_step_cost [threads...]`
//!
//! Every cell is wet: a lake over rolling ground, held at a fixed stage along
//! half of one side, with a river entering on another and leaving over an open
//! edge, so each face does the full pipe update. `dt` is the stable step the
//! grid chose; ms per simulated second is the cost of keeping up with time.
//! Report only; nothing here is a gate.

use std::time::Instant;
use svc_water::{Boundary, Side, WaterGrid, DEFAULT_COURANT};

const SIZES: [usize; 4] = [128, 256, 512, 1024];
const CELL: f32 = 2.0;
const HILL_HEIGHT: f32 = 0.6;
const HILL_WAVELENGTH_CELLS: f32 = 37.0;
const LAKE_SURFACE: f32 = 1.5;
const RIVER_DISCHARGE: f32 = 50.0;
const WARM_STEPS: usize = 10;
const MEASURED_CELL_STEPS: usize = 200_000_000;
const MIN_STEPS: usize = 20;

fn main() {
    let available = std::thread::available_parallelism().map_or(1, |n| n.get());
    let threads: Vec<usize> = match std::env::args().skip(1).map(|a| a.parse()).collect() {
        Ok(list) if !Vec::<usize>::is_empty(&list) => list,
        _ => vec![1, 2, 4, 8, available],
    };
    // Bed, depth and source, four face outflows, two velocity components.
    let bytes_per_cell = 3 * std::mem::size_of::<f32>()
        + std::mem::size_of::<[f32; 4]>()
        + std::mem::size_of::<[f32; 2]>();
    println!("resident bytes per cell: {bytes_per_cell}");
    println!("cells\tthreads\tms/step\tns/cell\tdt s\tms per simulated s");
    for size in SIZES {
        for &t in &threads {
            let mut grid = lake(size);
            for _ in 0..WARM_STEPS {
                let dt = grid.stable_step(DEFAULT_COURANT);
                grid.step(dt, t);
            }
            let steps = (MEASURED_CELL_STEPS / (size * size)).max(MIN_STEPS);
            let dt = grid.stable_step(DEFAULT_COURANT);
            let start = Instant::now();
            for _ in 0..steps {
                grid.step(dt, t);
            }
            let ms = start.elapsed().as_secs_f64() * 1000.0 / steps as f64;
            let ns_cell = ms * 1.0e6 / (size * size) as f64;
            let per_second = ms / f64::from(dt);
            println!("{size}²\t{t}\t{ms:.3}\t{ns_cell:.2}\t{dt:.3}\t{per_second:.1}");
        }
    }
}

fn lake(size: usize) -> WaterGrid {
    let bed = (0..size * size)
        .map(|i| {
            let (x, z) = ((i % size) as f32, (i / size) as f32);
            let phase = std::f32::consts::TAU / HILL_WAVELENGTH_CELLS;
            HILL_HEIGHT * ((x * phase).sin() * (z * phase * 0.7).cos())
        })
        .collect();
    let mut grid = WaterGrid::new(size, size, CELL, bed);
    grid.fill_to_surface(LAKE_SURFACE);
    grid.set_boundary(Side::XMin, 0..size / 8, Boundary::Inflow(RIVER_DISCHARGE));
    grid.set_boundary(Side::XMax, 0..size, Boundary::Open);
    grid.set_boundary(Side::ZMin, 0..size / 2, Boundary::Stage(LAKE_SURFACE));
    grid
}
