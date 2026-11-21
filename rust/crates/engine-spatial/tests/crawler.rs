//! Surface crawling (#10967): a ball-bodied crawler goes up walls, across
//! ceilings and round edges on cube, dual-contoured and static-mesh
//! collision, drops off a ceiling on request and is lost when its surface is
//! cut away.

use core_math::Vec3;
use engine_spatial::{
    step_crawler, CharacterCollisionSource, CrawlerCommand, CrawlerConfig, CrawlerMotion,
    CrawlerStepReceipt, CrawlerSurface, CrawlerTransition, MaterialSurface, MaterialVoxel,
    StaticMeshAssetId, StaticMeshColliderAsset, StaticMeshColliderInstance, StaticMeshInstanceId,
    StaticMeshTransform, SurfaceCharacter, SurfaceMaterials, SurfaceMeshOptions, SurfaceMode,
    VertexPlacement, VoxelCollisionScene, VoxelEdit, VoxelEditService,
};

const STEP: f32 = 1.0 / 60.0;

/// A closed room of 1 m voxels: inside x and z from −3 to 3, y from 1 to 5.
fn room_voxels() -> Vec<[i64; 3]> {
    let mut voxels = Vec::new();
    for x in -4..=3 {
        for y in 0..=5 {
            for z in -4..=3 {
                let shell = y == 0 || y == 5 || x == -4 || x == 3 || z == -4 || z == 3;
                if shell {
                    voxels.push([x, y, z]);
                }
            }
        }
    }
    voxels
}

fn cube_room() -> VoxelCollisionScene {
    VoxelCollisionScene::from_solid_voxels(1.0, 8, room_voxels()).unwrap()
}

fn dual_contoured(voxels: Vec<[i64; 3]>, placement: VertexPlacement) -> VoxelCollisionScene {
    let options = SurfaceMeshOptions {
        mode: SurfaceMode::DualContouring,
        materials: SurfaceMaterials::new([(
            1,
            MaterialSurface {
                mode: SurfaceMode::DualContouring,
                character: SurfaceCharacter {
                    placement,
                    crease_angle_degrees: 40.0,
                    roughness: 0.0,
                },
            },
        )])
        .unwrap(),
        ..SurfaceMeshOptions::default()
    };
    VoxelCollisionScene::from_material_voxels_with_mesh_options(
        1.0,
        8,
        voxels.into_iter().map(|address| MaterialVoxel {
            state: 0,
            address,
            material_slot: 1,
        }),
        options,
    )
    .unwrap()
}

fn dual_contoured_room() -> VoxelCollisionScene {
    dual_contoured(room_voxels(), VertexPlacement::Smooth)
}

struct Crawler {
    position: Vec3,
    motion: CrawlerMotion,
    sequence: u64,
    transitions: Vec<CrawlerTransition>,
    last: Option<CrawlerStepReceipt>,
}

impl Crawler {
    fn at(position: Vec3) -> Self {
        Self {
            position,
            motion: CrawlerMotion::default(),
            sequence: 0,
            transitions: Vec::new(),
            last: None,
        }
    }

    fn step(
        &mut self,
        scene: &VoxelCollisionScene,
        direction: Vec3,
        cling: bool,
    ) -> CrawlerStepReceipt {
        self.sequence += 1;
        let receipt = step_crawler(
            scene,
            self.position,
            self.motion,
            &CrawlerConfig::default(),
            CrawlerCommand {
                direction,
                cling,
                step_seconds: STEP,
                sequence: self.sequence,
            },
        )
        .unwrap();
        self.position = receipt.position;
        self.motion = receipt.motion;
        if receipt.transition != CrawlerTransition::None {
            self.transitions.push(receipt.transition);
        }
        self.last = Some(receipt);
        receipt
    }

