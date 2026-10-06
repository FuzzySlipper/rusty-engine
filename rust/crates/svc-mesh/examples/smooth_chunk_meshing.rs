//! Per-chunk meshing cost of each surface mode on a dungeon-sized world.
//!
//! Run with:
//! `cargo run --release -p svc-mesh --example smooth_chunk_meshing`
//!
//! The world is 6 × 5 × 6 chunks of 16³ one-metre cells (180 chunks, the
//! size of a CraftSurvive dungeon): rock with carved tunnels and rooms and a
//! block of a second material. Each mode meshes every resident chunk with
//! its resident neighbours, as a scene build does.

use std::time::Instant;

use core_space::{ChunkCoord, ChunkDims, GridId, LocalVoxelCoord, VoxelGridSpec};
use core_voxel::{VoxelMaterialId, VoxelValue};
use svc_mesh::{mesh_chunk_in_world_with_options, SurfaceMeshOptions, SurfaceMode};
use svc_spatial::VoxelWorld;
use svc_volume::VoxelChunk;

const CHUNK: u32 = 16;
const CHUNKS: [i64; 3] = [6, 5, 6];
const ROUNDS: usize = 3;

fn main() {
    let world = dungeon();
    let chunks: Vec<ChunkCoord> = world.resident_chunks().map(|(c, _)| c).collect();
    for mode in [
        SurfaceMode::GreedyCubes,
        SurfaceMode::MarchingCubes,
        SurfaceMode::DualContouring,
    ] {
        let options = SurfaceMeshOptions {
            mode,
            ..SurfaceMeshOptions::default()
        };
        let mut best = f64::INFINITY;
        let mut triangles = 0;
        for _ in 0..ROUNDS {
            let started = Instant::now();
            triangles = 0;
            for coord in &chunks {
                let mesh = mesh_chunk_in_world_with_options(&world, *coord, &options)
                    .expect("resident")
                    .expect("meshes");
                triangles += mesh.indices.len() / 3;
            }
            best = best.min(started.elapsed().as_secs_f64());
        }
        // The payloads' identity, outside the timing.
        let mut payload_hash = 0xcbf2_9ce4_8422_2325_u64;
        for coord in &chunks {
            let mesh = mesh_chunk_in_world_with_options(&world, *coord, &options)
                .expect("resident")
                .expect("meshes");
            {
                for bytes in [
                    bytemuck_free(&mesh.positions),
                    bytemuck_free(&mesh.normals),
                    bytemuck_free(&mesh.tile_coordinates),
                ] {
                    payload_hash = fnv(payload_hash, &bytes);
                }
                payload_hash = fnv(
                    payload_hash,
                    &mesh
                        .indices
                        .iter()
                        .flat_map(|index| index.to_le_bytes())
                        .collect::<Vec<_>>(),
                );
                payload_hash = fnv(
                    payload_hash,
                    &mesh
                        .triangle_owners
                        .iter()
                        .flat_map(|owner| owner.iter().flat_map(|value| value.to_le_bytes()))
                        .collect::<Vec<_>>(),
                );
            }
        }
        println!(
            "{:>15}: {:8.1} ms total, {:6.2} ms/chunk, {triangles} triangles, payload {payload_hash:016x}",
            mode.as_str(),
            best * 1000.0,
            best * 1000.0 / chunks.len() as f64,
        );
    }
}

/// The bytes of `f32`s, little-endian.
fn bytemuck_free(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

/// FNV-1a over `bytes`, continued from `hash`.
fn fnv(mut hash: u64, bytes: &[u8]) -> u64 {
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn solid(x: i64, y: i64, z: i64) -> Option<u16> {
    let (fx, fy, fz) = (x as f64, y as f64, z as f64);
    let tunnel = ((fx * 0.11).sin() * 9.0 + 40.0 - fz).abs() < 3.5 && (fy - 30.0).abs() < 4.0;
    let room =
        ((fx - 48.0).powi(2) + (fy - 34.0).powi(2) * 2.0 + (fz - 48.0).powi(2)).sqrt() < 14.0;
    let ripple = (fx * 0.37).sin() + (fz * 0.29).cos() + (fy * 0.21).sin();
    let ground = fy < 60.0 + ripple * 3.0;
    if !ground || tunnel || room {
        return None;
    }
    let block = (40..48).contains(&x) && (20..34).contains(&y) && (36..44).contains(&z);
    Some(if block { 2 } else { 1 })
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
