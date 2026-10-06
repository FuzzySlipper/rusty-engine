//! Deterministic navigation/pathfinding projection over host-derived cells or
//! voxel authority.
//!
//! # Lane
//!
//! `rust-service` — builds read-only navigation projections from authoritative
//! voxel worlds and answers deterministic path queries. It does not own AI,
//! policy mutation, movement application, render state, or demo behavior.

#![forbid(unsafe_code)]

mod columns;

use std::{
    cell::RefCell,
    cmp::Reverse,
    collections::{BTreeMap, BTreeSet, BinaryHeap, VecDeque},
};

use columns::ColumnTable;
use core_math::Vec3;
use core_space::{VoxelCoord, VoxelGridSpec, WorldPos};
use svc_spatial::VoxelWorld;

/// Configuration for deriving walkable nav cells from voxel authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NavProjectionConfig {
    /// Number of empty vertical cells required for an agent to stand.
    pub agent_height_voxels: u32,
    /// Whether the cell immediately below the agent must be solid.
    pub require_solid_floor: bool,
}

impl Default for NavProjectionConfig {
    fn default() -> Self {
        Self {
            agent_height_voxels: 2,
            require_solid_floor: true,
        }
    }
}

/// Read-only navigation projection suitable for policy/devtools inspection.
///
/// The walkable cells are held by X/Z column in flat arrays: every column of
/// their bounds while the cells fill enough of it, else only the occupied
/// columns.
#[derive(Debug, Clone)]
pub struct NavProjection {
    grid: VoxelGridSpec,
    walkable: ColumnTable<()>,
    projection_hash: u64,
}

impl PartialEq for NavProjection {
    fn eq(&self, other: &Self) -> bool {
        self.grid == other.grid
            && self.projection_hash == other.projection_hash
            && self.walkable.len() == other.walkable.len()
            && self
                .walkable
                .iter()
                .all(|(cell, _)| other.walkable.position(cell).is_some())
    }
}

impl NavProjection {
    /// Build a projection from walkable cells derived by a host-owned authority.
    ///
    /// Input order and duplicate cells do not affect the retained cells or
    /// projection hash. The caller remains responsible for deriving walkability
    /// according to its collision and agent policy.
    pub fn from_walkable_cells(
        grid: VoxelGridSpec,
        cells: impl IntoIterator<Item = VoxelCoord>,
    ) -> Self {
        let walkable = ColumnTable::new(cells.into_iter().map(|cell| (cell, ())));
        let projection_hash = walkable
            .iter()
            .fold(0u64, |sum, (cell, _)| sum.wrapping_add(cell_hash(cell)));
        Self {
            grid,
            walkable,
            projection_hash,
        }
    }

    pub fn grid(&self) -> VoxelGridSpec {
        self.grid
    }

    pub fn walkable_len(&self) -> usize {
        self.walkable.len()
    }

    pub fn projection_hash(&self) -> u64 {
        self.projection_hash
    }

    pub fn is_walkable(&self, coord: VoxelCoord) -> bool {
        self.walkable.position(coord).is_some()
    }

    #[cfg(test)]
    fn without_walkable(mut self, coord: VoxelCoord) -> Self {
        self.set_walkable(coord, false);
        self
    }

    /// The walkable cells in coordinate order.
    pub fn walkable_cells(&self) -> impl Iterator<Item = VoxelCoord> + '_ {
        let mut cells: Vec<_> = self.walkable.iter().map(|(cell, _)| cell).collect();
        cells.sort_unstable();
        cells.into_iter()
    }

    /// Adds (`true`) or removes one walkable cell, as an owner that keeps
    /// its projection current applies a local change. The hash follows.
    pub fn set_walkable(&mut self, cell: VoxelCoord, walkable: bool) {
        self.set_walkable_cells([(cell, walkable)]);
    }

    /// [`Self::set_walkable`] for many cells in turn; the cells' columns are
    /// laid out again at most once.
    pub fn set_walkable_cells(&mut self, changes: impl IntoIterator<Item = (VoxelCoord, bool)>) {
        // Cells new to the table, added together at the end.
        let mut added = BTreeSet::new();
        for (cell, walkable) in changes {
            let changed = if self.walkable.position(cell).is_some() {
                !walkable && self.walkable.remove(cell).is_some()
            } else if walkable {
                added.insert(cell)
            } else {
                added.remove(&cell)
            };
            if changed && walkable {
                self.projection_hash = self.projection_hash.wrapping_add(cell_hash(cell));
            } else if changed {
                self.projection_hash = self.projection_hash.wrapping_sub(cell_hash(cell));
            }
        }
        self.walkable
            .insert_new(added.into_iter().map(|cell| (cell, ())).collect());
    }

    /// Recompute walkability for `cells` after a local voxel or residency
    /// change. `solid` reports a cell's collision solidity, or `None` outside
    /// every resident chunk (never walkable). The hash follows each change.
    pub fn refresh_cells(
        &mut self,
        config: NavProjectionConfig,
        cells: impl IntoIterator<Item = VoxelCoord>,
        solid: impl Fn(VoxelCoord) -> Option<bool>,
    ) {
        let is_solid = |cell| solid(cell).unwrap_or(false);
        let changes: Vec<_> = cells
            .into_iter()
            .map(|cell| {
                let walkable = solid(cell) == Some(false)
                    && (!config.require_solid_floor
                        || is_solid(VoxelCoord::new(cell.x, cell.y - 1, cell.z)))
                    && (0..config.agent_height_voxels).all(|dy| {
                        !is_solid(VoxelCoord::new(cell.x, cell.y + i64::from(dy), cell.z))
                    });
                (cell, walkable)
            })
            .collect();
        self.set_walkable_cells(changes);
    }
}

/// Cells whose walkability can depend on the voxel at `voxel`: the cells an
/// agent standing there or just above would occupy, and the cell resting on it.
pub fn nav_cells_affected_by_voxel(
    voxel: VoxelCoord,
    config: NavProjectionConfig,
) -> impl Iterator<Item = VoxelCoord> {
    let below = i64::from(config.agent_height_voxels.saturating_sub(1));
    let above = i64::from(config.require_solid_floor);
    (voxel.y - below..=voxel.y + above).map(move |y| VoxelCoord::new(voxel.x, y, voxel.z))
}

/// Path query over an existing nav projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NavPathQuery {
    pub start: VoxelCoord,
    pub goal: VoxelCoord,
    pub max_visited: usize,
}

/// Vertical tolerance for horizontal neighbors in walkable-surface navigation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PlanarNavNeighborPolicy {
    /// Maximum upward or downward cell difference across one X/Z edge.
    pub max_step_cells: u8,
    /// Also step to the four diagonal X/Z neighbours.
    pub diagonal: bool,
}

/// Canonical directed surface edges admitted by the collision owner.
///
/// A surface projection owns which columns can hold an agent. Some collision
/// scenes also need an edge test: a thin wall can separate two otherwise
/// walkable columns, while a character-sized step can connect columns at
/// different support heights. Keeping this distinct from cell traversal facts
/// lets the collision owner retain its geometry authority without teaching the
/// general pathfinding service about collision shapes.
///
/// An admitted edge may also reach past a cell's planar neighbours (a jump
/// across a gap); searches then offer it as a neighbour too. An edge may carry
/// an extra cost the weighted search adds to its destination cell's.
///
/// The edges are held by origin cell, in the same flat column layout as a
/// projection: per origin, the rise of one edge toward each planar
/// neighbour column. A side table holds the rest: edges past the planar
/// neighbours, a second edge toward one neighbour column, and rises too
/// large to hold inline.
#[derive(Debug, Clone)]
pub struct NavEdgeAdmission {
    origins: ColumnTable<OriginEdges>,
    /// Admitted edges an origin's inline rises do not hold, by origin.
    others: BTreeMap<VoxelCoord, BTreeSet<VoxelCoord>>,
    costs: BTreeMap<(VoxelCoord, VoxelCoord), u64>,
    /// How many edges past the planar neighbours reach how far, at what extra
    /// cost: what bounds a search's estimate of the cost still to come.
    reaches: BTreeMap<FarReach, usize>,
    admission_hash: u64,
}

/// The edges an origin holds inline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OriginEdges {
    /// Per direction of [`planar_nav_offsets`], the rise of the admitted edge
    /// to that neighbour column held here, or [`NO_EDGE`].
    rise: [i8; 8],
    /// Whether [`NavEdgeAdmission::others`] holds edges from this origin.
    others: bool,
    /// Whether an edge from this origin carries an extra cost.
    costed: bool,
}

const NO_EDGE: i8 = i8::MIN;

impl Default for OriginEdges {
    fn default() -> Self {
        Self {
            rise: [NO_EDGE; 8],
            others: false,
            costed: false,
        }
    }
}

impl OriginEdges {
    /// The direction this record would hold the edge to `to` in.
    fn inline_direction(from: VoxelCoord, to: VoxelCoord) -> Option<(usize, i8)> {
        let direction = planar_direction(from, to)?;
        let rise = i8::try_from(to.y.checked_sub(from.y)?).ok()?;
        (rise != NO_EDGE).then_some((direction, rise))
    }

    fn holds(&self, from: VoxelCoord, to: VoxelCoord) -> bool {
        Self::inline_direction(from, to)
            .is_some_and(|(direction, rise)| self.rise[direction] == rise)
    }

    fn is_empty(&self) -> bool {
        !self.others && self.rise == [NO_EDGE; 8]
    }
}

/// How far one edge past the planar neighbours reaches, and its extra cost.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct FarReach {
    chebyshev: u64,
    manhattan: u64,
    extra: u64,
}

impl FarReach {
    fn of(from: VoxelCoord, to: VoxelCoord, extra: u64) -> Self {
        let (across, along) = (from.x.abs_diff(to.x), from.z.abs_diff(to.z));
        Self {
            chebyshev: across.max(along),
            manhattan: across.saturating_add(along),
            extra,
        }
    }
}

/// The index in [`planar_nav_offsets`] of the step from `from` to `to`.
fn planar_direction(from: VoxelCoord, to: VoxelCoord) -> Option<usize> {
    // By X step, then Z step, each from -1.
    const DIRECTIONS: [Option<usize>; 9] = [
        Some(6),
        Some(2),
        Some(5),
        Some(3),
        None,
        Some(1),
        Some(7),
        Some(0),
        Some(4),
    ];
    let step = |from: i64, to: i64| match to.checked_sub(from)? {
        step @ -1..=1 => Some((step + 1) as usize),
        _ => None,
    };
    DIRECTIONS[step(from.x, to.x)? * 3 + step(from.z, to.z)?]
}

/// Whether an edge reaches past its origin's planar neighbours.
fn is_far(from: VoxelCoord, to: VoxelCoord) -> bool {
    from.x.abs_diff(to.x) > 1 || from.z.abs_diff(to.z) > 1
}

impl NavEdgeAdmission {
    /// Retain deterministic directed edges from a collision-derived surface.
    pub fn from_allowed_edges(edges: impl IntoIterator<Item = (VoxelCoord, VoxelCoord)>) -> Self {
        let mut admission = Self {
            origins: ColumnTable::new([]),
            others: BTreeMap::new(),
            costs: BTreeMap::new(),
            reaches: BTreeMap::new(),
            admission_hash: 0,
        };
        admission.set_allowed_edges(edges.into_iter().map(|(from, to)| (from, to, true)));
        admission
    }

    /// Admits (`true`) or withdraws one directed edge, as an owner that keeps
    /// its admission current applies a local change. Withdrawing drops its
    /// extra cost. The hash follows.
    pub fn set_allowed(&mut self, from: VoxelCoord, to: VoxelCoord, allowed: bool) {
        self.set_allowed_edges([(from, to, allowed)]);
    }

    /// [`Self::set_allowed`] for many edges in turn; the origins' columns are
    /// laid out again at most once.
    pub fn set_allowed_edges(
        &mut self,
        changes: impl IntoIterator<Item = (VoxelCoord, VoxelCoord, bool)>,
    ) {
        // Origins new to the table, added together at the end, and origins
        // left without edges, dropped at the end unless admitted again.
        let mut added: BTreeMap<VoxelCoord, OriginEdges> = BTreeMap::new();
        let mut emptied = Vec::new();
        for (from, to, allowed) in changes {
            let record = match self.origins.get_mut(from) {
                Some(record) => record,
                None if allowed => added.entry(from).or_default(),
                None => match added.get_mut(&from) {
                    Some(record) => record,
                    None => continue,
                },
            };
            let inline = OriginEdges::inline_direction(from, to);
            let held = inline.is_some_and(|(direction, rise)| record.rise[direction] == rise)
                || record.others && self.others.get(&from).is_some_and(|set| set.contains(&to));
            if held == allowed {
                continue;
            }
            if allowed {
                self.admission_hash = self.admission_hash.wrapping_add(edge_hash(from, to));
                match inline.filter(|&(direction, _)| record.rise[direction] == NO_EDGE) {
                    Some((direction, rise)) => record.rise[direction] = rise,
                    None => {
                        record.others = true;
                        self.others.entry(from).or_default().insert(to);
                        if is_far(from, to) {
                            *self.reaches.entry(FarReach::of(from, to, 0)).or_default() += 1;
                        }
                    }
                }
                continue;
            }
            self.admission_hash = self.admission_hash.wrapping_sub(edge_hash(from, to));
            match inline.filter(|&(direction, rise)| record.rise[direction] == rise) {
                Some((direction, _)) => record.rise[direction] = NO_EDGE,
                None => {
                    let targets = self.others.get_mut(&from).expect("a held edge");
                    targets.remove(&to);
                    if targets.is_empty() {
                        self.others.remove(&from);
                        record.others = false;
                    }
                }
            }
            let extra = self.costs.remove(&(from, to));
            if let Some(extra) = extra {
                self.admission_hash = self
                    .admission_hash
                    .wrapping_sub(edge_cost_hash(from, to, extra));
                record.costed = has_costs_from(&self.costs, from);
            }
            if is_far(from, to) {
                remove_reach(
                    &mut self.reaches,
                    FarReach::of(from, to, extra.unwrap_or(0)),
                );
            }
            if record.is_empty() {
                emptied.push(from);
            }
        }
        for from in emptied {
            if self.origins.get(from).is_some_and(OriginEdges::is_empty) {
                self.origins.remove(from);
            }
        }
        added.retain(|_, record| !record.is_empty());
        self.origins.insert_new(added.into_iter().collect());
    }

    /// Sets the extra cost the weighted search pays for one admitted edge;
    /// zero removes it. The hash follows.
    pub fn set_cost(&mut self, from: VoxelCoord, to: VoxelCoord, cost: u64) {
        if !self.allows(from, to) {
            return;
        }
        let previous = self.costs.remove(&(from, to));
        if let Some(previous) = previous {
            self.admission_hash = self
                .admission_hash
                .wrapping_sub(edge_cost_hash(from, to, previous));
        }
        if cost > 0 {
            self.costs.insert((from, to), cost);
            self.admission_hash = self
                .admission_hash
                .wrapping_add(edge_cost_hash(from, to, cost));
        }
        if is_far(from, to) {
            remove_reach(
                &mut self.reaches,
                FarReach::of(from, to, previous.unwrap_or(0)),
            );
            *self
                .reaches
                .entry(FarReach::of(from, to, cost))
                .or_default() += 1;
        }
        let costed = has_costs_from(&self.costs, from);
        if let Some(record) = self.origins.get_mut(from) {
            record.costed = costed;
        }
    }

    /// The extra weighted cost of one edge, zero for most.
    pub fn extra_cost(&self, from: VoxelCoord, to: VoxelCoord) -> u64 {
        self.costs.get(&(from, to)).copied().unwrap_or(0)
    }

