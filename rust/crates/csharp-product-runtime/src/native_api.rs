//! The sole Rust-owned C ABI declaration for trusted NativeAOT products.
//!
//! C# raw declarations and its safe direct facade are generated mechanically
//! from this module. The table names Engine service families; it intentionally
//! has no generic invocation, target strings, capability catalogue, or JSON
//! command protocol.

use std::ffi::c_void;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeVec2 {
    pub x: f32,
    pub y: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeVec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeQuat {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeTransform {
    pub translation: NativeVec3,
    pub rotation: NativeQuat,
    pub scale: NativeVec3,
}

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
#[derive(Debug, Clone, Copy)]
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
}

/// The continuity facts the C# product retains between Engine-owned proposal calls.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeCharacterMotion {
    pub controlled_velocity: NativeVec3,
    pub external_velocity: NativeVec3,
    pub grounded: u32,
    pub stance: u32,
    pub jump_buffer_remaining: f32,
    pub coyote_remaining: f32,
    pub landing_lockout_remaining: f32,
    pub support_entity_present: u32,
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
    pub present: u32,
    pub lifecycle: u32,
    pub entity: u64,
    pub transform: NativeTransform,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCharacterControllerConfig {
    pub standing_height: f32,
    pub crouched_height: f32,
    pub radius: f32,
    pub contact_skin: f32,
    pub forward_speed: f32,
    pub backward_speed: f32,
    pub strafe_speed: f32,
    pub acceleration: f32,
    pub braking: f32,
    pub friction: f32,
    pub gravity: f32,
    pub jump_speed: f32,
    pub maximum_slope_radians: f32,
    pub maximum_step_height: f32,
    pub floor_snap_distance: f32,
    pub maximum_displacement_per_step: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCharacterControllerCommand {
    pub planar_intent: NativeVec2,
    pub heading_yaw_radians: f32,
    pub jump_pressed: u32,
    pub jump_held: u32,
    pub crouch_requested: u32,
    pub reserved: u32,
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
    pub present: u32,
    pub kind: u32,
    pub start_solid: u32,
    pub reserved: u32,
    pub point: NativeVec3,
    pub normal: NativeVec3,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeCharacterGround {
    pub present: u32,
    pub reserved: u32,
    pub point: NativeVec3,
    pub normal: NativeVec3,
    pub snapped_distance: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeCharacterStepReceipt {
    pub transform: NativeTransform,
    pub motion: NativeCharacterMotion,
    pub displacement: NativeVec3,
    pub contact: NativeCharacterContact,
    pub ground: NativeCharacterGround,
    pub stepped: u32,
    pub step_accepted: u32,
    pub cast_count: u32,
    pub recovery_passes: u32,
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
    pub next_waypoint: NativeVec3,
    pub reached: u32,
    pub visited: u32,
    pub path_len: u32,
    pub reserved: u32,
    pub projection_hash: u64,
    pub path_hash: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeAppearanceHandle {
    pub value: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeRenderResourceHandle {
    pub value: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeRngHandle {
    pub value: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeUiStreamHandle {
    pub value: u64,
}

/// One borrowed UTF-8 identity. It is valid only for the immediate direct
/// service call that accepts it.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeUtf8Slice {
    pub bytes: *const u8,
    pub len: usize,
}

/// One file borrowed by trusted product code for the duration of creation.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContentFile {
    pub path: *const u8,
    pub path_len: usize,
    pub bytes: *const u8,
    pub bytes_len: usize,
}

/// One input event borrowed for the duration of a product turn.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeInputEvent {
    pub kind: u32,
    pub edge: u32,
    pub sequence: u64,
    pub x: f32,
    pub y: f32,
    pub label: *const u8,
    pub label_len: usize,
}

/// Explicit turn timing and its borrowed input slice.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeTurnArgs {
    /// 1 realtime (nanoseconds), 2 demand (step), 3 external (step).
    pub kind: u32,
    pub reserved: u32,
    pub observed_time_or_step: u64,
    pub events: *const NativeInputEvent,
    pub event_count: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeLookState {
    pub yaw_radians: f32,
    pub pitch_radians: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeLookConfig {
    pub horizontal_radians_per_unit: f32,
    pub vertical_radians_per_unit: f32,
    pub minimum_pitch_radians: f32,
    pub maximum_pitch_radians: f32,
    pub maximum_delta_radians: f32,
    pub invert_horizontal: u32,
    pub invert_vertical: u32,
    pub wrap_yaw: u32,
    pub reserved: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeLookRequest {
    pub state: NativeLookState,
    pub delta: NativeVec2,
    pub config: NativeLookConfig,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeLookReceipt {
    pub state: NativeLookState,
    pub orientation: NativeQuat,
    pub forward: NativeVec3,
    pub right: NativeVec3,
    pub up: NativeVec3,
}

/// One admitted immutable renderer resource selected by its product content path.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeRenderResourceRequest {
    pub path: NativeUtf8Slice,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeRenderResourceInfo {
    pub handle: NativeRenderResourceHandle,
    /// 1 texture, 2 static mesh.
    pub kind: u32,
    pub byte_length: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePrimitiveAppearanceRequest {
    /// 1 cube, 2 sphere, 3 quad, 4 point.
    pub geometry: u32,
    pub wireframe: u32,
    pub color: NativeColor,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMeshGroup {
    pub material_slot: u32,
    pub start: u32,
    pub count: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStaticMeshAppearanceRequest {
    pub resource: NativeRenderResourceHandle,
    /// 1 packed streams LE v1, 2 v2, 3 v3.
    pub encoding: u32,
    pub vertex_count: u32,
    pub index_count: u32,
    pub positions_byte_offset: u32,
    pub normals_byte_offset: u32,
    pub uvs_byte_offset: u32,
    pub colors_byte_offset: u32,
    pub indices_byte_offset: u32,
    pub bounds_min: NativeVec3,
    pub bounds_max: NativeVec3,
    pub color: NativeColor,
    pub groups: *const NativeMeshGroup,
    pub groups_len: usize,
}

/// Creates a retained visual-only static mesh from an inline `StaticMeshAsset`
/// JSON document already collected from product content.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStaticMeshContentAppearanceRequest {
    pub path: NativeUtf8Slice,
    pub color: NativeColor,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSpriteAppearanceRequest {
    pub texture: NativeRenderResourceHandle,
    pub uv_min: NativeVec2,
    pub uv_max: NativeVec2,
    pub pivot: NativeVec2,
    pub size: NativeVec2,
    /// 0 none, 1 spherical, 2 cylindrical.
    pub billboard: u32,
    pub render_order: i32,
    pub tint: NativeColor,
}

/// One complete renderer-neutral product appearance fact.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAppearanceFact {
    pub object_id: u64,
    pub transform: NativeTransform,
    pub appearance: NativeAppearanceHandle,
    pub visible: u32,
    pub reserved: u32,
}

/// Tags for one borrowed node in a fixed-layout UI value arena.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeStructuredValueKind {
    Null = 0,
    Bool = 1,
    Number = 2,
    String = 3,
    Array = 4,
    Object = 5,
}

/// A node's object key/text ranges refer to `NativeStructuredValue::utf8`.
/// Array/object child ranges refer to its separate edge array, so nested
/// values never depend on incidental node layout. This is presentation data,
/// never an invocation or semantic-program representation.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStructuredValueNode {
    pub kind: u32,
    pub bool_value: u32,
    pub number_value: f64,
    pub key_offset: u32,
    pub key_len: u32,
    pub text_offset: u32,
    pub text_len: u32,
    pub first_edge: u32,
    pub child_count: u32,
}

/// Borrowed structured UI value storage. Rust copies it to `serde_json::Value`
/// before an envelope is staged; neither side retains these pointers.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStructuredValue {
    pub nodes: *const NativeStructuredValueNode,
    pub node_count: usize,
    pub edges: *const u32,
    pub edge_count: usize,
    pub root: u32,
    pub utf8: *const u8,
    pub utf8_len: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeUiProjection {
    pub stream: NativeUiStreamHandle,
    pub sequence: u64,
    pub value: NativeStructuredValue,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeUiStreamRequest {
    pub stream: NativeUtf8Slice,
    pub contract: NativeUtf8Slice,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeKeyedRngRequest {
    pub seed: u64,
    pub scope: NativeUtf8Slice,
    pub key: NativeUtf8Slice,
    pub minimum: i64,
    pub maximum: i64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeKeyedRngReceipt {
    pub value: i64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeScopedRngCreateRequest {
    pub seed: u64,
    pub scope: NativeUtf8Slice,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeScopedRngForkRequest {
    pub parent: NativeRngHandle,
    pub scope: NativeUtf8Slice,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeScopedRngBoundedRequest {
    pub stream: NativeRngHandle,
    pub upper: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeRngValue {
    pub value: u64,
}

pub type NativeIntegrateLook =
    unsafe extern "C" fn(*mut c_void, NativeLookRequest, *mut NativeLookReceipt) -> i32;
pub type NativeCreateSpatialSession = unsafe extern "C" fn(
    *mut c_void,
    NativeSpatialSessionConfig,
    *mut NativeSpatialSessionHandle,
) -> i32;
pub type NativeDestroySpatialSession =
    unsafe extern "C" fn(*mut c_void, NativeSpatialSessionHandle) -> i32;
pub type NativeReplaceCollision = unsafe extern "C" fn(
    *mut c_void,
    *const NativeCollisionReplaceRequest,
    *mut NativeCollisionReplaceReceipt,
) -> i32;
pub type NativeReplaceNavigation = unsafe extern "C" fn(
    *mut c_void,
    *const NativeNavigationReplaceRequest,
    *mut NativeNavigationReplaceReceipt,
) -> i32;
pub type NativeProposeCharacterStep = unsafe extern "C" fn(
    *mut c_void,
    NativeCharacterStepRequest,
    *mut NativeCharacterStepReceipt,
) -> i32;
pub type NativeProposeNavigationStep = unsafe extern "C" fn(
    *mut c_void,
    NativeNavigationStepRequest,
    *mut NativeNavigationStepReceipt,
) -> i32;
pub type NativeOpenRenderResource = unsafe extern "C" fn(
    *mut c_void,
    *const NativeRenderResourceRequest,
    *mut NativeRenderResourceInfo,
) -> i32;
pub type NativeCreatePrimitiveAppearance = unsafe extern "C" fn(
    *mut c_void,
    NativePrimitiveAppearanceRequest,
    *mut NativeAppearanceHandle,
) -> i32;
pub type NativeCreateStaticMeshAppearance = unsafe extern "C" fn(
    *mut c_void,
    *const NativeStaticMeshAppearanceRequest,
    *mut NativeAppearanceHandle,
) -> i32;
pub type NativeCreateStaticMeshContentAppearance = unsafe extern "C" fn(
    *mut c_void,
    *const NativeStaticMeshContentAppearanceRequest,
    *mut NativeAppearanceHandle,
) -> i32;
pub type NativeCreateSpriteAppearance = unsafe extern "C" fn(
    *mut c_void,
    NativeSpriteAppearanceRequest,
    *mut NativeAppearanceHandle,
) -> i32;
pub type NativePublishAppearanceSnapshot =
    unsafe extern "C" fn(*mut c_void, *const NativeAppearanceFact, usize) -> i32;
pub type NativeOpenUiStream = unsafe extern "C" fn(
    *mut c_void,
    *const NativeUiStreamRequest,
    *mut NativeUiStreamHandle,
) -> i32;
pub type NativePublishUiProjection =
    unsafe extern "C" fn(*mut c_void, *const NativeUiProjection) -> i32;
pub type NativeDrawKeyedRng = unsafe extern "C" fn(
    *mut c_void,
    *const NativeKeyedRngRequest,
    *mut NativeKeyedRngReceipt,
) -> i32;
pub type NativeCreateScopedRng = unsafe extern "C" fn(
    *mut c_void,
    *const NativeScopedRngCreateRequest,
    *mut NativeRngHandle,
) -> i32;
pub type NativeForkScopedRng = unsafe extern "C" fn(
    *mut c_void,
    *const NativeScopedRngForkRequest,
    *mut NativeRngHandle,
) -> i32;
pub type NativeDestroyScopedRng = unsafe extern "C" fn(*mut c_void, NativeRngHandle) -> i32;
pub type NativeNextScopedRng =
    unsafe extern "C" fn(*mut c_void, NativeRngHandle, *mut NativeRngValue) -> i32;
pub type NativeNextBoundedScopedRng =
    unsafe extern "C" fn(*mut c_void, NativeScopedRngBoundedRequest, *mut NativeRngValue) -> i32;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeLookApi {
    pub context: *mut c_void,
    pub integrate: NativeIntegrateLook,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSpatialApi {
    pub context: *mut c_void,
    pub create_session: NativeCreateSpatialSession,
    pub destroy_session: NativeDestroySpatialSession,
    pub replace_collision: NativeReplaceCollision,
    pub replace_navigation: NativeReplaceNavigation,
    pub propose_character_step: NativeProposeCharacterStep,
    pub propose_navigation_step: NativeProposeNavigationStep,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeUiApi {
    pub context: *mut c_void,
    pub open_stream: NativeOpenUiStream,
    pub publish_projection: NativePublishUiProjection,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAppearanceApi {
    pub context: *mut c_void,
    pub open_resource: NativeOpenRenderResource,
    pub create_primitive: NativeCreatePrimitiveAppearance,
    pub create_static_mesh: NativeCreateStaticMeshAppearance,
    pub create_static_mesh_from_content: NativeCreateStaticMeshContentAppearance,
    pub create_sprite: NativeCreateSpriteAppearance,
    pub publish_snapshot: NativePublishAppearanceSnapshot,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeRngApi {
    pub context: *mut c_void,
    pub draw_keyed: NativeDrawKeyedRng,
    pub create_scoped: NativeCreateScopedRng,
    pub fork_scoped: NativeForkScopedRng,
    pub destroy_scoped: NativeDestroyScopedRng,
    pub next_u64: NativeNextScopedRng,
    pub next_bounded_u32: NativeNextBoundedScopedRng,
    pub next_bool: NativeNextScopedRng,
}

/// Direct named Engine service families available to trusted NativeAOT code.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeEngineApi {
    pub look: NativeLookApi,
    pub spatial: NativeSpatialApi,
    pub appearance: NativeAppearanceApi,
    pub rng: NativeRngApi,
    pub ui: NativeUiApi,
}

/// Borrowed creation inputs plus the direct Engine API.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeProductCreateArgs {
    pub content: *const NativeContentFile,
    pub content_len: usize,
    pub engine: NativeEngineApi,
}

pub type NativeProductCreate =
    unsafe extern "C" fn(*const NativeProductCreateArgs, *mut *mut c_void) -> i32;
pub type NativeProductAction = unsafe extern "C" fn(*mut c_void) -> i32;
pub type NativeProductTurn = unsafe extern "C" fn(*mut c_void, *const NativeTurnArgs) -> i32;
pub type NativeProductDestroy = unsafe extern "C" fn(*mut c_void);

/// Product functions supplied to Rust by the one NativeAOT bootstrap export.
/// Nullable fields let Rust receive and inspect an initially empty table safely.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeProductApi {
    pub create:
        Option<unsafe extern "C" fn(*const NativeProductCreateArgs, *mut *mut c_void) -> i32>,
    pub start: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    pub turn: Option<unsafe extern "C" fn(*mut c_void, *const NativeTurnArgs) -> i32>,
    pub pause: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    pub resume: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    pub shutdown: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    pub destroy: Option<unsafe extern "C" fn(*mut c_void)>,
}

pub type NativeProductBind = unsafe extern "C" fn(*mut NativeProductApi) -> i32;
