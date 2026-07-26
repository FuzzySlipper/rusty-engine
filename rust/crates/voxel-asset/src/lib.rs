//! Successor-owned durable voxel-volume assets and offline conversion inputs.
//!
//! This crate deliberately contains no mesh parser, filesystem access, project
//! loader, runtime mutation, or replay protocol. Runtime code may validate and
//! expand the stored artifact; the separate authoring tool owns conversion.

#![forbid(unsafe_code)]

mod asset;
mod codec;
mod conversion;
mod frame;
mod object;
mod object_codec;
mod palette;

pub use asset::{
    VoxelAsset, VoxelAssetBounds, VoxelAssetGrid, VoxelAssetMaterialBinding,
    VoxelAssetMaterialMapping, VoxelAssetProvenance, VoxelAssetProvenanceKind,
    VoxelCoordinateSystem, VoxelRepresentation, VoxelRepresentationKind, VoxelSparseRun,
    VOXEL_ASSET_SCHEMA_VERSION,
};
pub use codec::{
    canonicalize_voxel_asset, decode_voxel_asset, encode_voxel_asset, validate_voxel_asset,
    with_computed_content_hash, VoxelAssetDiagnostic, VoxelAssetError, MAX_ARTIFACT_BYTES,
    MAX_MATERIAL_MAPPINGS, MAX_REPRESENTED_VOXELS, MAX_STRING_BYTES,
};
pub use conversion::{
    conversion_settings_sha256, validate_conversion_request, VoxelConversionFitPolicy,
    VoxelConversionInputDiagnostic, VoxelConversionInputError, VoxelConversionMode,
    VoxelConversionOriginPolicy, VoxelConversionRequest, VoxelConversionSettings,
    MAX_CONVERSION_CELLS, MAX_CONVERSION_RESOLUTION_AXIS, MAX_CONVERSION_SOURCE_BYTES,
    MAX_CONVERSION_SOURCE_INDICES, MAX_CONVERSION_SOURCE_VERTICES,
};
pub use frame::{
    canonicalize_voxel_frame, represented_voxel_count, resolve_voxel_asset, resolve_voxel_frame,
    validate_voxel_frame, with_computed_voxel_frame_hash, VoxelFrame, VoxelFrameCell,
    VoxelFrameDiagnostic, VoxelFrameError, MAX_VOXEL_FRAME_COORDINATE_ABS,
};
pub use object::{
    VoxelObjectAnimationFrame, VoxelObjectAsset, VoxelObjectClip, VoxelObjectFrameSelectionError,
    VoxelObjectGrid, VoxelObjectProvenance, VoxelObjectProvenanceKind,
    VoxelObjectSourceClipProvenance, VOXEL_OBJECT_SCHEMA_VERSION,
};
pub use object_codec::{
    canonicalize_voxel_object, decode_voxel_object, encode_voxel_object, validate_voxel_object,
    with_computed_voxel_object_hashes, VoxelObjectDiagnostic, VoxelObjectError,
    MAX_VOXEL_OBJECT_ARTIFACT_BYTES, MAX_VOXEL_OBJECT_CLIPS, MAX_VOXEL_OBJECT_FRAMES_PER_CLIP,
    MAX_VOXEL_OBJECT_FRAMES_PER_SECOND, MAX_VOXEL_OBJECT_FRAME_DURATION_SECONDS,
    MAX_VOXEL_OBJECT_TOTAL_FRAMES, MAX_VOXEL_OBJECT_TOTAL_VOXELS,
};
pub use palette::{
    replace_voxel_palette, VoxelPaletteUpdateError, VoxelPaletteUpdateReceipt,
    VoxelPaletteUpdateRequest,
};
