use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{
    validate_asset_id, RenderAssetError, RenderAssetKind, RenderMetadata, Transform,
    TransformError, JSON_SAFE_U64_MAX,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MeshAttributeKind {
    F32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MeshAttributeName {
    Position,
    Normal,
    Uv,
    Color,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MeshAttribute {
    pub name: MeshAttributeName,
    pub components: u8,
    pub kind: MeshAttributeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MeshIndexWidth {
    U32,
}

/// Stable byte layout used by content-addressed mesh resources. The matching
/// resource header and stream order are defined by `mesh_resource`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MeshResourceEncoding {
    PackedStreamsLeV1,
    PackedStreamsLeV2,
    PackedStreamsLeV3,
}

/// Largest integer-valued tile coordinate that survives an f32 transport
/// exactly. Voxel producers use the same limit when admitting coordinates.
pub const MAX_EXACT_VOXEL_TILE_COORDINATE: f32 = 16_777_216.0;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MeshBufferLayout {
    pub vertex_count: u32,
    pub index_count: u32,
    pub index_width: MeshIndexWidth,
    pub attributes: Vec<MeshAttribute>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MeshGroupDescriptor {
    pub material_slot: u16,
    pub start: u32,
    pub count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MeshBoundsDescriptor {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

impl MeshBoundsDescriptor {
    pub fn validate(self) -> Result<(), MeshDescriptorError> {
        if !self
            .min
            .iter()
            .chain(self.max.iter())
            .all(|value| value.is_finite())
        {
            return Err(MeshDescriptorError::InvalidBounds);
        }
        if (0..3).any(|axis| self.max[axis] < self.min[axis]) {
            return Err(MeshDescriptorError::InvalidBounds);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MeshProvenance {
    VoxelChunk,
    VoxelObject,
    StaticAsset,
    #[default]
    Generated,
    Debug,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum MeshPayloadSource {
    Inline {
        positions: Vec<f32>,
        normals: Vec<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        uvs: Option<Vec<f32>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        colors: Option<Vec<f32>>,
        indices: Vec<u32>,
    },
    /// Shared bytes are resolved through the renderer resource provider. This
    /// handle is scoped to that provider and is not a general runtime bridge.
    SharedBuffer {
        buffer: u64,
        positions_byte_offset: u32,
        normals_byte_offset: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        uvs_byte_offset: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        colors_byte_offset: Option<u32>,
        indices_byte_offset: u32,
    },
    /// Durable, content-addressed bytes resolved by an explicit renderer host.
    /// The identity names bytes, not a filesystem path or network location.
    Resource {
        resource: String,
        content_hash: String,
        byte_length: u32,
        encoding: MeshResourceEncoding,
        positions_byte_offset: u32,
        normals_byte_offset: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        uvs_byte_offset: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        colors_byte_offset: Option<u32>,
        indices_byte_offset: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MeshPayloadDescriptor {
    pub layout: MeshBufferLayout,
    pub groups: Vec<MeshGroupDescriptor>,
    pub bounds: MeshBoundsDescriptor,
    pub source: MeshPayloadSource,
    pub provenance: MeshProvenance,
}

impl MeshPayloadDescriptor {
    pub fn validate(&self) -> Result<(), MeshDescriptorError> {
        self.bounds.validate()?;
        validate_attributes(&self.layout)?;
        match &self.source {
            MeshPayloadSource::Inline {
                positions,
                normals,
                uvs,
                colors,
                indices,
            } => {
                if !positions
                    .iter()
                    .chain(normals)
                    .chain(uvs.iter().flatten())
                    .chain(colors.iter().flatten())
                    .all(|value| value.is_finite())
                {
                    return Err(MeshDescriptorError::NonFiniteAttribute);
                }
                let expected = self.layout.vertex_count as usize * 3;
                if positions.len() != expected {
                    return Err(MeshDescriptorError::AttributeLengthMismatch {
                        name: MeshAttributeName::Position,
                        expected,
                        actual: positions.len(),
                    });
                }
                if normals.len() != expected {
                    return Err(MeshDescriptorError::AttributeLengthMismatch {
                        name: MeshAttributeName::Normal,
                        expected,
                        actual: normals.len(),
                    });
                }
                validate_optional_uv_stream(&self.layout, uvs.as_deref())?;
                validate_optional_color_stream(&self.layout, colors.as_deref())?;
                if matches!(
                    self.provenance,
                    MeshProvenance::VoxelChunk | MeshProvenance::VoxelObject
                ) && uvs.as_ref().is_some_and(|values| {
                    values
                        .iter()
                        .any(|value| value.abs() > MAX_EXACT_VOXEL_TILE_COORDINATE)
                }) {
                    return Err(MeshDescriptorError::VoxelTileCoordinateOutOfRange);
                }
                if indices.len() != self.layout.index_count as usize {
                    return Err(MeshDescriptorError::IndexLengthMismatch {
                        expected: self.layout.index_count as usize,
                        actual: indices.len(),
                    });
                }
                if let Some(index) = indices
                    .iter()
                    .copied()
                    .find(|index| *index >= self.layout.vertex_count)
                {
                    return Err(MeshDescriptorError::IndexOutOfRange {
                        index,
                        vertex_count: self.layout.vertex_count,
                    });
                }
            }
            MeshPayloadSource::SharedBuffer {
                buffer,
                uvs_byte_offset,
                colors_byte_offset,
                ..
            } => {
                if *buffer > JSON_SAFE_U64_MAX {
                    return Err(MeshDescriptorError::UnsafeSharedBufferId { buffer: *buffer });
                }
                validate_optional_uv_offset(&self.layout, *uvs_byte_offset)?;
                validate_optional_color_offset(&self.layout, *colors_byte_offset)?;
            }
            MeshPayloadSource::Resource {
                resource,
                content_hash,
                byte_length,
                encoding,
                positions_byte_offset,
                normals_byte_offset,
                uvs_byte_offset,
                colors_byte_offset,
                indices_byte_offset,
                ..
            } => validate_resource_source(
                &self.layout,
                resource,
                content_hash,
                *byte_length,
                ResourceStreamSource {
                    encoding: *encoding,
                    positions_byte_offset: *positions_byte_offset,
                    normals_byte_offset: *normals_byte_offset,
                    uvs_byte_offset: *uvs_byte_offset,
                    colors_byte_offset: *colors_byte_offset,
                    indices_byte_offset: *indices_byte_offset,
                },
            )?,
        }

        let mut cursor = 0_u32;
        for group in &self.groups {
            if group.start != cursor {
                return Err(MeshDescriptorError::GroupsDoNotTile {
                    expected_start: cursor,
                    actual_start: group.start,
                });
            }
            cursor =
                cursor
                    .checked_add(group.count)
                    .ok_or(MeshDescriptorError::GroupOutOfRange {
                        start: group.start,
                        count: group.count,
                        index_count: self.layout.index_count,
                    })?;
            if cursor > self.layout.index_count {
                return Err(MeshDescriptorError::GroupOutOfRange {
                    start: group.start,
                    count: group.count,
                    index_count: self.layout.index_count,
                });
            }
        }
        if cursor != self.layout.index_count {
            return Err(MeshDescriptorError::GroupsDoNotCover {
                covered: cursor,
                index_count: self.layout.index_count,
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct ResourceStreamSource {
    encoding: MeshResourceEncoding,
    positions_byte_offset: u32,
    normals_byte_offset: u32,
    uvs_byte_offset: Option<u32>,
    colors_byte_offset: Option<u32>,
    indices_byte_offset: u32,
}

fn validate_resource_source(
    layout: &MeshBufferLayout,
    resource: &str,
    content_hash: &str,
    byte_length: u32,
    streams: ResourceStreamSource,
) -> Result<(), MeshDescriptorError> {
    let ResourceStreamSource {
        encoding,
        positions_byte_offset,
        normals_byte_offset,
        uvs_byte_offset,
        colors_byte_offset,
        indices_byte_offset,
    } = streams;
    crate::validate_mesh_resource_identity(resource, content_hash)
        .map_err(|_| MeshDescriptorError::InvalidResourceIdentity)?;
    if !(crate::MESH_RESOURCE_HEADER_BYTES..=crate::MAX_MESH_RESOURCE_BYTES).contains(&byte_length)
    {
        return Err(MeshDescriptorError::InvalidResourceByteLength { byte_length });
    }
    validate_optional_uv_offset(layout, uvs_byte_offset)?;
    validate_optional_color_offset(layout, colors_byte_offset)?;
    if matches!(encoding, MeshResourceEncoding::PackedStreamsLeV1)
        && (uvs_byte_offset.is_some() || colors_byte_offset.is_some())
    {
        return Err(MeshDescriptorError::ResourceEncodingDoesNotMatchAttributes);
    }
    if matches!(encoding, MeshResourceEncoding::PackedStreamsLeV2)
        && (uvs_byte_offset.is_none() || colors_byte_offset.is_some())
    {
        return Err(MeshDescriptorError::ResourceEncodingDoesNotMatchAttributes);
    }
    if matches!(encoding, MeshResourceEncoding::PackedStreamsLeV3) && colors_byte_offset.is_none() {
        return Err(MeshDescriptorError::ResourceEncodingDoesNotMatchAttributes);
    }
    for offset in [
        positions_byte_offset,
        normals_byte_offset,
        indices_byte_offset,
    ]
    .into_iter()
    .chain(uvs_byte_offset)
    .chain(colors_byte_offset)
    {
        if offset < crate::MESH_RESOURCE_HEADER_BYTES || offset % 4 != 0 {
            return Err(MeshDescriptorError::InvalidResourceOffset { offset });
        }
    }

    let positions_bytes = u64::from(layout.vertex_count) * 3 * 4;
    let normals_bytes = positions_bytes;
    let uvs_bytes = u64::from(layout.vertex_count) * 2 * 4;
    let colors_bytes = u64::from(layout.vertex_count) * 4 * 4;
    let indices_bytes = u64::from(layout.index_count) * 4;
    let positions_end = u64::from(positions_byte_offset) + positions_bytes;
    let normals_end = u64::from(normals_byte_offset) + normals_bytes;
    let uvs_end = uvs_byte_offset.map(|offset| u64::from(offset) + uvs_bytes);
    let colors_end = colors_byte_offset.map(|offset| u64::from(offset) + colors_bytes);
    let indices_end = u64::from(indices_byte_offset) + indices_bytes;
    if positions_end > u64::from(byte_length)
        || normals_end > u64::from(byte_length)
        || uvs_end.is_some_and(|end| end > u64::from(byte_length))
        || colors_end.is_some_and(|end| end > u64::from(byte_length))
        || indices_end > u64::from(byte_length)
    {
        return Err(MeshDescriptorError::ResourceStreamOutOfRange { byte_length });
    }
    if positions_end > u64::from(normals_byte_offset)
        || uvs_byte_offset.is_some_and(|offset| normals_end > u64::from(offset))
        || colors_byte_offset
            .is_some_and(|offset| uvs_end.unwrap_or(normals_end) > u64::from(offset))
        || colors_end.or(uvs_end).unwrap_or(normals_end) > u64::from(indices_byte_offset)
    {
        return Err(MeshDescriptorError::ResourceStreamsOverlap);
    }
    Ok(())
}

fn validate_optional_uv_stream(
    layout: &MeshBufferLayout,
    uvs: Option<&[f32]>,
) -> Result<(), MeshDescriptorError> {
    let declared = layout
        .attributes
        .iter()
        .any(|attribute| attribute.name == MeshAttributeName::Uv);
    if declared != uvs.is_some() {
        return Err(MeshDescriptorError::OptionalAttributeSourceMismatch {
            name: MeshAttributeName::Uv,
        });
    }
    if let Some(values) = uvs {
        let expected = layout.vertex_count as usize * 2;
        if values.len() != expected {
            return Err(MeshDescriptorError::AttributeLengthMismatch {
                name: MeshAttributeName::Uv,
                expected,
                actual: values.len(),
            });
        }
    }
    Ok(())
}

fn validate_optional_uv_offset(
    layout: &MeshBufferLayout,
    offset: Option<u32>,
) -> Result<(), MeshDescriptorError> {
    let declared = layout
        .attributes
        .iter()
        .any(|attribute| attribute.name == MeshAttributeName::Uv);
    if declared != offset.is_some() {
        return Err(MeshDescriptorError::OptionalAttributeSourceMismatch {
            name: MeshAttributeName::Uv,
        });
    }
    Ok(())
}

fn validate_optional_color_stream(
    layout: &MeshBufferLayout,
    colors: Option<&[f32]>,
) -> Result<(), MeshDescriptorError> {
    let declared = layout
        .attributes
        .iter()
        .any(|attribute| attribute.name == MeshAttributeName::Color);
    if declared != colors.is_some() {
        return Err(MeshDescriptorError::OptionalAttributeSourceMismatch {
            name: MeshAttributeName::Color,
        });
    }
    if let Some(values) = colors {
        let expected = layout.vertex_count as usize * 4;
        if values.len() != expected {
            return Err(MeshDescriptorError::AttributeLengthMismatch {
                name: MeshAttributeName::Color,
                expected,
                actual: values.len(),
            });
        }
        if values.iter().any(|value| !(0.0..=1.0).contains(value)) {
            return Err(MeshDescriptorError::ColorOutOfRange);
        }
    }
    Ok(())
}

fn validate_optional_color_offset(
    layout: &MeshBufferLayout,
    offset: Option<u32>,
) -> Result<(), MeshDescriptorError> {
    let declared = layout
        .attributes
        .iter()
        .any(|attribute| attribute.name == MeshAttributeName::Color);
    if declared != offset.is_some() {
        return Err(MeshDescriptorError::OptionalAttributeSourceMismatch {
            name: MeshAttributeName::Color,
        });
    }
    Ok(())
}

fn validate_attributes(layout: &MeshBufferLayout) -> Result<(), MeshDescriptorError> {
    let mut names = BTreeSet::new();
    for attribute in &layout.attributes {
        if !names.insert(attribute.name as u8) {
            return Err(MeshDescriptorError::DuplicateAttribute {
                name: attribute.name,
            });
        }
        let expected = match attribute.name {
            MeshAttributeName::Position | MeshAttributeName::Normal => 3,
            MeshAttributeName::Uv => 2,
            MeshAttributeName::Color => 4,
        };
        if attribute.components != expected {
            return Err(MeshDescriptorError::InvalidAttributeComponents {
                name: attribute.name,
                expected,
                actual: attribute.components,
            });
        }
    }
    for required in [MeshAttributeName::Position, MeshAttributeName::Normal] {
        if !layout.attributes.iter().any(|value| value.name == required) {
            return Err(MeshDescriptorError::MissingAttribute { name: required });
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MeshDescriptorError {
    MissingAttribute {
        name: MeshAttributeName,
    },
    DuplicateAttribute {
        name: MeshAttributeName,
    },
    InvalidAttributeComponents {
        name: MeshAttributeName,
        expected: u8,
        actual: u8,
    },
    AttributeLengthMismatch {
        name: MeshAttributeName,
        expected: usize,
        actual: usize,
    },
    IndexLengthMismatch {
        expected: usize,
        actual: usize,
    },
    IndexOutOfRange {
        index: u32,
        vertex_count: u32,
    },
    GroupsDoNotTile {
        expected_start: u32,
        actual_start: u32,
    },
    GroupsDoNotCover {
        covered: u32,
        index_count: u32,
    },
    GroupOutOfRange {
        start: u32,
        count: u32,
        index_count: u32,
    },
    InvalidBounds,
    NonFiniteAttribute,
    OptionalAttributeSourceMismatch {
        name: MeshAttributeName,
    },
    ColorOutOfRange,
    VoxelTileCoordinateOutOfRange,
    UnsafeSharedBufferId {
        buffer: u64,
    },
    InvalidResourceIdentity,
    InvalidResourceByteLength {
        byte_length: u32,
    },
    InvalidResourceOffset {
        offset: u32,
    },
    ResourceStreamOutOfRange {
        byte_length: u32,
    },
    ResourceStreamsOverlap,
    ResourceEncodingDoesNotMatchAttributes,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MeshMaterialSlot {
    pub slot: u16,
    pub material: String,
}

impl MeshMaterialSlot {
    fn validate(&self) -> Result<(), RenderAssetError> {
        validate_asset_id(&self.material, RenderAssetKind::Material)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum MeshCollisionPolicy {
    #[default]
    VisualOnly,
    Proxy {
        proxy_asset: String,
    },
    AabbFallback,
    /// Use the exact validated mesh positions and triangle indices. The caller
    /// resolves any content-addressed payload bytes before constructing the
    /// host-neutral collision projection.
    Trimesh,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StaticMeshAsset {
    pub asset: String,
    pub payload: MeshPayloadDescriptor,
    pub material_slots: Vec<MeshMaterialSlot>,
    pub collision: MeshCollisionPolicy,
}

impl StaticMeshAsset {
    pub fn validate(&self) -> Result<(), StaticMeshError> {
        validate_asset_id(&self.asset, RenderAssetKind::StaticMesh)
            .map_err(StaticMeshError::Asset)?;
        self.payload.validate().map_err(StaticMeshError::Payload)?;
        validate_slots(&self.material_slots).map_err(StaticMeshError::MaterialSlot)?;
        for group in &self.payload.groups {
            if !self
                .material_slots
                .iter()
                .any(|slot| slot.slot == group.material_slot)
            {
                return Err(StaticMeshError::GroupSlotUnbound {
                    slot: group.material_slot,
                });
            }
        }
        if let MeshCollisionPolicy::Proxy { proxy_asset } = &self.collision {
            validate_asset_id(proxy_asset, RenderAssetKind::StaticMesh)
                .map_err(StaticMeshError::CollisionProxy)?;
        }
        Ok(())
    }

    pub fn resolve_collision(&self) -> CollisionResolution {
        match &self.collision {
            MeshCollisionPolicy::VisualOnly => CollisionResolution::None,
            MeshCollisionPolicy::Proxy { proxy_asset } => CollisionResolution::Proxy {
                proxy_asset: proxy_asset.clone(),
            },
            MeshCollisionPolicy::AabbFallback => CollisionResolution::Aabb {
                min: self.payload.bounds.min,
                max: self.payload.bounds.max,
            },
            MeshCollisionPolicy::Trimesh => CollisionResolution::Trimesh {
                payload: self.payload.clone(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum CollisionResolution {
    None,
    Proxy { proxy_asset: String },
    Aabb { min: [f32; 3], max: [f32; 3] },
    Trimesh { payload: MeshPayloadDescriptor },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StaticMeshError {
    Asset(RenderAssetError),
    Payload(MeshDescriptorError),
    MaterialSlot(MeshMaterialSlotError),
    GroupSlotUnbound { slot: u16 },
    CollisionProxy(RenderAssetError),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StaticMeshInstanceDescriptor {
    pub asset: String,
    pub transform: Transform,
    pub visible: bool,
    pub material_overrides: Vec<MeshMaterialSlot>,
    pub metadata: RenderMetadata,
}

impl StaticMeshInstanceDescriptor {
    pub fn validate(&self) -> Result<(), StaticMeshInstanceError> {
        validate_asset_id(&self.asset, RenderAssetKind::StaticMesh)
            .map_err(StaticMeshInstanceError::Asset)?;
        self.transform
            .validate()
            .map_err(StaticMeshInstanceError::Transform)?;
        validate_slots(&self.material_overrides).map_err(StaticMeshInstanceError::MaterialSlot)?;
        self.metadata
            .validate()
            .map_err(StaticMeshInstanceError::Metadata)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StaticMeshInstanceError {
    Asset(RenderAssetError),
    Transform(TransformError),
    MaterialSlot(MeshMaterialSlotError),
    Metadata(crate::NodeError),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AnimatedMeshRuntimeFormat {
    #[default]
    Glb,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AnimationLoopMode {
    Once,
    #[default]
    Repeat,
    PingPong,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnimationClipDescriptor {
    pub id: String,
    pub name: Option<String>,
    pub duration_seconds: Option<f32>,
}

/// A semantic signature for direct clip playback against a particular rig.
///
/// This is deliberately a declared, hash-addressable fact rather than a loose
/// "humanoid" label. Renderer backends must additionally compare it with the
/// skeleton and channels they actually decode from the GLB before binding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnimationRigSignature {
    pub joints: Vec<AnimationRigJoint>,
    pub bind_rest_hash: String,
    pub bind_rest_convention: AnimationBindRestConvention,
    pub root_convention: AnimationRootConvention,
    pub root_joint_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AnimationBindRestConvention {
    LocalMatrixV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnimationRigJoint {
    pub id: String,
    pub parent: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AnimationRootConvention {
    InPlace,
    AuthoredRootTranslation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnimationClipPack {
    pub asset: String,
    pub runtime_format: AnimatedMeshRuntimeFormat,
    pub content_hash: String,
    pub rig: AnimationRigSignature,
    pub clips: Vec<AnimationClipDescriptor>,
    pub provenance: AnimationClipPackProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnimationClipPackProvenance {
    pub producer: String,
    pub source_hash: String,
    pub target_hash: String,
    pub license: String,
}

impl AnimationClipPack {
    pub fn validate(&self) -> Result<(), AnimationClipPackError> {
        if self.asset.trim().is_empty() {
            return Err(AnimationClipPackError::EmptyAsset);
        }
        if !valid_sha256(&self.content_hash) {
            return Err(AnimationClipPackError::EmptyContentHash);
        }
        if self.provenance.producer.trim().is_empty()
            || !valid_sha256(&self.provenance.source_hash)
            || !valid_sha256(&self.provenance.target_hash)
            || self.provenance.license.trim().is_empty()
        {
            return Err(AnimationClipPackError::EmptyProvenance);
        }
        if self.rig.joints.is_empty() || self.rig.joints.len() > 256 {
            return Err(AnimationClipPackError::InvalidRig);
        }
        if !valid_sha256(&self.rig.bind_rest_hash) {
            return Err(AnimationClipPackError::InvalidRig);
        }
        let mut joints = BTreeSet::new();
        for joint in &self.rig.joints {
            if !valid_joint_id(&joint.id) || !joints.insert(joint.id.as_str()) {
                return Err(AnimationClipPackError::InvalidRig);
            }
        }
        for joint in &self.rig.joints {
            if joint
                .parent
                .as_ref()
                .is_some_and(|parent| parent == &joint.id || !joints.contains(parent.as_str()))
            {
                return Err(AnimationClipPackError::InvalidRig);
            }
        }
        if self
            .rig
            .joints
            .iter()
            .filter(|joint| joint.parent.is_none())
            .count()
            != 1
            || !valid_joint_id(&self.rig.root_joint_id)
            || !self
                .rig
                .joints
                .iter()
                .any(|joint| joint.id == self.rig.root_joint_id && joint.parent.is_none())
        {
            return Err(AnimationClipPackError::InvalidRig);
        }
        for joint in &self.rig.joints {
            let mut seen = BTreeSet::new();
            let mut current = Some(joint.id.as_str());
            while let Some(id) = current {
                if !seen.insert(id) {
                    return Err(AnimationClipPackError::InvalidRig);
                }
                current = self
                    .rig
                    .joints
                    .iter()
                    .find(|candidate| candidate.id == id)
                    .and_then(|candidate| candidate.parent.as_deref());
            }
        }
        if self.clips.is_empty() || self.clips.len() > 256 {
            return Err(AnimationClipPackError::InvalidClips);
        }
        let mut ids = BTreeSet::new();
        for clip in &self.clips {
            if clip.id.trim().is_empty()
                || !ids.insert(clip.id.as_str())
                || clip
                    .duration_seconds
                    .is_some_and(|value| !value.is_finite() || value <= 0.0)
            {
                return Err(AnimationClipPackError::InvalidClips);
            }
        }
        Ok(())
    }
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .chars()
            .all(|character| character.is_ascii_hexdigit() && !character.is_ascii_uppercase())
}

fn valid_joint_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnimationClipPackError {
    EmptyAsset,
    EmptyContentHash,
    EmptyProvenance,
    InvalidRig,
    InvalidClips,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnimatedMeshAsset {
    pub asset: String,
    pub runtime_format: AnimatedMeshRuntimeFormat,
    pub content_hash: Option<String>,
    pub clips: Vec<AnimationClipDescriptor>,
    #[serde(default)]
    pub clip_packs: Vec<AnimationClipPack>,
    pub default_clip: Option<String>,
    /// Immutable mapping from the Engine-facing dense slot to the material
    /// index in the admitted GLB. This is deliberately distinct from
    /// `material_slots`: embedded GLB materials are not Engine material
    /// assets, and must not be validated as such.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub embedded_material_slots: Vec<AnimatedMeshEmbeddedMaterialSlot>,
    pub material_slots: Vec<MeshMaterialSlot>,
    pub bounds: MeshBoundsDescriptor,
}

impl AnimatedMeshAsset {
    pub fn validate(&self) -> Result<(), AnimatedMeshAssetError> {
        validate_asset_id(&self.asset, RenderAssetKind::AnimatedMesh)
            .map_err(AnimatedMeshAssetError::Asset)?;
        self.bounds
            .validate()
            .map_err(AnimatedMeshAssetError::Bounds)?;
        if self
            .content_hash
            .as_ref()
            .is_some_and(|hash| hash.trim().is_empty())
        {
            return Err(AnimatedMeshAssetError::EmptyContentHash);
        }
        let mut ids = BTreeSet::new();
        for clip in &self.clips {
            if clip.id.trim().is_empty() {
                return Err(AnimatedMeshAssetError::EmptyClipId);
            }
            if !ids.insert(clip.id.as_str()) {
                return Err(AnimatedMeshAssetError::DuplicateClipId {
                    clip: clip.id.clone(),
                });
            }
            if clip
                .duration_seconds
                .is_some_and(|value| !value.is_finite() || value <= 0.0)
            {
                return Err(AnimatedMeshAssetError::InvalidClipDuration {
                    clip: clip.id.clone(),
                });
            }
        }
        let mut pack_assets = BTreeSet::new();
        for pack in &self.clip_packs {
            pack.validate().map_err(AnimatedMeshAssetError::ClipPack)?;
            if !pack_assets.insert(pack.asset.as_str()) {
                return Err(AnimatedMeshAssetError::DuplicateClipPack {
                    asset: pack.asset.clone(),
                });
            }
            for clip in &pack.clips {
                if ids.contains(clip.id.as_str()) {
                    return Err(AnimatedMeshAssetError::EffectiveClipCollision {
                        clip: clip.id.clone(),
                    });
                }
                ids.insert(clip.id.as_str());
            }
        }
        if self
            .default_clip
            .as_ref()
            .is_some_and(|clip| !ids.contains(clip.as_str()))
        {
            return Err(AnimatedMeshAssetError::DefaultClipMissing {
                clip: self.default_clip.clone().unwrap_or_default(),
            });
        }
        validate_animated_embedded_material_slots(&self.embedded_material_slots)
            .map_err(AnimatedMeshAssetError::EmbeddedMaterialSlot)?;
        validate_slots(&self.material_slots).map_err(AnimatedMeshAssetError::MaterialSlot)
    }
}

/// A deterministic Engine-facing material slot for an admitted GLB resource.
///
/// `slot` is dense and stable for the exact admitted source. The renderer
/// resolves `source_material_slot` through GLTFLoader's material association,
/// never by Three scene traversal order. A separate future appearance override
/// may target `slot`; this mapping does not create an Engine material asset or
/// override behavior by itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnimatedMeshEmbeddedMaterialSlot {
    pub slot: u16,
    pub source_material_slot: u16,
}

pub fn validate_animated_embedded_material_slots(
    slots: &[AnimatedMeshEmbeddedMaterialSlot],
) -> Result<(), AnimatedMeshEmbeddedMaterialSlotError> {
    let mut source_slots = BTreeSet::new();
    for (expected_slot, binding) in slots.iter().enumerate() {
        let expected_slot = u16::try_from(expected_slot)
            .expect("animated GLB admission bounds material slots to u16");
        if binding.slot != expected_slot {
            return Err(AnimatedMeshEmbeddedMaterialSlotError::NonDense {
                expected: expected_slot,
                actual: binding.slot,
            });
        }
        if !source_slots.insert(binding.source_material_slot) {
            return Err(AnimatedMeshEmbeddedMaterialSlotError::DuplicateSource {
                source_material_slot: binding.source_material_slot,
            });
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnimatedMeshEmbeddedMaterialSlotError {
    NonDense { expected: u16, actual: u16 },
    DuplicateSource { source_material_slot: u16 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnimatedMeshAssetError {
    Asset(RenderAssetError),
    Bounds(MeshDescriptorError),
    EmptyContentHash,
    EmptyClipId,
    DuplicateClipId { clip: String },
    InvalidClipDuration { clip: String },
    ClipPack(AnimationClipPackError),
    DuplicateClipPack { asset: String },
    EffectiveClipCollision { clip: String },
    DefaultClipMissing { clip: String },
    EmbeddedMaterialSlot(AnimatedMeshEmbeddedMaterialSlotError),
    MaterialSlot(MeshMaterialSlotError),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnimatedMeshInstanceDescriptor {
    pub asset: String,
    pub transform: Transform,
    pub visible: bool,
    pub material_overrides: Vec<MeshMaterialSlot>,
    pub playback: Option<AnimatedMeshPlaybackCommand>,
    pub metadata: RenderMetadata,
}

impl AnimatedMeshInstanceDescriptor {
    pub fn validate(&self) -> Result<(), AnimatedMeshInstanceError> {
        validate_asset_id(&self.asset, RenderAssetKind::AnimatedMesh)
            .map_err(AnimatedMeshInstanceError::Asset)?;
        self.transform
            .validate()
            .map_err(AnimatedMeshInstanceError::Transform)?;
        validate_slots(&self.material_overrides)
            .map_err(AnimatedMeshInstanceError::MaterialSlot)?;
        if let Some(playback) = &self.playback {
            playback
                .validate()
                .map_err(AnimatedMeshInstanceError::Playback)?;
        }
        self.metadata
            .validate()
            .map_err(AnimatedMeshInstanceError::Metadata)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnimatedMeshInstanceError {
    Asset(RenderAssetError),
    Transform(TransformError),
    MaterialSlot(MeshMaterialSlotError),
    Playback(AnimatedMeshPlaybackError),
    Metadata(crate::NodeError),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum AnimatedMeshPlaybackCommand {
    Play {
        clip: String,
        r#loop: AnimationLoopMode,
        speed: f32,
        weight: f32,
        restart: bool,
        fade_seconds: Option<f32>,
    },
    Stop {
        fade_seconds: Option<f32>,
    },
    /// Hold this instance at one exact point in a named clip. This is
    /// presentation playback only: callers retain all animation policy.
    Sample {
        clip: String,
        normalized_time: f32,
    },
    Pause,
    Resume,
}

impl AnimatedMeshPlaybackCommand {
    pub fn validate(&self) -> Result<(), AnimatedMeshPlaybackError> {
        match self {
            Self::Play {
                clip,
                speed,
                weight,
                fade_seconds,
                ..
            } => {
                if clip.trim().is_empty() {
                    return Err(AnimatedMeshPlaybackError::EmptyClip);
                }
                if !speed.is_finite() || *speed <= 0.0 {
                    return Err(AnimatedMeshPlaybackError::InvalidSpeed);
                }
                if !weight.is_finite() || !(0.0..=1.0).contains(weight) {
                    return Err(AnimatedMeshPlaybackError::InvalidWeight);
                }
                validate_fade(*fade_seconds)
            }
            Self::Stop { fade_seconds } => validate_fade(*fade_seconds),
            Self::Sample {
                clip,
                normalized_time,
            } => {
                if clip.trim().is_empty() {
                    return Err(AnimatedMeshPlaybackError::EmptyClip);
                }
                if !normalized_time.is_finite() || !(0.0..=1.0).contains(normalized_time) {
                    return Err(AnimatedMeshPlaybackError::InvalidNormalizedTime);
                }
                Ok(())
            }
            Self::Pause | Self::Resume => Ok(()),
        }
    }
}

fn validate_fade(fade: Option<f32>) -> Result<(), AnimatedMeshPlaybackError> {
    if fade.is_some_and(|value| !value.is_finite() || value < 0.0) {
        return Err(AnimatedMeshPlaybackError::InvalidFade);
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimatedMeshPlaybackError {
    EmptyClip,
    InvalidSpeed,
    InvalidWeight,
    InvalidFade,
    InvalidNormalizedTime,
}

pub(crate) fn validate_slots(slots: &[MeshMaterialSlot]) -> Result<(), MeshMaterialSlotError> {
    let mut seen = BTreeSet::new();
    for slot in slots {
        if !seen.insert(slot.slot) {
            return Err(MeshMaterialSlotError::Duplicate { slot: slot.slot });
        }
        slot.validate()
            .map_err(|source| MeshMaterialSlotError::InvalidMaterial {
                slot: slot.slot,
                source,
            })?;
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MeshMaterialSlotError {
    Duplicate { slot: u16 },
    InvalidMaterial { slot: u16, source: RenderAssetError },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn triangle() -> MeshPayloadDescriptor {
        MeshPayloadDescriptor {
            layout: MeshBufferLayout {
                vertex_count: 3,
                index_count: 3,
                index_width: MeshIndexWidth::U32,
                attributes: vec![
                    MeshAttribute {
                        name: MeshAttributeName::Position,
                        components: 3,
                        kind: MeshAttributeKind::F32,
                    },
                    MeshAttribute {
                        name: MeshAttributeName::Normal,
                        components: 3,
                        kind: MeshAttributeKind::F32,
                    },
                ],
            },
            groups: vec![MeshGroupDescriptor {
                material_slot: 0,
                start: 0,
                count: 3,
            }],
            bounds: MeshBoundsDescriptor {
                min: [0.0, 0.0, 0.0],
                max: [1.0, 1.0, 0.0],
            },
            source: MeshPayloadSource::Inline {
                positions: vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
                normals: vec![0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0],
                uvs: None,
                colors: None,
                indices: vec![0, 1, 2],
            },
            provenance: MeshProvenance::StaticAsset,
        }
    }

    #[test]
    fn shared_mesh_payload_validation_is_source_agnostic() {
        let payload = triangle();
        assert_eq!(payload.validate(), Ok(()));
        let asset = StaticMeshAsset {
            asset: "mesh/triangle".to_string(),
            payload,
            material_slots: vec![MeshMaterialSlot {
                slot: 0,
                material: "material/plain".to_string(),
            }],
            collision: MeshCollisionPolicy::AabbFallback,
        };
        assert_eq!(asset.validate(), Ok(()));
        assert!(matches!(
            asset.resolve_collision(),
            CollisionResolution::Aabb { .. }
        ));
    }

    #[test]
    fn embedded_animated_material_slots_are_dense_and_source_unique() {
        assert_eq!(
            validate_animated_embedded_material_slots(&[
                AnimatedMeshEmbeddedMaterialSlot {
                    slot: 0,
                    source_material_slot: 3,
                },
                AnimatedMeshEmbeddedMaterialSlot {
                    slot: 1,
                    source_material_slot: 7,
                },
            ]),
            Ok(())
        );
        assert!(matches!(
            validate_animated_embedded_material_slots(&[AnimatedMeshEmbeddedMaterialSlot {
                slot: 1,
                source_material_slot: 3,
            }]),
            Err(AnimatedMeshEmbeddedMaterialSlotError::NonDense { .. })
        ));
        assert!(matches!(
            validate_animated_embedded_material_slots(&[
                AnimatedMeshEmbeddedMaterialSlot {
                    slot: 0,
                    source_material_slot: 3,
                },
                AnimatedMeshEmbeddedMaterialSlot {
                    slot: 1,
                    source_material_slot: 3,
                },
            ]),
            Err(AnimatedMeshEmbeddedMaterialSlotError::DuplicateSource { .. })
        ));
    }

    #[test]
    fn trimesh_collision_resolves_the_exact_validated_payload() {
        let payload = triangle();
        let asset = StaticMeshAsset {
            asset: "mesh/ramp".to_string(),
            payload: payload.clone(),
            material_slots: vec![MeshMaterialSlot {
                slot: 0,
                material: "material/plain".to_string(),
            }],
            collision: MeshCollisionPolicy::Trimesh,
        };
        assert_eq!(asset.validate(), Ok(()));
        assert_eq!(
            asset.resolve_collision(),
            CollisionResolution::Trimesh { payload }
        );
    }

    #[test]
    fn group_gaps_are_rejected_instead_of_only_counted() {
        let mut payload = triangle();
        payload.groups[0].start = 1;
        assert_eq!(
            payload.validate(),
            Err(MeshDescriptorError::GroupsDoNotTile {
                expected_start: 0,
                actual_start: 1,
            })
        );
    }

    #[test]
    fn shared_buffer_ids_must_survive_the_javascript_border_exactly() {
        let mut payload = triangle();
        payload.source = MeshPayloadSource::SharedBuffer {
            buffer: JSON_SAFE_U64_MAX + 1,
            positions_byte_offset: 0,
            normals_byte_offset: 36,
            uvs_byte_offset: None,
            colors_byte_offset: None,
            indices_byte_offset: 72,
        };
        assert!(matches!(
            payload.validate(),
            Err(MeshDescriptorError::UnsafeSharedBufferId { .. })
        ));
    }

    #[test]
    fn voxel_uv_stream_must_match_layout_length_finiteness_and_exact_range() {
        let mut payload = triangle();
        payload.provenance = MeshProvenance::VoxelObject;
        payload.layout.attributes.push(MeshAttribute {
            name: MeshAttributeName::Uv,
            components: 2,
            kind: MeshAttributeKind::F32,
        });
        assert_eq!(
            payload.validate(),
            Err(MeshDescriptorError::OptionalAttributeSourceMismatch {
                name: MeshAttributeName::Uv,
            })
        );

        if let MeshPayloadSource::Inline { uvs, .. } = &mut payload.source {
            *uvs = Some(vec![0.0; 5]);
        }
        assert!(matches!(
            payload.validate(),
            Err(MeshDescriptorError::AttributeLengthMismatch {
                name: MeshAttributeName::Uv,
                ..
            })
        ));
        if let MeshPayloadSource::Inline { uvs, .. } = &mut payload.source {
            *uvs = Some(vec![0.0, 0.0, 1.0, 0.0, f32::NAN, 1.0]);
        }
        assert_eq!(
            payload.validate(),
            Err(MeshDescriptorError::NonFiniteAttribute)
        );
        if let MeshPayloadSource::Inline { uvs, .. } = &mut payload.source {
            *uvs = Some(vec![
                0.0,
                0.0,
                1.0,
                0.0,
                0.0,
                MAX_EXACT_VOXEL_TILE_COORDINATE + 2.0,
            ]);
        }
        assert_eq!(
            payload.validate(),
            Err(MeshDescriptorError::VoxelTileCoordinateOutOfRange)
        );
        if let MeshPayloadSource::Inline { uvs, .. } = &mut payload.source {
            *uvs = Some(vec![0.0, 0.0, 1.0, 0.0, 0.0, 1.0]);
        }
        assert_eq!(payload.validate(), Ok(()));
    }

    #[test]
    fn animation_clip_pack_rejects_reserved_three_binding_characters() {
        let hash = format!("sha256:{}", "a".repeat(64));
        let mut pack = AnimationClipPack {
            asset: "animation-clip-pack/test".to_owned(),
            runtime_format: AnimatedMeshRuntimeFormat::Glb,
            content_hash: hash.clone(),
            rig: AnimationRigSignature {
                joints: vec![AnimationRigJoint {
                    id: "Root".to_owned(),
                    parent: None,
                }],
                bind_rest_hash: hash.clone(),
                bind_rest_convention: AnimationBindRestConvention::LocalMatrixV1,
                root_convention: AnimationRootConvention::InPlace,
                root_joint_id: "Root".to_owned(),
            },
            clips: vec![AnimationClipDescriptor {
                id: "idle".to_owned(),
                name: Some("idle".to_owned()),
                duration_seconds: Some(1.0),
            }],
            provenance: AnimationClipPackProvenance {
                producer: "fixture".to_owned(),
                source_hash: hash.clone(),
                target_hash: hash,
                license: "CC0-1.0".to_owned(),
            },
        };
        assert_eq!(pack.validate(), Ok(()));
        for invalid in ["mixamorig:Hips", "joint.part", "joint/path", "joint[0]"] {
            pack.rig.joints[0].id = invalid.to_owned();
            pack.rig.root_joint_id = invalid.to_owned();
            assert_eq!(pack.validate(), Err(AnimationClipPackError::InvalidRig));
        }
    }

    #[test]
    fn exact_held_sample_requires_a_named_finite_unit_time() {
        let valid = AnimatedMeshPlaybackCommand::Sample {
            clip: "idle".to_owned(),
            normalized_time: 0.0,
        };
        assert_eq!(valid.validate(), Ok(()));
        assert_eq!(
            AnimatedMeshPlaybackCommand::Sample {
                clip: "idle".to_owned(),
                normalized_time: 1.0,
            }
            .validate(),
            Ok(())
        );
        for normalized_time in [f32::NAN, f32::INFINITY, -0.01, 1.01] {
            assert_eq!(
                AnimatedMeshPlaybackCommand::Sample {
                    clip: "idle".to_owned(),
                    normalized_time,
                }
                .validate(),
                Err(AnimatedMeshPlaybackError::InvalidNormalizedTime)
            );
        }
        assert_eq!(
            AnimatedMeshPlaybackCommand::Sample {
                clip: "  ".to_owned(),
                normalized_time: 0.5,
            }
            .validate(),
            Err(AnimatedMeshPlaybackError::EmptyClip)
        );
    }
}
