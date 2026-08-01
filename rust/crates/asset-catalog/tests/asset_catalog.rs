use asset_catalog::{
    classify_material_change, decode_catalog, decode_lock, encode_catalog, encode_lock,
    fallback_for, generate_lock, material_change_impact, revalidate_asset, validate_catalog,
    validate_lock, AdmittedAssetCatalog, AssetCatalog, AssetCatalogStore, AssetContext, AtlasInset,
    AtlasPadding, AtlasRegionDefinition, CatalogEntry, CatalogValidationError, ChangeKind,
    DependencyGraph, FallbackOutcome, FallbackVisual, LockIssue, MaterialAuthority,
    MaterialDefinition, MaterialStyle, ReloadSuggestion, ResolvedVoxelSurfaceMapping, Rgba,
    StructuralClass, TextureDefinition, TextureFilter, TextureWrap, UvStrategy, VoxelAlphaMode,
    VoxelAtlasDefinition, VoxelMaterialError, VoxelMaterialTable, VoxelSurfaceBinding,
    VoxelSurfaceMapping,
};
use core_assets::{AssetHash, AssetId, AssetKind, AssetReference, AssetVersionReq};
use core_voxel::VoxelMaterialId;

fn id(value: &str) -> AssetId {
    AssetId::parse(value).expect("fixture asset id")
}

fn reference(value: &str) -> AssetReference {
    AssetReference::new(id(value), AssetVersionReq::Any, None)
}

fn pinned(value: &str, version: u32, hash: &str) -> AssetReference {
    AssetReference::new(
        id(value),
        AssetVersionReq::Exact(version),
        Some(AssetHash::parse(hash).expect("fixture hash")),
    )
}

fn material(structural_class: StructuralClass, color: Rgba) -> MaterialDefinition {
    MaterialDefinition {
        authority: MaterialAuthority {
            solid: true,
            collidable: true,
            occludes: true,
            structural_class,
        },
        style: MaterialStyle::flat(color),
    }
}

fn sample_catalog() -> AssetCatalog {
    let texture = CatalogEntry::new(id("texture/surface-atlas-a"), 1)
        .with_hash(AssetHash::parse("aa01").expect("fixture hash"))
        .with_source("textures/surface-atlas-a.png");
    let mut definition = material(StructuralClass::Structural, Rgba::DEBUG_GREY);
    definition.style.texture = Some(reference("texture/surface-atlas-a"));
    let surface = CatalogEntry::new(id("material/surface-a"), 2)
        .with_hash(AssetHash::parse("bb02").expect("fixture hash"))
        .with_material(definition)
        .with_dependencies(vec![reference("texture/surface-atlas-a")]);
    let mesh = CatalogEntry::new(id("mesh/fixture-a"), 1)
        .with_hash(AssetHash::parse("cc03").expect("fixture hash"))
        .with_dependencies(vec![reference("material/surface-a")]);
    AssetCatalog::from_entries(vec![mesh, surface, texture])
}

