//! Deterministic reconstructed voxel surfaces.
//!
//! The sampled scalar field is deliberately small and explicit: every voxel
//! center is a lattice sample with value `+0.5` when occupied and `-0.5` when
//! empty, one empty-sample layer pads the occupied bounds, and the isovalue is
//! zero. Linear edge interpolation therefore places an unsmoothed boundary at
//! the canonical voxel face. Material identity is not blended: the most common
//! occupied corner material wins, with the lower slot breaking a tie.

use std::collections::BTreeMap;

use super::{
    MeshBounds, MeshError, MeshGroup, MeshPayload, MeshStats, SurfaceMeshLimits,
    SurfaceMeshOptions, SurfaceMode,
};

const CORNERS: [[i64; 3]; 8] = [
    [0, 0, 0],
    [1, 0, 0],
    [1, 1, 0],
    [0, 1, 0],
    [0, 0, 1],
    [1, 0, 1],
    [1, 1, 1],
    [0, 1, 1],
];

const EDGES: [(usize, usize); 12] = [
    (0, 1),
    (1, 2),
    (2, 3),
    (3, 0),
    (4, 5),
    (5, 6),
    (6, 7),
    (7, 4),
    (0, 4),
    (1, 5),
    (2, 6),
    (3, 7),
];

#[derive(Clone, Copy)]
struct Face {
    corners: [usize; 4],
    edges: [usize; 4],
    axis: usize,
    fixed: i64,
}

const FACES: [Face; 6] = [
    Face {
        corners: [0, 1, 2, 3],
        edges: [0, 1, 2, 3],
        axis: 2,
        fixed: 0,
    },
    Face {
        corners: [4, 7, 6, 5],
        edges: [7, 6, 5, 4],
        axis: 2,
        fixed: 1,
    },
    Face {
        corners: [0, 3, 7, 4],
        edges: [3, 11, 7, 8],
        axis: 0,
        fixed: 0,
    },
    Face {
        corners: [1, 5, 6, 2],
        edges: [9, 5, 10, 1],
        axis: 0,
        fixed: 1,
    },
    Face {
        corners: [0, 4, 5, 1],
        edges: [8, 4, 9, 0],
        axis: 1,
        fixed: 0,
    },
    Face {
        corners: [3, 2, 6, 7],
        edges: [2, 10, 6, 11],
        axis: 1,
        fixed: 1,
    },
];

#[derive(Debug)]
struct ScalarField {
    origin: [i64; 3],
    dimensions: [usize; 3],
    materials: Vec<Option<u16>>,
    sampled_cells: u64,
}

impl ScalarField {
    fn build(
        occupied: &BTreeMap<[i64; 3], u16>,
        limits: SurfaceMeshLimits,
    ) -> Result<Option<Self>, MeshError> {
        let Some((&first, _)) = occupied.first_key_value() else {
            return Ok(None);
        };
        let mut minimum = first;
        let mut maximum = first;
        for coordinate in occupied.keys() {
            for axis in 0..3 {
                minimum[axis] = minimum[axis].min(coordinate[axis]);
                maximum[axis] = maximum[axis].max(coordinate[axis]);
            }
        }
        for axis in 0..3 {
            minimum[axis] = minimum[axis]
                .checked_sub(1)
                .ok_or(MeshError::CoordinateRangeTooLarge)?;
            maximum[axis] = maximum[axis]
                .checked_add(1)
                .ok_or(MeshError::CoordinateRangeTooLarge)?;
        }
        let mut dimensions = [0_usize; 3];
        for axis in 0..3 {
            let width = maximum[axis]
                .checked_sub(minimum[axis])
                .and_then(|value| value.checked_add(1))
                .ok_or(MeshError::CoordinateRangeTooLarge)?;
            dimensions[axis] =
                usize::try_from(width).map_err(|_| MeshError::CoordinateRangeTooLarge)?;
        }
        let point_count = checked_product(dimensions)?;
        let sampled_cells =
            checked_product([dimensions[0] - 1, dimensions[1] - 1, dimensions[2] - 1])? as u64;
        if sampled_cells > limits.max_sampled_cells {
            return Err(MeshError::TooManySampledCells {
                cells: sampled_cells,
                limit: limits.max_sampled_cells,
            });
        }
        let temporary_bytes = (point_count as u64)
            .checked_mul(std::mem::size_of::<Option<u16>>() as u64)
            .ok_or(MeshError::CoordinateRangeTooLarge)?;
        if temporary_bytes > limits.max_temporary_field_bytes {
            return Err(MeshError::TemporaryFieldTooLarge {
                bytes: temporary_bytes,
                limit: limits.max_temporary_field_bytes,
            });
        }
        let mut field = Self {
            origin: minimum,
            dimensions,
            materials: vec![None; point_count],
            sampled_cells,
        };
        for (&coordinate, &slot) in occupied {
            let index = field
                .index(coordinate)
                .expect("occupied coordinate is in padded field");
            field.materials[index] = Some(slot);
        }
        Ok(Some(field))
    }

