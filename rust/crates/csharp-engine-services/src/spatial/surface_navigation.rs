//! Surface navigation (`ReplaceSurfaceNavigation`): places on floors, walls
//! and ceilings where a surface crawler's ball rests, and paths across them.
//! A surface grid stands beside the planar grids under its own id; planar
//! requests naming it are refused, and every publication derives its whole
//! region.

use super::collision_navigation::{in_parallel, Lane};
use super::*;
use engine_spatial::{
    sample_surface_line, surface_corner, surface_edge, surface_edge_kind, surface_lines,
    CrawlerContact, CrawlerSurface, SurfaceCell, SurfaceEdgeKind, SurfaceEdgeOutcome, SurfaceFace,
    SurfaceLine, SurfaceNavigation, SurfaceNavigationConfig, SurfaceNavigationError, SurfaceNode,
    SurfaceRegion, SurfaceSampleOutcome,
};
use svc_pathfinding::{find_surface_path, SurfaceGraph, SurfacePathOutcome};

/// The default grid's cells, in metres: four 0.25 m balls across.
const DEFAULT_CELL_SIZE: f64 = 1.0;
const DEFAULT_MAXIMUM_CELLS: u32 = 1 << 20;
const DEFAULT_FLOOR_COST: u32 = 1;
const DEFAULT_WALL_COST: u32 = 2;
const DEFAULT_CEILING_COST: u32 = 3;

/// A checked surface-grid publication.
#[derive(Debug, Clone, Copy)]
pub(super) struct SurfacePolicy {
    grid_id: u64,
    config: SurfaceNavigationConfig,
    /// Floor, wall and ceiling.
    costs: [u32; 3],
    snap: f64,
    region: SurfaceRegion,
}

impl SurfacePolicy {
    fn cost(&self, surface: CrawlerSurface) -> u32 {
        match surface {
            CrawlerSurface::Wall => self.costs[1],
            CrawlerSurface::Ceiling => self.costs[2],
            CrawlerSurface::Floor | CrawlerSurface::None => self.costs[0],
        }
    }
}

/// An installed surface grid.
pub(super) struct SurfaceGrid {
    policy: SurfacePolicy,
    navigation: SurfaceNavigation,
    graph: SurfaceGraph,
    receipt: NativeSurfaceNavigationReplaceReceipt,
}

/// What a deferred derivation's worker hands back: the grid, `None` once
/// cancelled, or why it was refused.
type SurfaceOutcome = Result<Option<SurfaceGrid>, String>;

pub(super) struct PendingSurfaceDerivation {
    /// Held for its drop.
    _cancel: CancelOnDrop,
    worker: std::thread::JoinHandle<SurfaceOutcome>,
}

fn refusal(code: &'static str, message: impl Into<String>) -> CsharpEngineServicesError {
    CsharpEngineServicesError::new(code, message)
}

fn failed(error: SurfaceNavigationError) -> CsharpEngineServicesError {
    refusal(error.code(), error.to_string())
}

fn face(value: NativeSurfaceFace) -> SurfaceFace {
    match value {
        NativeSurfaceFace::PositiveX => SurfaceFace::PositiveX,
        NativeSurfaceFace::NegativeX => SurfaceFace::NegativeX,
        NativeSurfaceFace::PositiveY => SurfaceFace::PositiveY,
        NativeSurfaceFace::NegativeY => SurfaceFace::NegativeY,
        NativeSurfaceFace::PositiveZ => SurfaceFace::PositiveZ,
        NativeSurfaceFace::NegativeZ => SurfaceFace::NegativeZ,
    }
}

fn native_face(value: SurfaceFace) -> NativeSurfaceFace {
    match value {
        SurfaceFace::PositiveX => NativeSurfaceFace::PositiveX,
        SurfaceFace::NegativeX => NativeSurfaceFace::NegativeX,
        SurfaceFace::PositiveY => NativeSurfaceFace::PositiveY,
        SurfaceFace::NegativeY => NativeSurfaceFace::NegativeY,
        SurfaceFace::PositiveZ => NativeSurfaceFace::PositiveZ,
        SurfaceFace::NegativeZ => NativeSurfaceFace::NegativeZ,
    }
}

