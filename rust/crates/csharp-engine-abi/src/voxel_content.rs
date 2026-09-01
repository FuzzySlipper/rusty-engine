//! Typed retained voxel artifacts for trusted NativeAOT products.
//!
//! This family admits one complete, bounded voxel asset or object from a
//! borrowed byte body and retains the validated owner value behind an opaque
//! disposable handle. An admitted asset can be explicitly published into a
//! fresh canonical Spatial session through the named asset-to-scene operation;
//! object presentation remains a separate renderer-neutral capability.

use crate::{NativeByteSlice, NativeSpatialSessionHandle, NativeUtf8Slice};
use std::ffi::c_void;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelAssetHandle {
    pub value: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelObjectHandle {
    pub value: u64,
}

/// Opaque Engine-owned explicit-time player identity. A player retains its
/// admitted voxel-object owner, so it remains safe after the product releases
/// the object's direct handle.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelObjectPlayerHandle {
    pub value: u64,
}

/// Opaque Engine-owned retained presentation identity for one admitted voxel
/// object. The product never receives a renderer handle, mesh, or resource
/// through this owner.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelObjectPresentationHandle {
    pub value: u64,
}

/// Opaque retained annotation layer. It owns an admitted annotation layer and
/// retains its admitted voxel-asset target independently of the asset's direct
/// product handle.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelAnnotationHandle {
    pub value: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelAnnotationRegionLeaseHandle {
    pub value: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelAnnotationEditLeaseHandle {
    pub value: u64,
}

/// Bounds use explicit scalar coordinates so they stay a fixed generated C#
/// value rather than an ABI-specific fixed-array projection.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelAnnotationBounds {
    pub min_x: i64,
    pub min_y: i64,
    pub min_z: i64,
    pub max_x: i64,
    pub max_y: i64,
    pub max_z: i64,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeVoxelAnnotationKind {
    #[default]
    Selection = 1,
    Room = 2,
    Portal = 3,
    SpawnArea = 4,
    Cover = 5,
    Hazard = 6,
    NavigationHint = 7,
    Custom = 8,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeVoxelAnnotationQueryMode {
    #[default]
    Cell = 1,
    Bounds = 2,
    Region = 3,
    LayerSummary = 4,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeVoxelObjectLoopMode {
    #[default]
    Once = 1,
    Repeat = 2,
    PingPong = 3,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeVoxelObjectPlaybackStatus {
    #[default]
    Stopped = 1,
    Playing = 2,
    Paused = 3,
}

/// The only source-axis conversion currently admitted from ordinary
/// MagicaVoxel v150 model chunks. MagicaVoxel's +Z-up coordinates become the
/// Engine's +Y-up right-handed object-local coordinates.
#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeMagicaVoxelOrientation {
    #[default]
    XRightYUpNegativeZForward = 1,
}

/// Product-selected local pivot policy for a MagicaVoxel source. Explicit
/// pivots are already expressed in the converted Engine-local coordinates.
#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeMagicaVoxelPivotPolicy {
    #[default]
    Explicit = 1,
    BoundsCenter = 2,
    BaseCenter = 3,
}

/// Stable rejection statuses for the bounded MagicaVoxel source-admission
/// operation. `1` remains the shared ABI success status.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeMagicaVoxelAdmissionStatus {
    InvalidRequest = 2,
    SourceLimit = 3,
    InvalidHeader = 4,
    UnsupportedVersion = 5,
    MalformedSource = 6,
    UnsupportedFeature = 7,
    DuplicateCell = 8,
    VoxelLimit = 9,
    CanonicalObject = 10,
    HandleExhausted = 11,
    NotMagicaVoxelObject = 12,
    PaletteLeaseExhausted = 13,
}

/// A fixed SHA-256 fact. The words are big-endian groups from the canonical
/// hexadecimal digest; no borrowed identity text escapes the direct call.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelContentHash {
    pub word0: u64,
    pub word1: u64,
    pub word2: u64,
    pub word3: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAdmitVoxelAssetRequest {
    /// A complete bounded voxel-asset artifact borrowed for this call only.
    pub bytes: NativeByteSlice,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAdmitVoxelObjectRequest {
    /// A complete bounded voxel-object artifact borrowed for this call only.
    pub bytes: NativeByteSlice,
}

/// One bounded trusted-product request for a single ordinary MagicaVoxel v150
/// model. Bytes and UTF-8 identities are borrowed only for this call; the
/// Engine copies the admitted source facts before retaining the object.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAdmitMagicaVoxelObjectRequest {
    pub bytes: NativeByteSlice,
    pub asset_id: NativeUtf8Slice,
    pub source_path: NativeUtf8Slice,
    pub cell_size: f64,
    pub pivot_policy: NativeMagicaVoxelPivotPolicy,
    pub pivot_x: f64,
    pub pivot_y: f64,
    pub pivot_z: f64,
    pub orientation: NativeMagicaVoxelOrientation,
    pub max_source_bytes: u64,
    pub max_dimension: u32,
    pub max_voxel_count: u64,
    pub max_chunk_count: u32,
    pub max_material_slots: u32,
}

/// One copied source palette fact. `source_color_index` is MagicaVoxel's
/// 1-based color index; only occupied indices appear in an admission read.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeMagicaVoxelPaletteRow {
    pub material_slot: u32,
    pub source_color_index: u32,
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeMagicaVoxelPaletteLeaseHandle {
    pub value: u64,
}

/// A copied, disposable palette readout associated with one admitted object.
/// The pointer is valid only until its matching destroy callback.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMagicaVoxelPaletteLease {
    pub handle: NativeMagicaVoxelPaletteLeaseHandle,
    pub palette: *const NativeMagicaVoxelPaletteRow,
    pub palette_len: usize,
    pub source_hash: NativeVoxelContentHash,
    pub source_byte_count: u64,
}

