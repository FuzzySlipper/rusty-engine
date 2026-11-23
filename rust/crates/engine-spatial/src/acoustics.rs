//! How sound meets a collision scene: the solid a straight line crosses,
//! the shortest open-air path round it, and how enclosed a point is.
//!
//! These are geometric readings of the scene's collidable voxels and static
//! meshes; the audio model that turns them into level, filtering and reverb
//! lives with the audio service. Entity boxes take no part.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use core_space::{ChunkCoord, VoxelCoord, WorldPos, WorldVec};
use svc_collision::Ray;
use svc_volume::VoxelChunk;

use crate::VoxelCollisionScene;

/// The side of one cell of an open-air search, in metres.
pub const AIR_CELL_METRES: f64 = 1.0;
/// The most cells one open-air search settles.
pub const AIR_SEARCH_CELLS: usize = 4096;
/// How far a path is followed from either end to read the way it leaves.
const PATH_BEARING_METRES: f64 = 3.0;
/// How far an enclosure ray looks.
pub const ENCLOSURE_METRES: f64 = 24.0;
/// The most static-mesh surfaces one solid measurement crosses.
const MESH_CROSSINGS: usize = 16;
/// How far past a surface a measurement starts its next cast.
const MESH_STEP: f64 = 1e-3;
const NO_CELL: u32 = u32::MAX;

/// The six neighbours of an air cell.
const FACES: [[i64; 3]; 6] = [
    [1, 0, 0],
    [-1, 0, 0],
    [0, 1, 0],
    [0, -1, 0],
    [0, 0, 1],
    [0, 0, -1],
];

/// How enclosed a point is: the share of its level and upward rays that meet
/// collision within [`ENCLOSURE_METRES`], and their mean free length.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Enclosure {
    pub closed: f64,
    pub mean_free_metres: f64,
}

impl VoxelCollisionScene {
    /// Metres of collidable solid between `from` and `to`: collidable voxels
    /// and static-mesh interiors, where they overlap counted once.
    pub fn solid_metres(&self, from: [f64; 3], to: [f64; 3]) -> f64 {
        let Some((direction, length)) = unit(sub(to, from)) else {
            return 0.0;
        };
        let mut spans = Vec::new();
        self.voxel_spans(from, direction, length, false, &mut spans);
        self.mesh_spans(from, direction, length, &mut spans);
        union_length(&mut spans)
    }

    /// Whether the segment from `from` to `to` meets nothing collidable.
    pub fn segment_clear(&self, from: [f64; 3], to: [f64; 3]) -> bool {
        let Some((direction, length)) = unit(sub(to, from)) else {
            return true;
        };
        let mut spans = Vec::new();
        self.voxel_spans(from, direction, length, true, &mut spans);
        spans.is_empty()
            && self
                .static_mesh_projection()
                .raycast(ray(from, direction), length)
                .is_none()
    }

    /// How enclosed `at` is, from thirteen rays: straight up, eight level
    /// bearings and four rising ones. Ground below does not count.
    pub fn enclosure(&self, at: [f64; 3]) -> Enclosure {
        const LEVEL: f64 = std::f64::consts::FRAC_1_SQRT_2;
        let mut directions = vec![[0.0, 1.0, 0.0]];
        for step in 0..8 {
            let angle = f64::from(step) * std::f64::consts::FRAC_PI_4;
            directions.push([angle.cos(), 0.0, angle.sin()]);
        }
        for step in 0..4 {
            let angle = (f64::from(step) + 0.5) * std::f64::consts::FRAC_PI_2;
            directions.push([angle.cos() * LEVEL, LEVEL, angle.sin() * LEVEL]);
        }
        let mut hits = 0_u32;
        let mut free = 0.0;
        for direction in &directions {
            match self.raycast_world(at, *direction, ENCLOSURE_METRES) {
                Some(hit) => {
                    hits += 1;
                    free += hit_distance(&hit);
                }
                None => free += ENCLOSURE_METRES,
            }
        }
        let rays = directions.len() as f64;
        Enclosure {
            closed: f64::from(hits) / rays,
            mean_free_metres: free / rays,
        }
    }

