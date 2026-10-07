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

use std::collections::{BTreeMap, BTreeSet, HashMap};

use core_space::Direction6;

use super::{
    terrain_layers::LayerField, texture_mapping::voxel_surface_texture_basis, MaterialSurface,
    MeshBounds, MeshError, MeshGroup, MeshPayload, MeshStats, SurfaceMaterials, SurfaceMeshLimits,
    SurfaceMode, VertexPlacement,
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
    /// The see-through samples, read as empty.
    see_through: Vec<usize>,
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
            see_through: Vec::new(),
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

    /// Make the solid samples of `slots` see-through: they read as empty at
    /// the same magnitude, so other materials meet them as they meet air.
    /// Returns the slots present.
    pub(super) fn set_see_through(&mut self, slots: &BTreeSet<u16>) -> BTreeSet<u16> {
        let mut present = BTreeSet::new();
        if slots.is_empty() {
            return present;
        }
        for index in 0..self.values.len() {
            let slot = self.materials[index];
            if self.values[index] > 0.0 && slots.contains(&slot) {
                self.values[index] = -self.values[index];
                self.see_through.push(index);
                present.insert(slot);
            }
        }
        present
    }

    /// The lattice at half the resolution: each sample stands for a 2×2×2
    /// block, solid only where the whole block is, at the block's smallest
    /// value in coarse units and with its majority material. The origin and
    /// dimensions are even.
    pub(super) fn coarsened(&self, limits: SurfaceMeshLimits) -> Result<Self, MeshError> {
        debug_assert!(self.origin.iter().all(|value| value % 2 == 0));
        debug_assert!(self.dims.iter().all(|value| value % 2 == 0));
        let dims = self.dims.map(|value| value / 2);
        let mut coarse = Self::voxels(self.origin.map(|value| value / 2), dims, limits)?;
        for z in 0..dims[2] {
            for y in 0..dims[1] {
                for x in 0..dims[0] {
                    let block = CORNERS.map(|corner| {
                        self.index([2 * x + corner[0], 2 * y + corner[1], 2 * z + corner[2]])
                    });
                    let value = block
                        .iter()
                        .map(|index| self.values[*index])
                        .fold(f64::INFINITY, f64::min);
                    let index = coarse.index([x, y, z]);
                    coarse.values[index] =
                        value.signum() * (value.abs() / 2.0).max(MINIMUM_MAGNITUDE);
                    if value > 0.0 {
                        coarse.materials[index] =
                            majority_material(block.map(|index| Some(self.materials[index])))
                                .expect("a solid block has materials");
                    }
                }
            }
        }
        Ok(coarse)
    }

    /// Flip the see-through samples of `slot` between solid and empty.
    pub(super) fn flip_see_through(&mut self, slot: u16) {
        for &index in &self.see_through {
            if self.materials[index] == slot {
                self.values[index] = -self.values[index];
            }
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
            see_through: Vec::new(),
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
    /// Triangles owned by a box of voxels from their owner, as many along
    /// each axis: those of merged block faces.
    pub owner_spans: BTreeMap<usize, [i64; 3]>,
    /// The owned region of a world chunk. Neighbouring chunks draw their own
    /// surfaces, so a block face touching the region's border is not merged:
    /// a neighbour's vertices along it meet no T-junction.
    pub chunk_region: Option<Owner>,
    pub rank_deficient: u32,
    pub fallbacks: u32,
    pub sampled_cells: u64,
}

const NO_VERTEX: u32 = u32::MAX;
const NOT_COMPUTED: u32 = u32::MAX - 1;

struct DualContouring<'a> {
    lattice: &'a Lattice,
    characters: Characters<'a>,
    kept: Option<u16>,
    cells: Vec<u32>,
    cell_dims: [usize; 3],
    out: Reconstruction,
    limits: SurfaceMeshLimits,
}

/// Dual-contour the lattice. Quads whose inside endpoint lies in `owner` are
/// kept; with `ring`, quads within one more sample are kept as halo. With
/// `kept`, only quads of that material are.
pub(super) fn dual_contour(
    lattice: &Lattice,
    characters: Characters<'_>,
    owner: Owner,
    ring: bool,
    kept: Option<u16>,
    limits: SurfaceMeshLimits,
    out: &mut Reconstruction,
) -> Result<(), MeshError> {
    let cell_dims = lattice.dims.map(|dimension| dimension - 1);
    let mut contour = DualContouring {
        lattice,
        characters,
        kept,
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
        let stride = lattice.index(std::array::from_fn(|other| usize::from(other == axis)));
        let inside = |index: usize| lattice.values[index] > 0.0;
        for z in low[2]..high[2] {
            for y in low[1]..high[1] {
                let row = lattice.index([low[0], y, z]);
                for x in low[0]..high[0] {
                    let start = row + (x - low[0]);
                    // Most edges do not cross the surface.
                    if inside(start) != inside(start + stride) {
                        contour.edge(axis, [x, y, z], owner, emitted)?;
                    }
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
        if self.kept.is_some_and(|kept| kept != slot)
            || (!lattice.explicit && self.characters.of(slot).mode != SurfaceMode::DualContouring)
        {
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
            // A Blocky or cube material's crossing sits on the face between
            // its sample and the outside one, whatever the densities: blocks
            // stay on the sample grid.
            let inside_corner = if inside[a] { a } else { b };
            let t = if surfaces[inside_corner].is_some_and(|surface| sharpness(surface) >= 2) {
                0.5
            } else {
                values[a] / (values[a] - values[b])
            };
            let local = edge_local_point(edge, t);
            let point = std::array::from_fn(|axis| base[axis] + local[axis] + 0.5);
            // A cell a Blocky or cube material wins is a block cell: every
            // crossing's normal snaps to its edge axis, so the shared vertex
            // stays on the block's planes and neighbouring materials meet
            // them there.
            let normal = if placement_rank >= 2 {
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
/// first inside corner lies in `owner`; with `kept`, only cells of that
/// majority material.
pub(super) fn march(
    lattice: &Lattice,
    characters: Characters<'_>,
    owner: Owner,
    kept: Option<u16>,
    limits: SurfaceMeshLimits,
    out: &mut Reconstruction,
) -> Result<(), MeshError> {
    let cell_dims = lattice.dims.map(|dimension| dimension - 1);
    for z in 0..cell_dims[2] {
        for y in 0..cell_dims[1] {
            for x in 0..cell_dims[0] {
                march_cell(lattice, characters, owner, [x, y, z], kept, limits, out)?;
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
    kept: Option<u16>,
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
    if !owner.contains(owner_coordinate)
        || kept.is_some_and(|kept| {
            (0..8).all(|corner| !inside[corner] || lattice.materials[corners[corner]] != kept)
        })
    {
        return Ok(());
    }
    let slot = majority_material(std::array::from_fn(|corner| {
        inside[corner].then_some(lattice.materials[corners[corner]])
    }))
    .expect("active cell has an inside corner");
    if kept.is_some_and(|kept| kept != slot)
        || (!lattice.explicit && characters.of(slot).mode != SurfaceMode::MarchingCubes)
    {
        return Ok(());
    }
    let crossings = EDGES.map(|(a, b)| inside[a] != inside[b]);
    let mut adjacency = [Adjacent::default(); 12];
    for face in FACES {
        let mut crossed = [0_usize; 4];
        let mut count = 0;
        for edge in face.edges {
            if crossings[edge] {
                crossed[count] = edge;
                count += 1;
            }
        }
        match &crossed[..count] {
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
        let mut loop_edges = [0_usize; 12];
        let mut length = 0;
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
            loop_edges[length] = current;
            length += 1;
            let Adjacent {
                neighbours,
                count: 2,
            } = adjacency[current]
            else {
                return Err(MeshError::CoordinateRangeTooLarge);
            };
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
        if length < 3 {
            return Err(MeshError::CoordinateRangeTooLarge);
        }
        check_output_growth(
            out.positions.len() as u64,
            out.triangles.len() as u64 * 3,
            length as u64 + 1,
            length as u64 * 3,
            limits,
        )?;
        let mut point_storage = [[0.0; 3]; 12];
        let mut normal_storage = [[0.0; 3]; 12];
        let points = &mut point_storage[..length];
        let normals = &mut normal_storage[..length];
        for (index, edge) in loop_edges[..length].iter().enumerate() {
            let local = edge_local_point(*edge, edge_t(*edge));
            let point = std::array::from_fn(|axis| base[axis] + local[axis] + 0.5);
            normals[index] = outward_normal(
                trilinear_gradient(values, local),
                sub(point, add(base, [1.0; 3])),
            );
            points[index] = point;
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
        if dot(polygon_normal(points), centroid_normal) < 0.0 {
            points.reverse();
            normals.reverse();
        }
        let direction = dominant_direction(polygon_normal(points), centroid_normal);
        let first = out.positions.len() as u32;
        out.positions.push(centroid);
        out.normals.push(centroid_normal);
        out.positions.extend_from_slice(points);
        out.normals.extend_from_slice(normals);
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

/// Hang a skirt from every open edge of the owned surface, reaching `depth`
/// lattice units into the solid along its vertices' normals and as far out
/// across the edge, so a coarse chunk hides the gap against a finer
/// neighbour's surface: a step where the two meet at different heights, and
/// a gap where their edges stop short of each other (each surface's border
/// vertices lie anywhere within its border cells). An edge is open when no
/// other triangle shares its end positions (marched polygons do not share
/// vertex indices). The skirt faces up and out, toward the neighbour; its
/// vertices keep the edge's normals and it takes the triangle's material,
/// face and owner.
pub(super) fn add_skirts(reconstruction: &mut Reconstruction, depth: f64) {
    let key = |vertex: u32| reconstruction.positions[vertex as usize].map(f64::to_bits);
    // Undirected edge to (its last directed use, how many triangles use it).
    let mut edges = HashMap::<_, ((usize, u32, u32), u32)>::new();
    for (triangle, corners) in reconstruction.triangles.iter().enumerate() {
        if reconstruction.halo[triangle] {
            continue;
        }
        for k in 0..3 {
            let (a, b) = (corners[k], corners[(k + 1) % 3]);
            let (ka, kb) = (key(a), key(b));
            let use_of = edges
                .entry((ka.min(kb), ka.max(kb)))
                .or_insert(((triangle, a, b), 0));
            use_of.0 = (triangle, a, b);
            use_of.1 += 1;
        }
    }
    let mut open: Vec<_> = edges
        .into_values()
        .filter(|(_, uses)| *uses == 1)
        .map(|(edge, _)| edge)
        .collect();
    open.sort_unstable();
    // Each open vertex's outward direction: across its open edges, away
    // from their triangles, in their planes.
    let mut outward = BTreeMap::<u32, [f64; 3]>::new();
    for &(triangle, a, b) in &open {
        let corners = reconstruction.triangles[triangle];
        let [pa, pb, pc] = corners.map(|vertex| reconstruction.positions[vertex as usize]);
        let edge = sub(
            reconstruction.positions[b as usize],
            reconstruction.positions[a as usize],
        );
        let away = normalize_or(cross(edge, cross(sub(pb, pa), sub(pc, pa))), [0.0; 3]);
        for vertex in [a, b] {
            let sum = outward.entry(vertex).or_insert([0.0; 3]);
            *sum = add(*sum, away);
        }
    }
    let mut lowered = HashMap::<u32, u32>::new();
    for (triangle, a, b) in open {
        let [below_a, below_b] = [a, b].map(|vertex| {
            *lowered.entry(vertex).or_insert_with(|| {
                let index = reconstruction.positions.len() as u32;
                let normal = reconstruction.normals[vertex as usize];
                let out = normalize_or(outward[&vertex], [0.0; 3]);
                let position = reconstruction.positions[vertex as usize];
                reconstruction
                    .positions
                    .push(add(position, scale(sub(out, normal), depth)));
                reconstruction.normals.push(normal);
                index
            })
        });
        let (slot, owner, direction) = (
            reconstruction.slots[triangle],
            reconstruction.owners[triangle],
            reconstruction.directions[triangle],
        );
        // The triangle runs a -> b, so b -> a -> below faces up and away
        // from it.
        for corners in [[b, a, below_a], [b, below_a, below_b]] {
            reconstruction.triangles.push(corners);
            reconstruction.slots.push(slot);
            reconstruction.owners.push(owner);
            reconstruction.directions.push(direction);
            reconstruction.halo.push(false);
        }
    }
}

/// Merge the unit faces of exact blocks (dual-contoured Blocky materials
/// without roughness) lying in one plane into rectangles, as greedy cubes
/// merge cube faces, without moving any surface. A face merges only when
/// every corner it uses is used by nothing but such faces of its plane, and
/// lies inside a world chunk's region, so a neighbouring surface in another
/// plane, of another character or in another chunk keeps every vertex it
/// shares with the blocks (no T-junction meets it) and a merged corner keeps
/// its snapped normal. A merged triangle is owned by the box of voxels under
/// it.
pub(super) fn merge_block_faces(reconstruction: &mut Reconstruction, characters: Characters<'_>) {
    // (direction, axis, plane coordinate): faces of one plane facing one way.
    type Plane = (Direction6, usize, i64);
    let exact_block = |slot: u16| characters.of(slot).draws_exact_blocks();
    if !characters
        .materials
        .entries()
        .iter()
        .any(|(_, surface)| surface.draws_exact_blocks())
    {
        return;
    }
    let key = |vertex: u32| reconstruction.positions[vertex as usize].map(f64::to_bits);
    // A face: two consecutive triangles of one dual-contoured edge, a unit
    // square on the voxel grid.
    struct Face {
        first: usize,
        slot: u16,
        plane: Plane,
        cell: [i64; 2],
    }
    let mut faces = Vec::new();
    let mut triangle = 0;
    let count = reconstruction.triangles.len();
    while triangle + 1 < count {
        let (a, b) = (triangle, triangle + 1);
        let face = (|| {
            let slot = reconstruction.slots[a];
            if reconstruction.halo[a]
                || reconstruction.halo[b]
                || reconstruction.slots[b] != slot
                || reconstruction.owners[a] != reconstruction.owners[b]
                || reconstruction.directions[a] != reconstruction.directions[b]
                || !exact_block(slot)
            {
                return None;
            }
            let mut corners: Vec<u32> = reconstruction.triangles[a]
                .iter()
                .chain(&reconstruction.triangles[b])
                .copied()
                .collect();
            corners.sort_unstable();
            corners.dedup();
            if corners.len() != 4 {
                return None;
            }
            let points = corners
                .iter()
                .map(|corner| reconstruction.positions[*corner as usize]);
            let direction = reconstruction.directions[a];
            let axis = direction.axis().index();
            let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
            let mut low = [f64::INFINITY; 3];
            let mut high = [f64::NEG_INFINITY; 3];
            for point in points {
                if point.iter().any(|value| value.fract() != 0.0) {
                    return None;
                }
                for k in 0..3 {
                    low[k] = low[k].min(point[k]);
                    high[k] = high[k].max(point[k]);
                }
            }
            (low[axis] == high[axis] && high[u] - low[u] == 1.0 && high[v] - low[v] == 1.0).then(
                || Face {
                    first: a,
                    slot,
                    plane: (direction, axis, low[axis] as i64),
                    cell: [low[u] as i64, low[v] as i64],
                },
            )
        })();
        match face {
            Some(face) => {
                faces.push(face);
                triangle += 2;
            }
            None => triangle += 1,
        }
    }
    if faces.is_empty() {
        return;
    }
    // The plane each corner position is used in, if only faces of one plane
    // use it.
    let mut face_of = vec![None; count];
    for (index, face) in faces.iter().enumerate() {
        face_of[face.first] = Some(index);
        face_of[face.first + 1] = Some(index);
    }
    let mut corner_planes = HashMap::<[u64; 3], Option<Plane>>::new();
    for (triangle, corners) in reconstruction.triangles.iter().enumerate() {
        if reconstruction.halo[triangle] {
            continue;
        }
        let plane = face_of[triangle].map(|face| faces[face].plane);
        for corner in corners {
            corner_planes
                .entry(key(*corner))
                .and_modify(|seen| {
                    if *seen != plane {
                        *seen = None;
                    }
                })
                .or_insert(plane);
        }
    }
    // Mergeable faces by material and plane, keyed by cell.
    let mut planes = BTreeMap::<(u16, Plane), BTreeMap<[i64; 2], usize>>::new();
    for (index, face) in faces.iter().enumerate() {
        let corners = reconstruction.triangles[face.first]
            .iter()
            .chain(&reconstruction.triangles[face.first + 1]);
        let inside = |corner: &u32| {
            let point = reconstruction.positions[*corner as usize];
            reconstruction.chunk_region.is_none_or(|region| {
                (0..3).all(|k| point[k] > region.min[k] as f64 && point[k] < region.max[k] as f64)
            })
        };
        if corners
            .clone()
            .all(|corner| corner_planes[&key(*corner)] == Some(face.plane) && inside(corner))
        {
            planes
                .entry((face.slot, face.plane))
                .or_default()
                .insert(face.cell, index);
        }
    }
    for ((slot, (direction, axis, coordinate)), cells) in planes {
        let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
        let mut taken = BTreeSet::new();
        // Rows of cells by v, then u, as greedy cube faces merge.
        let mut order: Vec<[i64; 2]> = cells.keys().copied().collect();
        order.sort_unstable_by_key(|cell| (cell[1], cell[0]));
        for start in order {
            if taken.contains(&start) {
                continue;
            }
            let free = |cell: [i64; 2], taken: &BTreeSet<[i64; 2]>| {
                cells.contains_key(&cell) && !taken.contains(&cell)
            };
            let mut width = 1;
            while free([start[0] + width, start[1]], &taken) {
                width += 1;
            }
            let mut height = 1;
            while (0..width).all(|du| free([start[0] + du, start[1] + height], &taken)) {
                height += 1;
            }
            let members: Vec<usize> = (0..height)
                .flat_map(|dv| (0..width).map(move |du| [start[0] + du, start[1] + dv]))
                .map(|cell| {
                    taken.insert(cell);
                    cells[&cell]
                })
                .collect();
            if members.len() == 1 {
                continue;
            }
            // The rectangle's corners are corners of its corner faces.
            let point = |du: i64, dv: i64| {
                let mut point = [0.0; 3];
                point[axis] = coordinate as f64;
                point[u] = (start[0] + du) as f64;
                point[v] = (start[1] + dv) as f64;
                point
            };
            let vertex_at = |target: [f64; 3]| {
                members
                    .iter()
                    .flat_map(|member| {
                        let first = faces[*member].first;
                        reconstruction.triangles[first]
                            .into_iter()
                            .chain(reconstruction.triangles[first + 1])
                    })
                    .find(|corner| reconstruction.positions[*corner as usize] == target)
                    .expect("a rectangle corner is a corner of its corner face")
            };
            let rectangle = [
                vertex_at(point(0, 0)),
                vertex_at(point(width, 0)),
                vertex_at(point(width, height)),
                vertex_at(point(0, height)),
            ];
            // Face the way the merged faces do.
            let first = reconstruction.triangles[faces[members[0]].first];
            let facing = |corners: [u32; 3]| {
                let [p0, p1, p2] = corners.map(|corner| reconstruction.positions[corner as usize]);
                cross(sub(p1, p0), sub(p2, p0))[axis] > 0.0
            };
            let triangles = if facing([rectangle[0], rectangle[1], rectangle[2]]) == facing(first) {
                [
                    [rectangle[0], rectangle[1], rectangle[2]],
                    [rectangle[0], rectangle[2], rectangle[3]],
                ]
            } else {
                [
                    [rectangle[0], rectangle[2], rectangle[1]],
                    [rectangle[0], rectangle[3], rectangle[2]],
                ]
            };
            let mut low = [i64::MAX; 3];
            let mut high = [i64::MIN; 3];
            for member in &members {
                let first = faces[*member].first;
                let owner = reconstruction.owners[first];
                for k in 0..3 {
                    low[k] = low[k].min(owner[k]);
                    high[k] = high[k].max(owner[k]);
                }
                reconstruction.halo[first] = true;
                reconstruction.halo[first + 1] = true;
            }
            for corners in triangles {
                reconstruction.owner_spans.insert(
                    reconstruction.triangles.len(),
                    std::array::from_fn(|k| high[k] - low[k] + 1),
                );
                reconstruction.triangles.push(corners);
                reconstruction.slots.push(slot);
                reconstruction.owners.push(low);
                reconstruction.directions.push(direction);
                reconstruction.halo.push(false);
            }
        }
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
/// tile coordinates continuous across regions. A lattice unit is `scale`
/// voxels (2 for a coarse lattice); `pivot` and owners are in voxels.
#[allow(clippy::too_many_arguments, reason = "one payload")]
pub(super) fn voxel_payload(
    mut reconstruction: Reconstruction,
    characters: Characters<'_>,
    cell_size: f64,
    pivot: [f64; 3],
    scale: f64,
    limits: SurfaceMeshLimits,
    layers: Option<&LayerField<'_>>,
    occlusion: Option<&crate::occlusion::OcclusionField>,
) -> Result<MeshPayload, MeshError> {
    merge_block_faces(&mut reconstruction, characters);
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
    let mut layer_weights = Vec::new();
    let mut occlusions = Vec::new();
    let mut indices = Vec::new();
    let mut groups = Vec::with_capacity(lanes.len());
    let mut owners = Vec::new();
    let mut owner_spans = Vec::new();
    let mut minimum = [f32::INFINITY; 3];
    let mut maximum = [f32::NEG_INFINITY; 3];
    // The lane, emitted index and normal of each source vertex's first
    // emission. A vertex emitted again in the same lane with another normal
    // (across a crease) is found in `emitted`.
    let mut first = vec![(usize::MAX, 0_u32, [0_u64; 3]); reconstruction.positions.len()];
    let mut emitted = BTreeMap::<(u32, [u64; 3]), u32>::new();
    let mut mode = None;
    for (lane, ((slot, direction), triangles)) in lanes.into_iter().enumerate() {
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
                let bits = normal.map(f64::to_bits);
                let seen = &first[vertex as usize];
                let found = if seen.0 != lane {
                    None
                } else if seen.2 == bits {
                    Some(seen.1)
                } else {
                    emitted.get(&(vertex, bits)).copied()
                };
                let index = match found {
                    Some(index) => index,
                    None => {
                        let index = (positions.len() / 3) as u32;
                        let point =
                            reconstruction.positions[vertex as usize].map(|value| value * scale);
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
                        if let Some(field) = layers {
                            layer_weights.extend(field.weights_or_slot(point, slot));
                        }
                        if let Some(field) = occlusion {
                            occlusions.push(field.surface_occlusion(point, normal));
                        }
                        let seen = &mut first[vertex as usize];
                        if seen.0 == lane {
                            emitted.insert((vertex, bits), index);
                        } else {
                            *seen = (lane, index, bits);
                        }
                        index
                    }
                };
                indices.push(index);
            }
            owners.push(reconstruction.owners[triangle].map(|value| value * scale as i64));
            if !reconstruction.owner_spans.is_empty() {
                let span = reconstruction
                    .owner_spans
                    .get(&triangle)
                    .copied()
                    .unwrap_or([1; 3]);
                owner_spans.push(span.map(|value| (value * scale as i64) as u32));
            }
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
        layer_weights,
        occlusion: occlusions,
        indices,
        groups,
        triangle_owners: owners,
        triangle_owner_spans: owner_spans,
        distance_field: None,
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

/// The edges a marched cell's crossing edge connects to, ascending. A
/// crossing edge lies on two faces, each connecting it once; `count` keeps
/// counting past two so a malformed cell is detected.
#[derive(Clone, Copy, Default)]
struct Adjacent {
    neighbours: [usize; 2],
    count: usize,
}

impl Adjacent {
    fn add(&mut self, edge: usize) {
        if self.neighbours[..self.count.min(2)].contains(&edge) {
            return;
        }
        if self.count < 2 {
            self.neighbours[self.count] = edge;
            self.neighbours[..=self.count].sort_unstable();
        }
        self.count += 1;
    }
}

fn connect(adjacency: &mut [Adjacent; 12], left: usize, right: usize) {
    adjacency[left].add(right);
    adjacency[right].add(left);
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
