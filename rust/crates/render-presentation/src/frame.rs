use serde::{Deserialize, Serialize};

use crate::{
    AnimationProjectionOp, AudioProjectionOp, BillboardProjectionOp, ParticleProjectionOp,
    TelemetryOverlayProjectionOp,
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
    NonContiguousSequence { expected: u32, actual: u32 },
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
