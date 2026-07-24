use voxel_asset::{
    conversion_settings_sha256, decode_voxel_asset, encode_voxel_asset,
    validate_conversion_request, validate_voxel_asset, with_computed_content_hash, VoxelAsset,
    VoxelAssetBounds, VoxelAssetGrid, VoxelAssetMaterialMapping, VoxelAssetProvenance,
    VoxelAssetProvenanceKind, VoxelConversionFitPolicy, VoxelConversionMode,
    VoxelConversionOriginPolicy, VoxelConversionRequest, VoxelConversionSettings,
    VoxelCoordinateSystem, VoxelRepresentation, VoxelRepresentationKind, VoxelSparseRun,
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

    let first = encode_voxel_asset(&asset).expect("encoded asset");
    let decoded = decode_voxel_asset(&first).expect("decoded asset");
    let second = encode_voxel_asset(&decoded).expect("re-encoded asset");
    assert_eq!(first, second);
    assert!(first.ends_with('\n'));
    assert!(first.contains("\"rightHandedYUp\""));
    assert!(first.contains("\"sparseRuns\""));
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

    let mut excessive_resolution = request.clone();
    excessive_resolution.settings.resolution = [257, 1, 1];
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
            source_path: "../asha-engine/harness/fixtures/voxel-conversion/kenney-wall-a.glb"
                .to_string(),
            source_sha256:
                "sha256:6fceda24c30d2c22694f232f03fe2115fb1a462046fbbf719a90eea10dc9af00"
                    .to_string(),
            source_byte_count: 3_352,
            converter: "rusty-engine.mesh-to-voxel.v1".to_string(),
            settings_sha256:
                "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    .to_string(),
            license_path: Some(
                "../asha-engine/harness/fixtures/voxel-conversion/KENNEY-RETRO-URBAN-KIT-LICENSE.txt"
                    .to_string(),
            ),
        },
        content_hash: String::new(),
    }
}

fn valid_request() -> VoxelConversionRequest {
    VoxelConversionRequest {
        asset_id: "voxel-volume/kenney-wall-a".to_string(),
        source_path: "../asha-engine/harness/fixtures/voxel-conversion/kenney-wall-a.glb"
            .to_string(),
        expected_source_sha256:
            "sha256:6fceda24c30d2c22694f232f03fe2115fb1a462046fbbf719a90eea10dc9af00".to_string(),
        license_path: Some(
            "../asha-engine/harness/fixtures/voxel-conversion/KENNEY-RETRO-URBAN-KIT-LICENSE.txt"
                .to_string(),
        ),
        settings: VoxelConversionSettings {
            resolution: [8, 8, 2],
            cell_size: 0.5,
            chunk_size: 16,
            origin: [4, 0, 6],
            fit_policy: VoxelConversionFitPolicy::Contain,
            origin_policy: VoxelConversionOriginPolicy::TargetMin,
            mode: VoxelConversionMode::Solid,
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
