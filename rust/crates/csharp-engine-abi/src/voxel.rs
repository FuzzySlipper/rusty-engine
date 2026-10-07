use crate::{NativeLightDescriptor, NativeVec3};
use crate::{NativeOperationErrorReceipt, NativeSpatialSessionHandle};
use std::ffi::c_void;

/// One signed voxel address in the session's canonical world grid.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelAddress {
    pub x: i64,
    pub y: i64,
    pub z: i64,
}

/// One signed resident chunk identity in the session's canonical world grid.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelChunkIdentity {
    pub x: i64,
    pub y: i64,
    pub z: i64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelSceneReadRequest {
    pub session: NativeSpatialSessionHandle,
}

/// Fixed facts about the one canonical scene and all of its derived spatial
/// projections. No mesh payload or renderer object crosses this boundary.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct NativeVoxelSceneReadout {
    pub present: bool,
    pub voxel_size: f64,
    pub chunk_size: u32,
    pub source_revision: u64,
    pub authority_hash: u64,
    pub collision_revision: u64,
    pub mesh_revision: u64,
    pub projection_version: u64,
    pub resident_chunk_count: u64,
    pub collider_chunk_count: u64,
    pub solid_voxel_count: u64,
    pub dirty_chunk_count: u32,
    pub rebuilt_mesh_chunks: u32,
    pub reused_mesh_chunks: u32,
    pub removed_mesh_chunks: u32,
    /// Time meshing the chunks the last change rebuilt, summed over chunks.
    pub mesh_microseconds: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelReadRequest {
    pub session: NativeSpatialSessionHandle,
    pub address: NativeVoxelAddress,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelReadout {
    pub state: u32,
    pub present: bool,
    pub address: NativeVoxelAddress,
    pub material_slot: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelChunkReadRequest {
    pub session: NativeSpatialSessionHandle,
    pub chunk: NativeVoxelChunkIdentity,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelChunkReadout {
    pub present: bool,
    pub chunk: NativeVoxelChunkIdentity,
    pub content_hash: u64,
    pub solid_voxel_count: u64,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeVoxelEditKind {
    #[default]
    Set = 0,
    Clear = 1,
}

/// The outcome of one voxel edit transaction. `NoChanges` is an expected
/// product-control result; other failures stay on the ABI diagnostic lane.
#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeVoxelEditStatus {
    #[default]
    Accepted = 0,
    NoChanges = 1,
}

/// Flat edit records are borrowed for one call and copied into the Engine
/// owner before any retained state is changed.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelEdit {
    pub state: u32,
    pub kind: NativeVoxelEditKind,
    pub address: NativeVoxelAddress,
    pub material_slot: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeVoxelEditTransaction {
    pub session: NativeSpatialSessionHandle,
    pub edits: *const NativeVoxelEdit,
    pub edits_len: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelEditReceipt {
    pub revision_before: u64,
    pub accepted_revision: u64,
    pub solid_voxel_count: u64,
    pub authority_hash: u64,
    pub collision_revision: u64,
    pub mesh_revision: u64,
    pub changed_voxels: u32,
    pub changed_min: NativeVoxelAddress,
    pub changed_max_inclusive: NativeVoxelAddress,
    pub dirty_chunk_count: u32,
    pub rebuilt_mesh_chunks: u32,
    pub reused_mesh_chunks: u32,
    pub removed_mesh_chunks: u32,
    pub status: NativeVoxelEditStatus,
    pub mesh_microseconds: u64,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeVoxelResidencyOperationKind {
    #[default]
    Admit = 0,
    Replace = 1,
    Evict = 2,
}

/// One flat chunk operation. For Admit/Replace, material_offset/count select
/// the dense u32 material-slot range carried by the enclosing transaction,
/// and density_offset/count the chunk's densities (count zero: none).
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelResidencyOperation {
    pub kind: NativeVoxelResidencyOperationKind,
    pub chunk: NativeVoxelChunkIdentity,
    pub material_offset: u32,
    pub material_count: u32,
    pub density_offset: u32,
    pub density_count: u32,
}

/// Densities are signed, negative inside, in voxel units, one per slot and
/// negative exactly where the slot is solid.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeVoxelResidencyTransaction {
    pub states: *const u32,
    pub states_len: usize,
    pub session: NativeSpatialSessionHandle,
    pub operations: *const NativeVoxelResidencyOperation,
    pub operations_len: usize,
    pub material_slots: *const u32,
    pub material_slots_len: usize,
    pub densities: *const f32,
    pub densities_len: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelResidencyReceipt {
    pub revision_before: u64,
    pub accepted_revision: u64,
    pub admitted_count: u32,
    pub replaced_count: u32,
    pub evicted_count: u32,
    pub retained_count: u32,
    pub resident_chunk_count: u64,
    pub resident_solid_voxel_count: u64,
    pub authority_hash: u64,
    pub collision_revision: u64,
    pub mesh_revision: u64,
    pub dirty_chunk_count: u32,
    pub rebuilt_mesh_chunks: u32,
    pub reused_mesh_chunks: u32,
    pub removed_mesh_chunks: u32,
    pub mesh_microseconds: u64,
}

/// Collision policy for one occupied material slot; visuals retain the cell.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelMaterialCollision {
    pub material_slot: u32,
    pub collidable: bool,
}

/// Configure a fresh session's product-owned material collision declarations.
/// Unlisted slots collide. Configure before residency or edits.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeVoxelMaterialCollisionRequest {
    pub session: NativeSpatialSessionHandle,
    pub materials: *const NativeVoxelMaterialCollision,
    pub materials_len: usize,
}

pub type NativeConfigureVoxelMaterialCollision = unsafe extern "C" fn(
    *mut c_void,
    *const NativeVoxelMaterialCollisionRequest,
    *mut NativeOperationErrorReceipt,
) -> i32;

/// Whether one occupied material slot hides its neighbours' cube faces.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelMaterialOcclusion {
    pub material_slot: u32,
    pub occludes: bool,
}

/// Configure a session's product-owned material occlusion declarations.
/// Unlisted slots occlude. Two voxels of one non-occluding slot still hide
/// their shared face. Configuring again replaces the declarations and
/// remeshes every chunk.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeVoxelMaterialOcclusionRequest {
    pub session: NativeSpatialSessionHandle,
    pub materials: *const NativeVoxelMaterialOcclusion,
    pub materials_len: usize,
}

pub type NativeConfigureVoxelMaterialOcclusion = unsafe extern "C" fn(
    *mut c_void,
    *const NativeVoxelMaterialOcclusionRequest,
    *mut NativeOperationErrorReceipt,
) -> i32;

/// How one voxel material slot is surfaced.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct NativeVoxelMaterialSurface {
    pub material_slot: u32,
    pub mode: crate::NativeVoxelSurfaceMode,
    /// Used by the reconstructed modes.
    pub character: crate::NativeSurfaceCharacter,
}

