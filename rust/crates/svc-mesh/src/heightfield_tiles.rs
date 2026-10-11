//! Which tiles a heightfield holds, asks for, meshes and draws (#9842).
//!
//! Each ring wants the tiles of its region about the focus and keeps them a
//! region tile beyond, so a focus wavering on a boundary samples nothing
//! twice. A wanted tile without current samples is requested, coarsest ring
//! first and nearest first; every request carries a revision unique to the
//! heightfield, renewed when the tile is invalidated or comes back after
//! leaving, so samples worked out before either are ignored. A tile is
//! meshed once it has samples and, below the coarsest ring, its coarser
//! tile has them too (its morph targets lie on that surface); new samples
//! remesh the tile and the finer tiles within it. A coarser tile stays
//! drawn until every finer tile within it is meshed, then gives way to
//! them all at once, so nothing overlaps and nothing goes missing.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Instant;

use crate::heightfield::{
    mesh_tile, HeightfieldLayout, TileHit, TileKey, TileMesh, TileRect, TileSamples, TileSurface,
};

#[derive(Debug, Clone, Default)]
struct Tile {
    revision: u32,
    /// The samples last committed, and the revision they answered.
    surface: Option<(u32, Arc<TileSurface>)>,
    mesh: Option<Arc<TileMesh>>,
    /// Its samples or its coarser tile's changed since it was meshed.
    remesh: bool,
}

/// What changed for the renderer since the last [`HeightfieldTiles::take_changes`].
/// A tile shown and hidden again within the span is in both lists: draw each
/// as [`HeightfieldTiles::is_drawn`] says.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HeightfieldChanges {
    /// Tiles meshed anew or again (a meshless result removes the mesh).
    pub meshed: Vec<TileKey>,
    /// Tiles now drawn, and tiles no longer drawn but still held.
    pub shown: Vec<TileKey>,
    pub hidden: Vec<TileKey>,
    /// Tiles let go, meshes and all.
    pub evicted: Vec<TileKey>,
}

/// A tile to mesh: its samples, and its coarser tile's with where it stands.
type MeshJob = (
    TileKey,
    Arc<TileSurface>,
    Option<(Arc<TileSurface>, [f64; 2])>,
);

/// A tile to sample.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeightfieldRequest {
    pub key: TileKey,
    pub revision: u32,
}

/// A heightfield's tiles.
#[derive(Debug, Clone)]
pub struct HeightfieldTiles {
    layout: HeightfieldLayout,
    focus: Option<[f64; 2]>,
    tiles: BTreeMap<TileKey, Tile>,
    drawn: BTreeSet<TileKey>,
    next_revision: u32,
    changes: HeightfieldChanges,
    /// The last meshing's summed time and tile count.
    pub mesh_microseconds: u64,
    pub meshed_tiles: u32,
}

impl HeightfieldTiles {
    pub fn new(layout: HeightfieldLayout) -> Self {
        Self {
            layout,
            focus: None,
            tiles: BTreeMap::new(),
            drawn: BTreeSet::new(),
            next_revision: 1,
            changes: HeightfieldChanges::default(),
            mesh_microseconds: 0,
            meshed_tiles: 0,
        }
    }

    pub fn layout(&self) -> &HeightfieldLayout {
        &self.layout
    }

    pub fn focus(&self) -> Option<[f64; 2]> {
        self.focus
    }

    fn revision(&mut self) -> u32 {
        let revision = self.next_revision;
        self.next_revision = self.next_revision.wrapping_add(1).max(1);
        revision
    }

    fn regions(&self, focus: [f64; 2]) -> Vec<TileRect> {
        (0..self.layout.rings.len())
            .map(|ring| self.layout.region(ring, focus))
            .collect()
    }

    fn wanted(&self, key: TileKey) -> bool {
        self.focus.is_some_and(|focus| {
            self.layout
                .region(usize::from(key.ring), focus)
                .contains(key.x, key.z)
        })
    }

