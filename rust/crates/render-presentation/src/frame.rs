use render_model::JSON_SAFE_U64_MAX;
use serde::{Deserialize, Serialize};

use crate::{
    AnimationControllerProjectionState, AnimationProjectionOp, AudioEmitter, AudioProjectionOp,
    BillboardAnchor, BillboardProjectionOp, GhostPlateProjectionOp, ParticleAnchor,
    ParticleEmitterDescriptor, ParticleProjectionOp, VideoProjectionOp,
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
    Animation {
        meta: PresentationOpMeta,
        op: AnimationProjectionOp,
    },
    GhostPlate {
        meta: PresentationOpMeta,
        op: GhostPlateProjectionOp,
    },
    Video {
        meta: PresentationOpMeta,
        op: VideoProjectionOp,
    },
}

impl PresentationOp {
    pub const fn meta(&self) -> PresentationOpMeta {
        match self {
            Self::Audio { meta, .. }
            | Self::Billboard { meta, .. }
            | Self::Particle { meta, .. }
            | Self::Animation { meta, .. }
            | Self::GhostPlate { meta, .. }
            | Self::Video { meta, .. } => *meta,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PresentationFrameDiff {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publication: Option<render_model::RenderFramePublication>,
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
            publication: None,
            ops: Vec::new(),
        }
    }

    pub fn try_from_ops(ops: Vec<PresentationOp>) -> Result<Self, PresentationFrameError> {
        let frame = Self {
            schema_version: PRESENTATION_FRAME_SCHEMA_VERSION,
            publication: None,
            ops,
        };
        frame.validate()?;
        Ok(frame)
    }

    /// Every integer a frame carries is exactly representable in JSON, where
    /// it crosses to the browser host. The descriptors themselves were
    /// validated by the projector that emitted them, and the sequence and
    /// schema are set by Engine code.
    pub fn validate(&self) -> Result<(), PresentationFrameError> {
        for op in &self.ops {
            validate_json_safe_integers(op, op.meta().sequence)?;
        }
        Ok(())
    }

    /// Transient audio events from the current operation may accompany its
    /// full baseline once. They are never copied into retained recovery state.
    pub fn transient_events(&self) -> Self {
        let mut ops = Vec::new();
        for op in &self.ops {
            let meta = PresentationOpMeta::new(ops.len() as u32);
            match op {
                PresentationOp::Audio {
                    op: op @ AudioProjectionOp::Emit { .. },
                    ..
                } => {
                    ops.push(PresentationOp::Audio {
                        meta,
                        op: op.clone(),
                    });
                }
                PresentationOp::Audio {
                    op: op @ AudioProjectionOp::Retire { .. },
                    ..
                } => {
                    ops.push(PresentationOp::Audio {
                        meta,
                        op: op.clone(),
                    });
                }
                PresentationOp::Particle {
                    op: op @ ParticleProjectionOp::Emit { .. },
                    ..
                } => {
                    ops.push(PresentationOp::Particle {
                        meta,
                        op: op.clone(),
                    });
                }
                _ => {}
            }
        }
        Self { ops, ..Self::new() }
    }

    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentationFrameError {
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
            AudioProjectionOp::Emit {
                signal_handle,
                descriptor,
                ..
            } => {
                json_safe(signal_handle.raw(), sequence, "audio.signalHandle")?;
                audio_emitter(&descriptor.emitter, sequence)
            }
            AudioProjectionOp::Retire { signal_handle } => {
                json_safe(signal_handle.raw(), sequence, "audio.signalHandle")
            }
            AudioProjectionOp::Create { handle, descriptor }
            | AudioProjectionOp::Restore {
                handle, descriptor, ..
            } => {
                json_safe(handle.raw(), sequence, "audio.handle")?;
                audio_emitter(&descriptor.emitter, sequence)
            }
            AudioProjectionOp::Update { handle, patch } => {
                json_safe(handle.raw(), sequence, "audio.handle")?;
                patch
                    .emitter
                    .as_ref()
                    .map_or(Ok(()), |emitter| audio_emitter(emitter, sequence))
            }
            AudioProjectionOp::Destroy { handle }
            | AudioProjectionOp::VoiceControl { handle, .. } => {
                json_safe(handle.raw(), sequence, "audio.handle")
            }
            AudioProjectionOp::BusControl { .. } => Ok(()),
        },
        PresentationOp::Video { op, .. } => match op {
            VideoProjectionOp::Play { handle, .. }
            | VideoProjectionOp::Stop { handle }
            | VideoProjectionOp::Skip { handle } => {
                json_safe(handle.raw(), sequence, "video.handle")
            }
        },
        PresentationOp::Billboard { op, .. } => match op {
            BillboardProjectionOp::Create { handle, descriptor } => {
                json_safe(handle.raw(), sequence, "billboard.handle")?;
                billboard_anchor(&descriptor.anchor, sequence)
            }
            BillboardProjectionOp::Update { handle, patch } => {
                json_safe(handle.raw(), sequence, "billboard.handle")?;
                patch
                    .anchor
                    .as_ref()
                    .map_or(Ok(()), |anchor| billboard_anchor(anchor, sequence))
            }
            BillboardProjectionOp::Destroy { handle } => {
                json_safe(handle.raw(), sequence, "billboard.handle")
            }
        },
        PresentationOp::Particle { op, .. } => match op {
            ParticleProjectionOp::Emit { descriptor, .. } => particle(descriptor, sequence),
            ParticleProjectionOp::Create { handle, descriptor } => {
                json_safe(handle.raw(), sequence, "particle.handle")?;
                particle(descriptor, sequence)
            }
            ParticleProjectionOp::Update { handle, patch } => {
                json_safe(handle.raw(), sequence, "particle.handle")?;
                patch
                    .anchor
                    .as_ref()
                    .map_or(Ok(()), |anchor| particle_anchor(anchor, sequence))
            }
            ParticleProjectionOp::Destroy { handle } => {
                json_safe(handle.raw(), sequence, "particle.handle")
            }
        },
        PresentationOp::Animation { op, .. } => match op {
            AnimationProjectionOp::Create { handle, descriptor } => {
                json_safe(handle.raw(), sequence, "animation.handle")?;
                json_safe(descriptor.target.raw(), sequence, "animation.target")?;
                animation_controller(&descriptor.controller, sequence)
            }
            AnimationProjectionOp::Update { handle, controller } => {
                json_safe(handle.raw(), sequence, "animation.handle")?;
                animation_controller(controller, sequence)
            }
            AnimationProjectionOp::Destroy { handle } => {
                json_safe(handle.raw(), sequence, "animation.handle")
            }
        },
        PresentationOp::GhostPlate { op, .. } => match op {
            GhostPlateProjectionOp::Create { handle, descriptor } => {
                json_safe(handle.raw(), sequence, "ghostPlate.handle")?;
                json_safe(descriptor.source.raw(), sequence, "ghostPlate.source")
            }
            GhostPlateProjectionOp::Update { handle, .. }
            | GhostPlateProjectionOp::Recapture { handle, .. }
            | GhostPlateProjectionOp::Destroy { handle } => {
                json_safe(handle.raw(), sequence, "ghostPlate.handle")
            }
        },
    }
}

