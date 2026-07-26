use asset_catalog::{encode_catalog, encode_lock, AssetCatalog, AssetLock};
use authored_scene::{encode_scene, FlatSceneDocument};
use entity_state::{encode_durable_snapshot, EntityState};
use voxel_annotation::{encode_annotation_layer, VoxelAnnotationLayer};
use voxel_asset::{encode_voxel_asset, encode_voxel_object, VoxelAsset, VoxelObjectAsset};

use crate::{encode_prefab_registry, ContentBody, ValidatedPrefabRegistry};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnerCodecError {
    pub owner: &'static str,
    pub message: String,
}

impl OwnerCodecError {
    fn new(owner: &'static str, error: impl std::fmt::Display) -> Self {
        Self {
            owner,
            message: error.to_string(),
        }
    }
}

impl std::fmt::Display for OwnerCodecError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{} codec failed: {}", self.owner, self.message)
    }
}

impl std::error::Error for OwnerCodecError {}

pub fn asset_catalog_body(
    path: impl Into<String>,
    catalog: &AssetCatalog,
) -> Result<ContentBody, OwnerCodecError> {
    encode_catalog(catalog)
        .map(|encoded| ContentBody::new(path, encoded.into_bytes()))
        .map_err(|error| OwnerCodecError::new("asset-catalog", error))
}

pub fn asset_lock_body(
    path: impl Into<String>,
    lock: &AssetLock,
) -> Result<ContentBody, OwnerCodecError> {
    encode_lock(lock)
        .map(|encoded| ContentBody::new(path, encoded.into_bytes()))
        .map_err(|error| OwnerCodecError::new("asset-lock", error))
}

pub fn scene_document_body(
    path: impl Into<String>,
    scene: &FlatSceneDocument,
) -> Result<ContentBody, OwnerCodecError> {
    encode_scene(scene)
        .map(|encoded| ContentBody::new(path, encoded.into_bytes()))
        .map_err(|error| OwnerCodecError::new("authored-scene", error))
}

pub fn durable_entity_state_body(
    path: impl Into<String>,
    state: &EntityState,
) -> Result<ContentBody, OwnerCodecError> {
    encode_durable_snapshot(state)
        .map(|encoded| ContentBody::new(path, encoded.into_bytes()))
        .map_err(|error| OwnerCodecError::new("entity-state", error))
}

pub fn prefab_registry_body(
    path: impl Into<String>,
    registry: &ValidatedPrefabRegistry,
) -> Result<ContentBody, OwnerCodecError> {
    encode_prefab_registry(registry)
        .map(|encoded| ContentBody::new(path, encoded.into_bytes()))
        .map_err(|error| OwnerCodecError::new("prefab-registry", error))
}

pub fn voxel_asset_body(
    path: impl Into<String>,
    asset: &VoxelAsset,
) -> Result<ContentBody, OwnerCodecError> {
    encode_voxel_asset(asset)
        .map(|encoded| ContentBody::new(path, encoded.into_bytes()))
        .map_err(|error| OwnerCodecError::new("voxel-asset", error))
}

pub fn voxel_object_body(
    path: impl Into<String>,
    object: &VoxelObjectAsset,
) -> Result<ContentBody, OwnerCodecError> {
    encode_voxel_object(object)
        .map(|encoded| ContentBody::new(path, encoded.into_bytes()))
        .map_err(|error| OwnerCodecError::new("voxel-object", error))
}

pub fn voxel_annotation_body(
    path: impl Into<String>,
    layer: &VoxelAnnotationLayer,
) -> Result<ContentBody, OwnerCodecError> {
    encode_annotation_layer(layer)
        .map(|encoded| ContentBody::new(path, encoded.into_bytes()))
        .map_err(|error| OwnerCodecError::new("voxel-annotation", error))
}
