use std::fs;
use std::process::Command;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use voxel_asset::{
    decode_voxel_object, resolve_voxel_frame, VoxelAssetMaterialBinding, VoxelAssetMaterialMapping,
    VoxelConversionFitPolicy, VoxelConversionMode, VoxelConversionOriginPolicy,
    VoxelConversionSettings, VoxelObjectProvenanceKind,
};
use voxel_convert::{
    apply_voxel_object_conversion, apply_voxel_object_conversion_and_install,
    decode_voxel_object_conversion_request, identity_transform, import_animated_mesh_source,
    import_mesh_source, plan_animated_voxel_object_conversion, plan_static_voxel_object_conversion,
    preview_voxel_object_conversion, query_voxel_object_frame, query_voxel_object_info,
    query_voxel_object_window, sample_animation_clip_range, AnimationAnchorPolicy,
    AnimationEndPolicy, AnimationSampleRangeRequest, ConversionMaterialPolicy,
    ConversionPlanSettings, MeshSourceBounds, MeshSourceFormat, MeshSourceImportRequest,
    VoxelObjectClipConversionRequest, VoxelObjectConversionApplyRequest,
    VoxelObjectConversionPlanRequest, VoxelObjectConversionPreviewRequest,
    VoxelObjectConversionSettings, VoxelObjectFrameRequest, VoxelObjectFrameSelection,
    VoxelObjectInfoRequest, VoxelObjectWindowRequest,
    MAX_VOXEL_OBJECT_CONVERSION_RETAINED_SNAPSHOT_BYTES,
};

const STATIC_SOURCE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../fixtures/voxel-conversion/kenney-wall-a.glb"
));
const ANIMATED_SOURCE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../fixtures/render/assets/kenney-retro-character/character-medium.glb"
));

