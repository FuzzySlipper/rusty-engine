//! Terrain layer weights on reconstructed voxel surfaces.

use std::collections::BTreeMap;

use core_space::{ChunkCoord, ChunkDims, GridId, LocalVoxelCoord, VoxelGridSpec};
use core_voxel::{VoxelMaterialId, VoxelValue};
use svc_mesh::{
    mesh_chunk_in_world_with_options, MeshError, MeshPayload, SurfaceMeshOptions, SurfaceMode,
    TerrainLayers,
};
use svc_spatial::VoxelWorld;
use svc_volume::VoxelChunk;

const SAND: u16 = 3;
const ROCK: u16 = 5;
const SIZE: i64 = 8;
const CHUNKS: i64 = 3;
/// Sand west of this voxel column, rock from it on.
const BOUNDARY: i64 = 13;

/// A slope rising east across three chunks, sand then rock.
fn slope() -> VoxelWorld {
    let grid =
        VoxelGridSpec::new(GridId::new(0), 0.5, ChunkDims::cubic(SIZE as u32).unwrap()).unwrap();
    let mut world = VoxelWorld::new(grid);
    for cx in 0..CHUNKS {
        let coord = ChunkCoord::new(cx, 0, 0);
        let mut chunk = VoxelChunk::from_spec(&grid);
        for x in 0..SIZE as u32 {
            for y in 0..SIZE as u32 {
                for z in 0..SIZE as u32 {
                    let local = LocalVoxelCoord::new(x, y, z);
                    let [vx, vy, _] = grid.chunk_local_to_voxel(coord, local).to_array();
                    if vy < 2 + vx / 6 {
                        let slot = if vx < BOUNDARY { SAND } else { ROCK };
                        chunk
                            .set(local, VoxelValue::solid(VoxelMaterialId::new(slot)))
                            .unwrap();
                    }
                }
            }
        }
        world.insert(coord, chunk);
    }
    world
}

fn options(transition_cells: Option<u8>) -> SurfaceMeshOptions {
    SurfaceMeshOptions {
        terrain_layers: transition_cells
            .map(|cells| TerrainLayers::new(vec![SAND, ROCK], cells).unwrap()),
        ..SurfaceMeshOptions::with_mode(SurfaceMode::DualContouring)
    }
}

fn mesh(world: &VoxelWorld, cx: i64, options: &SurfaceMeshOptions) -> MeshPayload {
    mesh_chunk_in_world_with_options(world, ChunkCoord::new(cx, 0, 0), options)
        .unwrap()
        .unwrap()
}

/// Each vertex's absolute voxel-space position (voxel centres at integers)
/// and weights, over every chunk.
fn vertices(world: &VoxelWorld, options: &SurfaceMeshOptions) -> Vec<([f32; 3], [f32; 4])> {
    let mut all = Vec::new();
    for cx in 0..CHUNKS {
        let payload = mesh(world, cx, options);
        assert_eq!(payload.layer_weights.len(), payload.positions.len() / 3 * 4);
        for (position, weights) in payload
            .positions
            .chunks(3)
            .zip(payload.layer_weights.chunks(4))
        {
            all.push((
                [
                    position[0] / 0.5 + (cx * SIZE) as f32,
                    position[1] / 0.5,
                    position[2] / 0.5,
                ],
                [weights[0], weights[1], weights[2], weights[3]],
            ));
        }
    }
    all
}

#[test]
fn layers_leave_geometry_and_slots_unchanged() {
    let world = slope();
    for cx in 0..CHUNKS {
        let plain = mesh(&world, cx, &options(None));
        let layered = mesh(&world, cx, &options(Some(2)));
        assert!(plain.layer_weights.is_empty());
        assert_eq!(plain.positions, layered.positions);
        assert_eq!(plain.normals, layered.normals);
        assert_eq!(plain.tile_coordinates, layered.tile_coordinates);
        assert_eq!(plain.indices, layered.indices);
        assert_eq!(plain.groups, layered.groups);
        assert_eq!(plain.triangle_owners, layered.triangle_owners);
    }
}

#[test]
fn weights_are_shares_that_follow_the_materials() {
    let world = slope();
    for (position, weights) in vertices(&world, &options(Some(1))) {
        let total: f32 = weights.iter().sum();
        assert!((total - 1.0).abs() < 1e-5, "{weights:?}");
        assert!(weights.iter().all(|weight| (0.0..=1.0).contains(weight)));
        assert_eq!(weights[2..], [0.0, 0.0], "only two layers");
        // Two voxels from the boundary a one-voxel transition is pure.
        if position[0] < BOUNDARY as f32 - 2.0 {
            assert_eq!(weights[0], 1.0, "sand at {position:?}");
        }
        if position[0] > BOUNDARY as f32 + 1.0 {
            assert_eq!(weights[1], 1.0, "rock at {position:?}");
        }
    }
}

#[test]
fn a_shared_seam_vertex_has_the_same_weights_in_both_chunks() {
    // The seam at x = 8 is mid-transition with a wide reach.
    let world = slope();
    let options = SurfaceMeshOptions {
        terrain_layers: Some(TerrainLayers::new(vec![SAND, ROCK], 4).unwrap()),
        ..SurfaceMeshOptions::with_mode(SurfaceMode::DualContouring)
    };
    let mut seen = BTreeMap::<[i64; 3], [u32; 4]>::new();
    let mut shared = 0;
    for (position, weights) in vertices(&world, &options) {
        let key = position.map(|value| (value * 1024.0).round() as i64);
        let bits = weights.map(f32::to_bits);
        if let Some(previous) = seen.insert(key, bits) {
            shared += 1;
            assert_eq!(previous, bits, "weights differ at {position:?}");
        }
    }
    assert!(shared > 0, "the chunks share seam vertices");
}

#[test]
fn the_transition_width_sets_how_far_layers_mix() {
    let world = slope();
    let mixed_span = |cells: u8| {
        let xs: Vec<f32> = vertices(&world, &options(Some(cells)))
            .into_iter()
            .filter(|(_, weights)| weights[0] > 0.01 && weights[0] < 0.99)
            .map(|(position, _)| position[0])
            .collect();
        let min = xs.iter().copied().fold(f32::INFINITY, f32::min);
        let max = xs.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        max - min
    };
    let narrow = mixed_span(1);
    let broad = mixed_span(4);
    // One voxel's vertices mix at the narrowest; the widest reaches several.
    assert!(narrow <= 1.0, "narrow transition spans {narrow} voxels");
    assert!(broad >= 4.0, "broad transition spans {broad} voxels");
}

#[test]
fn a_vertex_beyond_the_set_takes_its_own_layer() {
    // Only rock is a layer: sand vertices away from rock read layer 0.
    let world = slope();
    let options = SurfaceMeshOptions {
        terrain_layers: Some(TerrainLayers::new(vec![ROCK], 1).unwrap()),
        ..SurfaceMeshOptions::with_mode(SurfaceMode::DualContouring)
    };
    for (_, weights) in vertices(&world, &options) {
        assert_eq!(weights, [1.0, 0.0, 0.0, 0.0]);
    }
}

#[test]
fn malformed_sets_are_refused() {
    for (slots, cells) in [
        (vec![], 1),
        (vec![1, 2, 3, 4, 5], 1),
        (vec![1, 1], 1),
        (vec![1], 0),
        (vec![1], 5),
    ] {
        assert_eq!(
            TerrainLayers::new(slots, cells),
            Err(MeshError::InvalidTerrainLayers)
        );
    }
}
