//! Collision-derived planar navigation. A column's supports depend only on
//! the collision inside its footprint and the requested vertical range, and a
//! directed edge only on the two columns it joins, so a publication keeps the
//! columns and edges of the previous one wherever neither the box nor the
//! scene changed there, and derives the rest.

use super::*;

/// A scene change may reach one cell beyond its bounds: a standing capsule is
/// narrower than a cell, and an edge sweep stays within the two cells it
/// joins, so re-deriving every column within one cell of a change is exact.
const DIRTY_MARGIN_CELLS: f64 = 1.0;
/// Beyond this many recorded changes the next publication derives everything.
const MAX_DIRTY_REGIONS: usize = 256;

/// A validated collision-navigation policy: everything besides the scene
/// and the vertical range that a derived column or edge depends on.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct CollisionNavigationPolicy {
    pub(super) grid_id: u64,
    pub(super) cell_size: f64,
    pub(super) chunk_size: u32,
    pub(super) character: CharacterControllerConfig,
    pub(super) maximum_drop: f64,
    pub(super) vertical_search_cells: u8,
    pub(super) supports_per_column: u32,
    pub(super) diagonal: bool,
    pub(super) jump: JumpPolicy,
}

/// Which jump edges a publication derives, from the character's own jump.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) struct JumpPolicy {
    pub(super) ledges: bool,
    pub(super) gap_cells: u8,
    pub(super) cost: u32,
}

/// One admitted edge's destination, and whether it is a jump.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct EdgeTarget {
    pub(super) to: VoxelCoord,
    pub(super) jump: bool,
}

/// How far a query point may be from a support and still stand on it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct SupportSnap {
    pub(super) above: f64,
    pub(super) below: f64,
    pub(super) across: f64,
}

impl CollisionNavigationPolicy {
    pub(super) fn from_native(
        config: &NativeCollisionNavigationConfig,
    ) -> Result<(Self, SupportSnap), CsharpEngineServicesError> {
        let invalid =
            |message| CsharpEngineServicesError::new("CSHARP_COLLISION_NAVIGATION_CONFIG", message);
        let character = character_config(config.character)?;
        character
            .validate()
            .map_err(|_| invalid("collision navigation character configuration is invalid"))?;
        let snap = SupportSnap {
            above: config.snap_above,
            below: config.snap_below,
            across: config.snap_across,
        };
        if !(config.cell_size.is_finite() && config.cell_size > 0.0)
            || f64::from(character.shape.radius) * 2.0 > config.cell_size
            || character.surface.maximum_slope_radians > 89.0_f32.to_radians()
        {
            return Err(invalid(
                "collision navigation needs a positive cell at least the agent's diameter and a slope of at most 89 degrees",
            ));
        }
        if !(config.maximum_drop.is_finite() && config.maximum_drop >= 0.0)
            || config.supports_per_column == 0
            || [snap.above, snap.below, snap.across]
                .iter()
                .any(|value| !(value.is_finite() && *value >= 0.0))
        {
            return Err(invalid(
                "collision navigation drop and snap distances must be finite and non-negative, with at least one support per column",
            ));
        }
        let vertical_search_cells = u8::try_from(config.vertical_search_cells)
            .map_err(|_| invalid("collision navigation searches at most 255 cell levels"))?;
        let gap_cells = u8::try_from(config.jump_gap_cells)
            .map_err(|_| invalid("collision navigation jumps gaps of at most 255 cells"))?;
        let policy = Self {
            grid_id: config.grid_id,
            cell_size: config.cell_size,
            chunk_size: config.chunk_size,
            character,
            maximum_drop: config.maximum_drop,
            vertical_search_cells,
            supports_per_column: config.supports_per_column,
            diagonal: config.diagonal_neighbors,
            jump: JumpPolicy {
                ledges: config.jump_ledges,
                gap_cells,
                cost: config.jump_cost,
            },
        };
        Ok((policy, snap))
    }

    /// How many cell levels up or down a neighbour is searched.
    pub(super) fn reach_cells(&self) -> u32 {
        u32::from(self.vertical_search_cells)
    }

    pub(super) fn neighbor_policy(&self) -> PlanarNavNeighborPolicy {
        PlanarNavNeighborPolicy {
            max_step_cells: self.vertical_search_cells,
            diagonal: self.diagonal,
        }
    }

    pub(super) fn grid(&self) -> Result<VoxelGridSpec, CsharpEngineServicesError> {
        navigation_grid(NativePlanarNavConfig {
            grid_id: self.grid_id,
            cell_size: self.cell_size,
            chunk_size: self.chunk_size,
            max_step_cells: self.reach_cells(),
        })
    }
}

/// Everything besides the scene that a derived column depends on.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct DerivationKey {
    pub(super) policy: CollisionNavigationPolicy,
    pub(super) world_min_y: f64,
    pub(super) world_max_y: f64,
}

impl DerivationKey {
    fn translated(self, shift_y: f64) -> Self {
        Self {
            world_min_y: self.world_min_y + shift_y,
            world_max_y: self.world_max_y + shift_y,
            ..self
        }
    }
}

/// Identity of the collision a cache describes. Every mutation of voxels,
/// static meshes or the origin advances one of the revisions; the collidable
/// material rule has no revision, so it is hashed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SceneRevisions {
    voxel: u64,
    static_mesh: u64,
    rebase: u64,
    noncollidable: u64,
    /// Surface modes decide whether collision follows cubes or drawn
    /// surfaces, and have no revision either.
    surface: u64,
}

impl SceneRevisions {
    pub(super) fn of(scene: &VoxelCollisionScene) -> Self {
        let mix = |hash: u64, value: u64| (hash ^ value).wrapping_mul(0x0100_0000_01b3);
        let noncollidable = scene
            .noncollidable_materials()
            .iter()
            .fold(0xcbf2_9ce4_8422_2325_u64, |hash, slot| {
                mix(hash, u64::from(*slot))
            });
        let options = scene.mesh_options();
        let surface = options.materials.entries().iter().fold(
            mix(0xcbf2_9ce4_8422_2325_u64, options.mode as u64),
            |hash, (slot, surface)| {
                let hash = mix(mix(hash, u64::from(*slot)), surface.mode as u64);
                let character = surface.character;
                let hash = mix(hash, character.placement as u64);
                let hash = mix(hash, u64::from(character.crease_angle_degrees.to_bits()));
                mix(hash, u64::from(character.roughness.to_bits()))
            },
        );
        Self {
            voxel: scene.source_revision().raw(),
            static_mesh: scene.static_mesh_collision_revision(),
            rebase: scene.rebase_revision(),
            noncollidable,
            surface,
        }
    }
}

/// One X/Z column of the navigation grid.
type Column = (i64, i64);

/// The columns a publication's box covers, inclusive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ColumnBounds {
    pub(super) min_x: i64,
    pub(super) max_x: i64,
    pub(super) min_z: i64,
    pub(super) max_z: i64,
}

impl ColumnBounds {
    fn contains(self, (x, z): Column) -> bool {
        (self.min_x..=self.max_x).contains(&x) && (self.min_z..=self.max_z).contains(&z)
    }

    fn columns(self) -> impl Iterator<Item = Column> {
        (self.min_x..=self.max_x).flat_map(move |x| (self.min_z..=self.max_z).map(move |z| (x, z)))
    }

    fn len(self) -> u64 {
        (self.max_x - self.min_x + 1) as u64 * (self.max_z - self.min_z + 1) as u64
    }
}

/// The columns and edges of the last collision-derived publication, and the
/// world regions whose collision changed since.
#[derive(Debug, Clone)]
pub(super) struct CollisionNavigationCache {
    key: DerivationKey,
    /// The scene the columns plus `dirty` describe. A publication against any
    /// other scene derives everything.
    scene: SceneRevisions,
    bounds: ColumnBounds,
    columns: BTreeMap<Column, Vec<(VoxelCoord, f64)>>,
    edges: BTreeMap<VoxelCoord, Vec<EdgeTarget>>,
    /// World-space `[min_x, min_z, max_x, max_z]` of changes since; `None`
    /// when too many were recorded.
    dirty: Option<Vec<[f64; 4]>>,
    /// The navigation revision whose installed projection, supports and edge
    /// admission hold exactly these columns and edges; `None` once a rebase
    /// moved them.
    installed: Option<u64>,
}

/// The parts of an installed collision-derived navigation a publication
/// updates in place.
pub(super) struct CollisionNavigationGraph {
    pub(super) projection: NavProjection,
    pub(super) supports: BTreeMap<VoxelCoord, f64>,
    pub(super) edge_admission: NavEdgeAdmission,
    /// The admitted edges that are jumps.
    pub(super) jumps: BTreeSet<(VoxelCoord, VoxelCoord)>,
}

impl CollisionNavigationGraph {
    fn from_cache(grid: VoxelGridSpec, cache: &CollisionNavigationCache) -> Self {
        let supports: BTreeMap<VoxelCoord, f64> =
            cache.columns.values().flatten().copied().collect();
        let mut graph = Self {
            projection: NavProjection::from_walkable_cells(grid, supports.keys().copied()),
            supports,
            edge_admission: NavEdgeAdmission::from_allowed_edges(std::iter::empty()),
            jumps: BTreeSet::new(),
        };
        for (&from, targets) in &cache.edges {
            for &target in targets {
                graph.admit(from, target, cache.key.policy.jump.cost);
            }
        }
        graph
    }

    fn admit(&mut self, from: VoxelCoord, target: EdgeTarget, jump_cost: u32) {
        self.edge_admission.set_allowed(from, target.to, true);
        if target.jump {
            self.jumps.insert((from, target.to));
            self.edge_admission
                .set_cost(from, target.to, u64::from(jump_cost));
        }
    }

    fn withdraw(&mut self, from: VoxelCoord, target: EdgeTarget) {
        self.edge_admission.set_allowed(from, target.to, false);
        self.jumps.remove(&(from, target.to));
    }
}

impl CollisionNavigationCache {
    /// Records a change from `before` to the scene now described by `after`,
    /// confined to the given world boxes. A change the cache did not see
    /// coming (its scene is not `before`) leaves it describing nothing.
    pub(super) fn record_change(
        &mut self,
        before: SceneRevisions,
        after: SceneRevisions,
        regions: impl IntoIterator<Item = ([f64; 3], [f64; 3])>,
    ) {
        if self.scene != before {
            return;
        }
        self.scene = after;
        let Some(dirty) = &mut self.dirty else {
            return;
        };
        dirty.extend(
            regions
                .into_iter()
                .map(|(min, max)| [min[0], min[2], max[0], max[2]]),
        );
        if dirty.len() > MAX_DIRTY_REGIONS {
            self.dirty = None;
        }
    }

    /// Follows a world-origin rebase that moved local coordinates by
    /// `shift`. The grid is world-aligned, so a shift of whole cells moves
    /// every column and edge unchanged; any other shift leaves the cache
    /// describing nothing. The installed navigation does not move, so the
    /// next publication rebuilds it from the moved columns and edges.
    pub(super) fn rebase(
        &mut self,
        before: SceneRevisions,
        after: SceneRevisions,
        shift: [f64; 3],
    ) {
        if self.scene != before {
            return;
        }
        let cell_size = self.key.policy.cell_size;
        let cells = shift.map(|value| value / cell_size);
        if cells
            .iter()
            .any(|value| value.fract() != 0.0 || value.abs() > 1.0e15)
        {
            return;
        }
        let [dx, dy, dz] = cells.map(|value| value as i64);
        let moved = |cell: VoxelCoord| VoxelCoord::new(cell.x + dx, cell.y + dy, cell.z + dz);
        self.columns = std::mem::take(&mut self.columns)
            .into_iter()
            .map(|((x, z), supports)| {
                let supports = supports
                    .into_iter()
                    .map(|(cell, height)| (moved(cell), height + shift[1]))
                    .collect();
                ((x + dx, z + dz), supports)
            })
            .collect();
        self.edges = std::mem::take(&mut self.edges)
            .into_iter()
            .map(|(from, targets)| {
                let targets = targets
                    .into_iter()
                    .map(|target| EdgeTarget {
                        to: moved(target.to),
                        ..target
                    })
                    .collect();
                (moved(from), targets)
            })
            .collect();
        if let Some(dirty) = &mut self.dirty {
            for region in dirty {
                *region = [
                    region[0] + shift[0],
                    region[1] + shift[2],
                    region[2] + shift[0],
                    region[3] + shift[2],
                ];
            }
        }
        self.bounds = ColumnBounds {
            min_x: self.bounds.min_x + dx,
            max_x: self.bounds.max_x + dx,
            min_z: self.bounds.min_z + dz,
            max_z: self.bounds.max_z + dz,
        };
        self.key = self.key.clone().translated(shift[1]);
        self.scene = after;
        self.installed = None;
    }

    /// The navigation revision this cache was installed as, while the
    /// installed graph still matches it.
    pub(super) fn installed_revision(&self) -> Option<u64> {
        self.installed
    }

    pub(super) fn set_installed(&mut self, revision: u64) {
        self.installed = Some(revision);
    }

    pub(super) fn key(&self) -> &DerivationKey {
        &self.key
    }

    pub(super) fn admits(&self, from: VoxelCoord, to: VoxelCoord) -> bool {
        self.edges
            .get(&from)
            .is_some_and(|targets| targets.iter().any(|target| target.to == to))
    }

    fn supports(&self, column: Column) -> &[(VoxelCoord, f64)] {
        self.columns.get(&column).map_or(&[], Vec::as_slice)
    }
}

/// What one publication derives: the columns new to the box or near a
/// recorded change, the columns that left the box, and the edges of every
/// cell in or beside either. Everything else is kept from the last
/// publication.
pub(super) struct CollisionNavigationDelta {
    key: DerivationKey,
    scene: SceneRevisions,
    bounds: ColumnBounds,
    /// Derived against the last publication rather than from nothing.
    incremental: bool,
    derived: BTreeMap<Column, Vec<(VoxelCoord, f64)>>,
    removed: Vec<Column>,
    edge_columns: BTreeSet<Column>,
    edges: Vec<(VoxelCoord, Vec<EdgeTarget>)>,
    edge_tests: u64,
}

impl CollisionNavigationDelta {
    pub(super) fn derived_columns(&self) -> u64 {
        self.derived.len() as u64
    }

    pub(super) fn edge_tests(&self) -> u64 {
        self.edge_tests
    }

    pub(super) fn reused_columns(&self) -> u64 {
        self.bounds.len() - self.derived_columns()
    }

