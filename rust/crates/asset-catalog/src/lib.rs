//! Host-neutral authored asset catalog and dependency authority.
//!
//! The crate owns reusable asset definitions, validation, locks, material
//! projections, contextual fallbacks, canonical authored JSON, and local
//! change-impact analysis. It has no renderer, filesystem, runtime-session, or
//! replay dependency.

#![forbid(unsafe_code)]

mod admission;
mod catalog;
mod change;
mod codec;
mod dependency;
mod fallback;
mod lock;
mod material;
mod validation;
mod voxel;

pub use admission::{AdmittedAssetCatalog, AssetCatalogStore, CatalogAdmissionError};
pub use catalog::{AssetCatalog, CatalogEntry};
pub use change::{
    classify_material_change, material_change_impact, revalidate_asset, ChangeImpactReport,
    ChangeKind, ReloadSuggestion,
};
pub use codec::{
    decode_catalog, decode_lock, encode_catalog, encode_lock, AssetCatalogCodecError,
    StoredAssetCatalog, StoredAssetLock, StoredAssetLockEntry, StoredAssetReference,
    StoredAssetVersionRequirement, StoredAtlasPadding, StoredAtlasRegionDefinition,
    StoredCatalogEntry, StoredMaterialAuthority, StoredMaterialDefinition, StoredMaterialStyle,
    StoredTextureDefinition, StoredVoxelAlphaMode, StoredVoxelAtlasDefinition,
    StoredVoxelSurfaceBinding, StoredVoxelSurfaceMapping,
};
pub use dependency::DependencyGraph;
pub use fallback::{fallback_for, AssetContext, FallbackOutcome, FallbackVisual};
pub use lock::{
    generate_lock, validate_lock, AssetLock, AssetLockEntry, LockFinding, LockIssue,
    LockValidationReport,
};
pub use material::{
    AtlasInset, AtlasPadding, AtlasRegionDefinition, CollisionMaterial, MaterialAuthority,
    MaterialDefinition, MaterialStyle, RenderMaterial, ResolvedVoxelSurface,
    ResolvedVoxelSurfaceMapping, Rgba, StructuralClass, TextureDefinition, TextureFilter,
    TextureWrap, UvStrategy, VoxelAlphaMode, VoxelAtlasDefinition, VoxelSurfaceBinding,
    VoxelSurfaceMapping, VoxelSurfaceResolutionError, MAX_AGGREGATE_ATLAS_REGIONS,
    MAX_ATLAS_PADDING, MAX_ATLAS_REGIONS, MAX_TEXTURE_DIMENSION, MAX_TEXTURE_TEXELS,
    MAX_TILE_ORIGIN_CELLS, MAX_TILE_SCALE_CELLS, MIN_TILE_SCALE_CELLS,
};
pub use validation::{
    validate_catalog, CatalogDiagnostic, CatalogValidationError, CatalogValidationReport,
};
pub use voxel::{
    VoxelMaterialError, VoxelMaterialTable, VoxelMaterialTableReport, VoxelRenderResolution,
};
