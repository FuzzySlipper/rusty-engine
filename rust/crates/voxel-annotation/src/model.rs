use serde::{Deserialize, Serialize};

pub const VOXEL_ANNOTATION_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoxelAnnotationLimits {
    pub max_regions: usize,
    pub max_runs_per_region: usize,
    pub max_total_assigned_cells: u64,
    pub max_tags_per_region: usize,
    pub max_provenance_refs: usize,
    pub max_string_bytes: usize,
}

impl Default for VoxelAnnotationLimits {
    fn default() -> Self {
        Self {
            max_regions: 4_096,
            max_runs_per_region: 16_384,
            max_total_assigned_cells: 8_388_608,
            max_tags_per_region: 32,
            max_provenance_refs: 4_096,
            max_string_bytes: 4_096,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VoxelAnnotationKind {
    Selection,
    Room,
    Portal,
    SpawnArea,
    Cover,
    Hazard,
    NavigationHint,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VoxelAnnotationProvenanceKind {
    Authored,
    ImportedReference,
    RuntimeExport,
    Generated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VoxelAnnotationDiagnosticCode {
    UnsupportedSchema,
    InvalidLayerId,
    InvalidTarget,
    TargetHashMismatch,
    InvalidBounds,
    InvalidRegionId,
    DuplicateRegionId,
    UnknownParentRegion,
    ParentCycle,
    InvalidSparseRun,
    DuplicateCell,
    RegionOutOfBounds,
    QuotaExceeded,
    InvalidProvenance,
    ContentHashMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelAnnotationDiagnostic {
    pub code: VoxelAnnotationDiagnosticCode,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelAnnotationBounds {
    pub min: [i64; 3],
    pub max: [i64; 3],
}

impl VoxelAnnotationBounds {
    pub fn contains(self, coordinate: [i64; 3]) -> bool {
        (0..3).all(|axis| coordinate[axis] >= self.min[axis] && coordinate[axis] <= self.max[axis])
    }

    pub fn contains_bounds(self, inner: Self) -> bool {
        self.contains(inner.min) && self.contains(inner.max)
    }

    pub fn intersects(self, other: Self) -> bool {
        (0..3).all(|axis| self.min[axis] <= other.max[axis] && other.min[axis] <= self.max[axis])
    }

    pub fn is_valid(self) -> bool {
        (0..3).all(|axis| self.min[axis] <= self.max[axis])
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelAnnotationSparseRun {
    pub start: [i64; 3],
    pub length: u32,
}

impl VoxelAnnotationSparseRun {
    pub fn end_x(self) -> Option<i64> {
        self.start[0].checked_add(i64::from(self.length).checked_sub(1)?)
    }

    pub fn bounds(self) -> Option<VoxelAnnotationBounds> {
        Some(VoxelAnnotationBounds {
            min: self.start,
            max: [self.end_x()?, self.start[1], self.start[2]],
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelAnnotationSelection {
    pub sparse_runs: Vec<VoxelAnnotationSparseRun>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelAnnotationProvenanceRef {
    pub kind: VoxelAnnotationProvenanceKind,
    pub uri: String,
    pub content_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelAnnotationRegion {
    pub region_id: String,
    pub label: String,
    pub kind: VoxelAnnotationKind,
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_region_id: Option<String>,
    pub bounds: VoxelAnnotationBounds,
    pub selection: VoxelAnnotationSelection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelAnnotationContentHashes {
    pub canonical_layer: String,
    pub membership_data: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelAnnotationLayerDraft {
    pub layer_id: String,
    pub target_voxel_asset_id: String,
    pub target_voxel_data_hash: String,
    pub target_bounds: VoxelAnnotationBounds,
    pub regions: Vec<VoxelAnnotationRegion>,
    pub provenance: Vec<VoxelAnnotationProvenanceRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelAnnotationLayer {
    pub schema_version: u32,
    pub layer_id: String,
    pub target_voxel_asset_id: String,
    pub target_voxel_data_hash: String,
    pub target_bounds: VoxelAnnotationBounds,
    pub regions: Vec<VoxelAnnotationRegion>,
    pub provenance: Vec<VoxelAnnotationProvenanceRef>,
    pub content_hashes: VoxelAnnotationContentHashes,
}
