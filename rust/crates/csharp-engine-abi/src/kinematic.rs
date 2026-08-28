use crate::{NativeSpatialSessionHandle, NativeVec3};
use std::ffi::c_void;

/// Caller-owned collision selection. `SpatialSession` is deliberately the
/// only scene-backed option: collision queries and backend objects never
/// cross the product boundary.
#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeKinematicCollisionMode {
    #[default]
    None = 0,
    SpatialSession = 1,
}

/// A positive duration expressed as a product-selected tick count and tick
/// duration. The Engine validates it before every integration call.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativePhysicsStep {
    pub ticks: u64,
    pub seconds_per_tick: f32,
}

/// Call-local world facts used by a kinematic integration.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativePhysicsWorld {
    pub gravity: NativeVec3,
}

/// Caller-owned kinematic state. The returned integration facts are not
/// retained or published by the Engine.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeKinematicBody {
    pub position: NativeVec3,
    pub velocity: NativeVec3,
    pub acceleration: NativeVec3,
    pub gravity_scale: f32,
    pub collision_mode: NativeKinematicCollisionMode,
}

/// Axis-aligned collision bounds for an explicit Spatial-session integration.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeKinematicShape {
    pub half_extents: NativeVec3,
}

/// Pure no-collision integration. The product retains both inputs and output.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeKinematicIntegrationRequest {
    pub body: NativeKinematicBody,
    pub world: NativePhysicsWorld,
    pub step: NativePhysicsStep,
}

/// Integration against the immutable voxel/static-mesh snapshot currently
/// owned by one existing Spatial session.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeKinematicSpatialIntegrationRequest {
    pub session: NativeSpatialSessionHandle,
    pub body: NativeKinematicBody,
    pub world: NativePhysicsWorld,
    pub step: NativePhysicsStep,
    pub shape: NativeKinematicShape,
}

/// Complete call-local integration facts, including all independently blocked
/// axes. No result field represents retained native state.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeIntegrationResult {
    pub previous_position: NativeVec3,
    pub next_position: NativeVec3,
    pub previous_velocity: NativeVec3,
    pub next_velocity: NativeVec3,
    pub elapsed_seconds: f32,
    pub blocked_x: bool,
    pub blocked_y: bool,
    pub blocked_z: bool,
}

/// One caller-owned row projected into the call-local kinematic motion phase.
/// The service copies it into a temporary EntityState and never retains an ECS
/// mirror, scheduler, or product entity state.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeKinematicMotionEntityRow {
    pub entity_id: u64,
    pub transform: crate::NativeTransform,
    pub half_extents: NativeVec3,
    pub velocity: NativeVec3,
    pub collision_enabled: bool,
    pub collision_static: bool,
}

/// Fixed axis order used by the existing Engine kinematic motion system.
#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeKinematicMotionAxis {
    #[default]
    X = 0,
    Y = 1,
    Z = 2,
}

/// One ordered fact emitted by a call-local kinematic motion phase.
#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeKinematicMotionFactKind {
    #[default]
    Moved = 0,
    Blocked = 1,
}

/// A fixed-shape moved or blocked fact. Moved facts use the before/after
/// fields; blocked facts use axis and attempted_delta.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeKinematicMotionFact {
    pub entity_id: u64,
    pub kind: NativeKinematicMotionFactKind,
    pub axis: NativeKinematicMotionAxis,
    pub before: NativeVec3,
    pub after: NativeVec3,
    pub attempted_delta: f32,
}

/// One changed selected-body candidate. The product decides whether and when
/// to publish these caller-owned Transform and velocity values.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeKinematicMotionCandidate {
    pub entity_id: u64,
    pub before_transform: crate::NativeTransform,
    pub after_transform: crate::NativeTransform,
    pub before_velocity: NativeVec3,
    pub after_velocity: NativeVec3,
}

/// Borrowed bounded input for one call-local KinematicMotionSystem phase.
/// `selection_present` selects `run_selected`; when false the system runs all
/// projected bodies and selected_entity_ids must be empty.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeKinematicMotionRequest {
    pub session: NativeSpatialSessionHandle,
    pub delta_seconds: f32,
    pub rows: *const NativeKinematicMotionEntityRow,
    pub rows_len: usize,
    pub selection_present: bool,
    pub selected_entity_ids: *const u64,
    pub selected_entity_ids_len: usize,
}

/// Typed owner for one bounded call-local kinematic-motion result lease.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeKinematicMotionLeaseHandle {
    pub value: u64,
}

/// Temporary Engine-owned backing for one completed call-local motion phase.
/// Generated C# copies all rows and metadata, then releases this exact lease.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeKinematicMotionLease {
    pub handle: NativeKinematicMotionLeaseHandle,
    pub candidates: *const NativeKinematicMotionCandidate,
    pub candidates_len: usize,
    pub facts: *const NativeKinematicMotionFact,
    pub facts_len: usize,
    pub bodies_considered: u64,
    pub moved_bodies: u64,
    pub blocked_axes: u64,
    pub revision_before: u64,
    pub revision_after: u64,
}

/// Stable non-success status values for `NativeKinematicApi` operations.
/// `1` remains ABI success. `0` is reserved for boundary/service failures;
/// these values preserve each `engine_spatial::PhysicsError` in generated
/// `EngineCallException.Status` without a retained diagnostic lease.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeKinematicErrorStatus {
    InvalidStep = 101,
    StepOverflow = 102,
    CollisionQueryRequired = 103,
    InvalidShape = 104,
    NonFiniteInput = 105,
    InvalidMotionDelta = 106,
    InvalidMotionRows = 107,
    MotionBatchRejected = 108,
}

pub type NativeKinematicIntegrate = unsafe extern "C" fn(
    *mut c_void,
    NativeKinematicIntegrationRequest,
    *mut NativeIntegrationResult,
) -> i32;
pub type NativeKinematicIntegrateSpatial = unsafe extern "C" fn(
    *mut c_void,
    NativeKinematicSpatialIntegrationRequest,
    *mut NativeIntegrationResult,
) -> i32;
pub type NativeRunKinematicMotion = unsafe extern "C" fn(
    *mut c_void,
    *const NativeKinematicMotionRequest,
    *mut NativeKinematicMotionLease,
) -> i32;
pub type NativeDestroyKinematicMotionLease =
    unsafe extern "C" fn(*mut c_void, NativeKinematicMotionLeaseHandle) -> i32;

/// Purpose-neutral, caller-owned kinematic integration. It owns no bodies,
/// world, scheduling, or product-state publication.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeKinematicApi {
    pub context: *mut c_void,
    pub integrate: NativeKinematicIntegrate,
    pub integrate_spatial: NativeKinematicIntegrateSpatial,
    pub run_motion: NativeRunKinematicMotion,
    pub destroy_motion_lease: NativeDestroyKinematicMotionLease,
}