    /// Drives until `done` holds, within `steps` steps.
    fn drive(
        &mut self,
        scene: &VoxelCollisionScene,
        direction: Vec3,
        cling: bool,
        steps: usize,
        what: &str,
        done: impl Fn(&CrawlerStepReceipt) -> bool,
    ) -> CrawlerStepReceipt {
        for _ in 0..steps {
            let receipt = self.step(scene, direction, cling);
            if done(&receipt) {
                return receipt;
            }
        }
        panic!(
            "{what}: not done after {steps} steps; last {:?}, transitions {:?}",
            self.last, self.transitions
        );
    }

    fn settle(&mut self, scene: &VoxelCollisionScene) -> CrawlerStepReceipt {
        self.drive(scene, Vec3::ZERO, true, 120, "settle", |r| {
            r.motion.attached && r.motion.velocity.length() < 1.0e-3
        })
    }
}

fn close(a: Vec3, b: Vec3, tolerance: f32) -> bool {
    (a - b).length() <= tolerance
}

fn on(receipt: &CrawlerStepReceipt, surface: CrawlerSurface, up: Vec3, tolerance: f32) -> bool {
    receipt.surface == surface && close(receipt.motion.up, up, tolerance)
}

/// The rotation turns +Y to up and +Z to forward.
fn rotated(receipt: &CrawlerStepReceipt, v: Vec3) -> Vec3 {
    let q = receipt.rotation;
    let u = Vec3::new(q.x, q.y, q.z);
    let t = u.cross(v) * 2.0;
    v + t * q.w + u.cross(t)
}

const X: Vec3 = Vec3::new(1.0, 0.0, 0.0);
const Y: Vec3 = Vec3::new(0.0, 1.0, 0.0);

fn neg(v: Vec3) -> Vec3 {
    v * -1.0
}

/// Floor → +X wall → ceiling → −X wall → floor, the receipt naming each
/// surface with its normal and the rotation drawing the body on it.
fn round_the_room(scene: &VoxelCollisionScene, tolerance: f32) {
    let mut crawler = Crawler::at(Vec3::new(0.0, 1.5, 0.5));
    let landed = crawler.settle(scene);
    assert!(
        on(&landed, CrawlerSurface::Floor, Y, tolerance),
        "{landed:?}"
    );
    assert!(
        (landed.position.y - 1.27).abs() < 0.02,
        "rests its skin off the floor: {landed:?}"
    );

    let wall = crawler.drive(scene, X, true, 240, "up the +X wall", |r| {
        on(r, CrawlerSurface::Wall, neg(X), tolerance)
    });
    assert!(crawler.transitions.contains(&CrawlerTransition::Concave));
    assert!(close(rotated(&wall, Y), neg(X), tolerance), "{wall:?}");
    assert!(wall.position.x > 2.6, "{wall:?}");

    let ceiling = crawler.drive(scene, Y, true, 400, "onto the ceiling", |r| {
        on(r, CrawlerSurface::Ceiling, neg(Y), tolerance)
    });
    assert!(
        close(rotated(&ceiling, Y), neg(Y), tolerance),
        "{ceiling:?}"
    );
    assert!(ceiling.position.y > 4.6, "{ceiling:?}");

    // Across the ceiling and down the far wall.
    let across = crawler.drive(scene, neg(X), true, 400, "across the ceiling", |r| {
        on(r, CrawlerSurface::Wall, X, tolerance)
    });
    assert!(across.position.x < -2.6, "{across:?}");
    let floor = crawler.drive(scene, neg(Y), true, 400, "down to the floor", |r| {
        on(r, CrawlerSurface::Floor, Y, tolerance)
    });
    assert!(floor.position.y < 1.4, "{floor:?}");
    assert!(
        matches!(
            floor.contact.unwrap().source,
            CharacterCollisionSource::VoxelChunk(_)
        ),
        "{floor:?}"
    );
}

#[test]
fn a_crawler_goes_round_a_cube_room_floor_wall_ceiling_wall_floor() {
    round_the_room(&cube_room(), 1.0e-3);
}

