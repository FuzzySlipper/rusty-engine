//! Surface grids through the bridge: a route across a trench goes up a wall,
//! over the ceiling and down, and a crawler follows it; a deferred grid
//! installs at a later call; planar requests refuse a surface grid; the
//! explanations name a blocked corner.

use super::*;
use crate::operation_diagnostics::receipt_codes;
use engine_spatial::{step_crawler, CrawlerCommand, CrawlerConfig, CrawlerMotion};

const GRID: u64 = 5;

fn bridge_with(scene: VoxelCollisionScene) -> (RuntimeSpatialBridge, NativeSpatialSessionHandle) {
    let mut bridge = RuntimeSpatialBridge::new();
    let session = bridge
        .create(NativeSpatialSessionConfig {
            collision_voxel_size: 1.0,
            collision_chunk_size: 8,
            voxel_surface_mode: NativeVoxelSurfaceMode::GreedyCubes,
        })
        .unwrap();
    bridge.sessions.get_mut(&session.value).unwrap().scene = Arc::new(scene);
    (bridge, session)
}

fn v(x: f32, y: f32, z: f32) -> NativeVec3 {
    NativeVec3 { x, y, z }
}

/// A closed room of 1 m voxels, inside x and z from −3 to 3 and y from 1 to
/// 5, with a trench across x = −1..1 cut through the floor and both side
/// walls up to the ceiling, which alone spans it.
fn trench_room() -> VoxelCollisionScene {
    let mut voxels = Vec::new();
    for x in -4..4 {
        for y in 0..6 {
            for z in -4..4 {
                let inside = (-3..3).contains(&x) && (1..5).contains(&y) && (-3..3).contains(&z);
                let trench = (-1..1).contains(&x) && y < 5;
                if !inside && !trench {
                    voxels.push([x, y, z]);
                }
            }
        }
    }
    VoxelCollisionScene::from_solid_voxels(1.0, 8, voxels).unwrap()
}

fn request(
    session: NativeSpatialSessionHandle,
    deferred: bool,
) -> NativeSurfaceNavigationReplaceRequest {
    NativeSurfaceNavigationReplaceRequest {
        session,
        world_min: v(-3.0, 1.0, -3.0),
        world_max: v(3.0, 5.0, 3.0),
        config: NativeSurfaceNavigationConfig {
            grid_id: GRID,
            ..default_native_config()
        },
        deferred,
    }
}

fn path(
    bridge: &mut RuntimeSpatialBridge,
    session: NativeSpatialSessionHandle,
    from: NativeVec3,
    to: NativeVec3,
) -> (
    NativeSurfaceNavigationPathResult,
    Vec<NativeSurfaceWaypoint>,
) {
    let result = bridge
        .request_surface_navigation_path(NativeSurfaceNavigationPathRequest {
            session,
            grid_id: GRID,
            from,
            to,
            max_visited: 10_000,
        })
        .unwrap();
    let waypoints = if result.waypoints_len == 0 {
        Vec::new()
    } else {
        unsafe { std::slice::from_raw_parts(result.waypoints, result.waypoints_len) }.to_vec()
    };
    (result, waypoints)
}

/// The surfaces in order, each run once.
fn runs(surfaces: impl IntoIterator<Item = NativeCrawlerSurface>) -> Vec<NativeCrawlerSurface> {
    let mut runs = Vec::new();
    for surface in surfaces {
        if runs.last() != Some(&surface) {
            runs.push(surface);
        }
    }
    runs
}

const START: NativeVec3 = NativeVec3 {
    x: -2.5,
    y: 1.27,
    z: 0.5,
};
const GOAL: NativeVec3 = NativeVec3 {
    x: 2.5,
    y: 1.27,
    z: 0.5,
};

