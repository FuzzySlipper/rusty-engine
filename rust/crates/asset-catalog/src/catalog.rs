use core_assets::{AssetHash, AssetId, AssetKind, AssetReference, AssetVersionReq};

use crate::{
    MaterialDefinition, RenderMaterial, ResolvedVoxelSurface, ResolvedVoxelSurfaceMapping,
    TextureDefinition, VoxelAtlasDefinition, VoxelSurfaceBinding, VoxelSurfaceMapping,
    VoxelSurfaceResolutionError,
};

/// One authored asset definition.
#[derive(Debug, Clone, PartialEq)]
pub struct CatalogEntry {
    pub id: AssetId,
    pub version: u32,
    pub hash: Option<AssetHash>,
    /// Source location is metadata, never stable identity.
    pub source_path: Option<String>,
    pub label: Option<String>,
    pub dependencies: Vec<AssetReference>,
    /// Required for material IDs and rejected for every other asset kind.
    pub material: Option<MaterialDefinition>,
    /// Optional canonical texture facts. Legacy texture entries may omit this.
    pub texture: Option<TextureDefinition>,
    /// Optional voxel atlas facts, owned by a sprite-sheet asset.
    pub voxel_atlas: Option<VoxelAtlasDefinition>,
}

impl CatalogEntry {
    pub fn new(id: AssetId, version: u32) -> Self {
        Self {
            id,
            version,
            hash: None,
            source_path: None,
            label: None,
            dependencies: Vec::new(),
            material: None,
            texture: None,
            voxel_atlas: None,
        }
    }

    pub fn kind(&self) -> AssetKind {
        self.id.kind()
    }

    pub fn with_hash(mut self, hash: AssetHash) -> Self {
        self.hash = Some(hash);
        self
    }

    pub fn with_source(mut self, path: impl Into<String>) -> Self {
        self.source_path = Some(path.into());
        self
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn with_dependencies(mut self, dependencies: Vec<AssetReference>) -> Self {
        self.dependencies = dependencies;
        self
    }

    pub fn with_material(mut self, material: MaterialDefinition) -> Self {
        self.material = Some(material);
        self
    }

    pub fn with_texture(mut self, texture: TextureDefinition) -> Self {
        self.texture = Some(texture);
        self
    }

    pub fn with_voxel_atlas(mut self, atlas: VoxelAtlasDefinition) -> Self {
        self.voxel_atlas = Some(atlas);
        self
    }
}

/// Authored asset definitions. Construction is intentionally separate from
/// validation so decoders can return complete classified reports.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AssetCatalog {
    pub entries: Vec<CatalogEntry>,
}

