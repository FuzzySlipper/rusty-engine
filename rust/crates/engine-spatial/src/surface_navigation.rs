//! Surface navigation: where a crawler's ball can rest on floors, walls and
//! ceilings, as cells of a grid each holding up to six surfaces, one per
//! facing, and which of those it can crawl between.
//!
//! - **Surface cells.** A surface facing `+Y` (a floor) belongs to the cell
//!   half a cell above it, and likewise for the other five facings. One ray
//!   down each line of cells along an axis finds every surface on that line
//!   facing back along it; through collidable voxels it skips to the run's
//!   far side. A surface tilted more than 45° from the facing belongs to
//!   another facing's lines. It is a surface cell when the ball fits resting
//!   its skin off it with its centre outside collision: a reconstructed
//!   surface can lie above its voxels' run, and a line going on below it
//!   meets the deep voxels' cubes inside the rock.
//! - **Edges.** A surface cell joins the cells of the same facing beside it
//!   (and one cell up or down, for slopes), the other facings in its own
//!   cell (a concave corner, as floor into wall) and, past a lip, the facing
//!   beyond it one cell down (a convex corner). The ball is swept along the
//!   way, round a convex corner on an arc about the edge; a hit facing more
//!   against the move than along the surfaces is a block, and anything else
//!   is the surface itself and grazed past.
//!
//! Nothing is retained here: a derivation reads the scene and returns cells
//! and edges.

use std::collections::HashMap;

use core_space::{VoxelCoord, WorldPos, WorldVec};
use svc_collision::{CharacterCapsule, CharacterCollisionSource};

use crate::crawler::V;
use crate::{CrawlerConfig, CrawlerSurface, SpatialCollisionHit, VoxelCollisionScene};

/// A surface tilted further than this from a facing belongs to another.
const FACING_COS: f64 = std::f64::consts::FRAC_1_SQRT_2;
/// How far past a surface a ray continues.
const RAY_EPSILON: f64 = 1.0e-6;
/// The fit test lifts a ball with less skin than this to it, so one resting
/// touching its surface still fits.
const FIT_CLEARANCE: f64 = 1.0e-3;
/// The least a grazing sweep steps off, as a crawler's move does.
const MINIMUM_GRAZE_STEP: f64 = 1.0e-4;
/// How many grazes one sweep of an edge steps past.
const GRAZE_RETRIES: usize = 4;

/// Which way a surface faces: the open side, where the ball rests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SurfaceFace {
    PositiveX,
    NegativeX,
    /// A floor.
    PositiveY,
    /// A ceiling.
    NegativeY,
    PositiveZ,
    NegativeZ,
}

impl SurfaceFace {
    pub const ALL: [Self; 6] = [
        Self::PositiveX,
        Self::NegativeX,
        Self::PositiveY,
        Self::NegativeY,
        Self::PositiveZ,
        Self::NegativeZ,
    ];

    /// 0 for X, 1 for Y, 2 for Z.
    pub const fn axis(self) -> usize {
        match self {
            Self::PositiveX | Self::NegativeX => 0,
            Self::PositiveY | Self::NegativeY => 1,
            Self::PositiveZ | Self::NegativeZ => 2,
        }
    }

    pub const fn positive(self) -> bool {
        matches!(self, Self::PositiveX | Self::PositiveY | Self::PositiveZ)
    }

    pub const fn of(axis: usize, positive: bool) -> Self {
        match (axis, positive) {
            (0, true) => Self::PositiveX,
            (0, false) => Self::NegativeX,
            (1, true) => Self::PositiveY,
            (1, false) => Self::NegativeY,
            (_, true) => Self::PositiveZ,
            (_, false) => Self::NegativeZ,
        }
    }

    /// The unit cell offset it faces along.
    pub const fn step(self) -> [i64; 3] {
        let mut step = [0; 3];
        step[self.axis()] = if self.positive() { 1 } else { -1 };
        step
    }

    pub fn normal(self) -> WorldVec {
        let [x, y, z] = self.step().map(|value| value as f64);
        WorldVec::new(x, y, z)
    }