    /// Spans of collidable voxels along the ray, as distances from `from`,
    /// walked voxel by voxel. `first` stops at the first one.
    fn voxel_spans(
        &self,
        from: [f64; 3],
        direction: [f64; 3],
        length: f64,
        first: bool,
        spans: &mut Vec<(f64, f64)>,
    ) {
        let grid = self.voxel_world.grid();
        let size = self.voxel_size;
        let start = grid.world_to_voxel(WorldPos::new(from[0], from[1], from[2]));
        let corner = grid.voxel_min_world(start);
        let corner = [corner.x, corner.y, corner.z];
        let mut cell = [start.x, start.y, start.z];
        let mut step = [0_i64; 3];
        let mut next = [f64::INFINITY; 3];
        let mut delta = [f64::INFINITY; 3];
        for axis in 0..3 {
            if direction[axis] > 0.0 {
                step[axis] = 1;
                next[axis] = (corner[axis] + size - from[axis]) / direction[axis];
                delta[axis] = size / direction[axis];
            } else if direction[axis] < 0.0 {
                step[axis] = -1;
                next[axis] = (corner[axis] - from[axis]) / direction[axis];
                delta[axis] = -size / direction[axis];
            }
        }
        let mut chunk: Option<(ChunkCoord, Option<&VoxelChunk>)> = None;
        let mut entered: Option<f64> = None;
        let mut at = 0.0;
        loop {
            let (coordinate, local) =
                grid.voxel_to_chunk_local(VoxelCoord::new(cell[0], cell[1], cell[2]));
            let resident = match chunk {
                Some((held, resident)) if held == coordinate => resident,
                _ => {
                    let resident = self.voxel_world.get(coordinate);
                    chunk = Some((coordinate, resident));
                    resident
                }
            };
            let solid = resident.is_some_and(|resident| {
                resident.get(local).is_some_and(|value| {
                    value.material().is_some_and(|material| {
                        !self.noncollidable_materials.contains(&material.raw())
                    })
                })
            });
            match (solid, entered) {
                (true, None) => {
                    entered = Some(at);
                    if first {
                        break;
                    }
                }
                (false, Some(start)) => {
                    spans.push((start, at));
                    entered = None;
                }
                _ => {}
            }
            let axis = if next[0] <= next[1] && next[0] <= next[2] {
                0
            } else if next[1] <= next[2] {
                1
            } else {
                2
            };
            if next[axis] >= length {
                break;
            }
            at = next[axis];
            cell[axis] += step[axis];
            next[axis] += delta[axis];
        }
        if let Some(start) = entered {
            spans.push((start, if first { start } else { length }));
        }
    }

    /// Spans inside static meshes along the ray: each surface met enters or
    /// leaves by which way it faces.
    fn mesh_spans(
        &self,
        from: [f64; 3],
        direction: [f64; 3],
        length: f64,
        spans: &mut Vec<(f64, f64)>,
    ) {
        let meshes = self.static_mesh_projection();
        let mut at = 0.0;
        let mut entered: Option<f64> = None;
        for _ in 0..MESH_CROSSINGS {
            let origin = add(from, scale(direction, at));
            let Some(hit) = meshes.raycast(ray(origin, direction), length - at) else {
                break;
            };
            let surface = at + hit.distance;
            let facing = hit.normal.x * direction[0]
                + hit.normal.y * direction[1]
                + hit.normal.z * direction[2];
            if facing < 0.0 {
                entered.get_or_insert(surface);
            } else {
                // Leaving a mesh not seen entered: the segment began inside.
                spans.push((entered.take().unwrap_or(0.0), surface));
            }
            at = surface + MESH_STEP;
            if at >= length {
                break;
            }
        }
        if let Some(start) = entered {
            spans.push((start, length));
        }
    }
}

/// The shortest open-air paths from one root point through a scene, over a
/// lattice of [`AIR_CELL_METRES`] cells joined where the segment between
/// their centres is clear. Distances solve the eikonal equation over the
/// lattice (a fast-marching search), so a diagonal path measures close to
/// its straight length rather than its steps.
pub struct AirSearch {
    root: [f64; 3],
    cells: Vec<AirCell>,
    index: HashMap<[i64; 3], u32>,
    settled: usize,
    /// The search stopped at [`AIR_SEARCH_CELLS`] with cells still open.
    pub truncated: bool,
    /// Segments tested for the search.
    pub segments: u32,
}

