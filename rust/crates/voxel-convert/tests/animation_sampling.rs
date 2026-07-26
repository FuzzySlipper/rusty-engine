use voxel_convert::{
    import_animated_glb, sample_animation_bind_pose, sample_animation_clip,
    sample_animation_clip_range, AnimationAnchorPolicy, AnimationBindPoseRequest,
    AnimationChannelValues, AnimationEndPolicy, AnimationSampleRangeRequest,
    AnimationSampleRequest, MAX_ANIMATION_MATERIALIZED_SNAPSHOT_BYTES,
    MAX_IMPORTED_ANIMATION_CLIPS,
};

const CHARACTER: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../fixtures/render/assets/kenney-retro-character/character-medium.glb"
));
const MORPH_FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../fixtures/voxel-conversion/morph-animation.fixture.json"
));

#[test]
fn licensed_character_imports_named_clips_skin_and_deterministic_samples() {
    let model = import_animated_glb(CHARACTER).unwrap();
    assert_eq!(model.nodes.len(), 61);
    assert_eq!(model.scene.meshes.len(), 1);
    assert_eq!(model.skins.len(), 1);
    assert_eq!(model.skins[0].joint_node_indices.len(), 45);
    assert_eq!(
        model
            .clips
            .iter()
            .map(|clip| clip.name.as_str())
            .collect::<Vec<_>>(),
        vec!["idle", "run", "jump"]
    );

    let bind = sample_animation_bind_pose(
        &model,
        &AnimationBindPoseRequest {
            expected_source_sha256: model.source_sha256.clone(),
            anchor_policy: AnimationAnchorPolicy::PreserveSourceSpace,
        },
    )
    .unwrap();
    assert!(!bind.mesh.positions.is_empty());
    assert!(bind
        .mesh
        .positions
        .iter()
        .flatten()
        .all(|value| value.is_finite()));

    let request = AnimationSampleRequest {
        expected_source_sha256: model.source_sha256.clone(),
        clip_name: "run".to_owned(),
        sample_rate_hz: 24,
        end_policy: AnimationEndPolicy::IncludeClipEnd,
        anchor_policy: AnimationAnchorPolicy::PreserveSourceSpace,
    };
    let sampled = sample_animation_clip(&model, &request).unwrap();
    assert_eq!(sampled.duration_microseconds, 666_667);
    assert_eq!(sampled.snapshots.len(), 17);
    assert_eq!(
        sampled.estimated_materialized_snapshot_bytes,
        bind.estimated_materialized_snapshot_bytes * sampled.snapshots.len() as u64
    );
    assert!(
        sampled.estimated_materialized_snapshot_bytes <= MAX_ANIMATION_MATERIALIZED_SNAPSHOT_BYTES
    );
    assert_eq!(sampled.snapshots.first().unwrap().timestamp_microseconds, 0);
    assert_eq!(
        sampled.snapshots.last().unwrap().timestamp_microseconds,
        sampled.duration_microseconds
    );
    assert_eq!(sampled, sample_animation_clip(&model, &request).unwrap());
    assert_eq!(
        bind.mesh.primitive_groups,
        sampled.snapshots[0].mesh.primitive_groups
    );
    assert_eq!(bind.mesh.materials, sampled.snapshots[0].mesh.materials);
    assert_eq!(bind.mesh.triangles, sampled.snapshots[0].mesh.triangles);
    assert!(model.primitive_deformations.iter().any(|primitive| {
        primitive.vertex_joints.is_some() && primitive.vertex_weights.is_some()
    }));
    assert_ne!(
        sampled.snapshots[0].mesh.positions,
        sampled.snapshots[8].mesh.positions
    );
    assert_point_near(
        bind.mesh.positions[0],
        [0.439_315_728_1, 3.472_003_063_5, 0.009_933_417_8],
    );
    assert_point_near(
        sampled.snapshots[8].mesh.positions[0],
        [0.462_027_882_4, 3.334_096_302_1, 0.356_896_480_3],
    );

    let loop_sampled = sample_animation_clip(
        &model,
        &AnimationSampleRequest {
            end_policy: AnimationEndPolicy::ExcludeLoopSeam,
            ..request
        },
    )
    .unwrap();
    assert_eq!(loop_sampled.snapshots.len(), 16);
    assert!(
        loop_sampled
            .snapshots
            .last()
            .unwrap()
            .timestamp_microseconds
            < loop_sampled.duration_microseconds
    );
}

