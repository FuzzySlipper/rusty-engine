use serde::{Deserialize, Serialize};

use crate::{
    resolve_voxel_frame, VoxelAssetBounds, VoxelAssetMaterialBinding, VoxelAssetMaterialMapping,
    VoxelCoordinateSystem, VoxelFrame, VoxelFrameCell, VoxelFrameError,
};

pub const VOXEL_OBJECT_SCHEMA_VERSION: u32 = 1;

/// A reusable local-space voxel model with an optional set of frame-swap clips.
///
/// Unlike a [`crate::VoxelAsset`], this value is not an authoritative world
/// volume. Its pivot and frames are object-local presentation data. A caller
/// must make an explicit, separate choice if one stable frame is also used as
/// collision input.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelObjectAsset {
    pub schema_version: u32,
    pub asset_id: String,
    pub grid: VoxelObjectGrid,
    /// Union of the default frame and every clip frame.
    pub bounds: VoxelAssetBounds,
    pub default_frame: VoxelFrame,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clips: Vec<VoxelObjectClip>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_clip: Option<String>,
    pub material_palette: Vec<VoxelAssetMaterialBinding>,
    pub material_map: Vec<VoxelAssetMaterialMapping>,
    pub provenance: VoxelObjectProvenance,
    pub content_hash: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelObjectGrid {
    pub coordinate_system: VoxelCoordinateSystem,
    pub cell_size: f64,
    pub chunk_size: u32,
    /// Pivot in local voxel-cell coordinates. Fractional pivots are allowed so
    /// conversion can retain a stable foot, root, or geometric anchor.
    pub pivot: [f64; 3],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelObjectClip {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub frames_per_second: f64,
    pub frames: Vec<VoxelObjectAnimationFrame>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelObjectAnimationFrame {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<f64>,
    /// Schema 1 stores a complete resolved frame. Delta/reference encodings are
    /// intentionally deferred until measured artifacts justify their cost.
    pub frame: VoxelFrame,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelObjectProvenance {
    pub kind: VoxelObjectProvenanceKind,
    pub source_path: String,
    pub source_sha256: String,
    pub source_byte_count: u64,
    pub converter: String,
    pub settings_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license_path: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VoxelObjectProvenanceKind {
    Authored,
    ConvertedStaticMesh,
    ConvertedAnimatedMesh,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoxelObjectFrameSelectionError {
    UnknownClip {
        clip: String,
    },
    FrameOutOfRange {
        clip: String,
        index: usize,
        frame_count: usize,
    },
    InvalidFrame(VoxelFrameError),
}

impl std::fmt::Display for VoxelObjectFrameSelectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownClip { clip } => write!(formatter, "unknown voxel-object clip `{clip}`"),
            Self::FrameOutOfRange {
                clip,
                index,
                frame_count,
            } => write!(
                formatter,
                "voxel-object clip `{clip}` frame {index} is outside 0..{frame_count}"
            ),
            Self::InvalidFrame(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for VoxelObjectFrameSelectionError {}

impl VoxelObjectAsset {
    pub fn clip(&self, clip_id: &str) -> Option<&VoxelObjectClip> {
        self.clips.iter().find(|clip| clip.id == clip_id)
    }

    pub fn clip_frame(
        &self,
        clip_id: &str,
        frame_index: usize,
    ) -> Result<&VoxelObjectAnimationFrame, VoxelObjectFrameSelectionError> {
        let clip =
            self.clip(clip_id)
                .ok_or_else(|| VoxelObjectFrameSelectionError::UnknownClip {
                    clip: clip_id.to_string(),
                })?;
        clip.frames.get(frame_index).ok_or_else(|| {
            VoxelObjectFrameSelectionError::FrameOutOfRange {
                clip: clip_id.to_string(),
                index: frame_index,
                frame_count: clip.frames.len(),
            }
        })
    }

    pub fn resolve_default_frame(
        &self,
    ) -> Result<Vec<VoxelFrameCell>, VoxelObjectFrameSelectionError> {
        resolve_voxel_frame(&self.default_frame, self.material_slots())
            .map_err(VoxelObjectFrameSelectionError::InvalidFrame)
    }

    pub fn resolve_clip_frame(
        &self,
        clip_id: &str,
        frame_index: usize,
    ) -> Result<Vec<VoxelFrameCell>, VoxelObjectFrameSelectionError> {
        let frame = self.clip_frame(clip_id, frame_index)?;
        resolve_voxel_frame(&frame.frame, self.material_slots())
            .map_err(VoxelObjectFrameSelectionError::InvalidFrame)
    }

    fn material_slots(&self) -> impl Iterator<Item = u16> + '_ {
        self.material_palette
            .iter()
            .map(|binding| binding.material_slot)
    }
}