#[test]
fn a_route_across_a_trench_goes_up_a_wall_over_the_ceiling_and_down_and_a_crawler_follows_it() {
    use NativeCrawlerSurface::{Ceiling, Floor, Wall};
    let (mut bridge, session) = bridge_with(trench_room());
    let receipt = bridge
        .replace_surface_navigation(&request(session, false))
        .unwrap();
    assert!(!receipt.pending && receipt.surface_cell_count > 0);
    assert_eq!(receipt.component_count, 1);
    let (result, waypoints) = path(&mut bridge, session, START, GOAL);
    assert_eq!(result.outcome, NativeSurfaceNavigationPathOutcome::Reached);
    assert_eq!(
        runs(waypoints.iter().map(|waypoint| waypoint.surface)),
        [Floor, Wall, Ceiling, Wall, Floor]
    );
    let costs: u64 = waypoints[1..]
        .iter()
        .map(|waypoint| match waypoint.surface {
            Floor => 1,
            Wall => 2,
            _ => 3,
        })
        .sum();
    assert_eq!(result.total_cost, costs);
    assert!(waypoints
        .iter()
        .any(|waypoint| waypoint.edge == NativeSurfaceEdgeKind::Concave));
    let goal = waypoints.last().unwrap();
    assert_eq!((goal.cell.x, goal.cell.y, goal.cell.z), (2, 1, 0));
    assert_eq!(result.nearest.cell, goal.cell);

    // Phase 1's movement, steered at each waypoint in turn, gets there.
    let scene = Arc::clone(&bridge.sessions[&session.value].scene);
    let config = CrawlerConfig::default();
    let at = |value: NativeVec3| Vec3::new(value.x, value.y, value.z);
    let mut position = at(waypoints[0].position);
    let mut motion = CrawlerMotion::default();
    let mut surfaces = Vec::new();
    let mut next = 1;
    for sequence in 1..=3_000 {
        if next == waypoints.len() {
            break;
        }
        let target = at(waypoints[next].position);
        let toward = target - position;
        let arrived = if next + 1 == waypoints.len() {
            0.1
        } else {
            0.35
        };
        if toward.length() < arrived {
            next += 1;
            continue;
        }
        // A waypoint on another surface facing back toward the crawler is
        // round a concave corner, and may lie straight along the normal of
        // the surface it is on: lean into the waypoint's surface.
        let mut direction = toward * (1.0 / toward.length());
        let normal = at(waypoints[next].normal);
        if normal.dot(motion.up) < 0.9 && normal.dot(position - target) >= -1.0e-3 {
            direction = direction - normal;
        }
        let receipt = step_crawler(
            &scene,
            position,
            motion,
            &config,
            CrawlerCommand {
                direction,
                cling: true,
                step_seconds: 1.0 / 60.0,
                sequence,
            },
        )
        .unwrap();
        position = receipt.position;
        motion = receipt.motion;
        surfaces.push(super::super::crawler::native_surface(receipt.surface));
    }
    assert_eq!(next, waypoints.len(), "followed to {position:?}");
    assert_eq!(
        runs(
            surfaces
                .into_iter()
                .filter(|&surface| surface != NativeCrawlerSurface::None)
        ),
        [Floor, Wall, Ceiling, Wall, Floor]
    );
}

#[test]
fn snapping_misses_and_an_unpublished_grid_are_outcomes() {
    let (mut bridge, session) = bridge_with(trench_room());
    let (result, _) = path(&mut bridge, session, START, GOAL);
    assert_eq!(
        result.outcome,
        NativeSurfaceNavigationPathOutcome::Unavailable
    );
    bridge
        .replace_surface_navigation(&request(session, false))
        .unwrap();
    // In the middle of the room, more than a cell from any resting ball.
    let (result, _) = path(&mut bridge, session, v(0.5, 2.9, 0.5), GOAL);
    assert_eq!(
        result.outcome,
        NativeSurfaceNavigationPathOutcome::StartOffSurface
    );
    let (result, _) = path(&mut bridge, session, START, v(-0.5, -6.0, 0.5));
    assert_eq!(
        result.outcome,
        NativeSurfaceNavigationPathOutcome::GoalOffSurface
    );
}