/// Replace a session's surface modes: `mode` for every unlisted material and
/// each listed material's own. Every chunk is remeshed and its collision
/// rebuilt; collision follows reconstructed materials' drawn surfaces.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeVoxelMaterialSurfaceRequest {
    pub session: NativeSpatialSessionHandle,
    pub mode: crate::NativeVoxelSurfaceMode,
    pub materials: *const NativeVoxelMaterialSurface,
    pub materials_len: usize,
}

pub type NativeConfigureVoxelMaterialSurfaces = unsafe extern "C" fn(
    *mut c_void,
    *const NativeVoxelMaterialSurfaceRequest,
    *mut NativeVoxelSceneReadout,
    *mut NativeOperationErrorReceipt,
) -> i32;

/// Give a session's reconstructed voxel surfaces terrain layer weights:
/// `slots` are distinct material slots drawn by the terrain layer material
/// bound to them, and each vertex weighs the solid voxels of each layer's
/// slots within `transition_cells` voxels (1 to 4, less than the chunk size).
/// Without `layers`, 1 to 4 slots are layers 0 to 3 in order. With them, 1 to
/// 16 slots each take the layer (0 to 3) at the same index, so several slots
/// can share one layer. No slots removes the weights. Every chunk is
/// remeshed; geometry, material slots and collision are unchanged.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeVoxelTerrainLayerRequest {
    pub session: NativeSpatialSessionHandle,
    pub slots: *const u32,
    pub slots_len: usize,
    pub transition_cells: u32,
    pub layers: *const u32,
    pub layers_len: usize,
}