struct AirCell {
    key: [i64; 3],
    metres: f64,
    /// The settled neighbour this cell's distance came through.
    parent: u32,
    /// The cell about [`PATH_BEARING_METRES`] along this cell's path from the root.
    anchor: u32,
    /// Neighbours (by [`FACES`] bit) whose shared segment was tested; and of
    /// them, the clear ones.
    tested: u8,
    /// Each tested neighbour's cell, by [`FACES`] index.
    neighbours: [u32; 6],
    linked: u8,
    settled: bool,
}

/// Where a point is reached by open air.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AirReach {
    /// The path's length from the root to the point.
    pub metres: f64,
    /// A point about three metres along the path from the root: the way the
    /// path leaves the root.
    pub near_root: [f64; 3],
    /// A point about three metres back along the path from the point: the
    /// way the path arrives.
    pub near_point: [f64; 3],
}

/// A cell on the search's front: its order (its distance, plus with
/// targets the straight distance on to the nearest), its distance, and it.
#[derive(PartialEq)]
struct Front(f64, f64, u32);

impl Eq for Front {}

impl PartialOrd for Front {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Front {
    // The nearest first from a max-heap.
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .0
            .total_cmp(&self.0)
            .then_with(|| other.2.cmp(&self.2))
    }
}

impl AirSearch {
    /// Searches open air from `root` out to `max_metres` of path, settling
    /// at most [`AIR_SEARCH_CELLS`] cells. With `targets`, it searches
    /// toward them first (an A* order by the straight distance on to the
    /// nearest) and stops once every target's cell is settled or out of
    /// reach.
    pub fn run(
        scene: &VoxelCollisionScene,
        root: [f64; 3],
        max_metres: f64,
        targets: &[[f64; 3]],
    ) -> Self {
        let mut search = Self {
            root,
            cells: Vec::new(),
            index: HashMap::new(),
            settled: 0,
            truncated: false,
            segments: 0,
        };
        let mut front = BinaryHeap::new();
        let home = cell_of(root);
        for dz in -1..=1 {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let key = [home[0] + dx, home[1] + dy, home[2] + dz];
                    let metres = distance(root, centre(key));
                    if metres > AIR_CELL_METRES * 1.75 {
                        continue;
                    }
                    search.segments += 1;
                    if !scene.segment_clear(root, centre(key)) {
                        continue;
                    }
                    let id = search.cell(key);
                    let cell = &mut search.cells[id as usize];
                    cell.metres = metres;
                    cell.anchor = id;
                    front.push(Front(metres + toward(targets, key), metres, id));
                }
            }
        }
        let target_keys: Vec<[i64; 3]> = targets.iter().map(|target| cell_of(*target)).collect();
        while let Some(Front(order, metres, id)) = front.pop() {
            let cell = &search.cells[id as usize];
            if cell.settled || metres > cell.metres {
                continue;
            }
            // No path on from here can be shorter than its order.
            if order > max_metres {
                break;
            }
            if search.settled >= AIR_SEARCH_CELLS {
                search.truncated = true;
                break;
            }
            search.cells[id as usize].settled = true;
            search.settled += 1;
            let key = search.cells[id as usize].key;
            for (face, offset) in FACES.iter().enumerate() {
                let neighbour_key = [key[0] + offset[0], key[1] + offset[1], key[2] + offset[2]];
                let neighbour = search.cell(neighbour_key);
                if search.cells[neighbour as usize].settled {
                    continue;
                }
                let bit = 1_u8 << face;
                if search.cells[id as usize].tested & bit == 0 {
                    search.segments += 1;
                    let clear = scene.segment_clear(centre(key), centre(neighbour_key));
                    let back = 1_u8 << (face ^ 1);
                    search.cells[id as usize].tested |= bit;
                    search.cells[neighbour as usize].tested |= back;
                    search.cells[id as usize].neighbours[face] = neighbour;
                    search.cells[neighbour as usize].neighbours[face ^ 1] = id;
                    if clear {
                        search.cells[id as usize].linked |= bit;
                        search.cells[neighbour as usize].linked |= back;
                    }
                }
                if search.cells[id as usize].linked & bit == 0 {
                    continue;
                }
                if let Some((value, parent)) = search.solve(neighbour) {
                    let cell = &mut search.cells[neighbour as usize];
                    if value < cell.metres {
                        cell.metres = value;
                        cell.parent = parent;
                        front.push(Front(
                            value + toward(targets, neighbour_key),
                            value,
                            neighbour,
                        ));
                    }
                }
            }
            if !target_keys.is_empty() && search.targets_settled(&target_keys, order) {
                break;
            }
        }
        // Anchors follow parents, which are final once settled.
        for id in 0..search.cells.len() {
            search.anchor(id as u32);
        }
        search
    }

    /// Where `point` is reached by open air within the searched cells: the
    /// best settled cell beside it with a clear segment to it.
    pub fn reach(&self, scene: &VoxelCollisionScene, point: [f64; 3]) -> Option<AirReach> {
        let home = cell_of(point);
        let mut candidates = Vec::new();
        for dz in -1..=1 {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let key = [home[0] + dx, home[1] + dy, home[2] + dz];
                    let Some(&id) = self.index.get(&key) else {
                        continue;
                    };
                    let cell = &self.cells[id as usize];
                    if cell.settled {
                        candidates.push((cell.metres + distance(centre(key), point), id));
                    }
                }
            }
        }
        candidates.sort_by(|left, right| left.0.total_cmp(&right.0));
        let (metres, id) = candidates
            .into_iter()
            .take(4)
            .find(|(_, id)| scene.segment_clear(centre(self.cells[*id as usize].key), point))?;
        let cell = &self.cells[id as usize];
        let near_root = centre(self.cells[cell.anchor as usize].key);
        let mut back = id;
        while self.cells[back as usize].parent != NO_CELL
            && cell.metres - self.cells[back as usize].metres < PATH_BEARING_METRES
        {
            back = self.cells[back as usize].parent;
        }
        let near_point = if cell.metres - self.cells[back as usize].metres < PATH_BEARING_METRES {
            self.root
        } else {
            centre(self.cells[back as usize].key)
        };
        Some(AirReach {
            metres,
            near_root,
            near_point,
        })
    }

    /// Cells settled by the search.
    pub fn settled_cells(&self) -> usize {
        self.settled
    }

    fn cell(&mut self, key: [i64; 3]) -> u32 {
        if let Some(&id) = self.index.get(&key) {
            return id;
        }
        let id = self.cells.len() as u32;
        self.cells.push(AirCell {
            key,
            metres: f64::INFINITY,
            parent: NO_CELL,
            anchor: NO_CELL,
            tested: 0,
            neighbours: [NO_CELL; 6],
            linked: 0,
            settled: false,
        });
        self.index.insert(key, id);
        id
    }

    /// The eikonal update of `id` from its settled, linked neighbours, and
    /// the nearest of them.
    fn solve(&self, id: u32) -> Option<(f64, u32)> {
        let cell = &self.cells[id as usize];
        let mut axes = [(f64::INFINITY, NO_CELL); 3];
        for face in 0..FACES.len() {
            if cell.linked & (1 << face) == 0 {
                continue;
            }
            let neighbour = cell.neighbours[face];
            let neighbour_cell = &self.cells[neighbour as usize];
            if neighbour_cell.settled && neighbour_cell.metres < axes[face / 2].0 {
                axes[face / 2] = (neighbour_cell.metres, neighbour);
            }
        }
        axes.sort_by(|left, right| left.0.total_cmp(&right.0));
        let (nearest, parent) = axes[0];
        if !nearest.is_finite() {
            return None;
        }
        let h = AIR_CELL_METRES;
        let mut value = nearest + h;
        let mut used = 1;
        while used < 3 && axes[used].0 < value {
            used += 1;
            let values = &axes[..used];
            let sum: f64 = values.iter().map(|axis| axis.0).sum();
            let squares: f64 = values.iter().map(|axis| axis.0 * axis.0).sum();
            let count = used as f64;
            let discriminant = sum * sum - count * (squares - h * h);
            if discriminant < 0.0 {
                break;
            }
            value = (sum + discriminant.sqrt()) / count;
        }
        Some((value, parent))
    }

    /// Gives a settled cell, and the cells its path runs through, their
    /// anchor: the cell where the path is [`PATH_BEARING_METRES`] from the root.
    fn anchor(&mut self, id: u32) {
        let mut chain = Vec::new();
        let mut at = id;
        let anchor = loop {
            let cell = &self.cells[at as usize];
            if cell.anchor != NO_CELL || !cell.settled {
                break cell.anchor;
            }
            if cell.parent == NO_CELL || cell.metres <= PATH_BEARING_METRES {
                break at;
            }
            chain.push(at);
            at = cell.parent;
        };
        self.cells[at as usize].anchor = anchor;
        for link in chain {
            self.cells[link as usize].anchor = anchor;
        }
    }

    /// Every target's cell and its neighbours are settled, or the front has
    /// passed beyond where any of them could still improve.
    fn targets_settled(&self, targets: &[[i64; 3]], front: f64) -> bool {
        targets.iter().all(|target| {
            let Some(&id) = self.index.get(target) else {
                return false;
            };
            let cell = &self.cells[id as usize];
            cell.settled && front > cell.metres + 2.0 * AIR_CELL_METRES
        })
    }
}

