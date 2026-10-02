//! Reconstructed voxel and scalar surfaces on a dense sample lattice.
//!
//! Samples are lattice points and a cell spans eight neighbouring samples.
//! Values are positive inside. Voxel data samples each voxel centre: a solid
//! voxel reads its density magnitude (0.5 without one) and an empty voxel the
//! negated magnitude, so linear edge interpolation places the default surface
//! exactly on the canonical cube face. Scalar data reads `isovalue - sample`.
//!
//! A region owns a primitive through one sample: Dual Contouring by the
//! inside endpoint of the crossing edge, Marching Cubes by the
//! lexicographically first inside corner of its cell. Adjacent regions meshed
//! from the same samples therefore make identical decisions without
//! duplicating a primitive, and a region needs only a one-sample halo.
//!
//! Each material chooses its surface mode (voxel data only) and its character:
//! vertex placement, crease angle and roughness. Where materials meet in one
//! cell, the sharper placement and the smaller roughness place the shared
//! vertex.

use std::collections::BTreeMap;

use core_space::Direction6;

use super::{
    texture_mapping::voxel_surface_texture_basis, MaterialSurface, MeshBounds, MeshError,
    MeshGroup, MeshPayload, MeshStats, SurfaceMaterials, SurfaceMeshLimits, SurfaceMode,
    VertexPlacement,
};

const CORNERS: [[usize; 3]; 8] = [
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
    fixed: usize,
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

/// The material character table as seen by one extraction. Voxel data
/// resolves an absent slot to the session mode with the default character;
/// scalar data is always dual contoured.
#[derive(Clone, Copy)]
pub(super) struct Characters<'a> {
    pub materials: &'a SurfaceMaterials,
    pub default_mode: SurfaceMode,
}

impl Characters<'_> {
    pub(super) fn of(&self, slot: u16) -> MaterialSurface {
        self.materials
            .get(slot)
            .copied()
            .unwrap_or(MaterialSurface {
                mode: self.default_mode,
                character: Default::default(),
            })
    }
}

/// Placement order for "the sharper material wins".
fn sharpness(surface: MaterialSurface) -> u8 {
    if surface.mode == SurfaceMode::GreedyCubes {
        return 3;
    }
    match surface.character.placement {
        VertexPlacement::Smooth => 0,
        VertexPlacement::Sharp => 1,
        VertexPlacement::Blocky => 2,
    }
}

/// A dense lattice of positive-inside samples and their materials.
#[derive(Debug)]
pub(super) struct Lattice {
    /// Global coordinate of sample zero. Positions, ownership and roughness
    /// all use global coordinates so independent regions agree.
    pub origin: [i64; 3],
    pub dims: [usize; 3],
    pub values: Vec<f64>,
    pub materials: Vec<u16>,
    /// Scalar data: the lattice is the whole domain, so crossings at its
    /// boundary stay open, and an escaped QEF minimizer keeps the mass point.
    pub explicit: bool,
}

impl Lattice {
    /// An all-empty voxel lattice covering `[minimum, minimum + dims)`.
    pub(super) fn voxels(
        minimum: [i64; 3],
        dims: [usize; 3],
        limits: SurfaceMeshLimits,
    ) -> Result<Self, MeshError> {
        let count = checked_product(dims)?;
        check_lattice_budget(dims, count, limits)?;
        Ok(Self {
            origin: minimum,
            dims,
            values: vec![-f64::from(svc_volume::DEFAULT_DENSITY_MAGNITUDE); count],
            materials: vec![0; count],
            explicit: false,
        })
    }

    /// Record one voxel. `density` is the signed density (negative inside);
    /// solidity is authoritative and its magnitude places the surface.
    pub(super) fn set_voxel(&mut self, global: [i64; 3], material: Option<u16>, density: f32) {
        let Some(index) = self.global_index(global) else {
            return;
        };
        let magnitude = f64::from(density.abs()).max(MINIMUM_MAGNITUDE);
        match material {
            Some(slot) => {
                self.values[index] = magnitude;
                self.materials[index] = slot;
            }
            None => self.values[index] = -magnitude,
        }
    }

