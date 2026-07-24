//! Successor-owned durable voxel-volume assets and offline conversion inputs.
//!
//! This crate deliberately contains no mesh parser, filesystem access, project
//! loader, runtime mutation, or replay protocol. Runtime code may validate and
//! expand the stored artifact; the separate authoring tool owns conversion.

#![forbid(unsafe_code)]

mod asset;
mod codec;
mod conversion;

pub use asset::{
    VoxelAsset, VoxelAssetBounds, VoxelAssetGrid, VoxelAssetMaterialMapping, VoxelAssetProvenance,
    VoxelAssetProvenanceKind, VoxelCoordinateSystem, VoxelRepresentation, VoxelRepresentationKind,
    VoxelSparseRun, VOXEL_ASSET_SCHEMA_VERSION,
};
pub use codec::{
    decode_voxel_asset, encode_voxel_asset, validate_voxel_asset, with_computed_content_hash,
    VoxelAssetDiagnostic, VoxelAssetError, MAX_ARTIFACT_BYTES, MAX_MATERIAL_MAPPINGS,
    MAX_REPRESENTED_VOXELS, MAX_STRING_BYTES,
};
pub use conversion::{
    conversion_settings_sha256, validate_conversion_request, VoxelConversionFitPolicy,
    VoxelConversionInputDiagnostic, VoxelConversionInputError, VoxelConversionMode,
    VoxelConversionOriginPolicy, VoxelConversionRequest, VoxelConversionSettings,
    MAX_CONVERSION_CELLS, MAX_CONVERSION_RESOLUTION_AXIS, MAX_CONVERSION_SOURCE_BYTES,
    MAX_CONVERSION_SOURCE_INDICES, MAX_CONVERSION_SOURCE_VERTICES,
};