/// How far toward its targets a search leans: a full straight-distance
/// heuristic settles cells ahead of their nearer neighbours and lengthens
/// paths by a few percent, so it leans halfway.
const TARGET_LEAN: f64 = 0.5;

/// The straight distance from a cell's centre on to the nearest target, less
/// a cell's reach so it never exceeds the path that remains, leaned by
/// [`TARGET_LEAN`]; none without targets.
fn toward(targets: &[[f64; 3]], key: [i64; 3]) -> f64 {
    let nearest = targets
        .iter()
        .map(|target| distance(centre(key), *target))
        .fold(f64::INFINITY, f64::min);
    if nearest.is_finite() {
        (nearest - AIR_CELL_METRES).max(0.0) * TARGET_LEAN
    } else {
        0.0
    }
}

fn cell_of(point: [f64; 3]) -> [i64; 3] {
    point.map(|value| (value / AIR_CELL_METRES).floor() as i64)
}

fn centre(key: [i64; 3]) -> [f64; 3] {
    key.map(|value| (value as f64 + 0.5) * AIR_CELL_METRES)
}

fn hit_distance(hit: &crate::SpatialCollisionHit) -> f64 {
    match hit {
        crate::SpatialCollisionHit::Voxel(hit) => hit.distance,
        crate::SpatialCollisionHit::StaticMesh(hit) => hit.distance,
    }
}

