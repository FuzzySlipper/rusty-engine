//! Bounded offline conversion from one static GLB mesh to a durable voxel asset.
//!
//! This crate is an authoring/build tool. It has no dependency on a downstream
//! game runtime and is never invoked while admitting or running a project.

#![forbid(unsafe_code)]

mod convert;
mod diagnostic;
mod import;
mod material;
mod planning;
mod query;
mod source;
mod store;

pub use convert::{convert_glb, ConversionReceipt, CONVERTER_ID, MAX_SURFACE_SAMPLE_WORK};
pub use diagnostic::{ConversionDiagnostic, ConversionError};
pub use import::{
    import_static_glb, ImportedMaterial, ImportedPrimitiveGroup, ImportedStaticMesh,
    ImportedTriangle,
};
pub use material::{
    ConversionMaterialPolicy, TextureChannelLayout, TextureColorSpace, TextureMaterialBinding,
    TextureMaterialMode, TextureSampleAsset, TextureSamplingPolicy, TextureSourceRef,
    TextureUvAttributeRef, TextureWrapPolicy, MAX_CONVERSION_TEXTURE_TEXELS,
};
pub use planning::{
    apply_conversion, apply_conversion_and_install, conversion_plan_hash, identity_transform,
    plan_conversion, plan_settings_sha256, preview_conversion, AppliedVoxelConversion,
    ConversionApplyRequest, ConversionPlanRequest, ConversionPlanSettings,
    ConversionPreviewRequest, ConversionPreviewVoxel, PreparedVoxelConversion, VoxelConversionPlan,
    VoxelConversionPreview, CONVERSION_PLANNER_ID, MAX_CONVERSION_PREVIEW_SAMPLES,
};
pub use query::{
    query_model_info, query_model_window, VoxelModelInfoReadout, VoxelModelInfoRequest,
    VoxelModelMaterialCount, VoxelModelWindowReadout, VoxelModelWindowRequest,
    VoxelModelWindowSample, MAX_MODEL_WINDOW_CELLS, MAX_MODEL_WINDOW_SAMPLES,
};
pub use source::{
    decode_mesh_source_import_request, import_mesh_source, source_sha256, ImportedMeshSource,
    MeshSourceBounds, MeshSourceFormat, MeshSourceGroup, MeshSourceImportReceipt,
    MeshSourceImportRequest, MeshSourceMaterialSlot, MeshSourceMetadata, MeshSourceRef,
    MAX_MESH_IMPORT_REQUEST_BYTES, MAX_MESH_PRIMITIVE_BYTES, MAX_MESH_SOURCE_ASSET_ID_BYTES,
    MAX_MESH_SOURCE_PATH_BYTES,
};
pub use store::{convert_and_install, decode_conversion_request, MAX_CONVERSION_REQUEST_BYTES};
pub use voxel_asset::{
    MAX_CONVERSION_CELLS, MAX_CONVERSION_RESOLUTION_AXIS, MAX_CONVERSION_SOURCE_BYTES,
    MAX_CONVERSION_SOURCE_INDICES, MAX_CONVERSION_SOURCE_VERTICES,
};
