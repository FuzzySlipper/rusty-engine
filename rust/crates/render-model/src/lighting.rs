use serde::{Deserialize, Serialize};

pub const MAX_RENDER_LIGHT_INTENSITY: f32 = 10_000.0;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LightShadowIntent {
    #[default]
    Disabled,
    Requested,
}

/// How a light's requested shadow renders (render-wgpu `shadows.rs`); the
/// default leaves every choice to the renderer.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LightShadowSettings {
    /// Texels on a side of each of the light's shadow layers: 256, 512,
    /// 1024 or 2048 (other values round up to one of them, at most 2048);
    /// 0 for the renderer's default for the light's kind.
    pub resolution: u32,
    /// Which requested shadows a shadow budget keeps: higher first, then
    /// nearer the camera.
    pub priority: i32,
    /// A wider, softer filter: 5×5 samples rather than 3×3.
    pub soft: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum LightDescriptor {
    Ambient {
        color: [f32; 3],
        intensity: f32,
        enabled: bool,
        /// With a requested shadow, half the side of the square of sky the
        /// light looks down over, in metres; the renderer's default (32 m)
        /// when absent.
        #[serde(default)]
        range: Option<f32>,
        /// Requested: the light is the sky's, reaching a surface only where
        /// the sky above it is open (render-wgpu `shadows.rs`).
        shadow_intent: LightShadowIntent,
        #[serde(default)]
        shadow: LightShadowSettings,
    },
    /// Sky light from above and ground light from below, blended by how far
    /// a surface faces up: `color` is the sky's, `ground_color` the ground's.
    /// It casts no shadow.
    Hemisphere {
        color: [f32; 3],
        ground_color: [f32; 3],
        intensity: f32,
        enabled: bool,
    },
    Directional {
        color: [f32; 3],
        intensity: f32,
        enabled: bool,
        direction: [f32; 3],
        /// How far from the camera its requested shadow reaches; the
        /// renderer's default when absent (render-wgpu `shadows.rs`).
        range: Option<f32>,
        shadow_intent: LightShadowIntent,
        #[serde(default)]
        shadow: LightShadowSettings,
    },
    Point {
        color: [f32; 3],
        intensity: f32,
        enabled: bool,
        position: [f32; 3],
        range: Option<f32>,
        decay: f32,
        shadow_intent: LightShadowIntent,
        #[serde(default)]
        shadow: LightShadowSettings,
    },
    Spot {
        color: [f32; 3],
        intensity: f32,
        enabled: bool,
        position: [f32; 3],
        direction: [f32; 3],
        range: Option<f32>,
        decay: f32,
        outer_angle_radians: f32,
        penumbra: f32,
        shadow_intent: LightShadowIntent,
        #[serde(default)]
        shadow: LightShadowSettings,
    },
}

impl LightDescriptor {
    pub fn validate(&self) -> Result<(), LightDescriptorError> {
        let (color, intensity) = match self {
            Self::Ambient {
                color, intensity, ..
            }
            | Self::Hemisphere {
                color, intensity, ..
            }
            | Self::Directional {
                color, intensity, ..
            }
            | Self::Point {
                color, intensity, ..
            }
            | Self::Spot {
                color, intensity, ..
            } => (color, *intensity),
        };
        if !color
            .iter()
            .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
        {
            return Err(LightDescriptorError::InvalidColor);
        }
        if !intensity.is_finite() || !(0.0..=MAX_RENDER_LIGHT_INTENSITY).contains(&intensity) {
            return Err(LightDescriptorError::InvalidIntensity);
        }
        let valid_color = |color: &[f32; 3]| {
            color
                .iter()
                .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
        };
        match self {
            Self::Ambient { range, .. } => validate_range_decay(*range, 0.0),
            Self::Hemisphere { ground_color, .. } => valid_color(ground_color)
                .then_some(())
                .ok_or(LightDescriptorError::InvalidColor),
            Self::Directional {
                direction, range, ..
            } => {
                validate_direction(*direction)?;
                validate_range_decay(*range, 0.0)
            }
            Self::Point {
                position,
                range,
                decay,
                ..
            } => {
                validate_position(*position)?;
                validate_range_decay(*range, *decay)
            }
            Self::Spot {
                position,
                direction,
                range,
                decay,
                outer_angle_radians,
                penumbra,
                ..
            } => {
                validate_position(*position)?;
                validate_direction(*direction)?;
                validate_range_decay(*range, *decay)?;
                if !outer_angle_radians.is_finite()
                    || *outer_angle_radians <= 0.0
                    || *outer_angle_radians > std::f32::consts::FRAC_PI_2
                {
                    return Err(LightDescriptorError::InvalidSpotAngle);
                }
                if !penumbra.is_finite() || !(0.0..=1.0).contains(penumbra) {
                    return Err(LightDescriptorError::InvalidPenumbra);
                }
                Ok(())
            }
        }
    }