fn cell(value: NativeSurfaceCell) -> SurfaceCell {
    SurfaceCell {
        cell: VoxelCoord::new(value.x, value.y, value.z),
        face: face(value.face),
    }
}

fn native_cell(value: SurfaceCell) -> NativeSurfaceCell {
    NativeSurfaceCell {
        x: value.cell.x,
        y: value.cell.y,
        z: value.cell.z,
        face: native_face(value.face),
    }
}

fn native_kind(value: SurfaceEdgeKind) -> NativeSurfaceEdgeKind {
    match value {
        SurfaceEdgeKind::Along => NativeSurfaceEdgeKind::Along,
        SurfaceEdgeKind::Concave => NativeSurfaceEdgeKind::Concave,
        SurfaceEdgeKind::Convex => NativeSurfaceEdgeKind::Convex,
    }
}

/// The graph's tag for an edge kind.
fn tag(value: SurfaceEdgeKind) -> u8 {
    match value {
        SurfaceEdgeKind::Along => 0,
        SurfaceEdgeKind::Concave => 1,
        SurfaceEdgeKind::Convex => 2,
    }
}

fn kind_of(tag: u8) -> NativeSurfaceEdgeKind {
    match tag {
        1 => NativeSurfaceEdgeKind::Concave,
        2 => NativeSurfaceEdgeKind::Convex,
        _ => NativeSurfaceEdgeKind::Along,
    }
}

fn pos3(value: core_space::WorldPos) -> NativeVec3 {
    NativeVec3 {
        x: value.x as f32,
        y: value.y as f32,
        z: value.z as f32,
    }
}

fn vec3(value: core_space::WorldVec) -> NativeVec3 {
    NativeVec3 {
        x: value.x as f32,
        y: value.y as f32,
        z: value.z as f32,
    }
}

fn contact(
    source: engine_spatial::CharacterCollisionSource,
    point: core_space::WorldPos,
    normal: core_space::WorldVec,
    distance: f64,
) -> NativeCrawlerContact {
    super::crawler::native_contact(Some(CrawlerContact {
        source,
        point: Vec3::new(point.x as f32, point.y as f32, point.z as f32),
        normal: Vec3::new(normal.x as f32, normal.y as f32, normal.z as f32),
        distance: distance as f32,
    }))
}

fn waypoint(node: &SurfaceNode, edge: NativeSurfaceEdgeKind) -> NativeSurfaceWaypoint {
    NativeSurfaceWaypoint {
        cell: native_cell(node.cell),
        position: pos3(node.rest),
        normal: vec3(node.normal),
        surface: super::crawler::native_surface(node.surface),
        edge,
    }
}

pub(super) fn default_native_config() -> NativeSurfaceNavigationConfig {
    NativeSurfaceNavigationConfig {
        // Planar and surface grids share ids, so the product names one.
        grid_id: 0,
        cell_size: DEFAULT_CELL_SIZE,
        maximum_cells: DEFAULT_MAXIMUM_CELLS,
        crawler: super::crawler::native_config(engine_spatial::CrawlerConfig::default()),
        floor_cost: DEFAULT_FLOOR_COST,
        wall_cost: DEFAULT_WALL_COST,
        ceiling_cost: DEFAULT_CEILING_COST,
        snap_distance: DEFAULT_CELL_SIZE,
    }
}

