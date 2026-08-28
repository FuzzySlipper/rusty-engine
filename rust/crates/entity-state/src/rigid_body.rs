use core_math::Vec3;
use serde::{Deserialize, Serialize};

use crate::component::{ComponentCodec, ComponentRegistration, ComponentTypeId};
use crate::components::{
    RigidBodyComponent, RigidBodyInertiaPolicy, RigidBodyMode, RigidBodyShape,
};
use crate::value::Quat;

pub const RIGID_BODY_COMPONENT_TYPE_ID: &str = "rusty.entity.rigid-body";
pub const RIGID_BODY_CODEC_ID: &str = "rusty.entity.rigid-body.json";
pub const RIGID_BODY_CODEC_VERSION: u32 = 2;
pub const MAX_RIGID_BODY_MASS: f32 = 1_000_000.0;
pub const MAX_RIGID_BODY_SHAPE_EXTENT: f32 = 10_000.0;
pub const MAX_RIGID_BODY_CENTER_OF_MASS: f32 = MAX_RIGID_BODY_SHAPE_EXTENT;
pub const MAX_RIGID_BODY_PRINCIPAL_INERTIA: f32 = f32::MAX;
pub const RIGID_BODY_INERTIA_FRAME_NORMALIZATION_TOLERANCE: f32 = 0.001;
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
    InvalidCenterOfMass,
    InvalidPrincipalInertia,
    InvalidPrincipalInertiaFrame,
    InvalidLinearVelocity,
    InvalidAngularVelocity,
    LockedTranslationAxisVelocity,
    LockedRotationAxisVelocity,
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
            Self::InvalidCenterOfMass => "invalid-rigid-body-center-of-mass",
            Self::InvalidPrincipalInertia => "invalid-rigid-body-principal-inertia",
            Self::InvalidPrincipalInertiaFrame => "invalid-rigid-body-inertia-frame",
            Self::InvalidLinearVelocity => "invalid-rigid-body-linear-velocity",
            Self::InvalidAngularVelocity => "invalid-rigid-body-angular-velocity",
            Self::LockedTranslationAxisVelocity => "locked-rigid-body-translation-axis-velocity",
            Self::LockedRotationAxisVelocity => "locked-rigid-body-rotation-axis-velocity",
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
    match value.inertia {
        RigidBodyInertiaPolicy::DeriveFromShapeAndMass => {}
        RigidBodyInertiaPolicy::Explicit {
            center_of_mass,
            principal_inertia,
            principal_inertia_local_frame,
        } => {
            if !bounded_vector(center_of_mass, MAX_RIGID_BODY_CENTER_OF_MASS) {
                return Err(RigidBodyValidationError::InvalidCenterOfMass);
            }
            if !bounded_positive_vector(principal_inertia, MAX_RIGID_BODY_PRINCIPAL_INERTIA) {
                return Err(RigidBodyValidationError::InvalidPrincipalInertia);
            }
            if !normalized_quaternion(principal_inertia_local_frame) {
                return Err(RigidBodyValidationError::InvalidPrincipalInertiaFrame);
            }
        }
    }
    if !bounded_vector(value.linear_velocity, MAX_RIGID_BODY_SPEED) {
        return Err(RigidBodyValidationError::InvalidLinearVelocity);
    }
    if !bounded_vector(value.angular_velocity, MAX_RIGID_BODY_SPEED) {
        return Err(RigidBodyValidationError::InvalidAngularVelocity);
    }
    if has_velocity_on_locked_axis(value.linear_velocity, value.locked_translation_axes) {
        return Err(RigidBodyValidationError::LockedTranslationAxisVelocity);
    }
    if has_velocity_on_locked_axis(value.angular_velocity, value.locked_rotation_axes) {
        return Err(RigidBodyValidationError::LockedRotationAxisVelocity);
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

fn bounded_positive_vector(value: Vec3, maximum: f32) -> bool {
    [value.x, value.y, value.z]
        .into_iter()
        .all(|component| component.is_finite() && component > 0.0 && component <= maximum)
}

fn normalized_quaternion(value: Quat) -> bool {
    value.x.is_finite()
        && value.y.is_finite()
        && value.z.is_finite()
        && value.w.is_finite()
        && (value.norm_squared() - 1.0).abs() <= RIGID_BODY_INERTIA_FRAME_NORMALIZATION_TOLERANCE
}

fn has_velocity_on_locked_axis(velocity: Vec3, locked_axes: [bool; 3]) -> bool {
    locked_axes
        .into_iter()
        .zip(velocity.to_array())
        .any(|(locked, component)| locked && component != 0.0)
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
    .expect("built-in rigid-body codec identity and version are valid")
    .with_migration(migrate);
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
    #[serde(default)]
    locked_translation_axes: [bool; 3],
    #[serde(default)]
    locked_rotation_axes: [bool; 3],
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
            locked_translation_axes: value.locked_translation_axes,
            locked_rotation_axes: value.locked_rotation_axes,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RigidBodySnapshotV2 {
    mode: RigidBodyModeV1,
    shape: RigidBodyShapeV1,
    mass: f32,
    inertia: RigidBodyInertiaPolicyV2,
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
    #[serde(default)]
    locked_translation_axes: [bool; 3],
    #[serde(default)]
    locked_rotation_axes: [bool; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
enum RigidBodyInertiaPolicyV2 {
    DeriveFromShapeAndMass,
    Explicit {
        center_of_mass: [f32; 3],
        principal_inertia: [f32; 3],
        principal_inertia_local_frame: [f32; 4],
    },
}

fn encode(value: &RigidBodyComponent) -> serde_json::Value {
    serde_json::to_value(RigidBodySnapshotV2::from(*value))
        .expect("rigid-body schema-2 values always encode")
}

fn decode(value: serde_json::Value) -> Result<RigidBodyComponent, String> {
    serde_json::from_value::<RigidBodySnapshotV2>(value)
        .map(RigidBodyComponent::from)
        .map_err(|error| error.to_string())
}

fn migrate(version: u32, value: serde_json::Value) -> Result<RigidBodyComponent, String> {
    match version {
        1 => serde_json::from_value::<RigidBodySnapshotV1>(value)
            .map(RigidBodyComponent::from)
            .map_err(|error| error.to_string()),
        version => Err(format!("unsupported rigid-body codec version {version}")),
    }
}

impl From<RigidBodyComponent> for RigidBodySnapshotV2 {
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
            inertia: match value.inertia {
                RigidBodyInertiaPolicy::DeriveFromShapeAndMass => {
                    RigidBodyInertiaPolicyV2::DeriveFromShapeAndMass
                }
                RigidBodyInertiaPolicy::Explicit {
                    center_of_mass,
                    principal_inertia,
                    principal_inertia_local_frame,
                } => RigidBodyInertiaPolicyV2::Explicit {
                    center_of_mass: center_of_mass.to_array(),
                    principal_inertia: principal_inertia.to_array(),
                    principal_inertia_local_frame: [
                        principal_inertia_local_frame.x,
                        principal_inertia_local_frame.y,
                        principal_inertia_local_frame.z,
                        principal_inertia_local_frame.w,
                    ],
                },
            },
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
            locked_translation_axes: value.locked_translation_axes,
            locked_rotation_axes: value.locked_rotation_axes,
        }
    }
}

impl From<RigidBodySnapshotV2> for RigidBodyComponent {
    fn from(value: RigidBodySnapshotV2) -> Self {
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
            inertia: match value.inertia {
                RigidBodyInertiaPolicyV2::DeriveFromShapeAndMass => {
                    RigidBodyInertiaPolicy::DeriveFromShapeAndMass
                }
                RigidBodyInertiaPolicyV2::Explicit {
                    center_of_mass,
                    principal_inertia,
                    principal_inertia_local_frame,
                } => RigidBodyInertiaPolicy::Explicit {
                    center_of_mass: Vec3::new(
                        center_of_mass[0],
                        center_of_mass[1],
                        center_of_mass[2],
                    ),
                    principal_inertia: Vec3::new(
                        principal_inertia[0],
                        principal_inertia[1],
                        principal_inertia[2],
                    ),
                    principal_inertia_local_frame: Quat::new(
                        principal_inertia_local_frame[0],
                        principal_inertia_local_frame[1],
                        principal_inertia_local_frame[2],
                        principal_inertia_local_frame[3],
                    ),
                },
            },
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
            locked_translation_axes: value.locked_translation_axes,
            locked_rotation_axes: value.locked_rotation_axes,
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
            locked_translation_axes: value.locked_translation_axes,
            locked_rotation_axes: value.locked_rotation_axes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_two_round_trips_and_rejects_unknown_fields() {
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
    fn explicit_mass_properties_round_trip_and_validate_without_physics_assumptions() {
        let body = RigidBodyComponent::dynamic(RigidBodyShape::Sphere { radius: 0.5 }, 4.0)
            .with_explicit_inertia(
                Vec3::new(0.2, -0.1, 0.0),
                Vec3::new(0.5, 1.0, 2.0),
                Quat::IDENTITY,
            );
        assert_eq!(decode(encode(&body)), Ok(body));
        assert_eq!(validate_rigid_body(&body), Ok(()));

        let mut invalid = body;
        invalid.inertia = RigidBodyInertiaPolicy::Explicit {
            center_of_mass: Vec3::new(f32::INFINITY, 0.0, 0.0),
            principal_inertia: Vec3::ONE,
            principal_inertia_local_frame: Quat::IDENTITY,
        };
        assert_eq!(
            validate_rigid_body(&invalid),
            Err(RigidBodyValidationError::InvalidCenterOfMass)
        );

        invalid.inertia = RigidBodyInertiaPolicy::Explicit {
            center_of_mass: Vec3::ZERO,
            principal_inertia: Vec3::new(0.0, 1.0, 1.0),
            principal_inertia_local_frame: Quat::IDENTITY,
        };
        assert_eq!(
            validate_rigid_body(&invalid),
            Err(RigidBodyValidationError::InvalidPrincipalInertia)
        );

        invalid.inertia = RigidBodyInertiaPolicy::Explicit {
            center_of_mass: Vec3::ZERO,
            principal_inertia: Vec3::ONE,
            principal_inertia_local_frame: Quat::new(0.0, 0.0, 0.0, 2.0),
        };
        assert_eq!(
            validate_rigid_body(&invalid),
            Err(RigidBodyValidationError::InvalidPrincipalInertiaFrame)
        );
    }

    #[test]
    fn schema_one_derive_payload_migrates_to_schema_two() {
        let body = RigidBodyComponent::dynamic(
            RigidBodyShape::Cuboid {
                half_extents: Vec3::new(0.5, 1.0, 1.5),
            },
            3.0,
        );
        let old = serde_json::to_value(RigidBodySnapshotV1::from(body)).expect("schema one value");

        assert_eq!(migrate(1, old), Ok(body));
        assert!(migrate(0, serde_json::Value::Null).is_err());
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
        body.collision_groups = u32::MAX;
        body.locked_translation_axes = [false, true, false];
        body.linear_velocity = Vec3::new(0.0, 1.0, 0.0);
        assert_eq!(
            validate_rigid_body(&body),
            Err(RigidBodyValidationError::LockedTranslationAxisVelocity)
        );
        body.linear_velocity = Vec3::ZERO;
        body.locked_rotation_axes = [true, false, true];
        body.angular_velocity = Vec3::new(1.0, 0.0, 0.0);
        assert_eq!(
            validate_rigid_body(&body),
            Err(RigidBodyValidationError::LockedRotationAxisVelocity)
        );
    }

    #[test]
    fn old_schema_one_snapshot_defaults_axis_locks_to_unlocked() {
        let body = RigidBodyComponent::dynamic(RigidBodyShape::Sphere { radius: 0.5 }, 1.0);
        let mut old = encode(&body);
        let object = old.as_object_mut().expect("rigid-body object");
        object.remove("lockedTranslationAxes");
        object.remove("lockedRotationAxes");

        assert_eq!(decode(old), Ok(body));
    }
}