#[test]
fn clip_ranges_sample_absolute_times_without_rescaling_the_selected_interval() {
    let source = morph_fixture_glb();
    let model = import_animated_glb(&source).unwrap();
    let request = AnimationSampleRangeRequest {
        expected_source_sha256: model.source_sha256.clone(),
        clip_name: "morph-linear".to_owned(),
        sample_rate_hz: 2,
        start_microseconds: 250_000,
        end_microseconds: 750_000,
        end_policy: AnimationEndPolicy::IncludeClipEnd,
        anchor_policy: AnimationAnchorPolicy::PreserveSourceSpace,
    };
    let sampled = sample_animation_clip_range(&model, &request).unwrap();
    assert_eq!(sampled.start_microseconds, 250_000);
    assert_eq!(sampled.end_microseconds, 750_000);
    assert_eq!(
        sampled
            .snapshots
            .iter()
            .map(|snapshot| snapshot.timestamp_microseconds)
            .collect::<Vec<_>>(),
        vec![250_000, 750_000]
    );
    assert_eq!(sampled.snapshots[0].mesh.positions[0], [-0.5, 0.0, 0.0]);
    assert_eq!(sampled.snapshots[1].mesh.positions[0], [0.5, 0.0, 0.0]);
    assert_eq!(
        sampled,
        sample_animation_clip_range(&model, &request).unwrap()
    );

    let error = sample_animation_clip_range(
        &model,
        &AnimationSampleRangeRequest {
            start_microseconds: 750_001,
            end_microseconds: 750_000,
            ..request
        },
    )
    .unwrap_err();
    assert_eq!(error.diagnostics()[0].code, "conversion.invalidSampleRange");
}

#[test]
fn cc0_morph_corpus_proves_known_times_interpolation_seams_and_anchor_locking() {
    let source = morph_fixture_glb();
    let model = import_animated_glb(&source).unwrap();
    assert_eq!(
        model
            .clips
            .iter()
            .map(|clip| clip.name.as_str())
            .collect::<Vec<_>>(),
        vec!["morph-linear", "translation-step", "translation-cubic"]
    );
    assert_eq!(
        model.primitive_deformations[0].morph_position_deltas.len(),
        1
    );

    let bind = sample_animation_bind_pose(
        &model,
        &AnimationBindPoseRequest {
            expected_source_sha256: model.source_sha256.clone(),
            anchor_policy: AnimationAnchorPolicy::PreserveSourceSpace,
        },
    )
    .unwrap();
    assert_eq!(bind.mesh.positions[0], [-1.0, 0.0, 0.0]);
    assert_eq!(bind.mesh.positions[2], [1.0, 2.0, 0.0]);

    let preserve = sample_animation_clip(
        &model,
        &AnimationSampleRequest {
            expected_source_sha256: model.source_sha256.clone(),
            clip_name: "morph-linear".to_owned(),
            sample_rate_hz: 2,
            end_policy: AnimationEndPolicy::IncludeClipEnd,
            anchor_policy: AnimationAnchorPolicy::PreserveSourceSpace,
        },
    )
    .unwrap();
    assert_eq!(
        preserve
            .snapshots
            .iter()
            .map(|snapshot| snapshot.timestamp_microseconds)
            .collect::<Vec<_>>(),
        vec![0, 500_000, 1_000_000]
    );
    assert_eq!(preserve.snapshots[1].mesh.positions[0], [0.0, 0.0, 0.0]);
    assert_eq!(preserve.snapshots[1].mesh.positions[2], [2.0, 2.0, 1.0]);
    assert_eq!(preserve.snapshots[2].mesh.positions[2], [3.0, 2.0, 0.0]);

    let locked = sample_animation_clip(
        &model,
        &AnimationSampleRequest {
            expected_source_sha256: model.source_sha256.clone(),
            clip_name: "morph-linear".to_owned(),
            sample_rate_hz: 2,
            end_policy: AnimationEndPolicy::IncludeClipEnd,
            anchor_policy: AnimationAnchorPolicy::LockNodeToBindPose {
                source_node_index: 0,
            },
        },
    )
    .unwrap();
    assert_eq!(locked.snapshots[1].mesh.positions[0], [-1.0, 0.0, 0.0]);
    assert_eq!(locked.snapshots[1].mesh.positions[2], [1.0, 2.0, 1.0]);
    assert_eq!(locked.snapshots[0].mesh, locked.snapshots[2].mesh);

    let loop_schedule = sample_animation_clip(
        &model,
        &AnimationSampleRequest {
            expected_source_sha256: model.source_sha256.clone(),
            clip_name: "morph-linear".to_owned(),
            sample_rate_hz: 2,
            end_policy: AnimationEndPolicy::ExcludeLoopSeam,
            anchor_policy: AnimationAnchorPolicy::LockNodeToBindPose {
                source_node_index: 0,
            },
        },
    )
    .unwrap();
    assert_eq!(loop_schedule.snapshots.len(), 2);
    assert_eq!(loop_schedule.snapshots[1].timestamp_microseconds, 500_000);

    let step = sample_fixture_clip(&model, "translation-step");
    assert_eq!(step.snapshots[1].mesh.positions[0], [-1.0, 0.0, 0.0]);
    assert_eq!(step.snapshots[2].mesh.positions[0], [1.0, 0.0, 0.0]);
    let cubic = sample_fixture_clip(&model, "translation-cubic");
    assert_eq!(cubic.snapshots[1].mesh.positions[0], [0.0, 0.0, 0.0]);
}

