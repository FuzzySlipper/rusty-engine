use voxel_asset::{
    conversion_settings_sha256, decode_voxel_asset, encode_voxel_asset, replace_voxel_palette,
    validate_conversion_request, validate_voxel_asset, with_computed_content_hash, VoxelAsset,
    VoxelAssetBounds, VoxelAssetGrid, VoxelAssetMaterialBinding, VoxelAssetMaterialMapping,
    VoxelAssetProvenance, VoxelAssetProvenanceKind, VoxelConversionFitPolicy, VoxelConversionMode,
    VoxelConversionOriginPolicy, VoxelConversionRequest, VoxelConversionSettings,
    VoxelCoordinateSystem, VoxelPaletteUpdateError, VoxelPaletteUpdateRequest, VoxelRepresentation,
    VoxelRepresentationKind, VoxelSparseRun, MAX_CONVERSION_RESOLUTION_AXIS,
    MAX_CONVERSION_SOURCE_BYTES, VOXEL_ASSET_SCHEMA_VERSION,
};

#[test]
fn schema_one_sparse_asset_is_canonical_and_byte_stable() {
    let mut source = valid_asset();
    source.representation.sparse_runs.reverse();
    source.representation.sparse_runs.extend([
        VoxelSparseRun {
            start: [1, 1, 0],
            length: 2,
            material_slot: 3,
        },
        VoxelSparseRun {
            start: [0, 1, 0],
            length: 1,
            material_slot: 3,
        },
    ]);
    source.bounds.max[1] = 1;
    source.material_map.reverse();
    let asset = with_computed_content_hash(source).expect("canonical asset");

    assert_eq!(asset.representation.sparse_runs.len(), 2);
    assert_eq!(asset.representation.sparse_runs[0].length, 3);
    assert_eq!(asset.representation.sparse_runs[1].length, 3);
    assert!(asset.content_hash.starts_with("sha256:"));
    assert!(asset.voxel_data_hash.starts_with("sha256:"));

    let first = encode_voxel_asset(&asset).expect("encoded asset");
    let decoded = decode_voxel_asset(&first).expect("decoded asset");
    let second = encode_voxel_asset(&decoded).expect("re-encoded asset");
    assert_eq!(first, second);
    assert!(first.ends_with('\n'));
    assert!(first.contains("\"rightHandedYUp\""));
    assert!(first.contains("\"sparseRuns\""));
}

#[test]
fn palette_replacement_is_fail_atomic_and_preserves_voxel_identity() {
    let mut asset = with_computed_content_hash(valid_asset()).unwrap();
    let original = asset.clone();
    let receipt = replace_voxel_palette(
        &mut asset,
        VoxelPaletteUpdateRequest {
            expected_content_hash: original.content_hash.clone(),
            expected_voxel_data_hash: original.voxel_data_hash.clone(),
            replacement: vec![VoxelAssetMaterialBinding {
                material_slot: 3,
                material_asset_id: "material/polished-concrete".to_string(),
                display_name: Some("Polished concrete".to_string()),
            }],
        },
    )
    .unwrap();
    assert_ne!(receipt.content_hash_before, receipt.content_hash_after);
    assert_eq!(asset.voxel_data_hash, original.voxel_data_hash);

    let accepted = asset.clone();
    let stale = replace_voxel_palette(
        &mut asset,
        VoxelPaletteUpdateRequest {
            expected_content_hash: original.content_hash,
            expected_voxel_data_hash: original.voxel_data_hash,
            replacement: vec![],
        },
    );
    assert!(matches!(
        stale,
        Err(VoxelPaletteUpdateError::StaleContentHash { .. })
    ));
    assert_eq!(asset, accepted);

    let invalid = replace_voxel_palette(
        &mut asset,
        VoxelPaletteUpdateRequest {
            expected_content_hash: accepted.content_hash.clone(),
            expected_voxel_data_hash: accepted.voxel_data_hash.clone(),
            replacement: vec![VoxelAssetMaterialBinding {
                material_slot: 3,
                material_asset_id: "static-mesh/not-a-material".to_string(),
                display_name: None,
            }],
        },
    );
    assert!(matches!(invalid, Err(VoxelPaletteUpdateError::Invalid(_))));
    assert_eq!(asset, accepted);
}