fn surface_catalog() -> AssetCatalog {
    let tile_ref = pinned("texture/stone-tile", 2, "aa02");
    let atlas_texture_ref = pinned("texture/voxel-atlas", 3, "bb03");
    let atlas_ref = pinned("sprite-sheet/voxel-atlas", 1, "cc01");

    let tile_texture = CatalogEntry::new(id("texture/stone-tile"), 2)
        .with_hash(AssetHash::parse("aa02").unwrap())
        .with_texture(TextureDefinition {
            width: 32,
            height: 32,
            filter: TextureFilter::Nearest,
            wrap: TextureWrap::Repeat,
        });
    let atlas_texture = CatalogEntry::new(id("texture/voxel-atlas"), 3)
        .with_hash(AssetHash::parse("bb03").unwrap())
        .with_texture(TextureDefinition {
            width: 64,
            height: 32,
            filter: TextureFilter::Linear,
            wrap: TextureWrap::Clamp,
        });
    let atlas = CatalogEntry::new(id("sprite-sheet/voxel-atlas"), 1)
        .with_hash(AssetHash::parse("cc01").unwrap())
        .with_dependencies(vec![atlas_texture_ref.clone()])
        .with_voxel_atlas(VoxelAtlasDefinition {
            schema_version: 1,
            texture: atlas_texture_ref,
            regions: vec![
                AtlasRegionDefinition {
                    id: "moss".to_string(),
                    content_min: [34, 2],
                    content_extent: [28, 28],
                    padding: AtlasPadding::ONE,
                    inset: AtlasInset::HalfTexel,
                },
                AtlasRegionDefinition {
                    id: "stone".to_string(),
                    content_min: [2, 2],
                    content_extent: [28, 28],
                    padding: AtlasPadding::ONE,
                    inset: AtlasInset::HalfTexel,
                },
            ],
        });
    let mut repeating = material(StructuralClass::Solid, Rgba::WHITE);
    repeating.style.texture = Some(tile_ref.clone());
    repeating.style.uv_strategy = UvStrategy::Planar;
    repeating.style.voxel_surface = Some(VoxelSurfaceBinding {
        schema_version: 1,
        mapping: VoxelSurfaceMapping::Repeat {
            texture: tile_ref.clone(),
            tile_scale_cells: [0.5, 2.0],
            tile_origin_cells: [-4.0, 8.0],
        },
        alpha_mode: VoxelAlphaMode::Opaque,
    });
    let repeating = CatalogEntry::new(id("material/repeating-stone"), 1)
        .with_hash(AssetHash::parse("dd01").unwrap())
        .with_dependencies(vec![tile_ref])
        .with_material(repeating);

    let mut atlas_material = material(StructuralClass::Solid, Rgba::WHITE);
    atlas_material.style.texture = Some(pinned("texture/voxel-atlas", 3, "bb03"));
    atlas_material.style.uv_strategy = UvStrategy::Atlas;
    atlas_material.style.voxel_surface = Some(VoxelSurfaceBinding {
        schema_version: 1,
        mapping: VoxelSurfaceMapping::Atlas {
            atlas: atlas_ref.clone(),
            region: "stone".to_string(),
            tile_scale_cells: [1.0, 1.0],
            tile_origin_cells: [0.0, 0.0],
        },
        alpha_mode: VoxelAlphaMode::Mask { cutoff: 0.5 },
    });
    let atlas_material = CatalogEntry::new(id("material/atlas-stone"), 1)
        .with_hash(AssetHash::parse("ee01").unwrap())
        .with_dependencies(vec![atlas_ref])
        .with_material(atlas_material);

    AssetCatalog::from_entries(vec![
        atlas_material,
        atlas,
        tile_texture,
        repeating,
        atlas_texture,
    ])
}

fn quota_atlas(name: &str, region_count: usize) -> CatalogEntry {
    let texture = pinned("texture/quota-atlas", 1, "aa01");
    CatalogEntry::new(id(&format!("sprite-sheet/{name}")), 1)
        .with_dependencies(vec![texture.clone()])
        .with_voxel_atlas(VoxelAtlasDefinition {
            schema_version: 1,
            texture,
            regions: (0..region_count)
                .map(|index| AtlasRegionDefinition {
                    id: format!("r-{index:04}"),
                    content_min: [index as u32, 0],
                    content_extent: [1, 1],
                    padding: AtlasPadding::ZERO,
                    inset: AtlasInset::HalfTexel,
                })
                .collect(),
        })
}

#[test]
fn catalog_validation_preserves_classified_dependency_and_material_failures() {
    assert!(validate_catalog(&sample_catalog()).is_ok());

    let mut invalid = sample_catalog();
    invalid
        .entries
        .push(CatalogEntry::new(id("mesh/fixture-a"), 9));
    invalid.entries.push(
        CatalogEntry::new(id("mesh/missing-dependency"), 1)
            .with_dependencies(vec![reference("material/absent")]),
    );
    invalid
        .entries
        .push(CatalogEntry::new(id("material/no-payload"), 1));
    invalid.entries.push(
        CatalogEntry::new(id("sprite/material-payload"), 1)
            .with_material(material(StructuralClass::Solid, Rgba::DEBUG_MAGENTA)),
    );
    invalid.entries[1]
        .material
        .as_mut()
        .expect("material")
        .style
        .texture = Some(reference("material/surface-a"));

    let report = validate_catalog(&invalid);
    assert!(report.errors.iter().any(|error| matches!(
        error,
        CatalogValidationError::DuplicateAssetId { id } if id.as_str() == "mesh/fixture-a"
    )));
    assert!(report.errors.iter().any(|error| matches!(
        error,
        CatalogValidationError::UnknownDependency { dependency, .. }
            if dependency.as_str() == "material/absent"
    )));
    assert!(report
        .errors
        .iter()
        .any(|error| matches!(error, CatalogValidationError::MaterialPayloadMissing { .. })));
    assert!(report.errors.iter().any(|error| matches!(
        error,
        CatalogValidationError::MaterialPayloadOnNonMaterial { .. }
    )));
    assert!(report.errors.iter().any(|error| matches!(
        error,
        CatalogValidationError::WrongKindReference {
            expected: AssetKind::Texture,
            actual: AssetKind::Material,
            ..
        }
    )));
    assert_eq!(report.diagnostics().len(), report.errors.len());
}

