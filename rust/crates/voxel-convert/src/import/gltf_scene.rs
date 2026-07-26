use std::collections::{BTreeMap, BTreeSet};

use gltf::{buffer::Source as BufferSource, mesh::Mode, Semantic};
use voxel_asset::{
    MAX_CONVERSION_SOURCE_BYTES, MAX_CONVERSION_SOURCE_INDICES, MAX_CONVERSION_SOURCE_VERTICES,
    MAX_MATERIAL_MAPPINGS,
};

use super::{
    ensure_total_limit, identity_matrix, matrix_from_gltf, multiply_matrices,
    validate_affine_matrix, validate_imported_name, ImportedMaterial, ImportedModelMesh,
    ImportedModelNode, ImportedModelPrimitive, ImportedModelScene, ImportedTextureCoordinates,
    MAX_IMPORTED_SCENE_DEPTH, MAX_IMPORTED_SCENE_EDGES, MAX_IMPORTED_SCENE_MESHES,
    MAX_IMPORTED_SCENE_MESH_INSTANCES, MAX_IMPORTED_SCENE_NODES, MAX_IMPORTED_SCENE_PRIMITIVES,
    MAX_IMPORTED_TEXCOORD_SETS,
};
use crate::ConversionError;

#[derive(Debug)]
struct SourceNode {
    source_node_index: usize,
    source_node_name: Option<String>,
    child_node_indices: Vec<usize>,
    source_mesh_index: Option<usize>,
    local_transform: [f64; 16],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VisitState {
    Unseen,
    Visiting,
    Complete,
}

pub(super) fn import_static_glb_scene(
    source: &[u8],
) -> Result<ImportedModelScene, ConversionError> {
    let parsed = parse_embedded_glb(source)?;
    import_glb_scene(&parsed, false)
}

pub(super) fn parse_animated_glb(
    source: &[u8],
) -> Result<(gltf::Gltf, ImportedModelScene), ConversionError> {
    let parsed = parse_embedded_glb(source)?;
    let scene = import_glb_scene(&parsed, true)?;
    Ok((parsed, scene))
}

fn parse_embedded_glb(source: &[u8]) -> Result<gltf::Gltf, ConversionError> {
    if source.is_empty() || source.len() as u64 > MAX_CONVERSION_SOURCE_BYTES {
        return Err(ConversionError::one(
            "conversion.resourceLimit",
            "source",
            format!(
                "source byte count {} is outside 1..={MAX_CONVERSION_SOURCE_BYTES}",
                source.len()
            ),
        ));
    }
    let parsed = gltf::Gltf::from_slice(source).map_err(|error| {
        ConversionError::one(
            "conversion.invalidSource",
            "source",
            format!("invalid GLB 2.0 source: {error}"),
        )
    })?;
    parsed.blob.as_deref().ok_or_else(|| {
        ConversionError::one(
            "conversion.unsupportedFeature",
            "source",
            "GLB source must contain one embedded BIN chunk",
        )
    })?;
    for buffer in parsed.document.buffers() {
        if !matches!(buffer.source(), BufferSource::Bin) {
            return Err(ConversionError::one(
                "conversion.unsupportedFeature",
                format!("source.buffers[{}].uri", buffer.index()),
                "GLB conversion never resolves external buffer resources",
            ));
        }
    }
    Ok(parsed)
}

fn import_glb_scene(
    parsed: &gltf::Gltf,
    allow_animated_features: bool,
) -> Result<ImportedModelScene, ConversionError> {
    let blob = parsed
        .blob
        .as_deref()
        .expect("embedded GLB parser requires a BIN chunk");
    if !allow_animated_features
        && (parsed.document.animations().next().is_some()
            || parsed.document.skins().next().is_some())
    {
        return Err(ConversionError::one(
            "conversion.unsupportedFeature",
            "source",
            "animation and skin sampling belong to the animated import stage",
        ));
    }
    let default_scene = parsed.document.default_scene().ok_or_else(|| {
        ConversionError::one(
            "conversion.unsupportedFeature",
            "source.scene",
            "GLB conversion requires an explicit default scene",
        )
    })?;
    let source_scene_index = u32::try_from(default_scene.index()).map_err(|_| {
        ConversionError::one(
            "conversion.resourceLimit",
            "source.scene",
            "default scene index exceeds u32",
        )
    })?;
    let source_scene_name = validate_imported_name(
        default_scene.name(),
        format!("source.scenes[{}].name", default_scene.index()),
    )?;
    let source_nodes = collect_source_nodes(&parsed.document, allow_animated_features)?;
    let roots = default_scene
        .nodes()
        .map(|node| node.index())
        .collect::<Vec<_>>();
    let (nodes, referenced_meshes) = traverse_default_scene(&source_nodes, &roots)?;
    let (meshes, materials) = import_referenced_meshes(
        &parsed.document,
        blob,
        &referenced_meshes,
        allow_animated_features,
    )?;

    Ok(ImportedModelScene {
        source_scene_index,
        source_scene_name,
        nodes,
        meshes,
        materials,
    })
}

fn collect_source_nodes(
    document: &gltf::Document,
    allow_animated_features: bool,
) -> Result<Vec<SourceNode>, ConversionError> {
    let node_count = document.nodes().count();
    if node_count == 0 || node_count > MAX_IMPORTED_SCENE_NODES {
        return Err(ConversionError::one(
            "conversion.resourceLimit",
            "source.nodes",
            format!("node count must be in 1..={MAX_IMPORTED_SCENE_NODES}"),
        ));
    }
    let mesh_count = document.meshes().count();
    if mesh_count == 0 || mesh_count > MAX_IMPORTED_SCENE_MESHES {
        return Err(ConversionError::one(
            "conversion.resourceLimit",
            "source.meshes",
            format!("mesh count must be in 1..={MAX_IMPORTED_SCENE_MESHES}"),
        ));
    }

    let mut edge_count = 0usize;
    document
        .nodes()
        .map(|node| {
            let source_node_index = node.index();
            if !allow_animated_features && (node.skin().is_some() || node.weights().is_some()) {
                return Err(ConversionError::one(
                    "conversion.unsupportedFeature",
                    format!("source.nodes[{source_node_index}]"),
                    "skins and instance morph weights belong to animated import",
                ));
            }
            let child_node_indices = node
                .children()
                .map(|child| child.index())
                .collect::<Vec<_>>();
            edge_count = edge_count
                .checked_add(child_node_indices.len())
                .ok_or_else(|| hierarchy_limit("scene edge count overflowed"))?;
            if edge_count > MAX_IMPORTED_SCENE_EDGES {
                return Err(hierarchy_limit(&format!(
                    "scene contains more than {MAX_IMPORTED_SCENE_EDGES} child edges"
                )));
            }
            let local_transform = matrix_from_gltf(node.transform().matrix());
            validate_affine_matrix(
                local_transform,
                format!("source.nodes[{source_node_index}].transform"),
            )?;
            Ok(SourceNode {
                source_node_index,
                source_node_name: validate_imported_name(
                    node.name(),
                    format!("source.nodes[{source_node_index}].name"),
                )?,
                child_node_indices,
                source_mesh_index: node.mesh().map(|mesh| mesh.index()),
                local_transform,
            })
        })
        .collect()
}

fn traverse_default_scene(
    source_nodes: &[SourceNode],
    roots: &[usize],
) -> Result<(Vec<ImportedModelNode>, BTreeSet<usize>), ConversionError> {
    if roots.is_empty() {
        return Err(ConversionError::one(
            "conversion.invalidGeometry",
            "source.scene.nodes",
            "default scene has no root nodes",
        ));
    }
    let mut states = vec![VisitState::Unseen; source_nodes.len()];
    let mut parents = vec![None; source_nodes.len()];
    let mut model_transforms = vec![identity_matrix(); source_nodes.len()];
    let mut ordered_indices = Vec::new();

    for &root in roots {
        if root >= source_nodes.len() {
            return Err(ConversionError::one(
                "conversion.invalidSceneHierarchy",
                "source.scene.nodes",
                format!("default scene references missing node {root}"),
            ));
        }
        if states[root] != VisitState::Unseen {
            return Err(ambiguous_node(root));
        }
        states[root] = VisitState::Visiting;
        model_transforms[root] = source_nodes[root].local_transform;
        validate_affine_matrix(
            model_transforms[root],
            format!("source.nodes[{root}].transform"),
        )?;
        ordered_indices.push(root);
        traverse_root(
            root,
            source_nodes,
            &mut states,
            &mut parents,
            &mut model_transforms,
            &mut ordered_indices,
        )?;
    }

    let mut mesh_instances = 0usize;
    let mut referenced_meshes = BTreeSet::new();
    let mut nodes = Vec::with_capacity(ordered_indices.len());
    for source_node_index in ordered_indices {
        let source = &source_nodes[source_node_index];
        if let Some(mesh_index) = source.source_mesh_index {
            mesh_instances = mesh_instances.saturating_add(1);
            if mesh_instances > MAX_IMPORTED_SCENE_MESH_INSTANCES {
                return Err(ConversionError::one(
                    "conversion.resourceLimit",
                    "source.scene.meshInstances",
                    format!(
                        "selected scene contains more than {MAX_IMPORTED_SCENE_MESH_INSTANCES} mesh instances"
                    ),
                ));
            }
            referenced_meshes.insert(mesh_index);
        }
        nodes.push(ImportedModelNode {
            source_node_index: u32::try_from(source.source_node_index)
                .map_err(|_| hierarchy_limit("source node index exceeds u32"))?,
            source_node_name: source.source_node_name.clone(),
            parent_node_index: parents[source_node_index]
                .map(u32::try_from)
                .transpose()
                .map_err(|_| hierarchy_limit("parent node index exceeds u32"))?,
            child_node_indices: source
                .child_node_indices
                .iter()
                .copied()
                .map(u32::try_from)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| hierarchy_limit("child node index exceeds u32"))?,
            source_mesh_index: source
                .source_mesh_index
                .map(u32::try_from)
                .transpose()
                .map_err(|_| hierarchy_limit("source mesh index exceeds u32"))?,
            local_transform: source.local_transform,
            model_transform: model_transforms[source_node_index],
        });
    }
    if referenced_meshes.is_empty() {
        return Err(ConversionError::one(
            "conversion.invalidGeometry",
            "source.scene",
            "default scene contains no mesh instances",
        ));
    }
    Ok((nodes, referenced_meshes))
}