#[test]
fn strict_decode_and_hash_reject_unknown_or_changed_content() {
    let asset = with_computed_content_hash(valid_asset()).unwrap();
    let encoded = encode_voxel_asset(&asset).unwrap();
    let mut value: serde_json::Value = serde_json::from_str(&encoded).unwrap();
    value["grid"]["unexpected"] = true.into();
    let error = decode_voxel_asset(&serde_json::to_string(&value).unwrap()).unwrap_err();
    assert_eq!(error.diagnostics()[0].code, "voxelAsset.decode");
    assert!(error.diagnostics()[0].path.starts_with("grid"));

    let mut changed = asset;
    changed.representation.sparse_runs[0].material_slot = 4;
    let error = validate_voxel_asset(&changed).unwrap_err();
    assert!(error
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code == "voxelAsset.unknownMaterial"));
    assert!(error
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code == "voxelAsset.contentHashMismatch"));
}

#[test]
fn schema_provenance_and_sparse_resource_limits_are_structured() {
    let mut wrong_schema = valid_asset();
    wrong_schema.schema_version = 2;
    let error = with_computed_content_hash(wrong_schema).unwrap_err();
    assert_eq!(error.diagnostics()[0].code, "voxelAsset.unsupportedSchema");
    assert_eq!(error.diagnostics()[0].path, "schemaVersion");

    let mut bad_provenance = valid_asset();
    bad_provenance.provenance.source_sha256 = "sha256:stale".to_string();
    let error = with_computed_content_hash(bad_provenance).unwrap_err();
    assert!(error
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.path == "provenance.sourceSha256"));

    let mut oversized = valid_asset();
    oversized.bounds.max[0] = 1_000_000;
    oversized.representation.sparse_runs = vec![VoxelSparseRun {
        start: [0, 0, 0],
        length: 1_000_001,
        material_slot: 3,
    }];
    let error = with_computed_content_hash(oversized).unwrap_err();
    assert!(error.diagnostics().iter().any(|diagnostic| {
        diagnostic.code == "voxelAsset.resourceLimit"
            && diagnostic.path == "representation.sparseRuns"
    }));
}

#[test]
fn conversion_input_fixes_identity_settings_and_hard_limits_before_parsing() {
    let request = valid_request();
    validate_conversion_request(&request, MAX_CONVERSION_SOURCE_BYTES).unwrap();
    let settings_hash = conversion_settings_sha256(&request.settings);
    assert!(settings_hash.starts_with("sha256:"));
    let mut reordered = request.settings.clone();
    reordered.material_map.reverse();
    assert_eq!(conversion_settings_sha256(&reordered), settings_hash);

    let error = validate_conversion_request(&request, MAX_CONVERSION_SOURCE_BYTES + 1).unwrap_err();
    assert!(error.diagnostics().iter().any(|diagnostic| {
        diagnostic.code == "conversion.resourceLimit" && diagnostic.path == "source"
    }));

    let mut high_density = request.clone();
    high_density.settings.resolution = [1_024, 1_024, 4];
    high_density.settings.origin = [0, 0, 0];
    validate_conversion_request(&high_density, 3_352).unwrap();

    let mut representational_limit = request.clone();
    representational_limit.settings.resolution = [MAX_CONVERSION_RESOLUTION_AXIS, 1, 1];
    representational_limit.settings.origin = [-1_000_000, 0, 0];
    validate_conversion_request(&representational_limit, 3_352).unwrap();

    let mut excessive_resolution = request.clone();
    excessive_resolution.settings.resolution = [MAX_CONVERSION_RESOLUTION_AXIS + 1, 1, 1];
    excessive_resolution.settings.origin = [-1_000_000, 0, 0];
    let error = validate_conversion_request(&excessive_resolution, 3_352).unwrap_err();
    assert!(error
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.path == "settings.resolution"));

    let mut duplicate_mapping = request;
    duplicate_mapping
        .settings
        .material_map
        .push(duplicate_mapping.settings.material_map[0].clone());
    let error = validate_conversion_request(&duplicate_mapping, 3_352).unwrap_err();
    assert!(error
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code == "conversion.invalidMaterialMap"));
}

