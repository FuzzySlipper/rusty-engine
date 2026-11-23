//! A coarse, shared graph of a collision scene's open air.
//!
//! Space is cut into chunks of [`AIR_CHUNK_METRES`], each read once per
//! collision change as a lattice of [`AIR_CELL_METRES`] cells. A cell is
//! open when its centre is in no collidable solid, and two neighbouring
//! cells join when no solid lies on the segment between their centres, so
//! a door thinner than a cell still parts them. A chunk keeps its **pockets**
//! (the air its joined cells connect) and its **ports**: where a pocket
//! meets one of the chunk's faces, tiled [`AIR_PORT_METRES`] square. A port
//! joins the ports of the neighbouring chunk whose cells its own cells join
//! across the face. A chunk with no solid at all, or nothing but solid, is
//! kept without cells.
//!
//! A route through air runs over ports between built chunks; only the
//! chunks at its two ends are read cell by cell. Chunks are built by
//! whoever keeps the graph ([`AirGraph::with_chunk`]), as residency and
//! collision changes call for ([`AirSceneWatch`]).

use std::cmp::Ordering;
use std::collections::{BTreeSet, BinaryHeap, HashMap};
use std::sync::{Arc, LazyLock};

use core_space::{VoxelCoord, WorldOrigin};
use svc_collision::{StaticMeshBounds, StaticMeshInstanceId};

use crate::VoxelCollisionScene;

/// The side of one air cell, in metres.
pub const AIR_CELL_METRES: f64 = 0.5;
/// Cells along each side of an air chunk.
pub const AIR_CHUNK_CELLS: usize = 32;
/// The side of one air chunk, in metres.
pub const AIR_CHUNK_METRES: f64 = AIR_CELL_METRES * AIR_CHUNK_CELLS as f64;
/// The side of the square tiles a chunk face's ports are cut into, in
/// metres, so a wide face is not one port with one centre.
pub const AIR_PORT_METRES: f64 = AIR_CELL_METRES * TILE as f64;
/// How far along a route from either end its bearing is read, in metres.
pub const AIR_BEARING_METRES: f64 = 3.0;

const N: usize = AIR_CHUNK_CELLS;
const CELLS: usize = N * N * N;
const WORDS: usize = CELLS / 64;
const FACE_WORDS: usize = N * N / 64;
/// Face cells along each side of a port's tile.
const TILE: usize = 8;
const TILES: usize = N / TILE;
const STRIDE: [usize; 3] = [1, N, N * N];
/// A chunk face's two other axes, in order, by its own.
const ACROSS: [[usize; 2]; 3] = [[1, 2], [0, 2], [0, 1]];
/// An endpoint takes the cells within this many metres of it.
const ANCHOR_METRES: f64 = AIR_CELL_METRES * 1.75;
/// The nearest cells an endpoint tries a clear line to.
const ANCHOR_TRIES: usize = 4;
/// A traced cell path keeps every this many cells as a waypoint.
const TRACE_STEP: usize = 4;
const NO_LABEL: u16 = u16::MAX;
const NO_CELL: u16 = u16::MAX;

/// A chunk's coordinate: its minimum corner is this times [`AIR_CHUNK_METRES`].
pub type AirChunkKey = [i64; 3];

/// One pocket of air: the connected open cells of one chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AirPocketId {
    pub chunk: AirChunkKey,
    pub index: u16,
}

/// What a pocket holds.
#[derive(Debug, Clone, PartialEq)]
pub struct AirPocket {
    pub volume_m3: f64,
    /// The bounds of its cells.
    pub min: [f64; 3],
    pub max: [f64; 3],
    pub centroid: [f64; 3],
    /// Its volume in each layer of cells, from the chunk's floor up: how
    /// much it holds below a height.
    pub layer_volumes_m3: Vec<f64>,
}

/// Where a pocket meets a face of its chunk, within one tile.
#[derive(Debug, Clone, PartialEq)]
pub struct AirOpening {
    /// Its cell nearest its middle, at the face.
    pub centre: [f64; 3],
    pub area_m2: f64,
    /// The bounds of its cells.
    pub min: [f64; 3],
    pub max: [f64; 3],
    /// The pockets it opens into, in the built chunk beyond the face; none
    /// when that chunk is solid there or not built.
    pub leads_to: Vec<AirPocketId>,
    /// The chunk beyond the face is built.
    pub beyond_built: bool,
}

/// A path through air.
#[derive(Debug, Clone, PartialEq)]
pub struct AirRoute {
    /// Its length.
    pub metres: f64,
    /// From the start to the end: the endpoints, the cells beside them it
    /// leaves from and arrives at, and the ports and cells between.
    pub waypoints: Vec<[f64; 3]>,
}

impl AirRoute {
    /// The point [`AIR_BEARING_METRES`] along the route from its start, or
    /// its end when it is shorter: the way it leaves.
    pub fn near_start(&self) -> [f64; 3] {
        along(self.waypoints.iter().copied(), AIR_BEARING_METRES)
    }

    /// The point [`AIR_BEARING_METRES`] back along the route from its end,
    /// or its start when it is shorter: the way it arrives.
    pub fn near_end(&self) -> [f64; 3] {
        along(self.waypoints.iter().rev().copied(), AIR_BEARING_METRES)
    }
}

/// Why no route was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AirRouteFailure {
    /// No air joins the two within the range.
    Unreachable,
    /// An endpoint is in a chunk not yet built, or the search met one
    /// within the range before finding a way.
    NotBuilt,
}

/// What one route search found.
#[derive(Debug, Clone, PartialEq)]
pub struct AirRoutes {
    /// For each target, its route or why there is none.
    pub routes: Vec<Result<AirRoute, AirRouteFailure>>,
    /// Ports the search settled.
    pub ports_settled: u32,
    /// Cells marched where an end could not see its pocket's ports.
    pub cells_marched: u32,
    /// The search met unbuilt chunks within the range.
    pub incomplete: bool,
}

/// One chunk as built.
pub struct AirChunk {
    body: Body,
}

enum Body {
    /// No solid anywhere in it, or on the segments to the cells beyond it.
    Open,
    /// No open cell.
    Solid,
    Mixed(Box<Mixed>),
}

struct Mixed {
    open: Box<[u64]>,
    /// By axis, bit `cell`: the cell and the next one along the axis join.
    links: [Box<[u64]>; 3],
    /// By axis and side (low, high), bit `u * N + v`: the face cell joins
    /// the cell beyond the face.
    faces: [[[u64; FACE_WORDS]; 2]; 3],
    labels: Labels,
    pockets: Vec<PocketData>,
    ports: Vec<Port>,
    /// By pocket, the paths between its ports.
    paths: Vec<PocketPaths>,
}

/// Each cell's pocket, a byte each while a chunk has few pockets.
enum Labels {
    Narrow(Box<[u8]>),
    Wide(Box<[u16]>),
}

impl Labels {
    fn get(&self, cell: usize) -> u16 {
        match self {
            Self::Narrow(labels) => match labels[cell] {
                u8::MAX => NO_LABEL,
                label => u16::from(label),
            },
            Self::Wide(labels) => labels[cell],
        }
    }

    fn bytes(&self) -> usize {
        match self {
            Self::Narrow(labels) => labels.len(),
            Self::Wide(labels) => labels.len() * 2,
        }
    }
}

/// The paths between one pocket's ports: its ports, and the path between
/// each two in centimetres ([`UNREACHED`] where none), row by row.
#[derive(Default)]
struct PocketPaths {
    ports: Vec<u16>,
    centimetres: Vec<u16>,
}

const UNREACHED: u16 = u16::MAX;