    pub(super) fn scalar(
        origin: [i64; 3],
        dims: [usize; 3],
        samples: impl ExactSizeIterator<Item = f32>,
        materials: Option<Vec<u16>>,
        isovalue: f32,
        limits: SurfaceMeshLimits,
    ) -> Result<Self, MeshError> {
        if dims.iter().any(|dimension| *dimension < 2) {
            return Err(MeshError::InvalidSampleDimensions { dimensions: dims });
        }
        let count = checked_product(dims)?;
        if samples.len() != count {
            return Err(MeshError::InvalidSampleCount {
                expected: count,
                actual: samples.len(),
            });
        }
        check_lattice_budget(dims, count, limits)?;
        if !isovalue.is_finite() {
            return Err(MeshError::InvalidIsovalue);
        }
        let values = samples
            .map(|sample| {
                if !sample.is_finite() {
                    return Err(MeshError::InvalidScalarSample);
                }
                Ok(f64::from(isovalue) - f64::from(sample))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let materials = match materials {
            Some(materials) if materials.len() == count => materials,
            Some(materials) => {
                return Err(MeshError::InvalidSampleCount {
                    expected: count,
                    actual: materials.len(),
                })
            }
            None => vec![0; count],
        };
        Ok(Self {
            origin,
            dims,
            values,
            materials,
            explicit: true,
        })
    }

    fn global_index(&self, global: [i64; 3]) -> Option<usize> {
        let mut local = [0_usize; 3];
        for axis in 0..3 {
            let offset = global[axis].checked_sub(self.origin[axis])?;
            local[axis] = usize::try_from(offset).ok()?;
            if local[axis] >= self.dims[axis] {
                return None;
            }
        }
        Some(self.index(local))
    }

    fn index(&self, local: [usize; 3]) -> usize {
        (local[2] * self.dims[1] + local[1]) * self.dims[0] + local[0]
    }

    fn global(&self, local: [usize; 3]) -> [i64; 3] {
        std::array::from_fn(|axis| self.origin[axis] + local[axis] as i64)
    }

    fn cell_count(&self) -> u64 {
        ((self.dims[0] - 1) * (self.dims[1] - 1) * (self.dims[2] - 1)) as u64
    }
}

/// The smallest magnitude a voxel sample keeps, so a crossing between two
/// zero densities still has a defined position.
const MINIMUM_MAGNITUDE: f64 = 1.0e-6;

fn check_lattice_budget(
    dims: [usize; 3],
    count: usize,
    limits: SurfaceMeshLimits,
) -> Result<(), MeshError> {
    let cells = checked_product([
        dims[0].saturating_sub(1),
        dims[1].saturating_sub(1),
        dims[2].saturating_sub(1),
    ])? as u64;
    if cells > limits.max_sampled_cells {
        return Err(MeshError::TooManySampledCells {
            cells,
            limit: limits.max_sampled_cells,
        });
    }
    let bytes = (count as u64)
        .checked_mul((std::mem::size_of::<f64>() + std::mem::size_of::<u16>()) as u64)
        .ok_or(MeshError::CoordinateRangeTooLarge)?;
    if bytes > limits.max_temporary_field_bytes {
        return Err(MeshError::TemporaryFieldTooLarge {
            bytes,
            limit: limits.max_temporary_field_bytes,
        });
    }
    Ok(())
}

/// Global sample bounds `[min, max)` that own primitives.
#[derive(Clone, Copy, Debug)]
pub(super) struct Owner {
    pub min: [i64; 3],
    pub max: [i64; 3],
}

impl Owner {
    fn contains(&self, point: [i64; 3]) -> bool {
        (0..3).all(|axis| point[axis] >= self.min[axis] && point[axis] < self.max[axis])
    }

    fn grown(self, by: i64) -> Self {
        Self {
            min: self.min.map(|value| value - by),
            max: self.max.map(|value| value + by),
        }
    }
}

/// Reconstructed triangles before render attributes are assembled.
#[derive(Debug, Default)]
pub(super) struct Reconstruction {
    /// Vertex positions in global lattice units: sample `c` sits at `c + 0.5`.
    pub positions: Vec<[f64; 3]>,
    /// Per-vertex Hermite (or interpolated gradient) normals.
    pub normals: Vec<[f64; 3]>,
    pub triangles: Vec<[u32; 3]>,
    pub slots: Vec<u16>,
    /// The global sample that owns each triangle.
    pub owners: Vec<[i64; 3]>,
    /// The polygon facing of each triangle (both triangles of a quad agree).
    pub directions: Vec<Direction6>,
    /// Ring triangles outside the owned region, kept only for normals.
    pub halo: Vec<bool>,
    pub rank_deficient: u32,
    pub fallbacks: u32,
    pub sampled_cells: u64,
}

const NO_VERTEX: u32 = u32::MAX;
const NOT_COMPUTED: u32 = u32::MAX - 1;

struct DualContouring<'a> {
    lattice: &'a Lattice,
    characters: Characters<'a>,
    cells: Vec<u32>,
    cell_dims: [usize; 3],
    out: Reconstruction,
    limits: SurfaceMeshLimits,
}

/// Dual-contour the lattice. Quads whose inside endpoint lies in `owner` are
/// kept; with `ring`, quads within one more sample are kept as halo.
pub(super) fn dual_contour(
    lattice: &Lattice,
    characters: Characters<'_>,
    owner: Owner,
    ring: bool,
    limits: SurfaceMeshLimits,
    out: &mut Reconstruction,
) -> Result<(), MeshError> {
    let cell_dims = lattice.dims.map(|dimension| dimension - 1);
    let mut contour = DualContouring {
        lattice,
        characters,
        cells: vec![NOT_COMPUTED; cell_dims[0] * cell_dims[1] * cell_dims[2]],
        cell_dims,
        out: std::mem::take(out),
        limits,
    };
    let emitted = if ring { owner.grown(1) } else { owner };
    let dims = lattice.dims;
    for axis in 0..3 {
        // Edge starts whose inside endpoint can lie in the emitted region.
        let mut low = [0_usize; 3];
        let mut high = [0_usize; 3];
        for other in 0..3 {
            let reach = i64::from(other == axis);
            let start = emitted.min[other] - reach - lattice.origin[other];
            let end = emitted.max[other] - lattice.origin[other];
            let limit = (dims[other] - usize::from(other == axis)) as i64;
            low[other] = start.clamp(0, limit) as usize;
            high[other] = end.clamp(0, limit) as usize;
        }
        for z in low[2]..high[2] {
            for y in low[1]..high[1] {
                for x in low[0]..high[0] {
                    contour.edge(axis, [x, y, z], owner, emitted)?;
                }
            }
        }
    }
    contour.out.sampled_cells = lattice.cell_count();
    *out = contour.out;
    Ok(())
}

impl DualContouring<'_> {
    fn edge(
        &mut self,
        axis: usize,
        start: [usize; 3],
        owner: Owner,
        emitted: Owner,
    ) -> Result<(), MeshError> {
        let lattice = self.lattice;
        let mut end = start;
        end[axis] += 1;
        let start_index = lattice.index(start);
        let end_index = lattice.index(end);
        let start_inside = lattice.values[start_index] > 0.0;
        if start_inside == (lattice.values[end_index] > 0.0) {
            return Ok(());
        }
        let (inside_local, inside_index) = if start_inside {
            (start, start_index)
        } else {
            (end, end_index)
        };
        let inside = lattice.global(inside_local);
        if !emitted.contains(inside) {
            return Ok(());
        }
        let slot = lattice.materials[inside_index];
        if !lattice.explicit && self.characters.of(slot).mode != SurfaceMode::DualContouring {
            return Ok(());
        }
        let Some(cells) = incident_cells(axis, start, self.cell_dims) else {
            // Only a scalar domain's own boundary lacks incident cells: an
            // honest open boundary rather than an invented cap.
            debug_assert!(lattice.explicit, "voxel lattices carry a full halo");
            return Ok(());
        };
        let mut vertices = [0_u32; 4];
        for (slot_index, cell) in cells.into_iter().enumerate() {
            vertices[slot_index] = self.vertex(cell)?;
        }
        if lattice.explicit {
            let quads = self.out.triangles.len() as u64 / 2 + 1;
            if quads > self.limits.max_source_faces {
                return Err(MeshError::TooManyFaces {
                    faces: quads,
                    limit: self.limits.max_source_faces,
                });
            }
        }
        check_output_growth(
            self.out.positions.len() as u64,
            self.out.triangles.len() as u64 * 3,
            0,
            6,
            self.limits,
        )?;
        // `incident_cells` enumerates a right-handed loop facing the positive
        // edge axis, which is outward exactly when the edge exits the solid.
        let mut order = [0_usize, 1, 2, 3];
        if !start_inside {
            order.reverse();
        }
        let p = |index: usize| self.out.positions[vertices[order[index]] as usize];
        let triangles = if squared_distance(p(0), p(2)) <= squared_distance(p(1), p(3)) {
            [[0, 1, 2], [0, 2, 3]]
        } else {
            [[0, 1, 3], [1, 2, 3]]
        };
        let mut facing = [0.0; 3];
        for triangle in triangles {
            facing = add(
                facing,
                cross(
                    sub(p(triangle[1]), p(triangle[0])),
                    sub(p(triangle[2]), p(triangle[0])),
                ),
            );
        }
        let mut outward = [0.0; 3];
        outward[axis] = if start_inside { 1.0 } else { -1.0 };
        let direction = dominant_direction(facing, outward);
        let halo = !owner.contains(inside);
        for triangle in triangles {
            self.out
                .triangles
                .push(triangle.map(|index| vertices[order[index]]));
            self.out.slots.push(slot);
            self.out.owners.push(inside);
            self.out.directions.push(direction);
            self.out.halo.push(halo);
        }
        Ok(())
    }