    /// Applies the delta to the cache it was derived against and to the
    /// graph installed from that cache, in place: the cost follows the
    /// derived columns, not the box. Without an installed graph (after a
    /// rebase) the graph is rebuilt from the updated cache.
    pub(super) fn apply(
        self,
        grid: VoxelGridSpec,
        previous: Option<CollisionNavigationCache>,
        installed: Option<CollisionNavigationGraph>,
    ) -> (CollisionNavigationCache, CollisionNavigationGraph) {
        let (mut cache, mut graph) = match previous.filter(|_| self.incremental) {
            Some(cache) => (cache, installed),
            None => (
                CollisionNavigationCache {
                    key: self.key.clone(),
                    scene: self.scene,
                    bounds: self.bounds,
                    columns: BTreeMap::new(),
                    edges: BTreeMap::new(),
                    dirty: None,
                    installed: None,
                },
                Some(CollisionNavigationGraph {
                    projection: NavProjection::from_walkable_cells(grid, std::iter::empty()),
                    supports: BTreeMap::new(),
                    edge_admission: NavEdgeAdmission::from_allowed_edges(std::iter::empty()),
                    jumps: BTreeSet::new(),
                }),
            ),
        };
        for column in &self.edge_columns {
            let Some(supports) = cache.columns.get(column) else {
                continue;
            };
            for &(from, _) in supports {
                let Some(targets) = cache.edges.remove(&from) else {
                    continue;
                };
                if let Some(graph) = &mut graph {
                    for target in targets {
                        graph.withdraw(from, target);
                    }
                }
            }
        }
        for column in self.removed.iter().chain(self.derived.keys()) {
            for (cell, _) in cache.columns.remove(column).unwrap_or_default() {
                if let Some(graph) = &mut graph {
                    graph.projection.set_walkable(cell, false);
                    graph.supports.remove(&cell);
                }
            }
        }
        for (column, supports) in self.derived {
            if let Some(graph) = &mut graph {
                for &(cell, height) in &supports {
                    graph.projection.set_walkable(cell, true);
                    graph.supports.insert(cell, height);
                }
            }
            cache.columns.insert(column, supports);
        }
        let jump_cost = self.key.policy.jump.cost;
        for (from, targets) in self.edges {
            if let Some(graph) = &mut graph {
                for &target in &targets {
                    graph.admit(from, target, jump_cost);
                }
            }
            cache.edges.insert(from, targets);
        }
        cache.key = self.key;
        cache.scene = self.scene;
        cache.bounds = self.bounds;
        cache.dirty = Some(Vec::new());
        cache.installed = None;
        let graph = graph.unwrap_or_else(|| CollisionNavigationGraph::from_cache(grid, &cache));
        (cache, graph)
    }
}

/// Derive a conservative, finite planar projection from the session's coherent
/// collision authority. A candidate owns no geometry: support, slope, and
/// headroom are all tested by the same voxel/static-mesh projection used by
/// ordinary spatial queries. Cells prove only that a capsule can stand at
/// their center; directed edges then use the character step solver to prove a
/// wall cannot be crossed and a bounded step can be climbed. Columns and edges
/// that `previous` holds for the same policy, range and scene are kept. This
/// reads only; [`CollisionNavigationDelta::apply`] installs the result.
pub(super) fn derive_collision_navigation(
    scene: &VoxelCollisionScene,
    grid: VoxelGridSpec,
    world_y: [f64; 2],
    policy: CollisionNavigationPolicy,
    bounds: ColumnBounds,
    previous: Option<&CollisionNavigationCache>,
) -> Result<CollisionNavigationDelta, CharacterControllerError> {
    let key = DerivationKey {
        policy: policy.clone(),
        world_min_y: world_y[0],
        world_max_y: world_y[1],
    };
    let revisions = SceneRevisions::of(scene);
    let previous = previous.filter(|cache| cache.key == key && cache.scene == revisions);
    let mut to_derive = BTreeSet::new();
    let mut removed = Vec::new();
    match previous {
        Some(cache) => {
            to_derive.extend(
                bounds
                    .columns()
                    .filter(|&column| !cache.bounds.contains(column)),
            );
            removed.extend(
                cache
                    .bounds
                    .columns()
                    .filter(|&column| !bounds.contains(column)),
            );
            match &cache.dirty {
                Some(regions) => {
                    for region in regions {
                        to_derive.extend(columns_near(grid, bounds, *region));
                    }
                }
                None => to_derive.extend(bounds.columns()),
            }
        }
        None => to_derive.extend(bounds.columns()),
    }
    let to_derive: Vec<Column> = to_derive.into_iter().collect();
    let samples = in_parallel(&to_derive, |&(x, z)| {
        sample_column(scene, grid, world_y, &policy, x, z, None).map(|(supports, _)| supports)
    });
    let mut derived = BTreeMap::new();
    for (column, supports) in to_derive.into_iter().zip(samples) {
        derived.insert(column, supports?);
    }
    let supports_of = |column: Column| -> &[(VoxelCoord, f64)] {
        match derived.get(&column) {
            Some(supports) => supports,
            None if bounds.contains(column) => previous.map_or(&[], |cache| cache.supports(column)),
            None => &[],
        }
    };
    let support = |cell: VoxelCoord| {
        supports_of((cell.x, cell.z))
            .iter()
            .find(|(support, _)| *support == cell)
            .map(|&(_, height)| height)
    };
    // An edge depends on the columns it joins, and a gap jump also on the
    // columns it crosses, so a changed column changes the edges of every
    // support that far from it.
    let gap_reach = i64::from(policy.jump.gap_cells) + 1;
    let mut edge_columns = BTreeSet::new();
    for &(x, z) in derived.keys().chain(&removed) {
        edge_columns.insert((x, z));
        for (dx, dz) in planar_nav_offsets(policy.diagonal) {
            edge_columns.insert((x + dx, z + dz));
        }
        if policy.jump.gap_cells > 0 {
            for reach in 2..=gap_reach {
                for (dx, dz) in planar_nav_offsets(policy.diagonal) {
                    edge_columns.insert((x + dx * reach, z + dz * reach));
                }
            }
        }
    }
    let sources: Vec<(VoxelCoord, f64)> = edge_columns
        .iter()
        .filter(|&&column| bounds.contains(column))
        .flat_map(|&column| supports_of(column).iter().copied())
        .collect();
    let reach = i64::from(policy.reach_cells());
    let near_level = |column: Column, level: i64| {
        supports_of(column)
            .iter()
            .any(|(cell, _)| (cell.y - level).abs() <= reach)
    };
    let targets = in_parallel(&sources, |&(from, from_y)| {
        derive_edges(scene, grid, &policy, &support, &near_level, from, from_y)
    });
    let mut edges = Vec::with_capacity(sources.len());
    let mut edge_tests = 0;
    for ((from, _), derived) in sources.into_iter().zip(targets) {
        let (targets, tested) = derived?;
        edge_tests += tested;
        edges.push((from, targets));
    }
    Ok(CollisionNavigationDelta {
        key,
        scene: revisions,
        bounds,
        incremental: previous.is_some(),
        derived,
        removed,
        edge_columns,
        edges,
        edge_tests,
    })
}

/// Below this many items a derivation step runs on the calling thread.
const PARALLEL_MINIMUM: usize = 64;

/// `work` over every item, in order, spread across the machine's cores in
/// small runs taken as each thread frees up (columns of rock cost less than
/// columns of rough floor). Each item reads the published scene only, so the
/// result does not depend on the split.
fn in_parallel<T: Sync, R: Send>(items: &[T], work: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let threads = std::thread::available_parallelism().map_or(1, usize::from);
    if threads <= 1 || items.len() < PARALLEL_MINIMUM {
        return items.iter().map(work).collect();
    }
    let runs: Vec<&[T]> = items.chunks(PARALLEL_MINIMUM / 4).collect();
    let next = std::sync::atomic::AtomicUsize::new(0);
    let (work, runs, next) = (&work, &runs, &next);
    let mut done: Vec<(usize, Vec<R>)> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..threads.min(runs.len()))
            .map(|_| {
                scope.spawn(move || {
                    let mut done = Vec::new();
                    loop {
                        let index = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some(run) = runs.get(index) else {
                            return done;
                        };
                        done.push((index, run.iter().map(work).collect::<Vec<_>>()));
                    }
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|handle| {
                handle
                    .join()
                    .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
            })
            .collect()
    });
    done.sort_unstable_by_key(|(index, _)| *index);
    done.into_iter().flat_map(|(_, results)| results).collect()
}

/// The columns of `bounds` within one cell of a changed world region.
fn columns_near(
    grid: VoxelGridSpec,
    bounds: ColumnBounds,
    [min_x, min_z, max_x, max_z]: [f64; 4],
) -> impl Iterator<Item = Column> {
    let cell_size = grid.voxel_size();
    let margin = cell_size * DIRTY_MARGIN_CELLS + COLLISION_NAVIGATION_CLEARANCE_EPSILON;
    let low = grid.world_to_voxel(core_space::WorldPos::new(
        min_x - margin - cell_size,
        0.0,
        min_z - margin - cell_size,
    ));
    let high = grid.world_to_voxel(core_space::WorldPos::new(
        max_x + margin + cell_size,
        0.0,
        max_z + margin + cell_size,
    ));
    let candidates = ColumnBounds {
        min_x: low.x.max(bounds.min_x),
        max_x: high.x.min(bounds.max_x),
        min_z: low.z.max(bounds.min_z),
        max_z: high.z.min(bounds.max_z),
    };
    let empty = candidates.min_x > candidates.max_x || candidates.min_z > candidates.max_z;
    (!empty)
        .then(|| candidates.columns())
        .into_iter()
        .flatten()
        .filter(move |&(x, z)| {
            let min = grid.voxel_min_world(VoxelCoord::new(x, 0, z));
            min.x - margin <= max_x
                && min.x + cell_size + margin >= min_x
                && min.z - margin <= max_z
                && min.z + cell_size + margin >= min_z
        })
}

/// One surface a column's downward casts hit, and what became of it.
#[derive(Debug, Clone, Copy)]
pub(super) struct ColumnSample {
    pub(super) outcome: NativeCollisionNavigationSampleOutcome,
    pub(super) hit_kind: NativeSpatialHitKind,
    pub(super) surface_y: f64,
    pub(super) normal_y: f64,
    pub(super) standing_y: f64,
    pub(super) cell: VoxelCoord,
    pub(super) overlap: Option<(CharacterCollisionSource, core_space::WorldPos)>,
}

/// The supports of one X/Z column, top down, each as its cell and the height
/// a standing capsule's feet rest at; and whether sampling stopped at the
/// per-column budget. With `record`, every surface hit is recorded with what
/// became of it, and the budget flag is set only when a surface remains.
pub(super) fn sample_column(
    scene: &VoxelCollisionScene,
    grid: VoxelGridSpec,
    world_y: [f64; 2],
    policy: &CollisionNavigationPolicy,
    x: i64,
    z: i64,
    mut record: Option<&mut Vec<ColumnSample>>,
) -> Result<(Vec<(VoxelCoord, f64)>, bool), CharacterControllerError> {
    let standing = &policy.character;
    let minimum_upward_normal = f64::from(standing.surface.maximum_slope_radians).cos();
    let radius_and_skin = f64::from(standing.shape.radius + standing.shape.contact_skin);
    let center = grid.voxel_center_world(VoxelCoord::new(x, 0, z));
    let cast = |origin_y: f64| {
        let maximum_distance = origin_y - world_y[0] + COLLISION_NAVIGATION_EPSILON;
        scene
            .raycast_world(
                [center.x, origin_y, center.z],
                [0.0, -1.0, 0.0],
                maximum_distance,
            )
            .map(collision_navigation_support)
            .filter(|&(support_y, _, _)| support_y >= world_y[0] - COLLISION_NAVIGATION_EPSILON)
    };
    let mut supports: Vec<(VoxelCoord, f64)> = Vec::new();
    let mut origin_y = world_y[1] + COLLISION_NAVIGATION_EPSILON;
    let mut exhausted = true;
    for _ in 0..policy.supports_per_column {
        let Some((support_y, normal_y, hit_kind)) = cast(origin_y) else {
            exhausted = false;
            break;
        };
        // Just under a voxel surface the ray is inside solid, where it would
        // hit again at once; continue from the air below the solid run.
        let below = support_y - COLLISION_NAVIGATION_EPSILON;
        origin_y = scene
            .collidable_voxel_run_bottom([center.x, below, center.z], world_y[0])
            .map_or(below, |bottom| bottom - COLLISION_NAVIGATION_EPSILON);
        // On a floor tilted by θ the capsule's sphere, kept its skin off the
        // plane, rests (r + skin)(1 / cos θ - 1) higher than on flat ground.
        let standing_y = if normal_y > 0.0 {
            support_y + radius_and_skin * (1.0 / normal_y - 1.0)
        } else {
            support_y
        };
        let cell = grid.world_to_voxel(core_space::WorldPos::new(center.x, standing_y, center.z));
        let mut sample = ColumnSample {
            outcome: NativeCollisionNavigationSampleOutcome::Support,
            hit_kind,
            surface_y: support_y,
            normal_y,
            standing_y,
            cell,
            overlap: None,
        };
        let overlaps = |standing_y: f64| {
            scene.character_capsule_intersects(collision_navigation_capsule(
                center, standing_y, standing,
            ))
        };
        let mut overlapping = false;
        if normal_y >= minimum_upward_normal && overlaps(standing_y)? {
            // A curved or filleted floor (a reconstructed surface) can
            // rise under the capsule's rim; rest the capsule on it, as
            // the character does, within one step of the support.
            match rest_height(scene, center, standing_y, standing)? {
                Some(rest) if !overlaps(rest)? => {
                    sample.standing_y = rest;
                    sample.cell =
                        grid.world_to_voxel(core_space::WorldPos::new(center.x, rest, center.z));
                }
                _ => overlapping = true,
            }
        }
        let (standing_y, cell) = (sample.standing_y, sample.cell);
        if normal_y < minimum_upward_normal {
            sample.outcome = NativeCollisionNavigationSampleOutcome::TooSteep;
        } else if overlapping {
            sample.outcome = NativeCollisionNavigationSampleOutcome::CapsuleOverlap;
            // Only an explanation reports what the capsule overlaps.
            if record.is_some() {
                sample.overlap = scene
                    .character_capsule_overlap(collision_navigation_capsule(
                        center, standing_y, standing,
                    ))?
                    .map(|overlap| (overlap.source, overlap.point));
            }
        } else if supports.iter().any(|(existing, _)| *existing == cell) {
            // The first support found for a cell wins, as the highest one.
            sample.outcome = NativeCollisionNavigationSampleOutcome::SameCell;
        } else {
            supports.push((cell, standing_y));
        }
        if let Some(record) = record.as_deref_mut() {
            record.push(sample);
        }
    }
    if exhausted && record.is_some() {
        exhausted = cast(origin_y).is_some();
    }
    Ok((supports, exhausted))
}

