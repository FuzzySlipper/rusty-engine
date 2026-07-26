use voxel_asset::{
    decode_voxel_object, encode_voxel_object, resolve_voxel_frame, validate_voxel_object,
    with_computed_voxel_object_hashes, VoxelAssetBounds, VoxelAssetMaterialBinding,
    VoxelAssetMaterialMapping, VoxelCoordinateSystem, VoxelFrame, VoxelObjectAnimationFrame,
    VoxelObjectAsset, VoxelObjectClip, VoxelObjectGrid, VoxelObjectProvenance,
    VoxelObjectProvenanceKind, VoxelRepresentation, VoxelRepresentationKind, VoxelSparseRun,
    MAX_STRING_BYTES, MAX_VOXEL_OBJECT_TOTAL_VOXELS, VOXEL_OBJECT_SCHEMA_VERSION,
};

#[test]
fn static_and_animated_objects_are_canonical_and_frame_resolvable() {
    let mut source = object();
    source.material_palette.reverse();
    source.clips.reverse();
    let object = with_computed_voxel_object_hashes(source).unwrap();
    let first = encode_voxel_object(&object).unwrap();
    let decoded = decode_voxel_object(&first).unwrap();
    let second = encode_voxel_object(&decoded).unwrap();

    assert_eq!(first, second);
    assert_eq!(decoded.clips[0].id, "idle");
    assert_eq!(decoded.clips[1].id, "walk");
    assert_eq!(decoded.default_clip.as_deref(), Some("walk"));
    assert_eq!(
        decoded
            .resolve_default_frame()
            .unwrap()
            .iter()
            .map(|cell| cell.coordinate)
            .collect::<Vec<_>>(),
        vec![[0, 0, 0], [1, 0, 0]]
    );
    assert_eq!(
        decoded
            .resolve_clip_frame("walk", 1)
            .unwrap()
            .iter()
            .map(|cell| (cell.coordinate, cell.material_slot))
            .collect::<Vec<_>>(),
        vec![([0, 0, 0], 1), ([0, 1, 0], 2)]
    );
    assert_eq!(decoded.bounds.min, [0, 0, 0]);
    assert_eq!(decoded.bounds.max, [1, 1, 0]);
}

#[test]
fn full_frame_hashes_are_semantic_but_timing_changes_object_identity() {
    let object = with_computed_voxel_object_hashes(object()).unwrap();
    let base_hash = object.default_frame.voxel_data_hash.clone();
    let content_hash = object.content_hash.clone();

    let mut reordered = object.clone();
    reordered.default_frame.representation.sparse_runs =
        vec![run([1, 0, 0], 1, 1), run([0, 0, 0], 1, 1)];
    reordered.default_frame.voxel_data_hash.clear();
    reordered.content_hash.clear();
    let reordered = with_computed_voxel_object_hashes(reordered).unwrap();
    assert_eq!(reordered.default_frame.voxel_data_hash, base_hash);
    assert_eq!(reordered.content_hash, content_hash);

    let mut retimed = object;
    retimed.clips[1].frames[0].duration_seconds = Some(0.2);
    retimed.content_hash.clear();
    for clip in &mut retimed.clips {
        for frame in &mut clip.frames {
            frame.frame.voxel_data_hash.clear();
        }
    }
    retimed.default_frame.voxel_data_hash.clear();
    let retimed = with_computed_voxel_object_hashes(retimed).unwrap();
    assert_ne!(retimed.content_hash, content_hash);
    assert_eq!(retimed.default_frame.voxel_data_hash, base_hash);
}

