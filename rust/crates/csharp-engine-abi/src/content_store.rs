use crate::*;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeContentStoreHandle {
    pub value: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeContentStoreSnapshotHandle {
    pub value: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeContentStoreSnapshotLeaseHandle {
    pub value: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContentStoreOpenRequest {
    pub scope: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeContentStoreIdentity {
    pub revision: u64,
    pub manifest_hash: NativeContentSha256,
    pub content_set_hash: NativeContentSha256,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeContentStoreHash {
    pub word0: u64,
    pub word1: u64,
    pub word2: u64,
    pub word3: u64,
}
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeContentStoreArtifactClass {
    Durable = 0,
    Generated = 1,
    Cache = 2,
}
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeContentStoreArtifactRoleKind {
    AssetCatalog = 0,
    AssetLock = 1,
    SceneDocument = 2,
    PrefabRegistry = 3,
    EntityStateSnapshot = 4,
    VoxelAsset = 5,
    VoxelObject = 6,
    VoxelAnnotation = 7,
    ImportedAsset = 8,
    GeneratedMetadata = 9,
    Resource = 10,
    Cache = 11,
}
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeContentStoreLoadStage {
    AssetAuthority = 0,
    AssetData = 1,
    Annotations = 2,
    Prefabs = 3,
    Scenes = 4,
    EntityState = 5,
    Resources = 6,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContentStoreArtifactDefinition {
    pub path: NativeUtf8Slice,
    pub class: NativeContentStoreArtifactClass,
    pub role_kind: NativeContentStoreArtifactRoleKind,
    pub resource_role: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContentStoreArtifactReadoutRow {
    pub path: NativeUtf8Slice,
    pub class: NativeContentStoreArtifactClass,
    pub role_kind: NativeContentStoreArtifactRoleKind,
    pub resource_role: NativeUtf8Slice,
    pub has_hash: bool,
    pub hash: NativeContentStoreHash,
    pub has_byte_length: bool,
    pub byte_length: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContentStoreLoadPlanRow {
    pub path: NativeUtf8Slice,
    pub stage: NativeContentStoreLoadStage,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContentStoreSnapshotLease {
    pub handle: NativeContentStoreSnapshotLeaseHandle,
    pub identity: NativeContentStoreIdentity,
    pub artifacts: *const NativeContentStoreArtifactReadoutRow,
    pub artifacts_len: usize,
    pub load_plan: *const NativeContentStoreLoadPlanRow,
    pub load_plan_len: usize,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContentStoreBodyRequest {
    pub snapshot: NativeContentStoreSnapshotHandle,
    pub path: NativeUtf8Slice,
    pub offset: u64,
    pub max_bytes: u32,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContentStoreWriteRow {
    pub path: NativeUtf8Slice,
    pub bytes: NativeByteSlice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContentStoreMoveRow {
    pub from: NativeUtf8Slice,
    pub to: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContentStoreDeleteRow {
    pub path: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContentStorePublishRequest {
    pub store: NativeContentStoreHandle,
    pub expected: NativeContentStoreIdentity,
    pub artifacts: *const NativeContentStoreArtifactDefinition,
    pub artifacts_len: usize,
    pub writes: *const NativeContentStoreWriteRow,
    pub writes_len: usize,
    pub moves: *const NativeContentStoreMoveRow,
    pub moves_len: usize,
    pub deletes: *const NativeContentStoreDeleteRow,
    pub deletes_len: usize,
}
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeContentStorePublishStatus {
    Published = 0,
    Stale = 1,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContentStorePublishReceipt {
    pub status: NativeContentStorePublishStatus,
    pub identity: NativeContentStoreIdentity,
    pub candidate_hash: NativeContentSha256,
}
