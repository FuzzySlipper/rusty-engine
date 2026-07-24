use render_model::JSON_SAFE_U64_MAX;
use serde::{Deserialize, Serialize};

use crate::{
    AnimationControllerProjectionState, AnimationProjectionOp, AudioEmitter, AudioProjectionOp,
    AudioSourceDescriptor, AudioSourcePatch, BillboardAnchor, BillboardDescriptor, BillboardPatch,
    BillboardProjectionOp, ParticleAnchor, ParticleEmitterDescriptor, ParticleEmitterPatch,
    ParticleProjectionOp, TelemetryOverlayProjectionOp,
};

pub const PRESENTATION_FRAME_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PresentationOpMeta {
    pub sequence: u32,
}

impl PresentationOpMeta {
    pub const fn new(sequence: u32) -> Self {
        Self { sequence }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "domain", rename_all = "camelCase", deny_unknown_fields)]
pub enum PresentationOp {
    Audio {
        meta: PresentationOpMeta,
        op: AudioProjectionOp,
    },
    Billboard {
        meta: PresentationOpMeta,
        op: BillboardProjectionOp,
    },
    Particle {
        meta: PresentationOpMeta,
        op: ParticleProjectionOp,
    },
    TelemetryOverlay {
        meta: PresentationOpMeta,
        op: TelemetryOverlayProjectionOp,
    },
    Animation {
        meta: PresentationOpMeta,
        op: AnimationProjectionOp,
    },
}

