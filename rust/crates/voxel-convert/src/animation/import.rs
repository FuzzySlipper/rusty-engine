use std::collections::{BTreeMap, BTreeSet};

use gltf::{buffer::Source as BufferSource, mesh::Semantic, scene::Transform};

use super::{
    ImportedAnimatedModel, ImportedAnimationNode, ImportedNodeTransform,
    ImportedPrimitiveDeformation, ImportedSkin, MAX_IMPORTED_JOINTS_PER_SKIN,
    MAX_IMPORTED_MORPH_POSITION_DELTAS, MAX_IMPORTED_MORPH_TARGETS, MAX_IMPORTED_SKINS,
};
use crate::import::{
    identity_matrix, import_animated_glb_scene, matrix_from_gltf, validate_affine_matrix,
    validate_imported_name,
};
use crate::{source_sha256, ConversionError, ImportedModelScene};

mod clip;

pub(super) fn import_animated_glb(source: &[u8]) -> Result<ImportedAnimatedModel, ConversionError> {
    let (parsed, scene) = import_animated_glb_scene(source)?;
    let blob = parsed
        .blob
        .as_deref()
        .expect("animated GLB scene import requires an embedded BIN chunk");
    let (primitive_deformations, mesh_morph_target_counts) =
        import_primitive_deformations(&parsed.document, blob, &scene)?;
    let nodes = import_nodes(&parsed.document, &scene, &mesh_morph_target_counts)?;
    let skins = import_skins(&parsed.document, blob, &scene, &nodes)?;
    validate_skin_bindings(&scene, &nodes, &skins, &primitive_deformations)?;
    let clips = clip::import_clips(&parsed.document, blob, &nodes)?;

    Ok(ImportedAnimatedModel {
        source_sha256: source_sha256(source),
        scene,
        nodes,
        skins,
        primitive_deformations,
        clips,
    })
}

