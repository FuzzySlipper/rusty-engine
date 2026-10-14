use serde::{Deserialize, Serialize};

use crate::{
    AnimationProjectionOp, AudioProjectionOp, BillboardProjectionOp, GhostPlateProjectionOp,
    ParticleProjectionOp, VideoProjectionOp,
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

    pub fn from_ops(ops: Vec<PresentationOp>) -> Self {
        Self {
            schema_version: PRESENTATION_FRAME_SCHEMA_VERSION,
            publication: None,
            ops,
        }
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