#[test]
fn dependency_cycles_and_reverse_dependents_are_deterministic() {
    let cycle = AssetCatalog::from_entries(vec![
        CatalogEntry::new(id("mesh/a"), 1).with_dependencies(vec![reference("material/b")]),
        CatalogEntry::new(id("material/b"), 1)
            .with_material(material(StructuralClass::Solid, Rgba::DEBUG_GREY))
            .with_dependencies(vec![reference("texture/c")]),
        CatalogEntry::new(id("texture/c"), 1).with_dependencies(vec![reference("mesh/a")]),
    ]);
    let path = DependencyGraph::build(&cycle)
        .detect_cycle()
        .expect("cycle path");
    assert_eq!(path.first(), path.last());
    assert_eq!(path.len(), 4);
    assert!(validate_catalog(&cycle)
        .errors
        .iter()
        .any(|error| matches!(error, CatalogValidationError::DependencyCycle { .. })));

    let dependents = DependencyGraph::build(&sample_catalog())
        .dependents_of(&id("texture/surface-atlas-a"))
        .into_iter()
        .map(|asset| asset.as_str().to_string())
        .collect::<Vec<_>>();
    assert_eq!(dependents, ["material/surface-a", "mesh/fixture-a"]);
}

#[test]
fn canonical_authored_json_is_a_strict_fixed_point() {
    let encoded = encode_catalog(&sample_catalog()).expect("encode catalog");
    let restored = decode_catalog(&encoded).expect("decode catalog");
    assert_eq!(restored, sample_catalog().canonical());
    assert_eq!(encode_catalog(&restored).expect("re-encode"), encoded);
    assert!(encoded.ends_with('\n'));
    assert!(encoded.find("mesh/fixture-a") > encoded.find("material/surface-a"));
    assert!(!encoded.contains("voxelSurface"));
    assert!(!encoded.contains("voxelAtlas"));

    let unknown_top = encoded.replacen("\"entries\": [", "\"mystery\": true,\n  \"entries\": [", 1);
    let error = decode_catalog(&unknown_top).expect_err("unknown top-level field");
    assert!(error.message.contains("unknown field"));

    let unknown_nested = encoded.replacen(
        "\"solid\": true,",
        "\"solid\": true,\n          \"mystery\": true,",
        1,
    );
    let error = decode_catalog(&unknown_nested).expect_err("unknown nested field");
    assert!(error.message.contains("unknown field"));

    let error = decode_catalog(&format!("{encoded} true"))
        .expect_err("trailing values must not be ignored");
    assert_eq!(error.path, "$");
}

#[test]
fn voxel_surface_catalog_is_canonical_and_resolves_immutable_provenance() {
    let catalog = surface_catalog();
    assert!(validate_catalog(&catalog).is_ok());
    let encoded = encode_catalog(&catalog).expect("encode textured catalog");
    let restored = decode_catalog(&encoded).expect("decode textured catalog");
    assert_eq!(restored, catalog.canonical());
    assert_eq!(encode_catalog(&restored).unwrap(), encoded);
    assert!(encoded.find("\"id\": \"moss\"") < encoded.find("\"id\": \"stone\""));

    let resolved = restored
        .render_material(&id("material/atlas-stone"))
        .expect("resolve atlas material")
        .voxel_surface
        .expect("resolved surface");
    assert_eq!(resolved.filter, TextureFilter::Linear);
    assert_eq!(resolved.wrap, TextureWrap::Clamp);
    match resolved.mapping {
        ResolvedVoxelSurfaceMapping::Atlas {
            atlas_version,
            texture_version,
            region,
            ..
        } => {
            assert_eq!(atlas_version, 1);
            assert_eq!(texture_version, 3);
            assert_eq!(region.id, "stone");
            assert_eq!(region.content_min, [2, 2]);
        }
        other => panic!("unexpected mapping: {other:?}"),
    }
}

