//! Canonical facts crossing the private renderer host boundary.
//!
//! These values describe presentation, physical input, bounded resources, and
//! renderer observations. They do not assign gameplay meaning, choose storage
//! policy, expose browser objects, or provide a generic command protocol.

#![forbid(unsafe_code)]

use render_model::{RenderHandle, RenderLayer, JSON_SAFE_U64_MAX};
use serde::{Deserialize, Serialize};

pub const RENDERER_VIEW_COMPOSITION_SCHEMA_VERSION: u32 = 1;
pub const MAX_RENDERER_COMPOSITION_CAMERAS: usize = 4;
pub const MAX_RENDERER_COMPOSITION_TARGETS: usize = 4;
pub const MAX_RENDERER_COMPOSITION_VIEWS: usize = 8;
pub const MAX_RENDERER_COMPOSITION_PRESENTATIONS: usize = 4;
pub const MAX_RENDERER_TARGET_DIMENSION: u32 = 2_048;
pub const MAX_RENDERER_TARGET_PIXELS: u64 = 8_388_608;

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RendererCompositionCamera {
    pub id: String,
    pub pose: RendererCameraPose,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub basis: Option<RendererCameraBasis>,
    pub projection: RendererCameraProjection,
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
}

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
    pub schema_version: u32,
    pub cameras: Vec<RendererCompositionCamera>,
    pub targets: Vec<RendererCompositionTarget>,
    pub views: Vec<RendererCompositionView>,
    pub presentations: Vec<RendererCompositionPresentation>,
}

