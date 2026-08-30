use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest, Sha256};
use voxel_asset::{MAX_CONVERSION_SOURCE_INDICES, MAX_CONVERSION_SOURCE_VERTICES};

use crate::ConversionError;

mod gltf_scene;

pub const MAX_IMPORTED_SCENE_NODES: usize = 4_096;
pub const MAX_IMPORTED_SCENE_DEPTH: usize = 256;
pub const MAX_IMPORTED_SCENE_EDGES: usize = 16_384;
pub const MAX_IMPORTED_SCENE_MESHES: usize = 4_096;
pub const MAX_IMPORTED_SCENE_PRIMITIVES: usize = 8_192;
pub const MAX_IMPORTED_SCENE_MESH_INSTANCES: usize = 4_096;
pub const MAX_IMPORTED_TEXCOORD_SETS: usize = 8;
pub const MAX_IMPORTED_NAME_BYTES: usize = 4_096;

#[derive(Debug, Clone, PartialEq)]
pub struct ImportedStaticMesh {
    pub positions: Vec<[f64; 3]>,
    /// Every imported `TEXCOORD_n` set in ascending source-set order.
    /// Values align one-for-one with `positions`; `None` means that the owning
    /// source primitive did not define that set.
    pub texture_coordinates: Vec<ImportedStaticTextureCoordinates>,
    pub triangles: Vec<ImportedTriangle>,
    pub primitive_groups: Vec<ImportedPrimitiveGroup>,
    pub materials: Vec<ImportedMaterial>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportedTriangle {
    pub indices: [u32; 3],
    pub source_material_slot: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportedPrimitiveGroup {
    pub source_node_index: u32,
    pub source_mesh_index: u32,
    pub source_primitive_index: u32,
    pub source_material_slot: u32,
    pub triangle_start: u32,
    pub triangle_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedMaterial {
    pub source_material_slot: u32,
    pub source_material_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportedStaticTextureCoordinates {
    pub source_set_index: u32,
    pub coordinates: Vec<Option<[f64; 2]>>,
}

/// One selected GLB scene in source-local form.
///
/// Mesh geometry is retained once in mesh-local coordinates. Nodes retain the
/// deterministic hierarchy and composed model transform separately. Static
/// conversion flattens this family explicitly; animation sampling can deform
/// the same indexed primitive family without defining another mesh authority.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportedModelScene {
    pub source_scene_index: u32,
    pub source_scene_name: Option<String>,
    pub nodes: Vec<ImportedModelNode>,
    pub meshes: Vec<ImportedModelMesh>,
    pub materials: Vec<ImportedMaterial>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportedModelNode {
    pub source_node_index: u32,
    pub source_node_name: Option<String>,
    pub parent_node_index: Option<u32>,
    pub child_node_indices: Vec<u32>,
    pub source_mesh_index: Option<u32>,
    pub local_transform: [f64; 16],
    pub model_transform: [f64; 16],
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportedModelMesh {
    pub source_mesh_index: u32,
    pub source_mesh_name: Option<String>,
    pub primitives: Vec<ImportedModelPrimitive>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportedModelPrimitive {
    pub source_primitive_index: u32,
    pub source_material_slot: u32,
    pub positions: Vec<[f64; 3]>,
    pub texture_coordinates: Vec<ImportedTextureCoordinates>,
    pub indices: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportedTextureCoordinates {
    pub source_set_index: u32,
    pub coordinates: Vec<[f64; 2]>,
}

pub fn import_static_glb_scene(source: &[u8]) -> Result<ImportedModelScene, ConversionError> {
    gltf_scene::import_static_glb_scene(source)
}

pub fn import_static_glb(source: &[u8]) -> Result<ImportedStaticMesh, ConversionError> {
    flatten_static_scene(&import_static_glb_scene(source)?)
}

pub(crate) fn import_animated_glb_scene(
    source: &[u8],
) -> Result<(gltf::Gltf, ImportedModelScene), ConversionError> {
    gltf_scene::parse_animated_glb(source)
}

/// Hash one flattened UV attribute exactly as the converter samples it.
///
/// The identity includes the selected/instanced vertex order and explicit
/// missing values, so a group/node selection cannot silently reuse the hash of
/// another geometry view.
pub fn texture_coordinate_source_hash(
    mesh: &ImportedStaticMesh,
    source_set_index: u32,
) -> Option<String> {
    let texture_coordinates = mesh
        .texture_coordinates
        .iter()
        .find(|candidate| candidate.source_set_index == source_set_index)?;
    let mut digest = Sha256::new();
    digest.update(b"rusty-engine.texture-coordinates.v1\0");
    digest.update(source_set_index.to_le_bytes());
    digest.update((texture_coordinates.coordinates.len() as u64).to_le_bytes());
    for coordinate in &texture_coordinates.coordinates {
        match coordinate {
            Some([u, v]) => {
                digest.update([1]);
                digest.update(u.to_bits().to_le_bytes());
                digest.update(v.to_bits().to_le_bytes());
            }
            None => digest.update([0]),
        }
    }
    Some(format!("sha256:{:x}", digest.finalize()))
}

pub fn flatten_static_scene(
    scene: &ImportedModelScene,
) -> Result<ImportedStaticMesh, ConversionError> {
    flatten_model_scene(
        scene,
        DegenerateTrianglePolicy::Reject,
        |node, _mesh, primitive| {
            primitive
                .positions
                .iter()
                .map(|position| {
                    transform_point(node.model_transform, *position).ok_or_else(|| {
                        ConversionError::one(
                            "conversion.invalidTransform",
                            format!("source.nodes[{}].transform", node.source_node_index),
                            "composed node transform produced a non-finite position",
                        )
                    })
                })
                .collect()
        },
    )
}

#[derive(Clone, Copy)]
pub(crate) enum DegenerateTrianglePolicy {
    Reject,
    DropForVisualMetadata,
}

pub(crate) fn flatten_model_scene(
    scene: &ImportedModelScene,
    degenerate_policy: DegenerateTrianglePolicy,
    mut positions_for_primitive: impl FnMut(
        &ImportedModelNode,
        &ImportedModelMesh,
        &ImportedModelPrimitive,
    ) -> Result<Vec<[f64; 3]>, ConversionError>,
) -> Result<ImportedStaticMesh, ConversionError> {
    let mut positions = Vec::new();
    let texture_set_indices = collect_texture_set_indices(scene)?;
    let mut texture_coordinates = texture_set_indices
        .into_iter()
        .map(|source_set_index| (source_set_index, Vec::new()))
        .collect::<BTreeMap<u32, Vec<Option<[f64; 2]>>>>();
    let mut triangles = Vec::new();
    let mut primitive_groups = Vec::new();
    let mut used_materials = BTreeSet::new();
    let mut mesh_instance_count = 0usize;
    let mut source_index_count = 0usize;

    for node in &scene.nodes {
        let Some(mesh_index) = node.source_mesh_index else {
            continue;
        };
        mesh_instance_count = mesh_instance_count.saturating_add(1);
        if mesh_instance_count > MAX_IMPORTED_SCENE_MESH_INSTANCES {
            return Err(ConversionError::one(
                "conversion.resourceLimit",
                "source.scene.meshInstances",
                format!(
                    "selected scene contains more than {MAX_IMPORTED_SCENE_MESH_INSTANCES} mesh instances"
                ),
            ));
        }
        let mesh = scene
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
            let instance_positions = positions_for_primitive(node, mesh, primitive)?;
            if instance_positions.len() != primitive.positions.len() {
                return Err(ConversionError::one(
                    "conversion.invalidDeformation",
                    format!(
                        "source.nodes[{}].meshes[{}].primitives[{}]",
                        node.source_node_index,
                        mesh.source_mesh_index,
                        primitive.source_primitive_index
                    ),
                    "deformed POSITION count does not match the imported primitive",
                ));
            }
            ensure_total_limit(
                positions.len(),
                instance_positions.len(),
                MAX_CONVERSION_SOURCE_VERTICES,
                "source.positions",
            )?;
            ensure_total_limit(
                source_index_count,
                primitive.indices.len(),
                MAX_CONVERSION_SOURCE_INDICES,
                "source.indices",
            )?;
            source_index_count += primitive.indices.len();
            let vertex_offset = u32::try_from(positions.len()).map_err(|_| {
                ConversionError::one(
                    "conversion.resourceLimit",
                    "source.positions",
                    "expanded vertex offset exceeds u32",
                )
            })?;
            positions.extend(instance_positions);
            for (source_set_index, flattened) in &mut texture_coordinates {
                match primitive
                    .texture_coordinates
                    .iter()
                    .find(|candidate| candidate.source_set_index == *source_set_index)
                {
                    Some(source) => flattened.extend(source.coordinates.iter().copied().map(Some)),
                    None => flattened.extend(std::iter::repeat_n(None, primitive.positions.len())),
                }
            }
            let triangle_start = u32::try_from(triangles.len()).map_err(|_| {
                ConversionError::one(
                    "conversion.resourceLimit",
                    "source.groups",
                    "primitive triangle start exceeds u32",
                )
            })?;
            for indices in primitive.indices.as_chunks::<3>().0 {
                let triangle = ImportedTriangle {
                    indices: [
                        indices[0] + vertex_offset,
                        indices[1] + vertex_offset,
                        indices[2] + vertex_offset,
                    ],
                    source_material_slot: primitive.source_material_slot,
                };
                let [a, b, c] = triangle.indices;
                if matches!(
                    degenerate_policy,
                    DegenerateTrianglePolicy::DropForVisualMetadata
                ) && (a == b
                    || b == c
                    || c == a
                    || triangle_is_degenerate(&positions, &triangle))
                {
                    continue;
                }
                triangles.push(triangle);
            }
            let triangle_count =
                u32::try_from(triangles.len() - triangle_start as usize).map_err(|_| {
                    ConversionError::one(
                        "conversion.resourceLimit",
                        "source.groups",
                        "primitive triangle count exceeds u32",
                    )
                })?;
            if triangle_count > 0 {
                primitive_groups.push(ImportedPrimitiveGroup {
                    source_node_index: node.source_node_index,
                    source_mesh_index: mesh.source_mesh_index,
                    source_primitive_index: primitive.source_primitive_index,
                    source_material_slot: primitive.source_material_slot,
                    triangle_start,
                    triangle_count,
                });
                used_materials.insert(primitive.source_material_slot);
            }
        }
    }

    if positions.is_empty() || triangles.is_empty() || used_materials.is_empty() {
        return Err(ConversionError::one(
            "conversion.invalidGeometry",
            "source.scene",
            "default scene produced no indexed triangle geometry",
        ));
    }
    if texture_coordinates
        .values()
        .any(|coordinates| coordinates.len() != positions.len())
    {
        return Err(ConversionError::one(
            "conversion.invalidGeometry",
            "source.textureCoordinates",
            "preserved TEXCOORD values do not align with imported positions",
        ));
    }
    if matches!(
        degenerate_policy,
        DegenerateTrianglePolicy::DropForVisualMetadata
    ) {
        compact_vertices_to_retained_triangles(
            &mut positions,
            &mut texture_coordinates,
            &mut triangles,
        )?;
    }
    validate_triangles(&positions, &triangles)?;

    Ok(ImportedStaticMesh {
        positions,
        texture_coordinates: texture_coordinates
            .into_iter()
            .map(
                |(source_set_index, coordinates)| ImportedStaticTextureCoordinates {
                    source_set_index,
                    coordinates,
                },
            )
            .collect(),
        triangles,
        primitive_groups,
        materials: scene
            .materials
            .iter()
            .filter(|material| used_materials.contains(&material.source_material_slot))
            .cloned()
            .collect(),
    })
}

fn compact_vertices_to_retained_triangles(
    positions: &mut Vec<[f64; 3]>,
    texture_coordinates: &mut BTreeMap<u32, Vec<Option<[f64; 2]>>>,
    triangles: &mut [ImportedTriangle],
) -> Result<(), ConversionError> {
    let mut referenced = vec![false; positions.len()];
    for triangle in triangles.iter() {
        for index in triangle.indices {
            referenced[index as usize] = true;
        }
    }

    let mut remap = vec![u32::MAX; positions.len()];
    let mut compacted_positions = Vec::with_capacity(positions.len());
    for (old_index, position) in positions.iter().copied().enumerate() {
        if !referenced[old_index] {
            continue;
        }
        remap[old_index] = u32::try_from(compacted_positions.len()).map_err(|_| {
            ConversionError::one(
                "conversion.resourceLimit",
                "source.positions",
                "compacted visual metadata vertex index exceeds u32",
            )
        })?;
        compacted_positions.push(position);
    }
    for triangle in triangles {
        triangle.indices = triangle.indices.map(|index| remap[index as usize]);
    }
    *positions = compacted_positions;

    for coordinates in texture_coordinates.values_mut() {
        let original = std::mem::take(coordinates);
        *coordinates = original
            .into_iter()
            .enumerate()
            .filter_map(|(index, coordinate)| referenced[index].then_some(coordinate))
            .collect();
    }
    Ok(())
}

fn collect_texture_set_indices(
    scene: &ImportedModelScene,
) -> Result<BTreeSet<u32>, ConversionError> {
    let mut source_set_indices = BTreeSet::new();
    for mesh in &scene.meshes {
        for primitive in &mesh.primitives {
            for texture_coordinates in &primitive.texture_coordinates {
                source_set_indices.insert(texture_coordinates.source_set_index);
                if source_set_indices.len() > MAX_IMPORTED_TEXCOORD_SETS {
                    return Err(ConversionError::one(
                        "conversion.resourceLimit",
                        format!(
                            "source.meshes[{}].primitives[{}].attributes.TEXCOORD_{}",
                            mesh.source_mesh_index,
                            primitive.source_primitive_index,
                            texture_coordinates.source_set_index
                        ),
                        format!(
                            "selected model defines more than {MAX_IMPORTED_TEXCOORD_SETS} distinct TEXCOORD sets"
                        ),
                    ));
                }
            }
        }
    }
    Ok(source_set_indices)
}

fn validate_triangles(
    positions: &[[f64; 3]],
    triangles: &[ImportedTriangle],
) -> Result<(), ConversionError> {
    for (index, triangle) in triangles.iter().enumerate() {
        let [a, b, c] = triangle.indices;
        if a == b || b == c || c == a || triangle_is_degenerate(positions, triangle) {
            return Err(ConversionError::one(
                "conversion.invalidGeometry",
                format!("source.triangles[{index}]"),
                "triangle is degenerate after scene transform composition",
            ));
        }
    }
    Ok(())
}

fn triangle_is_degenerate(positions: &[[f64; 3]], triangle: &ImportedTriangle) -> bool {
    let [a, b, c] = triangle.indices.map(|index| positions[index as usize]);
    let ab = subtract(b, a);
    let ac = subtract(c, a);
    let bc = subtract(c, b);
    let component_scale = ab
        .iter()
        .chain(ac.iter())
        .map(|component| component.abs())
        .fold(0.0_f64, f64::max);
    if component_scale == 0.0 {
        return true;
    }

    // Normalize before comparing squared area with squared edge length.  This
    // keeps the decision invariant under uniform source-unit changes and also
    // avoids overflow/underflow for otherwise finite transformed positions.
    let ab = ab.map(|component| component / component_scale);
    let ac = ac.map(|component| component / component_scale);
    let bc = bc.map(|component| component / component_scale);
    let cross = [
        ab[1] * ac[2] - ab[2] * ac[1],
        ab[2] * ac[0] - ab[0] * ac[2],
        ab[0] * ac[1] - ab[1] * ac[0],
    ];
    let normalized_area_squared = dot(cross, cross);
    let longest_edge_squared = dot(ab, ab).max(dot(ac, ac)).max(dot(bc, bc));
    normalized_area_squared <= f64::EPSILON * longest_edge_squared * longest_edge_squared
}

pub(crate) fn identity_matrix() -> [f64; 16] {
    [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ]
}

pub(crate) fn matrix_from_gltf(matrix: [[f32; 4]; 4]) -> [f64; 16] {
    std::array::from_fn(|index| f64::from(matrix[index / 4][index % 4]))
}

pub(crate) fn multiply_matrices(left: [f64; 16], right: [f64; 16]) -> [f64; 16] {
    let mut product = [0.0; 16];
    for column in 0..4 {
        for row in 0..4 {
            product[column * 4 + row] = (0..4)
                .map(|inner| left[inner * 4 + row] * right[column * 4 + inner])
                .sum();
        }
    }
    product
}

pub(crate) fn validate_affine_matrix(
    matrix: [f64; 16],
    path: impl Into<String>,
) -> Result<(), ConversionError> {
    if matrix.iter().any(|value| !value.is_finite())
        || matrix[3].abs() > f64::EPSILON
        || matrix[7].abs() > f64::EPSILON
        || matrix[11].abs() > f64::EPSILON
        || (matrix[15] - 1.0).abs() > f64::EPSILON
    {
        return Err(ConversionError::one(
            "conversion.invalidTransform",
            path,
            "node transform must be a finite affine column-major matrix",
        ));
    }
    Ok(())
}

pub(crate) fn transform_point(matrix: [f64; 16], point: [f64; 3]) -> Option<[f64; 3]> {
    let [x, y, z] = point;
    let transformed = [
        matrix[0] * x + matrix[4] * y + matrix[8] * z + matrix[12],
        matrix[1] * x + matrix[5] * y + matrix[9] * z + matrix[13],
        matrix[2] * x + matrix[6] * y + matrix[10] * z + matrix[14],
    ];
    transformed
        .iter()
        .all(|component| component.is_finite())
        .then_some(transformed)
}

fn subtract(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

fn dot(left: [f64; 3], right: [f64; 3]) -> f64 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

pub(crate) fn validate_imported_name(
    value: Option<&str>,
    path: impl Into<String>,
) -> Result<Option<String>, ConversionError> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.trim().is_empty() || value.len() > MAX_IMPORTED_NAME_BYTES {
        return Err(ConversionError::one(
            "conversion.invalidString",
            path,
            format!("name must contain 1..={MAX_IMPORTED_NAME_BYTES} UTF-8 bytes when present"),
        ));
    }
    Ok(Some(value.to_owned()))
}

pub(super) fn ensure_total_limit(
    current: usize,
    incoming: usize,
    limit: usize,
    path: &str,
) -> Result<(), ConversionError> {
    let total = current.checked_add(incoming).ok_or_else(|| {
        ConversionError::one(
            "conversion.resourceLimit",
            path,
            "cumulative source count overflowed",
        )
    })?;
    if total > limit {
        return Err(ConversionError::one(
            "conversion.resourceLimit",
            path,
            format!("source count {total} exceeds limit {limit}"),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::source::mesh_metadata;

    use super::{
        flatten_model_scene, identity_matrix, triangle_is_degenerate, validate_triangles,
        DegenerateTrianglePolicy, ImportedMaterial, ImportedModelMesh, ImportedModelNode,
        ImportedModelPrimitive, ImportedModelScene, ImportedTextureCoordinates, ImportedTriangle,
    };

    const TRIANGLE: ImportedTriangle = ImportedTriangle {
        indices: [0, 1, 2],
        source_material_slot: 0,
    };

    fn scene_with_one_valid_and_one_degenerate_triangle() -> ImportedModelScene {
        ImportedModelScene {
            source_scene_index: 0,
            source_scene_name: None,
            nodes: vec![ImportedModelNode {
                source_node_index: 0,
                source_node_name: None,
                parent_node_index: None,
                child_node_indices: Vec::new(),
                source_mesh_index: Some(0),
                local_transform: identity_matrix(),
                model_transform: identity_matrix(),
            }],
            meshes: vec![ImportedModelMesh {
                source_mesh_index: 0,
                source_mesh_name: None,
                primitives: vec![ImportedModelPrimitive {
                    source_primitive_index: 0,
                    source_material_slot: 0,
                    positions: vec![
                        [0.0, 0.0, 0.0],
                        [1.0, 0.0, 0.0],
                        [0.0, 1.0, 0.0],
                        [1_000_000.0, 0.0, 0.0],
                    ],
                    texture_coordinates: vec![ImportedTextureCoordinates {
                        source_set_index: 0,
                        coordinates: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [99.0, 99.0]],
                    }],
                    indices: vec![0, 1, 2, 0, 1, 3],
                }],
            }],
            materials: vec![ImportedMaterial {
                source_material_slot: 0,
                source_material_name: None,
            }],
        }
    }

    #[test]
    fn strict_flattening_still_rejects_degenerate_source_faces() {
        let scene = scene_with_one_valid_and_one_degenerate_triangle();
        let error = flatten_model_scene(
            &scene,
            DegenerateTrianglePolicy::Reject,
            |_node, _mesh, primitive| Ok(primitive.positions.clone()),
        )
        .unwrap_err();

        assert!(error
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == "conversion.invalidGeometry"));
    }

    #[test]
    fn visual_metadata_flattening_drops_only_degenerate_source_faces() {
        let scene = scene_with_one_valid_and_one_degenerate_triangle();
        let mesh = flatten_model_scene(
            &scene,
            DegenerateTrianglePolicy::DropForVisualMetadata,
            |_node, _mesh, primitive| Ok(primitive.positions.clone()),
        )
        .unwrap();

        assert_eq!(mesh.triangles.len(), 1);
        assert_eq!(mesh.triangles[0].indices, [0, 1, 2]);
        assert_eq!(mesh.positions.len(), 3);
        assert_eq!(
            mesh.positions,
            vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]
        );
        assert_eq!(mesh.primitive_groups.len(), 1);
        assert_eq!(mesh.primitive_groups[0].triangle_count, 1);
        assert_eq!(mesh.materials.len(), 1);
        assert_eq!(
            mesh.texture_coordinates[0].coordinates,
            vec![Some([0.0, 0.0]), Some([1.0, 0.0]), Some([0.0, 1.0])]
        );
        let metadata = mesh_metadata(&scene, &mesh).unwrap();
        assert_eq!(metadata.source_bounds.min, [0.0, 0.0, 0.0]);
        assert_eq!(metadata.source_bounds.max, [1.0, 1.0, 0.0]);
    }

    #[test]
    fn legitimate_triangle_admission_is_uniform_scale_invariant() {
        let source = [[0.0, 0.0, 0.0], [3.0e-4, 0.0, 0.0], [0.0, 4.75e-5, 0.0]];

        for scale in [1.0 / 128.0, 1.0e-2, 1.0, 1.0e2] {
            let positions = source.map(|position| position.map(|component| component * scale));
            assert!(
                !triangle_is_degenerate(&positions, &TRIANGLE),
                "legitimate triangle rejected at scale {scale}"
            );
            validate_triangles(&positions, &[TRIANGLE]).unwrap();
        }
    }

    #[test]
    fn collinear_triangle_rejection_is_uniform_scale_invariant() {
        let source = [[0.0, 0.0, 0.0], [3.0e-4, 0.0, 0.0], [6.0e-4, 0.0, 0.0]];

        for scale in [1.0e-2, 1.0, 1.0e2] {
            let positions = source.map(|position| position.map(|component| component * scale));
            assert!(triangle_is_degenerate(&positions, &TRIANGLE));
            assert!(validate_triangles(&positions, &[TRIANGLE]).is_err());
        }
    }
}
