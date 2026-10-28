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
    pub water: NativeDynamicsWaterFact,
}

/// A flat stretch of water: a horizontal surface over a rectangle of x and z,
/// with a horizontal current. Densities are mass per unit volume (1000 for
/// fresh water in kilograms and metres); drags are the rates, per second at
/// full immersion, at which a body's velocity relative to the current and its
/// spin decay.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeDynamicsWaterPlane {
    pub min_x: f32,
    pub min_z: f32,
    pub max_x: f32,
    pub max_z: f32,
    pub surface: f32,
    pub current_x: f32,
    pub current_z: f32,
    pub density: f32,
    pub linear_drag: f32,
    pub angular_drag: f32,
}

/// A water field as Dynamics water: as each step begins, every floating body
/// over the field's wet cells takes the surface and current its latest
/// snapshot reports there.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsWaterFieldEntry {
    pub field: NativeWaterFieldHandle,
    pub density: f32,
    pub linear_drag: f32,
    pub angular_drag: f32,
}

/// Replaces a world's water and the bodies it acts on. A floating body takes
/// the first water field whose wet cells lie under its centre, else the
/// first plane whose rectangle holds its centre; the water does not feel the
/// body.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsWaterRequest {
    pub world: NativeDynamicsWorldHandle,
    pub planes: *const NativeDynamicsWaterPlane,
    pub planes_len: usize,
    pub fields: *const NativeDynamicsWaterFieldEntry,
    pub fields_len: usize,
    pub bodies: *const NativeDynamicsBodyHandle,
    pub bodies_len: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeDynamicsWaterReceipt {
    pub planes: u32,
    pub fields: u32,
    pub bodies: u32,
}

/// What the water did to a body over its latest step, averaged over the
/// step's substeps; all zero for a body the water does not act on.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeDynamicsWaterFact {
    pub floating: bool,
    /// The part of the body under the surface at its last substep, 0 to 1.
    pub submerged_fraction: f32,
    pub buoyancy: NativeVec3,
    pub drag: NativeVec3,
    /// The water's torque about the centre of mass: the buoyant force's
    /// moment and the angular drag.
    pub torque: NativeVec3,
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
    Hinge = 0,
    /// The second frame's X axis within `swing_angle` of the first's,
    /// twisting about it between `min_angle` and `max_angle`.
    Cone = 1,
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
    Capsule = 0,
    /// A box as long as the bone, `radius` half wide (joint X) and
    /// `half_depth` half deep (joint Z).
    Box = 1,
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
/// previous call, moving as that pose moved. The instance must have been
/// published a call before, so it has a pose. Linked bones never
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

/// How strongly one link turns its child bone toward the target pose: a
/// spring of `stiffness` (N·m per radian) with `damping` (N·m·s per
/// radian), never with more than `max_torque` (N·m) about each axis. All
/// zero leaves the link limp.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsRagdollLinkDrive {
    pub stiffness: f32,
    pub damping: f32,
    pub max_torque: f32,
}

/// Whether a driven ragdoll's root bones (those no link has as its child)
/// simulate, or follow the target pose kinematically.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeDynamicsRagdollRoot {
    Simulated = 0,
    Kinematic = 1,
}

/// Drives a ragdoll's links with motors toward its instance's pose after
/// the clips, IK and pose overrides (not its own layer), each step. `drives`
/// has one entry per link, in the links' order; none stops driving and
/// simulates the root. Set again to change strengths while it runs.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsRagdollDriveRequest {
    pub ragdoll: NativeDynamicsRagdollHandle,
    pub drives: *const NativeDynamicsRagdollLinkDrive,
    pub drives_len: usize,
    pub root: NativeDynamicsRagdollRoot,
}

/// An impulse (N·s) at a world point of one body, as from a hit or a
/// contact: it changes the body's linear and angular velocity at once and
/// wakes it; the next step moves it.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsPointImpulseRequest {
    pub body: NativeDynamicsBodyHandle,
    pub point: NativeVec3,
    pub impulse: NativeVec3,
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

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeDynamicsSeveredPartHandle {
    pub value: u64,
}