#[test]
fn animated_sampling_rejects_stale_absent_unsupported_and_bounded_work() {
    let source = morph_fixture_glb();
    let model = import_animated_glb(&source).unwrap();
    let error = sample_animation_clip(
        &model,
        &AnimationSampleRequest {
            expected_source_sha256: "sha256:stale".to_owned(),
            clip_name: "morph-linear".to_owned(),
            sample_rate_hz: 2,
            end_policy: AnimationEndPolicy::IncludeClipEnd,
            anchor_policy: AnimationAnchorPolicy::PreserveSourceSpace,
        },
    )
    .unwrap_err();
    assert_eq!(error.diagnostics()[0].code, "conversion.sourceHashMismatch");

    let error = sample_animation_clip(
        &model,
        &AnimationSampleRequest {
            expected_source_sha256: model.source_sha256.clone(),
            clip_name: "missing".to_owned(),
            sample_rate_hz: 2,
            end_policy: AnimationEndPolicy::IncludeClipEnd,
            anchor_policy: AnimationAnchorPolicy::PreserveSourceSpace,
        },
    )
    .unwrap_err();
    assert_eq!(error.diagnostics()[0].code, "conversion.clipNotFound");

    let unsupported = mutate_glb(&source, |document, _bin| {
        let joints = document["meshes"][0]["primitives"][0]["attributes"]["POSITION"].clone();
        document["meshes"][0]["primitives"][0]["attributes"]["JOINTS_1"] = joints;
    });
    let error = import_animated_glb(&unsupported).unwrap_err();
    assert_eq!(error.diagnostics()[0].code, "conversion.unsupportedFeature");

    let mut excessive_clips = source.clone();
    excessive_clips = mutate_glb(&excessive_clips, |document, _bin| {
        let template = document["animations"][0].clone();
        document["animations"] = serde_json::Value::Array(
            (0..=MAX_IMPORTED_ANIMATION_CLIPS)
                .map(|index| {
                    let mut clip = template.clone();
                    clip["name"] = format!("clip-{index}").into();
                    clip
                })
                .collect(),
        );
    });
    let error = import_animated_glb(&excessive_clips).unwrap_err();
    assert_eq!(error.diagnostics()[0].code, "conversion.resourceLimit");
    assert_eq!(error.diagnostics()[0].path, "source.animations");

    let mut excessive_frames = model.clone();
    excessive_frames.clips[0].duration_microseconds = 20_000_000;
    let error = sample_animation_clip(
        &excessive_frames,
        &AnimationSampleRequest {
            expected_source_sha256: excessive_frames.source_sha256.clone(),
            clip_name: "morph-linear".to_owned(),
            sample_rate_hz: 240,
            end_policy: AnimationEndPolicy::IncludeClipEnd,
            anchor_policy: AnimationAnchorPolicy::PreserveSourceSpace,
        },
    )
    .unwrap_err();
    assert_eq!(error.diagnostics()[0].code, "conversion.resourceLimit");
    assert_eq!(error.diagnostics()[0].path, "request.sampleSchedule");

    let character = import_animated_glb(CHARACTER).unwrap();
    let mut excessive_work = character.clone();
    excessive_work.clips[1].duration_microseconds = 10_000_000;
    let error = sample_animation_clip(
        &excessive_work,
        &AnimationSampleRequest {
            expected_source_sha256: excessive_work.source_sha256.clone(),
            clip_name: "run".to_owned(),
            sample_rate_hz: 240,
            end_policy: AnimationEndPolicy::IncludeClipEnd,
            anchor_policy: AnimationAnchorPolicy::PreserveSourceSpace,
        },
    )
    .unwrap_err();
    assert_eq!(error.diagnostics()[0].code, "conversion.resourceLimit");
    assert_eq!(error.diagnostics()[0].path, "request.deformationWork");
}

