use crate::{with_computed_content_hash, VoxelAsset, VoxelAssetError, VoxelAssetMaterialBinding};

/// Compare-and-swap request for changing material meaning without changing
/// voxel occupancy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoxelPaletteUpdateRequest {
    pub expected_content_hash: String,
    pub expected_voxel_data_hash: String,
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
    StaleVoxelDataHash { expected: String, actual: String },
    Invalid(VoxelAssetError),
    VoxelDataChanged { before: String, after: String },
}

impl std::fmt::Display for VoxelPaletteUpdateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for VoxelPaletteUpdateError {}

/// Validate the complete candidate before replacing the caller's asset. Any
/// rejection leaves `asset` untouched.
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
    if request.expected_voxel_data_hash != asset.voxel_data_hash {
        return Err(VoxelPaletteUpdateError::StaleVoxelDataHash {
            expected: request.expected_voxel_data_hash,
            actual: asset.voxel_data_hash.clone(),
        });
    }

    let before_content = asset.content_hash.clone();
    let before_voxels = asset.voxel_data_hash.clone();
    let material_count_before = asset.material_palette.len();
    let mut candidate = asset.clone();
    candidate.material_palette = request.replacement;
    candidate.voxel_data_hash.clear();
    candidate.content_hash.clear();
    candidate = with_computed_content_hash(candidate).map_err(VoxelPaletteUpdateError::Invalid)?;
    if candidate.voxel_data_hash != before_voxels {
        return Err(VoxelPaletteUpdateError::VoxelDataChanged {
            before: before_voxels,
            after: candidate.voxel_data_hash,
        });
    }
    let receipt = VoxelPaletteUpdateReceipt {
        content_hash_before: before_content,
        content_hash_after: candidate.content_hash.clone(),
        voxel_data_hash: candidate.voxel_data_hash.clone(),
        material_count_before,
        material_count_after: candidate.material_palette.len(),
    };
    *asset = candidate;
    Ok(receipt)
}
