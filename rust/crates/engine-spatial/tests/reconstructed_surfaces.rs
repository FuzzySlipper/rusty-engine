//! Collision, raycasts and characters follow reconstructed surfaces, and
//! density edits reshape them locally.

use std::collections::BTreeSet;

use core_ids::EntityId;
use core_math::{Vec2, Vec3};
use engine_spatial::{
    CharacterControllerCommand, CharacterControllerConfig, CharacterControllerService,
    MaterialSurface, MaterialVoxel, SurfaceCharacter, SurfaceMaterials, SurfaceMeshOptions,
    SurfaceMode, VertexPlacement, VoxelCollisionScene, VoxelDensityApplyError, VoxelDensityEdit,
    VoxelDensityEditService, VoxelDensityOperation, VoxelDensityRejection, VoxelDensityShape,
};
use entity_state::{CharacterMotionComponent, EntityDefinition, EntityState};

const STONE: u16 = 1;
const BRICK: u16 = 2;
const WIDTH: i64 = 16;
const DEPTH: i64 = 24;
const HEIGHT: i64 = 16;

/// Ground height of the density slope: rising half a metre per metre toward -z.
fn slope_height(z: f64) -> f64 {
    2.0 + 0.5 * (DEPTH as f64 - z) * 0.5
}

fn dual_contoured(materials: SurfaceMaterials) -> SurfaceMeshOptions {
    SurfaceMeshOptions {
        mode: SurfaceMode::DualContouring,
        materials,
        ..SurfaceMeshOptions::default()
    }
}

/// A slab of stone whose densities describe the plane `slope_height`.
fn slope_scene(options: SurfaceMeshOptions) -> VoxelCollisionScene {
    slope_scene_in_chunks(options, 8)
}

fn slope_scene_in_chunks(options: SurfaceMeshOptions, chunk_size: u32) -> VoxelCollisionScene {
    let mut voxels = Vec::new();
    let mut densities = Vec::new();
    let mut materials = Vec::new();
    for z in 0..DEPTH {
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let density = (y as f64 + 0.5 - slope_height(z as f64 + 0.5)) as f32;
                densities.push(density);
                materials.push(if density < 0.0 { STONE } else { 0 });
                if density < 0.0 {
                    voxels.push(MaterialVoxel {
                        state: 0,
                        address: [x, y, z],
                        material_slot: STONE,
                    });
                }
            }
        }
    }
    let mut scene = VoxelCollisionScene::from_material_voxels_with_mesh_options(
        1.0, chunk_size, voxels, options,
    )
    .unwrap();
    VoxelDensityEditService::apply(
        &mut scene,
        &[VoxelDensityEdit::Region {
            min: [0, 0, 0],
            size: [WIDTH as u32, HEIGHT as u32, DEPTH as u32],
            densities,
            materials,
        }],
    )
    .unwrap();
    scene
}

fn down(scene: &VoxelCollisionScene, x: f64, z: f64) -> engine_spatial::CollisionRayHit {
    scene
        .raycast([x, HEIGHT as f64 + 4.0, z], [0.0, -1.0, 0.0], 64.0)
        .expect("the ray meets the ground")
}

#[test]
fn raycasts_meet_the_drawn_slope_and_name_a_solid_voxel() {
    let scene = slope_scene(dual_contoured(SurfaceMaterials::default()));
    for (x, z) in [(5.3, 12.25), (8.0, 7.6), (3.7, 17.1)] {
        let hit = down(&scene, x, z);
        assert!(
            (hit.point[1] - slope_height(z)).abs() < 1.0e-3,
            "hit {:?} at z {z} should be at {}",
            hit.point,
            slope_height(z)
        );
        assert!(scene.material_voxel(hit.voxel).is_some());
        assert!((hit.voxel[0] as f64 - x).abs() <= 1.5 && (hit.voxel[2] as f64 - z).abs() <= 1.5);
    }
    // The cube projection of the same voxels steps instead.
    let cubes = VoxelCollisionScene::from_material_voxels(1.0, 8, scene.material_voxels()).unwrap();
    let stepped = down(&cubes, 5.3, 12.25);
    assert_eq!(stepped.point[1].fract(), 0.0);
}