#[test]
fn animation_snapshot_storage_is_bounded_before_topology_is_materialized() {
    let source = repeated_triangle_animation_glb();
    let model = import_animated_glb(&source).unwrap();
    assert_eq!(model.scene.meshes[0].primitives[0].positions.len(), 4);
    assert_eq!(model.scene.meshes[0].primitives[0].indices.len(), 30_000);

    let error = sample_animation_clip(
        &model,
        &AnimationSampleRequest {
            expected_source_sha256: model.source_sha256.clone(),
            clip_name: "morph-linear".to_owned(),
            sample_rate_hz: 240,
            end_policy: AnimationEndPolicy::IncludeClipEnd,
            anchor_policy: AnimationAnchorPolicy::PreserveSourceSpace,
        },
    )
    .unwrap_err();
    let diagnostic = &error.diagnostics()[0];
    assert_eq!(diagnostic.code, "conversion.resourceLimit");
    assert_eq!(diagnostic.path, "request.snapshotStorage");
    assert!(diagnostic.message.contains(&format!(
        "limit is {MAX_ANIMATION_MATERIALIZED_SNAPSHOT_BYTES}"
    )));
}

#[test]
fn animated_import_rejects_bad_joint_weight_and_non_finite_deformation_data() {
    let bad_joint = mutate_glb(CHARACTER, |document, bin| {
        overwrite_first_accessor_component(document, bin, 5, ComponentMutation::Maximum);
    });
    let error = import_animated_glb(&bad_joint).unwrap_err();
    assert_eq!(error.diagnostics()[0].code, "conversion.invalidSkin");

    let bad_weight = mutate_glb(CHARACTER, |document, bin| {
        overwrite_first_accessor_element_with_zero(document, bin, 6);
    });
    let error = import_animated_glb(&bad_weight).unwrap_err();
    assert_eq!(error.diagnostics()[0].code, "conversion.invalidSkin");

    let bad_accessor = mutate_glb(CHARACTER, |document, _bin| {
        let count = document["accessors"][6]["count"].as_u64().unwrap();
        document["accessors"][6]["count"] = (count - 1).into();
    });
    let error = import_animated_glb(&bad_accessor).unwrap_err();
    assert!(matches!(
        error.diagnostics()[0].code,
        "conversion.invalidSource" | "conversion.invalidSkin" | "conversion.invalidAccessor"
    ));

    let source = morph_fixture_glb();
    let mut non_finite = import_animated_glb(&source).unwrap();
    non_finite.primitive_deformations[0].morph_position_deltas[0][2][2] = f64::MAX;
    let AnimationChannelValues::MorphWeights { values, .. } =
        &mut non_finite.clips[0].channels[0].values
    else {
        panic!("fixture first channel is morph weights");
    };
    values[1] = f64::MAX;
    let error = sample_animation_clip(
        &non_finite,
        &AnimationSampleRequest {
            expected_source_sha256: non_finite.source_sha256.clone(),
            clip_name: "morph-linear".to_owned(),
            sample_rate_hz: 2,
            end_policy: AnimationEndPolicy::IncludeClipEnd,
            anchor_policy: AnimationAnchorPolicy::PreserveSourceSpace,
        },
    )
    .unwrap_err();
    assert_eq!(
        error.diagnostics()[0].code,
        "conversion.nonFiniteDeformation"
    );
}