/// A complete bounded annotation artifact. Its target is an already admitted
/// asset, not a scene or renderer resource. The bridge validates the exact
/// asset identity, voxel-data hash, and bounds before retaining the layer.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAdmitVoxelAnnotationRequest {
    pub asset: NativeVoxelAssetHandle,
    pub bytes: NativeByteSlice,
}

/// One typed bounded annotation query. Only fields selected by `mode` are
/// read; `has_expected_layer_hash` controls whether stale-hash rejection is
/// requested.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeVoxelAnnotationQueryRequest {
    pub annotation: NativeVoxelAnnotationHandle,
    pub mode: NativeVoxelAnnotationQueryMode,
    pub coordinate_x: i64,
    pub coordinate_y: i64,
    pub coordinate_z: i64,
    pub bounds: NativeVoxelAnnotationBounds,
    pub region_id: NativeUtf8Slice,
    pub has_expected_layer_hash: bool,
    pub expected_layer_hash: NativeVoxelContentHash,
    pub max_results: u32,
}

/// One bounded metadata item. UTF-8 slices point into the owning region lease
/// and are copied by generated C# before its matching destroy callback.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeVoxelAnnotationRegionReadout {
    pub region_id: NativeUtf8Slice,
    pub label: NativeUtf8Slice,
    pub kind: NativeVoxelAnnotationKind,
    pub has_parent_region_id: bool,
    pub parent_region_id: NativeUtf8Slice,
    pub bounds: NativeVoxelAnnotationBounds,
    pub assigned_cell_count: u64,
}

