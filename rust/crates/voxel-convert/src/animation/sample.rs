use std::collections::{BTreeMap, BTreeSet};
use std::mem::size_of;

use super::{
    AnimationAnchorPolicy, AnimationBindPoseReceipt, AnimationBindPoseRequest, AnimationEndPolicy,
    AnimationMeshSnapshot, AnimationProperty, AnimationSampleRangeReceipt,
    AnimationSampleRangeRequest, AnimationSampleReceipt, AnimationSampleRequest,
    ImportedAnimatedModel, ImportedAnimationClip, ImportedNodeTransform,
    ANIMATION_TIMESTAMP_TICKS_PER_SECOND, MAX_ANIMATION_DEFORMATION_WORK,
    MAX_ANIMATION_MATERIALIZED_SNAPSHOT_BYTES, MAX_ANIMATION_SAMPLE_FRAMES,
    MAX_ANIMATION_SAMPLE_RATE_HZ,
};
use crate::import::{
    flatten_model_scene, identity_matrix, multiply_matrices, transform_point,
    validate_affine_matrix,
};
use crate::{ConversionError, ImportedModelMesh, ImportedModelNode, ImportedModelPrimitive};

mod interpolation;
mod matrix;

use interpolation::{sample_morph_weights, sample_rotation, sample_scale, sample_translation};
use matrix::{compose_trs, invert_affine};

struct EvaluatedPose {
    model_transforms: BTreeMap<u32, [f64; 16]>,
    morph_weights: BTreeMap<u32, Vec<f64>>,
}

pub(super) fn sample_animation_clip(
    model: &ImportedAnimatedModel,
    request: &AnimationSampleRequest,
) -> Result<AnimationSampleReceipt, ConversionError> {
    validate_source_identity(model, &request.expected_source_sha256)?;
    validate_sample_rate(request.sample_rate_hz)?;
    validate_anchor(model, request.anchor_policy)?;
    let clip = find_clip(model, &request.clip_name)?;
    let timestamps = build_sample_schedule(
        clip.duration_microseconds,
        request.sample_rate_hz,
        request.end_policy,
    )?;
    let (deformation_work, estimated_materialized_snapshot_bytes, snapshots) =
        sample_timestamps(model, clip, timestamps, request.anchor_policy)?;

    Ok(AnimationSampleReceipt {
        source_sha256: model.source_sha256.clone(),
        source_animation_index: clip.source_animation_index,
        clip_name: clip.name.clone(),
        duration_microseconds: clip.duration_microseconds,
        sample_rate_hz: request.sample_rate_hz,
        end_policy: request.end_policy,
        anchor_policy: request.anchor_policy,
        deformation_work,
        estimated_materialized_snapshot_bytes,
        snapshots,
    })
}

pub(super) fn sample_animation_clip_range(
    model: &ImportedAnimatedModel,
    request: &AnimationSampleRangeRequest,
) -> Result<AnimationSampleRangeReceipt, ConversionError> {
    validate_source_identity(model, &request.expected_source_sha256)?;
    validate_sample_rate(request.sample_rate_hz)?;
    validate_anchor(model, request.anchor_policy)?;
    let clip = find_clip(model, &request.clip_name)?;
    if request.start_microseconds > request.end_microseconds
        || request.end_microseconds > clip.duration_microseconds
    {
        return Err(ConversionError::one(
            "conversion.invalidSampleRange",
            "request.sampleRange",
            format!(
                "sample range {}..={} must be ordered inside clip duration {}",
                request.start_microseconds, request.end_microseconds, clip.duration_microseconds
            ),
        ));
    }
    let timestamps = build_sample_schedule_range(
        request.start_microseconds,
        request.end_microseconds,
        request.sample_rate_hz,
        request.end_policy,
    )?;
    let (deformation_work, estimated_materialized_snapshot_bytes, snapshots) =
        sample_timestamps(model, clip, timestamps, request.anchor_policy)?;
    Ok(AnimationSampleRangeReceipt {
        source_sha256: model.source_sha256.clone(),
        source_animation_index: clip.source_animation_index,
        clip_name: clip.name.clone(),
        clip_duration_microseconds: clip.duration_microseconds,
        start_microseconds: request.start_microseconds,
        end_microseconds: request.end_microseconds,
        sample_rate_hz: request.sample_rate_hz,
        end_policy: request.end_policy,
        anchor_policy: request.anchor_policy,
        deformation_work,
        estimated_materialized_snapshot_bytes,
        snapshots,
    })
}

