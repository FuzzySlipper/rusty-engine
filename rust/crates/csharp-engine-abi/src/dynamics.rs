use crate::*;

/// Opaque Engine-owned retained dynamics world and body identities. Values are
/// only transport tokens for the generated owner types; product code does not
/// derive meaning from their numeric representation.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeDynamicsWorldHandle {
    pub value: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeDynamicsBodyHandle {
    pub value: u64,
}

/// A non-owning body identity included only in bounded world/contact receipts.
/// It never grants disposal or mutation authority; retained `DynamicsBody`
/// owners remain the only inputs for those operations.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeDynamicsBodyReference {
    pub value: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsWorldConfig {
    pub gravity: NativeVec3,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeAxisLocks {
    pub translation_x: bool,
    pub translation_y: bool,
    pub translation_z: bool,
    pub rotation_x: bool,
    pub rotation_y: bool,
    pub rotation_z: bool,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsBodyConfig {
    pub transform: NativeTransform,
    pub half_extents: NativeVec3,
    pub mass: f32,
    pub axis_locks: NativeAxisLocks,
    pub gravity_scale: f32,
}

/// Complete dynamic-body behavior supported by the current Engine owner.
/// Static and kinematic bodies remain Spatial/character families rather than
/// alternate modes hidden inside this request.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsBodyProperties {
    pub mass: f32,
    pub linear_velocity: NativeVec3,
    pub angular_velocity: NativeVec3,
    pub axis_locks: NativeAxisLocks,
    pub linear_damping: f32,
    pub angular_damping: f32,
    pub gravity_scale: f32,
    pub friction: f32,
    pub restitution: f32,
    pub collision_groups: u32,
    pub collision_mask: u32,
    pub enabled: bool,
    pub sleeping: bool,
    pub continuous_collision: bool,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsCuboidBodyConfig {
    pub transform: NativeTransform,
    pub half_extents: NativeVec3,
    pub properties: NativeDynamicsBodyProperties,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsSphereBodyPropertiesConfig {
    pub transform: NativeTransform,
    pub radius: f32,
    pub properties: NativeDynamicsBodyProperties,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsCapsuleBodyConfig {
    pub transform: NativeTransform,
    pub half_height: f32,
    pub radius: f32,
    pub properties: NativeDynamicsBodyProperties,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsCreateCuboidBodyRequest {
    pub world: NativeDynamicsWorldHandle,
    pub body: NativeDynamicsCuboidBodyConfig,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsCreateSphereBodyPropertiesRequest {
    pub world: NativeDynamicsWorldHandle,
    pub body: NativeDynamicsSphereBodyPropertiesConfig,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsCreateCapsuleBodyRequest {
    pub world: NativeDynamicsWorldHandle,
    pub body: NativeDynamicsCapsuleBodyConfig,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsCreateBodyRequest {
    pub world: NativeDynamicsWorldHandle,
    pub body: NativeDynamicsBodyConfig,
}

/// Dynamic sphere admission deliberately remains separate from the established
/// cuboid request so existing product contracts stay source-compatible.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsSphereBodyConfig {
    pub transform: NativeTransform,
    pub radius: f32,
    pub mass: f32,
    pub axis_locks: NativeAxisLocks,
    pub gravity_scale: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsCreateSphereBodyRequest {
    pub world: NativeDynamicsWorldHandle,
    pub body: NativeDynamicsSphereBodyConfig,
}

/// Attaches a Dynamics world to the current immutable collision projection of
/// an Engine-owned Spatial session. The resulting world keeps that projection
/// snapshot until a later explicit bind; product code never owns the scene.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsWorldCollisionBindingRequest {
    pub world: NativeDynamicsWorldHandle,
    pub spatial_session: NativeSpatialSessionHandle,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsAction {
    pub body: NativeDynamicsBodyHandle,
    pub force: NativeVec3,
    pub torque: NativeVec3,
    pub impulse: NativeVec3,
    pub torque_impulse: NativeVec3,
    pub wake: bool,
}

/// Product code owns when a step occurs and what wrench to submit. Engine
/// owns the admitted simulation and publication of its resulting state.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsStepRequest {
    pub world: NativeDynamicsWorldHandle,
    pub step_seconds: f32,
    pub steps: u32,
    pub actions: *const NativeDynamicsAction,
    pub actions_len: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeDynamicsStepReceipt {
    pub generation: u64,
    pub body_count: u32,
    pub contact_count: u32,
}

/// One deterministic fact from the latest successful step for a body. It is
/// intentionally not a contact enumeration: `contact_count` in the body
/// readout reports the complete count while this exposes only the first fact.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeDynamicsContactFact {
    pub present: bool,
    pub environment: bool,
    pub impulse: NativeVec3,
    pub impulse_magnitude: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsReadRequest {
    pub body: NativeDynamicsBodyHandle,
}

/// Engine-derived mass facts for the admitted dynamic shape. This reports the
/// same shape/mass semantics used by the dynamics bridge without exposing a
/// solver representation or asking C# to reproduce inertia policy.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeMassProperties {
    /// Whether the current Engine shape policy has a derived principal-inertia
    /// readout. Custom inertia belongs to the separately tracked 7219 owner.
    pub available: bool,
    pub mass: f32,
    pub principal_inertia: NativeVec3,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeDynamicsReadout {
    pub transform: NativeTransform,
    pub linear_velocity: NativeVec3,
    pub angular_velocity: NativeVec3,
    pub sleeping: bool,
    pub mass_properties: NativeMassProperties,
    pub contact_count: u32,
    pub first_contact: NativeDynamicsContactFact,
}

/// Whole-body values that can change without replacing the retained shape.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsUpdateBodyRequest {
    pub body: NativeDynamicsBodyHandle,
    pub properties: NativeDynamicsBodyProperties,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsWorldReadRequest {
    pub world: NativeDynamicsWorldHandle,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeDynamicsWorldReadout {
    pub generation: u64,
    pub entity_revision: u64,
    pub body_count: u32,
    pub contact_count: u32,
}

/// Fixed-size indexed queries keep contact and body receipts bounded without
/// exposing retained solver buffers or native memory to product code.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsBodyAtRequest {
    pub world: NativeDynamicsWorldHandle,
    pub index: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeDynamicsBodyAtReceipt {
    pub present: bool,
    pub body: NativeDynamicsBodyReference,
    pub readout: NativeDynamicsReadout,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsContactAtRequest {
    pub world: NativeDynamicsWorldHandle,
    pub index: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeDynamicsContactAtReceipt {
    pub present: bool,
    pub environment: bool,
    pub first: NativeDynamicsBodyReference,
    pub second: NativeDynamicsBodyReference,
    pub impulse: NativeVec3,
    pub impulse_magnitude: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsResetRequest {
    pub body: NativeDynamicsBodyHandle,
    pub transform: NativeTransform,
    pub linear_velocity: NativeVec3,
    pub angular_velocity: NativeVec3,
    pub sleeping: bool,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsReplaceBodyRequest {
    pub body: NativeDynamicsBodyHandle,
    pub replacement: NativeDynamicsBodyConfig,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsReplaceCuboidBodyRequest {
    pub body: NativeDynamicsBodyHandle,
    pub replacement: NativeDynamicsCuboidBodyConfig,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsReplaceSphereBodyRequest {
    pub body: NativeDynamicsBodyHandle,
    pub replacement: NativeDynamicsSphereBodyPropertiesConfig,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsReplaceCapsuleBodyRequest {
    pub body: NativeDynamicsBodyHandle,
    pub replacement: NativeDynamicsCapsuleBodyConfig,
}
