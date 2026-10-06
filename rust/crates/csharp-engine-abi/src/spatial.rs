use crate::*;
/// Opaque domain handles reserve typed direct service families without a
/// universal identity/capability table. Phase A needs the UI stream handle.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeSpatialSessionHandle {
    pub value: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSpatialSessionConfig {
    pub collision_voxel_size: f64,
    pub collision_chunk_size: u32,
    /// The surface mode of every voxel material without its own surface
    /// (Voxel.ConfigureMaterialSurfaces). A reconstructed mode also makes
    /// collision follow the drawn surface.
    pub voxel_surface_mode: NativeVoxelSurfaceMode,
}

/// How voxels are drawn: cube faces, or a surface reconstructed from the
/// voxels (and their densities). The Engine retains the chosen modes through
/// later voxel edits, residency changes, and origin rebases.
#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeVoxelSurfaceMode {
    #[default]
    GreedyCubes = 0,
    MarchingCubes = 1,
    DualContouring = 2,
}

/// How a reconstructed material places each surface cell's vertex. Where
/// materials meet in one cell the sharper placement wins (Smooth, Sharp,
/// Blocky, then cube materials).
#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeVertexPlacement {
    /// Minimizes the error to the crossings' planes: keeps corners and edges.
    #[default]
    Sharp = 0,
    /// The mean of the crossings: rounded, blob-like surfaces.
    Smooth = 1,
    /// Blocks on the voxel grid: crossings sit on voxel faces and normals snap
    /// to the axes, so a cube of material meshes as a cube.
    Blocky = 2,
}

/// The character of one material's reconstructed surface.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct NativeSurfaceCharacter {
    pub placement: NativeVertexPlacement,
    /// Faces bending further than this from their vertex's normal shade flat;
    /// 0 shades every facet flat, 180 everything smooth.
    pub crease_angle_degrees: f32,
    /// Deterministic per-cell vertex displacement, 0 to 0.5 of a cell.
    pub roughness: f32,
}

/// Named source of a spatial query result. `None` is a miss, while the other
/// values identify the Engine-owned projection that produced the fact.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NativeSpatialHitKind {
    #[default]
    None = 0,
    Voxel = 1,
    StaticMesh = 2,
    Entity = 3,
}

/// Stable face vocabulary used by voxel raycasts and picking.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NativeSpatialFace {
    #[default]
    None = 0,
    PosX = 1,
    NegX = 2,
    PosY = 3,
    NegY = 4,
    PosZ = 5,
    NegZ = 6,
}

/// Trigger geometry is an explicit owner policy; the default keeps the
/// existing active-collision behavior while bounds-only sensors remain
/// available for products that do not want a trigger to be a solid obstacle.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NativeSpatialTriggerGeometry {
    #[default]
    ActiveCollision = 0,
    EntityBounds = 1,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NativeSpatialTriggerCause {
    #[default]
    Scheduled = 0,
    Spawn = 1,
    Movement = 2,
    Teleport = 3,
    ActivationChanged = 4,
    LifecycleChanged = 5,
    Restore = 6,
}

/// A bounded caller-owned entity collider. `min` and `max` are world-space
/// AABB endpoints. Group/mask are optional product filtering facts: zero on
/// either side means the ordinary unfiltered path.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeSpatialEntityCollider {
    pub entity: u64,
    pub min: NativeVec3,
    pub max: NativeVec3,
    pub collision_group: u32,
    pub collision_mask: u32,
    pub enabled: bool,
    pub static_collider: bool,
    pub trigger: bool,
}

/// A query-level collision filter. The Engine applies the same symmetric
/// group/mask rule to caller-owned entity records; zero preserves all records.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeSpatialQueryFilter {
    pub collision_group: u32,
    pub collision_mask: u32,
}