struct PocketData {
    cells: u32,
    min: [u8; 3],
    max: [u8; 3],
    sum: [u32; 3],
    layers: [u16; N],
}

#[derive(Clone, Copy)]
struct Port {
    /// `axis * 2 + side`, side 0 the low face.
    face: u8,
    tile: u8,
    pocket: u16,
    /// Bit `(u % TILE) * TILE + v % TILE` for each of its face cells.
    mask: u64,
    centre: u16,
    cells: u16,
    /// Its place among its pocket's ports.
    slot: u16,
}

/// The ports of an open chunk: every tile of every face, one pocket.
static OPEN_PORTS: LazyLock<Vec<Port>> = LazyLock::new(|| {
    let mut ports = Vec::with_capacity(6 * TILES * TILES);
    for face in 0..6_u8 {
        let axis = usize::from(face / 2);
        let layer = if face.is_multiple_of(2) { 0 } else { N - 1 };
        for tile in 0..(TILES * TILES) as u8 {
            let (u, v) = (
                usize::from(tile) / TILES * TILE + TILE / 2,
                usize::from(tile) % TILES * TILE + TILE / 2,
            );
            ports.push(Port {
                face,
                tile,
                pocket: 0,
                mask: u64::MAX,
                centre: face_cell(axis, layer, u, v) as u16,
                cells: (TILE * TILE) as u16,
                slot: u16::from(face) * (TILES * TILES) as u16 + u16::from(tile),
            });
        }
    }
    ports
});

impl AirChunk {
    /// Reads the chunk at `key` from `scene`'s collidable voxels and static
    /// meshes.
    pub fn build(scene: &VoxelCollisionScene, key: AirChunkKey) -> Self {
        let min = chunk_min(key);
        // Cells some scanline through their centre finds in solid.
        let mut solid_cells = vec![0_u64; WORDS].into_boxed_slice();
        let mut links = [(); 3].map(|_| vec![0_u64; WORDS].into_boxed_slice());
        let mut faces = [[[0_u64; FACE_WORDS]; 2]; 3];
        let mut any_solid = false;
        let mut spans = Vec::new();
        // Each scanline runs from the centre of the cell before the chunk
        // to the centre of the cell after it, through its cells' centres.
        let length = AIR_CELL_METRES * (N + 1) as f64;
        for axis in 0..3 {
            let [first, second] = ACROSS[axis];
            let mut direction = [0.0; 3];
            direction[axis] = 1.0;
            for u in 0..N {
                for v in 0..N {
                    let mut from = [0.0; 3];
                    from[axis] = min[axis] - AIR_CELL_METRES * 0.5;
                    from[first] = min[first] + (u as f64 + 0.5) * AIR_CELL_METRES;
                    from[second] = min[second] + (v as f64 + 0.5) * AIR_CELL_METRES;
                    spans.clear();
                    scene.solid_spans(from, direction, length, &mut spans);
                    any_solid |= !spans.is_empty();
                    let (solid, blocked) = scanline(&spans);
                    for cell in 0..N {
                        let index = face_cell(axis, cell, u, v);
                        if solid[cell + 1] {
                            set(&mut solid_cells, index);
                        }
                        if cell + 1 < N && !blocked[cell + 1] {
                            set(&mut links[axis], index);
                        }
                    }
                    let bit = u * N + v;
                    if !blocked[0] {
                        set(&mut faces[axis][0], bit);
                    }
                    if !blocked[N] {
                        set(&mut faces[axis][1], bit);
                    }
                }
            }
        }
        if !any_solid {
            return Self { body: Body::Open };
        }
        // A cell is open only where every scanline through it agrees: one
        // that meets no surface cannot tell rock from air, and reads air.
        let open: Box<[u64]> = solid_cells.iter().map(|word| !word).collect();
        if open.iter().all(|word| *word == 0) {
            return Self { body: Body::Solid };
        }
        // A join needs both its cells open, whichever scanline found them.
        for (axis, axis_links) in links.iter_mut().enumerate() {
            for cell in 0..CELLS {
                if get(axis_links, cell) && !(get(&open, cell) && get(&open, cell + STRIDE[axis])) {
                    clear(axis_links, cell);
                }
            }
        }
        for (axis, sides) in faces.iter_mut().enumerate() {
            for (side, bits) in sides.iter_mut().enumerate() {
                let layer = if side == 0 { 0 } else { N - 1 };
                for u in 0..N {
                    for v in 0..N {
                        if !get(&open, face_cell(axis, layer, u, v)) {
                            clear(bits, u * N + v);
                        }
                    }
                }
            }
        }
        let mut mixed = Mixed {
            open,
            links,
            faces,
            labels: Labels::Wide(vec![NO_LABEL; CELLS].into_boxed_slice()),
            pockets: Vec::new(),
            ports: Vec::new(),
            paths: Vec::new(),
        };
        mixed.label();
        mixed.find_ports();
        mixed.measure_paths();
        Self {
            body: Body::Mixed(Box::new(mixed)),
        }
    }

    /// No solid in it.
    pub fn is_open(&self) -> bool {
        matches!(self.body, Body::Open)
    }

    /// No air in it.
    pub fn is_solid(&self) -> bool {
        matches!(self.body, Body::Solid)
    }

    /// Bytes it holds.
    pub fn bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + match &self.body {
                Body::Mixed(mixed) => mixed.bytes(),
                Body::Open | Body::Solid => 0,
            }
    }

    fn ports(&self) -> &[Port] {
        match &self.body {
            Body::Open => &OPEN_PORTS,
            Body::Solid => &[],
            Body::Mixed(mixed) => &mixed.ports,
        }
    }

    fn label(&self, cell: usize) -> u16 {
        match &self.body {
            Body::Open => 0,
            Body::Solid => NO_LABEL,
            Body::Mixed(mixed) => mixed.labels.get(cell),
        }
    }

    /// Whether the straight line between two cells' centres stays in
    /// joined cells.
    fn sees(&self, from: usize, to: usize) -> bool {
        match &self.body {
            Body::Open => true,
            Body::Solid => false,
            Body::Mixed(mixed) => mixed.sees(from, to),
        }
    }

    /// The other ports of a port's pocket and the path to each.
    fn paths(&self, port: usize, mut each: impl FnMut(usize, f64)) {
        match &self.body {
            Body::Open => {
                let ports = &OPEN_PORTS;
                let from = cell_offset(ports[port].centre.into());
                for (other, next) in ports.iter().enumerate() {
                    if other != port {
                        each(other, distance(from, cell_offset(next.centre.into())));
                    }
                }
            }
            Body::Solid => {}
            Body::Mixed(mixed) => {
                let held = mixed.ports[port];
                let paths = &mixed.paths[usize::from(held.pocket)];
                let count = paths.ports.len();
                let row = &paths.centimetres[usize::from(held.slot) * count..][..count];
                for (other, centimetres) in paths.ports.iter().zip(row) {
                    if usize::from(*other) != port && *centimetres != UNREACHED {
                        each(usize::from(*other), f64::from(*centimetres) * 0.01);
                    }
                }
            }
        }
    }

    /// Distances by fast marching from `seeds` through joined cells; none
    /// in a chunk without cells.
    fn march(&self, seeds: &[(usize, f64)]) -> Option<Field> {
        match &self.body {
            Body::Mixed(mixed) => Some(mixed.march(seeds)),
            Body::Open | Body::Solid => None,
        }
    }
}