/// The admitted edges from one support: walking edges to its neighbours,
/// and with jumps on, jumps up a ledge too high to step onto and straight
/// across a gap, along X or Z or with diagonal neighbours also diagonally,
/// whose columns have no support within the vertical reach of the start. Also
/// how many edges were tested.
fn derive_edges(
    scene: &VoxelCollisionScene,
    grid: VoxelGridSpec,
    policy: &CollisionNavigationPolicy,
    support: &(dyn Fn(VoxelCoord) -> Option<f64> + Sync),
    near_level: &(dyn Fn(Column, i64) -> bool + Sync),
    from: VoxelCoord,
    from_y: f64,
) -> Result<(Vec<EdgeTarget>, u64), CharacterControllerError> {
    let mut targets = Vec::new();
    let mut tested = 0;
    let jump = |to: VoxelCoord, to_y: f64, tested: &mut u64| {
        *tested += 1;
        jump_outcome(scene, grid, policy, (from, from_y), (to, to_y))
            .map(|outcome| outcome == CharacterJumpOutcome::Traversable)
    };
    for to in collision_navigation_neighbors(from, policy) {
        let Some(to_y) = support(to) else {
            continue;
        };
        tested += 1;
        // Column sampling admits a support only where the standing capsule
        // is clear, so the edge need not test either end again.
        let outcome = character_edge_outcome_between_clear_supports(
            scene,
            &policy.character,
            support_position(grid, from, from_y),
            support_position(grid, to, to_y),
            policy.maximum_drop,
        )?;
        match outcome {
            CharacterEdgeOutcome::Traversable => targets.push(EdgeTarget { to, jump: false }),
            CharacterEdgeOutcome::RiseOverStep
                if policy.jump.ledges && jump(to, to_y, &mut tested)? =>
            {
                targets.push(EdgeTarget { to, jump: true });
            }
            _ => {}
        }
    }
    let reach = i64::from(policy.reach_cells());
    for (dx, dz) in planar_nav_offsets(policy.diagonal) {
        for distance in 2..=i64::from(policy.jump.gap_cells) + 1 {
            let crossed = (from.x + dx * (distance - 1), from.z + dz * (distance - 1));
            if near_level(crossed, from.y) {
                break;
            }
            for dy in -reach..=reach {
                let to =
                    VoxelCoord::new(from.x + dx * distance, from.y + dy, from.z + dz * distance);
                let Some(to_y) = support(to) else {
                    continue;
                };
                if jump(to, to_y, &mut tested)? {
                    targets.push(EdgeTarget { to, jump: true });
                }
            }
        }
    }
    Ok((targets, tested))
}

/// A jump between two supports, from their columns' centres.
pub(super) fn jump_outcome(
    scene: &VoxelCollisionScene,
    grid: VoxelGridSpec,
    policy: &CollisionNavigationPolicy,
    from: (VoxelCoord, f64),
    to: (VoxelCoord, f64),
) -> Result<CharacterJumpOutcome, CharacterControllerError> {
    jump_plan(scene, grid, policy, from, to).map(|plan| plan.outcome)
}

/// [`jump_outcome`] with when the mover holds toward the landing.
pub(super) fn jump_plan(
    scene: &VoxelCollisionScene,
    grid: VoxelGridSpec,
    policy: &CollisionNavigationPolicy,
    (from, from_y): (VoxelCoord, f64),
    (to, to_y): (VoxelCoord, f64),
) -> Result<CharacterJumpPlan, CharacterControllerError> {
    let from_center = grid.voxel_center_world(VoxelCoord::new(from.x, 0, from.z));
    let to_center = grid.voxel_center_world(VoxelCoord::new(to.x, 0, to.z));
    character_jump_plan(
        scene,
        &policy.character,
        core_space::WorldPos::new(from_center.x, from_y, from_center.z),
        core_space::WorldPos::new(to_center.x, to_y, to_center.z),
        policy.maximum_drop,
    )
}

/// A support's world position: its column's centre at the support height.
fn support_position(grid: VoxelGridSpec, cell: VoxelCoord, height: f64) -> core_space::WorldPos {
    let center = grid.voxel_center_world(VoxelCoord::new(cell.x, 0, cell.z));
    core_space::WorldPos::new(center.x, height, center.z)
}

pub(super) fn edge_outcome(
    scene: &VoxelCollisionScene,
    grid: VoxelGridSpec,
    policy: &CollisionNavigationPolicy,
    (from, from_y): (VoxelCoord, f64),
    (to, to_y): (VoxelCoord, f64),
) -> Result<CharacterEdgeOutcome, CharacterControllerError> {
    let from_center = grid.voxel_center_world(VoxelCoord::new(from.x, 0, from.z));
    let to_center = grid.voxel_center_world(VoxelCoord::new(to.x, 0, to.z));
    character_edge_outcome(
        scene,
        &policy.character,
        core_space::WorldPos::new(from_center.x, from_y, from_center.z),
        core_space::WorldPos::new(to_center.x, to_y, to_center.z),
        policy.maximum_drop,
    )
}

/// Where a standing capsule above `standing_y` comes to rest when lowered
/// onto the surface, searching up to one step height above it; `None` when it
/// rests no higher than `standing_y` or is blocked from above.
fn rest_height(
    scene: &VoxelCollisionScene,
    center: core_space::WorldPos,
    standing_y: f64,
    config: &CharacterControllerConfig,
) -> Result<Option<f64>, CharacterControllerError> {
    let lift = f64::from(config.surface.maximum_step_height);
    if lift <= 0.0 {
        return Ok(None);
    }
    let start = collision_navigation_capsule(center, standing_y + lift, config);
    if scene.character_capsule_intersects(start)? {
        return Ok(None);
    }
    let Some(hit) = scene.cast_character_capsule(
        start,
        core_space::WorldVec::new(0.0, -lift, 0.0),
        f64::from(config.shape.contact_skin),
    )?
    else {
        return Ok(None);
    };
    let rest = standing_y + lift * (1.0 - hit.time_of_impact);
    Ok((rest > standing_y).then_some(rest))
}

fn collision_navigation_capsule(
    center: core_space::WorldPos,
    standing_y: f64,
    config: &CharacterControllerConfig,
) -> CharacterCapsule {
    let radius = f64::from(config.shape.radius);
    let half_height =
        f64::from((config.shape.standing_height * 0.5 - config.shape.radius).max(0.0));
    CharacterCapsule {
        center: core_space::WorldPos::new(
            center.x,
            standing_y + half_height + radius + f64::from(config.shape.contact_skin),
            center.z,
        ),
        half_height,
        radius,
    }
}

pub(super) fn collision_navigation_neighbors(
    coord: VoxelCoord,
    policy: &CollisionNavigationPolicy,
) -> impl Iterator<Item = VoxelCoord> {
    let reach = i64::from(policy.reach_cells());
    planar_nav_offsets(policy.diagonal)
        .iter()
        .flat_map(move |&(dx, dz)| {
            std::iter::once(0)
                .chain((1..=reach).flat_map(|step| [step, -step]))
                .map(move |dy| VoxelCoord::new(coord.x + dx, coord.y + dy, coord.z + dz))
        })
}

fn collision_navigation_support(
    hit: engine_spatial::SpatialCollisionHit,
) -> (f64, f64, NativeSpatialHitKind) {
    match hit {
        // A cube face's normal is its axis; a reconstructed surface's is
        // its triangle's.
        engine_spatial::SpatialCollisionHit::Voxel(hit) => {
            (hit.point[1], hit.normal[1], NativeSpatialHitKind::Voxel)
        }
        engine_spatial::SpatialCollisionHit::StaticMesh(hit) => {
            (hit.point.y, hit.normal.y, NativeSpatialHitKind::StaticMesh)
        }
    }
}

/// A weighted path's extra cost for a jump edge by default: a jump is
/// preferred only over a detour of more than this many cells.
const DEFAULT_JUMP_COST: u32 = 4;

/// A query point may lie this far above or below a support: a tenth of a
/// metre, plus the derivation's own rounding allowance.
const DEFAULT_SNAP: f64 = 0.1 + COLLISION_NAVIGATION_EPSILON;

pub(super) unsafe extern "C" fn default_config(
    context: *mut c_void,
    config: *mut NativeCollisionNavigationConfig,
) -> i32 {
    if context.is_null() || config.is_null() {
        return 0;
    }
    let character = CharacterControllerConfig::default();
    let value = NativeCollisionNavigationConfig {
        grid_id: 1,
        cell_size: 1.0,
        chunk_size: 16,
        maximum_cells: 65_536,
        maximum_drop: f64::from(character.surface.maximum_step_height),
        character: native_character_config(character),
        vertical_search_cells: 1,
        supports_per_column: 8,
        diagonal_neighbors: false,
        snap_above: DEFAULT_SNAP,
        snap_below: DEFAULT_SNAP,
        snap_across: 0.0,
        jump_ledges: false,
        jump_gap_cells: 0,
        jump_cost: DEFAULT_JUMP_COST,
    };
    // SAFETY: the caller owns the output for this direct call.
    unsafe { *config = value };
    ABI_OK
}

fn unpublished() -> CsharpEngineServicesError {
    CsharpEngineServicesError::new(
        "CSHARP_COLLISION_NAVIGATION_UNAVAILABLE",
        "the session has no collision-derived navigation to explain",
    )
}

pub(super) fn projection_error(error: CharacterControllerError) -> CsharpEngineServicesError {
    CsharpEngineServicesError::new("CSHARP_COLLISION_NAVIGATION_PROJECTION", error.code())
}

fn native_sample(sample: ColumnSample) -> NativeCollisionNavigationSample {
    let mut value = NativeCollisionNavigationSample {
        outcome: sample.outcome,
        hit_kind: sample.hit_kind,
        surface_y: sample.surface_y,
        normal_y: sample.normal_y,
        standing_y: sample.standing_y,
        cell: native_nav_cell(sample.cell),
        ..Default::default()
    };
    if let Some((source, point)) = sample.overlap {
        (
            value.overlap_kind,
            _,
            value.overlap_instance,
            value.overlap_asset,
            _,
            value.overlap_chunk_x,
            value.overlap_chunk_y,
            value.overlap_chunk_z,
        ) = native_character_source(source, &BTreeMap::new());
        value.overlap_point = NativeVec3 {
            x: point.x as f32,
            y: point.y as f32,
            z: point.z as f32,
        };
    }
    value
}

fn native_jump_outcome(outcome: CharacterJumpOutcome) -> NativeCollisionNavigationEdgeOutcome {
    match outcome {
        CharacterJumpOutcome::Traversable => NativeCollisionNavigationEdgeOutcome::JumpTraversable,
        CharacterJumpOutcome::RiseOverJump => NativeCollisionNavigationEdgeOutcome::RiseOverJump,
        CharacterJumpOutcome::DropOverMaximum => {
            NativeCollisionNavigationEdgeOutcome::DropOverMaximum
        }
        CharacterJumpOutcome::GapTooWide => NativeCollisionNavigationEdgeOutcome::GapTooWide,
        CharacterJumpOutcome::StartOverlap => NativeCollisionNavigationEdgeOutcome::StartOverlap,
        CharacterJumpOutcome::EndOverlap => NativeCollisionNavigationEdgeOutcome::EndOverlap,
        CharacterJumpOutcome::HeadroomBlocked => {
            NativeCollisionNavigationEdgeOutcome::JumpHeadroomBlocked
        }
        CharacterJumpOutcome::ArcBlocked => NativeCollisionNavigationEdgeOutcome::JumpArcBlocked,
    }
}

fn native_edge_outcome(outcome: CharacterEdgeOutcome) -> NativeCollisionNavigationEdgeOutcome {
    match outcome {
        CharacterEdgeOutcome::Traversable => NativeCollisionNavigationEdgeOutcome::Traversable,
        CharacterEdgeOutcome::NoHorizontalMove => NativeCollisionNavigationEdgeOutcome::NotNeighbor,
        CharacterEdgeOutcome::RiseOverStep => NativeCollisionNavigationEdgeOutcome::RiseOverStep,
        CharacterEdgeOutcome::DropOverMaximum => {
            NativeCollisionNavigationEdgeOutcome::DropOverMaximum
        }
        CharacterEdgeOutcome::StartOverlap => NativeCollisionNavigationEdgeOutcome::StartOverlap,
        CharacterEdgeOutcome::EndOverlap => NativeCollisionNavigationEdgeOutcome::EndOverlap,
        CharacterEdgeOutcome::HorizontalSweepBlocked => {
            NativeCollisionNavigationEdgeOutcome::HorizontalSweepBlocked
        }
        CharacterEdgeOutcome::DescentBlocked => {
            NativeCollisionNavigationEdgeOutcome::DescentBlocked
        }
        CharacterEdgeOutcome::StepManeuverFailed => {
            NativeCollisionNavigationEdgeOutcome::StepManeuverFailed
        }
    }
}

impl RuntimeSpatialBridge {
    fn explain_collision_navigation_column(
        &mut self,
        request: &NativeCollisionNavigationColumnRequest,
    ) -> Result<NativeCollisionNavigationColumnResult, CsharpEngineServicesError> {
        let session = self.session_mut(request.session)?;
        let key = session
            .collision_navigation
            .as_ref()
            .ok_or_else(unpublished)?
            .key();
        let mut samples = Vec::new();
        let (_, budget_exhausted) = sample_column(
            &session.scene,
            key.policy.grid()?,
            [key.world_min_y, key.world_max_y],
            &key.policy,
            request.x,
            request.z,
            Some(&mut samples),
        )
        .map_err(projection_error)?;
        let navigation_revision = session.navigation_revision;
        let samples: Box<[_]> = samples.into_iter().map(native_sample).collect();
        let result = NativeCollisionNavigationColumnResult {
            samples: samples.as_ptr(),
            samples_len: samples.len(),
            budget_exhausted,
            navigation_revision,
        };
        self.borrowed.hold(samples);
        Ok(result)
    }