/// One bounded result fact shared by ray, segment, capsule, and overlap
/// operations. A miss has `kind == None`; voxel coordinates and face are only
/// meaningful for voxel hits, while IDs distinguish static meshes/entities.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeSpatialHit {
    pub present: bool,
    pub kind: NativeSpatialHitKind,
    pub entity: u64,
    pub instance: u64,
    pub asset: u64,
    pub geometry_hash: u64,
    pub voxel_x: i64,
    pub voxel_y: i64,
    pub voxel_z: i64,
    pub face: NativeSpatialFace,
    pub point: NativeVec3,
    pub normal: NativeVec3,
    pub distance: f64,
    pub time_of_impact: f64,
    pub penetration_depth: f64,
    pub start_solid: bool,
    pub converged: bool,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeSpatialProjectionReadout {
    pub source_revision: u64,
    pub collision_revision: u64,
    pub projection_version: u64,
    pub authority_hash: u64,
    pub resident_chunk_count: u64,
    pub collider_chunk_count: u64,
    pub static_mesh_revision: u64,
    pub static_mesh_asset_count: u64,
    pub static_mesh_instance_count: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeSpatialQueryReceipt {
    pub present: bool,
    pub blocked: bool,
    pub overlaps: u32,
    pub projection_version: u64,
    pub source_revision: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSpatialProjectionReadRequest {
    pub session: NativeSpatialSessionHandle,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSpatialContainsPointRequest {
    pub session: NativeSpatialSessionHandle,
    pub point: NativeVec3,
}

/// Shared combined ray request. Entity records, ignored IDs, and hitbox
/// overrides are borrowed only for the duration of the call.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSpatialRaycastRequest {
    pub session: NativeSpatialSessionHandle,
    pub origin: NativeVec3,
    pub direction: NativeVec3,
    pub max_distance: f64,
    pub filter: NativeSpatialQueryFilter,
    pub entities: *const NativeSpatialEntityCollider,
    pub entities_len: usize,
    pub ignored_entities: *const u64,
    pub ignored_entities_len: usize,
    pub hitbox_overrides: *const NativeSpatialEntityCollider,
    pub hitbox_overrides_len: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSpatialSegmentCastRequest {
    pub session: NativeSpatialSessionHandle,
    pub start: NativeVec3,
    pub end: NativeVec3,
    pub filter: NativeSpatialQueryFilter,
    pub entities: *const NativeSpatialEntityCollider,
    pub entities_len: usize,
    pub ignored_entities: *const u64,
    pub ignored_entities_len: usize,
    pub hitbox_overrides: *const NativeSpatialEntityCollider,
    pub hitbox_overrides_len: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSpatialAabbQueryRequest {
    pub session: NativeSpatialSessionHandle,
    pub min: NativeVec3,
    pub max: NativeVec3,
    pub translation: NativeVec3,
    pub filter: NativeSpatialQueryFilter,
    pub entities: *const NativeSpatialEntityCollider,
    pub entities_len: usize,
    pub ignored_entities: *const u64,
    pub ignored_entities_len: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSpatialCapsuleQueryRequest {
    pub session: NativeSpatialSessionHandle,
    pub center: NativeVec3,
    pub half_height: f64,
    pub radius: f64,
    pub translation: NativeVec3,
    pub contact_skin: f64,
    pub filter: NativeSpatialQueryFilter,
    pub entities: *const NativeSpatialEntityCollider,
    pub entities_len: usize,
    pub ignored_entities: *const u64,
    pub ignored_entities_len: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSpatialPickRequest {
    pub session: NativeSpatialSessionHandle,
    pub origin: NativeVec3,
    pub direction: NativeVec3,
    pub max_distance: f64,
    pub claimed_voxel_x: i64,
    pub claimed_voxel_y: i64,
    pub claimed_voxel_z: i64,
    pub claimed_face: NativeSpatialFace,
}

/// One trigger registration keeps scope and an optional single tag as direct
/// UTF-8 values. Products that need richer tag sets can register multiple
/// named trigger entities; the spatial owner remains responsible for overlap
/// truth and event ordering.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSpatialTriggerRegisterRequest {
    pub session: NativeSpatialSessionHandle,
    pub trigger: u64,
    pub scope: NativeUtf8Slice,
    pub tag: NativeUtf8Slice,
    pub geometry: NativeSpatialTriggerGeometry,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSpatialTriggerReconcileRequest {
    pub session: NativeSpatialSessionHandle,
    pub tick: u64,
    pub cause: NativeSpatialTriggerCause,
    pub entities: *const NativeSpatialEntityCollider,
    pub entities_len: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSpatialTriggerSetActiveRequest {
    pub session: NativeSpatialSessionHandle,
    pub trigger: u64,
    pub active: bool,
    pub tick: u64,
}

/// One enter or exit edge between a trigger and a subject.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeSpatialTriggerFact {
    pub enter: bool,
    pub trigger: u64,
    pub subject: u64,
    pub tick: u64,
    pub cause: NativeSpatialTriggerCause,
}

/// Borrowed result of one activation change. `facts` holds the exit edges of
/// a deactivation, points into Spatial bridge storage and stays valid until
/// the next call on the same context; the generated managed binding copies
/// it before returning.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSpatialTriggerLifecycleResult {
    pub facts: *const NativeSpatialTriggerFact,
    pub facts_len: usize,
    pub trigger: u64,
    pub active: bool,
    pub removed_overlap_count: u32,
}

/// Replaces the active-trigger and current-overlap baseline after product
/// restore. The borrowed active IDs are the complete desired active set among
/// the session's registered definitions. No enter or exit facts are emitted.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSpatialTriggerRestoreRequest {
    pub session: NativeSpatialSessionHandle,
    pub active_triggers: *const u64,
    pub active_triggers_len: usize,
    pub entities: *const NativeSpatialEntityCollider,
    pub entities_len: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeSpatialTriggerRestoreReceipt {
    pub registered_count: u32,
    pub active_count: u32,
    pub active_overlap_count: u32,
    pub diagnostic_count: u32,
}

/// Borrowed result of one reconcile: the enter and exit edges it produced.
/// `facts` points into Spatial bridge storage and stays valid until the next
/// call on the same context.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSpatialTriggerReconcileResult {
    pub facts: *const NativeSpatialTriggerFact,
    pub facts_len: usize,
    pub tick: u64,
    pub cause: NativeSpatialTriggerCause,
    pub continued_count: u32,
    pub active_overlap_count: u32,
    pub diagnostic_count: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSpatialTriggerReadRequest {
    pub session: NativeSpatialSessionHandle,
    pub trigger: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeSpatialTriggerOverlapSubject {
    pub subject: u64,
}

/// Borrowed readout of one trigger: its active flag and every current
/// overlap subject. `subjects` points into Spatial bridge storage and stays
/// valid until the next call on the same context.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSpatialTriggerReadResult {
    pub subjects: *const NativeSpatialTriggerOverlapSubject,
    pub subjects_len: usize,
    pub trigger: u64,
    pub active: bool,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMeshResourceReference {
    /// Zero preserves the borrowed raw-range collision input path.
    pub value: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStaticMeshAsset {
    pub id: u64,
    /// A live inline Graphics mesh copied into Spatial during this call.
    pub mesh_resource: NativeMeshResourceReference,
    pub first_vertex: u32,
    pub vertex_count: u32,
    pub first_triangle: u32,
    pub triangle_count: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeTriangle {
    pub a: u32,
    pub b: u32,
    pub c: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStaticMeshInstance {
    pub id: u64,
    pub asset: u64,
    pub transform: NativeTransform,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCollisionReplaceRequest {
    pub session: NativeSpatialSessionHandle,
    pub assets: *const NativeStaticMeshAsset,
    pub assets_len: usize,
    pub vertices: *const NativeVec3,
    pub vertices_len: usize,
    pub triangles: *const NativeTriangle,
    pub triangles_len: usize,
    pub instances: *const NativeStaticMeshInstance,
    pub instances_len: usize,
}

/// Atomically removes and upserts selected static collision identities. Geometry
/// is copied into the Spatial owner. Unmentioned assets/instances remain resident.
/// Instance transforms are in the session's current local origin frame.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCollisionResidencyRequest {
    pub session: NativeSpatialSessionHandle,
    pub assets: *const NativeStaticMeshAsset,
    pub assets_len: usize,
    pub vertices: *const NativeVec3,
    pub vertices_len: usize,
    pub triangles: *const NativeTriangle,
    pub triangles_len: usize,
    pub instances: *const NativeStaticMeshInstance,
    pub instances_len: usize,
    pub removed_assets: *const u64,
    pub removed_assets_len: usize,
    pub removed_instances: *const u64,
    pub removed_instances_len: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeCollisionReplaceReceipt {
    pub revision_before: u64,
    pub revision_after: u64,
    pub asset_count: u64,
    pub instance_count: u64,
    pub projection_hash: u64,
}

/// One spatial content artifact placed beside the session's others, under a
/// stable product identity. It turns `quarter_turns` times 90 degrees about +Y
/// around its own origin (a quarter turn takes +X to -Z; four is a full turn),
/// then moves by whole navigation cells from the session's navigation grid
/// (columns along x, levels of the artifact's level quantum along y, rows
/// along z), followed by the finite continuous `translation` in world units.
/// Rotation is about the artifact origin; the continuous translation is
/// applied after the integer placement and rotation.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSpatialContentArtifactInstance {
    pub id: u64,
    pub content: NativeContentReferenceHandle,
    pub column_offset: i64,
    pub level_offset: i64,
    pub row_offset: i64,
    pub quarter_turns: u32,
    pub translation: NativeVec3,
}

/// Admits and removes placed spatial artifacts in one session: their
/// collision joins the session's static collision, and planar navigation
/// becomes the union of the base artifact's cells and every placed one's.
/// Removals apply first, so one request can replace an identity. The
/// content references are borrowed for this call.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSpatialContentArtifactResidencyRequest {
    pub session: NativeSpatialSessionHandle,
    pub admitted: *const NativeSpatialContentArtifactInstance,
    pub admitted_len: usize,
    pub removed: *const u64,
    pub removed_len: usize,
    pub navigation_grid_id: u64,
    pub navigation_chunk_size: u32,
    pub navigation_max_step_cells: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeSpatialContentArtifactResidencyReceipt {
    pub collision_revision_before: u64,
    pub collision_revision_after: u64,
    pub navigation_revision: u64,
    /// Placed artifacts now resident, not counting the base.
    pub instance_count: u64,
    pub navigation_cell_count: u64,
    pub collision_projection_hash: u64,
    pub navigation_projection_hash: u64,
}

/// Replaces retained collision and planar navigation from one immutable
/// Engine-admitted spatial artifact. The content reference is borrowed for
/// this call; the resulting Spatial state retains copied mechanism facts and
/// immutable source identity only.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSpatialContentArtifactReplaceRequest {
    pub session: NativeSpatialSessionHandle,
    pub content: NativeContentReferenceHandle,
    pub navigation_grid_id: u64,
    pub navigation_chunk_size: u32,
    pub navigation_max_step_cells: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeSpatialContentArtifactReplaceReceipt {
    /// Non-owning identity of the Content reference used for this admission.
    /// The original managed `ContentReference` retains sole disposal ownership.
    pub content_reference_value: u64,
    pub content_sha256: NativeContentSha256,
    pub collision_revision_before: u64,
    pub collision_revision_after: u64,
    pub navigation_revision: u64,
    pub collision_vertex_count: u64,
    pub collision_triangle_count: u64,
    pub navigation_cell_count: u64,
    pub collision_projection_hash: u64,
    pub navigation_projection_hash: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSpatialContentArtifactReadRequest {
    pub session: NativeSpatialSessionHandle,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeSpatialContentArtifactReadout {
    pub present: bool,
    /// Non-owning identity of the Content reference used for this admission.
    pub content_reference_value: u64,
    pub content_sha256: NativeContentSha256,
    pub collision_revision: u64,
    pub navigation_revision: u64,
    pub collision_vertex_count: u64,
    pub collision_triangle_count: u64,
    pub navigation_cell_count: u64,
    pub collision_projection_hash: u64,
    pub navigation_projection_hash: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePlanarNavConfig {
    pub grid_id: u64,
    pub cell_size: f64,
    pub chunk_size: u32,
    pub max_step_cells: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativePlanarNavCell {
    pub x: i64,
    pub y: i64,
    pub z: i64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeNavigationReplaceRequest {
    pub session: NativeSpatialSessionHandle,
    pub config: NativePlanarNavConfig,
    pub cells: *const NativePlanarNavCell,
    pub cells_len: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeNavigationReplaceReceipt {
    pub walkable_cell_count: u64,
    pub projection_hash: u64,
    pub navigation_revision: u64,
}

/// One bounded purpose-neutral traversal rule. `allowed == false` prevents
/// entry; omitted cells remain allowed with traversal cost one.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeNavigationTraversalCell {
    pub cell: NativePlanarNavCell,
    pub allowed: bool,
    pub traversal_cost: u64,
}

/// Replaces the retained traversal overlay for the session's existing planar
/// navigation projection. The records are borrowed only for this call.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeNavigationTraversalReplaceRequest {
    pub session: NativeSpatialSessionHandle,
    pub cells: *const NativeNavigationTraversalCell,
    pub cells_len: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeNavigationTraversalReplaceReceipt {
    pub traversal_cell_count: u64,
    pub traversal_overlay_hash: u64,
    pub navigation_revision: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeNavigationTraversalClearRequest {
    pub session: NativeSpatialSessionHandle,
}

/// One bounded purpose-neutral traversal rule for volumetric navigation.
/// Unlike the planar traversal cell, this is not validated against a
/// NavProjection: volumetric occupancy is admitted dynamically by the query.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeNavigationVolumetricTraversalCell {
    pub cell: NativePlanarNavCell,
    pub allowed: bool,
    pub traversal_cost: u64,
}

/// Replaces the retained traversal overlay for the session's voxel-derived
/// volumetric navigation source. Records are borrowed only for this call.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeNavigationVolumetricTraversalReplaceRequest {
    pub session: NativeSpatialSessionHandle,
    pub cells: *const NativeNavigationVolumetricTraversalCell,
    pub cells_len: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeNavigationVolumetricTraversalReplaceReceipt {
    pub traversal_cell_count: u64,
    pub traversal_overlay_hash: u64,
    pub volumetric_source_hash: u64,
    pub navigation_revision: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeNavigationVolumetricTraversalClearRequest {
    pub session: NativeSpatialSessionHandle,
}

/// The admitted Engine-owned substrate for a navigation projection. Host cells
/// are already-walkable facts; voxel-derived projections remain derived by the
/// pathfinding owner from admitted solid voxels and agent dimensions.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NativeNavigationProjectionKind {
    #[default]
    None = 0,
    HostWalkableCells = 1,
    VoxelDerived = 2,
    CollisionDerived = 3,
}

/// Typed, non-exceptional navigation query outcomes. A query failure is an
/// ordinary Engine fact, not a product-defined error protocol.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NativeNavigationPathOutcome {
    #[default]
    Reached = 0,
    NoPath = 1,
    BudgetExhausted = 2,
    InvalidQueryBudget = 3,
    StartNotWalkable = 4,
    GoalNotWalkable = 5,
    StartNotTraversable = 6,
    GoalNotTraversable = 7,
    InvalidAgentVolume = 8,
    InvalidStep = 9,
    NonFinitePosition = 10,
    ProjectionUnavailable = 11,
    InvalidAgentHeight = 12,
    StartBlocked = 13,
    GoalBlocked = 14,
    CostOverflow = 15,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeNavigationVoxelReplaceRequest {
    pub session: NativeSpatialSessionHandle,
    pub config: NativePlanarNavConfig,
    pub agent_height_voxels: u32,
    pub require_solid_floor: bool,
    pub solid_cells: *const NativePlanarNavCell,
    pub solid_cells_len: usize,
}

/// Bounded agent policy for deriving a planar navigation projection from the
/// session's retained voxel and static-mesh collision authority. The default
/// configuration (`default_collision_navigation_config`) is a starting point
/// to vary one option at a time.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCollisionNavigationConfig {
    pub grid_id: u64,
    pub cell_size: f64,
    pub chunk_size: u32,
    pub maximum_cells: u32,
    /// The body navigation stands and steps with, as the product's character
    /// controller uses it: shape radius, standing height and contact skin;
    /// surface maximum slope, maximum step height (metres up), minimum step
    /// width and floor snap; recovery tolerances.
    pub character: NativeCharacterControllerConfig,
    /// Farthest a one-way downward edge may fall, in metres.
    pub maximum_drop: f64,
    /// How many cell levels up or down a neighbouring support is searched
    /// for. The step height and drop decide which of those are edges; a
    /// neighbour beyond this reach is never considered.
    pub vertical_search_cells: u32,
    /// Surfaces sampled per X/Z column, top down; a deeper layer is unknown.
    pub supports_per_column: u32,
    /// Also connect diagonal neighbours. A diagonal is one path step, as an
    /// orthogonal neighbour is.
    pub diagonal_neighbors: bool,
    /// How far a query point may lie above a support and still stand on it.
    pub snap_above: f64,
    /// How far a query point may lie below a support and still stand on it.
    pub snap_below: f64,
    /// How far beyond the footprint of the cell containing a query point a
    /// support may be taken from; zero keeps the containing cell only.
    pub snap_across: f64,
    /// Also connect a neighbour too high to step onto when the character's
    /// own standing jump (`character.vertical.jump_speed` and `gravity`,
    /// moving across with the controller's air acceleration up to
    /// `character.air.maximum_speed`) clears it.
    pub jump_ledges: bool,
    /// Also connect supports straight across a gap of up to this many cells,
    /// along X or Z or, with `diagonal_neighbors`, diagonally, with no
    /// support near the jump's height, when the same jump carries there;
    /// zero for none.
    pub jump_gap_cells: u32,
    /// What a weighted path pays for a jump edge beyond its destination
    /// cell's cost, so a planner can prefer walking.
    pub jump_cost: u32,
}

/// How a path reaches one of its cells from the one before.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NativeNavigationPathEdge {
    pub kind: NativeNavigationEdgeKind,
}

/// How a path reaches one of its cells from the one before.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NativeNavigationEdgeKind {
    /// Walking, stepping up or down within the step height, or along a slope;
    /// also the path's first cell.
    #[default]
    Walk = 0,
    /// Falling to a support lower than the step height.
    Drop = 1,
    /// The character's jump, up a ledge or across a gap.
    Jump = 2,
}

/// Why one surface a collision-navigation column sampled is or is not a
/// support.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NativeCollisionNavigationSampleOutcome {
    #[default]
    Support = 0,
    /// Steeper than the character's maximum slope.
    TooSteep = 1,
    /// The standing capsule overlaps collision; the overlap source says what.
    CapsuleOverlap = 2,
    /// A higher support already holds this cell.
    SameCell = 3,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeCollisionNavigationSample {
    pub outcome: NativeCollisionNavigationSampleOutcome,
    pub hit_kind: NativeSpatialHitKind,
    pub surface_y: f64,
    pub normal_y: f64,
    /// Where the capsule's feet stand: above the surface on a slope.
    pub standing_y: f64,
    pub cell: NativePlanarNavCell,
    /// For `CapsuleOverlap`: what the capsule overlaps (a voxel collision
    /// chunk or a static-mesh instance) and the deepest contact point.
    pub overlap_kind: NativeCharacterCollisionSourceKind,
    pub overlap_instance: u64,
    pub overlap_asset: u64,
    pub overlap_chunk_x: i64,
    pub overlap_chunk_y: i64,
    pub overlap_chunk_z: i64,
    pub overlap_point: NativeVec3,
}

/// One X/Z column of the session's collision-derived navigation, derived
/// again against current collision with the published policy and range.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCollisionNavigationColumnRequest {
    pub session: NativeSpatialSessionHandle,
    pub x: i64,
    pub z: i64,
}

/// Borrowed until the next Spatial call. No samples means the column hit
/// nothing in the published vertical range.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCollisionNavigationColumnResult {
    pub samples: *const NativeCollisionNavigationSample,
    pub samples_len: usize,
    /// Sampling stopped at the per-column budget, not at the range's bottom.
    pub budget_exhausted: bool,
    pub navigation_revision: u64,
}

impl Default for NativeCollisionNavigationColumnResult {
    fn default() -> Self {
        Self {
            samples: std::ptr::null(),
            samples_len: 0,
            budget_exhausted: false,
            navigation_revision: 0,
        }
    }
}

/// Why a directed collision-navigation edge is or is not traversable.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NativeCollisionNavigationEdgeOutcome {
    #[default]
    Traversable = 0,
    FromNotSupport = 1,
    ToNotSupport = 2,
    /// Not a neighbour under the published neighbourhood and reach.
    NotNeighbor = 3,
    RiseOverStep = 4,
    DropOverMaximum = 5,
    StartOverlap = 6,
    EndOverlap = 7,
    /// A level or downward move is blocked at the start height, and neither a
    /// step nor a walk over ground within the maximum slope clears it.
    HorizontalSweepBlocked = 8,
    /// The fall onto the lower support is blocked.
    DescentBlocked = 9,
    /// The rise-forward-drop step manoeuvre found no landing on the target.
    StepManeuverFailed = 10,
    /// Too high to step onto, but the character's jump clears it.
    JumpTraversable = 11,
    /// Too high for the jump's peak.
    RiseOverJump = 12,
    /// The capsule cannot rise straight up to the jump's peak.
    JumpHeadroomBlocked = 13,
    /// The capsule meets collision along the jump's arc.
    JumpArcBlocked = 14,
    /// Farther than the jump carries at the maximum air speed.
    GapTooWide = 15,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCollisionNavigationEdgeRequest {
    pub session: NativeSpatialSessionHandle,
    pub from: NativePlanarNavCell,
    pub to: NativePlanarNavCell,
}

/// The edge evaluated against current collision with the published policy;
/// `admitted` says whether the installed navigation holds it.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeCollisionNavigationEdgeReadout {
    pub outcome: NativeCollisionNavigationEdgeOutcome,
    pub admitted: bool,
    pub from_y: f64,
    pub to_y: f64,
    pub navigation_revision: u64,
}

/// Atomically replaces the session's retained planar projection by sampling
/// the coherent Engine collision scene in the explicit finite world region.
/// The product supplies policy and bounds only; collision geometry never
/// crosses the ABI boundary.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCollisionNavigationReplaceRequest {
    pub session: NativeSpatialSessionHandle,
    pub world_min: NativeVec3,
    pub world_max: NativeVec3,
    pub config: NativeCollisionNavigationConfig,
}

/// A collision-derived publication keeps the previous one's columns wherever
/// neither the box, the policy, the vertical range nor the collision there
/// changed; the counts show how many columns were derived and how many kept.
/// `edge_test_count` is the support-to-support edges tested with capsule
/// casts, and `derivation_microseconds` the time deriving and installing took,
/// for budgeting a republish. `component_count` is how many groups the cells
/// fall into with no edge between the groups in either direction: a query
/// across groups is `NoPath` without a search.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeCollisionNavigationReplaceReceipt {
    pub walkable_cell_count: u64,
    pub projection_hash: u64,
    pub navigation_revision: u64,
    pub derived_column_count: u64,
    pub reused_column_count: u64,
    pub edge_test_count: u64,
    pub derivation_microseconds: u64,
    pub component_count: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeNavigationProjectionReadRequest {
    pub session: NativeSpatialSessionHandle,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeNavigationProjectionReadout {
    pub present: bool,
    pub kind: NativeNavigationProjectionKind,
    pub walkable_cell_count: u64,
    pub projection_hash: u64,
    pub navigation_revision: u64,
    pub agent_height_voxels: u32,
    pub require_solid_floor: bool,
    pub max_step_cells: u32,
    pub traversal_cell_count: u64,
    pub traversal_overlay_hash: u64,
}

/// A bounded, full planar path request over the session's admitted projection.
/// `max_visited` bounds both the owner search and the retained indexed result.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeNavigationPathRequest {
    pub session: NativeSpatialSessionHandle,
    pub start: NativePlanarNavCell,
    pub goal: NativePlanarNavCell,
    pub max_visited: u32,
}

/// Borrowed result of one path request. `path` lists the path cells from
/// start to goal, points into Spatial bridge storage and stays valid until
/// the next call on the same context; the generated managed binding copies
/// it before returning.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeNavigationPathResult {
    pub path: *const NativePlanarNavCell,
    pub path_len: usize,
    /// How each path cell is reached from the one before, borrowed like
    /// `path` and as long.
    pub edges: *const NativeNavigationPathEdge,
    pub edges_len: usize,
    pub outcome: NativeNavigationPathOutcome,
    pub kind: NativeNavigationProjectionKind,
    pub visited: u32,
    pub navigation_revision: u64,
    pub projection_hash: u64,
    pub path_hash: u64,
}

impl Default for NativeNavigationPathResult {
    fn default() -> Self {
        Self {
            path: std::ptr::null(),
            path_len: 0,
            edges: std::ptr::null(),
            edges_len: 0,
            outcome: Default::default(),
            kind: Default::default(),
            visited: 0,
            navigation_revision: 0,
            projection_hash: 0,
            path_hash: 0,
        }
    }
}

/// A bounded full planar minimum-cost path request over the session's
/// projection and retained traversal overlay.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeNavigationWeightedPathRequest {
    pub session: NativeSpatialSessionHandle,
    pub start: NativePlanarNavCell,
    pub goal: NativePlanarNavCell,
    pub max_visited: u32,
}

/// Borrowed result of one weighted planar path request; `path` is borrowed
/// like [`NativeNavigationPathResult::path`].
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeNavigationWeightedPathResult {
    pub path: *const NativePlanarNavCell,
    pub path_len: usize,
    /// How each path cell is reached from the one before, borrowed like
    /// `path` and as long.
    pub edges: *const NativeNavigationPathEdge,
    pub edges_len: usize,
    pub outcome: NativeNavigationPathOutcome,
    pub kind: NativeNavigationProjectionKind,
    pub visited: u32,
    pub total_traversal_cost: u64,
    pub navigation_revision: u64,
    pub projection_hash: u64,
    pub traversal_overlay_hash: u64,
    pub path_hash: u64,
}

impl Default for NativeNavigationWeightedPathResult {
    fn default() -> Self {
        Self {
            path: std::ptr::null(),
            path_len: 0,
            edges: std::ptr::null(),
            edges_len: 0,
            outcome: Default::default(),
            kind: Default::default(),
            visited: 0,
            total_traversal_cost: 0,
            navigation_revision: 0,
            projection_hash: 0,
            traversal_overlay_hash: 0,
            path_hash: 0,
        }
    }
}

/// A bounded full 3D minimum-cost path request over the session's retained
/// voxel-derived navigation source and volumetric traversal overlay.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeNavigationVolumetricWeightedPathRequest {
    pub session: NativeSpatialSessionHandle,
    pub start: NativePlanarNavCell,
    pub goal: NativePlanarNavCell,
    pub max_visited: u32,
    pub config: NativeNavigationVolumetricConfig,
}

/// Borrowed result of one weighted volumetric path request; `path` is
/// borrowed like [`NativeNavigationPathResult::path`].
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeNavigationVolumetricWeightedPathResult {
    pub path: *const NativePlanarNavCell,
    pub path_len: usize,
    pub outcome: NativeNavigationPathOutcome,
    pub kind: NativeNavigationProjectionKind,
    pub visited: u32,
    pub total_traversal_cost: u64,
    pub navigation_revision: u64,
    pub volumetric_source_hash: u64,
    pub traversal_overlay_hash: u64,
    pub path_hash: u64,
}

impl Default for NativeNavigationVolumetricWeightedPathResult {
    fn default() -> Self {
        Self {
            path: std::ptr::null(),
            path_len: 0,
            outcome: Default::default(),
            kind: Default::default(),
            visited: 0,
            total_traversal_cost: 0,
            navigation_revision: 0,
            volumetric_source_hash: 0,
            traversal_overlay_hash: 0,
            path_hash: 0,
        }
    }
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NativeNavigationVolumetricNeighborSet {
    Planar4 = 0,
    #[default]
    Faces6 = 1,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NativeNavigationVolumetricVerticalPolicy {
    DisallowVertical = 0,
    #[default]
    AllowVertical = 1,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NativeNavigationVolumetricTraversalRule {
    #[default]
    EmptyCells = 0,
    SolidCells = 1,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeNavigationVolumetricConfig {
    pub size_x: u32,
    pub size_y: u32,
    pub size_z: u32,
    pub neighbor_set: NativeNavigationVolumetricNeighborSet,
    pub vertical_policy: NativeNavigationVolumetricVerticalPolicy,
    pub traversal_rule: NativeNavigationVolumetricTraversalRule,
}

/// A bounded full 3D path over a retained voxel-derived navigation source.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeNavigationVolumetricPathRequest {
    pub session: NativeSpatialSessionHandle,
    pub start: NativePlanarNavCell,
    pub goal: NativePlanarNavCell,
    pub max_visited: u32,
    pub config: NativeNavigationVolumetricConfig,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeNavigationClearRequest {
    pub session: NativeSpatialSessionHandle,
}

/// The continuity facts the C# product retains between Engine-owned proposal calls.
#[repr(u32)]
#[derive(Debug, Clone, Copy, Default)]
pub enum NativeCharacterStance {
    #[default]
    Standing = 0,
    Crouched = 1,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default)]
pub enum NativeCharacterSupportLifecycle {
    #[default]
    Active = 0,
    Disabled = 1,
    Destroyed = 2,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default)]
pub enum NativeCharacterContactKind {
    #[default]
    None = 0,
    Ground = 1,
    SteepSlope = 2,
    Wall = 3,
    Ceiling = 4,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default)]
pub enum NativeCharacterCollisionSourceKind {
    #[default]
    None = 0,
    VoxelChunk = 1,
    StaticMesh = 2,
    ActiveEntity = 3,
}

/// Bitwise summary of the finite controller block vocabulary. Every combined
/// value is named so Rust never needs to construct an invalid enum while C#
/// receives a normal flags-shaped enum rather than an untyped integer.
#[repr(u32)]
#[derive(Debug, Clone, Copy, Default)]
pub enum NativeCharacterBlockFlags {
    #[default]
    None = 0,
    Wall = 1,
    Ceiling = 2,
    WallCeiling = 3,
    SteepSlope = 4,
    WallSteepSlope = 5,
    CeilingSteepSlope = 6,
    WallCeilingSteepSlope = 7,
    StartSolid = 8,
    WallStartSolid = 9,
    CeilingStartSolid = 10,
    WallCeilingStartSolid = 11,
    SteepSlopeStartSolid = 12,
    WallSteepSlopeStartSolid = 13,
    CeilingSteepSlopeStartSolid = 14,
    WallCeilingSteepSlopeStartSolid = 15,
    SolverBudget = 16,
    WallSolverBudget = 17,
    CeilingSolverBudget = 18,
    WallCeilingSolverBudget = 19,
    SteepSlopeSolverBudget = 20,
    WallSteepSlopeSolverBudget = 21,
    CeilingSteepSlopeSolverBudget = 22,
    WallCeilingSteepSlopeSolverBudget = 23,
    StartSolidSolverBudget = 24,
    WallStartSolidSolverBudget = 25,
    CeilingStartSolidSolverBudget = 26,
    WallCeilingStartSolidSolverBudget = 27,
    SteepSlopeStartSolidSolverBudget = 28,
    WallSteepSlopeStartSolidSolverBudget = 29,
    CeilingSteepSlopeStartSolidSolverBudget = 30,
    WallCeilingSteepSlopeStartSolidSolverBudget = 31,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeCharacterMotion {
    pub tether_attached: bool,
    pub tether_id: u64,
    pub tether_length: f32,
    pub tether_taut: bool,
    pub tether_anchor_id: u64,
    pub tether_anchor_point: NativeVec3,
    pub tether_local_anchor: NativeVec3,
    pub controlled_velocity: NativeVec3,
    pub external_velocity: NativeVec3,
    pub grounded: bool,
    pub stance: NativeCharacterStance,
    pub jump_buffer_remaining: f32,
    pub coyote_remaining: f32,
    pub landing_lockout_remaining: f32,
    pub support_entity_present: bool,
    pub support_entity: u64,
    pub support_local_anchor: NativeVec3,
    pub support_previous_translation: NativeVec3,
    pub support_previous_rotation: NativeQuat,
    pub support_point_velocity: NativeVec3,
    pub fall_origin_y: f32,
    pub peak_y: f32,
    pub last_command_sequence: u64,
    pub collision_world_hash: u64,
}

/// The current call's support-entity facts. This is deliberately borrowed by
/// value into one controller proposal; spatial sessions never retain product
/// entities or poses.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeCharacterSupport {
    pub present: bool,
    pub lifecycle: NativeCharacterSupportLifecycle,
    pub entity: u64,
    pub transform: NativeTransform,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCharacterShapeConfig {
    pub standing_height: f32,
    pub crouched_height: f32,
    pub radius: f32,
    pub contact_skin: f32,
    pub clearance_padding: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCharacterGroundConfig {
    pub forward_speed: f32,
    pub backward_speed: f32,
    pub strafe_speed: f32,
    pub acceleration: f32,
    pub braking: f32,
    pub friction: f32,
    pub stop_speed: f32,
    pub direction_change_multiplier: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCharacterAirConfig {
    pub maximum_speed: f32,
    pub acceleration: f32,
    pub braking: f32,
    pub wish_speed_cap: f32,
    pub lateral_control: f32,
    pub drag: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCharacterVerticalConfig {
    pub gravity: f32,
    pub terminal_rise_speed: f32,
    pub terminal_fall_speed: f32,
    pub jump_speed: f32,
    pub grounded_downward_bias: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCharacterJumpConfig {
    pub buffer_seconds: f32,
    pub coyote_seconds: f32,
    pub landing_lockout_seconds: f32,
    pub held_input_retriggers: bool,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCharacterSurfaceConfig {
    pub maximum_slope_radians: f32,
    pub slope_hysteresis_radians: f32,
    pub steep_slide_acceleration: f32,
    pub steep_slide_speed: f32,
    pub maximum_step_height: f32,
    pub minimum_step_width: f32,
    pub floor_snap_distance: f32,
    pub floor_snap_speed_limit: f32,
    pub ledge_support_fraction: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCharacterRecoveryConfig {
    pub maximum_distance: f32,
    pub maximum_speed: f32,
    pub normal_nudge: f32,
    pub unresolved_tolerance: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCharacterPlatformConfig {
    pub carry_translation: bool,
    pub carry_rotation: bool,
    pub inherit_departure_velocity: bool,
    pub departure_velocity_factor: f32,
    pub support_loss_grace_seconds: f32,
    pub crush_tolerance: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCharacterExternalMotionConfig {
    pub impulse_scale: f32,
    pub external_decay_per_second: f32,
    pub maximum_external_speed: f32,
    pub authored_mass: f32,
    pub dynamic_impulse_factor: f32,
    pub maximum_dynamic_impulse: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCharacterSolverConfig {
    pub maximum_slide_planes: u32,
    pub maximum_cast_iterations: u32,
    pub maximum_recovery_passes: u32,
    pub maximum_contacts: u32,
    pub maximum_step_attempts: u32,
    pub maximum_displacement_per_step: f32,
    pub maximum_queries_per_step: u32,
}

/// Complete typed tuning for one Engine-owned character proposal. Product code
/// selects tuning; Engine validates and performs the collision solve.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCharacterControllerConfig {
    pub shape: NativeCharacterShapeConfig,
    pub ground: NativeCharacterGroundConfig,
    pub air: NativeCharacterAirConfig,
    pub vertical: NativeCharacterVerticalConfig,
    pub jump: NativeCharacterJumpConfig,
    pub surface: NativeCharacterSurfaceConfig,
    pub recovery: NativeCharacterRecoveryConfig,
    pub platform: NativeCharacterPlatformConfig,
    pub external_motion: NativeCharacterExternalMotionConfig,
    pub solver: NativeCharacterSolverConfig,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeCharacterMovementMode {
    #[default]
    Walking = 0,
    Swimming = 1,
    Climbing = 2,
    Flying = 3,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeCharacterMovementRequest {
    pub mode: NativeCharacterMovementMode,
    pub vertical_intent: f32,
    pub speed: f32,
    pub acceleration: f32,
    pub drag: f32,
    pub minimum: NativeVec3,
    pub maximum: NativeVec3,
    pub gravity_scale: f32,
    pub buoyancy: f32,
    pub climb_reach: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeCharacterMovementFact {
    pub mode: NativeCharacterMovementMode,
    pub immersion: f32,
    pub head_submerged: bool,
    pub climb_attached: bool,
    pub climb_at_bottom: bool,
    pub climb_at_top: bool,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCharacterControllerCommand {
    pub movement: NativeCharacterMovementRequest,
    pub planar_intent: NativeVec2,
    pub heading_yaw_radians: f32,
    pub jump_pressed: bool,
    pub jump_held: bool,
    pub crouch_requested: bool,
    pub external_velocity: NativeVec3,
    pub external_impulse: NativeVec3,
    pub step_seconds: f32,
    pub sequence: u64,
}

/// Complete values borrowed for validation only. The Engine retains neither
/// the configuration nor the command and performs no controller mutation.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCharacterControllerValidationRequest {
    pub config: NativeCharacterControllerConfig,
    pub command: NativeCharacterControllerCommand,
}

/// One product-authored collider borrowed for a single character proposal.
/// Bounds are translation-offset AABBs local to `transform`; rotation is used
/// by platform carry and scale must remain one. The Engine retains neither
/// this value nor the entity it describes after the direct call returns.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeCharacterObstacle {
    pub entity: u64,
    pub transform: NativeTransform,
    pub bounds_min: NativeVec3,
    pub bounds_max: NativeVec3,
    pub collision_enabled: bool,
    pub linear_velocity: NativeVec3,
    pub angular_velocity: NativeVec3,
}

/// One retained collision-resident mesh instance admitted for a single
/// character proposal. The instance identity selects the already-copied
/// Engine Spatial mesh and its current transform; the product supplies only
/// the stable entity identity and current motion facts needed for support and
/// carry. No native handle or product-owned bounds are borrowed here.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeCharacterMeshInstance {
    pub instance: u64,
    pub entity: u64,
    pub linear_velocity: NativeVec3,
    pub angular_velocity: NativeVec3,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCharacterStepRequest {
    pub tether: NativeCharacterTetherRequest,
    pub session: NativeSpatialSessionHandle,
    pub position: NativeVec3,
    pub motion: NativeCharacterMotion,
    pub support: NativeCharacterSupport,
    pub obstacles: *const NativeCharacterObstacle,
    pub obstacles_len: usize,
    pub mesh_instances: *const NativeCharacterMeshInstance,
    pub mesh_instances_len: usize,
    pub config: NativeCharacterControllerConfig,
    pub command: NativeCharacterControllerCommand,
}

/// Captures the latest admitted controller continuation from one Spatial
/// session. `expected_generation` prevents a caller from checkpointing a
/// newer controller result by mistake.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeCharacterContinuationCaptureRequest {
    pub session: NativeSpatialSessionHandle,
    pub expected_generation: u64,
}

/// Durable, copied controller continuation facts. This is a value, never a
/// borrowed result or session handle: product persistence may retain it, then
/// restore it only into a compatible newly-created Spatial session. The
/// source identity and generation are diagnostic provenance from the capture;
/// compatibility is established by the typed config, motion, target-session,
/// and canonical-content checks rather than by resolving an old native handle.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCharacterContinuationCheckpoint {
    pub source_session_identity: u64,
    pub source_generation: u64,
    pub spatial_session_fingerprint: u64,
    pub content_authority_hash: u64,
    pub config_fingerprint: u64,
    pub config: NativeCharacterControllerConfig,
    pub motion: NativeCharacterMotion,
}

/// Restores a copied continuation after the product has restored its authored
/// pose and recreated compatible Spatial content. The checkpoint carries its
/// own controller configuration so the Engine can reject corruption or drift
/// before returning the next-call motion value.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCharacterContinuationRestoreRequest {
    pub session: NativeSpatialSessionHandle,
    pub checkpoint: NativeCharacterContinuationCheckpoint,
}

/// The Engine-confirmed continuation to supply to the next character step.
/// Current support transforms remain product-authored, call-local facts and
/// must be resubmitted with that later step.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeCharacterContinuationRestoreReceipt {
    pub source_generation: u64,
    pub motion: NativeCharacterMotion,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeCharacterContact {
    pub present: bool,
    pub kind: NativeCharacterContactKind,
    pub start_solid: bool,
    pub point: NativeVec3,
    pub normal: NativeVec3,
    pub time_of_impact: f32,
    pub source_kind: NativeCharacterCollisionSourceKind,
    pub source_entity: u64,
    pub source_instance: u64,
    pub source_asset: u64,
    pub source_geometry_hash: u64,
    pub source_voxel_x: i64,
    pub source_voxel_y: i64,
    pub source_voxel_z: i64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeCharacterGround {
    pub present: bool,
    pub point: NativeVec3,
    pub normal: NativeVec3,
    pub snapped_distance: f32,
    pub source_kind: NativeCharacterCollisionSourceKind,
    pub source_entity: u64,
    pub source_instance: u64,
    pub source_asset: u64,
    pub source_geometry_hash: u64,
    pub source_voxel_x: i64,
    pub source_voxel_y: i64,
    pub source_voxel_z: i64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeCharacterFloorProbe {
    pub present: bool,
    pub rejected_hit: NativeCharacterContact,
    pub accepted_support: NativeCharacterGround,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeCharacterStanceFact {
    pub requested: NativeCharacterStance,
    pub accepted: NativeCharacterStance,
    pub blocked: bool,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeCharacterStep {
    pub present: bool,
    pub attempted: bool,
    pub accepted: bool,
    pub rise: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeCharacterPlatform {
    pub present: bool,
    pub entity: u64,
    pub carried_displacement: NativeVec3,
    pub point_velocity: NativeVec3,
    pub departed: bool,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeCharacterControllerReadRequest {
    pub session: NativeSpatialSessionHandle,
}

/// Borrowed, session-scoped diagnostic readout of the latest proposal.
/// Character motion remains product-held continuity returned by every
/// proposal; disposing the session drops this observation only. `contacts`
/// points into Spatial bridge storage and stays valid until the next call on
/// the same Spatial context; the generated managed binding copies it before
/// returning.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCharacterControllerResult {
    pub contacts: *const NativeCharacterContact,
    pub contacts_len: usize,
    pub present: bool,
    pub generation: u64,
    pub entity: u64,
    pub command_sequence: u64,
    pub grounded: bool,
    pub block_count: u32,
    pub collision_world_hash: u64,
    pub recovery_distance: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeCharacterStepReceipt {
    pub movement: NativeCharacterMovementFact,
    pub tether: NativeCharacterTetherFact,
    pub generation: u64,
    /// Always 0: a proposal names no character entity. The product owns its
    /// character's identity, and every obstacle, mesh and support identity
    /// it supplies is its own.
    pub entity: u64,
    pub command_sequence: u64,
    pub transform_before: NativeTransform,
    pub transform: NativeTransform,
    pub motion: NativeCharacterMotion,
    pub wish_velocity: NativeVec3,
    pub displacement: NativeVec3,
    pub contact: NativeCharacterContact,
    pub ground: NativeCharacterGround,
    pub floor_probe: NativeCharacterFloorProbe,
    pub stance: NativeCharacterStanceFact,
    pub step: NativeCharacterStep,
    pub platform: NativeCharacterPlatform,
    pub block_flags: NativeCharacterBlockFlags,
    pub contact_count: u32,
    pub cast_count: u32,
    pub recovery_passes: u32,
    pub recovery_distance: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeNavigationStepRequest {
    pub session: NativeSpatialSessionHandle,
    pub from: NativeVec3,
    pub target: NativeVec3,
    pub max_step_units: f32,
    pub max_visited: u32,
}

/// Borrowed result of one navigation step evaluation. `path` is the full
/// path the step follows and is borrowed like
/// [`NativeNavigationPathResult::path`].
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeNavigationStepResult {
    pub path: *const NativePlanarNavCell,
    pub path_len: usize,
    /// How each path cell is reached from the one before, borrowed like
    /// `path` and as long.
    pub edges: *const NativeNavigationPathEdge,
    pub edges_len: usize,
    pub outcome: NativeNavigationPathOutcome,
    pub next_waypoint: NativeVec3,
    pub next_path_cell: NativePlanarNavCell,
    /// How the next path cell is reached: a jump means the mover jumps now,
    /// standing at its cell's centre.
    pub next_edge_kind: NativeNavigationEdgeKind,
    /// For a jump, seconds after it jumps at which the mover starts holding
    /// toward the next path cell, until it lands; zero otherwise.
    pub next_jump_departure: f32,
    pub reached: u32,
    pub visited: u32,
    pub navigation_revision: u64,
    pub projection_hash: u64,
    pub path_hash: u64,
    /// On `NoPath` or `BudgetExhausted`, the visited cell nearest the goal
    /// and where it stands.
    pub nearest_present: bool,
    pub nearest_cell: NativePlanarNavCell,
    pub nearest: NativeVec3,
}

impl Default for NativeNavigationStepResult {
    fn default() -> Self {
        Self {
            path: std::ptr::null(),
            path_len: 0,
            edges: std::ptr::null(),
            edges_len: 0,
            next_edge_kind: Default::default(),
            next_jump_departure: 0.0,
            outcome: Default::default(),
            next_waypoint: Default::default(),
            next_path_cell: Default::default(),
            reached: 0,
            visited: 0,
            nearest_present: false,
            nearest_cell: Default::default(),
            nearest: Default::default(),
            navigation_revision: 0,
            projection_hash: 0,
            path_hash: 0,
        }
    }
}

/// Bounded world-aligned X/Z observation. Collision is sampled over each cell's
/// full footprint and the explicit Y interval; navigation samples cell centers.
/// Caller-owned colliders are borrowed for this observation only.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSpatialMapRequest {
    pub session: NativeSpatialSessionHandle,
    pub origin: NativeVec3,
    pub cell_size: f64,
    pub columns: u32,
    pub rows: u32,
    pub collision_min_y: f64,
    pub collision_max_y: f64,
    pub navigation_min_y: f64,
    pub navigation_max_y: f64,
    pub entities: *const NativeSpatialEntityCollider,
    pub entities_len: usize,
}

/// No navigation sample means unknown, not traversable. Multiple support levels
/// remain explicit. Collision occupancy is not a character-clearance guarantee.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeSpatialMapCell {
    pub static_collision: bool,
    pub dynamic_collision_count: u32,
    pub first_dynamic_entity: u64,
    pub navigation_samples: u32,
    pub navigation_allowed_samples: u32,
    pub minimum_support_y: f64,
    pub maximum_support_y: f64,
}

/// Row-major cells (+X across, +Z down), copied before the generated call returns.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSpatialMapResult {
    pub cells: *const NativeSpatialMapCell,
    pub cells_len: usize,
    pub projection_identity: u64,
    pub source_revision: u64,
    pub collision_revision: u64,
    pub navigation_revision: u64,
    pub navigation_present: bool,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeCharacterTetherRequest {
    pub enabled: bool,
    pub id: u64,
    pub local_anchor: NativeVec3,
    pub dynamic: bool,
    pub fixed_anchor: NativeVec3,
    pub dynamic_anchor: NativeDynamicsAnchorObservation,
    pub maximum_length: f32,
    pub target_length: f32,
    pub reel_speed: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeCharacterTetherFact {
    pub id: u64,
    pub attached: bool,
    pub released: bool,
    pub invalidated: bool,
    pub taut: bool,
    pub caught: bool,
    pub saturated: bool,
    pub unresolved: bool,
    pub character_point: NativeVec3,
    pub anchor_point: NativeVec3,
    pub maximum_length: f32,
    pub distance: f32,
    pub radial_velocity: f32,
    pub tangential_velocity: NativeVec3,
    pub correction: NativeVec3,
    pub reaction: NativeDynamicsAnchorReaction,
}