/// Engine-owned bounded collection plus fixed query facts. The outer facts are
/// deliberately copied into the generated receipt, not inferred from the
/// returned collection length.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeVoxelAnnotationRegionLease {
    pub handle: NativeVoxelAnnotationRegionLeaseHandle,
    pub regions: *const NativeVoxelAnnotationRegionReadout,
    pub regions_len: usize,
    pub total_layer_regions: u32,
    pub truncated: bool,
    pub revision: u64,
    pub layer_hash: NativeVoxelContentHash,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSetVoxelAnnotationLabelRequest {
    pub annotation: NativeVoxelAnnotationHandle,
    pub expected_layer_hash: NativeVoxelContentHash,
    pub region_id: NativeUtf8Slice,
    pub label: NativeUtf8Slice,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSetVoxelAnnotationKindRequest {
    pub annotation: NativeVoxelAnnotationHandle,
    pub expected_layer_hash: NativeVoxelContentHash,
    pub region_id: NativeUtf8Slice,
    pub kind: NativeVoxelAnnotationKind,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSetVoxelAnnotationParentRequest {
    pub annotation: NativeVoxelAnnotationHandle,
    pub expected_layer_hash: NativeVoxelContentHash,
    pub region_id: NativeUtf8Slice,
    pub has_parent_region_id: bool,
    pub parent_region_id: NativeUtf8Slice,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSetVoxelAnnotationBoundsRequest {
    pub annotation: NativeVoxelAnnotationHandle,
    pub expected_layer_hash: NativeVoxelContentHash,
    pub region_id: NativeUtf8Slice,
    pub bounds: NativeVoxelAnnotationBounds,
}

/// One replacement tag borrowed only for a single named annotation edit.
/// The bridge copies the UTF-8 text before the callback returns.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeVoxelAnnotationTag {
    pub value: NativeUtf8Slice,
}

/// Atomically replace one region's tags at an expected owner hash. `tags` is
/// a synchronous, bounded borrowed array with no retained product pointers.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSetVoxelAnnotationTagsRequest {
    pub annotation: NativeVoxelAnnotationHandle,
    pub expected_layer_hash: NativeVoxelContentHash,
    pub region_id: NativeUtf8Slice,
    pub tags: *const NativeVoxelAnnotationTag,
    pub tags_len: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeVoxelAnnotationAffectedId {
    pub region_id: NativeUtf8Slice,
}

/// A successful atomic owner edit. The associated IDs and every fixed receipt
/// fact survive generated C# copying until this exact lease is released.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeVoxelAnnotationEditLease {
    pub handle: NativeVoxelAnnotationEditLeaseHandle,
    pub affected_ids: *const NativeVoxelAnnotationAffectedId,
    pub affected_ids_len: usize,
    pub layer_hash_before: NativeVoxelContentHash,
    pub layer_hash_after: NativeVoxelContentHash,
    pub membership_hash_before: NativeVoxelContentHash,
    pub membership_hash_after: NativeVoxelContentHash,
    pub revision: u64,
    pub command_count: u32,
    pub region_count: u32,
    pub assigned_cell_count: u64,
}

/// Stable facts for one retained voxel asset. Its opaque identity stays at the
/// owning `NativeVoxelAssetHandle` call boundary; no identity text is borrowed.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelAssetReadout {
    pub schema_version: u32,
    pub represented_voxel_count: u64,
    pub material_palette_count: u32,
    pub material_mapping_count: u32,
    pub voxel_data_hash: NativeVoxelContentHash,
    pub content_hash: NativeVoxelContentHash,
}

/// A fresh Spatial session may be initialized directly from one retained
/// asset. The Engine resolves the retained owner and constructs the canonical
/// collision/navigation scene; C# supplies no voxel mirror or mesh payload.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePublishVoxelAssetToSpatialRequest {
    pub asset: NativeVoxelAssetHandle,
    pub session: NativeSpatialSessionHandle,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelAssetSpatialPublishLeaseHandle {
    pub value: u64,
}

/// One copied semantic palette row from the admitted asset. The UTF-8 slices
/// borrow the matching publish lease only; generated C# copies them before it
/// releases that lease.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeVoxelAssetSpatialPaletteRow {
    pub material_slot: u32,
    pub material_asset_id: NativeUtf8Slice,
    pub display_name: NativeUtf8Slice,
}

