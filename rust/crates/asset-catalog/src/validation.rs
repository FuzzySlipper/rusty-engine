use core_assets::{AssetHash, AssetId, AssetKind, AssetReference, AssetVersionReq};
use serde::Serialize;

use crate::{
    AssetCatalog, AtlasRegionDefinition, CatalogEntry, DependencyGraph, TextureWrap,
    VoxelAlphaMode, VoxelSurfaceMapping,
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
    DuplicateAtlasRegion {
        id: AssetId,
        region: String,
    },
    InvalidAtlasRegionRect {
        id: AssetId,
        region: String,
    },
    AtlasRegionOutOfBounds {
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
            Self::ConflictingSurfaceTexture { .. } => "conflicting_surface_texture",
            Self::MissingTextureDefinition { .. } => "missing_texture_definition",
            Self::InvalidTextureDimensions { .. } => "invalid_texture_dimensions",
            Self::InvalidSurfaceSchema { .. } => "invalid_surface_schema",
            Self::InvalidAtlasSchema { .. } => "invalid_atlas_schema",
            Self::InvalidTileScale { .. } => "invalid_tile_scale",
            Self::InvalidTileOrigin { .. } => "invalid_tile_origin",
            Self::InvalidAlphaCutoff { .. } => "invalid_alpha_cutoff",
            Self::InvalidTextureWrap { .. } => "invalid_texture_wrap",
            Self::DuplicateAtlasRegion { .. } => "duplicate_atlas_region",
            Self::InvalidAtlasRegionRect { .. } => "invalid_atlas_region_rect",
            Self::AtlasRegionOutOfBounds { .. } => "atlas_region_out_of_bounds",
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

/// Validate a canonical catalog ([`AssetCatalog::canonical`]): entries in id
/// order and each entry's dependencies complete. Admission is the one caller.
pub(crate) fn validate_catalog(catalog: &AssetCatalog) -> CatalogValidationReport {
    let mut errors = Vec::new();
    for (index, pair) in catalog.entries.windows(2).enumerate() {
        let repeated = index > 0 && catalog.entries[index - 1].id == pair[0].id;
        if pair[0].id == pair[1].id && !repeated {
            errors.push(CatalogValidationError::DuplicateAssetId {
                id: pair[1].id.clone(),
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
            if texture.width == 0 || texture.height == 0 {
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
        // Canonical dependencies are sorted with exact repeats removed, so
        // neighbours sharing an id ask for different versions or hashes.
        for pair in entry.dependencies.windows(2) {
            if pair[0].id() == pair[1].id() {
                errors.push(CatalogValidationError::DuplicateDependency {
                    from: entry.id.clone(),
                    dependency: pair[1].id().clone(),
                });
            }
        }
        for dependency in &entry.dependencies {
            let Some(target) = find(catalog, dependency.id()) else {
                errors.push(CatalogValidationError::UnknownDependency {
                    from: entry.id.clone(),
                    dependency: dependency.id().clone(),
                });
                continue;
            };
            validate_reference_target(entry, dependency, target, &mut errors);
        }

        if let Some(atlas) = &entry.voxel_atlas {
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

    if let Some(path) = DependencyGraph::build(catalog).detect_cycle() {
        errors.push(CatalogValidationError::DependencyCycle { path });
    }
    CatalogValidationReport { errors }
}

/// The first entry with `id`, as [`AssetCatalog::get`] finds it, by binary
/// search over the canonical id order.
fn find<'a>(catalog: &'a AssetCatalog, id: &AssetId) -> Option<&'a CatalogEntry> {
    let index = catalog
        .entries
        .partition_point(|entry| entry.id.as_str() < id.as_str());
    catalog.entries.get(index).filter(|entry| entry.id == *id)
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
        VoxelSurfaceMapping::Atlas { atlas, .. } => find(catalog, atlas.id())
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

/// A surface or atlas reference's resolution and pins are checked as one of
/// the entry's dependencies, which the canonical catalog includes it in.
fn validate_owned_reference_kind(
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
        if !(0.0..=1.0).contains(&cutoff) {
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
    validate_owned_reference_kind(entry, reference, expected, errors);
    validate_tile_mapping(entry, *scale, *origin, errors);

    match &surface.mapping {
        VoxelSurfaceMapping::Repeat { texture, .. } => {
            if let Some(target) = find(catalog, texture.id()) {
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
            if let Some(target) = find(catalog, atlas.id()) {
                // Canonical atlas regions are sorted by id.
                if target.voxel_atlas.as_ref().is_none_or(|definition| {
                    definition
                        .regions
                        .binary_search_by(|item| item.id.as_str().cmp(region))
                        .is_err()
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

/// The voxel mesher divides by the tile scale and keeps the origin in its
/// tile phase; how large or small either may be is the mesher's limit.
fn validate_tile_mapping(
    entry: &CatalogEntry,
    scale: [f32; 2],
    origin: [f32; 2],
    errors: &mut Vec<CatalogValidationError>,
) {
    if scale
        .into_iter()
        .any(|value| !value.is_finite() || value <= 0.0)
    {
        errors.push(CatalogValidationError::InvalidTileScale {
            id: entry.id.clone(),
        });
    }
    if origin.into_iter().any(|value| !value.is_finite()) {
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
    validate_owned_reference_kind(entry, &atlas.texture, AssetKind::Texture, errors);
    let Some(texture_entry) = find(catalog, atlas.texture.id()) else {
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

    // Canonical atlas regions are sorted by id, so a repeated id is adjacent.
    for (index, region) in atlas.regions.iter().enumerate() {
        if index > 0 && atlas.regions[index - 1].id == region.id {
            errors.push(CatalogValidationError::DuplicateAtlasRegion {
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
        if !padded_rect_fits(region, texture.width, texture.height) {
            errors.push(CatalogValidationError::AtlasRegionOutOfBounds {
                id: entry.id.clone(),
                region: region.id.clone(),
            });
        }
    }
}

fn padded_rect_fits(region: &AtlasRegionDefinition, width: u32, height: u32) -> bool {
    let end_fits = |axis: usize, padding: u16, limit: u32| {
        region.content_min[axis]
            .checked_add(region.content_extent[axis])
            .and_then(|end| end.checked_add(u32::from(padding)))
            .is_some_and(|end| end <= limit)
    };
    region.content_min[0] >= u32::from(region.padding.left)
        && region.content_min[1] >= u32::from(region.padding.bottom)
        && end_fits(0, region.padding.right, width)
        && end_fits(1, region.padding.top, height)
}
