//! Vegetation copies that collide (#10951): a layer's tree, placed on a
//! field's tile, stands in the session as an upright cylinder that stops a
//! character and that a collision-derived grid routes around, and leaves the
//! session when the field no longer wants it.

use super::*;
use collision_navigation::flat_config;
use render_projection::{
    VegetationCollider, VegetationFields, VegetationLayer, VegetationLayout, VegetationRecord,
    VegetationTileContent,
};

const FIELD: u64 = 1;
const TREES: u32 = 4;
const GRID: u64 = 7;
/// The floor's top, and where the trunk stands on it.
const FLOOR: f32 = 2.0;
const TRUNK: [f32; 2] = [4.0, 4.0];
const RADIUS: f32 = 0.8;

/// A flat floor whose top is at `FLOOR`, about the trunk.
fn floor() -> Arc<VoxelCollisionScene> {
    let mut voxels = Vec::new();
    for x in -12..12 {
        for z in -12..12 {
            for y in 0..FLOOR as i64 {
                voxels.push([x, y, z]);
            }
        }
    }
    Arc::new(VoxelCollisionScene::from_solid_voxels(1.0, 16, voxels).unwrap())
}

fn trees() -> VegetationLayer {
    VegetationLayer {
        mesh: "mesh/tree".into(),
        material_overrides: Vec::new(),
        shadow_casting: render_model::ShadowCasting::None,
        density: 0.0,
        grounds: 1,
        slope_limit_degrees: 45.0,
        align: 0.0,
        scale: [1.0, 1.0],
        tints: [[1.0; 3]; 2],
        seed: 3,
        reach: 60.0,
        fade: 4.0,
        near: 0.0,
        near_fade: 0.0,
        maximum_instances: 1_000,
        lods: Vec::new(),
        lod_band: 0.0,
        thinning: Default::default(),
        collider: Some(VegetationCollider {
            radius: RADIUS,
            height: 3.0,
            reach: 30.0,
        }),
    }
}

/// A field focused on the trunk, its tiles flat at the floor's top, the
/// tile at the origin holding the one tree; settled until it is placed.
fn grown() -> VegetationFields {
    let mut fields = VegetationFields::default();
    let layout = VegetationLayout {
        tile_size: 16.0,
        tile_cells: 8,
        layer: render_model::RenderLayer::Scene,
        scale: 1.0,
    };
    fields.create(FIELD, layout, 64).unwrap();
    fields.set_layer(FIELD, TREES, trees()).unwrap();
    let focus = Some([f64::from(TRUNK[0]), f64::from(FLOOR), f64::from(TRUNK[1])]);
    fields.settle(FIELD, [0.0; 3], focus).unwrap();
    for request in fields.requests(FIELD, 0) {
        let records = if (request.x, request.z) == (0, 0) {
            vec![VegetationRecord {
                layer: TREES,
                position: [TRUNK[0], FLOOR, TRUNK[1]],
                yaw: 0.3,
                scale: 1.0,
            }]
        } else {
            Vec::new()
        };
        let content = VegetationTileContent {
            samples: svc_mesh::heightfield::TileSamples {
                heights: vec![FLOOR; 11 * 11],
                materials: Some(vec![0; 9 * 9]),
                ..Default::default()
            },
            cover: None,
            records,
        };
        assert!(fields
            .commit(FIELD, (request.x, request.z), request.revision, content)
            .unwrap());
    }
    for _ in 0..1_000 {
        fields.settle(FIELD, [0.0; 3], focus).unwrap();
        if fields.readout(FIELD).unwrap().deferred == 0 {
            return fields;
        }
        fields.wait_for_placement();
    }
    panic!("the tree was never placed");
}

fn session() -> (RuntimeSpatialBridge, NativeSpatialSessionHandle) {
    let mut bridge = RuntimeSpatialBridge::new();
    let session = bridge
        .create(NativeSpatialSessionConfig {
            collision_voxel_size: 1.0,
            collision_chunk_size: 16,
            voxel_surface_mode: NativeVoxelSurfaceMode::GreedyCubes,
        })
        .unwrap();
    bridge.sessions.get_mut(&session.value).unwrap().scene = floor();
    (bridge, session)
}