    fn explain_collision_navigation_edge(
        &mut self,
        request: &NativeCollisionNavigationEdgeRequest,
    ) -> Result<NativeCollisionNavigationEdgeReadout, CsharpEngineServicesError> {
        let session = self.session_mut(request.session)?;
        let cache = session
            .collision_navigation
            .as_ref()
            .ok_or_else(unpublished)?;
        let key = cache.key();
        let grid = key.policy.grid()?;
        let world_y = [key.world_min_y, key.world_max_y];
        let support = |cell: VoxelCoord| {
            sample_column(
                &session.scene,
                grid,
                world_y,
                &key.policy,
                cell.x,
                cell.z,
                None,
            )
            .map(|(supports, _)| {
                supports
                    .into_iter()
                    .find(|(support, _)| *support == cell)
                    .map(|(_, height)| height)
            })
        };
        let (from, to) = (nav_cell(request.from), nav_cell(request.to));
        let from_y = support(from).map_err(projection_error)?;
        let to_y = support(to).map_err(projection_error)?;
        let jump = |from_y, to_y| {
            jump_outcome(
                &session.scene,
                grid,
                &key.policy,
                (from, from_y),
                (to, to_y),
            )
            .map(native_jump_outcome)
            .map_err(projection_error)
        };
        let outcome = match (from_y, to_y) {
            (None, _) => NativeCollisionNavigationEdgeOutcome::FromNotSupport,
            (_, None) => NativeCollisionNavigationEdgeOutcome::ToNotSupport,
            (Some(from_y), Some(to_y))
                if collision_navigation_neighbors(from, &key.policy).any(|cell| cell == to) =>
            {
                match edge_outcome(
                    &session.scene,
                    grid,
                    &key.policy,
                    (from, from_y),
                    (to, to_y),
                )
                .map_err(projection_error)?
                {
                    CharacterEdgeOutcome::RiseOverStep if key.policy.jump.ledges => {
                        jump(from_y, to_y)?
                    }
                    walk => native_edge_outcome(walk),
                }
            }
            (Some(from_y), Some(to_y)) => {
                // A gap jump goes straight along X or Z, or diagonally with
                // diagonal neighbours, over columns with no support within
                // reach of the start's level.
                let (dx, dz) = (to.x - from.x, to.z - from.z);
                let distance = dx.abs().max(dz.abs());
                let reach = i64::from(key.policy.reach_cells());
                let straight = ((dx == 0) != (dz == 0)
                    || (key.policy.diagonal && dx.abs() == dz.abs()))
                    && (2..=i64::from(key.policy.jump.gap_cells) + 1).contains(&distance)
                    && (to.y - from.y).abs() <= reach;
                let mut open = straight;
                for step in 1..distance {
                    if !open {
                        break;
                    }
                    let crossed = VoxelCoord::new(
                        from.x + dx.signum() * step,
                        from.y,
                        from.z + dz.signum() * step,
                    );
                    let (supports, _) = sample_column(
                        &session.scene,
                        grid,
                        world_y,
                        &key.policy,
                        crossed.x,
                        crossed.z,
                        None,
                    )
                    .map_err(projection_error)?;
                    open = !supports
                        .iter()
                        .any(|(cell, _)| (cell.y - from.y).abs() <= reach);
                }
                if open {
                    jump(from_y, to_y)?
                } else {
                    NativeCollisionNavigationEdgeOutcome::NotNeighbor
                }
            }
        };
        Ok(NativeCollisionNavigationEdgeReadout {
            outcome,
            admitted: cache.admits(from, to),
            from_y: from_y.unwrap_or_default(),
            to_y: to_y.unwrap_or_default(),
            navigation_revision: session.navigation_revision,
        })
    }
}

pub(super) unsafe extern "C" fn explain_column(
    context: *mut c_void,
    request: *const NativeCollisionNavigationColumnRequest,
    result: *mut NativeCollisionNavigationColumnResult,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(error);
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeSpatialBridge>() };
    match bridge.explain_collision_navigation_column(unsafe { &*request }) {
        Ok(value) => {
            unsafe { *result = value };
            ABI_OK
        }
        Err(refusal) => refuse(&refusal, error),
    }
}

pub(super) unsafe extern "C" fn explain_edge(
    context: *mut c_void,
    request: *const NativeCollisionNavigationEdgeRequest,
    readout: *mut NativeCollisionNavigationEdgeReadout,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(error);
    if context.is_null() || request.is_null() || readout.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeSpatialBridge>() };
    match bridge.explain_collision_navigation_edge(unsafe { &*request }) {
        Ok(value) => {
            unsafe { *readout = value };
            ABI_OK
        }
        Err(refusal) => refuse(&refusal, error),
    }
}