fn validate_sample_rate(sample_rate_hz: u32) -> Result<(), ConversionError> {
    if sample_rate_hz == 0 || sample_rate_hz > MAX_ANIMATION_SAMPLE_RATE_HZ {
        return Err(ConversionError::one(
            "conversion.resourceLimit",
            "request.sampleRateHz",
            format!("sample rate must be in 1..={MAX_ANIMATION_SAMPLE_RATE_HZ} Hz"),
        ));
    }
    Ok(())
}

fn find_clip<'a>(
    model: &'a ImportedAnimatedModel,
    clip_name: &str,
) -> Result<&'a ImportedAnimationClip, ConversionError> {
    model
        .clips
        .iter()
        .find(|clip| clip.name == clip_name)
        .ok_or_else(|| {
            ConversionError::one(
                "conversion.clipNotFound",
                "request.clipName",
                format!("animation clip {clip_name:?} is not present"),
            )
        })
}

fn sample_timestamps(
    model: &ImportedAnimatedModel,
    clip: &ImportedAnimationClip,
    timestamps: Vec<u64>,
    anchor_policy: AnimationAnchorPolicy,
) -> Result<(u64, u64, Vec<AnimationMeshSnapshot>), ConversionError> {
    let work_per_snapshot = deformation_work_per_snapshot(model)?;
    let deformation_work = work_per_snapshot
        .checked_mul(timestamps.len() as u64)
        .ok_or_else(|| deformation_limit("animation deformation work overflowed"))?;
    if deformation_work > MAX_ANIMATION_DEFORMATION_WORK {
        return Err(deformation_limit(&format!(
            "animation deformation work {deformation_work} exceeds {MAX_ANIMATION_DEFORMATION_WORK}"
        )));
    }
    let estimated_materialized_snapshot_bytes =
        estimated_materialized_snapshot_bytes(model, timestamps.len())?;
    let mut snapshots = Vec::with_capacity(timestamps.len());
    for timestamp_microseconds in timestamps {
        let pose = evaluate_pose(model, Some(clip), timestamp_microseconds)?;
        let mesh = deform_pose(model, &pose, anchor_policy)?;
        snapshots.push(AnimationMeshSnapshot {
            timestamp_microseconds,
            mesh,
        });
    }
    Ok((
        deformation_work,
        estimated_materialized_snapshot_bytes,
        snapshots,
    ))
}

pub(super) fn sample_animation_bind_pose(
    model: &ImportedAnimatedModel,
    request: &AnimationBindPoseRequest,
) -> Result<AnimationBindPoseReceipt, ConversionError> {
    validate_source_identity(model, &request.expected_source_sha256)?;
    validate_anchor(model, request.anchor_policy)?;
    let deformation_work = deformation_work_per_snapshot(model)?;
    if deformation_work > MAX_ANIMATION_DEFORMATION_WORK {
        return Err(deformation_limit(&format!(
            "bind-pose deformation work {deformation_work} exceeds {MAX_ANIMATION_DEFORMATION_WORK}"
        )));
    }
    let estimated_materialized_snapshot_bytes = estimated_materialized_snapshot_bytes(model, 1)?;
    let pose = evaluate_pose(model, None, 0)?;
    let mesh = deform_pose(model, &pose, request.anchor_policy)?;
    Ok(AnimationBindPoseReceipt {
        source_sha256: model.source_sha256.clone(),
        anchor_policy: request.anchor_policy,
        deformation_work,
        estimated_materialized_snapshot_bytes,
        mesh,
    })
}