#[test]
fn clip_timing_identity_and_aggregate_limits_fail_closed() {
    let baseline = with_computed_voxel_object_hashes(object()).unwrap();
    let baseline_bytes = encode_voxel_object(&baseline).unwrap();

    let mut bad_duration = baseline.clone();
    bad_duration.clips[1].frames[0].duration_seconds = Some(f64::NAN);
    assert!(validate_voxel_object(&bad_duration)
        .unwrap_err()
        .diagnostics()
        .iter()
        .any(|item| item.code == "voxelObject.invalidFrameDuration"));

    let mut missing_default = baseline.clone();
    missing_default.default_clip = Some("missing".to_owned());
    assert!(validate_voxel_object(&missing_default)
        .unwrap_err()
        .diagnostics()
        .iter()
        .any(|item| item.code == "voxelObject.missingDefaultClip"));

    let mut duplicate = baseline.clone();
    duplicate.clips[1].id = duplicate.clips[0].id.clone();
    assert!(validate_voxel_object(&duplicate)
        .unwrap_err()
        .diagnostics()
        .iter()
        .any(|item| item.code == "voxelObject.invalidClipId"));

    let million = frame([0, 0, 0], 1_000_000, 1);
    let mut excessive = object();
    excessive.default_frame = million.clone();
    excessive.bounds = million.bounds;
    excessive.clips = vec![VoxelObjectClip {
        id: "long".to_owned(),
        name: None,
        frames_per_second: 12.0,
        frames: (0..16)
            .map(|_| VoxelObjectAnimationFrame {
                duration_seconds: None,
                frame: million.clone(),
            })
            .collect(),
    }];
    excessive.default_clip = Some("long".to_owned());
    let error = with_computed_voxel_object_hashes(excessive).unwrap_err();
    assert!(error.diagnostics().iter().any(|item| {
        item.code == "voxelObject.resourceLimit"
            && item
                .message
                .contains(&MAX_VOXEL_OBJECT_TOTAL_VOXELS.to_string())
    }));

    assert_eq!(encode_voxel_object(&baseline).unwrap(), baseline_bytes);
}

#[test]
fn strict_object_decode_and_standalone_frame_validation_reject_drift() {
    let object = with_computed_voxel_object_hashes(object()).unwrap();
    let encoded = encode_voxel_object(&object).unwrap();
    let mut value: serde_json::Value = serde_json::from_str(&encoded).unwrap();
    value["runtimeSession"] = serde_json::json!({});
    assert_eq!(
        decode_voxel_object(&serde_json::to_string(&value).unwrap())
            .unwrap_err()
            .diagnostics()[0]
            .code,
        "voxelObject.decode"
    );

    let mut changed = object.default_frame.clone();
    changed.representation.sparse_runs[0].material_slot = 2;
    assert!(resolve_voxel_frame(&changed, [1, 2]).is_err());
    assert!(object.resolve_clip_frame("missing", 0).is_err());
    assert!(object.resolve_clip_frame("walk", 99).is_err());
}

#[test]
fn object_and_material_identities_enforce_the_string_byte_limit() {
    let mut at_limit = object();
    at_limit.asset_id = identity_with_byte_len("voxel-object/", MAX_STRING_BYTES);
    at_limit.material_palette[0].material_asset_id =
        identity_with_byte_len("material/", MAX_STRING_BYTES);
    let at_limit = with_computed_voxel_object_hashes(at_limit).unwrap();
    let encoded = encode_voxel_object(&at_limit).unwrap();
    let decoded = decode_voxel_object(&encoded).unwrap();
    assert_eq!(decoded.asset_id.len(), MAX_STRING_BYTES);
    assert!(decoded
        .material_palette
        .iter()
        .any(|binding| binding.material_asset_id.len() == MAX_STRING_BYTES));

    let mut over_limit = object();
    over_limit.asset_id = identity_with_byte_len("voxel-object/", MAX_STRING_BYTES + 1);
    over_limit.material_palette[0].material_asset_id =
        identity_with_byte_len("material/", MAX_STRING_BYTES + 1);
    let construction_error = with_computed_voxel_object_hashes(over_limit).unwrap_err();
    assert_identity_limit_diagnostics(&construction_error);

    let mut value: serde_json::Value = serde_json::from_str(&encoded).unwrap();
    value["assetId"] = serde_json::json!(identity_with_byte_len(
        "voxel-object/",
        MAX_STRING_BYTES + 1
    ));
    value["materialPalette"][0]["materialAssetId"] =
        serde_json::json!(identity_with_byte_len("material/", MAX_STRING_BYTES + 1));
    let decode_error = decode_voxel_object(&serde_json::to_string(&value).unwrap()).unwrap_err();
    assert_identity_limit_diagnostics(&decode_error);
}

