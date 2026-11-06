//! A heightfield collider's residency: square tiles of one cell size,
//! resident about one or more focuses, sampled by the product and meshed
//! into the triangles the finest drawn ring would draw there
//! (`heightfield::surface_triangles`). What holds the triangles is the
//! caller's; this keeps which tiles are wanted, waiting, on their way and live.

use std::collections::BTreeMap;

use crate::heightfield::surface_triangles;

/// A collider's tiles: `tile_cells` cells a side (1 to 256) of `cell_size`
/// units, resident within `reach` units of a focus, rounded out to whole
/// tiles.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColliderLayout {
    pub tile_cells: u32,
    pub cell_size: f64,
    pub reach: f64,
}

impl ColliderLayout {
    pub fn valid(&self) -> bool {
        (1..=256).contains(&self.tile_cells)
            && self.cell_size.is_finite()
            && self.cell_size > 0.0
            && self.reach.is_finite()
            && self.reach >= 0.0
    }

    /// A tile's side, in units.
    pub fn tile_side(&self) -> f64 {
        self.cell_size * f64::from(self.tile_cells)
    }

    /// Heights along a tile's side, as a drawn tile takes them.
    pub fn height_side(&self) -> usize {
        self.tile_cells as usize + 3
    }

    pub fn origin(&self, tile: (i64, i64)) -> [f64; 2] {
        [
            tile.0 as f64 * self.tile_side(),
            tile.1 as f64 * self.tile_side(),
        ]
    }

    /// The tiles within `reach` of `point`, as (lowest, highest) corners.
    fn around(&self, point: [f64; 2], reach: f64) -> ((i64, i64), (i64, i64)) {
        let tile = |value: f64| (value / self.tile_side()).floor() as i64;
        (
            (tile(point[0] - reach), tile(point[1] - reach)),
            (tile(point[0] + reach), tile(point[1] + reach)),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ColliderTile {
    id: u64,
    revision: u32,
    /// New samples are asked for; earlier ones may still stand.
    waiting: bool,
    /// Its collision was handed over and not yet removed.
    submitted: bool,
    /// The ground stands: its collision is in the scene, or it has none.
    live: bool,
    refused: bool,
}

/// A tile's triangles, in its own space (its first vertex at the origin).
#[derive(Debug, Clone, PartialEq)]
pub struct ColliderGeometry {
    pub id: u64,
    pub origin: [f64; 2],
    pub positions: Vec<[f64; 3]>,
    pub triangles: Vec<[u32; 3]>,
}

/// A tile as a batch carries it: its place and which tile stood there, since
/// a tile that leaves and returns before its batch installs is a new one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CarriedTile {
    pub tile: (i64, i64),
    pub id: u64,
}

/// What has changed since the last batch: collision to install and remove,
/// and the tiles to report back to [`ColliderTiles::finished`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ColliderBatch {
    pub geometry: Vec<ColliderGeometry>,
    pub removed: Vec<u64>,
    pub tiles: Vec<CarriedTile>,
}

/// A tile waiting for samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColliderRequest {
    pub tile: (i64, i64),
    pub revision: u32,
}

/// The tiles held, as a readout counts them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ColliderCounts {
    /// Whose ground stands.
    pub live: u32,
    /// A focus wants, whose ground does not stand yet.
    pub pending: u32,
    /// Whose collision was refused.
    pub refused: u32,
}

#[derive(Debug, Clone)]
pub struct ColliderTiles {
    layout: ColliderLayout,
    focuses: Vec<[f64; 2]>,
    tiles: BTreeMap<(i64, i64), ColliderTile>,
    first_id: u64,
    next_id: u64,
    next_revision: u32,
    /// Sampled and not yet handed over: None where the tile is all holes.
    sampled: BTreeMap<(i64, i64), Option<ColliderGeometry>>,
    removed: Vec<u64>,
}

impl ColliderTiles {
    /// Tiles take ids from `first_id` up.
    pub fn new(layout: ColliderLayout, first_id: u64) -> Self {
        Self {
            layout,
            focuses: Vec::new(),
            tiles: BTreeMap::new(),
            first_id,
            next_id: first_id,
            next_revision: 1,
            sampled: BTreeMap::new(),
            removed: Vec::new(),
        }
    }

    pub fn layout(&self) -> &ColliderLayout {
        &self.layout
    }

    pub fn first_id(&self) -> u64 {
        self.first_id
    }

    fn revision(&mut self) -> u32 {
        let revision = self.next_revision;
        self.next_revision = self.next_revision.wrapping_add(1).max(1);
        revision
    }

    fn wanted(&self, tile: (i64, i64), margin: i64) -> bool {
        self.focuses.iter().any(|focus| {
            let (low, high) = self.layout.around(*focus, self.layout.reach);
            (low.0 - margin..=high.0 + margin).contains(&tile.0)
                && (low.1 - margin..=high.1 + margin).contains(&tile.1)
        })
    }

