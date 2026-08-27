//! Typed retained voxel artifacts for trusted NativeAOT products.
//!
//! This family admits one complete, bounded voxel asset or object from a
//! borrowed byte body and retains the validated owner value behind an opaque
//! disposable handle. It deliberately has no relationship to the canonical
//! Spatial session scene, meshes, renderer resources, or collision projection.

use crate::{NativeByteSlice, NativeUtf8Slice};
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
pub type NativeAdmitVoxelObject = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAdmitVoxelObjectRequest,
    *mut NativeVoxelObjectHandle,
) -> i32;
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

/// Direct typed voxel-artifact services. This is intentionally separate from
/// `NativeVoxelApi`, whose owner is the canonical mutable Spatial scene.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeVoxelContentApi {
    pub context: *mut c_void,
    pub admit_asset: NativeAdmitVoxelAsset,
    pub destroy_asset: NativeDestroyVoxelAsset,
    pub read_asset: NativeReadVoxelAsset,
    pub admit_object: NativeAdmitVoxelObject,
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
}