    fn vertex(&mut self, cell: [usize; 3]) -> Result<u32, MeshError> {
        let cell_index = (cell[2] * self.cell_dims[1] + cell[1]) * self.cell_dims[0] + cell[0];
        let cached = self.cells[cell_index];
        if cached != NOT_COMPUTED {
            debug_assert_ne!(cached, NO_VERTEX, "a crossing edge's cells are active");
            return Ok(cached);
        }
        let lattice = self.lattice;
        let corners: [usize; 8] =
            CORNERS.map(|corner| lattice.index(std::array::from_fn(|a| cell[a] + corner[a])));
        let values = corners.map(|index| lattice.values[index]);
        let inside = values.map(|value| value > 0.0);
        let surfaces = std::array::from_fn::<_, 8, _>(|corner| {
            inside[corner].then(|| self.characters.of(lattice.materials[corners[corner]]))
        });
        let mut placement_rank = 0_u8;
        let mut roughness = f32::INFINITY;
        for surface in surfaces.iter().flatten() {
            placement_rank = placement_rank.max(sharpness(*surface));
            let corner_roughness = if surface.mode == SurfaceMode::GreedyCubes {
                0.0
            } else {
                surface.character.roughness
            };
            roughness = roughness.min(corner_roughness);
        }
        let global_cell = lattice.global(cell);
        let base = std::array::from_fn::<_, 3, _>(|axis| global_cell[axis] as f64);
        let mut samples = Vec::with_capacity(12);
        for (edge, &(a, b)) in EDGES.iter().enumerate() {
            if inside[a] == inside[b] {
                continue;
            }
            let t = values[a] / (values[a] - values[b]);
            let local = edge_local_point(edge, t);
            let point = std::array::from_fn(|axis| base[axis] + local[axis] + 0.5);
            let inside_corner = if inside[a] { a } else { b };
            let snapped = surfaces[inside_corner].is_some_and(|surface| sharpness(surface) >= 2);
            let normal = if snapped {
                let mut direction = [0.0; 3];
                for axis in 0..3 {
                    direction[axis] = CORNERS[b][axis] as f64 - CORNERS[a][axis] as f64;
                }
                if inside[a] {
                    direction
                } else {
                    scale(direction, -1.0)
                }
            } else {
                outward_normal(
                    trilinear_gradient(values, local),
                    sub(point, add(base, [1.0; 3])),
                )
            };
            samples.push((point, normal));
        }
        if samples.is_empty() {
            self.cells[cell_index] = NO_VERTEX;
            return Ok(NO_VERTEX);
        }
        let (mut position, rank_deficient, fallback) = if placement_rank == 0 {
            (mass_point(&samples), false, false)
        } else {
            solve_qef(base, &samples, lattice.explicit)
        };
        self.out.rank_deficient = self
            .out
            .rank_deficient
            .saturating_add(u32::from(rank_deficient));
        self.out.fallbacks = self.out.fallbacks.saturating_add(u32::from(fallback));
        if roughness.is_finite() && roughness > 0.0 {
            let jitter = cell_jitter(global_cell);
            for axis in 0..3 {
                position[axis] = (position[axis] + f64::from(roughness) * jitter[axis])
                    .clamp(base[axis] + 0.5, base[axis] + 1.5);
            }
        }
        let normal = normalize_or(
            samples
                .iter()
                .fold([0.0; 3], |sum, (_, normal)| add(sum, *normal)),
            [0.0, 1.0, 0.0],
        );
        let next = self.out.positions.len() as u64 + 1;
        if next > u64::from(self.limits.max_vertices) {
            return Err(MeshError::TooManyVertices { vertices: next });
        }
        let index = self.out.positions.len() as u32;
        self.out.positions.push(position);
        self.out.normals.push(normal);
        self.cells[cell_index] = index;
        Ok(index)
    }
}

