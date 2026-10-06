//! Chunk distance fields follow `SurfaceMeshOptions::distance_fields`: a
//! scene builds none until the option is on, the toggle builds or drops
//! every resident chunk's field as one mesh update, and nothing else moves.
use engine_spatial::{MaterialVoxel, SurfaceMeshOptions, VoxelCollisionScene};

const CHUNK: u32 = 8;

/// A floor across four chunks and a pillar on it.
fn voxels() -> Vec<MaterialVoxel> {
    let mut voxels = Vec::new();
    for x in -8..8 {
        for z in -8..8 {
            voxels.push(MaterialVoxel {
                state: 0,
                address: [x, -1, z],
                material_slot: 1,
            });
        }
    }
    for y in 0..3 {
        voxels.push(MaterialVoxel {
            state: 0,
            address: [1, y, 1],
            material_slot: 2,
        });
    }
    voxels
}

fn scene(distance_fields: bool) -> VoxelCollisionScene {
    VoxelCollisionScene::from_material_voxels_with_mesh_options(
        1.0,
        CHUNK,
        voxels(),
        SurfaceMeshOptions {
            distance_fields,
            ..SurfaceMeshOptions::default()
        },
    )
    .expect("voxel scene")
}

fn fields(scene: &VoxelCollisionScene) -> Vec<usize> {
    scene
        .mesh_chunks()
        .map(|chunk| chunk.distance_field.len())
        .collect()
}

#[test]
fn a_scene_builds_no_fields_until_the_option_is_on() {
    let plain = scene(false);
    assert!(plain.mesh_chunks().len() >= 4, "the floor spans chunks");
    assert!(
        fields(&plain).iter().all(|len| *len == 0),
        "no chunk carries a field by default"
    );
    assert!(!plain.distance_fields());
    let traced = scene(true);
    assert!(
        fields(&traced).iter().all(|len| *len == 512),
        "every chunk carries its 8³ field when the option is on"
    );
}

#[test]
fn the_toggle_builds_then_drops_every_field_as_one_mesh_update() {
    let mut scene = scene(false);
    let chunks = scene.mesh_chunks().len();
    let before = scene.source_revision();
    let built_state = scene.mesh_update().previous_mesh_state;

    scene.set_distance_fields(true);
    assert!(scene.distance_fields());
    assert!(fields(&scene).iter().all(|len| *len == 512));
    let update = scene.mesh_update();
    assert_eq!(update.dirty_chunks.len(), chunks, "every chunk republishes");
    assert_eq!(update.rebuilt_chunks, chunks);
    assert_eq!(update.removed_chunks, 0);
    assert!(
        update.previous_mesh_state.is_some() && update.previous_mesh_state != built_state,
        "the update follows the build"
    );
    assert_eq!(scene.source_revision(), before, "no voxel changed");

    scene.set_distance_fields(false);
    assert!(fields(&scene).iter().all(|len| *len == 0));
    assert_eq!(scene.mesh_update().dirty_chunks.len(), chunks);

    // Already off: nothing to republish.
    let state = scene.mesh_update().previous_mesh_state;
    scene.set_distance_fields(false);
    assert_eq!(scene.mesh_update().previous_mesh_state, state);
}