impl Mixed {
    fn bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + 4 * WORDS * 8
            + self.labels.bytes()
            + self.pockets.capacity() * std::mem::size_of::<PocketData>()
            + self.ports.capacity() * std::mem::size_of::<Port>()
            + self
                .paths
                .iter()
                .map(|paths| {
                    std::mem::size_of::<PocketPaths>()
                        + (paths.ports.capacity() + paths.centimetres.capacity()) * 2
                })
                .sum::<usize>()
    }

    fn open(&self, cell: usize) -> bool {
        get(&self.open, cell)
    }

    /// The cells joined to `cell`, each with its axis, and how many.
    fn joined(&self, cell: usize) -> ([(usize, usize); 6], usize) {
        joined(&self.links, cell)
    }

    /// Labels each open cell with its pocket.
    fn label(&mut self) {
        let mut labels = vec![NO_LABEL; CELLS];
        let mut stack = Vec::new();
        for start in 0..CELLS {
            if !self.open(start) || labels[start] != NO_LABEL {
                continue;
            }
            let label = self.pockets.len() as u16;
            let mut pocket = PocketData {
                cells: 0,
                min: [u8::MAX; 3],
                max: [0; 3],
                sum: [0; 3],
                layers: [0; N],
            };
            labels[start] = label;
            stack.push(start);
            while let Some(cell) = stack.pop() {
                let at = coordinates(cell);
                pocket.cells += 1;
                for axis in 0..3 {
                    pocket.min[axis] = pocket.min[axis].min(at[axis] as u8);
                    pocket.max[axis] = pocket.max[axis].max(at[axis] as u8);
                    pocket.sum[axis] += at[axis] as u32;
                }
                pocket.layers[at[1]] += 1;
                let (neighbours, count) = joined(&self.links, cell);
                for &(neighbour, _) in &neighbours[..count] {
                    if labels[neighbour] == NO_LABEL {
                        labels[neighbour] = label;
                        stack.push(neighbour);
                    }
                }
            }
            self.pockets.push(pocket);
        }
        self.labels = if self.pockets.len() < usize::from(u8::MAX) {
            Labels::Narrow(
                labels
                    .iter()
                    .map(|label| {
                        if *label == NO_LABEL {
                            u8::MAX
                        } else {
                            *label as u8
                        }
                    })
                    .collect(),
            )
        } else {
            Labels::Wide(labels.into_boxed_slice())
        };
    }

    /// Groups each face's passable cells by tile and pocket.
    fn find_ports(&mut self) {
        for face in 0..6_u8 {
            let axis = usize::from(face / 2);
            let side = usize::from(face % 2);
            let layer = if side == 0 { 0 } else { N - 1 };
            let mut groups: HashMap<(u8, u16), (u64, [usize; 2], u16)> = HashMap::new();
            for u in 0..N {
                for v in 0..N {
                    if !get(&self.faces[axis][side], u * N + v) {
                        continue;
                    }
                    let cell = face_cell(axis, layer, u, v);
                    let tile = (u / TILE * TILES + v / TILE) as u8;
                    let group =
                        groups
                            .entry((tile, self.labels.get(cell)))
                            .or_insert((0, [0, 0], 0));
                    group.0 |= 1 << ((u % TILE) * TILE + v % TILE);
                    group.1[0] += u;
                    group.1[1] += v;
                    group.2 += 1;
                }
            }
            let mut groups: Vec<_> = groups.into_iter().collect();
            groups.sort_unstable_by_key(|(key, _)| *key);
            for ((tile, pocket), (mask, sum, cells)) in groups {
                let mean = [
                    sum[0] as f64 / f64::from(cells),
                    sum[1] as f64 / f64::from(cells),
                ];
                let (base_u, base_v) = (
                    usize::from(tile) / TILES * TILE,
                    usize::from(tile) % TILES * TILE,
                );
                let mut centre = (f64::INFINITY, 0);
                for bit in 0..64 {
                    if mask & (1 << bit) == 0 {
                        continue;
                    }
                    let (u, v) = (base_u + bit / TILE, base_v + bit % TILE);
                    let off = (u as f64 - mean[0]).powi(2) + (v as f64 - mean[1]).powi(2);
                    if off < centre.0 {
                        centre = (off, face_cell(axis, layer, u, v));
                    }
                }
                self.ports.push(Port {
                    face,
                    tile,
                    pocket,
                    mask,
                    centre: centre.1 as u16,
                    cells,
                    slot: 0,
                });
            }
        }
    }

    /// The path between each two ports of a pocket: straight where one sees
    /// the other, otherwise the shorter of the marches from either end.
    fn measure_paths(&mut self) {
        let mut paths: Vec<PocketPaths> = (0..self.pockets.len())
            .map(|_| PocketPaths::default())
            .collect();
        for (index, port) in self.ports.iter_mut().enumerate() {
            let members = &mut paths[usize::from(port.pocket)].ports;
            port.slot = members.len() as u16;
            members.push(index as u16);
        }
        let centre = |port: u16| usize::from(self.ports[usize::from(port)].centre);
        for pocket in &mut paths {
            let count = pocket.ports.len();
            pocket.centimetres = vec![UNREACHED; count * count];
            for from in 0..count {
                let a = centre(pocket.ports[from]);
                let hidden: Vec<usize> = (from + 1..count)
                    .filter(|to| !self.sees(a, centre(pocket.ports[*to])))
                    .collect();
                // One march from the lower port of each hidden pair, ended
                // once it has reached them all.
                let field = (!hidden.is_empty()).then(|| {
                    let goals: Vec<usize> =
                        hidden.iter().map(|to| centre(pocket.ports[*to])).collect();
                    self.march_to(&[(a, 0.0)], &goals)
                });
                for to in from + 1..count {
                    let b = centre(pocket.ports[to]);
                    let metres = match &field {
                        Some(field) if hidden.contains(&to) => field.metres(b),
                        _ => distance(cell_offset(a), cell_offset(b)),
                    };
                    if metres.is_finite() {
                        let centimetres =
                            (metres * 100.0).round().min(f64::from(UNREACHED - 1)) as u16;
                        pocket.centimetres[from * count + to] = centimetres;
                        pocket.centimetres[to * count + from] = centimetres;
                    }
                }
            }
        }
        self.paths = paths;
    }

    /// Whether the straight line between two cells' centres crosses only
    /// joined cells, walked the same way whichever end it starts from.
    fn sees(&self, from: usize, to: usize) -> bool {
        let (from, to) = if from <= to { (from, to) } else { (to, from) };
        if !self.open(from) || !self.open(to) {
            return false;
        }
        let start = coordinates(from);
        let end = coordinates(to);
        let mut cell = start;
        let mut step = [0_isize; 3];
        let mut next = [f64::INFINITY; 3];
        let mut delta = [f64::INFINITY; 3];
        for axis in 0..3 {
            let span = end[axis] as f64 - start[axis] as f64;
            if span != 0.0 {
                step[axis] = span.signum() as isize;
                delta[axis] = 1.0 / span.abs();
                next[axis] = 0.5 * delta[axis];
            }
        }
        while cell != end {
            let axis = if next[0] <= next[1] && next[0] <= next[2] {
                0
            } else if next[1] <= next[2] {
                1
            } else {
                2
            };
            let here = index(cell);
            let mut ahead = cell;
            ahead[axis] = (ahead[axis] as isize + step[axis]) as usize;
            let joined = if step[axis] > 0 {
                get(&self.links[axis], here)
            } else {
                get(&self.links[axis], index(ahead))
            };
            if !joined {
                return false;
            }
            cell = ahead;
            next[axis] += delta[axis];
        }
        true
    }

    /// Distances through joined cells from `seeds` (a cell and its start),
    /// solving the eikonal equation over the lattice so a diagonal measures
    /// close to its straight length rather than its steps.
    fn march(&self, seeds: &[(usize, f64)]) -> Field {
        self.march_to(seeds, &[])
    }

    /// [`Self::march`], ended once every one of `goals` is settled; the
    /// whole of the seeds' pocket without goals.
    fn march_to(&self, seeds: &[(usize, f64)], goals: &[usize]) -> Field {
        let mut left = goals.len();
        let mut field = Field {
            metres: vec![f32::INFINITY; CELLS],
            parent: vec![NO_CELL; CELLS],
            marched: 0,
        };
        let mut settled = vec![false; CELLS];
        let mut front = BinaryHeap::new();
        for &(cell, metres) in seeds {
            if self.open(cell) && (metres as f32) < field.metres[cell] {
                field.metres[cell] = metres as f32;
                front.push(Front(metres as f32, cell as u32));
            }
        }
        while let Some(Front(metres, cell)) = front.pop() {
            let cell = cell as usize;
            if settled[cell] || metres > field.metres[cell] {
                continue;
            }
            settled[cell] = true;
            field.marched += 1;
            if left > 0 && goals.contains(&cell) {
                left -= goals.iter().filter(|goal| **goal == cell).count();
                if left == 0 {
                    break;
                }
            }
            let (neighbours, count) = self.joined(cell);
            for &(neighbour, _) in &neighbours[..count] {
                if settled[neighbour] {
                    continue;
                }
                if let Some((value, parent)) = self.solve(&field, &settled, neighbour) {
                    if value < field.metres[neighbour] {
                        field.metres[neighbour] = value;
                        field.parent[neighbour] = parent as u16;
                        front.push(Front(value, neighbour as u32));
                    }
                }
            }
        }
        field
    }

    /// The eikonal update of `cell` from its settled, joined neighbours,
    /// and the nearest of them.
    fn solve(&self, field: &Field, settled: &[bool], cell: usize) -> Option<(f32, usize)> {
        let mut axes = [(f32::INFINITY, usize::MAX); 3];
        let (neighbours, count) = self.joined(cell);
        for &(neighbour, axis) in &neighbours[..count] {
            if settled[neighbour] && field.metres[neighbour] < axes[axis].0 {
                axes[axis] = (field.metres[neighbour], neighbour);
            }
        }
        axes.sort_by(|left, right| left.0.total_cmp(&right.0));
        let (nearest, parent) = axes[0];
        if !nearest.is_finite() {
            return None;
        }
        let h = AIR_CELL_METRES as f32;
        let mut value = nearest + h;
        let mut used = 1;
        while used < 3 && axes[used].0 < value {
            used += 1;
            let values = &axes[..used];
            let sum: f32 = values.iter().map(|axis| axis.0).sum();
            let squares: f32 = values.iter().map(|axis| axis.0 * axis.0).sum();
            let count = used as f32;
            let discriminant = sum * sum - count * (squares - h * h);
            if discriminant < 0.0 {
                break;
            }
            value = (sum + discriminant.sqrt()) / count;
        }
        Some((value, parent))
    }
}

