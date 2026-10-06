//! Cost of admitting a dungeon-sized world, per surface mode, in one residency
//! transaction and in small slices: the load pattern of a product filling a
//! space behind a loading screen. A transaction meshes each chunk it touches
//! once; a later slice remeshes the resident neighbours of what it admits.
//!
//! Run with:
//! `cargo run --release -p engine-spatial --example smooth_residency_load`

use std::time::{Duration, Instant};

use engine_spatial::{
    SurfaceMeshOptions, SurfaceMode, VoxelChunkIdentity, VoxelChunkPayload,
    VoxelChunkResidencyOperation, VoxelChunkResidencyService, VoxelCollisionScene,
};

const CHUNK: u32 = 16;
const CHUNKS: [i64; 3] = [6, 5, 6];
/// Chunks per transaction; `usize::MAX` admits the whole world at once.
const SLICES: [usize; 2] = [usize::MAX, 6];

fn main() {
    for slice in SLICES {
        for mode in [
            SurfaceMode::GreedyCubes,
            SurfaceMode::MarchingCubes,
            SurfaceMode::DualContouring,
        ] {
            admit(mode, slice);
        }
    }
}

fn admit(mode: SurfaceMode, slice: usize) {
    let mut scene = VoxelCollisionScene::from_solid_voxels_with_mesh_options(
        1.0,
        CHUNK,
        std::iter::empty(),
        SurfaceMeshOptions::with_mode(mode),
    )
    .unwrap();
    let mut chunks = Vec::new();
    for z in 0..CHUNKS[2] {
        for y in 0..CHUNKS[1] {
            for x in 0..CHUNKS[0] {
                chunks.push([x, y, z]);
            }
        }
    }
    let mut total = Duration::ZERO;
    let mut longest = Duration::ZERO;
    let mut meshing = Duration::ZERO;
    let mut rebuilt = 0;
    for slice in chunks.chunks(slice) {
        let operations: Vec<_> = slice
            .iter()
            .map(|&[x, y, z]| VoxelChunkResidencyOperation::Admit {
                chunk: VoxelChunkIdentity::new(x, y, z),
                payload: payload([x, y, z]),
            })
            .collect();
        let started = Instant::now();
        let receipt = VoxelChunkResidencyService::apply(&mut scene, &operations).unwrap();
        let elapsed = started.elapsed();
        total += elapsed;
        longest = longest.max(elapsed);
        rebuilt += receipt.rebuilt_mesh_chunks;
        meshing += Duration::from_micros(scene.mesh_update().mesh_microseconds);
    }
    let pattern = if slice >= chunks.len() {
        "at once".to_owned()
    } else {
        format!("slices of {slice}")
    };
    println!(
        "{:>15} {pattern:>12}: {:6.1} ms total, {:6.1} ms longest call, {:6.1} ms meshing (summed over threads), {rebuilt} chunk meshes for {} chunks",
        mode.as_str(),
        total.as_secs_f64() * 1000.0,
        longest.as_secs_f64() * 1000.0,
        meshing.as_secs_f64() * 1000.0,
        chunks.len()
    );
}

fn payload(chunk: [i64; 3]) -> VoxelChunkPayload {
    let mut slots = Vec::with_capacity((CHUNK * CHUNK * CHUNK) as usize);
    for z in 0..i64::from(CHUNK) {
        for y in 0..i64::from(CHUNK) {
            for x in 0..i64::from(CHUNK) {
                let size = i64::from(CHUNK);
                slots.push(solid(
                    chunk[0] * size + x,
                    chunk[1] * size + y,
                    chunk[2] * size + z,
                ));
            }
        }
    }
    VoxelChunkPayload::new([CHUNK; 3], slots)
}

fn solid(x: i64, y: i64, z: i64) -> u16 {
    let (fx, fy, fz) = (x as f64, y as f64, z as f64);
    let tunnel = ((fx * 0.11).sin() * 9.0 + 40.0 - fz).abs() < 3.5 && (fy - 30.0).abs() < 4.0;
    let room =
        ((fx - 48.0).powi(2) + (fy - 34.0).powi(2) * 2.0 + (fz - 48.0).powi(2)).sqrt() < 14.0;
    let ripple = (fx * 0.37).sin() + (fz * 0.29).cos() + (fy * 0.21).sin();
    if fy >= 60.0 + ripple * 3.0 || tunnel || room {
        0
    } else {
        1
    }
}