fn valid_asset() -> VoxelAsset {
    VoxelAsset {
        schema_version: VOXEL_ASSET_SCHEMA_VERSION,
        asset_id: "voxel-volume/kenney-wall-a".to_string(),
        grid: VoxelAssetGrid {
            coordinate_system: VoxelCoordinateSystem::RightHandedYUp,
            cell_size: 0.5,
            chunk_size: 16,
            origin: [4, 0, 6],
        },
        bounds: VoxelAssetBounds {
            min: [0, 0, 0],
            max: [2, 0, 0],
        },
        representation: VoxelRepresentation {
            kind: VoxelRepresentationKind::SparseRuns,
            sparse_runs: vec![
                VoxelSparseRun {
                    start: [2, 0, 0],
                    length: 1,
                    material_slot: 3,
                },
                VoxelSparseRun {
                    start: [0, 0, 0],
                    length: 2,
                    material_slot: 3,
                },
            ],
        },
        material_palette: vec![VoxelAssetMaterialBinding {
            material_slot: 3,
            material_asset_id: "material/concrete".to_string(),
            display_name: Some("Concrete".to_string()),
        }],
        material_map: vec![
            VoxelAssetMaterialMapping {
                source_material_slot: 1,
                source_material_name: Some("concrete".to_string()),
                voxel_material_slot: 3,
            },
            VoxelAssetMaterialMapping {
                source_material_slot: 0,
                source_material_name: Some("wall-lines".to_string()),
                voxel_material_slot: 3,
            },
        ],
        provenance: VoxelAssetProvenance {
            kind: VoxelAssetProvenanceKind::ConvertedStaticMesh,
            source_path: "fixtures/voxel-conversion/kenney-wall-a.glb".to_string(),
            source_sha256:
                "sha256:6fceda24c30d2c22694f232f03fe2115fb1a462046fbbf719a90eea10dc9af00"
                    .to_string(),
            source_byte_count: 3_352,
            converter: "rusty-engine.mesh-to-voxel.v1".to_string(),
            settings_sha256:
                "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    .to_string(),
            license_path: Some(
                "fixtures/voxel-conversion/KENNEY-RETRO-URBAN-KIT-LICENSE.txt".to_string(),
            ),
        },
        voxel_data_hash: String::new(),
        content_hash: String::new(),
    }
}

fn valid_request() -> VoxelConversionRequest {
    VoxelConversionRequest {
        asset_id: "voxel-volume/kenney-wall-a".to_string(),
        source_path: "fixtures/voxel-conversion/kenney-wall-a.glb".to_string(),
        expected_source_sha256:
            "sha256:6fceda24c30d2c22694f232f03fe2115fb1a462046fbbf719a90eea10dc9af00".to_string(),
        license_path: Some(
            "fixtures/voxel-conversion/KENNEY-RETRO-URBAN-KIT-LICENSE.txt".to_string(),
        ),
        settings: VoxelConversionSettings {
            resolution: [8, 8, 2],
            cell_size: 0.5,
            chunk_size: 16,
            origin: [4, 0, 6],
            fit_policy: VoxelConversionFitPolicy::Contain,
            origin_policy: VoxelConversionOriginPolicy::TargetMin,
            mode: VoxelConversionMode::Solid,
            material_palette: vec![
                VoxelAssetMaterialBinding {
                    material_slot: 3,
                    material_asset_id: "material/wall-lines".to_string(),
                    display_name: Some("Wall lines".to_string()),
                },
                VoxelAssetMaterialBinding {
                    material_slot: 4,
                    material_asset_id: "material/concrete".to_string(),
                    display_name: Some("Concrete".to_string()),
                },
            ],
            material_map: vec![
                VoxelAssetMaterialMapping {
                    source_material_slot: 0,
                    source_material_name: Some("wall-lines".to_string()),
                    voxel_material_slot: 3,
                },
                VoxelAssetMaterialMapping {
                    source_material_slot: 1,
                    source_material_name: Some("concrete".to_string()),
                    voxel_material_slot: 4,
                },
            ],
            max_output_voxels: 128,
        },
    }
}