/// Distances through one chunk's cells from a march's seeds.
pub struct Field {
    metres: Vec<f32>,
    parent: Vec<u16>,
    marched: u32,
}

impl Field {
    fn metres(&self, cell: usize) -> f64 {
        f64::from(self.metres[cell])
    }

    /// The cells from `cell` back to a seed, every [`TRACE_STEP`]th one.
    fn trace(&self, mut cell: usize) -> Vec<usize> {
        let mut cells = vec![cell];
        let mut steps = 0;
        while self.parent[cell] != NO_CELL {
            cell = usize::from(self.parent[cell]);
            steps += 1;
            if steps % TRACE_STEP == 0 {
                cells.push(cell);
            }
        }
        if cells.last() != Some(&cell) {
            cells.push(cell);
        }
        cells
    }
}

/// A cell on a march's front, nearest first from a max-heap.
#[derive(PartialEq)]
struct Front(f32, u32);

impl Eq for Front {}

impl PartialOrd for Front {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Front {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .0
            .total_cmp(&self.0)
            .then_with(|| other.1.cmp(&self.1))
    }
}

/// Which cells along a scanline are in solid, and which gaps between
/// neighbouring cells' centres meet solid: positions `0..=N + 1` are the
/// cell before the chunk, its cells, and the cell after; gap `k` lies
/// between positions `k` and `k + 1`.
fn scanline(spans: &[(f64, f64)]) -> ([bool; N + 2], [bool; N + 1]) {
    let mut solid = [false; N + 2];
    let mut blocked = [false; N + 1];
    for &(start, end) in spans {
        let first = (start / AIR_CELL_METRES).ceil().max(0.0) as usize;
        let last = (end / AIR_CELL_METRES).floor();
        if last >= 0.0 {
            for flag in solid
                .iter_mut()
                .take((last as usize).min(N + 1) + 1)
                .skip(first)
            {
                *flag = true;
            }
        }
        // Gap k spans [k, k + 1] cells: it meets [start, end] unless one
        // ends before the other starts.
        let first_gap = ((start / AIR_CELL_METRES).floor() - 1.0).max(0.0) as usize;
        let last_gap = (end / AIR_CELL_METRES).floor();
        if last_gap >= 0.0 {
            for (gap, flag) in blocked
                .iter_mut()
                .enumerate()
                .take((last_gap as usize).min(N) + 1)
                .skip(first_gap)
            {
                let (from, to) = (
                    gap as f64 * AIR_CELL_METRES,
                    (gap + 1) as f64 * AIR_CELL_METRES,
                );
                if start <= to && end >= from {
                    *flag = true;
                }
            }
        }
    }
    (solid, blocked)
}

/// An endpoint of a route: the open cell beside a point it starts from.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Anchor {
    point: [f64; 3],
    chunk: AirChunkKey,
    cell: usize,
    /// From the point to the cell's centre.
    metres: f64,
}

/// One end of a route: its pocket's ports and the way to each.
struct End {
    anchor: Anchor,
    chunk: Arc<AirChunk>,
    pocket: u16,
    /// Each port of the pocket, its distance from the endpoint and how.
    ports: Vec<(usize, f64, Via)>,
    field: Option<Field>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Via {
    /// A straight line from the endpoint's cell.
    Straight,
    /// Straight to this port first, then its path on.
    Port(usize),
    /// The march from the endpoint.
    Marched,
}

/// A port of a built chunk, as the route search walks them.
type Node = (AirChunkKey, u16);

/// The ports a route search has reached.
type Reached = HashMap<Node, Visit, std::hash::BuildHasherDefault<NodeHasher>>;

/// A reached port: its distance so far, the straight distance on to the
/// nearest target, the port it came from, and whether it is settled.
struct Visit {
    metres: f64,
    ahead: f64,
    parent: Option<Node>,
    settled: bool,
}

impl Visit {
    fn new(ahead: f64) -> Self {
        Self {
            metres: f64::INFINITY,
            ahead,
            parent: None,
            settled: false,
        }
    }
}

/// A multiply-and-rotate hash for port keys, which are small integers a
/// search hashes many times; nothing outside the Engine chooses them.
#[derive(Default)]
struct NodeHasher(u64);

impl std::hash::Hasher for NodeHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.write_u64(u64::from(*byte));
        }
    }

    fn write_u64(&mut self, value: u64) {
        self.0 = (self.0.rotate_left(5) ^ value).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }

    fn write_i64(&mut self, value: i64) {
        self.write_u64(value as u64);
    }

    fn write_u16(&mut self, value: u16) {
        self.write_u64(u64::from(value));
    }

    fn write_usize(&mut self, value: usize) {
        self.write_u64(value as u64);
    }
}