fn sample_fixture_clip(
    model: &voxel_convert::ImportedAnimatedModel,
    clip_name: &str,
) -> voxel_convert::AnimationSampleReceipt {
    sample_animation_clip(
        model,
        &AnimationSampleRequest {
            expected_source_sha256: model.source_sha256.clone(),
            clip_name: clip_name.to_owned(),
            sample_rate_hz: 2,
            end_policy: AnimationEndPolicy::IncludeClipEnd,
            anchor_policy: AnimationAnchorPolicy::PreserveSourceSpace,
        },
    )
    .unwrap()
}

fn morph_fixture_glb() -> Vec<u8> {
    let fixture: serde_json::Value = serde_json::from_str(MORPH_FIXTURE).unwrap();
    let mut bin = Vec::new();
    let mut buffer_views = Vec::new();
    let mut accessors = Vec::new();

    let positions = flattened_f32(&fixture["positions"]);
    let position_accessor = push_f32_accessor(
        &mut bin,
        &mut buffer_views,
        &mut accessors,
        &positions,
        4,
        "VEC3",
        Some(serde_json::json!([-1.0, 0.0, 0.0])),
        Some(serde_json::json!([1.0, 2.0, 0.0])),
        Some(34_962),
    );
    let indices = fixture["indices"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_u64().unwrap() as u16)
        .collect::<Vec<_>>();
    let index_accessor = push_u16_accessor(&mut bin, &mut buffer_views, &mut accessors, &indices);
    let morph_accessor = push_f32_accessor(
        &mut bin,
        &mut buffer_views,
        &mut accessors,
        &flattened_f32(&fixture["morphPositionDeltas"]),
        4,
        "VEC3",
        None,
        None,
        Some(34_962),
    );
    let morph_time_accessor = push_f32_accessor(
        &mut bin,
        &mut buffer_views,
        &mut accessors,
        &flattened_f32(&fixture["morphTimes"]),
        3,
        "SCALAR",
        Some(serde_json::json!([0.0])),
        Some(serde_json::json!([1.0])),
        None,
    );
    let morph_weight_accessor = push_f32_accessor(
        &mut bin,
        &mut buffer_views,
        &mut accessors,
        &flattened_f32(&fixture["morphWeights"]),
        3,
        "SCALAR",
        None,
        None,
        None,
    );
    let translation_time_accessor = push_f32_accessor(
        &mut bin,
        &mut buffer_views,
        &mut accessors,
        &flattened_f32(&fixture["translationTimes"]),
        2,
        "SCALAR",
        Some(serde_json::json!([0.0])),
        Some(serde_json::json!([1.0])),
        None,
    );
    let translation_accessor = push_f32_accessor(
        &mut bin,
        &mut buffer_views,
        &mut accessors,
        &flattened_f32(&fixture["translationValues"]),
        2,
        "VEC3",
        None,
        None,
        None,
    );
    let cubic_accessor = push_f32_accessor(
        &mut bin,
        &mut buffer_views,
        &mut accessors,
        &[
            0.0, 0.0, 0.0, // key 0 in tangent
            0.0, 0.0, 0.0, // key 0 value
            0.0, 0.0, 0.0, // key 0 out tangent
            0.0, 0.0, 0.0, // key 1 in tangent
            2.0, 0.0, 0.0, // key 1 value
            0.0, 0.0, 0.0, // key 1 out tangent
        ],
        6,
        "VEC3",
        None,
        None,
        None,
    );

    let document = serde_json::json!({
        "asset": {"version": "2.0", "generator": "rusty-engine-cc0-test-corpus"},
        "scene": 0,
        "scenes": [{"name": "morph-animation", "nodes": [0]}],
        "nodes": [{"name": "morph-anchor", "mesh": 0, "weights": [0.0]}],
        "meshes": [{
            "name": "morph-quad",
            "weights": [0.0],
            "primitives": [{
                "attributes": {"POSITION": position_accessor},
                "indices": index_accessor,
                "mode": 4,
                "targets": [{"POSITION": morph_accessor}]
            }]
        }],
        "animations": [
            {
                "name": "morph-linear",
                "samplers": [
                    {"input": morph_time_accessor, "output": morph_weight_accessor, "interpolation": "LINEAR"},
                    {"input": translation_time_accessor, "output": translation_accessor, "interpolation": "LINEAR"}
                ],
                "channels": [
                    {"sampler": 0, "target": {"node": 0, "path": "weights"}},
                    {"sampler": 1, "target": {"node": 0, "path": "translation"}}
                ]
            },
            {
                "name": "translation-step",
                "samplers": [
                    {"input": translation_time_accessor, "output": translation_accessor, "interpolation": "STEP"}
                ],
                "channels": [
                    {"sampler": 0, "target": {"node": 0, "path": "translation"}}
                ]
            },
            {
                "name": "translation-cubic",
                "samplers": [
                    {"input": translation_time_accessor, "output": cubic_accessor, "interpolation": "CUBICSPLINE"}
                ],
                "channels": [
                    {"sampler": 0, "target": {"node": 0, "path": "translation"}}
                ]
            }
        ],
        "buffers": [{"byteLength": bin.len()}],
        "bufferViews": buffer_views,
        "accessors": accessors
    });
    encode_glb(document, bin)
}

