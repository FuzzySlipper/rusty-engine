use std::sync::Arc;
use std::time::Instant;

use super::*;
use engine_spatial::{
    StaticMeshAssetId, StaticMeshColliderAsset, StaticMeshColliderInstance, StaticMeshInstanceId,
    VoxelEdit, VoxelEditService,
};

#[test]
fn solid_muffles_without_silencing_a_metre_of_it() {
    assert_eq!(transmission(0.0), (1.0, None));
    let (gain, cutoff) = transmission(1.0);
    let decibels = 20.0 * gain.log10();
    assert!((decibels + 16.0).abs() < 0.01, "{decibels}");
    assert!((cutoff.unwrap() - 1250.0).abs() < 1.0, "{cutoff:?}");
    assert_eq!(transmission(4.0).1, Some(FLOOR_CUTOFF_HZ));
}

#[test]
fn a_query_hears_through_a_wall_and_round_its_end() {
    // A wall at x = 6, from z = 0 to 12, in a closed room 16 × 4 × 16 m.
    let scene = Scene::from_solid_voxels(1.0, 8, room(|x, _, z| x == 6 && z < 12)).unwrap();
    let source = [3.5, 1.5, 2.5];
    let points = [
        [3.5, 1.5, 8.5],  // same side: open
        [9.5, 1.5, 2.5],  // behind the wall near its foot: through it
        [9.5, 1.5, 13.5], // behind, past its end: round it
        [60.0, 1.5, 2.5], // out of range
    ];
    let air = AirGraph::around(&scene, &[source], 32.0);
    let outcome = query(
        &scene,
        &air,
        source,
        24.0,
        AudioRolloff::LinearDecibels,
        &points,
    );
    let paths: Vec<ArrivalPath> = outcome
        .arrivals
        .iter()
        .map(|(arrival, _)| arrival.path)
        .collect();
    assert_eq!(
        paths,
        [
            ArrivalPath::Open,
            ArrivalPath::Occluded,
            ArrivalPath::Around,
            ArrivalPath::Silent
        ]
    );
    let (open, from) = outcome.arrivals[0];
    assert_eq!(from, source);
    assert_eq!(
        open.gain,
        AudioRolloff::LinearDecibels.gain(24.0, open.metres as f32)
    );
    let (around, from) = outcome.arrivals[2];
    assert!(
        around.metres > 11.0 && around.low_pass_hz.is_none(),
        "{around:?}"
    );
    // It arrives from the gap past the wall's end, not from the source.
    assert!(from[2] > 9.0, "{from:?}");
    assert!(outcome.cells_searched > 0 && !outcome.truncated);
}

/// A closed room 16 × 4 × 16 m (walls, floor and ceiling one voxel thick)
/// with `solid` also rock.
fn room(solid: impl Fn(i64, i64, i64) -> bool) -> Vec<[i64; 3]> {
    let mut solids = Vec::new();
    for x in -1..17 {
        for y in -1..5 {
            for z in -1..17 {
                let shell = !(0..16).contains(&x) || !(0..4).contains(&y) || !(0..16).contains(&z);
                if shell || solid(x, y, z) {
                    solids.push([x, y, z]);
                }
            }
        }
    }
    solids
}

/// A cave 96 × 24 × 96 m: chambers joined by tunnels through rock, as 1 m
/// voxels. Chamber centres and radii, then the tunnels' ends by chamber.
const CHAMBERS: [([f64; 3], f64); 6] = [
    ([14.0, 10.0, 14.0], 8.0),
    ([48.0, 12.0, 16.0], 10.0),
    ([80.0, 10.0, 20.0], 8.0),
    ([76.0, 11.0, 70.0], 9.0),
    ([44.0, 10.0, 78.0], 8.0),
    ([16.0, 12.0, 62.0], 10.0),
];
const TUNNELS: [(usize, usize, f64); 7] = [
    (0, 1, 2.5),
    (1, 2, 2.0),
    (2, 3, 3.0),
    (3, 4, 2.5),
    (4, 5, 2.0),
    (5, 0, 2.5),
    (1, 4, 2.0),
];
const CAVE: [i64; 3] = [96, 24, 96];

