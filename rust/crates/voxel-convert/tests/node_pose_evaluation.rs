use voxel_convert::{
    evaluate_clip_node_poses, sample_animation_clip_range, AnimationAnchorPolicy,
    AnimationChannelValues, AnimationEndPolicy, AnimationInterpolation, AnimationProperty,
    AnimationSampleRangeRequest, ImportedAnimatedModel, ImportedAnimationChannel,
    ImportedAnimationClip, ImportedAnimationNode, ImportedMaterial, ImportedModelMesh,
    ImportedModelNode, ImportedModelPrimitive, ImportedModelScene, ImportedNodeTransform,
    ImportedPrimitiveDeformation, NodePoseRigidScalePolicy,
};

const CLIP_DURATION: u64 = 1_000_000;
const MIDPOINT: u64 = CLIP_DURATION / 2;

#[test]
fn node_poses_use_canonical_step_linear_and_tangent_aware_cubic_sampling() {
    let model = pose_model();

    let step = evaluate_clip_node_poses(&model, "step", MIDPOINT).unwrap();
    assert_eq!(step.nodes.len(), 2);
    assert_eq!(step.nodes[0].source_node_index, 0);
    assert_near(step.nodes[0].world_transform[12], 0.0);

    let linear = evaluate_clip_node_poses(&model, "linear", MIDPOINT).unwrap();
    assert_near(linear.nodes[0].world_transform[12], 1.0);

    let cubic = evaluate_clip_node_poses(&model, "cubic", MIDPOINT).unwrap();
    let root = &cubic.nodes[0];
    assert_near(root.world_transform[12], 0.5);
    assert_near(root.world_transform[0], 15.0 / 17.0);
    assert_near(root.world_transform[1], 8.0 / 17.0);
    assert_near(root.world_transform[4], -8.0 / 17.0);
    assert_near(root.world_transform[5], 15.0 / 17.0);
}

#[test]
fn node_poses_preserve_animated_and_base_scale_through_hierarchy_composition() {
    let model = pose_model();
    let poses = evaluate_clip_node_poses(&model, "scale", MIDPOINT).unwrap();
    let root = &poses.nodes[0];
    let child = &poses.nodes[1];

    assert_near(root.world_transform[0], 3.0);
    assert_near(root.world_transform[5], 4.0);
    assert_near(root.world_transform[10], 5.0);
    assert_near(child.world_transform[0], 1.5);
    assert_near(child.world_transform[5], 2.0);
    assert_near(child.world_transform[10], 2.5);
    assert_near(child.world_transform[12], 3.0);

    let error = root
        .admit_rigid_world_transform(NodePoseRigidScalePolicy::AllowUniformScale)
        .unwrap_err();
    assert_eq!(error.diagnostics()[0].code, "conversion.nonRigidNodePose");

    let mut uniform_model = model.clone();
    let AnimationChannelValues::Scales(scales) = &mut uniform_model.clips[3].channels[0].values
    else {
        panic!("scale fixture clip must retain scale values");
    };
    *scales = vec![[2.0; 3], [4.0; 3]];
    let uniform = evaluate_clip_node_poses(&uniform_model, "scale", MIDPOINT).unwrap();
    let admitted = uniform.nodes[0]
        .admit_rigid_world_transform(NodePoseRigidScalePolicy::AllowUniformScale)
        .unwrap();
    assert_near(admitted.uniform_scale, 3.0);
    assert_eq!(
        admitted.affine_world_transform,
        uniform.nodes[0].world_transform
    );
    let error = uniform.nodes[0]
        .admit_rigid_world_transform(NodePoseRigidScalePolicy::RequireUnitScale)
        .unwrap_err();
    assert_eq!(error.diagnostics()[0].code, "conversion.nonRigidNodePose");

    let unit = evaluate_clip_node_poses(&model, "linear", MIDPOINT).unwrap();
    assert_near(
        unit.nodes[0]
            .admit_rigid_world_transform(NodePoseRigidScalePolicy::RequireUnitScale)
            .unwrap()
            .uniform_scale,
        1.0,
    );
}

#[test]
fn transforms_only_evaluation_accepts_morph_only_clips_and_matches_mesh_sampling() {
    let model = pose_model();
    let morph_pose = evaluate_clip_node_poses(&model, "morph-only", MIDPOINT).unwrap();
    assert_eq!(morph_pose.nodes[0].world_transform, identity_matrix());

    let morph_mesh = sample_one(&model, "morph-only", MIDPOINT);
    assert_point_near(morph_mesh.positions[0], [0.0, 0.0, 1.0]);

    let cubic_pose = evaluate_clip_node_poses(&model, "cubic", MIDPOINT).unwrap();
    let cubic_mesh = sample_one(&model, "cubic", MIDPOINT);
    let root_world = cubic_pose.node(0).unwrap().world_transform;
    for (source, sampled) in model.scene.meshes[0].primitives[0]
        .positions
        .iter()
        .zip(&cubic_mesh.positions)
    {
        assert_point_near(*sampled, transform_point(root_world, *source));
    }

    let mut transforms_only = model.clone();
    transforms_only.primitive_deformations.clear();
    assert!(evaluate_clip_node_poses(&transforms_only, "cubic", MIDPOINT).is_ok());
    let error = sample_animation_clip_range(
        &transforms_only,
        &AnimationSampleRangeRequest {
            expected_source_sha256: transforms_only.source_sha256.clone(),
            clip_name: "cubic".to_owned(),
            sample_rate_hz: 1,
            start_microseconds: MIDPOINT,
            end_microseconds: MIDPOINT,
            end_policy: AnimationEndPolicy::IncludeClipEnd,
            anchor_policy: AnimationAnchorPolicy::PreserveSourceSpace,
        },
    )
    .unwrap_err();
    assert_eq!(error.diagnostics()[0].code, "conversion.invalidDeformation");
}