    fn maximum_point(&self) -> [i64; 3] {
        std::array::from_fn(|axis| {
            self.origin[axis] + i64::try_from(self.dimensions[axis] - 1).unwrap()
        })
    }

    fn maximum_cell(&self) -> [i64; 3] {
        std::array::from_fn(|axis| {
            self.origin[axis] + i64::try_from(self.dimensions[axis] - 2).unwrap()
        })
    }

    fn index(&self, coordinate: [i64; 3]) -> Option<usize> {
        let mut local = [0_usize; 3];
        for axis in 0..3 {
            let value = coordinate[axis].checked_sub(self.origin[axis])?;
            local[axis] = usize::try_from(value).ok()?;
            if local[axis] >= self.dimensions[axis] {
                return None;
            }
        }
        local[2]
            .checked_mul(self.dimensions[1])?
            .checked_add(local[1])?
            .checked_mul(self.dimensions[0])?
            .checked_add(local[0])
    }

    fn material(&self, coordinate: [i64; 3]) -> Option<u16> {
        self.index(coordinate)
            .and_then(|index| self.materials[index])
    }

    fn cube_materials(&self, cell: [i64; 3]) -> [Option<u16>; 8] {
        std::array::from_fn(|index| self.material(add_i64(cell, CORNERS[index])))
    }
}

#[derive(Default)]
struct Lane {
    positions: Vec<[f64; 3]>,
    normals: Vec<[f64; 3]>,
    indices: Vec<u32>,
}

pub(super) fn mesh_reconstructed_cells(
    cell_size: f64,
    pivot: [f64; 3],
    occupied: &BTreeMap<[i64; 3], u16>,
    options: SurfaceMeshOptions,
) -> Result<MeshPayload, MeshError> {
    mesh_reconstructed_cells_owned(cell_size, pivot, occupied, options, None)
}

pub(super) fn mesh_reconstructed_cells_owned(
    cell_size: f64,
    pivot: [f64; 3],
    occupied: &BTreeMap<[i64; 3], u16>,
    options: SurfaceMeshOptions,
    owner: Option<([i64; 3], [i64; 3])>,
) -> Result<MeshPayload, MeshError> {
    let (source_faces, faces_culled) = count_source_faces(occupied, options.limits, owner)?;
    let Some(field) = ScalarField::build(occupied, options.limits)? else {
        return Ok(empty_payload(options.mode));
    };
    match options.mode {
        SurfaceMode::GreedyCubes => unreachable!("greedy mode is routed by the parent module"),
        SurfaceMode::MarchingCubes => marching_cubes(
            &field,
            cell_size,
            pivot,
            options.limits,
            source_faces,
            faces_culled,
            owner,
        ),
        SurfaceMode::DualContouring => dual_contouring(
            &field,
            cell_size,
            pivot,
            options.limits,
            source_faces,
            faces_culled,
            owner,
        ),
    }
}

fn count_source_faces(
    occupied: &BTreeMap<[i64; 3], u16>,
    limits: SurfaceMeshLimits,
    owner: Option<([i64; 3], [i64; 3])>,
) -> Result<(u32, u32), MeshError> {
    let mut visible = 0_u64;
    let mut culled = 0_u64;
    const DIRECTIONS: [[i64; 3]; 6] = [
        [-1, 0, 0],
        [1, 0, 0],
        [0, -1, 0],
        [0, 1, 0],
        [0, 0, -1],
        [0, 0, 1],
    ];
    for coordinate in occupied.keys() {
        if owner.is_some_and(|bounds| !within_owner(*coordinate, bounds)) {
            continue;
        }
        for direction in DIRECTIONS {
            let neighbour = checked_add_i64(*coordinate, direction)?;
            if occupied.contains_key(&neighbour) {
                culled = culled.saturating_add(1);
            } else {
                visible = visible.saturating_add(1);
                if visible > limits.max_source_faces {
                    return Err(MeshError::TooManyFaces {
                        faces: visible,
                        limit: limits.max_source_faces,
                    });
                }
            }
        }
    }
    Ok((
        u32::try_from(visible).map_err(|_| MeshError::TooManyFaces {
            faces: visible,
            limit: limits.max_source_faces,
        })?,
        u32::try_from(culled).unwrap_or(u32::MAX),
    ))
}

