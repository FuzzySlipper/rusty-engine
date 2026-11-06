//! A heightfield collider as a product uses it: tiles sampled when asked,
//! admitted off the update, and read by everything that reads the session's
//! collision, with no static mesh of the product's in the scene.

use super::*;

const CELLS: u32 = 8;
const REACH: f64 = 12.0;

fn ground(x: f64, z: f64) -> f32 {
    (0.8 * (x / 6.0).sin() + 0.5 * (z / 5.0).cos()) as f32
}

fn origin_anchor() -> NativeWorldOriginGlobalPosition {
    NativeWorldOriginGlobalPosition::default()
}

struct Walk {
    bridge: Box<RuntimeSpatialBridge>,
    session: NativeSpatialSessionHandle,
    collider: NativeHeightfieldColliderHandle,
    surface: fn(f64, f64) -> f32,
}

impl Walk {
    fn new(surface: fn(f64, f64) -> f32, anchor: NativeWorldOriginGlobalPosition) -> Self {
        let mut bridge = Box::new(RuntimeSpatialBridge::new());
        let session = bridge
            .create(NativeSpatialSessionConfig {
                collision_voxel_size: 1.0,
                collision_chunk_size: 8,
                voxel_surface_mode: NativeVoxelSurfaceMode::GreedyCubes,
            })
            .unwrap();
        let collider = bridge
            .create_collider(NativeHeightfieldColliderCreateRequest {
                session,
                tile_cells: CELLS,
                cell_size: 1.0,
                reach: REACH,
                anchor,
            })
            .unwrap();
        Self {
            bridge,
            session,
            collider,
            surface,
        }
    }

    fn focus(&mut self, x: f64, z: f64) {
        let focuses = [NativeHeightfieldColliderFocus { x, z }];
        self.bridge
            .set_focuses(&NativeHeightfieldColliderFocusRequest {
                collider: self.collider,
                focuses: focuses.as_ptr(),
                focuses_len: focuses.len(),
            })
            .unwrap();
    }

    fn requests(&mut self) -> Vec<NativeHeightfieldTileRequest> {
        let result = self
            .bridge
            .read_requests(NativeHeightfieldColliderRequestsRequest {
                collider: self.collider,
                max_requests: 0,
            })
            .unwrap();
        unsafe { std::slice::from_raw_parts(result.requests, result.requests_len) }.to_vec()
    }

    /// Sample every tile asked for, as a streamer would.
    fn sample(&mut self) {
        for tile in self.requests() {
            assert!(self.commit(&tile, self.surface));
        }
    }

    fn commit(
        &mut self,
        tile: &NativeHeightfieldTileRequest,
        surface: fn(f64, f64) -> f32,
    ) -> bool {
        let side = CELLS as usize + 3;
        let heights: Vec<f32> = (0..side * side)
            .map(|index| {
                let (i, j) = ((index % side) as f64 - 1.0, (index / side) as f64 - 1.0);
                surface(
                    tile.origin_x + i * tile.cell_size,
                    tile.origin_z + j * tile.cell_size,
                )
            })
            .collect();
        self.bridge
            .commit_tile(&NativeHeightfieldColliderTileCommit {
                collider: self.collider,
                x: tile.x,
                z: tile.z,
                revision: tile.revision,
                heights: heights.as_ptr(),
                heights_len: heights.len(),
            })
            .unwrap()
            .accepted
    }

    fn read(&mut self, x: f64, z: f64, reach: f64) -> NativeHeightfieldColliderReadout {
        self.bridge
            .read(NativeHeightfieldColliderReadRequest {
                collider: self.collider,
                x,
                z,
                reach,
            })
            .unwrap()
    }

