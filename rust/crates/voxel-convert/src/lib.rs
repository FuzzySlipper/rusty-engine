//! Bounded offline conversion from static GLB scenes to durable voxel assets.
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
mod voxelize;

pub use convert::{convert_glb, ConversionReceipt, CONVERTER_ID, MAX_SURFACE_SAMPLE_WORK};
pub use diagnostic::{ConversionDiagnostic, ConversionError};
pub use import::{
    flatten_static_scene, import_static_glb, import_static_glb_scene,
    texture_coordinate_source_hash, ImportedMaterial, ImportedModelMesh, ImportedModelNode,
    ImportedModelPrimitive, ImportedModelScene, ImportedPrimitiveGroup, ImportedStaticMesh,
    ImportedStaticTextureCoordinates, ImportedTextureCoordinates, ImportedTriangle,
    MAX_IMPORTED_NAME_BYTES, MAX_IMPORTED_SCENE_DEPTH, MAX_IMPORTED_SCENE_EDGES,
    MAX_IMPORTED_SCENE_MESHES, MAX_IMPORTED_SCENE_MESH_INSTANCES, MAX_IMPORTED_SCENE_NODES,
    MAX_IMPORTED_SCENE_PRIMITIVES, MAX_IMPORTED_TEXCOORD_SETS,
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
    MeshSourceImportRequest, MeshSourceMaterialSlot, MeshSourceMetadata, MeshSourceNode,
    MeshSourceRef, MeshSourceTextureCoordinates, MAX_MESH_IMPORT_REQUEST_BYTES,
    MAX_MESH_PRIMITIVE_BYTES, MAX_MESH_SOURCE_ASSET_ID_BYTES, MAX_MESH_SOURCE_PATH_BYTES,
};
pub use store::{convert_and_install, decode_conversion_request, MAX_CONVERSION_REQUEST_BYTES};
pub use voxel_asset::{
    MAX_CONVERSION_CELLS, MAX_CONVERSION_RESOLUTION_AXIS, MAX_CONVERSION_SOURCE_BYTES,
    MAX_CONVERSION_SOURCE_INDICES, MAX_CONVERSION_SOURCE_VERTICES,
};
pub use voxelize::MAX_GEOMETRIC_VOXELIZATION_WORK;
