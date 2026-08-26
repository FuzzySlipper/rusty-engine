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