fn marching_cubes(
    field: &ScalarField,
    cell_size: f64,
    pivot: [f64; 3],
    limits: SurfaceMeshLimits,
    source_faces: u32,
    faces_culled: u32,
    owner: Option<([i64; 3], [i64; 3])>,
) -> Result<MeshPayload, MeshError> {
    let mut lanes = BTreeMap::<u16, Lane>::new();
    let mut total_vertices = 0_u64;
    let mut total_indices = 0_u64;
    for_each_coordinate(field.origin, field.maximum_cell(), |cell| {
        let materials = field.cube_materials(cell);
        if materials.iter().all(Option::is_none) || materials.iter().all(Option::is_some) {
            return Ok(());
        }
        let owner_coordinate = materials
            .iter()
            .enumerate()
            .filter_map(|(index, material)| material.map(|_| add_i64(cell, CORNERS[index])))
            .min()
            .expect("active cube has an occupied corner");
        if owner.is_some_and(|bounds| !within_owner(owner_coordinate, bounds)) {
            return Ok(());
        }
        let signed: [f64; 8] =
            materials.map(|material| if material.is_some() { 0.5 } else { -0.5 });
        let crossings =
            EDGES.map(|(a, b)| signed[a].is_sign_positive() != signed[b].is_sign_positive());
        let mut adjacency: [Vec<usize>; 12] = std::array::from_fn(|_| Vec::new());
        for face in FACES {
            let crossed = face
                .edges
                .iter()
                .copied()
                .filter(|edge| crossings[*edge])
                .collect::<Vec<_>>();
            match crossed.as_slice() {
                [a, b] => connect(&mut adjacency, *a, *b),
                [_, _, _, _] => {
                    let [c0, c1, c2, c3] = face.corners;
                    let determinant = signed[c0] * signed[c2] - signed[c1] * signed[c3];
                    let face_coordinate = {
                        let mut value = cell;
                        value[face.axis] += face.fixed;
                        value
                    };
                    let positive = if determinant.abs() > f64::EPSILON {
                        determinant > 0.0
                    } else {
                        // Binary diagonal saddles are exactly tied. The parity is
                        // based on the global face, so adjacent chunks/cells make
                        // the same asymptotic-decider tie decision.
                        ((face_coordinate[0]
                            ^ face_coordinate[1]
                            ^ face_coordinate[2]
                            ^ face.axis as i64)
                            & 1)
                            == 0
                    };
                    if positive {
                        connect(&mut adjacency, face.edges[0], face.edges[1]);
                        connect(&mut adjacency, face.edges[2], face.edges[3]);
                    } else {
                        connect(&mut adjacency, face.edges[0], face.edges[3]);
                        connect(&mut adjacency, face.edges[1], face.edges[2]);
                    }
                }
                [] => {}
                _ => return Err(MeshError::CoordinateRangeTooLarge),
            }
        }

        let mut visited = [false; 12];
        for start in 0..12 {
            if !crossings[start] || visited[start] {
                continue;
            }
            let mut loop_edges = Vec::new();
            let mut previous = usize::MAX;
            let mut current = start;
            loop {
                if visited[current] {
                    if current == start {
                        break;
                    }
                    return Err(MeshError::CoordinateRangeTooLarge);
                }
                visited[current] = true;
                loop_edges.push(current);
                let neighbours = &adjacency[current];
                if neighbours.len() != 2 {
                    return Err(MeshError::CoordinateRangeTooLarge);
                }
                let next = if neighbours[0] != previous {
                    neighbours[0]
                } else {
                    neighbours[1]
                };
                previous = current;
                current = next;
                if current == start {
                    break;
                }
            }
            if loop_edges.len() < 3 {
                return Err(MeshError::CoordinateRangeTooLarge);
            }
            let added_vertices = loop_edges.len() as u64 + 1;
            let added_indices = loop_edges.len() as u64 * 3;
            check_output_growth(
                total_vertices,
                total_indices,
                added_vertices,
                added_indices,
                limits,
            )?;
            let slot = majority_material(materials).expect("active cube has an occupied corner");
            if !lanes.contains_key(&slot)
                && lanes.len() as u64 + 1 > u64::from(limits.max_material_partitions)
            {
                return Err(MeshError::TooManyMaterialPartitions {
                    partitions: lanes.len() as u64 + 1,
                    limit: limits.max_material_partitions,
                });
            }
            let lane = lanes.entry(slot).or_default();
            let mut points = loop_edges
                .iter()
                .map(|edge| edge_point(cell, *edge))
                .collect::<Vec<_>>();
            let mut normals = loop_edges
                .iter()
                .map(|edge| {
                    let local = edge_local_point(*edge);
                    outward_normal(
                        trilinear_gradient(signed, local),
                        sub_f64(edge_point(cell, *edge), add_f64(cell_f64(cell), [1.0; 3])),
                    )
                })
                .collect::<Vec<_>>();
            let mut centroid = [0.0; 3];
            let mut centroid_normal = [0.0; 3];
            for point in &points {
                centroid = add_f64(centroid, *point);
            }
            for normal in &normals {
                centroid_normal = add_f64(centroid_normal, *normal);
            }
            centroid = scale_f64(centroid, 1.0 / points.len() as f64);
            centroid_normal = normalize_or(centroid_normal, [0.0, 1.0, 0.0]);
            let geometric = polygon_normal(&points);
            if dot_f64(geometric, centroid_normal) < 0.0 {
                points.reverse();
                normals.reverse();
            }
            let base = u32::try_from(lane.positions.len()).unwrap();
            lane.positions.push(centroid);
            lane.normals.push(centroid_normal);
            lane.positions.extend(points.iter().copied());
            lane.normals.extend(normals.iter().copied());
            for index in 0..points.len() {
                lane.indices.extend_from_slice(&[
                    base,
                    base + 1 + index as u32,
                    base + 1 + ((index + 1) % points.len()) as u32,
                ]);
            }
            total_vertices += added_vertices;
            total_indices += added_indices;
        }
        Ok(())
    })?;

    finalize_lanes(
        SurfaceMode::MarchingCubes,
        lanes,
        cell_size,
        pivot,
        MeshStats {
            surface_mode: SurfaceMode::MarchingCubes,
            vertices: total_vertices as u32,
            indices: total_indices as u32,
            triangles: (total_indices / 3) as u32,
            quads: 0,
            faces_emitted: (total_indices / 3) as u32,
            source_faces,
            faces_culled,
            sampled_cells: field.sampled_cells,
            qef_rank_deficient: 0,
            qef_fallbacks: 0,
        },
    )
}

