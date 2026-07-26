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
    let mut positions = Vec::new();
    let texture_set_indices = scene
        .meshes
        .iter()
        .flat_map(|mesh| &mesh.primitives)
        .flat_map(|primitive| &primitive.texture_coordinates)
        .map(|texture_coordinates| texture_coordinates.source_set_index)
        .collect::<BTreeSet<_>>();
    let mut texture_coordinates = texture_set_indices
        .into_iter()
        .map(|source_set_index| (source_set_index, Vec::new()))
        .collect::<BTreeMap<u32, Vec<Option<[f64; 2]>>>>();
    let mut triangles = Vec::new();
    let mut primitive_groups = Vec::new();
    let mut used_materials = BTreeSet::new();
    let mut mesh_instance_count = 0usize;

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
            ensure_total_limit(
                positions.len(),
                primitive.positions.len(),
                MAX_CONVERSION_SOURCE_VERTICES,
                "source.positions",
            )?;
            ensure_total_limit(
                triangles.len().saturating_mul(3),
                primitive.indices.len(),
                MAX_CONVERSION_SOURCE_INDICES,
                "source.indices",
            )?;
            let vertex_offset = u32::try_from(positions.len()).map_err(|_| {
                ConversionError::one(
                    "conversion.resourceLimit",
                    "source.positions",
                    "expanded vertex offset exceeds u32",
                )
            })?;
            for position in &primitive.positions {
                positions.push(transform_point(node.model_transform, *position).ok_or_else(
                    || {
                        ConversionError::one(
                            "conversion.invalidTransform",
                            format!("source.nodes[{}].transform", node.source_node_index),
                            "composed node transform produced a non-finite position",
                        )
                    },
                )?);
            }
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
            triangles.extend(
                primitive
                    .indices
                    .chunks_exact(3)
                    .map(|triangle| ImportedTriangle {
                        indices: [
                            triangle[0] + vertex_offset,
                            triangle[1] + vertex_offset,
                            triangle[2] + vertex_offset,
                        ],
                        source_material_slot: primitive.source_material_slot,
                    }),
            );
            primitive_groups.push(ImportedPrimitiveGroup {
                source_node_index: node.source_node_index,
                source_mesh_index: mesh.source_mesh_index,
                source_primitive_index: primitive.source_primitive_index,
                source_material_slot: primitive.source_material_slot,
                triangle_start,
                triangle_count: u32::try_from(primitive.indices.len() / 3).map_err(|_| {
                    ConversionError::one(
                        "conversion.resourceLimit",
                        "source.groups",
                        "primitive triangle count exceeds u32",
                    )
                })?,
            });
            used_materials.insert(primitive.source_material_slot);
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

fn validate_triangles(
    positions: &[[f64; 3]],
    triangles: &[ImportedTriangle],
) -> Result<(), ConversionError> {
    for (index, triangle) in triangles.iter().enumerate() {
        let [a, b, c] = triangle.indices;
        if a == b || b == c || c == a || area_squared(positions, triangle) <= f64::EPSILON {
            return Err(ConversionError::one(
                "conversion.invalidGeometry",
                format!("source.triangles[{index}]"),
                "triangle is degenerate after scene transform composition",
            ));
        }
    }
    Ok(())
}

pub(crate) fn area_squared(positions: &[[f64; 3]], triangle: &ImportedTriangle) -> f64 {
    let [a, b, c] = triangle.indices.map(|index| positions[index as usize]);
    let ab = subtract(b, a);
    let ac = subtract(c, a);
    let cross = [
        ab[1] * ac[2] - ab[2] * ac[1],
        ab[2] * ac[0] - ab[0] * ac[2],
        ab[0] * ac[1] - ab[1] * ac[0],
    ];
    dot(cross, cross)
}

pub(super) fn identity_matrix() -> [f64; 16] {
    [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ]
}

pub(super) fn matrix_from_gltf(matrix: [[f32; 4]; 4]) -> [f64; 16] {
    std::array::from_fn(|index| f64::from(matrix[index / 4][index % 4]))
}

pub(super) fn multiply_matrices(left: [f64; 16], right: [f64; 16]) -> [f64; 16] {
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

pub(super) fn validate_affine_matrix(
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

fn transform_point(matrix: [f64; 16], point: [f64; 3]) -> Option<[f64; 3]> {
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

pub(super) fn validate_imported_name(
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