fn policy(
    request: &NativeSurfaceNavigationReplaceRequest,
) -> Result<SurfacePolicy, CsharpEngineServicesError> {
    let native = request.config;
    let invalid = |message: &str| refusal("invalid-surface-navigation-config", message);
    if native.grid_id == 0 {
        return Err(invalid(
            "name the surface grid's GridId: not 0, and not a planar grid's",
        ));
    }
    let config = SurfaceNavigationConfig {
        cell_size: native.cell_size,
        crawler: super::crawler::config(native.crawler),
    };
    config.validate().map_err(failed)?;
    let costs = [native.floor_cost, native.wall_cost, native.ceiling_cost];
    if costs.contains(&0) {
        return Err(invalid("floor, wall and ceiling costs are at least 1"));
    }
    if !(native.snap_distance.is_finite() && native.snap_distance >= 0.0) {
        return Err(invalid("the snap distance must be finite and not negative"));
    }
    let (low, high) = (
        native_vec3_value(request.world_min),
        native_vec3_value(request.world_max),
    );
    if !finite_vec3(low) || !finite_vec3(high) {
        return Err(refusal(
            "invalid-surface-navigation-region",
            "the surface navigation region must be finite",
        ));
    }
    // The region lies between the corners in either order; a cell is in it
    // when its centre's cell is.
    let corner = |pick: fn(f32, f32) -> f32, nudge: f64| {
        core_space::WorldPos::new(
            f64::from(pick(low.x, high.x)) + nudge,
            f64::from(pick(low.y, high.y)) + nudge,
            f64::from(pick(low.z, high.z)) + nudge,
        )
    };
    let region = SurfaceRegion {
        min: config.cell_of(corner(f32::min, 0.0)),
        max: config.cell_of(corner(f32::max, -1.0e-9)),
    };
    let span = |low: i64, high: i64| (i128::from(high) - i128::from(low) + 1).max(0);
    let cells = span(region.min.x, region.max.x)
        * span(region.min.y, region.max.y)
        * span(region.min.z, region.max.z);
    if cells == 0 || cells > i128::from(native.maximum_cells) {
        return Err(refusal(
            "surface-navigation-over-budget",
            format!(
                "the region covers {cells} cells; the budget is {}",
                native.maximum_cells
            ),
        ));
    }
    Ok(SurfacePolicy {
        grid_id: native.grid_id,
        config,
        costs,
        snap: native.snap_distance,
        region,
    })
}

/// Every place of the region and the edges between them, spread across the
/// lane's threads; `None` once a worker is cancelled.
fn derive(
    scene: &VoxelCollisionScene,
    policy: SurfacePolicy,
    lane: Lane,
    started: Instant,
) -> Result<Option<SurfaceGrid>, SurfaceNavigationError> {
    let (config, region) = (policy.config, policy.region);
    let lines = surface_lines(region);
    let Some(sampled) = in_parallel(&lines, lane, |line| {
        sample_surface_line(scene, &config, region, *line, None)
    }) else {
        return Ok(None);
    };
    let mut nodes = Vec::new();
    for line in sampled {
        nodes.extend(line?);
    }
    let mut navigation = SurfaceNavigation::of_nodes(nodes);
    let slots: Vec<u32> = (0..navigation.nodes.len() as u32).collect();
    let Some(edges) = in_parallel(&slots, lane, |&slot| {
        navigation.edges_from(scene, &config, slot)
    }) else {
        return Ok(None);
    };
    for found in edges {
        let (edges, tests) = found?;
        navigation.edges.extend(edges);
        navigation.edge_tests += tests;
    }
    let graph = SurfaceGraph::new(
        navigation
            .nodes
            .iter()
            .map(|node| (node.cell.cell, policy.cost(node.surface))),
        navigation
            .edges
            .iter()
            .map(|edge| (edge.from, edge.to, tag(edge.kind))),
    );
    let receipt = NativeSurfaceNavigationReplaceReceipt {
        surface_cell_count: navigation.nodes.len() as u64,
        edge_count: graph.edge_count() as u64,
        edge_test_count: navigation.edge_tests,
        component_count: u64::from(graph.component_count()),
        navigation_revision: 0,
        derivation_microseconds: u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX),
        pending: false,
    };
    Ok(Some(SurfaceGrid {
        policy,
        navigation,
        graph,
        receipt,
    }))
}