#[test]
fn voxel_surface_dependency_and_atlas_failures_reject_before_resolution() {
    let original = surface_catalog();

    let mut stale = original.clone();
    stale
        .entries
        .iter_mut()
        .find(|entry| entry.id.as_str() == "texture/stone-tile")
        .unwrap()
        .hash = Some(AssetHash::parse("ffff").unwrap());
    assert!(validate_catalog(&stale)
        .errors
        .iter()
        .any(|error| matches!(error, CatalogValidationError::StaleDependencyHash { .. })));
    assert_eq!(
        original,
        surface_catalog(),
        "candidate validation is immutable"
    );

    let mut duplicate_dependency = original.clone();
    let material = duplicate_dependency
        .entries
        .iter_mut()
        .find(|entry| entry.id.as_str() == "material/repeating-stone")
        .unwrap();
    material.dependencies.push(material.dependencies[0].clone());
    assert!(validate_catalog(&duplicate_dependency)
        .errors
        .iter()
        .any(|error| matches!(error, CatalogValidationError::DuplicateDependency { .. })));

    let mut invalid_atlas = original.clone();
    invalid_atlas
        .entries
        .iter_mut()
        .find(|entry| entry.id.as_str() == "sprite-sheet/voxel-atlas")
        .unwrap()
        .voxel_atlas
        .as_mut()
        .unwrap()
        .regions[1]
        .content_min = [30, 2];
    assert!(validate_catalog(&invalid_atlas)
        .errors
        .iter()
        .any(|error| matches!(error, CatalogValidationError::AtlasRegionOverlap { .. })));
    invalid_atlas
        .entries
        .iter_mut()
        .find(|entry| entry.id.as_str() == "sprite-sheet/voxel-atlas")
        .unwrap()
        .voxel_atlas
        .as_mut()
        .unwrap()
        .regions[1]
        .content_min = [63, 2];
    assert!(validate_catalog(&invalid_atlas)
        .errors
        .iter()
        .any(|error| matches!(error, CatalogValidationError::AtlasRegionOutOfBounds { .. })));

    let unsupported = encode_catalog(&original)
        .unwrap()
        .replacen("\"linear\"", "\"trilinear\"", 1);
    let error = decode_catalog(&unsupported).expect_err("unsupported filter");
    assert!(error.message.contains("unsupported texture filter"));
}

#[test]
fn atlas_padding_tile_and_region_quota_boundaries_are_typed() {
    let mut catalog = surface_catalog();
    let atlas_entry = catalog
        .entries
        .iter_mut()
        .find(|entry| entry.id.as_str() == "sprite-sheet/voxel-atlas")
        .unwrap();
    let atlas = atlas_entry.voxel_atlas.as_mut().unwrap();
    atlas.regions[0].padding.left = 0;
    assert!(validate_catalog(&catalog)
        .errors
        .iter()
        .any(|error| matches!(
            error,
            CatalogValidationError::InsufficientAtlasPadding { .. }
        )));

    let mut catalog = surface_catalog();
    {
        let material = catalog
            .entries
            .iter_mut()
            .find(|entry| entry.id.as_str() == "material/repeating-stone")
            .unwrap()
            .material
            .as_mut()
            .unwrap();
        if let VoxelSurfaceMapping::Repeat {
            tile_scale_cells, ..
        } = &mut material.style.voxel_surface.as_mut().unwrap().mapping
        {
            *tile_scale_cells = [1.0 / 256.0, 4_096.0];
        }
    }
    assert!(validate_catalog(&catalog).is_ok());
    {
        let material = catalog
            .entries
            .iter_mut()
            .find(|entry| entry.id.as_str() == "material/repeating-stone")
            .unwrap()
            .material
            .as_mut()
            .unwrap();
        if let VoxelSurfaceMapping::Repeat {
            tile_scale_cells, ..
        } = &mut material.style.voxel_surface.as_mut().unwrap().mapping
        {
            tile_scale_cells[0] = (1.0 / 256.0) / 2.0;
        }
    }
    assert!(validate_catalog(&catalog)
        .errors
        .iter()
        .any(|error| matches!(error, CatalogValidationError::InvalidTileScale { .. })));
}