#[derive(Debug, Clone)]
struct DualVertex {
    position: [f64; 3],
    normal: [f64; 3],
    index: u32,
}

fn dual_contouring(
    field: &ScalarField,
    cell_size: f64,
    pivot: [f64; 3],
    limits: SurfaceMeshLimits,
    source_faces: u32,
    faces_culled: u32,
    owner: Option<([i64; 3], [i64; 3])>,
) -> Result<MeshPayload, MeshError> {
    let mut active = BTreeMap::<[i64; 3], DualVertex>::new();
    let mut rank_deficient = 0_u32;
    let mut fallbacks = 0_u32;
    for_each_coordinate(field.origin, field.maximum_cell(), |cell| {
        let materials = field.cube_materials(cell);
        if materials.iter().all(Option::is_none) || materials.iter().all(Option::is_some) {
            return Ok(());
        }
        let signed: [f64; 8] =
            materials.map(|material| if material.is_some() { 0.5 } else { -0.5 });
        let mut samples = Vec::new();
        for (edge, &(a, b)) in EDGES.iter().enumerate() {
            if signed[a].is_sign_positive() == signed[b].is_sign_positive() {
                continue;
            }
            let point = edge_point(cell, edge);
            let normal = outward_normal(
                trilinear_gradient(signed, edge_local_point(edge)),
                sub_f64(point, add_f64(cell_f64(cell), [1.0; 3])),
            );
            samples.push((point, normal));
        }
        let (position, was_rank_deficient, fallback) = solve_qef(cell, &samples);
        rank_deficient = rank_deficient.saturating_add(u32::from(was_rank_deficient));
        fallbacks = fallbacks.saturating_add(u32::from(fallback));
        let normal = normalize_or(
            samples
                .iter()
                .fold([0.0; 3], |sum, (_, normal)| add_f64(sum, *normal)),
            [0.0, 1.0, 0.0],
        );
        let next_count = active.len() as u64 + 1;
        if next_count > u64::from(limits.max_vertices) {
            return Err(MeshError::TooManyVertices {
                vertices: next_count,
            });
        }
        let temporary_bytes = (field.materials.len() as u64)
            .checked_mul(std::mem::size_of::<Option<u16>>() as u64)
            .and_then(|bytes| {
                bytes.checked_add(
                    next_count * (std::mem::size_of::<([i64; 3], DualVertex)>() as u64),
                )
            })
            .ok_or(MeshError::CoordinateRangeTooLarge)?;
        if temporary_bytes > limits.max_temporary_field_bytes {
            return Err(MeshError::TemporaryFieldTooLarge {
                bytes: temporary_bytes,
                limit: limits.max_temporary_field_bytes,
            });
        }
        active.insert(
            cell,
            DualVertex {
                position,
                normal,
                index: 0,
            },
        );
        Ok(())
    })?;

    for (index, vertex) in active.values_mut().enumerate() {
        vertex.index = index as u32;
    }
    let mut lane_indices = BTreeMap::<u16, Vec<u32>>::new();
    let mut total_indices = 0_u64;
    for axis in 0..3 {
        let mut edge_maximum = field.maximum_point();
        edge_maximum[axis] -= 1;
        for_each_coordinate(field.origin, edge_maximum, |start| {
            let mut end = start;
            end[axis] += 1;
            let start_material = field.material(start);
            let end_material = field.material(end);
            if start_material.is_some() == end_material.is_some() {
                return Ok(());
            }
            let owner_coordinate = if start_material.is_some() { start } else { end };
            if owner.is_some_and(|bounds| !within_owner(owner_coordinate, bounds)) {
                return Ok(());
            }
            let cells = incident_cells(axis, start);
            let Some(vertices) = cells
                .map(|cell| active.get(&cell))
                .into_iter()
                .collect::<Option<Vec<_>>>()
            else {
                return Err(MeshError::CoordinateRangeTooLarge);
            };
            let slot = start_material
                .or(end_material)
                .expect("crossing has occupied endpoint");
            if !lane_indices.contains_key(&slot)
                && lane_indices.len() as u64 + 1 > u64::from(limits.max_material_partitions)
            {
                return Err(MeshError::TooManyMaterialPartitions {
                    partitions: lane_indices.len() as u64 + 1,
                    limit: limits.max_material_partitions,
                });
            }
            check_output_growth(active.len() as u64, total_indices, 0, 6, limits)?;
            let mut order = [0_usize, 1, 2, 3];
            let average_normal = normalize_or(
                vertices
                    .iter()
                    .fold([0.0; 3], |sum, vertex| add_f64(sum, vertex.normal)),
                [0.0, 1.0, 0.0],
            );
            let geometric = cross_f64(
                sub_f64(vertices[1].position, vertices[0].position),
                sub_f64(vertices[2].position, vertices[0].position),
            );
            if dot_f64(geometric, average_normal) < 0.0 {
                order.reverse();
            }
            let diagonal_02 =
                squared_distance(vertices[order[0]].position, vertices[order[2]].position);
            let diagonal_13 =
                squared_distance(vertices[order[1]].position, vertices[order[3]].position);
            let indices = lane_indices.entry(slot).or_default();
            if diagonal_02 <= diagonal_13 {
                indices.extend_from_slice(&[
                    vertices[order[0]].index,
                    vertices[order[1]].index,
                    vertices[order[2]].index,
                    vertices[order[0]].index,
                    vertices[order[2]].index,
                    vertices[order[3]].index,
                ]);
            } else {
                indices.extend_from_slice(&[
                    vertices[order[0]].index,
                    vertices[order[1]].index,
                    vertices[order[3]].index,
                    vertices[order[1]].index,
                    vertices[order[2]].index,
                    vertices[order[3]].index,
                ]);
            }
            total_indices += 6;
            Ok(())
        })?;
    }

    let mut indices = Vec::with_capacity(total_indices as usize);
    let mut groups = Vec::with_capacity(lane_indices.len());
    for (slot, lane) in lane_indices {
        let start = indices.len() as u32;
        indices.extend(lane);
        groups.push(MeshGroup {
            material_slot: slot,
            start,
            count: indices.len() as u32 - start,
        });
    }
    let positions = active
        .values()
        .map(|vertex| vertex.position)
        .collect::<Vec<_>>();
    let normals = active
        .values()
        .map(|vertex| vertex.normal)
        .collect::<Vec<_>>();
    finalize_raw(
        RawMesh {
            mode: SurfaceMode::DualContouring,
            positions,
            normals,
            indices,
            groups,
            stats: MeshStats {
                surface_mode: SurfaceMode::DualContouring,
                vertices: active.len() as u32,
                indices: total_indices as u32,
                triangles: (total_indices / 3) as u32,
                quads: (total_indices / 6) as u32,
                faces_emitted: (total_indices / 3) as u32,
                source_faces,
                faces_culled,
                sampled_cells: field.sampled_cells,
                qef_rank_deficient: rank_deficient,
                qef_fallbacks: fallbacks,
            },
        },
        cell_size,
        pivot,
    )
}

