use core_math::Vec3;
use serde::{Deserialize, Serialize};

use crate::component::{ComponentCodec, ComponentRegistration, ComponentTypeId};
use crate::{
    CharacterMotionComponent, CharacterStance, Quat, MAX_ABS_TRANSLATION, MAX_ABS_VELOCITY,
};

pub const CHARACTER_MOTION_COMPONENT_TYPE_ID: &str = "rusty.entity.character-motion";
pub const CHARACTER_MOTION_CODEC_ID: &str = "rusty.entity.character-motion.json";
pub const CHARACTER_MOTION_CODEC_VERSION: u32 = 1;
pub const MAX_CHARACTER_TIMER_SECONDS: f32 = 60.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharacterMotionValidationError {
    InvalidControlledVelocity,
    InvalidExternalVelocity,
    InvalidTimer,
    InvalidSupportAnchor,
    InvalidSupportTransform,
    InvalidSupportVelocity,
    InvalidFallHeight,
}

impl CharacterMotionValidationError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidControlledVelocity => "invalid-character-controlled-velocity",
            Self::InvalidExternalVelocity => "invalid-character-external-velocity",
            Self::InvalidTimer => "invalid-character-motion-timer",
            Self::InvalidSupportAnchor => "invalid-character-support-anchor",
            Self::InvalidSupportTransform => "invalid-character-support-transform",
            Self::InvalidSupportVelocity => "invalid-character-support-velocity",
            Self::InvalidFallHeight => "invalid-character-fall-height",
        }
    }
}

impl std::fmt::Display for CharacterMotionValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for CharacterMotionValidationError {}

pub fn validate_character_motion(
    value: &CharacterMotionComponent,
) -> Result<(), CharacterMotionValidationError> {
    if !bounded_vector(value.controlled_velocity, MAX_ABS_VELOCITY) {
        return Err(CharacterMotionValidationError::InvalidControlledVelocity);
    }
    if !bounded_vector(value.external_velocity, MAX_ABS_VELOCITY) {
        return Err(CharacterMotionValidationError::InvalidExternalVelocity);
    }
    if [
        value.jump_buffer_remaining,
        value.coyote_remaining,
        value.landing_lockout_remaining,
    ]
    .into_iter()
    .any(|timer| !timer.is_finite() || !(0.0..=MAX_CHARACTER_TIMER_SECONDS).contains(&timer))
    {
        return Err(CharacterMotionValidationError::InvalidTimer);
    }
    if !bounded_vector(value.support_local_anchor, MAX_ABS_TRANSLATION) {
        return Err(CharacterMotionValidationError::InvalidSupportAnchor);
    }
    if !bounded_vector(value.support_previous_translation, MAX_ABS_TRANSLATION)
        || !quat_is_valid(value.support_previous_rotation)
    {
        return Err(CharacterMotionValidationError::InvalidSupportTransform);
    }
    if !bounded_vector(value.support_point_velocity, MAX_ABS_VELOCITY) {
        return Err(CharacterMotionValidationError::InvalidSupportVelocity);
    }
    if !value.fall_origin_y.is_finite()
        || !value.peak_y.is_finite()
        || value.fall_origin_y.abs() > MAX_ABS_TRANSLATION
        || value.peak_y.abs() > MAX_ABS_TRANSLATION
    {
        return Err(CharacterMotionValidationError::InvalidFallHeight);
    }
    Ok(())
}