/// Hand the session these colliders, and begin calls until they install.
fn stand(
    bridge: &mut RuntimeSpatialBridge,
    session: NativeSpatialSessionHandle,
    wanted: &[((u32, (i64, i64)), render_projection::VegetationColliderTile)],
) {
    bridge
        .session_mut(session)
        .unwrap()
        .set_vegetation_colliders(FIELD, [0.0; 3], 1.0, wanted);
    for _ in 0..5_000 {
        bridge.settle_deferred_residency();
        if bridge.sessions[&session.value].static_deferred.is_empty() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    panic!("the colliders never installed");
}

fn copies(bridge: &RuntimeSpatialBridge, session: NativeSpatialSessionHandle) -> usize {
    bridge.sessions[&session.value]
        .scene
        .static_mesh_projection()
        .instance_count()
}

/// A character walking toward −z from in front of the trunk for two
/// seconds: where it ends.
fn walk(bridge: &mut RuntimeSpatialBridge, session: NativeSpatialSessionHandle) -> NativeVec3 {
    let mut position = NativeVec3 {
        x: TRUNK[0],
        y: FLOOR + 1.0,
        z: TRUNK[1] + 5.0,
    };
    let mut motion = NativeCharacterMotion {
        stance: NativeCharacterStance::Standing,
        fall_origin_y: position.y,
        peak_y: position.y,
        ..Default::default()
    };
    let config = bridge.default_character_controller_config();
    for sequence in 1..=120 {
        let receipt = bridge
            .propose_character(NativeCharacterStepRequest {
                tether: NativeCharacterTetherRequest::default(),
                session,
                position,
                motion,
                support: NativeCharacterSupport::default(),
                obstacles: std::ptr::null(),
                obstacles_len: 0,
                mesh_instances: std::ptr::null(),
                mesh_instances_len: 0,
                config,
                command: NativeCharacterControllerCommand {
                    movement: Default::default(),
                    planar_intent: NativeVec2 { x: 0.0, y: 1.0 },
                    heading_yaw_radians: 0.0,
                    jump_pressed: false,
                    jump_held: false,
                    crouch_requested: false,
                    external_velocity: NativeVec3::default(),
                    external_impulse: NativeVec3::default(),
                    step_seconds: 1.0 / 60.0,
                    sequence,
                },
            })
            .unwrap();
        position = receipt.transform.translation;
        motion = receipt.motion;
    }
    position
}

/// The wild grid's path from in front of the trunk to behind it, as cell
/// centres.
fn route(bridge: &mut RuntimeSpatialBridge, session: NativeSpatialSessionHandle) -> Vec<[f32; 2]> {
    let mut config = flat_config(0.5, 1, 0.3, 1.6, 45.0);
    config.grid_id = GRID;
    config.maximum_cells = 1 << 16;
    bridge
        .replace_collision_navigation(&NativeCollisionNavigationReplaceRequest {
            session,
            world_min: NativeVec3 {
                x: -4.0,
                y: 0.0,
                z: -4.0,
            },
            world_max: NativeVec3 {
                x: 12.0,
                y: 8.0,
                z: 12.0,
            },
            config,
            deferred: false,
        })
        .unwrap();
    let result = bridge
        .evaluate_navigation(NativeNavigationStepRequest {
            session,
            from: NativeVec3 {
                x: TRUNK[0],
                y: FLOOR,
                z: TRUNK[1] + 4.0,
            },
            target: NativeVec3 {
                x: TRUNK[0],
                y: FLOOR,
                z: TRUNK[1] - 4.0,
            },
            max_step_units: 0.5,
            max_visited: 1 << 16,
            grid_id: GRID,
        })
        .unwrap();
    assert_eq!(result.outcome, NativeNavigationPathOutcome::Reached);
    unsafe { std::slice::from_raw_parts(result.path, result.path_len) }
        .iter()
        .map(|cell| [(cell.x as f32 + 0.5) * 0.5, (cell.z as f32 + 0.5) * 0.5])
        .collect()
}

/// How far a path's nearest cell centre passes the trunk's axis.
fn clearance(path: &[[f32; 2]]) -> f32 {
    path.iter()
        .map(|[x, z]| (x - TRUNK[0]).hypot(z - TRUNK[1]))
        .fold(f32::INFINITY, f32::min)
}

#[test]
fn a_colliding_tree_stops_a_character_and_a_derived_grid_routes_around_it() {
    let fields = grown();
    assert_eq!(fields.colliders(FIELD).len(), 1);

    // With no tree standing, the character walks straight through its place
    // and the grid runs straight past it.
    let (mut bridge, session) = session();
    assert!(walk(&mut bridge, session).z < TRUNK[1] - 2.0);
    assert!(clearance(&route(&mut bridge, session)) < RADIUS);

    // Standing, it stops the character at its face and the grid goes round.
    stand(&mut bridge, session, &fields.colliders(FIELD));
    assert_eq!(copies(&bridge, session), 1);
    let stopped = walk(&mut bridge, session);
    assert!(
        stopped.z > TRUNK[1] + RADIUS && stopped.z < TRUNK[1] + RADIUS + 0.6,
        "stopped at the trunk: {stopped:?}"
    );
    let around = route(&mut bridge, session);
    assert!(
        clearance(&around) > RADIUS,
        "the path keeps off the trunk: {around:?}"
    );

    // Wanted no more, it leaves the session.
    stand(&mut bridge, session, &[]);
    assert_eq!(copies(&bridge, session), 0);
    assert!(walk(&mut bridge, session).z < TRUNK[1] - 2.0);
}

#[test]
fn a_tree_placed_again_replaces_its_collider_and_one_unchanged_is_not_sent_again() {
    let fields = grown();
    let (mut bridge, session) = session();
    stand(&mut bridge, session, &fields.colliders(FIELD));
    let revision = bridge.sessions[&session.value]
        .scene
        .static_mesh_projection()
        .revision();
    // The same placement again changes nothing.
    stand(&mut bridge, session, &fields.colliders(FIELD));
    assert_eq!(
        bridge.sessions[&session.value]
            .scene
            .static_mesh_projection()
            .revision(),
        revision
    );
    // Placed again (a new stamp), its old collider goes and the new comes.
    let mut wanted = fields.colliders(FIELD);
    wanted[0].1.stamp += 1;
    stand(&mut bridge, session, &wanted);
    assert_eq!(copies(&bridge, session), 1);
    assert_ne!(
        bridge.sessions[&session.value]
            .scene
            .static_mesh_projection()
            .revision(),
        revision
    );
}