impl SurfaceGrid {
    /// The place whose resting ball is nearest `at`, within the snap
    /// distance.
    fn snap(&self, at: core_space::WorldPos) -> Option<u32> {
        let center = self.policy.config.cell_of(at);
        let mut best: Option<(f64, u32)> = None;
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let near = VoxelCoord::new(center.x + dx, center.y + dy, center.z + dz);
                    for face in SurfaceFace::ALL {
                        let Some(&slot) =
                            self.navigation.index.get(&SurfaceCell { cell: near, face })
                        else {
                            continue;
                        };
                        let rest = self.navigation.nodes[slot as usize].rest;
                        let distance = ((rest.x - at.x).powi(2)
                            + (rest.y - at.y).powi(2)
                            + (rest.z - at.z).powi(2))
                        .sqrt();
                        if best.is_none_or(|(least, _)| distance < least) {
                            best = Some((distance, slot));
                        }
                    }
                }
            }
        }
        best.filter(|&(distance, _)| distance <= self.policy.snap)
            .map(|(_, slot)| slot)
    }

    fn node(&self, slot: u32) -> &SurfaceNode {
        &self.navigation.nodes[slot as usize]
    }

    /// The grid after a world-origin rebase moved local coordinates by
    /// `shift`, when that is whole cells; otherwise none.
    fn rebased(mut self, shift: [f64; 3]) -> Option<Self> {
        let cells = shift.map(|value| value / self.policy.config.cell_size);
        if cells
            .iter()
            .any(|value| value.fract() != 0.0 || value.abs() > 1.0e15)
        {
            return None;
        }
        let [dx, dy, dz] = cells.map(|value| value as i64);
        let moved = |cell: VoxelCoord| VoxelCoord::new(cell.x + dx, cell.y + dy, cell.z + dz);
        let shifted = |at: core_space::WorldPos| {
            core_space::WorldPos::new(at.x + shift[0], at.y + shift[1], at.z + shift[2])
        };
        for node in &mut self.navigation.nodes {
            node.cell.cell = moved(node.cell.cell);
            node.rest = shifted(node.rest);
            node.point = shifted(node.point);
        }
        self.navigation.index = self
            .navigation
            .nodes
            .iter()
            .enumerate()
            .map(|(slot, node)| (node.cell, slot as u32))
            .collect();
        self.policy.region = SurfaceRegion {
            min: moved(self.policy.region.min),
            max: moved(self.policy.region.max),
        };
        // The graph's cells only estimate distances, which a whole-cell move
        // keeps.
        Some(self)
    }

    fn info(&self, grid_id: u64) -> NativeNavigationGridInfo {
        let region = self.policy.region;
        let radius = self.policy.config.crawler.radius;
        NativeNavigationGridInfo {
            grid_id,
            kind: NativeNavigationProjectionKind::CollisionSurface,
            latest: false,
            walkable_cell_count: self.receipt.surface_cell_count,
            projection_hash: 0,
            navigation_revision: self.receipt.navigation_revision,
            cell_size: self.policy.config.cell_size,
            agent_radius: radius,
            agent_height: 2.0 * radius,
            min_x: region.min.x,
            max_x: region.max.x,
            min_z: region.min.z,
            max_z: region.max.z,
        }
    }
}

impl SpatialSession {
    /// Refuses a planar request naming a surface grid.
    pub(super) fn refuse_surface_grid(
        &self,
        grid_id: u64,
    ) -> Result<(), CsharpEngineServicesError> {
        if grid_id != 0
            && (self.surface_grids.contains_key(&grid_id)
                || self.surface_derivations.contains_key(&grid_id))
        {
            return Err(refusal(
                "navigation-grid-is-surface",
                format!(
                    "grid {grid_id} is a surface grid: request its paths with \
                     RequestSurfaceNavigationPath"
                ),
            ));
        }
        Ok(())
    }

    fn holds_planar_grid(&self, grid_id: u64) -> bool {
        self.navigation
            .as_ref()
            .is_some_and(|latest| latest.grid_id() == grid_id)
            || self.grids_beside.contains_key(&grid_id)
            || self.derivations.contains_key(&grid_id)
    }

    fn install_surface_grid(
        &mut self,
        mut grid: SurfaceGrid,
    ) -> NativeSurfaceNavigationReplaceReceipt {
        self.navigation_revision += 1;
        grid.receipt.navigation_revision = self.navigation_revision;
        let receipt = grid.receipt;
        let grid_id = grid.policy.grid_id;
        self.refused_surface_derivations.remove(&grid_id);
        if let Some(replaced) = self.surface_grids.insert(grid_id, grid) {
            // Freeing a large grid takes an update milliseconds.
            let _ = std::thread::Builder::new()
                .name("rusty-navigation-release".to_owned())
                .spawn(move || drop(replaced));
        }
        receipt
    }

