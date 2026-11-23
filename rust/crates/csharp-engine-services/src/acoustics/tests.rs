use std::sync::Arc;
use std::time::Instant;

use super::*;
use engine_spatial::{StaticMeshAssetId, StaticMeshColliderAsset, StaticMeshColliderInstance};

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
    let outcome = query(&scene, source, 24.0, AudioRolloff::LinearDecibels, &points);
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

/// The cave's rock as one static mesh of its air-facing voxel faces, facing
/// the air, with no voxels: rock the way a sculpted dungeon admits it.
fn cave_mesh_scene(solids: &[[i64; 3]]) -> Scene {
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
                if inside(beside) || !(0..3).all(|a| (0..CAVE[a]).contains(&beside[a])) {
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

/// Costs per occluded voice and per query in a cave-sized dungeon session.
/// `cargo test -p csharp-engine-services --release cave_costs -- --ignored --nocapture`
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
        ("static-mesh", cave_mesh_scene(&solids)),
    ] {
        let scene = Arc::new(scene);
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

        // The listener search, as the worker runs it.
        let started = Instant::now();
        let mut search = AirSearch::run(&scene, listener, f64::from(RANGE), &[]);
        for _ in 1..ROUNDS {
            search = AirSearch::run(&scene, listener, f64::from(RANGE), &[]);
        }
        let search_us = started.elapsed().as_secs_f64() * 1e6 / f64::from(ROUNDS);

        // A refresh on the update: straight rays and lookups in the search.
        let mut listening = ListenerAcoustics::default();
        let ear = listener.map(|value| value as f32);
        listening.hear(&scene, ear, &sounds, 0.0);
        while listening.searching() {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let started = Instant::now();
        let mut heard = Vec::new();
        for _ in 0..ROUNDS {
            heard = listening.hear(&scene, ear, &sounds, 0.0);
        }
        let refresh_us = started.elapsed().as_secs_f64() * 1e6 / f64::from(ROUNDS);
        let around = heard
            .iter()
            .filter(|heard| heard.path == ArrivalPath::Around)
            .count();
        let occluded = heard
            .iter()
            .filter(|heard| heard.path == ArrivalPath::Occluded)
            .count();
        let started = Instant::now();
        for _ in 0..ROUNDS {
            listening.space = None;
            listening.space(&scene, ear, 1.0);
        }
        let enclosure_us = started.elapsed().as_secs_f64() * 1e6 / f64::from(ROUNDS);

        // Queries from a noise at the listener's place to eight occluded
        // points, to one, and to one in the open.
        let time_query = |points: &[[f64; 3]]| {
            let started = Instant::now();
            let mut outcome = query(
                &scene,
                listener,
                RANGE,
                AudioRolloff::LinearDecibels,
                points,
            );
            for _ in 1..ROUNDS {
                outcome = query(
                    &scene,
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
        println!(
            "sound-world-cost {{\"rock\":\"{rock}\",\"cave_m\":[96,24,96],\"range_m\":{RANGE},\"listener_search_us\":{search_us:.0},\"listener_search_cells\":{},\"listener_search_truncated\":{},\"sounds\":{},\"around\":{around},\"occluded\":{occluded},\"refresh_us\":{refresh_us:.1},\"per_occluded_voice_us\":{:.2},\"enclosure_us\":{enclosure_us:.1},\"query8_us\":{eight_us:.0},\"query8_cells\":{},\"query8_rays\":{},\"query8_truncated\":{},\"query8_around\":{},\"query1_us\":{one_us:.0},\"query1_cells\":{},\"query_clear_us\":{clear_us:.1}}}",
            search.settled_cells(),
            search.truncated,
            sounds.len(),
            refresh_us / sounds.len().max(1) as f64,
            eight.cells_searched,
            eight.rays_cast,
            eight.truncated,
            eight
                .arrivals
                .iter()
                .filter(|(arrival, _)| arrival.path == ArrivalPath::Around)
                .count(),
            one.cells_searched,
        );
    }
}
