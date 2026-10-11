use super::*;
use crate::heightfield::HeightfieldRingSpec;

/// 4-cell tiles: 1 m cells within 10 m (4 m tiles, 16 m region tiles), 4 m
/// cells within 80 m (16 m tiles).
fn tiles() -> HeightfieldTiles {
    HeightfieldTiles::new(
        HeightfieldLayout::new(
            4,
            &[
                HeightfieldRingSpec {
                    cell_size: 1.0,
                    reach: 10.0,
                },
                HeightfieldRingSpec {
                    cell_size: 4.0,
                    reach: 80.0,
                },
            ],
        )
        .unwrap(),
    )
}

fn ground(x: f64, z: f64) -> f32 {
    (3.0 * (x / 11.0).sin() + 2.0 * (z / 7.0).cos()) as f32
}

fn samples(layout: &HeightfieldLayout, key: TileKey) -> TileSamples {
    let cell = layout.rings[key.ring as usize].cell_size;
    let origin = layout.origin(key);
    let side = layout.height_side() as i64;
    TileSamples {
        heights: (0..side * side)
            .map(|index| {
                let (i, j) = (index % side - 1, index / side - 1);
                ground(origin[0] + i as f64 * cell, origin[1] + j as f64 * cell)
            })
            .collect(),
        ..TileSamples::default()
    }
}

/// Answer every request (of `ring` alone, if given).
fn sample_all(heightfield: &mut HeightfieldTiles, ring: Option<u8>) {
    for request in heightfield.requests(0) {
        if ring.is_none_or(|ring| ring == request.key.ring) {
            let samples = samples(heightfield.layout(), request.key);
            assert!(heightfield.commit(request.key, request.revision, samples));
        }
    }
    heightfield.mesh();
}

/// How many drawn tiles cover each point of the coarsest ring's region, two
/// to a finest tile's side: every point exactly once means no overlap and no
/// gap.
fn coverage(heightfield: &HeightfieldTiles) -> BTreeMap<usize, usize> {
    let layout = heightfield.layout();
    let top = layout.rings.len() - 1;
    let region = layout.region(top, heightfield.focus().unwrap());
    let size = layout.rings[top].tile_size;
    let step = layout.rings[0].tile_size / 2.0;
    let mut counts = BTreeMap::new();
    let (lo, hi) = (
        region.min.map(|v| v as f64 * size),
        region.max.map(|v| (v + 1) as f64 * size),
    );
    let mut z = lo[1] + 0.25;
    while z < hi[1] {
        let mut x = lo[0] + 0.25;
        while x < hi[0] {
            let covering = heightfield
                .drawn()
                .filter(|key| {
                    let at = layout.origin(*key);
                    let s = layout.rings[key.ring as usize].tile_size;
                    (at[0]..at[0] + s).contains(&x) && (at[1]..at[1] + s).contains(&z)
                })
                .count();
            *counts.entry(covering).or_insert(0) += 1;
            x += step;
        }
        z += step;
    }
    counts
}

#[test]
fn rings_load_coarse_first_and_cover_their_region_exactly_once_at_every_stage() {
    let mut heightfield = tiles();
    heightfield.set_focus(Some([3.0, -5.0]));
    let requests = heightfield.requests(0);
    // 80 m in 16 m tiles: 5 each side, 11² coarse; 10 m in 16 m region tiles:
    // 1 each side, 3² region tiles of 4² fine tiles.
    assert_eq!(requests.len(), 121 + 144);
    assert!(requests[..121].iter().all(|request| request.key.ring == 1));
    assert!(!heightfield.settled());
    // The coarse ring alone covers everything.
    sample_all(&mut heightfield, Some(1));
    assert_eq!(
        coverage(&heightfield).keys().copied().collect::<Vec<_>>(),
        [1]
    );
    assert_eq!(heightfield.drawn().count(), 121);
    // Half the fine ring: a coarse tile gives way only when all 16 of its
    // fine tiles are meshed, so coverage stays exact.
    for request in heightfield.requests(70) {
        let samples = samples(heightfield.layout(), request.key);
        assert!(heightfield.commit(request.key, request.revision, samples));
    }
    heightfield.mesh();
    assert_eq!(
        coverage(&heightfield).keys().copied().collect::<Vec<_>>(),
        [1]
    );
    sample_all(&mut heightfield, None);
    assert_eq!(
        coverage(&heightfield).keys().copied().collect::<Vec<_>>(),
        [1]
    );
    assert_eq!(heightfield.drawn().count(), 121 - 9 + 144);
    assert!(heightfield.settled());
    let changes = heightfield.take_changes();
    assert_eq!(changes.meshed.len(), 121 + 144);
    assert!(changes.evicted.is_empty());
}

