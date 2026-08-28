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

/// Purpose-neutral, caller-owned kinematic integration. It owns no bodies,
/// world, scheduling, or product-state publication.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeKinematicApi {
    pub context: *mut c_void,
    pub integrate: NativeKinematicIntegrate,
    pub integrate_spatial: NativeKinematicIntegrateSpatial,
}