    /// Installs the surface grids whose deferred derivations are done,
    /// without waiting for any still deriving.
    pub(super) fn install_surface_derivations(&mut self) {
        let finished: Vec<u64> = self
            .surface_derivations
            .iter()
            .filter(|(_, pending)| pending.worker.is_finished())
            .map(|(grid_id, _)| *grid_id)
            .collect();
        for grid_id in finished {
            let pending = self
                .surface_derivations
                .remove(&grid_id)
                .expect("listed above");
            let outcome = pending
                .worker
                .join()
                .unwrap_or_else(|_| Err("the worker panicked".to_owned()));
            match outcome {
                Ok(Some(grid)) => {
                    self.install_surface_grid(grid);
                }
                // Only a dropped entry is cancelled, and this one was not.
                Ok(None) => {}
                Err(reason) => {
                    eprintln!("SURFACE_NAVIGATION_DEFERRED: grid {grid_id}: {reason}");
                    self.refused_surface_derivations.insert(grid_id);
                }
            }
        }
    }

    pub(super) fn clear_surface_grid(&mut self, grid_id: u64) {
        self.surface_grids.remove(&grid_id);
        self.surface_derivations.remove(&grid_id);
        self.refused_surface_derivations.remove(&grid_id);
    }

    pub(super) fn clear_surface_grids(&mut self) {
        self.surface_grids.clear();
        self.surface_derivations.clear();
        self.refused_surface_derivations.clear();
    }

    pub(super) fn surface_grid_infos(&self) -> impl Iterator<Item = NativeNavigationGridInfo> + '_ {
        self.surface_grids
            .iter()
            .map(|(grid_id, grid)| grid.info(*grid_id))
    }

    /// After a world-origin rebase: derivations still running are dropped
    /// and grids move by whole cells, or are dropped.
    pub(super) fn rebase_surface_grids(&mut self, shift: [f64; 3]) {
        self.surface_derivations.clear();
        self.surface_grids = std::mem::take(&mut self.surface_grids)
            .into_iter()
            .filter_map(|(grid_id, grid)| Some((grid_id, grid.rebased(shift)?)))
            .collect();
    }

    fn surface_grid(&self, grid_id: u64) -> Result<&SurfaceGrid, CsharpEngineServicesError> {
        self.surface_grids.get(&grid_id).ok_or_else(|| {
            refusal(
                "surface-navigation-unpublished",
                format!("the session holds no surface grid {grid_id}"),
            )
        })
    }
}

impl RuntimeSpatialBridge {
    /// The session, refusing a planar request that names a surface grid.
    pub(super) fn planar_session(
        &mut self,
        handle: NativeSpatialSessionHandle,
        grid_id: u64,
    ) -> Result<&mut SpatialSession, CsharpEngineServicesError> {
        let session = self.session_mut(handle)?;
        session.refuse_surface_grid(grid_id)?;
        Ok(session)
    }

    fn replace_surface_navigation(
        &mut self,
        request: &NativeSurfaceNavigationReplaceRequest,
    ) -> Result<NativeSurfaceNavigationReplaceReceipt, CsharpEngineServicesError> {
        // Checking the request reads no session state; only a derivation in
        // the call installs the deferred batches first (below).
        let policy = policy(request)?;
        if self
            .session_mut(request.session)?
            .holds_planar_grid(policy.grid_id)
        {
            return Err(refusal(
                "surface-navigation-grid-in-use",
                format!("grid {} is a planar grid", policy.grid_id),
            ));
        }
        if request.deferred {
            return self.derive_surface_deferred(request.session, policy);
        }
        self.settle_deferred(request.session)?;
        let session = self.session_mut(request.session)?;
        session.surface_derivations.remove(&policy.grid_id);
        let grid = derive(&session.scene, policy, Lane::Call, Instant::now())
            .map_err(failed)?
            .expect("a derivation in the call is never cancelled");
        Ok(session.install_surface_grid(grid))
    }