    /// The four facings across it, along the other two axes.
    pub fn across(self) -> [Self; 4] {
        let [first, second] = other_axes(self.axis());
        [
            Self::of(first, true),
            Self::of(first, false),
            Self::of(second, true),
            Self::of(second, false),
        ]
    }

    fn v(self) -> V {
        V(self.step().map(|value| value as f64))
    }
}

fn other_axes(axis: usize) -> [usize; 2] {
    match axis {
        0 => [1, 2],
        1 => [0, 2],
        _ => [0, 1],
    }
}

fn offset(cell: VoxelCoord, step: [i64; 3]) -> VoxelCoord {
    VoxelCoord::new(cell.x + step[0], cell.y + step[1], cell.z + step[2])
}

/// One surface place: a cell and a facing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SurfaceCell {
    pub cell: VoxelCoord,
    pub face: SurfaceFace,
}

/// The inclusive box of cells a derivation covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceRegion {
    pub min: VoxelCoord,
    pub max: VoxelCoord,
}

impl SurfaceRegion {
    pub fn contains(self, cell: VoxelCoord) -> bool {
        (self.min.x..=self.max.x).contains(&cell.x)
            && (self.min.y..=self.max.y).contains(&cell.y)
            && (self.min.z..=self.max.z).contains(&cell.z)
    }

    fn low(self) -> [i64; 3] {
        [self.min.x, self.min.y, self.min.z]
    }

    fn high(self) -> [i64; 3] {
        [self.max.x, self.max.y, self.max.z]
    }
}

/// The crawler a derivation is for, and its cells' size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceNavigationConfig {
    /// At least the ball's diameter and skin on both sides, so a ball
    /// resting in a cell fits in it.
    pub cell_size: f64,
    /// Its radius, contact skin and floor slope; the rest is movement.
    pub crawler: CrawlerConfig,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SurfaceNavigationError {
    InvalidConfig { field: &'static str },
    Collision(svc_collision::CharacterCollisionQueryError),
}

impl SurfaceNavigationError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidConfig { .. } => "invalid-surface-navigation-config",
            Self::Collision(_) => "surface-navigation-collision-query-failed",
        }
    }
}

impl std::fmt::Display for SurfaceNavigationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {self:?}", self.code())
    }
}

impl std::error::Error for SurfaceNavigationError {}

impl From<svc_collision::CharacterCollisionQueryError> for SurfaceNavigationError {
    fn from(value: svc_collision::CharacterCollisionQueryError) -> Self {
        Self::Collision(value)
    }
}

impl SurfaceNavigationConfig {
    pub fn validate(&self) -> Result<(), SurfaceNavigationError> {
        self.crawler.validate().map_err(|error| match error {
            crate::CrawlerError::InvalidConfig { field } => {
                SurfaceNavigationError::InvalidConfig { field }
            }
            _ => SurfaceNavigationError::InvalidConfig { field: "crawler" },
        })?;
        let body = 2.0 * f64::from(self.crawler.radius + self.crawler.contact_skin);
        if !(self.cell_size.is_finite() && self.cell_size >= body) {
            return Err(SurfaceNavigationError::InvalidConfig { field: "cellSize" });
        }
        Ok(())
    }

    fn radius(&self) -> f64 {
        f64::from(self.crawler.radius)
    }

    fn skin(&self) -> f64 {
        f64::from(self.crawler.contact_skin)
    }

    fn rest_distance(&self) -> f64 {
        self.radius() + self.skin()
    }

    /// The cell holding a world point.
    pub fn cell_of(&self, at: WorldPos) -> VoxelCoord {
        let cell = |value: f64| (value / self.cell_size).floor() as i64;
        VoxelCoord::new(cell(at.x), cell(at.y), cell(at.z))
    }

    pub fn cell_center(&self, cell: VoxelCoord) -> WorldPos {
        let center = |value: i64| (value as f64 + 0.5) * self.cell_size;
        WorldPos::new(center(cell.x), center(cell.y), center(cell.z))
    }

