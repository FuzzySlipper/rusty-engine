use asset_catalog::{
    AssetCatalog, AtlasInset, AtlasPadding, AtlasRegionDefinition, CatalogEntry, MaterialAuthority,
    MaterialDefinition, MaterialStyle, Rgba, StructuralClass, TextureDefinition, TextureFilter,
    TextureWrap, UvStrategy, VoxelAlphaMode, VoxelAtlasDefinition, VoxelSurfaceBinding,
    VoxelSurfaceMapping,
};
use core_assets::{AssetHash, AssetId, AssetReference, AssetVersionReq};
use render_model::{MaterialUvStrategy, VoxelSurfaceMappingDescriptor};
use render_projection::{project_catalog_material, CatalogMaterialProjectionError};

const TEXTURE_HASH: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const ATLAS_HASH: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const MATERIAL_HASH: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

fn id(value: &str) -> AssetId {
    AssetId::parse(value).unwrap()
}

fn pinned(value: &str, version: u32, hash: &str) -> AssetReference {
    AssetReference::new(
        id(value),
        AssetVersionReq::Exact(version),
        Some(AssetHash::parse(hash).unwrap()),
    )
}

fn catalog() -> AssetCatalog {
    catalog_with_hashes(TEXTURE_HASH, ATLAS_HASH)
}

fn catalog_with_hashes(texture_hash: &str, atlas_hash: &str) -> AssetCatalog {
    let texture = pinned("texture/voxel-atlas", 2, texture_hash);
    let atlas = pinned("sprite-sheet/voxel-atlas", 3, atlas_hash);
    let texture_entry = CatalogEntry::new(texture.id().clone(), 2)
        .with_hash(AssetHash::parse(texture_hash).unwrap())
        .with_texture(TextureDefinition {
            width: 32,
            height: 32,
            filter: TextureFilter::Linear,
            wrap: TextureWrap::Clamp,
        });
    let atlas_entry = CatalogEntry::new(atlas.id().clone(), 3)
        .with_hash(AssetHash::parse(atlas_hash).unwrap())
        .with_dependencies(vec![texture.clone()])
        .with_voxel_atlas(VoxelAtlasDefinition {
            schema_version: 1,
            texture: texture.clone(),
            regions: vec![AtlasRegionDefinition {
                id: "stone".to_string(),
                content_min: [1, 1],
                content_extent: [30, 30],
                padding: AtlasPadding::ONE,
                inset: AtlasInset::HalfTexel,
            }],
        });
    let mut style = MaterialStyle::flat(Rgba::WHITE);
    style.texture = Some(texture);
    style.uv_strategy = UvStrategy::Atlas;
    style.voxel_surface = Some(VoxelSurfaceBinding {
        schema_version: 1,
        mapping: VoxelSurfaceMapping::Atlas {
            atlas: atlas.clone(),
            region: "stone".to_string(),
            tile_scale_cells: [1.0, 2.0],
            tile_origin_cells: [-4.0, 8.0],
        },
        alpha_mode: VoxelAlphaMode::Blend,
    });
    let material = CatalogEntry::new(id("material/stone"), 1)
        .with_hash(AssetHash::parse(MATERIAL_HASH).unwrap())
        .with_dependencies(vec![atlas])
        .with_material(MaterialDefinition {
            authority: MaterialAuthority {
                solid: true,
                collidable: true,
                occludes: true,
                structural_class: StructuralClass::Structural,
            },
            style,
        });
    AssetCatalog::from_entries(vec![material, atlas_entry, texture_entry])
}

#[test]
fn canonical_surface_projects_exact_renderer_provenance_without_voxel_authority() {
    let descriptor = project_catalog_material(&catalog(), &id("material/stone")).unwrap();
    assert_eq!(descriptor.id, "material/stone");
    assert_eq!(descriptor.texture.as_deref(), Some("texture/voxel-atlas"));
    assert_eq!(descriptor.uv_strategy, MaterialUvStrategy::Atlas);
    let surface = descriptor.voxel_surface.unwrap();
    match surface.mapping {
        VoxelSurfaceMappingDescriptor::Atlas {
            atlas,
            atlas_version,
            atlas_content_hash,
            texture_version,
            texture_content_hash,
            region,
            ..
        } => {
            assert_eq!(atlas, "sprite-sheet/voxel-atlas");
            assert_eq!(atlas_version, 3);
            assert_eq!(atlas_content_hash, format!("sha256:{ATLAS_HASH}"));
            assert_eq!(texture_version, 2);
            assert_eq!(texture_content_hash, format!("sha256:{TEXTURE_HASH}"));
            assert_eq!(region.id, "stone");
        }
        other => panic!("unexpected mapping: {other:?}"),
    }
}

#[test]
fn algorithm_ambiguous_surface_hash_never_reaches_the_renderer_contract() {
    let candidate = catalog_with_hashes("aa02", "bb03");
    assert!(matches!(
        project_catalog_material(&candidate, &id("material/stone")),
        Err(CatalogMaterialProjectionError::InvalidSha256ContentHash)
    ));
}

#[test]
fn invalid_candidate_never_projects_or_mutates_the_admitted_catalog() {
    let admitted = catalog();
    let mut candidate = admitted.clone();
    candidate
        .entries
        .iter_mut()
        .find(|entry| entry.id.as_str() == "texture/voxel-atlas")
        .unwrap()
        .hash = Some(AssetHash::parse("ffff").unwrap());
    assert!(matches!(
        project_catalog_material(&candidate, &id("material/stone")),
        Err(CatalogMaterialProjectionError::InvalidCatalog { .. })
    ));
    assert_eq!(
        project_catalog_material(&admitted, &id("material/stone"))
            .unwrap()
            .texture
            .as_deref(),
        Some("texture/voxel-atlas")
    );
}

#[test]
fn conflicting_legacy_texture_never_projects_over_surface_authority() {
    let mut candidate = catalog();
    candidate
        .entries
        .iter_mut()
        .find(|entry| entry.id.as_str() == "material/stone")
        .unwrap()
        .material
        .as_mut()
        .unwrap()
        .style
        .texture = Some(pinned("texture/conflicting", 1, "ffff"));
    assert!(matches!(
        project_catalog_material(&candidate, &id("material/stone")),
        Err(CatalogMaterialProjectionError::InvalidCatalog { codes })
            if codes.iter().any(|code| code == "conflicting_surface_texture")
    ));
}