/// Cuts `bone.joint` and its descendants off `instance` and spawns them in
/// `world` as one body shaped by `bone`, at the pose drawn for the previous
/// call. The instance must have been published a call before, so it has a
/// pose. The piece moves as that pose moved, plus
/// `linear_velocity` (m/s) and `angular_velocity` (rad/s, world axes).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsSeveredPartRequest {
    pub world: NativeDynamicsWorldHandle,
    pub instance: NativeAnimationInstanceHandle,
    pub bone: NativeDynamicsRagdollBone,
    pub linear_velocity: NativeVec3,
    pub angular_velocity: NativeVec3,
    pub collision_groups: u32,
    pub collision_mask: u32,
    pub friction: f32,
    pub restitution: f32,
    pub linear_damping: f32,
    pub angular_damping: f32,
}

/// Where a severed part's cut joint is in the world, from its body, and
/// whether the body sleeps.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsSeveredPartReadout {
    pub placement: NativeTransform,
    pub resting: bool,
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

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeDynamicsSoftBodyHandle {
    pub value: u64,
}

/// What a soft body is made of.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeDynamicsSoftBodyShape {
    /// A sheet of `resolution_x` columns × `resolution_y` rows of particles
    /// at `origin + column * across + row * down`, with structural, shear
    /// and bending edges. Particle `column * rows + row`; UVs run 0 to 1
    /// across the columns and down the rows.
    Cloth = 0,
    /// A solid box of tetrahedra about `origin`, `across` its half extents,
    /// on a grid of `resolution_x/y/z` particles per axis.
    Box = 1,
    /// A hollow icosphere about `origin` of `radius`, refined
    /// `resolution_x` times (0 to 6), with bending across its edges.
    Shell = 2,
    /// A triangle mesh whose vertices are the particles (`positions`,
    /// `uvs`, three `indices` per triangle): a banner, a cloak, a shell of
    /// any shape.
    Surface = 3,
    /// Tetrahedra over the particles (four `indices` each); the drawn
    /// surface is their boundary.
    Solid = 4,
    /// A closed, outward-wound triangle mesh filled with tetrahedra of
    /// about `cell_size`. The mesh rides the cells and is what is drawn.
    Filled = 5,
}

/// How cells resist deformation.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeDynamicsSoftBodyCellModel {
    /// Each cell keeps its volume; with the edges, a jelly.
    Volume = 0,
    /// Linear elasticity (Young's modulus, Poisson's ratio) in each cell's
    /// own frame, recovering from inverted cells.
    Corotational = 1,
    /// Stiffer under compression, softer under stretch; twice the cost.
    NeoHookean = 2,
}

/// How a soft body's elasticity is solved.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeDynamicsSoftBodySolver {
    /// Constraints swept with contacts each substep: cheap; a stiff body
    /// keeps some give.
    Constraints = 0,
    /// Implicit finite elements over the whole body: stiff bodies hold
    /// their shape, at a factorization per step.
    Fem = 1,
}

/// A constraint family's stiffness: how fast it springs back (Hz) and its
/// damping ratio (1 is critical), independent of mass and step length.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsSoftness {
    pub frequency: f32,
    pub damping_ratio: f32,
}

/// How a soft body resists, yields and tears. `edge` springs hold edge
/// lengths, `bend` resists folding, `volume` keeps cells' and closed
/// pieces' volumes, `shape_matching` pulls toward the rest shape. Elastic
/// cells take `young_modulus` (Pa), `poisson_ratio` (below 0.5) and
/// `elastic_damping_ratio`. `deformation_damping` (per second) damps wobble
/// but not the body's motion. Past `plastic_yield` (cells) or
/// `edge_plastic_yield` (edges) the rest shape flows toward the current one
/// at the creep rate, up to the max. A positive `tear_strain` (stretch over
/// rest length) or `tear_force` (N) tears elements; zero never tears.
/// `max_tears_per_step` 0 is unlimited; `min_piece` 0 is Rapier's default
/// smallest piece.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsSoftBodyMaterial {
    pub edge: NativeDynamicsSoftness,
    pub bend: NativeDynamicsSoftness,
    pub volume: NativeDynamicsSoftness,
    pub shape_matching: NativeDynamicsSoftness,
    pub young_modulus: f32,
    pub poisson_ratio: f32,
    pub elastic_damping_ratio: f32,
    pub deformation_damping: f32,
    pub plastic_yield: f32,
    pub plastic_creep: f32,
    pub plastic_max: f32,
    pub edge_plastic_yield: f32,
    pub edge_plastic_creep: f32,
    pub edge_plastic_max: f32,
    pub tear_strain: f32,
    pub tear_force: f32,
    pub tear_smoothing: f32,
    pub interior_strength: f32,
    pub max_tears_per_step: u32,
    pub min_piece: u32,
}