    /// Centre the tiles on `focuses`. A tile stays a tile past its region,
    /// so a focus wavering on a boundary samples nothing twice.
    pub fn set_focuses(&mut self, focuses: Vec<[f64; 2]>) {
        self.focuses = focuses;
        for focus in self.focuses.clone() {
            let (low, high) = self.layout.around(focus, self.layout.reach);
            for z in low.1..=high.1 {
                for x in low.0..=high.0 {
                    if !self.tiles.contains_key(&(x, z)) {
                        let revision = self.revision();
                        let id = self.next_id;
                        self.next_id += 1;
                        self.tiles.insert(
                            (x, z),
                            ColliderTile {
                                id,
                                revision,
                                waiting: true,
                                submitted: false,
                                live: false,
                                refused: false,
                            },
                        );
                    }
                }
            }
        }
        let leaving: Vec<(i64, i64)> = self
            .tiles
            .keys()
            .copied()
            .filter(|tile| !self.wanted(*tile, 1))
            .collect();
        for tile in leaving {
            let held = self.tiles.remove(&tile).expect("listed above");
            self.sampled.remove(&tile);
            if held.submitted {
                self.removed.push(held.id);
            }
        }
    }

    /// The tiles a focus wants without samples, nearest a focus first, at
    /// most `max` (0: all).
    pub fn requests(&self, max: usize) -> Vec<ColliderRequest> {
        let side = self.layout.tile_side();
        let distance = |tile: (i64, i64)| {
            let centre = self.layout.origin(tile).map(|value| value + side * 0.5);
            self.focuses
                .iter()
                .map(|focus| {
                    (focus[0] - centre[0])
                        .abs()
                        .max((focus[1] - centre[1]).abs())
                })
                .fold(f64::INFINITY, f64::min)
        };
        let mut waiting: Vec<ColliderRequest> = self
            .tiles
            .iter()
            .filter(|(tile, held)| held.waiting && self.wanted(**tile, 0))
            .map(|(tile, held)| ColliderRequest {
                tile: *tile,
                revision: held.revision,
            })
            .collect();
        waiting.sort_by(|a, b| distance(a.tile).total_cmp(&distance(b.tile)));
        if max != 0 {
            waiting.truncate(max);
        }
        waiting
    }

    /// Take a tile's heights (`height_side()`² from vertex (−1, −1), NaN no
    /// surface). False for a revision no longer the tile's.
    pub fn commit(&mut self, tile: (i64, i64), revision: u32, heights: &[f32]) -> bool {
        let layout = self.layout;
        let Some(held) = self
            .tiles
            .get_mut(&tile)
            .filter(|held| held.waiting && held.revision == revision)
        else {
            return false;
        };
        held.waiting = false;
        let id = held.id;
        let geometry = surface_triangles(layout.tile_cells as usize, layout.cell_size, heights)
            .map(|(positions, triangles)| ColliderGeometry {
                id,
                origin: layout.origin(tile),
                positions,
                triangles,
            });
        self.sampled.insert(tile, geometry);
        true
    }

    /// Ask again for every held tile overlapping the rectangle; each keeps
    /// its ground until its new samples land.
    pub fn invalidate(&mut self, min: [f64; 2], max: [f64; 2]) {
        let side = self.layout.tile_side();
        let tiles: Vec<(i64, i64)> = self
            .tiles
            .keys()
            .copied()
            .filter(|tile| {
                let origin = self.layout.origin(*tile);
                origin[0] <= max[0]
                    && origin[0] + side >= min[0]
                    && origin[1] <= max[1]
                    && origin[1] + side >= min[1]
            })
            .collect();
        for tile in tiles {
            let revision = self.revision();
            let held = self.tiles.get_mut(&tile).expect("listed above");
            held.revision = revision;
            held.waiting = true;
            self.sampled.remove(&tile);
        }
    }

    pub fn has_batch(&self) -> bool {
        !self.sampled.is_empty() || !self.removed.is_empty()
    }

    /// What has changed since the last call. A tile that is all holes
    /// stands at once, its earlier collision removed.
    pub fn take_batch(&mut self) -> ColliderBatch {
        let mut batch = ColliderBatch {
            removed: std::mem::take(&mut self.removed),
            ..ColliderBatch::default()
        };
        for (tile, geometry) in std::mem::take(&mut self.sampled) {
            let held = self.tiles.get_mut(&tile).expect("a sampled tile is held");
            held.refused = false;
            match geometry {
                Some(geometry) => {
                    held.submitted = true;
                    batch.tiles.push(CarriedTile { tile, id: held.id });
                    batch.geometry.push(geometry);
                }
                None => {
                    if std::mem::take(&mut held.submitted) {
                        batch.removed.push(held.id);
                    }
                    held.live = true;
                }
            }
        }
        batch
    }

    /// Batch `tiles` installed, or was refused. A tile retired since says
    /// nothing of whatever stands in its place now.
    pub fn finished(&mut self, tiles: &[CarriedTile], installed: bool) {
        for carried in tiles {
            if let Some(held) = self
                .tiles
                .get_mut(&carried.tile)
                .filter(|held| held.id == carried.id)
            {
                held.live |= installed;
                held.refused = !installed;
            }
        }
    }

    /// Whether the ground stands on every tile within `reach` of `point`.
    pub fn ready(&self, point: [f64; 2], reach: f64) -> bool {
        let (low, high) = self.layout.around(point, reach);
        (low.1..=high.1).all(|z| {
            (low.0..=high.0).all(|x| self.tiles.get(&(x, z)).is_some_and(|held| held.live))
        })
    }

    pub fn counts(&self) -> ColliderCounts {
        let mut counts = ColliderCounts::default();
        for (tile, held) in &self.tiles {
            counts.live += u32::from(held.live);
            counts.refused += u32::from(held.refused);
            counts.pending += u32::from(!held.live && !held.refused && self.wanted(*tile, 0));
        }
        counts
    }
}

#[cfg(test)]
#[path = "heightfield_collider_tests.rs"]
mod tests;
