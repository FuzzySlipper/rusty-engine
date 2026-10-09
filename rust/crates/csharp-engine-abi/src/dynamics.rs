use crate::*;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsTetherConfig {
    pub id: u64,
    pub maximum_length: f32,
    pub target_length: f32,
    pub reel_speed: f32,
    pub contacts_enabled: bool,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsFixedTetherRequest {
    pub world: NativeDynamicsWorldHandle,
    pub body: NativeDynamicsBodyHandle,
    pub local_anchor: NativeVec3,
    pub world_anchor: NativeVec3,
    pub config: NativeDynamicsTetherConfig,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsBodyTetherRequest {
    pub world: NativeDynamicsWorldHandle,
    pub first: NativeDynamicsBodyHandle,
    pub second: NativeDynamicsBodyHandle,
    pub first_anchor: NativeVec3,
    pub second_anchor: NativeVec3,
    pub config: NativeDynamicsTetherConfig,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsTetherRequest {
    pub world: NativeDynamicsWorldHandle,
    pub id: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeDynamicsTetherReadout {
    pub present: bool,
    pub invalidated: bool,
    pub simulated: bool,
    pub first: NativeVec3,
    pub second: NativeVec3,
    pub maximum_length: f32,
    pub target_length: f32,
    pub slack_distance: f32,
    pub distance: f32,
    pub taut: bool,
    pub caught: bool,
    /// Sampled terminal solver-substep force in N; not a peak or breaking load.
    pub force_proxy: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeDynamicsTetherReleaseReceipt {
    pub released: bool,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsChainLengthRequest {
    pub world: NativeDynamicsWorldHandle,
    pub id: u64,
    pub target_length: f32,
    /// Total rope length change per second, distributed equally across links.
    pub reel_speed: f32,
}

/// Engine-owned sphere-bead chain. The first point is anchored; the final bead is free.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsChainConfig {
    pub id: u64,
    pub bead_count: u32,
    pub link_length: f32,
    pub radius: f32,
    pub properties: NativeDynamicsBodyProperties,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsFixedChainRequest {
    pub world: NativeDynamicsWorldHandle,
    pub anchor: NativeVec3,
    /// Initial final bead position; intermediate beads are evenly spaced.
    pub end: NativeVec3,
    pub config: NativeDynamicsChainConfig,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsBodyChainRequest {
    pub world: NativeDynamicsWorldHandle,
    pub body: NativeDynamicsBodyHandle,
    pub local_anchor: NativeVec3,
    pub end: NativeVec3,
    pub config: NativeDynamicsChainConfig,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsChainRequest {
    pub world: NativeDynamicsWorldHandle,
    pub id: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsChainPointRequest {
    pub world: NativeDynamicsWorldHandle,
    pub id: u64,
    pub index: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeDynamicsChainReadout {
    pub present: bool,
    pub invalidated: bool,
    pub simulated: bool,
    pub point_count: u32,
    pub effective_length: f32,
    pub target_length: f32,
    pub force_proxy: f32,
    pub taut_links: u32,
    pub caught_links: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeDynamicsChainPointReadout {
    pub present: bool,
    pub position: NativeVec3,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeDynamicsChainReleaseReceipt {
    pub released: bool,
    pub removed_bodies: u32,
}

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

/// Selects whether the Engine derives inertia from the admitted primitive and
/// total mass or uses the authored local tuple below.
#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeDynamicsMassPolicyKind {
    #[default]
    DeriveFromShapeAndMass = 0,
    Explicit = 1,
}

/// Pointer-free local mass properties for an authored dynamic body. `mass` is
/// deliberately kept on each body/config as the sole authoritative total
/// mass; this tuple supplies only the remaining properties.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeDynamicsExplicitMassProperties {
    pub center_of_mass: NativeVec3,
    pub principal_inertia: NativeVec3,
    pub principal_inertia_local_frame: NativeQuat,
}

/// Fixed-size mass policy passed by value through the generated ABI. The
/// explicit tuple is ignored for the derived kind.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeDynamicsMassPolicy {
    pub kind: NativeDynamicsMassPolicyKind,
    pub explicit: NativeDynamicsExplicitMassProperties,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsBodyConfig {
    pub transform: NativeTransform,
    pub half_extents: NativeVec3,
    pub properties: NativeDynamicsBodyProperties,
}

/// Complete dynamic-body behavior supported by the current Engine owner.
/// Static and kinematic bodies remain Spatial/character families rather than
/// alternate modes hidden inside this request.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsBodyProperties {
    pub mass: f32,
    pub mass_policy: NativeDynamicsMassPolicy,
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
    pub mass_policy: NativeDynamicsMassPolicy,
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

/// Applies one committed Spatial world-origin rebase to a Dynamics world: its
/// bodies and fixed rope anchors move with the origin, and the world binds the
/// session's rebased collision scene.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsRebaseWorldOriginRequest {
    pub world: NativeDynamicsWorldHandle,
    pub spatial_session: NativeSpatialSessionHandle,
    pub receipt: NativeWorldOriginCommitReceipt,
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
    pub rope_substeps: u32,
    pub rope_iterations: u32,
    pub rope_link_count: u32,
    /// Link/substep/iteration work for all requested ticks (zero without ropes).
    pub rope_solver_link_steps: u32,
    pub generation: u64,
    pub body_count: u32,
    pub contact_count: u32,
}

/// One explicit retained body selected for correlated post-step readout.
/// Request order is preserved in the returned result so product-side adapters
/// can retain their own identity mapping without a native entity mirror.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsStepAndReadRequest {
    pub world: NativeDynamicsWorldHandle,
    pub step_seconds: f32,
    pub steps: u32,
    pub actions: *const NativeDynamicsAction,
    pub actions_len: usize,
    pub bodies: *const NativeDynamicsBodyHandle,
    pub bodies_len: usize,
}

/// One retained body and its current readout.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeDynamicsBodyFact {
    pub body: NativeDynamicsBodyReference,
    pub readout: NativeDynamicsReadout,
}

/// Borrowed result of one completed Dynamics step/read. `bodies` points into
/// Dynamics bridge storage and stays valid until the next call on the same
/// Dynamics context; the generated managed binding copies it before returning.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsStepAndReadResult {
    pub bodies: *const NativeDynamicsBodyFact,
    pub bodies_len: usize,
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
    pub policy: NativeDynamicsMassPolicyKind,
    pub center_of_mass: NativeVec3,
    pub principal_inertia_local_frame: NativeQuat,
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

/// Borrowed readout of every retained body and solver contact in one world.
/// Both collections point into Dynamics bridge storage and stay valid until
/// the next call on the same Dynamics context.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsWorldResult {
    pub bodies: *const NativeDynamicsBodyFact,
    pub bodies_len: usize,
    pub contacts: *const NativeDynamicsContact,
    pub contacts_len: usize,
    pub generation: u64,
}

/// One solver contact from the latest successful step. `second` is zero for a
/// contact with the bound static environment.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeDynamicsContact {
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

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsRopeSolverRequest {
    pub world: NativeDynamicsWorldHandle,
    pub substeps: u32,
    pub iterations: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsObserveAnchorRequest {
    pub world: NativeDynamicsWorldHandle,
    pub body: NativeDynamicsBodyHandle,
    pub local_anchor: NativeVec3,
}

/// Copied point observation; references do not confer body disposal ownership.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeDynamicsAnchorObservation {
    pub valid: bool,
    pub body: NativeDynamicsBodyReference,
    pub local_anchor: NativeVec3,
    pub point: NativeVec3,
    pub point_velocity: NativeVec3,
    pub center_of_mass: NativeVec3,
    /// Point velocity per unit X/Y/Z world impulse, including angular response.
    pub response_x: NativeVec3,
    pub response_y: NativeVec3,
    pub response_z: NativeVec3,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeDynamicsAnchorReaction {
    pub present: bool,
    pub anchor: NativeDynamicsAnchorObservation,
    pub impulse: NativeVec3,
}

/// One Dynamics update that also applies point reactions, such as a
/// character's pull on a rope anchor, at their observed points.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsStepWithReactionsRequest {
    pub world: NativeDynamicsWorldHandle,
    pub step_seconds: f32,
    pub steps: u32,
    pub actions: *const NativeDynamicsAction,
    pub actions_len: usize,
    pub reactions: *const NativeDynamicsAnchorReaction,
    pub reactions_len: usize,
}

/// How a limited joint may turn. Angles are radians from the rest
/// relation of its two frames.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeDynamicsJointKind {
    /// About the frames' X axis only, between `min_angle` and `max_angle`.
    Hinge = 1,
    /// The second frame's X axis within `swing_angle` of the first's,
    /// twisting about it between `min_angle` and `max_angle`.
    Cone = 2,
}

/// A joint's limits and damping. `damping` (N·m·s per radian) resists
/// turning about the hinge or twist axis.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsJointLimits {
    pub kind: NativeDynamicsJointKind,
    pub min_angle: f32,
    pub max_angle: f32,
    pub swing_angle: f32,
    pub damping: f32,
}

/// Creates the limited joint `id` between two bodies of `world`, or replaces
/// it. Each frame is body-local; its X axis is the hinge or twist axis.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsJointRequest {
    pub world: NativeDynamicsWorldHandle,
    pub id: u64,
    pub first: NativeDynamicsBodyHandle,
    pub second: NativeDynamicsBodyHandle,
    pub first_anchor: NativeVec3,
    pub first_rotation: NativeQuat,
    pub second_anchor: NativeVec3,
    pub second_rotation: NativeQuat,
    pub limits: NativeDynamicsJointLimits,
    pub contacts_enabled: bool,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsJointRemoveRequest {
    pub world: NativeDynamicsWorldHandle,
    pub id: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeDynamicsJointReleaseReceipt {
    pub released: bool,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeDynamicsRagdollHandle {
    pub value: u64,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeDynamicsRagdollShape {
    /// A capsule of `radius` from the joint to the bone's end.
    Capsule = 1,
    /// A box as long as the bone, `radius` half wide (joint X) and
    /// `half_depth` half deep (joint Z).
    Box = 2,
}

/// One simulated bone: the rig joint (`Animation.ReadJoints` index) its body
/// hangs from. The body spans to `end_joint` where it is at rest, or with
/// `end_joint` `u32::MAX` `length` metres along the joint's +Y.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsRagdollBone {
    pub joint: u32,
    pub end_joint: u32,
    pub length: f32,
    pub shape: NativeDynamicsRagdollShape,
    pub radius: f32,
    pub half_depth: f32,
    pub mass: f32,
}

/// A limited joint at the `child` bone's joint, linking it to the `parent`
/// bone (indices into the bones). `axis` is the hinge or twist axis in the
/// child joint's frame at rest; limits are from the rest pose.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsRagdollLink {
    pub parent: u32,
    pub child: u32,
    pub axis: NativeVec3,
    pub limits: NativeDynamicsJointLimits,
}

/// Spawns a ragdoll of `instance` in `world` at the pose drawn for the
/// previous call, moving as that pose moved. The instance must report its
/// joints (`Animation.SetPose` with `ReportJoints`). Linked bones never
/// collide with each other; other bones follow `collision_groups` and
/// `collision_mask`. `blend` (0 to 1) mixes the bodies over the clips.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsRagdollRequest {
    pub world: NativeDynamicsWorldHandle,
    pub instance: NativeAnimationInstanceHandle,
    pub bones: *const NativeDynamicsRagdollBone,
    pub bones_len: usize,
    pub links: *const NativeDynamicsRagdollLink,
    pub links_len: usize,
    pub collision_groups: u32,
    pub collision_mask: u32,
    pub friction: f32,
    pub restitution: f32,
    pub linear_damping: f32,
    pub angular_damping: f32,
    pub blend: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsRagdollBlendRequest {
    pub ragdoll: NativeDynamicsRagdollHandle,
    pub blend: f32,
}

/// An impulse (N·s) at a world point of one bone's body, as from a hit.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsRagdollImpulseRequest {
    pub ragdoll: NativeDynamicsRagdollHandle,
    pub bone: u32,
    pub point: NativeVec3,
    pub impulse: NativeVec3,
}

/// Each bone's joint placement in the world, from its body, in bone order.
/// `resting` is true once every body sleeps. `bones` points into Dynamics
/// bridge storage until the next call on the same context.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsRagdollResult {
    pub bones: *const NativeTransform,
    pub bones_len: usize,
    pub resting: bool,
    pub blend: f32,
}
