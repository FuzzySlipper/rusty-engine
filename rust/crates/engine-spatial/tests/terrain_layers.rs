//! Terrain layer weights follow edits near chunk borders and chunk
//! residency, and survive a world-origin rebase.

use engine_spatial::{
    MaterialVoxel, SurfaceMeshOptions, SurfaceMode, TerrainLayers, VoxelChunkIdentity,
    VoxelChunkPayload, VoxelChunkResidencyOperation, VoxelChunkResidencyService,
    VoxelCollisionScene, VoxelEdit, VoxelEditService, WorldOrigin, WorldOriginRebaseRequest,
    WorldOriginRebaseService, WorldOriginState,
};

const SAND: u16 = 1;
const ROCK: u16 = 2;
/// Under sand, drawn as sand.
const DIRT: u16 = 3;
/// Under rock, drawn as rock.
const GRAVEL: u16 = 4;
const CHUNK: u32 = 8;
const WIDTH: i64 = 24;

fn options() -> SurfaceMeshOptions {
    SurfaceMeshOptions {
        terrain_layers: Some(
            TerrainLayers::mapped(vec![SAND, DIRT, ROCK, GRAVEL], vec![0, 0, 1, 1], 2).unwrap(),
        ),
        ..SurfaceMeshOptions::with_mode(SurfaceMode::DualContouring)
    }
}

/// The slot at (x, y) of a slope with rock from `rock_from` on: sand over
/// dirt, then rock over gravel.
fn slot_at(x: i64, y: i64, rock_from: i64) -> u16 {
    let top = y + 1 == 2 + x / 6;
    match (x < rock_from, top) {
        (true, true) => SAND,
        (true, false) => DIRT,
        (false, true) => ROCK,
        (false, false) => GRAVEL,
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
                    material_slot: slot_at(x, y, rock_from),
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
    // Turning the sand and dirt column at x = 14 (local 6 of chunk 1) to
    // rock and gravel: chunk 2's vertices from x = 15 weigh it, though it is
    // not on their border.
    let mut edited = scene(voxels(15));
    let edits: Vec<_> = (0..CHUNK as i64)
        .flat_map(|z| {
            (0..2 + 14 / 6).map(move |y| VoxelEdit::Set {
                address: [14, y, z],
                material_slot: slot_at(14, y, 14),
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
                exclude_outside_envelope: false,
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

#[test]
fn admitting_and_evicting_a_neighbour_reweighs_the_seam() {
    let without_last = |voxels: Vec<MaterialVoxel>| {
        voxels
            .into_iter()
            .filter(|voxel| voxel.address[0] < 2 * CHUNK as i64)
            .collect::<Vec<_>>()
    };
    let mut streamed = scene(without_last(voxels(12)));
    let full = weights(&scene(voxels(12)));
    let partial = weights(&streamed);
    assert_ne!(partial[1], full[1], "chunk 1's seam weighs chunk 2");

    let size = CHUNK as i64;
    let mut slots = Vec::new();
    for _z in 0..size {
        for y in 0..size {
            for x in 2 * size..3 * size {
                slots.push(if y < 2 + x / 6 { slot_at(x, y, 12) } else { 0 });
            }
        }
    }
    let last = VoxelChunkIdentity::new(2, 0, 0);
    VoxelChunkResidencyService::apply(
        &mut streamed,
        &[VoxelChunkResidencyOperation::Admit {
            chunk: last,
            payload: VoxelChunkPayload::new([CHUNK; 3], slots),
        }],
    )
    .unwrap();
    assert_eq!(weights(&streamed), full);
    VoxelChunkResidencyService::apply(
        &mut streamed,
        &[VoxelChunkResidencyOperation::Evict { chunk: last }],
    )
    .unwrap();
    assert_eq!(weights(&streamed), partial);
}