#[test]
fn static_object_plan_preview_apply_query_and_cli_are_hash_guarded() {
    let imported = import_mesh_source(&static_import_request()).unwrap();
    let request = object_request(
        &imported,
        "voxel-object/kenney-wall-a",
        Vec::new(),
        None,
        [6, 5, 4],
    );
    let prepared = plan_static_voxel_object_conversion(&request, &imported).unwrap();
    let repeated = plan_static_voxel_object_conversion(&request, &imported).unwrap();
    assert_eq!(prepared.plan(), repeated.plan());
    assert_eq!(prepared.candidate(), repeated.candidate());
    assert!(prepared.candidate().asset.clips.is_empty());
    assert_eq!(
        prepared.candidate().asset.provenance.kind,
        VoxelObjectProvenanceKind::ConvertedStaticMesh
    );

    let preview = preview_voxel_object_conversion(
        &VoxelObjectConversionPreviewRequest {
            plan_id: prepared.plan().plan_id.clone(),
            expected_plan_hash: prepared.plan().plan_hash.clone(),
            frame: VoxelObjectFrameSelection::Default,
            max_samples: 2,
        },
        &prepared,
    )
    .unwrap();
    assert_eq!(preview.sampled_frame_count, 1);
    assert_eq!(preview.stored_frame_count, 1);
    assert_eq!(preview.selected_frame.sample_voxels.len(), 2);
    assert!(preview.selected_frame.samples_truncated);

    let applied = apply_voxel_object_conversion(
        &VoxelObjectConversionApplyRequest {
            plan_id: prepared.plan().plan_id.clone(),
            expected_plan_hash: prepared.plan().plan_hash.clone(),
            expected_output_hash: Some(preview.output_hash.clone()),
        },
        &prepared,
    )
    .unwrap();
    assert_eq!(applied.conversion.asset, prepared.candidate().asset);
    let info = query_voxel_object_info(
        &applied.conversion.asset,
        &VoxelObjectInfoRequest {
            expected_content_hash: applied.output_hash.clone(),
        },
    )
    .unwrap();
    assert_eq!(info.total_stored_frame_count, 1);
    let frame = query_voxel_object_frame(
        &applied.conversion.asset,
        &VoxelObjectFrameRequest {
            expected_content_hash: applied.output_hash.clone(),
            frame: VoxelObjectFrameSelection::Default,
            include_material_counts: true,
        },
    )
    .unwrap();
    assert_eq!(
        frame
            .material_counts
            .iter()
            .map(|count| count.voxel_count)
            .sum::<usize>(),
        frame.voxel_count
    );
    let window = query_voxel_object_window(
        &applied.conversion.asset,
        &VoxelObjectWindowRequest {
            expected_content_hash: applied.output_hash.clone(),
            frame: VoxelObjectFrameSelection::Default,
            bounds: frame.bounds,
            include_empty: false,
            material_filter: Vec::new(),
            max_samples: 1,
        },
    )
    .unwrap();
    assert_eq!(window.samples.len(), 1);
    assert!(window.samples_truncated);

    let directory = temporary_directory("static-object");
    fs::create_dir(&directory).unwrap();
    let output = directory.join("wall.voxel-object.json");
    fs::write(&output, "known-good\n").unwrap();
    let stale = VoxelObjectConversionApplyRequest {
        plan_id: prepared.plan().plan_id.clone(),
        expected_plan_hash: hash('f'),
        expected_output_hash: None,
    };
    assert!(apply_voxel_object_conversion_and_install(&stale, &prepared, &output).is_err());
    assert_eq!(fs::read_to_string(&output).unwrap(), "known-good\n");
    assert!(!directory.join("wall.voxel-object.json.pending").exists());

    let request_path = directory.join("request.json");
    let source_path = directory.join("source.glb");
    fs::write(&request_path, serde_json::to_vec_pretty(&request).unwrap()).unwrap();
    fs::write(&source_path, STATIC_SOURCE).unwrap();
    let command = Command::new(env!("CARGO_BIN_EXE_voxel-object-convert"))
        .args([
            "--request",
            request_path.to_str().unwrap(),
            "--source",
            source_path.to_str().unwrap(),
            "--output",
            output.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        command.status.success(),
        "{}",
        String::from_utf8_lossy(&command.stderr)
    );
    assert!(String::from_utf8_lossy(&command.stdout).contains("sampledFrames=1"));
    assert_eq!(
        decode_voxel_object(&fs::read_to_string(&output).unwrap()).unwrap(),
        prepared.candidate().asset
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn static_piece_bakes_share_one_explicit_source_envelope() {
    let whole = import_mesh_source(&static_import_request()).unwrap();
    let shared_bounds = whole.receipt.metadata.source_bounds;
    let resolution = [24, 16, 8];
    let mut whole_request = object_request(
        &whole,
        "voxel-object/kenney-wall-whole",
        Vec::new(),
        None,
        resolution,
    );
    whole_request.settings.source_bounds = Some(shared_bounds);
    let whole_conversion = plan_static_voxel_object_conversion(&whole_request, &whole).unwrap();
    let whole_cells = frame_coordinates(&whole_conversion.candidate().asset);

    let mut assembled_cells = std::collections::BTreeSet::new();
    for group_index in 0..2 {
        let mut import = static_import_request();
        import.mesh_primitive = Some(format!("group/{group_index}"));
        let piece = import_mesh_source(&import).unwrap();
        let mut request = object_request(
            &piece,
            &format!("voxel-object/kenney-wall-piece-{group_index}"),
            Vec::new(),
            None,
            resolution,
        );
        request.settings.source_bounds = Some(shared_bounds);
        let encoded = serde_json::to_string(&request).unwrap();
        assert_eq!(
            decode_voxel_object_conversion_request(&encoded).unwrap(),
            request
        );
        let prepared = plan_static_voxel_object_conversion(&request, &piece).unwrap();
        let repeated = plan_static_voxel_object_conversion(&request, &piece).unwrap();
        assert_eq!(prepared.candidate(), repeated.candidate());
        assembled_cells.extend(frame_coordinates(&prepared.candidate().asset));
    }
    assert_eq!(assembled_cells, whole_cells);

    let mut expanded = whole_request.clone();
    expanded.settings.source_bounds = Some(MeshSourceBounds {
        min: shared_bounds.min.map(|component| component - 0.25),
        max: shared_bounds.max.map(|component| component + 0.25),
    });
    let expanded = plan_static_voxel_object_conversion(&expanded, &whole).unwrap();
    assert_ne!(
        expanded.plan().settings_sha256,
        whole_conversion.plan().settings_sha256
    );

    let mut invalid = whole_request.clone();
    invalid.settings.source_bounds = Some(MeshSourceBounds {
        min: shared_bounds.max,
        max: shared_bounds.min,
    });
    let error = plan_static_voxel_object_conversion(&invalid, &whole).unwrap_err();
    assert_eq!(error.diagnostics()[0].code, "conversion.invalidSettings");

    let mut excludes_source = whole_request;
    excludes_source.settings.source_bounds = Some(MeshSourceBounds {
        min: [0.0, 0.0, 0.0],
        max: [0.5, 0.5, 0.5],
    });
    let error = plan_static_voxel_object_conversion(&excludes_source, &whole).unwrap_err();
    assert_eq!(error.diagnostics()[0].code, "conversion.invalidGeometry");
}

#[test]
fn animated_clips_use_one_grid_stable_palette_and_durable_source_schedules() {
    let imported = import_animated_mesh_source(&animated_import_request()).unwrap();
    let clips = vec![VoxelObjectClipConversionRequest {
        source_clip_name: "run".to_owned(),
        output_clip_id: "locomotion/run".to_owned(),
        output_name: Some("Run".to_owned()),
        sample_rate_hz: 4,
        start_microseconds: 0,
        end_microseconds: None,
        end_policy: AnimationEndPolicy::ExcludeLoopSeam,
    }];
    let request = object_request(
        &imported.source,
        "voxel-object/retro-character",
        clips,
        Some("locomotion/run".to_owned()),
        [10, 14, 10],
    );
    let started = Instant::now();
    let prepared = plan_animated_voxel_object_conversion(&request, &imported).unwrap();
    let elapsed = started.elapsed();
    let repeated = plan_animated_voxel_object_conversion(&request, &imported).unwrap();
    assert_eq!(prepared.plan(), repeated.plan());
    assert_eq!(prepared.candidate().asset, repeated.candidate().asset);

    let receipt = prepared.candidate();
    assert_eq!(receipt.clips.len(), 1);
    assert_eq!(receipt.clips[0].source_clip_name, "run");
    assert_eq!(receipt.clips[0].output_clip_id, "locomotion/run");
    assert_eq!(receipt.clips[0].sampled_frame_count, 3);
    assert!(receipt.clips[0].stored_frame_count <= 3);
    assert_eq!(receipt.sampled_frames, 4);
    assert_eq!(
        receipt.asset.provenance.kind,
        VoxelObjectProvenanceKind::ConvertedAnimatedMesh
    );
    assert_eq!(receipt.asset.provenance.source_clips.len(), 1);
    assert_eq!(
        receipt.asset.provenance.source_clips[0].output_clip_id,
        "locomotion/run"
    );
    assert!(!receipt.asset.provenance.source_clips[0].included_clip_end);
    assert_eq!(receipt.artifact_bytes, receipt.canonical_json.len());
    eprintln!(
        "animated voxel object: {} bytes, {} sampled frames, {} stored frames, {:?}",
        receipt.artifact_bytes, receipt.sampled_frames, receipt.stored_frames, elapsed
    );

    let palette_slots = receipt
        .asset
        .material_palette
        .iter()
        .map(|binding| binding.material_slot)
        .collect::<std::collections::BTreeSet<_>>();
    for clip in &receipt.asset.clips {
        for frame in &clip.frames {
            let cells = resolve_voxel_frame(&frame.frame, palette_slots.iter().copied()).unwrap();
            assert!(cells
                .iter()
                .all(|cell| palette_slots.contains(&cell.material_slot)));
            assert!((0..3).all(|axis| {
                frame.frame.bounds.min[axis] >= 0
                    && frame.frame.bounds.max[axis]
                        < i64::from(request.settings.mesh.conversion.resolution[axis])
            }));
        }
    }

    let preview = preview_voxel_object_conversion(
        &VoxelObjectConversionPreviewRequest {
            plan_id: prepared.plan().plan_id.clone(),
            expected_plan_hash: prepared.plan().plan_hash.clone(),
            frame: VoxelObjectFrameSelection::Clip {
                clip_id: "locomotion/run".to_owned(),
                frame_index: 1,
            },
            max_samples: 3,
        },
        &prepared,
    )
    .unwrap();
    assert_eq!(preview.clips[0].sampled_frame_count, 3);
    assert_eq!(
        preview.selected_frame.duration_microseconds,
        Some(preview.clips[0].frames[1].duration_microseconds)
    );
    let queried = query_voxel_object_frame(
        &receipt.asset,
        &VoxelObjectFrameRequest {
            expected_content_hash: receipt.content_hash.clone(),
            frame: VoxelObjectFrameSelection::Clip {
                clip_id: "locomotion/run".to_owned(),
                frame_index: 1,
            },
            include_material_counts: false,
        },
    )
    .unwrap();
    assert_eq!(queried.bounds, preview.selected_frame.bounds);

    let mut locked_request = request.clone();
    locked_request.settings.anchor_policy = AnimationAnchorPolicy::LockNodeToBindPose {
        source_node_index: imported.model.scene.nodes[0].source_node_index,
    };
    let locked = plan_animated_voxel_object_conversion(&locked_request, &imported).unwrap();
    assert_ne!(
        locked.plan().settings_sha256,
        prepared.plan().settings_sha256
    );
    assert_eq!(
        locked.plan().settings.anchor_policy,
        locked_request.settings.anchor_policy
    );
}

#[test]
fn malformed_clip_budget_and_stale_requests_fail_before_install() {
    let imported = import_animated_mesh_source(&animated_import_request()).unwrap();
    let mut request = object_request(
        &imported.source,
        "voxel-object/retro-character-budget",
        vec![VoxelObjectClipConversionRequest {
            source_clip_name: "missing".to_owned(),
            output_clip_id: "missing".to_owned(),
            output_name: None,
            sample_rate_hz: 4,
            start_microseconds: 0,
            end_microseconds: None,
            end_policy: AnimationEndPolicy::ExcludeLoopSeam,
        }],
        None,
        [4, 4, 4],
    );
    let error = plan_animated_voxel_object_conversion(&request, &imported).unwrap_err();
    assert_eq!(error.diagnostics()[0].code, "conversion.clipNotFound");

    request.clips[0].source_clip_name = "run".to_owned();
    request.clips[0].sample_rate_hz = 241;
    let error = plan_animated_voxel_object_conversion(&request, &imported).unwrap_err();
    assert_eq!(error.diagnostics()[0].code, "conversion.resourceLimit");

    let encoded = serde_json::to_string(&request).unwrap();
    let mut malformed: serde_json::Value = serde_json::from_str(&encoded).unwrap();
    malformed["unknown"] = true.into();
    let error = decode_voxel_object_conversion_request(&malformed.to_string()).unwrap_err();
    assert_eq!(error.diagnostics()[0].code, "conversion.requestDecode");

    let mut malformed_anchor: serde_json::Value = serde_json::from_str(&encoded).unwrap();
    malformed_anchor["settings"]["anchorPolicy"]["unknown"] = true.into();
    let error = decode_voxel_object_conversion_request(&malformed_anchor.to_string()).unwrap_err();
    assert_eq!(error.diagnostics()[0].code, "conversion.requestDecode");
}

#[test]
fn aggregate_clip_snapshot_storage_is_bounded_before_accumulation() {
    let source_bytes = repeated_triangle_animation_glb();
    let imported = import_animated_mesh_source(&MeshSourceImportRequest {
        source_asset_id: "mesh-animation/repeated-triangles".to_owned(),
        asset_version: 1,
        source_path: "fixtures/generated/repeated-triangles.glb".to_owned(),
        format: MeshSourceFormat::Glb,
        source_bytes,
        expected_source_sha256: None,
        mesh_primitive: None,
    })
    .unwrap();
    assert_eq!(
        imported.model.scene.meshes[0].primitives[0].positions.len(),
        4
    );
    assert_eq!(
        imported.model.scene.meshes[0].primitives[0].indices.len(),
        30_000
    );

    let one_sample = sample_animation_clip_range(
        &imported.model,
        &AnimationSampleRangeRequest {
            expected_source_sha256: imported.model.source_sha256.clone(),
            clip_name: "move".to_owned(),
            sample_rate_hz: 1,
            start_microseconds: 0,
            end_microseconds: 0,
            end_policy: AnimationEndPolicy::IncludeClipEnd,
            anchor_policy: AnimationAnchorPolicy::PreserveSourceSpace,
        },
    )
    .unwrap();
    assert_eq!(one_sample.snapshots.len(), 1);
    assert!(
        one_sample.estimated_materialized_snapshot_bytes
            < MAX_VOXEL_OBJECT_CONVERSION_RETAINED_SNAPSHOT_BYTES
    );

    let clips = (0..256)
        .map(|index| VoxelObjectClipConversionRequest {
            source_clip_name: "move".to_owned(),
            output_clip_id: format!("variants/clip-{index}"),
            output_name: None,
            sample_rate_hz: 1,
            start_microseconds: 0,
            end_microseconds: Some(0),
            end_policy: AnimationEndPolicy::IncludeClipEnd,
        })
        .collect();
    let request = object_request(
        &imported.source,
        "voxel-object/repeated-triangles",
        clips,
        None,
        [2, 2, 2],
    );
    let error = plan_animated_voxel_object_conversion(&request, &imported).unwrap_err();
    let diagnostic = &error.diagnostics()[0];
    assert_eq!(diagnostic.code, "conversion.resourceLimit");
    assert_eq!(diagnostic.path, "clips.snapshotStorage");
    assert!(diagnostic.message.contains(&format!(
        "limit is {MAX_VOXEL_OBJECT_CONVERSION_RETAINED_SNAPSHOT_BYTES}"
    )));
}

#[test]
fn identical_quantized_frames_merge_without_changing_the_selected_timing() {
    let imported = import_animated_mesh_source(&animated_import_request()).unwrap();
    let request = object_request(
        &imported.source,
        "voxel-object/retro-character-dedup",
        vec![VoxelObjectClipConversionRequest {
            source_clip_name: "idle".to_owned(),
            output_clip_id: "idle".to_owned(),
            output_name: None,
            sample_rate_hz: 24,
            start_microseconds: 0,
            end_microseconds: None,
            end_policy: AnimationEndPolicy::ExcludeLoopSeam,
        }],
        Some("idle".to_owned()),
        [4, 4, 4],
    );
    let started = Instant::now();
    let prepared = plan_animated_voxel_object_conversion(&request, &imported).unwrap();
    let elapsed = started.elapsed();
    let clip = &prepared.candidate().clips[0];
    assert!(clip.stored_frame_count < clip.sampled_frame_count);
    assert_eq!(
        clip.frames
            .iter()
            .map(|frame| frame.source_timestamps_microseconds.len())
            .sum::<usize>(),
        clip.sampled_frame_count
    );
    assert_eq!(
        clip.duration_microseconds,
        clip.end_microseconds - clip.start_microseconds
    );
    eprintln!(
        "idle dedup: {} bytes, {} sampled frames, {} stored frames, {:?}",
        prepared.candidate().artifact_bytes,
        clip.sampled_frame_count,
        clip.stored_frame_count,
        elapsed
    );
}

fn static_import_request() -> MeshSourceImportRequest {
    MeshSourceImportRequest {
        source_asset_id: "mesh/kenney-wall-a".to_owned(),
        asset_version: 1,
        source_path: "fixtures/voxel-conversion/kenney-wall-a.glb".to_owned(),
        format: MeshSourceFormat::Glb,
        source_bytes: STATIC_SOURCE.to_vec(),
        expected_source_sha256: None,
        mesh_primitive: None,
    }
}

fn frame_coordinates(
    object: &voxel_asset::VoxelObjectAsset,
) -> std::collections::BTreeSet<[i64; 3]> {
    let material_slots = object
        .material_palette
        .iter()
        .map(|binding| binding.material_slot);
    resolve_voxel_frame(&object.default_frame, material_slots)
        .unwrap()
        .into_iter()
        .map(|cell| cell.coordinate)
        .collect()
}

fn animated_import_request() -> MeshSourceImportRequest {
    MeshSourceImportRequest {
        source_asset_id: "mesh-animation/retro-character".to_owned(),
        asset_version: 1,
        source_path: "fixtures/render/assets/kenney-retro-character/character-medium.glb"
            .to_owned(),
        format: MeshSourceFormat::Glb,
        source_bytes: ANIMATED_SOURCE.to_vec(),
        expected_source_sha256: None,
        mesh_primitive: None,
    }
}

fn object_request(
    source: &voxel_convert::ImportedMeshSource,
    target_asset_id: &str,
    clips: Vec<VoxelObjectClipConversionRequest>,
    default_clip: Option<String>,
    resolution: [u32; 3],
) -> VoxelObjectConversionPlanRequest {
    let mut material_palette = Vec::new();
    let mut material_map = Vec::new();
    for (index, material) in source.mesh.materials.iter().enumerate() {
        let voxel_material_slot = u16::try_from(index + 1).unwrap();
        material_palette.push(VoxelAssetMaterialBinding {
            material_slot: voxel_material_slot,
            material_asset_id: format!("material/converted-slot-{index}"),
            display_name: material.source_material_name.clone(),
        });
        material_map.push(VoxelAssetMaterialMapping {
            source_material_slot: material.source_material_slot,
            source_material_name: material.source_material_name.clone(),
            voxel_material_slot,
        });
    }
    VoxelObjectConversionPlanRequest {
        source: source.receipt.source.clone(),
        source_path: source.receipt.source_path.clone(),
        target_asset_id: target_asset_id.to_owned(),
        license_path: Some("fixtures/LICENSE.txt".to_owned()),
        settings: VoxelObjectConversionSettings {
            mesh: ConversionPlanSettings {
                conversion: VoxelConversionSettings {
                    resolution,
                    cell_size: 1.0,
                    chunk_size: 16,
                    origin: [0, 0, 0],
                    fit_policy: VoxelConversionFitPolicy::Contain,
                    origin_policy: VoxelConversionOriginPolicy::Centered,
                    mode: VoxelConversionMode::Surface,
                    material_palette,
                    material_map,
                    max_output_voxels: resolution.into_iter().product(),
                },
                transform: identity_transform(),
                material_policy: ConversionMaterialPolicy::default(),
            },
            source_bounds: None,
            pivot: [
                f64::from(resolution[0].saturating_sub(1)) / 2.0,
                0.0,
                f64::from(resolution[2].saturating_sub(1)) / 2.0,
            ],
            anchor_policy: AnimationAnchorPolicy::PreserveSourceSpace,
        },
        clips,
        default_clip,
    }
}

fn repeated_triangle_animation_glb() -> Vec<u8> {
    let mut bin = Vec::new();

    let position_offset = bin.len();
    for value in [
        -1.0f32, 0.0, 0.0, // vertex 0
        1.0, 0.0, 0.0, // vertex 1
        1.0, 2.0, 0.0, // vertex 2
        -1.0, 2.0, 0.0, // vertex 3
    ] {
        bin.extend_from_slice(&value.to_le_bytes());
    }
    let position_bytes = bin.len() - position_offset;

    let index_offset = bin.len();
    for _ in 0..5_000 {
        for index in [0u16, 1, 2, 0, 2, 3] {
            bin.extend_from_slice(&index.to_le_bytes());
        }
    }
    let index_bytes = bin.len() - index_offset;

    let time_offset = bin.len();
    for value in [0.0f32, 1.0] {
        bin.extend_from_slice(&value.to_le_bytes());
    }
    let time_bytes = bin.len() - time_offset;

    let translation_offset = bin.len();
    for value in [0.0f32, 0.0, 0.0, 0.25, 0.0, 0.0] {
        bin.extend_from_slice(&value.to_le_bytes());
    }
    let translation_bytes = bin.len() - translation_offset;

    let document = serde_json::json!({
        "asset": {"version": "2.0", "generator": "rusty-engine-cc0-test-corpus"},
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": [{"name": "moving-quad", "mesh": 0}],
        "meshes": [{
            "name": "repeated-triangle-quad",
            "primitives": [{
                "attributes": {"POSITION": 0},
                "indices": 1,
                "material": 0,
                "mode": 4
            }]
        }],
        "materials": [{"name": "repeated-triangle-material"}],
        "animations": [{
            "name": "move",
            "samplers": [{"input": 2, "output": 3, "interpolation": "LINEAR"}],
            "channels": [{"sampler": 0, "target": {"node": 0, "path": "translation"}}]
        }],
        "buffers": [{"byteLength": bin.len()}],
        "bufferViews": [
            {"buffer": 0, "byteOffset": position_offset, "byteLength": position_bytes, "target": 34962},
            {"buffer": 0, "byteOffset": index_offset, "byteLength": index_bytes, "target": 34963},
            {"buffer": 0, "byteOffset": time_offset, "byteLength": time_bytes},
            {"buffer": 0, "byteOffset": translation_offset, "byteLength": translation_bytes}
        ],
        "accessors": [
            {"bufferView": 0, "componentType": 5126, "count": 4, "type": "VEC3", "min": [-1.0, 0.0, 0.0], "max": [1.0, 2.0, 0.0]},
            {"bufferView": 1, "componentType": 5123, "count": 30_000, "type": "SCALAR"},
            {"bufferView": 2, "componentType": 5126, "count": 2, "type": "SCALAR", "min": [0.0], "max": [1.0]},
            {"bufferView": 3, "componentType": 5126, "count": 2, "type": "VEC3"}
        ]
    });
    encode_glb(document, bin)
}

fn encode_glb(document: serde_json::Value, mut bin: Vec<u8>) -> Vec<u8> {
    let mut json = serde_json::to_vec(&document).unwrap();
    align_four(&mut json, b' ');
    align_four(&mut bin, 0);
    let total_length = 12 + 8 + json.len() + 8 + bin.len();
    let mut glb = Vec::with_capacity(total_length);
    glb.extend_from_slice(b"glTF");
    glb.extend_from_slice(&2u32.to_le_bytes());
    glb.extend_from_slice(&(total_length as u32).to_le_bytes());
    glb.extend_from_slice(&(json.len() as u32).to_le_bytes());
    glb.extend_from_slice(b"JSON");
    glb.extend_from_slice(&json);
    glb.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    glb.extend_from_slice(b"BIN\0");
    glb.extend_from_slice(&bin);
    glb
}

fn align_four(bytes: &mut Vec<u8>, padding: u8) {
    while !bytes.len().is_multiple_of(4) {
        bytes.push(padding);
    }
}

fn hash(digit: char) -> String {
    format!("sha256:{}", digit.to_string().repeat(64))
}

fn temporary_directory(label: &str) -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "rusty-engine-{label}-{}-{unique}",
        std::process::id()
    ))
}