    pub const fn shadow_settings(&self) -> LightShadowSettings {
        match self {
            Self::Ambient { shadow, .. }
            | Self::Directional { shadow, .. }
            | Self::Point { shadow, .. }
            | Self::Spot { shadow, .. } => *shadow,
            Self::Hemisphere { .. } => LightShadowSettings {
                resolution: 0,
                priority: 0,
                soft: false,
            },
        }
    }

    pub const fn shadow_intent(&self) -> LightShadowIntent {
        match self {
            Self::Ambient { shadow_intent, .. }
            | Self::Directional { shadow_intent, .. }
            | Self::Point { shadow_intent, .. }
            | Self::Spot { shadow_intent, .. } => *shadow_intent,
            Self::Hemisphere { .. } => LightShadowIntent::Disabled,
        }
    }
}

fn validate_position(position: [f32; 3]) -> Result<(), LightDescriptorError> {
    position
        .iter()
        .all(|value| value.is_finite())
        .then_some(())
        .ok_or(LightDescriptorError::InvalidPosition)
}

fn validate_direction(direction: [f32; 3]) -> Result<(), LightDescriptorError> {
    if !direction.iter().all(|value| value.is_finite()) {
        return Err(LightDescriptorError::InvalidDirection);
    }
    let length_squared = direction.iter().map(|value| value * value).sum::<f32>();
    (length_squared > f32::EPSILON)
        .then_some(())
        .ok_or(LightDescriptorError::InvalidDirection)
}

fn validate_range_decay(range: Option<f32>, decay: f32) -> Result<(), LightDescriptorError> {
    if range.is_some_and(|value| !value.is_finite() || value <= 0.0) {
        return Err(LightDescriptorError::InvalidRange);
    }
    if !decay.is_finite() || decay < 0.0 {
        return Err(LightDescriptorError::InvalidDecay);
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LightDescriptorError {
    InvalidColor,
    InvalidIntensity,
    InvalidPosition,
    InvalidDirection,
    InvalidRange,
    InvalidDecay,
    InvalidSpotAngle,
    InvalidPenumbra,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_ordinary_light_kinds_validate() {
        let lights = [
            LightDescriptor::Ambient {
                color: [0.2, 0.3, 0.4],
                intensity: 0.5,
                enabled: true,
                shadow_intent: LightShadowIntent::Disabled,
                shadow: Default::default(),
                range: None,
            },
            LightDescriptor::Directional {
                color: [1.0, 0.9, 0.8],
                intensity: 2.0,
                enabled: true,
                direction: [-1.0, -2.0, -1.0],
                range: Some(80.0),
                shadow_intent: LightShadowIntent::Requested,
                shadow: Default::default(),
            },
            LightDescriptor::Point {
                color: [1.0, 0.4, 0.2],
                intensity: 4.0,
                enabled: true,
                position: [2.0, 3.0, 4.0],
                range: Some(12.0),
                decay: 2.0,
                shadow_intent: LightShadowIntent::Disabled,
                shadow: Default::default(),
            },
            LightDescriptor::Spot {
                color: [0.4, 0.6, 1.0],
                intensity: 6.0,
                enabled: true,
                position: [0.0, 8.0, 0.0],
                direction: [0.0, -1.0, 0.0],
                range: Some(20.0),
                decay: 2.0,
                outer_angle_radians: 0.7,
                penumbra: 0.25,
                shadow_intent: LightShadowIntent::Requested,
                shadow: Default::default(),
            },
        ];
        assert!(lights.iter().all(|light| light.validate().is_ok()));
    }

    #[test]
    fn light_intensity_has_a_shared_hard_ceiling() {
        let exact = LightDescriptor::Ambient {
            color: [1.0; 3],
            intensity: MAX_RENDER_LIGHT_INTENSITY,
            enabled: true,
            shadow_intent: LightShadowIntent::Disabled,
            shadow: Default::default(),
            range: None,
        };
        let over = LightDescriptor::Ambient {
            color: [1.0; 3],
            intensity: MAX_RENDER_LIGHT_INTENSITY + 1.0,
            enabled: true,
            shadow_intent: LightShadowIntent::Disabled,
            shadow: Default::default(),
            range: None,
        };
        assert_eq!(exact.validate(), Ok(()));
        assert_eq!(over.validate(), Err(LightDescriptorError::InvalidIntensity));
    }
}
