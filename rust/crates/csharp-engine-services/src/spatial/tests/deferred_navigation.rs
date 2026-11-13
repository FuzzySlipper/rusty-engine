//! Collision-derived navigation derived off the update: the call returns at
//! once, the grid's previous publication answers until the first call after
//! the worker is done, and dropping the pending derivation cancels it.

use super::*;
use collision_navigation::flat_config;

const WIDTH: i64 = 64;
const GRID: u64 = 7;

/// Uneven ground with a wall across part of it.
fn terrain() -> Arc<VoxelCollisionScene> {
    let mut voxels = Vec::new();
    for x in -4..WIDTH + 4 {
        for z in -4..WIDTH + 4 {
            let height = 2 + ((x * 3 + z * 5).rem_euclid(11) / 3);
            let wall = (20..24).contains(&x) && (4..40).contains(&z);
            for y in 0..height + if wall { 4 } else { 0 } {
                voxels.push([x, y, z]);
            }
        }
    }
    Arc::new(VoxelCollisionScene::from_solid_voxels(1.0, 16, voxels).unwrap())
}

fn bridge_with(
    scene: Arc<VoxelCollisionScene>,
) -> (RuntimeSpatialBridge, NativeSpatialSessionHandle) {
    let mut bridge = RuntimeSpatialBridge::new();
    let session = bridge
        .create(NativeSpatialSessionConfig {
            collision_voxel_size: 1.0,
            collision_chunk_size: 16,
            voxel_surface_mode: NativeVoxelSurfaceMode::GreedyCubes,
        })
        .unwrap();
    bridge.sessions.get_mut(&session.value).unwrap().scene = scene;
    (bridge, session)
}

fn request(
    session: NativeSpatialSessionHandle,
    width: i64,
    deferred: bool,
) -> NativeCollisionNavigationReplaceRequest {
    let mut config = flat_config(1.0, 1, 0.3, 1.6, 45.0);
    config.maximum_cells = 1 << 20;
    NativeCollisionNavigationReplaceRequest {
        session,
        world_min: NativeVec3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        world_max: NativeVec3 {
            x: width as f32,
            y: 12.0,
            z: width as f32,
        },
        config,
        deferred,
    }
}

fn read(
    bridge: &mut RuntimeSpatialBridge,
    session: NativeSpatialSessionHandle,
) -> NativeCollisionNavigationReadout {
    bridge
        .read_collision_navigation(NativeCollisionNavigationReadRequest {
            session,
            grid_id: GRID,
        })
        .unwrap()
}

