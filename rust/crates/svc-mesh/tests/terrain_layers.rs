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
    slope_of(|vx, _| if vx < BOUNDARY { SAND } else { ROCK })
}

/// The same slope with each voxel's slot chosen from its x and y.
fn slope_of(slot_at: impl Fn(i64, i64) -> u16) -> VoxelWorld {
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
                        let slot = slot_at(vx, vy);
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

// Six physical slots drawn as four layers.
const GRASS: u16 = 10;
const DIRT: u16 = 11;
const STONE: u16 = 12;
const DUNE: u16 = 13;
const SNOW: u16 = 14;
const GRAVEL: u16 = 15;

/// Grass over dirt, stone, dune sand, then snow over gravel, west to east.
fn six_slot_slope() -> VoxelWorld {
    slope_of(|vx, vy| {
        let top = vy + 1 == 2 + vx / 6;
        match vx {
            0..6 if top => GRASS,
            0..6 => DIRT,
            6..12 => STONE,
            12..18 => DUNE,
            _ if top => SNOW,
            _ => GRAVEL,
        }
    })
}

fn six_slot_layers(transition_cells: u8) -> SurfaceMeshOptions {
    SurfaceMeshOptions {
        terrain_layers: Some(
            TerrainLayers::mapped(
                vec![GRASS, DIRT, STONE, DUNE, SNOW, GRAVEL],
                vec![0, 0, 1, 2, 3, 3],
                transition_cells,
            )
            .unwrap(),
        ),
        ..SurfaceMeshOptions::with_mode(SurfaceMode::DualContouring)
    }
}

#[test]
fn slots_sharing_a_layer_weigh_as_one_material() {
    // The six-slot slope weighs exactly as the same slope with each layer's
    // slots replaced by one, under the plain one-slot-per-layer set.
    let collapsed = slope_of(|vx, _| match vx {
        0..6 => GRASS,
        6..12 => STONE,
        12..18 => DUNE,
        _ => SNOW,
    });
    let one_per_layer = SurfaceMeshOptions {
        terrain_layers: Some(TerrainLayers::new(vec![GRASS, STONE, DUNE, SNOW], 2).unwrap()),
        ..SurfaceMeshOptions::with_mode(SurfaceMode::DualContouring)
    };
    let world = six_slot_slope();
    // More slots split more groups, so vertices are matched by position.
    let key = |position: [f32; 3]| position.map(|value| (value * 1024.0).round() as i64);
    let expected: BTreeMap<_, _> = vertices(&collapsed, &one_per_layer)
        .into_iter()
        .map(|(position, weights)| (key(position), weights.map(f32::to_bits)))
        .collect();
    let mapped = vertices(&world, &six_slot_layers(2));
    for (position, weights) in &mapped {
        assert_eq!(
            Some(&weights.map(f32::to_bits)),
            expected.get(&key(*position)),
            "{position:?}"
        );
    }
    // Every layer is reached, and grass/dirt blends into stone.
    for layer in 0..4 {
        assert!(mapped.iter().any(|(_, weights)| weights[layer] == 1.0));
    }
    assert!(mapped
        .iter()
        .any(|(_, weights)| weights[0] > 0.0 && weights[1] > 0.0));
    // Geometry and physical slots are those of the unlayered surface.
    for cx in 0..CHUNKS {
        let plain = mesh(
            &world,
            cx,
            &SurfaceMeshOptions::with_mode(SurfaceMode::DualContouring),
        );
        let layered = mesh(&world, cx, &six_slot_layers(2));
        assert_eq!(plain.positions, layered.positions);
        assert_eq!(plain.indices, layered.indices);
        assert_eq!(plain.groups, layered.groups);
        assert_eq!(plain.triangle_owners, layered.triangle_owners);
    }
}

#[test]
fn a_cube_face_takes_its_slots_mapped_layer() {
    // Cube faces take their own slot's layer whole: dirt and gravel draw as
    // the layers they are mapped to, not by their place in the list.
    let world = six_slot_slope();
    let options = SurfaceMeshOptions {
        mode: SurfaceMode::GreedyCubes,
        ..six_slot_layers(1)
    };
    let mut seen = [false; 4];
    for cx in 0..CHUNKS {
        let payload = mesh(&world, cx, &options);
        for weights in payload.layer_weights.chunks(4) {
            let layer = weights.iter().position(|weight| *weight == 1.0).unwrap();
            assert_eq!(weights.iter().sum::<f32>(), 1.0, "{weights:?}");
            seen[layer] = true;
        }
    }
    assert_eq!(seen, [true; 4]);
}

#[test]
fn malformed_mappings_are_refused() {
    let many: Vec<u16> = (1..=17).collect();
    for (slots, layers) in [
        (vec![], vec![]),
        (vec![1, 2, 1], vec![0, 1, 0]),
        (vec![1, 2, 1], vec![0, 1, 1]),
        (vec![1, 2], vec![0, 4]),
        (vec![1, 2], vec![0]),
        (many.clone(), vec![0; many.len()]),
    ] {
        assert_eq!(
            TerrainLayers::mapped(slots, layers, 1),
            Err(MeshError::InvalidTerrainLayers)
        );
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
