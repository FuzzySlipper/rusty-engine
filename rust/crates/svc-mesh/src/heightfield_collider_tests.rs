use super::*;

const LAYOUT: ColliderLayout = ColliderLayout {
    tile_cells: 4,
    cell_size: 1.0,
    reach: 3.0,
};

fn flat() -> Vec<f32> {
    vec![0.0; LAYOUT.height_side().pow(2)]
}

/// Sample and hand over every tile asked for, as live.
fn fill(tiles: &mut ColliderTiles) {
    for request in tiles.requests(0) {
        assert!(tiles.commit(request.tile, request.revision, &flat()));
    }
    let batch = tiles.take_batch();
    tiles.finished(&batch.tiles, true);
}

#[test]
fn tiles_gather_about_every_focus_nearest_first() {
    let mut tiles = ColliderTiles::new(LAYOUT, 100);
    tiles.set_focuses(vec![[1.0, 1.0], [41.0, 1.0]]);
    let requests = tiles.requests(0);
    // Each focus reaches 3 units: tiles -1 to 1 about it on each axis.
    assert_eq!(requests.len(), 9 + 9);
    assert!(requests.iter().any(|request| request.tile == (10, 0)));
    assert_eq!(tiles.requests(3).len(), 3);
    assert_eq!(requests[0].tile, (0, 0), "nearest first");
}

#[test]
fn ground_is_ready_only_once_its_batch_has_installed() {
    let mut tiles = ColliderTiles::new(LAYOUT, 100);
    tiles.set_focuses(vec![[1.0, 1.0]]);
    assert!(!tiles.ready([1.0, 1.0], 3.0));
    for request in tiles.requests(0) {
        tiles.commit(request.tile, request.revision, &flat());
    }
    assert!(!tiles.ready([1.0, 1.0], 3.0), "sampled is not ground");
    let batch = tiles.take_batch();
    assert_eq!(batch.geometry.len(), 9);
    assert!(!tiles.ready([1.0, 1.0], 3.0), "handed over is not ground");
    tiles.finished(&batch.tiles, true);
    assert!(tiles.ready([1.0, 1.0], 3.0));
    assert!(!tiles.ready([40.0, 1.0], 1.0), "ground not held");
    assert_eq!(
        tiles.counts(),
        ColliderCounts {
            live: 9,
            pending: 0,
            refused: 0
        }
    );
}

#[test]
fn a_refused_batch_is_counted_and_not_ground() {
    let mut tiles = ColliderTiles::new(LAYOUT, 100);
    tiles.set_focuses(vec![[1.0, 1.0]]);
    for request in tiles.requests(0) {
        tiles.commit(request.tile, request.revision, &flat());
    }
    let batch = tiles.take_batch();
    tiles.finished(&batch.tiles, false);
    assert!(!tiles.ready([1.0, 1.0], 3.0));
    assert_eq!(tiles.counts().refused, 9);
}

#[test]
fn leaving_tiles_go_past_a_margin_and_take_their_collision() {
    let mut tiles = ColliderTiles::new(LAYOUT, 100);
    tiles.set_focuses(vec![[1.0, 1.0]]);
    fill(&mut tiles);
    // One tile over: the old tiles stay within the margin.
    tiles.set_focuses(vec![[5.0, 1.0]]);
    assert!(tiles.take_batch().removed.is_empty());
    tiles.set_focuses(vec![[40.0, 1.0]]);
    assert_eq!(tiles.take_batch().removed.len(), 9);
}

#[test]
fn a_stale_revision_is_ignored_and_invalidating_keeps_the_ground_until_it_lands() {
    let mut tiles = ColliderTiles::new(LAYOUT, 100);
    tiles.set_focuses(vec![[1.0, 1.0]]);
    fill(&mut tiles);
    tiles.invalidate([0.5, 0.5], [1.5, 1.5]);
    let requests = tiles.requests(0);
    assert_eq!(requests.len(), 1, "only the tile it overlaps");
    assert!(tiles.ready([1.0, 1.0], 3.0), "the ground stands meanwhile");
    assert!(!tiles.commit(requests[0].tile, requests[0].revision - 1, &flat()));
    assert!(tiles.commit(requests[0].tile, requests[0].revision, &flat()));
    let batch = tiles.take_batch();
    assert_eq!(batch.geometry.len(), 1);
    assert_eq!(
        batch.geometry[0].id,
        100 + tiles_index_of(&tiles, requests[0].tile),
        "it replaces the same collision"
    );
}

fn tiles_index_of(tiles: &ColliderTiles, tile: (i64, i64)) -> u64 {
    tiles.tiles[&tile].id - tiles.first_id()
}

#[test]
fn a_tile_of_holes_stands_with_no_collision_and_drops_what_it_had() {
    let mut tiles = ColliderTiles::new(LAYOUT, 100);
    tiles.set_focuses(vec![[1.0, 1.0]]);
    fill(&mut tiles);
    tiles.invalidate([1.0, 1.0], [1.0, 1.0]);
    let request = tiles.requests(0)[0];
    let holes = vec![f32::NAN; LAYOUT.height_side().pow(2)];
    assert!(tiles.commit(request.tile, request.revision, &holes));
    let batch = tiles.take_batch();
    assert!(batch.geometry.is_empty());
    assert_eq!(batch.removed.len(), 1);
    assert!(tiles.ready([1.0, 1.0], 3.0));
}