impl PresentationOp {
    pub const fn meta(&self) -> PresentationOpMeta {
        match self {
            Self::Audio { meta, .. }
            | Self::Billboard { meta, .. }
            | Self::Particle { meta, .. }
            | Self::TelemetryOverlay { meta, .. }
            | Self::Animation { meta, .. } => *meta,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PresentationFrameDiff {
    pub schema_version: u32,
    pub ops: Vec<PresentationOp>,
}

impl Default for PresentationFrameDiff {
    fn default() -> Self {
        Self::new()
    }
}

impl PresentationFrameDiff {
    pub const fn new() -> Self {
        Self {
            schema_version: PRESENTATION_FRAME_SCHEMA_VERSION,
            ops: Vec::new(),
        }
    }

    pub fn try_from_ops(ops: Vec<PresentationOp>) -> Result<Self, PresentationFrameError> {
        let frame = Self {
            schema_version: PRESENTATION_FRAME_SCHEMA_VERSION,
            ops,
        };
        frame.validate()?;
        Ok(frame)
    }

    pub fn validate(&self) -> Result<(), PresentationFrameError> {
        if self.schema_version != PRESENTATION_FRAME_SCHEMA_VERSION {
            return Err(PresentationFrameError::UnsupportedSchemaVersion(
                self.schema_version,
            ));
        }
        for (index, op) in self.ops.iter().enumerate() {
            let expected = u32::try_from(index).map_err(|_| PresentationFrameError::TooManyOps)?;
            let actual = op.meta().sequence;
            if actual != expected {
                return Err(PresentationFrameError::NonContiguousSequence { expected, actual });
            }
            validate_json_safe_integers(op, actual)?;
        }
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    pub fn encode_json(&self) -> Result<String, PresentationJsonError> {
        self.validate()
            .map_err(PresentationJsonError::InvalidFrame)?;
        serde_json::to_string_pretty(self).map_err(PresentationJsonError::Encode)
    }

    pub fn decode_json(input: &str) -> Result<Self, PresentationJsonError> {
        let frame: Self = serde_json::from_str(input).map_err(PresentationJsonError::Decode)?;
        frame
            .validate()
            .map_err(PresentationJsonError::InvalidFrame)?;
        Ok(frame)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentationFrameError {
    UnsupportedSchemaVersion(u32),
    TooManyOps,
    NonContiguousSequence {
        expected: u32,
        actual: u32,
    },
    UnsafeJsonInteger {
        sequence: u32,
        field: &'static str,
        value: u64,
    },
}

fn validate_json_safe_integers(
    op: &PresentationOp,
    sequence: u32,
) -> Result<(), PresentationFrameError> {
    match op {
        PresentationOp::Audio { op, .. } => match op {
            AudioProjectionOp::Emit { descriptor, .. } => validate_audio(descriptor, sequence),
            AudioProjectionOp::Create { handle, descriptor } => {
                json_safe(handle.raw(), sequence, "audio.handle")?;
                validate_audio(descriptor, sequence)
            }
            AudioProjectionOp::Update { handle, patch } => {
                json_safe(handle.raw(), sequence, "audio.handle")?;
                validate_audio_patch(patch, sequence)
            }
            AudioProjectionOp::Destroy { handle } => {
                json_safe(handle.raw(), sequence, "audio.handle")
            }
        },
        PresentationOp::Billboard { op, .. } => match op {
            BillboardProjectionOp::Create { handle, descriptor } => {
                json_safe(handle.raw(), sequence, "billboard.handle")?;
                validate_billboard(descriptor, sequence)
            }
            BillboardProjectionOp::Update { handle, patch } => {
                json_safe(handle.raw(), sequence, "billboard.handle")?;
                validate_billboard_patch(patch, sequence)
            }
            BillboardProjectionOp::Destroy { handle } => {
                json_safe(handle.raw(), sequence, "billboard.handle")
            }
        },
        PresentationOp::Particle { op, .. } => match op {
            ParticleProjectionOp::Emit { descriptor, .. } => {
                validate_particle(descriptor, sequence)
            }
            ParticleProjectionOp::Create { handle, descriptor } => {
                json_safe(handle.raw(), sequence, "particle.handle")?;
                validate_particle(descriptor, sequence)
            }
            ParticleProjectionOp::Update { handle, patch } => {
                json_safe(handle.raw(), sequence, "particle.handle")?;
                validate_particle_patch(patch, sequence)
            }
            ParticleProjectionOp::Destroy { handle } => {
                json_safe(handle.raw(), sequence, "particle.handle")
            }
        },
        PresentationOp::TelemetryOverlay { op, .. } => {
            let handle = match op {
                TelemetryOverlayProjectionOp::Create { handle, .. }
                | TelemetryOverlayProjectionOp::Update { handle, .. }
                | TelemetryOverlayProjectionOp::Destroy { handle } => handle,
            };
            json_safe(handle.raw(), sequence, "telemetryOverlay.handle")
        }
        PresentationOp::Animation { op, .. } => match op {
            AnimationProjectionOp::Create { handle, descriptor } => {
                json_safe(handle.raw(), sequence, "animation.handle")?;
                json_safe(descriptor.target.raw(), sequence, "animation.target")?;
                validate_animation_controller(&descriptor.controller, sequence)
            }
            AnimationProjectionOp::Update { handle, controller } => {
                json_safe(handle.raw(), sequence, "animation.handle")?;
                validate_animation_controller(controller, sequence)
            }
            AnimationProjectionOp::Destroy { handle } => {
                json_safe(handle.raw(), sequence, "animation.handle")
            }
        },
    }
}

fn validate_audio(
    descriptor: &AudioSourceDescriptor,
    sequence: u32,
) -> Result<(), PresentationFrameError> {
    validate_audio_emitter(&descriptor.emitter, sequence)
}

fn validate_audio_patch(
    patch: &AudioSourcePatch,
    sequence: u32,
) -> Result<(), PresentationFrameError> {
    patch
        .emitter
        .as_ref()
        .map_or(Ok(()), |emitter| validate_audio_emitter(emitter, sequence))
}

fn validate_audio_emitter(
    emitter: &AudioEmitter,
    sequence: u32,
) -> Result<(), PresentationFrameError> {
    match emitter {
        AudioEmitter::EntityAttached { entity, .. } => {
            json_safe(*entity, sequence, "audio.emitter.entity")
        }
        AudioEmitter::Global2d | AudioEmitter::World3d { .. } => Ok(()),
    }
}

fn validate_billboard(
    descriptor: &BillboardDescriptor,
    sequence: u32,
) -> Result<(), PresentationFrameError> {
    validate_billboard_anchor(&descriptor.anchor, sequence)
}

fn validate_billboard_patch(
    patch: &BillboardPatch,
    sequence: u32,
) -> Result<(), PresentationFrameError> {
    patch
        .anchor
        .as_ref()
        .map_or(Ok(()), |anchor| validate_billboard_anchor(anchor, sequence))
}

fn validate_billboard_anchor(
    anchor: &BillboardAnchor,
    sequence: u32,
) -> Result<(), PresentationFrameError> {
    match anchor {
        BillboardAnchor::EntityAttached { entity, .. } => {
            json_safe(*entity, sequence, "billboard.anchor.entity")
        }
        BillboardAnchor::World { .. } => Ok(()),
    }
}

fn validate_particle(
    descriptor: &ParticleEmitterDescriptor,
    sequence: u32,
) -> Result<(), PresentationFrameError> {
    json_safe(descriptor.seed, sequence, "particle.seed")?;
    validate_particle_anchor(&descriptor.anchor, sequence)
}

fn validate_particle_patch(
    patch: &ParticleEmitterPatch,
    sequence: u32,
) -> Result<(), PresentationFrameError> {
    patch
        .anchor
        .as_ref()
        .map_or(Ok(()), |anchor| validate_particle_anchor(anchor, sequence))
}

fn validate_particle_anchor(
    anchor: &ParticleAnchor,
    sequence: u32,
) -> Result<(), PresentationFrameError> {
    match anchor {
        ParticleAnchor::EntityAttached { entity, .. } => {
            json_safe(*entity, sequence, "particle.anchor.entity")
        }
        ParticleAnchor::World { .. } => Ok(()),
    }
}

fn validate_animation_controller(
    controller: &AnimationControllerProjectionState,
    sequence: u32,
) -> Result<(), PresentationFrameError> {
    json_safe(controller.entity, sequence, "animation.controller.entity")?;
    json_safe(
        controller.revision,
        sequence,
        "animation.controller.revision",
    )?;
    json_safe(
        controller.controller_tick,
        sequence,
        "animation.controller.controllerTick",
    )?;
    if let Some(fact) = &controller.transition_fact {
        json_safe(
            fact.controller_tick,
            sequence,
            "animation.controller.transitionFact.controllerTick",
        )?;
    }
    Ok(())
}

fn json_safe(value: u64, sequence: u32, field: &'static str) -> Result<(), PresentationFrameError> {
    if value <= JSON_SAFE_U64_MAX {
        Ok(())
    } else {
        Err(PresentationFrameError::UnsafeJsonInteger {
            sequence,
            field,
            value,
        })
    }
}

#[derive(Debug)]
pub enum PresentationJsonError {
    Encode(serde_json::Error),
    Decode(serde_json::Error),
    InvalidFrame(PresentationFrameError),
}

impl core::fmt::Display for PresentationJsonError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for PresentationJsonError {}