#[test]
fn node_pose_evaluation_rejects_out_of_range_cycles_and_non_finite_values() {
    let model = pose_model();
    let error = evaluate_clip_node_poses(&model, "linear", CLIP_DURATION + 1).unwrap_err();
    assert_eq!(error.diagnostics()[0].code, "conversion.invalidSampleTime");
    assert_eq!(error.diagnostics()[0].path, "request.timestampMicroseconds");

    let mut cyclic = model.clone();
    cyclic.scene.nodes[0].parent_node_index = Some(1);
    cyclic.scene.nodes[1].child_node_indices = vec![0];
    let error = evaluate_clip_node_poses(&cyclic, "linear", MIDPOINT).unwrap_err();
    assert_eq!(
        error.diagnostics()[0].code,
        "conversion.invalidSceneHierarchy"
    );
    assert!(error.diagnostics()[0].message.contains("cycle"));

    let mut non_finite = model;
    let AnimationChannelValues::Translations(values) = &mut non_finite.clips[1].channels[0].values
    else {
        panic!("linear fixture clip must retain translation values");
    };
    values[1][0] = f64::NAN;
    let error = evaluate_clip_node_poses(&non_finite, "linear", MIDPOINT).unwrap_err();
    assert_eq!(
        error.diagnostics()[0].code,
        "conversion.nonFiniteDeformation"
    );
}

fn pose_model() -> ImportedAnimatedModel {
    ImportedAnimatedModel {
        source_sha256: "sha256:node-pose-fixture".to_owned(),
        scene: ImportedModelScene {
            source_scene_index: 0,
            source_scene_name: Some("node-pose-fixture".to_owned()),
            nodes: vec![
                ImportedModelNode {
                    source_node_index: 0,
                    source_node_name: Some("root".to_owned()),
                    parent_node_index: None,
                    child_node_indices: vec![1],
                    source_mesh_index: Some(0),
                    local_transform: identity_matrix(),
                    model_transform: identity_matrix(),
                },
                ImportedModelNode {
                    source_node_index: 1,
                    source_node_name: Some("child".to_owned()),
                    parent_node_index: Some(0),
                    child_node_indices: Vec::new(),
                    source_mesh_index: None,
                    local_transform: child_bind_transform(),
                    model_transform: child_bind_transform(),
                },
            ],
            meshes: vec![ImportedModelMesh {
                source_mesh_index: 0,
                source_mesh_name: Some("triangle".to_owned()),
                primitives: vec![ImportedModelPrimitive {
                    source_primitive_index: 0,
                    source_material_slot: 0,
                    positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
                    texture_coordinates: Vec::new(),
                    indices: vec![0, 1, 2],
                }],
            }],
            materials: vec![ImportedMaterial {
                source_material_slot: 0,
                source_material_name: Some("plain".to_owned()),
            }],
        },
        nodes: vec![
            ImportedAnimationNode {
                source_node_index: 0,
                source_skin_index: None,
                base_transform: ImportedNodeTransform::Decomposed {
                    translation: [0.0; 3],
                    rotation: [0.0, 0.0, 0.0, 1.0],
                    scale: [1.0; 3],
                },
                base_morph_weights: vec![0.0],
            },
            ImportedAnimationNode {
                source_node_index: 1,
                source_skin_index: None,
                base_transform: ImportedNodeTransform::Decomposed {
                    translation: [1.0, 0.0, 0.0],
                    rotation: [0.0, 0.0, 0.0, 1.0],
                    scale: [0.5; 3],
                },
                base_morph_weights: Vec::new(),
            },
        ],
        skins: Vec::new(),
        primitive_deformations: vec![ImportedPrimitiveDeformation {
            source_mesh_index: 0,
            source_primitive_index: 0,
            vertex_joints: None,
            vertex_weights: None,
            morph_position_deltas: vec![vec![[0.0, 0.0, 2.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]]],
        }],
        clips: vec![
            clip(
                0,
                "step",
                vec![translation_channel(
                    0,
                    AnimationInterpolation::Step,
                    vec![[0.0, 0.0, 0.0], [2.0, 0.0, 0.0]],
                )],
            ),
            clip(
                1,
                "linear",
                vec![translation_channel(
                    0,
                    AnimationInterpolation::Linear,
                    vec![[0.0, 0.0, 0.0], [2.0, 0.0, 0.0]],
                )],
            ),
            clip(
                2,
                "cubic",
                vec![
                    translation_channel(
                        0,
                        AnimationInterpolation::CubicSpline,
                        vec![
                            [0.0, 0.0, 0.0],
                            [0.0, 0.0, 0.0],
                            [4.0, 0.0, 0.0],
                            [0.0, 0.0, 0.0],
                            [0.0, 0.0, 0.0],
                            [0.0, 0.0, 0.0],
                        ],
                    ),
                    ImportedAnimationChannel {
                        source_channel_index: 1,
                        target_node_index: 0,
                        property: AnimationProperty::Rotation,
                        interpolation: AnimationInterpolation::CubicSpline,
                        timestamps_microseconds: vec![0, CLIP_DURATION],
                        values: AnimationChannelValues::Rotations(vec![
                            [0.0, 0.0, 0.0, 0.0],
                            [0.0, 0.0, 0.0, 1.0],
                            [0.0, 0.0, 2.0, 0.0],
                            [0.0, 0.0, 0.0, 0.0],
                            [0.0, 0.0, 0.0, 1.0],
                            [0.0, 0.0, 0.0, 0.0],
                        ]),
                    },
                ],
            ),
            clip(
                3,
                "scale",
                vec![ImportedAnimationChannel {
                    source_channel_index: 0,
                    target_node_index: 0,
                    property: AnimationProperty::Scale,
                    interpolation: AnimationInterpolation::Linear,
                    timestamps_microseconds: vec![0, CLIP_DURATION],
                    values: AnimationChannelValues::Scales(vec![[2.0, 3.0, 4.0], [4.0, 5.0, 6.0]]),
                }],
            ),
            clip(
                4,
                "morph-only",
                vec![ImportedAnimationChannel {
                    source_channel_index: 0,
                    target_node_index: 0,
                    property: AnimationProperty::MorphWeights,
                    interpolation: AnimationInterpolation::Linear,
                    timestamps_microseconds: vec![0, CLIP_DURATION],
                    values: AnimationChannelValues::MorphWeights {
                        target_count: 1,
                        values: vec![0.0, 1.0],
                    },
                }],
            ),
        ],
    }
}

