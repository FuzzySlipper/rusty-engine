use std::collections::BTreeMap;

use render_model::{RenderHandle, Transform, JSON_SAFE_U64_MAX};
use serde::{Deserialize, Serialize};

use crate::{PresentationOp, PresentationOpMeta, RenderTargetLookup};

pub const GHOST_PLATE_MIN_CAPTURE_RESOLUTION: u16 = 8;
pub const GHOST_PLATE_MAX_CAPTURE_RESOLUTION: u16 = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GhostPlateHandle(u64);

impl GhostPlateHandle {
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }
    pub const fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GhostPlateCaptureLightingMode {
    Scene,
    Isolated,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GhostPlateCaptureLighting {
    pub mode: GhostPlateCaptureLightingMode,
    pub ambient_color: [f32; 3],
    pub ambient_intensity: f32,
    pub key_direction: [f32; 3],
    pub key_color: [f32; 3],
    pub key_intensity: f32,
    pub fill_direction: [f32; 3],
    pub fill_color: [f32; 3],
    pub fill_intensity: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GhostPlateCaptureSettings {
    pub resolution: u16,
    pub azimuth_degrees: f32,
    pub elevation_degrees: f32,
    pub near: f32,
    pub far: f32,
    pub field_of_view_degrees: f32,
    pub lighting: GhostPlateCaptureLighting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GhostPlateAnchorPolicy {
    BoundsCenter,
    BoundsNormalized,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GhostPlateMapping {
    PlateLocked,
    ProjectiveSurface,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GhostPlateShellMode {
    WholeMesh,
    StrictSource,
    RepairedSource,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GhostPlateConfig {
    pub depth_retention: f32,
    pub anchor_policy: GhostPlateAnchorPolicy,
    pub anchor_value: f32,
    pub plate_mapping: GhostPlateMapping,
    pub shell_mode: GhostPlateShellMode,
    pub shell_depth_epsilon: f32,
    pub sector_count: u8,
    pub sector_hysteresis_degrees: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GhostPlatePlacement {
    pub transform: Transform,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GhostPlateDescriptor {
    /// Engine-owned retained render identity. Backend resources never cross this boundary.
    pub source: RenderHandle,
    pub placement: GhostPlatePlacement,
    pub capture: GhostPlateCaptureSettings,
    pub config: GhostPlateConfig,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GhostPlatePatch {
    pub placement: Option<GhostPlatePlacement>,
    pub config: Option<GhostPlateConfig>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "op",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum GhostPlateProjectionOp {
    Create {
        handle: GhostPlateHandle,
        descriptor: GhostPlateDescriptor,
    },
    Update {
        handle: GhostPlateHandle,
        patch: GhostPlatePatch,
    },
    /// Rebuilds the retained capture bank atomically; omitted settings recapture with the current settings.
    Recapture {
        handle: GhostPlateHandle,
        capture: Option<GhostPlateCaptureSettings>,
    },
    Destroy {
        handle: GhostPlateHandle,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GhostPlateProjectionDiagnosticCode {
    InvalidDescriptor,
    DuplicateHandle,
    UnknownHandle,
    UnknownSource,
    UnavailableHost,
    HostFailure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GhostPlateProjectionDiagnostic {
    pub code: GhostPlateProjectionDiagnosticCode,
    pub sequence: u32,
    pub handle: Option<GhostPlateHandle>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GhostPlateProjectionReadout {
    pub active_plates: u32,
    pub diagnostics: Vec<GhostPlateProjectionDiagnostic>,
}

#[derive(Debug, Clone, Default)]
pub struct GhostPlateProjector {
    active: BTreeMap<GhostPlateHandle, GhostPlateDescriptor>,
    diagnostics: Vec<GhostPlateProjectionDiagnostic>,
}

impl GhostPlateProjector {
    pub fn project(
        &mut self,
        targets: &impl RenderTargetLookup,
        meta: PresentationOpMeta,
        op: GhostPlateProjectionOp,
    ) -> Result<PresentationOp, GhostPlateProjectionDiagnostic> {
        let mut output = self.project_batch(targets, vec![(meta, op)])?;
        Ok(output.pop().expect("one input produces one operation"))
    }

    pub fn project_batch(
        &mut self,
        targets: &impl RenderTargetLookup,
        ops: Vec<(PresentationOpMeta, GhostPlateProjectionOp)>,
    ) -> Result<Vec<PresentationOp>, GhostPlateProjectionDiagnostic> {
        let mut staged = self.clone();
        let mut output = Vec::with_capacity(ops.len());
        for (meta, op) in ops {
            if let Err(code) = staged.validate_and_apply(targets, &op) {
                let diagnostic = GhostPlateProjectionDiagnostic {
                    code,
                    sequence: meta.sequence,
                    handle: operation_handle(&op),
                    message: diagnostic_message(code).to_string(),
                };
                self.diagnostics.push(diagnostic.clone());
                return Err(diagnostic);
            }
            output.push(PresentationOp::GhostPlate { meta, op });
        }
        *self = staged;
        Ok(output)
    }

    pub fn descriptor(&self, handle: GhostPlateHandle) -> Option<&GhostPlateDescriptor> {
        self.active.get(&handle)
    }
    pub fn readout(&self) -> GhostPlateProjectionReadout {
        GhostPlateProjectionReadout {
            active_plates: self.active.len() as u32,
            diagnostics: self.diagnostics.clone(),
        }
    }
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    fn validate_and_apply(
        &mut self,
        targets: &impl RenderTargetLookup,
        op: &GhostPlateProjectionOp,
    ) -> Result<(), GhostPlateProjectionDiagnosticCode> {
        match op {
            GhostPlateProjectionOp::Create { handle, descriptor } => {
                if handle.raw() > JSON_SAFE_U64_MAX {
                    return Err(GhostPlateProjectionDiagnosticCode::InvalidDescriptor);
                }
                if self.active.contains_key(handle) {
                    return Err(GhostPlateProjectionDiagnosticCode::DuplicateHandle);
                }
                validate_descriptor(targets, descriptor)?;
                self.active.insert(*handle, descriptor.clone());
            }
            GhostPlateProjectionOp::Update { handle, patch } => {
                let current = self
                    .active
                    .get(handle)
                    .cloned()
                    .ok_or(GhostPlateProjectionDiagnosticCode::UnknownHandle)?;
                let next = apply_patch(current, patch);
                validate_descriptor(targets, &next)?;
                self.active.insert(*handle, next);
            }
            GhostPlateProjectionOp::Recapture { handle, capture } => {
                let mut current = self
                    .active
                    .get(handle)
                    .cloned()
                    .ok_or(GhostPlateProjectionDiagnosticCode::UnknownHandle)?;
                if let Some(capture) = capture {
                    current.capture = capture.clone();
                }
                validate_descriptor(targets, &current)?;
                self.active.insert(*handle, current);
            }
            GhostPlateProjectionOp::Destroy { handle } => {
                if self.active.remove(handle).is_none() {
                    return Err(GhostPlateProjectionDiagnosticCode::UnknownHandle);
                }
            }
        }
        Ok(())
    }
}

fn apply_patch(
    mut descriptor: GhostPlateDescriptor,
    patch: &GhostPlatePatch,
) -> GhostPlateDescriptor {
    if let Some(placement) = &patch.placement {
        descriptor.placement = placement.clone();
    }
    if let Some(config) = &patch.config {
        descriptor.config = config.clone();
    }
    descriptor
}

fn validate_descriptor(
    targets: &impl RenderTargetLookup,
    descriptor: &GhostPlateDescriptor,
) -> Result<(), GhostPlateProjectionDiagnosticCode> {
    if descriptor.source.raw() > JSON_SAFE_U64_MAX
        || !targets.contains_render_target(descriptor.source)
    {
        return Err(GhostPlateProjectionDiagnosticCode::UnknownSource);
    }
    descriptor
        .placement
        .transform
        .validate()
        .map_err(|_| GhostPlateProjectionDiagnosticCode::InvalidDescriptor)?;
    finite_range(descriptor.placement.width, 0.05, 64.0)?;
    finite_range(descriptor.placement.height, 0.05, 64.0)?;
    let capture = &descriptor.capture;
    if !(GHOST_PLATE_MIN_CAPTURE_RESOLUTION..=GHOST_PLATE_MAX_CAPTURE_RESOLUTION)
        .contains(&capture.resolution)
        || finite_range(capture.azimuth_degrees, -360.0, 360.0).is_err()
        || finite_range(capture.elevation_degrees, -89.0, 89.0).is_err()
        || finite_range(capture.near, 0.001, 100.0).is_err()
        || !capture.far.is_finite()
        || capture.far <= capture.near + 0.001
        || capture.far > 10_000.0
        || finite_range(capture.field_of_view_degrees, 10.0, 120.0).is_err()
        || !valid_lighting(&capture.lighting)
    {
        return Err(GhostPlateProjectionDiagnosticCode::InvalidDescriptor);
    }
    let config = &descriptor.config;
    if finite_range(config.depth_retention, 0.02, 1.0).is_err()
        || finite_range(config.anchor_value, 0.0, 1.0).is_err()
        || finite_range(config.shell_depth_epsilon, 0.0, 2.0).is_err()
        || finite_range(config.sector_hysteresis_degrees, 0.0, 22.5).is_err()
        || ![1, 4, 8, 16].contains(&config.sector_count)
    {
        return Err(GhostPlateProjectionDiagnosticCode::InvalidDescriptor);
    }
    Ok(())
}

fn valid_lighting(lighting: &GhostPlateCaptureLighting) -> bool {
    lighting
        .ambient_color
        .iter()
        .chain(lighting.key_color.iter())
        .chain(lighting.fill_color.iter())
        .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
        && [
            lighting.ambient_intensity,
            lighting.key_intensity,
            lighting.fill_intensity,
        ]
        .iter()
        .all(|v| v.is_finite() && (0.0..=8.0).contains(v))
        && [lighting.key_direction, lighting.fill_direction]
            .iter()
            .all(|direction| {
                direction.iter().all(|v| v.is_finite())
                    && direction.iter().map(|v| v * v).sum::<f32>() > 1e-8
            })
}

fn finite_range(
    value: f32,
    minimum: f32,
    maximum: f32,
) -> Result<(), GhostPlateProjectionDiagnosticCode> {
    if value.is_finite() && (minimum..=maximum).contains(&value) {
        Ok(())
    } else {
        Err(GhostPlateProjectionDiagnosticCode::InvalidDescriptor)
    }
}

fn operation_handle(op: &GhostPlateProjectionOp) -> Option<GhostPlateHandle> {
    Some(match op {
        GhostPlateProjectionOp::Create { handle, .. }
        | GhostPlateProjectionOp::Update { handle, .. }
        | GhostPlateProjectionOp::Recapture { handle, .. }
        | GhostPlateProjectionOp::Destroy { handle } => *handle,
    })
}

const fn diagnostic_message(code: GhostPlateProjectionDiagnosticCode) -> &'static str {
    match code {
        GhostPlateProjectionDiagnosticCode::InvalidDescriptor => {
            "ghost plate descriptor is invalid"
        }
        GhostPlateProjectionDiagnosticCode::DuplicateHandle => {
            "ghost plate handle is already active"
        }
        GhostPlateProjectionDiagnosticCode::UnknownHandle => "ghost plate handle is not active",
        GhostPlateProjectionDiagnosticCode::UnknownSource => {
            "ghost plate retained source is unavailable"
        }
        GhostPlateProjectionDiagnosticCode::UnavailableHost => "ghost plate host is unavailable",
        GhostPlateProjectionDiagnosticCode::HostFailure => "ghost plate host failed",
    }
}
