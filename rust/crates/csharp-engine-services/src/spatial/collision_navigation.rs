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

/// The columns and edges of the last collision-derived publication, and the
/// world regions whose collision changed since.
#[derive(Debug, Clone)]
pub(super) struct CollisionNavigationCache {
    key: DerivationKey,
    /// The scene the columns plus `dirty` describe. A publication against any
    /// other scene derives everything.
    scene: SceneRevisions,
    columns: BTreeMap<(i64, i64), Vec<(VoxelCoord, f64)>>,
    edges: BTreeMap<VoxelCoord, Vec<VoxelCoord>>,
    /// World-space `[min_x, min_z, max_x, max_z]` of changes since; `None`
    /// when too many were recorded.
    dirty: Option<Vec<[f64; 4]>>,
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
    /// describing nothing.
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
        self.key = self.key.translated(shift[1]);
        self.scene = after;
    }

    fn column_is_clean(&self, grid: VoxelGridSpec, x: i64, z: i64) -> bool {
        let Some(dirty) = &self.dirty else {
            return false;
        };
        let cell_size = grid.voxel_size();
        let margin = cell_size * DIRTY_MARGIN_CELLS + COLLISION_NAVIGATION_CLEARANCE_EPSILON;
        let min = grid.voxel_min_world(VoxelCoord::new(x, 0, z));
        let (min_x, min_z) = (min.x - margin, min.z - margin);
        let (max_x, max_z) = (min.x + cell_size + margin, min.z + cell_size + margin);
        !dirty.iter().any(|region| {
            region[0] <= max_x && region[2] >= min_x && region[1] <= max_z && region[3] >= min_z
        })
    }
}

/// One collision-derived publication and the counts that show how much of
/// it was derived rather than kept.
pub(super) struct CollisionNavigationDerivation {
    pub(super) projection: NavProjection,
    pub(super) supports: BTreeMap<VoxelCoord, f64>,
    pub(super) edge_admission: NavEdgeAdmission,
    pub(super) derived_columns: u64,
    pub(super) reused_columns: u64,
    pub(super) cache: CollisionNavigationCache,
}

/// Derive a conservative, finite planar projection from the session's coherent
/// collision authority. A candidate owns no geometry: support, slope, and
/// headroom are all tested by the same voxel/static-mesh projection used by
/// ordinary spatial queries. Cells prove only that a capsule can stand at
/// their center; directed edges then use the character step solver to prove a
/// wall cannot be crossed and a bounded step can be climbed. Columns and edges
/// that `previous` holds for the same policy, range and scene are kept.
#[allow(clippy::too_many_arguments)]
pub(super) fn collision_navigation_projection(
    scene: &VoxelCollisionScene,
    grid: VoxelGridSpec,
    world_min: [f64; 3],
    world_max: [f64; 3],
    config: NativeCollisionNavigationConfig,
    columns: std::ops::RangeInclusive<i64>,
    rows: std::ops::RangeInclusive<i64>,
    previous: Option<CollisionNavigationCache>,
) -> Result<CollisionNavigationDerivation, CharacterControllerError> {
    let key = DerivationKey::new(config, world_min[1], world_max[1]);
    let revisions = SceneRevisions::of(scene);
    let previous = previous.filter(|cache| cache.key == key && cache.scene == revisions);
    let mut derived = BTreeSet::new();
    let mut column_supports = BTreeMap::new();
    for x in columns {
        for z in rows.clone() {
            let kept = previous
                .as_ref()
                .filter(|cache| cache.column_is_clean(grid, x, z))
                .and_then(|cache| cache.columns.get(&(x, z)));
            let supports = match kept {
                Some(supports) => supports.clone(),
                None => {
                    derived.insert((x, z));
                    derive_column(scene, grid, world_min, world_max, config, x, z)?
                }
            };
            column_supports.insert((x, z), supports);
        }
    }
    let supports: BTreeMap<VoxelCoord, f64> = column_supports
        .values()
        .flatten()
        .map(|&(cell, height)| (cell, height))
        .collect();
    let projection = NavProjection::from_walkable_cells(grid, supports.keys().copied());
    let character = collision_navigation_character_config(config);
    let mut edges = BTreeMap::new();
    for (&from, &from_y) in &supports {
        let near_derived = derived.contains(&(from.x, from.z))
            || [(1, 0), (0, 1), (-1, 0), (0, -1)]
                .into_iter()
                .any(|(dx, dz)| derived.contains(&(from.x + dx, from.z + dz)));
        let kept = (!near_derived)
            .then(|| previous.as_ref().and_then(|cache| cache.edges.get(&from)))
            .flatten();
        let targets = match kept {
            Some(targets) => targets
                .iter()
                .copied()
                .filter(|to| supports.contains_key(to))
                .collect(),
            None => derive_edges(scene, grid, &character, &supports, from, from_y, config)?,
        };
        edges.insert(from, targets);
    }
    let edge_admission = NavEdgeAdmission::from_allowed_edges(
        edges
            .iter()
            .flat_map(|(&from, targets)| targets.iter().map(move |&to| (from, to))),
    );
    let derived_columns = derived.len() as u64;
    let reused_columns = column_supports.len() as u64 - derived_columns;
    Ok(CollisionNavigationDerivation {
        projection,
        supports,
        edge_admission,
        derived_columns,
        reused_columns,
        cache: CollisionNavigationCache {
            key,
            scene: revisions,
            columns: column_supports,
            edges,
            dirty: Some(Vec::new()),
        },
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
        origin_y = support_y - COLLISION_NAVIGATION_EPSILON;
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
    supports: &BTreeMap<VoxelCoord, f64>,
    from: VoxelCoord,
    from_y: f64,
    config: NativeCollisionNavigationConfig,
) -> Result<Vec<VoxelCoord>, CharacterControllerError> {
    let from_center = grid.voxel_center_world(VoxelCoord::new(from.x, 0, from.z));
    let mut targets = Vec::new();
    for to in collision_navigation_neighbors(from, config.max_step_cells) {
        let Some(&to_y) = supports.get(&to) else {
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
