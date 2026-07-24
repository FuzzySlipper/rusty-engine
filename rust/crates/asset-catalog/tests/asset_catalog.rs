use asset_catalog::{
    classify_material_change, decode_catalog, decode_lock, encode_catalog, encode_lock,
    fallback_for, generate_lock, material_change_impact, revalidate_asset, validate_catalog,
    validate_lock, AssetCatalog, AssetContext, CatalogEntry, CatalogValidationError, ChangeKind,
    DependencyGraph, FallbackOutcome, FallbackVisual, LockIssue, MaterialAuthority,
    MaterialDefinition, MaterialStyle, ReloadSuggestion, Rgba, StructuralClass, UvStrategy,
    VoxelMaterialError, VoxelMaterialTable,
};
use core_assets::{AssetHash, AssetId, AssetKind, AssetReference, AssetVersionReq};
use core_voxel::VoxelMaterialId;

fn id(value: &str) -> AssetId {
    AssetId::parse(value).expect("fixture asset id")
}

fn reference(value: &str) -> AssetReference {
    AssetReference::new(id(value), AssetVersionReq::Any, None)
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
