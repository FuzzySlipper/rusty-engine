use std::collections::{BTreeMap, BTreeSet};

use core_assets::{AssetId, AssetKind};
use serde::{Deserialize, Serialize};
use voxel_asset::VoxelAssetMaterialMapping;

use crate::{
    voxelize::MaterialEvidence, ConversionError, ConversionPlanSettings, ImportedMeshSource,
    ImportedStaticMesh, MeshSourceTextureCoordinates,
};

pub const MAX_CONVERSION_TEXTURE_TEXELS: usize = 4_194_304;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextureColorSpace {
    Linear,
    Srgb,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextureChannelLayout {
    PaletteIndexU16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextureSamplingPolicy {
    NearestTexel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextureWrapPolicy {
    ClampToEdge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextureMaterialMode {
    SamplePaletteIndex,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TextureSourceRef {
    pub texture_asset_id: String,
    pub asset_version: u64,
    pub content_hash: String,
    pub width: u32,
    pub height: u32,
    pub color_space: TextureColorSpace,
    pub channel_layout: TextureChannelLayout,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TextureSampleAsset {
    pub texture: TextureSourceRef,
    pub texel_materials: Vec<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TextureUvAttributeRef {
    pub attribute_name: String,
    pub source_hash: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TextureMaterialBinding {
    pub source_material_slot: u32,
    pub texture: TextureSourceRef,
    pub uv_attribute: TextureUvAttributeRef,
    pub sample_uv: [f64; 2],
    pub sampling_policy: TextureSamplingPolicy,
    pub wrap_policy: TextureWrapPolicy,
    pub material_mode: TextureMaterialMode,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ConversionMaterialPolicy {
    pub texture_assets: Vec<TextureSampleAsset>,
    pub texture_bindings: Vec<TextureMaterialBinding>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_voxel_material: Option<u16>,
}

pub(crate) fn canonicalize_material_policy(policy: &mut ConversionMaterialPolicy) {
    policy
        .texture_assets
        .sort_by(|left, right| texture_key(&left.texture).cmp(&texture_key(&right.texture)));
    policy
        .texture_bindings
        .sort_by_key(|binding| binding.source_material_slot);
}

pub(crate) fn resolve_material_map(
    settings: &ConversionPlanSettings,
    source: &ImportedMeshSource,
) -> Result<Vec<VoxelAssetMaterialMapping>, ConversionError> {
    let source_materials = source
        .mesh
        .materials
        .iter()
        .map(|material| {
            (
                material.source_material_slot,
                material.source_material_name.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut resolved = BTreeMap::<u32, VoxelAssetMaterialMapping>::new();
    for mapping in &settings.conversion.material_map {
        if resolved
            .insert(mapping.source_material_slot, mapping.clone())
            .is_some()
        {
            return Err(ConversionError::one(
                "conversion.invalidMaterialMap",
                "settings.conversion.materialMap",
                "source material slots must be unique",
            ));
        }
    }
    validate_texture_assets(settings)?;
    let mut texture_slots = BTreeSet::new();
    for binding in &settings.material_policy.texture_bindings {
        if !texture_slots.insert(binding.source_material_slot) {
            return Err(ConversionError::one(
                "conversion.invalidTextureMaterialRule",
                "settings.materialPolicy.textureBindings",
                "source material slots may have at most one texture binding",
            ));
        }
        let source_name = source_materials
            .get(&binding.source_material_slot)
            .ok_or_else(|| {
                ConversionError::one(
                    "conversion.invalidTextureMaterialRule",
                    "settings.materialPolicy.textureBindings",
                    "texture binding references a material slot absent from the source mesh",
                )
            })?;
        validate_texture_binding(binding)?;
        validate_uv_attribute(source, binding)?;
        let texture = settings
            .material_policy
            .texture_assets
            .iter()
            .find(|asset| asset.texture == binding.texture)
            .ok_or_else(|| {
                let same_identity = settings.material_policy.texture_assets.iter().any(|asset| {
                    asset.texture.texture_asset_id == binding.texture.texture_asset_id
                        && asset.texture.asset_version == binding.texture.asset_version
                });
                ConversionError::one(
                    if same_identity {
                        "conversion.textureHashMismatch"
                    } else {
                        "conversion.missingTextureSource"
                    },
                    "settings.materialPolicy.textureBindings",
                    "texture binding does not match an authority-visible texture snapshot",
                )
            })?;
        let material_slot = sample_texture_at(texture, binding.sample_uv);
        resolved.insert(
            binding.source_material_slot,
            VoxelAssetMaterialMapping {
                source_material_slot: binding.source_material_slot,
                source_material_name: source_name.clone(),
                voxel_material_slot: material_slot,
            },
        );
    }
    if let Some(fallback) = settings.material_policy.default_voxel_material {
        for (source_slot, source_name) in &source_materials {
            resolved
                .entry(*source_slot)
                .or_insert_with(|| VoxelAssetMaterialMapping {
                    source_material_slot: *source_slot,
                    source_material_name: source_name.clone(),
                    voxel_material_slot: fallback,
                });
        }
    }
    Ok(resolved.into_values().collect())
}

pub(crate) struct MaterialSamplingContext<'a> {
    bindings: BTreeMap<u32, ResolvedTextureBinding<'a>>,
}

struct ResolvedTextureBinding<'a> {
    texture: &'a TextureSampleAsset,
    source_set_index: u32,
}

impl<'a> MaterialSamplingContext<'a> {
    pub(crate) fn resolve(
        &self,
        mesh: &ImportedStaticMesh,
        evidence: MaterialEvidence,
        fallback: u16,
    ) -> Result<u16, ConversionError> {
        let Some(binding) = self.bindings.get(&evidence.source_material_slot) else {
            return Ok(fallback);
        };
        let triangle = mesh.triangles.get(evidence.triangle_index).ok_or_else(|| {
            ConversionError::one(
                "conversion.invalidGeometry",
                "source.triangles",
                "voxel material evidence references a missing source triangle",
            )
        })?;
        if triangle.source_material_slot != evidence.source_material_slot {
            return Err(ConversionError::one(
                "conversion.invalidGeometry",
                format!("source.triangles[{}]", evidence.triangle_index),
                "voxel material evidence drifted from the source triangle material",
            ));
        }
        let coordinates = mesh
            .texture_coordinates
            .iter()
            .find(|candidate| candidate.source_set_index == binding.source_set_index)
            .ok_or_else(|| missing_uv(evidence.source_material_slot))?;
        let mut uv = [0.0; 2];
        for (vertex, weight) in triangle.indices.iter().zip(evidence.barycentric) {
            let coordinate = coordinates
                .coordinates
                .get(*vertex as usize)
                .copied()
                .flatten()
                .ok_or_else(|| missing_uv(evidence.source_material_slot))?;
            uv[0] += coordinate[0] * weight;
            uv[1] += coordinate[1] * weight;
        }
        if uv.iter().any(|component| !component.is_finite()) {
            return Err(ConversionError::one(
                "conversion.invalidGeometry",
                format!(
                    "source.triangles[{}].textureCoordinates",
                    evidence.triangle_index
                ),
                "interpolated texture coordinates must remain finite",
            ));
        }
        Ok(sample_texture_at(binding.texture, uv))
    }
}

pub(crate) fn material_sampling_context<'a>(
    settings: &'a ConversionPlanSettings,
    source: &'a ImportedMeshSource,
) -> Result<MaterialSamplingContext<'a>, ConversionError> {
    let mut bindings = BTreeMap::new();
    for binding in &settings.material_policy.texture_bindings {
        let uv_attribute = validate_uv_attribute(source, binding)?;
        let texture = settings
            .material_policy
            .texture_assets
            .iter()
            .find(|asset| asset.texture == binding.texture)
            .ok_or_else(|| {
                ConversionError::one(
                    "conversion.missingTextureSource",
                    "settings.materialPolicy.textureBindings",
                    "texture binding does not match an authority-visible texture snapshot",
                )
            })?;
        bindings.insert(
            binding.source_material_slot,
            ResolvedTextureBinding {
                texture,
                source_set_index: uv_attribute.source_set_index,
            },
        );
    }
    Ok(MaterialSamplingContext { bindings })
}

fn validate_texture_assets(settings: &ConversionPlanSettings) -> Result<(), ConversionError> {
    let palette = settings
        .conversion
        .material_palette
        .iter()
        .map(|binding| binding.material_slot)
        .collect::<BTreeSet<_>>();
    let mut identities = BTreeSet::new();
    for asset in &settings.material_policy.texture_assets {
        validate_texture_source(&asset.texture)?;
        if !identities.insert(texture_key(&asset.texture)) {
            return Err(ConversionError::one(
                "conversion.invalidTextureMaterialRule",
                "settings.materialPolicy.textureAssets",
                "texture snapshot identities must be unique",
            ));
        }
        let expected = u64::from(asset.texture.width) * u64::from(asset.texture.height);
        if expected == 0
            || expected > MAX_CONVERSION_TEXTURE_TEXELS as u64
            || asset.texel_materials.len() as u64 != expected
        {
            return Err(ConversionError::one(
                "conversion.invalidTextureMaterialRule",
                "settings.materialPolicy.textureAssets.texelMaterials",
                format!(
                    "texel material count must equal width * height within {MAX_CONVERSION_TEXTURE_TEXELS} texels"
                ),
            ));
        }
        if asset
            .texel_materials
            .iter()
            .any(|material| !palette.contains(material))
        {
            return Err(ConversionError::one(
                "conversion.invalidTextureMaterialRule",
                "settings.materialPolicy.textureAssets.texelMaterials",
                "every sampled material must have a target materialPalette binding",
            ));
        }
    }
    Ok(())
}

fn validate_texture_source(texture: &TextureSourceRef) -> Result<(), ConversionError> {
    match AssetId::parse(&texture.texture_asset_id) {
        Ok(id) if id.kind() == AssetKind::Texture => {}
        Ok(id) => {
            return Err(ConversionError::one(
                "conversion.unsupportedTextureFormat",
                "settings.materialPolicy.textureAssets.textureAssetId",
                format!("expected texture identity, found {}", id.kind()),
            ));
        }
        Err(error) => {
            return Err(ConversionError::one(
                "conversion.unsupportedTextureFormat",
                "settings.materialPolicy.textureAssets.textureAssetId",
                error.to_string(),
            ));
        }
    }
    if texture.asset_version == 0 || !valid_sha256(&texture.content_hash) {
        return Err(ConversionError::one(
            "conversion.missingTextureSource",
            "settings.materialPolicy.textureAssets",
            "texture snapshot requires a positive version and lowercase sha256 content hash",
        ));
    }
    Ok(())
}

fn validate_texture_binding(binding: &TextureMaterialBinding) -> Result<(), ConversionError> {
    if binding.uv_attribute.attribute_name.trim().is_empty()
        || !valid_sha256(&binding.uv_attribute.source_hash)
    {
        return Err(ConversionError::one(
            "conversion.missingUvAttribute",
            "settings.materialPolicy.textureBindings.uvAttribute",
            "texture binding requires a named, hash-pinned UV attribute",
        ));
    }
    if binding.sample_uv.iter().any(|value| !value.is_finite()) {
        return Err(ConversionError::one(
            "conversion.invalidTextureMaterialRule",
            "settings.materialPolicy.textureBindings.sampleUv",
            "texture sample UV must be finite",
        ));
    }
    Ok(())
}

fn validate_uv_attribute<'a>(
    source: &'a ImportedMeshSource,
    binding: &TextureMaterialBinding,
) -> Result<&'a MeshSourceTextureCoordinates, ConversionError> {
    let attribute = source
        .receipt
        .metadata
        .texture_coordinates
        .iter()
        .find(|attribute| attribute.attribute_name == binding.uv_attribute.attribute_name)
        .ok_or_else(|| missing_uv(binding.source_material_slot))?;
    if attribute.source_hash != binding.uv_attribute.source_hash {
        return Err(ConversionError::one(
            "conversion.uvHashMismatch",
            "settings.materialPolicy.textureBindings.uvAttribute.sourceHash",
            format!(
                "UV attribute {} expected {}, imported {}",
                attribute.attribute_name, binding.uv_attribute.source_hash, attribute.source_hash
            ),
        ));
    }
    let coordinates = source
        .mesh
        .texture_coordinates
        .iter()
        .find(|candidate| candidate.source_set_index == attribute.source_set_index)
        .ok_or_else(|| missing_uv(binding.source_material_slot))?;
    for triangle in source
        .mesh
        .triangles
        .iter()
        .filter(|triangle| triangle.source_material_slot == binding.source_material_slot)
    {
        if triangle.indices.iter().any(|index| {
            coordinates
                .coordinates
                .get(*index as usize)
                .is_none_or(Option::is_none)
        }) {
            return Err(missing_uv(binding.source_material_slot));
        }
    }
    Ok(attribute)
}

fn missing_uv(source_material_slot: u32) -> ConversionError {
    ConversionError::one(
        "conversion.missingUvAttribute",
        "settings.materialPolicy.textureBindings.uvAttribute",
        format!(
            "source material slot {source_material_slot} does not have the complete requested UV attribute"
        ),
    )
}

fn sample_texture_at(asset: &TextureSampleAsset, uv: [f64; 2]) -> u16 {
    let x = nearest_texel_axis(uv[0], asset.texture.width);
    let y = nearest_texel_axis(uv[1], asset.texture.height);
    asset.texel_materials[y * asset.texture.width as usize + x]
}

fn nearest_texel_axis(uv: f64, size: u32) -> usize {
    let max_index = f64::from(size.saturating_sub(1));
    (uv.clamp(0.0, 1.0) * max_index).round() as usize
}

fn texture_key(texture: &TextureSourceRef) -> (&str, u64, &str) {
    (
        &texture.texture_asset_id,
        texture.asset_version,
        &texture.content_hash,
    )
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
