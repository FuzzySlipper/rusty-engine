use std::collections::BTreeSet;

use core_assets::{AssetHash, AssetId, AssetKind, AssetReference, AssetVersionReq};
use serde::Serialize;

use crate::{
    AssetCatalog, AtlasRegionDefinition, CatalogEntry, DependencyGraph, TextureFilter, TextureWrap,
    VoxelAlphaMode, VoxelSurfaceMapping, MAX_AGGREGATE_ATLAS_REGIONS, MAX_ATLAS_PADDING,
    MAX_ATLAS_REGIONS, MAX_TEXTURE_DIMENSION, MAX_TEXTURE_TEXELS, MAX_TILE_ORIGIN_CELLS,
    MAX_TILE_SCALE_CELLS, MIN_TILE_SCALE_CELLS,
};

#[derive(Debug, Clone, PartialEq)]
pub enum CatalogValidationError {
    DuplicateAssetId {
        id: AssetId,
    },
    MaterialPayloadMissing {
        id: AssetId,
    },
    MaterialPayloadOnNonMaterial {
        id: AssetId,
        kind: AssetKind,
    },
    WrongKindReference {
        from: AssetId,
        slot: &'static str,
        expected: AssetKind,
        actual: AssetKind,
        reference: AssetId,
    },
    UnknownDependency {
        from: AssetId,
        dependency: AssetId,
    },
    DependencyCycle {
        path: Vec<AssetId>,
    },
    EmptySourcePath {
        id: AssetId,
    },
    PayloadOnWrongKind {
        id: AssetId,
        payload: &'static str,
        expected: AssetKind,
        actual: AssetKind,
    },
    DuplicateDependency {
        from: AssetId,
        dependency: AssetId,
    },
    StaleDependencyVersion {
        from: AssetId,
        dependency: AssetId,
        required: AssetVersionReq,
        actual: u32,
    },
    StaleDependencyHash {
        from: AssetId,
        dependency: AssetId,
        required: AssetHash,
        actual: Option<AssetHash>,
    },
    UnpinnedSurfaceReference {
        from: AssetId,
        reference: AssetId,
    },
    UndeclaredSurfaceDependency {
        from: AssetId,
        reference: AssetId,
    },
    ConflictingSurfaceTexture {
        from: AssetId,
        style_texture: AssetId,
        surface_texture: AssetId,
    },
    MissingTextureDefinition {
        from: AssetId,
        texture: AssetId,
    },
    InvalidTextureDimensions {
        id: AssetId,
    },
    InvalidSurfaceSchema {
        id: AssetId,
    },
    InvalidAtlasSchema {
        id: AssetId,
    },
    InvalidTileScale {
        id: AssetId,
    },
    InvalidTileOrigin {
        id: AssetId,
    },
    InvalidAlphaCutoff {
        id: AssetId,
    },
    InvalidTextureWrap {
        id: AssetId,
        expected: TextureWrap,
        actual: TextureWrap,
    },
    AtlasRegionQuotaExceeded {
        id: AssetId,
    },
    AggregateAtlasRegionQuotaExceeded,
    DuplicateAtlasRegion {
        id: AssetId,
        region: String,
    },
    InvalidAtlasRegionId {
        id: AssetId,
        region: String,
    },
    InvalidAtlasRegionRect {
        id: AssetId,
        region: String,
    },
    InvalidAtlasPadding {
        id: AssetId,
        region: String,
    },
    AtlasRegionOutOfBounds {
        id: AssetId,
        region: String,
    },
    AtlasRegionOverlap {
        id: AssetId,
        first: String,
        second: String,
    },
    InsufficientAtlasPadding {
        id: AssetId,
        region: String,
    },
    MissingAtlasRegion {
        id: AssetId,
        atlas: AssetId,
        region: String,
    },
}