fn solve_qef(cell: [i64; 3], samples: &[([f64; 3], [f64; 3])]) -> ([f64; 3], bool, bool) {
    let mass_point = scale_f64(
        samples
            .iter()
            .fold([0.0; 3], |sum, (point, _)| add_f64(sum, *point)),
        1.0 / samples.len() as f64,
    );
    let mut ata = [[0.0_f64; 3]; 3];
    let mut rhs = [0.0_f64; 3];
    for (point, normal) in samples {
        let relative = sub_f64(*point, mass_point);
        let projected = dot_f64(*normal, relative);
        for row in 0..3 {
            rhs[row] += normal[row] * projected;
            for column in 0..3 {
                ata[row][column] += normal[row] * normal[column];
            }
        }
    }
    let (eigenvalues, eigenvectors) = jacobi_eigen(ata);
    let maximum = eigenvalues.iter().copied().fold(0.0_f64, f64::max);
    let threshold = (maximum * 1.0e-8).max(1.0e-12);
    let rank = eigenvalues
        .iter()
        .filter(|value| **value > threshold)
        .count();
    let mut offset = [0.0; 3];
    if rank > 0 {
        for axis in 0..3 {
            if eigenvalues[axis] <= threshold {
                continue;
            }
            let vector = [
                eigenvectors[0][axis],
                eigenvectors[1][axis],
                eigenvectors[2][axis],
            ];
            let scale = dot_f64(vector, rhs) / eigenvalues[axis];
            offset = add_f64(offset, scale_f64(vector, scale));
        }
    }
    let candidate = add_f64(mass_point, offset);
    let fallback = rank == 0 || !candidate.iter().all(|value| value.is_finite());
    let candidate = if fallback { mass_point } else { candidate };
    let minimum = add_f64(cell_f64(cell), [0.5; 3]);
    let maximum = add_f64(cell_f64(cell), [1.5; 3]);
    (
        std::array::from_fn(|axis| candidate[axis].clamp(minimum[axis], maximum[axis])),
        rank < 3,
        fallback,
    )
}