#[test]
fn a_moving_focus_asks_only_for_new_tiles_and_lets_go_beyond_a_region_tile() {
    let mut heightfield = tiles();
    heightfield.set_focus(Some([3.0, -5.0]));
    sample_all(&mut heightfield, None);
    heightfield.take_changes();
    // Into the next 16 m column: one new column of coarse tiles and of fine
    // region tiles; nothing let go yet (kept a region tile beyond).
    heightfield.set_focus(Some([17.0, -5.0]));
    let requests = heightfield.requests(0);
    assert_eq!(requests.iter().filter(|r| r.key.ring == 1).count(), 11);
    assert_eq!(requests.iter().filter(|r| r.key.ring == 0).count(), 3 * 16);
    let changes = heightfield.take_changes();
    assert!(changes.evicted.is_empty());
    sample_all(&mut heightfield, None);
    assert_eq!(
        coverage(&heightfield).keys().copied().collect::<Vec<_>>(),
        [1]
    );
    // Two columns on, the first column's tiles go.
    heightfield.set_focus(Some([33.0, -5.0]));
    let changes = heightfield.take_changes();
    assert_eq!(
        changes.evicted.iter().filter(|key| key.ring == 1).count(),
        11
    );
    assert!(changes
        .evicted
        .iter()
        .all(|key| key.ring == 1 && key.x == -5 || key.ring == 0 && key.x < 0));
    // Back again: the tiles it let go are asked for afresh.
    heightfield.set_focus(Some([3.0, -5.0]));
    assert!(heightfield.requests(0).iter().any(|request| request.key
        == TileKey {
            ring: 1,
            x: -5,
            z: 0
        }));
}

#[test]
fn samples_worked_out_before_an_invalidation_or_an_eviction_are_ignored() {
    let mut heightfield = tiles();
    heightfield.set_focus(Some([3.0, -5.0]));
    let first = heightfield.requests(1)[0];
    let old = samples(heightfield.layout(), first.key);
    let at = heightfield.layout().origin(first.key);
    heightfield.invalidate([at[0] + 1.0, at[1] + 1.0], [at[0] + 2.0, at[1] + 2.0]);
    let renewed = heightfield
        .requests(0)
        .into_iter()
        .find(|request| request.key == first.key)
        .unwrap();
    assert_ne!(renewed.revision, first.revision);
    assert!(!heightfield.commit(first.key, first.revision, old.clone()));
    assert!(heightfield.commit(first.key, renewed.revision, old.clone()));
    // An invalidated drawn tile keeps drawing until its new samples land.
    sample_all(&mut heightfield, None);
    let tile = TileKey {
        ring: 1,
        x: 3,
        z: 3,
    };
    assert!(heightfield.is_drawn(tile));
    heightfield.invalidate([50.0, 50.0], [51.0, 51.0]);
    heightfield.mesh();
    assert!(heightfield.is_drawn(tile));
    let request = heightfield.requests(0)[0];
    assert_eq!(request.key, tile);
    // Gone and back: the old request is stale.
    heightfield.set_focus(Some([1000.0, -5.0]));
    heightfield.set_focus(Some([3.0, -5.0]));
    let fresh = heightfield
        .requests(0)
        .into_iter()
        .find(|r| r.key == tile)
        .unwrap();
    assert_ne!(fresh.revision, request.revision);
    assert!(!heightfield.commit(tile, request.revision, samples(heightfield.layout(), tile)));
    assert!(!heightfield.commit(
        TileKey {
            ring: 1,
            x: 99,
            z: 0
        },
        1,
        old
    ));
}

