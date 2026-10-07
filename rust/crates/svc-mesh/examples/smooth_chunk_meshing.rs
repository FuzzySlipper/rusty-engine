//! Per-chunk meshing cost of each surface mode on a dungeon-sized world.
//!
//! Run with:
//! `cargo run --release -p svc-mesh --example smooth_chunk_meshing`
//!
//! The world is 6 × 5 × 6 chunks of 16³ one-metre cells (180 chunks, the
//! size of a CraftSurvive dungeon): rock with carved tunnels and rooms, a
//! block of a second material (Blocky when dual contoured, as a dungeon's
//! masonry is) and a pool of see-through water in the room.
//! Each mode meshes every resident chunk with its resident neighbours, as a
//! scene build does, and reports the chunks whose samples include water
//! separately. `SMOOTH_CHUNK_MESHING_OCCLUSION=1` meshes with vertex
//! occlusion (#9506) to measure its cost. Each mode also samples every
//! chunk's upward ground for the scatter service (#9546), at 4 spots per
//! square metre on ground up to 35 degrees steep. Reconstructed modes also
//! mesh every chunk from the coarse lattice a distant chunk is drawn from.

use std::time::{Duration, Instant};

use core_space::{ChunkCoord, ChunkDims, GridId, LocalVoxelCoord, VoxelGridSpec};
use core_voxel::{VoxelMaterialId, VoxelValue};
use svc_mesh::scatter::{surface_points, ChunkSurface, SurfaceSampling};
use svc_mesh::{
    mesh_chunk_coarse_in_world, mesh_chunk_in_world_with_options, MaterialSurface,
    SurfaceCharacter, SurfaceMaterials, SurfaceMeshOptions, SurfaceMode, VertexPlacement,
};
use svc_spatial::VoxelWorld;
use svc_volume::VoxelChunk;

const CHUNK: u32 = 16;
const CHUNKS: [i64; 3] = [6, 5, 6];
const ROUNDS: usize = 3;
const BLOCK: u16 = 2;
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
        let block = MaterialSurface {
            mode,
            character: SurfaceCharacter {
                placement: VertexPlacement::Blocky,
                crease_angle_degrees: 0.0,
                roughness: 0.0,
            },
        };
        let options = SurfaceMeshOptions {
            mode,
            non_occluding: [WATER].into(),
            materials: SurfaceMaterials::new(
                (mode == SurfaceMode::DualContouring).then_some((BLOCK, block)),
            )
            .expect("one material"),
            vertex_occlusion: if std::env::var_os("SMOOTH_CHUNK_MESHING_OCCLUSION").is_some() {
                1.0
            } else {
                0.0
            },
            ..SurfaceMeshOptions::default()
        };
        let mut best = Duration::MAX;
        let mut best_water = Duration::MAX;
        let mut triangles = 0;
        let mut block_triangles = 0;
        for _ in 0..ROUNDS {
            let mut total = Duration::ZERO;
            let mut water = Duration::ZERO;
            triangles = 0;
            block_triangles = 0;
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
                block_triangles += mesh
                    .groups
                    .iter()
                    .filter(|group| group.material_slot == BLOCK)
                    .map(|group| group.count as usize / 3)
                    .sum::<usize>();
            }
            best = best.min(total);
            best_water = best_water.min(water);
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
        let per_chunk = |time: Duration, count: usize| time.as_secs_f64() * 1000.0 / count as f64;
        println!(
            "{:>15}: {:8.1} ms total, {:6.2} ms/chunk, {triangles} triangles ({block_triangles} of the block); {water_chunks} chunks sampling water {:6.2} ms/chunk; payload {payload_hash:016x}",
            mode.as_str(),
            best.as_secs_f64() * 1000.0,
            per_chunk(best, chunks.len()),
            per_chunk(best_water, water_chunks),
        );
        // The scatter service's surface sampling, on the meshes as drawn.
        let meshes: Vec<_> = chunks
            .iter()
            .map(|coord| {
                let mesh = mesh_chunk_in_world_with_options(&world, *coord, &options)
                    .expect("resident")
                    .expect("meshes");
                let groups: Vec<(u16, u32, u32)> = mesh
                    .groups
                    .iter()
                    .map(|group| (group.material_slot, group.start, group.count))
                    .collect();
                let origin = coord
                    .to_array()
                    .map(|value| (value * i64::from(CHUNK)) as f64);
                (mesh, groups, origin)
            })
            .collect();
        let mut best = Duration::MAX;
        let mut spots = 0;
        for _ in 0..ROUNDS {
            let started = Instant::now();
            spots = 0;
            for (mesh, groups, origin) in &meshes {
                spots += surface_points(
                    &ChunkSurface {
                        positions: &mesh.positions,
                        indices: &mesh.indices,
                        groups,
                        origin: *origin,
                        band: 1.0,
                    },
                    &SurfaceSampling {
                        density: 4.0,
                        seed: 9546,
                        minimum_normal_y: 35f64.to_radians().cos(),
                    },
                )
                .len();
            }
            best = best.min(started.elapsed());
        }
        println!(
            "{:>15} scatter: {:6.1} ms total, {:6.3} ms/chunk, {spots} spots at 4 per m²",
            "",
            best.as_secs_f64() * 1000.0,
            per_chunk(best, chunks.len()),
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
    if room && fy < 30.0 {
        return Some(WATER);
    }
    if !ground || tunnel || room {
        return None;
    }
    let block = (40..48).contains(&x) && (20..34).contains(&y) && (36..44).contains(&z);
    Some(if block { BLOCK } else { 1 })
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
