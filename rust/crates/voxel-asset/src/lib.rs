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
    decode_voxel_asset, encode_voxel_asset, validate_voxel_asset, with_computed_content_hash,
    VoxelAssetDiagnostic, VoxelAssetError, MAX_REPRESENTED_VOXELS,
};
pub use conversion::{
    conversion_settings_sha256, validate_conversion_request, VoxelConversionFitPolicy,
    VoxelConversionInputDiagnostic, VoxelConversionInputError, VoxelConversionMode,
    VoxelConversionOriginPolicy, VoxelConversionRequest, VoxelConversionSettings,
};
pub use frame::{
    represented_voxel_count, resolve_voxel_asset, resolve_voxel_frame, VoxelFrame, VoxelFrameCell,
};
pub use object::{
    VoxelObjectAnimationFrame, VoxelObjectAsset, VoxelObjectClip, VoxelObjectCollisionPrimitive,
    VoxelObjectFrameAnchor, VoxelObjectFrameCollision, VoxelObjectFrameSelectionError,
    VoxelObjectGrid, VoxelObjectHitRegion, VoxelObjectProvenance, VoxelObjectProvenanceKind,
    VoxelObjectSourceClipProvenance, VOXEL_OBJECT_SCHEMA_VERSION,
};
pub use object_codec::{
    decode_voxel_object, encode_voxel_object, validate_voxel_object,
    with_computed_voxel_object_hashes, VoxelObjectDiagnostic, VoxelObjectError,
    MAX_VOXEL_OBJECT_TOTAL_VOXELS,
};
pub use palette::{
    replace_voxel_palette, VoxelPaletteUpdateError, VoxelPaletteUpdateReceipt,
    VoxelPaletteUpdateRequest,
};
