use std::collections::{BTreeMap, BTreeSet};

use super::interpolation::{
    sample_morph_weights, sample_rotation, sample_scale, sample_translation,
};
use super::matrix::compose_trs;
use crate::animation::{
    AnimationProperty, ImportedAnimatedModel, ImportedAnimationClip, ImportedNodeTransform,
};
use crate::import::{multiply_matrices, validate_affine_matrix, MAX_IMPORTED_SCENE_DEPTH};
use crate::ConversionError;

/// Maximum absolute numerical drift admitted when classifying an affine pose
/// as a rigid or uniformly scaled rigid transform.
const NODE_POSE_RIGID_TOLERANCE: f64 = 1.0e-6;

/// One source node's evaluated transforms at an explicit clip timestamp.
///
/// `world_transform` is the node's composed, column-major affine transform in
/// the selected scene's model/world space. Both transforms preserve authored
/// scale; this type never silently converts an affine pose into a rigid pose.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnimationNodePose {
    pub source_node_index: u32,
    pub local_transform: [f64; 16],
    pub world_transform: [f64; 16],
}

/// Complete transforms-only evaluation of one imported clip timestamp.
#[derive(Debug, Clone, PartialEq)]
pub struct AnimationNodePoseReceipt {
    pub source_sha256: String,
    pub source_animation_index: u32,
    pub clip_name: String,
    pub clip_duration_microseconds: u64,
    pub timestamp_microseconds: u64,
    /// Nodes remain in the imported scene's deterministic traversal order.
    pub nodes: Vec<AnimationNodePose>,
}

impl AnimationNodePoseReceipt {
    /// Find one evaluated node by its stable authored source identity.
    pub fn node(&self, source_node_index: u32) -> Option<&AnimationNodePose> {
        self.nodes
            .iter()
            .find(|node| node.source_node_index == source_node_index)
    }
}

/// Explicit scale policy for callers that need rigid part placement.
///
/// Callers that intentionally support arbitrary affine placement should use
/// `AnimationNodePose::world_transform` directly instead of invoking the
/// rigid admission hook.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodePoseRigidScalePolicy {
    /// Admit only a proper rigid transform whose three axis scales are one.
    RequireUnitScale,
    /// Also admit one positive uniform scale. Non-uniform scale, shear,
    /// singular transforms, and reflections remain typed failures.
    AllowUniformScale,
}

/// A node pose admitted for a caller-selected rigid scale policy.
///
/// The original affine transform is retained so uniform scale is never
/// discarded implicitly. `uniform_scale` tells a caller what it must preserve
/// or deliberately remove in its own rigid-part policy.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AdmittedRigidNodePose {
    pub source_node_index: u32,
    pub affine_world_transform: [f64; 16],
    pub uniform_scale: f64,
}

impl AnimationNodePose {
    /// Validate this world transform for a caller's explicit rigid scale
    /// policy. Evaluation itself always preserves the complete affine value.
    pub fn admit_rigid_world_transform(
        &self,
        policy: NodePoseRigidScalePolicy,
    ) -> Result<AdmittedRigidNodePose, ConversionError> {
        let path = format!("nodePoses.nodes[{}].worldTransform", self.source_node_index);
        validate_affine_matrix(self.world_transform, &path)
            .map_err(|_| non_rigid_pose(&path, "world transform is not finite affine data"))?;

        let columns = [
            [
                self.world_transform[0],
                self.world_transform[1],
                self.world_transform[2],
            ],
            [
                self.world_transform[4],
                self.world_transform[5],
                self.world_transform[6],
            ],
            [
                self.world_transform[8],
                self.world_transform[9],
                self.world_transform[10],
            ],
        ];
        let scales = columns.map(vector_length);
        if scales
            .iter()
            .any(|scale| !scale.is_finite() || *scale <= NODE_POSE_RIGID_TOLERANCE)
        {
            return Err(non_rigid_pose(
                &path,
                "world transform has a singular scale axis",
            ));
        }
        let axes = std::array::from_fn::<_, 3, _>(|index| {
            columns[index].map(|value| value / scales[index])
        });
        if dot(axes[0], axes[1]).abs() > NODE_POSE_RIGID_TOLERANCE
            || dot(axes[0], axes[2]).abs() > NODE_POSE_RIGID_TOLERANCE
            || dot(axes[1], axes[2]).abs() > NODE_POSE_RIGID_TOLERANCE
        {
            return Err(non_rigid_pose(&path, "world transform contains shear"));
        }
        let determinant = dot(cross(axes[0], axes[1]), axes[2]);
        if (determinant - 1.0).abs() > NODE_POSE_RIGID_TOLERANCE {
            return Err(non_rigid_pose(
                &path,
                "world transform contains a reflection or a non-rigid basis",
            ));
        }

        let uniform_scale = (scales[0] + scales[1] + scales[2]) / 3.0;
        let scale_tolerance = NODE_POSE_RIGID_TOLERANCE * uniform_scale.max(1.0);
        if scales
            .iter()
            .any(|scale| (*scale - uniform_scale).abs() > scale_tolerance)
        {
            return Err(non_rigid_pose(
                &path,
                "world transform has non-uniform scale",
            ));
        }
        if policy == NodePoseRigidScalePolicy::RequireUnitScale
            && (uniform_scale - 1.0).abs() > NODE_POSE_RIGID_TOLERANCE
        {
            return Err(non_rigid_pose(
                &path,
                "world transform scale is not one under RequireUnitScale",
            ));
        }

        Ok(AdmittedRigidNodePose {
            source_node_index: self.source_node_index,
            affine_world_transform: self.world_transform,
            uniform_scale,
        })
    }
}

