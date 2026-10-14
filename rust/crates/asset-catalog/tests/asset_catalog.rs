use asset_catalog::{
    classify_material_change, decode_catalog, decode_lock, encode_catalog, encode_lock,
    fallback_for, generate_lock, material_change_impact, revalidate_asset, validate_lock,
    AdmittedAssetCatalog, AssetCatalog, AssetContext, AtlasInset, AtlasPadding,
    AtlasRegionDefinition, CatalogAdmissionError, CatalogEntry, CatalogValidationError,
    CatalogValidationReport, ChangeKind, DependencyGraph, FallbackOutcome, FallbackVisual,
    LockIssue, MaterialAuthority, MaterialDefinition, MaterialStyle, ReloadSuggestion,
    ResolvedVoxelSurfaceMapping, Rgba, StructuralClass, TextureDefinition, TextureFilter,
    TextureWrap, UvStrategy, VoxelAlphaMode, VoxelAtlasDefinition, VoxelMaterialError,
    VoxelMaterialTable, VoxelSurfaceBinding, VoxelSurfaceMapping,
};
use core_assets::{AssetHash, AssetId, AssetKind, AssetReference, AssetVersionReq};
use core_voxel::VoxelMaterialId;

fn id(value: &str) -> AssetId {
    AssetId::parse(value).expect("fixture asset id")
}

