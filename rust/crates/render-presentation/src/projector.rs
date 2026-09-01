use serde::{Deserialize, Serialize};

use crate::{
    AnimationProjectionDiagnostic, AnimationProjectionReadout, AnimationProjector,
    AudioProjectionDiagnostic, AudioProjectionReadout, AudioProjector,
    BillboardProjectionDiagnostic, BillboardProjectionReadout, BillboardProjector,
    GhostPlateProjectionDiagnostic, GhostPlateProjectionReadout, GhostPlateProjector,
    ParticleProjectionDiagnostic, ParticleProjectionReadout, ParticleProjector,
    PresentationAssetLookup, PresentationFrameDiff, PresentationFrameError, PresentationOp,
    RenderTargetLookup, TelemetryOverlayDiagnostic, TelemetryOverlayProjector,
    TelemetryOverlayReadout,
};

#[derive(Debug, Clone, PartialEq)]
pub enum PresentationProjectionError {
    Frame(PresentationFrameError),
    Audio(AudioProjectionDiagnostic),
    Billboard(BillboardProjectionDiagnostic),
    Particle(ParticleProjectionDiagnostic),
    Telemetry(TelemetryOverlayDiagnostic),
    Animation(AnimationProjectionDiagnostic),
    GhostPlate(GhostPlateProjectionDiagnostic),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PresentationProjectionReadout {
    pub audio: AudioProjectionReadout,
    pub billboards: BillboardProjectionReadout,
    pub particles: ParticleProjectionReadout,
    pub telemetry: TelemetryOverlayReadout,
    pub animation: AnimationProjectionReadout,
    pub ghost_plates: GhostPlateProjectionReadout,
}

/// All presentation families can be advanced as one transaction when a host
/// needs a single ordered frame boundary.
#[derive(Debug, Clone, Default)]
pub struct PresentationProjectorSet {
    audio: AudioProjector,
    billboards: BillboardProjector,
    particles: ParticleProjector,
    telemetry: TelemetryOverlayProjector,
    animation: AnimationProjector,
    ghost_plates: GhostPlateProjector,
}

impl PresentationProjectorSet {
    pub fn project_frame(
        &mut self,
        assets: &impl PresentationAssetLookup,
        targets: &impl RenderTargetLookup,
        frame: PresentationFrameDiff,
    ) -> Result<PresentationFrameDiff, PresentationProjectionError> {
        frame
            .validate()
            .map_err(PresentationProjectionError::Frame)?;
        let mut staged = self.clone();
        let mut projected = Vec::with_capacity(frame.ops.len());
        for operation in frame.ops {
            let output = match operation {
                PresentationOp::Audio { meta, op } => staged
                    .audio
                    .project(assets, meta, op)
                    .map_err(PresentationProjectionError::Audio)?,
                PresentationOp::Billboard { meta, op } => staged
                    .billboards
                    .project(assets, meta, op)
                    .map_err(PresentationProjectionError::Billboard)?,
                PresentationOp::Particle { meta, op } => staged
                    .particles
                    .project(assets, meta, op)
                    .map_err(PresentationProjectionError::Particle)?,
                PresentationOp::TelemetryOverlay { meta, op } => staged
                    .telemetry
                    .project(meta, op)
                    .map_err(PresentationProjectionError::Telemetry)?,
                PresentationOp::Animation { meta, op } => staged
                    .animation
                    .project(assets, targets, meta, op)
                    .map_err(PresentationProjectionError::Animation)?,
                PresentationOp::GhostPlate { meta, op } => staged
                    .ghost_plates
                    .project(targets, meta, op)
                    .map_err(PresentationProjectionError::GhostPlate)?,
            };
            projected.push(output);
        }
        let output = PresentationFrameDiff::try_from_ops(projected)
            .map_err(PresentationProjectionError::Frame)?;
        *self = staged;
        Ok(output)
    }

    pub fn readout(&self) -> PresentationProjectionReadout {
        PresentationProjectionReadout {
            audio: self.audio.readout(),
            billboards: self.billboards.readout(),
            particles: self.particles.readout(),
            telemetry: self.telemetry.readout(),
            animation: self.animation.readout(),
            ghost_plates: self.ghost_plates.readout(),
        }
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}
