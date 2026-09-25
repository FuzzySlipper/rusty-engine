use std::collections::{BTreeMap, BTreeSet};

use rapier3d_f64::prelude::{
    ColliderBuilder, Group, IntegrationParameters, InteractionGroups, InteractionTestMode,
    MassProperties, PhysicsWorld, RigidBodyBuilder, RigidBodyHandle, RigidBodyType, Rotation,
    SharedShape, Vector,
};

use crate::tether::{
    self, DynamicsTether, DynamicsTetherError, DynamicsTetherReadout, SolverTether,
};
use crate::CollisionProjection;

pub const MAX_DYNAMICS_BODIES: usize = 1_024;
pub const MAX_DYNAMICS_ACTIONS: usize = 4_096;
pub const MAX_DYNAMICS_CONTACTS: usize = 4_096;
pub const MAX_DYNAMICS_STEPS: u8 = 8;
// The owning Engine request uses f32 seconds. Use those exact promoted boundary
// values so a nominal public-limit request is not rejected by f32 rounding.
pub const MIN_DYNAMICS_STEP_SECONDS: f64 = (1.0_f32 / 1_000.0) as f64;
pub const MAX_DYNAMICS_STEP_SECONDS: f64 = (1.0_f32 / 15.0) as f64;
pub const MAX_DISCRETE_TRANSLATION_PER_STEP: f64 = 1.0;
pub const MAX_CCD_TRANSLATION_PER_STEP: f64 = 100.0;
const MASS_PROPERTIES_FRAME_NORMALIZATION_TOLERANCE: f64 = 1.0e-3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DynamicsBodyId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DynamicsShape {
    Sphere { radius: f64 },
    Cuboid { half_extents: [f64; 3] },
    CapsuleY { half_height: f64, radius: f64 },
}