/// What admission reports about a candidate; empty when it admits.
fn admission_report(catalog: &AssetCatalog) -> CatalogValidationReport {
    match AdmittedAssetCatalog::admit(catalog.clone()) {
        Ok(_) => CatalogValidationReport::default(),
        Err(CatalogAdmissionError::Validation(report)) => report,
        Err(error) => panic!("unexpected admission error: {error:?}"),
    }
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

#[test]
fn catalog_validation_preserves_classified_dependency_and_material_failures() {
    assert!(admission_report(&sample_catalog()).is_ok());

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

    let report = admission_report(&invalid);
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
    assert!(admission_report(&cycle)
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
    assert!(admission_report(&catalog).is_ok());
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
    assert!(admission_report(&stale)
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
    assert!(admission_report(&duplicate_dependency).is_ok());
    let material = duplicate_dependency
        .entries
        .iter_mut()
        .find(|entry| entry.id.as_str() == "material/repeating-stone")
        .unwrap();
    material.dependencies.push(reference("texture/stone-tile"));
    assert!(admission_report(&duplicate_dependency)
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
    assert!(
        admission_report(&invalid_atlas).is_ok(),
        "overlapping regions admit"
    );
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
    assert!(admission_report(&invalid_atlas)
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
fn voxel_surface_rejects_conflicting_legacy_texture_facts() {
    let original = surface_catalog();

    for material_id in ["material/repeating-stone", "material/atlas-stone"] {
        let mut conflicting = original.clone();
        conflicting
            .entries
            .iter_mut()
            .find(|entry| entry.id.as_str() == material_id)
            .unwrap()
            .material
            .as_mut()
            .unwrap()
            .style
            .texture = Some(pinned("texture/conflicting", 1, "ffff"));
        assert!(admission_report(&conflicting)
            .errors
            .iter()
            .any(|error| matches!(
                error,
                CatalogValidationError::ConflictingSurfaceTexture { from, .. }
                    if from.as_str() == material_id
            )));
        let encoded = encode_catalog(&conflicting).unwrap();
        let reopened = decode_catalog(&encoded).unwrap();
        assert!(admission_report(&reopened)
            .errors
            .iter()
            .any(|error| matches!(
                error,
                CatalogValidationError::ConflictingSurfaceTexture { .. }
            )));
    }

    let mut omitted = original.clone();
    for entry in &mut omitted.entries {
        if let Some(style) = entry.material.as_mut().map(|material| &mut material.style) {
            if style.voxel_surface.is_some() {
                style.texture = None;
            }
        }
    }
    assert!(admission_report(&omitted).is_ok());
}

fn set_repeat_tile(catalog: &mut AssetCatalog, scale: [f32; 2], origin: [f32; 2]) {
    let material = catalog
        .entries
        .iter_mut()
        .find(|entry| entry.id.as_str() == "material/repeating-stone")
        .unwrap()
        .material
        .as_mut()
        .unwrap();
    if let VoxelSurfaceMapping::Repeat {
        tile_scale_cells,
        tile_origin_cells,
        ..
    } = &mut material.style.voxel_surface.as_mut().unwrap().mapping
    {
        *tile_scale_cells = scale;
        *tile_origin_cells = origin;
    }
}

#[test]
fn atlas_padding_region_ids_and_tile_magnitudes_admit() {
    let mut catalog = surface_catalog();
    let atlas = catalog
        .entries
        .iter_mut()
        .find(|entry| entry.id.as_str() == "sprite-sheet/voxel-atlas")
        .unwrap()
        .voxel_atlas
        .as_mut()
        .unwrap();
    // The atlas texture is linear; an unpadded region only bleeds at its edges.
    atlas.regions[0].padding = AtlasPadding::ZERO;
    atlas.regions[1].padding.left = 2;
    atlas.regions.push(AtlasRegionDefinition {
        id: "Whole Atlas".to_string(),
        content_min: [0, 0],
        content_extent: [64, 32],
        padding: AtlasPadding::ZERO,
        inset: AtlasInset::HalfTexel,
    });
    assert!(admission_report(&catalog).is_ok());

    let mut catalog = surface_catalog();
    set_repeat_tile(&mut catalog, [1.0e-6, 1.0e6], [-1.0e9, 3.0e30]);
    assert!(admission_report(&catalog).is_ok());
    for scale in [
        [0.0, 1.0],
        [1.0, -2.0],
        [f32::NAN, 1.0],
        [1.0, f32::INFINITY],
    ] {
        set_repeat_tile(&mut catalog, scale, [0.0, 0.0]);
        assert!(admission_report(&catalog)
            .errors
            .iter()
            .any(|error| matches!(error, CatalogValidationError::InvalidTileScale { .. })));
    }
    set_repeat_tile(&mut catalog, [1.0, 1.0], [f32::NEG_INFINITY, 0.0]);
    assert!(admission_report(&catalog)
        .errors
        .iter()
        .any(|error| matches!(error, CatalogValidationError::InvalidTileOrigin { .. })));
}

#[test]
fn strict_reopen_keeps_the_admitted_identity() {
    let source = surface_catalog();
    let entries = source.entries.as_ptr();
    let admitted = AdmittedAssetCatalog::admit(source).unwrap();
    assert_eq!(admitted.catalog().entries.as_ptr(), entries);
    let reopened = AdmittedAssetCatalog::reopen(admitted.canonical_json()).unwrap();
    assert_eq!(reopened, admitted);
    assert!(admitted.canonical_hash().starts_with("sha256:"));
    assert_eq!(reopened.canonical_hash(), admitted.canonical_hash());

    let mut invalid = surface_catalog();
    invalid
        .entries
        .iter_mut()
        .find(|entry| entry.id.as_str() == "material/atlas-stone")
        .unwrap()
        .hash = None;
    invalid
        .entries
        .iter_mut()
        .find(|entry| entry.id.as_str() == "texture/voxel-atlas")
        .unwrap()
        .version = 4;
    assert!(AdmittedAssetCatalog::admit(invalid).is_err());
}

#[test]
fn atlases_admit_any_number_of_regions_and_refuse_a_repeated_region() {
    let texture_ref = pinned("texture/large-atlas", 1, "aa01");
    let texture = CatalogEntry::new(id("texture/large-atlas"), 1)
        .with_hash(AssetHash::parse("aa01").unwrap())
        .with_texture(TextureDefinition {
            width: 8_192,
            height: 8_192,
            filter: TextureFilter::Nearest,
            wrap: TextureWrap::Clamp,
        });
    let atlas = |name: &str, region_count: u32| {
        CatalogEntry::new(id(&format!("sprite-sheet/{name}")), 1).with_voxel_atlas(
            VoxelAtlasDefinition {
                schema_version: 1,
                texture: texture_ref.clone(),
                regions: (0..region_count)
                    .map(|index| AtlasRegionDefinition {
                        id: format!("r-{index:05}"),
                        content_min: [index % 8_192, index / 8_192],
                        content_extent: [1, 1],
                        padding: AtlasPadding::ZERO,
                        inset: AtlasInset::HalfTexel,
                    })
                    .collect(),
            },
        )
    };
    let catalog = AssetCatalog::from_entries(vec![
        texture.clone(),
        atlas("large-a", 5_000),
        atlas("large-b", 5_000),
    ]);
    assert!(admission_report(&catalog).is_ok());

    let mut repeated = atlas("repeated", 3);
    repeated.voxel_atlas.as_mut().unwrap().regions[2].id = "r-00000".to_string();
    assert!(
        admission_report(&AssetCatalog::from_entries(vec![texture, repeated]))
            .errors
            .iter()
            .any(|error| matches!(
                error,
                CatalogValidationError::DuplicateAtlasRegion { region, .. } if region == "r-00000"
            ))
    );
}

#[test]
fn surface_references_are_dependencies_without_being_listed_twice() {
    let mut source = surface_catalog();
    for entry in &mut source.entries {
        entry.dependencies.clear();
    }
    // The surface references are any-version and unhashed; the renderer takes
    // the content hash from the entry each resolves to.
    let tile = reference("texture/stone-tile");
    for entry in &mut source.entries {
        if let Some(material) = &mut entry.material {
            material.style.texture = None;
            if let VoxelSurfaceMapping::Repeat { texture, .. } =
                &mut material.style.voxel_surface.as_mut().unwrap().mapping
            {
                *texture = tile.clone();
            }
        }
    }
    let admitted = AdmittedAssetCatalog::admit(source.clone()).expect("surface-only references");
    let dependencies = |id_value: &str| {
        admitted
            .catalog()
            .get(&id(id_value))
            .unwrap()
            .dependencies
            .iter()
            .map(|dependency| dependency.id().as_str().to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        dependencies("material/repeating-stone"),
        ["texture/stone-tile"]
    );
    assert_eq!(
        dependencies("material/atlas-stone"),
        ["sprite-sheet/voxel-atlas"]
    );
    assert_eq!(
        dependencies("sprite-sheet/voxel-atlas"),
        ["texture/voxel-atlas"]
    );

    let lock = generate_lock(admitted.catalog());
    let locked = lock
        .entries
        .iter()
        .find(|entry| entry.id.as_str() == "material/repeating-stone")
        .unwrap();
    assert_eq!(locked.dependencies, [id("texture/stone-tile")]);
    assert_eq!(
        DependencyGraph::build(admitted.catalog()).dependents_of(&id("texture/voxel-atlas")),
        [id("material/atlas-stone"), id("sprite-sheet/voxel-atlas")]
    );
    let reopened = AdmittedAssetCatalog::reopen(admitted.canonical_json()).unwrap();
    assert_eq!(reopened.canonical_hash(), admitted.canonical_hash());
    let resolved = admitted
        .catalog()
        .render_material(&id("material/repeating-stone"))
        .unwrap()
        .voxel_surface
        .unwrap();
    match resolved.mapping {
        ResolvedVoxelSurfaceMapping::Repeat { texture_hash, .. } => {
            assert_eq!(texture_hash, Some(AssetHash::parse("aa02").unwrap()));
        }
        other => panic!("unexpected mapping: {other:?}"),
    }

    // A surface reference is resolved, pinned and cycle-checked as a
    // dependency even though no dependency lists it.
    let mut stale = source.clone();
    let material = stale
        .entries
        .iter_mut()
        .find(|entry| entry.id.as_str() == "material/repeating-stone")
        .unwrap();
    if let VoxelSurfaceMapping::Repeat { texture, .. } = &mut material
        .material
        .as_mut()
        .unwrap()
        .style
        .voxel_surface
        .as_mut()
        .unwrap()
        .mapping
    {
        *texture = pinned("texture/stone-tile", 2, "ffff");
    }
    assert!(admission_report(&stale)
        .errors
        .iter()
        .any(|error| matches!(error, CatalogValidationError::StaleDependencyHash { .. })));

    let mut missing = source.clone();
    missing
        .entries
        .retain(|entry| entry.id.as_str() != "texture/voxel-atlas");
    assert!(admission_report(&missing)
        .errors
        .iter()
        .any(|error| matches!(
            error,
            CatalogValidationError::UnknownDependency { dependency, .. }
                if dependency.as_str() == "texture/voxel-atlas"
        )));

    let mut cycle = source;
    cycle
        .entries
        .iter_mut()
        .find(|entry| entry.id.as_str() == "texture/stone-tile")
        .unwrap()
        .dependencies
        .push(reference("material/repeating-stone"));
    assert!(admission_report(&cycle)
        .errors
        .iter()
        .any(|error| matches!(error, CatalogValidationError::DependencyCycle { .. })));
}

#[test]
fn texture_dimension_validation_defers_capacity_to_the_renderer() {
    let mut catalog = surface_catalog();
    {
        let texture = catalog
            .entries
            .iter_mut()
            .find(|entry| entry.id.as_str() == "texture/stone-tile")
            .and_then(|entry| entry.texture.as_mut())
            .expect("fixture texture definition");
        texture.width = u32::MAX;
        texture.height = u32::MAX;
    }
    assert!(admission_report(&catalog).is_ok());

    catalog
        .entries
        .iter_mut()
        .find(|entry| entry.id.as_str() == "texture/stone-tile")
        .and_then(|entry| entry.texture.as_mut())
        .expect("fixture texture definition")
        .width = 0;
    assert!(admission_report(&catalog)
        .errors
        .iter()
        .any(|error| matches!(
            error,
            CatalogValidationError::InvalidTextureDimensions { id }
                if id.as_str() == "texture/stone-tile"
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
