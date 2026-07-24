use core_assets::{AssetHash, AssetId, AssetKind, AssetReference, AssetVersionReq};
use serde::{Deserialize, Serialize};

use crate::{
    AssetCatalog, AssetLock, AssetLockEntry, CatalogEntry, MaterialAuthority, MaterialDefinition,
    MaterialStyle, Rgba, StructuralClass, UvStrategy,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetCatalogCodecError {
    pub path: String,
    pub message: String,
}

impl AssetCatalogCodecError {
    fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for AssetCatalogCodecError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.path, self.message)
    }
}

impl std::error::Error for AssetCatalogCodecError {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct StoredAssetCatalog {
    pub entries: Vec<StoredCatalogEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct StoredCatalogEntry {
    pub id: String,
    pub version: u32,
    pub hash: Option<String>,
    pub source_path: Option<String>,
    pub label: Option<String>,
    #[serde(default)]
    pub dependencies: Vec<StoredAssetReference>,
    pub material: Option<StoredMaterialDefinition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct StoredAssetReference {
    pub id: String,
    #[serde(default)]
    pub version: StoredAssetVersionRequirement,
    pub hash: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, tag = "req", rename_all = "camelCase")]
pub enum StoredAssetVersionRequirement {
    #[default]
    Any,
    Exact {
        value: u32,
    },
    AtLeast {
        value: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct StoredMaterialDefinition {
    pub authority: StoredMaterialAuthority,
    pub style: StoredMaterialStyle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct StoredMaterialAuthority {
    pub solid: bool,
    pub collidable: bool,
    pub occludes: bool,
    pub structural_class: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct StoredMaterialStyle {
    pub color: [f32; 4],
    pub texture: Option<StoredAssetReference>,
    pub texture_tint: [f32; 4],
    pub emission_color: [f32; 4],
    pub roughness: f32,
    pub emissive: f32,
    pub uv_strategy: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct StoredAssetLock {
    pub entries: Vec<StoredAssetLockEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct StoredAssetLockEntry {
    pub id: String,
    pub kind: String,
    pub version: u32,
    pub hash: Option<String>,
    pub dependencies: Vec<String>,
}

pub fn encode_catalog(catalog: &AssetCatalog) -> Result<String, AssetCatalogCodecError> {
    let stored = StoredAssetCatalog::from_catalog(&catalog.canonical())?;
    encode_json(&stored)
}

pub fn decode_catalog(input: &str) -> Result<AssetCatalog, AssetCatalogCodecError> {
    let stored: StoredAssetCatalog = decode_json(input)?;
    stored.into_catalog()
}

pub fn encode_lock(lock: &AssetLock) -> Result<String, AssetCatalogCodecError> {
    let mut stored = StoredAssetLock::from(lock);
    stored.entries.sort_by(|left, right| left.id.cmp(&right.id));
    for entry in &mut stored.entries {
        entry.dependencies.sort();
        entry.dependencies.dedup();
    }
    encode_json(&stored)
}

pub fn decode_lock(input: &str) -> Result<AssetLock, AssetCatalogCodecError> {
    let stored: StoredAssetLock = decode_json(input)?;
    stored.into_lock()
}

fn encode_json(value: &impl Serialize) -> Result<String, AssetCatalogCodecError> {
    let mut encoded = serde_json::to_string_pretty(value)
        .map_err(|error| AssetCatalogCodecError::new("$", error.to_string()))?;
    encoded.push('\n');
    Ok(encoded)
}

fn decode_json<T: for<'de> Deserialize<'de>>(input: &str) -> Result<T, AssetCatalogCodecError> {
    let mut deserializer = serde_json::Deserializer::from_str(input);
    let value = serde_path_to_error::deserialize(&mut deserializer).map_err(|error| {
        let path = error.path().to_string();
        AssetCatalogCodecError::new(
            if path.is_empty() {
                "$".to_string()
            } else {
                path
            },
            error.inner().to_string(),
        )
    })?;
    deserializer
        .end()
        .map_err(|error| AssetCatalogCodecError::new("$", error.to_string()))?;
    Ok(value)
}

impl StoredAssetCatalog {
    pub fn from_catalog(catalog: &AssetCatalog) -> Result<Self, AssetCatalogCodecError> {
        let entries = catalog
            .entries
            .iter()
            .enumerate()
            .map(|(index, entry)| StoredCatalogEntry::from_entry(entry, index))
            .collect::<Result<_, _>>()?;
        Ok(Self { entries })
    }

    pub fn into_catalog(self) -> Result<AssetCatalog, AssetCatalogCodecError> {
        let entries = self
            .entries
            .into_iter()
            .enumerate()
            .map(|(index, entry)| entry.into_entry(index))
            .collect::<Result<_, _>>()?;
        Ok(AssetCatalog::from_entries(entries))
    }
}

impl StoredCatalogEntry {
    fn from_entry(entry: &CatalogEntry, index: usize) -> Result<Self, AssetCatalogCodecError> {
        Ok(Self {
            id: entry.id.as_str().to_string(),
            version: entry.version,
            hash: entry.hash.as_ref().map(|hash| hash.as_str().to_string()),
            source_path: entry.source_path.clone(),
            label: entry.label.clone(),
            dependencies: entry
                .dependencies
                .iter()
                .map(StoredAssetReference::from)
                .collect(),
            material: entry
                .material
                .as_ref()
                .map(|material| StoredMaterialDefinition::from_material(material, index))
                .transpose()?,
        })
    }

    fn into_entry(self, index: usize) -> Result<CatalogEntry, AssetCatalogCodecError> {
        let base = format!("entries[{index}]");
        Ok(CatalogEntry {
            id: parse_id(&format!("{base}.id"), &self.id)?,
            version: self.version,
            hash: parse_optional_hash(&format!("{base}.hash"), self.hash)?,
            source_path: self.source_path,
            label: self.label,
            dependencies: self
                .dependencies
                .into_iter()
                .enumerate()
                .map(|(dependency_index, dependency)| {
                    dependency.into_reference(&format!("{base}.dependencies[{dependency_index}]"))
                })
                .collect::<Result<_, _>>()?,
            material: self
                .material
                .map(|material| material.into_material(&format!("{base}.material")))
                .transpose()?,
        })
    }
}

impl From<&AssetReference> for StoredAssetReference {
    fn from(reference: &AssetReference) -> Self {
        Self {
            id: reference.id().as_str().to_string(),
            version: reference.version().into(),
            hash: reference.hash().map(|hash| hash.as_str().to_string()),
        }
    }
}

impl StoredAssetReference {
    fn into_reference(self, path: &str) -> Result<AssetReference, AssetCatalogCodecError> {
        Ok(AssetReference::new(
            parse_id(&format!("{path}.id"), &self.id)?,
            self.version.into(),
            parse_optional_hash(&format!("{path}.hash"), self.hash)?,
        ))
    }
}

impl From<AssetVersionReq> for StoredAssetVersionRequirement {
    fn from(requirement: AssetVersionReq) -> Self {
        match requirement {
            AssetVersionReq::Any => Self::Any,
            AssetVersionReq::Exact(value) => Self::Exact { value },
            AssetVersionReq::AtLeast(value) => Self::AtLeast { value },
        }
    }
}

impl From<StoredAssetVersionRequirement> for AssetVersionReq {
    fn from(requirement: StoredAssetVersionRequirement) -> Self {
        match requirement {
            StoredAssetVersionRequirement::Any => Self::Any,
            StoredAssetVersionRequirement::Exact { value } => Self::Exact(value),
            StoredAssetVersionRequirement::AtLeast { value } => Self::AtLeast(value),
        }
    }
}

impl StoredMaterialDefinition {
    fn from_material(
        material: &MaterialDefinition,
        entry_index: usize,
    ) -> Result<Self, AssetCatalogCodecError> {
        let path = format!("entries[{entry_index}].material.style");
        ensure_rgba_finite(&format!("{path}.color"), material.style.color)?;
        ensure_rgba_finite(&format!("{path}.textureTint"), material.style.texture_tint)?;
        ensure_rgba_finite(
            &format!("{path}.emissionColor"),
            material.style.emission_color,
        )?;
        for (field, value) in [
            ("roughness", material.style.roughness),
            ("emissive", material.style.emissive),
        ] {
            if !value.is_finite() {
                return Err(AssetCatalogCodecError::new(
                    format!("{path}.{field}"),
                    "value must be finite",
                ));
            }
        }
        Ok(Self {
            authority: StoredMaterialAuthority {
                solid: material.authority.solid,
                collidable: material.authority.collidable,
                occludes: material.authority.occludes,
                structural_class: structural_class_tag(material.authority.structural_class)
                    .to_string(),
            },
            style: StoredMaterialStyle {
                color: rgba_array(material.style.color),
                texture: material
                    .style
                    .texture
                    .as_ref()
                    .map(StoredAssetReference::from),
                texture_tint: rgba_array(material.style.texture_tint),
                emission_color: rgba_array(material.style.emission_color),
                roughness: material.style.roughness,
                emissive: material.style.emissive,
                uv_strategy: uv_strategy_tag(material.style.uv_strategy).to_string(),
            },
        })
    }

    fn into_material(self, path: &str) -> Result<MaterialDefinition, AssetCatalogCodecError> {
        let style_path = format!("{path}.style");
        let style = MaterialStyle {
            color: rgba_from_array(&format!("{style_path}.color"), self.style.color)?,
            texture: self
                .style
                .texture
                .map(|texture| texture.into_reference(&format!("{style_path}.texture")))
                .transpose()?,
            roughness: finite(&format!("{style_path}.roughness"), self.style.roughness)?,
            texture_tint: rgba_from_array(
                &format!("{style_path}.textureTint"),
                self.style.texture_tint,
            )?,
            emission_color: rgba_from_array(
                &format!("{style_path}.emissionColor"),
                self.style.emission_color,
            )?,
            emissive: finite(&format!("{style_path}.emissive"), self.style.emissive)?,
            uv_strategy: match self.style.uv_strategy.as_str() {
                "flat" => UvStrategy::Flat,
                "planar" => UvStrategy::Planar,
                "atlas" => UvStrategy::Atlas,
                other => {
                    return Err(AssetCatalogCodecError::new(
                        format!("{style_path}.uvStrategy"),
                        format!("unknown UV strategy `{other}`"),
                    ));
                }
            },
        };
        let authority = MaterialAuthority {
            solid: self.authority.solid,
            collidable: self.authority.collidable,
            occludes: self.authority.occludes,
            structural_class: match self.authority.structural_class.as_str() {
                "decorative" => StructuralClass::Decorative,
                "solid" => StructuralClass::Solid,
                "structural" => StructuralClass::Structural,
                other => {
                    return Err(AssetCatalogCodecError::new(
                        format!("{path}.authority.structuralClass"),
                        format!("unknown structural class `{other}`"),
                    ));
                }
            },
        };
        Ok(MaterialDefinition { authority, style })
    }
}

impl From<&AssetLock> for StoredAssetLock {
    fn from(lock: &AssetLock) -> Self {
        Self {
            entries: lock
                .entries
                .iter()
                .map(StoredAssetLockEntry::from)
                .collect(),
        }
    }
}

impl StoredAssetLock {
    pub fn into_lock(self) -> Result<AssetLock, AssetCatalogCodecError> {
        Ok(AssetLock {
            entries: self
                .entries
                .into_iter()
                .enumerate()
                .map(|(index, entry)| entry.into_entry(index))
                .collect::<Result<_, _>>()?,
        })
    }
}

impl From<&AssetLockEntry> for StoredAssetLockEntry {
    fn from(entry: &AssetLockEntry) -> Self {
        Self {
            id: entry.id.as_str().to_string(),
            kind: entry.kind.prefix().to_string(),
            version: entry.version,
            hash: entry.hash.as_ref().map(|hash| hash.as_str().to_string()),
            dependencies: entry
                .dependencies
                .iter()
                .map(|dependency| dependency.as_str().to_string())
                .collect(),
        }
    }
}

impl StoredAssetLockEntry {
    fn into_entry(self, index: usize) -> Result<AssetLockEntry, AssetCatalogCodecError> {
        let base = format!("entries[{index}]");
        Ok(AssetLockEntry {
            id: parse_id(&format!("{base}.id"), &self.id)?,
            kind: AssetKind::from_prefix(&self.kind).ok_or_else(|| {
                AssetCatalogCodecError::new(
                    format!("{base}.kind"),
                    format!("unknown asset kind `{}`", self.kind),
                )
            })?,
            version: self.version,
            hash: parse_optional_hash(&format!("{base}.hash"), self.hash)?,
            dependencies: self
                .dependencies
                .into_iter()
                .enumerate()
                .map(|(dependency_index, dependency)| {
                    parse_id(
                        &format!("{base}.dependencies[{dependency_index}]"),
                        &dependency,
                    )
                })
                .collect::<Result<_, _>>()?,
        })
    }
}

fn parse_id(path: &str, value: &str) -> Result<AssetId, AssetCatalogCodecError> {
    AssetId::parse(value).map_err(|error| AssetCatalogCodecError::new(path, error.to_string()))
}

fn parse_optional_hash(
    path: &str,
    value: Option<String>,
) -> Result<Option<AssetHash>, AssetCatalogCodecError> {
    value
        .map(|hash| {
            AssetHash::parse(&hash)
                .map_err(|error| AssetCatalogCodecError::new(path, error.to_string()))
        })
        .transpose()
}

fn structural_class_tag(value: StructuralClass) -> &'static str {
    match value {
        StructuralClass::Decorative => "decorative",
        StructuralClass::Solid => "solid",
        StructuralClass::Structural => "structural",
    }
}

fn uv_strategy_tag(value: UvStrategy) -> &'static str {
    match value {
        UvStrategy::Flat => "flat",
        UvStrategy::Planar => "planar",
        UvStrategy::Atlas => "atlas",
    }
}

fn rgba_array(value: Rgba) -> [f32; 4] {
    [value.r, value.g, value.b, value.a]
}

fn rgba_from_array(path: &str, value: [f32; 4]) -> Result<Rgba, AssetCatalogCodecError> {
    let rgba = Rgba {
        r: value[0],
        g: value[1],
        b: value[2],
        a: value[3],
    };
    ensure_rgba_finite(path, rgba)?;
    Ok(rgba)
}

fn ensure_rgba_finite(path: &str, value: Rgba) -> Result<(), AssetCatalogCodecError> {
    if [value.r, value.g, value.b, value.a]
        .into_iter()
        .all(f32::is_finite)
    {
        Ok(())
    } else {
        Err(AssetCatalogCodecError::new(
            path,
            "every channel must be finite",
        ))
    }
}

fn finite(path: &str, value: f32) -> Result<f32, AssetCatalogCodecError> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(AssetCatalogCodecError::new(path, "value must be finite"))
    }
}