fn estimated_materialized_snapshot_bytes(
    model: &ImportedAnimatedModel,
    snapshot_count: usize,
) -> Result<u64, ConversionError> {
    let mut position_count = 0u64;
    let mut triangle_count = 0u64;
    let mut primitive_group_count = 0u64;
    let mut texture_set_indices = BTreeSet::new();
    let mut used_material_slots = BTreeSet::new();

    for mesh in &model.scene.meshes {
        for primitive in &mesh.primitives {
            texture_set_indices.extend(
                primitive
                    .texture_coordinates
                    .iter()
                    .map(|coordinates| coordinates.source_set_index),
            );
        }
    }
    for node in &model.scene.nodes {
        let Some(mesh_index) = node.source_mesh_index else {
            continue;
        };
        let mesh = model
            .scene
            .meshes
            .iter()
            .find(|mesh| mesh.source_mesh_index == mesh_index)
            .ok_or_else(|| {
                ConversionError::one(
                    "conversion.invalidGeometry",
                    format!("source.nodes[{}].mesh", node.source_node_index),
                    format!("referenced mesh {mesh_index} was not imported"),
                )
            })?;
        for primitive in &mesh.primitives {
            position_count = checked_snapshot_add(
                position_count,
                snapshot_count_from_usize(primitive.positions.len())?,
                "flattened position count",
            )?;
            triangle_count = checked_snapshot_add(
                triangle_count,
                snapshot_count_from_usize(primitive.indices.len() / 3)?,
                "flattened triangle count",
            )?;
            primitive_group_count =
                checked_snapshot_add(primitive_group_count, 1, "flattened primitive-group count")?;
            used_material_slots.insert(primitive.source_material_slot);
        }
    }

    let texture_set_count = snapshot_count_from_usize(texture_set_indices.len())?;
    let texture_coordinate_count = position_count
        .checked_mul(texture_set_count)
        .ok_or_else(|| snapshot_storage_limit("texture-coordinate count overflowed"))?;
    let mut material_count = 0u64;
    let mut material_name_bytes = 0u64;
    for material in model
        .scene
        .materials
        .iter()
        .filter(|material| used_material_slots.contains(&material.source_material_slot))
    {
        material_count = checked_snapshot_add(material_count, 1, "material count")?;
        if let Some(name) = &material.source_material_name {
            material_name_bytes = checked_snapshot_add(
                material_name_bytes,
                snapshot_count_from_usize(name.len())?,
                "material-name bytes",
            )?;
        }
    }

    let mut bytes_per_snapshot = 0u64;
    add_snapshot_allocation::<AnimationMeshSnapshot>(
        &mut bytes_per_snapshot,
        1,
        "snapshot record",
    )?;
    add_snapshot_allocation::<[f64; 3]>(&mut bytes_per_snapshot, position_count, "positions")?;
    add_snapshot_allocation::<crate::ImportedTriangle>(
        &mut bytes_per_snapshot,
        triangle_count,
        "triangles",
    )?;
    add_snapshot_allocation::<crate::ImportedPrimitiveGroup>(
        &mut bytes_per_snapshot,
        primitive_group_count,
        "primitive groups",
    )?;
    add_snapshot_allocation::<crate::ImportedStaticTextureCoordinates>(
        &mut bytes_per_snapshot,
        texture_set_count,
        "texture-coordinate set records",
    )?;
    add_snapshot_allocation::<Option<[f64; 2]>>(
        &mut bytes_per_snapshot,
        texture_coordinate_count,
        "texture coordinates",
    )?;
    add_snapshot_allocation::<crate::ImportedMaterial>(
        &mut bytes_per_snapshot,
        material_count,
        "materials",
    )?;
    bytes_per_snapshot = checked_snapshot_add(
        bytes_per_snapshot,
        material_name_bytes,
        "material-name storage",
    )?;

    let total = bytes_per_snapshot
        .checked_mul(snapshot_count_from_usize(snapshot_count)?)
        .ok_or_else(|| snapshot_storage_limit("aggregate snapshot storage overflowed"))?;
    if total > MAX_ANIMATION_MATERIALIZED_SNAPSHOT_BYTES {
        return Err(snapshot_storage_limit(&format!(
            "materialized animation snapshots require {total} estimated retained bytes; limit is {MAX_ANIMATION_MATERIALIZED_SNAPSHOT_BYTES}"
        )));
    }
    Ok(total)
}