/// A port on the route search's front: its order (its distance plus the
/// straight distance on to the nearest target), its distance, and it.
#[derive(PartialEq)]
struct Step(f64, f64, Node);

impl Eq for Step {}

impl PartialOrd for Step {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Step {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .0
            .total_cmp(&self.0)
            .then_with(|| other.2.cmp(&self.2))
    }
}

/// A scene's open air as built so far: an immutable snapshot readers share.
#[derive(Clone, Default)]
pub struct AirGraph {
    revision: u64,
    chunks: HashMap<AirChunkKey, Arc<AirChunk>>,
}

impl AirGraph {
    /// Builds every chunk within `radius` of each of `points` from `scene`,
    /// here and now.
    pub fn around(scene: &VoxelCollisionScene, points: &[[f64; 3]], radius: f64) -> Self {
        let mut graph = Self::default();
        let keys: BTreeSet<AirChunkKey> = points
            .iter()
            .flat_map(|point| chunks_within(*point, radius))
            .collect();
        for key in keys {
            graph = graph.with_chunk(key, Arc::new(AirChunk::build(scene, key)));
        }
        graph
    }

    /// Changes with every chunk built or dropped.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// This graph with `chunk` at `key`, under the next revision.
    pub fn with_chunk(&self, key: AirChunkKey, chunk: Arc<AirChunk>) -> Self {
        let mut next = self.clone();
        next.chunks.insert(key, chunk);
        next.revision += 1;
        next
    }

    /// This graph without the chunks at `keys`, under the next revision.
    pub fn without(&self, keys: &[AirChunkKey]) -> Self {
        let mut next = self.clone();
        for key in keys {
            next.chunks.remove(key);
        }
        next.revision += 1;
        next
    }

    pub fn chunk(&self, key: AirChunkKey) -> Option<&Arc<AirChunk>> {
        self.chunks.get(&key)
    }