fn import_primitive_deformations(
    document: &gltf::Document,
    blob: &[u8],
    scene: &ImportedModelScene,
) -> Result<(Vec<ImportedPrimitiveDeformation>, BTreeMap<u32, usize>), ConversionError> {
    let mut imported = Vec::new();
    let mut mesh_morph_target_counts = BTreeMap::new();
    let mut total_morph_position_deltas = 0usize;

    for mesh in &scene.meshes {
        let source_mesh = document
            .meshes()
            .nth(mesh.source_mesh_index as usize)
            .ok_or_else(|| {
                ConversionError::one(
                    "conversion.invalidGeometry",
                    format!("source.meshes[{}]", mesh.source_mesh_index),
                    "selected mesh disappeared during animated import",
                )
            })?;
        let mut mesh_target_count = None;
        for primitive in &mesh.primitives {
            let source_primitive = source_mesh
                .primitives()
                .nth(primitive.source_primitive_index as usize)
                .ok_or_else(|| {
                    ConversionError::one(
                        "conversion.invalidGeometry",
                        format!(
                            "source.meshes[{}].primitives[{}]",
                            mesh.source_mesh_index, primitive.source_primitive_index
                        ),
                        "selected primitive disappeared during animated import",
                    )
                })?;
            let primitive_path = format!(
                "source.meshes[{}].primitives[{}]",
                mesh.source_mesh_index, primitive.source_primitive_index
            );
            reject_additional_joint_sets(&source_primitive, &primitive_path)?;
            validate_vertex_attribute_count(
                source_primitive.get(&Semantic::Joints(0)),
                primitive.positions.len(),
                &format!("{primitive_path}.attributes.JOINTS_0"),
            )?;
            validate_vertex_attribute_count(
                source_primitive.get(&Semantic::Weights(0)),
                primitive.positions.len(),
                &format!("{primitive_path}.attributes.WEIGHTS_0"),
            )?;
            let reader = source_primitive.reader(|buffer| match buffer.source() {
                BufferSource::Bin => Some(blob),
                BufferSource::Uri(_) => None,
            });

            let vertex_joints = reader
                .read_joints(0)
                .map(|values| values.into_u16().collect::<Vec<_>>());
            let vertex_weights = reader
                .read_weights(0)
                .map(|values| values.into_f32().collect::<Vec<_>>())
                .transpose_f64_weights(&primitive_path)?;
            if vertex_joints.is_some() != vertex_weights.is_some() {
                return Err(ConversionError::one(
                    "conversion.invalidSkin",
                    format!("{primitive_path}.attributes"),
                    "JOINTS_0 and WEIGHTS_0 must either both be present or both be absent",
                ));
            }
            if vertex_joints
                .as_ref()
                .is_some_and(|values| values.len() != primitive.positions.len())
                || vertex_weights
                    .as_ref()
                    .is_some_and(|values| values.len() != primitive.positions.len())
            {
                return Err(ConversionError::one(
                    "conversion.invalidSkin",
                    format!("{primitive_path}.attributes"),
                    "joint and weight attributes must contain one value per POSITION",
                ));
            }

            let target_count = source_primitive.morph_targets().count();
            if target_count > MAX_IMPORTED_MORPH_TARGETS {
                return Err(ConversionError::one(
                    "conversion.resourceLimit",
                    format!("{primitive_path}.targets"),
                    format!("morph target count exceeds {MAX_IMPORTED_MORPH_TARGETS}"),
                ));
            }
            if let Some(expected) = mesh_target_count {
                if expected != target_count {
                    return Err(ConversionError::one(
                        "conversion.invalidMorphTarget",
                        format!("{primitive_path}.targets"),
                        "every primitive in one mesh must expose the same morph target count",
                    ));
                }
            } else {
                mesh_target_count = Some(target_count);
            }
            let incoming_morph_deltas = target_count
                .checked_mul(primitive.positions.len())
                .ok_or_else(|| morph_delta_limit("morph displacement count overflowed"))?;
            total_morph_position_deltas = total_morph_position_deltas
                .checked_add(incoming_morph_deltas)
                .ok_or_else(|| morph_delta_limit("total morph displacement count overflowed"))?;
            if total_morph_position_deltas > MAX_IMPORTED_MORPH_POSITION_DELTAS {
                return Err(morph_delta_limit(&format!(
                    "total morph displacement count exceeds {MAX_IMPORTED_MORPH_POSITION_DELTAS}"
                )));
            }
            for (target_index, target) in source_primitive.morph_targets().enumerate() {
                if let Some(accessor) = target.positions() {
                    validate_accessor_count(
                        accessor.count(),
                        primitive.positions.len(),
                        &format!("{primitive_path}.targets[{target_index}].POSITION"),
                    )?;
                }
            }

            let mut morph_position_deltas = Vec::with_capacity(target_count);
            for (target_index, (positions, _normals, _tangents)) in
                reader.read_morph_targets().enumerate()
            {
                let deltas = match positions {
                    Some(values) => values.map(|value| value.map(f64::from)).collect::<Vec<_>>(),
                    None => vec![[0.0; 3]; primitive.positions.len()],
                };
                if deltas.len() != primitive.positions.len()
                    || deltas.iter().flatten().any(|value| !value.is_finite())
                {
                    return Err(ConversionError::one(
                        "conversion.invalidMorphTarget",
                        format!("{primitive_path}.targets[{target_index}].POSITION"),
                        "morph POSITION must contain one finite displacement per vertex",
                    ));
                }
                morph_position_deltas.push(deltas);
            }

            imported.push(ImportedPrimitiveDeformation {
                source_mesh_index: mesh.source_mesh_index,
                source_primitive_index: primitive.source_primitive_index,
                vertex_joints,
                vertex_weights,
                morph_position_deltas,
            });
        }
        mesh_morph_target_counts.insert(mesh.source_mesh_index, mesh_target_count.unwrap_or(0));
    }

    Ok((imported, mesh_morph_target_counts))
}