    /// Admitted edges from `from` that reach past its planar neighbours.
    pub fn beyond(&self, from: VoxelCoord) -> impl Iterator<Item = VoxelCoord> + '_ {
        self.others
            .get(&from)
            .into_iter()
            .flatten()
            .copied()
            .filter(move |&to| is_far(from, to))
    }

    /// Whether the directed step is admitted by the owning collision policy.
    pub fn allows(&self, from: VoxelCoord, to: VoxelCoord) -> bool {
        self.origins.get(from).is_some_and(|record| {
            record.holds(from, to)
                || record.others && self.others.get(&from).is_some_and(|set| set.contains(&to))
        })
    }

    /// Stable identity for the retained directed-edge policy.
    pub const fn admission_hash(&self) -> u64 {
        self.admission_hash
    }

    /// The least cost per column crossed that any admitted edge can cost, as
    /// `(cost, columns)`, at most one: an edge past the planar neighbours
    /// crosses several columns at once. A step costs one, or under `weighted`
    /// at least one plus the edge's extra cost.
    fn cost_per_column(&self, diagonal: bool, weighted: bool) -> (u64, u64) {
        self.reaches.keys().fold((1, 1), |least, reach| {
            let columns = if diagonal {
                reach.chebyshev
            } else {
                reach.manhattan
            };
            let cost = if weighted {
                reach.extra.saturating_add(1)
            } else {
                1
            };
            if u128::from(cost) * u128::from(least.1) < u128::from(least.0) * u128::from(columns) {
                (cost, columns)
            } else {
                least
            }
        })
    }
}

fn has_costs_from(costs: &BTreeMap<(VoxelCoord, VoxelCoord), u64>, from: VoxelCoord) -> bool {
    let lowest = VoxelCoord::new(i64::MIN, i64::MIN, i64::MIN);
    costs
        .range((from, lowest)..)
        .next()
        .is_some_and(|((origin, _), _)| *origin == from)
}

fn remove_reach(reaches: &mut BTreeMap<FarReach, usize>, reach: FarReach) {
    if let Some(count) = reaches.get_mut(&reach) {
        *count -= 1;
        if *count == 0 {
            reaches.remove(&reach);
        }
    }
}

/// The connected components of a projection's cells over an edge admission,
/// labelled once by the owner after it installs both, so that a query whose
/// goal no path can reach is answered without a search.
///
/// Cells are joined when an admitted edge the search would take leads from
/// one to the other, in either direction: a one-way drop joins the cells
/// above and below. So cells with different labels have no path between
/// them either way, while cells with the same label leave the one-way case
/// to the search. Traversal overlays are product facts applied at query
/// time and take no part in the labels.
///
/// The labels describe the projection and admission they were made from;
/// the owner labels again when either changes.
#[derive(Debug, Clone)]
pub struct NavComponents {
    projection_hash: u64,
    admission_hash: u64,
    /// Per projection position, its component.
    labels: Vec<u32>,
    /// The positions of each component in turn: component `c` holds
    /// `members[starts[c]..starts[c + 1]]`.
    members: Vec<usize>,
    starts: Vec<usize>,
}

impl NavComponents {
    /// Labels every cell of `projection` with its component over the edges
    /// of `edges` that a search under `policy` would take.
    pub fn label(
        projection: &NavProjection,
        edges: &NavEdgeAdmission,
        policy: PlanarNavNeighborPolicy,
    ) -> Self {
        let table = &projection.walkable;
        let positions = table.positions();
        let mut parent: Vec<usize> = (0..positions).collect();
        fn root(parent: &mut [usize], mut position: usize) -> usize {
            while parent[position] != position {
                parent[position] = parent[parent[position]];
                position = parent[position];
            }
            position
        }
        let offsets = planar_nav_offsets(policy.diagonal);
        let max_step = u64::from(policy.max_step_cells);
        let mut candidates = Vec::new();
        for (position, cell) in table.held() {
            candidates.clear();
            admitted_steps(edges, cell, offsets, max_step, &mut candidates);
            for &to in &candidates {
                let Some(next) = table.position(to) else {
                    continue;
                };
                let (a, b) = (root(&mut parent, position), root(&mut parent, next));
                parent[a.max(b)] = a.min(b);
            }
        }
        // Components are numbered as their first position comes.
        let mut labels = vec![u32::MAX; positions];
        let mut of_root = vec![u32::MAX; positions];
        let mut starts = vec![0; 1];
        for (position, _) in table.held() {
            let root = root(&mut parent, position);
            if of_root[root] == u32::MAX {
                of_root[root] = u32::try_from(starts.len() - 1).expect("fewer than 2^32 cells");
                starts.push(0);
            }
            labels[position] = of_root[root];
            starts[of_root[root] as usize + 1] += 1;
        }
        for component in 1..starts.len() {
            starts[component] += starts[component - 1];
        }
        let mut members = vec![0; table.len()];
        let mut next = starts.clone();
        for (position, _) in table.held() {
            let label = labels[position] as usize;
            members[next[label]] = position;
            next[label] += 1;
        }
        Self {
            projection_hash: projection.projection_hash(),
            admission_hash: edges.admission_hash(),
            labels,
            members,
            starts,
        }
    }

    /// How many components the cells form.
    pub fn len(&self) -> usize {
        self.starts.len() - 1
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// When no path from `start` to `goal` can exist, the cell of `start`'s
    /// component nearest the goal, ranked as a search ranks the cells it
    /// reached. Where the component holds one-way edges, the cell can be one
    /// the start does not reach. Both cells are walkable.
    fn separation(
        &self,
        projection: &NavProjection,
        edges: &NavEdgeAdmission,
        start: VoxelCoord,
        goal: VoxelCoord,
    ) -> Option<VoxelCoord> {
        debug_assert_eq!(
            (self.projection_hash, self.admission_hash),
            (projection.projection_hash(), edges.admission_hash()),
            "the components describe another projection or admission"
        );
        let table = &projection.walkable;
        let start = table.position(start)?;
        let goal_position = table.position(goal)?;
        let label = self.labels[start] as usize;
        if label == self.labels[goal_position] as usize {
            return None;
        }
        self.members[self.starts[label]..self.starts[label + 1]]
            .iter()
            .map(|&position| table.cell(position))
            .map(|cell| (squared_distance(cell, goal), cell))
            .min()
            .map(|(_, cell)| cell)
    }
}

/// The squared distance between two cells, by which a search ranks the cells
/// it reached by their nearness to the goal.
fn squared_distance(a: VoxelCoord, b: VoxelCoord) -> u128 {
    [a.x.abs_diff(b.x), a.y.abs_diff(b.y), a.z.abs_diff(b.z)]
        .map(|delta| u128::from(delta) * u128::from(delta))
        .into_iter()
        .fold(0_u128, u128::saturating_add)
}

/// Deterministic path readout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavPathReadout {
    pub outcome: NavPathOutcome,
    pub visited: usize,
    pub path: Vec<VoxelCoord>,
    pub path_hash: u64,
    /// On `NoPath`, the visited cell nearest the goal.
    pub nearest: Option<VoxelCoord>,
}

/// Maximum number of caller-supplied per-cell traversal records retained by
/// one navigation projection. This bounds both retained owner state and the
/// deterministic search lookup surface.
pub const MAX_NAV_TRAVERSAL_CELLS: usize = 65_536;

/// One purpose-neutral traversal rule for a projected navigation cell.
///
/// A missing cell remains allowed with the unit cost. Costs are charged when a
/// path enters a cell; the start cell contributes no traversal cost.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NavTraversalCell {
    pub coord: VoxelCoord,
    pub allowed: bool,
    pub cost: u64,
}

/// Canonical, owned per-cell traversal rules for a [`NavProjection`].
///
/// The map deliberately carries no product interpretation: callers decide
/// the values while this owner validates membership, admissibility, bounded
/// retention, deterministic lookup, and the stable hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavTraversalOverlay {
    cells: BTreeMap<VoxelCoord, NavTraversalCell>,
    overlay_hash: u64,
}

impl NavTraversalOverlay {
    /// Build canonical traversal rules for one already-admitted projection.
    /// Input order does not affect retained state or the overlay hash.
    pub fn from_cells(
        projection: &NavProjection,
        cells: impl IntoIterator<Item = NavTraversalCell>,
    ) -> Result<Self, NavTraversalOverlayError> {
        let mut canonical = BTreeMap::new();
        for cell in cells {
            if cell.cost == 0 {
                return Err(NavTraversalOverlayError::ZeroCost { coord: cell.coord });
            }
            if !projection.is_walkable(cell.coord) {
                return Err(NavTraversalOverlayError::CellOutsideProjection { coord: cell.coord });
            }
            if canonical.contains_key(&cell.coord) {
                return Err(NavTraversalOverlayError::DuplicateCell { coord: cell.coord });
            }
            if canonical.len() == MAX_NAV_TRAVERSAL_CELLS {
                return Err(NavTraversalOverlayError::TooManyCells {
                    maximum: MAX_NAV_TRAVERSAL_CELLS,
                });
            }
            canonical.insert(cell.coord, cell);
        }
        let overlay_hash = hash_traversal_cells(&canonical);
        Ok(Self {
            cells: canonical,
            overlay_hash,
        })
    }

    /// The empty overlay preserves ordinary allowed, unit-cost traversal.
    pub fn empty(projection: &NavProjection) -> Self {
        Self::from_cells(projection, std::iter::empty()).expect("empty overlay is valid")
    }

    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    pub fn overlay_hash(&self) -> u64 {
        self.overlay_hash
    }

    pub fn is_allowed(&self, coord: VoxelCoord) -> bool {
        self.cells.get(&coord).is_none_or(|cell| cell.allowed)
    }

    pub fn cost_for(&self, coord: VoxelCoord) -> u64 {
        self.cells.get(&coord).map_or(1, |cell| cell.cost)
    }
}

/// Why a caller-provided traversal overlay was rejected before replacement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavTraversalOverlayError {
    TooManyCells { maximum: usize },
    DuplicateCell { coord: VoxelCoord },
    CellOutsideProjection { coord: VoxelCoord },
    ZeroCost { coord: VoxelCoord },
}

/// One purpose-neutral traversal rule for a volumetric navigation cell.
///
/// Volumetric queries use dynamic voxel/agent-volume occupancy rather than the
/// planar [`NavProjection`] membership set.  Consequently these records are
/// deliberately not checked against a planar projection.  A missing record is
/// allowed with the unit cost, just as it is for [`NavTraversalOverlay`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VolumetricNavTraversalCell {
    pub coord: VoxelCoord,
    pub allowed: bool,
    pub cost: u64,
}

/// Canonical, bounded traversal rules for a volumetric navigation query.
///
/// The overlay is independent of the planar surface projection.  Admission
/// only owns the concerns that are meaningful for a caller-supplied set of
/// records: duplicate coordinates, non-zero costs, bounded retention, and a
/// deterministic hash.  Voxel occupancy and agent-volume validity remain
/// query concerns owned by the volumetric pathfinding service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolumetricNavTraversalOverlay {
    cells: BTreeMap<VoxelCoord, VolumetricNavTraversalCell>,
    overlay_hash: u64,
}

impl VolumetricNavTraversalOverlay {
    /// Build canonical traversal rules without requiring planar projection
    /// membership. Input order does not affect retained state or its hash.
    pub fn from_cells(
        cells: impl IntoIterator<Item = VolumetricNavTraversalCell>,
    ) -> Result<Self, VolumetricNavTraversalOverlayError> {
        let mut canonical = BTreeMap::new();
        for cell in cells {
            if cell.cost == 0 {
                return Err(VolumetricNavTraversalOverlayError::ZeroCost { coord: cell.coord });
            }
            if canonical.contains_key(&cell.coord) {
                return Err(VolumetricNavTraversalOverlayError::DuplicateCell {
                    coord: cell.coord,
                });
            }
            if canonical.len() == MAX_NAV_TRAVERSAL_CELLS {
                return Err(VolumetricNavTraversalOverlayError::TooManyCells {
                    maximum: MAX_NAV_TRAVERSAL_CELLS,
                });
            }
            canonical.insert(cell.coord, cell);
        }
        let overlay_hash = hash_volumetric_traversal_cells(&canonical);
        Ok(Self {
            cells: canonical,
            overlay_hash,
        })
    }

    /// The empty overlay preserves ordinary allowed, unit-cost traversal.
    pub fn empty() -> Self {
        Self::from_cells(std::iter::empty()).expect("empty overlay is valid")
    }

    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    pub fn overlay_hash(&self) -> u64 {
        self.overlay_hash
    }

    pub fn is_allowed(&self, coord: VoxelCoord) -> bool {
        self.cells.get(&coord).is_none_or(|cell| cell.allowed)
    }

    pub fn cost_for(&self, coord: VoxelCoord) -> u64 {
        self.cells.get(&coord).map_or(1, |cell| cell.cost)
    }
}

/// Why a caller-provided volumetric traversal overlay was rejected before
/// replacement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolumetricNavTraversalOverlayError {
    TooManyCells { maximum: usize },
    DuplicateCell { coord: VoxelCoord },
    ZeroCost { coord: VoxelCoord },
}

impl VolumetricNavTraversalOverlayError {
    pub const fn label(self) -> &'static str {
        match self {
            Self::TooManyCells { .. } => "tooManyCells",
            Self::DuplicateCell { .. } => "duplicateCell",
            Self::ZeroCost { .. } => "zeroCost",
        }
    }
}

impl NavTraversalOverlayError {
    pub const fn label(self) -> &'static str {
        match self {
            NavTraversalOverlayError::TooManyCells { .. } => "tooManyCells",
            NavTraversalOverlayError::DuplicateCell { .. } => "duplicateCell",
            NavTraversalOverlayError::CellOutsideProjection { .. } => "cellOutsideProjection",
            NavTraversalOverlayError::ZeroCost { .. } => "zeroCost",
        }
    }
}

/// Deterministic weighted-path outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeightedNavPathOutcome {
    Reached,
    NoPath,
    BudgetExhausted,
}

/// Deterministic weighted path facts, including the exact accumulated cost.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeightedNavPathReadout {
    pub outcome: WeightedNavPathOutcome,
    pub visited: usize,
    pub total_cost: u64,
    pub path: Vec<VoxelCoord>,
    pub path_hash: u64,
    pub overlay_hash: u64,
}

/// Why a weighted navigation query could not produce an ordinary readout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeightedNavPathError {
    InvalidQueryBudget,
    StartNotWalkable { start: VoxelCoord },
    GoalNotWalkable { goal: VoxelCoord },
    StartBlocked { start: VoxelCoord },
    GoalBlocked { goal: VoxelCoord },
    CostOverflow,
}

/// Which query substrate produced a path readout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavQueryMode {
    PlanarSurface,
    Volumetric3d,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavPathOutcome {
    Reached,
    NoPath,
}

/// A voxel-space agent volume used by opt-in volumetric navigation.
///
/// `find_volumetric_path` treats the query coordinate as the minimum corner of
/// this axis-aligned volume. Dimensions are measured in whole voxels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VolumetricAgentVolume {
    pub size_x: u32,
    pub size_y: u32,
    pub size_z: u32,
}

impl VolumetricAgentVolume {
    pub const fn single_cell() -> Self {
        Self {
            size_x: 1,
            size_y: 1,
            size_z: 1,
        }
    }

    pub const fn is_valid(self) -> bool {
        self.size_x > 0 && self.size_y > 0 && self.size_z > 0
    }
}

impl Default for VolumetricAgentVolume {
    fn default() -> Self {
        Self::single_cell()
    }
}

/// Deterministic neighbor set for opt-in volumetric navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolumetricNeighborSet {
    /// Four horizontal face neighbors in the same X/Z order as planar nav.
    Planar4,
    /// Six axis-aligned face neighbors: planar X/Z first, then +Y, then -Y.
    Faces6,
}

/// Whether vertical steps are allowed when the neighbor set contains them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolumetricVerticalPolicy {
    DisallowVertical,
    AllowVertical,
}

/// Which resident voxel values may be traversed by volumetric navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolumetricTraversalRule {
    EmptyCells,
    SolidCells,
}

/// Explicit opt-in configuration for 3D/volumetric pathfinding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VolumetricNavConfig {
    pub agent_volume: VolumetricAgentVolume,
    pub neighbor_set: VolumetricNeighborSet,
    pub vertical_policy: VolumetricVerticalPolicy,
    pub traversal_rule: VolumetricTraversalRule,
}

impl Default for VolumetricNavConfig {
    fn default() -> Self {
        Self {
            agent_volume: VolumetricAgentVolume::single_cell(),
            neighbor_set: VolumetricNeighborSet::Faces6,
            vertical_policy: VolumetricVerticalPolicy::AllowVertical,
            traversal_rule: VolumetricTraversalRule::EmptyCells,
        }
    }
}