    pub fn chunk_keys(&self) -> impl Iterator<Item = AirChunkKey> + '_ {
        self.chunks.keys().copied()
    }

    /// Built chunks.
    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// Built chunks with both air and solid in them.
    pub fn mixed_chunk_count(&self) -> usize {
        self.chunks
            .values()
            .filter(|chunk| matches!(chunk.body, Body::Mixed(_)))
            .count()
    }

    /// Bytes the built chunks hold.
    pub fn bytes(&self) -> usize {
        self.chunks.values().map(|chunk| chunk.bytes()).sum()
    }

    /// The pocket `point` is in; none in solid or an unbuilt chunk.
    pub fn pocket_at(&self, point: [f64; 3]) -> Option<AirPocketId> {
        let (chunk, cell) = locate(point);
        let label = self.chunks.get(&chunk)?.label(cell);
        (label != NO_LABEL).then_some(AirPocketId {
            chunk,
            index: label,
        })
    }

    pub fn pocket(&self, id: AirPocketId) -> Option<AirPocket> {
        let chunk = self.chunks.get(&id.chunk)?;
        let min = chunk_min(id.chunk);
        let cell_volume = AIR_CELL_METRES.powi(3);
        match &chunk.body {
            Body::Open if id.index == 0 => Some(AirPocket {
                volume_m3: AIR_CHUNK_METRES.powi(3),
                min,
                max: min.map(|value| value + AIR_CHUNK_METRES),
                centroid: min.map(|value| value + AIR_CHUNK_METRES * 0.5),
                layer_volumes_m3: vec![cell_volume * (N * N) as f64; N],
            }),
            Body::Mixed(mixed) => {
                let pocket = mixed.pockets.get(usize::from(id.index))?;
                let corner = |cell: [u8; 3], end: f64| {
                    [0, 1, 2]
                        .map(|axis| min[axis] + (f64::from(cell[axis]) + end) * AIR_CELL_METRES)
                };
                let cells = f64::from(pocket.cells);
                Some(AirPocket {
                    volume_m3: cells * cell_volume,
                    min: corner(pocket.min, 0.0),
                    max: corner(pocket.max, 1.0),
                    centroid: [0, 1, 2].map(|axis| {
                        min[axis] + (f64::from(pocket.sum[axis]) / cells + 0.5) * AIR_CELL_METRES
                    }),
                    layer_volumes_m3: pocket
                        .layers
                        .iter()
                        .map(|count| f64::from(*count) * cell_volume)
                        .collect(),
                })
            }
            _ => None,
        }
    }

    /// Where a pocket meets its chunk's faces, one opening per tile.
    pub fn openings(&self, id: AirPocketId) -> Vec<AirOpening> {
        let Some(chunk) = self.chunks.get(&id.chunk) else {
            return Vec::new();
        };
        let min = chunk_min(id.chunk);
        chunk
            .ports()
            .iter()
            .enumerate()
            .filter(|(_, port)| port.pocket == id.index)
            .map(|(index, port)| {
                let axis = usize::from(port.face / 2);
                let layer = if port.face.is_multiple_of(2) {
                    0
                } else {
                    N - 1
                };
                let (base_u, base_v) = (
                    usize::from(port.tile) / TILES * TILE,
                    usize::from(port.tile) % TILES * TILE,
                );
                let mut low = [f64::INFINITY; 3];
                let mut high = [f64::NEG_INFINITY; 3];
                for bit in 0..64 {
                    if port.mask & (1 << bit) == 0 {
                        continue;
                    }
                    let at = coordinates(face_cell(
                        axis,
                        layer,
                        base_u + bit / TILE,
                        base_v + bit % TILE,
                    ));
                    for k in 0..3 {
                        low[k] = low[k].min(min[k] + at[k] as f64 * AIR_CELL_METRES);
                        high[k] = high[k].max(min[k] + (at[k] + 1) as f64 * AIR_CELL_METRES);
                    }
                }
                let beyond = self.beyond(id.chunk, *port);
                AirOpening {
                    centre: add(min, cell_offset(port.centre.into())),
                    area_m2: f64::from(port.cells) * AIR_CELL_METRES * AIR_CELL_METRES,
                    min: low,
                    max: high,
                    leads_to: self
                        .across(id.chunk, index)
                        .into_iter()
                        .map(|((chunk, other), _)| AirPocketId {
                            chunk,
                            index: self.chunks[&chunk].ports()[usize::from(other)].pocket,
                        })
                        .collect(),
                    beyond_built: self.chunks.contains_key(&beyond),
                }
            })
            .collect()
    }

    /// The shortest paths through air from `from` to each of `targets`, out
    /// to `max_metres`. Rays into `scene` place the two ends in their
    /// cells; between them the route runs over the graph.
    pub fn route(
        &self,
        scene: &VoxelCollisionScene,
        from: [f64; 3],
        targets: &[[f64; 3]],
        max_metres: f64,
    ) -> AirRoutes {
        let mut result = AirRoutes {
            routes: Vec::with_capacity(targets.len()),
            ports_settled: 0,
            cells_marched: 0,
            incomplete: false,
        };
        let start = match self.anchor(scene, from) {
            Ok(anchor) => self.end(anchor),
            Err(failure) => {
                result.routes = vec![Err(failure); targets.len()];
                result.incomplete = failure == AirRouteFailure::NotBuilt;
                return result;
            }
        };
        let mut ends: Vec<Result<End, AirRouteFailure>> = targets
            .iter()
            .map(|target| self.anchor(scene, *target).map(|anchor| self.end(anchor)))
            .collect();
        let mut start = start;
        // A target in the start's own pocket may be reached within it.
        let mut best: Vec<(f64, Option<Node>)> = vec![(f64::INFINITY, None); targets.len()];
        for (index, end) in ends.iter_mut().enumerate() {
            let Ok(end) = end else { continue };
            if end.anchor.chunk != start.anchor.chunk || end.pocket != start.pocket {
                continue;
            }
            best[index].0 = if start.chunk.sees(start.anchor.cell, end.anchor.cell) {
                start.anchor.metres
                    + distance(cell_offset(start.anchor.cell), cell_offset(end.anchor.cell))
                    + end.anchor.metres
            } else {
                let field = self.lower_field(&mut start, end);
                field.unwrap_or(f64::INFINITY)
            };
        }
        for end in [&start]
            .into_iter()
            .chain(ends.iter().filter_map(|end| end.as_ref().ok()))
        {
            if let Some(field) = &end.field {
                result.cells_marched += field.marched;
            }
        }

        // Each target's ports, by node, with the way on from each.
        let mut arrivals: HashMap<Node, Vec<(usize, f64)>> = HashMap::new();
        for (index, end) in ends.iter().enumerate() {
            if let Ok(end) = end {
                for &(port, metres, _) in &end.ports {
                    arrivals
                        .entry((end.anchor.chunk, port as u16))
                        .or_default()
                        .push((index, metres));
                }
            }
        }
        let goals: Vec<[f64; 3]> = ends
            .iter()
            .zip(targets)
            .filter(|(end, _)| end.is_ok())
            .map(|(_, target)| *target)
            .collect();
        let toward = |node_point: [f64; 3]| {
            goals
                .iter()
                .map(|goal| distance(node_point, *goal))
                .fold(f64::INFINITY, f64::min)
        };

        let mut reached: Reached = Reached::default();
        let mut front = BinaryHeap::new();
        if !goals.is_empty() {
            for &(port, metres, _) in &start.ports {
                let node = (start.anchor.chunk, port as u16);
                let ahead = toward(self.port_point(node));
                let held = reached.entry(node).or_insert(Visit::new(ahead));
                if metres < held.metres {
                    held.metres = metres;
                    front.push(Step(metres + ahead, metres, node));
                }
            }
        }
        while let Some(Step(order, metres, node)) = front.pop() {
            let visit = reached.get_mut(&node).expect("pushed nodes are reached");
            if visit.settled || metres > visit.metres {
                continue;
            }
            if order > max_metres {
                break;
            }
            let pending = ends
                .iter()
                .zip(&best)
                .any(|(end, held)| end.is_ok() && held.0 > order);
            if !pending {
                break;
            }
            visit.settled = true;
            result.ports_settled += 1;
            if let Some(arriving) = arrivals.get(&node) {
                for &(index, on) in arriving {
                    if metres + on < best[index].0 {
                        best[index] = (metres + on, Some(node));
                    }
                }
            }
            let chunk = &self.chunks[&node.0];
            let chunk_min = chunk_min(node.0);
            let ports = chunk.ports();
            let mut relax =
                |next: Node, point: [f64; 3], metres: f64, front: &mut BinaryHeap<Step>| {
                    let held = reached
                        .entry(next)
                        .or_insert_with(|| Visit::new(toward(point)));
                    if !held.settled && metres < held.metres {
                        held.metres = metres;
                        held.parent = Some(node);
                        front.push(Step(metres + held.ahead, metres, next));
                    }
                };
            chunk.paths(usize::from(node.1), |other, path| {
                let point = add(chunk_min, cell_offset(ports[other].centre.into()));
                relax((node.0, other as u16), point, metres + path, &mut front);
            });
            let port = ports[usize::from(node.1)];
            if !self.chunks.contains_key(&self.beyond(node.0, port)) {
                result.incomplete = true;
            }
            for (next, step) in self.across(node.0, usize::from(node.1)) {
                relax(next, self.port_point(next), metres + step, &mut front);
            }
        }

        for (index, end) in ends.into_iter().enumerate() {
            let route = match end {
                Err(failure) => Err(failure),
                Ok(end) => {
                    let (metres, node) = best[index];
                    if !metres.is_finite() || metres > max_metres {
                        Err(if result.incomplete {
                            AirRouteFailure::NotBuilt
                        } else {
                            AirRouteFailure::Unreachable
                        })
                    } else {
                        Ok(AirRoute {
                            metres,
                            waypoints: self.waypoints(&start, &end, node, &reached),
                        })
                    }
                }
            };
            result.routes.push(route);
        }
        result
    }

    /// The length within one pocket between two of its ends, by the march
    /// from the end with the lower cell (so either way round measures the
    /// same), and keeps the march on that end.
    fn lower_field(&self, start: &mut End, end: &mut End) -> Option<f64> {
        let (from, to) = if start.anchor.cell <= end.anchor.cell {
            (start, end)
        } else {
            (end, start)
        };
        if from.field.is_none() {
            from.field = from.chunk.march(&[(from.anchor.cell, from.anchor.metres)]);
        }
        let marched = from.field.as_ref()?.metres(to.anchor.cell) + to.anchor.metres;
        marched.is_finite().then_some(marched)
    }

    /// Places `point` in the open cell beside it nearest it with a clear
    /// line to its centre.
    fn anchor(
        &self,
        scene: &VoxelCollisionScene,
        point: [f64; 3],
    ) -> Result<Anchor, AirRouteFailure> {
        let (home, _) = locate(point);
        if !self.chunks.contains_key(&home) {
            return Err(AirRouteFailure::NotBuilt);
        }
        let global = point.map(|value| (value / AIR_CELL_METRES).floor() as i64);
        let mut candidates = Vec::new();
        for dz in -1..=1 {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let cell = [global[0] + dx, global[1] + dy, global[2] + dz];
                    let centre = cell.map(|value| (value as f64 + 0.5) * AIR_CELL_METRES);
                    let metres = distance(point, centre);
                    if metres > ANCHOR_METRES {
                        continue;
                    }
                    let (chunk, local) = locate(centre);
                    let Some(built) = self.chunks.get(&chunk) else {
                        continue;
                    };
                    if built.label(local) != NO_LABEL {
                        candidates.push((metres, chunk, local, centre));
                    }
                }
            }
        }
        candidates.sort_by(|left, right| left.0.total_cmp(&right.0));
        candidates
            .into_iter()
            .take(ANCHOR_TRIES)
            .find(|candidate| scene.segment_clear(point, candidate.3))
            .map(|(metres, chunk, cell, _)| Anchor {
                point,
                chunk,
                cell,
                metres,
            })
            .ok_or(AirRouteFailure::Unreachable)
    }

    /// An endpoint's way to each port of its pocket: straight to those its
    /// cell sees, through a seen port to the rest, by a march when it sees
    /// none.
    fn end(&self, anchor: Anchor) -> End {
        let chunk = Arc::clone(&self.chunks[&anchor.chunk]);
        let pocket = chunk.label(anchor.cell);
        let members: Vec<usize> = chunk
            .ports()
            .iter()
            .enumerate()
            .filter(|(_, port)| port.pocket == pocket)
            .map(|(index, _)| index)
            .collect();
        let mut ways: HashMap<usize, (f64, Via)> = HashMap::new();
        let here = cell_offset(anchor.cell);
        for &port in &members {
            let centre = usize::from(chunk.ports()[port].centre);
            if chunk.sees(anchor.cell, centre) {
                ways.insert(
                    port,
                    (
                        anchor.metres + distance(here, cell_offset(centre)),
                        Via::Straight,
                    ),
                );
            }
        }
        // Through a seen port to those not seen.
        let seen: Vec<(usize, f64)> = if ways.len() < members.len() {
            ways.iter().map(|(port, way)| (*port, way.0)).collect()
        } else {
            Vec::new()
        };
        for (port, metres) in seen {
            chunk.paths(port, |other, path| {
                let held = ways.get(&other).map_or(f64::INFINITY, |way| way.0);
                if metres + path < held {
                    ways.insert(other, (metres + path, Via::Port(port)));
                }
            });
        }
        let mut field = None;
        if members.iter().any(|port| !ways.contains_key(port)) {
            field = chunk.march(&[(anchor.cell, anchor.metres)]);
            if let Some(field) = &field {
                for &port in &members {
                    let marched = field.metres(usize::from(chunk.ports()[port].centre));
                    let held = ways.get(&port).map_or(f64::INFINITY, |way| way.0);
                    if marched < held {
                        ways.insert(port, (marched, Via::Marched));
                    }
                }
            }
        }
        let mut ports: Vec<(usize, f64, Via)> = ways
            .into_iter()
            .map(|(port, (metres, via))| (port, metres, via))
            .collect();
        ports.sort_unstable_by_key(|(port, _, _)| *port);
        End {
            anchor,
            chunk,
            pocket,
            ports,
            field,
        }
    }

    /// The ports joined to a port across its face, and the step to each.
    fn across(&self, key: AirChunkKey, port: usize) -> Vec<(Node, f64)> {
        let Some(chunk) = self.chunks.get(&key) else {
            return Vec::new();
        };
        let port = chunk.ports()[port];
        let beyond_key = self.beyond(key, port);
        let Some(beyond) = self.chunks.get(&beyond_key) else {
            return Vec::new();
        };
        let facing = port.face ^ 1;
        let here = add(chunk_min(key), cell_offset(port.centre.into()));
        beyond
            .ports()
            .iter()
            .enumerate()
            .filter(|(_, other)| {
                other.face == facing && other.tile == port.tile && other.mask & port.mask != 0
            })
            .map(|(index, other)| {
                let there = add(chunk_min(beyond_key), cell_offset(other.centre.into()));
                ((beyond_key, index as u16), distance(here, there))
            })
            .collect()
    }

    /// The chunk across a port's face.
    fn beyond(&self, key: AirChunkKey, port: Port) -> AirChunkKey {
        let mut beyond = key;
        let axis = usize::from(port.face / 2);
        beyond[axis] += if port.face.is_multiple_of(2) { -1 } else { 1 };
        beyond
    }

    fn port_point(&self, node: Node) -> [f64; 3] {
        let chunk = &self.chunks[&node.0];
        add(
            chunk_min(node.0),
            cell_offset(chunk.ports()[usize::from(node.1)].centre.into()),
        )
    }

    /// The waypoints of a found route, start to end.
    fn waypoints(
        &self,
        start: &End,
        end: &End,
        node: Option<Node>,
        reached: &Reached,
    ) -> Vec<[f64; 3]> {
        let start_min = chunk_min(start.anchor.chunk);
        let end_min = chunk_min(end.anchor.chunk);
        let start_cell = add(start_min, cell_offset(start.anchor.cell));
        let end_cell = add(end_min, cell_offset(end.anchor.cell));
        let mut points = vec![start.anchor.point, start_cell];
        match node {
            None => {
                // Within the start's pocket: straight, or by a march.
                let marched = [start, end]
                    .into_iter()
                    .find(|held| held.field.is_some() && held.anchor.chunk == start.anchor.chunk);
                if let Some(held) = marched {
                    let field = held.field.as_ref().expect("found with a field");
                    let other = if std::ptr::eq(held, start) {
                        end
                    } else {
                        start
                    };
                    let mut cells: Vec<[f64; 3]> = field
                        .trace(other.anchor.cell)
                        .into_iter()
                        .map(|cell| add(start_min, cell_offset(cell)))
                        .collect();
                    if std::ptr::eq(held, end) {
                        // Traced from the start back to the end.
                    } else {
                        cells.reverse();
                    }
                    points.extend(cells);
                }
            }
            Some(last) => {
                let mut chain = vec![last];
                let mut at = last;
                while let Some(previous) = reached.get(&at).and_then(|visit| visit.parent) {
                    chain.push(previous);
                    at = previous;
                }
                chain.reverse();
                points.extend(self.end_leg(start, usize::from(chain[0].1)));
                points.extend(chain.iter().map(|node| self.port_point(*node)));
                let mut back = self.end_leg(end, usize::from(last.1));
                back.reverse();
                points.extend(back);
            }
        }
        points.push(end_cell);
        points.push(end.anchor.point);
        points
    }

    /// The points from an endpoint's cell to one of its pocket's ports,
    /// that port last.
    fn end_leg(&self, end: &End, port: usize) -> Vec<[f64; 3]> {
        let min = chunk_min(end.anchor.chunk);
        let centre = |port: usize| add(min, cell_offset(end.chunk.ports()[port].centre.into()));
        let via = end
            .ports
            .iter()
            .find(|(held, _, _)| *held == port)
            .map_or(Via::Straight, |way| way.2);
        match via {
            Via::Straight => vec![centre(port)],
            Via::Port(first) => vec![centre(first), centre(port)],
            Via::Marched => {
                let Some(field) = &end.field else {
                    return vec![centre(port)];
                };
                let mut cells: Vec<[f64; 3]> = field
                    .trace(usize::from(end.chunk.ports()[port].centre))
                    .into_iter()
                    .map(|cell| add(min, cell_offset(cell)))
                    .collect();
                cells.reverse();
                cells
            }
        }
    }
}