#[test]
fn a_pick_finds_the_nearest_drawn_tile_along_the_ray() {
    let mut heightfield = tiles();
    heightfield.set_focus(Some([3.0, -5.0]));
    sample_all(&mut heightfield, None);
    // Straight down onto the fine ring, then onto the coarse ring far out.
    for (x, z) in [(2.3, -4.1), (61.7, 40.2)] {
        let (key, hit) = heightfield.pick([x, 100.0, z], [0.0, -1.0, 0.0]).unwrap();
        assert!((hit.point[0] - x).abs() < 1e-9 && (hit.point[2] - z).abs() < 1e-9);
        assert!(
            (hit.point[1] - f64::from(ground(x, z))).abs() < 0.6,
            "{key:?} {hit:?}"
        );
    }
    // A slanting ray across many tiles stops at the first surface it meets.
    let (_, hit) = heightfield
        .pick([-60.0, 20.0, 0.0], [1.0, -0.2, 0.0])
        .unwrap();
    assert!(hit.point[1] > -6.0 && hit.point[1] < 6.0);
    assert!(heightfield
        .pick([0.0, 100.0, 0.0], [0.0, 1.0, 0.0])
        .is_none());
    let footprint = heightfield.footprint();
    assert!(footprint.vertices > 0 && footprint.sample_bytes == (121 + 144) * 49 * 4);
}

#[test]
fn with_three_rings_tiles_landing_in_any_order_cover_the_region_exactly_once() {
    let spec = |cell_size, reach| HeightfieldRingSpec { cell_size, reach };
    let layout =
        HeightfieldLayout::new(4, &[spec(1.0, 6.0), spec(4.0, 24.0), spec(16.0, 80.0)]).unwrap();
    let mut heightfield = HeightfieldTiles::new(layout);
    heightfield.set_focus(Some([3.0, 3.0]));
    sample_all(&mut heightfield, Some(2));
    assert_eq!(
        coverage(&heightfield).keys().copied().collect::<Vec<_>>(),
        [1]
    );
    // A middle tile and all its fine tiles land before the middle tile's
    // siblings: the coarse tile above them still draws, so they wait.
    let middle = TileKey {
        ring: 1,
        x: 0,
        z: 0,
    };
    let first: Vec<TileKey> = std::iter::once(middle)
        .chain(heightfield.layout().children(middle))
        .collect();
    for request in heightfield.requests(0) {
        if first.contains(&request.key) {
            let samples = samples(heightfield.layout(), request.key);
            assert!(heightfield.commit(request.key, request.revision, samples));
        }
    }
    heightfield.mesh();
    assert!(!heightfield.is_drawn(middle) && first.iter().all(|key| !heightfield.is_drawn(*key)));
    assert_eq!(
        coverage(&heightfield).keys().copied().collect::<Vec<_>>(),
        [1]
    );
    // The rest land scrambled, a few at a time.
    let mut rest = heightfield.requests(0);
    rest.sort_by_key(|request| {
        (request.key.x * 7919 + request.key.z * 104_729 + i64::from(request.key.ring) * 31)
            .rem_euclid(97)
    });
    for batch in rest.chunks(5) {
        for request in batch {
            let samples = samples(heightfield.layout(), request.key);
            assert!(heightfield.commit(request.key, request.revision, samples));
        }
        heightfield.mesh();
        assert_eq!(
            coverage(&heightfield).keys().copied().collect::<Vec<_>>(),
            [1]
        );
    }
    assert!(heightfield.settled() && heightfield.is_drawn(first[1]));
}
