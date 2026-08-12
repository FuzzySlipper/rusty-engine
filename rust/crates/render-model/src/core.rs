use serde::{Deserialize, Serialize};

use crate::{
    AnimatedMeshAsset, AnimatedMeshPlaybackCommand, LightDescriptor, MaterialInstanceParameters,
    MeshPayloadDescriptor, RenderMaterialDescriptor, SpriteAtlasDescriptor,
    SpriteInstanceDescriptor, StaticMeshAsset, StaticMeshInstanceDescriptor, TextureDescriptor,
    VoxelObjectInstanceDescriptor, VoxelObjectRenderAsset,
};

pub const RENDER_FRAME_SCHEMA_VERSION: u32 = 1;
pub const JSON_SAFE_U64_MAX: u64 = (1_u64 << 53) - 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RenderHandle(u64);

impl RenderHandle {
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }

    pub const fn validate(self) -> Result<(), RenderHandleError> {
        if self.0 <= JSON_SAFE_U64_MAX {
            Ok(())
        } else {
            Err(RenderHandleError::OutsideJsonSafeRange(self.0))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderHandleError {
    OutsideJsonSafeRange(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Transform {
    pub translation: [f32; 3],
    /// Quaternion in `[x, y, z, w]` order.
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
}

impl Transform {
    pub const IDENTITY: Self = Self {
        translation: [0.0, 0.0, 0.0],
        rotation: [0.0, 0.0, 0.0, 1.0],
        scale: [1.0, 1.0, 1.0],
    };

    pub fn validate(self) -> Result<(), TransformError> {
        if !self.translation.iter().all(|value| value.is_finite()) {
            return Err(TransformError::InvalidTranslation);
        }
        if !self.rotation.iter().all(|value| value.is_finite()) {
            return Err(TransformError::InvalidRotation);
        }
        let rotation_length = self.rotation.iter().map(|value| value * value).sum::<f32>();
        if rotation_length <= f32::EPSILON {
            return Err(TransformError::InvalidRotation);
        }
        if !self.scale.iter().all(|value| value.is_finite()) {
            return Err(TransformError::InvalidScale);
        }
        Ok(())
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformError {
    InvalidTranslation,
    InvalidRotation,
    InvalidScale,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Geometry {
    /// Transform-only hierarchy node with no drawable geometry.
    Group,
    Cube,
    Sphere,
    Quad,
    Point,
    Line {
        a: [f32; 3],
        b: [f32; 3],
    },
}

impl Geometry {
    fn validate(self) -> Result<(), NodeError> {
        match self {
            Self::Line { a, b }
                if !a
                    .into_iter()
                    .chain(b)
                    .all(|component| component.is_finite()) =>
            {
                Err(NodeError::InvalidGeometry)
            }
            _ => Ok(()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Material {
    pub color: [f32; 4],
    pub wireframe: bool,
}

impl Material {
    pub const DEFAULT: Self = Self {
        color: [1.0, 1.0, 1.0, 1.0],
        wireframe: false,
    };

    fn validate(self) -> Result<(), NodeError> {
        self.color
            .iter()
            .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
            .then_some(())
            .ok_or(NodeError::InvalidMaterial)
    }
}

impl Default for Material {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RenderLayer {
    #[default]
    Scene,
    Debug,
    Ui,
    Viewmodel,
}

/// Authority provenance remains raw identity data at this border. The renderer
/// may report it in a pick, but cannot turn it into gameplay authority.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RenderMetadata {
    pub source_entity: Option<u64>,
    pub source_scene_node: Option<u64>,
    pub tags: Vec<String>,
    pub label: Option<String>,
}

impl RenderMetadata {
    pub fn validate(&self) -> Result<(), NodeError> {
        if self
            .source_entity
            .into_iter()
            .chain(self.source_scene_node)
            .any(|value| value > JSON_SAFE_U64_MAX)
        {
            return Err(NodeError::UnsafeSourceIdentity);
        }
        if self.tags.iter().any(|tag| tag.trim().is_empty()) {
            return Err(NodeError::EmptyTag);
        }
        if self.tags.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(NodeError::TagsNotCanonical);
        }
        if self
            .label
            .as_ref()
            .is_some_and(|value| value.trim().is_empty())
        {
            return Err(NodeError::EmptyLabel);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RenderNode {
    pub geometry: Geometry,
    pub material: Material,
    pub transform: Transform,
    pub visible: bool,
    pub layer: RenderLayer,
    pub metadata: RenderMetadata,
}

impl RenderNode {
    pub fn new(geometry: Geometry) -> Self {
        Self {
            geometry,
            material: Material::DEFAULT,
            transform: Transform::IDENTITY,
            visible: true,
            layer: RenderLayer::Scene,
            metadata: RenderMetadata::default(),
        }
    }

    pub fn validate(&self) -> Result<(), NodeError> {
        self.geometry.validate()?;
        self.material.validate()?;
        self.transform.validate().map_err(NodeError::Transform)?;
        self.metadata.validate()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeError {
    InvalidGeometry,
    InvalidMaterial,
    Transform(TransformError),
    EmptyTag,
    TagsNotCanonical,
    EmptyLabel,
    UnsafeSourceIdentity,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "op",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum RenderDiff {
    Create {
        handle: RenderHandle,
        parent: Option<RenderHandle>,
        node: RenderNode,
    },
    Update {
        handle: RenderHandle,
        transform: Option<Transform>,
        material: Option<Material>,
        visible: Option<bool>,
        metadata: Option<RenderMetadata>,
    },
    Destroy {
        handle: RenderHandle,
    },
    ReplaceMeshPayload {
        handle: RenderHandle,
        payload: MeshPayloadDescriptor,
    },
    CreateLight {
        handle: RenderHandle,
        parent: Option<RenderHandle>,
        light: LightDescriptor,
    },
    UpdateLight {
        handle: RenderHandle,
        light: LightDescriptor,
    },
    DefineMaterial {
        material: RenderMaterialDescriptor,
    },
    SetMaterialInstanceParameters {
        handle: RenderHandle,
        slot: u16,
        parameters: Option<MaterialInstanceParameters>,
    },
    DefineTexture {
        texture: TextureDescriptor,
    },
    DefineSpriteAtlas {
        atlas: SpriteAtlasDescriptor,
    },
    DefineStaticMesh {
        asset: StaticMeshAsset,
    },
    DefineAnimatedMesh {
        asset: AnimatedMeshAsset,
    },
    DefineVoxelObject {
        asset: VoxelObjectRenderAsset,
    },
    ReleaseVoxelObject {
        asset: String,
    },
    CreateStaticMeshInstance {
        handle: RenderHandle,
        parent: Option<RenderHandle>,
        instance: StaticMeshInstanceDescriptor,
    },
    CreateAnimatedMeshInstance {
        handle: RenderHandle,
        parent: Option<RenderHandle>,
        instance: crate::AnimatedMeshInstanceDescriptor,
    },
    SetAnimatedMeshPlayback {
        handle: RenderHandle,
        playback: AnimatedMeshPlaybackCommand,
    },
    CreateVoxelObjectInstance {
        handle: RenderHandle,
        parent: Option<RenderHandle>,
        instance: VoxelObjectInstanceDescriptor,
    },
    SetVoxelObjectFrame {
        handle: RenderHandle,
        frame: u32,
    },
    CreateSprite {
        handle: RenderHandle,
        parent: Option<RenderHandle>,
        sprite: SpriteInstanceDescriptor,
    },
    UpdateSprite {
        handle: RenderHandle,
        frame: Option<u32>,
        tint: Option<[f32; 4]>,
        render_order: Option<i32>,
        visible: Option<bool>,
    },
}

impl RenderDiff {
    pub fn validate(&self) -> Result<(), RenderOperationError> {
        self.validate_handles()
            .map_err(RenderOperationError::Handle)?;
        match self {
            Self::Create { node, .. } => node.validate().map_err(RenderOperationError::Node),
            Self::Update {
                transform,
                material,
                metadata,
                ..
            } => {
                if let Some(value) = transform {
                    value.validate().map_err(RenderOperationError::Transform)?;
                }
                if let Some(value) = material {
                    value.validate().map_err(RenderOperationError::Node)?;
                }
                if let Some(value) = metadata {
                    value.validate().map_err(RenderOperationError::Node)?;
                }
                Ok(())
            }
            Self::Destroy { .. }
            | Self::SetMaterialInstanceParameters {
                parameters: None, ..
            } => Ok(()),
            Self::ReplaceMeshPayload { payload, .. } => {
                payload.validate().map_err(RenderOperationError::Mesh)
            }
            Self::CreateLight { light, .. } | Self::UpdateLight { light, .. } => {
                light.validate().map_err(RenderOperationError::Light)
            }
            Self::DefineMaterial { material } => material
                .validate()
                .map_err(RenderOperationError::MaterialDescriptor),
            Self::SetMaterialInstanceParameters {
                parameters: Some(parameters),
                ..
            } => parameters
                .validate()
                .map_err(RenderOperationError::MaterialParameters),
            Self::DefineTexture { texture } => {
                texture.validate().map_err(RenderOperationError::Texture)
            }
            Self::DefineSpriteAtlas { atlas } => {
                atlas.validate().map_err(RenderOperationError::SpriteAtlas)
            }
            Self::DefineStaticMesh { asset } => {
                asset.validate().map_err(RenderOperationError::StaticMesh)
            }
            Self::DefineAnimatedMesh { asset } => {
                asset.validate().map_err(RenderOperationError::AnimatedMesh)
            }
            Self::DefineVoxelObject { asset } => {
                asset.validate().map_err(RenderOperationError::VoxelObject)
            }
            Self::ReleaseVoxelObject { asset } => {
                crate::validate_asset_id(asset, crate::RenderAssetKind::VoxelObject)
                    .map_err(RenderOperationError::Asset)
            }
            Self::CreateStaticMeshInstance { instance, .. } => instance
                .validate()
                .map_err(RenderOperationError::StaticMeshInstance),
            Self::CreateAnimatedMeshInstance { instance, .. } => instance
                .validate()
                .map_err(RenderOperationError::AnimatedMeshInstance),
            Self::SetAnimatedMeshPlayback { playback, .. } => playback
                .validate()
                .map_err(RenderOperationError::AnimatedPlayback),
            Self::CreateVoxelObjectInstance { instance, .. } => instance
                .validate()
                .map_err(RenderOperationError::VoxelObjectInstance),
            Self::SetVoxelObjectFrame { .. } => Ok(()),
            Self::CreateSprite { sprite, .. } => {
                sprite.validate().map_err(RenderOperationError::Sprite)
            }
            Self::UpdateSprite { tint, .. } => {
                if tint.is_some_and(|color| !valid_color(color)) {
                    return Err(RenderOperationError::InvalidSpriteTint);
                }
                Ok(())
            }
        }
    }

    fn validate_handles(&self) -> Result<(), RenderHandleError> {
        match self {
            Self::Create { handle, parent, .. }
            | Self::CreateLight { handle, parent, .. }
            | Self::CreateStaticMeshInstance { handle, parent, .. }
            | Self::CreateAnimatedMeshInstance { handle, parent, .. }
            | Self::CreateVoxelObjectInstance { handle, parent, .. }
            | Self::CreateSprite { handle, parent, .. } => {
                handle.validate()?;
                if let Some(parent) = parent {
                    parent.validate()?;
                }
            }
            Self::Update { handle, .. }
            | Self::Destroy { handle }
            | Self::ReplaceMeshPayload { handle, .. }
            | Self::UpdateLight { handle, .. }
            | Self::SetMaterialInstanceParameters { handle, .. }
            | Self::SetAnimatedMeshPlayback { handle, .. }
            | Self::SetVoxelObjectFrame { handle, .. }
            | Self::UpdateSprite { handle, .. } => handle.validate()?,
            Self::DefineMaterial { .. }
            | Self::DefineTexture { .. }
            | Self::DefineSpriteAtlas { .. }
            | Self::DefineStaticMesh { .. }
            | Self::DefineAnimatedMesh { .. }
            | Self::DefineVoxelObject { .. }
            | Self::ReleaseVoxelObject { .. } => {}
        }
        Ok(())
    }
}

fn valid_color<const N: usize>(color: [f32; N]) -> bool {
    color
        .iter()
        .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
}

#[derive(Debug, Clone, PartialEq)]
pub enum RenderOperationError {
    Handle(RenderHandleError),
    Node(NodeError),
    Transform(TransformError),
    Mesh(crate::MeshDescriptorError),
    Light(crate::LightDescriptorError),
    MaterialDescriptor(crate::MaterialDescriptorError),
    MaterialParameters(crate::MaterialParametersError),
    Texture(crate::TextureError),
    SpriteAtlas(crate::SpriteAtlasError),
    StaticMesh(crate::StaticMeshError),
    StaticMeshInstance(crate::StaticMeshInstanceError),
    AnimatedMesh(crate::AnimatedMeshAssetError),
    AnimatedMeshInstance(crate::AnimatedMeshInstanceError),
    AnimatedPlayback(crate::AnimatedMeshPlaybackError),
    VoxelObject(crate::VoxelObjectRenderAssetError),
    VoxelObjectInstance(crate::VoxelObjectInstanceError),
    Asset(crate::RenderAssetError),
    Sprite(crate::SpriteError),
    InvalidSpriteTint,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RenderFrameDiff {
    pub schema_version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publication: Option<RenderFramePublication>,
    pub ops: Vec<RenderDiff>,
}

/// Optional monotonic publication identity for one independently ordered
/// retained stream. The operation count makes a clipped chunk update reject
/// before any renderer state changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RenderFramePublication {
    pub stream: String,
    pub base_revision: u64,
    pub revision: u64,
    pub operation_count: u32,
}

impl Default for RenderFrameDiff {
    fn default() -> Self {
        Self {
            schema_version: RENDER_FRAME_SCHEMA_VERSION,
            publication: None,
            ops: Vec::new(),
        }
    }
}

impl RenderFrameDiff {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn try_from_ops(ops: Vec<RenderDiff>) -> Result<Self, RenderFrameError> {
        let frame = Self {
            schema_version: RENDER_FRAME_SCHEMA_VERSION,
            publication: None,
            ops,
        };
        frame.validate()?;
        Ok(frame)
    }

    pub fn try_from_published_ops(
        stream: impl Into<String>,
        base_revision: u64,
        revision: u64,
        ops: Vec<RenderDiff>,
    ) -> Result<Self, RenderFrameError> {
        let operation_count =
            u32::try_from(ops.len()).map_err(|_| RenderFrameError::PublicationOperationCount {
                expected: u32::MAX,
                actual: ops.len(),
            })?;
        let frame = Self {
            schema_version: RENDER_FRAME_SCHEMA_VERSION,
            publication: Some(RenderFramePublication {
                stream: stream.into(),
                base_revision,
                revision,
                operation_count,
            }),
            ops,
        };
        frame.validate()?;
        Ok(frame)
    }

    pub fn validate(&self) -> Result<(), RenderFrameError> {
        if self.schema_version != RENDER_FRAME_SCHEMA_VERSION {
            return Err(RenderFrameError::UnsupportedSchemaVersion {
                expected: RENDER_FRAME_SCHEMA_VERSION,
                actual: self.schema_version,
            });
        }
        if let Some(publication) = &self.publication {
            if publication.stream.trim().is_empty() || publication.stream.len() > 256 {
                return Err(RenderFrameError::InvalidPublicationStream);
            }
            if publication.base_revision > JSON_SAFE_U64_MAX {
                return Err(RenderFrameError::PublicationRevisionNotJsonSafe {
                    revision: publication.base_revision,
                });
            }
            if publication.revision > JSON_SAFE_U64_MAX {
                return Err(RenderFrameError::PublicationRevisionNotJsonSafe {
                    revision: publication.revision,
                });
            }
            if publication.base_revision.checked_add(1) != Some(publication.revision) {
                return Err(RenderFrameError::InvalidPublicationRevisionStep {
                    base_revision: publication.base_revision,
                    revision: publication.revision,
                });
            }
            if publication.operation_count as usize != self.ops.len() {
                return Err(RenderFrameError::PublicationOperationCount {
                    expected: publication.operation_count,
                    actual: self.ops.len(),
                });
            }
        }
        for (index, operation) in self.ops.iter().enumerate() {
            operation
                .validate()
                .map_err(|source| RenderFrameError::Operation { index, source })?;
        }
        Ok(())
    }

    pub fn push(&mut self, operation: RenderDiff) {
        self.ops.push(operation);
    }

    pub fn len(&self) -> usize {
        self.ops.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    pub fn encode_json(&self) -> Result<String, RenderJsonError> {
        self.validate().map_err(RenderJsonError::InvalidFrame)?;
        serde_json::to_string_pretty(self).map_err(RenderJsonError::Encode)
    }

    pub fn decode_json(input: &str) -> Result<Self, RenderJsonError> {
        let frame: Self = serde_json::from_str(input).map_err(RenderJsonError::Decode)?;
        frame.validate().map_err(RenderJsonError::InvalidFrame)?;
        Ok(frame)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum RenderFrameError {
    UnsupportedSchemaVersion {
        expected: u32,
        actual: u32,
    },
    Operation {
        index: usize,
        source: RenderOperationError,
    },
    InvalidPublicationStream,
    PublicationRevisionNotJsonSafe {
        revision: u64,
    },
    InvalidPublicationRevisionStep {
        base_revision: u64,
        revision: u64,
    },
    PublicationOperationCount {
        expected: u32,
        actual: usize,
    },
}

#[derive(Debug)]
pub enum RenderJsonError {
    Decode(serde_json::Error),
    InvalidFrame(RenderFrameError),
    Encode(serde_json::Error),
}

impl std::fmt::Display for RenderJsonError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for RenderJsonError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versioned_json_round_trips_the_retained_core() {
        let frame = RenderFrameDiff::try_from_ops(vec![RenderDiff::Create {
            handle: RenderHandle::new(7),
            parent: None,
            node: RenderNode::new(Geometry::Cube),
        }])
        .unwrap();
        let json = frame.encode_json().unwrap();
        assert!(json.contains("\"schemaVersion\": 1"));
        assert_eq!(RenderFrameDiff::decode_json(&json).unwrap(), frame);
    }

    #[test]
    fn published_frame_requires_one_exact_revision_step() {
        let frame = RenderFrameDiff::try_from_published_ops("voxel:test", 4, 5, Vec::new())
            .expect("one exact step");
        assert_eq!(frame.publication.unwrap().base_revision, 4);
        assert!(matches!(
            RenderFrameDiff::try_from_published_ops("voxel:test", 4, 6, Vec::new()),
            Err(RenderFrameError::InvalidPublicationRevisionStep {
                base_revision: 4,
                revision: 6,
            })
        ));
    }

    #[test]
    fn camera_relative_viewmodel_layer_round_trips_without_backend_vocabulary() {
        let mut node = RenderNode::new(Geometry::Group);
        node.layer = RenderLayer::Viewmodel;
        let frame = RenderFrameDiff::try_from_ops(vec![RenderDiff::Create {
            handle: RenderHandle::new(8),
            parent: None,
            node,
        }])
        .unwrap();

        let json = frame.encode_json().unwrap();
        assert!(json.contains("\"layer\": \"viewmodel\""));
        assert_eq!(RenderFrameDiff::decode_json(&json).unwrap(), frame);
    }

    #[test]
    fn invalid_operation_rejects_the_whole_frame() {
        let invalid = RenderFrameDiff {
            schema_version: RENDER_FRAME_SCHEMA_VERSION,
            publication: None,
            ops: vec![RenderDiff::Create {
                handle: RenderHandle::new(1),
                parent: None,
                node: RenderNode {
                    material: Material {
                        color: [2.0, 0.0, 0.0, 1.0],
                        wireframe: false,
                    },
                    ..RenderNode::new(Geometry::Cube)
                },
            }],
        };
        assert!(matches!(
            invalid.validate(),
            Err(RenderFrameError::Operation { index: 0, .. })
        ));
    }

    #[test]
    fn frame_rejects_handles_that_javascript_cannot_represent_exactly() {
        let invalid = RenderFrameDiff {
            schema_version: RENDER_FRAME_SCHEMA_VERSION,
            publication: None,
            ops: vec![RenderDiff::Create {
                handle: RenderHandle::new(JSON_SAFE_U64_MAX + 1),
                parent: None,
                node: RenderNode::new(Geometry::Cube),
            }],
        };
        assert!(matches!(
            invalid.validate(),
            Err(RenderFrameError::Operation {
                index: 0,
                source: RenderOperationError::Handle(RenderHandleError::OutsideJsonSafeRange(_))
            })
        ));
    }
}
