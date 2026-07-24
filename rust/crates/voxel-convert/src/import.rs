use std::collections::BTreeMap;

use gltf::{buffer::Source as BufferSource, mesh::Mode};
use voxel_asset::{MAX_CONVERSION_SOURCE_INDICES, MAX_CONVERSION_SOURCE_VERTICES};

use crate::ConversionError;

#[derive(Debug, Clone, PartialEq)]
pub struct ImportedStaticMesh {
    pub positions: Vec<[f64; 3]>,
    pub triangles: Vec<ImportedTriangle>,
    pub materials: Vec<ImportedMaterial>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportedTriangle {
    pub indices: [u32; 3],
    pub source_material_slot: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedMaterial {
    pub source_material_slot: u32,
    pub source_material_name: Option<String>,
}

pub fn import_static_glb(source: &[u8]) -> Result<ImportedStaticMesh, ConversionError> {
    let parsed = gltf::Gltf::from_slice(source).map_err(|error| {
        ConversionError::one(
            "conversion.invalidSource",
            "source",
            format!("invalid GLB 2.0 source: {error}"),
        )
    })?;
    let blob = parsed.blob.as_deref().ok_or_else(|| {
        ConversionError::one(
            "conversion.unsupportedFeature",
            "source",
            "GLB source must contain one embedded BIN chunk",
        )
    })?;
    if parsed.document.animations().next().is_some() || parsed.document.skins().next().is_some() {
        return Err(ConversionError::one(
            "conversion.unsupportedFeature",
            "source",
            "animated or skinned sources are outside the static conversion boundary",
        ));
    }
    for buffer in parsed.document.buffers() {
        if !matches!(buffer.source(), BufferSource::Bin) {
            return Err(ConversionError::one(
                "conversion.unsupportedFeature",
                "source",
                "GLB source may not reference external buffers",
            ));
        }
    }

    let mut meshes = parsed.document.meshes();
    let mesh = meshes.next().ok_or_else(|| {
        ConversionError::one(
            "conversion.invalidGeometry",
            "source.meshes",
            "GLB contains no mesh",
        )
    })?;
    if meshes.next().is_some() {
        return Err(ConversionError::one(
            "conversion.unsupportedFeature",
            "source.meshes",
            "conversion accepts exactly one static mesh",
        ));
    }

    let material_count = parsed.document.materials().count() as u32;
    let mut positions = Vec::new();
    let mut triangles = Vec::new();
    let mut materials = BTreeMap::<u32, Option<String>>::new();
    for primitive in mesh.primitives() {
        if primitive.mode() != Mode::Triangles || primitive.morph_targets().next().is_some() {
            return Err(ConversionError::one(
                "conversion.unsupportedPrimitive",
                format!("source.meshes[0].primitives[{}]", primitive.index()),
                "primitives must be non-morphing indexed triangle lists",
            ));
        }
        let reader = primitive.reader(|buffer| match buffer.source() {
            BufferSource::Bin => Some(blob),
            BufferSource::Uri(_) => None,
        });
        let source_positions = reader.read_positions().ok_or_else(|| {
            ConversionError::one(
                "conversion.invalidGeometry",
                format!(
                    "source.meshes[0].primitives[{}].attributes.POSITION",
                    primitive.index()
                ),
                "primitive is missing POSITION data",
            )
        })?;
        ensure_total_limit(
            positions.len(),
            source_positions.len(),
            MAX_CONVERSION_SOURCE_VERTICES,
            "source.positions",
        )?;
        let vertex_offset = u32::try_from(positions.len()).map_err(|_| {
            ConversionError::one(
                "conversion.resourceLimit",
                "source.positions",
                "vertex offset exceeds u32",
            )
        })?;
        let primitive_positions = source_positions
            .map(|position| position.map(f64::from))
            .collect::<Vec<_>>();
        if primitive_positions
            .iter()
            .flatten()
            .any(|component| !component.is_finite())
        {
            return Err(ConversionError::one(
                "conversion.invalidGeometry",
                format!(
                    "source.meshes[0].primitives[{}].attributes.POSITION",
                    primitive.index()
                ),
                "POSITION contains a non-finite component",
            ));
        }

        let index_count = primitive
            .indices()
            .ok_or_else(|| {
                ConversionError::one(
                    "conversion.unsupportedPrimitive",
                    format!("source.meshes[0].primitives[{}].indices", primitive.index()),
                    "primitive must provide an explicit index accessor",
                )
            })?
            .count();
        ensure_total_limit(
            triangles.len().saturating_mul(3),
            index_count,
            MAX_CONVERSION_SOURCE_INDICES,
            "source.indices",
        )?;
        let source_indices = reader.read_indices().expect("validated indexed primitive");
        let indices = source_indices.into_u32().collect::<Vec<_>>();
        if !indices.len().is_multiple_of(3)
            || indices
                .iter()
                .any(|index| *index as usize >= primitive_positions.len())
        {
            return Err(ConversionError::one(
                "conversion.invalidGeometry",
                format!("source.meshes[0].primitives[{}].indices", primitive.index()),
                "indices are not a valid triangle list for this primitive",
            ));
        }
        let material = primitive.material();
        let material_slot = material
            .index()
            .map(|index| index as u32)
            .unwrap_or(material_count + primitive.index() as u32);
        let material_name = material.name().map(str::to_string).or_else(|| {
            material
                .index()
                .map(|index| format!("gltf-material/{index}"))
        });
        materials.entry(material_slot).or_insert(material_name);

        positions.extend(primitive_positions);
        triangles.extend(indices.chunks_exact(3).map(|triangle| ImportedTriangle {
            indices: [
                triangle[0] + vertex_offset,
                triangle[1] + vertex_offset,
                triangle[2] + vertex_offset,
            ],
            source_material_slot: material_slot,
        }));
    }
    if positions.is_empty() || triangles.is_empty() || materials.is_empty() {
        return Err(ConversionError::one(
            "conversion.invalidGeometry",
            "source.meshes[0]",
            "mesh produced no indexed triangle geometry",
        ));
    }
    validate_triangles(&positions, &triangles)?;

    Ok(ImportedStaticMesh {
        positions,
        triangles,
        materials: materials
            .into_iter()
            .map(
                |(source_material_slot, source_material_name)| ImportedMaterial {
                    source_material_slot,
                    source_material_name,
                },
            )
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
                "triangle is degenerate",
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

fn subtract(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

fn dot(left: [f64; 3], right: [f64; 3]) -> f64 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

fn ensure_total_limit(
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