    /// Which kind of surface a normal is, as a crawler's receipt names it.
    pub fn classify(&self, normal: WorldVec) -> CrawlerSurface {
        let floor_cos = f64::from(self.crawler.floor_maximum_slope_radians).cos();
        if normal.y >= floor_cos - 1.0e-6 {
            CrawlerSurface::Floor
        } else if normal.y <= -floor_cos + 1.0e-6 {
            CrawlerSurface::Ceiling
        } else {
            CrawlerSurface::Wall
        }
    }

    fn ball(&self, at: V) -> CharacterCapsule {
        CharacterCapsule {
            center: at.pos(),
            half_height: 0.0,
            radius: self.radius(),
        }
    }
}

/// Where a crawler's ball can rest.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceNode {
    pub cell: SurfaceCell,
    /// The ball's centre resting its skin off the surface.
    pub rest: WorldPos,
    /// Where the cell's line meets the surface.
    pub point: WorldPos,
    pub normal: WorldVec,
    pub surface: CrawlerSurface,
    pub source: CharacterCollisionSource,
}

/// One line of cells along a facing's axis: the cells with these
/// coordinates on the other two axes, lowest axis first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceLine {
    pub face: SurfaceFace,
    pub across: [i64; 2],
}

impl SurfaceLine {
    pub fn through(cell: SurfaceCell) -> Self {
        let at = [cell.cell.x, cell.cell.y, cell.cell.z];
        let [first, second] = other_axes(cell.face.axis());
        Self {
            face: cell.face,
            across: [at[first], at[second]],
        }
    }
}

/// Every line of every facing across `region`.
pub fn surface_lines(region: SurfaceRegion) -> Vec<SurfaceLine> {
    let (low, high) = (region.low(), region.high());
    let mut lines = Vec::new();
    for face in SurfaceFace::ALL {
        let [first, second] = other_axes(face.axis());
        for a in low[first]..=high[first] {
            for b in low[second]..=high[second] {
                lines.push(SurfaceLine {
                    face,
                    across: [a, b],
                });
            }
        }
    }
    lines
}

/// What became of one surface a line met.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceSampleOutcome {
    Surface,
    /// Tilted more than 45° from the facing: another facing's lines hold it.
    OtherFacing,
    /// The ball resting on it overlaps collision.
    BallOverlap,
    /// The ball's centre lies inside collision: the line met it from inside
    /// rock, below a reconstructed surface.
    InsideCollision,
    /// A surface nearer the open side already holds the cell.
    SameCell,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceSample {
    pub outcome: SurfaceSampleOutcome,
    pub cell: SurfaceCell,
    pub point: WorldPos,
    pub normal: WorldVec,
    pub rest: WorldPos,
    pub source: CharacterCollisionSource,
    /// For `BallOverlap`, when recorded: what the ball overlaps and where.
    pub overlap: Option<(CharacterCollisionSource, WorldPos)>,
}

fn hit_facts(
    scene: &VoxelCollisionScene,
    hit: SpatialCollisionHit,
) -> (V, V, f64, CharacterCollisionSource) {
    match hit {
        SpatialCollisionHit::Voxel(hit) => {
            let [x, y, z] = hit.voxel;
            let chunk = scene
                .voxel_world()
                .grid()
                .voxel_to_chunk(VoxelCoord::new(x, y, z));
            (
                V(hit.point),
                V(hit.normal),
                hit.distance,
                CharacterCollisionSource::VoxelChunk(chunk),
            )
        }
        SpatialCollisionHit::StaticMesh(hit) => (
            V::at(hit.point),
            V::world(hit.normal),
            hit.distance,
            CharacterCollisionSource::StaticMesh {
                instance: hit.instance,
                asset: hit.asset,
                geometry_hash: hit.geometry_hash,
            },
        ),
    }
}

