use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{MeshProvenance, RenderHandle, RenderMetadata, Transform, JSON_SAFE_U64_MAX};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RenderAssetKind {
    Material,
    Texture,
    Sprite,
    SpriteAtlas,
    StaticMesh,
    AnimatedMesh,
    VoxelObject,
    Audio,
    Font,
}

impl RenderAssetKind {
    pub const fn accepted_prefixes(self) -> &'static [&'static str] {
        match self {
            Self::Material => &["material/", "voxel-material/"],
            Self::Texture => &["texture/"],
            Self::Sprite => &["sprite/", "sprite-sheet/"],
            Self::SpriteAtlas => &["sprite-sheet/", "sprite/"],
            Self::StaticMesh => &["mesh/"],
            Self::AnimatedMesh => &["mesh-animation/", "animated-mesh/"],
            Self::VoxelObject => &["voxel-object/"],
            Self::Audio => &["audio/"],
            Self::Font => &["font/"],
        }
    }
}

/// Immutable resource information supplied to projection or a renderer host.
/// This is deliberately not a catalog: it cannot enumerate, mutate, resolve a
/// project, or decide gameplay authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResolvedRenderAsset {
    pub id: String,
    pub kind: RenderAssetKind,
    pub content_hash: Option<String>,
    pub version: u32,
}

impl ResolvedRenderAsset {
    pub fn validate(&self) -> Result<(), RenderAssetError> {
        validate_asset_id(&self.id, self.kind)?;
        if self
            .content_hash
            .as_ref()
            .is_some_and(|hash| hash.trim().is_empty())
        {
            return Err(RenderAssetError::EmptyContentHash);
        }
        Ok(())
    }

