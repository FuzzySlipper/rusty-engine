//! Engine-owned rendering projection for canonical Spatial voxel scenes.
//!
//! Product code binds live Appearance materials to material slots and asks the
//! Engine to project a Spatial session.  It never supplies mesh payloads,
//! renderer handles, or a parallel scene representation.

use crate::{
    NativeAppearanceHandle, NativeMaterialHandle, NativeOperationErrorReceipt, NativeRenderLayer,
    NativeSpatialFace, NativeSpatialSessionHandle, NativeVec3,
};
use std::ffi::c_void;

/// Opaque retained projection identity.  The generated C# facade owns its
/// matching destroy call through `IDisposable`.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelScenePresentationHandle {
    pub value: u64,
}

/// One Engine material selected for one canonical voxel material slot.
/// A retained palette may include currently unused slots; every meshed slot
/// requires a base binding. Atlas identity and texture belong to each material.
/// Bindings are borrowed for one callback and copied into the retained
/// projection state before it returns.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelSceneMaterialBinding {
    pub material_slot: u32,
    pub material: NativeMaterialHandle,
}

/// Sparse face-specific override for one canonical source material slot.
/// Omitted faces continue to resolve through the complete base bindings.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelSceneFaceMaterialBinding {
    pub variant: u32,
    pub material_slot: u32,
    pub face: NativeSpatialFace,
    pub material: NativeMaterialHandle,
}

/// Creates one retained projection of the canonical voxel scene owned by a
/// Spatial session.  There is no mesh, renderer resource, or transform input:
/// the scene's own world-origin-aware mesh projection is authoritative.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeProjectVoxelSceneRequest {
    pub session: NativeSpatialSessionHandle,
    pub materials: *const NativeVoxelSceneMaterialBinding,
    pub materials_len: usize,
}

/// Adds sparse canonical-face overrides to the unchanged base scene request.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeProjectVoxelSceneDirectionalRequest {
    pub session: NativeSpatialSessionHandle,
    pub materials: *const NativeVoxelSceneMaterialBinding,
    pub materials_len: usize,
    pub face_materials: *const NativeVoxelSceneFaceMaterialBinding,
    pub face_materials_len: usize,
}

/// Rebinds the complete material palette for an existing retained scene
/// projection.  The Engine copies resolved descriptors before returning.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeUpdateVoxelScenePresentationRequest {
    pub presentation: NativeVoxelScenePresentationHandle,
    pub materials: *const NativeVoxelSceneMaterialBinding,
    pub materials_len: usize,
}

/// Complete base bindings plus sparse face overrides for an existing retained
/// presentation. All inputs are copied before the callback returns.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeUpdateVoxelScenePresentationDirectionalRequest {
    pub presentation: NativeVoxelScenePresentationHandle,
    pub materials: *const NativeVoxelSceneMaterialBinding,
    pub materials_len: usize,
    pub face_materials: *const NativeVoxelSceneFaceMaterialBinding,
    pub face_materials_len: usize,
}

/// Draws a retained presentation's distant chunks from their coarse meshes.
/// A chunk farther than `coarse_distance` world units from the camera of the
/// lowest-ordered primary view (in the backdrop, backdrop units from where
/// that camera stands in it) is drawn from a lattice twice as coarse; zero
/// draws every chunk at full resolution. Collision and picking keep the full
/// mesh. Sessions whose materials are all cubes, or whose chunk edge is odd,
/// draw at full resolution.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct NativeVoxelSceneLevelOfDetailRequest {
    pub presentation: NativeVoxelScenePresentationHandle,
    pub coarse_distance: f64,
}

/// Draws a presentation in the scene's layer or the backdrop's
/// (`RenderLayer.Backdrop`, linked by `CameraView.SetBackdrop`): there its
/// scene stands in backdrop units, and its level of detail and scatters
/// measure from where the primary camera stands in the backdrop (full
/// resolution and no scatters without a link). Its collision is the
/// session's as ever; the presentation only changes where it draws.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeVoxelSceneLayerRequest {
    pub presentation: NativeVoxelScenePresentationHandle,
    pub layer: NativeRenderLayer,
}