fn repeated_triangle_animation_glb() -> Vec<u8> {
    mutate_glb(&morph_fixture_glb(), |document, bin| {
        let accessor_index = document["meshes"][0]["primitives"][0]["indices"]
            .as_u64()
            .unwrap() as usize;
        align_four(bin, 0);
        let byte_offset = bin.len();
        for _ in 0..5_000 {
            for index in [0u16, 1, 2, 0, 2, 3] {
                bin.extend_from_slice(&index.to_le_bytes());
            }
        }
        let buffer_views = document["bufferViews"].as_array_mut().unwrap();
        let view_index = buffer_views.len();
        buffer_views.push(serde_json::json!({
            "buffer": 0,
            "byteOffset": byte_offset,
            "byteLength": 30_000 * 2,
            "target": 34963
        }));
        let accessor = document["accessors"][accessor_index]
            .as_object_mut()
            .unwrap();
        accessor.insert("bufferView".to_owned(), view_index.into());
        accessor.remove("byteOffset");
        accessor.insert("count".to_owned(), 30_000.into());
    })
}

#[allow(clippy::too_many_arguments)]
fn push_f32_accessor(
    bin: &mut Vec<u8>,
    buffer_views: &mut Vec<serde_json::Value>,
    accessors: &mut Vec<serde_json::Value>,
    values: &[f32],
    count: usize,
    accessor_type: &str,
    min: Option<serde_json::Value>,
    max: Option<serde_json::Value>,
    target: Option<u32>,
) -> usize {
    align_four(bin, 0);
    let offset = bin.len();
    for value in values {
        bin.extend_from_slice(&value.to_le_bytes());
    }
    let mut view = serde_json::json!({
        "buffer": 0,
        "byteOffset": offset,
        "byteLength": values.len() * 4
    });
    if let Some(target) = target {
        view["target"] = target.into();
    }
    let view_index = buffer_views.len();
    buffer_views.push(view);
    let mut accessor = serde_json::json!({
        "bufferView": view_index,
        "componentType": 5126,
        "count": count,
        "type": accessor_type
    });
    if let Some(min) = min {
        accessor["min"] = min;
    }
    if let Some(max) = max {
        accessor["max"] = max;
    }
    let accessor_index = accessors.len();
    accessors.push(accessor);
    accessor_index
}

