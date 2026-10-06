//! Per-chunk meshing cost of each surface mode on a dungeon-sized world.
//!
//! Run with:
//! `cargo run --release -p svc-mesh --example smooth_chunk_meshing`
//!
//! The world is 6 × 5 × 6 chunks of 16³ one-metre cells (180 chunks, the
//! size of a CraftSurvive dungeon): rock with carved tunnels and rooms, a
//! block of a second material and a pool of see-through water in the room.
//! Each mode meshes every resident chunk with its resident neighbours, as a
//! scene build does, and reports the chunks whose samples include water
//! separately. Reconstructed modes also mesh every chunk from the coarse
//! lattice a distant chunk is drawn from.

use std::time::{Duration, Instant};

use core_space::{ChunkCoord, ChunkDims, GridId, LocalVoxelCoord, VoxelGridSpec};
use core_voxel::{VoxelMaterialId, VoxelValue};
use svc_mesh::{
    mesh_chunk_coarse_in_world, mesh_chunk_in_world_with_options, SurfaceMeshOptions, SurfaceMode,
};
use svc_spatial::VoxelWorld;
use svc_volume::VoxelChunk;

const CHUNK: u32 = 16;
const CHUNKS: [i64; 3] = [6, 5, 6];
const ROUNDS: usize = 3;
const WATER: u16 = 3;

fn main() {
    let world = dungeon();
    let chunks: Vec<ChunkCoord> = world.resident_chunks().map(|(c, _)| c).collect();
    let watery: Vec<bool> = chunks.iter().map(|coord| samples_water(*coord)).collect();
    let water_chunks = watery.iter().filter(|watery| **watery).count();
    for mode in [
        SurfaceMode::GreedyCubes,
        SurfaceMode::MarchingCubes,
        SurfaceMode::DualContouring,
    ] {
        let options = SurfaceMeshOptions {
            mode,
            non_occluding: [WATER].into(),
            ..SurfaceMeshOptions::default()
        };
        let mut best = Duration::MAX;
        let mut best_water = Duration::MAX;
        let mut triangles = 0;
        for _ in 0..ROUNDS {
            let mut total = Duration::ZERO;
            let mut water = Duration::ZERO;
            triangles = 0;
            for (coord, watery) in chunks.iter().zip(&watery) {
                let started = Instant::now();
                let mesh = mesh_chunk_in_world_with_options(&world, *coord, &options)
                    .expect("resident")
                    .expect("meshes");
                let elapsed = started.elapsed();
                total += elapsed;
                if *watery {
                    water += elapsed;
                }
                triangles += mesh.indices.len() / 3;
            }
            best = best.min(total);
            best_water = best_water.min(water);
        }
        let per_chunk = |time: Duration, count: usize| time.as_secs_f64() * 1000.0 / count as f64;
        println!(
            "{:>15}: {:8.1} ms total, {:6.2} ms/chunk, {triangles} triangles; {water_chunks} chunks sampling water {:6.2} ms/chunk",
            mode.as_str(),
            best.as_secs_f64() * 1000.0,
            per_chunk(best, chunks.len()),
            per_chunk(best_water, water_chunks),
        );
        if mode == SurfaceMode::GreedyCubes {
            continue;
        }
        let mut best = Duration::MAX;
        let mut triangles = 0;
        for _ in 0..ROUNDS {
            let started = Instant::now();
            triangles = 0;
            for coord in &chunks {
                let mesh = mesh_chunk_coarse_in_world(&world, *coord, &options)
                    .expect("resident, reconstructed and even")
                    .expect("meshes");
                triangles += mesh.indices.len() / 3;
            }
            best = best.min(started.elapsed());
        }
        println!(
            "{:>15}  coarse: {:6.1} ms total, {:6.2} ms/chunk, {triangles} triangles, skirts included",
            "",
            best.as_secs_f64() * 1000.0,
            per_chunk(best, chunks.len()),
        );
    }
}

fn solid(x: i64, y: i64, z: i64) -> Option<u16> {
    let (fx, fy, fz) = (x as f64, y as f64, z as f64);
    let tunnel = ((fx * 0.11).sin() * 9.0 + 40.0 - fz).abs() < 3.5 && (fy - 30.0).abs() < 4.0;
    let room =
        ((fx - 48.0).powi(2) + (fy - 34.0).powi(2) * 2.0 + (fz - 48.0).powi(2)).sqrt() < 14.0;
    let ripple = (fx * 0.37).sin() + (fz * 0.29).cos() + (fy * 0.21).sin();
    let ground = fy < 60.0 + ripple * 3.0;
    if room && fy < 30.0 {
        return Some(WATER);
    }
    if !ground || tunnel || room {
        return None;
    }
    let block = (40..48).contains(&x) && (20..34).contains(&y) && (36..44).contains(&z);
    Some(if block { 2 } else { 1 })
}

/// Whether a chunk's samples, its own voxels and the one-voxel halo, include
/// water.
fn samples_water(coord: ChunkCoord) -> bool {
    let size = i64::from(CHUNK);
    let origin = [coord.x, coord.y, coord.z].map(|value| value * size);
    (-1..=size).any(|x| {
        (-1..=size).any(|y| {
            (-1..=size).any(|z| solid(origin[0] + x, origin[1] + y, origin[2] + z) == Some(WATER))
        })
    })
}

fn dungeon() -> VoxelWorld {
    let dims = ChunkDims::cubic(CHUNK).unwrap();
    let grid = VoxelGridSpec::new(GridId::new(0), 1.0, dims).unwrap();
    let mut world = VoxelWorld::new(grid);
    for cx in 0..CHUNKS[0] {
        for cy in 0..CHUNKS[1] {
            for cz in 0..CHUNKS[2] {
                let coord = ChunkCoord::new(cx, cy, cz);
                let mut chunk = VoxelChunk::from_spec(&grid);
                for x in 0..CHUNK {
                    for y in 0..CHUNK {
                        for z in 0..CHUNK {
                            let local = LocalVoxelCoord::new(x, y, z);
                            let voxel = grid.chunk_local_to_voxel(coord, local);
                            if let Some(material) = solid(voxel.x, voxel.y, voxel.z) {
                                chunk
                                    .set(local, VoxelValue::solid(VoxelMaterialId::new(material)))
                                    .unwrap();
                            }
                        }
                    }
                }
                world.insert(coord, chunk);
            }
        }
    }
    world
}