fn add_snapshot_allocation<T>(
    total: &mut u64,
    count: u64,
    label: &str,
) -> Result<(), ConversionError> {
    let element_size = u64::try_from(size_of::<T>())
        .map_err(|_| snapshot_storage_limit("platform element size exceeds u64"))?;
    let bytes = count
        .checked_mul(element_size)
        .ok_or_else(|| snapshot_storage_limit(&format!("{label} storage overflowed")))?;
    *total = checked_snapshot_add(*total, bytes, label)?;
    Ok(())
}

fn checked_snapshot_add(left: u64, right: u64, label: &str) -> Result<u64, ConversionError> {
    left.checked_add(right)
        .ok_or_else(|| snapshot_storage_limit(&format!("{label} overflowed")))
}

fn snapshot_count_from_usize(value: usize) -> Result<u64, ConversionError> {
    u64::try_from(value).map_err(|_| snapshot_storage_limit("snapshot count exceeds u64"))
}

fn snapshot_storage_limit(message: &str) -> ConversionError {
    ConversionError::one(
        "conversion.resourceLimit",
        "request.snapshotStorage",
        message,
    )
}

fn validate_source_identity(
    model: &ImportedAnimatedModel,
    expected: &str,
) -> Result<(), ConversionError> {
    if expected != model.source_sha256 {
        return Err(ConversionError::one(
            "conversion.sourceHashMismatch",
            "request.expectedSourceSha256",
            format!("expected {expected}, imported {}", model.source_sha256),
        ));
    }
    Ok(())
}

fn validate_anchor(
    model: &ImportedAnimatedModel,
    policy: AnimationAnchorPolicy,
) -> Result<(), ConversionError> {
    let AnimationAnchorPolicy::LockNodeToBindPose { source_node_index } = policy else {
        return Ok(());
    };
    let node = model
        .scene
        .nodes
        .iter()
        .find(|node| node.source_node_index == source_node_index)
        .ok_or_else(|| {
            ConversionError::one(
                "conversion.invalidAnchor",
                "request.anchorPolicy.sourceNodeIndex",
                format!("anchor node {source_node_index} is not reachable in the selected scene"),
            )
        })?;
    invert_affine(node.model_transform).ok_or_else(|| {
        ConversionError::one(
            "conversion.invalidAnchor",
            "request.anchorPolicy.sourceNodeIndex",
            format!("anchor node {source_node_index} has a non-invertible bind transform"),
        )
    })?;
    Ok(())
}

fn build_sample_schedule(
    duration_microseconds: u64,
    sample_rate_hz: u32,
    end_policy: AnimationEndPolicy,
) -> Result<Vec<u64>, ConversionError> {
    build_sample_schedule_range(0, duration_microseconds, sample_rate_hz, end_policy)
}

fn build_sample_schedule_range(
    start_microseconds: u64,
    end_microseconds: u64,
    sample_rate_hz: u32,
    end_policy: AnimationEndPolicy,
) -> Result<Vec<u64>, ConversionError> {
    let duration_microseconds = end_microseconds - start_microseconds;
    if duration_microseconds == 0 {
        return Ok(vec![start_microseconds]);
    }
    let rounded_threshold = duration_microseconds as u128 * sample_rate_hz as u128;
    let rounding_bias = u128::from(sample_rate_hz / 2);
    let strict_before_count = if rounded_threshold <= rounding_bias {
        0
    } else {
        (rounded_threshold - 1 - rounding_bias) / ANIMATION_TIMESTAMP_TICKS_PER_SECOND as u128 + 1
    };
    let include_end = end_policy == AnimationEndPolicy::IncludeClipEnd;
    let estimated_count = strict_before_count + u128::from(include_end);
    if estimated_count > MAX_ANIMATION_SAMPLE_FRAMES as u128 {
        return Err(ConversionError::one(
            "conversion.resourceLimit",
            "request.sampleSchedule",
            format!("sample schedule exceeds {MAX_ANIMATION_SAMPLE_FRAMES} frames"),
        ));
    }

    let mut timestamps = Vec::with_capacity(estimated_count as usize);
    let mut sample_index = 0u128;
    loop {
        let numerator = sample_index
            .checked_mul(ANIMATION_TIMESTAMP_TICKS_PER_SECOND as u128)
            .ok_or_else(|| deformation_limit("sample timestamp numerator overflowed"))?;
        let offset = ((numerator + u128::from(sample_rate_hz / 2)) / sample_rate_hz as u128) as u64;
        if offset >= duration_microseconds {
            break;
        }
        let timestamp = start_microseconds
            .checked_add(offset)
            .ok_or_else(|| deformation_limit("sample timestamp overflowed"))?;
        if timestamps.last().copied() != Some(timestamp) {
            timestamps.push(timestamp);
        }
        sample_index += 1;
    }
    if include_end && timestamps.last().copied() != Some(end_microseconds) {
        timestamps.push(end_microseconds);
    }
    if timestamps.len() > MAX_ANIMATION_SAMPLE_FRAMES {
        return Err(ConversionError::one(
            "conversion.resourceLimit",
            "request.sampleSchedule",
            format!("sample schedule exceeds {MAX_ANIMATION_SAMPLE_FRAMES} frames"),
        ));
    }
    Ok(timestamps)
}

