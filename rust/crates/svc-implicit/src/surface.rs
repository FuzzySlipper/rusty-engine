//! Renderer-neutral attributes assembled from extracted implicit geometry.

use std::collections::{BTreeMap, HashMap};

use super::{Error, Field, Geometry, Node};

/// Selects a material slot for triangles whose centroid is inside `node`.
/// Regions are considered in slice order because arbitrary field values are
/// only meaningful as inside/outside tests, not as comparable distances.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MaterialRegion {
    pub node: Node,
    pub slot: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceOptions {
    /// The largest angle between a face and an incident face that contributes
    /// to its area-weighted vertex normal. Zero keeps every face flat.
    pub crease_angle_degrees: f32,
    /// World units per UV unit multiplier for planar charts.
    pub uv_scale: f32,
    pub default_slot: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Surface {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
    pub groups: Vec<SurfaceGroup>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SurfaceGroup {
    pub slot: u32,
    pub index_start: u32,
    pub index_count: u32,
}

#[derive(Clone, Copy)]
struct Face {
    vertices: [u32; 3],
    normal: [f32; 3],
    area_normal: [f32; 3],
    projection_axis: u8,
    slot: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct VertexKey {
    source_vertex: u32,
    normal_bits: [u32; 3],
    projection_axis: u8,
}

/// Builds ordinary indexed render attributes from extracted triangles.
///
/// Material regions select the first field whose centroid sample is at or
/// below zero. UVs use the face's dominant normal axis and remain in world
/// space, so extraction density does not affect their scale.
pub fn assemble(
    field: &Field,
    geometry: &Geometry,
    regions: &[MaterialRegion],
    options: SurfaceOptions,
) -> Result<Surface, Error> {
    validate_options(options)?;
    validate_positions(&geometry.positions)?;

    let mut faces = Vec::with_capacity(geometry.triangles.len());
    let mut centroids = Vec::with_capacity(geometry.triangles.len());
    for &triangle in &geometry.triangles {
        let vertices = triangle_positions(&geometry.positions, triangle)?;
        let area_normal = cross(sub(vertices[1], vertices[0]), sub(vertices[2], vertices[0]));
        let length_squared = dot(area_normal, area_normal);
        // Extraction already omits degenerate triangles. Keeping that contract
        // at this boundary avoids emitting non-finite normals for hand-built
        // or partially recovered Geometry values too.
        if !length_squared.is_finite() {
            return Err(Error(
                "geometry triangle has an unrepresentable area".into(),
            ));
        }
        if length_squared == 0.0 {
            continue;
        }
        let normal = normalize(area_normal).expect("nonzero finite face normal");
        faces.push(Face {
            vertices: triangle,
            normal,
            area_normal,
            projection_axis: major_axis(normal),
            slot: options.default_slot,
        });
        let centroid = scale(add(add(vertices[0], vertices[1]), vertices[2]), 1.0 / 3.0);
        if centroid.iter().any(|value| !value.is_finite()) {
            return Err(Error(
                "geometry triangle has an unrepresentable centroid".into(),
            ));
        }
        centroids.push(centroid);
    }

    if faces.is_empty() {
        return Ok(Surface {
            positions: Vec::new(),
            normals: Vec::new(),
            uvs: Vec::new(),
            indices: Vec::new(),
            groups: Vec::new(),
        });
    }
    assign_materials(field, &mut faces, &centroids, regions)?;
    let incidents = incident_faces(geometry.positions.len(), &faces);
    let normals = corner_normals(&faces, &incidents, options.crease_angle_degrees);
    emit_surface(&geometry.positions, &faces, &normals, options.uv_scale)
}

fn validate_options(options: SurfaceOptions) -> Result<(), Error> {
    if !options.crease_angle_degrees.is_finite()
        || !(0.0..=180.0).contains(&options.crease_angle_degrees)
    {
        return Err(Error(
            "crease angle must be finite and between 0 and 180 degrees".into(),
        ));
    }
    if !options.uv_scale.is_finite() || options.uv_scale <= 0.0 {
        return Err(Error("UV scale must be finite and positive".into()));
    }
    Ok(())
}

fn validate_positions(positions: &[[f32; 3]]) -> Result<(), Error> {
    if positions.iter().flatten().any(|value| !value.is_finite()) {
        return Err(Error("geometry positions must be finite".into()));
    }
    Ok(())
}

fn triangle_positions(positions: &[[f32; 3]], triangle: [u32; 3]) -> Result<[[f32; 3]; 3], Error> {
    let mut vertices = [[0.0; 3]; 3];
    for (corner, index) in triangle.into_iter().enumerate() {
        vertices[corner] = positions
            .get(index as usize)
            .copied()
            .ok_or_else(|| Error("geometry triangle references an unknown vertex".into()))?;
    }
    Ok(vertices)
}

fn assign_materials(
    field: &Field,
    faces: &mut [Face],
    centroids: &[[f32; 3]],
    regions: &[MaterialRegion],
) -> Result<(), Error> {
    let mut assigned = vec![false; faces.len()];
    for region in regions {
        let values = field.sample(region.node, centroids)?;
        for ((face, assigned), value) in faces.iter_mut().zip(&mut assigned).zip(values) {
            if !value.is_finite() {
                return Err(Error("material region produced a non-finite sample".into()));
            }
            if !*assigned && value <= 0.0 {
                face.slot = region.slot;
                *assigned = true;
            }
        }
    }
    Ok(())
}

fn incident_faces(vertex_count: usize, faces: &[Face]) -> Vec<Vec<usize>> {
    let mut incidents = vec![Vec::new(); vertex_count];
    for (face_index, face) in faces.iter().enumerate() {
        for vertex in face.vertices {
            incidents[vertex as usize].push(face_index);
        }
    }
    incidents
}

fn corner_normals(
    faces: &[Face],
    incidents: &[Vec<usize>],
    crease_degrees: f32,
) -> Vec<[[f32; 3]; 3]> {
    if crease_degrees == 0.0 {
        return faces.iter().map(|face| [face.normal; 3]).collect();
    }
    let cosine = crease_degrees.to_radians().cos();
    faces
        .iter()
        .map(|face| {
            face.vertices.map(|vertex| {
                let sum = incidents[vertex as usize]
                    .iter()
                    .filter_map(|&incident| {
                        let candidate = faces[incident];
                        (dot(face.normal, candidate.normal).clamp(-1.0, 1.0) >= cosine)
                            .then_some(candidate.area_normal)
                    })
                    .fold([0.0; 3], add);
                normalize(sum).unwrap_or(face.normal)
            })
        })
        .collect()
}

fn emit_surface(
    source_positions: &[[f32; 3]],
    faces: &[Face],
    normals: &[[[f32; 3]; 3]],
    uv_scale: f32,
) -> Result<Surface, Error> {
    let mut positions = Vec::new();
    let mut output_normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::with_capacity(faces.len() * 3);
    let mut vertices = HashMap::<VertexKey, u32>::new();
    let mut groups = BTreeMap::<u32, Vec<usize>>::new();
    for (index, face) in faces.iter().enumerate() {
        groups.entry(face.slot).or_default().push(index);
    }

    let mut output_groups = Vec::with_capacity(groups.len());
    for (slot, face_indices) in groups {
        let index_start = u32::try_from(indices.len())
            .map_err(|_| Error("surface index capacity exceeded".into()))?;
        for face_index in face_indices {
            let face = faces[face_index];
            for corner in 0..3 {
                let normal = canonical_zero(normals[face_index][corner]);
                let key = VertexKey {
                    source_vertex: face.vertices[corner],
                    normal_bits: normal.map(f32::to_bits),
                    projection_axis: face.projection_axis,
                };
                let index = if let Some(&existing) = vertices.get(&key) {
                    existing
                } else {
                    let position = source_positions[face.vertices[corner] as usize];
                    let uv = project(position, face.projection_axis, uv_scale);
                    if uv.iter().any(|value| !value.is_finite()) {
                        return Err(Error("UV projection must remain finite".into()));
                    }
                    let created = u32::try_from(positions.len())
                        .map_err(|_| Error("surface vertex capacity exceeded".into()))?;
                    positions.push(position);
                    output_normals.push(normal);
                    uvs.push(uv);
                    vertices.insert(key, created);
                    created
                };
                indices.push(index);
            }
        }
        output_groups.push(SurfaceGroup {
            slot,
            index_start,
            index_count: u32::try_from(indices.len())
                .map_err(|_| Error("surface index capacity exceeded".into()))?
                - index_start,
        });
    }
    Ok(Surface {
        positions,
        normals: output_normals,
        uvs,
        indices,
        groups: output_groups,
    })
}

fn major_axis(normal: [f32; 3]) -> u8 {
    let mut axis = 0;
    for candidate in 1..3 {
        if normal[candidate].abs() > normal[axis].abs() {
            axis = candidate;
        }
    }
    axis as u8
}

fn project(position: [f32; 3], axis: u8, scale_factor: f32) -> [f32; 2] {
    match axis {
        0 => [position[1] * scale_factor, position[2] * scale_factor],
        1 => [position[0] * scale_factor, position[2] * scale_factor],
        _ => [position[0] * scale_factor, position[1] * scale_factor],
    }
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn scale(value: [f32; 3], factor: f32) -> [f32; 3] {
    [value[0] * factor, value[1] * factor, value[2] * factor]
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn normalize(value: [f32; 3]) -> Option<[f32; 3]> {
    let length_squared = dot(value, value);
    if !length_squared.is_finite() || length_squared == 0.0 {
        return None;
    }
    let inverse_length = length_squared.sqrt().recip();
    inverse_length
        .is_finite()
        .then(|| scale(value, inverse_length))
}
fn canonical_zero(value: [f32; 3]) -> [f32; 3] {
    value.map(|component| if component == 0.0 { 0.0 } else { component })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geometry(positions: Vec<[f32; 3]>, triangles: Vec<[u32; 3]>) -> Geometry {
        Geometry {
            positions,
            triangles,
            depth: 0,
            cell_size: [1.0; 3],
            generation_seconds: 0.0,
            reoriented_triangles: 0,
            degenerate_triangles: 0,
        }
    }

    fn options(crease_angle_degrees: f32) -> SurfaceOptions {
        SurfaceOptions {
            crease_angle_degrees,
            uv_scale: 1.0,
            default_slot: 2,
        }
    }

    #[test]
    fn groups_tile_indices_and_regions_use_first_inside_match() {
        let mut field = Field::new();
        let left_half = field.plane([1.0, 0.0, 0.0], 0.0).unwrap();
        let surface = assemble(
            &field,
            &geometry(
                vec![
                    [-2.0, 0.0, 0.0],
                    [-1.0, 1.0, 0.0],
                    [-1.0, 0.0, 1.0],
                    [1.0, 0.0, 0.0],
                    [2.0, 0.0, 0.0],
                    [1.0, 1.0, 0.0],
                ],
                vec![[0, 1, 2], [3, 4, 5]],
            ),
            &[
                MaterialRegion {
                    node: left_half,
                    slot: 5,
                },
                MaterialRegion {
                    node: left_half,
                    slot: 9,
                },
            ],
            options(0.0),
        )
        .unwrap();

        assert_eq!(
            surface.groups,
            vec![
                SurfaceGroup {
                    slot: 2,
                    index_start: 0,
                    index_count: 3
                },
                SurfaceGroup {
                    slot: 5,
                    index_start: 3,
                    index_count: 3
                },
            ]
        );
        assert_eq!(
            surface
                .groups
                .iter()
                .map(|group| group.index_count)
                .sum::<u32>(),
            6
        );
    }

    #[test]
    fn crease_controls_smoothing_without_losing_chart_seams() {
        let field = Field::new();
        let geometry = geometry(
            vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 1.0],
            ],
            vec![[0, 1, 2], [0, 2, 3]],
        );
        let flat = assemble(&field, &geometry, &[], options(0.0)).unwrap();
        let smooth = assemble(&field, &geometry, &[], options(180.0)).unwrap();

        let flat_z = flat.normals[flat.indices[0] as usize];
        let flat_x = flat.normals[flat.indices[3] as usize];
        assert_eq!(flat_z, [0.0, 0.0, 1.0]);
        assert_eq!(flat_x, [1.0, 0.0, 0.0]);

        // The shared box corner has two projection charts, so it remains two
        // vertices even when its normals are smooth and equal.
        let smooth_z = smooth.indices[0] as usize;
        let smooth_x = smooth.indices[3] as usize;
        assert_ne!(smooth_z, smooth_x);
        assert_eq!(smooth.normals[smooth_z], smooth.normals[smooth_x]);
        assert!(smooth.normals[smooth_z][0] > 0.7 && smooth.normals[smooth_z][2] > 0.7);
    }
}