fn ray(origin: [f64; 3], direction: [f64; 3]) -> Ray {
    Ray::new(
        WorldPos::new(origin[0], origin[1], origin[2]),
        WorldVec::new(direction[0], direction[1], direction[2]),
    )
}

fn union_length(spans: &mut [(f64, f64)]) -> f64 {
    spans.sort_by(|left, right| left.0.total_cmp(&right.0));
    let mut total = 0.0;
    let mut open: Option<(f64, f64)> = None;
    for &(start, end) in spans.iter() {
        match open {
            Some((from, to)) if start <= to => open = Some((from, to.max(end))),
            Some((from, to)) => {
                total += to - from;
                open = Some((start, end));
            }
            None => open = Some((start, end)),
        }
    }
    if let Some((from, to)) = open {
        total += to - from;
    }
    total
}

fn sub(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

fn add(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [left[0] + right[0], left[1] + right[1], left[2] + right[2]]
}

fn scale(value: [f64; 3], by: f64) -> [f64; 3] {
    value.map(|component| component * by)
}

fn distance(left: [f64; 3], right: [f64; 3]) -> f64 {
    let difference = sub(left, right);
    (difference[0] * difference[0] + difference[1] * difference[1] + difference[2] * difference[2])
        .sqrt()
}

fn unit(value: [f64; 3]) -> Option<([f64; 3], f64)> {
    let length = distance(value, [0.0; 3]);
    (length.is_finite() && length > 0.0).then(|| (scale(value, 1.0 / length), length))
}

#[cfg(test)]
mod tests;