fn traverse_root(
    root: usize,
    source_nodes: &[SourceNode],
    states: &mut [VisitState],
    parents: &mut [Option<usize>],
    model_transforms: &mut [[f64; 16]],
    ordered_indices: &mut Vec<usize>,
) -> Result<(), ConversionError> {
    let mut stack = vec![(root, 0usize, 1usize)];
    while !stack.is_empty() {
        let (node_index, child_index, depth) = *stack.last().expect("checked non-empty stack");
        if child_index == source_nodes[node_index].child_node_indices.len() {
            states[node_index] = VisitState::Complete;
            stack.pop();
            continue;
        }
        stack.last_mut().expect("checked non-empty stack").1 += 1;
        let child = source_nodes[node_index].child_node_indices[child_index];
        if child >= source_nodes.len() {
            return Err(ConversionError::one(
                "conversion.invalidSceneHierarchy",
                format!("source.nodes[{node_index}].children[{child_index}]"),
                format!("child references missing node {child}"),
            ));
        }
        match states[child] {
            VisitState::Visiting => {
                return Err(ConversionError::one(
                    "conversion.invalidSceneHierarchy",
                    format!("source.nodes[{node_index}].children[{child_index}]"),
                    format!("node hierarchy contains a cycle through node {child}"),
                ));
            }
            VisitState::Complete => return Err(ambiguous_node(child)),
            VisitState::Unseen => {}
        }
        let child_depth = depth.saturating_add(1);
        if child_depth > MAX_IMPORTED_SCENE_DEPTH {
            return Err(ConversionError::one(
                "conversion.resourceLimit",
                format!("source.nodes[{child}]"),
                format!("scene depth exceeds {MAX_IMPORTED_SCENE_DEPTH}"),
            ));
        }
        parents[child] = Some(node_index);
        model_transforms[child] = multiply_matrices(
            model_transforms[node_index],
            source_nodes[child].local_transform,
        );
        validate_affine_matrix(
            model_transforms[child],
            format!("source.nodes[{child}].transform"),
        )?;
        states[child] = VisitState::Visiting;
        ordered_indices.push(child);
        stack.push((child, 0, child_depth));
    }
    Ok(())
}

