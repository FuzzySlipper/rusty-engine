use std::collections::{BTreeMap, BTreeSet};

use rapier3d_f64::prelude::{
    ColliderBuilder, Group, IntegrationParameters, InteractionGroups, InteractionTestMode,
    PhysicsWorld, RigidBodyBuilder, RigidBodyHandle, Rotation, SharedShape, Vector,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DynamicsBodyId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DynamicsShape {
    Sphere { radius: f64 },
    Cuboid { half_extents: [f64; 3] },
    CapsuleY { half_height: f64, radius: f64 },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DynamicsBodyInput {
    pub id: DynamicsBodyId,
    pub translation: [f64; 3],
    /// Quaternion in x/y/z/w order.
    pub rotation: [f64; 4],
    pub shape: DynamicsShape,
    pub mass: f64,
    pub linear_velocity: [f64; 3],
    pub angular_velocity: [f64; 3],
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
}

#[derive(Debug, Clone, PartialEq)]
pub enum DynamicsError {
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
            Self::InvalidStep { .. } => "invalid-dynamics-step",
            Self::InvalidStepCount { .. } => "invalid-dynamics-step-count",
            Self::TooManyBodies { .. } => "dynamics-body-quota-exceeded",
            Self::TooManyActions { .. } => "dynamics-action-quota-exceeded",
            Self::TooManyContacts { .. } => "dynamics-contact-quota-exceeded",
            Self::DuplicateBody { .. } => "duplicate-dynamics-body",
            Self::UnknownActionBody { .. } => "unknown-dynamics-action-body",
            Self::InvalidBody { .. } => "invalid-dynamics-body",
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

/// Run one bounded candidate simulation from canonical inputs.
///
/// The Rapier world is rebuilt off-side for every call. It is a derived cache,
/// not persistent authority; callers publish [`DynamicsStepOutput`] only after
/// their own exact-revision transaction succeeds.
pub fn simulate_dynamics(
    projection: &CollisionProjection,
    mut input: DynamicsStepInput,
) -> Result<DynamicsStepOutput, DynamicsError> {
    validate_header(&input)?;
    input.bodies.sort_by_key(|body| body.id);
    let mut seen = BTreeSet::new();
    for body in &input.bodies {
        if !seen.insert(body.id) {
            return Err(DynamicsError::DuplicateBody { body: body.id });
        }
        validate_body(body)?;
    }
    let actions = aggregate_actions(&input.actions, &seen)?;
    validate_motion(&input, &actions)?;

    let mut world = PhysicsWorld {
        gravity: vector(input.gravity),
        integration_parameters: IntegrationParameters {
            dt: input.step_seconds,
            ..IntegrationParameters::default()
        },
        ..PhysicsWorld::default()
    };
    insert_static_environment(&mut world, projection);

    let mut handles = BTreeMap::<DynamicsBodyId, RigidBodyHandle>::new();
    for body in &input.bodies {
        let mut builder = RigidBodyBuilder::dynamic()
            .translation(vector(body.translation))
            .rotation(vector3(body.rotation))
            .linvel(vector(body.linear_velocity))
            .angvel(vector(body.angular_velocity))
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
        let collider = ColliderBuilder::new(shared_shape(body.shape))
            .mass(body.mass)
            .friction(body.friction)
            .restitution(body.restitution)
            .collision_groups(InteractionGroups::new(
                Group::from_bits_retain(body.collision_groups),
                Group::from_bits_retain(body.collision_mask),
                InteractionTestMode::And,
            ))
            .user_data(u128::from(body.id.0) + 1);
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
    for _ in 0..input.steps {
        world.step();
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
    Ok(DynamicsStepOutput { bodies, contacts })
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
    let rotation_norm = body
        .rotation
        .into_iter()
        .map(|value| value * value)
        .sum::<f64>();
    if !finite
        || !shape_valid
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
    Ok(())
}

fn aggregate_actions(
    actions: &[DynamicsAction],
    bodies: &BTreeSet<DynamicsBodyId>,
) -> Result<BTreeMap<DynamicsBodyId, AggregatedAction>, DynamicsError> {
    let mut aggregated = BTreeMap::<DynamicsBodyId, AggregatedAction>::new();
    for action in actions {
        if !bodies.contains(&action.body) {
            return Err(DynamicsError::UnknownActionBody { body: action.body });
        }
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
        entry.force += vector(action.force);
        entry.torque += vector(action.torque);
        entry.impulse += vector(action.impulse);
        entry.torque_impulse += vector(action.torque_impulse);
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
        let acceleration = action.force / body.mass + gravity * body.gravity_scale;
        let estimated_velocity = vector(body.linear_velocity)
            + action.impulse / body.mass
            + acceleration * input.step_seconds * f64::from(input.steps);
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

fn insert_static_environment(world: &mut PhysicsWorld, projection: &CollisionProjection) {
    for shape in projection.dynamics_shapes() {
        world.insert(
            RigidBodyBuilder::fixed(),
            ColliderBuilder::new(shape)
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
    let rotation = Rotation::from_xyzw(value[0], value[1], value[2], value[3]);
    rotation.to_scaled_axis()
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
                .map(|chunk| SharedShape::new(chunk.shape.clone())),
        );
        shapes.extend(self.static_meshes.dynamics_shapes());
        shapes
    }
}
