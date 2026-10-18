use asset_catalog::{
    AdmittedAssetCatalog, ResolvedVoxelSurface, ResolvedVoxelSurfaceMapping,
    TextureFilter as CatalogTextureFilter, TextureWrap as CatalogTextureWrap, VoxelAlphaMode,
};
use core_assets::AssetId;
use render_model::{
    MaterialUvStrategy, RenderMaterialDescriptor, TextureFilter, TextureWrap,
    VoxelAtlasPaddingDescriptor, VoxelAtlasRegionDescriptor, VoxelSurfaceAlphaModeDescriptor,
    VoxelSurfaceDescriptor, VoxelSurfaceMappingDescriptor,
};

#[derive(Debug, Clone, PartialEq)]
pub enum CatalogMaterialProjectionError {
    MissingMaterial,
    UnresolvedSurface,
    MissingContentHash,
    InvalidSha256ContentHash,
}

/// One material of an admitted catalog, as the renderer draws it. The
/// catalog was validated when it was admitted; projecting one material
/// reads only that material and what it resolves to.
pub fn project_catalog_material(
    catalog: &AdmittedAssetCatalog,
    material_id: &AssetId,
) -> Result<RenderMaterialDescriptor, CatalogMaterialProjectionError> {
    let catalog = catalog.catalog();
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
        texture_transform: None,
        stochastic_tiling: None,
        terrain_layers: None,
        shader: None,
        id: entry.0.id.as_str().to_string(),
        color: rgba(entry.1.style.color),
        texture,
        roughness: entry.1.style.roughness,
        metalness: 0.0,
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
        alpha_mode: Default::default(),
        double_sided: false,
        voxel_surface,
        normal_map: None,
        triplanar: None,
        emission_map: None,
        occlusion_map: None,
        unlit: false,
        flat_shading: false,
        keep_dry: false,
        sinks_under_snow: false,
        wind: None,
        water: None,
        translucent_shadow: false,
    };
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
            texture_hash,
            tile_scale_cells,
            tile_origin_cells,
        } => VoxelSurfaceMappingDescriptor::Repeat {
            texture: texture.id().as_str().to_string(),
            texture_version: *texture_version,
            texture_content_hash: content_hash(texture_hash.as_ref())?,
            tile_scale_cells: *tile_scale_cells,
            tile_origin_cells: *tile_origin_cells,
        },
        ResolvedVoxelSurfaceMapping::Atlas {
            atlas,
            atlas_version,
            atlas_hash,
            texture,
            texture_version,
            texture_hash,
            region,
            tile_scale_cells,
            tile_origin_cells,
        } => VoxelSurfaceMappingDescriptor::Atlas {
            atlas: atlas.id().as_str().to_string(),
            atlas_version: *atlas_version,
            atlas_content_hash: content_hash(atlas_hash.as_ref())?,
            texture: texture.id().as_str().to_string(),
            texture_version: *texture_version,
            texture_content_hash: content_hash(texture_hash.as_ref())?,
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
            CatalogTextureFilter::NearestMipmapped => TextureFilter::NearestMipmapped,
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

/// The content hash of the entry a surface reference resolved to.
fn content_hash(
    hash: Option<&core_assets::AssetHash>,
) -> Result<String, CatalogMaterialProjectionError> {
    let digest = hash
        .map(core_assets::AssetHash::as_str)
        .ok_or(CatalogMaterialProjectionError::MissingContentHash)?;
    if digest.len() != 64 {
        return Err(CatalogMaterialProjectionError::InvalidSha256ContentHash);
    }
    Ok(format!("sha256:{digest}"))
}

fn rgba(value: asset_catalog::Rgba) -> [f32; 4] {
    [value.r, value.g, value.b, value.a]
}