pub(super) struct EvaluatedPose {
    pub(super) model_transforms: BTreeMap<u32, [f64; 16]>,
    pub(super) morph_weights: BTreeMap<u32, Vec<f64>>,
    local_transforms: Vec<[f64; 16]>,
    world_transforms: Vec<[f64; 16]>,
}

/// Evaluate every selected-scene node at one explicit clip timestamp without
/// materializing or deforming mesh geometry.
///
/// Times are integer microseconds in `0..=clip_duration_microseconds`.
/// Channel times outside their own key range follow glTF endpoint semantics,
/// while a timestamp outside the selected clip is rejected rather than
/// clamped. Morph channels are evaluated for canonical validation but do not
/// alter the returned transforms.
pub fn evaluate_clip_node_poses(
    model: &ImportedAnimatedModel,
    clip_name: &str,
    timestamp_microseconds: u64,
) -> Result<AnimationNodePoseReceipt, ConversionError> {
    let clip = find_clip(model, clip_name)?;
    let pose = evaluate_pose(model, Some(clip), timestamp_microseconds)?;
    let nodes = model
        .scene
        .nodes
        .iter()
        .zip(pose.local_transforms.into_iter().zip(pose.world_transforms))
        .map(
            |(node, (local_transform, world_transform))| AnimationNodePose {
                source_node_index: node.source_node_index,
                local_transform,
                world_transform,
            },
        )
        .collect();
    Ok(AnimationNodePoseReceipt {
        source_sha256: model.source_sha256.clone(),
        source_animation_index: clip.source_animation_index,
        clip_name: clip.name.clone(),
        clip_duration_microseconds: clip.duration_microseconds,
        timestamp_microseconds,
        nodes,
    })
}

pub(super) fn find_clip<'a>(
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

pub(super) fn evaluate_pose(
    model: &ImportedAnimatedModel,
    clip: Option<&ImportedAnimationClip>,
    timestamp_microseconds: u64,
) -> Result<EvaluatedPose, ConversionError> {
    let mut transforms = BTreeMap::new();
    let mut morph_weights = BTreeMap::new();
    for node in &model.nodes {
        if transforms
            .insert(node.source_node_index, node.base_transform)
            .is_some()
        {
            return Err(ConversionError::one(
                "conversion.invalidAnimation",
                format!("source.nodes[{}]", node.source_node_index),
                "animation node metadata contains a duplicate source node identity",
            ));
        }
        morph_weights.insert(node.source_node_index, node.base_morph_weights.clone());
    }

    if let Some(clip) = clip {
        if timestamp_microseconds > clip.duration_microseconds {
            return Err(ConversionError::one(
                "conversion.invalidSampleTime",
                "request.timestampMicroseconds",
                format!(
                    "sample timestamp {timestamp_microseconds} exceeds clip duration {}",
                    clip.duration_microseconds
                ),
            ));
        }
        apply_channels(
            clip,
            timestamp_microseconds,
            &mut transforms,
            &mut morph_weights,
        )?;
    }

    let local_transforms = build_local_transforms(model, &transforms)?;
    let composed = compose_world_transforms(model, &local_transforms)?;
    validate_child_links(model)?;
    Ok(EvaluatedPose {
        model_transforms: composed.by_identity,
        morph_weights,
        local_transforms,
        world_transforms: composed.world_transforms,
    })
}