fn reject_additional_joint_sets(
    primitive: &gltf::Primitive<'_>,
    primitive_path: &str,
) -> Result<(), ConversionError> {
    for (semantic, _) in primitive.attributes() {
        match semantic {
            Semantic::Joints(set) | Semantic::Weights(set) if set > 0 => {
                return Err(ConversionError::one(
                    "conversion.unsupportedFeature",
                    format!("{primitive_path}.attributes"),
                    "only JOINTS_0 and WEIGHTS_0 are supported for offline deformation",
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

fn validate_vertex_attribute_count(
    accessor: Option<gltf::Accessor<'_>>,
    expected: usize,
    path: &str,
) -> Result<(), ConversionError> {
    if let Some(accessor) = accessor {
        validate_accessor_count(accessor.count(), expected, path)?;
    }
    Ok(())
}

fn validate_accessor_count(
    actual: usize,
    expected: usize,
    path: &str,
) -> Result<(), ConversionError> {
    if actual != expected {
        return Err(ConversionError::one(
            "conversion.invalidAccessor",
            path,
            format!("accessor count must be {expected}; found {actual}"),
        ));
    }
    Ok(())
}

trait OptionalWeightsExt {
    fn transpose_f64_weights(
        self,
        primitive_path: &str,
    ) -> Result<Option<Vec<[f64; 4]>>, ConversionError>;
}

impl OptionalWeightsExt for Option<Vec<[f32; 4]>> {
    fn transpose_f64_weights(
        self,
        primitive_path: &str,
    ) -> Result<Option<Vec<[f64; 4]>>, ConversionError> {
        self.map(|weights| {
            weights
                .into_iter()
                .enumerate()
                .map(|(vertex_index, weights)| {
                    let mut weights = weights.map(f64::from);
                    if weights
                        .iter()
                        .any(|weight| !weight.is_finite() || *weight < 0.0)
                    {
                        return Err(ConversionError::one(
                            "conversion.invalidSkin",
                            format!("{primitive_path}.attributes.WEIGHTS_0[{vertex_index}]"),
                            "skin weights must be finite and non-negative",
                        ));
                    }
                    let sum = weights.iter().sum::<f64>();
                    if !sum.is_finite() || sum <= f64::EPSILON {
                        return Err(ConversionError::one(
                            "conversion.invalidSkin",
                            format!("{primitive_path}.attributes.WEIGHTS_0[{vertex_index}]"),
                            "each skinned vertex must have positive total weight",
                        ));
                    }
                    for weight in &mut weights {
                        *weight /= sum;
                    }
                    Ok(weights)
                })
                .collect()
        })
        .transpose()
    }
}

fn import_nodes(
    document: &gltf::Document,
    scene: &ImportedModelScene,
    mesh_morph_target_counts: &BTreeMap<u32, usize>,
) -> Result<Vec<ImportedAnimationNode>, ConversionError> {
    scene
        .nodes
        .iter()
        .map(|scene_node| {
            let node_index = scene_node.source_node_index as usize;
            let node = document.nodes().nth(node_index).ok_or_else(|| {
                ConversionError::one(
                    "conversion.invalidSceneHierarchy",
                    format!("source.nodes[{node_index}]"),
                    "selected node disappeared during animated import",
                )
            })?;
            let base_transform = match node.transform() {
                Transform::Matrix { matrix } => {
                    ImportedNodeTransform::Matrix(matrix_from_gltf(matrix))
                }
                Transform::Decomposed {
                    translation,
                    rotation,
                    scale,
                } => ImportedNodeTransform::Decomposed {
                    translation: finite_array(
                        translation.map(f64::from),
                        &format!("source.nodes[{node_index}].translation"),
                    )?,
                    rotation: normalize_quaternion(
                        rotation.map(f64::from),
                        &format!("source.nodes[{node_index}].rotation"),
                    )?,
                    scale: finite_array(
                        scale.map(f64::from),
                        &format!("source.nodes[{node_index}].scale"),
                    )?,
                },
            };
            let source_skin_index = node
                .skin()
                .map(|skin| u32::try_from(skin.index()))
                .transpose()
                .map_err(|_| {
                    ConversionError::one(
                        "conversion.resourceLimit",
                        format!("source.nodes[{node_index}].skin"),
                        "skin index exceeds u32",
                    )
                })?;
            if source_skin_index.is_some() && scene_node.source_mesh_index.is_none() {
                return Err(ConversionError::one(
                    "conversion.invalidSkin",
                    format!("source.nodes[{node_index}].skin"),
                    "a selected skin must be attached to a mesh node",
                ));
            }

            let target_count = scene_node
                .source_mesh_index
                .and_then(|mesh_index| mesh_morph_target_counts.get(&mesh_index).copied())
                .unwrap_or(0);
            let mesh_weights = scene_node.source_mesh_index.and_then(|mesh_index| {
                document
                    .meshes()
                    .nth(mesh_index as usize)
                    .and_then(|mesh| mesh.weights())
            });
            let authored_weights = node.weights().or(mesh_weights);
            let base_morph_weights = if target_count == 0 {
                if authored_weights
                    .as_ref()
                    .is_some_and(|weights| !weights.is_empty())
                {
                    return Err(ConversionError::one(
                        "conversion.invalidMorphTarget",
                        format!("source.nodes[{node_index}].weights"),
                        "node or mesh defines weights without morph targets",
                    ));
                }
                Vec::new()
            } else {
                if authored_weights.is_some_and(|weights| {
                    weights.len() != target_count
                        || weights.iter().any(|weight| !weight.is_finite())
                }) {
                    return Err(ConversionError::one(
                        "conversion.invalidMorphTarget",
                        format!("source.nodes[{node_index}].weights"),
                        "morph weights must contain one finite value per target",
                    ));
                }
                authored_weights.map_or_else(
                    || vec![0.0; target_count],
                    |weights| weights.iter().copied().map(f64::from).collect(),
                )
            };

            Ok(ImportedAnimationNode {
                source_node_index: scene_node.source_node_index,
                source_skin_index,
                base_transform,
                base_morph_weights,
            })
        })
        .collect()
}

fn import_skins(
    document: &gltf::Document,
    blob: &[u8],
    scene: &ImportedModelScene,
    nodes: &[ImportedAnimationNode],
) -> Result<Vec<ImportedSkin>, ConversionError> {
    let skin_count = document.skins().count();
    if skin_count > MAX_IMPORTED_SKINS {
        return Err(ConversionError::one(
            "conversion.resourceLimit",
            "source.skins",
            format!("skin count exceeds {MAX_IMPORTED_SKINS}"),
        ));
    }
    let referenced = nodes
        .iter()
        .filter_map(|node| node.source_skin_index)
        .collect::<BTreeSet<_>>();
    let reachable = scene
        .nodes
        .iter()
        .map(|node| node.source_node_index)
        .collect::<BTreeSet<_>>();
    let mut imported = Vec::with_capacity(referenced.len());

    for source_skin_index in referenced {
        let skin = document
            .skins()
            .nth(source_skin_index as usize)
            .ok_or_else(|| {
                ConversionError::one(
                    "conversion.invalidSkin",
                    format!("source.skins[{source_skin_index}]"),
                    "selected node references a missing skin",
                )
            })?;
        let path = format!("source.skins[{source_skin_index}]");
        let joint_count = skin.joints().count();
        if joint_count == 0 || joint_count > MAX_IMPORTED_JOINTS_PER_SKIN {
            return Err(ConversionError::one(
                "conversion.resourceLimit",
                format!("{path}.joints"),
                format!("joint count must be in 1..={MAX_IMPORTED_JOINTS_PER_SKIN}"),
            ));
        }
        let joint_node_indices = skin
            .joints()
            .map(|joint| u32::try_from(joint.index()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| {
                ConversionError::one(
                    "conversion.resourceLimit",
                    format!("{path}.joints"),
                    "joint node index exceeds u32",
                )
            })?;
        if joint_node_indices
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .len()
            != joint_node_indices.len()
        {
            return Err(ConversionError::one(
                "conversion.invalidSkin",
                format!("{path}.joints"),
                "skin joint node identities must be unique",
            ));
        }
        if let Some(missing) = joint_node_indices
            .iter()
            .find(|joint| !reachable.contains(joint))
        {
            return Err(ConversionError::one(
                "conversion.invalidSkin",
                format!("{path}.joints"),
                format!("joint node {missing} is not reachable from the selected scene"),
            ));
        }
        let skeleton_node_index = skin
            .skeleton()
            .map(|node| u32::try_from(node.index()))
            .transpose()
            .map_err(|_| {
                ConversionError::one(
                    "conversion.resourceLimit",
                    format!("{path}.skeleton"),
                    "skeleton node index exceeds u32",
                )
            })?;
        if skeleton_node_index.is_some_and(|node| !reachable.contains(&node)) {
            return Err(ConversionError::one(
                "conversion.invalidSkin",
                format!("{path}.skeleton"),
                "skeleton node is not reachable from the selected scene",
            ));
        }

        let inverse_bind_matrices = if let Some(accessor) = skin.inverse_bind_matrices() {
            validate_accessor_count(
                accessor.count(),
                joint_node_indices.len(),
                &format!("{path}.inverseBindMatrices"),
            )?;
            skin.reader(|buffer| match buffer.source() {
                BufferSource::Bin => Some(blob),
                BufferSource::Uri(_) => None,
            })
            .read_inverse_bind_matrices()
            .ok_or_else(|| {
                ConversionError::one(
                    "conversion.invalidAccessor",
                    format!("{path}.inverseBindMatrices"),
                    "inverse bind matrix accessor could not be read",
                )
            })?
            .map(matrix_from_gltf)
            .collect::<Vec<_>>()
        } else {
            vec![identity_matrix(); joint_node_indices.len()]
        };
        if inverse_bind_matrices.len() != joint_node_indices.len() {
            return Err(ConversionError::one(
                "conversion.invalidSkin",
                format!("{path}.inverseBindMatrices"),
                "inverse bind matrix count must equal joint count",
            ));
        }
        for (matrix_index, matrix) in inverse_bind_matrices.iter().copied().enumerate() {
            validate_affine_matrix(
                matrix,
                format!("{path}.inverseBindMatrices[{matrix_index}]"),
            )?;
        }

        imported.push(ImportedSkin {
            source_skin_index,
            source_skin_name: validate_imported_name(skin.name(), format!("{path}.name"))?,
            skeleton_node_index,
            joint_node_indices,
            inverse_bind_matrices,
        });
    }
    Ok(imported)
}

fn validate_skin_bindings(
    scene: &ImportedModelScene,
    nodes: &[ImportedAnimationNode],
    skins: &[ImportedSkin],
    deformations: &[ImportedPrimitiveDeformation],
) -> Result<(), ConversionError> {
    for node in nodes {
        let Some(skin_index) = node.source_skin_index else {
            continue;
        };
        let scene_node = scene
            .nodes
            .iter()
            .find(|candidate| candidate.source_node_index == node.source_node_index)
            .expect("animation nodes are imported from the selected scene");
        let mesh_index = scene_node
            .source_mesh_index
            .expect("skin attachment was validated as a mesh node");
        let skin = skins
            .iter()
            .find(|skin| skin.source_skin_index == skin_index)
            .expect("referenced skins are imported before binding validation");
        for deformation in deformations
            .iter()
            .filter(|deformation| deformation.source_mesh_index == mesh_index)
        {
            let joints = deformation.vertex_joints.as_ref().ok_or_else(|| {
                ConversionError::one(
                    "conversion.invalidSkin",
                    format!(
                        "source.meshes[{mesh_index}].primitives[{}].attributes.JOINTS_0",
                        deformation.source_primitive_index
                    ),
                    "a skinned mesh primitive must define JOINTS_0 and WEIGHTS_0",
                )
            })?;
            if let Some(joint) = joints
                .iter()
                .flatten()
                .find(|joint| **joint as usize >= skin.joint_node_indices.len())
            {
                return Err(ConversionError::one(
                    "conversion.invalidSkin",
                    format!(
                        "source.meshes[{mesh_index}].primitives[{}].attributes.JOINTS_0",
                        deformation.source_primitive_index
                    ),
                    format!("joint index {joint} exceeds skin {skin_index}'s joint table"),
                ));
            }
        }
    }
    Ok(())
}

fn finite_array<const N: usize>(value: [f64; N], path: &str) -> Result<[f64; N], ConversionError> {
    if value.iter().any(|component| !component.is_finite()) {
        return Err(ConversionError::one(
            "conversion.invalidTransform",
            path,
            "node transform components must be finite",
        ));
    }
    Ok(value)
}

pub(super) fn normalize_quaternion(
    mut value: [f64; 4],
    path: &str,
) -> Result<[f64; 4], ConversionError> {
    if value.iter().any(|component| !component.is_finite()) {
        return Err(ConversionError::one(
            "conversion.invalidAnimation",
            path,
            "rotation quaternion must be finite",
        ));
    }
    let length = value
        .iter()
        .map(|component| component * component)
        .sum::<f64>()
        .sqrt();
    if !length.is_finite() || length <= f64::EPSILON {
        return Err(ConversionError::one(
            "conversion.invalidAnimation",
            path,
            "rotation quaternion must have non-zero finite length",
        ));
    }
    for component in &mut value {
        *component /= length;
    }
    Ok(value)
}

fn morph_delta_limit(message: &str) -> ConversionError {
    ConversionError::one(
        "conversion.resourceLimit",
        "source.meshes.morphPositionDeltas",
        message.to_owned(),
    )
}