#[test]
fn a_deferred_grid_installs_at_a_later_call_and_planar_requests_refuse_it() {
    let (mut bridge, session) = bridge_with(trench_room());
    let pending = bridge
        .replace_surface_navigation(&request(session, true))
        .unwrap();
    assert!(pending.pending);
    let read = |bridge: &mut RuntimeSpatialBridge| {
        bridge
            .read_surface_navigation(NativeSurfaceNavigationReadRequest {
                session,
                grid_id: GRID,
            })
            .unwrap()
    };
    let started = Instant::now();
    while read(&mut bridge).pending {
        assert!(started.elapsed().as_secs() < 60, "never installed");
        bridge.settle_deferred_residency();
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let readout = read(&mut bridge);
    assert!(!readout.refused && readout.installed.surface_cell_count > 0);
    let synchronous = {
        let (mut other, other_session) = bridge_with(trench_room());
        other
            .replace_surface_navigation(&request(other_session, false))
            .unwrap()
    };
    assert_eq!(
        (
            readout.installed.surface_cell_count,
            readout.installed.edge_count
        ),
        (synchronous.surface_cell_count, synchronous.edge_count)
    );
    let (result, _) = path(&mut bridge, session, START, GOAL);
    assert_eq!(result.outcome, NativeSurfaceNavigationPathOutcome::Reached);

    // The grid is listed as a surface grid beside a planar one.
    let mut planar = super::super::collision_navigation::flat_config(1.0, 1, 0.3, 1.6, 45.0);
    planar.maximum_cells = 1 << 16;
    let planar_request = |grid_id| NativeCollisionNavigationReplaceRequest {
        session,
        world_min: v(-3.0, 0.0, -3.0),
        world_max: v(3.0, 6.0, 3.0),
        config: NativeCollisionNavigationConfig { grid_id, ..planar },
        deferred: false,
    };
    bridge
        .replace_collision_navigation(&planar_request(7))
        .unwrap();
    let grids = bridge
        .read_navigation_grids(NativeNavigationGridsRequest { session })
        .unwrap();
    let grids = unsafe { std::slice::from_raw_parts(grids.grids, grids.grids_len) }.to_vec();
    let kinds: Vec<_> = grids
        .iter()
        .map(|grid| (grid.grid_id, grid.kind, grid.latest))
        .collect();
    assert_eq!(
        kinds,
        [
            (
                GRID,
                NativeNavigationProjectionKind::CollisionSurface,
                false
            ),
            (7, NativeNavigationProjectionKind::CollisionDerived, true),
        ]
    );
    assert_eq!(
        grids[0].walkable_cell_count,
        readout.installed.surface_cell_count
    );

    // A planar request naming it is refused; one naming no grid reads the
    // planar grid.
    let codes = |error: CsharpEngineServicesError| {
        let mut receipt = unsafe { std::mem::zeroed::<NativeOperationErrorReceipt>() };
        refuse(&error, &mut receipt);
        receipt_codes(&receipt)
    };
    let planar_path = |bridge: &mut RuntimeSpatialBridge, grid_id| {
        bridge.request_navigation_path(NativeNavigationPathRequest {
            session,
            start: NativePlanarNavCell { x: -3, y: 1, z: 0 },
            goal: NativePlanarNavCell { x: -2, y: 1, z: 0 },
            max_visited: 100,
            grid_id,
        })
    };
    let refused = planar_path(&mut bridge, GRID).unwrap_err();
    assert_eq!(codes(refused), ["navigation-grid-is-surface"]);
    assert!(planar_path(&mut bridge, 0).is_ok());
    let refused = bridge
        .replace_collision_navigation(&planar_request(GRID))
        .unwrap_err();
    assert_eq!(codes(refused), ["navigation-grid-is-surface"]);
    let mut taken = request(session, false);
    taken.config.grid_id = 7;
    let refused = bridge.replace_surface_navigation(&taken).unwrap_err();
    assert_eq!(codes(refused), ["surface-navigation-grid-in-use"]);

    // Clearing it by its id leaves the planar grid.
    bridge
        .clear_navigation(NativeNavigationClearRequest {
            session,
            grid_id: GRID,
        })
        .unwrap();
    let (result, _) = path(&mut bridge, session, START, GOAL);
    assert_eq!(
        result.outcome,
        NativeSurfaceNavigationPathOutcome::Unavailable
    );
    assert!(planar_path(&mut bridge, 7).is_ok());
}

/// A step a metre high for x ≥ 0 on a floor at y = 1, of half-metre voxels,
/// and with `bar` a bar of voxels half a metre over its lip.
fn step(bar: bool) -> VoxelCollisionScene {
    let mut voxels = Vec::new();
    for x in -8..8 {
        for z in -4..4 {
            voxels.push([x, 1, z]);
            if x >= 0 {
                voxels.push([x, 2, z]);
                voxels.push([x, 3, z]);
            }
            if bar && x == -1 {
                voxels.push([x, 5, z]);
            }
        }
    }
    VoxelCollisionScene::from_solid_voxels(0.5, 8, voxels).unwrap()
}

#[test]
fn the_explanations_name_a_surface_cell_and_a_blocked_corner() {
    let top = NativeSurfaceCell {
        x: 0,
        y: 2,
        z: 0,
        face: NativeSurfaceFace::PositiveY,
    };
    let side = NativeSurfaceCell {
        x: -1,
        y: 1,
        z: 0,
        face: NativeSurfaceFace::NegativeX,
    };
    for bar in [false, true] {
        let (mut bridge, session) = bridge_with(step(bar));
        let mut publish = request(session, false);
        publish.world_min = v(-3.0, 0.0, -1.0);
        publish.world_max = v(3.0, 4.0, 2.0);
        bridge.replace_surface_navigation(&publish).unwrap();
        let explain_cell = |bridge: &mut RuntimeSpatialBridge, cell| {
            bridge
                .explain_surface_navigation_cell(&NativeSurfaceNavigationCellRequest {
                    session,
                    grid_id: GRID,
                    cell,
                })
                .unwrap()
        };
        let readout = explain_cell(&mut bridge, top);
        assert_eq!(readout.outcome, NativeSurfaceCellOutcome::Surface);
        assert!(readout.admitted && readout.surface_contact.present);
        assert_eq!(readout.surface, NativeCrawlerSurface::Floor);
        assert!((readout.position.y - 2.27).abs() < 1.0e-5);
        assert!(matches!(
            readout.surface_contact.source_kind,
            NativeCharacterCollisionSourceKind::VoxelChunk
        ));
        let air = explain_cell(
            &mut bridge,
            NativeSurfaceCell {
                x: -3,
                y: 3,
                z: 0,
                face: NativeSurfaceFace::PositiveY,
            },
        );
        assert_eq!(air.outcome, NativeSurfaceCellOutcome::NoSurface);
        assert!(!air.admitted);

        let edge = bridge
            .explain_surface_navigation_edge(&NativeSurfaceNavigationEdgeRequest {
                session,
                grid_id: GRID,
                from: top,
                to: side,
            })
            .unwrap();
        assert_eq!(edge.kind, NativeSurfaceEdgeKind::Convex);
        assert!(edge.corner_present);
        assert!(edge.corner.x.abs() < 1.0e-5 && (edge.corner.y - 2.0).abs() < 1.0e-5);
        assert_eq!(edge.cost, DEFAULT_WALL_COST);
        if bar {
            assert_eq!(edge.outcome, NativeSurfaceEdgeOutcome::Blocked);
            assert!(!edge.admitted && edge.blocker.present);
            assert!(edge.blocker.point.y >= 2.5 - 1.0e-5);
        } else {
            assert_eq!(edge.outcome, NativeSurfaceEdgeOutcome::Traversable);
            assert!(edge.admitted);
        }
        let far = bridge
            .explain_surface_navigation_edge(&NativeSurfaceNavigationEdgeRequest {
                session,
                grid_id: GRID,
                from: top,
                to: NativeSurfaceCell { x: 2, ..top },
            })
            .unwrap();
        assert_eq!(far.outcome, NativeSurfaceEdgeOutcome::NotNeighbor);
    }
}

#[test]
fn a_bad_configuration_or_region_is_refused_with_its_code() {
    let (mut bridge, session) = bridge_with(trench_room());
    let codes = |request: NativeSurfaceNavigationReplaceRequest,
                 bridge: &mut RuntimeSpatialBridge| {
        let error = bridge.replace_surface_navigation(&request).unwrap_err();
        let mut receipt = unsafe { std::mem::zeroed::<NativeOperationErrorReceipt>() };
        refuse(&error, &mut receipt);
        receipt_codes(&receipt)
    };
    let mut zero = request(session, false);
    zero.config.grid_id = 0;
    let mut small = request(session, false);
    small.config.cell_size = 0.4;
    let mut costless = request(session, false);
    costless.config.wall_cost = 0;
    let mut unbounded = request(session, false);
    unbounded.world_max.x = f32::INFINITY;
    let mut large = request(session, false);
    large.config.maximum_cells = 10;
    for (request, code) in [
        (zero, "invalid-surface-navigation-config"),
        (small, "invalid-surface-navigation-config"),
        (costless, "invalid-surface-navigation-config"),
        (unbounded, "invalid-surface-navigation-region"),
        (large, "surface-navigation-over-budget"),
    ] {
        assert_eq!(codes(request, &mut bridge), [code]);
    }
}

/// Ruins on rolling ground in 1 m voxels across a CraftSurvive creature's
/// box (64 by 64 columns, 16 below and above its feet): rooms with walls
/// three metres high, doorways, half their roofs, and pillars.
fn ruins(mode: NativeVoxelSurfaceMode) -> VoxelCollisionScene {
    let mut voxels = Vec::new();
    for x in -36..36_i64 {
        for z in -36..36_i64 {
            let ground = ((x * 7 + z * 3).rem_euclid(13) / 5) - 1;
            for y in -18..=ground {
                voxels.push([x, y, z]);
            }
            let (room_x, room_z) = (x.rem_euclid(12), z.rem_euclid(12));
            let wall = (room_x == 0 || room_z == 0) && !(room_x == 6 || room_z == 6);
            let pillar = room_x == 9 && room_z == 3;
            if wall || pillar {
                for y in ground + 1..=ground + 3 {
                    voxels.push([x, y, z]);
                }
            }
            let roofed = (x.div_euclid(12) + z.div_euclid(12)).rem_euclid(2) == 0;
            if roofed && room_x != 0 && room_z != 0 {
                voxels.push([x, ground + 4, z]);
            }
        }
    }
    match mode {
        NativeVoxelSurfaceMode::DualContouring => {
            let options = engine_spatial::SurfaceMeshOptions {
                mode: engine_spatial::SurfaceMode::DualContouring,
                ..engine_spatial::SurfaceMeshOptions::default()
            };
            VoxelCollisionScene::from_solid_voxels_with_mesh_options(1.0, 16, voxels, options)
                .unwrap()
        }
        _ => VoxelCollisionScene::from_solid_voxels(1.0, 16, voxels).unwrap(),
    }
}

#[test]
#[ignore = "measurement: cargo test --release -p csharp-engine-services measure_surface_derivation_cost -- --ignored --nocapture"]
fn measure_surface_derivation_cost() {
    for mode in [
        NativeVoxelSurfaceMode::GreedyCubes,
        NativeVoxelSurfaceMode::DualContouring,
    ] {
        let scene = ruins(mode);
        let publish = |deferred: bool| {
            let (mut bridge, session) = bridge_with(scene.clone());
            let mut request = request(session, deferred);
            request.world_min = v(-32.0, -16.0, -32.0);
            request.world_max = v(32.0, 16.0, 32.0);
            let started = Instant::now();
            let mut receipt = bridge.replace_surface_navigation(&request).unwrap();
            if deferred {
                loop {
                    bridge.settle_deferred_residency();
                    let readout = bridge
                        .read_surface_navigation(NativeSurfaceNavigationReadRequest {
                            session,
                            grid_id: GRID,
                        })
                        .unwrap();
                    if !readout.pending {
                        receipt = readout.installed;
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_micros(200));
                }
            }
            let elapsed = started.elapsed().as_secs_f64() * 1000.0;
            // How many places the largest component holds.
            let grid = &bridge.sessions[&session.value].surface_grids[&GRID];
            let mut sizes = vec![0_usize; grid.graph.component_count() as usize];
            for slot in 0..grid.graph.len() as u32 {
                sizes[grid.graph.component(slot) as usize] += 1;
            }
            let largest = sizes.iter().copied().max().unwrap_or(0);
            (elapsed, receipt, largest)
        };
        for deferred in [false, true] {
            let runs: Vec<_> = (0..5).map(|_| publish(deferred)).collect();
            let mut times: Vec<f64> = runs.iter().map(|(ms, _, _)| *ms).collect();
            times.sort_by(f64::total_cmp);
            let (_, receipt, largest) = runs[0];
            println!(
                "{mode:?} {}: {:.1} ms median ({:.1}..{:.1}), {} places, {} edges, {} edge tests, {} components, the largest {largest} places",
                if deferred { "worker" } else { "call" },
                times[2],
                times[0],
                times[4],
                receipt.surface_cell_count,
                receipt.edge_count,
                receipt.edge_test_count,
                receipt.component_count,
            );
        }
    }
}
