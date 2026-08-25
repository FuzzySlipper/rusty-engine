//! Deterministic navigation/pathfinding projection over host-derived cells or
//! voxel authority.
//!
//! # Lane
//!
//! `rust-service` — builds read-only navigation projections from authoritative
//! voxel worlds and answers deterministic path queries. It does not own AI,
//! policy mutation, movement application, render state, or demo behavior.

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet, VecDeque};

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
#[derive(Debug, Clone, PartialEq)]
pub struct NavProjection {
    grid: VoxelGridSpec,
    walkable: BTreeSet<VoxelCoord>,
    projection_hash: u64,
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
        let walkable = cells.into_iter().collect::<BTreeSet<_>>();
        let projection_hash = hash_walkable(&walkable);
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
        self.walkable.contains(&coord)
    }

    #[cfg(test)]
    fn without_walkable(mut self, coord: VoxelCoord) -> Self {
        self.walkable.remove(&coord);
        self.projection_hash = hash_walkable(&self.walkable);
        self
    }

    pub fn walkable_cells(&self) -> impl Iterator<Item = VoxelCoord> + '_ {
        self.walkable.iter().copied()
    }
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
}

/// Deterministic path readout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavPathReadout {
    pub outcome: NavPathOutcome,
    pub visited: usize,
    pub path: Vec<VoxelCoord>,
    pub path_hash: u64,
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

/// Build a read-only nav projection from authoritative voxel data.
pub fn build_nav_projection(
    world: &VoxelWorld,
    config: NavProjectionConfig,
) -> Result<NavProjection, NavError> {
    if config.agent_height_voxels == 0 {
        return Err(NavError::InvalidAgentHeight);
    }
    let grid = world.grid();
    let mut walkable = BTreeSet::new();
    for (chunk_coord, chunk) in world.resident_chunks() {
        for (local, value) in chunk.iter() {
            if value.is_solid() {
                continue;
            }
            let coord = grid.chunk_local_to_voxel(chunk_coord, local);
            if is_walkable_cell(world, coord, config) {
                walkable.insert(coord);
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
/// by upward then downward cells at increasing distance.
pub fn find_path_with_policy(
    projection: &NavProjection,
    query: NavPathQuery,
    policy: PlanarNavNeighborPolicy,
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
    if query.start == query.goal {
        let path = vec![query.start];
        return Ok(NavPathReadout {
            outcome: NavPathOutcome::Reached,
            visited: 1,
            path_hash: hash_path(&path),
            path,
        });
    }

    let mut queue = VecDeque::new();
    let mut visited = BTreeSet::new();
    let mut came_from = BTreeMap::new();
    queue.push_back(query.start);
    visited.insert(query.start);

    while let Some(current) = queue.pop_front() {
        if visited.len() > query.max_visited {
            break;
        }
        for next in planar_nav_neighbors(current, policy) {
            if !projection.is_walkable(next) || visited.contains(&next) {
                continue;
            }
            came_from.insert(next, current);
            if next == query.goal {
                let path = reconstruct_path(query.start, query.goal, &came_from);
                return Ok(NavPathReadout {
                    outcome: NavPathOutcome::Reached,
                    visited: visited.len() + 1,
                    path_hash: hash_path(&path),
                    path,
                });
            }
            visited.insert(next);
            queue.push_back(next);
        }
    }

    Ok(NavPathReadout {
        outcome: NavPathOutcome::NoPath,
        visited: visited.len(),
        path: Vec::new(),
        path_hash: hash_path(&[]),
    })
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

fn planar_nav_neighbors(
    coord: VoxelCoord,
    policy: PlanarNavNeighborPolicy,
) -> impl Iterator<Item = VoxelCoord> {
    let horizontal = [(1, 0), (0, 1), (-1, 0), (0, -1)];
    let mut neighbors = Vec::with_capacity(4 * (1 + usize::from(policy.max_step_cells) * 2));
    for (dx, dz) in horizontal {
        neighbors.push(VoxelCoord::new(coord.x + dx, coord.y, coord.z + dz));
        for step in 1..=i64::from(policy.max_step_cells) {
            neighbors.push(VoxelCoord::new(coord.x + dx, coord.y + step, coord.z + dz));
            neighbors.push(VoxelCoord::new(coord.x + dx, coord.y - step, coord.z + dz));
        }
    }
    neighbors.into_iter()
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

fn hash_walkable(walkable: &BTreeSet<VoxelCoord>) -> u64 {
    let mut h = fnv_offset();
    feed_u64(&mut h, walkable.len() as u64);
    for coord in walkable {
        feed_coord(&mut h, *coord);
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
        let policy = PlanarNavNeighborPolicy { max_step_cells: 1 };

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
            PlanarNavNeighborPolicy { max_step_cells: 1 },
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
            PlanarNavNeighborPolicy { max_step_cells: 2 },
        )
        .expect("explicit drop query");
        assert_eq!(allowed.path, vec![start, goal]);
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
        assert_eq!(projection.projection_hash(), 0x59b4_0936_25b1_0e49);
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
        assert_eq!(first.projection_hash, 0x59b4_0936_25b1_0e49);
        assert_eq!(first.path_hash, 0x09ed_0284_f7c1_75e1);
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