fn jacobi_eigen(mut matrix: [[f64; 3]; 3]) -> ([f64; 3], [[f64; 3]; 3]) {
    let mut vectors = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    for _ in 0..16 {
        let mut pair = (0, 1);
        for candidate in [(0, 2), (1, 2)] {
            if matrix[candidate.0][candidate.1].abs() > matrix[pair.0][pair.1].abs() {
                pair = candidate;
            }
        }
        let (p, q) = pair;
        if matrix[p][q].abs() <= 1.0e-15 {
            break;
        }
        let angle = 0.5 * (2.0 * matrix[p][q]).atan2(matrix[q][q] - matrix[p][p]);
        let (sine, cosine) = angle.sin_cos();
        for row in &mut matrix {
            let mp = row[p];
            let mq = row[q];
            row[p] = cosine * mp - sine * mq;
            row[q] = sine * mp + cosine * mq;
        }
        let (before_q, from_q) = matrix.split_at_mut(q);
        let p_row = &mut before_q[p];
        let q_row = &mut from_q[0];
        for (p_value, q_value) in p_row.iter_mut().zip(q_row.iter_mut()) {
            let mp = *p_value;
            let mq = *q_value;
            *p_value = cosine * mp - sine * mq;
            *q_value = sine * mp + cosine * mq;
        }
        for row in &mut vectors {
            let vp = row[p];
            let vq = row[q];
            row[p] = cosine * vp - sine * vq;
            row[q] = sine * vp + cosine * vq;
        }
    }
    let mut order = [0_usize, 1, 2];
    order.sort_by(|left, right| matrix[*right][*right].total_cmp(&matrix[*left][*left]));
    (
        order.map(|index| matrix[index][index].max(0.0)),
        std::array::from_fn(|row| order.map(|index| vectors[row][index])),
    )
}

fn incident_cells(axis: usize, edge: [i64; 3]) -> [[i64; 3]; 4] {
    match axis {
        0 => [
            [edge[0], edge[1] - 1, edge[2] - 1],
            [edge[0], edge[1], edge[2] - 1],
            [edge[0], edge[1], edge[2]],
            [edge[0], edge[1] - 1, edge[2]],
        ],
        1 => [
            [edge[0] - 1, edge[1], edge[2] - 1],
            [edge[0] - 1, edge[1], edge[2]],
            [edge[0], edge[1], edge[2]],
            [edge[0], edge[1], edge[2] - 1],
        ],
        _ => [
            [edge[0] - 1, edge[1] - 1, edge[2]],
            [edge[0], edge[1] - 1, edge[2]],
            [edge[0], edge[1], edge[2]],
            [edge[0] - 1, edge[1], edge[2]],
        ],
    }
}

fn finalize_lanes(
    mode: SurfaceMode,
    lanes: BTreeMap<u16, Lane>,
    cell_size: f64,
    pivot: [f64; 3],
    stats: MeshStats,
) -> Result<MeshPayload, MeshError> {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();
    let mut groups = Vec::with_capacity(lanes.len());
    for (slot, lane) in lanes {
        let vertex_base = positions.len() as u32;
        let group_start = indices.len() as u32;
        positions.extend(lane.positions);
        normals.extend(lane.normals);
        indices.extend(lane.indices.into_iter().map(|index| index + vertex_base));
        groups.push(MeshGroup {
            material_slot: slot,
            start: group_start,
            count: indices.len() as u32 - group_start,
        });
    }
    finalize_raw(
        RawMesh {
            mode,
            positions,
            normals,
            indices,
            groups,
            stats,
        },
        cell_size,
        pivot,
    )
}

struct RawMesh {
    mode: SurfaceMode,
    positions: Vec<[f64; 3]>,
    normals: Vec<[f64; 3]>,
    indices: Vec<u32>,
    groups: Vec<MeshGroup>,
    stats: MeshStats,
}