    /// Centre the rings on `focus` (None: want nothing, keep what is held).
    pub fn set_focus(&mut self, focus: Option<[f64; 2]>) {
        if focus == self.focus {
            return;
        }
        self.focus = focus;
        let Some(focus) = focus else {
            return;
        };
        let regions = self.regions(focus);
        for (ring, region) in regions.iter().enumerate() {
            for (x, z) in region.tiles() {
                let key = TileKey {
                    ring: ring as u8,
                    x,
                    z,
                };
                if !self.tiles.contains_key(&key) {
                    let revision = self.revision();
                    self.tiles.insert(
                        key,
                        Tile {
                            revision,
                            ..Tile::default()
                        },
                    );
                }
            }
        }
        let gone: Vec<TileKey> = self
            .tiles
            .keys()
            .filter(|key| {
                let ring = usize::from(key.ring);
                let kept = regions[ring].grown(self.layout.rings[ring].per_parent);
                !kept.contains(key.x, key.z)
            })
            .copied()
            .collect();
        for key in gone {
            if self
                .tiles
                .remove(&key)
                .is_some_and(|tile| tile.mesh.is_some())
            {
                self.changes.evicted.push(key);
            }
            self.drawn.remove(&key);
        }
    }

    /// Wanted tiles without current samples, coarsest ring first and
    /// nearest the focus first; `max` of them (0: all).
    pub fn requests(&self, max: usize) -> Vec<HeightfieldRequest> {
        let Some(focus) = self.focus else {
            return Vec::new();
        };
        let mut waiting: Vec<(std::cmp::Reverse<u8>, f64, HeightfieldRequest)> = self
            .tiles
            .iter()
            .filter(|(key, tile)| {
                self.wanted(**key)
                    && tile.surface.as_ref().map(|(revision, _)| *revision) != Some(tile.revision)
            })
            .map(|(key, tile)| {
                let size = self.layout.rings[usize::from(key.ring)].tile_size;
                let origin = self.layout.origin(*key);
                let reach = (origin[0] + size / 2.0 - focus[0])
                    .abs()
                    .max((origin[1] + size / 2.0 - focus[1]).abs());
                (
                    std::cmp::Reverse(key.ring),
                    reach,
                    HeightfieldRequest {
                        key: *key,
                        revision: tile.revision,
                    },
                )
            })
            .collect();
        waiting.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then(a.1.total_cmp(&b.1))
                .then(a.2.key.cmp(&b.2.key))
        });
        let limit = if max == 0 { waiting.len() } else { max };
        waiting
            .into_iter()
            .take(limit)
            .map(|(_, _, request)| request)
            .collect()
    }

    /// Take a tile's samples. False, and nothing changes, when the revision
    /// is not the tile's (invalidated since, or no longer held).
    pub fn commit(&mut self, key: TileKey, revision: u32, samples: TileSamples) -> bool {
        let ring = usize::from(key.ring);
        let Some(tile) = self.tiles.get_mut(&key) else {
            return false;
        };
        if tile.revision != revision {
            return false;
        }
        tile.surface = Some((
            revision,
            Arc::new(TileSurface::new(&self.layout, ring, samples)),
        ));
        tile.remesh = true;
        let children: Vec<TileKey> = self.layout.children(key).collect();
        for child in children {
            if let Some(child) = self.tiles.get_mut(&child) {
                child.remesh = child.surface.is_some();
            }
        }
        true
    }

    /// Sample again every held tile overlapping the rectangle (heightfield
    /// units); each keeps drawing until its new samples land.
    pub fn invalidate(&mut self, min: [f64; 2], max: [f64; 2]) {
        let keys: Vec<TileKey> = self
            .tiles
            .keys()
            .filter(|key| {
                let size = self.layout.rings[usize::from(key.ring)].tile_size;
                let origin = self.layout.origin(**key);
                origin[0] <= max[0]
                    && origin[0] + size >= min[0]
                    && origin[1] <= max[1]
                    && origin[1] + size >= min[1]
            })
            .copied()
            .collect();
        for key in keys {
            let revision = self.revision();
            if let Some(tile) = self.tiles.get_mut(&key) {
                tile.revision = revision;
            }
        }
    }

    /// Mesh every tile that waits for it and can be (its coarser tile has
    /// samples), in parallel, then redraw.
    pub fn mesh(&mut self) {
        let jobs: Vec<MeshJob> = self
            .tiles
            .iter()
            .filter(|(_, tile)| tile.remesh)
            .filter_map(|(key, tile)| {
                let surface = tile.surface.as_ref()?.1.clone();
                let parent = match self.layout.parent(*key) {
                    Some(parent) => {
                        let coarser = self.tiles.get(&parent)?.surface.as_ref()?.1.clone();
                        Some((coarser, self.layout.origin(parent)))
                    }
                    None => None,
                };
                Some((*key, surface, parent))
            })
            .collect();
        let layout = &self.layout;
        let results = in_parallel(&jobs, |(key, surface, parent)| {
            let started = Instant::now();
            let mesh = mesh_tile(
                layout,
                *key,
                surface,
                parent.as_ref().map(|(coarser, at)| (coarser.as_ref(), *at)),
            );
            (
                *key,
                mesh.map(Arc::new),
                started.elapsed().as_micros() as u64,
            )
        });
        self.meshed_tiles = results.len() as u32;
        self.mesh_microseconds = results.iter().map(|result| result.2).sum();
        for (key, mesh, _) in results {
            if let Some(tile) = self.tiles.get_mut(&key) {
                tile.remesh = false;
                tile.mesh = mesh;
                self.changes.meshed.push(key);
            }
        }
        self.redraw();
    }

    /// Every finer tile within `key` is meshed (or is a hole).
    fn complete(&self, key: TileKey) -> bool {
        self.layout.children(key).all(|child| {
            self.tiles
                .get(&child)
                .is_some_and(|tile| tile.mesh.is_some() || (tile.surface.is_some() && !tile.remesh))
        })
    }

    fn redraw(&mut self) {
        let Some(focus) = self.focus else {
            return;
        };
        let regions = self.regions(focus);
        let top = self.layout.rings.len() - 1;
        let mut drawn = BTreeSet::new();
        for (key, tile) in &self.tiles {
            let ring = usize::from(key.ring);
            if tile.mesh.is_none() || !regions[ring].contains(key.x, key.z) {
                continue;
            }
            if ring < top
                && !self
                    .layout
                    .parent(*key)
                    .is_some_and(|parent| self.complete(parent))
            {
                continue;
            }
            let covered = ring > 0 && {
                let k = self.layout.rings[ring - 1].per_parent;
                let finer = &regions[ring - 1];
                finer.contains(key.x * k, key.z * k)
                    && finer.contains(key.x * k + k - 1, key.z * k + k - 1)
                    && self.complete(*key)
            };
            if !covered {
                drawn.insert(*key);
            }
        }
        self.changes
            .shown
            .extend(drawn.difference(&self.drawn).copied());
        self.changes.hidden.extend(
            self.drawn
                .difference(&drawn)
                .filter(|key| self.tiles.contains_key(key))
                .copied(),
        );
        self.drawn = drawn;
    }

    /// What changed for the renderer since the last call. A tile meshed and
    /// shown in the same span is listed in both.
    pub fn take_changes(&mut self) -> HeightfieldChanges {
        self.redraw();
        std::mem::take(&mut self.changes)
    }

    pub fn mesh_of(&self, key: TileKey) -> Option<&Arc<TileMesh>> {
        self.tiles.get(&key)?.mesh.as_ref()
    }

    pub fn is_drawn(&self, key: TileKey) -> bool {
        self.drawn.contains(&key)
    }

    pub fn drawn(&self) -> impl Iterator<Item = TileKey> + '_ {
        self.drawn.iter().copied()
    }

    /// Tiles holding samples.
    pub fn resident(&self) -> usize {
        self.tiles
            .values()
            .filter(|tile| tile.surface.is_some())
            .count()
    }

    /// Every wanted tile has current samples and nothing waits to mesh.
    pub fn settled(&self) -> bool {
        self.focus.is_some()
            && self.requests(1).is_empty()
            && !self
                .tiles
                .iter()
                .any(|(key, tile)| tile.remesh && self.wanted(*key))
    }

    /// The samples and meshes held, in bytes, and the meshes' vertices and
    /// indices.
    pub fn footprint(&self) -> HeightfieldFootprint {
        let mut footprint = HeightfieldFootprint::default();
        for tile in self.tiles.values() {
            if let Some((_, surface)) = &tile.surface {
                let samples = surface.samples();
                footprint.sample_bytes += (samples.heights.len() * 4
                    + samples
                        .colors
                        .as_ref()
                        .map_or(0, |values| values.len() * 16)
                    + samples
                        .layer_weights
                        .as_ref()
                        .map_or(0, |values| values.len() * 16)
                    + samples
                        .materials
                        .as_ref()
                        .map_or(0, |values| values.len() * 4))
                    as u64;
            }
            if let Some(mesh) = &tile.mesh {
                let vertices = mesh.positions.len() / 3;
                footprint.vertices += vertices as u64;
                footprint.indices += mesh.indices.len() as u64;
                let floats = mesh.positions.len()
                    + mesh.normals.len()
                    + mesh.uvs.len()
                    + mesh.colors.as_ref().map_or(0, Vec::len)
                    + mesh.morph.as_ref().map_or(0, Vec::len);
                footprint.mesh_bytes += (floats * 4 + mesh.indices.len() * 4) as u64;
            }
        }
        footprint
    }

    /// Where a ray (heightfield units) first meets the drawn surface, its
    /// vertices morphed as the renderer morphs them about the focus.
    pub fn pick(&self, origin: [f64; 3], direction: [f64; 3]) -> Option<(TileKey, TileHit)> {
        let focus = self.focus;
        let mut candidates: Vec<(f64, TileKey, &Arc<TileMesh>)> = self
            .drawn
            .iter()
            .filter_map(|key| {
                let mesh = self.tiles.get(key)?.mesh.as_ref()?;
                let at = self.layout.origin(*key);
                let min = [
                    at[0] + f64::from(mesh.min[0]),
                    f64::from(mesh.min[1]),
                    at[1] + f64::from(mesh.min[2]),
                ];
                let max = [
                    at[0] + f64::from(mesh.max[0]),
                    f64::from(mesh.max[1]),
                    at[1] + f64::from(mesh.max[2]),
                ];
                ray_box(origin, direction, min, max).map(|enter| (enter, *key, mesh))
            })
            .collect();
        candidates.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut best: Option<(TileKey, TileHit)> = None;
        for (enter, key, mesh) in candidates {
            if best.is_some_and(|(_, hit)| hit.distance < enter) {
                break;
            }
            let at = self.layout.origin(key);
            let morph = self.layout.rings[usize::from(key.ring)].morph;
            let share = |x: f64, z: f64| match (focus, morph) {
                (Some(focus), Some((start, end))) => {
                    let reach = (at[0] + x - focus[0])
                        .abs()
                        .max((at[1] + z - focus[1]).abs());
                    ((reach - start) / (end - start)).clamp(0.0, 1.0)
                }
                _ => 0.0,
            };
            let local = [origin[0] - at[0], origin[1], origin[2] - at[1]];
            if let Some(mut hit) = mesh.pick(local, direction, share) {
                hit.point[0] += at[0];
                hit.point[2] += at[1];
                if best.is_none_or(|(_, current)| hit.distance < current.distance) {
                    best = Some((key, hit));
                }
            }
        }
        best
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HeightfieldFootprint {
    pub sample_bytes: u64,
    pub mesh_bytes: u64,
    pub vertices: u64,
    pub indices: u64,
}

/// Where a ray enters a box (0 if it starts inside), or None if it misses.
fn ray_box(origin: [f64; 3], direction: [f64; 3], min: [f64; 3], max: [f64; 3]) -> Option<f64> {
    let (mut enter, mut leave) = (0.0_f64, f64::INFINITY);
    for axis in 0..3 {
        if direction[axis].abs() < 1e-12 {
            if origin[axis] < min[axis] || origin[axis] > max[axis] {
                return None;
            }
        } else {
            let a = (min[axis] - origin[axis]) / direction[axis];
            let b = (max[axis] - origin[axis]) / direction[axis];
            enter = enter.max(a.min(b));
            leave = leave.min(a.max(b));
        }
    }
    (enter <= leave).then_some(enter)
}

/// `work` over `items` on scoped threads, a few items each, in order.
fn in_parallel<T: Sync, R: Send>(items: &[T], work: impl Fn(&T) -> R + Sync) -> Vec<R> {
    const PER_THREAD: usize = 2;
    let threads = std::thread::available_parallelism()
        .map_or(1, |cores| cores.get())
        .min(items.len().div_ceil(PER_THREAD));
    if threads <= 1 {
        return items.iter().map(work).collect();
    }
    let chunk = items.len().div_ceil(threads);
    let work = &work;
    std::thread::scope(|scope| {
        let handles: Vec<_> = items
            .chunks(chunk)
            .map(|slice| scope.spawn(move || slice.iter().map(work).collect::<Vec<R>>()))
            .collect();
        handles
            .into_iter()
            .flat_map(|handle| handle.join().expect("heightfield meshing panicked"))
            .collect()
    })
}

#[cfg(test)]
#[path = "heightfield_tiles_tests.rs"]
mod tests;