/// Facts for one atomic asset-to-Spatial publication plus its bounded semantic
/// palette lease. No retained owner handle or renderer resource crosses the
/// boundary in this result.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeVoxelAssetSpatialPublishLease {
    pub handle: NativeVoxelAssetSpatialPublishLeaseHandle,
    pub palette: *const NativeVoxelAssetSpatialPaletteRow,
    pub palette_len: usize,
    pub revision_before: u64,
    pub revision_after: u64,
    pub voxel_size: f64,
    pub chunk_size: u32,
    pub solid_voxel_count: u64,
    pub resident_chunk_count: u64,
    pub authority_hash: u64,
    pub projection_version: u64,
    pub collision_revision: u64,
    pub navigation_revision: u64,
    pub mesh_revision: u64,
    pub navigation_cell_count: u64,
    pub voxel_data_hash: NativeVoxelContentHash,
    pub content_hash: NativeVoxelContentHash,
}

/// Stable facts for one retained voxel object. Its opaque identity stays at the
/// owning handle call boundary. Frame selection is object-local and cannot
/// publish to Spatial or rendering.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelObjectReadout {
    pub schema_version: u32,
    pub frame_count: u32,
    pub clip_count: u32,
    pub material_palette_count: u32,
    pub material_mapping_count: u32,
    pub default_runtime_frame: u32,
    pub selected_runtime_frame: u32,
    pub selection_revision: u64,
    pub content_hash: NativeVoxelContentHash,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSelectVoxelObjectClipFrameRequest {
    pub object_handle: NativeVoxelObjectHandle,
    /// A borrowed named clip identity used only to select a known retained
    /// object frame. The bridge copies neither pointer nor identity text.
    pub clip: NativeUtf8Slice,
    pub frame_index: u32,
}

/// Fixed selected-frame facts. `clip_index` is `u32::MAX` for the default
/// frame; no mesh or renderer handle is available through this value.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelObjectFrameReadout {
    pub runtime_frame: u32,
    pub clip_index: u32,
    pub clip_frame_index: u32,
    pub is_default: bool,
    pub cell_count: u64,
    pub anchor_count: u32,
    pub has_collision: bool,
    pub voxel_data_hash: NativeVoxelContentHash,
}

/// One exact material choice for an admitted object palette slot. The slice is
/// borrowed only for the named project/update callback; Engine resolves and
/// copies the material descriptor before that callback returns.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeVoxelObjectMaterialBinding {
    pub material_slot: u32,
    pub material: crate::NativeMaterialHandle,
}

/// Creates one retained renderer-neutral projection from an admitted object
/// and an explicitly selected runtime frame. All palette slots must be bound
/// by `materials`; no mesh or backend payload is accepted from C#.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeProjectVoxelObjectRequest {
    pub object: NativeVoxelObjectHandle,
    pub runtime_frame: u32,
    pub transform: crate::NativeTransform,
    pub visible: bool,
    pub materials: *const NativeVoxelObjectMaterialBinding,
    pub materials_len: usize,
}

/// Reprojects an existing retained voxel object. The complete current palette
/// binding list is supplied again so the Engine can retain an independent
/// material snapshot without product pointers or renderer handles.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeUpdateVoxelObjectPresentationRequest {
    pub presentation: NativeVoxelObjectPresentationHandle,
    pub runtime_frame: u32,
    pub transform: crate::NativeTransform,
    pub visible: bool,
    pub materials: *const NativeVoxelObjectMaterialBinding,
    pub materials_len: usize,
}

/// One explicit product-directed start. `now_micros` is supplied by the
/// caller; the Engine neither stores nor advances a host clock.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePlayVoxelObjectPlayerRequest {
    pub player_handle: NativeVoxelObjectPlayerHandle,
    pub clip: NativeUtf8Slice,
    pub loop_mode: NativeVoxelObjectLoopMode,
    pub rate_numerator: u32,
    pub rate_denominator: u32,
    pub now_micros: u64,
}

