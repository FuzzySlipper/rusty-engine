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
    pub reserved: u32,
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
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeSpatialTriggerReceipt {
    pub tick: u64,
    pub cause: NativeSpatialTriggerCause,
    pub revision: u64,
    pub fact_count: u32,
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
pub struct NativeSpatialTriggerReadReceipt {
    pub trigger: u64,
    pub revision: u64,
    pub overlap_count: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSpatialTriggerOverlapAtRequest {
    pub session: NativeSpatialSessionHandle,
    pub trigger: u64,
    pub index: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeSpatialTriggerOverlapAtReceipt {
    pub present: bool,
    pub trigger: u64,
    pub subject: u64,
    pub revision: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSpatialTriggerFactAtRequest {
    pub session: NativeSpatialSessionHandle,
    pub index: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeSpatialTriggerFactAtReceipt {
    pub present: bool,
    pub enter: bool,
    pub trigger: u64,
    pub subject: u64,
    pub tick: u64,
    pub cause: NativeSpatialTriggerCause,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStaticMeshAsset {
    pub id: u64,
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

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeCollisionReplaceReceipt {
    pub revision_before: u64,
    pub revision_after: u64,
    pub asset_count: u64,
    pub instance_count: u64,
    pub projection_hash: u64,
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

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeNavigationPathReadout {
    pub outcome: NativeNavigationPathOutcome,
    pub kind: NativeNavigationProjectionKind,
    pub visited: u32,
    pub path_len: u32,
    pub navigation_revision: u64,
    pub projection_hash: u64,
    pub path_hash: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeNavigationPathCellAtRequest {
    pub session: NativeSpatialSessionHandle,
    pub index: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeNavigationPathCellAtReceipt {
    pub present: bool,
    pub cell: NativePlanarNavCell,
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

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCharacterControllerCommand {
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

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCharacterStepRequest {
    pub session: NativeSpatialSessionHandle,
    pub position: NativeVec3,
    pub motion: NativeCharacterMotion,
    pub support: NativeCharacterSupport,
    pub config: NativeCharacterControllerConfig,
    pub command: NativeCharacterControllerCommand,
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
pub struct NativeCharacterDynamicImpulse {
    pub entity: u64,
    pub point: NativeVec3,
    pub impulse: NativeVec3,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeCharacterControllerReadRequest {
    pub session: NativeSpatialSessionHandle,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeCharacterContactAtRequest {
    pub session: NativeSpatialSessionHandle,
    pub index: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeCharacterContactAtReceipt {
    pub present: bool,
    pub contact: NativeCharacterContact,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeCharacterDynamicImpulseAtRequest {
    pub session: NativeSpatialSessionHandle,
    pub index: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeCharacterDynamicImpulseAtReceipt {
    pub present: bool,
    pub proposal: NativeCharacterDynamicImpulse,
}

/// Session-scoped diagnostic readout. Character motion remains product-held
/// continuity returned by every proposal; disposing the session drops this
/// last-proposal observation only.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeCharacterControllerReadout {
    pub present: bool,
    pub generation: u64,
    pub entity: u64,
    pub command_sequence: u64,
    pub grounded: bool,
    pub contact_count: u32,
    pub block_count: u32,
    pub dynamic_impulse_count: u32,
    pub collision_world_hash: u64,
    pub recovery_distance: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeCharacterStepReceipt {
    pub generation: u64,
    pub revision_before: u64,
    pub revision_after: u64,
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
    pub dynamic_impulse_count: u32,
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

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeNavigationStepReceipt {
    pub outcome: NativeNavigationPathOutcome,
    pub next_waypoint: NativeVec3,
    pub next_path_cell: NativePlanarNavCell,
    pub reached: u32,
    pub visited: u32,
    pub path_len: u32,
    pub navigation_revision: u64,
    pub projection_hash: u64,
    pub path_hash: u64,
}