/// Opt-in bounded 3D query over resident voxel authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VolumetricNavQuery {
    pub start: VoxelCoord,
    pub goal: VoxelCoord,
    pub max_visited: usize,
    pub config: VolumetricNavConfig,
}

/// Deterministic opt-in 3D path readout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolumetricNavReadout {
    pub mode: NavQueryMode,
    pub outcome: VolumetricNavOutcome,
    pub visited: usize,
    pub path_len: usize,
    pub path: Vec<VoxelCoord>,
    pub path_hash: u64,
}

/// Deterministic weighted path facts over resident voxel space.
///
/// `source_hash` identifies the resident voxel source used for occupancy
/// admission. It is intentionally distinct from the planar surface projection
/// hash because a volumetric route may occupy cells that are not walkable
/// surface cells at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeightedVolumetricNavPathReadout {
    pub outcome: WeightedNavPathOutcome,
    pub visited: usize,
    pub total_cost: u64,
    pub path: Vec<VoxelCoord>,
    pub path_hash: u64,
    pub source_hash: u64,
    pub overlay_hash: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolumetricNavOutcome {
    Reached,
    NoPath,
    BudgetExhausted,
}

/// Why nav projection/query failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavError {
    InvalidAgentHeight,
    InvalidQueryBudget,
    StartNotWalkable { start: VoxelCoord },
    GoalNotWalkable { goal: VoxelCoord },
}

/// Why an opt-in volumetric path query failed validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolumetricNavError {
    InvalidAgentVolume,
    InvalidQueryBudget,
    StartNotTraversable { start: VoxelCoord },
    GoalNotTraversable { goal: VoxelCoord },
}

/// Why a weighted volumetric path query could not produce an ordinary readout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeightedVolumetricNavPathError {
    InvalidAgentVolume,
    InvalidQueryBudget,
    StartNotTraversable { start: VoxelCoord },
    GoalNotTraversable { goal: VoxelCoord },
    StartBlocked { start: VoxelCoord },
    GoalBlocked { goal: VoxelCoord },
    CostOverflow,
}

/// A bounded live-position path request for an authority caller.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DirectNavMovementRequest {
    pub from: Vec3,
    pub target: Vec3,
    pub max_step_units: f32,
}

/// Deterministic direct-navigation movement proposal.
///
/// This readout is owned by `svc-pathfinding`; applying it to a runtime
/// transform remains a state/rule responsibility.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DirectNavMovementReadout {
    pub from: Vec3,
    pub target: Vec3,
    pub next_waypoint: Vec3,
    pub distance_units: f32,
    pub reached: bool,
    pub path_hash: u64,
}

/// A bounded live-position path-following request over a nav projection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProjectedDirectNavMovementRequest {
    pub from: Vec3,
    pub target: Vec3,
    pub max_step_units: f32,
    pub max_visited: usize,
}

/// Deterministic direct-navigation proposal backed by a [`NavProjection`].
///
/// The service keeps no internal path cache. `projection_hash`, `path_hash`, and
/// `movement_hash` are stable invalidation/readout tokens for callers that cache
/// projection or path-following work outside this crate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProjectedDirectNavMovementReadout {
    pub from: Vec3,
    pub target: Vec3,
    pub start: VoxelCoord,
    pub goal: VoxelCoord,
    pub next_path_cell: VoxelCoord,
    pub next_waypoint: Vec3,
    pub distance_to_waypoint_units: f32,
    pub reached: bool,
    pub visited: usize,
    pub path_len: usize,
    pub projection_hash: u64,
    pub path_hash: u64,
    pub movement_hash: u64,
}

/// Why a direct-nav movement request was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectNavMovementError {
    NonFinitePosition,
    InvalidStep,
}

impl DirectNavMovementError {
    pub const fn label(self) -> &'static str {
        match self {
            DirectNavMovementError::NonFinitePosition => "nonFinitePosition",
            DirectNavMovementError::InvalidStep => "invalidStep",
        }
    }
}

/// Why a projection-backed direct-nav request was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectedDirectNavMovementError {
    NonFinitePosition,
    InvalidStep,
    InvalidQueryBudget,
    StartNotWalkable { start: VoxelCoord },
    GoalNotWalkable { goal: VoxelCoord },
    NoPath { start: VoxelCoord, goal: VoxelCoord },
}

impl ProjectedDirectNavMovementError {
    pub const fn label(self) -> &'static str {
        match self {
            ProjectedDirectNavMovementError::NonFinitePosition => "nonFinitePosition",
            ProjectedDirectNavMovementError::InvalidStep => "invalidStep",
            ProjectedDirectNavMovementError::InvalidQueryBudget => "invalidQueryBudget",
            ProjectedDirectNavMovementError::StartNotWalkable { .. } => "startNotWalkable",
            ProjectedDirectNavMovementError::GoalNotWalkable { .. } => "goalNotWalkable",
            ProjectedDirectNavMovementError::NoPath { .. } => "noPath",
        }
    }
}

impl NavError {
    pub const fn label(self) -> &'static str {
        match self {
            NavError::InvalidAgentHeight => "invalidAgentHeight",
            NavError::InvalidQueryBudget => "invalidQueryBudget",
            NavError::StartNotWalkable { .. } => "startNotWalkable",
            NavError::GoalNotWalkable { .. } => "goalNotWalkable",
        }
    }
}

impl WeightedNavPathError {
    pub const fn label(self) -> &'static str {
        match self {
            WeightedNavPathError::InvalidQueryBudget => "invalidQueryBudget",
            WeightedNavPathError::StartNotWalkable { .. } => "startNotWalkable",
            WeightedNavPathError::GoalNotWalkable { .. } => "goalNotWalkable",
            WeightedNavPathError::StartBlocked { .. } => "startBlocked",
            WeightedNavPathError::GoalBlocked { .. } => "goalBlocked",
            WeightedNavPathError::CostOverflow => "costOverflow",
        }
    }
}

impl VolumetricNavError {
    pub const fn label(self) -> &'static str {
        match self {
            VolumetricNavError::InvalidAgentVolume => "invalidAgentVolume",
            VolumetricNavError::InvalidQueryBudget => "invalidQueryBudget",
            VolumetricNavError::StartNotTraversable { .. } => "startNotTraversable",
            VolumetricNavError::GoalNotTraversable { .. } => "goalNotTraversable",
        }
    }
}

impl WeightedVolumetricNavPathError {
    pub const fn label(self) -> &'static str {
        match self {
            Self::InvalidAgentVolume => "invalidAgentVolume",
            Self::InvalidQueryBudget => "invalidQueryBudget",
            Self::StartNotTraversable { .. } => "startNotTraversable",
            Self::GoalNotTraversable { .. } => "goalNotTraversable",
            Self::StartBlocked { .. } => "startBlocked",
            Self::GoalBlocked { .. } => "goalBlocked",
            Self::CostOverflow => "costOverflow",
        }
    }
}

/// Build a read-only nav projection from authoritative voxel data.
pub fn build_nav_projection(
    world: &VoxelWorld,
    config: NavProjectionConfig,
) -> Result<NavProjection, NavError> {
    if config.agent_height_voxels == 0 {
        return Err(NavError::InvalidAgentHeight);
    }
    let grid = world.grid();
    let mut walkable = Vec::new();
    for (chunk_coord, chunk) in world.resident_chunks() {
        for (local, value) in chunk.iter() {
            if value.is_solid() {
                continue;
            }
            let coord = grid.chunk_local_to_voxel(chunk_coord, local);
            if is_walkable_cell(world, coord, config) {
                walkable.push(coord);
            }
        }
    }
    Ok(NavProjection::from_walkable_cells(grid, walkable))
}

fn is_walkable_cell(world: &VoxelWorld, coord: VoxelCoord, config: NavProjectionConfig) -> bool {
    if config.require_solid_floor
        && !voxel_is_solid(world, VoxelCoord::new(coord.x, coord.y - 1, coord.z))
    {
        return false;
    }
    for dy in 0..config.agent_height_voxels {
        let check = VoxelCoord::new(coord.x, coord.y + dy as i64, coord.z);
        if voxel_is_solid(world, check) {
            return false;
        }
    }
    true
}

fn voxel_is_solid(world: &VoxelWorld, coord: VoxelCoord) -> bool {
    let grid = world.grid();
    let (chunk, local) = grid.voxel_to_chunk_local(coord);
    world
        .get(chunk)
        .and_then(|data| data.get(local))
        .is_some_and(|value| value.is_solid())
}

/// Query a deterministic shortest path through the nav projection.
pub fn find_path(
    projection: &NavProjection,
    query: NavPathQuery,
) -> Result<NavPathReadout, NavError> {
    find_path_with_policy(projection, query, PlanarNavNeighborPolicy::default())
}

/// Query a deterministic shortest path with explicit vertical step tolerance.
///
/// Candidate neighbors retain the canonical planar direction order. Within
/// each adjacent X/Z column, same-level cells are considered first, followed
/// by upward then downward cells at increasing distance. Between equally
/// short paths, the search prefers cells nearer the goal, then that order.
pub fn find_path_with_policy(
    projection: &NavProjection,
    query: NavPathQuery,
    policy: PlanarNavNeighborPolicy,
) -> Result<NavPathReadout, NavError> {
    find_path_with_optional_edge_admission(projection, query, policy, None)
}

/// Query a deterministic shortest path while requiring every transition to be
/// admitted by a collision-derived edge set. With `components`, a goal in
/// another component than the start is `NoPath` without a search: no cell
/// visited, and the nearest cell is the start's component's nearest the goal
/// (see [`NavComponents`]).
pub fn find_path_with_edge_admission(
    projection: &NavProjection,
    edges: &NavEdgeAdmission,
    components: Option<&NavComponents>,
    query: NavPathQuery,
    policy: PlanarNavNeighborPolicy,
) -> Result<NavPathReadout, NavError> {
    find_path_with_optional_edge_admission(projection, query, policy, Some((edges, components)))
}

fn find_path_with_optional_edge_admission(
    projection: &NavProjection,
    query: NavPathQuery,
    policy: PlanarNavNeighborPolicy,
    edges: Option<(&NavEdgeAdmission, Option<&NavComponents>)>,
) -> Result<NavPathReadout, NavError> {
    if query.max_visited == 0 {
        return Err(NavError::InvalidQueryBudget);
    }
    if !projection.is_walkable(query.start) {
        return Err(NavError::StartNotWalkable { start: query.start });
    }
    if !projection.is_walkable(query.goal) {
        return Err(NavError::GoalNotWalkable { goal: query.goal });
    }
    if let Some(nearest) = separated(projection, edges, query) {
        return Ok(nav_readout(PlanarSearch::Stopped {
            visited: 0,
            budget_spent: false,
            nearest,
        }));
    }
    let edges = edges.map(|(edges, _)| edges);
    match search_planar(projection, None, edges, query, policy, false) {
        Ok(search) => Ok(nav_readout(search)),
        Err(_) => unreachable!("a step of one cannot overflow"),
    }
}

/// The nearest cell a labelled query cannot reach its goal from, if the
/// labels say no path exists.
fn separated(
    projection: &NavProjection,
    edges: Option<(&NavEdgeAdmission, Option<&NavComponents>)>,
    query: NavPathQuery,
) -> Option<VoxelCoord> {
    let (edges, components) = edges?;
    components?.separation(projection, edges, query.start, query.goal)
}

/// Query a deterministic shortest path while respecting a retained traversal
/// overlay. This gives read-only assisted steps the ordinary planar query
/// budget semantics while product-owned blocked cells remain authoritative.
pub fn find_path_with_traversal_policy(
    projection: &NavProjection,
    overlay: &NavTraversalOverlay,
    query: NavPathQuery,
    policy: PlanarNavNeighborPolicy,
) -> Result<NavPathReadout, WeightedNavPathError> {
    find_path_with_traversal_and_optional_edge_admission(projection, overlay, query, policy, None)
}

/// Query a deterministic shortest path while respecting both caller-owned
/// traversal cells and collision-derived directed edge admission, with
/// `components` as in [`find_path_with_edge_admission`]. A query that fails
/// only because of the overlay still searches.
pub fn find_path_with_traversal_and_edge_admission(
    projection: &NavProjection,
    overlay: &NavTraversalOverlay,
    edges: &NavEdgeAdmission,
    components: Option<&NavComponents>,
    query: NavPathQuery,
    policy: PlanarNavNeighborPolicy,
) -> Result<NavPathReadout, WeightedNavPathError> {
    find_path_with_traversal_and_optional_edge_admission(
        projection,
        overlay,
        query,
        policy,
        Some((edges, components)),
    )
}

fn find_path_with_traversal_and_optional_edge_admission(
    projection: &NavProjection,
    overlay: &NavTraversalOverlay,
    query: NavPathQuery,
    policy: PlanarNavNeighborPolicy,
    edges: Option<(&NavEdgeAdmission, Option<&NavComponents>)>,
) -> Result<NavPathReadout, WeightedNavPathError> {
    check_traversal_query(projection, overlay, query)?;
    if let Some(nearest) = separated(projection, edges, query) {
        return Ok(nav_readout(PlanarSearch::Stopped {
            visited: 0,
            budget_spent: false,
            nearest,
        }));
    }
    let edges = edges.map(|(edges, _)| edges);
    search_planar(projection, Some(overlay), edges, query, policy, false).map(nav_readout)
}

/// Query a deterministic, bounded minimum-cost path through a planar
/// projection using caller-supplied [`NavTraversalOverlay`] facts.
///
/// This deliberately reuses the projection and planar-neighbor owners: it is
/// the same search as [`find_path_with_policy`], charging each cell's
/// traversal cost instead of one per step.
pub fn find_weighted_path_with_policy(
    projection: &NavProjection,
    overlay: &NavTraversalOverlay,
    query: NavPathQuery,
    policy: PlanarNavNeighborPolicy,
) -> Result<WeightedNavPathReadout, WeightedNavPathError> {
    find_weighted_path_with_optional_edge_admission(projection, overlay, query, policy, None)
}

/// Query a deterministic minimum-cost path while requiring collision-derived
/// directed edge admission in addition to the caller's cell traversal facts,
/// with `components` as in [`find_path_with_edge_admission`]: a goal in
/// another component is `NoPath` with no cell visited.
pub fn find_weighted_path_with_edge_admission(
    projection: &NavProjection,
    overlay: &NavTraversalOverlay,
    edges: &NavEdgeAdmission,
    components: Option<&NavComponents>,
    query: NavPathQuery,
    policy: PlanarNavNeighborPolicy,
) -> Result<WeightedNavPathReadout, WeightedNavPathError> {
    find_weighted_path_with_optional_edge_admission(
        projection,
        overlay,
        query,
        policy,
        Some((edges, components)),
    )
}

fn find_weighted_path_with_optional_edge_admission(
    projection: &NavProjection,
    overlay: &NavTraversalOverlay,
    query: NavPathQuery,
    policy: PlanarNavNeighborPolicy,
    edges: Option<(&NavEdgeAdmission, Option<&NavComponents>)>,
) -> Result<WeightedNavPathReadout, WeightedNavPathError> {
    check_traversal_query(projection, overlay, query)?;
    if separated(projection, edges, query).is_some() {
        return Ok(weighted_nav_readout(
            WeightedNavPathOutcome::NoPath,
            0,
            0,
            Vec::new(),
            overlay.overlay_hash(),
        ));
    }
    let edges = edges.map(|(edges, _)| edges);
    let (outcome, visited, total_cost, path) =
        match search_planar(projection, Some(overlay), edges, query, policy, true)? {
            PlanarSearch::Reached {
                visited,
                cost,
                path,
            } => (WeightedNavPathOutcome::Reached, visited, cost, path),
            PlanarSearch::Stopped {
                visited,
                budget_spent,
                ..
            } => (
                if budget_spent {
                    WeightedNavPathOutcome::BudgetExhausted
                } else {
                    WeightedNavPathOutcome::NoPath
                },
                visited,
                0,
                Vec::new(),
            ),
        };
    Ok(weighted_nav_readout(
        outcome,
        visited,
        total_cost,
        path,
        overlay.overlay_hash(),
    ))
}