#[test]
fn a_crawler_goes_round_a_dual_contoured_room() {
    round_the_room(&dual_contoured_room(), 0.05);
}

#[test]
fn a_crawler_drops_off_the_ceiling_on_request_and_lands_on_the_floor() {
    let scene = cube_room();
    let mut crawler = Crawler::at(Vec3::new(0.0, 1.5, 0.5));
    crawler.settle(&scene);
    crawler.drive(&scene, X, true, 240, "wall", |r| {
        r.surface == CrawlerSurface::Wall
    });
    crawler.drive(&scene, Y, true, 400, "ceiling", |r| {
        r.surface == CrawlerSurface::Ceiling
    });
    for _ in 0..20 {
        crawler.step(&scene, neg(X), true);
    }
    let held = crawler.step(&scene, Vec3::ZERO, true);
    assert_eq!(held.surface, CrawlerSurface::Ceiling, "{held:?}");
    let let_go = crawler.step(&scene, Vec3::ZERO, false);
    assert_eq!(let_go.transition, CrawlerTransition::Detached, "{let_go:?}");
    assert_eq!(let_go.surface, CrawlerSurface::None, "{let_go:?}");
    assert!(!let_go.motion.attached);
    let landed = crawler.drive(&scene, Vec3::ZERO, false, 120, "land", |r| {
        r.transition == CrawlerTransition::Landed
    });
    assert!(on(&landed, CrawlerSurface::Floor, Y, 1.0e-3), "{landed:?}");
    assert!((landed.position.y - 1.27).abs() < 0.02, "{landed:?}");
    // Without cling it does not climb: a wall stops it.
    let blocked = crawler.drive(&scene, X, false, 240, "to the wall", |r| {
        r.position.x > 2.7 && r.motion.velocity.length() < 0.1
    });
    assert_eq!(blocked.surface, CrawlerSurface::Floor, "{blocked:?}");
}

/// A block 2 m high and 4 m across: x and z from −2 to 2.
fn block_voxels() -> Vec<[i64; 3]> {
    let mut voxels = room_voxels();
    for x in -2..=1 {
        for y in 1..=2 {
            for z in -2..=1 {
                voxels.push([x, y, z]);
            }
        }
    }
    voxels
}

/// Over a 2 m block's top lip, down its side, back onto the floor.
fn over_the_block(scene: &VoxelCollisionScene, tolerance: f32, exact: bool) {
    let mut crawler = Crawler::at(Vec3::new(0.0, 3.5, 0.5));
    let top = crawler.settle(scene);
    assert!(on(&top, CrawlerSurface::Floor, Y, tolerance), "{top:?}");
    assert!(top.position.y > 3.2, "{top:?}");
    let side = crawler.drive(scene, X, true, 120, "over the lip", |r| {
        on(r, CrawlerSurface::Wall, X, tolerance)
    });
    assert!(
        crawler.transitions.contains(&CrawlerTransition::Convex),
        "{:?}",
        crawler.transitions
    );
    if exact {
        assert!(
            (side.position.x - 2.27).abs() < 0.02,
            "rests its skin off the side: {side:?}"
        );
    }
    let floor = crawler.drive(scene, neg(Y), true, 240, "down the side", |r| {
        on(r, CrawlerSurface::Floor, Y, tolerance)
    });
    assert!(
        floor.position.x > 2.0 && floor.position.y < 1.5,
        "{floor:?}"
    );
}

#[test]
fn a_crawler_rounds_a_convex_lip_down_a_block_side() {
    over_the_block(
        &VoxelCollisionScene::from_solid_voxels(1.0, 8, block_voxels()).unwrap(),
        1.0e-3,
        true,
    );
}

#[test]
fn a_crawler_rounds_a_dual_contoured_block() {
    // A dual-contoured riser leans a little off vertical.
    over_the_block(
        &dual_contoured(block_voxels(), VertexPlacement::Sharp),
        0.25,
        false,
    );
}