fn deformation_work_per_snapshot(model: &ImportedAnimatedModel) -> Result<u64, ConversionError> {
    let mut work = 0u64;
    for node in &model.scene.nodes {
        let Some(mesh_index) = node.source_mesh_index else {
            continue;
        };
        let mesh = model
            .scene
            .meshes
            .iter()
            .find(|mesh| mesh.source_mesh_index == mesh_index)
            .expect("selected scene mesh identities are internally consistent");
        let animation_node = animation_node(model, node.source_node_index)?;
        for primitive in &mesh.primitives {
            let deformation = primitive_deformation(
                model,
                mesh.source_mesh_index,
                primitive.source_primitive_index,
            )?;
            let operations_per_vertex = 1u64
                + deformation.morph_position_deltas.len() as u64
                + if animation_node.source_skin_index.is_some() {
                    4
                } else {
                    0
                };
            let primitive_work = (primitive.positions.len() as u64)
                .checked_mul(operations_per_vertex)
                .ok_or_else(|| deformation_limit("primitive deformation work overflowed"))?;
            work = work
                .checked_add(primitive_work)
                .ok_or_else(|| deformation_limit("snapshot deformation work overflowed"))?;
        }
    }
    Ok(work)
}

fn evaluate_pose(
    model: &ImportedAnimatedModel,
    clip: Option<&ImportedAnimationClip>,
    timestamp_microseconds: u64,
) -> Result<EvaluatedPose, ConversionError> {
    let mut transforms = model
        .nodes
        .iter()
        .map(|node| (node.source_node_index, node.base_transform))
        .collect::<BTreeMap<_, _>>();
    let mut morph_weights = model
        .nodes
        .iter()
        .map(|node| (node.source_node_index, node.base_morph_weights.clone()))
        .collect::<BTreeMap<_, _>>();

    if let Some(clip) = clip {
        if timestamp_microseconds > clip.duration_microseconds {
            return Err(ConversionError::one(
                "conversion.invalidAnimation",
                "request.sampleSchedule",
                "sample timestamp exceeds the selected clip duration",
            ));
        }
        for channel in &clip.channels {
            match channel.property {
                AnimationProperty::Translation => {
                    let sampled = sample_translation(channel, timestamp_microseconds)?;
                    let transform = transforms
                        .get_mut(&channel.target_node_index)
                        .expect("clip targets were validated during import");
                    let ImportedNodeTransform::Decomposed { translation, .. } = transform else {
                        unreachable!("matrix-authored animation targets are rejected at import")
                    };
                    *translation = sampled;
                }
                AnimationProperty::Rotation => {
                    let sampled = sample_rotation(channel, timestamp_microseconds)?;
                    let transform = transforms
                        .get_mut(&channel.target_node_index)
                        .expect("clip targets were validated during import");
                    let ImportedNodeTransform::Decomposed { rotation, .. } = transform else {
                        unreachable!("matrix-authored animation targets are rejected at import")
                    };
                    *rotation = sampled;
                }
                AnimationProperty::Scale => {
                    let sampled = sample_scale(channel, timestamp_microseconds)?;
                    let transform = transforms
                        .get_mut(&channel.target_node_index)
                        .expect("clip targets were validated during import");
                    let ImportedNodeTransform::Decomposed { scale, .. } = transform else {
                        unreachable!("matrix-authored animation targets are rejected at import")
                    };
                    *scale = sampled;
                }
                AnimationProperty::MorphWeights => {
                    let sampled = sample_morph_weights(channel, timestamp_microseconds)?;
                    *morph_weights
                        .get_mut(&channel.target_node_index)
                        .expect("clip targets were validated during import") = sampled;
                }
            }
        }
    }

    let mut model_transforms = BTreeMap::new();
    for scene_node in &model.scene.nodes {
        let transform = transforms
            .get(&scene_node.source_node_index)
            .expect("animation nodes are aligned with selected scene nodes");
        let local = match *transform {
            ImportedNodeTransform::Matrix(matrix) => matrix,
            ImportedNodeTransform::Decomposed {
                translation,
                rotation,
                scale,
            } => compose_trs(translation, rotation, scale),
        };
        validate_affine_matrix(
            local,
            format!(
                "sample.nodes[{}].localTransform",
                scene_node.source_node_index
            ),
        )?;
        let model_transform = match scene_node.parent_node_index {
            Some(parent) => multiply_matrices(
                *model_transforms
                    .get(&parent)
                    .expect("scene traversal orders parents before children"),
                local,
            ),
            None => local,
        };
        validate_affine_matrix(
            model_transform,
            format!(
                "sample.nodes[{}].modelTransform",
                scene_node.source_node_index
            ),
        )?;
        model_transforms.insert(scene_node.source_node_index, model_transform);
    }
    Ok(EvaluatedPose {
        model_transforms,
        morph_weights,
    })
}

