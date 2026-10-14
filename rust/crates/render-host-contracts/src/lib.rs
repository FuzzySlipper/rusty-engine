//! Canonical facts the runtime hands the renderer.
//!
//! These values describe view composition (cameras, render targets, views and
//! presentations) and render output jobs. They do not assign gameplay meaning,
//! choose storage policy, or provide a generic command protocol.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

/// The longest side of an offscreen target: wgpu's default
/// `max_texture_dimension_2d`, which every supported adapter meets.
pub const MAX_RENDERER_TARGET_DIMENSION: u32 = 8_192;
/// The pixels one composition's offscreen targets hold together.
const MAX_RENDERER_TARGET_PIXELS: u64 = 8_388_608;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RendererCameraPose {
    pub position: [f64; 3],
    pub pitch_degrees: f64,
    pub yaw_degrees: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RendererCameraBasis {
    pub forward: [f64; 3],
    pub right: [f64; 3],
    pub up: [f64; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RendererCameraProjection {
    Perspective {
        #[serde(rename = "fovYDegrees")]
        fov_y_degrees: f64,
        near: f64,
        far: f64,
    },
    Orthographic {
        #[serde(rename = "verticalSize")]
        vertical_size: f64,
        near: f64,
        far: f64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RendererCameraInterpolation {
    Position,
    Pose,
}

/// Renderer-owned retained sampling metadata for an opt-in camera update.
/// Camera state remains authoritative at its published descriptor; this only
/// describes how the host may present it between product updates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RendererCameraMotion {
    pub sample_id: String,
    pub sample_time_seconds: f64,
    pub delay_seconds: f64,
    pub interpolation: RendererCameraInterpolation,
    pub cut: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RendererCompositionCamera {
    pub id: String,
    pub pose: RendererCameraPose,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub basis: Option<RendererCameraBasis>,
    pub projection: RendererCameraProjection,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motion: Option<RendererCameraMotion>,
    /// The viewmodel layer's vertical field of view in degrees; 0 (the
    /// default) draws it with the camera's own projection.
    #[serde(
        default,
        skip_serializing_if = "is_zero",
        rename = "viewmodelFovYDegrees"
    )]
    pub viewmodel_fov_y_degrees: f64,
}

fn is_zero(value: &f64) -> bool {
    *value == 0.0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RendererTargetColor {
    Rgba8Srgb,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RendererTargetDepth {
    Depth24,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RendererTargetSampling {
    Linear,
    Nearest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RendererCompositionTarget {
    pub id: String,
    pub revision: u64,
    pub width: u32,
    pub height: u32,
    pub color: RendererTargetColor,
    pub depth: RendererTargetDepth,
    pub sampling: RendererTargetSampling,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RendererViewport {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RendererViewTarget {
    Primary,
    Offscreen {
        #[serde(rename = "targetId")]
        target_id: String,
        #[serde(rename = "targetRevision")]
        target_revision: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RendererCompositionView {
    pub id: String,
    pub camera_id: String,
    pub target: RendererViewTarget,
    pub viewport: RendererViewport,
    pub order: u64,
    /// A product UI element this primary view follows: while the page
    /// reports a rect under this name, the view draws there instead of at
    /// `viewport`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viewport_anchor: Option<String>,
}

/// Named rects the product UI reports for anchored views, normalized to the
/// presentation surface like [`RendererViewport`].
pub type RendererViewportAnchors = std::collections::BTreeMap<String, RendererViewport>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RendererCompositionPresentation {
    pub id: String,
    pub source_target_id: String,
    pub source_target_revision: u64,
    pub destination: RendererPrimaryDestination,
    pub order: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RendererPrimaryDestination {
    pub kind: RendererPrimaryDestinationKind,
    pub viewport: RendererViewport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RendererPrimaryDestinationKind {
    Primary,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RendererViewComposition {
    pub cameras: Vec<RendererCompositionCamera>,
    pub targets: Vec<RendererCompositionTarget>,
    pub views: Vec<RendererCompositionView>,
    pub presentations: Vec<RendererCompositionPresentation>,
}

// Whoever admits a fact checks it with these where it enters: the C# camera
// service checks a product's camera, target or view once, not the whole
// composition after each change.

impl RendererCameraPose {
    pub fn validate(&self) -> Result<(), RendererHostContractError> {
        if self.position.into_iter().all(f64::is_finite)
            && self.pitch_degrees.is_finite()
            && self.yaw_degrees.is_finite()
        {
            Ok(())
        } else {
            Err(RendererHostContractError::InvalidNumber)
        }
    }
}

impl RendererCameraBasis {
    pub fn validate(&self) -> Result<(), RendererHostContractError> {
        let values = self.forward.into_iter().chain(self.right).chain(self.up);
        if values.clone().all(f64::is_finite)
            && self
                .forward
                .into_iter()
                .map(|value| value * value)
                .sum::<f64>()
                > f64::EPSILON
            && self.up.into_iter().map(|value| value * value).sum::<f64>() > f64::EPSILON
        {
            Ok(())
        } else {
            Err(RendererHostContractError::InvalidNumber)
        }
    }
}

impl RendererCameraProjection {
    pub fn validate(&self) -> Result<(), RendererHostContractError> {
        let (measure, near, far) = match *self {
            RendererCameraProjection::Perspective {
                fov_y_degrees,
                near,
                far,
            } => (fov_y_degrees, near, far),
            RendererCameraProjection::Orthographic {
                vertical_size,
                near,
                far,
            } => (vertical_size, near, far),
        };
        if measure.is_finite()
            && measure > 0.0
            && near.is_finite()
            && near > 0.0
            && far.is_finite()
            && far > near
        {
            Ok(())
        } else {
            Err(RendererHostContractError::InvalidNumber)
        }
    }
}

impl RendererViewport {
    pub fn validate(&self) -> Result<(), RendererHostContractError> {
        if [self.x, self.y, self.width, self.height]
            .into_iter()
            .all(f64::is_finite)
            && self.x >= 0.0
            && self.y >= 0.0
            && self.width > 0.0
            && self.height > 0.0
            && self.x + self.width <= 1.0
            && self.y + self.height <= 1.0
        {
            Ok(())
        } else {
            Err(RendererHostContractError::InvalidNumber)
        }
    }
}

impl RendererCompositionTarget {
    pub fn validate(&self) -> Result<(), RendererHostContractError> {
        validate_identifier(&self.id)?;
        if self.width == 0
            || self.height == 0
            || self.width > MAX_RENDERER_TARGET_DIMENSION
            || self.height > MAX_RENDERER_TARGET_DIMENSION
        {
            return Err(RendererHostContractError::InvalidDimension);
        }
        Ok(())
    }
}

/// The offscreen targets one composition names fit the renderer's pixel
/// budget together.
pub fn validate_target_pixels<'a>(
    targets: impl IntoIterator<Item = &'a RendererCompositionTarget>,
) -> Result<(), RendererHostContractError> {
    let mut pixels = 0_u64;
    for target in targets {
        pixels = pixels
            .checked_add(u64::from(target.width) * u64::from(target.height))
            .ok_or(RendererHostContractError::LimitExceeded)?;
    }
    if pixels > MAX_RENDERER_TARGET_PIXELS {
        return Err(RendererHostContractError::LimitExceeded);
    }
    Ok(())
}

impl RendererCompositionView {
    pub fn validate(&self) -> Result<(), RendererHostContractError> {
        validate_identifier(&self.id)?;
        validate_identifier(&self.camera_id)?;
        self.viewport.validate()?;
        if let RendererViewTarget::Offscreen { target_id, .. } = &self.target {
            validate_identifier(target_id)?;
        }
        Ok(())
    }
}

impl RendererCompositionPresentation {
    pub fn validate(&self) -> Result<(), RendererHostContractError> {
        validate_identifier(&self.id)?;
        validate_identifier(&self.source_target_id)?;
        self.destination.viewport.validate()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RendererHostContractError {
    LimitExceeded,
    InvalidIdentifier,
    InvalidNumber,
    InvalidDimension,
}

fn validate_identifier(value: &str) -> Result<(), RendererHostContractError> {
    if value.is_empty() || value.len() > 128 || value.chars().any(char::is_control) {
        Err(RendererHostContractError::InvalidIdentifier)
    } else {
        Ok(())
    }
}

mod render_output;
pub use render_output::*;
