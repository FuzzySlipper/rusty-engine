//! A capsule overlaps solid cube voxels however deep it lies in them: a
//! solid chunk's collider is a few merged boxes, and a capsule well inside
//! one is still in collision (#9672).

use core_space::WorldPos;
use engine_spatial::{
    CharacterCapsule, VoxelChunkIdentity, VoxelChunkPayload, VoxelChunkResidencyOperation,
    VoxelChunkResidencyService, VoxelCollisionScene,
};

const EDGE: usize = 16;
const STONE_TOP: f64 = 8.0;
const HALF_HEIGHT: f64 = 0.575;
const RADIUS: f64 = 0.3;

/// One admitted 16³ chunk of stone below y = 8, air above.
fn stone() -> VoxelCollisionScene {
    let mut scene =
        VoxelCollisionScene::from_material_voxels(1.0, EDGE as u32, std::iter::empty()).unwrap();
    let mut slots = vec![0u16; EDGE * EDGE * EDGE];
    for z in 0..EDGE {
        for y in 0..STONE_TOP as usize {
            for x in 0..EDGE {
                slots[x + EDGE * (y + EDGE * z)] = 1;
            }
        }
    }
    VoxelChunkResidencyService::apply(
        &mut scene,
        &[VoxelChunkResidencyOperation::Admit {
            chunk: VoxelChunkIdentity::from_array([0, 0, 0]),
            payload: VoxelChunkPayload::new([EDGE as u32; 3], slots),
        }],
    )
    .unwrap();
    scene
}

fn overlaps(scene: &VoxelCollisionScene, x: f64, y: f64, z: f64) -> bool {
    scene
        .character_capsule_overlap(CharacterCapsule {
            center: WorldPos::new(x, y, z),
            half_height: HALF_HEIGHT,
            radius: RADIUS,
        })
        .unwrap()
        .is_some()
}

#[test]
fn a_capsule_buried_in_merged_stone_overlaps_it_and_one_above_does_not() {
    let scene = stone();
    assert!(overlaps(&scene, 8.5, 3.0, 8.5), "wholly inside the stone");
    assert!(overlaps(&scene, 8.5, 7.25, 8.5), "straddling its top face");
    assert!(!overlaps(&scene, 8.5, 9.0, 8.5), "just above it");
}

#[test]
fn every_capsule_reaching_into_the_stone_overlaps_it() {
    let scene = stone();
    let reach = HALF_HEIGHT + RADIUS;
    for xi in 0..32 {
        for zi in 0..32 {
            for yi in 0..30 {
                let (x, y, z) = (
                    0.25 + f64::from(xi) * 0.5,
                    0.05 + f64::from(yi) * 0.25,
                    0.25 + f64::from(zi) * 0.5,
                );
                assert_eq!(
                    overlaps(&scene, x, y, z),
                    y - reach < STONE_TOP,
                    "capsule at ({x}, {y}, {z})"
                );
            }
        }
    }
}

#[test]
fn a_capsule_beside_a_corner_or_edge_overlaps_only_within_its_radius() {
    let scene = stone();
    // Clear: the axis is 0.354 m from the stone's vertical edge, past the
    // radius, and 0.336 m from its top edge, past the rounded cap.
    assert!(
        !overlaps(&scene, -0.25, 3.0, -0.25),
        "beside the vertical edge"
    );
    assert!(!overlaps(&scene, -0.25, 8.8, 8.5), "beside the top edge");
    // Within the radius of the same edges.
    assert!(
        overlaps(&scene, -0.15, 3.0, -0.15),
        "touching the vertical edge"
    );
    assert!(overlaps(&scene, -0.15, 8.6, 8.5), "touching the top edge");
}
