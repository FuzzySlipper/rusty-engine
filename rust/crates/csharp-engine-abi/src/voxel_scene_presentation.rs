//! Engine-owned rendering projection for canonical Spatial voxel scenes.
//!
//! Product code binds live Appearance materials to material slots and asks the
//! Engine to project a Spatial session.  It never supplies mesh payloads,
//! renderer handles, or a parallel scene representation.

use crate::{NativeMaterialHandle, NativeSpatialFace, NativeSpatialSessionHandle};
use std::ffi::c_void;

/// Opaque retained projection identity.  The generated C# facade owns its
/// matching destroy call through `IDisposable`.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelScenePresentationHandle {
    pub value: u64,
}

/// One Engine material selected for one canonical voxel material slot.
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

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelSceneMaterialMappingLeaseHandle {
    pub value: u64,
}

/// Copied provenance for one effective source-slot/face renderer selection.
/// `material_value` identifies the selected retained Material at admission
/// time; it is diagnostic provenance, not a live disposable handle.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelSceneMaterialMappingRow {
    pub source_slot: u32,
    pub face: NativeSpatialFace,
    pub material_value: u64,
    pub renderer_slot: u32,
    pub overridden: bool,
}

/// Temporary backing for a copied effective mapping readout. The generated
/// binding copies rows and consumes this lease before returning to C#.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeVoxelSceneMaterialMappingLease {
    pub handle: NativeVoxelSceneMaterialMappingLeaseHandle,
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
) -> i32;
pub type NativeProjectVoxelSceneDirectional = unsafe extern "C" fn(
    *mut c_void,
    *const NativeProjectVoxelSceneDirectionalRequest,
    *mut NativeVoxelScenePresentationHandle,
) -> i32;
pub type NativeRefreshVoxelScenePresentation = unsafe extern "C" fn(
    *mut c_void,
    NativeVoxelScenePresentationHandle,
    *mut NativeVoxelScenePresentationReadout,
) -> i32;
pub type NativeUpdateVoxelScenePresentation = unsafe extern "C" fn(
    *mut c_void,
    *const NativeUpdateVoxelScenePresentationRequest,
    *mut NativeVoxelScenePresentationReadout,
) -> i32;
pub type NativeUpdateVoxelScenePresentationDirectional = unsafe extern "C" fn(
    *mut c_void,
    *const NativeUpdateVoxelScenePresentationDirectionalRequest,
    *mut NativeVoxelScenePresentationReadout,
) -> i32;
pub type NativeReadVoxelSceneMaterialMapping = unsafe extern "C" fn(
    *mut c_void,
    NativeVoxelScenePresentationHandle,
    *mut NativeVoxelSceneMaterialMappingLease,
) -> i32;
pub type NativeDestroyVoxelSceneMaterialMappingLease =
    unsafe extern "C" fn(*mut c_void, NativeVoxelSceneMaterialMappingLeaseHandle) -> i32;
pub type NativeDestroyVoxelScenePresentation =
    unsafe extern "C" fn(*mut c_void, NativeVoxelScenePresentationHandle) -> i32;
pub type NativeClearVoxelScenePresentations =
    unsafe extern "C" fn(*mut c_void, *mut NativeVoxelScenePresentationClearReceipt) -> i32;

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
    pub destroy_material_mapping_lease: NativeDestroyVoxelSceneMaterialMappingLease,
}