/// The surface cells of one line, from its open end; with `record`, every
/// surface the line met and what became of it.
pub fn sample_surface_line(
    scene: &VoxelCollisionScene,
    config: &SurfaceNavigationConfig,
    region: SurfaceRegion,
    line: SurfaceLine,
    mut record: Option<&mut Vec<SurfaceSample>>,
) -> Result<Vec<SurfaceNode>, SurfaceNavigationError> {
    let face = line.face;
    let axis = face.axis();
    let facing = face.v();
    let toward = facing.mul(-1.0);
    let (low, high) = (region.low()[axis], region.high()[axis]);
    // The ray starts at the centre of the cell at the open end and runs one
    // cell past the far end, which is as far as a surface of the region's
    // cells can lie.
    let (first, length) = if face.positive() {
        (high, (high - low + 1) as f64 * config.cell_size)
    } else {
        (low, (high - low + 1) as f64 * config.cell_size)
    };
    let [one, two] = other_axes(axis);
    let mut start = [0_i64; 3];
    start[axis] = first;
    start[one] = line.across[0];
    start[two] = line.across[1];
    let start = config.cell_center(VoxelCoord::new(start[0], start[1], start[2]));
    let mut origin = V::at(start);
    let mut travelled = 0.0;
    let half = config.cell_size * 0.5;
    let floor_rest = config.rest_distance();
    let mut nodes: Vec<SurfaceNode> = Vec::new();
    // Each surface costs at most two rays; this bounds a line of slivers.
    let bound = 2 * (high - low + 1) as usize + 8;
    for _ in 0..bound {
        let remaining = length - travelled;
        if remaining <= RAY_EPSILON {
            break;
        }
        let Some(hit) = scene.raycast_world(origin.0, toward.0, remaining) else {
            break;
        };
        let (point, normal, distance, source) = hit_facts(scene, hit);
        travelled += distance;
        let cell = SurfaceCell {
            cell: config.cell_of(point.add(facing.mul(half)).pos()),
            face,
        };
        let rest = point.add(normal.mul(floor_rest));
        let mut sample = SurfaceSample {
            outcome: SurfaceSampleOutcome::Surface,
            cell,
            point: point.pos(),
            normal: normal.wvec(),
            rest: rest.pos(),
            source,
            overlap: None,
        };
        if region.contains(cell.cell) {
            if normal.dot(facing) < FACING_COS - 1.0e-9 {
                sample.outcome = SurfaceSampleOutcome::OtherFacing;
            } else if nodes.last().is_some_and(|node| node.cell == cell) {
                sample.outcome = SurfaceSampleOutcome::SameCell;
            } else {
                let lift = config.skin().max(FIT_CLEARANCE);
                let fit = config.ball(point.add(normal.mul(config.radius() + lift)));
                if scene.character_capsule_intersects(fit)? {
                    sample.outcome = SurfaceSampleOutcome::BallOverlap;
                    if record.is_some() {
                        sample.overlap = scene
                            .character_capsule_overlap(fit)?
                            .map(|overlap| (overlap.source, overlap.point));
                    }
                } else if scene.contains_point(rest.0) {
                    sample.outcome = SurfaceSampleOutcome::InsideCollision;
                } else {
                    nodes.push(SurfaceNode {
                        cell,
                        rest: rest.pos(),
                        point: point.pos(),
                        normal: normal.wvec(),
                        surface: config.classify(normal.wvec()),
                        source,
                    });
                }
            }
            if let Some(record) = record.as_deref_mut() {
                record.push(sample);
            }
        }
        // Through a run of collidable voxels the ray goes on from its far
        // side, so nothing is found from inside rock.
        let beyond = point.add(toward.mul(RAY_EPSILON));
        let next = match scene.collidable_voxel_run_end(
            beyond.0,
            axis,
            !face.positive(),
            length - travelled,
        ) {
            Some(end) => {
                let mut next = beyond;
                next.0[axis] = end + toward.0[axis] * RAY_EPSILON;
                next
            }
            None => beyond,
        };
        travelled += (next.0[axis] - point.0[axis]).abs();
        origin = next;
    }
    Ok(nodes)
}

/// How an edge joins two surface cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SurfaceEdgeKind {
    /// Along one facing, to the next cell (or one up or down a slope).
    Along,
    /// Into another facing in the same cell, as floor into wall.
    Concave,
    /// Over a lip onto the facing beyond it.
    Convex,
}