fn character_at(position: Vec3) -> (EntityId, EntityState) {
    let entity = EntityId::new(1);
    let state = EntityState::from_definitions([EntityDefinition::new(entity, "character")
        .with_transform(position)
        .with_character_motion(CharacterMotionComponent::at_rest(position.y))])
    .unwrap();
    (entity, state)
}

fn command(sequence: u64, intent: Vec2) -> CharacterControllerCommand {
    CharacterControllerCommand {
        planar_intent: intent,
        ..CharacterControllerCommand::idle(1.0 / 60.0, sequence)
    }
}

#[test]
fn a_character_walks_up_the_smooth_slope_on_its_surface() {
    let scene = slope_scene(dual_contoured(SurfaceMaterials::default()));
    let start_z = 20.0;
    let (entity, mut state) = character_at(Vec3::new(
        8.0,
        slope_height(start_z as f64) as f32 + 1.2,
        start_z,
    ));
    let config = CharacterControllerConfig::default();
    let mut service = CharacterControllerService::default();
    let mut sequence = 0;
    let mut settle = None;
    for _ in 0..60 {
        sequence += 1;
        settle = Some(
            service
                .step(
                    &mut state,
                    &scene,
                    entity,
                    &config,
                    command(sequence, Vec2::ZERO),
                )
                .unwrap(),
        );
    }
    let settled = settle.unwrap();
    assert!(settled.motion_after.grounded);
    let offset = |translation: Vec3| translation.y as f64 - slope_height(translation.z as f64);
    let resting = offset(settled.transform_after.translation);
    let mut last = settled;
    for _ in 0..90 {
        sequence += 1;
        last = service
            .step(
                &mut state,
                &scene,
                entity,
                &config,
                command(sequence, Vec2::new(0.0, 1.0)),
            )
            .unwrap();
    }
    let end = last.transform_after.translation;
    assert!(end.z < start_z - 3.0, "walked uphill to z {}", end.z);
    assert!(last.motion_after.grounded);
    assert!(
        (offset(end) - resting).abs() < 0.08,
        "stands on the surface: offset {} at rest {resting}",
        offset(end)
    );
}

#[test]
fn cube_materials_keep_cuboid_collision_in_a_reconstructed_session() {
    let brick = MaterialSurface {
        mode: SurfaceMode::GreedyCubes,
        character: SurfaceCharacter::default(),
    };
    let scene = slope_scene(dual_contoured(
        SurfaceMaterials::new([(BRICK, brick)]).unwrap(),
    ));
    let mut voxels = scene.material_voxels();
    // A brick pillar standing on the slope.
    for y in 0..12 {
        voxels.push(MaterialVoxel {
            state: 0,
            address: [8, y, 10],
            material_slot: BRICK,
        });
    }
    voxels.sort_by_key(|voxel| voxel.address);
    voxels.dedup_by_key(|voxel| voxel.address);
    for voxel in &mut voxels {
        if voxel.address[0] == 8 && voxel.address[2] == 10 {
            voxel.material_slot = BRICK;
        }
    }
    let mixed = VoxelCollisionScene::from_material_voxels_with_mesh_options(
        1.0,
        8,
        voxels,
        scene.mesh_options().clone(),
    )
    .unwrap();
    let top = down(&mixed, 8.5, 10.5);
    assert_eq!(top.point[1], 12.0);
    assert_eq!(top.voxel, [8, 11, 10]);
    // A ray into the pillar's side stops on its exact face.
    let side = mixed
        .raycast([2.0, 11.5, 10.5], [1.0, 0.0, 0.0], 32.0)
        .unwrap();
    assert_eq!(side.point[0], 8.0);
}

