use asset_catalog::{
    validate_catalog, AssetCatalog, ResolvedVoxelSurface, ResolvedVoxelSurfaceMapping,
    TextureFilter as CatalogTextureFilter, TextureWrap as CatalogTextureWrap, VoxelAlphaMode,
};
use core_assets::AssetId;
use render_model::{
    MaterialDescriptorError, MaterialUvStrategy, RenderMaterialDescriptor, TextureFilter,
    TextureWrap, VoxelAtlasPaddingDescriptor, VoxelAtlasRegionDescriptor,
    VoxelSurfaceAlphaModeDescriptor, VoxelSurfaceDescriptor, VoxelSurfaceMappingDescriptor,
};

#[derive(Debug, Clone, PartialEq)]
pub enum CatalogMaterialProjectionError {
    InvalidCatalog { codes: Vec<String> },
    MissingMaterial,
    UnresolvedSurface,
    MissingContentHash,
    InvalidDescriptor(MaterialDescriptorError),
}

pub fn project_catalog_material(
    catalog: &AssetCatalog,
    material_id: &AssetId,
) -> Result<RenderMaterialDescriptor, CatalogMaterialProjectionError> {
    let report = validate_catalog(catalog);
    if !report.is_ok() {
        return Err(CatalogMaterialProjectionError::InvalidCatalog {
            codes: report
                .errors
                .iter()
                .map(|error| error.code().to_string())
                .collect(),
        });
    }
    let entry = catalog
        .get(material_id)
        .and_then(|entry| entry.material.as_ref().map(|material| (entry, material)))
        .ok_or(CatalogMaterialProjectionError::MissingMaterial)?;
    let resolved = catalog
        .render_material(material_id)
        .map_err(|_| CatalogMaterialProjectionError::UnresolvedSurface)?;
    let texture = resolved
        .voxel_surface
        .as_ref()
        .map(surface_texture_id)
        .or_else(|| {
            resolved
                .texture
                .as_ref()
                .map(|reference| reference.id().as_str())
        })
        .map(str::to_string);
    let voxel_surface = resolved
        .voxel_surface
        .as_ref()
        .map(project_surface)
        .transpose()?;
    let mut descriptor = RenderMaterialDescriptor {
        schema_version: 2,
        id: entry.0.id.as_str().to_string(),
        color: rgba(entry.1.style.color),
        texture,
        roughness: entry.1.style.roughness,
        texture_tint: rgba(entry.1.style.texture_tint),
        emission_color: [
            entry.1.style.emission_color.r,
            entry.1.style.emission_color.g,
            entry.1.style.emission_color.b,
        ],
        emission_intensity: entry.1.style.emissive,
        uv_strategy: match entry.1.style.uv_strategy {
            asset_catalog::UvStrategy::Flat => MaterialUvStrategy::Flat,
            asset_catalog::UvStrategy::Planar => MaterialUvStrategy::Planar,
            asset_catalog::UvStrategy::Atlas => MaterialUvStrategy::Atlas,
        },
        voxel_surface,
    };
    descriptor
        .validate()
        .map_err(CatalogMaterialProjectionError::InvalidDescriptor)?;
    // Keep the resolved texture identity as the only renderer dependency even
    // when a legacy texture field differs in an invalid candidate.
    if let Some(surface) = &descriptor.voxel_surface {
        descriptor.texture = Some(surface.texture().to_string());
    }
    Ok(descriptor)
}

fn project_surface(
    surface: &ResolvedVoxelSurface,
) -> Result<VoxelSurfaceDescriptor, CatalogMaterialProjectionError> {
    let alpha_mode = match surface.alpha_mode {
        VoxelAlphaMode::Opaque => VoxelSurfaceAlphaModeDescriptor::Opaque,
        VoxelAlphaMode::Mask { cutoff } => VoxelSurfaceAlphaModeDescriptor::Mask { cutoff },
        VoxelAlphaMode::Blend => VoxelSurfaceAlphaModeDescriptor::Blend,
    };
    let mapping = match &surface.mapping {
        ResolvedVoxelSurfaceMapping::Repeat {
            texture,
            texture_version,
            tile_scale_cells,
            tile_origin_cells,
        } => VoxelSurfaceMappingDescriptor::Repeat {
            texture: texture.id().as_str().to_string(),
            texture_version: *texture_version,
            texture_content_hash: reference_hash(texture)?,
            tile_scale_cells: *tile_scale_cells,
            tile_origin_cells: *tile_origin_cells,
        },
        ResolvedVoxelSurfaceMapping::Atlas {
            atlas,
            atlas_version,
            texture,
            texture_version,
            region,
            tile_scale_cells,
            tile_origin_cells,
        } => VoxelSurfaceMappingDescriptor::Atlas {
            atlas: atlas.id().as_str().to_string(),
            atlas_version: *atlas_version,
            atlas_content_hash: reference_hash(atlas)?,
            texture: texture.id().as_str().to_string(),
            texture_version: *texture_version,
            texture_content_hash: reference_hash(texture)?,
            region: VoxelAtlasRegionDescriptor {
                id: region.id.clone(),
                content_min: region.content_min,
                content_extent: region.content_extent,
                padding: VoxelAtlasPaddingDescriptor {
                    left: region.padding.left,
                    right: region.padding.right,
                    bottom: region.padding.bottom,
                    top: region.padding.top,
                },
                inset: "halfTexel".to_string(),
            },
            tile_scale_cells: *tile_scale_cells,
            tile_origin_cells: *tile_origin_cells,
        },
    };
    Ok(VoxelSurfaceDescriptor {
        schema_version: surface.schema_version,
        filter: match surface.filter {
            CatalogTextureFilter::Nearest => TextureFilter::Nearest,
            CatalogTextureFilter::Linear => TextureFilter::Linear,
        },
        wrap: match surface.wrap {
            CatalogTextureWrap::Clamp => TextureWrap::Clamp,
            CatalogTextureWrap::Repeat => TextureWrap::Repeat,
        },
        alpha_mode,
        mapping,
    })
}

fn surface_texture_id(surface: &ResolvedVoxelSurface) -> &str {
    match &surface.mapping {
        ResolvedVoxelSurfaceMapping::Repeat { texture, .. }
        | ResolvedVoxelSurfaceMapping::Atlas { texture, .. } => texture.id().as_str(),
    }
}

fn reference_hash(
    reference: &core_assets::AssetReference,
) -> Result<String, CatalogMaterialProjectionError> {
    reference
        .hash()
        .map(|hash| hash.as_str().to_string())
        .ok_or(CatalogMaterialProjectionError::MissingContentHash)
}

fn rgba(value: asset_catalog::Rgba) -> [f32; 4] {
    [value.r, value.g, value.b, value.a]
}