/// One exact paused clip-frame selection. The product chooses clip and frame;
/// the Engine derives its admitted elapsed-time posture.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeScrubVoxelObjectPlayerRequest {
    pub player_handle: NativeVoxelObjectPlayerHandle,
    pub clip: NativeUtf8Slice,
    pub clip_frame: u32,
    pub loop_mode: NativeVoxelObjectLoopMode,
}

/// Explicit caller time used for pause, resume, read, and sample operations.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeVoxelObjectPlayerTimeRequest {
    pub player_handle: NativeVoxelObjectPlayerHandle,
    pub now_micros: u64,
}

/// Fixed posture facts with no borrowed clip identity or renderer state.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelObjectPlayerReadout {
    pub status: NativeVoxelObjectPlaybackStatus,
    pub loop_mode: NativeVoxelObjectLoopMode,
    pub rate_numerator: u32,
    pub rate_denominator: u32,
    pub elapsed_micros: u64,
}

/// A fixed sampled runtime-frame fact. `has_clip_frame` is false for the
/// stopped/default posture and `clip_frame` is then zero.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeVoxelObjectPlayerSampleReadout {
    pub status: NativeVoxelObjectPlaybackStatus,
    pub loop_mode: NativeVoxelObjectLoopMode,
    pub rate_numerator: u32,
    pub rate_denominator: u32,
    pub elapsed_micros: u64,
    pub runtime_frame: u32,
    pub has_clip_frame: bool,
    pub clip_frame: u32,
    pub ended: bool,
}

pub type NativeAdmitVoxelAsset = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAdmitVoxelAssetRequest,
    *mut NativeVoxelAssetHandle,
) -> i32;
pub type NativeDestroyVoxelAsset = unsafe extern "C" fn(*mut c_void, NativeVoxelAssetHandle) -> i32;
pub type NativeReadVoxelAsset =
    unsafe extern "C" fn(*mut c_void, NativeVoxelAssetHandle, *mut NativeVoxelAssetReadout) -> i32;
pub type NativePublishVoxelAssetToSpatial = unsafe extern "C" fn(
    *mut c_void,
    *const NativePublishVoxelAssetToSpatialRequest,
    *mut NativeVoxelAssetSpatialPublishLease,
) -> i32;
pub type NativeDestroyVoxelAssetSpatialPublishLease =
    unsafe extern "C" fn(*mut c_void, NativeVoxelAssetSpatialPublishLeaseHandle) -> i32;
pub type NativeAdmitVoxelObject = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAdmitVoxelObjectRequest,
    *mut NativeVoxelObjectHandle,
) -> i32;
pub type NativeAdmitMagicaVoxelObject = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAdmitMagicaVoxelObjectRequest,
    *mut NativeVoxelObjectHandle,
) -> i32;
pub type NativeReadMagicaVoxelPalette = unsafe extern "C" fn(
    *mut c_void,
    NativeVoxelObjectHandle,
    *mut NativeMagicaVoxelPaletteLease,
) -> i32;
pub type NativeDestroyMagicaVoxelPaletteLease =
    unsafe extern "C" fn(*mut c_void, NativeMagicaVoxelPaletteLeaseHandle) -> i32;
pub type NativeAdmitVoxelAnnotation = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAdmitVoxelAnnotationRequest,
    *mut NativeVoxelAnnotationHandle,
) -> i32;
pub type NativeDestroyVoxelAnnotation =
    unsafe extern "C" fn(*mut c_void, NativeVoxelAnnotationHandle) -> i32;
pub type NativeQueryVoxelAnnotation = unsafe extern "C" fn(
    *mut c_void,
    *const NativeVoxelAnnotationQueryRequest,
    *mut NativeVoxelAnnotationRegionLease,
) -> i32;
pub type NativeDestroyVoxelAnnotationRegionLease =
    unsafe extern "C" fn(*mut c_void, NativeVoxelAnnotationRegionLeaseHandle) -> i32;
