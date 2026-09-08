//! Spatial presentation sections of an already extracted surface. Triangles are
//! assigned whole by centroid; boundaries may overlap but geometry never changes.
use std::collections::{BTreeMap, HashMap};

use crate::{MeshBoundsDescriptor, MeshGroupDescriptor, MeshPayloadDescriptor, MeshPayloadSource};

/// Partition a valid inline triangle mesh in its local coordinate space.
/// Every source triangle occurs exactly once. Attribute values and winding are
/// copied exactly; vertices shared by sections are duplicated. Actual vertex
/// bounds, rather than grid-cell bounds, must be used for frustum culling.
/// This is presentation partitioning, not independently watertight solids.
pub fn partition_mesh_spatially(
    mesh: &MeshPayloadDescriptor,
    origin: [f32; 3],
    cell_size: [f32; 3],
) -> Result<Vec<MeshPayloadDescriptor>, &'static str> {
    if origin.iter().any(|v| !v.is_finite())
        || cell_size.iter().any(|v| !v.is_finite() || *v <= 0.0)
    {
        return Err("spatial mesh partition requires finite origin and positive finite cell sizes");
    }
    let MeshPayloadSource::Inline {
        positions,
        normals,
        uvs,
        colors,
        indices,
    } = &mesh.source
    else {
        return Err("spatial mesh partition requires an inline mesh");
    };
    let mut bins: BTreeMap<[i64; 3], Vec<(u16, [u32; 3])>> = BTreeMap::new();
    // Groups tile the admitted index stream. Retain their order in each bin so
    // material boundaries and per-group drawing semantics are preserved.
    for group in &mesh.groups {
        let start = group.start as usize;
        for triangle in indices[start..start + group.count as usize]
            .as_chunks::<3>()
            .0
        {
            let mut key = [0; 3];
            for axis in 0..3 {
                let centroid = triangle
                    .iter()
                    .map(|&i| f64::from(positions[i as usize * 3 + axis]))
                    .sum::<f64>()
                    / 3.0;
                let cell =
                    ((centroid - f64::from(origin[axis])) / f64::from(cell_size[axis])).floor();
                // Do not saturate coordinates: different bins must not alias.
                if !cell.is_finite() || cell < i64::MIN as f64 || cell >= -(i64::MIN as f64) {
                    return Err("spatial mesh partition coordinate exceeds i64 representation");
                }
                key[axis] = cell as i64;
            }
            bins.entry(key)
                .or_default()
                .push((group.material_slot, [triangle[0], triangle[1], triangle[2]]));
        }
    }
    let mut parts = Vec::with_capacity(bins.len());
    for triangles in bins.into_values() {
        let mut remap = HashMap::new();
        let mut out_positions = Vec::new();
        let mut out_normals = Vec::new();
        let mut out_uvs = uvs.as_ref().map(|_| Vec::new());
        let mut out_colors = colors.as_ref().map(|_| Vec::new());
        let mut out_indices = Vec::with_capacity(triangles.len() * 3);
        let mut groups: Vec<MeshGroupDescriptor> = Vec::new();
        let mut bounds = MeshBoundsDescriptor {
            min: [f32::INFINITY; 3],
            max: [f32::NEG_INFINITY; 3],
        };
        for (slot, triangle) in triangles {
            if groups.last().is_none_or(|g| g.material_slot != slot) {
                groups.push(MeshGroupDescriptor {
                    material_slot: slot,
                    start: out_indices.len() as u32,
                    count: 0,
                });
            }
            groups.last_mut().unwrap().count += 3;
            for old in triangle {
                let next = (out_positions.len() / 3) as u32;
                let index = *remap.entry(old).or_insert_with(|| {
                    let old = old as usize;
                    let point = &positions[old * 3..old * 3 + 3];
                    out_positions.extend_from_slice(point);
                    out_normals.extend_from_slice(&normals[old * 3..old * 3 + 3]);
                    if let (Some(src), Some(dst)) = (uvs, &mut out_uvs) {
                        dst.extend_from_slice(&src[old * 2..old * 2 + 2]);
                    }
                    if let (Some(src), Some(dst)) = (colors, &mut out_colors) {
                        dst.extend_from_slice(&src[old * 4..old * 4 + 4]);
                    }
                    for (axis, &value) in point.iter().enumerate() {
                        bounds.min[axis] = bounds.min[axis].min(value);
                        bounds.max[axis] = bounds.max[axis].max(value);
                    }
                    next
                });
                out_indices.push(index);
            }
        }
        let mut layout = mesh.layout.clone();
        layout.vertex_count = (out_positions.len() / 3) as u32;
        layout.index_count = out_indices.len() as u32;
        parts.push(MeshPayloadDescriptor {
            layout,
            groups,
            bounds,
            provenance: mesh.provenance,
            source: MeshPayloadSource::Inline {
                positions: out_positions,
                normals: out_normals,
                uvs: out_uvs,
                colors: out_colors,
                indices: out_indices,
            },
        });
    }
    Ok(parts)
}

#[cfg(test)]
#[path = "mesh_partition_tests.rs"]
mod tests;