impl std::fmt::Debug for AirGraph {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AirGraph")
            .field("revision", &self.revision)
            .field("chunks", &self.chunks.len())
            .finish()
    }
}

/// What a keeper of a graph last saw of its scene, to learn which chunks a
/// later version of the scene changed.
#[derive(Default)]
pub struct AirSceneWatch {
    seen: Option<Seen>,
}

struct Seen {
    mesh_state: u64,
    static_revision: u64,
    footprints: HashMap<StaticMeshInstanceId, (u64, Option<StaticMeshBounds>)>,
    noncollidable: BTreeSet<u16>,
    origin: WorldOrigin,
    grid: core_space::VoxelGridSpec,
}

/// What changed in a scene since a watch last saw it.
#[derive(Debug, Clone, PartialEq)]
pub enum AirSceneChange {
    /// Nothing that moves air.
    None,
    /// Collision within these boxes.
    Regions(Vec<([f64; 3], [f64; 3])>),
    /// Anything may have; `moved` when the scene's frame moved, so built
    /// chunks no longer stand where they were.
    Everything { moved: bool },
}

impl AirSceneChange {
    /// Whether the change reaches the chunk at `key` (or the segments to the
    /// cells just beyond it, which it reads too).
    pub fn reaches(&self, key: AirChunkKey) -> bool {
        match self {
            Self::None => false,
            Self::Everything { .. } => true,
            Self::Regions(regions) => {
                let min = chunk_min(key).map(|value| value - AIR_CELL_METRES);
                let max = chunk_min(key).map(|value| value + AIR_CHUNK_METRES + AIR_CELL_METRES);
                regions.iter().any(|(low, high)| {
                    (0..3).all(|axis| low[axis] <= max[axis] && high[axis] >= min[axis])
                })
            }
        }
    }
}