#[test]
fn strict_reopen_and_catalog_replacement_are_atomic() {
    let admitted = AdmittedAssetCatalog::admit(surface_catalog()).unwrap();
    let reopened = AdmittedAssetCatalog::reopen(admitted.canonical_json()).unwrap();
    assert_eq!(reopened, admitted);
    assert!(admitted.canonical_hash().starts_with("sha256:"));
    assert_eq!(reopened.canonical_hash(), admitted.canonical_hash());
    let mut store = AssetCatalogStore::new(admitted);
    let before_json = store.current().canonical_json().to_string();
    let before_revision = store.revision();

    let mut invalid = surface_catalog();
    invalid
        .entries
        .iter_mut()
        .find(|entry| entry.id.as_str() == "material/atlas-stone")
        .unwrap()
        .dependencies
        .clear();
    assert!(store.replace(invalid).is_err());
    assert_eq!(store.revision(), before_revision);
    assert_eq!(store.current().canonical_json(), before_json);

    let mut valid = surface_catalog();
    valid
        .entries
        .iter_mut()
        .find(|entry| entry.id.as_str() == "material/atlas-stone")
        .unwrap()
        .label = Some("updated".to_string());
    assert_eq!(store.replace(valid).unwrap(), before_revision + 1);
    assert!(store.current().canonical_json().contains("updated"));
}

#[test]
fn atlas_per_asset_and_aggregate_region_quotas_have_exact_boundaries() {
    let texture = CatalogEntry::new(id("texture/quota-atlas"), 1)
        .with_hash(AssetHash::parse("aa01").unwrap())
        .with_texture(TextureDefinition {
            width: 4_096,
            height: 4_096,
            filter: TextureFilter::Nearest,
            wrap: TextureWrap::Clamp,
        });
    let exact = AssetCatalog::from_entries(vec![texture.clone(), quota_atlas("quota", 1_024)]);
    assert!(validate_catalog(&exact).is_ok());
    let over = AssetCatalog::from_entries(vec![texture.clone(), quota_atlas("quota", 1_025)]);
    assert!(validate_catalog(&over).errors.iter().any(|error| matches!(
        error,
        CatalogValidationError::AtlasRegionQuotaExceeded { .. }
    )));

    let aggregate_exact = AssetCatalog::from_entries(vec![
        texture.clone(),
        quota_atlas("quota-a", 1_024),
        quota_atlas("quota-b", 1_024),
        quota_atlas("quota-c", 1_024),
        quota_atlas("quota-d", 1_024),
    ]);
    assert!(validate_catalog(&aggregate_exact).is_ok());
    let mut aggregate_over = aggregate_exact;
    aggregate_over.entries.push(quota_atlas("quota-e", 1));
    assert!(validate_catalog(&aggregate_over)
        .errors
        .iter()
        .any(|error| matches!(
            error,
            CatalogValidationError::AggregateAtlasRegionQuotaExceeded
        )));
}

#[test]
fn codec_reports_semantic_authoring_paths_and_rejects_non_finite_output() {
    let encoded = encode_catalog(&sample_catalog()).expect("encode catalog");
    let bad_id = encoded.replacen("material/surface-a", "MATERIAL/surface-a", 1);
    let error = decode_catalog(&bad_id).expect_err("bad id");
    assert!(error.path.ends_with(".id"));

    let mut invalid_number = sample_catalog();
    invalid_number.entries[1]
        .material
        .as_mut()
        .expect("material")
        .style
        .roughness = f32::NAN;
    let error = encode_catalog(&invalid_number).expect_err("non-finite authoring value");
    assert!(error.path.ends_with(".roughness"));
}