/// Deterministic per-cell displacement in `[-1, 1]` on each axis.
fn cell_jitter(cell: [i64; 3]) -> [f64; 3] {
    let mut state = 0x9e37_79b9_7f4a_7c15_u64;
    for value in cell {
        state ^= value as u64;
        state = splitmix(state);
    }
    std::array::from_fn(|_| {
        state = splitmix(state);
        (state >> 11) as f64 / (1_u64 << 53) as f64 * 2.0 - 1.0
    })
}

fn splitmix(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

/// The four cells around an edge as a right-handed loop facing the positive
/// edge axis, or `None` when one lies outside the lattice.
fn incident_cells(axis: usize, edge: [usize; 3], cell_dims: [usize; 3]) -> Option<[[usize; 3]; 4]> {
    let (u, v) = match axis {
        0 => (1, 2),
        1 => (2, 0),
        _ => (0, 1),
    };
    if edge[u] == 0 || edge[v] == 0 || edge[axis] >= cell_dims[axis] {
        return None;
    }
    if edge[u] > cell_dims[u] || edge[v] > cell_dims[v] {
        return None;
    }
    let at = |du: usize, dv: usize| {
        let mut cell = edge;
        cell[u] = edge[u] - 1 + du;
        cell[v] = edge[v] - 1 + dv;
        cell
    };
    let cells = [at(0, 0), at(1, 0), at(1, 1), at(0, 1)];
    cells
        .iter()
        .all(|cell| (0..3).all(|a| cell[a] < cell_dims[a]))
        .then_some(cells)
}

/// March the lattice's cells whose majority material is marched and whose
/// first inside corner lies in `owner`.
pub(super) fn march(
    lattice: &Lattice,
    characters: Characters<'_>,
    owner: Owner,
    limits: SurfaceMeshLimits,
    out: &mut Reconstruction,
) -> Result<(), MeshError> {
    let cell_dims = lattice.dims.map(|dimension| dimension - 1);
    for z in 0..cell_dims[2] {
        for y in 0..cell_dims[1] {
            for x in 0..cell_dims[0] {
                march_cell(lattice, characters, owner, [x, y, z], limits, out)?;
            }
        }
    }
    out.sampled_cells = lattice.cell_count();
    Ok(())
}

fn march_cell(
    lattice: &Lattice,
    characters: Characters<'_>,
    owner: Owner,
    cell: [usize; 3],
    limits: SurfaceMeshLimits,
    out: &mut Reconstruction,
) -> Result<(), MeshError> {
    let corners: [usize; 8] =
        CORNERS.map(|corner| lattice.index(std::array::from_fn(|a| cell[a] + corner[a])));
    let values = corners.map(|index| lattice.values[index]);
    let inside = values.map(|value| value > 0.0);
    if inside.iter().all(|value| *value) || inside.iter().all(|value| !*value) {
        return Ok(());
    }
    let global_cell = lattice.global(cell);
    let owner_coordinate = (0..8)
        .filter(|corner| inside[*corner])
        .map(|corner| std::array::from_fn(|a| global_cell[a] + CORNERS[corner][a] as i64))
        .min()
        .expect("active cell has an inside corner");
    if !owner.contains(owner_coordinate) {
        return Ok(());
    }
    let slot = majority_material(std::array::from_fn(|corner| {
        inside[corner].then_some(lattice.materials[corners[corner]])
    }))
    .expect("active cell has an inside corner");
    if !lattice.explicit && characters.of(slot).mode != SurfaceMode::MarchingCubes {
        return Ok(());
    }
    let crossings = EDGES.map(|(a, b)| inside[a] != inside[b]);
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
                let determinant = values[c0] * values[c2] - values[c1] * values[c3];
                let positive = if determinant.abs() > f64::EPSILON {
                    determinant > 0.0
                } else {
                    // Exactly tied saddles take a parity of the global face,
                    // so adjacent regions make the same decision.
                    let mut face_coordinate = global_cell;
                    face_coordinate[face.axis] += face.fixed as i64;
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
    let base = std::array::from_fn::<_, 3, _>(|axis| global_cell[axis] as f64);
    let edge_t = |edge: usize| {
        let (a, b) = EDGES[edge];
        values[a] / (values[a] - values[b])
    };
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
        check_output_growth(
            out.positions.len() as u64,
            out.triangles.len() as u64 * 3,
            loop_edges.len() as u64 + 1,
            loop_edges.len() as u64 * 3,
            limits,
        )?;
        let mut points = Vec::with_capacity(loop_edges.len());
        let mut normals = Vec::with_capacity(loop_edges.len());
        for edge in &loop_edges {
            let local = edge_local_point(*edge, edge_t(*edge));
            let point = std::array::from_fn(|axis| base[axis] + local[axis] + 0.5);
            normals.push(outward_normal(
                trilinear_gradient(values, local),
                sub(point, add(base, [1.0; 3])),
            ));
            points.push(point);
        }
        let centroid = scale(
            points.iter().fold([0.0; 3], |sum, point| add(sum, *point)),
            1.0 / points.len() as f64,
        );
        let centroid_normal = normalize_or(
            normals
                .iter()
                .fold([0.0; 3], |sum, normal| add(sum, *normal)),
            [0.0, 1.0, 0.0],
        );
        if dot(polygon_normal(&points), centroid_normal) < 0.0 {
            points.reverse();
            normals.reverse();
        }
        let direction = dominant_direction(polygon_normal(&points), centroid_normal);
        let first = out.positions.len() as u32;
        out.positions.push(centroid);
        out.normals.push(centroid_normal);
        out.positions.extend(points.iter().copied());
        out.normals.extend(normals);
        for index in 0..points.len() {
            out.triangles.push([
                first,
                first + 1 + index as u32,
                first + 1 + ((index + 1) % points.len()) as u32,
            ]);
            out.slots.push(slot);
            out.owners.push(owner_coordinate);
            out.directions.push(direction);
            out.halo.push(false);
        }
    }
    Ok(())
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

/// The cube face whose axis best matches `facing`; `fallback` breaks a
/// degenerate polygon.
fn dominant_direction(facing: [f64; 3], fallback: [f64; 3]) -> Direction6 {
    let facing = if dot(facing, facing) > 1.0e-24 {
        facing
    } else {
        fallback
    };
    let mut axis = 0;
    for candidate in 1..3 {
        if facing[candidate].abs() > facing[axis].abs() {
            axis = candidate;
        }
    }
    let positive = facing[axis] >= 0.0;
    match (axis, positive) {
        (0, true) => Direction6::PosX,
        (0, false) => Direction6::NegX,
        (1, true) => Direction6::PosY,
        (1, false) => Direction6::NegY,
        (_, true) => Direction6::PosZ,
        (_, false) => Direction6::NegZ,
    }
}

/// Assemble render attributes for reconstructed voxel geometry: one group
/// per material slot and box-projection face, crease-angle normals, and
/// tile coordinates continuous across regions.
pub(super) fn voxel_payload(
    reconstruction: Reconstruction,
    characters: Characters<'_>,
    cell_size: f64,
    pivot: [f64; 3],
    limits: SurfaceMeshLimits,
) -> Result<MeshPayload, MeshError> {
    let mut lanes = BTreeMap::<(u16, Direction6), Vec<usize>>::new();
    for (triangle, (&slot, &direction)) in reconstruction
        .slots
        .iter()
        .zip(&reconstruction.directions)
        .enumerate()
    {
        if !reconstruction.halo[triangle] {
            lanes.entry((slot, direction)).or_default().push(triangle);
        }
    }
    if lanes.len() as u64 > u64::from(limits.max_material_partitions) {
        return Err(MeshError::TooManyMaterialPartitions {
            partitions: lanes.len() as u64,
            limit: limits.max_material_partitions,
        });
    }
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut tile_coordinates = Vec::new();
    let mut indices = Vec::new();
    let mut groups = Vec::with_capacity(lanes.len());
    let mut owners = Vec::new();
    let mut minimum = [f32::INFINITY; 3];
    let mut maximum = [f32::NEG_INFINITY; 3];
    let mut emitted = BTreeMap::<(u32, [u64; 3]), u32>::new();
    let mut mode = None;
    for ((slot, direction), triangles) in lanes {
        let surface = characters.of(slot);
        mode.get_or_insert(surface.mode);
        let cosine = f64::from(surface.character.crease_angle_degrees)
            .to_radians()
            .cos();
        let start = indices.len() as u32;
        emitted.clear();
        let basis = voxel_surface_texture_basis(direction);
        for triangle in triangles {
            let source = reconstruction.triangles[triangle];
            let corners = source.map(|index| reconstruction.positions[index as usize]);
            let face = normalize_or(
                cross(sub(corners[1], corners[0]), sub(corners[2], corners[0])),
                reconstruction.normals[source[0] as usize],
            );
            for vertex in source {
                let hermite = reconstruction.normals[vertex as usize];
                let normal = if surface.character.crease_angle_degrees >= 180.0
                    || dot(face, hermite) >= cosine
                {
                    hermite
                } else {
                    face
                };
                let key = (vertex, normal.map(f64::to_bits));
                let index = match emitted.get(&key) {
                    Some(index) => *index,
                    None => {
                        let index = (positions.len() / 3) as u32;
                        let point = reconstruction.positions[vertex as usize];
                        for axis in 0..3 {
                            let value = (point[axis] - pivot[axis]) * cell_size;
                            let rendered = value as f32;
                            let rendered_normal = normal[axis] as f32;
                            if !value.is_finite()
                                || !rendered.is_finite()
                                || !rendered_normal.is_finite()
                            {
                                return Err(MeshError::PositionOutOfRange);
                            }
                            minimum[axis] = minimum[axis].min(rendered);
                            maximum[axis] = maximum[axis].max(rendered);
                            positions.push(rendered);
                            normals.push(rendered_normal);
                        }
                        let project = |axis: super::texture_mapping::SignedTextureAxis| {
                            (point[axis.axis().index()] * f64::from(axis.sign())) as f32
                        };
                        tile_coordinates.push(project(basis.u));
                        tile_coordinates.push(project(basis.v));
                        emitted.insert(key, index);
                        index
                    }
                };
                indices.push(index);
            }
            owners.push(reconstruction.owners[triangle]);
        }
        groups.push(MeshGroup {
            state: 0,
            material_slot: slot,
            direction: Some(direction),
            surface_mode: surface.mode,
            start,
            count: indices.len() as u32 - start,
        });
    }
    let vertices = (positions.len() / 3) as u64;
    if vertices > u64::from(limits.max_vertices) {
        return Err(MeshError::TooManyVertices { vertices });
    }
    if indices.len() as u64 > u64::from(limits.max_indices) {
        return Err(MeshError::TooManyIndices {
            indices: indices.len() as u64,
            limit: limits.max_indices,
        });
    }
    let mode = mode.unwrap_or(characters.default_mode);
    let triangle_count = (indices.len() / 3) as u32;
    Ok(MeshPayload {
        surface_mode: mode,
        bounds: if positions.is_empty() {
            MeshBounds {
                min: [0.0; 3],
                max: [0.0; 3],
            }
        } else {
            MeshBounds {
                min: minimum,
                max: maximum,
            }
        },
        positions,
        normals,
        tile_coordinates,
        indices,
        groups,
        triangle_owners: owners,
        stats: MeshStats {
            surface_mode: mode,
            vertices: vertices as u32,
            indices: triangle_count * 3,
            triangles: triangle_count,
            quads: if mode == SurfaceMode::DualContouring {
                triangle_count / 2
            } else {
                0
            },
            faces_emitted: triangle_count,
            source_faces: 0,
            faces_culled: 0,
            sampled_cells: reconstruction.sampled_cells,
            qef_rank_deficient: reconstruction.rank_deficient,
            qef_fallbacks: reconstruction.fallbacks,
        },
    })
}

/// Solve the bounded QEF used by every Sharp and Blocky cell. Scalar data
/// keeps the mass point when the minimizer escapes its cell: clamping an
/// escaped fit to a sampled-domain edge would collapse the surface there,
/// while the Hermite mass point remains a local crossing-derived point.
fn solve_qef(
    cell: [f64; 3],
    samples: &[([f64; 3], [f64; 3])],
    mass_point_on_escape: bool,
) -> ([f64; 3], bool, bool) {
    let mass_point = mass_point(samples);
    let mut ata = [[0.0_f64; 3]; 3];
    let mut rhs = [0.0_f64; 3];
    for (point, normal) in samples {
        let relative = sub(*point, mass_point);
        let projected = dot(*normal, relative);
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
            offset = add(offset, scale(vector, dot(vector, rhs) / eigenvalues[axis]));
        }
    }
    let candidate = add(mass_point, offset);
    let fallback = rank == 0 || !candidate.iter().all(|value| value.is_finite());
    let candidate = if fallback { mass_point } else { candidate };
    let minimum = add(cell, [0.5; 3]);
    let maximum = add(cell, [1.5; 3]);
    if mass_point_on_escape
        && candidate
            .iter()
            .enumerate()
            .any(|(axis, value)| *value < minimum[axis] || *value > maximum[axis])
    {
        return (mass_point, rank < 3, true);
    }
    (
        std::array::from_fn(|axis| candidate[axis].clamp(minimum[axis], maximum[axis])),
        rank < 3,
        fallback,
    )
}

fn mass_point(samples: &[([f64; 3], [f64; 3])]) -> [f64; 3] {
    scale(
        samples
            .iter()
            .fold([0.0; 3], |sum, (point, _)| add(sum, *point)),
        1.0 / samples.len() as f64,
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

pub(super) fn check_output_growth(
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

fn edge_local_point(edge: usize, t: f64) -> [f64; 3] {
    let (a, b) = EDGES[edge];
    std::array::from_fn(|axis| {
        CORNERS[a][axis] as f64 + (CORNERS[b][axis] as f64 - CORNERS[a][axis] as f64) * t
    })
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
        scale(gradient, -1.0),
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

pub(super) fn checked_product(values: [usize; 3]) -> Result<usize, MeshError> {
    values
        .into_iter()
        .try_fold(1_usize, |total, value| total.checked_mul(value))
        .ok_or(MeshError::CoordinateRangeTooLarge)
}

fn add(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [left[0] + right[0], left[1] + right[1], left[2] + right[2]]
}

fn sub(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

fn scale(value: [f64; 3], factor: f64) -> [f64; 3] {
    [value[0] * factor, value[1] * factor, value[2] * factor]
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

fn normalize_or(value: [f64; 3], fallback: [f64; 3]) -> [f64; 3] {
    let length = dot(value, value).sqrt();
    if length.is_finite() && length > 1.0e-12 {
        scale(value, 1.0 / length)
    } else {
        fallback
    }
}

fn squared_distance(left: [f64; 3], right: [f64; 3]) -> f64 {
    let delta = sub(left, right);
    dot(delta, delta)
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
            let first = solve_qef([0.0; 3], &samples, false);
            let second = solve_qef([0.0; 3], &samples, false);
            assert_eq!(first, second);
            assert!(first.0.iter().all(|value| value.is_finite()));
            assert!(first.0.iter().all(|value| (0.5..=1.5).contains(value)));
        }
    }

    #[test]
    fn cell_jitter_is_a_pure_bounded_function_of_the_cell() {
        let jitter = cell_jitter([3, -7, 11]);
        assert_eq!(jitter, cell_jitter([3, -7, 11]));
        assert_ne!(jitter, cell_jitter([3, -7, 12]));
        assert!(jitter.iter().all(|value| (-1.0..=1.0).contains(value)));
    }
}