/// A slab hanging 2 m over the floor: over its top's lip, down its edge and
/// round the bottom lip onto its underside.
#[test]
fn a_crawler_rounds_two_lips_onto_a_slab_underside() {
    let mut voxels: Vec<[i64; 3]> = (-8..8)
        .flat_map(|x| (-8..8).map(move |z| [x, 0, z]))
        .collect();
    for x in -2..=1 {
        for z in -2..=1 {
            voxels.push([x, 3, z]);
        }
    }
    let scene = VoxelCollisionScene::from_solid_voxels(1.0, 8, voxels).unwrap();
    let mut crawler = Crawler::at(Vec3::new(0.0, 4.5, 0.0));
    crawler.settle(&scene);
    crawler.drive(&scene, X, true, 120, "over the top lip", |r| {
        on(r, CrawlerSurface::Wall, X, 1.0e-3)
    });
    let under_and_back = Vec3::new(-1.0, -1.0, 0.0);
    let under = crawler.drive(
        &scene,
        under_and_back,
        true,
        240,
        "under the bottom lip",
        |r| on(r, CrawlerSurface::Ceiling, neg(Y), 1.0e-3) && r.position.x < 1.0,
    );
    assert!(
        (under.position.y - 2.73).abs() < 0.02,
        "hangs its skin under the slab: {under:?}"
    );
    let convex = crawler
        .transitions
        .iter()
        .filter(|t| **t == CrawlerTransition::Convex)
        .count();
    assert!(convex >= 2, "{:?}", crawler.transitions);
}

/// A static-mesh box on a voxel floor: up its side and over onto its top.
#[test]
fn a_crawler_climbs_a_static_mesh_box() {
    let mut scene = VoxelCollisionScene::from_solid_voxels(
        1.0,
        8,
        (-8..8).flat_map(|x| (-8..8).map(move |z| [x, 0, z])),
    )
    .unwrap();
    // A 2 m cube standing on the floor between x 1..3, z −1..1.
    let (lo, hi) = ([1.0, 1.0, -1.0], [3.0, 3.0, 1.0]);
    let corner = |i: usize| {
        [
            if i & 1 == 0 { lo[0] } else { hi[0] },
            if i & 2 == 0 { lo[1] } else { hi[1] },
            if i & 4 == 0 { lo[2] } else { hi[2] },
        ]
    };
    let positions: Vec<[f64; 3]> = (0..8).map(corner).collect();
    let triangles = vec![
        [0, 2, 1],
        [1, 2, 3], // −z
        [4, 5, 6],
        [5, 7, 6], // +z
        [0, 1, 4],
        [1, 5, 4], // −y
        [2, 6, 3],
        [3, 6, 7], // +y
        [0, 4, 2],
        [2, 4, 6], // −x
        [1, 3, 5],
        [3, 7, 5], // +x
    ];
    let asset = StaticMeshColliderAsset::new(StaticMeshAssetId(7), positions, triangles).unwrap();
    scene
        .replace_static_mesh_colliders(
            [asset],
            [StaticMeshColliderInstance {
                id: StaticMeshInstanceId(7),
                asset: StaticMeshAssetId(7),
                transform: StaticMeshTransform::IDENTITY,
            }],
        )
        .unwrap();
    let mut crawler = Crawler::at(Vec3::new(-1.0, 1.5, 0.0));
    crawler.settle(&scene);
    let side = crawler.drive(&scene, X, true, 240, "up the box side", |r| {
        on(r, CrawlerSurface::Wall, neg(X), 1.0e-3)
    });
    assert!(
        matches!(
            side.contact.unwrap().source,
            CharacterCollisionSource::StaticMesh { .. }
        ),
        "{side:?}"
    );
    let top = crawler.drive(&scene, Y, true, 240, "over onto the top", |r| {
        on(r, CrawlerSurface::Floor, Y, 1.0e-3) && r.position.y > 3.0
    });
    assert!(
        crawler.transitions.contains(&CrawlerTransition::Convex),
        "{:?}",
        crawler.transitions
    );
    assert!((top.position.y - 3.27).abs() < 0.02, "{top:?}");
}

