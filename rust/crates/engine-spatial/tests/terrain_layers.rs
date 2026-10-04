//! Terrain layer weights follow edits near chunk borders and survive a
//! world-origin rebase.

use engine_spatial::{
    MaterialVoxel, SurfaceMeshOptions, SurfaceMode, TerrainLayers, VoxelCollisionScene, VoxelEdit,
    VoxelEditService, WorldOrigin, WorldOriginRebaseRequest, WorldOriginRebaseService,
    WorldOriginState,
};

const SAND: u16 = 1;
const ROCK: u16 = 2;
const CHUNK: u32 = 8;
const WIDTH: i64 = 24;

fn options() -> SurfaceMeshOptions {
    SurfaceMeshOptions {
        terrain_layers: Some(TerrainLayers::new(vec![SAND, ROCK], 2).unwrap()),
        ..SurfaceMeshOptions::with_mode(SurfaceMode::DualContouring)
    }
}

/// A slope of sand with rock from x = 12 on, across three chunks.
fn voxels(rock_from: i64) -> Vec<MaterialVoxel> {
    let mut voxels = Vec::new();
    for x in 0..WIDTH {
        for z in 0..CHUNK as i64 {
            for y in 0..2 + x / 6 {
                voxels.push(MaterialVoxel {
                    state: 0,
                    address: [x, y, z],
                    material_slot: if x < rock_from { SAND } else { ROCK },
                });
            }
        }
    }
    voxels
}

fn scene(voxels: Vec<MaterialVoxel>) -> VoxelCollisionScene {
    VoxelCollisionScene::from_material_voxels_with_mesh_options(1.0, CHUNK, voxels, options())
        .unwrap()
}

fn weights(scene: &VoxelCollisionScene) -> Vec<([i64; 3], Vec<f32>)> {
    scene
        .mesh_chunks()
        .map(|chunk| (chunk.chunk, chunk.layer_weights.clone()))
        .collect()
}

#[test]
fn an_edit_within_reach_of_a_border_reweighs_the_neighbour() {
    // Turning the sand column at x = 14 (local 6 of chunk 1) to rock: chunk
    // 2's vertices from x = 15 weigh it, though it is not on their border.
    let mut edited = scene(voxels(15));
    let edits: Vec<_> = (0..CHUNK as i64)
        .flat_map(|z| {
            (0..2 + 14 / 6).map(move |y| VoxelEdit::Set {
                address: [14, y, z],
                material_slot: ROCK,
            })
        })
        .collect();
    VoxelEditService::apply(&mut edited, &edits).unwrap();
    assert!(
        edited.mesh_update().dirty_chunks.contains(&[2, 0, 0]),
        "the neighbour within reach is remeshed"
    );
    // Incremental remeshing gives exactly what a fresh build does.
    let fresh = weights(&scene(voxels(14)));
    assert_eq!(weights(&edited), fresh);
    assert_ne!(weights(&scene(voxels(15)))[2], fresh[2], "chunk 2 changed");
}

#[test]
fn a_rebase_keeps_every_weight() {
    let scene = scene(voxels(12));
    let mut origin = WorldOriginState::default();
    let prepared = WorldOriginRebaseService
        .prepare(
            &origin,
            WorldOriginRebaseRequest {
                target_origin: WorldOrigin::new([8, 1, 0]),
                entities: Vec::new(),
            },
        )
        .unwrap();
    let (rebased, _) = WorldOriginRebaseService
        .commit(&mut origin, &scene, &prepared)
        .unwrap();
    assert_eq!(weights(&rebased), weights(&scene));
    let hashes = |scene: &VoxelCollisionScene| {
        scene
            .mesh_chunks()
            .map(|chunk| chunk.content_hash)
            .collect::<Vec<_>>()
    };
    assert_eq!(hashes(&rebased), hashes(&scene), "no mesh is replaced");
}

#[test]
fn a_transition_must_fit_in_a_chunk() {
    let options = SurfaceMeshOptions {
        terrain_layers: Some(TerrainLayers::new(vec![SAND], 4).unwrap()),
        ..SurfaceMeshOptions::with_mode(SurfaceMode::DualContouring)
    };
    assert!(VoxelCollisionScene::from_material_voxels_with_mesh_options(
        1.0,
        5,
        voxels(12),
        options
    )
    .is_err());
}