#[test]
fn a_blast_rebuilds_nearby_chunks_and_collision_follows_the_crater() {
    let mut scene = slope_scene_in_chunks(
        dual_contoured(
            SurfaceMaterials::new([(
                STONE,
                MaterialSurface {
                    mode: SurfaceMode::DualContouring,
                    character: SurfaceCharacter {
                        placement: VertexPlacement::Smooth,
                        crease_angle_degrees: 180.0,
                        roughness: 0.0,
                    },
                },
            )])
            .unwrap(),
        ),
        4,
    );
    let center = [8.0, slope_height(12.0), 12.0];
    assert!(scene.contains_point([8.0, 1.0, 12.0]), "deep rock is solid");
    let chunks = scene.mesh_chunks().len();
    let receipt = VoxelDensityEditService::apply(
        &mut scene,
        &[VoxelDensityEdit::Brush {
            shape: VoxelDensityShape::Sphere {
                center,
                radius: 2.5,
            },
            operation: VoxelDensityOperation::Subtract,
            material_slot: STONE,
        }],
    )
    .unwrap();
    assert!(receipt.solidity_changes > 0);
    assert!(receipt.rebuilt_mesh_chunks < chunks);
    // Voxels within the brush and its two-voxel margin (z 7 to 16) and the
    // chunks beside the boundary ones; chunks 0 and 5 along z are untouched.
    assert!(receipt
        .dirty_mesh_chunks
        .iter()
        .all(|chunk| (1..=4).contains(&chunk[2])));
    // The crater floor is the sphere's lower surface.
    let floor = down(&scene, 8.0, 12.0);
    assert!(
        (floor.point[1] - (center[1] - 2.5)).abs() < 0.2,
        "crater floor at {:?}",
        floor.point
    );
    assert!(!scene.contains_point([8.0, center[1] - 1.0, 12.0]));
    assert!(scene.contains_point([8.0, center[1] - 3.5, 12.0]));
    assert_eq!(scene.density([8, 1, 12]).map(|d| d < 0.0), Some(true));

    // Filling it back with a sphere of stone makes it solid again.
    VoxelDensityEditService::apply(
        &mut scene,
        &[VoxelDensityEdit::Brush {
            shape: VoxelDensityShape::Sphere {
                center,
                radius: 2.5,
            },
            operation: VoxelDensityOperation::Add,
            material_slot: STONE,
        }],
    )
    .unwrap();
    assert!(scene.contains_point([8.0, center[1] - 1.0, 12.0]));
}

#[test]
fn a_region_that_makes_empty_voxels_solid_needs_their_material() {
    let mut scene = slope_scene(dual_contoured(SurfaceMaterials::default()));
    let error = VoxelDensityEditService::apply(
        &mut scene,
        &[VoxelDensityEdit::Region {
            min: [2, 14, 2],
            size: [1, 1, 1],
            densities: vec![-0.3],
            materials: Vec::new(),
        }],
    )
    .unwrap_err();
    assert!(matches!(
        error,
        VoxelDensityApplyError::Rejected(VoxelDensityRejection::MissingMaterial { .. })
    ));
    let receipt = VoxelDensityEditService::apply(
        &mut scene,
        &[VoxelDensityEdit::Region {
            min: [2, 14, 2],
            size: [1, 1, 1],
            densities: vec![-0.3],
            materials: vec![BRICK],
        }],
    )
    .unwrap();
    assert_eq!(receipt.solidity_changes, 1);
    assert_eq!(
        scene.material_voxel([2, 14, 2]).unwrap().material_slot,
        BRICK
    );
    assert_eq!(scene.density([2, 14, 2]), Some(-0.3));
}