    fn derive_surface_deferred(
        &mut self,
        handle: NativeSpatialSessionHandle,
        policy: SurfacePolicy,
    ) -> Result<NativeSurfaceNavigationReplaceReceipt, CsharpEngineServicesError> {
        let session = self.session_mut(handle)?;
        // A derivation of the grid already running is cancelled.
        session.surface_derivations.remove(&policy.grid_id);
        session.refused_surface_derivations.remove(&policy.grid_id);
        let scene = Arc::clone(&session.scene);
        let cancel = Arc::new(AtomicBool::new(false));
        let worker = {
            let cancel = Arc::clone(&cancel);
            std::thread::Builder::new()
                .name("rusty-surface-navigation".to_owned())
                .spawn(move || -> SurfaceOutcome {
                    crate::worker_priority::below_normal();
                    let grid = derive(&scene, policy, Lane::Worker(&cancel), Instant::now())
                        .map_err(|error| error.to_string());
                    // Collision edits stop copying the scene once it is let go.
                    drop(scene);
                    grid
                })
                .map_err(|error| {
                    refusal(
                        "surface-navigation-worker-failed",
                        format!("no worker thread to derive the surface navigation: {error}"),
                    )
                })?
        };
        session.surface_derivations.insert(
            policy.grid_id,
            PendingSurfaceDerivation {
                _cancel: CancelOnDrop(cancel),
                worker,
            },
        );
        Ok(NativeSurfaceNavigationReplaceReceipt {
            pending: true,
            ..Default::default()
        })
    }

    fn read_surface_navigation(
        &mut self,
        request: NativeSurfaceNavigationReadRequest,
    ) -> Result<NativeSurfaceNavigationReadout, CsharpEngineServicesError> {
        let session = self.session_mut(request.session)?;
        Ok(NativeSurfaceNavigationReadout {
            pending: session.surface_derivations.contains_key(&request.grid_id),
            refused: session
                .refused_surface_derivations
                .contains(&request.grid_id),
            installed: session
                .surface_grids
                .get(&request.grid_id)
                .map(|grid| grid.receipt)
                .unwrap_or_default(),
        })
    }

    fn request_surface_navigation_path(
        &mut self,
        request: NativeSurfaceNavigationPathRequest,
    ) -> Result<NativeSurfaceNavigationPathResult, CsharpEngineServicesError> {
        let session = self.session_mut(request.session)?;
        let navigation_revision = session.navigation_revision;
        let Some(grid) = session.surface_grids.get(&request.grid_id) else {
            return Ok(NativeSurfaceNavigationPathResult {
                outcome: NativeSurfaceNavigationPathOutcome::Unavailable,
                navigation_revision,
                ..Default::default()
            });
        };
        let at = |value: NativeVec3| {
            core_space::WorldPos::new(f64::from(value.x), f64::from(value.y), f64::from(value.z))
        };
        let off = |outcome| NativeSurfaceNavigationPathResult {
            outcome,
            navigation_revision: grid.receipt.navigation_revision,
            ..Default::default()
        };
        let Some(start) = grid.snap(at(request.from)) else {
            return Ok(off(NativeSurfaceNavigationPathOutcome::StartOffSurface));
        };
        let Some(goal) = grid.snap(at(request.to)) else {
            return Ok(off(NativeSurfaceNavigationPathOutcome::GoalOffSurface));
        };
        let path = find_surface_path(&grid.graph, start, goal, request.max_visited);
        let waypoints: Box<[NativeSurfaceWaypoint]> = path
            .places
            .iter()
            .map(|&(slot, tag)| waypoint(grid.node(slot), kind_of(tag)))
            .collect();
        let result = NativeSurfaceNavigationPathResult {
            waypoints: waypoints.as_ptr(),
            waypoints_len: waypoints.len(),
            outcome: match path.outcome {
                SurfacePathOutcome::Reached => NativeSurfaceNavigationPathOutcome::Reached,
                SurfacePathOutcome::NoPath => NativeSurfaceNavigationPathOutcome::NoPath,
                SurfacePathOutcome::BudgetExhausted => {
                    NativeSurfaceNavigationPathOutcome::BudgetExhausted
                }
            },
            total_cost: path.total_cost,
            visited: path.visited,
            nearest: waypoint(grid.node(path.nearest), NativeSurfaceEdgeKind::Along),
            navigation_revision: grid.receipt.navigation_revision,
        };
        self.borrowed.hold(waypoints);
        Ok(result)
    }