fn cave_air(point: [f64; 3]) -> bool {
    let chamber = CHAMBERS.iter().any(|(centre, radius)| {
        // Flattened chambers: half as tall as wide.
        let offset = [
            point[0] - centre[0],
            (point[1] - centre[1]) * 2.0,
            point[2] - centre[2],
        ];
        length(offset) < *radius
    });
    chamber
        || TUNNELS.iter().any(|(from, to, radius)| {
            let (a, b) = (CHAMBERS[*from].0, CHAMBERS[*to].0);
            let along = sub(b, a);
            let t = (dot(sub(point, a), along) / dot(along, along)).clamp(0.0, 1.0);
            length(sub(point, add(a, along.map(|value| value * t)))) < *radius
        })
}

fn dot(left: [f64; 3], right: [f64; 3]) -> f64 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

fn cave_rock() -> Vec<[i64; 3]> {
    let mut solids = Vec::new();
    for x in 0..CAVE[0] {
        for y in 0..CAVE[1] {
            for z in 0..CAVE[2] {
                if !cave_air([x as f64 + 0.5, y as f64 + 0.5, z as f64 + 0.5]) {
                    solids.push([x, y, z]);
                }
            }
        }
    }
    solids
}

/// The cave's rock as one static mesh of its voxels' faces, facing out of
/// the rock, with no voxels: rock the way a sculpted dungeon admits it.
/// `closed` adds the faces on the cave's outer bounds, so the mesh encloses
/// its rock; without them only the faces onto air are kept.
fn cave_mesh_scene(solids: &[[i64; 3]], closed: bool) -> Scene {
    let solid: std::collections::HashSet<[i64; 3]> = solids.iter().copied().collect();
    let inside = |cell: [i64; 3]| {
        (0..3).all(|axis| (0..CAVE[axis]).contains(&cell[axis])) && solid.contains(&cell)
    };
    let mut positions = Vec::new();
    let mut triangles = Vec::new();
    for cell in solids {
        for axis in 0..3 {
            for sign in [-1_i64, 1] {
                let mut beside = *cell;
                beside[axis] += sign;
                if inside(beside) || (!closed && !(0..3).all(|a| (0..CAVE[a]).contains(&beside[a])))
                {
                    continue;
                }
                // The face's corners, wound so its normal points to the air.
                let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
                let mut corner = |du: i64, dv: i64| {
                    let mut at = cell.map(|value| value as f64);
                    at[axis] += if sign > 0 { 1.0 } else { 0.0 };
                    at[u] += du as f64;
                    at[v] += dv as f64;
                    positions.push(at);
                    (positions.len() - 1) as u32
                };
                let quad = [corner(0, 0), corner(1, 0), corner(1, 1), corner(0, 1)];
                if sign > 0 {
                    triangles.push([quad[0], quad[1], quad[2]]);
                    triangles.push([quad[0], quad[2], quad[3]]);
                } else {
                    triangles.push([quad[0], quad[2], quad[1]]);
                    triangles.push([quad[0], quad[3], quad[2]]);
                }
            }
        }
    }
    let mut scene = Scene::from_solid_voxels(1.0, 8, []).unwrap();
    let asset = StaticMeshColliderAsset::new(StaticMeshAssetId(1), positions, triangles).unwrap();
    scene
        .replace_static_mesh_colliders(
            [asset],
            [StaticMeshColliderInstance {
                id: engine_spatial::StaticMeshInstanceId(1),
                asset: StaticMeshAssetId(1),
                transform: Default::default(),
            }],
        )
        .unwrap();
    scene
}

/// Cave air points 8..40 m from `from` with rock on the straight line,
/// spread through the cave.
fn occluded_points(scene: &Scene, from: [f64; 3], count: usize) -> Vec<[f64; 3]> {
    let mut points = Vec::new();
    for x in (1..CAVE[0]).step_by(5) {
        for z in (1..CAVE[2]).step_by(5) {
            for y in (2..CAVE[1]).step_by(4) {
                let point = [x as f64 + 0.5, y as f64 + 0.5, z as f64 + 0.5];
                let distance = length(sub(point, from));
                if cave_air(point)
                    && (8.0..40.0).contains(&distance)
                    && scene.solid_metres(from, point) > 0.0
                {
                    points.push(point);
                }
            }
        }
    }
    let stride = (points.len() / count).max(1);
    points.into_iter().step_by(stride).take(count).collect()
}