/// Creates a soft body in `world`. The shape reads the fields its
/// `NativeDynamicsSoftBodyShape` names; positions are in the world. `mass`
/// (kg) spreads evenly over the particles. `particle_radius` is the
/// surface's contact thickness (0: half the mean edge). `pinned` particles
/// are held where they are. `self_contacts` lets the surface collide with
/// itself. `mesh_collision` makes a filled mesh meet the world through its
/// own triangles (which must be at least as fine as its cells) rather than
/// through its cells' boundary. `extra_substeps` and `extra_iterations`
/// buy accuracy for the body and what it touches.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsSoftBodyRequest {
    pub world: NativeDynamicsWorldHandle,
    pub shape: NativeDynamicsSoftBodyShape,
    pub origin: NativeVec3,
    pub across: NativeVec3,
    pub down: NativeVec3,
    pub radius: f32,
    pub cell_size: f32,
    pub resolution_x: u32,
    pub resolution_y: u32,
    pub resolution_z: u32,
    pub positions: *const NativeVec3,
    pub positions_len: usize,
    pub uvs: *const NativeVec2,
    pub uvs_len: usize,
    pub indices: *const u32,
    pub indices_len: usize,
    pub pinned: *const u32,
    pub pinned_len: usize,
    pub mass: f32,
    pub particle_radius: f32,
    pub linear_damping: f32,
    pub gravity_scale: f32,
    pub cell_model: NativeDynamicsSoftBodyCellModel,
    pub solver: NativeDynamicsSoftBodySolver,
    pub material: NativeDynamicsSoftBodyMaterial,
    pub volume_preservation: bool,
    pub volume_factor: f32,
    pub shape_matching: bool,
    pub self_contacts: bool,
    pub mesh_collision: bool,
    pub collision_groups: u32,
    pub collision_mask: u32,
    pub friction: f32,
    pub restitution: f32,
    pub extra_substeps: u32,
    pub extra_iterations: u32,
    pub can_sleep: bool,
}

/// Soft-body settings of a world. While it holds a soft body, each step is
/// split into `substeps` (at least ropes' substeps when it also holds
/// ropes). A body hit fast asks for up to `max_extra_substeps` more.
/// `contact_stiffening` stiffens soft contacts over rigid ones (Rapier's
/// default is 4). A constraint strained past `resweep_strain` is solved
/// again after the contacts (default 0.75).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsSoftBodySettingsRequest {
    pub world: NativeDynamicsWorldHandle,
    pub substeps: u32,
    pub max_extra_substeps: u32,
    pub contact_stiffening: f32,
    pub resweep_strain: f32,
}

/// Changes a live soft body's material and volume preservation in place.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsSoftBodyMaterialRequest {
    pub soft_body: NativeDynamicsSoftBodyHandle,
    pub material: NativeDynamicsSoftBodyMaterial,
    pub volume_preservation: bool,
    pub volume_factor: f32,
}

/// Holds `particles` where they are, and releases the particles pinned
/// before that are not listed.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsSoftBodyPinsRequest {
    pub soft_body: NativeDynamicsSoftBodyHandle,
    pub particles: *const u32,
    pub particles_len: usize,
}

/// A particle driven to a world `position` by the end of the next step.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsSoftBodyTarget {
    pub particle: u32,
    pub position: NativeVec3,
}

/// Drives the targets' particles along a straight path to their positions
/// over the next step; they push what they meet and nothing pushes them.
/// Driven particles not listed again go free (a pinned one holds where it
/// was left). Set the targets each step to move them along a path.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsSoftBodyTargetsRequest {
    pub soft_body: NativeDynamicsSoftBodyHandle,
    pub targets: *const NativeDynamicsSoftBodyTarget,
    pub targets_len: usize,
}

/// A particle held both ways to the point of `body` where the particle is
/// when attached.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsSoftBodyBodyAttachment {
    pub particle: u32,
    pub body: NativeDynamicsBodyReference,
}

/// Replaces a soft body's body attachments. The body and the cloth pull
/// on each other; a destroyed body's attachments end.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsSoftBodyBodyAttachmentsRequest {
    pub soft_body: NativeDynamicsSoftBodyHandle,
    pub attachments: *const NativeDynamicsSoftBodyBodyAttachment,
    pub attachments_len: usize,
}