fn apply_channels(
    clip: &ImportedAnimationClip,
    timestamp_microseconds: u64,
    transforms: &mut BTreeMap<u32, ImportedNodeTransform>,
    morph_weights: &mut BTreeMap<u32, Vec<f64>>,
) -> Result<(), ConversionError> {
    for channel in &clip.channels {
        match channel.property {
            AnimationProperty::Translation => {
                let sampled = sample_translation(channel, timestamp_microseconds)?;
                let transform = target_transform(transforms, channel.target_node_index)?;
                let ImportedNodeTransform::Decomposed { translation, .. } = transform else {
                    return Err(matrix_animation_target(channel.target_node_index));
                };
                *translation = sampled;
            }
            AnimationProperty::Rotation => {
                let sampled = sample_rotation(channel, timestamp_microseconds)?;
                let transform = target_transform(transforms, channel.target_node_index)?;
                let ImportedNodeTransform::Decomposed { rotation, .. } = transform else {
                    return Err(matrix_animation_target(channel.target_node_index));
                };
                *rotation = sampled;
            }
            AnimationProperty::Scale => {
                let sampled = sample_scale(channel, timestamp_microseconds)?;
                let transform = target_transform(transforms, channel.target_node_index)?;
                let ImportedNodeTransform::Decomposed { scale, .. } = transform else {
                    return Err(matrix_animation_target(channel.target_node_index));
                };
                *scale = sampled;
            }
            AnimationProperty::MorphWeights => {
                let sampled = sample_morph_weights(channel, timestamp_microseconds)?;
                let target = morph_weights
                    .get_mut(&channel.target_node_index)
                    .ok_or_else(|| missing_animation_node(channel.target_node_index))?;
                *target = sampled;
            }
        }
    }
    Ok(())
}

fn target_transform(
    transforms: &mut BTreeMap<u32, ImportedNodeTransform>,
    source_node_index: u32,
) -> Result<&mut ImportedNodeTransform, ConversionError> {
    transforms
        .get_mut(&source_node_index)
        .ok_or_else(|| missing_animation_node(source_node_index))
}

fn build_local_transforms(
    model: &ImportedAnimatedModel,
    transforms: &BTreeMap<u32, ImportedNodeTransform>,
) -> Result<Vec<[f64; 16]>, ConversionError> {
    model
        .scene
        .nodes
        .iter()
        .map(|scene_node| {
            let transform = transforms
                .get(&scene_node.source_node_index)
                .ok_or_else(|| missing_animation_node(scene_node.source_node_index))?;
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
            Ok(local)
        })
        .collect()
}

fn validate_child_links(model: &ImportedAnimatedModel) -> Result<(), ConversionError> {
    let indices = scene_node_indices(model)?;
    for node in &model.scene.nodes {
        let mut children = BTreeSet::new();
        for &child_node_index in &node.child_node_indices {
            if !children.insert(child_node_index) {
                return Err(invalid_hierarchy(
                    node.source_node_index,
                    format!("child node {child_node_index} is listed more than once"),
                ));
            }
            let child = indices
                .get(&child_node_index)
                .map(|index| &model.scene.nodes[*index])
                .ok_or_else(|| {
                    invalid_hierarchy(
                        node.source_node_index,
                        format!("child references missing node {child_node_index}"),
                    )
                })?;
            if child.parent_node_index != Some(node.source_node_index) {
                return Err(invalid_hierarchy(
                    node.source_node_index,
                    format!(
                        "child node {child_node_index} does not name {} as its parent",
                        node.source_node_index
                    ),
                ));
            }
        }
    }
    Ok(())
}