fn push_u16_accessor(
    bin: &mut Vec<u8>,
    buffer_views: &mut Vec<serde_json::Value>,
    accessors: &mut Vec<serde_json::Value>,
    values: &[u16],
) -> usize {
    align_four(bin, 0);
    let offset = bin.len();
    for value in values {
        bin.extend_from_slice(&value.to_le_bytes());
    }
    let view_index = buffer_views.len();
    buffer_views.push(serde_json::json!({
        "buffer": 0,
        "byteOffset": offset,
        "byteLength": values.len() * 2,
        "target": 34963
    }));
    let accessor_index = accessors.len();
    accessors.push(serde_json::json!({
        "bufferView": view_index,
        "componentType": 5123,
        "count": values.len(),
        "type": "SCALAR"
    }));
    accessor_index
}

fn flattened_f32(value: &serde_json::Value) -> Vec<f32> {
    fn visit(value: &serde_json::Value, output: &mut Vec<f32>) {
        if let Some(array) = value.as_array() {
            for child in array {
                visit(child, output);
            }
        } else {
            output.push(value.as_f64().unwrap() as f32);
        }
    }
    let mut output = Vec::new();
    visit(value, &mut output);
    output
}

fn mutate_glb(source: &[u8], change: impl FnOnce(&mut serde_json::Value, &mut Vec<u8>)) -> Vec<u8> {
    assert_eq!(&source[0..4], b"glTF");
    let json_length = u32::from_le_bytes(source[12..16].try_into().unwrap()) as usize;
    assert_eq!(&source[16..20], b"JSON");
    let json_end = 20 + json_length;
    let bin_header = json_end;
    let bin_length =
        u32::from_le_bytes(source[bin_header..bin_header + 4].try_into().unwrap()) as usize;
    assert_eq!(&source[bin_header + 4..bin_header + 8], b"BIN\0");
    let mut document: serde_json::Value = serde_json::from_slice(&source[20..json_end]).unwrap();
    let mut bin = source[bin_header + 8..bin_header + 8 + bin_length].to_vec();
    change(&mut document, &mut bin);
    document["buffers"][0]["byteLength"] = bin.len().into();
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

fn assert_point_near(actual: [f64; 3], expected: [f64; 3]) {
    for component in 0..3 {
        assert!(
            (actual[component] - expected[component]).abs() < 1.0e-9,
            "component {component}: expected {}, got {}",
            expected[component],
            actual[component]
        );
    }
}

enum ComponentMutation {
    Maximum,
}

fn overwrite_first_accessor_component(
    document: &serde_json::Value,
    bin: &mut [u8],
    accessor_index: usize,
    mutation: ComponentMutation,
) {
    let accessor = &document["accessors"][accessor_index];
    let view = &document["bufferViews"][accessor["bufferView"].as_u64().unwrap() as usize];
    let offset = view["byteOffset"].as_u64().unwrap_or(0) as usize
        + accessor["byteOffset"].as_u64().unwrap_or(0) as usize;
    match (accessor["componentType"].as_u64().unwrap(), mutation) {
        (5121, ComponentMutation::Maximum) => bin[offset] = u8::MAX,
        (5123, ComponentMutation::Maximum) => {
            bin[offset..offset + 2].copy_from_slice(&u16::MAX.to_le_bytes());
        }
        (5126, ComponentMutation::Maximum) => {
            bin[offset..offset + 4].copy_from_slice(&f32::MAX.to_le_bytes());
        }
        (component, _) => panic!("unsupported fixture component type {component}"),
    }
}

fn overwrite_first_accessor_element_with_zero(
    document: &serde_json::Value,
    bin: &mut [u8],
    accessor_index: usize,
) {
    let accessor = &document["accessors"][accessor_index];
    let view = &document["bufferViews"][accessor["bufferView"].as_u64().unwrap() as usize];
    let offset = view["byteOffset"].as_u64().unwrap_or(0) as usize
        + accessor["byteOffset"].as_u64().unwrap_or(0) as usize;
    let component_bytes = match accessor["componentType"].as_u64().unwrap() {
        5121 => 1,
        5123 => 2,
        5126 => 4,
        component => panic!("unsupported fixture component type {component}"),
    };
    bin[offset..offset + component_bytes * 4].fill(0);
}