pub type NativeConfigureVoxelTerrainLayers = unsafe extern "C" fn(
    *mut c_void,
    *const NativeVoxelTerrainLayerRequest,
    *mut NativeVoxelSceneReadout,
    *mut NativeOperationErrorReceipt,
) -> i32;

/// Darken every vertex of a session's surfaces by the solid voxels around
/// it, at mesh time: a reconstructed vertex by the fan of directions over
/// its normal, a cube face corner by the classic voxel rule. `strength` 0
/// (the default) turns it off, 1 applies the full occlusion; between scales
/// it. Every chunk is remeshed; geometry, material slots and collision are
/// unchanged.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeVoxelVertexOcclusionRequest {
    pub session: NativeSpatialSessionHandle,
    pub strength: f32,
}

pub type NativeConfigureVoxelVertexOcclusion = unsafe extern "C" fn(
    *mut c_void,
    *const NativeVoxelVertexOcclusionRequest,
    *mut NativeVoxelSceneReadout,
    *mut NativeOperationErrorReceipt,
) -> i32;

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeVoxelDensityEditKind {
    /// Replace the densities of a box of voxels.
    #[default]
    Region = 0,
    /// Blend a shape into the densities.
    Brush = 1,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeVoxelDensityShape {
    #[default]
    Sphere = 0,
    Box = 1,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeVoxelDensityOperation {
    /// Union: newly solid voxels take the edit's material.
    #[default]
    Add = 0,
    /// Subtraction: voxels whose density turns non-negative empty.
    Subtract = 1,
    /// Move densities inside the shape toward their neighbours' mean by
    /// `strength` (0 to 1].
    Smooth = 2,
    /// Give solid voxels inside the shape the edit's material.
    Paint = 3,
}

/// One density edit. Densities are signed, negative inside, in voxel units:
/// a solid voxel without one reads -0.5, putting the surface on its cube
/// face. A Region replaces the densities of `size` voxels from `min`
/// (x-fastest) with `density_count` values from the transaction's densities;
/// where a density is negative the voxel is solid with the matching
/// transaction material, or keeps its own when material_count is zero. A
/// Brush blends a sphere (center, radius) or box (box_min, box_max) given in
/// the scene's local frame.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeVoxelDensityEdit {
    pub kind: NativeVoxelDensityEditKind,
    pub min: NativeVoxelAddress,
    pub size_x: u32,
    pub size_y: u32,
    pub size_z: u32,
    pub density_offset: u32,
    pub density_count: u32,
    pub material_offset: u32,
    pub material_count: u32,
    pub shape: NativeVoxelDensityShape,
    pub operation: NativeVoxelDensityOperation,
    pub center: NativeVec3,
    pub radius: f32,
    pub box_min: NativeVec3,
    pub box_max: NativeVec3,
    pub strength: f32,
    pub material_slot: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeVoxelDensityTransaction {
    pub session: NativeSpatialSessionHandle,
    pub edits: *const NativeVoxelDensityEdit,
    pub edits_len: usize,
    pub densities: *const f32,
    pub densities_len: usize,
    pub materials: *const u32,
    pub materials_len: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelDensityReceipt {
    pub status: NativeVoxelEditStatus,
    pub revision_before: u64,
    pub accepted_revision: u64,
    /// Voxels whose density or material changed.
    pub changed_voxels: u32,
    /// Voxels that became solid or empty.
    pub solidity_changes: u32,
    pub changed_min: NativeVoxelAddress,
    pub changed_max_inclusive: NativeVoxelAddress,
    pub solid_voxel_count: u64,
    pub authority_hash: u64,
    pub collision_revision: u64,
    pub mesh_revision: u64,
    pub dirty_chunk_count: u32,
    pub rebuilt_mesh_chunks: u32,
    pub reused_mesh_chunks: u32,
    pub removed_mesh_chunks: u32,
    pub mesh_microseconds: u64,
}

pub type NativeApplyVoxelDensityEdits = unsafe extern "C" fn(
    *mut c_void,
    *const NativeVoxelDensityTransaction,
    *mut NativeVoxelDensityReceipt,
    *mut NativeOperationErrorReceipt,
) -> i32;

/// A box of `size` voxels from `min`, read x-fastest.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelDensityReadRequest {
    pub session: NativeSpatialSessionHandle,
    pub min: NativeVoxelAddress,
    pub size_x: u32,
    pub size_y: u32,
    pub size_z: u32,
}

/// One voxel's signed density and material (zero when empty). Voxels outside
/// the resident chunks are not resident and read as default-empty.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct NativeVoxelDensitySample {
    pub density: f32,
    pub material_slot: u32,
    pub resident: bool,
}

/// Borrowed until the next Voxel call.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeVoxelDensityResult {
    pub samples: *const NativeVoxelDensitySample,
    pub samples_len: usize,
}