pub(crate) fn character_motion_registration() -> ComponentRegistration<CharacterMotionComponent> {
    let type_id = ComponentTypeId::parse(CHARACTER_MOTION_COMPONENT_TYPE_ID)
        .expect("built-in character-motion identity is valid");
    let codec = ComponentCodec::new(
        CHARACTER_MOTION_CODEC_ID,
        CHARACTER_MOTION_CODEC_VERSION,
        encode,
        decode,
    )
    .expect("built-in character-motion codec is valid");
    ComponentRegistration::durable(type_id, codec).with_validator(|value| {
        validate_character_motion(value).map_err(|error| error.code().to_string())
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CharacterMotionSnapshotV1 {
    controlled_velocity: [f32; 3],
    external_velocity: [f32; 3],
    stance: CharacterStanceV1,
    grounded: bool,
    jump_buffer_remaining: f32,
    coyote_remaining: f32,
    landing_lockout_remaining: f32,
    support_entity: Option<u64>,
    support_local_anchor: [f32; 3],
    support_previous_translation: [f32; 3],
    support_previous_rotation: [f32; 4],
    support_point_velocity: [f32; 3],
    fall_origin_y: f32,
    peak_y: f32,
    last_command_sequence: u64,
    collision_world_hash: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum CharacterStanceV1 {
    Standing,
    Crouched,
}

fn encode(value: &CharacterMotionComponent) -> serde_json::Value {
    serde_json::to_value(CharacterMotionSnapshotV1::from(*value))
        .expect("valid character motion always encodes")
}

fn decode(value: serde_json::Value) -> Result<CharacterMotionComponent, String> {
    serde_json::from_value::<CharacterMotionSnapshotV1>(value)
        .map(CharacterMotionComponent::from)
        .map_err(|error| error.to_string())
}

impl From<CharacterMotionComponent> for CharacterMotionSnapshotV1 {
    fn from(value: CharacterMotionComponent) -> Self {
        Self {
            controlled_velocity: value.controlled_velocity.to_array(),
            external_velocity: value.external_velocity.to_array(),
            stance: match value.stance {
                CharacterStance::Standing => CharacterStanceV1::Standing,
                CharacterStance::Crouched => CharacterStanceV1::Crouched,
            },
            grounded: value.grounded,
            jump_buffer_remaining: value.jump_buffer_remaining,
            coyote_remaining: value.coyote_remaining,
            landing_lockout_remaining: value.landing_lockout_remaining,
            support_entity: value.support_entity.map(|entity| entity.raw()),
            support_local_anchor: value.support_local_anchor.to_array(),
            support_previous_translation: value.support_previous_translation.to_array(),
            support_previous_rotation: [
                value.support_previous_rotation.x,
                value.support_previous_rotation.y,
                value.support_previous_rotation.z,
                value.support_previous_rotation.w,
            ],
            support_point_velocity: value.support_point_velocity.to_array(),
            fall_origin_y: value.fall_origin_y,
            peak_y: value.peak_y,
            last_command_sequence: value.last_command_sequence,
            collision_world_hash: value.collision_world_hash,
        }
    }
}

impl From<CharacterMotionSnapshotV1> for CharacterMotionComponent {
    fn from(value: CharacterMotionSnapshotV1) -> Self {
        Self {
            controlled_velocity: vec3(value.controlled_velocity),
            external_velocity: vec3(value.external_velocity),
            stance: match value.stance {
                CharacterStanceV1::Standing => CharacterStance::Standing,
                CharacterStanceV1::Crouched => CharacterStance::Crouched,
            },
            grounded: value.grounded,
            jump_buffer_remaining: value.jump_buffer_remaining,
            coyote_remaining: value.coyote_remaining,
            landing_lockout_remaining: value.landing_lockout_remaining,
            support_entity: value.support_entity.map(core_ids::EntityId::new),
            support_local_anchor: vec3(value.support_local_anchor),
            support_previous_translation: vec3(value.support_previous_translation),
            support_previous_rotation: Quat::new(
                value.support_previous_rotation[0],
                value.support_previous_rotation[1],
                value.support_previous_rotation[2],
                value.support_previous_rotation[3],
            ),
            support_point_velocity: vec3(value.support_point_velocity),
            fall_origin_y: value.fall_origin_y,
            peak_y: value.peak_y,
            last_command_sequence: value.last_command_sequence,
            collision_world_hash: value.collision_world_hash,
        }
    }
}

fn vec3(value: [f32; 3]) -> Vec3 {
    Vec3::new(value[0], value[1], value[2])
}
fn bounded_vector(value: Vec3, maximum: f32) -> bool {
    [value.x, value.y, value.z]
        .into_iter()
        .all(|v| v.is_finite() && v.abs() <= maximum)
}
fn quat_is_valid(value: Quat) -> bool {
    [value.x, value.y, value.z, value.w]
        .into_iter()
        .all(f32::is_finite)
        && (value.norm_squared() - 1.0).abs() <= 1.0e-3
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_round_trips_and_validation_rejects_bad_timer() {
        let motion = CharacterMotionComponent::at_rest(3.0);
        assert_eq!(decode(encode(&motion)), Ok(motion));
        let mut invalid = motion;
        invalid.coyote_remaining = f32::NAN;
        assert_eq!(
            validate_character_motion(&invalid),
            Err(CharacterMotionValidationError::InvalidTimer)
        );
    }
}