/// Cutting away the wall a crawler holds leaves it lost; it falls and lands.
#[test]
fn a_crawler_is_lost_when_its_wall_is_cut_away() {
    let mut scene = cube_room();
    let mut crawler = Crawler::at(Vec3::new(0.0, 1.5, 0.5));
    crawler.settle(&scene);
    crawler.drive(&scene, X, true, 240, "wall", |r| {
        r.surface == CrawlerSurface::Wall
    });
    let high = crawler.drive(&scene, Y, true, 120, "up the wall", |r| r.position.y > 3.0);
    assert_eq!(high.surface, CrawlerSurface::Wall);
    let mut edits = Vec::new();
    for y in 1..=4 {
        for z in -1..=1 {
            edits.push(VoxelEdit::Clear { address: [3, y, z] });
        }
    }
    VoxelEditService::apply(&mut scene, &edits).unwrap();
    let lost = crawler.step(&scene, Vec3::ZERO, true);
    assert_eq!(lost.transition, CrawlerTransition::Lost, "{lost:?}");
    assert_eq!(lost.surface, CrawlerSurface::None);
    let landed = crawler.drive(&scene, Vec3::ZERO, true, 120, "fall", |r| {
        r.transition == CrawlerTransition::Landed
    });
    assert_eq!(landed.surface, CrawlerSurface::Floor, "{landed:?}");
}

#[test]
fn a_bad_config_command_or_motion_is_refused() {
    let scene = cube_room();
    let command = CrawlerCommand {
        direction: Vec3::ZERO,
        cling: true,
        step_seconds: STEP,
        sequence: 1,
    };
    let at = Vec3::new(0.0, 1.27, 0.0);
    let mut config = CrawlerConfig::default();
    config.contact_skin = config.radius;
    assert_eq!(
        step_crawler(&scene, at, CrawlerMotion::default(), &config, command)
            .unwrap_err()
            .code(),
        "invalid-crawler-config"
    );
    let config = CrawlerConfig::default();
    let long = CrawlerCommand {
        step_seconds: 2.0,
        ..command
    };
    assert_eq!(
        step_crawler(&scene, at, CrawlerMotion::default(), &config, long)
            .unwrap_err()
            .code(),
        "invalid-crawler-command"
    );
    let flat = CrawlerMotion {
        up: Vec3::ZERO,
        ..CrawlerMotion::default()
    };
    assert_eq!(
        step_crawler(&scene, at, flat, &config, command)
            .unwrap_err()
            .code(),
        "invalid-crawler-motion"
    );
}

/// Per-crawler cost of a 60 Hz step going round the room. Run with
/// `cargo test --release -p engine-spatial --test crawler -- --ignored --nocapture`.
#[test]
#[ignore]
fn measure_crawler_step_cost() {
    for (name, scene) in [
        ("cube", cube_room()),
        ("dual-contoured", dual_contoured_room()),
    ] {
        let mut steps = 0_u32;
        let started = std::time::Instant::now();
        for _ in 0..20 {
            let mut crawler = Crawler::at(Vec3::new(0.0, 1.5, 0.5));
            for (direction, count) in [
                (X, 120),
                (Y, 120),
                (neg(X), 200),
                (neg(Y), 120),
                (Vec3::new(0.0, 0.0, 1.0), 60),
            ] {
                for _ in 0..count {
                    crawler.step(&scene, direction, true);
                    steps += 1;
                }
            }
        }
        let micros = started.elapsed().as_secs_f64() * 1.0e6 / f64::from(steps);
        println!("{name}: {steps} steps, {micros:.2} us per 60 Hz crawler step");
    }
}