fn check_traversal_query(
    projection: &NavProjection,
    overlay: &NavTraversalOverlay,
    query: NavPathQuery,
) -> Result<(), WeightedNavPathError> {
    if query.max_visited == 0 {
        return Err(WeightedNavPathError::InvalidQueryBudget);
    }
    if !projection.is_walkable(query.start) {
        return Err(WeightedNavPathError::StartNotWalkable { start: query.start });
    }
    if !projection.is_walkable(query.goal) {
        return Err(WeightedNavPathError::GoalNotWalkable { goal: query.goal });
    }
    if !overlay.is_allowed(query.start) {
        return Err(WeightedNavPathError::StartBlocked { start: query.start });
    }
    if !overlay.is_allowed(query.goal) {
        return Err(WeightedNavPathError::GoalBlocked { goal: query.goal });
    }
    Ok(())
}

fn nav_readout(search: PlanarSearch) -> NavPathReadout {
    match search {
        PlanarSearch::Reached { visited, path, .. } => NavPathReadout {
            outcome: NavPathOutcome::Reached,
            visited,
            path_hash: hash_path(&path),
            path,
            nearest: None,
        },
        PlanarSearch::Stopped {
            visited, nearest, ..
        } => NavPathReadout {
            outcome: NavPathOutcome::NoPath,
            visited,
            path: Vec::new(),
            path_hash: hash_path(&[]),
            nearest: Some(nearest),
        },
    }
}

/// What a planar search found.
enum PlanarSearch {
    Reached {
        visited: usize,
        cost: u64,
        path: Vec<VoxelCoord>,
    },
    /// The search expanded every cell it could reach, or `budget_spent`
    /// first. `nearest` is the cell it reached nearest the goal.
    Stopped {
        visited: usize,
        budget_spent: bool,
        nearest: VoxelCoord,
    },
}

/// A lower bound on the cost from a cell to the goal: the planar distance,
/// Chebyshev with diagonal neighbours (a diagonal costs one step, as an
/// orthogonal one does) and Manhattan without, scaled by the least cost an
/// admitted edge pays per column it crosses. Its rounding up keeps it
/// consistent, so a cell's first expansion is along a cheapest path.
struct PlanarEstimate {
    goal: VoxelCoord,
    diagonal: bool,
    cost: u64,
    columns: u64,
}

impl PlanarEstimate {
    fn of(&self, cell: VoxelCoord) -> u64 {
        let (across, along) = (cell.x.abs_diff(self.goal.x), cell.z.abs_diff(self.goal.z));
        let distance = if self.diagonal {
            u128::from(across.max(along))
        } else {
            u128::from(across) + u128::from(along)
        };
        let scaled = distance
            .saturating_mul(u128::from(self.cost))
            .div_ceil(u128::from(self.columns));
        u64::try_from(scaled).unwrap_or(u64::MAX)
    }
}

/// The cells from `start` to `goal` along the search's parents.
fn planar_path(
    table: &ColumnTable<()>,
    parent: &[usize],
    start: usize,
    goal: usize,
) -> Vec<VoxelCoord> {
    let mut path = vec![table.cell(goal)];
    let mut at = goal;
    while at != start {
        at = parent[at];
        path.push(table.cell(at));
    }
    path.reverse();
    path
}

const UNSEEN: u8 = 0;
const OPEN: u8 = 1;
const CLOSED: u8 = 2;

/// Per-position search state reused by each search on a thread, so that a
/// search costs the cells it reaches rather than the projection's size. It
/// keeps the size of the largest projection the thread has searched.
#[derive(Default)]
struct SearchScratch {
    state: Vec<u8>,
    best: Vec<u64>,
    parent: Vec<usize>,
    /// The positions the last search marked; every other one is unseen.
    touched: Vec<usize>,
    frontier: BinaryHeap<Reverse<(u64, u64, u64, usize)>>,
    candidates: Vec<VoxelCoord>,
}

thread_local! {
    static SEARCH_SCRATCH: RefCell<SearchScratch> = RefCell::new(SearchScratch::default());
}

impl SearchScratch {
    /// Ready for a search over `positions`, whatever the last search left.
    fn reset(&mut self, positions: usize) {
        for &position in &self.touched {
            self.state[position] = UNSEEN;
        }
        self.touched.clear();
        self.frontier.clear();
        if self.state.len() < positions {
            self.state.resize(positions, UNSEEN);
            self.best.resize(positions, 0);
            self.parent.resize(positions, 0);
        }
    }
}

/// A* from `query.start` to `query.goal`, both walkable and allowed. A step
/// costs one, or under `weighted` the entered cell's traversal cost plus the
/// edge's extra cost. Of cells with equal estimates, the one nearer the goal
/// is expanded first, then the one found first; neighbours are found in the
/// canonical order. At most `query.max_visited` cells are expanded, and
/// `visited` counts them.
///
/// Unweighted, the goal is reached as soon as it is found, counting as
/// visited, as in a breadth-first search: every step costs one and a cell
/// beside the goal is at least one column from it, so the cell being
/// expanded already lies on a shortest path.
fn search_planar(
    projection: &NavProjection,
    overlay: Option<&NavTraversalOverlay>,
    edges: Option<&NavEdgeAdmission>,
    query: NavPathQuery,
    policy: PlanarNavNeighborPolicy,
    weighted: bool,
) -> Result<PlanarSearch, WeightedNavPathError> {
    SEARCH_SCRATCH.with_borrow_mut(|scratch| {
        scratch.reset(projection.walkable.positions());
        search_planar_with(scratch, projection, overlay, edges, query, policy, weighted)
    })
}

fn search_planar_with(
    scratch: &mut SearchScratch,
    projection: &NavProjection,
    overlay: Option<&NavTraversalOverlay>,
    edges: Option<&NavEdgeAdmission>,
    query: NavPathQuery,
    policy: PlanarNavNeighborPolicy,
    weighted: bool,
) -> Result<PlanarSearch, WeightedNavPathError> {
    let SearchScratch {
        state,
        best,
        parent,
        touched,
        frontier,
        candidates,
    } = scratch;
    let table = &projection.walkable;
    let overlay = overlay.filter(|overlay| !overlay.is_empty());
    let (cost, columns) = edges.map_or((1, 1), |edges| {
        edges.cost_per_column(policy.diagonal, weighted)
    });
    let estimate = PlanarEstimate {
        goal: query.goal,
        diagonal: policy.diagonal,
        cost,
        columns,
    };
    let offsets = planar_nav_offsets(policy.diagonal);
    let max_step = u64::from(policy.max_step_cells);

    let start = table.position(query.start).expect("the start is walkable");
    let goal = table.position(query.goal).expect("the goal is walkable");
    // `Reverse` makes the max-heap a min-heap over (estimate, remaining,
    // order found, position); the order found makes every key distinct.
    let mut found = 0_u64;
    let distance_to_goal = |cell: VoxelCoord| squared_distance(cell, query.goal);
    let mut nearest = (distance_to_goal(query.start), query.start);
    state[start] = OPEN;
    best[start] = 0;
    touched.push(start);
    let remaining = estimate.of(query.start);
    frontier.push(Reverse((remaining, remaining, found, start)));
    let mut visited = 0_usize;

    while let Some(Reverse((_, _, _, position))) = frontier.pop() {
        if state[position] == CLOSED {
            continue;
        }
        if visited == query.max_visited {
            return Ok(PlanarSearch::Stopped {
                visited,
                budget_spent: true,
                nearest: nearest.1,
            });
        }
        visited += 1;
        state[position] = CLOSED;
        let current = table.cell(position);
        let cost = best[position];
        if position == goal {
            return Ok(PlanarSearch::Reached {
                visited,
                cost,
                path: planar_path(table, parent, start, goal),
            });
        }

        candidates.clear();
        let costed = match edges {
            Some(edges) => admitted_steps(edges, current, offsets, max_step, candidates),
            None => {
                column_steps(table, current, offsets, max_step, candidates);
                false
            }
        };

        for &next_cell in candidates.iter() {
            let Some(next) = table.position(next_cell) else {
                continue;
            };
            if overlay.is_some_and(|overlay| !overlay.is_allowed(next_cell)) {
                continue;
            }
            let next_cost = if weighted {
                let extra = match edges {
                    Some(edges) if costed => edges.extra_cost(current, next_cell),
                    _ => 0,
                };
                cost.checked_add(overlay.map_or(1, |overlay| overlay.cost_for(next_cell)))
                    .and_then(|cost| cost.checked_add(extra))
                    .ok_or(WeightedNavPathError::CostOverflow)?
            } else {
                cost.saturating_add(1)
            };
            match state[next] {
                CLOSED => continue,
                OPEN if best[next] <= next_cost => continue,
                UNSEEN => {
                    touched.push(next);
                    nearest = nearest.min((distance_to_goal(next_cell), next_cell));
                }
                _ => {}
            }
            state[next] = OPEN;
            best[next] = next_cost;
            parent[next] = position;
            if next == goal && !weighted {
                return Ok(PlanarSearch::Reached {
                    visited: visited + 1,
                    cost: next_cost,
                    path: planar_path(table, parent, start, goal),
                });
            }
            found += 1;
            let remaining = estimate.of(next_cell);
            frontier.push(Reverse((
                next_cost.saturating_add(remaining),
                remaining,
                found,
                next,
            )));
        }
    }
    Ok(PlanarSearch::Stopped {
        visited,
        budget_spent: false,
        nearest: nearest.1,
    })
}

/// Within an adjacent column, same-level cells come first, then upward and
/// downward cells at increasing distance.
fn rank(from: VoxelCoord, to: VoxelCoord) -> (u64, bool) {
    (from.y.abs_diff(to.y), to.y < from.y)
}

/// The cells a search steps to from `current` under `edges`, whether or not
/// they are walkable, appended to `candidates` in the canonical order: per
/// direction of `offsets` the edges within `max_step` by [`rank`], then the
/// edges past the planar neighbours. Returns whether an edge from `current`
/// carries an extra cost.
fn admitted_steps(
    edges: &NavEdgeAdmission,
    current: VoxelCoord,
    offsets: &[(i64, i64)],
    max_step: u64,
    candidates: &mut Vec<VoxelCoord>,
) -> bool {
    let Some(record) = edges.origins.get(current) else {
        return false;
    };
    let others = if record.others {
        edges.others.get(&current)
    } else {
        None
    };
    for (direction, &(dx, dz)) in offsets.iter().enumerate() {
        let first = candidates.len();
        let rise = record.rise[direction];
        if rise != NO_EDGE && u64::from(rise.unsigned_abs()) <= max_step {
            candidates.push(VoxelCoord::new(
                current.x + dx,
                current.y + i64::from(rise),
                current.z + dz,
            ));
        }
        candidates.extend(others.into_iter().flatten().copied().filter(|&to| {
            planar_direction(current, to) == Some(direction) && current.y.abs_diff(to.y) <= max_step
        }));
        candidates[first..].sort_by_key(|&to| rank(current, to));
    }
    candidates.extend(
        others
            .into_iter()
            .flatten()
            .copied()
            .filter(|&to| is_far(current, to)),
    );
    record.costed
}

/// The walkable cells a search without an edge admission steps to from
/// `current`: those of each adjacent column within `max_step`, appended to
/// `candidates` in the canonical order.
fn column_steps(
    table: &ColumnTable<()>,
    current: VoxelCoord,
    offsets: &[(i64, i64)],
    max_step: u64,
    candidates: &mut Vec<VoxelCoord>,
) {
    for &(dx, dz) in offsets {
        let (Some(x), Some(z)) = (current.x.checked_add(dx), current.z.checked_add(dz)) else {
            continue;
        };
        let first = candidates.len();
        let (_, column) = table.column(x, z);
        candidates.extend(
            column
                .iter()
                .copied()
                .filter(|to| current.y.abs_diff(to.y) <= max_step),
        );
        candidates[first..].sort_by_key(|&to| rank(current, to));
    }
}

/// Query a deterministic, bounded 3D path through resident voxel space.
///
/// This is intentionally separate from [`find_path`]: planar walkable-surface
/// navigation remains the default runtime movement substrate, while this opt-in
/// query exists for procgen/conformance checks that need vertical connectivity.
/// Missing/unloaded chunks are non-traversable so searches cannot leak into
/// infinite implicit empty space.
pub fn find_volumetric_path(
    world: &VoxelWorld,
    query: VolumetricNavQuery,
) -> Result<VolumetricNavReadout, VolumetricNavError> {
    if !query.config.agent_volume.is_valid() {
        return Err(VolumetricNavError::InvalidAgentVolume);
    }
    if query.max_visited == 0 {
        return Err(VolumetricNavError::InvalidQueryBudget);
    }
    if !is_volumetric_traversable(world, query.start, query.config) {
        return Err(VolumetricNavError::StartNotTraversable { start: query.start });
    }
    if !is_volumetric_traversable(world, query.goal, query.config) {
        return Err(VolumetricNavError::GoalNotTraversable { goal: query.goal });
    }
    if query.start == query.goal {
        let path = vec![query.start];
        return Ok(volumetric_readout(VolumetricNavOutcome::Reached, 1, path));
    }

    let mut queue = VecDeque::new();
    let mut visited = BTreeSet::new();
    let mut came_from = BTreeMap::new();
    queue.push_back(query.start);
    visited.insert(query.start);

    while let Some(current) = queue.pop_front() {
        for next in volumetric_neighbors(current, query.config) {
            if visited.contains(&next) {
                continue;
            }
            if !is_volumetric_traversable(world, next, query.config) {
                continue;
            }
            if visited.len() >= query.max_visited {
                return Ok(volumetric_readout(
                    VolumetricNavOutcome::BudgetExhausted,
                    visited.len(),
                    Vec::new(),
                ));
            }
            came_from.insert(next, current);
            visited.insert(next);
            if next == query.goal {
                let path = reconstruct_path(query.start, query.goal, &came_from);
                return Ok(volumetric_readout(
                    VolumetricNavOutcome::Reached,
                    visited.len(),
                    path,
                ));
            }
            queue.push_back(next);
        }
    }

    Ok(volumetric_readout(
        VolumetricNavOutcome::NoPath,
        visited.len(),
        Vec::new(),
    ))
}

/// Query a deterministic, bounded minimum-cost path through resident voxel
/// space using a purpose-neutral volumetric traversal overlay.
///
/// This reuses the ordinary volumetric occupancy, agent-volume, neighbor, and
/// budget rules. The overlay only changes admission and accumulated traversal
/// cost for coordinates it contains; omitted coordinates remain allowed at
/// unit cost. It does not consult the planar [`NavProjection`], because a
/// volumetric route is allowed to occupy cells outside that surface set.
pub fn find_weighted_volumetric_path(
    world: &VoxelWorld,
    overlay: &VolumetricNavTraversalOverlay,
    query: VolumetricNavQuery,
) -> Result<WeightedVolumetricNavPathReadout, WeightedVolumetricNavPathError> {
    if !query.config.agent_volume.is_valid() {
        return Err(WeightedVolumetricNavPathError::InvalidAgentVolume);
    }
    if query.max_visited == 0 {
        return Err(WeightedVolumetricNavPathError::InvalidQueryBudget);
    }
    if !is_volumetric_traversable(world, query.start, query.config) {
        return Err(WeightedVolumetricNavPathError::StartNotTraversable { start: query.start });
    }
    if !is_volumetric_traversable(world, query.goal, query.config) {
        return Err(WeightedVolumetricNavPathError::GoalNotTraversable { goal: query.goal });
    }
    if !overlay.is_allowed(query.start) {
        return Err(WeightedVolumetricNavPathError::StartBlocked { start: query.start });
    }
    if !overlay.is_allowed(query.goal) {
        return Err(WeightedVolumetricNavPathError::GoalBlocked { goal: query.goal });
    }

    let source_hash = volumetric_navigation_source_hash(world);
    if query.start == query.goal {
        let path = vec![query.start];
        return Ok(weighted_volumetric_readout(
            WeightedNavPathOutcome::Reached,
            1,
            0,
            path,
            source_hash,
            overlay.overlay_hash(),
        ));
    }

    // The priority tuple is `(cost, coordinate)`. `Reverse` makes Rust's
    // max-heap a min-heap while preserving the coordinate's stable total
    // ordering for equal-cost candidates.
    let mut frontier = BinaryHeap::new();
    let mut best_cost = BTreeMap::new();
    let mut came_from = BTreeMap::new();
    frontier.push(Reverse((0_u64, query.start)));
    best_cost.insert(query.start, 0_u64);
    let mut visited = 0_usize;

    while let Some(Reverse((cost, current))) = frontier.pop() {
        if best_cost.get(&current) != Some(&cost) {
            continue;
        }
        if visited == query.max_visited {
            return Ok(weighted_volumetric_readout(
                WeightedNavPathOutcome::BudgetExhausted,
                visited,
                0,
                Vec::new(),
                source_hash,
                overlay.overlay_hash(),
            ));
        }
        visited += 1;
        if current == query.goal {
            let path = reconstruct_path(query.start, query.goal, &came_from);
            return Ok(weighted_volumetric_readout(
                WeightedNavPathOutcome::Reached,
                visited,
                cost,
                path,
                source_hash,
                overlay.overlay_hash(),
            ));
        }

        for next in volumetric_neighbors(current, query.config) {
            if !overlay.is_allowed(next) || !is_volumetric_traversable(world, next, query.config) {
                continue;
            }
            let next_cost = cost
                .checked_add(overlay.cost_for(next))
                .ok_or(WeightedVolumetricNavPathError::CostOverflow)?;
            if best_cost
                .get(&next)
                .is_some_and(|known| *known <= next_cost)
            {
                continue;
            }
            best_cost.insert(next, next_cost);
            came_from.insert(next, current);
            frontier.push(Reverse((next_cost, next)));
        }
    }

    Ok(weighted_volumetric_readout(
        WeightedNavPathOutcome::NoPath,
        visited,
        0,
        Vec::new(),
        source_hash,
        overlay.overlay_hash(),
    ))
}