/// Every surface cell `cell` may have an edge to, and how.
pub fn surface_neighbors(cell: SurfaceCell) -> Vec<(SurfaceCell, SurfaceEdgeKind)> {
    let face = cell.face;
    let facing = face.step();
    let mut neighbors = Vec::with_capacity(20);
    for across in face.across() {
        let step = across.step();
        let beside = offset(cell.cell, step);
        for level in [0, 1, -1] {
            neighbors.push((
                SurfaceCell {
                    cell: offset(beside, facing.map(|value| value * level)),
                    face,
                },
                SurfaceEdgeKind::Along,
            ));
        }
        neighbors.push((
            SurfaceCell {
                cell: cell.cell,
                face: across,
            },
            SurfaceEdgeKind::Concave,
        ));
        // Past the lip ahead, the facing beyond is one cell down.
        neighbors.push((
            SurfaceCell {
                cell: offset(beside, facing.map(|value| -value)),
                face: across,
            },
            SurfaceEdgeKind::Convex,
        ));
    }
    neighbors
}

/// How `from` and `to` are neighbours, if they are.
pub fn surface_edge_kind(from: SurfaceCell, to: SurfaceCell) -> Option<SurfaceEdgeKind> {
    surface_neighbors(from)
        .into_iter()
        .find(|(cell, _)| *cell == to)
        .map(|(_, kind)| kind)
}

/// What stopped a sweep along an edge.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceBlock {
    /// The ball's centre where it stopped.
    pub at: WorldPos,
    pub point: WorldPos,
    pub normal: WorldVec,
    pub source: CharacterCollisionSource,
    /// The corner the edge turns, for a concave or convex edge.
    pub corner: Option<WorldPos>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SurfaceEdgeOutcome {
    Traversable,
    Blocked(SurfaceBlock),
}

/// Where the surfaces of two places meet, for a concave or convex edge
/// between them: each one's own coordinate along its axis, the line's
/// elsewhere.
pub fn surface_corner(from: &SurfaceNode, to: &SurfaceNode) -> WorldPos {
    corner(from, to).pos()
}

fn corner(from: &SurfaceNode, to: &SurfaceNode) -> V {
    let mut corner = V::at(from.point);
    let (a, b) = (from.cell.face.axis(), to.cell.face.axis());
    corner.0[b] = V::at(to.point).0[b];
    corner.0[a] = V::at(from.point).0[a];
    corner
}

/// Whether the ball crawls from `from` to `to` by an edge of `kind`.
pub fn surface_edge(
    scene: &VoxelCollisionScene,
    config: &SurfaceNavigationConfig,
    from: &SurfaceNode,
    to: &SurfaceNode,
    kind: SurfaceEdgeKind,
) -> Result<SurfaceEdgeOutcome, SurfaceNavigationError> {
    let (start, end) = (V::at(from.rest), V::at(to.rest));
    let surfaces = V::world(from.normal)
        .add(V::world(to.normal))
        .normalized()
        .unwrap_or(V::world(from.normal));
    let (path, corner) = match kind {
        SurfaceEdgeKind::Along => (vec![start, end], None),
        SurfaceEdgeKind::Concave => (vec![start, end], Some(corner(from, to))),
        SurfaceEdgeKind::Convex => {
            // About the edge on an arc of two chords, their corners far
            // enough out that the chords keep the resting distance.
            let edge = corner(from, to);
            let reach = config.rest_distance() / (std::f64::consts::PI / 8.0).cos();
            let (first, second) = (from.cell.face.v(), to.cell.face.v());
            let middle = first.add(second).normalized().unwrap_or(first);
            (
                vec![
                    start,
                    edge.add(first.mul(reach)),
                    edge.add(middle.mul(reach)),
                    edge.add(second.mul(reach)),
                    end,
                ],
                Some(edge),
            )
        }
    };
    for leg in path.windows(2) {
        if let Some(mut block) = sweep_clear(scene, config, leg[0], leg[1], surfaces)? {
            block.corner = corner.map(V::pos);
            return Ok(SurfaceEdgeOutcome::Blocked(block));
        }
    }
    Ok(SurfaceEdgeOutcome::Traversable)
}