/// A static-mesh door: a slab 0.1 m thick across the tunnel from the first
/// chamber to the second, at x = 30.
fn door() -> (StaticMeshColliderAsset, StaticMeshColliderInstance) {
    let (min, max) = ([29.95, 8.0, 12.0], [30.05, 14.0, 18.0]);
    let mut positions = Vec::new();
    for corner in 0..8 {
        positions.push([
            if corner & 1 == 0 { min[0] } else { max[0] },
            if corner & 2 == 0 { min[1] } else { max[1] },
            if corner & 4 == 0 { min[2] } else { max[2] },
        ]);
    }
    let mut triangles = Vec::new();
    for [a, b, c, d] in [
        [0, 2, 6, 4],
        [1, 5, 7, 3],
        [0, 4, 5, 1],
        [2, 3, 7, 6],
        [0, 1, 3, 2],
        [4, 6, 7, 5],
    ] {
        triangles.push([a, c, b]);
        triangles.push([a, d, c]);
    }
    (
        StaticMeshColliderAsset::new(StaticMeshAssetId(2), positions, triangles).unwrap(),
        StaticMeshColliderInstance {
            id: StaticMeshInstanceId(2),
            asset: StaticMeshAssetId(2),
            transform: Default::default(),
        },
    )
}

/// Places or takes away the door in the session's scene and tells its
/// keeper, as publishing it would; how long until the graph shows it.
fn swing_door(
    scene: &mut Arc<Scene>,
    keeper: &mut crate::air::AirKeeper,
    shut: bool,
) -> (f64, u64) {
    let reader = keeper.reader();
    let before = reader.stats().rebuilds;
    let (asset, instance) = door();
    let editing = Arc::make_mut(scene);
    if shut {
        editing
            .apply_static_mesh_residency([asset], [instance], [], [])
            .unwrap();
    } else {
        editing
            .apply_static_mesh_residency([], [], [StaticMeshAssetId(2)], [StaticMeshInstanceId(2)])
            .unwrap();
    }
    let started = Instant::now();
    keeper.scene_changed(scene);
    assert!(reader.wait_idle(std::time::Duration::from_secs(30)));
    (
        started.elapsed().as_secs_f64(),
        reader.stats().rebuilds - before,
    )
}

#[test]
fn a_door_opening_lets_the_next_query_round_it_after_its_chunks_rebuild() {
    // The cave with the first tunnel shut by a door; a noise in the first
    // chamber heard from the second.
    let mut scene = Arc::new(Scene::from_solid_voxels(1.0, 8, cave_rock()).unwrap());
    let mut keeper = crate::air::AirKeeper::new(&scene);
    let reader = keeper.reader();
    swing_door(&mut scene, &mut keeper, true);
    // Points in the two chambers with rock between them, so the sound
    // goes round by the tunnel once the door is open.
    let within = |chamber: usize| {
        let (centre, radius) = CHAMBERS[chamber];
        let reach = radius as i64;
        (-reach..=reach).flat_map(move |dx| {
            (-reach..=reach).flat_map(move |dz| {
                (-3..=3).map(move |dy| add(centre, [dx as f64, dy as f64, dz as f64]))
            })
        })
    };
    let (noise, ear) = within(0)
        .filter(|point| cave_air(*point))
        .flat_map(|noise| within(1).map(move |ear| (noise, ear)))
        .find(|(noise, ear)| cave_air(*ear) && scene.solid_metres(*noise, *ear) > 2.0)
        .expect("rock between the chambers");
    reader.focus(noise);
    reader.focus(ear);
    assert!(reader.wait_idle(std::time::Duration::from_secs(60)));
    let built = reader.stats().builds;
    let hear = |scene: &Scene| {
        query(
            scene,
            &reader.latest(),
            noise,
            64.0,
            AudioRolloff::LinearDecibels,
            &[ear],
        )
    };
    let shut = hear(&scene);
    let (arrival, _) = shut.arrivals[0];
    assert!(!shut.truncated, "{arrival:?}");
    // Round by the other tunnels, through four chambers: far, if at all.
    let longer = (arrival.path == ArrivalPath::Around).then_some(arrival.metres);

    let (_, rebuilt) = swing_door(&mut scene, &mut keeper, false);
    assert!((1..=2).contains(&rebuilt), "{rebuilt}");
    assert_eq!(reader.stats().builds, built, "nothing else is built again");
    let open = hear(&scene);
    let (arrival, from) = open.arrivals[0];
    assert_eq!(arrival.path, ArrivalPath::Around, "{arrival:?}");
    assert!(
        longer.is_none_or(|longer| arrival.metres < longer - 10.0),
        "{arrival:?} {longer:?}"
    );
    // It arrives along the tunnel from the first chamber.
    assert!(from[0] < ear[0] - 1.0, "{from:?}");
}

