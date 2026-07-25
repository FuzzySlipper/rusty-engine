use core_math::Vec3;
use core_time::TickDelta;

use crate::VoxelCollisionScene;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhysicsStep {
    ticks: TickDelta,
    seconds_per_tick: f32,
}

impl PhysicsStep {
    pub fn new(ticks: TickDelta, seconds_per_tick: f32) -> Result<Self, PhysicsError> {
        if !seconds_per_tick.is_finite() || seconds_per_tick <= 0.0 {
            return Err(PhysicsError::InvalidStep { seconds_per_tick });
        }
        Ok(Self {
            ticks,
            seconds_per_tick,
        })
    }

    pub const fn ticks(self) -> TickDelta {
        self.ticks
    }

    pub const fn seconds_per_tick(self) -> f32 {
        self.seconds_per_tick
    }

    pub fn elapsed_seconds(self) -> f32 {
        self.ticks.raw() as f32 * self.seconds_per_tick
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhysicsWorld {
    pub gravity: Vec3,
}

impl PhysicsWorld {
    pub const ZERO_GRAVITY: Self = Self {
        gravity: Vec3::ZERO,
    };
    pub const Y_DOWN_GRAVITY: Self = Self {
        gravity: Vec3 {
            x: 0.0,
            y: -9.8,
            z: 0.0,
        },
    };
}

impl Default for PhysicsWorld {
    fn default() -> Self {
        Self::ZERO_GRAVITY
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollisionMode {
    None,
    QueryRequired,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KinematicBody {
    pub position: Vec3,
    pub velocity: Vec3,
    pub acceleration: Vec3,
    pub gravity_scale: f32,
    pub collision_mode: CollisionMode,
}

impl KinematicBody {
    pub const fn stationary(position: Vec3) -> Self {
        Self {
            position,
            velocity: Vec3::ZERO,
            acceleration: Vec3::ZERO,
            gravity_scale: 1.0,
            collision_mode: CollisionMode::None,
        }
    }

    pub const fn with_velocity(mut self, velocity: Vec3) -> Self {
        self.velocity = velocity;
        self
    }

    pub const fn with_acceleration(mut self, acceleration: Vec3) -> Self {
        self.acceleration = acceleration;
        self
    }

    pub const fn with_gravity_scale(mut self, gravity_scale: f32) -> Self {
        self.gravity_scale = gravity_scale;
        self
    }

    pub const fn requiring_collision_query(mut self) -> Self {
        self.collision_mode = CollisionMode::QueryRequired;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KinematicShape {
    pub half_extents: Vec3,
}

impl KinematicShape {
    pub fn new(half_extents: Vec3) -> Result<Self, PhysicsError> {
        if finite_vec3(half_extents)
            && half_extents.x > 0.0
            && half_extents.y > 0.0
            && half_extents.z > 0.0
        {
            Ok(Self { half_extents })
        } else {
            Err(PhysicsError::InvalidShape)
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CollisionResolution {
    pub blocked_axes: [bool; 3],
}

impl CollisionResolution {
    pub fn was_blocked(self) -> bool {
        self.blocked_axes.into_iter().any(|blocked| blocked)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IntegrationResult {
    pub previous_position: Vec3,
    pub next_position: Vec3,
    pub previous_velocity: Vec3,
    pub next_velocity: Vec3,
    pub elapsed_seconds: f32,
    pub collision: CollisionResolution,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PhysicsError {
    InvalidStep { seconds_per_tick: f32 },
    StepOverflow,
    CollisionQueryRequired,
    InvalidShape,
    NonFiniteInput,
}

impl PhysicsError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidStep { .. } => "invalid-physics-step",
            Self::StepOverflow => "physics-step-overflow",
            Self::CollisionQueryRequired => "collision-query-required",
            Self::InvalidShape => "invalid-kinematic-shape",
            Self::NonFiniteInput => "non-finite-physics-input",
        }
    }
}

impl std::fmt::Display for PhysicsError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "physics integration rejected: {self:?}")
    }
}

impl std::error::Error for PhysicsError {}

pub trait KinematicCollisionQuery {
    fn axis_sweep_blocked(&self, bounds_min: Vec3, bounds_max: Vec3, translation: Vec3) -> bool;
}

impl KinematicCollisionQuery for VoxelCollisionScene {
    fn axis_sweep_blocked(&self, bounds_min: Vec3, bounds_max: Vec3, translation: Vec3) -> bool {
        self.axis_sweep_overlaps(
            bounds_min.to_array().map(f64::from),
            bounds_max.to_array().map(f64::from),
            translation.to_array().map(f64::from),
        )
    }
}

pub fn integrate_kinematic(
    body: KinematicBody,
    world: PhysicsWorld,
    step: PhysicsStep,
) -> Result<IntegrationResult, PhysicsError> {
    if body.collision_mode == CollisionMode::QueryRequired {
        return Err(PhysicsError::CollisionQueryRequired);
    }
    let (next_velocity, elapsed_seconds) = proposed_velocity(body, world, step)?;
    let displacement = next_velocity * elapsed_seconds;
    let next_position = body.position + displacement;
    if !finite_vec3(displacement) || !finite_vec3(next_position) {
        return Err(PhysicsError::NonFiniteInput);
    }
    Ok(IntegrationResult {
        previous_position: body.position,
        next_position,
        previous_velocity: body.velocity,
        next_velocity,
        elapsed_seconds,
        collision: CollisionResolution::default(),
    })
}

pub fn integrate_kinematic_with_query(
    body: KinematicBody,
    world: PhysicsWorld,
    step: PhysicsStep,
    shape: KinematicShape,
    query: &impl KinematicCollisionQuery,
) -> Result<IntegrationResult, PhysicsError> {
    if !finite_vec3(shape.half_extents)
        || shape.half_extents.x <= 0.0
        || shape.half_extents.y <= 0.0
        || shape.half_extents.z <= 0.0
    {
        return Err(PhysicsError::InvalidShape);
    }
    let (mut next_velocity, elapsed_seconds) = proposed_velocity(body, world, step)?;
    let displacement = next_velocity * elapsed_seconds;
    if !finite_vec3(displacement) {
        return Err(PhysicsError::NonFiniteInput);
    }
    let mut position = body.position;
    let mut collision = CollisionResolution::default();
    for axis in 0..3 {
        let mut movement = Vec3::ZERO;
        match axis {
            0 => movement.x = displacement.x,
            1 => movement.y = displacement.y,
            _ => movement.z = displacement.z,
        }
        if movement == Vec3::ZERO {
            continue;
        }
        let min = position - shape.half_extents;
        let max = position + shape.half_extents;
        if body.collision_mode == CollisionMode::QueryRequired
            && query.axis_sweep_blocked(min, max, movement)
        {
            collision.blocked_axes[axis] = true;
            match axis {
                0 => next_velocity.x = 0.0,
                1 => next_velocity.y = 0.0,
                _ => next_velocity.z = 0.0,
            }
        } else {
            position = position + movement;
            if !finite_vec3(position) {
                return Err(PhysicsError::NonFiniteInput);
            }
        }
    }
    Ok(IntegrationResult {
        previous_position: body.position,
        next_position: position,
        previous_velocity: body.velocity,
        next_velocity,
        elapsed_seconds,
        collision,
    })
}

fn proposed_velocity(
    body: KinematicBody,
    world: PhysicsWorld,
    step: PhysicsStep,
) -> Result<(Vec3, f32), PhysicsError> {
    if !finite_vec3(body.position)
        || !finite_vec3(body.velocity)
        || !finite_vec3(body.acceleration)
        || !finite_vec3(world.gravity)
        || !body.gravity_scale.is_finite()
    {
        return Err(PhysicsError::NonFiniteInput);
    }
    let elapsed_seconds = step.elapsed_seconds();
    if !elapsed_seconds.is_finite() {
        return Err(PhysicsError::StepOverflow);
    }
    let acceleration = body.acceleration + world.gravity * body.gravity_scale;
    let next_velocity = body.velocity + acceleration * elapsed_seconds;
    if !finite_vec3(next_velocity) {
        return Err(PhysicsError::NonFiniteInput);
    }
    Ok((next_velocity, elapsed_seconds))
}

fn finite_vec3(value: Vec3) -> bool {
    value.x.is_finite() && value.y.is_finite() && value.z.is_finite()
}
