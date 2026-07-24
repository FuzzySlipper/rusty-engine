use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{PresentationOp, PresentationOpMeta};

const MAX_TITLE_BYTES: usize = 96;
const MIN_REFRESH_INTERVAL_MS: u32 = 100;
const MAX_REFRESH_INTERVAL_MS: u32 = 5_000;
const MIN_FRAME_TIME_SAMPLES: u16 = 1;
const MAX_FRAME_TIME_SAMPLES: u16 = 240;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TelemetryOverlayHandle(u64);

impl TelemetryOverlayHandle {
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TelemetryOverlayCorner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TelemetryOverlayDescriptor {
    pub title: String,
    pub corner: TelemetryOverlayCorner,
    pub refresh_interval_ms: u32,
    pub max_frame_time_samples: u16,
    pub visible: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TelemetryOverlayPatch {
    pub title: Option<String>,
    pub corner: Option<TelemetryOverlayCorner>,
    pub refresh_interval_ms: Option<u32>,
    pub max_frame_time_samples: Option<u16>,
    pub visible: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "op",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum TelemetryOverlayProjectionOp {
    Create {
        handle: TelemetryOverlayHandle,
        descriptor: TelemetryOverlayDescriptor,
    },
    Update {
        handle: TelemetryOverlayHandle,
        patch: TelemetryOverlayPatch,
    },
    Destroy {
        handle: TelemetryOverlayHandle,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TelemetryOverlayDiagnosticCode {
    InvalidDescriptor,
    DuplicateHandle,
    UnknownHandle,
    UnavailableHost,
    SnapshotUnavailable,
    HostFailure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TelemetryOverlayDiagnostic {
    pub code: TelemetryOverlayDiagnosticCode,
    pub sequence: u32,
    pub handle: Option<TelemetryOverlayHandle>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TelemetryOverlayReadout {
    pub active_overlays: u32,
    pub diagnostics: Vec<TelemetryOverlayDiagnostic>,
}

#[derive(Debug, Clone, Default)]
pub struct TelemetryOverlayProjector {
    active: BTreeMap<TelemetryOverlayHandle, TelemetryOverlayDescriptor>,
    diagnostics: Vec<TelemetryOverlayDiagnostic>,
}

impl TelemetryOverlayProjector {
    pub fn project(
        &mut self,
        meta: PresentationOpMeta,
        op: TelemetryOverlayProjectionOp,
    ) -> Result<PresentationOp, TelemetryOverlayDiagnostic> {
        let mut projected = self.project_batch(vec![(meta, op)])?;
        Ok(projected.pop().expect("one input produces one operation"))
    }

    pub fn project_batch(
        &mut self,
        ops: Vec<(PresentationOpMeta, TelemetryOverlayProjectionOp)>,
    ) -> Result<Vec<PresentationOp>, TelemetryOverlayDiagnostic> {
        let mut staged = self.clone();
        let mut projected = Vec::with_capacity(ops.len());
        for (meta, op) in ops {
            if let Err(code) = staged.validate_and_apply(&op) {
                let diagnostic = TelemetryOverlayDiagnostic {
                    code,
                    sequence: meta.sequence,
                    handle: operation_handle(&op),
                    message: diagnostic_message(code).to_string(),
                };
                self.diagnostics.push(diagnostic.clone());
                return Err(diagnostic);
            }
            projected.push(PresentationOp::TelemetryOverlay { meta, op });
        }
        *self = staged;
        Ok(projected)
    }

    pub fn descriptor(
        &self,
        handle: TelemetryOverlayHandle,
    ) -> Option<&TelemetryOverlayDescriptor> {
        self.active.get(&handle)
    }

    pub fn readout(&self) -> TelemetryOverlayReadout {
        TelemetryOverlayReadout {
            active_overlays: self.active.len() as u32,
            diagnostics: self.diagnostics.clone(),
        }
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    fn validate_and_apply(
        &mut self,
        op: &TelemetryOverlayProjectionOp,
    ) -> Result<(), TelemetryOverlayDiagnosticCode> {
        match op {
            TelemetryOverlayProjectionOp::Create { handle, descriptor } => {
                if self.active.contains_key(handle) {
                    return Err(TelemetryOverlayDiagnosticCode::DuplicateHandle);
                }
                validate_descriptor(descriptor)?;
                self.active.insert(*handle, descriptor.clone());
            }
            TelemetryOverlayProjectionOp::Update { handle, patch } => {
                let current = self
                    .active
                    .get(handle)
                    .cloned()
                    .ok_or(TelemetryOverlayDiagnosticCode::UnknownHandle)?;
                let updated = apply_patch(current, patch);
                validate_descriptor(&updated)?;
                self.active.insert(*handle, updated);
            }
            TelemetryOverlayProjectionOp::Destroy { handle } => {
                if self.active.remove(handle).is_none() {
                    return Err(TelemetryOverlayDiagnosticCode::UnknownHandle);
                }
            }
        }
        Ok(())
    }
}

fn validate_descriptor(
    descriptor: &TelemetryOverlayDescriptor,
) -> Result<(), TelemetryOverlayDiagnosticCode> {
    if descriptor.title.is_empty()
        || descriptor.title.len() > MAX_TITLE_BYTES
        || !(MIN_REFRESH_INTERVAL_MS..=MAX_REFRESH_INTERVAL_MS)
            .contains(&descriptor.refresh_interval_ms)
        || !(MIN_FRAME_TIME_SAMPLES..=MAX_FRAME_TIME_SAMPLES)
            .contains(&descriptor.max_frame_time_samples)
    {
        return Err(TelemetryOverlayDiagnosticCode::InvalidDescriptor);
    }
    Ok(())
}

fn apply_patch(
    mut descriptor: TelemetryOverlayDescriptor,
    patch: &TelemetryOverlayPatch,
) -> TelemetryOverlayDescriptor {
    if let Some(value) = &patch.title {
        descriptor.title = value.clone();
    }
    if let Some(value) = patch.corner {
        descriptor.corner = value;
    }
    if let Some(value) = patch.refresh_interval_ms {
        descriptor.refresh_interval_ms = value;
    }
    if let Some(value) = patch.max_frame_time_samples {
        descriptor.max_frame_time_samples = value;
    }
    if let Some(value) = patch.visible {
        descriptor.visible = value;
    }
    descriptor
}

fn operation_handle(op: &TelemetryOverlayProjectionOp) -> Option<TelemetryOverlayHandle> {
    Some(match op {
        TelemetryOverlayProjectionOp::Create { handle, .. }
        | TelemetryOverlayProjectionOp::Update { handle, .. }
        | TelemetryOverlayProjectionOp::Destroy { handle } => *handle,
    })
}

const fn diagnostic_message(code: TelemetryOverlayDiagnosticCode) -> &'static str {
    match code {
        TelemetryOverlayDiagnosticCode::InvalidDescriptor => {
            "telemetry overlay descriptor is invalid"
        }
        TelemetryOverlayDiagnosticCode::DuplicateHandle => {
            "telemetry overlay handle is already active"
        }
        TelemetryOverlayDiagnosticCode::UnknownHandle => "telemetry overlay handle is not active",
        TelemetryOverlayDiagnosticCode::UnavailableHost => "telemetry overlay host is unavailable",
        TelemetryOverlayDiagnosticCode::SnapshotUnavailable => "telemetry snapshot is unavailable",
        TelemetryOverlayDiagnosticCode::HostFailure => "telemetry overlay host failed",
    }
}