fn audio_emitter(emitter: &AudioEmitter, sequence: u32) -> Result<(), PresentationFrameError> {
    match emitter {
        AudioEmitter::EntityAttached { entity, .. } => {
            json_safe(*entity, sequence, "audio.emitter.entity")
        }
        AudioEmitter::Global2d | AudioEmitter::World3d { .. } => Ok(()),
    }
}

fn billboard_anchor(anchor: &BillboardAnchor, sequence: u32) -> Result<(), PresentationFrameError> {
    match anchor {
        BillboardAnchor::EntityAttached { entity, .. } => {
            json_safe(*entity, sequence, "billboard.anchor.entity")
        }
        BillboardAnchor::World { .. } => Ok(()),
    }
}

fn particle(
    descriptor: &ParticleEmitterDescriptor,
    sequence: u32,
) -> Result<(), PresentationFrameError> {
    json_safe(descriptor.seed, sequence, "particle.seed")?;
    particle_anchor(&descriptor.anchor, sequence)
}

fn particle_anchor(anchor: &ParticleAnchor, sequence: u32) -> Result<(), PresentationFrameError> {
    match anchor {
        ParticleAnchor::EntityAttached { entity, .. } => {
            json_safe(*entity, sequence, "particle.anchor.entity")
        }
        ParticleAnchor::World { .. } => Ok(()),
    }
}

fn animation_controller(
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