/// Costs per occluded voice, per query and per collision change in a
/// cave-sized dungeon session.
/// `cargo test -p csharp-engine-services --release --lib cave_costs -- --ignored --nocapture`
#[test]
#[ignore = "measurement; run in release"]
fn cave_costs() {
    const RANGE: f32 = 48.0;
    const ROUNDS: u32 = 20;
    let solids = cave_rock();
    for (rock, scene) in [
        (
            "voxels",
            Scene::from_solid_voxels(1.0, 8, solids.clone()).unwrap(),
        ),
        ("static-mesh", cave_mesh_scene(&solids, true)),
        ("static-mesh-air-faces", cave_mesh_scene(&solids, false)),
    ] {
        let mut scene = Arc::new(scene);
        // The listener stands in the second chamber, among the tunnels.
        let listener = CHAMBERS[1].0;
        let points = occluded_points(&scene, listener, 16);
        let sounds: Vec<Sounding> = points
            .iter()
            .map(|point| Sounding {
                position: point.map(|value| value as f32),
                max_distance: RANGE,
                rolloff: AudioRolloff::LinearDecibels,
            })
            .collect();

        // The graph built round the listener by the session's keeper.
        let mut keeper = crate::air::AirKeeper::new(&scene);
        let reader = keeper.reader();
        let started = Instant::now();
        reader.focus(listener);
        assert!(reader.wait_idle(std::time::Duration::from_secs(600)));
        let build_s = started.elapsed().as_secs_f64();
        let built = reader.stats();
        let air = reader.latest();
        let chunks = air.chunk_count();
        let mixed = air.mixed_chunk_count();
        let open = air
            .chunk_keys()
            .filter(|key| air.chunk(*key).is_some_and(|chunk| chunk.is_open()))
            .count();
        // Each chunk built again here, timed by what it holds.
        let mut times: [Vec<f64>; 3] = Default::default();
        for key in air.chunk_keys() {
            let started = Instant::now();
            let chunk = engine_spatial::AirChunk::build(&scene, key);
            let ms = started.elapsed().as_secs_f64() * 1e3;
            times[if chunk.is_open() {
                0
            } else if chunk.is_solid() {
                1
            } else {
                2
            }]
            .push(ms);
        }
        let mean = |times: &[f64]| times.iter().sum::<f64>() / times.len().max(1) as f64;
        let most = |times: &[f64]| times.iter().copied().fold(0.0, f64::max);

        // A refresh on the update, by the routes its worker found; and the
        // worker's routing, as a moving listener has it route each refresh.
        let ear = listener.map(|value| value as f32);
        let mut listening = ListenerAcoustics::default();
        listening.hear(&scene, &air, ear, &sounds);
        while listening.routing() {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let mut heard = Vec::new();
        let started = Instant::now();
        for _ in 0..ROUNDS {
            heard = listening.hear(&scene, &air, ear, &sounds);
        }
        let refresh_us = started.elapsed().as_secs_f64() * 1e6 / f64::from(ROUNDS);
        let started = Instant::now();
        for _ in 0..ROUNDS {
            air.route(&scene, listener, &points, f64::from(RANGE));
        }
        let routed_us = started.elapsed().as_secs_f64() * 1e6 / f64::from(ROUNDS);
        let around = heard
            .iter()
            .filter(|heard| heard.path == ArrivalPath::Around)
            .count();
        let occluded = heard
            .iter()
            .filter(|heard| heard.path == ArrivalPath::Occluded)
            .count();

        // Queries from a noise at the listener's place to eight occluded
        // points, to one, and to one in the open.
        let time_query = |points: &[[f64; 3]]| {
            let started = Instant::now();
            let mut outcome = query(
                &scene,
                &air,
                listener,
                RANGE,
                AudioRolloff::LinearDecibels,
                points,
            );
            for _ in 1..ROUNDS {
                outcome = query(
                    &scene,
                    &air,
                    listener,
                    RANGE,
                    AudioRolloff::LinearDecibels,
                    points,
                );
            }
            (
                started.elapsed().as_secs_f64() * 1e6 / f64::from(ROUNDS),
                outcome,
            )
        };
        let (eight_us, eight) = time_query(&points[..8.min(points.len())]);
        let (one_us, one) = time_query(&points[..1]);
        let (clear_us, _) = time_query(&[add(listener, [3.0, 0.0, 0.0])]);
        let longest = eight
            .arrivals
            .iter()
            .filter(|(arrival, _)| arrival.path == ArrivalPath::Around)
            .map(|(arrival, _)| arrival.metres)
            .fold(0.0, f64::max);

        // A door shutting and opening, and a voxel edit: until the graph
        // shows each.
        let (shut_s, shut_rebuilt) = swing_door(&mut scene, &mut keeper, true);
        let (open_s, open_rebuilt) = swing_door(&mut scene, &mut keeper, false);
        let before = reader.stats().rebuilds;
        let block: Vec<VoxelEdit> = (0..2)
            .flat_map(|x| (0..2).flat_map(move |y| (0..2).map(move |z| [x, y, z])))
            .map(|offset| VoxelEdit::Set {
                address: [50 + offset[0], 8 + offset[1], 18 + offset[2]],
                material_slot: 1,
            })
            .collect();
        VoxelEditService::apply(Arc::make_mut(&mut scene), &block).unwrap();
        let started = Instant::now();
        keeper.scene_changed(&scene);
        assert!(reader.wait_idle(std::time::Duration::from_secs(30)));
        let edit_s = started.elapsed().as_secs_f64();
        let edit_rebuilt = reader.stats().rebuilds - before;
        let stats = reader.stats();
        let rebuild_ms = stats.rebuild_seconds_total * 1e3 / stats.rebuilds.max(1) as f64;
        println!(
            "air-graph-cost {{\"rock\":\"{rock}\",\"cave_m\":[96,24,96],\"range_m\":{RANGE},\"build_s\":{build_s:.2},\"chunks\":{chunks},\"mixed\":{mixed},\"open\":{open},\"solid\":{},\"bytes\":{},\"bytes_per_mixed\":{},\"build_ms_mean\":{:.1},\"build_ms_max\":{:.1},\"open_build_ms\":{:.2},\"mixed_build_ms\":{:.1},\"mixed_build_ms_max\":{:.1},\"sounds\":{},\"around\":{around},\"occluded\":{occluded},\"refresh_us\":{refresh_us:.1},\"listener_route_worker_us\":{routed_us:.1},\"query8_us\":{eight_us:.1},\"query8_settled\":{},\"query8_truncated\":{},\"query8_around\":{},\"query8_longest_m\":{longest:.1},\"query1_us\":{one_us:.1},\"query1_settled\":{},\"query_clear_us\":{clear_us:.1},\"door_shut_ms\":{:.1},\"door_shut_rebuilt\":{shut_rebuilt},\"door_open_ms\":{:.1},\"door_open_rebuilt\":{open_rebuilt},\"voxel_edit_ms\":{:.1},\"voxel_edit_rebuilt\":{edit_rebuilt},\"rebuild_ms_mean\":{rebuild_ms:.1}}}",
            chunks - mixed - open,
            air.bytes(),
            air.bytes() / mixed.max(1),
            built.build_seconds_total * 1e3 / built.builds.max(1) as f64,
            built.build_seconds_max * 1e3,
            mean(&times[0]),
            mean(&times[2]),
            most(&times[2]),
            sounds.len(),
            eight.cells_searched,
            eight.truncated,
            eight
                .arrivals
                .iter()
                .filter(|(arrival, _)| arrival.path == ArrivalPath::Around)
                .count(),
            one.cells_searched,
            shut_s * 1e3,
            open_s * 1e3,
            edit_s * 1e3,
        );
    }
}
