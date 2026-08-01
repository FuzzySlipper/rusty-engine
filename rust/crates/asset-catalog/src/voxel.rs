use std::collections::{BTreeMap, BTreeSet};

use core_assets::AssetId;
use core_voxel::VoxelMaterialId;

use crate::{
    AssetCatalog, CollisionMaterial, MaterialDefinition, RenderMaterial, Rgba, UvStrategy,
};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct VoxelMaterialTable {
    by_id: BTreeMap<u16, AssetId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoxelMaterialError {
    Unmapped(VoxelMaterialId),
    NotAMaterial { id: VoxelMaterialId, asset: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct VoxelRenderResolution {
    pub material: RenderMaterial,
    pub used_fallback: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct VoxelMaterialTableReport {
    pub unresolved: Vec<VoxelMaterialError>,
}

impl VoxelMaterialTableReport {
    pub fn is_collision_safe(&self) -> bool {
        self.unresolved.is_empty()
    }
}

impl VoxelMaterialTable {
    pub fn from_pairs(pairs: impl IntoIterator<Item = (VoxelMaterialId, AssetId)>) -> Self {
        Self {
            by_id: pairs
                .into_iter()
                .map(|(id, asset)| (id.raw(), asset))
                .collect(),
        }
    }

    pub fn material_asset(&self, id: VoxelMaterialId) -> Option<&AssetId> {
        self.by_id.get(&id.raw())
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    pub fn render_material(
        &self,
        catalog: &AssetCatalog,
        id: VoxelMaterialId,
    ) -> VoxelRenderResolution {
        match self
            .material_asset(id)
            .ok_or(VoxelMaterialError::Unmapped(id))
            .and_then(|asset| {
                catalog
                    .render_material(asset)
                    .map_err(|_| VoxelMaterialError::NotAMaterial {
                        id,
                        asset: asset.as_str().to_string(),
                    })
            }) {
            Ok(material) => VoxelRenderResolution {
                material,
                used_fallback: false,
            },
            Err(_) => VoxelRenderResolution {
                material: fallback_render_material(),
                used_fallback: true,
            },
        }
    }

    pub fn collision_material(
        &self,
        catalog: &AssetCatalog,
        id: VoxelMaterialId,
    ) -> Result<CollisionMaterial, VoxelMaterialError> {
        self.material_definition(catalog, id)
            .map(MaterialDefinition::collision_projection)
    }

    pub fn validate_used(
        &self,
        catalog: &AssetCatalog,
        used: impl IntoIterator<Item = VoxelMaterialId>,
    ) -> VoxelMaterialTableReport {
        let mut seen = BTreeSet::new();
        let mut unresolved = Vec::new();
        for id in used {
            if seen.insert(id.raw()) {
                if let Err(error) = self.material_definition(catalog, id) {
                    unresolved.push(error);
                }
            }
        }
        VoxelMaterialTableReport { unresolved }
    }

    fn material_definition<'a>(
        &self,
        catalog: &'a AssetCatalog,
        id: VoxelMaterialId,
    ) -> Result<&'a MaterialDefinition, VoxelMaterialError> {
        let asset = self
            .material_asset(id)
            .ok_or(VoxelMaterialError::Unmapped(id))?;
        catalog
            .get(asset)
            .and_then(|entry| entry.material.as_ref())
            .ok_or_else(|| VoxelMaterialError::NotAMaterial {
                id,
                asset: asset.as_str().to_string(),
            })
    }
}

fn fallback_render_material() -> RenderMaterial {
    RenderMaterial {
        color: Rgba::DEBUG_GREY,
        texture: None,
        roughness: 1.0,
        texture_tint: Rgba::WHITE,
        emission_color: Rgba::DEBUG_GREY,
        emissive: 0.0,
        uv_strategy: UvStrategy::Flat,
        voxel_surface: None,
    }
}