    /// The place `at` sampled against current collision, with every surface
    /// its line met in the cell.
    fn sample_place(
        scene: &VoxelCollisionScene,
        policy: &SurfacePolicy,
        at: SurfaceCell,
    ) -> Result<Option<engine_spatial::SurfaceSample>, CsharpEngineServicesError> {
        let mut record = Vec::new();
        sample_surface_line(
            scene,
            &policy.config,
            policy.region,
            SurfaceLine::through(at),
            Some(&mut record),
        )
        .map_err(failed)?;
        Ok(record.into_iter().find(|sample| sample.cell == at))
    }

    fn explain_surface_navigation_cell(
        &mut self,
        request: &NativeSurfaceNavigationCellRequest,
    ) -> Result<NativeSurfaceNavigationCellReadout, CsharpEngineServicesError> {
        let session = self.session_mut(request.session)?;
        let grid = session.surface_grid(request.grid_id)?;
        let at = cell(request.cell);
        let mut readout = NativeSurfaceNavigationCellReadout {
            admitted: grid.navigation.index.contains_key(&at),
            navigation_revision: grid.receipt.navigation_revision,
            ..Default::default()
        };
        if !grid.policy.region.contains(at.cell) {
            readout.outcome = NativeSurfaceCellOutcome::OutsideGrid;
            return Ok(readout);
        }
        let Some(sample) = Self::sample_place(&session.scene, &grid.policy, at)? else {
            readout.outcome = NativeSurfaceCellOutcome::NoSurface;
            return Ok(readout);
        };
        readout.outcome = match sample.outcome {
            SurfaceSampleOutcome::Surface | SurfaceSampleOutcome::SameCell => {
                NativeSurfaceCellOutcome::Surface
            }
            SurfaceSampleOutcome::OtherFacing => NativeSurfaceCellOutcome::OtherFacing,
            SurfaceSampleOutcome::BallOverlap => NativeSurfaceCellOutcome::BallOverlap,
            SurfaceSampleOutcome::InsideCollision => NativeSurfaceCellOutcome::InsideCollision,
        };
        let rest_distance =
            f64::from(grid.policy.config.crawler.radius + grid.policy.config.crawler.contact_skin);
        readout.surface_contact =
            contact(sample.source, sample.point, sample.normal, rest_distance);
        readout.position = pos3(sample.rest);
        readout.surface =
            super::crawler::native_surface(grid.policy.config.classify(sample.normal));
        if let Some((source, point)) = sample.overlap {
            readout.overlap = contact(source, point, core_space::WorldVec::new(0.0, 0.0, 0.0), 0.0);
        }
        Ok(readout)
    }

    fn explain_surface_navigation_edge(
        &mut self,
        request: &NativeSurfaceNavigationEdgeRequest,
    ) -> Result<NativeSurfaceNavigationEdgeReadout, CsharpEngineServicesError> {
        let session = self.session_mut(request.session)?;
        let grid = session.surface_grid(request.grid_id)?;
        let (from, to) = (cell(request.from), cell(request.to));
        let slots = (
            grid.navigation.index.get(&from).copied(),
            grid.navigation.index.get(&to).copied(),
        );
        let mut readout = NativeSurfaceNavigationEdgeReadout {
            admitted: matches!(slots, (Some(a), Some(b)) if grid.graph.edge(a, b).is_some()),
            navigation_revision: grid.receipt.navigation_revision,
            ..Default::default()
        };
        let Some(kind) = surface_edge_kind(from, to) else {
            readout.outcome = NativeSurfaceEdgeOutcome::NotNeighbor;
            return Ok(readout);
        };
        readout.kind = native_kind(kind);
        let place = |at: SurfaceCell| -> Result<Option<SurfaceNode>, CsharpEngineServicesError> {
            if !grid.policy.region.contains(at.cell) {
                return Ok(None);
            }
            Ok(Self::sample_place(&session.scene, &grid.policy, at)?
                .filter(|sample| sample.outcome == SurfaceSampleOutcome::Surface)
                .map(|sample| SurfaceNode {
                    cell: at,
                    rest: sample.rest,
                    point: sample.point,
                    normal: sample.normal,
                    surface: grid.policy.config.classify(sample.normal),
                    source: sample.source,
                }))
        };
        let Some(from_node) = place(from)? else {
            readout.outcome = NativeSurfaceEdgeOutcome::FromNotSurface;
            return Ok(readout);
        };
        let Some(to_node) = place(to)? else {
            readout.outcome = NativeSurfaceEdgeOutcome::ToNotSurface;
            return Ok(readout);
        };
        readout.cost = grid.policy.cost(to_node.surface);
        if kind != SurfaceEdgeKind::Along {
            readout.corner_present = true;
            readout.corner = pos3(surface_corner(&from_node, &to_node));
        }
        match surface_edge(
            &session.scene,
            &grid.policy.config,
            &from_node,
            &to_node,
            kind,
        )
        .map_err(failed)?
        {
            SurfaceEdgeOutcome::Traversable => {
                readout.outcome = NativeSurfaceEdgeOutcome::Traversable;
            }
            SurfaceEdgeOutcome::Blocked(block) => {
                readout.outcome = NativeSurfaceEdgeOutcome::Blocked;
                readout.blocked_at = pos3(block.at);
                readout.blocker = contact(block.source, block.point, block.normal, 0.0);
            }
        }
        Ok(readout)
    }
}