fn import_referenced_meshes(
    document: &gltf::Document,
    blob: &[u8],
    referenced_meshes: &BTreeSet<usize>,
    allow_animated_features: bool,
) -> Result<(Vec<ImportedModelMesh>, Vec<ImportedMaterial>), ConversionError> {
    let material_count = u32::try_from(document.materials().count()).map_err(|_| {
        ConversionError::one(
            "conversion.resourceLimit",
            "source.materials",
            "material count exceeds u32",
        )
    })?;
    let mut total_vertices = 0usize;
    let mut total_indices = 0usize;
    let mut total_primitives = 0usize;
    let mut primitive_ordinal = 0u32;
    let mut meshes = Vec::new();
    let mut materials = BTreeMap::<u32, Option<String>>::new();

    for mesh in document.meshes() {
        let mesh_index = mesh.index();
        let primitive_count = mesh.primitives().count();
        if !referenced_meshes.contains(&mesh_index) {
            primitive_ordinal = primitive_ordinal
                .checked_add(
                    u32::try_from(primitive_count)
                        .map_err(|_| primitive_limit("document primitive count exceeds u32"))?,
                )
                .ok_or_else(|| primitive_limit("document primitive count overflowed"))?;
            continue;
        }
        if primitive_count == 0 {
            return Err(ConversionError::one(
                "conversion.invalidGeometry",
                format!("source.meshes[{mesh_index}]"),
                "referenced mesh contains no primitives",
            ));
        }
        let source_mesh_name =
            validate_imported_name(mesh.name(), format!("source.meshes[{mesh_index}].name"))?;
        let mut primitives = Vec::with_capacity(primitive_count);
        for primitive in mesh.primitives() {
            total_primitives = total_primitives.saturating_add(1);
            if total_primitives > MAX_IMPORTED_SCENE_PRIMITIVES {
                return Err(primitive_limit(&format!(
                    "selected scene contains more than {MAX_IMPORTED_SCENE_PRIMITIVES} primitives"
                )));
            }
            let imported = import_primitive(
                mesh_index,
                primitive,
                blob,
                material_count,
                primitive_ordinal,
                &mut total_vertices,
                &mut total_indices,
                &mut materials,
                allow_animated_features,
            )?;
            primitive_ordinal = primitive_ordinal
                .checked_add(1)
                .ok_or_else(|| primitive_limit("document primitive count overflowed"))?;
            primitives.push(imported);
        }
        meshes.push(ImportedModelMesh {
            source_mesh_index: u32::try_from(mesh_index)
                .map_err(|_| primitive_limit("source mesh index exceeds u32"))?,
            source_mesh_name,
            primitives,
        });
    }
    if materials.is_empty() || materials.len() > MAX_MATERIAL_MAPPINGS {
        return Err(ConversionError::one(
            "conversion.resourceLimit",
            "source.materials",
            format!("selected material count must be in 1..={MAX_MATERIAL_MAPPINGS}"),
        ));
    }
    Ok((
        meshes,
        materials
            .into_iter()
            .map(
                |(source_material_slot, source_material_name)| ImportedMaterial {
                    source_material_slot,
                    source_material_name,
                },
            )
            .collect(),
    ))
}

