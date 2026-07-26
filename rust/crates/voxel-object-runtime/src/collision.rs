use core_assets::{AssetId, AssetKind};
use serde::{Deserialize, Serialize};

use crate::{AdmittedVoxelObject, VoxelObjectRuntimeFrame};

/// Collision is an explicit, stable choice and never follows presentation
/// playback implicitly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum VoxelObjectCollisionPolicy {
    VisualOnly,
    StableDefaultFrame,
    StableClipFrame { clip: String, frame: u32 },
    ExternalProxy { asset: String },
}

#[derive(Debug, Clone, Copy)]
pub enum VoxelObjectCollisionResolution<'a> {
    VisualOnly,
    StableFrame(&'a VoxelObjectRuntimeFrame),
    ExternalProxy(&'a str),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoxelObjectCollisionError {
    UnknownClip {
        clip: String,
    },
    FrameOutOfRange {
        clip: String,
        frame: u32,
        frame_count: u32,
    },
    InvalidProxyAsset,
}

impl std::fmt::Display for VoxelObjectCollisionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownClip { clip } => write!(formatter, "unknown voxel-object clip `{clip}`"),
            Self::FrameOutOfRange {
                clip,
                frame,
                frame_count,
            } => write!(
                formatter,
                "voxel-object collision frame {frame} is outside clip `{clip}` range 0..{frame_count}"
            ),
            Self::InvalidProxyAsset => write!(
                formatter,
                "voxel-object collision proxy must be a static-mesh asset identity"
            ),
        }
    }
}

impl std::error::Error for VoxelObjectCollisionError {}

impl AdmittedVoxelObject {
    pub fn resolve_collision<'a>(
        &'a self,
        policy: &'a VoxelObjectCollisionPolicy,
    ) -> Result<VoxelObjectCollisionResolution<'a>, VoxelObjectCollisionError> {
        match policy {
            VoxelObjectCollisionPolicy::VisualOnly => {
                Ok(VoxelObjectCollisionResolution::VisualOnly)
            }
            VoxelObjectCollisionPolicy::StableDefaultFrame => Ok(
                VoxelObjectCollisionResolution::StableFrame(self.default_frame()),
            ),
            VoxelObjectCollisionPolicy::StableClipFrame { clip, frame } => {
                let runtime_clip = self
                    .clip(clip)
                    .ok_or_else(|| VoxelObjectCollisionError::UnknownClip { clip: clip.clone() })?;
                let runtime_index =
                    runtime_clip
                        .frame_indices
                        .get(*frame as usize)
                        .ok_or_else(|| VoxelObjectCollisionError::FrameOutOfRange {
                            clip: clip.clone(),
                            frame: *frame,
                            frame_count: runtime_clip.frame_indices.len() as u32,
                        })?;
                Ok(VoxelObjectCollisionResolution::StableFrame(
                    self.frame(*runtime_index)
                        .expect("admitted clip frame index is valid"),
                ))
            }
            VoxelObjectCollisionPolicy::ExternalProxy { asset } => {
                let valid = AssetId::parse(asset)
                    .is_ok_and(|identity| identity.kind() == AssetKind::StaticMesh);
                if !valid {
                    return Err(VoxelObjectCollisionError::InvalidProxyAsset);
                }
                Ok(VoxelObjectCollisionResolution::ExternalProxy(asset))
            }
        }
    }
}