/// Propose one deterministic, bounded waypoint toward a live target position.
///
/// This is intentionally small: it is not a full pathfinding replacement, nor
/// does it mutate authority. It gives host callers a canonical service readout
/// for simple enemy approach behavior while fuller
/// voxel-derived path following remains on [`find_path`].
pub fn propose_direct_nav_movement(
    request: DirectNavMovementRequest,
) -> Result<DirectNavMovementReadout, DirectNavMovementError> {
    if !vec3_is_finite(request.from) || !vec3_is_finite(request.target) {
        return Err(DirectNavMovementError::NonFinitePosition);
    }
    if !request.max_step_units.is_finite() || request.max_step_units <= 0.0 {
        return Err(DirectNavMovementError::InvalidStep);
    }

    let delta = request.target - request.from;
    let distance = delta.length();
    let reached = distance <= request.max_step_units;
    let next_waypoint = if distance <= f32::EPSILON || reached {
        request.target
    } else {
        request.from + (delta * (request.max_step_units / distance))
    };
    let readout = DirectNavMovementReadout {
        from: round_vec3(request.from),
        target: round_vec3(request.target),
        next_waypoint: round_vec3(next_waypoint),
        distance_units: round_f32(distance),
        reached,
        path_hash: 0,
    };
    Ok(DirectNavMovementReadout {
        path_hash: hash_direct_nav_movement(&readout),
        ..readout
    })
}

/// Propose one deterministic, bounded waypoint using a nav projection path.
///
/// This helper converts the live positions into the projection's grid, runs
/// [`find_path`], and then moves toward either the next path cell center or the
/// final target when the next path cell is the goal. It does not mutate authority
/// and does not maintain an internal cache.
pub fn propose_projected_direct_nav_movement(
    projection: &NavProjection,
    request: ProjectedDirectNavMovementRequest,
) -> Result<ProjectedDirectNavMovementReadout, ProjectedDirectNavMovementError> {
    if !vec3_is_finite(request.from) || !vec3_is_finite(request.target) {
        return Err(ProjectedDirectNavMovementError::NonFinitePosition);
    }
    if !request.max_step_units.is_finite() || request.max_step_units <= 0.0 {
        return Err(ProjectedDirectNavMovementError::InvalidStep);
    }
    if request.max_visited == 0 {
        return Err(ProjectedDirectNavMovementError::InvalidQueryBudget);
    }

    let start = projection
        .grid()
        .world_to_voxel(vec3_to_world_pos(request.from));
    let goal = projection
        .grid()
        .world_to_voxel(vec3_to_world_pos(request.target));
    let path = find_path(
        projection,
        NavPathQuery {
            start,
            goal,
            max_visited: request.max_visited,
        },
    )
    .map_err(projected_error_from_nav)?;

    if path.outcome == NavPathOutcome::NoPath {
        return Err(ProjectedDirectNavMovementError::NoPath { start, goal });
    }

    let next_path_cell = path.path.get(1).copied().unwrap_or(start);
    let step_target = if next_path_cell == goal {
        request.target
    } else {
        world_pos_to_vec3(projection.grid().voxel_center_world(next_path_cell))
    };
    let movement = propose_direct_nav_movement(DirectNavMovementRequest {
        from: request.from,
        target: step_target,
        max_step_units: request.max_step_units,
    })
    .map_err(projected_error_from_direct)?;
    let reached = next_path_cell == goal && movement.reached;
    let mut readout = ProjectedDirectNavMovementReadout {
        from: round_vec3(request.from),
        target: round_vec3(request.target),
        start,
        goal,
        next_path_cell,
        next_waypoint: movement.next_waypoint,
        distance_to_waypoint_units: movement.distance_units,
        reached,
        visited: path.visited,
        path_len: path.path.len(),
        projection_hash: projection.projection_hash(),
        path_hash: path.path_hash,
        movement_hash: 0,
    };
    readout.movement_hash = hash_projected_direct_nav_movement(&readout);
    Ok(readout)
}

fn projected_error_from_nav(error: NavError) -> ProjectedDirectNavMovementError {
    match error {
        NavError::InvalidAgentHeight => ProjectedDirectNavMovementError::InvalidQueryBudget,
        NavError::InvalidQueryBudget => ProjectedDirectNavMovementError::InvalidQueryBudget,
        NavError::StartNotWalkable { start } => {
            ProjectedDirectNavMovementError::StartNotWalkable { start }
        }
        NavError::GoalNotWalkable { goal } => {
            ProjectedDirectNavMovementError::GoalNotWalkable { goal }
        }
    }
}

fn projected_error_from_direct(error: DirectNavMovementError) -> ProjectedDirectNavMovementError {
    match error {
        DirectNavMovementError::NonFinitePosition => {
            ProjectedDirectNavMovementError::NonFinitePosition
        }
        DirectNavMovementError::InvalidStep => ProjectedDirectNavMovementError::InvalidStep,
    }
}

fn nav_neighbors(coord: VoxelCoord) -> [VoxelCoord; 4] {
    [
        VoxelCoord::new(coord.x + 1, coord.y, coord.z),
        VoxelCoord::new(coord.x, coord.y, coord.z + 1),
        VoxelCoord::new(coord.x - 1, coord.y, coord.z),
        VoxelCoord::new(coord.x, coord.y, coord.z - 1),
    ]
}

/// X/Z neighbour offsets: orthogonal first, then diagonal when asked.
pub fn planar_nav_offsets(diagonal: bool) -> &'static [(i64, i64)] {
    const OFFSETS: [(i64, i64); 8] = [
        (1, 0),
        (0, 1),
        (-1, 0),
        (0, -1),
        (1, 1),
        (-1, 1),
        (-1, -1),
        (1, -1),
    ];
    if diagonal {
        &OFFSETS
    } else {
        &OFFSETS[..4]
    }
}

fn volumetric_neighbors(
    coord: VoxelCoord,
    config: VolumetricNavConfig,
) -> impl Iterator<Item = VoxelCoord> {
    let mut neighbors = Vec::with_capacity(6);
    neighbors.extend(nav_neighbors(coord));
    if config.neighbor_set == VolumetricNeighborSet::Faces6
        && config.vertical_policy == VolumetricVerticalPolicy::AllowVertical
    {
        neighbors.push(VoxelCoord::new(coord.x, coord.y + 1, coord.z));
        neighbors.push(VoxelCoord::new(coord.x, coord.y - 1, coord.z));
    }
    neighbors.into_iter()
}

fn is_volumetric_traversable(
    world: &VoxelWorld,
    coord: VoxelCoord,
    config: VolumetricNavConfig,
) -> bool {
    let volume = config.agent_volume;
    for dz in 0..volume.size_z {
        for dy in 0..volume.size_y {
            for dx in 0..volume.size_x {
                let check = VoxelCoord::new(
                    coord.x + dx as i64,
                    coord.y + dy as i64,
                    coord.z + dz as i64,
                );
                if !voxel_matches_traversal_rule(world, check, config.traversal_rule) {
                    return false;
                }
            }
        }
    }
    true
}

fn voxel_matches_traversal_rule(
    world: &VoxelWorld,
    coord: VoxelCoord,
    rule: VolumetricTraversalRule,
) -> bool {
    let grid = world.grid();
    let (chunk, local) = grid.voxel_to_chunk_local(coord);
    world
        .get(chunk)
        .and_then(|data| data.get(local))
        .is_some_and(|value| match rule {
            VolumetricTraversalRule::EmptyCells => value.is_empty(),
            VolumetricTraversalRule::SolidCells => value.is_solid(),
        })
}

fn volumetric_readout(
    outcome: VolumetricNavOutcome,
    visited: usize,
    path: Vec<VoxelCoord>,
) -> VolumetricNavReadout {
    VolumetricNavReadout {
        mode: NavQueryMode::Volumetric3d,
        outcome,
        visited,
        path_len: path.len(),
        path_hash: hash_path(&path),
        path,
    }
}

fn weighted_volumetric_readout(
    outcome: WeightedNavPathOutcome,
    visited: usize,
    total_cost: u64,
    path: Vec<VoxelCoord>,
    source_hash: u64,
    overlay_hash: u64,
) -> WeightedVolumetricNavPathReadout {
    WeightedVolumetricNavPathReadout {
        outcome,
        visited,
        total_cost,
        path_hash: hash_path(&path),
        path,
        source_hash,
        overlay_hash,
    }
}

fn weighted_nav_readout(
    outcome: WeightedNavPathOutcome,
    visited: usize,
    total_cost: u64,
    path: Vec<VoxelCoord>,
    overlay_hash: u64,
) -> WeightedNavPathReadout {
    WeightedNavPathReadout {
        outcome,
        visited,
        total_cost,
        path_hash: hash_path(&path),
        path,
        overlay_hash,
    }
}

fn reconstruct_path(
    start: VoxelCoord,
    goal: VoxelCoord,
    came_from: &BTreeMap<VoxelCoord, VoxelCoord>,
) -> Vec<VoxelCoord> {
    let mut path = vec![goal];
    let mut current = goal;
    while current != start {
        current = came_from[&current];
        path.push(current);
    }
    path.reverse();
    path
}

/// Human-reviewable deterministic summary used by committed fixtures.
pub fn describe_nav_path(projection: &NavProjection, readout: &NavPathReadout) -> String {
    let mut out = String::new();
    out.push_str("nav-path 1\n");
    out.push_str(&format!("walkable={}\n", projection.walkable_len()));
    out.push_str(&format!(
        "projection_hash={:016x}\n",
        projection.projection_hash()
    ));
    out.push_str(&format!("outcome={:?}\n", readout.outcome));
    out.push_str(&format!("visited={}\n", readout.visited));
    out.push_str(&format!("path_len={}\n", readout.path.len()));
    out.push_str("path=");
    for (index, coord) in readout.path.iter().enumerate() {
        if index > 0 {
            out.push_str(" -> ");
        }
        out.push_str(&format!("{},{},{}", coord.x, coord.y, coord.z));
    }
    out.push('\n');
    out.push_str(&format!("path_hash={:016x}\n", readout.path_hash));
    out
}

/// A projection hashes to the sum of its cells' hashes, so local refreshes
/// can maintain it cell by cell.
fn cell_hash(coord: VoxelCoord) -> u64 {
    let mut h = fnv_offset();
    feed_coord(&mut h, coord);
    h
}

/// A sum of per-edge hashes, like the walkable hash, so a local change
/// updates it without visiting every edge.
fn edge_cost_hash(from: VoxelCoord, to: VoxelCoord, cost: u64) -> u64 {
    let mut h = edge_hash(from, to);
    feed_u64(&mut h, cost);
    h.rotate_left(29)
}

fn edge_hash(from: VoxelCoord, to: VoxelCoord) -> u64 {
    let mut h = fnv_offset();
    feed_coord(&mut h, from);
    feed_coord(&mut h, to);
    h
}

fn hash_traversal_cells(cells: &BTreeMap<VoxelCoord, NavTraversalCell>) -> u64 {
    let mut h = fnv_offset();
    feed_u64(&mut h, cells.len() as u64);
    for cell in cells.values() {
        feed_coord(&mut h, cell.coord);
        feed_byte(&mut h, u8::from(cell.allowed));
        feed_u64(&mut h, cell.cost);
    }
    h
}

fn hash_volumetric_traversal_cells(
    cells: &BTreeMap<VoxelCoord, VolumetricNavTraversalCell>,
) -> u64 {
    let mut h = fnv_offset();
    feed_u64(&mut h, cells.len() as u64);
    for cell in cells.values() {
        feed_coord(&mut h, cell.coord);
        feed_byte(&mut h, u8::from(cell.allowed));
        feed_u64(&mut h, cell.cost);
    }
    h
}

/// Stable identity for the resident voxel source used by volumetric queries.
///
/// This intentionally does not use the planar `NavProjection` hash. It covers
/// the grid identity/shape/origin plus resident chunk coordinates and content,
/// so callers can distinguish a volumetric occupancy source even when its
/// surface projection is unchanged.
pub fn volumetric_navigation_source_hash(world: &VoxelWorld) -> u64 {
    let grid = world.grid();
    let mut h = fnv_offset();
    feed_u64(&mut h, u64::from(grid.id().raw()));
    feed_u64(&mut h, grid.voxel_size().to_bits());
    for dimension in grid.chunk_dims().to_array() {
        feed_u64(&mut h, u64::from(dimension));
    }
    for value in grid.origin_world().to_array() {
        feed_u64(&mut h, value.to_bits());
    }
    let residents = world.resident_chunks().collect::<Vec<_>>();
    feed_u64(&mut h, residents.len() as u64);
    for (coordinate, chunk) in residents {
        for value in coordinate.to_array() {
            feed_i64(&mut h, value);
        }
        feed_u64(&mut h, chunk.content_hash().0);
    }
    h
}

fn hash_path(path: &[VoxelCoord]) -> u64 {
    let mut h = fnv_offset();
    feed_u64(&mut h, path.len() as u64);
    for coord in path {
        feed_coord(&mut h, *coord);
    }
    h
}

fn feed_coord(h: &mut u64, coord: VoxelCoord) {
    for value in coord.to_array() {
        feed_i64(h, value);
    }
}

fn fnv_offset() -> u64 {
    0xcbf2_9ce4_8422_2325
}

fn feed_byte(h: &mut u64, b: u8) {
    *h ^= b as u64;
    *h = h.wrapping_mul(0x0000_0100_0000_01b3);
}

fn feed_i64(h: &mut u64, value: i64) {
    for b in value.to_le_bytes() {
        feed_byte(h, b);
    }
}

fn feed_u64(h: &mut u64, value: u64) {
    for b in value.to_le_bytes() {
        feed_byte(h, b);
    }
}

fn hash_direct_nav_movement(readout: &DirectNavMovementReadout) -> u64 {
    let mut h = fnv_offset();
    feed_vec3_bits(&mut h, readout.from);
    feed_vec3_bits(&mut h, readout.target);
    feed_vec3_bits(&mut h, readout.next_waypoint);
    feed_f32_bits(&mut h, readout.distance_units);
    feed_byte(&mut h, u8::from(readout.reached));
    h
}