fn deform_pose(
    model: &ImportedAnimatedModel,
    pose: &EvaluatedPose,
    anchor_policy: AnimationAnchorPolicy,
) -> Result<crate::ImportedStaticMesh, ConversionError> {
    let anchor_correction = anchor_correction(model, pose, anchor_policy)?;
    flatten_model_scene(
        &model.scene,
        |node: &ImportedModelNode, mesh: &ImportedModelMesh, primitive: &ImportedModelPrimitive| {
            deform_primitive(model, pose, anchor_correction, node, mesh, primitive)
        },
    )
}

fn anchor_correction(
    model: &ImportedAnimatedModel,
    pose: &EvaluatedPose,
    policy: AnimationAnchorPolicy,
) -> Result<[f64; 16], ConversionError> {
    let AnimationAnchorPolicy::LockNodeToBindPose { source_node_index } = policy else {
        return Ok(identity_matrix());
    };
    let bind = model
        .scene
        .nodes
        .iter()
        .find(|node| node.source_node_index == source_node_index)
        .expect("anchor was validated before sampling")
        .model_transform;
    let sampled = *pose
        .model_transforms
        .get(&source_node_index)
        .expect("anchor was validated before sampling");
    let inverse_sampled = invert_affine(sampled).ok_or_else(|| {
        ConversionError::one(
            "conversion.invalidAnchor",
            "request.anchorPolicy.sourceNodeIndex",
            format!("sampled anchor node {source_node_index} has a non-invertible transform"),
        )
    })?;
    Ok(multiply_matrices(bind, inverse_sampled))
}

