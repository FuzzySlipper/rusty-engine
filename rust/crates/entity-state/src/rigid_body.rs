use core_math::Vec3;
use serde::{Deserialize, Serialize};

use crate::component::{ComponentCodec, ComponentRegistration, ComponentTypeId};
use crate::components::{
    RigidBodyComponent, RigidBodyInertiaPolicy, RigidBodyMode, RigidBodyShape,
};

pub const RIGID_BODY_COMPONENT_TYPE_ID: &str = "rusty.entity.rigid-body";
pub const RIGID_BODY_CODEC_ID: &str = "rusty.entity.rigid-body.json";
pub const RIGID_BODY_CODEC_VERSION: u32 = 1;
pub const MAX_RIGID_BODY_MASS: f32 = 1_000_000.0;
pub const MAX_RIGID_BODY_SHAPE_EXTENT: f32 = 10_000.0;
pub const MAX_RIGID_BODY_DAMPING: f32 = 1_000.0;
pub const MAX_RIGID_BODY_GRAVITY_SCALE: f32 = 100.0;
pub const MAX_RIGID_BODY_FRICTION: f32 = 10.0;
pub const MAX_RIGID_BODY_RESTITUTION: f32 = 1.0;
pub const MAX_RIGID_BODY_SPEED: f32 = 10_000.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RigidBodyValidationError {
    UnsupportedMode,
    InvalidShape,
    InvalidMass,
    UnsupportedInertiaPolicy,
    InvalidLinearVelocity,
    InvalidAngularVelocity,
    InvalidLinearDamping,
    InvalidAngularDamping,
    InvalidGravityScale,
    InvalidFriction,
    InvalidRestitution,
    EmptyCollisionGroups,
}

impl RigidBodyValidationError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedMode => "unsupported-rigid-body-mode",
            Self::InvalidShape => "invalid-rigid-body-shape",
            Self::InvalidMass => "invalid-rigid-body-mass",
            Self::UnsupportedInertiaPolicy => "unsupported-rigid-body-inertia-policy",
            Self::InvalidLinearVelocity => "invalid-rigid-body-linear-velocity",
            Self::InvalidAngularVelocity => "invalid-rigid-body-angular-velocity",
            Self::InvalidLinearDamping => "invalid-rigid-body-linear-damping",
            Self::InvalidAngularDamping => "invalid-rigid-body-angular-damping",
            Self::InvalidGravityScale => "invalid-rigid-body-gravity-scale",
            Self::InvalidFriction => "invalid-rigid-body-friction",
            Self::InvalidRestitution => "invalid-rigid-body-restitution",
            Self::EmptyCollisionGroups => "empty-rigid-body-collision-groups",
        }
    }
}

impl std::fmt::Display for RigidBodyValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for RigidBodyValidationError {}

pub fn validate_rigid_body(value: &RigidBodyComponent) -> Result<(), RigidBodyValidationError> {
    if value.mode != RigidBodyMode::Dynamic {
        return Err(RigidBodyValidationError::UnsupportedMode);
    }
    if !shape_is_valid(value.shape) {
        return Err(RigidBodyValidationError::InvalidShape);
    }
    if !value.mass.is_finite() || value.mass <= 0.0 || value.mass > MAX_RIGID_BODY_MASS {
        return Err(RigidBodyValidationError::InvalidMass);
    }
    if value.inertia != RigidBodyInertiaPolicy::DeriveFromShapeAndMass {
        return Err(RigidBodyValidationError::UnsupportedInertiaPolicy);
    }
    if !bounded_vector(value.linear_velocity, MAX_RIGID_BODY_SPEED) {
        return Err(RigidBodyValidationError::InvalidLinearVelocity);
    }
    if !bounded_vector(value.angular_velocity, MAX_RIGID_BODY_SPEED) {
        return Err(RigidBodyValidationError::InvalidAngularVelocity);
    }
    if !bounded_nonnegative(value.linear_damping, MAX_RIGID_BODY_DAMPING) {
        return Err(RigidBodyValidationError::InvalidLinearDamping);
    }
    if !bounded_nonnegative(value.angular_damping, MAX_RIGID_BODY_DAMPING) {
        return Err(RigidBodyValidationError::InvalidAngularDamping);
    }
    if !value.gravity_scale.is_finite() || value.gravity_scale.abs() > MAX_RIGID_BODY_GRAVITY_SCALE
    {
        return Err(RigidBodyValidationError::InvalidGravityScale);
    }
    if !bounded_nonnegative(value.friction, MAX_RIGID_BODY_FRICTION) {
        return Err(RigidBodyValidationError::InvalidFriction);
    }
    if !bounded_nonnegative(value.restitution, MAX_RIGID_BODY_RESTITUTION) {
        return Err(RigidBodyValidationError::InvalidRestitution);
    }
    if value.collision_groups == 0 {
        return Err(RigidBodyValidationError::EmptyCollisionGroups);
    }
    Ok(())
}