/// Grass, stones or flowers the Engine grows on a presentation's ground
/// around the camera of the lowest-ordered primary view (#9546). Copies stand
/// on upward surface within `slope_limit_degrees` of the material slots
/// named (all slots when none are), `density` per square metre of ground, on
/// full-resolution chunks within `radius` of the camera, shrinking away over
/// the last `fade` metres of it. Each copy draws `appearance` (a static mesh
/// appearance) with `material` at a scale between `scale_min` and
/// `scale_max`, tinted between `tint_low` and `tint_high`, turned at random
/// about its up axis and leaning `align` of the way from upright to the
/// ground's normal. The nearest chunks' copies come first within
/// `maximum_instances`. Copies are never nodes, entities, colliders or
/// pickable. `scatter` names this scatter within the presentation: setting it
/// again replaces it. The appearance and material must stay live while the
/// scatter grows them.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeVoxelSceneScatterRequest {
    pub presentation: NativeVoxelScenePresentationHandle,
    pub scatter: u32,
    pub appearance: NativeAppearanceHandle,
    pub material: NativeMaterialHandle,
    pub slots: *const u32,
    pub slots_len: usize,
    pub density: f32,
    pub radius: f32,
    pub fade: f32,
    pub scale_min: f32,
    pub scale_max: f32,
    pub tint_low: NativeVec3,
    pub tint_high: NativeVec3,
    pub slope_limit_degrees: f32,
    pub align: f32,
    pub casts_shadows: bool,
    pub maximum_instances: u32,
    pub seed: u32,
}

/// Stops growing one scatter: its copies are removed.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelSceneScatterRemoval {
    pub presentation: NativeVoxelScenePresentationHandle,
    pub scatter: u32,
}

/// A box, from `min` to `max` in the scene's space, that no scatter of a
/// presentation grows in: a copy whose base falls inside it is not placed.
/// It stays where it is when the world origin rebases, as the scene's
/// collision meshes do.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeVoxelSceneScatterExclusion {
    pub min: NativeVec3,
    pub max: NativeVec3,
}

/// Replaces every exclusion box of a presentation's scatters, such as the
/// ground under the product's built floors and inside its walls. The chunks
/// a box added or removed touches are placed again.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeVoxelSceneScatterExclusionRequest {
    pub presentation: NativeVoxelScenePresentationHandle,
    pub exclusions: *const NativeVoxelSceneScatterExclusion,
    pub exclusions_len: usize,
}

/// Copied provenance for one effective source-slot/face renderer selection.
/// `material_value` identifies the selected retained Material at admission
/// time; it is diagnostic provenance, not a live disposable handle.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelSceneMaterialMappingRow {
    pub variant: u32,
    pub source_slot: u32,
    pub face: NativeSpatialFace,
    pub material_value: u64,
    pub renderer_slot: u32,
    pub overridden: bool,
}

/// Borrowed effective mapping readout, valid until the next call on this
/// service. The generated binding copies the rows before returning to C#.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeVoxelSceneMaterialMappingResult {
    pub mappings: *const NativeVoxelSceneMaterialMappingRow,
    pub mappings_len: usize,
    pub source_revision: u64,
    pub mesh_revision: u64,
}

/// Small observation of the last renderer projection for one retained scene.
/// This intentionally exposes no renderer object or mesh payload.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelScenePresentationReadout {
    pub present: bool,
    pub source_revision: u64,
    pub mesh_revision: u64,
    pub chunk_count: u64,
    pub material_count: u32,
    /// Chunks drawn from their coarse meshes at the last projection.
    pub coarse_chunk_count: u64,
    /// Time the last projection spent meshing coarse chunks, summed over
    /// chunks (they mesh in parallel, so wall time can be shorter).
    pub coarse_mesh_microseconds: u64,
    /// Scatter patches (one per chunk and scatter) and the copies they hold.
    pub scatter_patch_count: u64,
    pub scatter_instance_count: u64,
    /// Chunks within a scatter's reach left bare by its instance budget.
    pub scatter_over_budget_count: u64,
}

