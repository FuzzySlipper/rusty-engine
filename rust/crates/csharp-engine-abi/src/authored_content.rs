//! Typed, retained authored-content catalog access for trusted NativeAOT products.

use crate::{
    NativeContentReferenceHandle, NativeEngineDiagnosticLeaseHandle, NativeOperationErrorReceipt,
    NativeUtf8Slice,
};
use std::ffi::c_void;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeAuthoredCatalogHandle {
    pub value: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeAuthoredCatalogReadoutLeaseHandle {
    pub value: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAssetKind {
    Material = 1,
    StaticMesh = 2,
    AnimatedMesh = 3,
    Sprite = 4,
    SpriteSheet = 5,
    Texture = 6,
    AudioClip = 7,
    Font = 8,
    VoxelVolume = 9,
    VoxelObject = 10,
    Script = 11,
    Scene = 12,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAssetVersionRequirementKind {
    Any = 1,
    Exact = 2,
    AtLeast = 3,
}

/// Typed stable identity. `id` carries its kind-prefixed canonical spelling.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredAssetReference {
    pub id: NativeUtf8Slice,
    pub version_kind: NativeAssetVersionRequirementKind,
    pub version: u32,
    pub has_hash: bool,
    /// Algorithm-agnostic lowercase even-length hex, never a Content SHA-256 word value.
    pub hash: NativeUtf8Slice,
}

/// Payload-free catalog entry. Material and the material/texture/voxel payload
/// families deliberately have no representation in this first authored-content tranche.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredCatalogEntryInput {
    pub id: NativeUtf8Slice,
    pub version: u32,
    pub has_hash: bool,
    pub hash: NativeUtf8Slice,
    pub has_source_path: bool,
    pub source_path: NativeUtf8Slice,
    pub has_label: bool,
    pub label: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredCatalogDependencyInput {
    pub entry_id: NativeUtf8Slice,
    pub reference_id: NativeUtf8Slice,
    pub reference_version_kind: NativeAssetVersionRequirementKind,
    pub reference_version: u32,
    pub reference_has_hash: bool,
    pub reference_hash: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredCatalogAdmitRequest {
    pub entries: *const NativeAuthoredCatalogEntryInput,
    pub entries_len: usize,
    pub dependencies: *const NativeAuthoredCatalogDependencyInput,
    pub dependencies_len: usize,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredCatalogFromContentRequest {
    pub content: NativeContentReferenceHandle,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredCatalogResolveRequest {
    pub catalog: NativeAuthoredCatalogHandle,
    pub reference_id: NativeUtf8Slice,
    pub reference_version_kind: NativeAssetVersionRequirementKind,
    pub reference_version: u32,
    pub reference_has_hash: bool,
    pub reference_hash: NativeUtf8Slice,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredCatalogEntryReadout {
    pub id: NativeUtf8Slice,
    pub kind: NativeAssetKind,
    pub version: u32,
    pub has_hash: bool,
    pub hash: NativeUtf8Slice,
    pub has_source_path: bool,
    pub source_path: NativeUtf8Slice,
    pub has_label: bool,
    pub label: NativeUtf8Slice,
    pub dependency_count: u32,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredCatalogDependencyReadout {
    pub entry_id: NativeUtf8Slice,
    pub reference: NativeAuthoredAssetReference,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredCatalogReadoutLease {
    pub handle: NativeAuthoredCatalogReadoutLeaseHandle,
    pub canonical_hash: NativeUtf8Slice,
    pub entry_count: u32,
    pub entries: *const NativeAuthoredCatalogEntryReadout,
    pub entries_len: usize,
    pub dependencies: *const NativeAuthoredCatalogDependencyReadout,
    pub dependencies_len: usize,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeAuthoredResolvedEntryLeaseHandle {
    pub value: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredResolvedEntryLease {
    pub handle: NativeAuthoredResolvedEntryLeaseHandle,
    pub entry: *const NativeAuthoredCatalogEntryReadout,
    pub entry_len: usize,
    pub dependencies: *const NativeAuthoredCatalogDependencyReadout,
    pub dependencies_len: usize,
}

pub type NativeAdmitAuthoredCatalog = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAuthoredCatalogAdmitRequest,
    *mut NativeAuthoredCatalogHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeAdmitAuthoredCatalogFromContent = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAuthoredCatalogFromContentRequest,
    *mut NativeAuthoredCatalogHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyAuthoredCatalog =
    unsafe extern "C" fn(*mut c_void, NativeAuthoredCatalogHandle) -> i32;
pub type NativeReadAuthoredCatalog = unsafe extern "C" fn(
    *mut c_void,
    NativeAuthoredCatalogHandle,
    *mut NativeAuthoredCatalogReadoutLease,
) -> i32;
pub type NativeDestroyAuthoredCatalogReadoutLease =
    unsafe extern "C" fn(*mut c_void, NativeAuthoredCatalogReadoutLeaseHandle) -> i32;
pub type NativeResolveAuthoredCatalogReference = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAuthoredCatalogResolveRequest,
    *mut NativeAuthoredResolvedEntryLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyAuthoredResolvedEntryLease =
    unsafe extern "C" fn(*mut c_void, NativeAuthoredResolvedEntryLeaseHandle) -> i32;
pub type NativeDestroyAuthoredContentOperationDiagnosticLease =
    unsafe extern "C" fn(*mut c_void, NativeEngineDiagnosticLeaseHandle) -> i32;