pub(super) unsafe extern "C" fn default_config(
    context: *mut c_void,
    config: *mut NativeSurfaceNavigationConfig,
) -> i32 {
    if context.is_null() || config.is_null() {
        return 0;
    }
    unsafe { *config = default_native_config() };
    ABI_OK
}

pub(super) unsafe extern "C" fn replace(
    context: *mut c_void,
    request: *const NativeSurfaceNavigationReplaceRequest,
    receipt: *mut NativeSurfaceNavigationReplaceReceipt,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(error);
    if context.is_null() || request.is_null() || receipt.is_null() {
        return 0;
    }
    let request = unsafe { &*request };
    match unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }
        .replace_surface_navigation(request)
    {
        Ok(value) => {
            unsafe { *receipt = value };
            ABI_OK
        }
        Err(refusal) => refuse(&refusal, error),
    }
}

pub(super) unsafe extern "C" fn read(
    context: *mut c_void,
    request: NativeSurfaceNavigationReadRequest,
    readout: *mut NativeSurfaceNavigationReadout,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(error);
    if context.is_null() || readout.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }.read_surface_navigation(request) {
        Ok(value) => {
            unsafe { *readout = value };
            ABI_OK
        }
        Err(refusal) => refuse(&refusal, error),
    }
}

pub(super) unsafe extern "C" fn request_path(
    context: *mut c_void,
    request: NativeSurfaceNavigationPathRequest,
    result: *mut NativeSurfaceNavigationPathResult,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(error);
    if context.is_null() || result.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }
        .request_surface_navigation_path(request)
    {
        Ok(value) => {
            unsafe { *result = value };
            ABI_OK
        }
        Err(refusal) => refuse(&refusal, error),
    }
}

pub(super) unsafe extern "C" fn explain_cell(
    context: *mut c_void,
    request: *const NativeSurfaceNavigationCellRequest,
    readout: *mut NativeSurfaceNavigationCellReadout,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(error);
    if context.is_null() || request.is_null() || readout.is_null() {
        return 0;
    }
    let request = unsafe { &*request };
    match unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }
        .explain_surface_navigation_cell(request)
    {
        Ok(value) => {
            unsafe { *readout = value };
            ABI_OK
        }
        Err(refusal) => refuse(&refusal, error),
    }
}

pub(super) unsafe extern "C" fn explain_edge(
    context: *mut c_void,
    request: *const NativeSurfaceNavigationEdgeRequest,
    readout: *mut NativeSurfaceNavigationEdgeReadout,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(error);
    if context.is_null() || request.is_null() || readout.is_null() {
        return 0;
    }
    let request = unsafe { &*request };
    match unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }
        .explain_surface_navigation_edge(request)
    {
        Ok(value) => {
            unsafe { *readout = value };
            ABI_OK
        }
        Err(refusal) => refuse(&refusal, error),
    }
}

#[cfg(test)]
mod tests;