fn hash_projected_direct_nav_movement(readout: &ProjectedDirectNavMovementReadout) -> u64 {
    let mut h = fnv_offset();
    feed_vec3_bits(&mut h, readout.from);
    feed_vec3_bits(&mut h, readout.target);
    feed_coord(&mut h, readout.start);
    feed_coord(&mut h, readout.goal);
    feed_coord(&mut h, readout.next_path_cell);
    feed_vec3_bits(&mut h, readout.next_waypoint);
    feed_f32_bits(&mut h, readout.distance_to_waypoint_units);
    feed_byte(&mut h, u8::from(readout.reached));
    feed_u64(&mut h, readout.visited as u64);
    feed_u64(&mut h, readout.path_len as u64);
    feed_u64(&mut h, readout.projection_hash);
    feed_u64(&mut h, readout.path_hash);
    h
}

fn feed_vec3_bits(h: &mut u64, value: Vec3) {
    feed_f32_bits(h, value.x);
    feed_f32_bits(h, value.y);
    feed_f32_bits(h, value.z);
}

fn feed_f32_bits(h: &mut u64, value: f32) {
    feed_u64(h, value.to_bits() as u64);
}

fn vec3_is_finite(value: Vec3) -> bool {
    value.x.is_finite() && value.y.is_finite() && value.z.is_finite()
}

fn round_vec3(value: Vec3) -> Vec3 {
    Vec3::new(round_f32(value.x), round_f32(value.y), round_f32(value.z))
}

fn round_f32(value: f32) -> f32 {
    (value * 1000.0).round() / 1000.0
}

fn vec3_to_world_pos(value: Vec3) -> WorldPos {
    WorldPos::new(value.x as f64, value.y as f64, value.z as f64)
}