fn compose_world_transforms(
    model: &ImportedAnimatedModel,
    local_transforms: &[[f64; 16]],
) -> Result<ComposedTransforms, ConversionError> {
    let indices = scene_node_indices(model)?;
    let mut states = vec![VisitState::Unseen; model.scene.nodes.len()];
    let mut world_transforms = vec![None; model.scene.nodes.len()];
    for index in 0..model.scene.nodes.len() {
        compose_world_transform(
            index,
            1,
            model,
            &indices,
            local_transforms,
            &mut states,
            &mut world_transforms,
        )?;
    }

    let world_transforms = world_transforms
        .into_iter()
        .map(|transform| transform.expect("complete hierarchy evaluation assigns every node"))
        .collect::<Vec<_>>();
    let by_identity = model
        .scene
        .nodes
        .iter()
        .zip(&world_transforms)
        .map(|(node, transform)| (node.source_node_index, *transform))
        .collect();
    Ok(ComposedTransforms {
        world_transforms,
        by_identity,
    })
}

struct ComposedTransforms {
    world_transforms: Vec<[f64; 16]>,
    by_identity: BTreeMap<u32, [f64; 16]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VisitState {
    Unseen,
    Visiting,
    Complete,
}

#[allow(clippy::too_many_arguments)]
fn compose_world_transform(
    index: usize,
    depth: usize,
    model: &ImportedAnimatedModel,
    indices: &BTreeMap<u32, usize>,
    local_transforms: &[[f64; 16]],
    states: &mut [VisitState],
    world_transforms: &mut [Option<[f64; 16]>],
) -> Result<[f64; 16], ConversionError> {
    let node = &model.scene.nodes[index];
    match states[index] {
        VisitState::Complete => {
            return Ok(world_transforms[index]
                .expect("complete hierarchy nodes retain their world transform"));
        }
        VisitState::Visiting => {
            return Err(invalid_hierarchy(
                node.source_node_index,
                format!(
                    "node hierarchy contains a cycle through node {}",
                    node.source_node_index
                ),
            ));
        }
        VisitState::Unseen => {}
    }
    if depth > MAX_IMPORTED_SCENE_DEPTH {
        return Err(ConversionError::one(
            "conversion.resourceLimit",
            format!("sample.nodes[{}]", node.source_node_index),
            format!("node hierarchy exceeds depth {MAX_IMPORTED_SCENE_DEPTH}"),
        ));
    }

    states[index] = VisitState::Visiting;
    let world = match node.parent_node_index {
        Some(parent_node_index) => {
            let parent_index = *indices.get(&parent_node_index).ok_or_else(|| {
                invalid_hierarchy(
                    node.source_node_index,
                    format!("parent references missing node {parent_node_index}"),
                )
            })?;
            let parent_world = compose_world_transform(
                parent_index,
                depth + 1,
                model,
                indices,
                local_transforms,
                states,
                world_transforms,
            )?;
            multiply_matrices(parent_world, local_transforms[index])
        }
        None => local_transforms[index],
    };
    validate_affine_matrix(
        world,
        format!("sample.nodes[{}].modelTransform", node.source_node_index),
    )?;
    world_transforms[index] = Some(world);
    states[index] = VisitState::Complete;
    Ok(world)
}

fn scene_node_indices(
    model: &ImportedAnimatedModel,
) -> Result<BTreeMap<u32, usize>, ConversionError> {
    let mut indices = BTreeMap::new();
    for (index, node) in model.scene.nodes.iter().enumerate() {
        if indices.insert(node.source_node_index, index).is_some() {
            return Err(invalid_hierarchy(
                node.source_node_index,
                "selected scene contains a duplicate source node identity",
            ));
        }
    }
    Ok(indices)
}

fn missing_animation_node(source_node_index: u32) -> ConversionError {
    ConversionError::one(
        "conversion.invalidAnimation",
        format!("source.nodes[{source_node_index}]"),
        "animation node metadata is missing",
    )
}

fn matrix_animation_target(source_node_index: u32) -> ConversionError {
    ConversionError::one(
        "conversion.invalidAnimation",
        format!("source.nodes[{source_node_index}].transform"),
        "TRS animation cannot target a matrix-authored node",
    )
}

fn invalid_hierarchy(source_node_index: u32, message: impl Into<String>) -> ConversionError {
    ConversionError::one(
        "conversion.invalidSceneHierarchy",
        format!("sample.nodes[{source_node_index}].parentNodeIndex"),
        message,
    )
}

fn non_rigid_pose(path: &str, message: impl Into<String>) -> ConversionError {
    ConversionError::one("conversion.nonRigidNodePose", path, message)
}

fn vector_length(vector: [f64; 3]) -> f64 {
    dot(vector, vector).sqrt()
}

fn dot(left: [f64; 3], right: [f64; 3]) -> f64 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

fn cross(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}