impl RendererViewComposition {
    pub fn validate(&self) -> Result<(), RendererHostContractError> {
        if self.schema_version != RENDERER_VIEW_COMPOSITION_SCHEMA_VERSION {
            return Err(RendererHostContractError::UnsupportedSchemaVersion);
        }
        if self.cameras.len() > MAX_RENDERER_COMPOSITION_CAMERAS
            || self.targets.len() > MAX_RENDERER_COMPOSITION_TARGETS
            || self.views.len() > MAX_RENDERER_COMPOSITION_VIEWS
            || self.presentations.len() > MAX_RENDERER_COMPOSITION_PRESENTATIONS
        {
            return Err(RendererHostContractError::LimitExceeded);
        }
        for camera in &self.cameras {
            validate_identifier(&camera.id)?;
            validate_pose(camera.pose)?;
            if let Some(basis) = camera.basis {
                validate_basis(basis)?;
            }
            validate_projection(camera.projection)?;
        }
        let mut target_pixels = 0_u64;
        for target in &self.targets {
            validate_identifier(&target.id)?;
            validate_json_safe(target.revision)?;
            if target.width == 0
                || target.height == 0
                || target.width > MAX_RENDERER_TARGET_DIMENSION
                || target.height > MAX_RENDERER_TARGET_DIMENSION
            {
                return Err(RendererHostContractError::InvalidDimension);
            }
            target_pixels = target_pixels
                .checked_add(u64::from(target.width) * u64::from(target.height))
                .ok_or(RendererHostContractError::LimitExceeded)?;
        }
        if target_pixels > MAX_RENDERER_TARGET_PIXELS {
            return Err(RendererHostContractError::LimitExceeded);
        }
        for view in &self.views {
            validate_identifier(&view.id)?;
            validate_identifier(&view.camera_id)?;
            validate_viewport(view.viewport)?;
            validate_json_safe(view.order)?;
            if let RendererViewTarget::Offscreen {
                target_id,
                target_revision,
            } = &view.target
            {
                validate_identifier(target_id)?;
                validate_json_safe(*target_revision)?;
            }
        }
        for presentation in &self.presentations {
            validate_identifier(&presentation.id)?;
            validate_identifier(&presentation.source_target_id)?;
            validate_json_safe(presentation.source_target_revision)?;
            validate_json_safe(presentation.order)?;
            validate_viewport(presentation.destination.viewport)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RendererPickRay {
    Viewport {
        point: [f64; 2],
    },
    WorldRay {
        direction: [f64; 3],
        origin: [f64; 3],
    },
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RendererPickFilter {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub handles: Vec<RenderHandle>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layers: Vec<RenderLayer>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RendererPickRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<RendererPickFilter>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_distance: Option<f64>,
    pub ray: RendererPickRay,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RendererPickHint {
    pub distance: f64,
    pub handle: RenderHandle,
    pub label: Option<String>,
    pub layer: RenderLayer,
    pub normal: [f64; 3],
    pub position: [f64; 3],
    pub source_trace: Option<RendererPickSourceTrace>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RendererPickSourceTrace {
    pub entity: u64,
    pub kind: RendererPickSourceTraceKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RendererPickSourceTraceKind {
    RenderMetadataEntity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RendererHostDiagnostic {
    pub code: String,
    pub message: String,
    #[serde(default)]
    pub sequence: Option<u64>,
    #[serde(default)]
    pub handle: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RendererPickReceipt {
    pub diagnostics: Vec<RendererHostDiagnostic>,
    pub hint: Option<RendererPickHint>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RendererPhysicalInputReadout {
    pub pressed_codes: Vec<String>,
    pub pointer: RendererPointerReadout,
    pub wheel: RendererWheelReadout,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RendererPointerReadout {
    pub x_pixels: f64,
    pub y_pixels: f64,
    pub buttons: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RendererWheelReadout {
    pub delta_x: f64,
    pub delta_y: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RendererHostContractError {
    UnsupportedSchemaVersion,
    LimitExceeded,
    InvalidIdentifier,
    InvalidNumber,
    InvalidDimension,
    OutsideJsonSafeRange,
}

fn validate_identifier(value: &str) -> Result<(), RendererHostContractError> {
    if value.is_empty() || value.len() > 128 || value.chars().any(char::is_control) {
        Err(RendererHostContractError::InvalidIdentifier)
    } else {
        Ok(())
    }
}

fn validate_pose(pose: RendererCameraPose) -> Result<(), RendererHostContractError> {
    if pose.position.into_iter().all(f64::is_finite)
        && pose.pitch_degrees.is_finite()
        && pose.yaw_degrees.is_finite()
    {
        Ok(())
    } else {
        Err(RendererHostContractError::InvalidNumber)
    }
}

fn validate_basis(basis: RendererCameraBasis) -> Result<(), RendererHostContractError> {
    let values = basis.forward.into_iter().chain(basis.right).chain(basis.up);
    if values.clone().all(f64::is_finite)
        && basis
            .forward
            .into_iter()
            .map(|value| value * value)
            .sum::<f64>()
            > f64::EPSILON
        && basis.up.into_iter().map(|value| value * value).sum::<f64>() > f64::EPSILON
    {
        Ok(())
    } else {
        Err(RendererHostContractError::InvalidNumber)
    }
}

fn validate_projection(
    projection: RendererCameraProjection,
) -> Result<(), RendererHostContractError> {
    let (measure, near, far) = match projection {
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

fn validate_viewport(viewport: RendererViewport) -> Result<(), RendererHostContractError> {
    if [viewport.x, viewport.y, viewport.width, viewport.height]
        .into_iter()
        .all(f64::is_finite)
        && viewport.x >= 0.0
        && viewport.y >= 0.0
        && viewport.width > 0.0
        && viewport.height > 0.0
        && viewport.x + viewport.width <= 1.0
        && viewport.y + viewport.height <= 1.0
    {
        Ok(())
    } else {
        Err(RendererHostContractError::InvalidNumber)
    }
}

fn validate_json_safe(value: u64) -> Result<(), RendererHostContractError> {
    if value <= JSON_SAFE_U64_MAX {
        Ok(())
    } else {
        Err(RendererHostContractError::OutsideJsonSafeRange)
    }
}