#[allow(clippy::too_many_arguments)]
fn import_primitive(
    mesh_index: usize,
    primitive: gltf::Primitive<'_>,
    blob: &[u8],
    material_count: u32,
    primitive_ordinal: u32,
    total_vertices: &mut usize,
    total_indices: &mut usize,
    materials: &mut BTreeMap<u32, Option<String>>,
    allow_animated_features: bool,
) -> Result<ImportedModelPrimitive, ConversionError> {
    let primitive_index = primitive.index();
    let primitive_path = format!("source.meshes[{mesh_index}].primitives[{primitive_index}]");
    if primitive.mode() != Mode::Triangles {
        return Err(ConversionError::one(
            "conversion.unsupportedPrimitive",
            format!("{primitive_path}.mode"),
            "primitive mode must be TRIANGLES",
        ));
    }
    if !allow_animated_features && primitive.morph_targets().next().is_some() {
        return Err(ConversionError::one(
            "conversion.unsupportedFeature",
            format!("{primitive_path}.targets"),
            "morph targets belong to animated import",
        ));
    }
    let reader = primitive.reader(|buffer| match buffer.source() {
        BufferSource::Bin => Some(blob),
        BufferSource::Uri(_) => None,
    });
    let positions = reader
        .read_positions()
        .ok_or_else(|| {
            ConversionError::one(
                "conversion.invalidGeometry",
                format!("{primitive_path}.attributes.POSITION"),
                "primitive is missing POSITION data",
            )
        })?
        .map(|position| position.map(f64::from))
        .collect::<Vec<_>>();
    if positions.is_empty()
        || positions
            .iter()
            .flatten()
            .any(|component| !component.is_finite())
    {
        return Err(ConversionError::one(
            "conversion.invalidGeometry",
            format!("{primitive_path}.attributes.POSITION"),
            "POSITION must contain finite vertices",
        ));
    }
    ensure_total_limit(
        *total_vertices,
        positions.len(),
        MAX_CONVERSION_SOURCE_VERTICES,
        "source.positions",
    )?;
    *total_vertices += positions.len();

    let indices = reader
        .read_indices()
        .ok_or_else(|| {
            ConversionError::one(
                "conversion.unsupportedPrimitive",
                format!("{primitive_path}.indices"),
                "primitive must provide an explicit index accessor",
            )
        })?
        .into_u32()
        .collect::<Vec<_>>();
    if indices.is_empty()
        || !indices.len().is_multiple_of(3)
        || indices
            .iter()
            .any(|index| *index as usize >= positions.len())
    {
        return Err(ConversionError::one(
            "conversion.invalidGeometry",
            format!("{primitive_path}.indices"),
            "indices are not a valid triangle list for this primitive",
        ));
    }
    ensure_total_limit(
        *total_indices,
        indices.len(),
        MAX_CONVERSION_SOURCE_INDICES,
        "source.indices",
    )?;
    *total_indices += indices.len();

    let texture_set_indices = primitive
        .attributes()
        .filter_map(|(semantic, _)| match semantic {
            Semantic::TexCoords(set_index) => Some(set_index),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    if texture_set_indices.len() > MAX_IMPORTED_TEXCOORD_SETS {
        return Err(ConversionError::one(
            "conversion.resourceLimit",
            format!("{primitive_path}.attributes"),
            format!("primitive defines more than {MAX_IMPORTED_TEXCOORD_SETS} TEXCOORD sets"),
        ));
    }
    let mut texture_coordinates = Vec::with_capacity(texture_set_indices.len());
    for source_set_index in texture_set_indices {
        let coordinates = reader
            .read_tex_coords(source_set_index)
            .expect("attribute enumeration found TEXCOORD set")
            .into_f32()
            .map(|coordinate| coordinate.map(f64::from))
            .collect::<Vec<_>>();
        if coordinates.len() != positions.len()
            || coordinates
                .iter()
                .flatten()
                .any(|component| !component.is_finite())
        {
            return Err(ConversionError::one(
                "conversion.invalidGeometry",
                format!("{primitive_path}.attributes.TEXCOORD_{source_set_index}"),
                "TEXCOORD must contain one finite value per POSITION",
            ));
        }
        texture_coordinates.push(ImportedTextureCoordinates {
            source_set_index,
            coordinates,
        });
    }

    let material = primitive.material();
    let source_material_slot = match material.index() {
        Some(index) => u32::try_from(index).map_err(|_| {
            ConversionError::one(
                "conversion.resourceLimit",
                format!("{primitive_path}.material"),
                "source material index exceeds u32",
            )
        })?,
        None => material_count
            .checked_add(primitive_ordinal)
            .ok_or_else(|| {
                ConversionError::one(
                    "conversion.resourceLimit",
                    format!("{primitive_path}.material"),
                    "synthetic default material slot overflowed",
                )
            })?,
    };
    let source_material_name = match material.index() {
        Some(index) => {
            validate_imported_name(material.name(), format!("source.materials[{index}].name"))?
                .or_else(|| Some(format!("gltf-material/{index}")))
        }
        None => None,
    };
    materials
        .entry(source_material_slot)
        .or_insert(source_material_name);

    Ok(ImportedModelPrimitive {
        source_primitive_index: u32::try_from(primitive_index)
            .map_err(|_| primitive_limit("source primitive index exceeds u32"))?,
        source_material_slot,
        positions,
        texture_coordinates,
        indices,
    })
}

fn hierarchy_limit(message: &str) -> ConversionError {
    ConversionError::one(
        "conversion.resourceLimit",
        "source.nodes",
        message.to_owned(),
    )
}

fn primitive_limit(message: &str) -> ConversionError {
    ConversionError::one(
        "conversion.resourceLimit",
        "source.meshes",
        message.to_owned(),
    )
}

fn ambiguous_node(node_index: usize) -> ConversionError {
    ConversionError::one(
        "conversion.ambiguousSceneNode",
        format!("source.nodes[{node_index}]"),
        "selected scene references one node from more than one parent or root",
    )
}