fn finalize_raw(raw: RawMesh, cell_size: f64, pivot: [f64; 3]) -> Result<MeshPayload, MeshError> {
    let mut positions = Vec::with_capacity(raw.positions.len() * 3);
    let mut normals = Vec::with_capacity(raw.normals.len() * 3);
    let mut minimum = [f32::INFINITY; 3];
    let mut maximum = [f32::NEG_INFINITY; 3];
    for (point, normal) in raw.positions.into_iter().zip(raw.normals) {
        for axis in 0..3 {
            let value = (point[axis] - pivot[axis]) * cell_size;
            let rendered = value as f32;
            let rendered_normal = normal[axis] as f32;
            if !value.is_finite() || !rendered.is_finite() || !rendered_normal.is_finite() {
                return Err(MeshError::PositionOutOfRange);
            }
            minimum[axis] = minimum[axis].min(rendered);
            maximum[axis] = maximum[axis].max(rendered);
            positions.push(rendered);
            normals.push(rendered_normal);
        }
    }
    let bounds = if positions.is_empty() {
        MeshBounds {
            min: [0.0; 3],
            max: [0.0; 3],
        }
    } else {
        MeshBounds {
            min: minimum,
            max: maximum,
        }
    };
    Ok(MeshPayload {
        surface_mode: raw.mode,
        positions,
        normals,
        tile_coordinates: Vec::new(),
        indices: raw.indices,
        groups: raw.groups,
        bounds,
        stats: raw.stats,
    })
}

fn empty_payload(mode: SurfaceMode) -> MeshPayload {
    MeshPayload {
        surface_mode: mode,
        positions: Vec::new(),
        normals: Vec::new(),
        tile_coordinates: Vec::new(),
        indices: Vec::new(),
        groups: Vec::new(),
        bounds: MeshBounds {
            min: [0.0; 3],
            max: [0.0; 3],
        },
        stats: MeshStats {
            surface_mode: mode,
            ..MeshStats::default()
        },
    }
}

fn majority_material(materials: [Option<u16>; 8]) -> Option<u16> {
    let mut counts = BTreeMap::<u16, u8>::new();
    for slot in materials.into_iter().flatten() {
        *counts.entry(slot).or_default() += 1;
    }
    counts
        .into_iter()
        .max_by(|(left_slot, left_count), (right_slot, right_count)| {
            left_count
                .cmp(right_count)
                .then_with(|| right_slot.cmp(left_slot))
        })
        .map(|(slot, _)| slot)
}

fn within_owner(coordinate: [i64; 3], bounds: ([i64; 3], [i64; 3])) -> bool {
    (0..3).all(|axis| coordinate[axis] >= bounds.0[axis] && coordinate[axis] < bounds.1[axis])
}

fn check_output_growth(
    vertices: u64,
    indices: u64,
    added_vertices: u64,
    added_indices: u64,
    limits: SurfaceMeshLimits,
) -> Result<(), MeshError> {
    let vertices = vertices
        .checked_add(added_vertices)
        .ok_or(MeshError::TooManyVertices { vertices: u64::MAX })?;
    if vertices > u64::from(limits.max_vertices) {
        return Err(MeshError::TooManyVertices { vertices });
    }
    let indices = indices
        .checked_add(added_indices)
        .ok_or(MeshError::TooManyIndices {
            indices: u64::MAX,
            limit: limits.max_indices,
        })?;
    if indices > u64::from(limits.max_indices) {
        return Err(MeshError::TooManyIndices {
            indices,
            limit: limits.max_indices,
        });
    }
    Ok(())
}

fn connect(adjacency: &mut [Vec<usize>; 12], left: usize, right: usize) {
    if !adjacency[left].contains(&right) {
        adjacency[left].push(right);
        adjacency[left].sort_unstable();
    }
    if !adjacency[right].contains(&left) {
        adjacency[right].push(left);
        adjacency[right].sort_unstable();
    }
}

fn edge_point(cell: [i64; 3], edge: usize) -> [f64; 3] {
    let local = edge_local_point(edge);
    std::array::from_fn(|axis| cell[axis] as f64 + local[axis] + 0.5)
}

fn edge_local_point(edge: usize) -> [f64; 3] {
    let (a, b) = EDGES[edge];
    std::array::from_fn(|axis| (CORNERS[a][axis] + CORNERS[b][axis]) as f64 * 0.5)
}

fn trilinear_gradient(values: [f64; 8], point: [f64; 3]) -> [f64; 3] {
    let [x, y, z] = point;
    let derivative_x = (1.0 - y) * (1.0 - z) * (values[1] - values[0])
        + y * (1.0 - z) * (values[2] - values[3])
        + (1.0 - y) * z * (values[5] - values[4])
        + y * z * (values[6] - values[7]);
    let derivative_y = (1.0 - x) * (1.0 - z) * (values[3] - values[0])
        + x * (1.0 - z) * (values[2] - values[1])
        + (1.0 - x) * z * (values[7] - values[4])
        + x * z * (values[6] - values[5]);
    let derivative_z = (1.0 - x) * (1.0 - y) * (values[4] - values[0])
        + x * (1.0 - y) * (values[5] - values[1])
        + (1.0 - x) * y * (values[7] - values[3])
        + x * y * (values[6] - values[2]);
    [derivative_x, derivative_y, derivative_z]
}