/// A particle driven to `offset` in a rig joint's frame (`Animation.ReadJoints`
/// index), as the joint is posed.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsSoftBodyJointAttachment {
    pub particle: u32,
    pub joint: u32,
    pub offset: NativeVec3,
}

/// Replaces a soft body's joint attachments to `instance`: before each step
/// each particle is driven where its joint's offset is in the instance's
/// evaluated pose, carried forward by the pose's latest motion so it keeps
/// up with the pose drawn this call. With `capture_offsets` each offset is
/// where the particle is now in its joint's frame, and the instance must
/// have a pose (published a call before). Empty `attachments` ends them.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsSoftBodyJointAttachmentsRequest {
    pub soft_body: NativeDynamicsSoftBodyHandle,
    pub instance: NativeAnimationInstanceHandle,
    pub attachments: *const NativeDynamicsSoftBodyJointAttachment,
    pub attachments_len: usize,
    pub capture_offsets: bool,
}

/// Forces a soft body feels every substep until set again. `acceleration`
/// (m/s²) pushes every free particle alike. Air moving at `wind` (m/s)
/// pushes each surface triangle along its normal by
/// `drag × area × v|v|`, `v` the air's speed through it: `drag` is half the
/// air density times a drag coefficient (kg/m³), about 0.6 for a flag.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsSoftBodyForcesRequest {
    pub soft_body: NativeDynamicsSoftBodyHandle,
    pub acceleration: NativeVec3,
    pub wind: NativeVec3,
    pub drag: f32,
}

/// Changes the velocity of every free particle within `radius` of `point`
/// by `velocity_change` (m/s), scaled down linearly to nothing at `radius`;
/// a radius of zero or less changes every free particle fully. A hit, a
/// jump, a squash from above.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsSoftBodyVelocityChangeRequest {
    pub soft_body: NativeDynamicsSoftBodyHandle,
    pub point: NativeVec3,
    pub velocity_change: NativeVec3,
    pub radius: f32,
}

/// Pushes every free particle within `radius` of `center` away from it at
/// `speed` (m/s), scaled down linearly to nothing at `radius`. A negative
/// speed pulls in.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsSoftBodyRadialRequest {
    pub soft_body: NativeDynamicsSoftBodyHandle,
    pub center: NativeVec3,
    pub speed: f32,
    pub radius: f32,
}

/// A soft body after the latest step, with every piece a tear split off
/// it. `volume` and `rest_volume` are its closed pieces' (zero for an open
/// sheet). `max_stretch` is the largest edge stretch over rest length.
/// `contact_count` counts the colliders its surface touches (environment,
/// bodies, soft bodies, itself). `topology_version` changes with each tear.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeDynamicsSoftBodyReadout {
    pub particle_count: u32,
    pub piece_count: u32,
    pub center_of_mass: NativeVec3,
    pub linear_velocity: NativeVec3,
    pub bounds_min: NativeVec3,
    pub bounds_max: NativeVec3,
    pub volume: f32,
    pub rest_volume: f32,
    pub max_stretch: f32,
    pub contact_count: u32,
    pub sleeping: bool,
    pub topology_version: u32,
}

/// One particle: `kinematic` when pinned or driven (moved, never pushed).
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeDynamicsSoftBodyParticle {
    pub position: NativeVec3,
    pub velocity: NativeVec3,
    pub mass: f32,
    pub kinematic: bool,
    pub on_surface: bool,
}

/// The body's particles in particle order (not its torn-off pieces').
/// `particles` points into Dynamics bridge storage until the next call on
/// the same context.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsSoftBodyParticlesResult {
    pub particles: *const NativeDynamicsSoftBodyParticle,
    pub particles_len: usize,
}

/// An appearance drawing a soft body's surface with `material`, reshaped
/// after every step that moves it. Publish it with an `AppearanceFact` at
/// the world origin (identity transform), as its vertices are in the
/// world. `two_sided` also draws each triangle's back with its own normal,
/// for cloth under a one-sided material. It keeps the last shape drawn
/// once its soft body is gone.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsSoftBodyAppearanceRequest {
    pub soft_body: NativeDynamicsSoftBodyHandle,
    pub material: NativeMaterialHandle,
    pub two_sided: bool,
}