/// The configuration that reproduces the navigation the flat parameters
/// (cell size, step cells, agent radius and height, slope) used to describe,
/// on grid 7 with 16-cell chunks and a 4096-column budget.
#[cfg(test)]
pub(super) fn flat_config(
    cell_size: f64,
    max_step_cells: u32,
    agent_radius: f64,
    agent_height: f64,
    maximum_slope_degrees: f64,
) -> NativeCollisionNavigationConfig {
    let mut character = CharacterControllerConfig::default();
    character.shape.radius = agent_radius as f32;
    character.shape.standing_height = agent_height as f32;
    character.shape.crouched_height = ((agent_radius * 2.0 + agent_height) * 0.5) as f32;
    character.shape.contact_skin = COLLISION_NAVIGATION_CLEARANCE_EPSILON as f32;
    character.surface.maximum_step_height = (cell_size * f64::from(max_step_cells)) as f32;
    character.surface.maximum_slope_radians = maximum_slope_degrees.to_radians() as f32;
    let snap = (cell_size * 0.25).min(0.1) + COLLISION_NAVIGATION_EPSILON;
    NativeCollisionNavigationConfig {
        grid_id: 7,
        cell_size,
        chunk_size: 16,
        maximum_cells: 4096,
        maximum_drop: f64::from(character.surface.maximum_step_height),
        character: native_character_config(character),
        vertical_search_cells: max_step_cells,
        supports_per_column: 8,
        diagonal_neighbors: false,
        snap_above: snap,
        snap_below: snap,
        snap_across: 0.0,
        jump_ledges: false,
        jump_gap_cells: 0,
        jump_cost: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WIDTH: i64 = 32;

    /// Stepped terrain with a wall, around the boxes the tests publish.
    fn terrain() -> VoxelCollisionScene {
        let mut voxels = Vec::new();
        for x in -12..WIDTH + 24 {
            for z in -4..WIDTH + 4 {
                let height = 2 + ((x * 3 + z * 5).rem_euclid(11) / 3);
                let wall = (10..14).contains(&x) && (4..20).contains(&z);
                for y in 0..height + if wall { 4 } else { 0 } {
                    voxels.push([x, y, z]);
                }
            }
        }
        VoxelCollisionScene::from_solid_voxels(1.0, 16, voxels).unwrap()
    }

    fn request(
        session: NativeSpatialSessionHandle,
        min_x: f32,
    ) -> NativeCollisionNavigationReplaceRequest {
        NativeCollisionNavigationReplaceRequest {
            session,
            world_min: NativeVec3 {
                x: min_x,
                y: 0.0,
                z: 0.0,
            },
            world_max: NativeVec3 {
                x: min_x + WIDTH as f32,
                y: 12.0,
                z: WIDTH as f32,
            },
            config: flat_config(1.0, 1, 0.3, 1.6, 45.0),
        }
    }

    fn bridge_with(
        scene: Arc<VoxelCollisionScene>,
    ) -> (RuntimeSpatialBridge, NativeSpatialSessionHandle) {
        let mut bridge = RuntimeSpatialBridge::new();
        let session = bridge
            .create(NativeSpatialSessionConfig {
                collision_voxel_size: 1.0,
                collision_chunk_size: 16,
                voxel_surface_mode: NativeVoxelSurfaceMode::GreedyCubes,
            })
            .unwrap();
        bridge.sessions.get_mut(&session.value).unwrap().scene = scene;
        (bridge, session)
    }

    /// What a publication installed, beyond revision numbers.
    fn published(
        bridge: &RuntimeSpatialBridge,
        session: NativeSpatialSessionHandle,
    ) -> (u64, usize, Vec<(VoxelCoord, u64)>) {
        let navigation = bridge.sessions[&session.value].navigation.as_ref().unwrap();
        let heights = navigation
            .vertical_mapping
            .as_ref()
            .unwrap()
            .support_heights
            .iter()
            .map(|(cell, height)| (*cell, height.to_bits()))
            .collect();
        (
            navigation.projection_hash(),
            navigation.projection.walkable_len(),
            heights,
        )
    }

    /// The same publication derived from scratch on a copy of the scene.
    fn from_scratch(
        bridge: &RuntimeSpatialBridge,
        session: NativeSpatialSessionHandle,
        request: NativeCollisionNavigationReplaceRequest,
    ) -> (u64, usize, Vec<(VoxelCoord, u64)>) {
        let scene = Arc::clone(&bridge.sessions[&session.value].scene);
        let (mut fresh, fresh_session) = bridge_with(scene);
        let receipt = fresh
            .replace_collision_navigation(&NativeCollisionNavigationReplaceRequest {
                session: fresh_session,
                ..request
            })
            .unwrap();
        assert_eq!(receipt.reused_column_count, 0);
        published(&fresh, fresh_session)
    }

    #[test]
    fn a_shifted_box_derives_only_its_new_columns_and_matches_a_full_derivation() {
        let (mut bridge, session) = bridge_with(Arc::new(terrain()));
        let first = bridge
            .replace_collision_navigation(&request(session, 0.0))
            .unwrap();
        assert_eq!(
            (first.derived_column_count, first.reused_column_count),
            ((WIDTH * WIDTH) as u64, 0)
        );
        let shifted = request(session, 6.0);
        let receipt = bridge.replace_collision_navigation(&shifted).unwrap();
        assert_eq!(
            (receipt.derived_column_count, receipt.reused_column_count),
            ((6 * WIDTH) as u64, ((WIDTH - 6) * WIDTH) as u64)
        );
        assert_eq!(
            published(&bridge, session),
            from_scratch(&bridge, session, shifted)
        );

        // Another policy or vertical range derives everything again.
        let mut taller = shifted;
        taller.world_max.y = 13.0;
        let receipt = bridge.replace_collision_navigation(&taller).unwrap();
        assert_eq!(receipt.reused_column_count, 0);
    }

    #[test]
    fn a_voxel_edit_rederives_only_the_columns_near_it() {
        let (mut bridge, session) = bridge_with(Arc::new(terrain()));
        let publish = request(session, 0.0);
        bridge.replace_collision_navigation(&publish).unwrap();
        let edits = [NativeVoxelEdit {
            state: 1,
            kind: NativeVoxelEditKind::Set,
            address: NativeVoxelAddress { x: 20, y: 9, z: 20 },
            material_slot: 1,
        }];
        let voxel = crate::voxel::api(&mut bridge);
        let mut edited = NativeVoxelEditReceipt::default();
        let mut refusal = crate::operation_diagnostics::empty_receipt();
        let status = unsafe {
            (voxel.apply_edits)(
                voxel.context,
                &NativeVoxelEditTransaction {
                    session,
                    edits: edits.as_ptr(),
                    edits_len: edits.len(),
                },
                &mut edited,
                &mut refusal,
            )
        };
        assert_eq!(
            status,
            ABI_OK,
            "{:?}",
            crate::operation_diagnostics::receipt_codes(&refusal)
        );
        let receipt = bridge.replace_collision_navigation(&publish).unwrap();
        assert!(
            (1..=25).contains(&receipt.derived_column_count),
            "derived {} columns",
            receipt.derived_column_count
        );
        assert_eq!(
            published(&bridge, session),
            from_scratch(&bridge, session, publish)
        );
    }

    fn clear_voxel(
        bridge: &mut RuntimeSpatialBridge,
        session: NativeSpatialSessionHandle,
        at: [i64; 3],
    ) {
        let edits = [NativeVoxelEdit {
            state: 0,
            kind: NativeVoxelEditKind::Clear,
            address: NativeVoxelAddress {
                x: at[0],
                y: at[1],
                z: at[2],
            },
            material_slot: 0,
        }];
        let voxel = crate::voxel::api(bridge);
        let mut edited = NativeVoxelEditReceipt::default();
        let mut refusal = crate::operation_diagnostics::empty_receipt();
        let status = unsafe {
            (voxel.apply_edits)(
                voxel.context,
                &NativeVoxelEditTransaction {
                    session,
                    edits: edits.as_ptr(),
                    edits_len: edits.len(),
                },
                &mut edited,
                &mut refusal,
            )
        };
        assert_eq!(status, ABI_OK);
    }

    #[test]
    fn successive_publications_update_the_installed_navigation_in_place() {
        let (mut bridge, session) = bridge_with(Arc::new(terrain()));
        let all = (WIDTH * WIDTH) as u64;
        let at = |min_x| request(session, min_x);
        bridge.replace_collision_navigation(&at(0.0)).unwrap();
        // Nothing changed: nothing is derived.
        let receipt = bridge.replace_collision_navigation(&at(0.0)).unwrap();
        assert_eq!(
            (receipt.derived_column_count, receipt.reused_column_count),
            (0, all)
        );
        assert_eq!(
            published(&bridge, session),
            from_scratch(&bridge, session, at(0.0))
        );
        bridge.replace_collision_navigation(&at(4.0)).unwrap();
        assert_eq!(
            published(&bridge, session),
            from_scratch(&bridge, session, at(4.0))
        );
        clear_voxel(&mut bridge, session, [12, 4, 10]);
        clear_voxel(&mut bridge, session, [25, 3, 25]);
        let receipt = bridge.replace_collision_navigation(&at(4.0)).unwrap();
        assert!((1..=50).contains(&receipt.derived_column_count));
        assert_eq!(
            published(&bridge, session),
            from_scratch(&bridge, session, at(4.0))
        );
        let receipt = bridge.replace_collision_navigation(&at(0.0)).unwrap();
        assert_eq!(receipt.derived_column_count, (4 * WIDTH) as u64);
        assert_eq!(
            published(&bridge, session),
            from_scratch(&bridge, session, at(0.0))
        );
    }

    #[test]
    fn a_whole_cell_rebase_keeps_every_column() {
        let (mut bridge, session) = bridge_with(Arc::new(terrain()));
        bridge
            .replace_collision_navigation(&request(session, 0.0))
            .unwrap();
        let world_origin = crate::world_origin::api(&mut bridge);
        let mut prepared = NativeWorldOriginPreparedHandle::default();
        let prepare = NativeWorldOriginPrepareRequest {
            session,
            target_cell_x: 8,
            target_cell_y: 0,
            target_cell_z: 0,
            entities: std::ptr::null(),
            entities_len: 0,
        };
        assert_eq!(
            unsafe {
                (world_origin.prepare)(
                    world_origin.context,
                    &prepare,
                    &mut prepared,
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        let mut committed = NativeWorldOriginCommitReceipt::default();
        assert_eq!(
            unsafe {
                (world_origin.commit)(
                    world_origin.context,
                    NativeWorldOriginCommitRequest { prepared },
                    &mut committed,
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        // The same world box, now 8 units lower in local coordinates.
        let moved = request(session, -8.0);
        let receipt = bridge.replace_collision_navigation(&moved).unwrap();
        assert_eq!(
            (receipt.derived_column_count, receipt.reused_column_count),
            (0, (WIDTH * WIDTH) as u64)
        );
        assert_eq!(
            published(&bridge, session),
            from_scratch(&bridge, session, moved)
        );
    }

    #[test]
    fn a_change_the_cache_did_not_record_derives_everything() {
        let (mut bridge, session) = bridge_with(Arc::new(terrain()));
        let publish = request(session, 0.0);
        bridge.replace_collision_navigation(&publish).unwrap();
        Arc::make_mut(&mut bridge.sessions.get_mut(&session.value).unwrap().scene)
            .set_noncollidable_materials(BTreeSet::from([1]));
        let receipt = bridge.replace_collision_navigation(&publish).unwrap();
        assert_eq!(receipt.reused_column_count, 0);
        assert_eq!(
            published(&bridge, session),
            from_scratch(&bridge, session, publish)
        );
    }

    /// Solid rock with two rooms carved one above the other and a stair
    /// between them: every floor lies below solid, not below air (#9024).
    #[test]
    fn an_enclosed_volume_finds_every_floor_beneath_its_rock() {
        let air = |x: i64, y: i64, z: i64| {
            let room = (1..15).contains(&x) && (1..7).contains(&z);
            let lower = room && z >= 3 && (3..6).contains(&y);
            let upper = room && (9..12).contains(&y);
            // One cell down per column from the upper floor at x = 7 to the
            // lower floor's height at x = 13, beside the lower room.
            let stair = (8..14).contains(&x) && (1..3).contains(&z) && (16 - x..12).contains(&y);
            lower || upper || stair
        };
        let mut voxels = Vec::new();
        for x in 0..16 {
            for y in 0..20 {
                for z in 0..8 {
                    if !air(x, y, z) {
                        voxels.push([x, y, z]);
                    }
                }
            }
        }
        let (mut bridge, session) = bridge_with(Arc::new(
            VoxelCollisionScene::from_solid_voxels(1.0, 16, voxels).unwrap(),
        ));
        let mut publish = request(session, 0.0);
        publish.world_max = NativeVec3 {
            x: 16.0,
            y: 20.0,
            z: 8.0,
        };
        bridge.replace_collision_navigation(&publish).unwrap();
        let (_, _, heights) = published(&bridge, session);
        let column = |x, z| {
            heights
                .iter()
                .filter(|(cell, _)| (cell.x, cell.z) == (x, z))
                .map(|(_, height)| f64::from_bits(*height))
                .collect::<Vec<_>>()
        };
        // The rock cap and both floors; a stair column has its own step.
        assert_eq!(column(4, 4), [3.0, 9.0, 20.0]);
        assert_eq!(column(10, 1), [6.0, 20.0]);
        let navigation = bridge.sessions[&session.value].navigation.as_ref().unwrap();
        let (step, path) = evaluate_navigation_step_facts(
            navigation,
            NativeNavigationStepRequest {
                session,
                from: NativeVec3 {
                    x: 3.5,
                    y: 9.0,
                    z: 4.5,
                },
                target: NativeVec3 {
                    x: 3.5,
                    y: 3.0,
                    z: 4.5,
                },
                max_step_units: 1.0,
                max_visited: 4096,
            },
        );
        assert_eq!(step.outcome, NativeNavigationPathOutcome::Reached);
        // Down the stair, not through the rock between the rooms.
        assert!(path.iter().any(|cell| cell.z < 3), "{path:?}");
    }

    /// Publishes `config` over `[min, max]` on a fresh session of `scene`.
    fn publish_over(
        scene: VoxelCollisionScene,
        config: NativeCollisionNavigationConfig,
        min: [f32; 3],
        max: [f32; 3],
    ) -> (RuntimeSpatialBridge, NativeSpatialSessionHandle) {
        let (mut bridge, session) = bridge_with(Arc::new(scene));
        let vec = |[x, y, z]: [f32; 3]| NativeVec3 { x, y, z };
        bridge
            .replace_collision_navigation(&NativeCollisionNavigationReplaceRequest {
                session,
                world_min: vec(min),
                world_max: vec(max),
                config,
            })
            .unwrap();
        (bridge, session)
    }

    fn edge(
        bridge: &mut RuntimeSpatialBridge,
        session: NativeSpatialSessionHandle,
        from: [i64; 3],
        to: [i64; 3],
    ) -> (NativeCollisionNavigationEdgeOutcome, bool) {
        let cell = |[x, y, z]: [i64; 3]| NativePlanarNavCell { x, y, z };
        let readout = bridge
            .explain_collision_navigation_edge(&NativeCollisionNavigationEdgeRequest {
                session,
                from: cell(from),
                to: cell(to),
            })
            .unwrap();
        (readout.outcome, readout.admitted)
    }

    fn column(
        bridge: &mut RuntimeSpatialBridge,
        session: NativeSpatialSessionHandle,
        x: i64,
        z: i64,
    ) -> (Vec<NativeCollisionNavigationSample>, bool) {
        let result = bridge
            .explain_collision_navigation_column(&NativeCollisionNavigationColumnRequest {
                session,
                x,
                z,
            })
            .unwrap();
        let samples =
            unsafe { std::slice::from_raw_parts(result.samples, result.samples_len) }.to_vec();
        (samples, result.budget_exhausted)
    }

    /// A floor of 1 m voxels at height 1 rising by one block at x = 4, under a
    /// ceiling at height 4: three cells of headroom below the riser.
    fn riser() -> VoxelCollisionScene {
        let mut voxels = Vec::new();
        for x in 0..8 {
            for z in 0..4 {
                for y in 0..if x >= 4 { 2 } else { 1 } {
                    voxels.push([x, y, z]);
                }
                voxels.push([x, 4, z]);
            }
        }
        VoxelCollisionScene::from_solid_voxels(1.0, 16, voxels).unwrap()
    }

    #[test]
    fn a_riser_of_exactly_the_step_height_is_an_edge_both_ways() {
        // A 1 m step, and steps whose full lift would reach the ceiling.
        for (step_cells, radius, height) in [(1, 0.3, 1.6), (2, 0.3, 1.6), (3, 0.2, 1.5)] {
            let config = flat_config(1.0, step_cells, radius, height, 45.0);
            let (mut bridge, session) =
                publish_over(riser(), config, [0.0, 0.0, 0.0], [8.0, 4.0, 4.0]);
            let traversable = (NativeCollisionNavigationEdgeOutcome::Traversable, true);
            assert_eq!(
                edge(&mut bridge, session, [3, 1, 2], [4, 2, 2]),
                traversable
            );
            assert_eq!(
                edge(&mut bridge, session, [4, 2, 2], [3, 1, 2]),
                traversable
            );
        }
        // A deeper search finds more candidates but climbs no higher.
        let mut config = flat_config(1.0, 1, 0.3, 1.6, 45.0);
        config.vertical_search_cells = 3;
        let (mut bridge, session) = publish_over(riser(), config, [0.0, 0.0, 0.0], [8.0, 4.0, 4.0]);
        assert_eq!(
            edge(&mut bridge, session, [3, 1, 2], [4, 2, 2]),
            (NativeCollisionNavigationEdgeOutcome::Traversable, true)
        );
        // Higher than the step is refused, and says why: beyond the search it
        // is no neighbour; within it, the rise is over the step.
        let mut config = flat_config(0.5, 1, 0.2, 1.5, 45.0);
        let (mut bridge, session) = publish_over(riser(), config, [0.0, 0.0, 0.0], [8.0, 4.0, 4.0]);
        assert_eq!(
            edge(&mut bridge, session, [7, 2, 4], [8, 4, 4]),
            (NativeCollisionNavigationEdgeOutcome::NotNeighbor, false)
        );
        config.vertical_search_cells = 2;
        let (mut bridge, session) = publish_over(riser(), config, [0.0, 0.0, 0.0], [8.0, 4.0, 4.0]);
        assert_eq!(
            edge(&mut bridge, session, [7, 2, 4], [8, 4, 4]),
            (NativeCollisionNavigationEdgeOutcome::RiseOverStep, false)
        );
    }

    #[test]
    fn a_drop_is_a_one_way_edge_within_the_maximum_drop() {
        // A ledge three blocks above the floor beside it.
        let mut voxels = Vec::new();
        for x in 0..8 {
            for z in 0..4 {
                for y in 0..if x < 4 { 4 } else { 1 } {
                    voxels.push([x, y, z]);
                }
            }
        }
        let scene = || VoxelCollisionScene::from_solid_voxels(1.0, 16, voxels.clone()).unwrap();
        let mut config = flat_config(1.0, 1, 0.3, 1.6, 45.0);
        let (mut bridge, session) = publish_over(scene(), config, [0.0, 0.0, 0.0], [8.0, 8.0, 4.0]);
        assert_eq!(
            edge(&mut bridge, session, [3, 4, 2], [4, 1, 2]),
            (NativeCollisionNavigationEdgeOutcome::NotNeighbor, false)
        );
        // The drop alone does not widen the search, and the search alone
        // does not lengthen the drop.
        config.maximum_drop = 3.0;
        let (mut bridge, session) = publish_over(scene(), config, [0.0, 0.0, 0.0], [8.0, 8.0, 4.0]);
        assert_eq!(
            edge(&mut bridge, session, [3, 4, 2], [4, 1, 2]),
            (NativeCollisionNavigationEdgeOutcome::NotNeighbor, false)
        );
        config.maximum_drop = 1.0;
        config.vertical_search_cells = 3;
        let (mut bridge, session) = publish_over(scene(), config, [0.0, 0.0, 0.0], [8.0, 8.0, 4.0]);
        assert_eq!(
            edge(&mut bridge, session, [3, 4, 2], [4, 1, 2]),
            (NativeCollisionNavigationEdgeOutcome::DropOverMaximum, false)
        );
        config.maximum_drop = 3.0;
        let (mut bridge, session) = publish_over(scene(), config, [0.0, 0.0, 0.0], [8.0, 8.0, 4.0]);
        assert_eq!(
            edge(&mut bridge, session, [3, 4, 2], [4, 1, 2]),
            (NativeCollisionNavigationEdgeOutcome::Traversable, true)
        );
        assert_eq!(
            edge(&mut bridge, session, [4, 1, 2], [3, 4, 2]),
            (NativeCollisionNavigationEdgeOutcome::RiseOverStep, false)
        );
        config.maximum_drop = 2.5;
        let (mut bridge, session) = publish_over(scene(), config, [0.0, 0.0, 0.0], [8.0, 8.0, 4.0]);
        assert_eq!(
            edge(&mut bridge, session, [3, 4, 2], [4, 1, 2]),
            (NativeCollisionNavigationEdgeOutcome::DropOverMaximum, false)
        );
    }

    /// A static-mesh plane through the origin rising along +X at `degrees`.
    fn slope(degrees: f64) -> VoxelCollisionScene {
        let mut scene = VoxelCollisionScene::from_solid_voxels(1.0, 16, Vec::new()).unwrap();
        let rise = 8.0 * degrees.to_radians().tan();
        let asset = StaticMeshColliderAsset::new(
            StaticMeshAssetId(1),
            vec![
                [0.0, 0.0, 0.0],
                [0.0, 0.0, 4.0],
                [8.0, rise, 0.0],
                [8.0, rise, 4.0],
            ],
            vec![[0, 1, 2], [2, 1, 3]],
        )
        .unwrap();
        scene
            .replace_static_mesh_colliders(
                [asset],
                [StaticMeshColliderInstance {
                    id: StaticMeshInstanceId(1),
                    asset: StaticMeshAssetId(1),
                    transform: StaticMeshTransform::IDENTITY,
                }],
            )
            .unwrap();
        scene
    }

    /// A static-mesh surface along +X through `profile`'s (x, height)
    /// points, 4 units deep in Z.
    fn surface(profile: &[(f64, f64)]) -> VoxelCollisionScene {
        let mut scene = VoxelCollisionScene::from_solid_voxels(1.0, 16, Vec::new()).unwrap();
        let positions = profile
            .iter()
            .flat_map(|&(x, y)| [[x, y, 0.0], [x, y, 4.0]])
            .collect();
        let triangles = (0..profile.len() as u32 - 1)
            .flat_map(|i| {
                [
                    [2 * i, 2 * i + 1, 2 * i + 2],
                    [2 * i + 2, 2 * i + 1, 2 * i + 3],
                ]
            })
            .collect();
        let asset =
            StaticMeshColliderAsset::new(StaticMeshAssetId(1), positions, triangles).unwrap();
        scene
            .replace_static_mesh_colliders(
                [asset],
                [StaticMeshColliderInstance {
                    id: StaticMeshInstanceId(1),
                    asset: StaticMeshAssetId(1),
                    transform: StaticMeshTransform::IDENTITY,
                }],
            )
            .unwrap();
        scene
    }

    /// The body #9035 reported (r 0.3, height 1.75, 50 degree slopes) with a
    /// step of `step` metres, on 1 m cells.
    fn walker(step: f32) -> NativeCollisionNavigationConfig {
        let mut config = flat_config(1.0, 2, 0.3, 1.75, 50.0);
        let mut character = character_config(config.character).unwrap();
        character.surface.maximum_step_height = step;
        config.character = native_character_config(character);
        config.maximum_drop = f64::from(step);
        config
    }

    /// The downstream body: a 1.05 m step, a jump of 8.5 m/s under 24 m/s²
    /// (a 1.5 m peak), 7 m/s on the ground and through the air, where it
    /// gains 70 m/s each second, and a 2 m drop.
    fn jumper(ledges: bool, gap_cells: u32) -> NativeCollisionNavigationConfig {
        let mut config = walker(1.05);
        let mut character = character_config(config.character).unwrap();
        character.vertical.jump_speed = 8.5;
        character.vertical.gravity = 24.0;
        character.ground.forward_speed = 7.0;
        character.air.maximum_speed = 7.0;
        character.air.wish_speed_cap = 7.0;
        character.air.acceleration = 10.0;
        config.character = native_character_config(character);
        config.maximum_drop = 2.0;
        config.jump_ledges = ledges;
        config.jump_gap_cells = gap_cells;
        config
    }

    /// Floor 1 m high in 0.1 m voxels, 2.2 m from x = 4, optionally under a
    /// ceiling 2.5 m over the low floor.
    fn ledge(ceiling: bool) -> VoxelCollisionScene {
        let mut voxels = Vec::new();
        for x in 0..80 {
            for z in 0..40 {
                let top = if x >= 40 { 22 } else { 10 };
                voxels.extend((0..top).map(|y| [x, y, z]));
                if ceiling && x < 40 {
                    voxels.extend((35..40).map(|y| [x, y, z]));
                }
            }
        }
        VoxelCollisionScene::from_solid_voxels(0.1, 16, voxels).unwrap()
    }

    /// Floor 1 m high in 1 m voxels along x = 0..12, with `gap` columns
    /// missing from x = 3.
    fn gap(gap: i64) -> VoxelCollisionScene {
        let voxels = (0..12)
            .filter(|x| !(3..3 + gap).contains(x))
            .flat_map(|x| (0..4).map(move |z| [x, 0, z]));
        VoxelCollisionScene::from_solid_voxels(1.0, 16, voxels).unwrap()
    }

    fn path_kinds(
        bridge: &mut RuntimeSpatialBridge,
        session: NativeSpatialSessionHandle,
        start: [i64; 3],
        goal: [i64; 3],
    ) -> (NativeNavigationPathOutcome, Vec<NativeNavigationEdgeKind>) {
        let cell = |[x, y, z]: [i64; 3]| NativePlanarNavCell { x, y, z };
        let result = bridge
            .request_navigation_path(NativeNavigationPathRequest {
                session,
                start: cell(start),
                goal: cell(goal),
                max_visited: 256,
            })
            .unwrap();
        assert_eq!(result.edges_len, result.path_len);
        let kinds = if result.edges_len == 0 {
            Vec::new()
        } else {
            unsafe { std::slice::from_raw_parts(result.edges, result.edges_len) }
                .iter()
                .map(|edge| edge.kind)
                .collect()
        };
        (result.outcome, kinds)
    }

    #[test]
    fn a_jump_capped_by_the_terminal_rise_speed_does_not_reach_the_ledge() {
        let mut config = jumper(true, 0);
        let mut character = character_config(config.character).unwrap();
        character.vertical.terminal_rise_speed = 1.0;
        character.surface.maximum_step_height = 0.1;
        character.validate().unwrap();
        config.character = native_character_config(character);
        let (mut bridge, session) =
            publish_over(ledge(false), config, [0.0, -2.0, 0.0], [8.0, 12.0, 4.0]);
        let result = edge(&mut bridge, session, [3, 1, 2], [4, 2, 2]);
        assert!(!result.1, "{result:?}");
    }

    #[test]
    fn a_ledge_over_the_step_height_is_a_jump_up_and_a_drop_down() {
        let (low, high) = ([3, 1, 2], [4, 2, 2]);
        let (mut bridge, session) = publish_over(
            ledge(false),
            jumper(false, 0),
            [0.0, -2.0, 0.0],
            [8.0, 12.0, 4.0],
        );
        assert_eq!(
            edge(&mut bridge, session, low, high),
            (NativeCollisionNavigationEdgeOutcome::RiseOverStep, false),
            "with jumps off a 1.2 m ledge is no edge"
        );

        let (mut bridge, session) = publish_over(
            ledge(false),
            jumper(true, 0),
            [0.0, -2.0, 0.0],
            [8.0, 12.0, 4.0],
        );
        assert_eq!(
            edge(&mut bridge, session, low, high),
            (NativeCollisionNavigationEdgeOutcome::JumpTraversable, true)
        );
        assert_eq!(
            edge(&mut bridge, session, high, low),
            (NativeCollisionNavigationEdgeOutcome::Traversable, true)
        );
        let (outcome, up) = path_kinds(&mut bridge, session, [0, 1, 2], [7, 2, 2]);
        assert_eq!(outcome, NativeNavigationPathOutcome::Reached);
        assert_eq!(
            up.iter()
                .filter(|kind| **kind == NativeNavigationEdgeKind::Jump)
                .count(),
            1,
            "{up:?}"
        );
        let (_, down) = path_kinds(&mut bridge, session, [7, 2, 2], [0, 1, 2]);
        assert!(down.contains(&NativeNavigationEdgeKind::Drop), "{down:?}");
        assert!(!down.contains(&NativeNavigationEdgeKind::Jump), "{down:?}");

        // A ceiling 2.5 m over the low floor leaves no room to jump.
        let (mut bridge, session) = publish_over(
            ledge(true),
            jumper(true, 0),
            [0.0, -2.0, 0.0],
            [8.0, 12.0, 4.0],
        );
        assert_eq!(
            edge(&mut bridge, session, low, high),
            (
                NativeCollisionNavigationEdgeOutcome::JumpHeadroomBlocked,
                false
            )
        );
    }

    #[test]
    fn a_gap_is_jumped_only_with_gap_jumps_on_and_within_the_jump() {
        let (from, to) = ([2, 1, 2], [6, 1, 2]);
        let (mut bridge, session) =
            publish_over(gap(3), jumper(true, 0), [0.0, -2.0, 0.0], [12.0, 12.0, 4.0]);
        assert_eq!(
            edge(&mut bridge, session, from, to),
            (NativeCollisionNavigationEdgeOutcome::NotNeighbor, false)
        );
        assert_eq!(
            path_kinds(&mut bridge, session, [0, 1, 2], [9, 1, 2]).0,
            NativeNavigationPathOutcome::NoPath
        );

        let (mut bridge, session) = publish_over(
            gap(3),
            jumper(false, 3),
            [0.0, -2.0, 0.0],
            [12.0, 12.0, 4.0],
        );
        assert_eq!(
            edge(&mut bridge, session, from, to),
            (NativeCollisionNavigationEdgeOutcome::JumpTraversable, true)
        );
        let (outcome, kinds) = path_kinds(&mut bridge, session, [0, 1, 2], [9, 1, 2]);
        assert_eq!(outcome, NativeNavigationPathOutcome::Reached);
        assert_eq!(
            kinds
                .iter()
                .filter(|kind| **kind == NativeNavigationEdgeKind::Jump)
                .count(),
            1,
            "{kinds:?}"
        );
        // A step from the gap's edge jumps now, and holds toward the landing
        // 4 m on from when the jump still carries it there: 8.5 m/s under
        // 24 m/s² falls back to the floor snap's 0.25 m in (8.5 + √60.25) / 24
        // s, and gaining 70 m/s each second up to 7 m/s it covers 4 m in
        // (4 / 7 + 0.05) s.
        let step = bridge
            .evaluate_navigation(NativeNavigationStepRequest {
                session,
                from: NativeVec3 {
                    x: 2.5,
                    y: 1.0,
                    z: 2.5,
                },
                target: NativeVec3 {
                    x: 9.5,
                    y: 1.0,
                    z: 2.5,
                },
                max_step_units: 1.0,
                max_visited: 256,
            })
            .unwrap();
        assert_eq!(step.next_edge_kind, NativeNavigationEdgeKind::Jump);
        let departure = (8.5 + 60.25f64.sqrt()) / 24.0 - (4.0 / 7.0 + 0.05);
        assert!(
            (f64::from(step.next_jump_departure) - departure).abs() < 1e-4,
            "{} s, not {departure} s",
            step.next_jump_departure
        );

        // Six metres is beyond the jump's 4.6 m reach from a standstill.
        let (mut bridge, session) = publish_over(
            gap(5),
            jumper(false, 6),
            [0.0, -2.0, 0.0],
            [12.0, 12.0, 4.0],
        );
        assert_eq!(
            edge(&mut bridge, session, [2, 1, 2], [8, 1, 2]),
            (NativeCollisionNavigationEdgeOutcome::GapTooWide, false)
        );
    }

    /// Two 3 × 3 floors 1 m high in 1 m voxels, corner to corner across
    /// `gap` empty columns on the diagonal.
    fn diagonal_gap(gap: i64) -> VoxelCollisionScene {
        let far = 3 + gap;
        let voxels = (0..far + 3)
            .flat_map(|x| (0..far + 3).map(move |z| [x, 0, z]))
            .filter(|&[x, _, z]| (x < 3 && z < 3) || (x >= far && z >= far));
        VoxelCollisionScene::from_solid_voxels(1.0, 16, voxels).unwrap()
    }

    #[test]
    fn a_diagonal_gap_is_jumped_only_with_diagonal_neighbours() {
        let (from, to) = ([2, 1, 2], [5, 1, 5]);
        let (mut bridge, session) = publish_over(
            diagonal_gap(2),
            jumper(false, 2),
            [0.0, -2.0, 0.0],
            [8.0, 12.0, 8.0],
        );
        assert_eq!(
            edge(&mut bridge, session, from, to),
            (NativeCollisionNavigationEdgeOutcome::NotNeighbor, false)
        );
        assert_eq!(
            path_kinds(&mut bridge, session, [0, 1, 0], [7, 1, 7]).0,
            NativeNavigationPathOutcome::NoPath
        );

        let mut config = jumper(false, 2);
        config.diagonal_neighbors = true;
        let (mut bridge, session) = bridge_with(Arc::new(diagonal_gap(2)));
        let publish = NativeCollisionNavigationReplaceRequest {
            session,
            world_min: NativeVec3 {
                x: 0.0,
                y: -2.0,
                z: 0.0,
            },
            world_max: NativeVec3 {
                x: 8.0,
                y: 12.0,
                z: 8.0,
            },
            config,
        };
        bridge.replace_collision_navigation(&publish).unwrap();
        assert_eq!(
            edge(&mut bridge, session, from, to),
            (NativeCollisionNavigationEdgeOutcome::JumpTraversable, true)
        );
        let (outcome, kinds) = path_kinds(&mut bridge, session, [0, 1, 0], [7, 1, 7]);
        assert_eq!(outcome, NativeNavigationPathOutcome::Reached);
        assert!(kinds.contains(&NativeNavigationEdgeKind::Jump), "{kinds:?}");

        // A block raising the landing column re-derives the jump from the
        // corner three columns away, as a fresh derivation does.
        let edits = [NativeVoxelEdit {
            state: 1,
            kind: NativeVoxelEditKind::Set,
            address: NativeVoxelAddress { x: 5, y: 1, z: 5 },
            material_slot: 1,
        }];
        let voxel = crate::voxel::api(&mut bridge);
        let mut edited = NativeVoxelEditReceipt::default();
        let mut refusal = crate::operation_diagnostics::empty_receipt();
        let status = unsafe {
            (voxel.apply_edits)(
                voxel.context,
                &NativeVoxelEditTransaction {
                    session,
                    edits: edits.as_ptr(),
                    edits_len: edits.len(),
                },
                &mut edited,
                &mut refusal,
            )
        };
        assert_eq!(status, ABI_OK);
        let receipt = bridge.replace_collision_navigation(&publish).unwrap();
        assert!(
            receipt.reused_column_count > 0,
            "the republish was incremental"
        );
        assert_eq!(
            edge(&mut bridge, session, from, [5, 2, 5]),
            (NativeCollisionNavigationEdgeOutcome::GapTooWide, false),
            "a metre up, the jump lands before it carries across"
        );
        assert_eq!(
            published(&bridge, session),
            from_scratch(&bridge, session, publish)
        );
    }

    /// A block hung over the middle of a gap blocks the jumps across it; the
    /// republish re-derives the edges of supports up to the gap away and
    /// matches a fresh derivation.
    #[test]
    fn an_edit_over_a_gap_rederives_the_jumps_across_it() {
        let (mut bridge, session) = bridge_with(Arc::new(gap(3)));
        let publish = NativeCollisionNavigationReplaceRequest {
            session,
            world_min: NativeVec3 {
                x: 0.0,
                y: -2.0,
                z: 0.0,
            },
            world_max: NativeVec3 {
                x: 12.0,
                y: 12.0,
                z: 4.0,
            },
            config: jumper(false, 3),
        };
        bridge.replace_collision_navigation(&publish).unwrap();
        assert!(edge(&mut bridge, session, [2, 1, 2], [6, 1, 2]).1);
        let edits = [NativeVoxelEdit {
            state: 1,
            kind: NativeVoxelEditKind::Set,
            address: NativeVoxelAddress { x: 4, y: 3, z: 2 },
            material_slot: 1,
        }];
        let voxel = crate::voxel::api(&mut bridge);
        let mut edited = NativeVoxelEditReceipt::default();
        let mut refusal = crate::operation_diagnostics::empty_receipt();
        let status = unsafe {
            (voxel.apply_edits)(
                voxel.context,
                &NativeVoxelEditTransaction {
                    session,
                    edits: edits.as_ptr(),
                    edits_len: edits.len(),
                },
                &mut edited,
                &mut refusal,
            )
        };
        assert_eq!(status, ABI_OK);
        let receipt = bridge.replace_collision_navigation(&publish).unwrap();
        assert!(
            receipt.reused_column_count > 0,
            "the republish was incremental"
        );
        assert_eq!(
            edge(&mut bridge, session, [2, 1, 2], [6, 1, 2]),
            (NativeCollisionNavigationEdgeOutcome::JumpArcBlocked, false)
        );
        assert_eq!(
            published(&bridge, session),
            from_scratch(&bridge, session, publish)
        );
    }

    /// Routes from the first to the last column of row z = 2 and back.
    fn route_both_ways(
        scene: VoxelCollisionScene,
        config: NativeCollisionNavigationConfig,
    ) -> [NativeNavigationPathOutcome; 2] {
        let (mut bridge, session) = publish_over(scene, config, [0.0, -2.0, 0.0], [8.0, 12.0, 4.0]);
        let feet = |bridge: &mut RuntimeSpatialBridge, x: i64| {
            let (samples, _) = column(bridge, session, x, 2);
            NativeVec3 {
                x: x as f32 + 0.5,
                y: samples[0].standing_y as f32,
                z: 2.5,
            }
        };
        let (low, high) = (feet(&mut bridge, 0), feet(&mut bridge, 7));
        let navigation = bridge.sessions[&session.value].navigation.as_ref().unwrap();
        [(low, high), (high, low)].map(|(from, target)| {
            evaluate_navigation_step_facts(
                navigation,
                NativeNavigationStepRequest {
                    session,
                    from,
                    target,
                    max_step_units: 1.0,
                    max_visited: 4096,
                },
            )
            .0
            .outcome
        })
    }

    #[test]
    fn a_route_climbs_and_descends_a_slope_within_the_maximum() {
        // #9035: past about 35 degrees every upward edge failed the step
        // manoeuvre. A slope is walked, so it needs no step height either.
        let reached = [NativeNavigationPathOutcome::Reached; 2];
        for degrees in [30.0, 40.0, 45.0, 49.0] {
            for step in [1.05, 0.4] {
                assert_eq!(
                    route_both_ways(slope(degrees), walker(step)),
                    reached,
                    "{degrees} degrees, step {step}"
                );
            }
        }
        // A smoothed stair: treads and 45 degree noses averaging about 31
        // degrees, so each 1 m cell rises more than a 0.4 m step.
        let profile: Vec<_> = (0..=16)
            .map(|i| {
                let x = f64::from(i) * 0.5;
                (
                    x,
                    f64::from(i / 2) * 0.6 + if i % 2 == 1 { 0.1 } else { 0.0 },
                )
            })
            .collect();
        assert_eq!(route_both_ways(surface(&profile), walker(0.4)), reached);
    }

    #[test]
    fn a_route_crosses_a_crater_dug_in_a_dual_contoured_floor() {
        use engine_spatial::{
            MaterialSurface, SurfaceCharacter, SurfaceMaterials, SurfaceMeshOptions, SurfaceMode,
            VertexPlacement, VoxelDensityEdit, VoxelDensityEditService, VoxelDensityOperation,
            VoxelDensityShape,
        };
        // A floor three voxels deep, wider than the published box so its
        // rounded edges stay outside it.
        let mut voxels = Vec::new();
        for x in -4..12 {
            for z in -4..8 {
                for y in 0..3 {
                    voxels.push([x, y, z]);
                }
            }
        }
        let options = SurfaceMeshOptions {
            mode: SurfaceMode::DualContouring,
            materials: SurfaceMaterials::new([(
                1,
                MaterialSurface {
                    mode: SurfaceMode::DualContouring,
                    character: SurfaceCharacter {
                        placement: VertexPlacement::Smooth,
                        crease_angle_degrees: 180.0,
                        roughness: 0.0,
                    },
                },
            )])
            .unwrap(),
            ..SurfaceMeshOptions::default()
        };
        let mut scene =
            VoxelCollisionScene::from_solid_voxels_with_mesh_options(1.0, 16, voxels, options)
                .unwrap();
        // A crater 0.6 m deep whose rim slopes about 40 degrees.
        VoxelDensityEditService::apply(
            &mut scene,
            &[VoxelDensityEdit::Brush {
                shape: VoxelDensityShape::Sphere {
                    center: [4.0, 5.0, 2.5],
                    radius: 2.6,
                },
                operation: VoxelDensityOperation::Subtract,
                material_slot: 1,
            }],
        )
        .unwrap();
        let floor = scene
            .raycast([4.0, 8.0, 2.5], [0.0, -1.0, 0.0], 10.0)
            .unwrap();
        assert!(floor.point[1] < 2.6, "crater floor {:?}", floor.point);
        assert_eq!(
            route_both_ways(scene, walker(0.4)),
            [NativeNavigationPathOutcome::Reached; 2]
        );
    }

    #[test]
    fn a_route_crosses_from_dual_contoured_stone_onto_cube_brick_and_back() {
        use engine_spatial::{
            MaterialSurface, MaterialVoxel, SurfaceCharacter, SurfaceMaterials, SurfaceMeshOptions,
            SurfaceMode,
        };
        const STONE: u16 = 1;
        const BRICK: u16 = 2;
        // Stone ground three voxels deep; a brick platform one voxel higher
        // from x = 4 to 5, then stone at the platform's height.
        let mut voxels = Vec::new();
        for x in -4..12_i64 {
            for z in -4..8_i64 {
                let top = if x < 4 { 3 } else { 4 };
                for y in 0..top {
                    let brick = (4..6).contains(&x) && y == 3;
                    voxels.push(MaterialVoxel {
                        state: 0,
                        address: [x, y, z],
                        material_slot: if brick { BRICK } else { STONE },
                    });
                }
            }
        }
        let options = SurfaceMeshOptions {
            mode: SurfaceMode::DualContouring,
            materials: SurfaceMaterials::new([(
                BRICK,
                MaterialSurface {
                    mode: SurfaceMode::GreedyCubes,
                    character: SurfaceCharacter::default(),
                },
            )])
            .unwrap(),
            ..SurfaceMeshOptions::default()
        };
        let scene =
            VoxelCollisionScene::from_material_voxels_with_mesh_options(1.0, 16, voxels, options)
                .unwrap();
        // The brick top is an exact cube face.
        let top = scene
            .raycast([4.5, 8.0, 2.5], [0.0, -1.0, 0.0], 10.0)
            .unwrap();
        assert_eq!(top.point[1], 4.0);
        assert_eq!(top.voxel, [4, 3, 2]);
        assert_eq!(
            route_both_ways(scene, walker(1.05)),
            [NativeNavigationPathOutcome::Reached; 2]
        );
    }

    #[test]
    fn a_route_climbs_and_descends_a_dual_contoured_voxel_stair() {
        use engine_spatial::{SurfaceMeshOptions, SurfaceMode};
        // One-voxel treads and risers along +X. Dual contouring fills each
        // riser's foot with a fillet that the standing capsule rests on.
        let mut voxels = Vec::new();
        for x in -4..12 {
            for z in -4..8 {
                let height = (x.clamp(0, 7) / 2) + 1;
                for y in 0..height {
                    voxels.push([x, y, z]);
                }
            }
        }
        let scene = VoxelCollisionScene::from_solid_voxels_with_mesh_options(
            1.0,
            16,
            voxels,
            SurfaceMeshOptions::with_mode(SurfaceMode::DualContouring),
        )
        .unwrap();
        assert_eq!(
            route_both_ways(scene, walker(1.05)),
            [NativeNavigationPathOutcome::Reached; 2]
        );
    }

    #[test]
    fn a_riser_or_cliff_within_the_slope_grade_is_still_no_slope() {
        // A 1 m riser and a 1 m cliff rise and fall no steeper than 50
        // degrees between 1 m cells, but neither is a slope.
        let mut voxels = Vec::new();
        for x in 0..8 {
            for z in 0..4 {
                for y in 0..if x >= 4 { 2 } else { 1 } {
                    voxels.push([x, y, z]);
                }
            }
        }
        let (mut bridge, session) = publish_over(
            VoxelCollisionScene::from_solid_voxels(1.0, 16, voxels).unwrap(),
            walker(0.4),
            [0.0, 0.0, 0.0],
            [8.0, 6.0, 4.0],
        );
        assert_eq!(
            edge(&mut bridge, session, [3, 1, 2], [4, 2, 2]),
            (NativeCollisionNavigationEdgeOutcome::RiseOverStep, false)
        );
        assert_eq!(
            edge(&mut bridge, session, [4, 2, 2], [3, 1, 2]),
            (NativeCollisionNavigationEdgeOutcome::DropOverMaximum, false)
        );
    }

    #[test]
    fn a_slope_up_to_the_maximum_is_a_support_with_its_feet_lifted() {
        // #9032 finding A: r = 0.3 and skin 0.02 refused anything steeper than
        // about 20 degrees although the maximum slope is 50.
        let config = flat_config(1.0, 1, 0.3, 1.6, 50.0);
        for degrees in [25.0_f64, 45.0, 10.0] {
            let (mut bridge, session) =
                publish_over(slope(degrees), config, [0.0, -1.0, 0.0], [8.0, 9.0, 4.0]);
            let (samples, exhausted) = column(&mut bridge, session, 3, 2);
            assert!(!exhausted);
            let [sample] = samples[..] else {
                panic!("{samples:?}")
            };
            assert_eq!(
                sample.outcome,
                NativeCollisionNavigationSampleOutcome::Support,
                "{degrees} degrees"
            );
            assert_eq!(sample.hit_kind, NativeSpatialHitKind::StaticMesh);
            let lift = 0.32 * (1.0 / degrees.to_radians().cos() - 1.0);
            assert!((sample.standing_y - sample.surface_y - lift).abs() < 1.0e-4);
            // Every column of the plane is a support, and they connect.
            let receipt =
                bridge.replace_collision_navigation(&NativeCollisionNavigationReplaceRequest {
                    session,
                    world_min: NativeVec3 {
                        x: 0.0,
                        y: -1.0,
                        z: 0.0,
                    },
                    world_max: NativeVec3 {
                        x: 8.0,
                        y: 9.0,
                        z: 4.0,
                    },
                    config,
                });
            assert_eq!(
                receipt.unwrap().walkable_cell_count,
                32,
                "{degrees} degrees"
            );
        }
        let (mut bridge, session) =
            publish_over(slope(60.0), config, [0.0, -1.0, 0.0], [8.0, 15.0, 4.0]);
        let (samples, _) = column(&mut bridge, session, 3, 2);
        assert_eq!(
            samples[0].outcome,
            NativeCollisionNavigationSampleOutcome::TooSteep
        );
    }

    #[test]
    fn a_column_explains_overlap_and_the_layer_budget() {
        // A floor at 1, a slab at 2 leaving no headroom, and floors below.
        let mut voxels = Vec::new();
        for x in 0..4 {
            for z in 0..4 {
                for y in [0, 2, 5, 8, 11] {
                    voxels.push([x, y, z]);
                }
            }
        }
        let mut config = flat_config(1.0, 1, 0.3, 1.6, 45.0);
        config.supports_per_column = 2;
        let (mut bridge, session) = publish_over(
            VoxelCollisionScene::from_solid_voxels(1.0, 16, voxels).unwrap(),
            config,
            [0.0, 0.0, 0.0],
            [4.0, 12.0, 4.0],
        );
        let (samples, exhausted) = column(&mut bridge, session, 1, 1);
        assert!(exhausted);
        let outcomes: Vec<_> = samples.iter().map(|sample| sample.outcome).collect();
        assert_eq!(
            outcomes,
            [
                NativeCollisionNavigationSampleOutcome::Support,
                NativeCollisionNavigationSampleOutcome::Support
            ]
        );
        config.supports_per_column = 8;
        let (mut bridge, session) = {
            let scene = Arc::clone(&bridge.sessions[&session.value].scene);
            let (mut bridge, session) = bridge_with(scene);
            let mut request = request(session, 0.0);
            request.world_max = NativeVec3 {
                x: 4.0,
                y: 12.0,
                z: 4.0,
            };
            request.config = config;
            bridge.replace_collision_navigation(&request).unwrap();
            (bridge, session)
        };
        let (samples, exhausted) = column(&mut bridge, session, 1, 1);
        assert!(!exhausted);
        let last = samples.last().unwrap();
        assert_eq!(last.surface_y, 1.0);
        assert_eq!(
            last.outcome,
            NativeCollisionNavigationSampleOutcome::CapsuleOverlap
        );
        assert!(matches!(
            last.overlap_kind,
            NativeCharacterCollisionSourceKind::VoxelChunk
        ));
        // Against the slab above, at the capsule's top.
        assert!((last.overlap_point.y - 2.0).abs() < 0.05, "{last:?}");
    }

    #[test]
    fn diagonal_neighbours_connect_corners_but_not_through_a_wall() {
        let mut voxels = Vec::new();
        for x in 0..6 {
            for z in 0..6 {
                voxels.push([x, 0, z]);
                // A wall corner at (3, 3): cutting it is not an edge.
                if (x, z) == (3, 3) {
                    voxels.extend([[x, 1, z], [x, 2, z]]);
                }
            }
        }
        let scene = || VoxelCollisionScene::from_solid_voxels(1.0, 16, voxels.clone()).unwrap();
        let mut config = flat_config(1.0, 1, 0.3, 1.6, 45.0);
        let (mut bridge, session) = publish_over(scene(), config, [0.0, 0.0, 0.0], [6.0, 4.0, 6.0]);
        assert_eq!(
            edge(&mut bridge, session, [1, 1, 1], [2, 1, 2]),
            (NativeCollisionNavigationEdgeOutcome::NotNeighbor, false)
        );
        config.diagonal_neighbors = true;
        let (mut bridge, session) = publish_over(scene(), config, [0.0, 0.0, 0.0], [6.0, 4.0, 6.0]);
        assert_eq!(
            edge(&mut bridge, session, [1, 1, 1], [2, 1, 2]),
            (NativeCollisionNavigationEdgeOutcome::Traversable, true)
        );
        assert!(
            !edge(&mut bridge, session, [2, 1, 3], [3, 1, 4]).1,
            "the capsule sweeps through the wall corner"
        );
        // A diagonal route is shorter, and republishing after an edit beside
        // a diagonal edge matches a full derivation.
        let navigation = bridge.sessions[&session.value].navigation.as_ref().unwrap();
        let (step, path) = evaluate_navigation_step_facts(
            navigation,
            NativeNavigationStepRequest {
                session,
                from: NativeVec3 {
                    x: 0.5,
                    y: 1.0,
                    z: 0.5,
                },
                target: NativeVec3 {
                    x: 2.5,
                    y: 1.0,
                    z: 2.5,
                },
                max_step_units: 1.0,
                max_visited: 4096,
            },
        );
        assert_eq!(step.outcome, NativeNavigationPathOutcome::Reached);
        assert_eq!(path.len(), 3);
        let publish = NativeCollisionNavigationReplaceRequest {
            session,
            world_min: NativeVec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            world_max: NativeVec3 {
                x: 6.0,
                y: 4.0,
                z: 6.0,
            },
            config,
        };
        clear_voxel(&mut bridge, session, [3, 2, 3]);
        clear_voxel(&mut bridge, session, [3, 1, 3]);
        bridge.replace_collision_navigation(&publish).unwrap();
        assert_eq!(
            published(&bridge, session),
            from_scratch(&bridge, session, publish)
        );
        assert_eq!(
            edge(&mut bridge, session, [2, 1, 3], [3, 1, 4]),
            (NativeCollisionNavigationEdgeOutcome::Traversable, true)
        );
    }

    #[test]
    fn a_query_snaps_to_a_support_within_the_configured_distances() {
        // A floor at 1, and one at 1.5 from x = 4.
        let mut voxels = Vec::new();
        for x in 0..8 {
            for z in 0..2 {
                voxels.push([x, 0, z]);
            }
        }
        let mut config = flat_config(1.0, 1, 0.3, 1.6, 45.0);
        let at = |x: f32, y: f32| NativeVec3 { x, y, z: 0.5 };
        let step = |bridge: &RuntimeSpatialBridge, from: NativeVec3, to: NativeVec3| {
            let navigation = bridge.sessions[&session_of(bridge)]
                .navigation
                .as_ref()
                .unwrap();
            evaluate_navigation_step_facts(
                navigation,
                NativeNavigationStepRequest {
                    session: NativeSpatialSessionHandle {
                        value: session_of(bridge),
                    },
                    from,
                    target: to,
                    max_step_units: 1.0,
                    max_visited: 4096,
                },
            )
            .0
            .outcome
        };
        let scene = || VoxelCollisionScene::from_solid_voxels(1.0, 16, voxels.clone()).unwrap();
        let (bridge, _) = publish_over(scene(), config, [0.0, 0.0, 0.0], [8.0, 4.0, 2.0]);
        // Within the default tenth of a metre, but not a quarter above.
        assert_eq!(
            step(&bridge, at(0.5, 1.05), at(7.5, 1.0)),
            NativeNavigationPathOutcome::Reached
        );
        assert_eq!(
            step(&bridge, at(0.5, 1.25), at(7.5, 1.0)),
            NativeNavigationPathOutcome::StartNotWalkable
        );
        assert_eq!(
            step(&bridge, at(0.5, 1.0), at(7.5, 0.8)),
            NativeNavigationPathOutcome::GoalNotWalkable
        );
        // Off the floor's edge, across, it lands on no cell.
        assert_eq!(
            step(&bridge, at(0.5, 1.0), at(8.2, 1.0)),
            NativeNavigationPathOutcome::GoalNotWalkable
        );
        config.snap_above = 0.3;
        config.snap_below = 0.3;
        config.snap_across = 0.25;
        let (bridge, _) = publish_over(scene(), config, [0.0, 0.0, 0.0], [8.0, 4.0, 2.0]);
        assert_eq!(
            step(&bridge, at(0.5, 1.25), at(7.5, 0.8)),
            NativeNavigationPathOutcome::Reached
        );
        assert_eq!(
            step(&bridge, at(0.5, 1.0), at(8.2, 1.0)),
            NativeNavigationPathOutcome::Reached
        );
    }

    fn session_of(bridge: &RuntimeSpatialBridge) -> u64 {
        *bridge.sessions.keys().next().unwrap()
    }

    #[test]
    fn no_path_reports_how_far_the_search_got() {
        // Two floors separated by a wall the agent cannot step over.
        let mut voxels = Vec::new();
        for x in 0..8 {
            for z in 0..3 {
                voxels.push([x, 0, z]);
                if x == 4 {
                    voxels.extend([[x, 1, z], [x, 2, z], [x, 3, z]]);
                }
            }
        }
        let (bridge, session) = publish_over(
            VoxelCollisionScene::from_solid_voxels(1.0, 16, voxels).unwrap(),
            flat_config(1.0, 1, 0.3, 1.6, 45.0),
            [0.0, 0.0, 0.0],
            [8.0, 3.0, 3.0],
        );
        let navigation = bridge.sessions[&session.value].navigation.as_ref().unwrap();
        let (step, _) = evaluate_navigation_step_facts(
            navigation,
            NativeNavigationStepRequest {
                session,
                from: NativeVec3 {
                    x: 0.5,
                    y: 1.0,
                    z: 1.5,
                },
                target: NativeVec3 {
                    x: 7.5,
                    y: 1.0,
                    z: 1.5,
                },
                max_step_units: 1.0,
                max_visited: 4096,
            },
        );
        assert_eq!(step.outcome, NativeNavigationPathOutcome::NoPath);
        assert_eq!(step.visited, 12);
        assert!(step.nearest_present);
        assert_eq!(
            (
                step.nearest_cell.x,
                step.nearest_cell.y,
                step.nearest_cell.z
            ),
            (3, 1, 1)
        );
        assert_eq!(
            (step.nearest.x, step.nearest.y, step.nearest.z),
            (3.5, 1.0, 1.5)
        );
    }

    /// A density edit on a reconstructed surface re-derives only the columns
    /// near it, and the result is what a fresh derivation gives.
    #[test]
    fn a_density_edit_on_a_reconstructed_floor_rederives_only_nearby_columns() {
        use engine_spatial::{
            MaterialSurface, SurfaceCharacter, SurfaceMaterials, SurfaceMeshOptions, SurfaceMode,
            VertexPlacement, VoxelDensityEdit, VoxelDensityEditService, VoxelDensityOperation,
            VoxelDensityShape,
        };
        let mut voxels = Vec::new();
        for x in -8..WIDTH + 8 {
            for z in -8..WIDTH + 8 {
                for y in 0..3 {
                    voxels.push([x, y, z]);
                }
            }
        }
        let options = SurfaceMeshOptions {
            mode: SurfaceMode::DualContouring,
            materials: SurfaceMaterials::new([(
                1,
                MaterialSurface {
                    mode: SurfaceMode::DualContouring,
                    character: SurfaceCharacter {
                        placement: VertexPlacement::Sharp,
                        crease_angle_degrees: 40.0,
                        roughness: 0.1,
                    },
                },
            )])
            .unwrap(),
            ..SurfaceMeshOptions::default()
        };
        let (mut bridge, session) = bridge_with(Arc::new(
            VoxelCollisionScene::from_solid_voxels_with_mesh_options(1.0, 16, voxels, options)
                .unwrap(),
        ));
        let publish = request(session, 0.0);
        let first = bridge.replace_collision_navigation(&publish).unwrap();
        assert_eq!(first.derived_column_count, (WIDTH * WIDTH) as u64);
        let scene = &mut bridge.sessions.get_mut(&session.value).unwrap();
        scene.change_collision(|scene| {
            let receipt = VoxelDensityEditService::apply(
                scene,
                &[VoxelDensityEdit::Brush {
                    shape: VoxelDensityShape::Sphere {
                        center: [16.0, 3.0, 16.0],
                        radius: 1.5,
                    },
                    operation: VoxelDensityOperation::Subtract,
                    material_slot: 1,
                }],
            )
            .unwrap();
            let changed = crate::voxel::collision_reach(
                scene,
                crate::voxel::voxel_box(scene, receipt.changed_min, receipt.changed_max_inclusive),
            );
            ((), vec![changed])
        });
        let edited = bridge.replace_collision_navigation(&publish).unwrap();
        assert!(
            edited.derived_column_count < (WIDTH * WIDTH / 4) as u64,
            "an edit near one column re-derived {} of {} columns",
            edited.derived_column_count,
            WIDTH * WIDTH
        );
        assert_eq!(
            published(&bridge, session),
            from_scratch(&bridge, session, publish)
        );
    }

    /// The downstream shape (#8999): a 64 x 64 x 32 box over relief of about
    /// ±16 cells, then a 12 m shift and an edit inside one chunk. Run with
    /// `cargo test --release -p csharp-engine-services --lib measure_ -- --ignored --nocapture`.
    #[test]
    #[ignore = "timing measurement"]
    fn measure_incremental_collision_navigation() {
        let mut voxels = Vec::new();
        for x in -8..96 {
            for z in -8..72 {
                let wave = ((x as f64 * 0.21).sin() + (z as f64 * 0.17).cos()) * 7.5;
                for y in 0..(16.0 + wave).round() as i64 {
                    voxels.push([x, y, z]);
                }
            }
        }
        let (mut bridge, session) = bridge_with(Arc::new(
            VoxelCollisionScene::from_solid_voxels(1.0, 16, voxels).unwrap(),
        ));
        let publish = |bridge: &mut RuntimeSpatialBridge, min_x: f32| {
            let mut request = request(session, min_x);
            request.world_max.x = min_x + 64.0;
            request.world_max.z = 64.0;
            request.world_max.y = 32.0;
            let started = Instant::now();
            let receipt = bridge.replace_collision_navigation(&request).unwrap();
            (started.elapsed(), receipt)
        };
        let (full, receipt) = publish(&mut bridge, 0.0);
        println!(
            "full: {full:?} derived {} cells {}",
            receipt.derived_column_count, receipt.walkable_cell_count
        );
        let (shift, receipt) = publish(&mut bridge, 12.0);
        println!(
            "12 m shift: {shift:?} derived {} reused {}",
            receipt.derived_column_count, receipt.reused_column_count
        );
        let (same, receipt) = publish(&mut bridge, 12.0);
        println!(
            "nothing changed: {same:?} derived {} reused {}",
            receipt.derived_column_count, receipt.reused_column_count
        );
        let edits: Vec<_> = (0..4)
            .flat_map(|dx| (0..4).map(move |dz| (dx, dz)))
            .map(|(dx, dz)| NativeVoxelEdit {
                state: 0,
                kind: NativeVoxelEditKind::Clear,
                address: NativeVoxelAddress {
                    x: 36 + dx,
                    y: 15,
                    z: 36 + dz,
                },
                material_slot: 0,
            })
            .collect();
        let voxel = crate::voxel::api(&mut bridge);
        let mut edited = NativeVoxelEditReceipt::default();
        let mut refusal = crate::operation_diagnostics::empty_receipt();
        let status = unsafe {
            (voxel.apply_edits)(
                voxel.context,
                &NativeVoxelEditTransaction {
                    session,
                    edits: edits.as_ptr(),
                    edits_len: edits.len(),
                },
                &mut edited,
                &mut refusal,
            )
        };
        assert_eq!(status, ABI_OK);
        let (edit, receipt) = publish(&mut bridge, 12.0);
        println!(
            "one-chunk edit: {edit:?} derived {} reused {}",
            receipt.derived_column_count, receipt.reused_column_count
        );
    }
}

/// Publication cost over a carved dungeon, as cubes and as reconstructed
/// surfaces. Run with
/// `cargo test --release -p csharp-engine-services --lib measure_dungeon -- --ignored --nocapture`.
#[cfg(test)]
mod cost {
    use super::*;
    use engine_spatial::{
        MaterialSurface, SurfaceCharacter, SurfaceMaterials, SurfaceMeshOptions, SurfaceMode,
        VertexPlacement,
    };

    const SIZE: i64 = 96;
    const HEIGHT: i64 = 40;

    /// Solid rock with a 4 × 4 grid of rooms at different floor heights,
    /// joined by corridors, in `scale` voxels per metre.
    fn dungeon(scale: i64) -> Vec<[i64; 3]> {
        let room = |x: i64, z: i64| -> Option<i64> {
            let (cx, cz) = (x / 24, z / 24);
            let (lx, lz) = (x % 24, z % 24);
            let floor = 8 + (cx * 3 + cz * 5) % 7;
            let in_room = (4..20).contains(&lx) && (4..20).contains(&lz);
            let corridor_x = (10..13).contains(&lz) && !(4..20).contains(&lx);
            let corridor_z = (10..13).contains(&lx) && !(4..20).contains(&lz);
            if in_room {
                Some(floor)
            } else if corridor_x || corridor_z {
                Some(8)
            } else {
                None
            }
        };
        let mut voxels = Vec::new();
        for x in 0..SIZE * scale {
            for z in 0..SIZE * scale {
                let open = room(x / scale, z / scale);
                for y in 0..HEIGHT * scale {
                    let metres = y / scale;
                    let carved = open.is_some_and(|floor| metres >= floor && metres < floor + 5);
                    if !carved {
                        voxels.push([x, y, z]);
                    }
                }
            }
        }
        voxels
    }

    fn scene(mode: SurfaceMode, roughness: f32, scale: i64) -> VoxelCollisionScene {
        let options = SurfaceMeshOptions {
            mode,
            materials: SurfaceMaterials::new([(
                1,
                MaterialSurface {
                    mode,
                    character: SurfaceCharacter {
                        placement: VertexPlacement::Sharp,
                        crease_angle_degrees: 40.0,
                        roughness,
                    },
                },
            )])
            .unwrap(),
            ..SurfaceMeshOptions::default()
        };
        VoxelCollisionScene::from_solid_voxels_with_mesh_options(
            1.0 / scale as f64,
            16,
            dungeon(scale),
            options,
        )
        .unwrap()
    }

    fn publish(scene: VoxelCollisionScene) -> (f64, NativeCollisionNavigationReplaceReceipt) {
        let mut bridge = RuntimeSpatialBridge::new();
        let session = bridge
            .create(NativeSpatialSessionConfig {
                collision_voxel_size: 1.0,
                collision_chunk_size: 16,
                voxel_surface_mode: NativeVoxelSurfaceMode::GreedyCubes,
            })
            .unwrap();
        bridge.sessions.get_mut(&session.value).unwrap().scene = Arc::new(scene);
        let mut config = flat_config(1.0, 1, 0.3, 1.6, 45.0);
        config.maximum_cells = 1 << 20;
        config.vertical_search_cells = 3;
        let request = NativeCollisionNavigationReplaceRequest {
            session,
            world_min: NativeVec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            world_max: NativeVec3 {
                x: SIZE as f32,
                y: HEIGHT as f32,
                z: SIZE as f32,
            },
            config,
        };
        let started = std::time::Instant::now();
        let receipt = bridge.replace_collision_navigation(&request).unwrap();
        (started.elapsed().as_secs_f64() * 1000.0, receipt)
    }

    #[test]
    #[ignore = "a measurement, not a check"]
    fn measure_dungeon_publication_cost() {
        let only =
            std::env::args().find_map(|argument| argument.strip_prefix("case=").map(str::to_owned));
        for (name, mode, roughness, scale) in [
            ("cubes", SurfaceMode::GreedyCubes, 0.0, 1),
            ("faceted", SurfaceMode::DualContouring, 0.0, 1),
            ("faceted-rough", SurfaceMode::DualContouring, 0.1, 1),
            ("faceted-half-metre", SurfaceMode::DualContouring, 0.0, 2),
        ] {
            if only.as_deref().is_some_and(|only| only != name) {
                continue;
            }
            let built = std::time::Instant::now();
            let scene = scene(mode, roughness, scale);
            let build_ms = built.elapsed().as_secs_f64() * 1000.0;
            let (ms, receipt) = publish(scene);
            println!(
                "{name}: build {build_ms:.0} ms, publish {ms:.0} ms ({} us in the receipt), {} walkable, {} derived, {} edge tests, hash {:016x}",
                receipt.derivation_microseconds,
                receipt.walkable_cell_count,
                receipt.derived_column_count,
                receipt.edge_test_count,
                receipt.projection_hash
            );
        }
    }
}
