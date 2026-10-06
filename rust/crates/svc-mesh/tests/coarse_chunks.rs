//! Coarse chunk meshes: the drawn-only lattice twice as coarse that a distant
//! chunk is drawn from.

use core_space::{ChunkCoord, ChunkDims, GridId, LocalVoxelCoord, VoxelGridSpec};
use core_voxel::{VoxelMaterialId, VoxelValue};
use svc_mesh::{
    mesh_chunk_coarse_in_world, mesh_chunk_in_world_with_options, SurfaceMeshOptions, SurfaceMode,
};
use svc_spatial::VoxelWorld;
use svc_volume::VoxelChunk;

/// Ground solid below `top` across a 3 × 2 × 3 block of 8³ chunks, the
/// lower layer full.
fn floor(top: i64) -> VoxelWorld {
    let grid = VoxelGridSpec::new(GridId::new(0), 1.0, ChunkDims::cubic(8).unwrap()).unwrap();
    let mut world = VoxelWorld::new(grid);
    for (cx, cy, cz) in
        (-1..=1).flat_map(|x| (-1..=0).flat_map(move |y| (-1..=1).map(move |z| (x, y, z))))
    {
        let coord = ChunkCoord::new(cx, cy, cz);
        let mut chunk = VoxelChunk::from_spec(&grid);
        for (x, y, z) in
            (0..8).flat_map(|x| (0..8).flat_map(move |y| (0..8).map(move |z| (x, y, z))))
        {
            if cy * 8 + i64::from(y) < top {
                chunk
                    .set(
                        LocalVoxelCoord::new(x, y, z),
                        VoxelValue::solid(VoxelMaterialId::new(1)),
                    )
                    .unwrap();
            }
        }
        world.insert(coord, chunk);
    }
    world
}

#[test]
fn a_coarse_floor_lies_inside_the_fine_one_with_skirts_below_its_borders() {
    let world = floor(5);
    let options = SurfaceMeshOptions::with_mode(SurfaceMode::DualContouring);
    let middle = ChunkCoord::new(0, 0, 0);
    let fine = mesh_chunk_in_world_with_options(&world, middle, &options)
        .unwrap()
        .unwrap();
    let coarse = mesh_chunk_coarse_in_world(&world, middle, &options)
        .unwrap()
        .unwrap();
    let heights = |mesh: &svc_mesh::MeshPayload| {
        mesh.positions
            .chunks(3)
            .map(|position| position[1])
            .collect::<Vec<_>>()
    };
    assert!(heights(&fine).iter().all(|y| *y == 5.0));
    // A 2 × 2 × 2 block is solid only where all of it is: voxels 4 and 5
    // are not, so the coarse floor is at 4, inside the fine one.
    let coarse_heights = heights(&coarse);
    assert!(coarse_heights.iter().all(|y| *y <= 4.0));
    assert!(coarse_heights.contains(&4.0));
    // Skirts hang one coarse cell, two voxels, below the open borders.
    assert_eq!(
        coarse_heights.iter().copied().fold(f32::INFINITY, f32::min),
        2.0
    );
    // Tile coordinates stay in voxel units, as the fine mesh's do.
    let span = |mesh: &svc_mesh::MeshPayload| {
        mesh.tile_coordinates
            .iter()
            .fold((f32::INFINITY, f32::NEG_INFINITY), |(low, high), value| {
                (low.min(*value), high.max(*value))
            })
    };
    let (fine_low, fine_high) = span(&fine);
    let (coarse_low, coarse_high) = span(&coarse);
    assert!(coarse_low >= fine_low - 2.0 && coarse_high <= fine_high + 2.0);
    assert!(coarse_high - coarse_low >= 8.0);
    assert!(coarse.indices.len() < fine.indices.len());
}

#[test]
fn cube_sessions_and_odd_chunks_have_no_coarse_mesh() {
    let world = floor(5);
    let middle = ChunkCoord::new(0, 0, 0);
    assert!(mesh_chunk_coarse_in_world(
        &world,
        middle,
        &SurfaceMeshOptions::with_mode(SurfaceMode::GreedyCubes)
    )
    .is_none());
    let grid = VoxelGridSpec::new(GridId::new(0), 1.0, ChunkDims::cubic(5).unwrap()).unwrap();
    let mut odd = VoxelWorld::new(grid);
    odd.insert(middle, VoxelChunk::from_spec(&grid));
    assert!(mesh_chunk_coarse_in_world(
        &odd,
        middle,
        &SurfaceMeshOptions::with_mode(SurfaceMode::DualContouring)
    )
    .is_none());
}