impl AirSceneWatch {
    /// What changed since the last scene seen, which `scene` now replaces.
    pub fn observe(&mut self, scene: &VoxelCollisionScene) -> AirSceneChange {
        let grid = scene.voxel_world().grid();
        let origin = scene.world_origin();
        let static_revision = scene.static_mesh_collision_revision();
        let Some(seen) = &mut self.seen else {
            self.seen = Some(Seen {
                mesh_state: scene.mesh_state(),
                static_revision,
                footprints: footprints(scene),
                noncollidable: scene.noncollidable_materials().clone(),
                origin,
                grid,
            });
            return AirSceneChange::Everything { moved: true };
        };
        if seen.origin != origin || seen.grid != grid {
            *seen = Seen {
                mesh_state: scene.mesh_state(),
                static_revision,
                footprints: footprints(scene),
                noncollidable: scene.noncollidable_materials().clone(),
                origin,
                grid,
            };
            return AirSceneChange::Everything { moved: true };
        }
        let mut everything = false;
        if seen.noncollidable != *scene.noncollidable_materials() {
            seen.noncollidable = scene.noncollidable_materials().clone();
            everything = true;
        }
        let mut regions = Vec::new();
        match scene.dirty_chunks_since(seen.mesh_state) {
            Some(chunks) => {
                let dims = grid.chunk_dims();
                let size = grid.voxel_size();
                for chunk in chunks {
                    let corner = grid.voxel_min_world(VoxelCoord::new(
                        chunk[0] * i64::from(dims.x()),
                        chunk[1] * i64::from(dims.y()),
                        chunk[2] * i64::from(dims.z()),
                    ));
                    let low = [corner.x, corner.y, corner.z];
                    let high = [
                        low[0] + f64::from(dims.x()) * size,
                        low[1] + f64::from(dims.y()) * size,
                        low[2] + f64::from(dims.z()) * size,
                    ];
                    regions.push((low, high));
                }
            }
            None => everything = true,
        }
        seen.mesh_state = scene.mesh_state();
        if seen.static_revision != static_revision {
            seen.static_revision = static_revision;
            let now = footprints(scene);
            for (id, (identity, bounds)) in &now {
                match seen.footprints.get(id) {
                    Some((held, _)) if held == identity => {}
                    Some((_, before)) => {
                        everything |= before.is_none() || bounds.is_none();
                        regions.extend(*before);
                        regions.extend(*bounds);
                    }
                    None => {
                        everything |= bounds.is_none();
                        regions.extend(*bounds);
                    }
                }
            }
            for (id, (_, before)) in &seen.footprints {
                if !now.contains_key(id) {
                    everything |= before.is_none();
                    regions.extend(*before);
                }
            }
            seen.footprints = now;
        }
        if everything {
            AirSceneChange::Everything { moved: false }
        } else if regions.is_empty() {
            AirSceneChange::None
        } else {
            AirSceneChange::Regions(regions)
        }
    }
}

fn footprints(
    scene: &VoxelCollisionScene,
) -> HashMap<StaticMeshInstanceId, (u64, Option<StaticMeshBounds>)> {
    scene
        .static_mesh_projection()
        .instance_footprints()
        .map(|(id, identity, bounds)| (id, (identity, bounds)))
        .collect()
}

/// The chunks whose boxes come within `radius` of `point`, nearest first.
pub fn chunks_within(point: [f64; 3], radius: f64) -> Vec<AirChunkKey> {
    let low = point.map(|value| ((value - radius) / AIR_CHUNK_METRES).floor() as i64);
    let high = point.map(|value| ((value + radius) / AIR_CHUNK_METRES).floor() as i64);
    let mut keys = Vec::new();
    for x in low[0]..=high[0] {
        for y in low[1]..=high[1] {
            for z in low[2]..=high[2] {
                let key = [x, y, z];
                let reach = chunk_distance(key, point);
                if reach <= radius {
                    keys.push((reach, key));
                }
            }
        }
    }
    keys.sort_by(|left, right| left.0.total_cmp(&right.0).then(left.1.cmp(&right.1)));
    keys.into_iter().map(|(_, key)| key).collect()
}

/// How far `point` is from the chunk at `key`'s box.
pub fn chunk_distance(key: AirChunkKey, point: [f64; 3]) -> f64 {
    let min = chunk_min(key);
    let mut squared = 0.0;
    for axis in 0..3 {
        let outside = (min[axis] - point[axis])
            .max(point[axis] - (min[axis] + AIR_CHUNK_METRES))
            .max(0.0);
        squared += outside * outside;
    }
    squared.sqrt()
}

fn chunk_min(key: AirChunkKey) -> [f64; 3] {
    key.map(|value| value as f64 * AIR_CHUNK_METRES)
}

/// The chunk `point` is in and its cell there.
fn locate(point: [f64; 3]) -> (AirChunkKey, usize) {
    let global = point.map(|value| (value / AIR_CELL_METRES).floor() as i64);
    let chunk = global.map(|value| value.div_euclid(N as i64));
    let local = global.map(|value| value.rem_euclid(N as i64) as usize);
    (chunk, index(local))
}

fn index(at: [usize; 3]) -> usize {
    at[0] + N * (at[1] + N * at[2])
}

fn coordinates(cell: usize) -> [usize; 3] {
    [cell % N, cell / N % N, cell / (N * N)]
}

/// A face cell: `layer` along `axis`, `u` and `v` along the other two.
fn face_cell(axis: usize, layer: usize, u: usize, v: usize) -> usize {
    let [first, second] = ACROSS[axis];
    let mut at = [0; 3];
    at[axis] = layer;
    at[first] = u;
    at[second] = v;
    index(at)
}

/// A cell's centre from its chunk's minimum corner.
fn cell_offset(cell: usize) -> [f64; 3] {
    coordinates(cell).map(|value| (value as f64 + 0.5) * AIR_CELL_METRES)
}

/// The cells joined to `cell` by `links`, each with its axis, and how many.
fn joined(links: &[Box<[u64]>; 3], cell: usize) -> ([(usize, usize); 6], usize) {
    let at = coordinates(cell);
    let mut found = [(0, 0); 6];
    let mut count = 0;
    for axis in 0..3 {
        if at[axis] + 1 < N && get(&links[axis], cell) {
            found[count] = (cell + STRIDE[axis], axis);
            count += 1;
        }
        if at[axis] > 0 && get(&links[axis], cell - STRIDE[axis]) {
            found[count] = (cell - STRIDE[axis], axis);
            count += 1;
        }
    }
    (found, count)
}

fn get(bits: &[u64], bit: usize) -> bool {
    bits[bit / 64] & (1 << (bit % 64)) != 0
}

fn set(bits: &mut [u64], bit: usize) {
    bits[bit / 64] |= 1 << (bit % 64);
}

fn clear(bits: &mut [u64], bit: usize) {
    bits[bit / 64] &= !(1 << (bit % 64));
}

/// The point `metres` along a polyline, or its last point when shorter.
fn along(mut points: impl Iterator<Item = [f64; 3]>, metres: f64) -> [f64; 3] {
    let Some(mut at) = points.next() else {
        return [0.0; 3];
    };
    let mut left = metres;
    for next in points {
        let step = distance(at, next);
        if step >= left && step > 0.0 {
            let share = left / step;
            return [0, 1, 2].map(|axis| at[axis] + (next[axis] - at[axis]) * share);
        }
        left -= step;
        at = next;
    }
    at
}

fn add(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [left[0] + right[0], left[1] + right[1], left[2] + right[2]]
}

fn distance(left: [f64; 3], right: [f64; 3]) -> f64 {
    let difference = [left[0] - right[0], left[1] - right[1], left[2] - right[2]];
    (difference[0] * difference[0] + difference[1] * difference[1] + difference[2] * difference[2])
        .sqrt()
}

#[cfg(test)]
mod tests;
