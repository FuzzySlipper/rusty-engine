use core_ids::EntityId;
use core_math::Vec3;
use entity_state::{
    BoundsComponent, EntityState, EntityTransform, Quat, TransformCommand, TransformError,
    TransformReceipt, TransformService,
};

use crate::active_collision::active_entity_colliders;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EntityMotionCommand {
    pub entity: EntityId,
    pub delta: Vec3,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EntityMotionOutcome {
    Moved { to: Vec3 },
    Blocked { at: Vec3 },
    Slid { to: Vec3, blocked_axes: [bool; 3] },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EntityMotionResolution {
    pub entity: EntityId,
    pub from: Vec3,
    pub outcome: EntityMotionOutcome,
    pub hit: Option<EntityId>,
}

impl EntityMotionResolution {
    pub const fn resolved_position(self) -> Vec3 {
        match self.outcome {
            EntityMotionOutcome::Moved { to } | EntityMotionOutcome::Slid { to, .. } => to,
            EntityMotionOutcome::Blocked { at } => at,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EntityMotionReceipt {
    pub resolution: EntityMotionResolution,
    pub transform: TransformReceipt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityMotionError {
    Transform(TransformError),
    MissingCollider { entity: EntityId },
    MissingBounds { entity: EntityId },
    ParentedEntity { entity: EntityId },
    InvalidDelta { entity: EntityId },
}

impl std::fmt::Display for EntityMotionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "entity motion rejected: {self:?}")
    }
}

impl std::error::Error for EntityMotionError {}

impl From<TransformError> for EntityMotionError {
    fn from(value: TransformError) -> Self {
        Self::Transform(value)
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct EntityMotionService;

impl EntityMotionService {
    pub fn resolve(
        self,
        state: &EntityState,
        command: EntityMotionCommand,
    ) -> Result<EntityMotionResolution, EntityMotionError> {
        let entity = command.entity;
        TransformService.eligible(state, entity)?;
        if !is_finite(command.delta) {
            return Err(EntityMotionError::InvalidDelta { entity });
        }
        if state.transform_parent(entity).is_some() {
            return Err(EntityMotionError::ParentedEntity { entity });
        }
        let collision = state
            .active_collision(entity)
            .ok_or(EntityMotionError::MissingCollider { entity })?;
        if collision.static_collider {
            return Err(EntityMotionError::Transform(TransformError::Immovable {
                entity,
            }));
        }
        let mover_bounds = state
            .bounds(entity)
            .copied()
            .ok_or(EntityMotionError::MissingBounds { entity })?;
        let from = state
            .transform(entity)
            .expect("transform eligibility checked")
            .translation;
        let obstacles = active_obstacles(state, entity);
        let mut position = from;
        let mut blocked_axes = [false; 3];
        let mut hit = None;
        for (axis, blocked) in blocked_axes.iter_mut().enumerate() {
            let step = axis_component(command.delta, axis);
            if step == 0.0 {
                continue;
            }
            let mut candidate = position;
            set_axis(&mut candidate, axis, axis_component(position, axis) + step);
            let mover_world = offset_bounds(mover_bounds, candidate);
            if let Some((obstacle, _)) = obstacles
                .iter()
                .find(|(_, bounds)| overlaps(mover_world, *bounds))
            {
                *blocked = true;
                hit.get_or_insert(*obstacle);
            } else {
                position = candidate;
            }
        }
        let outcome = if !blocked_axes.iter().any(|blocked| *blocked) {
            EntityMotionOutcome::Moved { to: position }
        } else if position == from {
            EntityMotionOutcome::Blocked { at: from }
        } else {
            EntityMotionOutcome::Slid {
                to: position,
                blocked_axes,
            }
        };
        Ok(EntityMotionResolution {
            entity,
            from,
            outcome,
            hit,
        })
    }

    pub fn apply(
        self,
        state: &mut EntityState,
        expected_revision: u64,
        command: EntityMotionCommand,
    ) -> Result<EntityMotionReceipt, EntityMotionError> {
        if state.revision() != expected_revision {
            return Err(EntityMotionError::Transform(
                TransformError::StaleRevision {
                    expected: expected_revision,
                    actual: state.revision(),
                },
            ));
        }
        let resolution = self.resolve(state, command)?;
        let current = state
            .transform(command.entity)
            .expect("motion resolution checked transform")
            .transform();
        let transform = TransformService.apply(
            state,
            expected_revision,
            TransformCommand::Set {
                entity: command.entity,
                transform: EntityTransform {
                    translation: resolution.resolved_position(),
                    ..current
                },
            },
        )?;
        Ok(EntityMotionReceipt {
            resolution,
            transform,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FirstPersonMotionInput {
    pub move_forward: f32,
    pub move_right: f32,
    pub move_up: f32,
    pub yaw_delta_degrees: f32,
    pub pitch_delta_degrees: f32,
    pub delta_seconds: f32,
    pub speed_units_per_second: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FirstPersonMotionCommand {
    pub entity: EntityId,
    pub tick: u64,
    pub input: FirstPersonMotionInput,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FirstPersonPose {
    pub position: Vec3,
    pub yaw_degrees: f32,
    pub pitch_degrees: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FirstPersonBasis {
    pub forward: Vec3,
    pub right: Vec3,
    pub up: Vec3,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FirstPersonMotionReadout {
    pub entity: EntityId,
    pub tick: u64,
    pub pose: FirstPersonPose,
    pub basis: FirstPersonBasis,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FirstPersonMotionReceipt {
    pub entity: EntityId,
    pub tick: u64,
    pub input: FirstPersonMotionInput,
    pub from: FirstPersonPose,
    pub attempted: FirstPersonPose,
    pub to: FirstPersonPose,
    pub collision: Option<EntityMotionResolution>,
    pub transform: TransformReceipt,
    pub readout: FirstPersonMotionReadout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirstPersonMotionError {
    InvalidInput { entity: EntityId },
    Transform(TransformError),
    Motion(EntityMotionError),
}

impl std::fmt::Display for FirstPersonMotionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "first-person motion rejected: {self:?}")
    }
}

impl std::error::Error for FirstPersonMotionError {}

#[derive(Debug, Default, Clone, Copy)]
pub struct FirstPersonMotionService;

impl FirstPersonMotionService {
    pub fn apply(
        self,
        state: &mut EntityState,
        expected_revision: u64,
        command: FirstPersonMotionCommand,
    ) -> Result<FirstPersonMotionReceipt, FirstPersonMotionError> {
        self.apply_resolved(state, expected_revision, command, false)
    }

    pub fn apply_with_entity_collision(
        self,
        state: &mut EntityState,
        expected_revision: u64,
        command: FirstPersonMotionCommand,
    ) -> Result<FirstPersonMotionReceipt, FirstPersonMotionError> {
        self.apply_resolved(state, expected_revision, command, true)
    }

    fn apply_resolved(
        self,
        state: &mut EntityState,
        expected_revision: u64,
        command: FirstPersonMotionCommand,
        collide: bool,
    ) -> Result<FirstPersonMotionReceipt, FirstPersonMotionError> {
        if state.revision() != expected_revision {
            return Err(FirstPersonMotionError::Transform(
                TransformError::StaleRevision {
                    expected: expected_revision,
                    actual: state.revision(),
                },
            ));
        }
        validate_first_person_input(command.entity, command.input)?;
        TransformService
            .eligible(state, command.entity)
            .map_err(FirstPersonMotionError::Transform)?;
        let current = state
            .transform(command.entity)
            .expect("transform eligibility checked")
            .transform();
        let from = pose_from_transform(current);
        let attempted = integrate_pose(from, command.input);
        let collision = if collide {
            Some(
                EntityMotionService
                    .resolve(
                        state,
                        EntityMotionCommand {
                            entity: command.entity,
                            delta: attempted.position - from.position,
                        },
                    )
                    .map_err(FirstPersonMotionError::Motion)?,
            )
        } else {
            None
        };
        let to = FirstPersonPose {
            position: collision
                .map(EntityMotionResolution::resolved_position)
                .unwrap_or(attempted.position),
            yaw_degrees: attempted.yaw_degrees,
            pitch_degrees: attempted.pitch_degrees,
        };
        let transform = TransformService
            .apply(
                state,
                expected_revision,
                TransformCommand::Set {
                    entity: command.entity,
                    transform: EntityTransform {
                        translation: to.position,
                        rotation: quat_from_yaw_pitch(to.yaw_degrees, to.pitch_degrees),
                        ..current
                    },
                },
            )
            .map_err(FirstPersonMotionError::Transform)?;
        let readout = FirstPersonMotionReadout {
            entity: command.entity,
            tick: command.tick,
            pose: to,
            basis: basis_from_pose(to),
        };
        Ok(FirstPersonMotionReceipt {
            entity: command.entity,
            tick: command.tick,
            input: command.input,
            from,
            attempted,
            to,
            collision,
            transform,
            readout,
        })
    }
}

fn validate_first_person_input(
    entity: EntityId,
    input: FirstPersonMotionInput,
) -> Result<(), FirstPersonMotionError> {
    let values = [
        input.move_forward,
        input.move_right,
        input.move_up,
        input.yaw_delta_degrees,
        input.pitch_delta_degrees,
        input.delta_seconds,
        input.speed_units_per_second,
    ];
    if values.iter().any(|value| !value.is_finite())
        || input.delta_seconds < 0.0
        || input.speed_units_per_second < 0.0
    {
        return Err(FirstPersonMotionError::InvalidInput { entity });
    }
    Ok(())
}

fn integrate_pose(from: FirstPersonPose, input: FirstPersonMotionInput) -> FirstPersonPose {
    let basis = basis_from_pose(from);
    let distance = input.delta_seconds * input.speed_units_per_second;
    let direction = basis.forward * input.move_forward
        + basis.right * input.move_right
        + basis.up * input.move_up;
    FirstPersonPose {
        position: from.position + direction * distance,
        yaw_degrees: from.yaw_degrees + input.yaw_delta_degrees,
        pitch_degrees: (from.pitch_degrees + input.pitch_delta_degrees).clamp(-89.0, 89.0),
    }
}

fn pose_from_transform(transform: EntityTransform) -> FirstPersonPose {
    let (yaw_degrees, pitch_degrees) = yaw_pitch_from_quat(transform.rotation);
    FirstPersonPose {
        position: transform.translation,
        yaw_degrees,
        pitch_degrees,
    }
}

fn basis_from_pose(pose: FirstPersonPose) -> FirstPersonBasis {
    let yaw = pose.yaw_degrees.to_radians();
    let pitch = pose.pitch_degrees.to_radians();
    let (sin_yaw, cos_yaw) = yaw.sin_cos();
    let (sin_pitch, cos_pitch) = pitch.sin_cos();
    FirstPersonBasis {
        forward: Vec3::new(sin_yaw * cos_pitch, sin_pitch, -cos_yaw * cos_pitch),
        right: Vec3::new(cos_yaw, 0.0, sin_yaw),
        up: Vec3::new(-sin_yaw * sin_pitch, cos_pitch, cos_yaw * sin_pitch),
    }
}

fn quat_from_yaw_pitch(yaw_degrees: f32, pitch_degrees: f32) -> Quat {
    let yaw = yaw_degrees.to_radians() * 0.5;
    let pitch = pitch_degrees.to_radians() * 0.5;
    let (sin_yaw, cos_yaw) = yaw.sin_cos();
    let (sin_pitch, cos_pitch) = pitch.sin_cos();
    Quat::new(
        cos_yaw * sin_pitch,
        sin_yaw * cos_pitch,
        -sin_yaw * sin_pitch,
        cos_yaw * cos_pitch,
    )
}

fn yaw_pitch_from_quat(value: Quat) -> (f32, f32) {
    let sin_pitch = 2.0 * (value.w * value.x - value.y * value.z);
    let pitch = sin_pitch.clamp(-1.0, 1.0).asin();
    let yaw = (2.0 * (value.w * value.y - value.z * value.x))
        .atan2(1.0 - 2.0 * (value.x * value.x + value.y * value.y));
    (yaw.to_degrees(), pitch.to_degrees())
}

fn active_obstacles(state: &EntityState, mover: EntityId) -> Vec<(EntityId, BoundsComponent)> {
    active_entity_colliders(state)
        .filter(|collider| collider.entity != mover)
        .map(|collider| (collider.entity, collider.bounds))
        .collect()
}

fn offset_bounds(bounds: BoundsComponent, origin: Vec3) -> BoundsComponent {
    BoundsComponent {
        min: bounds.min + origin,
        max: bounds.max + origin,
    }
}

fn overlaps(left: BoundsComponent, right: BoundsComponent) -> bool {
    left.min.x < right.max.x
        && left.max.x > right.min.x
        && left.min.y < right.max.y
        && left.max.y > right.min.y
        && left.min.z < right.max.z
        && left.max.z > right.min.z
}

fn axis_component(value: Vec3, axis: usize) -> f32 {
    match axis {
        0 => value.x,
        1 => value.y,
        _ => value.z,
    }
}

fn set_axis(value: &mut Vec3, axis: usize, component: f32) {
    match axis {
        0 => value.x = component,
        1 => value.y = component,
        _ => value.z = component,
    }
}

fn is_finite(value: Vec3) -> bool {
    value.x.is_finite() && value.y.is_finite() && value.z.is_finite()
}