#[test]
fn residency_carries_densities_through_eviction_and_rebase() {
    use engine_spatial::{
        VoxelChunkIdentity, VoxelChunkPayload, VoxelChunkResidencyOperation,
        VoxelChunkResidencyService, WorldOrigin, WorldOriginRebaseRequest,
        WorldOriginRebaseService, WorldOriginState,
    };
    const SIZE: u32 = 8;
    // Two chunks side by side holding the plane y = 2.3 as densities.
    let payload = |_: i64| {
        let mut slots = Vec::new();
        let mut densities = Vec::new();
        for _z in 0..SIZE {
            for y in 0..SIZE {
                for _x in 0..SIZE {
                    let density = y as f32 + 0.5 - 2.3;
                    slots.push(if density < 0.0 { STONE } else { 0 });
                    densities.push(density);
                }
            }
        }
        let mut payload = VoxelChunkPayload::new([SIZE; 3], slots);
        payload.densities = densities;
        payload
    };
    let mut scene = VoxelCollisionScene::from_material_voxels_with_mesh_options(
        1.0,
        SIZE,
        [],
        dual_contoured(SurfaceMaterials::default()),
    )
    .unwrap();
    let admit = |x: i64| VoxelChunkResidencyOperation::Admit {
        chunk: VoxelChunkIdentity::new(x, 0, 0),
        payload: payload(x),
    };
    VoxelChunkResidencyService::apply(&mut scene, &[admit(0), admit(1)]).unwrap();
    let height = |scene: &VoxelCollisionScene, x: f64| {
        scene
            .raycast([x, 7.5, 4.0], [0.0, -1.0, 0.0], 16.0)
            .unwrap()
            .point[1]
    };
    assert!((height(&scene, 3.3) - 2.3).abs() < 1.0e-4);
    assert!((height(&scene, 8.0) - 2.3).abs() < 1.0e-4, "chunk border");
    assert!((scene.density([3, 2, 4]).unwrap() - 0.2).abs() < 1.0e-6);
    assert!((scene.density([3, 1, 4]).unwrap() + 0.8).abs() < 1.0e-6);

    // A payload whose density sign disagrees with its slot is refused.
    let mut wrong = payload(2);
    wrong.densities[0] = 0.4;
    assert!(VoxelChunkResidencyService::apply(
        &mut scene,
        &[VoxelChunkResidencyOperation::Admit {
            chunk: VoxelChunkIdentity::new(2, 0, 0),
            payload: wrong,
        }],
    )
    .is_err());

    // Evicted and readmitted, the chunk draws and collides the same.
    VoxelChunkResidencyService::apply(
        &mut scene,
        &[VoxelChunkResidencyOperation::Evict {
            chunk: VoxelChunkIdentity::new(1, 0, 0),
        }],
    )
    .unwrap();
    VoxelChunkResidencyService::apply(&mut scene, &[admit(1)]).unwrap();
    assert!((height(&scene, 12.5) - 2.3).abs() < 1.0e-4);

    // A rebase by whole cells keeps the densities and moves the surface.
    let mut origin = WorldOriginState::default();
    let prepared = WorldOriginRebaseService
        .prepare(
            &origin,
            WorldOriginRebaseRequest {
                target_origin: WorldOrigin::new([4, 1, 0]),
                entities: Vec::new(),
            },
        )
        .unwrap();
    let (rebased, _) = WorldOriginRebaseService
        .commit(&mut origin, &scene, &prepared)
        .unwrap();
    assert!((height(&rebased, 3.3 - 4.0) - 1.3).abs() < 1.0e-4);
    assert_eq!(rebased.density([3, 2, 4]), scene.density([3, 2, 4]));
}

/// A 4×2×4 dual-contoured slab in an 8-cell chunk.
fn dual_contoured_slab() -> VoxelCollisionScene {
    VoxelCollisionScene::from_solid_voxels_with_mesh_options(
        1.0,
        8,
        (0..4).flat_map(|x| (0..4).flat_map(move |z| (0..2).map(move |y| [x, y, z]))),
        SurfaceMeshOptions::with_mode(SurfaceMode::DualContouring),
    )
    .unwrap()
}

#[test]
fn a_noncollidable_material_change_removes_retained_surface_collision() {
    let mut scene = dual_contoured_slab();
    let down = |scene: &VoxelCollisionScene| scene.raycast([1.5, 5.0, 1.5], [0.0, -1.0, 0.0], 10.0);
    assert!(down(&scene).is_some());
    // The drawn mesh is unchanged; only which materials collide changes.
    scene.set_noncollidable_materials(BTreeSet::from([1]));
    assert!(down(&scene).is_none(), "noncollidable surface still hit");
    scene.set_noncollidable_materials(BTreeSet::new());
    assert!(down(&scene).is_some());
}