fn clip(
    source_animation_index: u32,
    name: &str,
    channels: Vec<ImportedAnimationChannel>,
) -> ImportedAnimationClip {
    ImportedAnimationClip {
        source_animation_index,
        name: name.to_owned(),
        duration_microseconds: CLIP_DURATION,
        channels,
    }
}

fn translation_channel(
    source_channel_index: u32,
    interpolation: AnimationInterpolation,
    values: Vec<[f64; 3]>,
) -> ImportedAnimationChannel {
    ImportedAnimationChannel {
        source_channel_index,
        target_node_index: 0,
        property: AnimationProperty::Translation,
        interpolation,
        timestamps_microseconds: vec![0, CLIP_DURATION],
        values: AnimationChannelValues::Translations(values),
    }
}

fn sample_one(
    model: &ImportedAnimatedModel,
    clip_name: &str,
    timestamp_microseconds: u64,
) -> voxel_convert::ImportedStaticMesh {
    sample_animation_clip_range(
        model,
        &AnimationSampleRangeRequest {
            expected_source_sha256: model.source_sha256.clone(),
            clip_name: clip_name.to_owned(),
            sample_rate_hz: 1,
            start_microseconds: timestamp_microseconds,
            end_microseconds: timestamp_microseconds,
            end_policy: AnimationEndPolicy::IncludeClipEnd,
            anchor_policy: AnimationAnchorPolicy::PreserveSourceSpace,
        },
    )
    .unwrap()
    .snapshots
    .into_iter()
    .next()
    .unwrap()
    .mesh
}

fn identity_matrix() -> [f64; 16] {
    [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ]
}

fn child_bind_transform() -> [f64; 16] {
    [
        0.5, 0.0, 0.0, 0.0, 0.0, 0.5, 0.0, 0.0, 0.0, 0.0, 0.5, 0.0, 1.0, 0.0, 0.0, 1.0,
    ]
}

fn transform_point(matrix: [f64; 16], point: [f64; 3]) -> [f64; 3] {
    [
        matrix[0] * point[0] + matrix[4] * point[1] + matrix[8] * point[2] + matrix[12],
        matrix[1] * point[0] + matrix[5] * point[1] + matrix[9] * point[2] + matrix[13],
        matrix[2] * point[0] + matrix[6] * point[1] + matrix[10] * point[2] + matrix[14],
    ]
}

fn assert_point_near(actual: [f64; 3], expected: [f64; 3]) {
    for component in 0..3 {
        assert_near(actual[component], expected[component]);
    }
}

fn assert_near(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1.0e-9,
        "expected {expected}, got {actual}"
    );
}
