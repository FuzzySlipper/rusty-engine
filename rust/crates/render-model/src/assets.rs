use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

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

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum MaterialAlphaModeDescriptor {
    #[default]
    Opaque,
    Mask {
        cutoff: f32,
    },
    Blend,
}

impl MaterialAlphaModeDescriptor {
    fn is_opaque(&self) -> bool {
        matches!(self, Self::Opaque)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum VoxelSurfaceAlphaModeDescriptor {
    Opaque,
    Mask { cutoff: f32 },
    Blend,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VoxelAtlasPaddingDescriptor {
    pub left: u16,
    pub right: u16,
    pub bottom: u16,
    pub top: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VoxelAtlasRegionDescriptor {
    pub id: String,
    pub content_min: [u32; 2],
    pub content_extent: [u32; 2],
    pub padding: VoxelAtlasPaddingDescriptor,
    pub inset: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum VoxelSurfaceMappingDescriptor {
    Repeat {
        texture: String,
        texture_version: u32,
        texture_content_hash: String,
        tile_scale_cells: [f32; 2],
        tile_origin_cells: [f32; 2],
    },
    Atlas {
        atlas: String,
        atlas_version: u32,
        atlas_content_hash: String,
        texture: String,
        texture_version: u32,
        texture_content_hash: String,
        region: VoxelAtlasRegionDescriptor,
        tile_scale_cells: [f32; 2],
        tile_origin_cells: [f32; 2],
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VoxelSurfaceDescriptor {
    pub schema_version: u32,
    pub filter: TextureFilter,
    pub wrap: TextureWrap,
    pub alpha_mode: VoxelSurfaceAlphaModeDescriptor,
    pub mapping: VoxelSurfaceMappingDescriptor,
}

impl VoxelSurfaceDescriptor {
    pub fn validate(&self) -> Result<(), VoxelSurfaceDescriptorError> {
        if self.schema_version != 1 {
            return Err(VoxelSurfaceDescriptorError::InvalidSchemaVersion);
        }
        if let VoxelSurfaceAlphaModeDescriptor::Mask { cutoff } = self.alpha_mode {
            if !cutoff.is_finite() || !(0.0..=1.0).contains(&cutoff) {
                return Err(VoxelSurfaceDescriptorError::InvalidAlphaCutoff);
            }
        }
        let (texture, texture_version, texture_hash, scale, origin) = match &self.mapping {
            VoxelSurfaceMappingDescriptor::Repeat {
                texture,
                texture_version,
                texture_content_hash,
                tile_scale_cells,
                tile_origin_cells,
            } => {
                if self.wrap != TextureWrap::Repeat {
                    return Err(VoxelSurfaceDescriptorError::InvalidWrap);
                }
                (
                    texture,
                    *texture_version,
                    texture_content_hash,
                    tile_scale_cells,
                    tile_origin_cells,
                )
            }
            VoxelSurfaceMappingDescriptor::Atlas {
                atlas,
                atlas_version,
                atlas_content_hash,
                texture,
                texture_version,
                texture_content_hash,
                region,
                tile_scale_cells,
                tile_origin_cells,
            } => {
                validate_asset_id(atlas, RenderAssetKind::SpriteAtlas)
                    .map_err(|_| VoxelSurfaceDescriptorError::InvalidAtlasReference)?;
                if *atlas_version == 0 || atlas_content_hash.is_empty() {
                    return Err(VoxelSurfaceDescriptorError::InvalidAtlasProvenance);
                }
                if self.wrap != TextureWrap::Clamp {
                    return Err(VoxelSurfaceDescriptorError::InvalidWrap);
                }
                validate_voxel_region(region, self.filter)?;
                (
                    texture,
                    *texture_version,
                    texture_content_hash,
                    tile_scale_cells,
                    tile_origin_cells,
                )
            }
        };
        validate_asset_id(texture, RenderAssetKind::Texture)
            .map_err(|_| VoxelSurfaceDescriptorError::InvalidTextureReference)?;
        if texture_version == 0 || texture_hash.is_empty() {
            return Err(VoxelSurfaceDescriptorError::InvalidTextureProvenance);
        }
        if scale
            .iter()
            .any(|value| !value.is_finite() || !(1.0 / 256.0..=4_096.0).contains(value))
        {
            return Err(VoxelSurfaceDescriptorError::InvalidTileScale);
        }
        if origin
            .iter()
            .any(|value| !value.is_finite() || value.abs() > 16_777_216.0)
        {
            return Err(VoxelSurfaceDescriptorError::InvalidTileOrigin);
        }
        Ok(())
    }

    pub fn texture(&self) -> &str {
        match &self.mapping {
            VoxelSurfaceMappingDescriptor::Repeat { texture, .. }
            | VoxelSurfaceMappingDescriptor::Atlas { texture, .. } => texture,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoxelSurfaceDescriptorError {
    InvalidSchemaVersion,
    InvalidAlphaCutoff,
    InvalidTextureReference,
    InvalidTextureProvenance,
    InvalidAtlasReference,
    InvalidAtlasProvenance,
    InvalidAtlasRegion,
    InvalidAtlasPadding,
    InvalidWrap,
    InvalidTileScale,
    InvalidTileOrigin,
}

fn validate_voxel_region(
    region: &VoxelAtlasRegionDescriptor,
    filter: TextureFilter,
) -> Result<(), VoxelSurfaceDescriptorError> {
    if region.id.is_empty() || region.content_extent.contains(&0) || region.inset != "halfTexel" {
        return Err(VoxelSurfaceDescriptorError::InvalidAtlasRegion);
    }
    let padding = [
        region.padding.left,
        region.padding.right,
        region.padding.bottom,
        region.padding.top,
    ];
    if padding.into_iter().any(|value| value > 32)
        || (filter == TextureFilter::Linear && padding.contains(&0))
    {
        return Err(VoxelSurfaceDescriptorError::InvalidAtlasPadding);
    }
    Ok(())
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
    #[serde(
        default,
        skip_serializing_if = "MaterialAlphaModeDescriptor::is_opaque"
    )]
    pub alpha_mode: MaterialAlphaModeDescriptor,
    #[serde(default, skip_serializing_if = "is_false")]
    pub double_sided: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voxel_surface: Option<VoxelSurfaceDescriptor>,
}

fn is_false(value: &bool) -> bool {
    !*value
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
        if let MaterialAlphaModeDescriptor::Mask { cutoff } = self.alpha_mode {
            if !cutoff.is_finite() || !(0.0..=1.0).contains(&cutoff) {
                return Err(MaterialDescriptorError::InvalidAlphaCutoff);
            }
        }
        if let Some(surface) = &self.voxel_surface {
            surface
                .validate()
                .map_err(MaterialDescriptorError::InvalidVoxelSurface)?;
            if self.texture.as_deref() != Some(surface.texture()) {
                return Err(MaterialDescriptorError::VoxelTextureMismatch);
            }
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
    InvalidAlphaCutoff,
    InvalidVoxelSurface(VoxelSurfaceDescriptorError),
    VoxelTextureMismatch,
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

pub const MAX_TEXTURE_DIMENSION: u32 = 4_096;
pub const MAX_TEXTURE_TEXELS: u64 = 16_777_216;
pub const MAX_TEXTURE_ENCODED_BYTES: u32 = 16 * 1024 * 1024;
pub const MAX_RETAINED_TEXTURES: usize = 256;
pub const MAX_AGGREGATE_TEXTURE_ENCODED_BYTES: u64 = 128 * 1024 * 1024;
pub const MAX_AGGREGATE_TEXTURE_DECODED_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TextureEncoding {
    PngRgba8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TextureColorSpace {
    Srgb,
    Linear,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum TexturePayloadSource {
    Inline { encoded_bytes: Vec<u8> },
    Resource { resource: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TexturePayloadDescriptor {
    pub encoding: TextureEncoding,
    pub color_space: TextureColorSpace,
    pub content_hash: String,
    pub byte_length: u32,
    pub source: TexturePayloadSource,
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
    /// Omitted legacy descriptors retain their historical metadata-only,
    /// color-fallback meaning. New retained textures use this exact PNG source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<TexturePayloadDescriptor>,
}

impl TextureDescriptor {
    /// Admit exact encoded PNG bytes and derive every retained identity fact.
    ///
    /// The returned descriptor references the caller-owned content-addressed
    /// resource instead of cloning the bytes. Callers remain responsible for
    /// publishing those exact bytes and resolving the resource at the host
    /// boundary.
    pub fn admit_png_rgba8_resource(
        id: String,
        encoded_bytes: &[u8],
        filter: TextureFilter,
        wrap: TextureWrap,
        version: u32,
    ) -> Result<Self, TextureError> {
        let byte_length = u32::try_from(encoded_bytes.len()).map_err(|_| {
            TextureError::EncodedByteQuotaExceeded {
                byte_length: u32::MAX,
            }
        })?;
        if byte_length == 0 || byte_length > MAX_TEXTURE_ENCODED_BYTES {
            return Err(TextureError::EncodedByteQuotaExceeded { byte_length });
        }
        let [width, height] = png_rgba8_dimensions(encoded_bytes)?;
        validate_png_rgba8(encoded_bytes, width, height)?;
        let content_hash = format!("sha256:{:x}", Sha256::digest(encoded_bytes));
        let resource = format!(
            "texture-resource/{}",
            content_hash.strip_prefix("sha256:").expect("owned SHA-256")
        );
        let descriptor = Self {
            id,
            width,
            height,
            filter,
            wrap,
            content_hash: Some(content_hash.clone()),
            version,
            payload: Some(TexturePayloadDescriptor {
                encoding: TextureEncoding::PngRgba8,
                color_space: TextureColorSpace::Srgb,
                content_hash,
                byte_length,
                source: TexturePayloadSource::Resource { resource },
            }),
        };
        descriptor.validate()?;
        Ok(descriptor)
    }

    pub fn validate(&self) -> Result<(), TextureError> {
        validate_asset_id(&self.id, RenderAssetKind::Texture).map_err(TextureError::Asset)?;
        if self.width == 0 || self.height == 0 {
            return Err(TextureError::ZeroDimension {
                width: self.width,
                height: self.height,
            });
        }
        if self.width > MAX_TEXTURE_DIMENSION || self.height > MAX_TEXTURE_DIMENSION {
            return Err(TextureError::DimensionQuotaExceeded {
                width: self.width,
                height: self.height,
            });
        }
        let texels = u64::from(self.width)
            .checked_mul(u64::from(self.height))
            .ok_or(TextureError::TexelQuotaExceeded)?;
        if texels > MAX_TEXTURE_TEXELS {
            return Err(TextureError::TexelQuotaExceeded);
        }
        if self.version == 0 {
            return Err(TextureError::InvalidVersion);
        }
        if self
            .content_hash
            .as_ref()
            .is_some_and(|hash| hash.trim().is_empty())
        {
            return Err(TextureError::EmptyContentHash);
        }
        if let Some(payload) = &self.payload {
            validate_texture_payload(self, payload)?;
        }
        Ok(())
    }
}

fn png_rgba8_dimensions(bytes: &[u8]) -> Result<[u32; 2], TextureError> {
    const SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    if bytes.get(..8) != Some(SIGNATURE)
        || bytes.get(12..16) != Some(b"IHDR")
        || bytes.get(8..12) != Some(&13_u32.to_be_bytes())
    {
        return Err(TextureError::InvalidPng);
    }
    let width = u32::from_be_bytes(
        bytes
            .get(16..20)
            .and_then(|value| value.try_into().ok())
            .ok_or(TextureError::InvalidPng)?,
    );
    let height = u32::from_be_bytes(
        bytes
            .get(20..24)
            .and_then(|value| value.try_into().ok())
            .ok_or(TextureError::InvalidPng)?,
    );
    Ok([width, height])
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextureError {
    Asset(RenderAssetError),
    ZeroDimension { width: u32, height: u32 },
    DimensionQuotaExceeded { width: u32, height: u32 },
    TexelQuotaExceeded,
    InvalidVersion,
    EmptyContentHash,
    EncodedByteQuotaExceeded { byte_length: u32 },
    InvalidContentHash,
    InvalidResourceIdentity,
    InlineByteLengthMismatch,
    InlineContentHashMismatch,
    InvalidPng,
    PngDimensionMismatch,
    UnsupportedPng,
}

fn validate_texture_payload(
    texture: &TextureDescriptor,
    payload: &TexturePayloadDescriptor,
) -> Result<(), TextureError> {
    if payload.byte_length == 0 || payload.byte_length > MAX_TEXTURE_ENCODED_BYTES {
        return Err(TextureError::EncodedByteQuotaExceeded {
            byte_length: payload.byte_length,
        });
    }
    let digest = payload
        .content_hash
        .strip_prefix("sha256:")
        .filter(|value| {
            value.len() == 64
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        })
        .ok_or(TextureError::InvalidContentHash)?;
    if texture.content_hash.as_deref() != Some(payload.content_hash.as_str()) {
        return Err(TextureError::InvalidContentHash);
    }
    match &payload.source {
        TexturePayloadSource::Inline { encoded_bytes } => {
            if encoded_bytes.len() != payload.byte_length as usize {
                return Err(TextureError::InlineByteLengthMismatch);
            }
            let actual = format!("sha256:{:x}", Sha256::digest(encoded_bytes));
            if actual != payload.content_hash {
                return Err(TextureError::InlineContentHashMismatch);
            }
            validate_png_rgba8(encoded_bytes, texture.width, texture.height)?;
        }
        TexturePayloadSource::Resource { resource } => {
            if resource != &format!("texture-resource/{digest}") {
                return Err(TextureError::InvalidResourceIdentity);
            }
        }
    }
    Ok(())
}

fn validate_png_rgba8(bytes: &[u8], width: u32, height: u32) -> Result<(), TextureError> {
    const SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    if bytes.get(..8) != Some(SIGNATURE) {
        return Err(TextureError::InvalidPng);
    }
    let mut offset = 8_usize;
    let mut saw_ihdr = false;
    let mut saw_idat = false;
    let mut saw_iend = false;
    while offset < bytes.len() {
        let length = bytes
            .get(offset..offset + 4)
            .and_then(|value| value.try_into().ok())
            .map(u32::from_be_bytes)
            .ok_or(TextureError::InvalidPng)? as usize;
        let kind = bytes
            .get(offset + 4..offset + 8)
            .ok_or(TextureError::InvalidPng)?;
        let data_start = offset.checked_add(8).ok_or(TextureError::InvalidPng)?;
        let data_end = data_start
            .checked_add(length)
            .ok_or(TextureError::InvalidPng)?;
        let chunk_end = data_end.checked_add(4).ok_or(TextureError::InvalidPng)?;
        if chunk_end > bytes.len() {
            return Err(TextureError::InvalidPng);
        }
        let expected_crc = u32::from_be_bytes(
            bytes[data_end..chunk_end]
                .try_into()
                .map_err(|_| TextureError::InvalidPng)?,
        );
        if png_crc32(&bytes[offset + 4..data_end]) != expected_crc {
            return Err(TextureError::InvalidPng);
        }
        match kind {
            b"IHDR" if !saw_ihdr && offset == 8 && length == 13 => {
                let data = &bytes[data_start..data_end];
                let actual_width = u32::from_be_bytes(data[0..4].try_into().expect("IHDR width"));
                let actual_height = u32::from_be_bytes(data[4..8].try_into().expect("IHDR height"));
                if [actual_width, actual_height] != [width, height] {
                    return Err(TextureError::PngDimensionMismatch);
                }
                if data[8..13] != [8, 6, 0, 0, 0] {
                    return Err(TextureError::UnsupportedPng);
                }
                saw_ihdr = true;
            }
            b"IDAT" if saw_ihdr && !saw_iend => saw_idat = true,
            b"IEND" if saw_ihdr && saw_idat && length == 0 => {
                saw_iend = true;
                if chunk_end != bytes.len() {
                    return Err(TextureError::InvalidPng);
                }
            }
            b"IHDR" | b"IEND" => return Err(TextureError::InvalidPng),
            _ => {}
        }
        offset = chunk_end;
    }
    if saw_ihdr && saw_idat && saw_iend {
        Ok(())
    } else {
        Err(TextureError::InvalidPng)
    }
}

fn png_crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 == 0 {
                crc >> 1
            } else {
                (crc >> 1) ^ 0xedb8_8320
            };
        }
    }
    !crc
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpriteFrameRect {
    pub frame: u32,
    /// Inclusive normalized minimum in decoded PNG image space. The image
    /// origin is its top-left, U increases right, and V increases down.
    pub uv_min: [f32; 2],
    /// Exclusive normalized maximum in the same top-left image space.
    pub uv_max: [f32; 2],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<[f32; 2]>,
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
            if rect.size.is_some_and(|size| {
                size.into_iter()
                    .any(|component| !component.is_finite() || component <= 0.0)
            }) {
                return Err(SpriteAtlasError::InvalidFrameSize { frame: rect.frame });
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
    InvalidFrameSize { frame: u32 },
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
        !matches!(self, Self::Custom)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SpriteLightingMode {
    #[default]
    Unlit,
    AuthoredNormal,
    AuthoredDepth,
    DerivedGradient,
    Synthetic,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum SpriteAlphaMode {
    Opaque,
    Mask {
        cutoff: f32,
    },
    #[default]
    Blend,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SpriteShadowPolicy {
    #[default]
    None,
    Cast,
    Receive,
    CastAndReceive,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpriteMaterialDescriptor {
    pub lighting: SpriteLightingMode,
    pub normal_texture: Option<String>,
    pub depth_texture: Option<String>,
    pub normal_strength: f32,
    pub normal_bias: f32,
    pub alpha: SpriteAlphaMode,
    pub shadow: SpriteShadowPolicy,
}

impl Default for SpriteMaterialDescriptor {
    fn default() -> Self {
        Self {
            lighting: SpriteLightingMode::Unlit,
            normal_texture: None,
            depth_texture: None,
            normal_strength: 1.0,
            normal_bias: 0.0,
            alpha: SpriteAlphaMode::Blend,
            shadow: SpriteShadowPolicy::None,
        }
    }
}

impl SpriteMaterialDescriptor {
    pub fn is_default(&self) -> bool {
        self == &Self::default()
    }

    pub fn validate(&self) -> Result<(), SpriteMaterialError> {
        if !self.normal_strength.is_finite() || !(0.0..=4.0).contains(&self.normal_strength) {
            return Err(SpriteMaterialError::InvalidNormalStrength);
        }
        if !self.normal_bias.is_finite() || !(-1.0..=1.0).contains(&self.normal_bias) {
            return Err(SpriteMaterialError::InvalidNormalBias);
        }
        if let SpriteAlphaMode::Mask { cutoff } = self.alpha {
            if !cutoff.is_finite() || !(0.0..=1.0).contains(&cutoff) {
                return Err(SpriteMaterialError::InvalidAlphaCutoff);
            }
        }
        if let Some(texture) = &self.normal_texture {
            validate_asset_id(texture, RenderAssetKind::Texture)
                .map_err(SpriteMaterialError::Texture)?;
        }
        if let Some(texture) = &self.depth_texture {
            validate_asset_id(texture, RenderAssetKind::Texture)
                .map_err(SpriteMaterialError::Texture)?;
        }
        let references_match = match self.lighting {
            SpriteLightingMode::AuthoredNormal => {
                self.normal_texture.is_some() && self.depth_texture.is_none()
            }
            SpriteLightingMode::AuthoredDepth => {
                self.normal_texture.is_none() && self.depth_texture.is_some()
            }
            SpriteLightingMode::Unlit
            | SpriteLightingMode::DerivedGradient
            | SpriteLightingMode::Synthetic => {
                self.normal_texture.is_none() && self.depth_texture.is_none()
            }
        };
        if !references_match {
            return Err(SpriteMaterialError::TextureModeMismatch);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SpriteMaterialError {
    Texture(RenderAssetError),
    TextureModeMismatch,
    InvalidNormalStrength,
    InvalidNormalBias,
    InvalidAlphaCutoff,
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
    #[serde(default)]
    pub material: SpriteMaterialDescriptor,
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
        self.material.validate().map_err(SpriteError::Material)?;
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
    Material(SpriteMaterialError),
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

    const CHECKER_PNG: &[u8] = &[
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 2, 0, 0, 0, 1, 8, 6,
        0, 0, 0, 244, 34, 127, 138, 0, 0, 0, 15, 73, 68, 65, 84, 120, 156, 99, 248, 207, 0, 68,
        255, 25, 26, 0, 16, 121, 3, 126, 153, 113, 48, 89, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96,
        130,
    ];
    const CHECKER_HASH: &str =
        "sha256:a58d5395a03945e56638dba7ae6158b2fdaf013610a798c059a6d88231a052ae";

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
                    size: None,
                },
                SpriteFrameRect {
                    frame: 0,
                    uv_min: [0.5, 0.0],
                    uv_max: [1.0, 1.0],
                    size: None,
                },
            ],
        };
        assert_eq!(
            atlas.validate(),
            Err(SpriteAtlasError::DuplicateFrame { frame: 0 })
        );
    }

    #[test]
    fn sprite_material_modes_bind_only_their_bounded_texture_roles() {
        let authored = SpriteMaterialDescriptor {
            lighting: SpriteLightingMode::AuthoredNormal,
            normal_texture: Some("texture/sprite-normal".to_string()),
            depth_texture: None,
            normal_strength: 1.5,
            normal_bias: 0.1,
            alpha: SpriteAlphaMode::Mask { cutoff: 0.45 },
            shadow: SpriteShadowPolicy::CastAndReceive,
        };
        assert_eq!(authored.validate(), Ok(()));

        assert_eq!(
            SpriteMaterialDescriptor {
                depth_texture: Some("texture/depth".to_string()),
                ..authored.clone()
            }
            .validate(),
            Err(SpriteMaterialError::TextureModeMismatch)
        );
        assert_eq!(
            SpriteMaterialDescriptor {
                normal_strength: 4.1,
                ..authored
            }
            .validate(),
            Err(SpriteMaterialError::InvalidNormalStrength)
        );
        assert!(SpriteMaterialDescriptor::default().is_default());
    }

    #[test]
    fn atlas_validation_rejects_non_positive_or_non_finite_frame_sizes() {
        for invalid_size in [
            [0.0, 1.0],
            [-1.0, 1.0],
            [f32::NAN, 1.0],
            [f32::INFINITY, 1.0],
            [1.0, f32::NEG_INFINITY],
        ] {
            let atlas = SpriteAtlasDescriptor {
                id: "sprite-sheet/sparks".to_string(),
                texture: "texture/sparks".to_string(),
                frames: vec![SpriteFrameRect {
                    frame: 7,
                    uv_min: [0.0, 0.0],
                    uv_max: [1.0, 1.0],
                    size: Some(invalid_size),
                }],
            };
            assert_eq!(
                atlas.validate(),
                Err(SpriteAtlasError::InvalidFrameSize { frame: 7 })
            );
        }
    }

    #[test]
    fn texture_payload_admits_exact_rgba8_png_and_rejects_hash_or_png_drift() {
        let texture = TextureDescriptor {
            id: "texture/checker".to_string(),
            width: 2,
            height: 1,
            filter: TextureFilter::Nearest,
            wrap: TextureWrap::Repeat,
            content_hash: Some(CHECKER_HASH.to_string()),
            version: 1,
            payload: Some(TexturePayloadDescriptor {
                encoding: TextureEncoding::PngRgba8,
                color_space: TextureColorSpace::Srgb,
                content_hash: CHECKER_HASH.to_string(),
                byte_length: CHECKER_PNG.len() as u32,
                source: TexturePayloadSource::Inline {
                    encoded_bytes: CHECKER_PNG.to_vec(),
                },
            }),
        };
        assert_eq!(texture.validate(), Ok(()));

        let mut hash_drift = texture.clone();
        hash_drift.payload.as_mut().unwrap().content_hash = format!("sha256:{}", "0".repeat(64));
        assert_eq!(hash_drift.validate(), Err(TextureError::InvalidContentHash));

        let mut png_drift = texture;
        if let TexturePayloadSource::Inline { encoded_bytes } =
            &mut png_drift.payload.as_mut().unwrap().source
        {
            encoded_bytes[32] ^= 1;
            let drift_hash = format!("sha256:{:x}", Sha256::digest(encoded_bytes));
            png_drift.content_hash = Some(drift_hash.clone());
            png_drift.payload.as_mut().unwrap().content_hash = drift_hash;
        }
        assert_eq!(png_drift.validate(), Err(TextureError::InvalidPng));
    }

    #[test]
    fn png_resource_admission_derives_dimensions_hash_and_resource_identity() {
        let admitted = TextureDescriptor::admit_png_rgba8_resource(
            "texture/checker".to_string(),
            CHECKER_PNG,
            TextureFilter::Linear,
            TextureWrap::Clamp,
            3,
        )
        .expect("admit checked PNG");
        assert_eq!([admitted.width, admitted.height], [2, 1]);
        assert_eq!(admitted.content_hash.as_deref(), Some(CHECKER_HASH));
        assert_eq!(admitted.version, 3);
        assert!(matches!(
            admitted.payload.as_ref().map(|payload| &payload.source),
            Some(TexturePayloadSource::Resource { resource })
                if resource == &format!("texture-resource/{}", &CHECKER_HASH[7..])
        ));

        let mut unsupported = CHECKER_PNG.to_vec();
        unsupported[24] = 16;
        assert_eq!(
            TextureDescriptor::admit_png_rgba8_resource(
                "texture/checker".to_string(),
                &unsupported,
                TextureFilter::Nearest,
                TextureWrap::Repeat,
                1,
            ),
            Err(TextureError::InvalidPng),
        );
    }

    #[test]
    fn texture_payload_resource_identity_and_bounds_fail_closed() {
        let descriptor = TextureDescriptor {
            id: "texture/checker".to_string(),
            width: MAX_TEXTURE_DIMENSION,
            height: MAX_TEXTURE_DIMENSION,
            filter: TextureFilter::Linear,
            wrap: TextureWrap::Clamp,
            content_hash: Some(CHECKER_HASH.to_string()),
            version: 1,
            payload: Some(TexturePayloadDescriptor {
                encoding: TextureEncoding::PngRgba8,
                color_space: TextureColorSpace::Srgb,
                content_hash: CHECKER_HASH.to_string(),
                byte_length: CHECKER_PNG.len() as u32,
                source: TexturePayloadSource::Resource {
                    resource: format!("texture-resource/{}", &CHECKER_HASH[7..]),
                },
            }),
        };
        assert_eq!(descriptor.validate(), Ok(()));

        let mut wrong_resource = descriptor.clone();
        if let TexturePayloadSource::Resource { resource } =
            &mut wrong_resource.payload.as_mut().unwrap().source
        {
            *resource = "texture-resource/wrong".to_string();
        }
        assert_eq!(
            wrong_resource.validate(),
            Err(TextureError::InvalidResourceIdentity)
        );

        let mut too_wide = descriptor;
        too_wide.width = MAX_TEXTURE_DIMENSION + 1;
        assert_eq!(
            too_wide.validate(),
            Err(TextureError::DimensionQuotaExceeded {
                width: MAX_TEXTURE_DIMENSION + 1,
                height: MAX_TEXTURE_DIMENSION,
            })
        );
    }

    #[test]
    fn voxel_surface_material_border_is_strict_and_legacy_omission_is_exact() {
        let surface = VoxelSurfaceDescriptor {
            schema_version: 1,
            filter: TextureFilter::Linear,
            wrap: TextureWrap::Clamp,
            alpha_mode: VoxelSurfaceAlphaModeDescriptor::Mask { cutoff: 0.5 },
            mapping: VoxelSurfaceMappingDescriptor::Atlas {
                atlas: "sprite-sheet/voxel-surfaces".to_string(),
                atlas_version: 2,
                atlas_content_hash: "bb02".to_string(),
                texture: "texture/voxel-surfaces".to_string(),
                texture_version: 3,
                texture_content_hash: "aa03".to_string(),
                region: VoxelAtlasRegionDescriptor {
                    id: "stone".to_string(),
                    content_min: [2, 2],
                    content_extent: [28, 28],
                    padding: VoxelAtlasPaddingDescriptor {
                        left: 1,
                        right: 1,
                        bottom: 1,
                        top: 1,
                    },
                    inset: "halfTexel".to_string(),
                },
                tile_scale_cells: [1.0, 2.0],
                tile_origin_cells: [-4.0, 8.0],
            },
        };
        assert_eq!(surface.validate(), Ok(()));
        let material = RenderMaterialDescriptor {
            schema_version: 2,
            id: "material/stone".to_string(),
            color: [1.0; 4],
            texture: Some("texture/voxel-surfaces".to_string()),
            roughness: 1.0,
            texture_tint: [1.0; 4],
            emission_color: [0.0; 3],
            emission_intensity: 0.0,
            uv_strategy: MaterialUvStrategy::Atlas,
            alpha_mode: Default::default(),
            double_sided: false,
            voxel_surface: Some(surface.clone()),
        };
        assert_eq!(material.validate(), Ok(()));
        let material_json = serde_json::to_string(&material).unwrap();
        assert!(material_json.contains("\"textureContentHash\":\"aa03\""));
        assert!(material_json.contains("\"tileScaleCells\":[1.0,2.0]"));
        assert!(!material_json.contains("texture_content_hash"));
        assert_eq!(
            serde_json::from_str::<RenderMaterialDescriptor>(&material_json).unwrap(),
            material
        );

        let mut invalid_padding = surface;
        if let VoxelSurfaceMappingDescriptor::Atlas { region, .. } = &mut invalid_padding.mapping {
            region.padding.left = 0;
        }
        assert_eq!(
            invalid_padding.validate(),
            Err(VoxelSurfaceDescriptorError::InvalidAtlasPadding)
        );

        let legacy = RenderMaterialDescriptor {
            voxel_surface: None,
            ..material
        };
        let encoded = serde_json::to_string(&legacy).unwrap();
        assert!(!encoded.contains("voxelSurface"));
        assert_eq!(
            serde_json::from_str::<RenderMaterialDescriptor>(&encoded).unwrap(),
            legacy
        );
    }

    #[test]
    fn generic_material_alpha_and_sidedness_are_explicit_and_bounded() {
        let material = RenderMaterialDescriptor {
            schema_version: 3,
            id: "material/depth-splat".to_string(),
            color: [1.0; 4],
            texture: None,
            roughness: 1.0,
            texture_tint: [1.0; 4],
            emission_color: [0.0; 3],
            emission_intensity: 0.0,
            uv_strategy: MaterialUvStrategy::Flat,
            alpha_mode: MaterialAlphaModeDescriptor::Mask { cutoff: 0.5 },
            double_sided: true,
            voxel_surface: None,
        };
        material.validate().unwrap();
        let encoded = serde_json::to_string(&material).unwrap();
        assert!(encoded.contains("\"alphaMode\":{\"kind\":\"mask\",\"cutoff\":0.5}"));
        assert!(encoded.contains("\"doubleSided\":true"));

        let mut invalid = material;
        invalid.alpha_mode = MaterialAlphaModeDescriptor::Mask { cutoff: 1.5 };
        assert_eq!(
            invalid.validate(),
            Err(MaterialDescriptorError::InvalidAlphaCutoff)
        );
    }
}