pub type NativeSetVoxelAnnotationLabel = unsafe extern "C" fn(
    *mut c_void,
    *const NativeSetVoxelAnnotationLabelRequest,
    *mut NativeVoxelAnnotationEditLease,
) -> i32;
pub type NativeSetVoxelAnnotationKind = unsafe extern "C" fn(
    *mut c_void,
    *const NativeSetVoxelAnnotationKindRequest,
    *mut NativeVoxelAnnotationEditLease,
) -> i32;
pub type NativeSetVoxelAnnotationParent = unsafe extern "C" fn(
    *mut c_void,
    *const NativeSetVoxelAnnotationParentRequest,
    *mut NativeVoxelAnnotationEditLease,
) -> i32;
pub type NativeSetVoxelAnnotationBounds = unsafe extern "C" fn(
    *mut c_void,
    *const NativeSetVoxelAnnotationBoundsRequest,
    *mut NativeVoxelAnnotationEditLease,
) -> i32;
pub type NativeSetVoxelAnnotationTags = unsafe extern "C" fn(
    *mut c_void,
    *const NativeSetVoxelAnnotationTagsRequest,
    *mut NativeVoxelAnnotationEditLease,
) -> i32;
pub type NativeDestroyVoxelAnnotationEditLease =
    unsafe extern "C" fn(*mut c_void, NativeVoxelAnnotationEditLeaseHandle) -> i32;
pub type NativeDestroyVoxelObject =
    unsafe extern "C" fn(*mut c_void, NativeVoxelObjectHandle) -> i32;
pub type NativeReadVoxelObject = unsafe extern "C" fn(
    *mut c_void,
    NativeVoxelObjectHandle,
    *mut NativeVoxelObjectReadout,
) -> i32;
pub type NativeSelectDefaultVoxelObjectFrame = unsafe extern "C" fn(
    *mut c_void,
    NativeVoxelObjectHandle,
    *mut NativeVoxelObjectFrameReadout,
) -> i32;
pub type NativeSelectVoxelObjectClipFrame = unsafe extern "C" fn(
    *mut c_void,
    *const NativeSelectVoxelObjectClipFrameRequest,
    *mut NativeVoxelObjectFrameReadout,
) -> i32;
pub type NativeReadSelectedVoxelObjectFrame = unsafe extern "C" fn(
    *mut c_void,
    NativeVoxelObjectHandle,
    *mut NativeVoxelObjectFrameReadout,
) -> i32;
pub type NativeCreateVoxelObjectPlayer = unsafe extern "C" fn(
    *mut c_void,
    NativeVoxelObjectHandle,
    *mut NativeVoxelObjectPlayerHandle,
) -> i32;
pub type NativeDestroyVoxelObjectPlayer =
    unsafe extern "C" fn(*mut c_void, NativeVoxelObjectPlayerHandle) -> i32;
pub type NativePlayVoxelObjectPlayer =
    unsafe extern "C" fn(*mut c_void, *const NativePlayVoxelObjectPlayerRequest) -> i32;
pub type NativeScrubVoxelObjectPlayer =
    unsafe extern "C" fn(*mut c_void, *const NativeScrubVoxelObjectPlayerRequest) -> i32;
pub type NativePauseVoxelObjectPlayer =
    unsafe extern "C" fn(*mut c_void, NativeVoxelObjectPlayerTimeRequest) -> i32;
pub type NativeResumeVoxelObjectPlayer =
    unsafe extern "C" fn(*mut c_void, NativeVoxelObjectPlayerTimeRequest) -> i32;
pub type NativeStopVoxelObjectPlayer =
    unsafe extern "C" fn(*mut c_void, NativeVoxelObjectPlayerHandle) -> i32;