fn assert_identity_limit_diagnostics(error: &voxel_asset::VoxelObjectError) {
    assert!(error.diagnostics().iter().any(|item| {
        item.code == "voxelObject.invalidAssetId"
            && item.path == "assetId"
            && item.message.contains(&MAX_STRING_BYTES.to_string())
    }));
    assert!(error.diagnostics().iter().any(|item| {
        item.code == "voxelObject.invalidMaterialReference"
            && item.path == "materialPalette[0].materialAssetId"
            && item.message.contains(&MAX_STRING_BYTES.to_string())
    }));
}

fn identity_with_byte_len(prefix: &str, byte_len: usize) -> String {
    assert!(prefix.len() < byte_len);
    format!("{prefix}{}", "a".repeat(byte_len - prefix.len()))
}

fn object() -> VoxelObjectAsset {
    let default_frame = frame([0, 0, 0], 2, 1);
    let idle = VoxelObjectClip {
        id: "idle".to_owned(),
        name: Some("Idle".to_owned()),
        frames_per_second: 6.0,
        frames: vec![VoxelObjectAnimationFrame {
            duration_seconds: None,
            frame: default_frame.clone(),
        }],
    };
    let walk = VoxelObjectClip {
        id: "walk".to_owned(),
        name: Some("Walk".to_owned()),
        frames_per_second: 12.0,
        frames: vec![
            VoxelObjectAnimationFrame {
                duration_seconds: Some(0.1),
                frame: frame([0, 0, 0], 1, 1),
            },
            VoxelObjectAnimationFrame {
                duration_seconds: None,
                frame: VoxelFrame {
                    bounds: VoxelAssetBounds {
                        min: [0, 0, 0],
                        max: [0, 1, 0],
                    },
                    representation: VoxelRepresentation {
                        kind: VoxelRepresentationKind::SparseRuns,
                        sparse_runs: vec![run([0, 1, 0], 1, 2), run([0, 0, 0], 1, 1)],
                    },
                    voxel_data_hash: String::new(),
                },
            },
        ],
    };
    VoxelObjectAsset {
        schema_version: VOXEL_OBJECT_SCHEMA_VERSION,
        asset_id: "voxel-object/test-runner".to_owned(),
        grid: VoxelObjectGrid {
            coordinate_system: VoxelCoordinateSystem::RightHandedYUp,
            cell_size: 0.25,
            chunk_size: 16,
            pivot: [0.5, 0.0, 0.0],
        },
        bounds: VoxelAssetBounds {
            min: [0, 0, 0],
            max: [1, 1, 0],
        },
        default_frame,
        clips: vec![walk, idle],
        default_clip: Some("walk".to_owned()),
        material_palette: vec![
            VoxelAssetMaterialBinding {
                material_slot: 2,
                material_asset_id: "material/cloth".to_owned(),
                display_name: Some("Cloth".to_owned()),
            },
            VoxelAssetMaterialBinding {
                material_slot: 1,
                material_asset_id: "material/skin".to_owned(),
                display_name: Some("Skin".to_owned()),
            },
        ],
        material_map: vec![
            VoxelAssetMaterialMapping {
                source_material_slot: 1,
                source_material_name: Some("cloth".to_owned()),
                voxel_material_slot: 2,
            },
            VoxelAssetMaterialMapping {
                source_material_slot: 0,
                source_material_name: Some("skin".to_owned()),
                voxel_material_slot: 1,
            },
        ],
        provenance: VoxelObjectProvenance {
            kind: VoxelObjectProvenanceKind::ConvertedAnimatedMesh,
            source_path: "models/runner.glb".to_owned(),
            source_sha256:
                "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
            source_byte_count: 1024,
            converter: "rusty-engine.mesh-to-voxel-object.v1".to_owned(),
            settings_sha256:
                "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_owned(),
            license_path: Some("models/LICENSE.txt".to_owned()),
        },
        content_hash: String::new(),
    }
}

fn frame(start: [i64; 3], length: u32, material_slot: u16) -> VoxelFrame {
    VoxelFrame {
        bounds: VoxelAssetBounds {
            min: start,
            max: [start[0] + i64::from(length) - 1, start[1], start[2]],
        },
        representation: VoxelRepresentation {
            kind: VoxelRepresentationKind::SparseRuns,
            sparse_runs: vec![run(start, length, material_slot)],
        },
        voxel_data_hash: String::new(),
    }
}

fn run(start: [i64; 3], length: u32, material_slot: u16) -> VoxelSparseRun {
    VoxelSparseRun {
        start,
        length,
        material_slot,
    }
}