/// Begin calls until the grid's derivation is installed.
fn settle(
    bridge: &mut RuntimeSpatialBridge,
    session: NativeSpatialSessionHandle,
) -> NativeCollisionNavigationReadout {
    let started = Instant::now();
    loop {
        bridge.settle_deferred_residency();
        let readout = read(bridge, session);
        if !readout.pending {
            return readout;
        }
        assert!(
            started.elapsed().as_secs() < 60,
            "the derivation never finished"
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

/// A grid's hash, walkable cells and support heights.
type Held = (u64, usize, Vec<(VoxelCoord, u64)>);

/// What the installed grid holds, beyond revision numbers.
fn installed(bridge: &RuntimeSpatialBridge, session: NativeSpatialSessionHandle) -> Option<Held> {
    let navigation = bridge.sessions[&session.value].grid(GRID)?;
    let heights = navigation
        .vertical_mapping
        .as_ref()
        .unwrap()
        .support_heights
        .iter()
        .map(|(cell, height)| (*cell, height.to_bits()))
        .collect();
    Some((
        navigation.projection_hash(),
        navigation.projection.walkable_len(),
        heights,
    ))
}

/// The pending derivation's cancel flag.
fn cancel_flag(
    bridge: &RuntimeSpatialBridge,
    session: NativeSpatialSessionHandle,
) -> Arc<AtomicBool> {
    Arc::clone(&bridge.sessions[&session.value].derivations[&GRID]._cancel.0)
}

#[test]
fn a_deferred_grid_installs_what_a_synchronous_one_publishes_and_the_previous_answers_until_then() {
    let scene = terrain();
    let (mut synchronous, sync_session) = bridge_with(Arc::clone(&scene));
    let expected = synchronous
        .replace_collision_navigation(&request(sync_session, WIDTH, false))
        .unwrap();

    let (mut bridge, session) = bridge_with(scene);
    let previous = bridge
        .replace_collision_navigation(&request(session, 8, false))
        .unwrap();
    let before = installed(&bridge, session).unwrap();
    let receipt = bridge
        .replace_collision_navigation(&request(session, WIDTH, true))
        .unwrap();
    assert!(receipt.pending);
    assert_eq!(receipt.walkable_cell_count, 0);
    assert_eq!(receipt.navigation_revision, 0);

    // Until a call begins after the worker is done, the previous grid answers.
    let readout = read(&mut bridge, session);
    assert!(readout.pending);
    assert!(!readout.refused);
    assert_eq!(
        readout.installed.navigation_revision,
        previous.navigation_revision
    );
    assert_eq!(readout.installed.projection_hash, previous.projection_hash);
    while !bridge.sessions[&session.value].derivations[&GRID]
        .worker
        .is_finished()
    {
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(installed(&bridge, session).unwrap(), before);
    let spatial = api(&mut bridge);
    let mut column = NativeCollisionNavigationColumnResult::default();
    let mut refusal = crate::operation_diagnostics::empty_receipt();
    let status = unsafe {
        (spatial.explain_collision_navigation_column)(
            spatial.context,
            &NativeCollisionNavigationColumnRequest {
                session,
                x: 2,
                z: 2,
                grid_id: GRID,
            },
            &mut column,
            &mut refusal,
        )
    };
    assert_eq!(status, ABI_OK);
    assert_eq!(column.navigation_revision, previous.navigation_revision);

    let readout = settle(&mut bridge, session);
    assert!(!readout.refused);
    let receipt = readout.installed;
    assert!(!receipt.pending);
    assert_eq!(
        receipt.navigation_revision,
        bridge.sessions[&session.value].navigation_revision
    );
    assert!(receipt.navigation_revision > previous.navigation_revision);
    assert_eq!(
        (
            receipt.walkable_cell_count,
            receipt.projection_hash,
            receipt.derived_column_count,
            receipt.reused_column_count,
            receipt.edge_test_count,
            receipt.component_count,
        ),
        (
            expected.walkable_cell_count,
            expected.projection_hash,
            expected.derived_column_count,
            0,
            expected.edge_test_count,
            expected.component_count,
        )
    );
    assert!(receipt.derivation_microseconds > 0);
    assert_eq!(
        installed(&bridge, session),
        installed(&synchronous, sync_session)
    );

    // The installed grid's cache serves the next synchronous publication.
    let again = bridge
        .replace_collision_navigation(&request(session, WIDTH, false))
        .unwrap();
    assert_eq!(again.derived_column_count, 0);
    assert_eq!(again.projection_hash, expected.projection_hash);
}

#[test]
fn a_collision_edit_while_deriving_is_rederived_by_the_next_publication() {
    let (mut bridge, session) = bridge_with(terrain());
    bridge
        .replace_collision_navigation(&request(session, WIDTH, true))
        .unwrap();
    // The worker still holds the scene or has let it go; either way the
    // edit is recorded for the grid deriving.
    let edits = [NativeVoxelEdit {
        state: 0,
        kind: NativeVoxelEditKind::Clear,
        address: NativeVoxelAddress { x: 30, y: 1, z: 30 },
        material_slot: 0,
    }];
    let voxel = crate::voxel::api(&mut bridge);
    let mut edited = NativeVoxelEditReceipt::default();
    let mut refusal = crate::operation_diagnostics::empty_receipt();
    let status = unsafe {
        (voxel.apply_edits)(
            voxel.context,
            &NativeVoxelEditTransaction {
                session,
                edits: edits.as_ptr(),
                edits_len: edits.len(),
            },
            &mut edited,
            &mut refusal,
        )
    };
    assert_eq!(status, ABI_OK);
    settle(&mut bridge, session);

    let receipt = bridge
        .replace_collision_navigation(&request(session, WIDTH, false))
        .unwrap();
    assert!(
        (1..=25).contains(&receipt.derived_column_count),
        "derived {} columns",
        receipt.derived_column_count
    );
    let (mut fresh, fresh_session) =
        bridge_with(Arc::clone(&bridge.sessions[&session.value].scene));
    fresh
        .replace_collision_navigation(&request(fresh_session, WIDTH, false))
        .unwrap();
    assert_eq!(
        installed(&bridge, session),
        installed(&fresh, fresh_session)
    );
}

#[test]
fn replacing_clearing_rebasing_or_disposing_drops_a_pending_derivation() {
    let scene = terrain();
    type Drop = fn(&mut RuntimeSpatialBridge, NativeSpatialSessionHandle);
    let drops: [(&str, Drop); 5] = [
        ("replace", |bridge, session| {
            bridge
                .replace_collision_navigation(&request(session, 8, false))
                .unwrap();
        }),
        ("clear grid", |bridge, session| {
            bridge
                .clear_navigation(NativeNavigationClearRequest {
                    session,
                    grid_id: GRID,
                })
                .unwrap();
        }),
        ("clear every grid", |bridge, session| {
            bridge
                .clear_navigation(NativeNavigationClearRequest {
                    session,
                    grid_id: 0,
                })
                .unwrap();
        }),
        ("rebase", |bridge, session| {
            let session = bridge.sessions.get_mut(&session.value).unwrap();
            let scene = Arc::clone(&session.scene);
            session.rebase_collision(scene, [0; 3], [0; 3]);
        }),
        ("dispose", |bridge, session| {
            let api = api(bridge);
            assert_eq!(
                unsafe { (api.destroy_session)(api.context, session) },
                ABI_OK
            );
        }),
    ];
    for (name, drop_it) in drops {
        let (mut bridge, session) = bridge_with(Arc::clone(&scene));
        bridge
            .replace_collision_navigation(&request(session, WIDTH, true))
            .unwrap();
        let cancel = cancel_flag(&bridge, session);
        assert!(!cancel.load(Ordering::Relaxed));
        drop_it(&mut bridge, session);
        assert!(cancel.load(Ordering::Relaxed), "{name}");
        if name == "dispose" {
            assert!(bridge.sessions.is_empty());
            continue;
        }
        bridge.settle_deferred_residency();
        let readout = read(&mut bridge, session);
        assert!(!readout.pending, "{name}");
        let expected = match name {
            "replace" => 64,
            _ => 0,
        };
        assert_eq!(
            readout.installed.walkable_cell_count, expected,
            "{name}: the grid is as the drop left it"
        );
    }

    // A second deferred request cancels the first and derives in its place.
    let (mut bridge, session) = bridge_with(scene);
    bridge
        .replace_collision_navigation(&request(session, WIDTH, true))
        .unwrap();
    let first = cancel_flag(&bridge, session);
    bridge
        .replace_collision_navigation(&request(session, 8, true))
        .unwrap();
    assert!(first.load(Ordering::Relaxed));
    assert_eq!(
        settle(&mut bridge, session).installed.walkable_cell_count,
        64
    );
}

#[test]
fn a_cancelled_worker_stops_between_runs() {
    let scene = terrain();
    let config = request(NativeSpatialSessionHandle { value: 0 }, WIDTH, true).config;
    let (policy, _) = CollisionNavigationPolicy::from_native(&config).unwrap();
    let grid = policy.grid().unwrap();
    let bounds = ColumnBounds {
        min_x: 0,
        max_x: WIDTH - 1,
        min_z: 0,
        max_z: WIDTH - 1,
    };
    let cancelled = AtomicBool::new(true);
    let derived = derive_collision_navigation(
        &scene,
        grid,
        [0.0, 12.0],
        policy,
        bounds,
        None,
        Lane::Worker(&cancelled),
    )
    .unwrap();
    assert!(derived.is_none());
}

/// What a collision edit made while a deferred derivation holds the scene
/// pays for copying it, against the same edit with the scene unshared, on
/// voxel ground of growing size:
/// `cargo test --release -p csharp-engine-services --lib measure_edit_while_deriving -- --ignored --nocapture`.
#[test]
#[ignore = "timing measurement"]
fn measure_edit_while_deriving() {
    for side in [64_i64, 256, 512] {
        let mut voxels = Vec::new();
        for x in 0..side {
            for z in 0..side {
                for y in 0..2 + (x + z) % 3 {
                    voxels.push([x, y, z]);
                }
            }
        }
        let count = voxels.len();
        let scene = VoxelCollisionScene::from_solid_voxels(1.0, 16, voxels).unwrap();
        let edit = |held: bool| {
            let (mut bridge, session) = bridge_with(Arc::new(scene.clone()));
            // What a deferred derivation's worker holds.
            let worker = held.then(|| Arc::clone(&bridge.sessions[&session.value].scene));
            let started = Instant::now();
            bridge
                .edit_scene(session, |session| {
                    session.change_collision(|_| ((), Vec::new()))
                })
                .unwrap();
            let elapsed = started.elapsed();
            drop(worker);
            elapsed
        };
        println!(
            "EDIT_WHILE_DERIVING side={side} voxels={count} edit_held_ms={:.2} edit_unshared_ms={:.2}",
            edit(true).as_secs_f64() * 1e3,
            edit(false).as_secs_f64() * 1e3,
        );
    }
}
