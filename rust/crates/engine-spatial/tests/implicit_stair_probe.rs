use core_ids::EntityId;
use core_math::{Vec2, Vec3};
use engine_spatial::{
    CharacterControllerCommand, CharacterControllerConfig, CharacterControllerService,
    StaticMeshAssetId, StaticMeshColliderAsset, StaticMeshColliderInstance, StaticMeshInstanceId,
    StaticMeshTransform, VoxelCollisionScene,
};
use entity_state::{CharacterMotionComponent, EntityDefinition, EntityState};
use svc_implicit::{
    surface::{assemble, SurfaceOptions},
    Bounds, Field, GenerateOptions,
};

type StaticMesh = (Vec<[f64; 3]>, Vec<[u32; 3]>);
const BEVEL_CELL_SIZES: [f32; 3] = [0.16, 0.20, 0.26];

fn stairs(bevelled: bool, cell_size: f32) -> (StaticMesh, StaticMesh) {
    let mut field = Field::new();
    let mut source = None;
    for step in 0..6 {
        let front = 6.0 + 4.0 * step as f32 / 6.0;
        let top = 3.0 + 2.0 * (step + 1) as f32 / 6.0;
        let box_node = field
            .box_shape(Bounds {
                min: [-2.0, 2.9, front],
                max: [2.0, top, 10.08],
            })
            .unwrap();
        let step_node = if bevelled {
            // y - z <= top - (front + 1/3): a 45-degree nose through the
            // first 1/3 m of each tread, then a flat final 1/3 m.
            let plane = field
                .plane([0.0, 1.0, -1.0], top - (front + 1.0 / 3.0))
                .unwrap();
            field.intersection(box_node, plane).unwrap()
        } else {
            box_node
        };
        source = Some(match source {
            Some(existing) => field.union(existing, step_node).unwrap(),
            None => step_node,
        });
    }
    let geometry = field
        .generate(
            source.unwrap(),
            GenerateOptions {
                bounds: Bounds {
                    min: [-2.25, 2.65, 5.75],
                    max: [2.25, 5.25, 10.33],
                },
                cell_size,
                max_vertices: 262_144,
                max_triangles: 262_144,
            },
        )
        .unwrap();
    let attributed = assemble(
        &field,
        &geometry,
        &[],
        SurfaceOptions {
            crease_angle_degrees: 38.0,
            uv_scale: 1.0,
            default_slot: 0,
            material_boundary_mode: svc_implicit::surface::MaterialBoundaryMode::Centroid,
            material_sampling: None,
        },
    )
    .unwrap();
    let raw_positions = geometry
        .positions
        .into_iter()
        .map(|position| position.map(f64::from))
        .collect();
    let split_positions = attributed
        .positions
        .into_iter()
        .map(|position| position.map(f64::from))
        .collect();
    let (split_triangles, remainder) = attributed.indices.as_chunks::<3>();
    assert!(remainder.is_empty());
    (
        (raw_positions, geometry.triangles),
        (split_positions, split_triangles.to_vec()),
    )
}

fn box_mesh(min: [f64; 3], max: [f64; 3]) -> StaticMesh {
    let positions = vec![
        [min[0], min[1], min[2]],
        [max[0], min[1], min[2]],
        [max[0], max[1], min[2]],
        [min[0], max[1], min[2]],
        [min[0], min[1], max[2]],
        [max[0], min[1], max[2]],
        [max[0], max[1], max[2]],
        [min[0], max[1], max[2]],
    ];
    let triangles = vec![
        [0, 2, 1],
        [0, 3, 2],
        [4, 5, 6],
        [4, 6, 7],
        [0, 1, 5],
        [0, 5, 4],
        [1, 2, 6],
        [1, 6, 5],
        [2, 3, 7],
        [2, 7, 6],
        [3, 0, 4],
        [3, 4, 7],
    ];
    (positions, triangles)
}

fn scene(stairs: StaticMesh) -> VoxelCollisionScene {
    let (floor_positions, floor_triangles) = box_mesh([-12.0, 2.5, -10.0], [12.0, 3.0, 10.0]);
    let (passage_positions, passage_triangles) = box_mesh([-2.6, 4.5, 10.0], [2.6, 5.0, 22.0]);
    let assets = vec![
        StaticMeshColliderAsset::new(StaticMeshAssetId(1), floor_positions, floor_triangles)
            .unwrap(),
        StaticMeshColliderAsset::new(StaticMeshAssetId(2), stairs.0, stairs.1).unwrap(),
        StaticMeshColliderAsset::new(StaticMeshAssetId(3), passage_positions, passage_triangles)
            .unwrap(),
    ];
    let instances: Vec<_> = assets
        .iter()
        .enumerate()
        .map(|(index, asset)| StaticMeshColliderInstance {
            id: StaticMeshInstanceId((index + 1) as u64),
            asset: asset.id,
            expected_geometry_hash: asset.geometry_hash,
            transform: StaticMeshTransform::IDENTITY,
        })
        .collect();
    let mut scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, []).unwrap();
    scene
        .replace_static_mesh_colliders(0, assets, instances)
        .unwrap();
    scene
}

fn config() -> CharacterControllerConfig {
    let mut config = CharacterControllerConfig::default();
    config.shape.standing_height = 1.75;
    config.shape.radius = 0.3;
    config.shape.contact_skin = 0.015;
    config.ground.forward_speed = 7.0;
    config.vertical.gravity = 24.0;
    config.surface.maximum_step_height = 1.05;
    config.surface.floor_snap_distance = 0.25;
    config
}

fn march(scene: &VoxelCollisionScene) -> Vec3 {
    let entity = EntityId::new(1);
    let mut state = EntityState::from_definitions([EntityDefinition::new(entity, "character")
        .with_transform(Vec3::new(0.0, 3.875, -1.0))
        .with_character_motion(CharacterMotionComponent::at_rest(3.875))])
    .unwrap();
    let config = config();
    let mut service = CharacterControllerService::default();
    for sequence in 1..=480 {
        let mut command = CharacterControllerCommand::idle(1.0 / 120.0, sequence);
        command.planar_intent = Vec2::new(0.0, 1.0);
        command.heading_yaw_radians = std::f32::consts::PI;
        service
            .step(&mut state, scene, entity, &config, command)
            .unwrap();
    }
    state.transform(entity).unwrap().translation
}

fn assert_equivalent(raw: Vec3, split: Vec3) {
    for (raw, split) in [raw.x, raw.y, raw.z]
        .into_iter()
        .zip([split.x, split.y, split.z])
    {
        assert!((raw - split).abs() < 1.0e-5);
    }
}

#[test]
fn hard_riser_collision_is_unchanged_by_render_attribute_vertex_splits() {
    let (raw, split) = stairs(false, BEVEL_CELL_SIZES[0]);
    let raw_final = march(&scene(raw));
    let split_final = march(&scene(split));
    assert_equivalent(raw_final, split_final);
    assert!(raw_final.z > 11.0, "hard risers stopped at {raw_final:?}");
}

#[test]
fn bevelled_implicit_stairs_reach_the_upper_passage() {
    for cell_size in BEVEL_CELL_SIZES {
        let (raw, split) = stairs(true, cell_size);
        let raw_final = march(&scene(raw));
        let split_final = march(&scene(split));
        assert_equivalent(raw_final, split_final);
        assert!(
            raw_final.z > 11.0,
            "bevelled {cell_size} m flight stopped at {raw_final:?}"
        );
    }
}