#[test]
fn rays_meet_the_drawn_slope_under_passable_water_that_does_not_occlude() {
    const WATER: u16 = 3;
    const LEVEL: i64 = 6;
    let mut scene = slope_scene(dual_contoured(SurfaceMaterials::default()));
    // Fill the air below `LEVEL` with water whose densities continue the
    // slope's field, so emptied water reads exactly as the air did.
    let mut densities = Vec::new();
    let mut materials = Vec::new();
    for z in 0..DEPTH {
        for y in 0..HEIGHT {
            for _ in 0..WIDTH {
                let distance = (y as f64 + 0.5 - slope_height(z as f64 + 0.5)) as f32;
                let (density, material) = if distance < 0.0 {
                    (distance, STONE)
                } else if y < LEVEL {
                    (-distance.max(1.0e-3), WATER)
                } else {
                    (distance, 0)
                };
                densities.push(density);
                materials.push(material);
            }
        }
    }
    VoxelDensityEditService::apply(
        &mut scene,
        &[VoxelDensityEdit::Region {
            min: [0, 0, 0],
            size: [WIDTH as u32, HEIGHT as u32, DEPTH as u32],
            densities,
            materials,
        }],
    )
    .unwrap();
    scene.set_noncollidable_materials(BTreeSet::from([WATER]));
    // Water that occludes hides the slope, so the voxels under it collide
    // as cuboids.
    let (x, z) = (5.3, 17.1);
    assert!(slope_height(z) < LEVEL as f64);
    assert_eq!(down(&scene, x, z).point[1].fract(), 0.0);
    // Declared non-occluding, the slope is drawn under the water and rays
    // meet it there.
    scene
        .set_mesh_options(SurfaceMeshOptions {
            non_occluding: BTreeSet::from([WATER]),
            ..scene.mesh_options().clone()
        })
        .unwrap();
    let hit = down(&scene, x, z);
    assert!(
        (hit.point[1] - slope_height(z)).abs() < 1.0e-3,
        "hit {:?} should be at {}",
        hit.point,
        slope_height(z)
    );
    assert_eq!(
        scene.material_voxel(hit.voxel).unwrap().material_slot,
        STONE
    );
}

#[test]
fn box_queries_reach_surface_triangles_past_their_owning_chunk() {
    let mut scene = dual_contoured_slab();
    // Softened densities bulge the surface past x = 0, the chunk's edge.
    VoxelDensityEditService::apply(
        &mut scene,
        &[VoxelDensityEdit::Region {
            min: [0, 0, 0],
            size: [4, 2, 4],
            densities: vec![-2.0; 32],
            materials: vec![],
        }],
    )
    .unwrap();
    let hit = scene
        .raycast([-0.1, 5.0, 1.5], [0.0, -1.0, 0.0], 10.0)
        .expect("the drawn surface reaches into chunk -1");
    let point = hit.point;
    assert!(point[0] < 0.0);
    assert!(
        scene.aabb_overlaps_solid(point.map(|v| v - 0.02), point.map(|v| v + 0.02)),
        "a box around the hit at {point:?} misses the surface"
    );
}

#[test]
fn a_merged_blocky_floor_names_the_voxel_under_each_hit() {
    let blocky = SurfaceMaterials::new([(
        1,
        MaterialSurface {
            mode: SurfaceMode::DualContouring,
            character: SurfaceCharacter {
                placement: VertexPlacement::Blocky,
                crease_angle_degrees: 0.0,
                roughness: 0.0,
            },
        },
    )])
    .unwrap();
    let voxels = (0..24).flat_map(|x| {
        (0..24).flat_map(move |z| {
            (0..3).map(move |y| MaterialVoxel {
                state: 0,
                address: [x, y, z],
                material_slot: 1,
            })
        })
    });
    let scene = VoxelCollisionScene::from_material_voxels_with_mesh_options(
        1.0,
        8,
        voxels,
        dual_contoured(blocky),
    )
    .unwrap();
    // The middle chunk's top is one 6 × 6 rectangle inside the 28 faces
    // that touch its border: 58 triangles, not 128.
    let middle = scene.mesh_chunk([1, 0, 1]).unwrap();
    let top = middle
        .indices
        .chunks(3)
        .filter(|triangle| {
            triangle
                .iter()
                .all(|corner| middle.positions[*corner as usize * 3 + 1] == 3.0)
        })
        .count();
    assert_eq!(top, 28 * 2 + 2);
    assert!(!middle.triangle_owner_spans.is_empty());
    for (x, z) in [(10.5, 10.5), (12.25, 9.75), (13.9, 14.1)] {
        let hit = scene
            .raycast([x, 10.0, z], [0.0, -1.0, 0.0], 64.0)
            .expect("the ray meets the floor");
        assert_eq!(hit.point[1], 3.0);
        assert_eq!(hit.voxel, [x.floor() as i64, 2, z.floor() as i64]);
    }
}