    /// Calls begin until the ground about (x, z) stands.
    fn admit(&mut self, x: f64, z: f64) {
        for _ in 0..2_000 {
            self.bridge.settle_deferred_residency();
            if self.read(x, z, REACH).ready {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        panic!("the ground never stood: {:?}", self.read(x, z, REACH));
    }

    fn arrive(&mut self, x: f64, z: f64) {
        self.focus(x, z);
        self.sample();
        self.admit(x, z);
    }

    /// Where a ray straight down over (x, z) meets the world, from `top`.
    fn ground_at(&mut self, x: f64, z: f64, top: f64) -> Option<f64> {
        let hit = self
            .bridge
            .cast_ray(&NativeSpatialRaycastRequest {
                session: self.session,
                origin: NativeVec3 {
                    x: x as f32,
                    y: top as f32,
                    z: z as f32,
                },
                direction: NativeVec3 {
                    x: 0.0,
                    y: -1.0,
                    z: 0.0,
                },
                max_distance: 100.0,
                filter: NativeSpatialQueryFilter {
                    collision_group: u32::MAX,
                    collision_mask: u32::MAX,
                },
                entities: std::ptr::null(),
                entities_len: 0,
                ignored_entities: std::ptr::null(),
                ignored_entities_len: 0,
                hitbox_overrides: std::ptr::null(),
                hitbox_overrides_len: 0,
            })
            .unwrap();
        hit.present.then_some(top - hit.distance)
    }

    /// One step of a character walking straight on.
    fn step(
        &mut self,
        position: NativeVec3,
        motion: NativeCharacterMotion,
        sequence: u64,
    ) -> NativeCharacterStepReceipt {
        let config = self.bridge.default_character_controller_config();
        self.bridge
            .propose_character(NativeCharacterStepRequest {
                tether: NativeCharacterTetherRequest::default(),
                session: self.session,
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
                    heading_yaw_radians: -std::f32::consts::FRAC_PI_2,
                    jump_pressed: false,
                    jump_held: false,
                    crouch_requested: false,
                    external_velocity: NativeVec3::default(),
                    external_impulse: NativeVec3::default(),
                    step_seconds: 1.0 / 60.0,
                    sequence,
                },
            })
            .unwrap()
    }
}

/// The height the finest drawn ring draws at (x, z): the triangle of its cell,
/// by the drawn tile's own surface.
fn drawn(x: f64, z: f64) -> f32 {
    use svc_mesh::heightfield::{
        HeightfieldLayout, HeightfieldRingSpec, TileKey, TileSamples, TileSurface,
    };
    let layout = HeightfieldLayout::new(
        CELLS,
        &[
            HeightfieldRingSpec {
                cell_size: 1.0,
                reach: 16.0,
            },
            HeightfieldRingSpec {
                cell_size: 4.0,
                reach: 128.0,
            },
        ],
    )
    .unwrap();
    let side = f64::from(CELLS);
    let key = TileKey {
        ring: 0,
        x: (x / side).floor() as i64,
        z: (z / side).floor() as i64,
    };
    let origin = layout.origin(key);
    let heights = layout.height_side() as i64;
    let surface = TileSurface::new(
        &layout,
        0,
        TileSamples {
            heights: (0..heights * heights)
                .map(|index| {
                    let (i, j) = ((index % heights - 1) as f64, (index / heights - 1) as f64);
                    ground(origin[0] + i, origin[1] + j)
                })
                .collect(),
            ..TileSamples::default()
        },
    );
    surface.surface(x - origin[0], z - origin[1]).unwrap().0
}

#[test]
fn a_character_walks_sampled_ground_onto_tiles_it_had_to_wait_for() {
    let mut walk = Walk::new(ground, origin_anchor());
    walk.arrive(1.0, 1.0);
    let scene = walk.bridge.session_mut(walk.session).unwrap().scene.clone();
    assert!(
        scene.static_mesh_projection().instance_count() > 0,
        "the ground is in the collision scene"
    );
    let mut position = NativeVec3 {
        x: 1.0,
        y: ground(1.0, 1.0) + 2.0,
        z: 1.0,
    };
    let mut motion = NativeCharacterMotion {
        stance: NativeCharacterStance::Standing,
        fall_origin_y: position.y,
        peak_y: position.y,
        ..Default::default()
    };
    let mut feet_error = 0.0_f32;
    let mut walked = 0;
    for sequence in 1..1_200 {
        let receipt = walk.step(position, motion, sequence);
        position = receipt.transform.translation;
        motion = receipt.motion;
        if sequence % 20 == 0 {
            walk.arrive(f64::from(position.x), f64::from(position.z));
        }
        if sequence > 60 && receipt.ground.present {
            walked += 1;
            let feet = receipt.ground.point;
            let under = drawn(f64::from(feet.x), f64::from(feet.z));
            feet_error = feet_error.max((feet.y - under).abs());
        }
    }
    assert!(
        position.x.abs() > 40.0,
        "it walked across tiles: {position:?}"
    );
    assert!(walked > 900, "it stayed on the ground: {walked}");
    eprintln!("FEET {feet_error}");
    assert!(feet_error < 0.01, "feet and drawn ground: {feet_error}");
}

#[test]
fn ground_not_resident_is_not_ready_and_a_character_can_be_held_for_it() {
    let mut walk = Walk::new(ground, origin_anchor());
    walk.arrive(1.0, 1.0);
    assert!(
        walk.ground_at(200.0, 1.0, 20.0).is_none(),
        "nothing there yet"
    );
    assert!(!walk.read(200.0, 1.0, REACH).ready);
    walk.focus(200.0, 1.0);
    assert!(!walk.read(200.0, 1.0, REACH).ready, "wanted, not sampled");
    assert!(walk.read(200.0, 1.0, REACH).pending_tiles > 0);
    walk.sample();
    assert!(!walk.read(200.0, 1.0, REACH).ready, "sampled, not admitted");
    walk.admit(200.0, 1.0);
    let hit = walk.ground_at(200.0, 1.0, 20.0).unwrap();
    assert!((hit - f64::from(ground(200.0, 1.0))).abs() < 0.2);
    assert!(
        walk.ground_at(1.0, 1.0, 20.0).is_none(),
        "the old ground left"
    );
}

#[test]
fn a_ray_a_contact_and_a_navigation_grid_read_the_same_ground() {
    let mut walk = Walk::new(ground, origin_anchor());
    walk.arrive(4.0, 4.0);
    let (x, z) = (3.25, 2.5);
    let under = f64::from(drawn(x, z));
    let hit = walk.ground_at(x, z, 20.0).expect("a ray meets it");
    assert!(
        (hit - under).abs() < 1e-3,
        "ray {hit} against drawn {under}"
    );

    let shape = NativeSpatialShape {
        kind: NativeSpatialShapeKind::Sphere,
        center: NativeVec3::default(),
        half_extents: NativeVec3::default(),
        a: NativeVec3::default(),
        b: NativeVec3::default(),
        radius: 0.25,
    };
    let contacts_at = |walk: &mut Walk, height: f64| {
        let pose = NativeSpatialPose {
            translation: NativeVec3 {
                x: x as f32,
                y: height as f32,
                z: z as f32,
            },
            ..Default::default()
        };
        let api = api(&mut walk.bridge);
        let mut result = NativeSpatialContactResult {
            contacts: std::ptr::null(),
            contacts_len: 0,
            world_contact: false,
            truncated: false,
            projection_version: 0,
            source_revision: 0,
        };
        assert_eq!(
            unsafe {
                (api.overlap_contacts)(
                    api.context,
                    &NativeSpatialContactQueryRequest {
                        session: walk.session,
                        shape,
                        start: pose,
                        end: pose,
                        pivot: NativeVec3::default(),
                        filter: NativeSpatialQueryFilter {
                            collision_group: u32::MAX,
                            collision_mask: u32::MAX,
                        },
                        entities: std::ptr::null(),
                        entities_len: 0,
                        parts: std::ptr::null(),
                        parts_len: 0,
                        ignored_entities: std::ptr::null(),
                        ignored_entities_len: 0,
                        include_triggers: false,
                        exclude_world: false,
                        max_contacts: 0,
                    },
                    &mut result,
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        result.world_contact
    };
    assert!(contacts_at(&mut walk, under + 0.1), "touching the ground");
    assert!(!contacts_at(&mut walk, under + 0.5), "above it");

    let api = api(&mut walk.bridge);
    let mut request = collision_navigation_request(walk.session);
    request.world_min = NativeVec3 {
        x: 0.0,
        y: -3.0,
        z: 0.0,
    };
    request.world_max = NativeVec3 {
        x: 8.0,
        y: 3.0,
        z: 8.0,
    };
    request.config.maximum_cells = 256;
    let mut receipt = NativeCollisionNavigationReplaceReceipt::default();
    assert_eq!(
        unsafe {
            (api.replace_collision_navigation)(
                api.context,
                &request,
                &mut receipt,
                std::ptr::null_mut(),
            )
        },
        ABI_OK
    );
    assert!(
        receipt.walkable_cell_count >= 48,
        "the ground is walkable: {}",
        receipt.walkable_cell_count
    );
}

fn with_hole(x: f64, z: f64) -> f32 {
    if (8.0..16.0).contains(&x) && (0.0..8.0).contains(&z) {
        f32::NAN
    } else {
        ground(x, z)
    }
}

#[test]
fn nan_leaves_a_hole() {
    let mut walk = Walk::new(with_hole, origin_anchor());
    walk.arrive(4.0, 4.0);
    assert!(walk.ground_at(12.0, 4.0, 20.0).is_none(), "a hole");
    assert!(walk.ground_at(4.0, 4.0, 20.0).is_some());
    assert!(walk.ground_at(20.0, 4.0, 20.0).is_some());
    assert!(walk.read(12.0, 4.0, 0.0).ready, "a hole is settled ground");
}

fn raised(x: f64, z: f64) -> f32 {
    ground(x, z) + 3.0
}

#[test]
fn invalidating_resamples_and_the_old_ground_stands_until_the_new_lands() {
    let mut walk = Walk::new(ground, origin_anchor());
    walk.arrive(4.0, 4.0);
    let before = walk.ground_at(4.0, 4.0, 20.0).unwrap();
    walk.bridge
        .invalidate(NativeHeightfieldColliderInvalidateRequest {
            collider: walk.collider,
            min_x: 3.0,
            min_z: 3.0,
            max_x: 5.0,
            max_z: 5.0,
        })
        .unwrap();
    assert_eq!(walk.ground_at(4.0, 4.0, 20.0), Some(before));
    let requests = walk.requests();
    assert_eq!(requests.len(), 1);
    let mut stale = requests[0];
    stale.revision -= 1;
    assert!(!walk.commit(&stale, raised), "a stale revision is ignored");
    assert!(walk.commit(&requests[0], raised));
    walk.bridge.session_mut(walk.session).unwrap().settle();
    let after = walk.ground_at(4.0, 4.0, 20.0).unwrap();
    assert!((after - before - 3.0).abs() < 1e-3, "{before} then {after}");
}

#[test]
fn a_rebase_moves_the_ground_with_the_world_and_new_tiles_land_in_place() {
    let anchor = NativeWorldOriginGlobalPosition {
        cell_x: 100,
        ..Default::default()
    };
    let mut walk = Walk::new(ground, anchor);
    walk.arrive(4.0, 4.0);
    let before = walk.ground_at(104.0, 4.0, 20.0).unwrap();
    let world_origin = crate::world_origin::api(&mut walk.bridge);
    let request = NativeWorldOriginPrepareRequest {
        session: walk.session,
        target_cell_x: 96,
        target_cell_y: 0,
        target_cell_z: 0,
        entities: std::ptr::null(),
        entities_len: 0,
        exclude_outside_envelope: false,
    };
    let mut prepared = NativeWorldOriginPreparedHandle::default();
    assert_eq!(
        unsafe {
            (world_origin.prepare)(
                world_origin.context,
                &request,
                &mut prepared,
                std::ptr::null_mut(),
            )
        },
        ABI_OK
    );
    let mut receipt = NativeWorldOriginCommitReceipt::default();
    assert_eq!(
        unsafe {
            (world_origin.commit)(
                world_origin.context,
                NativeWorldOriginCommitRequest { prepared },
                &mut receipt,
                std::ptr::null_mut(),
            )
        },
        ABI_OK
    );
    // The world's 104 now stands at 8 in the session's frame.
    assert_eq!(walk.ground_at(8.0, 4.0, 20.0), Some(before));
    // Nothing moved in the collider's own units: a new tile lands in place.
    walk.arrive(40.0, 4.0);
    let hit = walk.ground_at(44.0, 4.0, 20.0).unwrap();
    assert!((hit - f64::from(drawn(40.0, 4.0))).abs() < 1e-3);
}

#[test]
fn a_destroyed_collider_takes_its_ground() {
    let mut walk = Walk::new(ground, origin_anchor());
    walk.arrive(4.0, 4.0);
    walk.bridge.destroy_collider(walk.collider).unwrap();
    walk.bridge.session_mut(walk.session).unwrap().settle();
    assert!(walk.ground_at(4.0, 4.0, 20.0).is_none());
    assert!(walk
        .bridge
        .read(NativeHeightfieldColliderReadRequest {
            collider: walk.collider,
            ..Default::default()
        })
        .is_err());
}