fn deform_primitive(
    model: &ImportedAnimatedModel,
    pose: &EvaluatedPose,
    anchor_correction: [f64; 16],
    node: &ImportedModelNode,
    mesh: &ImportedModelMesh,
    primitive: &ImportedModelPrimitive,
) -> Result<Vec<[f64; 3]>, ConversionError> {
    let animation_node = animation_node(model, node.source_node_index)?;
    let deformation = primitive_deformation(
        model,
        mesh.source_mesh_index,
        primitive.source_primitive_index,
    )?;
    let weights = pose
        .morph_weights
        .get(&node.source_node_index)
        .expect("every selected node has evaluated morph weights");
    if weights.len() != deformation.morph_position_deltas.len() {
        return Err(ConversionError::one(
            "conversion.invalidMorphTarget",
            format!("sample.nodes[{}].weights", node.source_node_index),
            "evaluated morph weight count does not match primitive target count",
        ));
    }
    let node_transform = *pose
        .model_transforms
        .get(&node.source_node_index)
        .expect("every selected node has an evaluated model transform");
    let skin = animation_node.source_skin_index.map(|skin_index| {
        model
            .skins
            .iter()
            .find(|skin| skin.source_skin_index == skin_index)
            .expect("skin bindings were validated during import")
    });

    primitive
        .positions
        .iter()
        .copied()
        .enumerate()
        .map(|(vertex_index, position)| {
            let mut morphed = position;
            for (weight, target) in weights.iter().zip(&deformation.morph_position_deltas) {
                for component in 0..3 {
                    morphed[component] += *weight * target[vertex_index][component];
                }
            }
            if morphed.iter().any(|component| !component.is_finite()) {
                return Err(non_finite_deformation(node, primitive, vertex_index));
            }

            let deformed = if let Some(skin) = skin {
                let joints = deformation
                    .vertex_joints
                    .as_ref()
                    .expect("skinned primitive joints were validated during import")[vertex_index];
                let vertex_weights = deformation
                    .vertex_weights
                    .as_ref()
                    .expect("skinned primitive weights were validated during import")[vertex_index];
                let mut position_sum = [0.0; 3];
                for influence in 0..4 {
                    let weight = vertex_weights[influence];
                    if weight == 0.0 {
                        continue;
                    }
                    let joint_ordinal = joints[influence] as usize;
                    let joint_node_index = skin.joint_node_indices[joint_ordinal];
                    let joint_transform = *pose
                        .model_transforms
                        .get(&joint_node_index)
                        .expect("skin joint reachability was validated during import");
                    let skin_transform = multiply_matrices(
                        joint_transform,
                        skin.inverse_bind_matrices[joint_ordinal],
                    );
                    let joint_position = transform_point(skin_transform, morphed)
                        .ok_or_else(|| non_finite_deformation(node, primitive, vertex_index))?;
                    for component in 0..3 {
                        position_sum[component] += weight * joint_position[component];
                    }
                }
                position_sum
            } else {
                transform_point(node_transform, morphed)
                    .ok_or_else(|| non_finite_deformation(node, primitive, vertex_index))?
            };
            transform_point(anchor_correction, deformed)
                .ok_or_else(|| non_finite_deformation(node, primitive, vertex_index))
        })
        .collect()
}

fn animation_node(
    model: &ImportedAnimatedModel,
    source_node_index: u32,
) -> Result<&super::ImportedAnimationNode, ConversionError> {
    model
        .nodes
        .iter()
        .find(|node| node.source_node_index == source_node_index)
        .ok_or_else(|| {
            ConversionError::one(
                "conversion.invalidAnimation",
                format!("source.nodes[{source_node_index}]"),
                "animation node metadata is missing",
            )
        })
}

fn primitive_deformation(
    model: &ImportedAnimatedModel,
    source_mesh_index: u32,
    source_primitive_index: u32,
) -> Result<&super::ImportedPrimitiveDeformation, ConversionError> {
    model
        .primitive_deformations
        .iter()
        .find(|deformation| {
            deformation.source_mesh_index == source_mesh_index
                && deformation.source_primitive_index == source_primitive_index
        })
        .ok_or_else(|| {
            ConversionError::one(
                "conversion.invalidDeformation",
                format!("source.meshes[{source_mesh_index}].primitives[{source_primitive_index}]"),
                "primitive deformation metadata is missing",
            )
        })
}

fn non_finite_deformation(
    node: &ImportedModelNode,
    primitive: &ImportedModelPrimitive,
    vertex_index: usize,
) -> ConversionError {
    ConversionError::one(
        "conversion.nonFiniteDeformation",
        format!(
            "sample.nodes[{}].primitives[{}].positions[{vertex_index}]",
            node.source_node_index, primitive.source_primitive_index
        ),
        "node, morph, or skin deformation produced a non-finite position",
    )
}

fn deformation_limit(message: &str) -> ConversionError {
    ConversionError::one(
        "conversion.resourceLimit",
        "request.deformationWork",
        message.to_owned(),
    )
}