fn world_pos_to_vec3(value: WorldPos) -> Vec3 {
    Vec3::new(value.x as f32, value.y as f32, value.z as f32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_space::{ChunkCoord, ChunkDims, GridId, LocalVoxelCoord, VoxelGridSpec};
    use core_voxel::VoxelValue;
    use svc_volume::VoxelChunk;

    fn projection() -> NavProjection {
        build_nav_projection(&tunnel_world(), NavProjectionConfig::default()).expect("nav")
    }

    fn tunnel_world() -> VoxelWorld {
        let grid = VoxelGridSpec::new(
            GridId::new(0),
            1.0,
            ChunkDims::new(8, 6, 12).expect("non-zero tunnel fixture dimensions"),
        )
        .expect("valid tunnel fixture grid");
        let mut chunk = VoxelChunk::from_spec(&grid);
        for z in 0..11 {
            for y in 0..6 {
                for x in 0..7 {
                    let shell = x == 0 || x == 6 || y == 0 || y == 5 || z == 0 || z == 10;
                    if shell {
                        chunk
                            .set(LocalVoxelCoord::new(x, y, z), VoxelValue::solid_raw(1))
                            .expect("tunnel fixture coordinate is in bounds");
                    }
                }
            }
        }
        chunk.mark_clean();
        let mut world = VoxelWorld::new(grid);
        world.insert(ChunkCoord::ORIGIN, chunk);
        world
    }

    #[test]
    fn host_walkable_cells_are_canonical_and_reuse_path_queries() {
        let voxel_projection = projection();
        let mut cells = voxel_projection.walkable_cells().collect::<Vec<_>>();
        cells.reverse();
        cells.push(cells[0]);

        let host_projection = NavProjection::from_walkable_cells(voxel_projection.grid(), cells);
        assert_eq!(host_projection, voxel_projection);
        assert_eq!(
            host_projection.projection_hash(),
            voxel_projection.projection_hash()
        );

        let (start, goal) = tunnel_nav_endpoints();
        assert_eq!(
            find_path(
                &host_projection,
                NavPathQuery {
                    start,
                    goal,
                    max_visited: 128,
                },
            ),
            find_path(
                &voxel_projection,
                NavPathQuery {
                    start,
                    goal,
                    max_visited: 128,
                },
            ),
        );
    }

    #[test]
    fn planar_step_policy_connects_a_staircase_in_both_directions() {
        let cells = [
            VoxelCoord::new(0, 0, 0),
            VoxelCoord::new(1, 1, 0),
            VoxelCoord::new(2, 2, 0),
            VoxelCoord::new(3, 3, 0),
        ];
        let projection = NavProjection::from_walkable_cells(test_grid(), cells);
        let policy = PlanarNavNeighborPolicy {
            max_step_cells: 1,
            diagonal: false,
        };

        for (start, goal, expected) in [
            (cells[0], cells[3], cells.to_vec()),
            (cells[3], cells[0], cells.into_iter().rev().collect()),
        ] {
            let readout = find_path_with_policy(
                &projection,
                NavPathQuery {
                    start,
                    goal,
                    max_visited: 16,
                },
                policy,
            )
            .expect("stair path");
            assert_eq!(readout.outcome, NavPathOutcome::Reached);
            assert_eq!(readout.path, expected);
            assert_eq!(readout.path_hash, hash_path(&expected));
        }
    }

    #[test]
    fn planar_step_policy_rejects_ledges_above_tolerance() {
        let start = VoxelCoord::new(0, 3, 0);
        let goal = VoxelCoord::new(1, 1, 0);
        let projection = NavProjection::from_walkable_cells(test_grid(), [start, goal]);

        let blocked = find_path_with_policy(
            &projection,
            NavPathQuery {
                start,
                goal,
                max_visited: 16,
            },
            PlanarNavNeighborPolicy {
                max_step_cells: 1,
                diagonal: false,
            },
        )
        .expect("bounded drop query");
        assert_eq!(blocked.outcome, NavPathOutcome::NoPath);

        let allowed = find_path_with_policy(
            &projection,
            NavPathQuery {
                start,
                goal,
                max_visited: 16,
            },
            PlanarNavNeighborPolicy {
                max_step_cells: 2,
                diagonal: false,
            },
        )
        .expect("explicit drop query");
        assert_eq!(allowed.path, vec![start, goal]);
    }

    /// Cells added and removed singly and in batches, beyond the first cells'
    /// box and into columns already full or emptied, leave the projection
    /// that the final cells give at once.
    #[test]
    fn cells_changed_in_place_match_the_cells_at_once() {
        let cell =
            |i: i64| VoxelCoord::new((i * 37) % 300 - 100, (i * 13) % 7, (i * 91) % 200 - 50);
        let mut projection = NavProjection::from_walkable_cells(test_grid(), (0..50).map(cell));
        let mut walkable: BTreeSet<_> = (0..50).map(cell).collect();
        let mut i = 0;
        while i < 5_000 {
            let batch: Vec<_> = (i..i + i % 7 + 1).map(|i| (cell(i), i % 3 != 0)).collect();
            projection.set_walkable_cells(batch.iter().copied());
            for (cell, walk) in batch {
                if walk {
                    walkable.insert(cell);
                } else {
                    walkable.remove(&cell);
                }
            }
            i += i % 7 + 1;
        }
        assert_eq!(
            projection,
            NavProjection::from_walkable_cells(test_grid(), walkable.iter().copied())
        );
        assert!(projection.walkable_cells().eq(walkable.iter().copied()));

        // A column emptied inside the box, refilled beside one outside it.
        let row = |x: i64| VoxelCoord::new(x, 0, 0);
        let held = [0, 5, 6, 7, 8, 9, 10];
        let mut projection = NavProjection::from_walkable_cells(test_grid(), held.map(row));
        projection.set_walkable(row(0), false);
        projection.set_walkable_cells([(row(0), true), (row(20), true)]);
        assert_eq!(
            projection,
            NavProjection::from_walkable_cells(
                test_grid(),
                held.map(row).into_iter().chain([row(20)])
            )
        );
    }

    /// Cells too far apart for a box of columns keep only their own columns
    /// and are searched as any others.
    #[test]
    fn far_apart_cells_route_within_and_report_between() {
        const FAR: i64 = 1_000_000_000;
        let row = |x0: i64| (0..4).map(move |x| VoxelCoord::new(x0 + x, 0, 0));
        let query = |start: VoxelCoord, goal: VoxelCoord| NavPathQuery {
            start,
            goal,
            max_visited: 64,
        };
        let mut projection =
            NavProjection::from_walkable_cells(test_grid(), row(0).chain(row(FAR)));
        let along = find_path(
            &projection,
            query(VoxelCoord::new(FAR, 0, 0), VoxelCoord::new(FAR + 3, 0, 0)),
        )
        .unwrap();
        assert_eq!(along.path, row(FAR).collect::<Vec<_>>());
        let between = find_path(
            &projection,
            query(VoxelCoord::new(0, 0, 0), VoxelCoord::new(FAR, 0, 0)),
        )
        .unwrap();
        assert_eq!(
            (between.outcome, between.visited, between.nearest),
            (NavPathOutcome::NoPath, 4, Some(VoxelCoord::new(3, 0, 0)))
        );
        // Growing by another far row is the projection built with it.
        projection.set_walkable_cells(row(-FAR).map(|cell| (cell, true)));
        assert_eq!(
            projection,
            NavProjection::from_walkable_cells(
                test_grid(),
                row(0).chain(row(FAR)).chain(row(-FAR))
            )
        );
        let grown = find_path(
            &projection,
            query(VoxelCoord::new(-FAR + 3, 0, 0), VoxelCoord::new(-FAR, 0, 0)),
        )
        .unwrap();
        assert_eq!(grown.path.len(), 4);
    }

    /// An admitted edge across a gap is a neighbour too, and its extra cost
    /// sends the weighted search the long way round when that is cheaper.
    #[test]
    fn far_admitted_edges_are_neighbours_with_their_extra_cost() {
        let (start, goal) = (VoxelCoord::new(0, 0, 0), VoxelCoord::new(4, 0, 0));
        // Two islands joined only by the gap edge, and a detour row.
        let detour: Vec<_> = (0..5).map(|x| VoxelCoord::new(x, 0, 1)).collect();
        let cells = [start, goal].into_iter().chain(detour.iter().copied());
        let projection = NavProjection::from_walkable_cells(test_grid(), cells);
        let policy = PlanarNavNeighborPolicy {
            max_step_cells: 0,
            diagonal: false,
        };
        let neighbours = |coord: VoxelCoord| {
            [(1, 0), (-1, 0), (0, 1), (0, -1)]
                .map(|(dx, dz)| VoxelCoord::new(coord.x + dx, coord.y, coord.z + dz))
        };
        let mut edges = NavEdgeAdmission::from_allowed_edges(
            [start, goal]
                .into_iter()
                .chain(detour.iter().copied())
                .flat_map(|from| neighbours(from).map(move |to| (from, to))),
        );
        let without_gap = edges.admission_hash();
        edges.set_allowed(start, goal, true);
        assert_ne!(edges.admission_hash(), without_gap);
        let query = NavPathQuery {
            start,
            goal,
            max_visited: 64,
        };
        let path = find_path_with_edge_admission(&projection, &edges, None, query, policy).unwrap();
        assert_eq!(path.path, vec![start, goal]);

        let overlay = NavTraversalOverlay::empty(&projection);
        let weighted = |edges: &NavEdgeAdmission| {
            find_weighted_path_with_edge_admission(
                &projection,
                &overlay,
                edges,
                None,
                query,
                policy,
            )
            .unwrap()
        };
        assert_eq!(weighted(&edges).path, vec![start, goal]);
        edges.set_cost(start, goal, 10);
        let around = weighted(&edges);
        assert_eq!(around.path.len(), 7, "{:?}", around.path);
        // Withdrawing the edge drops its cost and restores the hash.
        edges.set_allowed(start, goal, false);
        assert_eq!(edges.admission_hash(), without_gap);
    }

    /// A row of cells with every step between neighbours admitted both ways.
    fn admitted_row(cells: &[VoxelCoord]) -> impl Iterator<Item = (VoxelCoord, VoxelCoord)> + '_ {
        cells
            .windows(2)
            .flat_map(|pair| [(pair[0], pair[1]), (pair[1], pair[0])])
    }

    #[test]
    fn components_answer_a_disconnected_goal_without_a_search() {
        let near: Vec<_> = (0..4).map(|x| VoxelCoord::new(x, 0, 0)).collect();
        let far: Vec<_> = (0..4).map(|x| VoxelCoord::new(x, 0, 2)).collect();
        let projection =
            NavProjection::from_walkable_cells(test_grid(), near.iter().chain(&far).copied());
        let edges =
            NavEdgeAdmission::from_allowed_edges(admitted_row(&near).chain(admitted_row(&far)));
        let policy = PlanarNavNeighborPolicy {
            max_step_cells: 1,
            diagonal: true,
        };
        let components = NavComponents::label(&projection, &edges, policy);
        assert_eq!(components.len(), 2);
        let overlay = NavTraversalOverlay::empty(&projection);
        let query = NavPathQuery {
            start: near[0],
            goal: far[3],
            max_visited: 64,
        };
        let searched =
            find_path_with_edge_admission(&projection, &edges, None, query, policy).unwrap();
        let labelled =
            find_path_with_edge_admission(&projection, &edges, Some(&components), query, policy)
                .unwrap();
        assert_eq!(searched.outcome, NavPathOutcome::NoPath);
        assert_eq!(searched.visited, 4);
        assert_eq!(labelled.outcome, NavPathOutcome::NoPath);
        assert_eq!(labelled.visited, 0);
        assert_eq!(labelled.nearest, searched.nearest);
        assert_eq!(labelled.nearest, Some(near[3]));
        let stepped = find_path_with_traversal_and_edge_admission(
            &projection,
            &overlay,
            &edges,
            Some(&components),
            query,
            policy,
        )
        .unwrap();
        assert_eq!(stepped, labelled);
        let weighted = find_weighted_path_with_edge_admission(
            &projection,
            &overlay,
            &edges,
            Some(&components),
            query,
            policy,
        )
        .unwrap();
        assert_eq!(weighted.outcome, WeightedNavPathOutcome::NoPath);
        assert_eq!(weighted.visited, 0);
        // Within a component the search runs as before.
        let within = NavPathQuery {
            goal: near[3],
            ..query
        };
        assert_eq!(
            find_path_with_edge_admission(&projection, &edges, Some(&components), within, policy),
            find_path_with_edge_admission(&projection, &edges, None, within, policy)
        );
    }

    /// Labels join the cells of a one-way edge, and take no account of a
    /// traversal overlay: both leave the answer to the search.
    #[test]
    fn one_way_edges_and_overlays_leave_the_search_to_run() {
        let cells: Vec<_> = (0..3).map(|x| VoxelCoord::new(x, 0, 0)).collect();
        let projection = NavProjection::from_walkable_cells(test_grid(), cells.iter().copied());
        // The last step is a drop: admitted one way only.
        let edges = NavEdgeAdmission::from_allowed_edges(
            admitted_row(&cells[..2]).chain([(cells[1], cells[2])]),
        );
        let policy = PlanarNavNeighborPolicy {
            max_step_cells: 1,
            diagonal: false,
        };
        let components = NavComponents::label(&projection, &edges, policy);
        assert_eq!(components.len(), 1);
        let back = find_path_with_edge_admission(
            &projection,
            &edges,
            Some(&components),
            NavPathQuery {
                start: cells[2],
                goal: cells[0],
                max_visited: 64,
            },
            policy,
        )
        .unwrap();
        assert_eq!(back.outcome, NavPathOutcome::NoPath);
        assert_eq!(back.visited, 1);
        assert_eq!(back.nearest, Some(cells[2]));
        let overlay = NavTraversalOverlay::from_cells(
            &projection,
            [NavTraversalCell {
                coord: cells[1],
                allowed: false,
                cost: 1,
            }],
        )
        .unwrap();
        let blocked = find_path_with_traversal_and_edge_admission(
            &projection,
            &overlay,
            &edges,
            Some(&components),
            NavPathQuery {
                start: cells[0],
                goal: cells[2],
                max_visited: 64,
            },
            policy,
        )
        .unwrap();
        assert_eq!(blocked.outcome, NavPathOutcome::NoPath);
        assert_eq!(blocked.visited, 1);
    }

    #[test]
    fn weighted_overlay_prefers_a_longer_lower_cost_route() {
        let start = VoxelCoord::new(0, 0, 0);
        let goal = VoxelCoord::new(4, 0, 0);
        let direct = [
            start,
            VoxelCoord::new(1, 0, 0),
            VoxelCoord::new(2, 0, 0),
            VoxelCoord::new(3, 0, 0),
            goal,
        ];
        let detour = [
            VoxelCoord::new(0, 0, 1),
            VoxelCoord::new(1, 0, 1),
            VoxelCoord::new(2, 0, 1),
            VoxelCoord::new(3, 0, 1),
            VoxelCoord::new(4, 0, 1),
        ];
        let projection =
            NavProjection::from_walkable_cells(test_grid(), direct.into_iter().chain(detour));
        let overlay = NavTraversalOverlay::from_cells(
            &projection,
            [
                NavTraversalCell {
                    coord: direct[1],
                    allowed: true,
                    cost: 10,
                },
                NavTraversalCell {
                    coord: direct[2],
                    allowed: true,
                    cost: 10,
                },
                NavTraversalCell {
                    coord: direct[3],
                    allowed: true,
                    cost: 10,
                },
            ],
        )
        .expect("overlay");

        let readout = find_weighted_path_with_policy(
            &projection,
            &overlay,
            NavPathQuery {
                start,
                goal,
                max_visited: 32,
            },
            PlanarNavNeighborPolicy::default(),
        )
        .expect("weighted path");

        assert_eq!(readout.outcome, WeightedNavPathOutcome::Reached);
        assert_eq!(
            readout.path,
            vec![start, detour[0], detour[1], detour[2], detour[3], detour[4], goal]
        );
        assert_eq!(readout.total_cost, 6);
        assert_eq!(readout.path_hash, hash_path(&readout.path));
    }

    #[test]
    fn weighted_overlay_blocks_cells_and_reports_no_path() {
        let start = VoxelCoord::new(0, 0, 0);
        let goal = VoxelCoord::new(2, 0, 0);
        let alternate = [
            VoxelCoord::new(0, 0, 1),
            VoxelCoord::new(1, 0, 1),
            VoxelCoord::new(2, 0, 1),
        ];
        let direct = [start, VoxelCoord::new(1, 0, 0), goal];
        let projection =
            NavProjection::from_walkable_cells(test_grid(), direct.into_iter().chain(alternate));

        let fallback = NavTraversalOverlay::from_cells(
            &projection,
            [NavTraversalCell {
                coord: direct[1],
                allowed: false,
                cost: 1,
            }],
        )
        .expect("fallback overlay");
        let fallback_readout = find_weighted_path_with_policy(
            &projection,
            &fallback,
            NavPathQuery {
                start,
                goal,
                max_visited: 16,
            },
            PlanarNavNeighborPolicy::default(),
        )
        .expect("fallback path");
        assert_eq!(fallback_readout.outcome, WeightedNavPathOutcome::Reached);
        assert_eq!(
            fallback_readout.path,
            vec![start, alternate[0], alternate[1], alternate[2], goal]
        );

        let no_path = NavTraversalOverlay::from_cells(
            &projection,
            [
                NavTraversalCell {
                    coord: direct[1],
                    allowed: false,
                    cost: 1,
                },
                NavTraversalCell {
                    coord: alternate[1],
                    allowed: false,
                    cost: 1,
                },
            ],
        )
        .expect("no-path overlay");
        let no_path_readout = find_weighted_path_with_policy(
            &projection,
            &no_path,
            NavPathQuery {
                start,
                goal,
                max_visited: 16,
            },
            PlanarNavNeighborPolicy::default(),
        )
        .expect("no-path readout");
        assert_eq!(no_path_readout.outcome, WeightedNavPathOutcome::NoPath);
        assert!(no_path_readout.path.is_empty());
    }

    #[test]
    fn traversal_overlay_is_canonical_and_rejects_invalid_records() {
        let cells = [
            VoxelCoord::new(0, 0, 0),
            VoxelCoord::new(1, 0, 0),
            VoxelCoord::new(2, 0, 0),
        ];
        let projection = NavProjection::from_walkable_cells(test_grid(), cells);
        let records = [
            NavTraversalCell {
                coord: cells[1],
                allowed: false,
                cost: 7,
            },
            NavTraversalCell {
                coord: cells[2],
                allowed: true,
                cost: 3,
            },
        ];
        let forward = NavTraversalOverlay::from_cells(&projection, records).expect("forward");
        let reverse = NavTraversalOverlay::from_cells(&projection, records.into_iter().rev())
            .expect("reverse");
        assert_eq!(forward, reverse);
        assert_eq!(forward.overlay_hash(), reverse.overlay_hash());
        assert_eq!(forward.len(), 2);

        assert_eq!(
            NavTraversalOverlay::from_cells(
                &projection,
                [NavTraversalCell {
                    coord: cells[1],
                    allowed: true,
                    cost: 0,
                }],
            ),
            Err(NavTraversalOverlayError::ZeroCost { coord: cells[1] })
        );
        assert_eq!(
            NavTraversalOverlay::from_cells(
                &projection,
                [
                    NavTraversalCell {
                        coord: cells[1],
                        allowed: true,
                        cost: 1,
                    },
                    NavTraversalCell {
                        coord: cells[1],
                        allowed: false,
                        cost: 1,
                    },
                ],
            ),
            Err(NavTraversalOverlayError::DuplicateCell { coord: cells[1] })
        );
        let outside = VoxelCoord::new(9, 0, 0);
        assert_eq!(
            NavTraversalOverlay::from_cells(
                &projection,
                [NavTraversalCell {
                    coord: outside,
                    allowed: true,
                    cost: 1,
                }],
            ),
            Err(NavTraversalOverlayError::CellOutsideProjection { coord: outside })
        );

        let bounded_cells = (0..=MAX_NAV_TRAVERSAL_CELLS)
            .map(|x| VoxelCoord::new(x as i64, 1, 0))
            .collect::<Vec<_>>();
        let bounded_projection =
            NavProjection::from_walkable_cells(test_grid(), bounded_cells.iter().copied());
        assert_eq!(
            NavTraversalOverlay::from_cells(
                &bounded_projection,
                bounded_cells.iter().copied().map(|coord| NavTraversalCell {
                    coord,
                    allowed: true,
                    cost: 1,
                }),
            ),
            Err(NavTraversalOverlayError::TooManyCells {
                maximum: MAX_NAV_TRAVERSAL_CELLS
            })
        );
    }

    #[test]
    fn weighted_search_reports_budget_and_checked_cost_overflow() {
        let cells = [
            VoxelCoord::new(0, 0, 0),
            VoxelCoord::new(1, 0, 0),
            VoxelCoord::new(2, 0, 0),
        ];
        let projection = NavProjection::from_walkable_cells(test_grid(), cells);
        let empty = NavTraversalOverlay::empty(&projection);
        let budget = find_weighted_path_with_policy(
            &projection,
            &empty,
            NavPathQuery {
                start: cells[0],
                goal: cells[2],
                max_visited: 1,
            },
            PlanarNavNeighborPolicy::default(),
        )
        .expect("budget readout");
        assert_eq!(budget.outcome, WeightedNavPathOutcome::BudgetExhausted);
        assert_eq!(budget.visited, 1);

        let overflow = NavTraversalOverlay::from_cells(
            &projection,
            [
                NavTraversalCell {
                    coord: cells[1],
                    allowed: true,
                    cost: u64::MAX,
                },
                NavTraversalCell {
                    coord: cells[2],
                    allowed: true,
                    cost: 1,
                },
            ],
        )
        .expect("overflow overlay");
        assert_eq!(
            find_weighted_path_with_policy(
                &projection,
                &overflow,
                NavPathQuery {
                    start: cells[0],
                    goal: cells[2],
                    max_visited: 8,
                },
                PlanarNavNeighborPolicy::default(),
            ),
            Err(WeightedNavPathError::CostOverflow)
        );
    }

    fn tunnel_nav_endpoints() -> (VoxelCoord, VoxelCoord) {
        (VoxelCoord::new(4, 1, 8), VoxelCoord::new(2, 1, 2))
    }

    fn cell_center(projection: &NavProjection, coord: VoxelCoord) -> Vec3 {
        world_pos_to_vec3(projection.grid().voxel_center_world(coord))
    }

    fn test_grid() -> VoxelGridSpec {
        VoxelGridSpec::new(GridId::new(99), 1.0, ChunkDims::cubic(8).unwrap()).unwrap()
    }

    fn solid_test_world() -> VoxelWorld {
        let grid = test_grid();
        let mut world = VoxelWorld::new(grid);
        world.insert(
            ChunkCoord::ORIGIN,
            VoxelChunk::filled(grid.id(), grid.chunk_dims(), VoxelValue::solid_raw(1)),
        );
        world
    }

    fn set_test_voxel(world: &mut VoxelWorld, coord: VoxelCoord, value: VoxelValue) {
        let grid = world.grid();
        let (chunk, local) = grid.voxel_to_chunk_local(coord);
        world
            .get_mut(chunk)
            .expect("resident test chunk")
            .set(LocalVoxelCoord::new(local.x, local.y, local.z), value)
            .expect("local coordinate in bounds");
    }

    fn carve_empty(world: &mut VoxelWorld, coord: VoxelCoord) {
        set_test_voxel(world, coord, VoxelValue::EMPTY);
    }

    fn volumetric_query(
        start: VoxelCoord,
        goal: VoxelCoord,
        max_visited: usize,
    ) -> VolumetricNavQuery {
        VolumetricNavQuery {
            start,
            goal,
            max_visited,
            config: VolumetricNavConfig::default(),
        }
    }

    #[test]
    fn generated_tunnel_has_reachable_player_path() {
        let projection = projection();
        let (start, goal) = tunnel_nav_endpoints();
        let readout = find_path(
            &projection,
            NavPathQuery {
                start,
                goal,
                max_visited: 128,
            },
        )
        .expect("path");
        assert_eq!(readout.outcome, NavPathOutcome::Reached);
        assert_eq!(readout.path.first(), Some(&start));
        assert_eq!(readout.path.last(), Some(&goal));
    }

    #[test]
    fn blocked_tunnel_reports_no_path() {
        let mut projection = projection();
        let (start, goal) = tunnel_nav_endpoints();
        for x in 1..=5 {
            projection = projection.without_walkable(VoxelCoord::new(x, 1, 4));
        }
        let readout = find_path(
            &projection,
            NavPathQuery {
                start,
                goal,
                max_visited: 128,
            },
        )
        .expect("no path readout");
        assert_eq!(readout.outcome, NavPathOutcome::NoPath);
        assert_eq!(readout.visited, 25);
        assert!(readout.path.is_empty());
    }

    #[test]
    fn invalid_query_rejects_unwalkable_start() {
        let projection = projection();
        let (_, goal) = tunnel_nav_endpoints();
        assert_eq!(
            find_path(
                &projection,
                NavPathQuery {
                    start: VoxelCoord::new(0, 1, 0),
                    goal,
                    max_visited: 128,
                },
            ),
            Err(NavError::StartNotWalkable {
                start: VoxelCoord::new(0, 1, 0)
            })
        );
    }

    #[test]
    fn path_readout_matches_committed_golden() {
        let projection = projection();
        let (start, goal) = tunnel_nav_endpoints();
        let readout = find_path(
            &projection,
            NavPathQuery {
                start,
                goal,
                max_visited: 128,
            },
        )
        .expect("path");
        assert_eq!(
            describe_nav_path(&projection, &readout),
            include_str!("../../../../fixtures/nav/generated-tunnel-path.snapshot.txt")
        );
    }

    #[test]
    fn volumetric_path_reaches_vertical_connected_space() {
        let mut world = solid_test_world();
        let vertical_path = [
            VoxelCoord::new(1, 1, 1),
            VoxelCoord::new(1, 2, 1),
            VoxelCoord::new(1, 3, 1),
            VoxelCoord::new(1, 4, 1),
        ];
        for coord in vertical_path {
            carve_empty(&mut world, coord);
        }

        let readout = find_volumetric_path(
            &world,
            volumetric_query(vertical_path[0], vertical_path[3], 16),
        )
        .expect("vertical volumetric path");

        assert_eq!(readout.mode, NavQueryMode::Volumetric3d);
        assert_eq!(readout.outcome, VolumetricNavOutcome::Reached);
        assert_eq!(readout.path, vertical_path);
        assert_eq!(readout.path_len, 4);
        assert_eq!(readout.visited, 4);
        assert_ne!(readout.path_hash, 0);
    }

    #[test]
    fn volumetric_path_reports_unreachable_separated_volumes() {
        let mut world = solid_test_world();
        let start = VoxelCoord::new(1, 1, 1);
        let goal = VoxelCoord::new(3, 1, 1);
        carve_empty(&mut world, start);
        carve_empty(&mut world, goal);

        let readout =
            find_volumetric_path(&world, volumetric_query(start, goal, 16)).expect("no path");

        assert_eq!(readout.outcome, VolumetricNavOutcome::NoPath);
        assert_eq!(readout.visited, 1);
        assert_eq!(readout.path_len, 0);
        assert!(readout.path.is_empty());
    }

    #[test]
    fn volumetric_path_reports_budget_exhaustion() {
        let mut world = solid_test_world();
        let line = [
            VoxelCoord::new(1, 1, 1),
            VoxelCoord::new(2, 1, 1),
            VoxelCoord::new(3, 1, 1),
            VoxelCoord::new(4, 1, 1),
            VoxelCoord::new(5, 1, 1),
        ];
        for coord in line {
            carve_empty(&mut world, coord);
        }

        let readout =
            find_volumetric_path(&world, volumetric_query(line[0], line[4], 3)).expect("budget");

        assert_eq!(readout.outcome, VolumetricNavOutcome::BudgetExhausted);
        assert_eq!(readout.visited, 3);
        assert_eq!(readout.path_len, 0);
        assert!(readout.path.is_empty());
    }

    #[test]
    fn volumetric_path_rejects_invalid_or_non_traversable_endpoints() {
        let mut world = solid_test_world();
        let start = VoxelCoord::new(1, 1, 1);
        let goal = VoxelCoord::new(2, 1, 1);
        carve_empty(&mut world, goal);

        assert_eq!(
            find_volumetric_path(&world, volumetric_query(start, goal, 16)),
            Err(VolumetricNavError::StartNotTraversable { start })
        );

        carve_empty(&mut world, start);
        set_test_voxel(&mut world, goal, VoxelValue::solid_raw(1));
        assert_eq!(
            find_volumetric_path(&world, volumetric_query(start, goal, 16)),
            Err(VolumetricNavError::GoalNotTraversable { goal })
        );

        assert_eq!(
            find_volumetric_path(
                &world,
                VolumetricNavQuery {
                    max_visited: 0,
                    ..volumetric_query(start, start, 16)
                },
            ),
            Err(VolumetricNavError::InvalidQueryBudget)
        );
        assert_eq!(
            find_volumetric_path(
                &world,
                VolumetricNavQuery {
                    config: VolumetricNavConfig {
                        agent_volume: VolumetricAgentVolume {
                            size_x: 1,
                            size_y: 0,
                            size_z: 1,
                        },
                        ..VolumetricNavConfig::default()
                    },
                    ..volumetric_query(start, start, 16)
                },
            ),
            Err(VolumetricNavError::InvalidAgentVolume)
        );
    }

    #[test]
    fn volumetric_agent_volume_requires_empty_occupied_cells() {
        let mut world = solid_test_world();
        let start = VoxelCoord::new(1, 1, 1);
        let goal = VoxelCoord::new(2, 1, 1);
        carve_empty(&mut world, start);
        carve_empty(&mut world, goal);
        carve_empty(&mut world, VoxelCoord::new(2, 2, 1));

        assert_eq!(
            find_volumetric_path(
                &world,
                VolumetricNavQuery {
                    config: VolumetricNavConfig {
                        agent_volume: VolumetricAgentVolume {
                            size_x: 1,
                            size_y: 2,
                            size_z: 1,
                        },
                        ..VolumetricNavConfig::default()
                    },
                    ..volumetric_query(start, goal, 16)
                },
            ),
            Err(VolumetricNavError::StartNotTraversable { start })
        );
    }

    #[test]
    fn volumetric_vertical_policy_can_disable_vertical_neighbors() {
        let mut world = solid_test_world();
        let start = VoxelCoord::new(1, 1, 1);
        let goal = VoxelCoord::new(1, 2, 1);
        carve_empty(&mut world, start);
        carve_empty(&mut world, goal);

        let readout = find_volumetric_path(
            &world,
            VolumetricNavQuery {
                config: VolumetricNavConfig {
                    vertical_policy: VolumetricVerticalPolicy::DisallowVertical,
                    ..VolumetricNavConfig::default()
                },
                ..volumetric_query(start, goal, 16)
            },
        )
        .expect("vertical disallowed");

        assert_eq!(readout.outcome, VolumetricNavOutcome::NoPath);
        assert_eq!(readout.visited, 1);
    }

    #[test]
    fn volumetric_path_output_is_deterministic_and_planar_defaults_hold() {
        let projection = projection();
        let (start, goal) = tunnel_nav_endpoints();
        let planar = find_path(
            &projection,
            NavPathQuery {
                start,
                goal,
                max_visited: 128,
            },
        )
        .expect("planar path");
        assert_eq!(projection.projection_hash(), 0x3143_6e7d_f5df_baee);
        assert_eq!(planar.path_hash, 0x09ed_0284_f7c1_75e1);

        let mut world = solid_test_world();
        let path = [
            VoxelCoord::new(1, 1, 1),
            VoxelCoord::new(2, 1, 1),
            VoxelCoord::new(3, 1, 1),
        ];
        for coord in path {
            carve_empty(&mut world, coord);
        }
        let query = volumetric_query(path[0], path[2], 16);
        let first = find_volumetric_path(&world, query).expect("first volumetric path");
        let second = find_volumetric_path(&world, query).expect("second volumetric path");

        assert_eq!(first, second);
        assert_eq!(first.outcome, VolumetricNavOutcome::Reached);
        assert_eq!(first.path, path);
        assert_ne!(first.path_hash, planar.path_hash);
    }

    #[test]
    fn weighted_volumetric_overlay_is_independent_of_surface_projection() {
        let mut world = solid_test_world();
        let path = [
            VoxelCoord::new(1, 1, 1),
            VoxelCoord::new(1, 2, 1),
            VoxelCoord::new(1, 3, 1),
        ];
        for coord in path {
            carve_empty(&mut world, coord);
        }

        let surface = build_nav_projection(&world, NavProjectionConfig::default())
            .expect("surface projection");
        assert!(surface.is_walkable(path[0]));
        assert!(!surface.is_walkable(path[1]));
        assert!(!surface.is_walkable(path[2]));

        let records = [
            VolumetricNavTraversalCell {
                coord: path[1],
                allowed: true,
                cost: 5,
            },
            VolumetricNavTraversalCell {
                coord: path[2],
                allowed: true,
                cost: 2,
            },
        ];
        let forward = VolumetricNavTraversalOverlay::from_cells(records).expect("forward overlay");
        let reverse =
            VolumetricNavTraversalOverlay::from_cells(records.into_iter().rev()).expect("reverse");
        assert_eq!(forward, reverse);
        assert_eq!(forward.cost_for(path[0]), 1);
        assert!(forward.is_allowed(path[0]));

        let readout =
            find_weighted_volumetric_path(&world, &forward, volumetric_query(path[0], path[2], 16))
                .expect("weighted volumetric path");
        assert_eq!(readout.outcome, WeightedNavPathOutcome::Reached);
        assert_eq!(readout.path, path);
        assert_eq!(readout.total_cost, 7);
        assert_eq!(readout.overlay_hash, forward.overlay_hash());
        assert_eq!(
            readout.source_hash,
            volumetric_navigation_source_hash(&world)
        );
    }

    #[test]
    fn weighted_volumetric_overlay_blocks_and_rejects_invalid_records() {
        let mut world = solid_test_world();
        let start = VoxelCoord::new(1, 1, 1);
        let goal = VoxelCoord::new(1, 2, 1);
        carve_empty(&mut world, start);
        carve_empty(&mut world, goal);

        let blocked = VolumetricNavTraversalOverlay::from_cells([VolumetricNavTraversalCell {
            coord: goal,
            allowed: false,
            cost: 1,
        }])
        .expect("blocked overlay");
        assert_eq!(
            find_weighted_volumetric_path(&world, &blocked, volumetric_query(start, goal, 16)),
            Err(WeightedVolumetricNavPathError::GoalBlocked { goal })
        );

        assert_eq!(
            VolumetricNavTraversalOverlay::from_cells([
                VolumetricNavTraversalCell {
                    coord: start,
                    allowed: true,
                    cost: 1,
                },
                VolumetricNavTraversalCell {
                    coord: start,
                    allowed: true,
                    cost: 2,
                },
            ]),
            Err(VolumetricNavTraversalOverlayError::DuplicateCell { coord: start })
        );
        assert_eq!(
            VolumetricNavTraversalOverlay::from_cells([VolumetricNavTraversalCell {
                coord: start,
                allowed: true,
                cost: 0,
            }]),
            Err(VolumetricNavTraversalOverlayError::ZeroCost { coord: start })
        );
    }

    #[test]
    fn direct_nav_movement_proposes_bounded_next_waypoint() {
        let readout = propose_direct_nav_movement(DirectNavMovementRequest {
            from: Vec3::new(0.0, 0.5, -2.6),
            target: Vec3::new(0.0, 1.62, 1.25),
            max_step_units: 0.35,
        })
        .expect("direct nav movement");

        assert_eq!(readout.from, Vec3::new(0.0, 0.5, -2.6));
        assert_eq!(readout.target, Vec3::new(0.0, 1.62, 1.25));
        assert_eq!(readout.next_waypoint, Vec3::new(0.0, 0.598, -2.264));
        assert_eq!(readout.distance_units, 4.01);
        assert!(!readout.reached);
        assert_eq!(readout.path_hash, 0x69ed_74d6_9292_2db7);
    }

    #[test]
    fn projected_direct_nav_movement_uses_nav_projection_path() {
        let projection = projection();
        let (start, goal) = tunnel_nav_endpoints();
        let path = find_path(
            &projection,
            NavPathQuery {
                start,
                goal,
                max_visited: 128,
            },
        )
        .expect("path");
        let readout = propose_projected_direct_nav_movement(
            &projection,
            ProjectedDirectNavMovementRequest {
                from: cell_center(&projection, start),
                target: cell_center(&projection, goal),
                max_step_units: 1.0,
                max_visited: 128,
            },
        )
        .expect("projected direct nav");
        let straight_line = propose_direct_nav_movement(DirectNavMovementRequest {
            from: cell_center(&projection, start),
            target: cell_center(&projection, goal),
            max_step_units: 1.0,
        })
        .expect("straight line direct nav");

        assert_eq!(readout.start, start);
        assert_eq!(readout.goal, goal);
        assert_eq!(readout.next_path_cell, path.path[1]);
        assert_eq!(
            readout.next_waypoint,
            cell_center(&projection, path.path[1])
        );
        assert_ne!(readout.next_waypoint, straight_line.next_waypoint);
        assert_eq!(readout.projection_hash, projection.projection_hash());
        assert_eq!(readout.path_hash, path.path_hash);
        assert_eq!(readout.path_len, path.path.len());
        assert!(!readout.reached);
    }

    #[test]
    fn projected_direct_nav_movement_reports_reached_inside_same_cell() {
        let projection = projection();
        let (cell, _) = tunnel_nav_endpoints();
        let from = cell_center(&projection, cell);
        let target = from + Vec3::new(0.125, 0.0, 0.0);
        let readout = propose_projected_direct_nav_movement(
            &projection,
            ProjectedDirectNavMovementRequest {
                from,
                target,
                max_step_units: 0.25,
                max_visited: 128,
            },
        )
        .expect("same-cell projected direct nav");

        assert_eq!(readout.start, cell);
        assert_eq!(readout.goal, cell);
        assert_eq!(readout.next_path_cell, cell);
        assert_eq!(readout.next_waypoint, target);
        assert_eq!(readout.path_len, 1);
        assert!(readout.reached);
    }

    #[test]
    fn projected_direct_nav_movement_rejects_no_path() {
        let mut projection = projection();
        let (start, goal) = tunnel_nav_endpoints();
        for x in 1..=5 {
            projection = projection.without_walkable(VoxelCoord::new(x, 1, 4));
        }
        assert_eq!(
            propose_projected_direct_nav_movement(
                &projection,
                ProjectedDirectNavMovementRequest {
                    from: cell_center(&projection, start),
                    target: cell_center(&projection, goal),
                    max_step_units: 1.0,
                    max_visited: 128,
                },
            ),
            Err(ProjectedDirectNavMovementError::NoPath { start, goal })
        );
    }

    #[test]
    fn projected_direct_nav_movement_rejects_invalid_inputs() {
        let projection = projection();
        assert_eq!(
            propose_projected_direct_nav_movement(
                &projection,
                ProjectedDirectNavMovementRequest {
                    from: Vec3::new(f32::NAN, 0.0, 0.0),
                    target: Vec3::ZERO,
                    max_step_units: 1.0,
                    max_visited: 128,
                },
            ),
            Err(ProjectedDirectNavMovementError::NonFinitePosition)
        );
        assert_eq!(
            propose_projected_direct_nav_movement(
                &projection,
                ProjectedDirectNavMovementRequest {
                    from: Vec3::ZERO,
                    target: Vec3::ONE,
                    max_step_units: 0.0,
                    max_visited: 128,
                },
            ),
            Err(ProjectedDirectNavMovementError::InvalidStep)
        );
        assert_eq!(
            propose_projected_direct_nav_movement(
                &projection,
                ProjectedDirectNavMovementRequest {
                    from: Vec3::ZERO,
                    target: Vec3::ONE,
                    max_step_units: 1.0,
                    max_visited: 0,
                },
            ),
            Err(ProjectedDirectNavMovementError::InvalidQueryBudget)
        );
    }

    #[test]
    fn projected_direct_nav_movement_rejects_unwalkable_endpoints() {
        let projection = projection();
        let (start, goal) = tunnel_nav_endpoints();
        assert_eq!(
            propose_projected_direct_nav_movement(
                &projection,
                ProjectedDirectNavMovementRequest {
                    from: cell_center(&projection, VoxelCoord::new(0, 1, 0)),
                    target: cell_center(&projection, goal),
                    max_step_units: 1.0,
                    max_visited: 128,
                },
            ),
            Err(ProjectedDirectNavMovementError::StartNotWalkable {
                start: VoxelCoord::new(0, 1, 0)
            })
        );
        assert_eq!(
            propose_projected_direct_nav_movement(
                &projection,
                ProjectedDirectNavMovementRequest {
                    from: cell_center(&projection, start),
                    target: cell_center(&projection, VoxelCoord::new(0, 1, 0)),
                    max_step_units: 1.0,
                    max_visited: 128,
                },
            ),
            Err(ProjectedDirectNavMovementError::GoalNotWalkable {
                goal: VoxelCoord::new(0, 1, 0)
            })
        );
    }

    #[test]
    fn projected_direct_nav_movement_is_deterministic() {
        let projection = projection();
        let (start, goal) = tunnel_nav_endpoints();
        let request = ProjectedDirectNavMovementRequest {
            from: cell_center(&projection, start),
            target: cell_center(&projection, goal),
            max_step_units: 0.75,
            max_visited: 128,
        };
        let first =
            propose_projected_direct_nav_movement(&projection, request).expect("first readout");
        let second =
            propose_projected_direct_nav_movement(&projection, request).expect("second readout");

        assert_eq!(first, second);
        assert_ne!(first.movement_hash, 0);
        assert_eq!(first.projection_hash, 0x3143_6e7d_f5df_baee);
        assert_eq!(first.path_hash, 0x09ed_0284_f7c1_75e1);
    }

    /// Path queries over a rolling 64×64 box of columns with diagonal
    /// neighbours and edges admitted within two cells of height, the shape a
    /// collision-derived publication has, with a 4,096-cell budget. Routes run
    /// from the centre; one goes round a ridge, one ends on a pillar no edge
    /// reaches.
    #[test]
    #[ignore = "a measurement, not a check"]
    fn measure_planar_query_cost() {
        const SIDE: i64 = 64;
        const BUDGET: usize = 4_096;
        const REPEATS: u32 = 50;
        const LIFT: i64 = 10;
        let height = |x: i64, z: i64| {
            ((x as f64 * 0.3).sin() * 3.0 + (z as f64 * 0.25).cos() * 3.0).round() as i64
        };
        let pillar = (40, 52);
        let ridge = |x: i64, z: i64| x == 24 && (16..=48).contains(&z);
        let cell = |x: i64, z: i64| {
            let lift = if (x, z) == pillar || ridge(x, z) {
                LIFT
            } else {
                0
            };
            VoxelCoord::new(x, height(x, z) + lift, z)
        };
        let cells: Vec<_> = (0..SIDE)
            .flat_map(|x| (0..SIDE).map(move |z| (x, z)))
            .map(|(x, z)| cell(x, z))
            .collect();
        let projection = NavProjection::from_walkable_cells(test_grid(), cells.iter().copied());
        let policy = PlanarNavNeighborPolicy {
            max_step_cells: 2,
            diagonal: true,
        };
        let edges = NavEdgeAdmission::from_allowed_edges(cells.iter().flat_map(|&from| {
            planar_nav_offsets(true)
                .iter()
                .map(move |&(dx, dz)| (from.x + dx, from.z + dz))
                .filter(|&(x, z)| (0..SIDE).contains(&x) && (0..SIDE).contains(&z))
                .map(move |(x, z)| (from, cell(x, z)))
                .filter(|(from, to)| (to.y - from.y).abs() <= 2)
        }));
        let overlay = NavTraversalOverlay::empty(&projection);
        let started = std::time::Instant::now();
        for _ in 0..REPEATS {
            std::hint::black_box(NavComponents::label(&projection, &edges, policy));
        }
        let per_label = started.elapsed() / REPEATS;
        let components = NavComponents::label(&projection, &edges, policy);
        println!(
            "labelling {} cells into {} components: {per_label:?}",
            projection.walkable_len(),
            components.len()
        );
        let start = cell(32, 32);
        for (label, goal) in [
            ("5 cells", cell(37, 32)),
            ("15 cells", cell(47, 32)),
            ("30 cells", cell(62, 32)),
            ("round a ridge", cell(16, 32)),
            ("unreachable", cell(pillar.0, pillar.1)),
        ] {
            let query = NavPathQuery {
                start,
                goal,
                max_visited: BUDGET,
            };
            for (labelled, components) in [("unlabelled", None), ("labelled", Some(&components))] {
                let run = || {
                    find_path_with_traversal_and_edge_admission(
                        &projection,
                        &overlay,
                        &edges,
                        components,
                        query,
                        policy,
                    )
                    .expect("query")
                };
                let readout = run();
                let started = std::time::Instant::now();
                for _ in 0..REPEATS {
                    std::hint::black_box(run());
                }
                let per_query = started.elapsed() / REPEATS;
                let weighted = || {
                    find_weighted_path_with_edge_admission(
                        &projection,
                        &overlay,
                        &edges,
                        components,
                        query,
                        policy,
                    )
                    .expect("weighted query")
                };
                let weighted_readout = weighted();
                let started = std::time::Instant::now();
                for _ in 0..REPEATS {
                    std::hint::black_box(weighted());
                }
                let per_weighted = started.elapsed() / REPEATS;
                println!(
                    "{label} {labelled}: {:?} path {} visited {} nearest {:?} in {per_query:?}; weighted {:?} cost {} visited {} in {per_weighted:?}",
                    readout.outcome,
                    readout.path.len(),
                    readout.visited,
                    readout.nearest,
                    weighted_readout.outcome,
                    weighted_readout.total_cost,
                    weighted_readout.visited,
                );
            }
        }
    }

    #[test]
    fn direct_nav_movement_rejects_invalid_inputs() {
        assert_eq!(
            propose_direct_nav_movement(DirectNavMovementRequest {
                from: Vec3::new(f32::NAN, 0.0, 0.0),
                target: Vec3::ZERO,
                max_step_units: 0.35,
            }),
            Err(DirectNavMovementError::NonFinitePosition)
        );
        assert_eq!(
            propose_direct_nav_movement(DirectNavMovementRequest {
                from: Vec3::ZERO,
                target: Vec3::ONE,
                max_step_units: 0.0,
            }),
            Err(DirectNavMovementError::InvalidStep)
        );
    }
}