impl CatalogValidationError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::DuplicateAssetId { .. } => "duplicate_asset_id",
            Self::MaterialPayloadMissing { .. } => "material_payload_missing",
            Self::MaterialPayloadOnNonMaterial { .. } => "material_payload_on_non_material",
            Self::WrongKindReference { .. } => "wrong_kind_reference",
            Self::UnknownDependency { .. } => "unknown_dependency",
            Self::DependencyCycle { .. } => "dependency_cycle",
            Self::EmptySourcePath { .. } => "empty_source_path",
            Self::PayloadOnWrongKind { .. } => "payload_on_wrong_kind",
            Self::DuplicateDependency { .. } => "duplicate_dependency",
            Self::StaleDependencyVersion { .. } => "stale_dependency_version",
            Self::StaleDependencyHash { .. } => "stale_dependency_hash",
            Self::UnpinnedSurfaceReference { .. } => "unpinned_surface_reference",
            Self::UndeclaredSurfaceDependency { .. } => "undeclared_surface_dependency",
            Self::ConflictingSurfaceTexture { .. } => "conflicting_surface_texture",
            Self::MissingTextureDefinition { .. } => "missing_texture_definition",
            Self::InvalidTextureDimensions { .. } => "invalid_texture_dimensions",
            Self::InvalidSurfaceSchema { .. } => "invalid_surface_schema",
            Self::InvalidAtlasSchema { .. } => "invalid_atlas_schema",
            Self::InvalidTileScale { .. } => "invalid_tile_scale",
            Self::InvalidTileOrigin { .. } => "invalid_tile_origin",
            Self::InvalidAlphaCutoff { .. } => "invalid_alpha_cutoff",
            Self::InvalidTextureWrap { .. } => "invalid_texture_wrap",
            Self::AtlasRegionQuotaExceeded { .. } => "atlas_region_quota_exceeded",
            Self::AggregateAtlasRegionQuotaExceeded => "aggregate_atlas_region_quota_exceeded",
            Self::DuplicateAtlasRegion { .. } => "duplicate_atlas_region",
            Self::InvalidAtlasRegionId { .. } => "invalid_atlas_region_id",
            Self::InvalidAtlasRegionRect { .. } => "invalid_atlas_region_rect",
            Self::InvalidAtlasPadding { .. } => "invalid_atlas_padding",
            Self::AtlasRegionOutOfBounds { .. } => "atlas_region_out_of_bounds",
            Self::AtlasRegionOverlap { .. } => "atlas_region_overlap",
            Self::InsufficientAtlasPadding { .. } => "insufficient_atlas_padding",
            Self::MissingAtlasRegion { .. } => "missing_atlas_region",
        }
    }

    pub fn diagnostic(&self) -> CatalogDiagnostic {
        let (path, message) = match self {
            Self::DuplicateAssetId { id } => (
                format!("entries[{}]", id.as_str()),
                format!("asset id `{}` occurs more than once", id.as_str()),
            ),
            Self::MaterialPayloadMissing { id } => (
                format!("entries[{}].material", id.as_str()),
                "material asset has no material definition".to_string(),
            ),
            Self::MaterialPayloadOnNonMaterial { id, kind } => (
                format!("entries[{}].material", id.as_str()),
                format!("{} assets cannot carry material definitions", kind.prefix()),
            ),
            Self::WrongKindReference {
                from,
                slot,
                expected,
                actual,
                reference,
            } => (
                format!("entries[{}].{slot}", from.as_str()),
                format!(
                    "reference `{}` is {}, expected {}",
                    reference.as_str(),
                    actual.prefix(),
                    expected.prefix()
                ),
            ),
            Self::UnknownDependency { from, dependency } => (
                format!("entries[{}].dependencies", from.as_str()),
                format!("dependency `{}` is absent", dependency.as_str()),
            ),
            Self::DependencyCycle { path } => (
                "entries".to_string(),
                format!(
                    "dependency cycle: {}",
                    path.iter()
                        .map(AssetId::as_str)
                        .collect::<Vec<_>>()
                        .join(" -> ")
                ),
            ),
            Self::EmptySourcePath { id } => (
                format!("entries[{}].sourcePath", id.as_str()),
                "source path is empty".to_string(),
            ),
            other => (
                "entries".to_string(),
                format!("{}: {other:?}", other.code()),
            ),
        };
        CatalogDiagnostic {
            code: self.code().to_string(),
            path,
            message,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogDiagnostic {
    pub code: String,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CatalogValidationReport {
    pub errors: Vec<CatalogValidationError>,
}

impl CatalogValidationReport {
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn diagnostics(&self) -> Vec<CatalogDiagnostic> {
        self.errors
            .iter()
            .map(CatalogValidationError::diagnostic)
            .collect()
    }
}

pub fn validate_catalog(catalog: &AssetCatalog) -> CatalogValidationReport {
    let mut errors = Vec::new();
    let mut seen = BTreeSet::new();
    let mut reported = BTreeSet::new();
    let mut aggregate_regions = 0usize;
    for entry in &catalog.entries {
        if !seen.insert(entry.id.as_str()) && reported.insert(entry.id.as_str()) {
            errors.push(CatalogValidationError::DuplicateAssetId {
                id: entry.id.clone(),
            });
        }
    }

    for entry in &catalog.entries {
        match (entry.kind(), &entry.material) {
            (AssetKind::Material, None) => {
                errors.push(CatalogValidationError::MaterialPayloadMissing {
                    id: entry.id.clone(),
                });
            }
            (kind, Some(_)) if kind != AssetKind::Material => {
                errors.push(CatalogValidationError::MaterialPayloadOnNonMaterial {
                    id: entry.id.clone(),
                    kind,
                });
            }
            _ => {}
        }

        validate_payload_kind(entry, &mut errors);
        if let Some(texture) = &entry.texture {
            let texels = u64::from(texture.width).checked_mul(u64::from(texture.height));
            if texture.width == 0
                || texture.height == 0
                || texture.width > MAX_TEXTURE_DIMENSION
                || texture.height > MAX_TEXTURE_DIMENSION
                || texels.is_none_or(|value| value > MAX_TEXTURE_TEXELS)
            {
                errors.push(CatalogValidationError::InvalidTextureDimensions {
                    id: entry.id.clone(),
                });
            }
        }

        if let Some(texture) = entry
            .material
            .as_ref()
            .and_then(|material| material.style.texture.as_ref())
        {
            if texture.kind() != AssetKind::Texture {
                errors.push(CatalogValidationError::WrongKindReference {
                    from: entry.id.clone(),
                    slot: "material.style.texture",
                    expected: AssetKind::Texture,
                    actual: texture.kind(),
                    reference: texture.id().clone(),
                });
            }
        }

        if entry.source_path.as_deref() == Some("") {
            errors.push(CatalogValidationError::EmptySourcePath {
                id: entry.id.clone(),
            });
        }
        let mut dependency_ids = BTreeSet::new();
        for dependency in &entry.dependencies {
            if !dependency_ids.insert(dependency.id().as_str()) {
                errors.push(CatalogValidationError::DuplicateDependency {
                    from: entry.id.clone(),
                    dependency: dependency.id().clone(),
                });
            }
            let Some(target) = catalog.get(dependency.id()) else {
                errors.push(CatalogValidationError::UnknownDependency {
                    from: entry.id.clone(),
                    dependency: dependency.id().clone(),
                });
                continue;
            };
            validate_reference_target(entry, dependency, target, &mut errors);
        }

        if let Some(atlas) = &entry.voxel_atlas {
            aggregate_regions = aggregate_regions.saturating_add(atlas.regions.len());
            validate_atlas(catalog, entry, atlas, &mut errors);
        }
        if let Some(surface) = entry
            .material
            .as_ref()
            .and_then(|material| material.style.voxel_surface.as_ref())
        {
            validate_surface_texture_identity(catalog, entry, surface, &mut errors);
            validate_surface(catalog, entry, surface, &mut errors);
        }
    }

    if aggregate_regions > MAX_AGGREGATE_ATLAS_REGIONS {
        errors.push(CatalogValidationError::AggregateAtlasRegionQuotaExceeded);
    }

    if let Some(path) = DependencyGraph::build(catalog).detect_cycle() {
        errors.push(CatalogValidationError::DependencyCycle { path });
    }
    CatalogValidationReport { errors }
}

fn validate_surface_texture_identity(
    catalog: &AssetCatalog,
    entry: &CatalogEntry,
    surface: &crate::VoxelSurfaceBinding,
    errors: &mut Vec<CatalogValidationError>,
) {
    let Some(style_texture) = entry
        .material
        .as_ref()
        .and_then(|material| material.style.texture.as_ref())
    else {
        return;
    };
    let surface_texture = match &surface.mapping {
        VoxelSurfaceMapping::Repeat { texture, .. } => Some(texture),
        VoxelSurfaceMapping::Atlas { atlas, .. } => catalog
            .get(atlas.id())
            .and_then(|target| target.voxel_atlas.as_ref())
            .map(|definition| &definition.texture),
    };
    if let Some(surface_texture) = surface_texture {
        if style_texture != surface_texture {
            errors.push(CatalogValidationError::ConflictingSurfaceTexture {
                from: entry.id.clone(),
                style_texture: style_texture.id().clone(),
                surface_texture: surface_texture.id().clone(),
            });
        }
    }
}

fn validate_payload_kind(entry: &CatalogEntry, errors: &mut Vec<CatalogValidationError>) {
    if entry.texture.is_some() && entry.kind() != AssetKind::Texture {
        errors.push(CatalogValidationError::PayloadOnWrongKind {
            id: entry.id.clone(),
            payload: "texture",
            expected: AssetKind::Texture,
            actual: entry.kind(),
        });
    }
    if entry.voxel_atlas.is_some() && entry.kind() != AssetKind::SpriteSheet {
        errors.push(CatalogValidationError::PayloadOnWrongKind {
            id: entry.id.clone(),
            payload: "voxelAtlas",
            expected: AssetKind::SpriteSheet,
            actual: entry.kind(),
        });
    }
}

fn validate_reference_target(
    from: &CatalogEntry,
    reference: &AssetReference,
    target: &CatalogEntry,
    errors: &mut Vec<CatalogValidationError>,
) {
    let matches_version = match reference.version() {
        AssetVersionReq::Any => true,
        AssetVersionReq::Exact(version) => target.version == version,
        AssetVersionReq::AtLeast(version) => target.version >= version,
    };
    if !matches_version {
        errors.push(CatalogValidationError::StaleDependencyVersion {
            from: from.id.clone(),
            dependency: reference.id().clone(),
            required: reference.version(),
            actual: target.version,
        });
    }
    if let Some(required) = reference.hash() {
        if target.hash.as_ref() != Some(required) {
            errors.push(CatalogValidationError::StaleDependencyHash {
                from: from.id.clone(),
                dependency: reference.id().clone(),
                required: required.clone(),
                actual: target.hash.clone(),
            });
        }
    }
}

fn validate_owned_reference(
    from: &CatalogEntry,
    reference: &AssetReference,
    expected: AssetKind,
    errors: &mut Vec<CatalogValidationError>,
) {
    if reference.kind() != expected {
        errors.push(CatalogValidationError::WrongKindReference {
            from: from.id.clone(),
            slot: "surface",
            expected,
            actual: reference.kind(),
            reference: reference.id().clone(),
        });
    }
    if !matches!(reference.version(), AssetVersionReq::Exact(_)) || reference.hash().is_none() {
        errors.push(CatalogValidationError::UnpinnedSurfaceReference {
            from: from.id.clone(),
            reference: reference.id().clone(),
        });
    }
    if !from
        .dependencies
        .iter()
        .any(|dependency| dependency == reference)
    {
        errors.push(CatalogValidationError::UndeclaredSurfaceDependency {
            from: from.id.clone(),
            reference: reference.id().clone(),
        });
    }
}

fn validate_surface(
    catalog: &AssetCatalog,
    entry: &CatalogEntry,
    surface: &crate::VoxelSurfaceBinding,
    errors: &mut Vec<CatalogValidationError>,
) {
    if surface.schema_version != 1 {
        errors.push(CatalogValidationError::InvalidSurfaceSchema {
            id: entry.id.clone(),
        });
    }
    if let VoxelAlphaMode::Mask { cutoff } = surface.alpha_mode {
        if !cutoff.is_finite() || !(0.0..=1.0).contains(&cutoff) {
            errors.push(CatalogValidationError::InvalidAlphaCutoff {
                id: entry.id.clone(),
            });
        }
    }
    let (reference, scale, origin, expected) = match &surface.mapping {
        VoxelSurfaceMapping::Repeat {
            texture,
            tile_scale_cells,
            tile_origin_cells,
        } => (
            texture,
            tile_scale_cells,
            tile_origin_cells,
            AssetKind::Texture,
        ),
        VoxelSurfaceMapping::Atlas {
            atlas,
            tile_scale_cells,
            tile_origin_cells,
            ..
        } => (
            atlas,
            tile_scale_cells,
            tile_origin_cells,
            AssetKind::SpriteSheet,
        ),
    };
    validate_owned_reference(entry, reference, expected, errors);
    validate_tile_mapping(entry, *scale, *origin, errors);

    match &surface.mapping {
        VoxelSurfaceMapping::Repeat { texture, .. } => {
            if let Some(target) = catalog.get(texture.id()) {
                match &target.texture {
                    Some(definition) if definition.wrap != TextureWrap::Repeat => {
                        errors.push(CatalogValidationError::InvalidTextureWrap {
                            id: target.id.clone(),
                            expected: TextureWrap::Repeat,
                            actual: definition.wrap,
                        });
                    }
                    None => errors.push(CatalogValidationError::MissingTextureDefinition {
                        from: entry.id.clone(),
                        texture: target.id.clone(),
                    }),
                    _ => {}
                }
            }
        }
        VoxelSurfaceMapping::Atlas { atlas, region, .. } => {
            if let Some(target) = catalog.get(atlas.id()) {
                if target.voxel_atlas.as_ref().is_none_or(|definition| {
                    !definition.regions.iter().any(|item| item.id == *region)
                }) {
                    errors.push(CatalogValidationError::MissingAtlasRegion {
                        id: entry.id.clone(),
                        atlas: target.id.clone(),
                        region: region.clone(),
                    });
                }
            }
        }
    }
}

fn validate_tile_mapping(
    entry: &CatalogEntry,
    scale: [f32; 2],
    origin: [f32; 2],
    errors: &mut Vec<CatalogValidationError>,
) {
    if scale.into_iter().any(|value| {
        !value.is_finite() || !(MIN_TILE_SCALE_CELLS..=MAX_TILE_SCALE_CELLS).contains(&value)
    }) {
        errors.push(CatalogValidationError::InvalidTileScale {
            id: entry.id.clone(),
        });
    }
    if origin
        .into_iter()
        .any(|value| !value.is_finite() || value.abs() > MAX_TILE_ORIGIN_CELLS)
    {
        errors.push(CatalogValidationError::InvalidTileOrigin {
            id: entry.id.clone(),
        });
    }
}

fn validate_atlas(
    catalog: &AssetCatalog,
    entry: &CatalogEntry,
    atlas: &crate::VoxelAtlasDefinition,
    errors: &mut Vec<CatalogValidationError>,
) {
    if atlas.schema_version != 1 {
        errors.push(CatalogValidationError::InvalidAtlasSchema {
            id: entry.id.clone(),
        });
    }
    validate_owned_reference(entry, &atlas.texture, AssetKind::Texture, errors);
    if atlas.regions.len() > MAX_ATLAS_REGIONS {
        errors.push(CatalogValidationError::AtlasRegionQuotaExceeded {
            id: entry.id.clone(),
        });
    }
    let Some(texture_entry) = catalog.get(atlas.texture.id()) else {
        return;
    };
    let Some(texture) = &texture_entry.texture else {
        errors.push(CatalogValidationError::MissingTextureDefinition {
            from: entry.id.clone(),
            texture: texture_entry.id.clone(),
        });
        return;
    };
    if texture.wrap != TextureWrap::Clamp {
        errors.push(CatalogValidationError::InvalidTextureWrap {
            id: texture_entry.id.clone(),
            expected: TextureWrap::Clamp,
            actual: texture.wrap,
        });
    }

    let mut seen = BTreeSet::new();
    let mut rects: Vec<(&str, [u32; 4])> = Vec::new();
    for region in &atlas.regions {
        if !seen.insert(region.id.as_str()) {
            errors.push(CatalogValidationError::DuplicateAtlasRegion {
                id: entry.id.clone(),
                region: region.id.clone(),
            });
        }
        if !valid_region_id(&region.id) {
            errors.push(CatalogValidationError::InvalidAtlasRegionId {
                id: entry.id.clone(),
                region: region.id.clone(),
            });
        }
        if region.content_extent.contains(&0) {
            errors.push(CatalogValidationError::InvalidAtlasRegionRect {
                id: entry.id.clone(),
                region: region.id.clone(),
            });
            continue;
        }
        if [
            region.padding.left,
            region.padding.right,
            region.padding.bottom,
            region.padding.top,
        ]
        .into_iter()
        .any(|value| value > MAX_ATLAS_PADDING)
        {
            errors.push(CatalogValidationError::InvalidAtlasPadding {
                id: entry.id.clone(),
                region: region.id.clone(),
            });
        }
        if texture.filter == TextureFilter::Linear
            && [
                region.padding.left,
                region.padding.right,
                region.padding.bottom,
                region.padding.top,
            ]
            .contains(&0)
        {
            errors.push(CatalogValidationError::InsufficientAtlasPadding {
                id: entry.id.clone(),
                region: region.id.clone(),
            });
        }
        match padded_rect(region, texture.width, texture.height) {
            Some(rect) => rects.push((region.id.as_str(), rect)),
            None => errors.push(CatalogValidationError::AtlasRegionOutOfBounds {
                id: entry.id.clone(),
                region: region.id.clone(),
            }),
        }
    }
    for left in 0..rects.len() {
        for right in left + 1..rects.len() {
            if rects_overlap(rects[left].1, rects[right].1) {
                errors.push(CatalogValidationError::AtlasRegionOverlap {
                    id: entry.id.clone(),
                    first: rects[left].0.to_string(),
                    second: rects[right].0.to_string(),
                });
            }
        }
    }
}

fn valid_region_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && !value.starts_with('-')
        && !value.ends_with('-')
}

fn padded_rect(region: &AtlasRegionDefinition, width: u32, height: u32) -> Option<[u32; 4]> {
    let x0 = region.content_min[0].checked_sub(u32::from(region.padding.left))?;
    let y0 = region.content_min[1].checked_sub(u32::from(region.padding.bottom))?;
    let x1 = region.content_min[0]
        .checked_add(region.content_extent[0])?
        .checked_add(u32::from(region.padding.right))?;
    let y1 = region.content_min[1]
        .checked_add(region.content_extent[1])?
        .checked_add(u32::from(region.padding.top))?;
    (x1 <= width && y1 <= height).then_some([x0, y0, x1, y1])
}

fn rects_overlap(left: [u32; 4], right: [u32; 4]) -> bool {
    left[0] < right[2] && right[0] < left[2] && left[1] < right[3] && right[1] < left[3]
}