fn outward_normal(gradient: [f64; 3], fallback: [f64; 3]) -> [f64; 3] {
    normalize_or(
        scale_f64(gradient, -1.0),
        normalize_or(fallback, [0.0, 1.0, 0.0]),
    )
}

fn polygon_normal(points: &[[f64; 3]]) -> [f64; 3] {
    let mut normal = [0.0; 3];
    for index in 0..points.len() {
        let current = points[index];
        let next = points[(index + 1) % points.len()];
        normal[0] += (current[1] - next[1]) * (current[2] + next[2]);
        normal[1] += (current[2] - next[2]) * (current[0] + next[0]);
        normal[2] += (current[0] - next[0]) * (current[1] + next[1]);
    }
    normal
}

fn for_each_coordinate(
    minimum: [i64; 3],
    maximum: [i64; 3],
    mut visit: impl FnMut([i64; 3]) -> Result<(), MeshError>,
) -> Result<(), MeshError> {
    let mut z = minimum[2];
    loop {
        let mut y = minimum[1];
        loop {
            let mut x = minimum[0];
            loop {
                visit([x, y, z])?;
                if x == maximum[0] {
                    break;
                }
                x = x.checked_add(1).ok_or(MeshError::CoordinateRangeTooLarge)?;
            }
            if y == maximum[1] {
                break;
            }
            y = y.checked_add(1).ok_or(MeshError::CoordinateRangeTooLarge)?;
        }
        if z == maximum[2] {
            break;
        }
        z = z.checked_add(1).ok_or(MeshError::CoordinateRangeTooLarge)?;
    }
    Ok(())
}

fn checked_product(values: [usize; 3]) -> Result<usize, MeshError> {
    values
        .into_iter()
        .try_fold(1_usize, |total, value| total.checked_mul(value))
        .ok_or(MeshError::CoordinateRangeTooLarge)
}

fn checked_add_i64(left: [i64; 3], right: [i64; 3]) -> Result<[i64; 3], MeshError> {
    Ok([
        left[0]
            .checked_add(right[0])
            .ok_or(MeshError::PositionOutOfRange)?,
        left[1]
            .checked_add(right[1])
            .ok_or(MeshError::PositionOutOfRange)?,
        left[2]
            .checked_add(right[2])
            .ok_or(MeshError::PositionOutOfRange)?,
    ])
}

fn add_i64(left: [i64; 3], right: [i64; 3]) -> [i64; 3] {
    [left[0] + right[0], left[1] + right[1], left[2] + right[2]]
}

fn cell_f64(cell: [i64; 3]) -> [f64; 3] {
    [cell[0] as f64, cell[1] as f64, cell[2] as f64]
}

fn add_f64(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [left[0] + right[0], left[1] + right[1], left[2] + right[2]]
}

fn sub_f64(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

fn scale_f64(value: [f64; 3], scale: f64) -> [f64; 3] {
    [value[0] * scale, value[1] * scale, value[2] * scale]
}

fn dot_f64(left: [f64; 3], right: [f64; 3]) -> f64 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

fn cross_f64(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}

fn normalize_or(value: [f64; 3], fallback: [f64; 3]) -> [f64; 3] {
    let length = dot_f64(value, value).sqrt();
    if length.is_finite() && length > 1.0e-12 {
        scale_f64(value, 1.0 / length)
    } else {
        fallback
    }
}

fn squared_distance(left: [f64; 3], right: [f64; 3]) -> f64 {
    dot_f64(sub_f64(left, right), sub_f64(left, right))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jacobi_qef_eigenvalues_are_deterministic() {
        let (values, vectors) = jacobi_eigen([[2.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 0.0]]);
        assert_eq!(values, [2.0, 1.0, 0.0]);
        assert_eq!(vectors, [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
    }

    #[test]
    fn qef_degenerate_samples_have_finite_deterministic_outcomes() {
        let cases = [
            vec![([1.0, 1.0, 1.0], [1.0, 0.0, 0.0]); 3],
            vec![
                ([1.0, 1.0, 1.0], [1.0, 0.0, 0.0]),
                ([1.0, 1.0, 1.0], [0.0, 1.0, 0.0]),
                ([1.0, 1.0, 1.0], [0.0, 0.0, 1.0]),
            ],
            vec![
                ([0.5, 1.0, 1.0], [1.0, 0.0, 0.0]),
                ([1.5, 1.0, 1.0], [1.0, 1.0e-14, 0.0]),
            ],
            vec![([1.0, 1.0, 1.0], [1.0e-30, 0.0, 0.0])],
            vec![([1.0, 1.0, 1.0], [0.0, 0.0, 0.0])],
        ];
        for samples in cases {
            let first = solve_qef([0, 0, 0], &samples);
            let second = solve_qef([0, 0, 0], &samples);
            assert_eq!(first, second);
            assert!(first.0.iter().all(|value| value.is_finite()));
            assert!(first.0.iter().all(|value| (0.5..=1.5).contains(value)));
        }
    }
}
