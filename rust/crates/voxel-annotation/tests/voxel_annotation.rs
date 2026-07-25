use voxel_annotation::{
    decode_annotation_layer, encode_annotation_layer, export_annotation_layer,
    finalize_annotation_draft, query_annotation_layer, VoxelAnnotationBounds,
    VoxelAnnotationDiagnosticCode, VoxelAnnotationEditCommand, VoxelAnnotationEditError,
    VoxelAnnotationEditService, VoxelAnnotationEditTransaction, VoxelAnnotationKind,
    VoxelAnnotationLayerDraft, VoxelAnnotationLimits, VoxelAnnotationProvenanceKind,
    VoxelAnnotationProvenanceRef, VoxelAnnotationQuery, VoxelAnnotationQueryMode,
    VoxelAnnotationRegion, VoxelAnnotationSelection, VoxelAnnotationSparseRun,
};
use voxel_asset::{
    with_computed_content_hash, VoxelAsset, VoxelAssetBounds, VoxelAssetGrid,
    VoxelAssetMaterialBinding, VoxelAssetMaterialMapping, VoxelAssetProvenance,
    VoxelAssetProvenanceKind, VoxelCoordinateSystem, VoxelRepresentation, VoxelRepresentationKind,
    VoxelSparseRun, VOXEL_ASSET_SCHEMA_VERSION,
};

#[test]
fn draft_finalization_is_canonical_queryable_and_strict() {
    let target = target_asset();
    let layer =
        finalize_annotation_draft(draft(&target), &target, VoxelAnnotationLimits::default())
            .unwrap();
    assert_eq!(layer.regions[0].selection.sparse_runs.len(), 1);
    assert_eq!(layer.regions[0].selection.sparse_runs[0].length, 3);
    assert!(layer.content_hashes.canonical_layer.starts_with("sha256:"));
    assert!(layer.content_hashes.membership_data.starts_with("sha256:"));

    let encoded = encode_annotation_layer(&layer).unwrap();
    let decoded = decode_annotation_layer(&encoded).unwrap();
    assert_eq!(decoded, layer);
    assert_eq!(encode_annotation_layer(&decoded).unwrap(), encoded);
    let mut json: serde_json::Value = serde_json::from_str(&encoded).unwrap();
    json["regions"][0]["surprise"] = true.into();
    assert!(decode_annotation_layer(&serde_json::to_string(&json).unwrap()).is_err());

    let readout = query_annotation_layer(
        &layer,
        &VoxelAnnotationQuery {
            expected_layer_hash: Some(layer.content_hashes.canonical_layer.clone()),
            mode: VoxelAnnotationQueryMode::Cell {
                coordinate: [2, 0, 0],
            },
            max_results: 8,
        },
    )
    .unwrap();
    assert_eq!(readout.matched_regions.len(), 1);
    assert_eq!(readout.matched_regions[0].region_id, "region/spawn");
}

#[test]
fn validation_classifies_cycles_overlap_target_drift_and_quotas() {
    let target = target_asset();
    let mut invalid = draft(&target);
    invalid.target_voxel_data_hash =
        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string();
    invalid.regions[0].parent_region_id = Some("region/cover".to_string());
    invalid.regions.push(VoxelAnnotationRegion {
        region_id: "region/cover".to_string(),
        label: "Cover".to_string(),
        kind: VoxelAnnotationKind::Cover,
        tags: vec![],
        parent_region_id: Some("region/spawn".to_string()),
        bounds: bounds(2, 0, 0, 6, 0, 0),
        selection: VoxelAnnotationSelection {
            sparse_runs: vec![run(2, 0, 0, 3), run(3, 0, 0, 4)],
        },
    });
    let error = finalize_annotation_draft(
        invalid,
        &target,
        VoxelAnnotationLimits {
            max_regions: 1,
            ..VoxelAnnotationLimits::default()
        },
    )
    .unwrap_err();
    let codes: std::collections::BTreeSet<_> = error
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect();
    assert!(codes.contains(&VoxelAnnotationDiagnosticCode::TargetHashMismatch));
    assert!(codes.contains(&VoxelAnnotationDiagnosticCode::ParentCycle));
    assert!(codes.contains(&VoxelAnnotationDiagnosticCode::DuplicateCell));
    assert!(codes.contains(&VoxelAnnotationDiagnosticCode::QuotaExceeded));
}

