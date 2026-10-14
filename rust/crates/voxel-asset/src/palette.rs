use crate::{VoxelAsset, VoxelAssetError, VoxelAssetMaterialBinding};

/// Compare-and-swap request for changing material meaning without changing
/// voxel occupancy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoxelPaletteUpdateRequest {
    pub expected_content_hash: String,
    pub replacement: Vec<VoxelAssetMaterialBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoxelPaletteUpdateReceipt {
    pub content_hash_before: String,
    pub content_hash_after: String,
    pub voxel_data_hash: String,
    pub material_count_before: usize,
    pub material_count_after: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoxelPaletteUpdateError {
    StaleContentHash { expected: String, actual: String },
    Invalid(VoxelAssetError),
}

impl std::fmt::Display for VoxelPaletteUpdateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for VoxelPaletteUpdateError {}

/// Validate the complete candidate before replacing the caller's asset. Any
/// rejection leaves `asset` untouched. Only the palette changes, so the voxel
/// data identity is kept and only the content identity is recomputed.
pub fn replace_voxel_palette(
    asset: &mut VoxelAsset,
    request: VoxelPaletteUpdateRequest,
) -> Result<VoxelPaletteUpdateReceipt, VoxelPaletteUpdateError> {
    if request.expected_content_hash != asset.content_hash {
        return Err(VoxelPaletteUpdateError::StaleContentHash {
            expected: request.expected_content_hash,
            actual: asset.content_hash.clone(),
        });
    }
    let material_count_before = asset.material_palette.len();
    let mut candidate = asset.clone();
    candidate.material_palette = request.replacement;
    let candidate = crate::codec::with_recomputed_content_hash(candidate)
        .map_err(VoxelPaletteUpdateError::Invalid)?;
    let receipt = VoxelPaletteUpdateReceipt {
        content_hash_before: std::mem::take(&mut asset.content_hash),
        content_hash_after: candidate.content_hash.clone(),
        voxel_data_hash: candidate.voxel_data_hash.clone(),
        material_count_before,
        material_count_after: candidate.material_palette.len(),
    };
    *asset = candidate;
    Ok(receipt)
}
