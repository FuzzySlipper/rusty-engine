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
        indices: Vec<u32>,
    },
    /// Shared bytes are resolved through the renderer resource provider. This
    /// handle is scoped to that provider and is not a general runtime bridge.
    SharedBuffer {
        buffer: u64,
        positions_byte_offset: u32,
        normals_byte_offset: u32,
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
        if let MeshPayloadSource::Inline {
            positions,
            normals,
            indices,
        } = &self.source
        {
            if !positions
                .iter()
                .chain(normals)
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
        } else if let MeshPayloadSource::SharedBuffer { buffer, .. } = &self.source {
            if *buffer > JSON_SAFE_U64_MAX {
                return Err(MeshDescriptorError::UnsafeSharedBufferId { buffer: *buffer });
            }
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
    UnsafeSharedBufferId {
        buffer: u64,
    },
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnimatedMeshAsset {
    pub asset: String,
    pub runtime_format: AnimatedMeshRuntimeFormat,
    pub content_hash: Option<String>,
    pub clips: Vec<AnimationClipDescriptor>,
    pub default_clip: Option<String>,
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
        if self
            .default_clip
            .as_ref()
            .is_some_and(|clip| !ids.contains(clip.as_str()))
        {
            return Err(AnimatedMeshAssetError::DefaultClipMissing {
                clip: self.default_clip.clone().unwrap_or_default(),
            });
        }
        validate_slots(&self.material_slots).map_err(AnimatedMeshAssetError::MaterialSlot)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnimatedMeshAssetError {
    Asset(RenderAssetError),
    Bounds(MeshDescriptorError),
    EmptyContentHash,
    EmptyClipId,
    DuplicateClipId { clip: String },
    InvalidClipDuration { clip: String },
    DefaultClipMissing { clip: String },
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
            indices_byte_offset: 72,
        };
        assert!(matches!(
            payload.validate(),
            Err(MeshDescriptorError::UnsafeSharedBufferId { .. })
        ));
    }
}