    pub fn verify_requirement(
        &self,
        requirement: &RenderAssetRequirement,
    ) -> Result<(), RenderAssetError> {
        self.validate()?;
        requirement.validate()?;
        if self.id != requirement.id {
            return Err(RenderAssetError::IdMismatch {
                expected: requirement.id.clone(),
                actual: self.id.clone(),
            });
        }
        if self.kind != requirement.kind {
            return Err(RenderAssetError::WrongKind {
                id: self.id.clone(),
                expected: requirement.kind,
            });
        }
        if requirement
            .content_hash
            .as_ref()
            .is_some_and(|expected| self.content_hash.as_ref() != Some(expected))
        {
            return Err(RenderAssetError::ContentHashMismatch {
                expected: requirement.content_hash.clone().unwrap_or_default(),
                actual: self.content_hash.clone(),
            });
        }
        if self.version < requirement.minimum_version {
            return Err(RenderAssetError::VersionTooOld {
                minimum: requirement.minimum_version,
                actual: self.version,
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RenderAssetRequirement {
    pub id: String,
    pub kind: RenderAssetKind,
    pub content_hash: Option<String>,
    pub minimum_version: u32,
}

impl RenderAssetRequirement {
    pub fn validate(&self) -> Result<(), RenderAssetError> {
        validate_asset_id(&self.id, self.kind)?;
        if self
            .content_hash
            .as_ref()
            .is_some_and(|hash| hash.trim().is_empty())
        {
            return Err(RenderAssetError::EmptyContentHash);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderAssetError {
    EmptyId,
    WrongKind {
        id: String,
        expected: RenderAssetKind,
    },
    EmptyContentHash,
    IdMismatch {
        expected: String,
        actual: String,
    },
    ContentHashMismatch {
        expected: String,
        actual: Option<String>,
    },
    VersionTooOld {
        minimum: u32,
        actual: u32,
    },
}

pub fn validate_asset_id(id: &str, expected: RenderAssetKind) -> Result<(), RenderAssetError> {
    if id.is_empty() {
        return Err(RenderAssetError::EmptyId);
    }
    if !expected
        .accepted_prefixes()
        .iter()
        .any(|prefix| id.starts_with(prefix) && id.len() > prefix.len())
    {
        return Err(RenderAssetError::WrongKind {
            id: id.to_string(),
            expected,
        });
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MaterialUvStrategy {
    #[default]
    Flat,
    Planar,
    Atlas,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RenderMaterialDescriptor {
    pub schema_version: u32,
    pub id: String,
    pub color: [f32; 4],
    pub texture: Option<String>,
    pub roughness: f32,
    pub texture_tint: [f32; 4],
    pub emission_color: [f32; 3],
    pub emission_intensity: f32,
    pub uv_strategy: MaterialUvStrategy,
}

impl RenderMaterialDescriptor {
    pub fn validate(&self) -> Result<(), MaterialDescriptorError> {
        validate_asset_id(&self.id, RenderAssetKind::Material)
            .map_err(MaterialDescriptorError::Asset)?;
        if self.schema_version == 0 {
            return Err(MaterialDescriptorError::InvalidSchemaVersion);
        }
        if self
            .texture
            .as_ref()
            .is_some_and(|id| validate_asset_id(id, RenderAssetKind::Texture).is_err())
        {
            return Err(MaterialDescriptorError::InvalidTextureReference);
        }
        if !valid_color(self.color) || !valid_color(self.texture_tint) {
            return Err(MaterialDescriptorError::InvalidColor);
        }
        if !self.roughness.is_finite() || !(0.0..=1.0).contains(&self.roughness) {
            return Err(MaterialDescriptorError::InvalidRoughness);
        }
        if !valid_color(self.emission_color)
            || !self.emission_intensity.is_finite()
            || self.emission_intensity < 0.0
        {
            return Err(MaterialDescriptorError::InvalidEmission);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaterialDescriptorError {
    Asset(RenderAssetError),
    InvalidSchemaVersion,
    InvalidTextureReference,
    InvalidColor,
    InvalidRoughness,
    InvalidEmission,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MaterialInstanceParameters {
    pub texture_tint: [f32; 4],
    pub emission_color: [f32; 3],
    pub emission_intensity: f32,
}

impl MaterialInstanceParameters {
    pub fn validate(self) -> Result<(), MaterialParametersError> {
        if !valid_color(self.texture_tint) || !valid_color(self.emission_color) {
            return Err(MaterialParametersError::InvalidColor);
        }
        if !self.emission_intensity.is_finite() || self.emission_intensity < 0.0 {
            return Err(MaterialParametersError::InvalidEmissionIntensity);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaterialParametersError {
    InvalidColor,
    InvalidEmissionIntensity,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TextureFilter {
    #[default]
    Nearest,
    Linear,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TextureWrap {
    #[default]
    Clamp,
    Repeat,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TextureDescriptor {
    pub id: String,
    pub width: u32,
    pub height: u32,
    pub filter: TextureFilter,
    pub wrap: TextureWrap,
    pub content_hash: Option<String>,
    pub version: u32,
}

impl TextureDescriptor {
    pub fn validate(&self) -> Result<(), TextureError> {
        validate_asset_id(&self.id, RenderAssetKind::Texture).map_err(TextureError::Asset)?;
        if self.width == 0 || self.height == 0 {
            return Err(TextureError::ZeroDimension {
                width: self.width,
                height: self.height,
            });
        }
        if self
            .content_hash
            .as_ref()
            .is_some_and(|hash| hash.trim().is_empty())
        {
            return Err(TextureError::EmptyContentHash);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextureError {
    Asset(RenderAssetError),
    ZeroDimension { width: u32, height: u32 },
    EmptyContentHash,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpriteFrameRect {
    pub frame: u32,
    pub uv_min: [f32; 2],
    pub uv_max: [f32; 2],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpriteAtlasDescriptor {
    pub id: String,
    pub texture: String,
    pub frames: Vec<SpriteFrameRect>,
}

impl SpriteAtlasDescriptor {
    pub fn validate(&self) -> Result<(), SpriteAtlasError> {
        validate_asset_id(&self.id, RenderAssetKind::SpriteAtlas)
            .map_err(SpriteAtlasError::Asset)?;
        validate_asset_id(&self.texture, RenderAssetKind::Texture)
            .map_err(|_| SpriteAtlasError::InvalidTextureReference)?;
        if self.frames.is_empty() {
            return Err(SpriteAtlasError::NoFrames);
        }
        let mut seen = BTreeSet::new();
        for rect in &self.frames {
            if !seen.insert(rect.frame) {
                return Err(SpriteAtlasError::DuplicateFrame { frame: rect.frame });
            }
            if !rect
                .uv_min
                .iter()
                .chain(rect.uv_max.iter())
                .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
            {
                return Err(SpriteAtlasError::UvOutOfRange { frame: rect.frame });
            }
            if rect.uv_max[0] <= rect.uv_min[0] || rect.uv_max[1] <= rect.uv_min[1] {
                return Err(SpriteAtlasError::DegenerateRect { frame: rect.frame });
            }
        }
        Ok(())
    }

    pub fn frame_rect(&self, frame: u32) -> Option<&SpriteFrameRect> {
        self.frames.iter().find(|rect| rect.frame == frame)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpriteAtlasError {
    Asset(RenderAssetError),
    InvalidTextureReference,
    NoFrames,
    DuplicateFrame { frame: u32 },
    UvOutOfRange { frame: u32 },
    DegenerateRect { frame: u32 },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SpriteSizeMode {
    #[default]
    World,
    Pixel,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BillboardMode {
    None,
    #[default]
    Spherical,
    Cylindrical,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SpriteDepthPolicy {
    #[default]
    Default,
    DepthTestOff,
    DepthWriteOff,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SpriteShading {
    #[default]
    Unlit,
    Lit,
    Shadowed,
    Custom,
}

impl SpriteShading {
    pub const fn is_implemented(self) -> bool {
        matches!(self, Self::Unlit)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpriteAttachment {
    pub source_entity: Option<u64>,
    pub source_scene_node: Option<u64>,
    pub attachment_point: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpriteInstanceDescriptor {
    pub asset: String,
    pub frame: u32,
    pub pivot: [f32; 2],
    pub size: [f32; 2],
    pub size_mode: SpriteSizeMode,
    pub billboard: BillboardMode,
    pub tint: [f32; 4],
    pub render_order: i32,
    pub depth: SpriteDepthPolicy,
    pub shading: SpriteShading,
    pub visible: bool,
    pub transform: Transform,
    pub attachment: SpriteAttachment,
    pub metadata: RenderMetadata,
}

impl SpriteInstanceDescriptor {
    pub fn validate(&self) -> Result<(), SpriteError> {
        validate_asset_id(&self.asset, RenderAssetKind::Sprite).map_err(SpriteError::Asset)?;
        if !self
            .pivot
            .iter()
            .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
        {
            return Err(SpriteError::PivotOutOfRange { pivot: self.pivot });
        }
        if !self
            .size
            .iter()
            .all(|value| value.is_finite() && *value > 0.0)
        {
            return Err(SpriteError::NonPositiveSize { size: self.size });
        }
        if !valid_color(self.tint) {
            return Err(SpriteError::InvalidTint);
        }
        self.transform.validate().map_err(SpriteError::Transform)?;
        self.metadata.validate().map_err(SpriteError::Metadata)?;
        if self
            .attachment
            .source_entity
            .into_iter()
            .chain(self.attachment.source_scene_node)
            .any(|value| value > JSON_SAFE_U64_MAX)
        {
            return Err(SpriteError::UnsafeSourceIdentity);
        }
        if self
            .attachment
            .attachment_point
            .as_ref()
            .is_some_and(|value| value.trim().is_empty())
        {
            return Err(SpriteError::EmptyAttachmentPoint);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SpriteError {
    Asset(RenderAssetError),
    PivotOutOfRange { pivot: [f32; 2] },
    NonPositiveSize { size: [f32; 2] },
    InvalidTint,
    Transform(crate::TransformError),
    Metadata(crate::NodeError),
    EmptyAttachmentPoint,
    UnsafeSourceIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpritePickHit {
    pub handle: RenderHandle,
    pub source_entity: Option<u64>,
    pub source_scene_node: Option<u64>,
    pub asset: String,
    pub attachment_point: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MeshPickHit {
    pub handle: RenderHandle,
    pub provenance: MeshProvenance,
    pub source_entity: Option<u64>,
    pub source_scene_node: Option<u64>,
}

fn valid_color<const N: usize>(values: [f32; N]) -> bool {
    values
        .iter()
        .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn narrow_asset_view_checks_kind_without_becoming_a_catalog() {
        let asset = ResolvedRenderAsset {
            id: "mesh/crate".to_string(),
            kind: RenderAssetKind::StaticMesh,
            content_hash: Some("abc123".to_string()),
            version: 4,
        };
        assert_eq!(asset.validate(), Ok(()));
        assert!(matches!(
            ResolvedRenderAsset {
                kind: RenderAssetKind::Texture,
                ..asset.clone()
            }
            .validate(),
            Err(RenderAssetError::WrongKind { .. })
        ));

        let requirement = RenderAssetRequirement {
            id: "mesh/crate".to_string(),
            kind: RenderAssetKind::StaticMesh,
            content_hash: Some("different".to_string()),
            minimum_version: 3,
        };
        assert!(matches!(
            asset.verify_requirement(&requirement),
            Err(RenderAssetError::ContentHashMismatch { .. })
        ));
    }

    #[test]
    fn atlas_validation_rejects_duplicate_frames() {
        let atlas = SpriteAtlasDescriptor {
            id: "sprite-sheet/sparks".to_string(),
            texture: "texture/sparks".to_string(),
            frames: vec![
                SpriteFrameRect {
                    frame: 0,
                    uv_min: [0.0, 0.0],
                    uv_max: [0.5, 1.0],
                },
                SpriteFrameRect {
                    frame: 0,
                    uv_min: [0.5, 0.0],
                    uv_max: [1.0, 1.0],
                },
            ],
        };
        assert_eq!(
            atlas.validate(),
            Err(SpriteAtlasError::DuplicateFrame { frame: 0 })
        );
    }
}