pub type NativeReadVoxelDensities = unsafe extern "C" fn(
    *mut c_void,
    NativeVoxelDensityReadRequest,
    *mut NativeVoxelDensityResult,
    *mut NativeOperationErrorReceipt,
) -> i32;

pub type NativeReadVoxelScene = unsafe extern "C" fn(
    *mut c_void,
    NativeVoxelSceneReadRequest,
    *mut NativeVoxelSceneReadout,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadVoxel = unsafe extern "C" fn(
    *mut c_void,
    NativeVoxelReadRequest,
    *mut NativeVoxelReadout,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadVoxelChunk = unsafe extern "C" fn(
    *mut c_void,
    NativeVoxelChunkReadRequest,
    *mut NativeVoxelChunkReadout,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeApplyVoxelEdits = unsafe extern "C" fn(
    *mut c_void,
    *const NativeVoxelEditTransaction,
    *mut NativeVoxelEditReceipt,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeApplyVoxelResidency = unsafe extern "C" fn(
    *mut c_void,
    *const NativeVoxelResidencyTransaction,
    *mut NativeVoxelResidencyReceipt,
    *mut NativeOperationErrorReceipt,
) -> i32;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeVoxelApi {
    pub context: *mut c_void,
    pub configure_material_collision: NativeConfigureVoxelMaterialCollision,
    pub configure_material_occlusion: NativeConfigureVoxelMaterialOcclusion,
    pub read_scene: NativeReadVoxelScene,
    pub read: NativeReadVoxel,
    pub sample_direct_lighting: NativeSampleVoxelDirectLighting,
    pub read_chunk: NativeReadVoxelChunk,
    pub apply_edits: NativeApplyVoxelEdits,
    pub apply_residency: NativeApplyVoxelResidency,
    pub configure_material_surfaces: NativeConfigureVoxelMaterialSurfaces,
    pub apply_density_edits: NativeApplyVoxelDensityEdits,
    pub read_densities: NativeReadVoxelDensities,
    pub configure_terrain_layers: NativeConfigureVoxelTerrainLayers,
    pub configure_vertex_occlusion: NativeConfigureVoxelVertexOcclusion,
}

/// Samples direct incident light at address + offset (in voxel units). Descriptors
/// use local world coordinates, identical to unparented Graphics lights. Zero
/// normal measures incident light; nonzero normals select surface irradiance.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeVoxelLightSampleRequest {
    pub session: NativeSpatialSessionHandle,
    pub address: NativeVoxelAddress,
    pub offset: NativeVec3,
    pub normal: NativeVec3,
    pub directional_distance: f32,
    pub lights: *const NativeLightDescriptor,
    pub lights_len: usize,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeVoxelLightSample {
    pub irradiance: NativeVec3,
    pub luminance: f32,
    pub contributing_lights: u32,
    pub occluded_lights: u32,
    pub source_revision: u64,
    pub collision_revision: u64,
    pub static_collision_revision: u64,
    pub rebase_revision: u64,
}
pub type NativeSampleVoxelDirectLighting = unsafe extern "C" fn(
    *mut c_void,
    *const NativeVoxelLightSampleRequest,
    *mut NativeVoxelLightSample,
    *mut NativeOperationErrorReceipt,
) -> i32;
