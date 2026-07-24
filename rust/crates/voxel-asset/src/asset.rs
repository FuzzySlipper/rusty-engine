use serde::{Deserialize, Serialize};

pub const VOXEL_ASSET_SCHEMA_VERSION: u32 = 1;

/// One complete, self-validating voxel-volume artifact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelAsset {
    pub schema_version: u32,
    pub asset_id: String,
    pub grid: VoxelAssetGrid,
    pub bounds: VoxelAssetBounds,
    pub representation: VoxelRepresentation,
    pub material_map: Vec<VoxelAssetMaterialMapping>,
    pub provenance: VoxelAssetProvenance,
    pub content_hash: String,
}

/// Local voxel coordinates become engine addresses by adding `origin`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelAssetGrid {
    pub coordinate_system: VoxelCoordinateSystem,
    pub cell_size: f64,
    pub chunk_size: u32,
    pub origin: [i64; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VoxelCoordinateSystem {
    RightHandedYUp,
}

/// Inclusive bounds in the asset's local voxel coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelAssetBounds {
    pub min: [i64; 3],
    pub max: [i64; 3],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelRepresentation {
    pub kind: VoxelRepresentationKind,
    pub sparse_runs: Vec<VoxelSparseRun>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VoxelRepresentationKind {
    SparseRuns,
}

/// A run of solid cells along +X. Omitted cells are empty.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelSparseRun {
    pub start: [i64; 3],
    pub length: u32,
    pub material_slot: u16,
}

/// The explicit source-to-runtime material choice used by conversion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelAssetMaterialMapping {
    pub source_material_slot: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_material_name: Option<String>,
    pub voxel_material_slot: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelAssetProvenance {
    pub kind: VoxelAssetProvenanceKind,
    pub source_path: String,
    pub source_sha256: String,
    pub source_byte_count: u64,
    pub converter: String,
    pub settings_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license_path: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VoxelAssetProvenanceKind {
    ConvertedStaticMesh,
}