/// Sweeps the ball from `from` to `to`; what blocks it, if anything. A hit
/// facing more along `surfaces` than against the move is the surface
/// crawled on, which the crawler follows: the ball steps off it and sweeps
/// again.
fn sweep_clear(
    scene: &VoxelCollisionScene,
    config: &SurfaceNavigationConfig,
    from: V,
    to: V,
    surfaces: V,
) -> Result<Option<SurfaceBlock>, SurfaceNavigationError> {
    let mut at = from;
    for _ in 0..=GRAZE_RETRIES {
        let translation = to.sub(at);
        let Some(direction) = translation.normalized() else {
            return Ok(None);
        };
        let Some(hit) = scene.cast_character_capsule(config.ball(at), translation.wvec(), 0.0)?
        else {
            return Ok(None);
        };
        let normal = V::world(hit.normal).normalized().unwrap_or(surfaces);
        let block = SurfaceBlock {
            at: at
                .add(translation.mul(hit.time_of_impact.clamp(0.0, 1.0)))
                .pos(),
            point: hit.point,
            normal: normal.wvec(),
            source: hit.source,
            corner: None,
        };
        if -normal.dot(direction) > normal.dot(surfaces) {
            return Ok(Some(block));
        }
        let step = (config.skin() * 0.5).max(MINIMUM_GRAZE_STEP);
        at = V::at(block.at).add(normal.mul(step));
    }
    // Only the surfaces crawled on met: the crawler follows them.
    Ok(None)
}

/// One directed edge between surface cells, by their indices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceEdge {
    pub from: u32,
    pub to: u32,
    pub kind: SurfaceEdgeKind,
}

/// The surface cells of a region and the edges between them.
#[derive(Debug, Clone, Default)]
pub struct SurfaceNavigation {
    pub nodes: Vec<SurfaceNode>,
    pub index: HashMap<SurfaceCell, u32>,
    /// Both directions of every traversable edge.
    pub edges: Vec<SurfaceEdge>,
    /// How many edges were swept.
    pub edge_tests: u64,
}

impl SurfaceNavigation {
    /// The navigation of `nodes`, as lines found them.
    pub fn of_nodes(nodes: Vec<SurfaceNode>) -> Self {
        let index = nodes
            .iter()
            .enumerate()
            .map(|(slot, node)| (node.cell, slot as u32))
            .collect();
        Self {
            nodes,
            index,
            edges: Vec::new(),
            edge_tests: 0,
        }
    }

    /// The traversable edges from node `slot` to the neighbours ordered
    /// after it, both ways, and how many were swept.
    pub fn edges_from(
        &self,
        scene: &VoxelCollisionScene,
        config: &SurfaceNavigationConfig,
        slot: u32,
    ) -> Result<(Vec<SurfaceEdge>, u64), SurfaceNavigationError> {
        let from = &self.nodes[slot as usize];
        let mut edges = Vec::new();
        let mut tests = 0;
        for (cell, kind) in surface_neighbors(from.cell) {
            if cell <= from.cell {
                continue;
            }
            let Some(&other) = self.index.get(&cell) else {
                continue;
            };
            tests += 1;
            let to = &self.nodes[other as usize];
            if surface_edge(scene, config, from, to, kind)? == SurfaceEdgeOutcome::Traversable {
                edges.push(SurfaceEdge {
                    from: slot,
                    to: other,
                    kind,
                });
                edges.push(SurfaceEdge {
                    from: other,
                    to: slot,
                    kind,
                });
            }
        }
        Ok((edges, tests))
    }
}

/// Every surface cell of `region` and the edges between them, on the
/// calling thread.
pub fn derive_surface_navigation(
    scene: &VoxelCollisionScene,
    config: &SurfaceNavigationConfig,
    region: SurfaceRegion,
) -> Result<SurfaceNavigation, SurfaceNavigationError> {
    config.validate()?;
    let mut nodes = Vec::new();
    for line in surface_lines(region) {
        nodes.extend(sample_surface_line(scene, config, region, line, None)?);
    }
    let mut navigation = SurfaceNavigation::of_nodes(nodes);
    for slot in 0..navigation.nodes.len() as u32 {
        let (edges, tests) = navigation.edges_from(scene, config, slot)?;
        navigation.edges.extend(edges);
        navigation.edge_tests += tests;
    }
    Ok(navigation)
}