#[test]
fn edit_batches_are_hash_guarded_atomic_and_cover_region_operations() {
    let target = target_asset();
    let mut layer =
        finalize_annotation_draft(draft(&target), &target, VoxelAnnotationLimits::default())
            .unwrap();
    let before = layer.clone();
    let stale = VoxelAnnotationEditService::apply(
        &mut layer,
        VoxelAnnotationEditTransaction {
            expected_layer_hash: "sha256:stale".to_string(),
            commands: vec![VoxelAnnotationEditCommand::SetLabel {
                region_id: "region/spawn".to_string(),
                label: "Changed".to_string(),
            }],
        },
    );
    assert!(matches!(
        stale,
        Err(VoxelAnnotationEditError::StaleLayerHash { .. })
    ));
    assert_eq!(layer, before);

    let expected = layer.content_hashes.canonical_layer.clone();
    let receipt = VoxelAnnotationEditService::apply(
        &mut layer,
        VoxelAnnotationEditTransaction {
            expected_layer_hash: expected,
            commands: vec![
                VoxelAnnotationEditCommand::SetLabel {
                    region_id: "region/spawn".to_string(),
                    label: "Primary Spawn".to_string(),
                },
                VoxelAnnotationEditCommand::SetTags {
                    region_id: "region/spawn".to_string(),
                    tags: vec!["entry".to_string(), "safe".to_string()],
                },
                VoxelAnnotationEditCommand::RemoveRuns {
                    region_id: "region/spawn".to_string(),
                    sparse_runs: vec![run(2, 0, 0, 1)],
                },
            ],
        },
    )
    .unwrap();
    assert_ne!(receipt.layer_hash_before, receipt.layer_hash_after);
    assert_ne!(
        receipt.membership_hash_before,
        receipt.membership_hash_after
    );
    assert_eq!(receipt.assigned_cell_count, 2);

    let accepted = layer.clone();
    let expected = layer.content_hashes.canonical_layer.clone();
    let invalid = VoxelAnnotationEditService::apply(
        &mut layer,
        VoxelAnnotationEditTransaction {
            expected_layer_hash: expected,
            commands: vec![VoxelAnnotationEditCommand::SetParent {
                region_id: "region/spawn".to_string(),
                parent_region_id: Some("region/missing".to_string()),
            }],
        },
    );
    assert!(matches!(
        invalid,
        Err(VoxelAnnotationEditError::InvalidCandidate(_))
    ));
    assert_eq!(layer, accepted);
}

#[test]
fn export_is_compare_and_swap_guarded() {
    let target = target_asset();
    let layer =
        finalize_annotation_draft(draft(&target), &target, VoxelAnnotationLimits::default())
            .unwrap();
    assert!(export_annotation_layer(&layer, "sha256:stale").is_err());
    let exported = export_annotation_layer(&layer, &layer.content_hashes.canonical_layer).unwrap();
    assert_eq!(exported.layer, layer);
    assert!(exported.canonical_json.ends_with('\n'));
}

fn draft(target: &VoxelAsset) -> VoxelAnnotationLayerDraft {
    VoxelAnnotationLayerDraft {
        layer_id: "voxel-annotation/test-room/semantic".to_string(),
        target_voxel_asset_id: target.asset_id.clone(),
        target_voxel_data_hash: target.voxel_data_hash.clone(),
        target_bounds: bounds(0, 0, 0, 9, 0, 0),
        regions: vec![VoxelAnnotationRegion {
            region_id: "region/spawn".to_string(),
            label: "Spawn".to_string(),
            kind: VoxelAnnotationKind::SpawnArea,
            tags: vec!["safe".to_string(), "entry".to_string(), "safe".to_string()],
            parent_region_id: None,
            bounds: bounds(1, 0, 0, 3, 0, 0),
            selection: VoxelAnnotationSelection {
                sparse_runs: vec![run(2, 0, 0, 2), run(1, 0, 0, 1)],
            },
        }],
        provenance: vec![VoxelAnnotationProvenanceRef {
            kind: VoxelAnnotationProvenanceKind::Authored,
            uri: "content/annotations/test-room.json".to_string(),
            content_hash: "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                .to_string(),
        }],
    }
}

fn target_asset() -> VoxelAsset {
    with_computed_content_hash(VoxelAsset {
        schema_version: VOXEL_ASSET_SCHEMA_VERSION,
        asset_id: "voxel-volume/test-room".to_string(),
        grid: VoxelAssetGrid {
            coordinate_system: VoxelCoordinateSystem::RightHandedYUp,
            cell_size: 1.0,
            chunk_size: 16,
            origin: [0, 0, 0],
        },
        bounds: VoxelAssetBounds {
            min: [0, 0, 0],
            max: [9, 0, 0],
        },
        representation: VoxelRepresentation {
            kind: VoxelRepresentationKind::SparseRuns,
            sparse_runs: vec![VoxelSparseRun {
                start: [0, 0, 0],
                length: 10,
                material_slot: 1,
            }],
        },
        material_palette: vec![VoxelAssetMaterialBinding {
            material_slot: 1,
            material_asset_id: "material/test".to_string(),
            display_name: Some("Test".to_string()),
        }],
        material_map: vec![VoxelAssetMaterialMapping {
            source_material_slot: 0,
            source_material_name: Some("test".to_string()),
            voxel_material_slot: 1,
        }],
        provenance: VoxelAssetProvenance {
            kind: VoxelAssetProvenanceKind::ConvertedStaticMesh,
            source_path: "fixtures/test.glb".to_string(),
            source_sha256:
                "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    .to_string(),
            source_byte_count: 16,
            converter: "test".to_string(),
            settings_sha256:
                "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
                    .to_string(),
            license_path: None,
        },
        voxel_data_hash: String::new(),
        content_hash: String::new(),
    })
    .unwrap()
}

fn run(x: i64, y: i64, z: i64, length: u32) -> VoxelAnnotationSparseRun {
    VoxelAnnotationSparseRun {
        start: [x, y, z],
        length,
    }
}

fn bounds(
    min_x: i64,
    min_y: i64,
    min_z: i64,
    max_x: i64,
    max_y: i64,
    max_z: i64,
) -> VoxelAnnotationBounds {
    VoxelAnnotationBounds {
        min: [min_x, min_y, min_z],
        max: [max_x, max_y, max_z],
    }
}