impl AssetCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_entries(entries: Vec<CatalogEntry>) -> Self {
        Self { entries }
    }

    pub fn get(&self, id: &AssetId) -> Option<&CatalogEntry> {
        self.entries.iter().find(|entry| &entry.id == id)
    }

    pub fn contains(&self, id: &AssetId) -> bool {
        self.get(id).is_some()
    }

    pub fn iter(&self) -> impl Iterator<Item = &CatalogEntry> {
        self.entries.iter()
    }

    pub fn resolve_voxel_surface(
        &self,
        surface: &VoxelSurfaceBinding,
    ) -> Result<ResolvedVoxelSurface, VoxelSurfaceResolutionError> {
        match &surface.mapping {
            VoxelSurfaceMapping::Repeat {
                texture,
                tile_scale_cells,
                tile_origin_cells,
            } => {
                let texture_entry = self.resolve_exact(texture)?;
                let texture_definition = texture_entry
                    .texture
                    .as_ref()
                    .ok_or(VoxelSurfaceResolutionError::MissingTextureDefinition)?;
                Ok(ResolvedVoxelSurface {
                    schema_version: surface.schema_version,
                    filter: texture_definition.filter,
                    wrap: texture_definition.wrap,
                    alpha_mode: surface.alpha_mode,
                    mapping: ResolvedVoxelSurfaceMapping::Repeat {
                        texture: texture.clone(),
                        texture_version: texture_entry.version,
                        tile_scale_cells: *tile_scale_cells,
                        tile_origin_cells: *tile_origin_cells,
                    },
                })
            }
            VoxelSurfaceMapping::Atlas {
                atlas,
                region,
                tile_scale_cells,
                tile_origin_cells,
            } => {
                let atlas_entry = self.resolve_exact(atlas)?;
                let atlas_definition = atlas_entry
                    .voxel_atlas
                    .as_ref()
                    .ok_or(VoxelSurfaceResolutionError::MissingAtlasDefinition)?;
                let texture_entry = self.resolve_exact(&atlas_definition.texture)?;
                let texture_definition = texture_entry
                    .texture
                    .as_ref()
                    .ok_or(VoxelSurfaceResolutionError::MissingTextureDefinition)?;
                let region = atlas_definition
                    .regions
                    .iter()
                    .find(|candidate| candidate.id == *region)
                    .cloned()
                    .ok_or(VoxelSurfaceResolutionError::MissingAtlasRegion)?;
                Ok(ResolvedVoxelSurface {
                    schema_version: surface.schema_version,
                    filter: texture_definition.filter,
                    wrap: texture_definition.wrap,
                    alpha_mode: surface.alpha_mode,
                    mapping: ResolvedVoxelSurfaceMapping::Atlas {
                        atlas: atlas.clone(),
                        atlas_version: atlas_entry.version,
                        texture: atlas_definition.texture.clone(),
                        texture_version: texture_entry.version,
                        region,
                        tile_scale_cells: *tile_scale_cells,
                        tile_origin_cells: *tile_origin_cells,
                    },
                })
            }
        }
    }

    pub fn render_material(
        &self,
        id: &AssetId,
    ) -> Result<RenderMaterial, VoxelSurfaceResolutionError> {
        let entry = self
            .get(id)
            .ok_or(VoxelSurfaceResolutionError::MissingAsset)?;
        let definition = entry
            .material
            .as_ref()
            .ok_or(VoxelSurfaceResolutionError::MissingAsset)?;
        match &definition.style.voxel_surface {
            Some(surface) => {
                Ok(definition.render_projection_with_surface(self.resolve_voxel_surface(surface)?))
            }
            None => Ok(definition.render_projection()),
        }
    }

    fn resolve_exact(
        &self,
        reference: &AssetReference,
    ) -> Result<&CatalogEntry, VoxelSurfaceResolutionError> {
        let entry = self
            .get(reference.id())
            .ok_or(VoxelSurfaceResolutionError::MissingAsset)?;
        let version_matches = match reference.version() {
            AssetVersionReq::Any => true,
            AssetVersionReq::Exact(version) => entry.version == version,
            AssetVersionReq::AtLeast(version) => entry.version >= version,
        };
        if !version_matches
            || reference
                .hash()
                .is_some_and(|hash| entry.hash.as_ref() != Some(hash))
        {
            return Err(VoxelSurfaceResolutionError::StaleReference);
        }
        Ok(entry)
    }

    /// A deterministic copy. Entry identity controls order; dependency order is
    /// normalized without changing authored multiplicity so validation can still
    /// report the original semantic content.
    pub fn canonical(&self) -> Self {
        let mut catalog = self.clone();
        catalog
            .entries
            .sort_by(|left, right| left.id.as_str().cmp(right.id.as_str()));
        for entry in &mut catalog.entries {
            entry.dependencies.sort_by(|left, right| {
                left.id()
                    .as_str()
                    .cmp(right.id().as_str())
                    .then_with(|| version_key(left.version()).cmp(&version_key(right.version())))
                    .then_with(|| {
                        left.hash()
                            .map(AssetHash::as_str)
                            .cmp(&right.hash().map(AssetHash::as_str))
                    })
            });
            if let Some(atlas) = &mut entry.voxel_atlas {
                atlas.regions.sort_by(|left, right| left.id.cmp(&right.id));
            }
        }
        catalog
    }
}

fn version_key(requirement: AssetVersionReq) -> (u8, u32) {
    match requirement {
        AssetVersionReq::Any => (0, 0),
        AssetVersionReq::Exact(version) => (1, version),
        AssetVersionReq::AtLeast(version) => (2, version),
    }
}