#[test]
fn locks_round_trip_and_classify_every_drift_without_mutation() {
    let catalog = sample_catalog();
    let lock = generate_lock(&catalog);
    assert!(validate_lock(&lock, &catalog).is_clean());
    let encoded = encode_lock(&lock).expect("encode lock");
    let restored = decode_lock(&encoded).expect("decode lock");
    assert_eq!(restored, lock);

    let mut drifted = catalog.clone();
    drifted.entries[0].version = 7;
    drifted.entries[1].hash = Some(AssetHash::parse("eeee").expect("hash"));
    drifted.entries[0]
        .dependencies
        .push(reference("texture/surface-atlas-a"));
    drifted.entries.remove(2);
    drifted
        .entries
        .push(CatalogEntry::new(id("sprite/new-a"), 1));
    let report = validate_lock(&lock, &drifted);
    assert!(report
        .findings
        .iter()
        .any(|finding| matches!(finding.issue, LockIssue::StaleVersion { .. })));
    assert!(report
        .findings
        .iter()
        .any(|finding| matches!(finding.issue, LockIssue::StaleHash { .. })));
    assert!(report
        .findings
        .iter()
        .any(|finding| matches!(finding.issue, LockIssue::DependencyDrift { .. })));
    assert!(report
        .findings
        .iter()
        .any(|finding| matches!(finding.issue, LockIssue::Missing)));
    assert!(report
        .findings
        .iter()
        .any(|finding| matches!(finding.issue, LockIssue::NewInCatalog)));
}

#[test]
fn material_projections_fallback_and_change_impacts_remain_separate() {
    let before = material(StructuralClass::Structural, Rgba::DEBUG_GREY);
    let mut visual_after = before.clone();
    visual_after.style.color = Rgba::DEBUG_MAGENTA;
    assert_eq!(
        classify_material_change(&before, &visual_after),
        ChangeKind::VisualOnly
    );

    let render = before.render_projection();
    let collision = before.collision_projection();
    assert_eq!(render.uv_strategy, UvStrategy::Flat);
    assert_eq!(collision.structural_class, StructuralClass::Structural);
    assert!(matches!(
        fallback_for(AssetKind::Material, AssetContext::CosmeticSurface),
        FallbackOutcome::UseFallback {
            visual: FallbackVisual::GreyMaterial,
            ..
        }
    ));
    assert!(matches!(
        fallback_for(AssetKind::Material, AssetContext::CollisionCritical),
        FallbackOutcome::FailClosed { .. }
    ));

    let catalog = sample_catalog();
    let visual =
        material_change_impact(&catalog, &id("material/surface-a"), &before, &visual_after)
            .expect("present material");
    assert!(visual.safe);
    assert_eq!(visual.suggestion, ReloadSuggestion::Reproject);
    let structural = revalidate_asset(&catalog, &id("mesh/fixture-a"), ChangeKind::Structural)
        .expect("present mesh");
    assert!(structural.requires_full_reload);
}

#[test]
fn voxel_material_resolution_falls_back_only_for_visuals() {
    let catalog = AssetCatalog::from_entries(vec![
        CatalogEntry::new(id("material/stone"), 1)
            .with_material(material(StructuralClass::Structural, Rgba::DEBUG_GREY)),
        CatalogEntry::new(id("mesh/not-material"), 1),
    ]);
    let table = VoxelMaterialTable::from_pairs([
        (VoxelMaterialId::new(1), id("material/stone")),
        (VoxelMaterialId::new(2), id("mesh/not-material")),
    ]);
    assert!(
        !table
            .render_material(&catalog, VoxelMaterialId::new(1))
            .used_fallback
    );
    assert!(
        table
            .render_material(&catalog, VoxelMaterialId::new(99))
            .used_fallback
    );
    assert_eq!(
        table.collision_material(&catalog, VoxelMaterialId::new(99)),
        Err(VoxelMaterialError::Unmapped(VoxelMaterialId::new(99)))
    );
    assert!(matches!(
        table.collision_material(&catalog, VoxelMaterialId::new(2)),
        Err(VoxelMaterialError::NotAMaterial { .. })
    ));
    let report = table.validate_used(
        &catalog,
        [
            VoxelMaterialId::new(1),
            VoxelMaterialId::new(99),
            VoxelMaterialId::new(99),
        ],
    );
    assert_eq!(report.unresolved.len(), 1);
    assert!(!report.is_collision_safe());
}
