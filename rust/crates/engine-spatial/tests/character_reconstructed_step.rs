//! Step-up onto a ledge reconstructed by dual contouring (#9681): its riser
//! leans, so the capsule's sweep down meets the riser below the edge, and the
//! step still lands on the tread above as it does on a cube ledge.

use core_ids::EntityId;
use core_math::{Vec2, Vec3};
use engine_spatial::{
    CharacterControllerCommand, CharacterControllerConfig, CharacterControllerService,
    MaterialSurface, MaterialVoxel, SurfaceCharacter, SurfaceMaterials, SurfaceMeshOptions,
    SurfaceMode, VertexPlacement, VoxelCollisionScene, VoxelDensityEdit, VoxelDensityEditService,
    VoxelDensityOperation, VoxelDensityShape,
};
use entity_state::{CharacterMotionComponent, EntityDefinition, EntityState};

const STONE: u16 = 1;

fn options(mode: SurfaceMode) -> SurfaceMeshOptions {
    SurfaceMeshOptions {
        mode: SurfaceMode::DualContouring,
        materials: SurfaceMaterials::new([(
            STONE,
            MaterialSurface {
                mode,
                character: SurfaceCharacter {
                    placement: VertexPlacement::Sharp,
                    crease_angle_degrees: 40.0,
                    roughness: 0.0,
                },
            },
        )])
        .unwrap(),
        ..SurfaceMeshOptions::default()
    }
}

/// A floor whose top is y = 1, and a ledge `riser` m higher where z < 3,
/// drawn and collided in `mode`. The ledge is an added box: exact box
/// distances, as a product stamps a stone course.
fn scene(mode: SurfaceMode, riser: f64) -> VoxelCollisionScene {
    let voxels = (-8..8).flat_map(|x| {
        (-8..8).map(move |z| MaterialVoxel {
            state: 0,
            address: [x, 0, z],
            material_slot: STONE,
        })
    });
    let mut scene =
        VoxelCollisionScene::from_material_voxels_with_mesh_options(1.0, 8, voxels, options(mode))
            .unwrap();
    VoxelDensityEditService::apply(
        &mut scene,
        &[VoxelDensityEdit::Brush {
            shape: VoxelDensityShape::Box {
                min: [-8.0, 0.5, -8.0],
                max: [8.0, 1.0 + riser, 3.0],
            },
            operation: VoxelDensityOperation::Add,
            material_slot: STONE,
        }],
    )
    .unwrap();
    scene
}

/// Walks a CraftSurvive-sized capsule (step height 1.05 m) forward (-z) at
/// the ledge from z = 6 for 100 steps; returns its final height and the
/// steps it accepted.
fn walk_at(scene: &VoxelCollisionScene) -> (f32, usize) {
    let entity = EntityId::new(1);
    let start = Vec3::new(0.5, 1.0 + 0.875 + 0.015, 6.0);
    let mut state = EntityState::from_definitions([EntityDefinition::new(entity, "character")
        .with_transform(start)
        .with_character_motion(CharacterMotionComponent::at_rest(start.y))])
    .unwrap();
    let mut config = CharacterControllerConfig::default();
    config.shape.standing_height = 1.75;
    config.shape.radius = 0.3;
    config.shape.contact_skin = 0.015;
    config.ground.forward_speed = 5.0;
    config.surface.maximum_step_height = 1.05;
    config.surface.floor_snap_distance = 0.25;
    let mut service = CharacterControllerService::default();
    let mut steps = 0;
    for sequence in 1..=100 {
        let receipt = service
            .step(
                &mut state,
                scene,
                entity,
                &config,
                CharacterControllerCommand {
                    planar_intent: Vec2::new(0.0, 1.0),
                    ..CharacterControllerCommand::idle(1.0 / 60.0, sequence)
                },
            )
            .unwrap();
        steps += usize::from(receipt.step.is_some_and(|step| step.accepted));
    }
    (state.transform(entity).unwrap().translation.y, steps)
}

#[test]
fn a_reconstructed_ledge_within_the_step_height_is_climbed_as_a_cube_ledge_is() {
    let standing = 0.875 + 0.015;
    for (mode, riser) in [
        (SurfaceMode::GreedyCubes, 1.0),
        (SurfaceMode::DualContouring, 1.0),
        (SurfaceMode::DualContouring, 0.9),
    ] {
        let (height, steps) = walk_at(&scene(mode, riser));
        assert_eq!(steps, 1, "{mode:?} {riser} m: one step up");
        assert!(
            (height - (1.0 + riser as f32 + standing)).abs() < 0.01,
            "{mode:?} {riser} m: standing on the tread, at {height}"
        );
    }
}

#[test]
fn a_reconstructed_ledge_above_the_step_height_stays_a_wall() {
    let (height, steps) = walk_at(&scene(SurfaceMode::DualContouring, 1.3));
    assert_eq!(steps, 0);
    // The leaning riser may lift the body a little; it stays far below the
    // tread, whose standing height is 3.19.
    assert!(height < 1.0 + 0.875 + 0.2, "not on the ledge, at {height}");
}