pub type NativeReadVoxelObjectPlayer = unsafe extern "C" fn(
    *mut c_void,
    NativeVoxelObjectPlayerTimeRequest,
    *mut NativeVoxelObjectPlayerReadout,
) -> i32;
pub type NativeSampleVoxelObjectPlayer = unsafe extern "C" fn(
    *mut c_void,
    NativeVoxelObjectPlayerTimeRequest,
    *mut NativeVoxelObjectPlayerSampleReadout,
) -> i32;
pub type NativeProjectVoxelObject = unsafe extern "C" fn(
    *mut c_void,
    *const NativeProjectVoxelObjectRequest,
    *mut NativeVoxelObjectPresentationHandle,
) -> i32;
pub type NativeUpdateVoxelObjectPresentation =
    unsafe extern "C" fn(*mut c_void, *const NativeUpdateVoxelObjectPresentationRequest) -> i32;
pub type NativeDestroyVoxelObjectPresentation =
    unsafe extern "C" fn(*mut c_void, NativeVoxelObjectPresentationHandle) -> i32;

/// Direct typed voxel-artifact services. This is intentionally separate from
/// `NativeVoxelApi`, whose owner is the canonical mutable Spatial scene.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeVoxelContentApi {
    pub context: *mut c_void,
    pub admit_asset: NativeAdmitVoxelAsset,
    pub destroy_asset: NativeDestroyVoxelAsset,
    pub read_asset: NativeReadVoxelAsset,
    pub publish_asset_to_spatial: NativePublishVoxelAssetToSpatial,
    pub destroy_asset_spatial_publish_lease: NativeDestroyVoxelAssetSpatialPublishLease,
    pub admit_object: NativeAdmitVoxelObject,
    pub admit_magica_voxel_object: NativeAdmitMagicaVoxelObject,
    pub read_magica_voxel_palette: NativeReadMagicaVoxelPalette,
    pub destroy_magica_voxel_palette_lease: NativeDestroyMagicaVoxelPaletteLease,
    pub destroy_object: NativeDestroyVoxelObject,
    pub read_object: NativeReadVoxelObject,
    pub select_default_object_frame: NativeSelectDefaultVoxelObjectFrame,
    pub select_object_clip_frame: NativeSelectVoxelObjectClipFrame,
    pub read_selected_object_frame: NativeReadSelectedVoxelObjectFrame,
    pub create_object_player: NativeCreateVoxelObjectPlayer,
    pub destroy_object_player: NativeDestroyVoxelObjectPlayer,
    pub play_object_player: NativePlayVoxelObjectPlayer,
    pub scrub_object_player: NativeScrubVoxelObjectPlayer,
    pub pause_object_player: NativePauseVoxelObjectPlayer,
    pub resume_object_player: NativeResumeVoxelObjectPlayer,
    pub stop_object_player: NativeStopVoxelObjectPlayer,
    pub read_object_player: NativeReadVoxelObjectPlayer,
    pub sample_object_player: NativeSampleVoxelObjectPlayer,
    pub project_object: NativeProjectVoxelObject,
    pub update_object_presentation: NativeUpdateVoxelObjectPresentation,
    pub destroy_object_presentation: NativeDestroyVoxelObjectPresentation,
    pub admit_annotation: NativeAdmitVoxelAnnotation,
    pub destroy_annotation: NativeDestroyVoxelAnnotation,
    pub query_annotation: NativeQueryVoxelAnnotation,
    pub destroy_annotation_region_lease: NativeDestroyVoxelAnnotationRegionLease,
    pub set_annotation_label: NativeSetVoxelAnnotationLabel,
    pub set_annotation_kind: NativeSetVoxelAnnotationKind,
    pub set_annotation_parent: NativeSetVoxelAnnotationParent,
    pub set_annotation_bounds: NativeSetVoxelAnnotationBounds,
    pub set_annotation_tags: NativeSetVoxelAnnotationTags,
    pub destroy_annotation_edit_lease: NativeDestroyVoxelAnnotationEditLease,
}
