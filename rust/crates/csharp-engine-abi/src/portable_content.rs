//! Typed portable asset semantics resolved from an existing Content reference.
use crate::{
    NativeContentReferenceHandle, NativeEngineDiagnosticLeaseHandle, NativeOperationErrorReceipt,
    NativeUtf8Slice, NativeVec2,
};
use std::ffi::c_void;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativePortableAssetHandle {
    pub value: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativePortableAssetReadoutLeaseHandle {
    pub value: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePortableAssetLoadRequest {
    pub descriptor: NativeContentReferenceHandle,
    pub asset_id: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePortableAssetMemberRequest {
    pub asset: NativePortableAssetHandle,
    pub member_id: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativePortableAssetKind {
    Texture = 1,
    Model = 2,
    Material = 3,
    Sprite = 4,
    Attachment = 5,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePortableAssetMember {
    pub id: NativeUtf8Slice,
    pub kind: NativePortableAssetKind,
    /// Descriptor-relative location, empty for semantic-only members.
    pub path: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePortableSpriteFrame {
    pub id: NativeUtf8Slice,
    pub texture_id: NativeUtf8Slice,
    pub origin: NativeVec2,
    pub extent: NativeVec2,
    pub canvas: NativeVec2,
    pub trim: NativeVec2,
    pub pivot: NativeVec2,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePortableSpriteAnchor {
    pub frame_id: NativeUtf8Slice,
    pub name: NativeUtf8Slice,
    pub position: NativeVec2,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePortableSpriteAnimationFrame {
    pub animation: NativeUtf8Slice,
    pub frame_id: NativeUtf8Slice,
    pub duration_seconds: f64,
    pub looping: bool,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePortableSpriteDirection {
    pub id: NativeUtf8Slice,
    pub yaw_degrees: f32,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePortableSpriteAction {
    pub action: NativeUtf8Slice,
    pub direction: NativeUtf8Slice,
    pub animation: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativePortableAssetRelationshipKind {
    MaterialSlot = 1,
    TextureRole = 2,
    AnimationClip = 3,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePortableAssetRelationship {
    pub member_id: NativeUtf8Slice,
    pub kind: NativePortableAssetRelationshipKind,
    pub name: NativeUtf8Slice,
    pub target: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePortableMeshAttachment {
    pub target_id: NativeUtf8Slice,
    pub child_id: NativeUtf8Slice,
    pub joint: NativeUtf8Slice,
    pub convention: NativeUtf8Slice,
    pub transform: crate::NativeTransform,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePortableAssetReadoutLease {
    pub handle: NativePortableAssetReadoutLeaseHandle,
    pub asset_id: NativeUtf8Slice,
    pub direction_convention: NativeUtf8Slice,
    pub members: *const NativePortableAssetMember,
    pub members_len: usize,
    pub frames: *const NativePortableSpriteFrame,
    pub frames_len: usize,
    pub anchors: *const NativePortableSpriteAnchor,
    pub anchors_len: usize,
    /// Ordered rows; repeated frame IDs are meaningful.
    pub animation_frames: *const NativePortableSpriteAnimationFrame,
    pub animation_frames_len: usize,
    pub directions: *const NativePortableSpriteDirection,
    pub directions_len: usize,
    pub actions: *const NativePortableSpriteAction,
    pub actions_len: usize,
    pub relationships: *const NativePortableAssetRelationship,
    pub relationships_len: usize,
    pub attachments: *const NativePortableMeshAttachment,
    pub attachments_len: usize,
}
pub type NativeLoadPortableAsset = unsafe extern "C" fn(
    *mut c_void,
    *const NativePortableAssetLoadRequest,
    *mut NativePortableAssetHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyPortableAsset =
    unsafe extern "C" fn(*mut c_void, NativePortableAssetHandle) -> i32;
pub type NativeReadPortableAsset = unsafe extern "C" fn(
    *mut c_void,
    NativePortableAssetHandle,
    *mut NativePortableAssetReadoutLease,
) -> i32;
pub type NativeDestroyPortableAssetReadoutLease =
    unsafe extern "C" fn(*mut c_void, NativePortableAssetReadoutLeaseHandle) -> i32;
pub type NativeOpenPortableAssetMember = unsafe extern "C" fn(
    *mut c_void,
    *const NativePortableAssetMemberRequest,
    *mut NativeContentReferenceHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyContentOperationDiagnosticLease =
    unsafe extern "C" fn(*mut c_void, NativeEngineDiagnosticLeaseHandle) -> i32;
