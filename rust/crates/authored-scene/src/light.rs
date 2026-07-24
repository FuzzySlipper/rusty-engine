#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SceneLightShadowIntent {
    #[default]
    Disabled,
    Requested,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SceneLight {
    Ambient {
        color: [f32; 3],
        intensity: f32,
        enabled: bool,
        shadow_intent: SceneLightShadowIntent,
    },
    Directional {
        color: [f32; 3],
        intensity: f32,
        enabled: bool,
        shadow_intent: SceneLightShadowIntent,
    },
    Point {
        color: [f32; 3],
        intensity: f32,
        enabled: bool,
        range: Option<f32>,
        decay: f32,
        shadow_intent: SceneLightShadowIntent,
    },
    Spot {
        color: [f32; 3],
        intensity: f32,
        enabled: bool,
        range: Option<f32>,
        decay: f32,
        outer_angle_radians: f32,
        penumbra: f32,
        shadow_intent: SceneLightShadowIntent,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneLightInvalid {
    InvalidColor,
    InvalidIntensity,
    InvalidRange,
    InvalidDecay,
    InvalidSpotAngle,
    InvalidPenumbra,
    NonUnitScale,
    RequiresSchema2,
}

impl SceneLightInvalid {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidColor => "invalid-color",
            Self::InvalidIntensity => "invalid-intensity",
            Self::InvalidRange => "invalid-range",
            Self::InvalidDecay => "invalid-decay",
            Self::InvalidSpotAngle => "invalid-spot-angle",
            Self::InvalidPenumbra => "invalid-penumbra",
            Self::NonUnitScale => "non-unit-scale",
            Self::RequiresSchema2 => "requires-schema-2",
        }
    }
}

impl SceneLight {
    pub fn validate(&self) -> Result<(), SceneLightInvalid> {
        let (color, intensity) = match self {
            Self::Ambient {
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
            return Err(SceneLightInvalid::InvalidColor);
        }
        if !intensity.is_finite() || intensity < 0.0 {
            return Err(SceneLightInvalid::InvalidIntensity);
        }
        match self {
            Self::Ambient { .. } | Self::Directional { .. } => Ok(()),
            Self::Point { range, decay, .. } => validate_range_and_decay(*range, *decay),
            Self::Spot {
                range,
                decay,
                outer_angle_radians,
                penumbra,
                ..
            } => {
                validate_range_and_decay(*range, *decay)?;
                if !outer_angle_radians.is_finite()
                    || *outer_angle_radians <= 0.0
                    || *outer_angle_radians > std::f32::consts::FRAC_PI_2
                {
                    return Err(SceneLightInvalid::InvalidSpotAngle);
                }
                if !penumbra.is_finite() || !(0.0..=1.0).contains(penumbra) {
                    return Err(SceneLightInvalid::InvalidPenumbra);
                }
                Ok(())
            }
        }
    }
}

fn validate_range_and_decay(range: Option<f32>, decay: f32) -> Result<(), SceneLightInvalid> {
    if range.is_some_and(|value| !value.is_finite() || value <= 0.0) {
        return Err(SceneLightInvalid::InvalidRange);
    }
    if !decay.is_finite() || decay < 0.0 {
        return Err(SceneLightInvalid::InvalidDecay);
    }
    Ok(())
}
