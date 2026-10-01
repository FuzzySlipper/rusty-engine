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

/// Everything besides the scene that a derived column depends on, compared
/// bit for bit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DerivationKey {
    grid_id: u64,
    cell_size: u64,
    chunk_size: u32,
    max_step_cells: u32,
    agent_radius: u64,
    agent_height: u64,
    maximum_slope_degrees: u64,
    world_min_y: u64,
    world_max_y: u64,
}

impl DerivationKey {
    fn new(config: NativeCollisionNavigationConfig, world_min_y: f64, world_max_y: f64) -> Self {
        Self {
            grid_id: config.grid_id,
            cell_size: config.cell_size.to_bits(),
            chunk_size: config.chunk_size,
            max_step_cells: config.max_step_cells,
            agent_radius: config.agent_radius.to_bits(),
            agent_height: config.agent_height.to_bits(),
            maximum_slope_degrees: config.maximum_slope_degrees.to_bits(),
            world_min_y: world_min_y.to_bits(),
            world_max_y: world_max_y.to_bits(),
        }
    }

    fn translated(self, shift_y: f64) -> Self {
        Self {
            world_min_y: (f64::from_bits(self.world_min_y) + shift_y).to_bits(),
            world_max_y: (f64::from_bits(self.world_max_y) + shift_y).to_bits(),
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
}

impl SceneRevisions {
    pub(super) fn of(scene: &VoxelCollisionScene) -> Self {
        let noncollidable = scene
            .noncollidable_materials()
            .iter()
            .fold(0xcbf2_9ce4_8422_2325_u64, |hash, slot| {
                (hash ^ u64::from(*slot)).wrapping_mul(0x0100_0000_01b3)
            });
        Self {
            voxel: scene.source_revision().raw(),
            static_mesh: scene.static_mesh_collision_revision(),
            rebase: scene.rebase_revision(),
            noncollidable,
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
    edges: BTreeMap<VoxelCoord, Vec<VoxelCoord>>,
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
}

impl CollisionNavigationGraph {
    fn from_cache(grid: VoxelGridSpec, cache: &CollisionNavigationCache) -> Self {
        let supports: BTreeMap<VoxelCoord, f64> =
            cache.columns.values().flatten().copied().collect();
        Self {
            projection: NavProjection::from_walkable_cells(grid, supports.keys().copied()),
            supports,
            edge_admission: NavEdgeAdmission::from_allowed_edges(
                cache
                    .edges
                    .iter()
                    .flat_map(|(&from, targets)| targets.iter().map(move |&to| (from, to))),
            ),
        }
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
        let cell_size = f64::from_bits(self.key.cell_size);
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
            .map(|(from, targets)| (moved(from), targets.into_iter().map(moved).collect()))
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
        self.key = self.key.translated(shift[1]);
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
    edges: Vec<(VoxelCoord, Vec<VoxelCoord>)>,
}

impl CollisionNavigationDelta {
    pub(super) fn derived_columns(&self) -> u64 {
        self.derived.len() as u64
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
                    key: self.key,
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
                    for to in targets {
                        graph.edge_admission.set_allowed(from, to, false);
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
        for (from, targets) in self.edges {
            if let Some(graph) = &mut graph {
                for &to in &targets {
                    graph.edge_admission.set_allowed(from, to, true);
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
    world_min: [f64; 3],
    world_max: [f64; 3],
    config: NativeCollisionNavigationConfig,
    bounds: ColumnBounds,
    previous: Option<&CollisionNavigationCache>,
) -> Result<CollisionNavigationDelta, CharacterControllerError> {
    let key = DerivationKey::new(config, world_min[1], world_max[1]);
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
    let mut derived = BTreeMap::new();
    for (x, z) in to_derive {
        derived.insert(
            (x, z),
            derive_column(scene, grid, world_min, world_max, config, x, z)?,
        );
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
    let mut edge_columns = BTreeSet::new();
    for &(x, z) in derived.keys().chain(&removed) {
        for (dx, dz) in [(0, 0), (1, 0), (0, 1), (-1, 0), (0, -1)] {
            edge_columns.insert((x + dx, z + dz));
        }
    }
    let character = collision_navigation_character_config(config);
    let mut edges = Vec::new();
    for &column in edge_columns
        .iter()
        .filter(|&&column| bounds.contains(column))
    {
        for &(from, from_y) in supports_of(column) {
            let targets = derive_edges(scene, grid, &character, &support, from, from_y, config)?;
            edges.push((from, targets));
        }
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
    })
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

fn derive_column(
    scene: &VoxelCollisionScene,
    grid: VoxelGridSpec,
    world_min: [f64; 3],
    world_max: [f64; 3],
    config: NativeCollisionNavigationConfig,
    x: i64,
    z: i64,
) -> Result<Vec<(VoxelCoord, f64)>, CharacterControllerError> {
    let minimum_upward_normal = config.maximum_slope_degrees.to_radians().cos();
    let standing = collision_navigation_character_config(config);
    let center = grid.voxel_center_world(VoxelCoord::new(x, 0, z));
    let mut supports: Vec<(VoxelCoord, f64)> = Vec::new();
    let mut origin_y = world_max[1] + COLLISION_NAVIGATION_EPSILON;
    for _ in 0..MAX_COLLISION_NAVIGATION_SUPPORTS_PER_COLUMN {
        let maximum_distance = origin_y - world_min[1] + COLLISION_NAVIGATION_EPSILON;
        let Some(hit) = scene.raycast_world(
            [center.x, origin_y, center.z],
            [0.0, -1.0, 0.0],
            maximum_distance,
        ) else {
            break;
        };
        let (support_y, normal_y) = collision_navigation_support(hit);
        if support_y < world_min[1] - COLLISION_NAVIGATION_EPSILON {
            break;
        }
        // Just under a voxel surface the ray is inside solid, where it would
        // hit again at once; continue from the air below the solid run.
        let below = support_y - COLLISION_NAVIGATION_EPSILON;
        origin_y = scene
            .collidable_voxel_run_bottom([center.x, below, center.z], world_min[1])
            .map_or(below, |bottom| bottom - COLLISION_NAVIGATION_EPSILON);
        if normal_y < minimum_upward_normal {
            continue;
        }
        let capsule = collision_navigation_capsule(center, support_y, &standing);
        if scene.character_capsule_overlap(capsule)?.is_some() {
            continue;
        }
        let cell = grid.world_to_voxel(core_space::WorldPos::new(center.x, support_y, center.z));
        // The first support found for a cell wins, as the highest one.
        if !supports.iter().any(|(existing, _)| *existing == cell) {
            supports.push((cell, support_y));
        }
    }
    Ok(supports)
}

fn derive_edges(
    scene: &VoxelCollisionScene,
    grid: VoxelGridSpec,
    character: &CharacterControllerConfig,
    support: &dyn Fn(VoxelCoord) -> Option<f64>,
    from: VoxelCoord,
    from_y: f64,
    config: NativeCollisionNavigationConfig,
) -> Result<Vec<VoxelCoord>, CharacterControllerError> {
    let from_center = grid.voxel_center_world(VoxelCoord::new(from.x, 0, from.z));
    let mut targets = Vec::new();
    for to in collision_navigation_neighbors(from, config.max_step_cells) {
        let Some(to_y) = support(to) else {
            continue;
        };
        let to_center = grid.voxel_center_world(VoxelCoord::new(to.x, 0, to.z));
        if character_edge_is_traversable(
            scene,
            character,
            core_space::WorldPos::new(from_center.x, from_y, from_center.z),
            core_space::WorldPos::new(to_center.x, to_y, to_center.z),
        )? {
            targets.push(to);
        }
    }
    Ok(targets)
}

pub(super) fn collision_navigation_character_config(
    config: NativeCollisionNavigationConfig,
) -> CharacterControllerConfig {
    let mut character = CharacterControllerConfig::default();
    character.shape.radius = config.agent_radius as f32;
    character.shape.standing_height = config.agent_height as f32;
    character.shape.contact_skin = COLLISION_NAVIGATION_CLEARANCE_EPSILON as f32;
    character.surface.maximum_step_height =
        (config.cell_size * f64::from(config.max_step_cells)) as f32;
    character.surface.maximum_slope_radians = config.maximum_slope_degrees.to_radians() as f32;
    character
}

fn collision_navigation_capsule(
    center: core_space::WorldPos,
    support_y: f64,
    config: &CharacterControllerConfig,
) -> CharacterCapsule {
    let radius = f64::from(config.shape.radius);
    let half_height =
        f64::from((config.shape.standing_height * 0.5 - config.shape.radius).max(0.0));
    CharacterCapsule {
        center: core_space::WorldPos::new(
            center.x,
            support_y + half_height + radius + f64::from(config.shape.contact_skin),
            center.z,
        ),
        half_height,
        radius,
    }
}

fn collision_navigation_neighbors(
    coord: VoxelCoord,
    max_step_cells: u32,
) -> impl Iterator<Item = VoxelCoord> {
    let mut neighbors = Vec::with_capacity(4 * (1 + max_step_cells as usize * 2));
    for (dx, dz) in [(1, 0), (0, 1), (-1, 0), (0, -1)] {
        neighbors.push(VoxelCoord::new(coord.x + dx, coord.y, coord.z + dz));
        for step in 1..=i64::from(max_step_cells) {
            neighbors.push(VoxelCoord::new(coord.x + dx, coord.y + step, coord.z + dz));
            neighbors.push(VoxelCoord::new(coord.x + dx, coord.y - step, coord.z + dz));
        }
    }
    neighbors.into_iter()
}

fn collision_navigation_support(hit: engine_spatial::SpatialCollisionHit) -> (f64, f64) {
    match hit {
        engine_spatial::SpatialCollisionHit::Voxel(hit) => {
            (hit.point[1], f64::from((hit.face == Face::PosY) as u8))
        }
        engine_spatial::SpatialCollisionHit::StaticMesh(hit) => (hit.point.y, hit.normal.y),
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
            config: NativeCollisionNavigationConfig {
                grid_id: 7,
                cell_size: 1.0,
                chunk_size: 16,
                max_step_cells: 1,
                agent_radius: 0.3,
                agent_height: 1.6,
                maximum_slope_degrees: 45.0,
                maximum_cells: 4096,
            },
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