/// Result of clearing all retained voxel scene projections in this product
/// call.  Clearing emits the corresponding Engine renderer destroys.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelScenePresentationClearReceipt {
    pub cleared_count: u32,
    pub retained_count: u32,
}

pub type NativeProjectVoxelScene = unsafe extern "C" fn(
    *mut c_void,
    *const NativeProjectVoxelSceneRequest,
    *mut NativeVoxelScenePresentationHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeProjectVoxelSceneDirectional = unsafe extern "C" fn(
    *mut c_void,
    *const NativeProjectVoxelSceneDirectionalRequest,
    *mut NativeVoxelScenePresentationHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeRefreshVoxelScenePresentation = unsafe extern "C" fn(
    *mut c_void,
    NativeVoxelScenePresentationHandle,
    *mut NativeVoxelScenePresentationReadout,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeUpdateVoxelScenePresentation = unsafe extern "C" fn(
    *mut c_void,
    *const NativeUpdateVoxelScenePresentationRequest,
    *mut NativeVoxelScenePresentationReadout,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeUpdateVoxelScenePresentationDirectional = unsafe extern "C" fn(
    *mut c_void,
    *const NativeUpdateVoxelScenePresentationDirectionalRequest,
    *mut NativeVoxelScenePresentationReadout,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeSetVoxelSceneLayer = unsafe extern "C" fn(
    *mut c_void,
    *const NativeVoxelSceneLayerRequest,
    *mut NativeVoxelScenePresentationReadout,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeSetVoxelSceneLevelOfDetail = unsafe extern "C" fn(
    *mut c_void,
    *const NativeVoxelSceneLevelOfDetailRequest,
    *mut NativeVoxelScenePresentationReadout,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeSetVoxelSceneScatter = unsafe extern "C" fn(
    *mut c_void,
    *const NativeVoxelSceneScatterRequest,
    *mut NativeVoxelScenePresentationReadout,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeRemoveVoxelSceneScatter = unsafe extern "C" fn(
    *mut c_void,
    *const NativeVoxelSceneScatterRemoval,
    *mut NativeVoxelScenePresentationReadout,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeSetVoxelSceneScatterExclusions = unsafe extern "C" fn(
    *mut c_void,
    *const NativeVoxelSceneScatterExclusionRequest,
    *mut NativeVoxelScenePresentationReadout,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadVoxelSceneMaterialMapping = unsafe extern "C" fn(
    *mut c_void,
    NativeVoxelScenePresentationHandle,
    *mut NativeVoxelSceneMaterialMappingResult,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyVoxelScenePresentation =
    unsafe extern "C" fn(*mut c_void, NativeVoxelScenePresentationHandle) -> i32;
pub type NativeClearVoxelScenePresentations = unsafe extern "C" fn(
    *mut c_void,
    *mut NativeVoxelScenePresentationClearReceipt,
    *mut NativeOperationErrorReceipt,
) -> i32;

/// Named generated Engine service family for projecting canonical Spatial
/// voxel scenes through the Engine renderer.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeVoxelScenePresentationApi {
    pub context: *mut c_void,
    pub project_scene: NativeProjectVoxelScene,
    pub refresh_scene: NativeRefreshVoxelScenePresentation,
    pub update_scene: NativeUpdateVoxelScenePresentation,
    pub destroy_scene: NativeDestroyVoxelScenePresentation,
    pub clear: NativeClearVoxelScenePresentations,
    pub project_scene_directional: NativeProjectVoxelSceneDirectional,
    pub update_scene_directional: NativeUpdateVoxelScenePresentationDirectional,
    pub read_material_mapping: NativeReadVoxelSceneMaterialMapping,
    pub set_level_of_detail: NativeSetVoxelSceneLevelOfDetail,
    pub set_scatter: NativeSetVoxelSceneScatter,
    pub remove_scatter: NativeRemoveVoxelSceneScatter,
    pub set_scatter_exclusions: NativeSetVoxelSceneScatterExclusions,
    pub set_layer: NativeSetVoxelSceneLayer,
}