fn shape_is_valid(shape: RigidBodyShape) -> bool {
    match shape {
        RigidBodyShape::Sphere { radius } => positive_extent(radius),
        RigidBodyShape::Cuboid { half_extents } => {
            positive_extent(half_extents.x)
                && positive_extent(half_extents.y)
                && positive_extent(half_extents.z)
        }
        RigidBodyShape::CapsuleY {
            half_height,
            radius,
        } => positive_extent(half_height) && positive_extent(radius),
    }
}

fn positive_extent(value: f32) -> bool {
    value.is_finite() && value > 0.0 && value <= MAX_RIGID_BODY_SHAPE_EXTENT
}

fn bounded_nonnegative(value: f32, maximum: f32) -> bool {
    value.is_finite() && (0.0..=maximum).contains(&value)
}

fn bounded_vector(value: Vec3, maximum: f32) -> bool {
    [value.x, value.y, value.z]
        .into_iter()
        .all(|component| component.is_finite() && component.abs() <= maximum)
}

pub(crate) fn rigid_body_registration() -> ComponentRegistration<RigidBodyComponent> {
    let type_id = ComponentTypeId::parse(RIGID_BODY_COMPONENT_TYPE_ID)
        .expect("built-in rigid-body component identity is valid");
    let codec = ComponentCodec::new(
        RIGID_BODY_CODEC_ID,
        RIGID_BODY_CODEC_VERSION,
        encode,
        decode,
    )
    .expect("built-in rigid-body codec identity and version are valid");
    ComponentRegistration::durable(type_id, codec).with_validator(|value| {
        validate_rigid_body(value).map_err(|error| error.code().to_string())
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RigidBodySnapshotV1 {
    mode: RigidBodyModeV1,
    shape: RigidBodyShapeV1,
    mass: f32,
    inertia: RigidBodyInertiaPolicyV1,
    linear_velocity: [f32; 3],
    angular_velocity: [f32; 3],
    linear_damping: f32,
    angular_damping: f32,
    gravity_scale: f32,
    friction: f32,
    restitution: f32,
    collision_groups: u32,
    collision_mask: u32,
    enabled: bool,
    sleeping: bool,
    continuous_collision: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum RigidBodyModeV1 {
    Dynamic,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
enum RigidBodyShapeV1 {
    Sphere { radius: f32 },
    Cuboid { half_extents: [f32; 3] },
    CapsuleY { half_height: f32, radius: f32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum RigidBodyInertiaPolicyV1 {
    DeriveFromShapeAndMass,
}

fn encode(value: &RigidBodyComponent) -> serde_json::Value {
    serde_json::to_value(RigidBodySnapshotV1::from(*value))
        .expect("rigid-body schema-1 values always encode")
}

fn decode(value: serde_json::Value) -> Result<RigidBodyComponent, String> {
    serde_json::from_value::<RigidBodySnapshotV1>(value)
        .map(RigidBodyComponent::from)
        .map_err(|error| error.to_string())
}

impl From<RigidBodyComponent> for RigidBodySnapshotV1 {
    fn from(value: RigidBodyComponent) -> Self {
        Self {
            mode: RigidBodyModeV1::Dynamic,
            shape: match value.shape {
                RigidBodyShape::Sphere { radius } => RigidBodyShapeV1::Sphere { radius },
                RigidBodyShape::Cuboid { half_extents } => RigidBodyShapeV1::Cuboid {
                    half_extents: half_extents.to_array(),
                },
                RigidBodyShape::CapsuleY {
                    half_height,
                    radius,
                } => RigidBodyShapeV1::CapsuleY {
                    half_height,
                    radius,
                },
            },
            mass: value.mass,
            inertia: RigidBodyInertiaPolicyV1::DeriveFromShapeAndMass,
            linear_velocity: value.linear_velocity.to_array(),
            angular_velocity: value.angular_velocity.to_array(),
            linear_damping: value.linear_damping,
            angular_damping: value.angular_damping,
            gravity_scale: value.gravity_scale,
            friction: value.friction,
            restitution: value.restitution,
            collision_groups: value.collision_groups,
            collision_mask: value.collision_mask,
            enabled: value.enabled,
            sleeping: value.sleeping,
            continuous_collision: value.continuous_collision,
        }
    }
}

impl From<RigidBodySnapshotV1> for RigidBodyComponent {
    fn from(value: RigidBodySnapshotV1) -> Self {
        Self {
            mode: RigidBodyMode::Dynamic,
            shape: match value.shape {
                RigidBodyShapeV1::Sphere { radius } => RigidBodyShape::Sphere { radius },
                RigidBodyShapeV1::Cuboid { half_extents } => RigidBodyShape::Cuboid {
                    half_extents: Vec3::new(half_extents[0], half_extents[1], half_extents[2]),
                },
                RigidBodyShapeV1::CapsuleY {
                    half_height,
                    radius,
                } => RigidBodyShape::CapsuleY {
                    half_height,
                    radius,
                },
            },
            mass: value.mass,
            inertia: RigidBodyInertiaPolicy::DeriveFromShapeAndMass,
            linear_velocity: Vec3::new(
                value.linear_velocity[0],
                value.linear_velocity[1],
                value.linear_velocity[2],
            ),
            angular_velocity: Vec3::new(
                value.angular_velocity[0],
                value.angular_velocity[1],
                value.angular_velocity[2],
            ),
            linear_damping: value.linear_damping,
            angular_damping: value.angular_damping,
            gravity_scale: value.gravity_scale,
            friction: value.friction,
            restitution: value.restitution,
            collision_groups: value.collision_groups,
            collision_mask: value.collision_mask,
            enabled: value.enabled,
            sleeping: value.sleeping,
            continuous_collision: value.continuous_collision,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_one_round_trips_and_rejects_unknown_fields() {
        let body = RigidBodyComponent::dynamic(
            RigidBodyShape::Cuboid {
                half_extents: Vec3::new(0.5, 1.0, 1.5),
            },
            3.0,
        );
        let encoded = encode(&body);
        assert_eq!(decode(encoded), Ok(body));

        let mut invalid = encode(&body);
        invalid
            .as_object_mut()
            .expect("object")
            .insert("unknown".to_string(), serde_json::Value::Bool(true));
        assert!(decode(invalid).is_err());
    }

    #[test]
    fn validation_is_strict_and_typed() {
        let mut body = RigidBodyComponent::dynamic(RigidBodyShape::Sphere { radius: 0.5 }, 1.0);
        body.mass = f32::NAN;
        assert_eq!(
            validate_rigid_body(&body),
            Err(RigidBodyValidationError::InvalidMass)
        );
        body.mass = 1.0;
        body.shape = RigidBodyShape::Sphere { radius: 0.0 };
        assert_eq!(
            validate_rigid_body(&body),
            Err(RigidBodyValidationError::InvalidShape)
        );
        body.shape = RigidBodyShape::Sphere { radius: 0.5 };
        body.collision_groups = 0;
        assert_eq!(
            validate_rigid_body(&body),
            Err(RigidBodyValidationError::EmptyCollisionGroups)
        );
    }
}