/// Authored mass properties for one dynamics body.
///
/// The body's `mass` remains the sole authoritative total mass. This tuple
/// supplies the local center of mass, principal inertia, and the frame in
/// which that diagonal inertia is expressed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DynamicsMassProperties {
    pub center_of_mass: [f64; 3],
    pub principal_inertia: [f64; 3],
    pub principal_inertia_local_frame: [f64; 4],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DynamicsBodyInput {
    pub id: DynamicsBodyId,
    pub translation: [f64; 3],
    /// Quaternion in x/y/z/w order.
    pub rotation: [f64; 4],
    pub shape: DynamicsShape,
    pub mass: f64,
    pub mass_properties: Option<DynamicsMassProperties>,
    pub linear_velocity: [f64; 3],
    pub angular_velocity: [f64; 3],
    /// `true` locks the corresponding world-space X/Y/Z translation axis.
    pub locked_translation_axes: [bool; 3],
    /// `true` locks the corresponding world-space X/Y/Z rotation axis.
    pub locked_rotation_axes: [bool; 3],
    pub linear_damping: f64,
    pub angular_damping: f64,
    pub gravity_scale: f64,
    pub friction: f64,
    pub restitution: f64,
    pub collision_groups: u32,
    pub collision_mask: u32,
    pub enabled: bool,
    pub sleeping: bool,
    pub continuous_collision: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DynamicsAction {
    pub body: DynamicsBodyId,
    pub force: [f64; 3],
    pub torque: [f64; 3],
    pub impulse: [f64; 3],
    pub torque_impulse: [f64; 3],
    pub wake: bool,
}

impl DynamicsAction {
    pub const fn impulse(body: DynamicsBodyId, impulse: [f64; 3]) -> Self {
        Self {
            body,
            force: [0.0; 3],
            torque: [0.0; 3],
            impulse,
            torque_impulse: [0.0; 3],
            wake: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DynamicsStepInput {
    pub step_seconds: f64,
    pub steps: u8,
    pub gravity: [f64; 3],
    pub bodies: Vec<DynamicsBodyInput>,
    pub actions: Vec<DynamicsAction>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DynamicsBodyOutput {
    pub id: DynamicsBodyId,
    pub translation: [f64; 3],
    pub rotation: [f64; 4],
    pub linear_velocity: [f64; 3],
    pub angular_velocity: [f64; 3],
    pub sleeping: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DynamicsContact {
    pub first: DynamicsBodyId,
    pub second: Option<DynamicsBodyId>,
    pub impulse: [f64; 3],
    pub impulse_magnitude: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DynamicsStepOutput {
    pub bodies: Vec<DynamicsBodyOutput>,
    pub contacts: Vec<DynamicsContact>,
    pub tethers: Vec<DynamicsTetherReadout>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DynamicsError {
    Tether(DynamicsTetherError),
    InvalidStep {
        actual: f64,
    },
    InvalidStepCount {
        actual: u8,
        maximum: u8,
    },
    TooManyBodies {
        actual: usize,
        maximum: usize,
    },
    TooManyActions {
        actual: usize,
        maximum: usize,
    },
    TooManyContacts {
        maximum: usize,
    },
    DuplicateBody {
        body: DynamicsBodyId,
    },
    UnknownActionBody {
        body: DynamicsBodyId,
    },
    InvalidBody {
        body: DynamicsBodyId,
    },
    LockedTranslationAxisVelocity {
        body: DynamicsBodyId,
    },
    LockedRotationAxisVelocity {
        body: DynamicsBodyId,
    },
    InvalidAction {
        body: DynamicsBodyId,
    },
    MotionLimitExceeded {
        body: DynamicsBodyId,
        continuous_collision: bool,
        estimated_translation: f64,
        maximum: f64,
    },
    OutputNotFinite {
        body: DynamicsBodyId,
    },
}

impl DynamicsError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Tether(error) => error.code(),
            Self::InvalidStep { .. } => "invalid-dynamics-step",
            Self::InvalidStepCount { .. } => "invalid-dynamics-step-count",
            Self::TooManyBodies { .. } => "dynamics-body-quota-exceeded",
            Self::TooManyActions { .. } => "dynamics-action-quota-exceeded",
            Self::TooManyContacts { .. } => "dynamics-contact-quota-exceeded",
            Self::DuplicateBody { .. } => "duplicate-dynamics-body",
            Self::UnknownActionBody { .. } => "unknown-dynamics-action-body",
            Self::InvalidBody { .. } => "invalid-dynamics-body",
            Self::LockedTranslationAxisVelocity { .. } => {
                "locked-dynamics-translation-axis-velocity"
            }
            Self::LockedRotationAxisVelocity { .. } => "locked-dynamics-rotation-axis-velocity",
            Self::InvalidAction { .. } => "invalid-dynamics-action",
            Self::MotionLimitExceeded { .. } => "dynamics-motion-limit-exceeded",
            Self::OutputNotFinite { .. } => "non-finite-dynamics-output",
        }
    }
}

impl std::fmt::Display for DynamicsError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {self:?}", self.code())
    }
}

impl std::error::Error for DynamicsError {}

#[derive(Debug, Clone, Copy, Default)]
struct AggregatedAction {
    force: Vector,
    torque: Vector,
    impulse: Vector,
    torque_impulse: Vector,
    wake: bool,
}

#[derive(Debug, Clone, Copy)]
struct AxisLocks {
    translation: [bool; 3],
    rotation: [bool; 3],
}

/// Linear point-velocity response to unit world impulses, including locked
/// axes and rotational inertia. No solver world or step is created.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DynamicsAnchorObservation {
    pub point: [f64; 3],
    pub point_velocity: [f64; 3],
    pub center_of_mass: [f64; 3],
    pub response: [[f64; 3]; 3],
}

pub fn observe_dynamics_anchor(
    body: DynamicsBodyInput,
    local_anchor: [f64; 3],
) -> Result<DynamicsAnchorObservation, DynamicsError> {
    validate_body(&body)?;
    if !body.enabled || !local_anchor.into_iter().all(f64::is_finite) {
        return Err(DynamicsError::InvalidBody { body: body.id });
    }
    let (builder, collider) = body_builders(&body);
    let rigid_body = builder.build();
    let mut properties = rigid_body.mass_properties().clone();
    properties.local_mprops = collider.build().mass_properties();
    properties.update_world_mass_properties(RigidBodyType::Dynamic, rigid_body.position());
    let point = rigid_body.position().transform_point(vector(local_anchor));
    let lever = point - properties.world_com;
    let point_velocity = vector(body.linear_velocity) + vector(body.angular_velocity).cross(lever);
    let response = [Vector::X, Vector::Y, Vector::Z].map(|impulse| {
        let angular = properties.effective_world_inv_inertia * lever.cross(impulse);
        (impulse * properties.effective_inv_mass + angular.cross(lever)).to_array()
    });
    if !point.is_finite()
        || !point_velocity.is_finite()
        || !properties.world_com.is_finite()
        || !response.iter().flatten().all(|value| value.is_finite())
    {
        return Err(DynamicsError::OutputNotFinite { body: body.id });
    }
    Ok(DynamicsAnchorObservation {
        point: point.to_array(),
        point_velocity: point_velocity.to_array(),
        center_of_mass: properties.world_com.to_array(),
        response,
    })
}

/// Run one bounded candidate simulation from canonical inputs.
///
/// The Rapier world is rebuilt off-side for every call. It is a derived cache,
/// not persistent authority; callers publish [`DynamicsStepOutput`] only after
/// their own exact-revision transaction succeeds.
pub fn simulate_dynamics(
    projection: &CollisionProjection,
    input: DynamicsStepInput,
) -> Result<DynamicsStepOutput, DynamicsError> {
    simulate_dynamics_with_tethers(projection, input, Vec::new())
}

/// Simulate admitted bodies and maximum-distance tethers in the same derived world.
pub fn simulate_dynamics_with_tethers(
    projection: &CollisionProjection,
    input: DynamicsStepInput,
    tethers: Vec<DynamicsTether>,
) -> Result<DynamicsStepOutput, DynamicsError> {
    simulate_dynamics_with_rope_solver(
        projection,
        input,
        tethers,
        tether::DynamicsRopeSolverConfig::default(),
    )
}

pub fn simulate_dynamics_with_rope_solver(
    projection: &CollisionProjection,
    mut input: DynamicsStepInput,
    mut tethers: Vec<DynamicsTether>,
    solver: tether::DynamicsRopeSolverConfig,
) -> Result<DynamicsStepOutput, DynamicsError> {
    solver.validate().map_err(DynamicsError::Tether)?;
    validate_header(&input)?;
    input.bodies.sort_by_key(|body| body.id);
    let mut seen = BTreeSet::new();
    let mut body_locks = BTreeMap::new();
    for body in &input.bodies {
        if !seen.insert(body.id) {
            return Err(DynamicsError::DuplicateBody { body: body.id });
        }
        validate_body(body)?;
        body_locks.insert(
            body.id,
            AxisLocks {
                translation: body.locked_translation_axes,
                rotation: body.locked_rotation_axes,
            },
        );
    }
    let actions = aggregate_actions(&input.actions, &body_locks)?;
    validate_motion(&input, &actions)?;
    tether::validate_dynamics_tethers(&mut tethers, &input.bodies)
        .map_err(DynamicsError::Tether)?;
    let subdivisions = if tethers.is_empty() {
        1
    } else {
        solver.substeps
    };

    let mut world = PhysicsWorld {
        gravity: vector(input.gravity),
        integration_parameters: IntegrationParameters {
            dt: input.step_seconds / subdivisions as f64,
            num_solver_iterations: if tethers.is_empty() {
                IntegrationParameters::default().num_solver_iterations
            } else {
                solver.iterations
            },
            ..IntegrationParameters::default()
        },
        ..PhysicsWorld::default()
    };
    insert_static_environment(&mut world, projection);

    let mut handles = BTreeMap::<DynamicsBodyId, RigidBodyHandle>::new();
    for body in &input.bodies {
        let (builder, collider) = body_builders(body);
        let (handle, _) = world.insert(builder, collider);
        handles.insert(body.id, handle);
    }

    for (body, action) in &actions {
        let target = &mut world.bodies[handles[body]];
        target.apply_impulse(action.impulse, action.wake);
        target.apply_torque_impulse(action.torque_impulse, action.wake);
        target.add_force(action.force, action.wake);
        target.add_torque(action.torque, action.wake);
        if action.wake {
            target.wake_up(true);
        }
    }
    let mut solver_tethers: Vec<_> = tethers
        .into_iter()
        .map(|definition| SolverTether::insert(definition, &mut world, &handles))
        .collect();
    for _ in 0..usize::from(input.steps) * subdivisions {
        for tether in &mut solver_tethers {
            tether.reel(&mut world, input.step_seconds / subdivisions as f64);
        }
        world.step();
        for tether in &mut solver_tethers {
            tether.accumulate(&world);
        }
    }

    let mut bodies = Vec::with_capacity(handles.len());
    for (id, handle) in &handles {
        let body = &world.bodies[*handle];
        let translation = body.translation().to_array();
        let rotation = body.rotation().to_array();
        let linear_velocity = body.linvel().to_array();
        let angular_velocity = body.angvel().to_array();
        if !translation
            .into_iter()
            .chain(rotation)
            .chain(linear_velocity)
            .chain(angular_velocity)
            .all(f64::is_finite)
        {
            return Err(DynamicsError::OutputNotFinite { body: *id });
        }
        bodies.push(DynamicsBodyOutput {
            id: *id,
            translation,
            rotation,
            linear_velocity,
            angular_velocity,
            sleeping: body.is_sleeping(),
        });
    }

    let mut contacts = Vec::new();
    for pair in world
        .contact_pairs()
        .filter(|pair| pair.has_any_active_contact())
    {
        let first = dynamic_id(world.colliders[pair.collider1].user_data);
        let second = dynamic_id(world.colliders[pair.collider2].user_data);
        let (first, second) = match (first, second) {
            (Some(first), second) => (first, second),
            (None, Some(second)) => (second, None),
            (None, None) => continue,
        };
        if contacts.len() >= MAX_DYNAMICS_CONTACTS {
            return Err(DynamicsError::TooManyContacts {
                maximum: MAX_DYNAMICS_CONTACTS,
            });
        }
        let impulse = pair.total_impulse();
        contacts.push(DynamicsContact {
            first,
            second,
            impulse: impulse.to_array(),
            impulse_magnitude: pair.total_impulse_magnitude(),
        });
    }
    contacts.sort_by_key(|contact| (contact.first, contact.second));
    let tethers = solver_tethers
        .iter()
        .map(|tether| tether.readout(&world))
        .collect::<Result<Vec<_>, _>>()
        .map_err(DynamicsError::Tether)?;
    Ok(DynamicsStepOutput {
        bodies,
        contacts,
        tethers,
    })
}

fn body_builders(body: &DynamicsBodyInput) -> (RigidBodyBuilder, ColliderBuilder) {
    let mut builder = RigidBodyBuilder::dynamic()
        .translation(vector(body.translation))
        .rotation(vector3(body.rotation))
        .linvel(vector(body.linear_velocity))
        .angvel(vector(body.angular_velocity))
        .enabled_translations(
            !body.locked_translation_axes[0],
            !body.locked_translation_axes[1],
            !body.locked_translation_axes[2],
        )
        .enabled_rotations(
            !body.locked_rotation_axes[0],
            !body.locked_rotation_axes[1],
            !body.locked_rotation_axes[2],
        )
        .linear_damping(body.linear_damping)
        .angular_damping(body.angular_damping)
        .gravity_scale(body.gravity_scale)
        .ccd_enabled(body.continuous_collision)
        .sleeping(body.sleeping)
        .enabled(body.enabled)
        .user_data(u128::from(body.id.0) + 1);
    if !body.sleeping {
        builder = builder.can_sleep(true);
    }
    let collider = ColliderBuilder::new(shared_shape(body.shape));
    let collider = if let Some(properties) = body.mass_properties {
        collider.mass_properties(MassProperties::with_principal_inertia_frame(
            vector(properties.center_of_mass),
            body.mass,
            vector(properties.principal_inertia),
            rotation(properties.principal_inertia_local_frame),
        ))
    } else {
        collider.mass(body.mass)
    }
    .friction(body.friction)
    .restitution(body.restitution)
    .collision_groups(InteractionGroups::new(
        Group::from_bits_retain(body.collision_groups),
        Group::from_bits_retain(body.collision_mask),
        InteractionTestMode::And,
    ))
    .user_data(u128::from(body.id.0) + 1);
    (builder, collider)
}

fn validate_header(input: &DynamicsStepInput) -> Result<(), DynamicsError> {
    if !input.step_seconds.is_finite()
        || !(MIN_DYNAMICS_STEP_SECONDS..=MAX_DYNAMICS_STEP_SECONDS).contains(&input.step_seconds)
        || !input.gravity.into_iter().all(f64::is_finite)
    {
        return Err(DynamicsError::InvalidStep {
            actual: input.step_seconds,
        });
    }
    if input.steps == 0 || input.steps > MAX_DYNAMICS_STEPS {
        return Err(DynamicsError::InvalidStepCount {
            actual: input.steps,
            maximum: MAX_DYNAMICS_STEPS,
        });
    }
    if input.bodies.len() > MAX_DYNAMICS_BODIES {
        return Err(DynamicsError::TooManyBodies {
            actual: input.bodies.len(),
            maximum: MAX_DYNAMICS_BODIES,
        });
    }
    if input.actions.len() > MAX_DYNAMICS_ACTIONS {
        return Err(DynamicsError::TooManyActions {
            actual: input.actions.len(),
            maximum: MAX_DYNAMICS_ACTIONS,
        });
    }
    Ok(())
}

fn validate_body(body: &DynamicsBodyInput) -> Result<(), DynamicsError> {
    let finite = body
        .translation
        .into_iter()
        .chain(body.rotation)
        .chain(body.linear_velocity)
        .chain(body.angular_velocity)
        .chain([
            body.mass,
            body.linear_damping,
            body.angular_damping,
            body.gravity_scale,
            body.friction,
            body.restitution,
        ])
        .all(f64::is_finite);
    let shape_valid = match body.shape {
        DynamicsShape::Sphere { radius } => positive(radius),
        DynamicsShape::Cuboid { half_extents } => half_extents.into_iter().all(positive),
        DynamicsShape::CapsuleY {
            half_height,
            radius,
        } => positive(half_height) && positive(radius),
    };
    let mass_properties_valid = body.mass_properties.is_none_or(valid_mass_properties);
    let rotation_norm = body
        .rotation
        .into_iter()
        .map(|value| value * value)
        .sum::<f64>();
    if !finite
        || !shape_valid
        || !mass_properties_valid
        || body.mass <= 0.0
        || body.linear_damping < 0.0
        || body.angular_damping < 0.0
        || body.friction < 0.0
        || !(0.0..=1.0).contains(&body.restitution)
        || body.collision_groups == 0
        || (rotation_norm - 1.0).abs() > 1.0e-4
    {
        return Err(DynamicsError::InvalidBody { body: body.id });
    }
    if has_velocity_on_locked_axis(body.linear_velocity, body.locked_translation_axes) {
        return Err(DynamicsError::LockedTranslationAxisVelocity { body: body.id });
    }
    if has_velocity_on_locked_axis(body.angular_velocity, body.locked_rotation_axes) {
        return Err(DynamicsError::LockedRotationAxisVelocity { body: body.id });
    }
    Ok(())
}

fn valid_mass_properties(properties: DynamicsMassProperties) -> bool {
    let center_of_mass_valid = properties.center_of_mass.into_iter().all(f64::is_finite);
    let principal_inertia_valid = properties.principal_inertia.into_iter().all(positive);
    let frame_norm = properties
        .principal_inertia_local_frame
        .into_iter()
        .map(|value| value * value)
        .sum::<f64>();
    center_of_mass_valid
        && principal_inertia_valid
        && properties
            .principal_inertia_local_frame
            .into_iter()
            .all(f64::is_finite)
        && (frame_norm - 1.0).abs() <= MASS_PROPERTIES_FRAME_NORMALIZATION_TOLERANCE
}

fn has_velocity_on_locked_axis(velocity: [f64; 3], locked_axes: [bool; 3]) -> bool {
    locked_axes
        .into_iter()
        .zip(velocity)
        .any(|(locked, component)| locked && component != 0.0)
}

fn aggregate_actions(
    actions: &[DynamicsAction],
    bodies: &BTreeMap<DynamicsBodyId, AxisLocks>,
) -> Result<BTreeMap<DynamicsBodyId, AggregatedAction>, DynamicsError> {
    let mut aggregated = BTreeMap::<DynamicsBodyId, AggregatedAction>::new();
    for action in actions {
        let Some(locks) = bodies.get(&action.body) else {
            return Err(DynamicsError::UnknownActionBody { body: action.body });
        };
        if !action
            .force
            .into_iter()
            .chain(action.torque)
            .chain(action.impulse)
            .chain(action.torque_impulse)
            .all(f64::is_finite)
        {
            return Err(DynamicsError::InvalidAction { body: action.body });
        }
        let entry = aggregated.entry(action.body).or_default();
        entry.force += mask_locked_axes(vector(action.force), locks.translation);
        entry.torque += mask_locked_axes(vector(action.torque), locks.rotation);
        entry.impulse += mask_locked_axes(vector(action.impulse), locks.translation);
        entry.torque_impulse += mask_locked_axes(vector(action.torque_impulse), locks.rotation);
        entry.wake |= action.wake;
        if !entry
            .force
            .to_array()
            .into_iter()
            .chain(entry.torque.to_array())
            .chain(entry.impulse.to_array())
            .chain(entry.torque_impulse.to_array())
            .all(f64::is_finite)
        {
            return Err(DynamicsError::InvalidAction { body: action.body });
        }
    }
    Ok(aggregated)
}

fn validate_motion(
    input: &DynamicsStepInput,
    actions: &BTreeMap<DynamicsBodyId, AggregatedAction>,
) -> Result<(), DynamicsError> {
    let gravity = vector(input.gravity);
    for body in &input.bodies {
        let action = actions.get(&body.id).copied().unwrap_or_default();
        let acceleration = mask_locked_axes(
            action.force / body.mass + gravity * body.gravity_scale,
            body.locked_translation_axes,
        );
        let estimated_velocity = mask_locked_axes(
            vector(body.linear_velocity) + action.impulse / body.mass,
            body.locked_translation_axes,
        ) + acceleration * input.step_seconds * f64::from(input.steps);
        let estimated_translation = estimated_velocity.length() * input.step_seconds;
        let maximum = if body.continuous_collision {
            MAX_CCD_TRANSLATION_PER_STEP
        } else {
            MAX_DISCRETE_TRANSLATION_PER_STEP
        };
        if !estimated_translation.is_finite() || estimated_translation > maximum {
            return Err(DynamicsError::MotionLimitExceeded {
                body: body.id,
                continuous_collision: body.continuous_collision,
                estimated_translation,
                maximum,
            });
        }
    }
    Ok(())
}

fn mask_locked_axes(value: Vector, locked_axes: [bool; 3]) -> Vector {
    Vector::new(
        if locked_axes[0] { 0.0 } else { value.x },
        if locked_axes[1] { 0.0 } else { value.y },
        if locked_axes[2] { 0.0 } else { value.z },
    )
}

fn insert_static_environment(world: &mut PhysicsWorld, projection: &CollisionProjection) {
    for shape in projection.dynamics_shapes() {
        world.insert(
            RigidBodyBuilder::fixed(),
            ColliderBuilder::new(shape)
                // Fixed terrain contributes no dynamic mass. Avoid walking every
                // voxel child to derive unused compound inertia on each step.
                .mass_properties(MassProperties::default())
                .collision_groups(InteractionGroups::all())
                .user_data(0),
        );
    }
}

fn shared_shape(shape: DynamicsShape) -> SharedShape {
    match shape {
        DynamicsShape::Sphere { radius } => SharedShape::ball(radius),
        DynamicsShape::Cuboid { half_extents } => {
            SharedShape::cuboid(half_extents[0], half_extents[1], half_extents[2])
        }
        DynamicsShape::CapsuleY {
            half_height,
            radius,
        } => SharedShape::capsule_y(half_height, radius),
    }
}

fn vector(value: [f64; 3]) -> Vector {
    Vector::new(value[0], value[1], value[2])
}

fn vector3(value: [f64; 4]) -> Vector {
    let rotation = rotation(value);
    rotation.to_scaled_axis()
}

fn rotation(value: [f64; 4]) -> Rotation {
    Rotation::from_xyzw(value[0], value[1], value[2], value[3])
}

fn positive(value: f64) -> bool {
    value.is_finite() && value > 0.0
}

fn dynamic_id(user_data: u128) -> Option<DynamicsBodyId> {
    (user_data != 0).then(|| DynamicsBodyId((user_data - 1) as u64))
}

impl CollisionProjection {
    fn dynamics_shapes(&self) -> Vec<SharedShape> {
        let mut shapes =
            Vec::with_capacity(self.chunks.len() + self.static_meshes.instance_count());
        shapes.extend(
            self.chunks
                .values()
                .map(|chunk| SharedShape(chunk.shape.clone())),
        );
        shapes.extend(self.static_meshes.dynamics_shapes());
        shapes
    }
}

#[cfg(test)]
mod tests {
    use core_space::{ChunkDims, GridId, VoxelGridSpec};
    use svc_spatial::VoxelWorld;

    use super::*;

    fn empty_projection() -> CollisionProjection {
        let grid = VoxelGridSpec::new(GridId::new(0), 1.0, ChunkDims::cubic(8).unwrap())
            .expect("valid empty test grid");
        CollisionProjection::build(&VoxelWorld::new(grid))
    }

    fn body() -> DynamicsBodyInput {
        DynamicsBodyInput {
            id: DynamicsBodyId(1),
            translation: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
            shape: DynamicsShape::Sphere { radius: 0.5 },
            mass: 1.0,
            mass_properties: None,
            linear_velocity: [0.0; 3],
            angular_velocity: [0.0; 3],
            locked_translation_axes: [false; 3],
            locked_rotation_axes: [false; 3],
            linear_damping: 0.0,
            angular_damping: 0.0,
            gravity_scale: 0.0,
            friction: 0.5,
            restitution: 0.0,
            collision_groups: u32::MAX,
            collision_mask: u32::MAX,
            enabled: true,
            sleeping: false,
            continuous_collision: false,
        }
    }

    fn explicit_body() -> DynamicsBodyInput {
        let mut body = body();
        body.mass = 2.0;
        body.mass_properties = Some(DynamicsMassProperties {
            center_of_mass: [0.2, -0.1, 0.0],
            principal_inertia: [1.0, 2.0, 4.0],
            principal_inertia_local_frame: [0.0, 0.0, 0.0, 1.0],
        });
        body
    }

    fn input(body: DynamicsBodyInput) -> DynamicsStepInput {
        DynamicsStepInput {
            step_seconds: 1.0 / 60.0,
            steps: 1,
            gravity: [0.0; 3],
            bodies: vec![body],
            actions: Vec::new(),
        }
    }

    fn tether() -> DynamicsTether {
        DynamicsTether {
            id: 7,
            first: crate::DynamicsTetherEndpoint::Fixed([0.0; 3]),
            second: crate::DynamicsTetherEndpoint::Body {
                body: DynamicsBodyId(1),
                local_anchor: [0.0; 3],
            },
            maximum_length: 3.0,
            target_length: 3.0,
            reel_speed: 0.25,
            was_taut: false,
            wake: true,
            contacts_enabled: true,
        }
    }

    #[test]
    fn anchor_response_matches_off_center_impulse_with_rotated_inertia_and_locks() {
        for locked in [false, true] {
            let mut body = explicit_body();
            body.rotation = [0.0, 0.0, (0.3_f64).sin(), (0.3_f64).cos()];
            body.locked_translation_axes = [locked, false, false];
            body.locked_rotation_axes = [false, locked, false];
            let anchor = observe_dynamics_anchor(body, [0.7, 0.4, -0.3]).unwrap();
            let lever = vector(anchor.point) - vector(anchor.center_of_mass);
            let impulse = Vector::new(0.2, -0.1, 0.3);
            let expected = vector(anchor.response[0]) * impulse.x
                + vector(anchor.response[1]) * impulse.y
                + vector(anchor.response[2]) * impulse.z;
            let mut request = input(body);
            let mut action = DynamicsAction::impulse(body.id, impulse.to_array());
            action.torque_impulse = lever.cross(impulse).to_array();
            request.actions.push(action);
            let result = simulate_dynamics(&empty_projection(), request).unwrap();
            let actual = vector(result.bodies[0].linear_velocity)
                + vector(result.bodies[0].angular_velocity).cross(lever);
            assert!(
                (actual - expected).length() < 0.0001,
                "{actual:?} vs {expected:?}"
            );
        }
    }

    #[test]
    fn tether_hanging_mass_reports_weight_impulse_and_rebuilds() {
        let mut body = body();
        body.translation = [0.0, -3.0, 0.0];
        body.gravity_scale = 1.0;
        let mut tether = tether();
        for tick in 0..60 {
            let mut request = input(body);
            request.gravity = [0.0, -9.81, 0.0];
            let result =
                simulate_dynamics_with_tethers(&empty_projection(), request, vec![tether]).unwrap();
            let readout = result.tethers[0];
            assert!(readout.taut);
            assert_eq!(readout.caught, tick == 0);
            assert!((readout.distance - 3.0).abs() < 0.001);
            assert!((readout.force_proxy - 9.81).abs() < 0.005, "{readout:?}");
            body.translation = result.bodies[0].translation;
            body.linear_velocity = result.bodies[0].linear_velocity;
            tether.was_taut = readout.taut;
            tether.wake = false;
        }
    }

    #[test]
    fn tether_two_dynamic_bodies_exchange_momentum_and_preserve_local_anchor() {
        let mut first = body();
        first.translation = [-1.5, 0.0, 0.0];
        first.linear_velocity = [-3.0, 0.0, 0.0];
        let mut second = first;
        second.id = DynamicsBodyId(2);
        second.translation = [1.5, 0.0, 0.0];
        second.linear_velocity = [3.0, 0.0, 0.0];
        let mut rope = tether();
        rope.first = crate::DynamicsTetherEndpoint::Body {
            body: first.id,
            local_anchor: [0.5, 0.0, 0.0],
        };
        rope.second = crate::DynamicsTetherEndpoint::Body {
            body: second.id,
            local_anchor: [-0.5, 0.0, 0.0],
        };
        rope.maximum_length = 2.0;
        rope.target_length = 2.0;
        let mut request = input(first);
        request.bodies.push(second);
        let result =
            simulate_dynamics_with_tethers(&empty_projection(), request.clone(), vec![rope])
                .unwrap();
        assert_eq!(
            result,
            simulate_dynamics_with_tethers(&empty_projection(), request, vec![rope]).unwrap()
        );
        assert!(
            (result.bodies[0].linear_velocity[0] + result.bodies[1].linear_velocity[0]).abs()
                < 1e-9
        );
        assert!((result.tethers[0].first[0] - result.bodies[0].translation[0] - 0.5).abs() < 1e-9);
        assert!(result.tethers[0].distance < 2.01);
        assert!(result
            .bodies
            .iter()
            .all(|body| body.linear_velocity[0].abs() < 0.01));
    }

    #[test]
    fn tether_slack_catch_swing_and_release_do_not_create_energy() {
        let mut body = body();
        body.translation = [1.0, -1.0, 0.0];
        body.linear_velocity = [8.0, -15.0, 0.0];
        body.gravity_scale = 1.0;
        body.continuous_collision = true;
        let initial_energy =
            0.5 * vector(body.linear_velocity).length_squared() + 9.81 * body.translation[1];
        let mut rope = tether();
        let mut caught = false;
        let mut crossed = false;
        for _ in 0..600 {
            let mut request = input(body);
            request.gravity = [0.0, -9.81, 0.0];
            let result =
                simulate_dynamics_with_tethers(&empty_projection(), request, vec![rope]).unwrap();
            let after = result.bodies[0];
            let energy =
                0.5 * vector(after.linear_velocity).length_squared() + 9.81 * after.translation[1];
            assert!(energy <= initial_energy + 0.01, "energy grew: {energy}");
            assert!(result.tethers[0].distance < 3.01);
            caught |= result.tethers[0].caught;
            crossed |= after.translation[0] < -0.5;
            body.translation = after.translation;
            body.linear_velocity = after.linear_velocity;
            body.rotation = after.rotation;
            body.angular_velocity = after.angular_velocity;
            rope.was_taut = result.tethers[0].taut;
            rope.wake = false;
        }
        assert!(caught && crossed);
        let released = simulate_dynamics(&empty_projection(), input(body)).unwrap();
        assert_eq!(released.bodies[0].linear_velocity, body.linear_velocity);
    }

    #[test]
    fn tether_off_center_anchor_rotates_body_and_unequal_masses_conserve_momentum() {
        let mut first = body();
        first.mass = 1.0;
        first.translation = [-1.0, 0.0, 0.0];
        first.linear_velocity = [-2.0, 0.0, 0.0];
        let mut second = first;
        second.id = DynamicsBodyId(2);
        second.mass = 4.0;
        second.translation = [1.0, 0.0, 0.0];
        second.linear_velocity = [0.5, 0.0, 0.0];
        let mut rope = tether();
        rope.first = crate::DynamicsTetherEndpoint::Body {
            body: first.id,
            local_anchor: [0.0, 0.5, 0.0],
        };
        rope.second = crate::DynamicsTetherEndpoint::Body {
            body: second.id,
            local_anchor: [0.0, 0.5, 0.0],
        };
        rope.maximum_length = 2.0;
        rope.target_length = 2.0;
        let mut request = input(first);
        request.bodies.push(second);
        let result =
            simulate_dynamics_with_tethers(&empty_projection(), request, vec![rope]).unwrap();
        assert!(result.bodies[0].angular_velocity[2].abs() > 0.01);
        assert!(
            (result.bodies[0].linear_velocity[0] + 4.0 * result.bodies[1].linear_velocity[0]).abs()
                < 1e-8
        );
        assert!(result.tethers[0].distance < 2.001);
        let rotation = Rotation::from_xyzw(
            result.bodies[0].rotation[0],
            result.bodies[0].rotation[1],
            result.bodies[0].rotation[2],
            result.bodies[0].rotation[3],
        );
        let anchor = vector(result.bodies[0].translation) + rotation * Vector::new(0.0, 0.5, 0.0);
        assert!((anchor - vector(result.tethers[0].first)).length() < 1e-9);
    }

    #[test]
    fn tether_rebuild_preserves_sleep_until_reel_or_attachment_wakes() {
        let mut body = body();
        body.translation = [0.0, -2.0, 0.0];
        body.sleeping = true;
        let mut rope = tether();
        rope.wake = false;
        let sleeping =
            simulate_dynamics_with_tethers(&empty_projection(), input(body), vec![rope]).unwrap();
        assert!(sleeping.bodies[0].sleeping);
        assert_eq!(sleeping.bodies[0].translation, body.translation);
        rope.target_length = 1.0;
        let reeled =
            simulate_dynamics_with_tethers(&empty_projection(), input(body), vec![rope]).unwrap();
        assert!(!reeled.bodies[0].sleeping);
        rope.target_length = rope.maximum_length;
        rope.wake = true;
        let attached =
            simulate_dynamics_with_tethers(&empty_projection(), input(body), vec![rope]).unwrap();
        assert!(!attached.bodies[0].sleeping);
    }

    #[test]
    fn tether_reeling_changes_effective_length_at_bounded_rate() {
        let mut body = body();
        body.translation = [0.0, -3.0, 0.0];
        let mut rope = tether();
        rope.target_length = 1.0;
        let result =
            simulate_dynamics_with_tethers(&empty_projection(), input(body), vec![rope]).unwrap();
        assert!((result.tethers[0].maximum_length - (3.0 - 0.25 / 60.0)).abs() < 1e-9);
        assert!(result.bodies[0].linear_velocity[1] < 0.3);
    }

    #[test]
    fn tether_invalid_anchors_duplicates_and_budgets_reject_before_simulation() {
        let mut rope = tether();
        rope.second = crate::DynamicsTetherEndpoint::Body {
            body: DynamicsBodyId(99),
            local_anchor: [0.0; 3],
        };
        assert!(matches!(
            simulate_dynamics_with_tethers(&empty_projection(), input(body()), vec![rope]),
            Err(DynamicsError::Tether(DynamicsTetherError::InvalidAnchor {
                id: 7
            }))
        ));
        assert!(matches!(
            simulate_dynamics_with_tethers(&empty_projection(), input(body()), vec![tether(); 2]),
            Err(DynamicsError::Tether(DynamicsTetherError::DuplicateId {
                id: 7
            }))
        ));
        assert!(matches!(
            simulate_dynamics_with_tethers(
                &empty_projection(),
                input(body()),
                vec![tether(); crate::MAX_DYNAMICS_TETHERS + 1]
            ),
            Err(DynamicsError::Tether(DynamicsTetherError::BudgetExceeded))
        ));
    }

    #[test]
    fn explicit_mass_properties_use_authored_total_once_and_preserve_asymmetry() {
        let mut step = input(explicit_body());
        step.actions = vec![DynamicsAction {
            body: DynamicsBodyId(1),
            force: [2.0, 0.0, 0.0],
            torque: [0.0, 1.0, 0.0],
            impulse: [0.0; 3],
            torque_impulse: [0.0; 3],
            wake: true,
        }];

        let output = simulate_dynamics(&empty_projection(), step)
            .expect("explicit mass properties are a valid dynamics body");
        let body = output.bodies[0];

        // Force / authored total mass, not force / (collider mass + authored mass).
        assert!((body.linear_velocity[0] - (1.0 / 60.0)).abs() < 1.0e-6);
        // The Y principal inertia is 2, so the unit torque produces half the
        // angular velocity of a unit inertia axis.
        assert!((body.angular_velocity[1] - (1.0 / 120.0)).abs() < 1.0e-6);
    }

    #[test]
    fn explicit_mass_properties_reject_nonfinite_or_nonpositive_values() {
        let mut body = explicit_body();
        body.mass_properties.as_mut().unwrap().principal_inertia[0] = 0.0;
        assert_eq!(
            simulate_dynamics(&empty_projection(), input(body)),
            Err(DynamicsError::InvalidBody {
                body: DynamicsBodyId(1)
            })
        );

        let mut body = explicit_body();
        body.mass_properties.as_mut().unwrap().center_of_mass[0] = f64::NAN;
        assert_eq!(
            simulate_dynamics(&empty_projection(), input(body)),
            Err(DynamicsError::InvalidBody {
                body: DynamicsBodyId(1)
            })
        );
    }

    fn action(body: DynamicsBodyId, value: f64) -> DynamicsAction {
        DynamicsAction {
            body,
            force: [value; 3],
            torque: [value; 3],
            impulse: [value; 3],
            torque_impulse: [value; 3],
            wake: true,
        }
    }

    #[test]
    fn locked_translation_velocity_is_rejected_before_solver_construction() {
        let mut body = body();
        body.locked_translation_axes = [false, true, false];
        body.linear_velocity = [0.0, 1.0, 0.0];

        assert_eq!(
            simulate_dynamics(&empty_projection(), input(body)),
            Err(DynamicsError::LockedTranslationAxisVelocity {
                body: DynamicsBodyId(1)
            })
        );
    }

    #[test]
    fn locked_rotation_velocity_is_rejected_before_solver_construction() {
        let mut body = body();
        body.locked_rotation_axes = [true, false, true];
        body.angular_velocity = [1.0, 0.0, 0.0];

        assert_eq!(
            simulate_dynamics(&empty_projection(), input(body)),
            Err(DynamicsError::LockedRotationAxisVelocity {
                body: DynamicsBodyId(1)
            })
        );
    }

    #[test]
    fn all_locked_axes_admit_zero_initial_velocities() {
        let mut body = body();
        body.locked_translation_axes = [true; 3];
        body.locked_rotation_axes = [true; 3];

        let output = simulate_dynamics(&empty_projection(), input(body))
            .expect("zero velocity is compatible with every locked axis");
        assert_eq!(output.bodies.len(), 1);
        assert_eq!(output.bodies[0].translation, [0.0; 3]);
        assert_eq!(output.bodies[0].linear_velocity, [0.0; 3]);
        assert_eq!(output.bodies[0].angular_velocity, [0.0; 3]);
    }

    #[test]
    fn finite_locked_influences_are_masked_before_aggregation() {
        let mut body = body();
        body.locked_translation_axes = [true; 3];
        body.locked_rotation_axes = [true; 3];
        let mut step = input(body);
        step.actions = vec![action(DynamicsBodyId(1), f64::MAX); 2];

        let output = simulate_dynamics(&empty_projection(), step)
            .expect("suppressed locked axes never overflow the action accumulator");
        assert_eq!(output.bodies[0].translation, [0.0; 3]);
        assert_eq!(output.bodies[0].linear_velocity, [0.0; 3]);
        assert_eq!(output.bodies[0].angular_velocity, [0.0; 3]);
    }

    #[test]
    fn finite_unlocked_influence_overflow_still_rejects() {
        let mut step = input(body());
        step.actions = vec![action(DynamicsBodyId(1), f64::MAX); 2];

        assert_eq!(
            simulate_dynamics(&empty_projection(), step),
            Err(DynamicsError::InvalidAction {
                body: DynamicsBodyId(1)
            })
        );
    }

    #[test]
    fn nonfinite_locked_influence_still_rejects_before_masking() {
        let mut body = body();
        body.locked_translation_axes = [true; 3];
        body.locked_rotation_axes = [true; 3];
        let mut step = input(body);
        let mut invalid = action(DynamicsBodyId(1), 0.0);
        invalid.force[0] = f64::NAN;
        step.actions.push(invalid);

        assert_eq!(
            simulate_dynamics(&empty_projection(), step),
            Err(DynamicsError::InvalidAction {
                body: DynamicsBodyId(1)
            })
        );
    }
}
