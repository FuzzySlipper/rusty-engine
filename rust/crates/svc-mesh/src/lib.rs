//! Deterministic greedy visible-surface voxel mesher → [`MeshPayload`].
//!
//! Every solid voxel contributes faces whose neighbour does not hide them;
//! internal faces and resident-neighbour seams are culled. Remaining coplanar
//! faces with the same material and normal are merged into deterministic
//! rectangles.
//! Positions, normals, tile coordinates, indices, material groups, and bounds
//! form a renderer-neutral mesh payload. Vertices are chunk-local; callers own
//! world placement and renderer integration.

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use core_space::{ChunkCoord, Direction6, LocalVoxelCoord, VoxelCoord, VoxelGridSpec};
use svc_spatial::VoxelWorld;
use svc_volume::VoxelChunk;
use texture_mapping::{project_voxel_surface_tile_point, VoxelTextureMappingError};

mod surface;
mod terrain_layers;
pub mod texture_mapping;

pub use terrain_layers::{
    TerrainLayers, MAX_TERRAIN_LAYERS, MAX_TERRAIN_LAYER_SLOTS, MAX_TERRAIN_TRANSITION_CELLS,
};

/// Renderer-neutral derived presentation selected for canonical voxel facts.
///
/// Omission at every public caller remains [`GreedyCubes`](Self::GreedyCubes).
/// A mode selects how voxels are drawn; it never changes voxel storage or
/// edit semantics.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SurfaceMode {
    #[default]
    GreedyCubes,
    MarchingCubes,
    DualContouring,
}

impl SurfaceMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GreedyCubes => "greedyCubes",
            Self::MarchingCubes => "marchingCubes",
            Self::DualContouring => "dualContouring",
        }
    }

    /// Whether this mode reconstructs a smooth surface from samples.
    pub const fn is_reconstructed(self) -> bool {
        !matches!(self, Self::GreedyCubes)
    }
}

/// How a reconstructed material places the vertex of each surface cell.
///
/// Where materials meet in one cell the sharper placement wins
/// (`Smooth` < `Sharp` < `Blocky` < a cube material).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum VertexPlacement {
    /// The mean of the cell's edge crossings: rounded, blob-like surfaces.
    Smooth,
    /// The QEF minimizer of the crossings and interpolated gradients: keeps
    /// corners and edges at any angle.
    #[default]
    Sharp,
    /// Blocks on the sample grid: the material's crossings sit on the face
    /// between samples whatever the densities, and every crossing normal of a
    /// cell it wins snaps to its edge axis, so a cube of material meshes as a
    /// cube and neighbouring materials meet its planes.
    Blocky,
}

/// Per-material character of a reconstructed surface.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceCharacter {
    pub placement: VertexPlacement,
    /// The largest angle between a facet and its vertex's interpolated
    /// normal at which the vertex is shaded smooth; beyond it the facet is
    /// shaded flat. 180 shades everything smooth, 0 everything flat.
    pub crease_angle_degrees: f32,
    /// Deterministic per-cell vertex displacement, as a fraction of a cell
    /// (0 to 0.5). The displacement depends only on the global cell, so
    /// independently meshed regions agree.
    pub roughness: f32,
}

impl Default for SurfaceCharacter {
    fn default() -> Self {
        Self {
            placement: VertexPlacement::Sharp,
            crease_angle_degrees: 180.0,
            roughness: 0.0,
        }
    }
}

impl SurfaceCharacter {
    pub fn validate(&self) -> Result<(), MeshError> {
        if !(0.0..=180.0).contains(&self.crease_angle_degrees)
            || !(0.0..=0.5).contains(&self.roughness)
        {
            return Err(MeshError::InvalidSurfaceCharacter);
        }
        Ok(())
    }
}

/// How one material slot is surfaced.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MaterialSurface {
    /// Voxel data only; scalar volumes are always dual contoured.
    pub mode: SurfaceMode,
    pub character: SurfaceCharacter,
}

/// Material slots whose surface differs from the default: the session mode
/// with the default character. Cheap to clone.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SurfaceMaterials(Arc<[(u16, MaterialSurface)]>);

impl SurfaceMaterials {
    pub fn new(
        entries: impl IntoIterator<Item = (u16, MaterialSurface)>,
    ) -> Result<Self, MeshError> {
        let mut entries: Vec<_> = entries.into_iter().collect();
        entries.sort_by_key(|(slot, _)| *slot);
        for window in entries.windows(2) {
            if window[0].0 == window[1].0 {
                return Err(MeshError::DuplicateMaterialSurface { slot: window[0].0 });
            }
        }
        for (_, surface) in &entries {
            surface.character.validate()?;
        }
        Ok(Self(entries.into()))
    }

    pub fn get(&self, slot: u16) -> Option<&MaterialSurface> {
        self.0
            .binary_search_by_key(&slot, |(entry, _)| *entry)
            .ok()
            .map(|index| &self.0[index].1)
    }

    pub fn entries(&self) -> &[(u16, MaterialSurface)] {
        &self.0
    }
}

/// Prospective work and retention ceilings for one derived mesh.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceMeshLimits {
    pub max_source_faces: u64,
    pub max_sampled_cells: u64,
    pub max_vertices: u32,
    pub max_indices: u32,
    pub max_temporary_field_bytes: u64,
    pub max_material_partitions: u32,
}

impl Default for SurfaceMeshLimits {
    fn default() -> Self {
        Self {
            max_source_faces: 2_000_000,
            max_sampled_cells: 4_000_000,
            max_vertices: 8_000_000,
            max_indices: 12_000_000,
            max_temporary_field_bytes: 256 * 1024 * 1024,
            max_material_partitions: 4_096,
        }
    }
}

/// The session mode, limits and per-material surfaces of one voxel mesh.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SurfaceMeshOptions {
    /// The mode of every material without its own surface.
    pub mode: SurfaceMode,
    pub limits: SurfaceMeshLimits,
    pub materials: SurfaceMaterials,
    /// Material slots that do not hide a neighbour's cube face, such as water
    /// or glass. Two voxels of one such slot still hide their shared face.
    pub non_occluding: BTreeSet<u16>,
    /// Give every vertex weights for these layers ([`MeshPayload::layer_weights`]).
    pub terrain_layers: Option<TerrainLayers>,
}

impl SurfaceMeshOptions {
    pub fn with_mode(mode: SurfaceMode) -> Self {
        Self {
            mode,
            ..Self::default()
        }
    }

    /// The surface of one material slot.
    pub fn surface(&self, slot: u16) -> MaterialSurface {
        self.materials
            .get(slot)
            .copied()
            .unwrap_or(MaterialSurface {
                mode: self.mode,
                character: SurfaceCharacter::default(),
            })
    }

    /// Whether any material may surface with `mode`.
    pub fn uses_mode(&self, mode: SurfaceMode) -> bool {
        self.mode == mode
            || self
                .materials
                .entries()
                .iter()
                .any(|(_, surface)| surface.mode == mode)
    }

    /// Whether a `neighbour` voxel hides the cube face a `slot` voxel shows it.
    fn hides(&self, slot: u16, neighbour: u16) -> bool {
        neighbour == slot || !self.non_occluding.contains(&neighbour)
    }

    /// Whether every material is drawn as cubes.
    pub fn all_greedy(&self) -> bool {
        !self.uses_mode(SurfaceMode::MarchingCubes) && !self.uses_mode(SurfaceMode::DualContouring)
    }

    fn characters(&self) -> surface::Characters<'_> {
        surface::Characters {
            materials: &self.materials,
            default_mode: self.mode,
        }
    }
}

/// One contiguous run of indices sharing a material slot: one draw range of
/// the mesh, bound to that slot's material.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeshGroup {
    pub state: u16,
    pub material_slot: u16,
    /// The cube face of greedy output, or the box-projection face of
    /// reconstructed output. Scalar volumes have none.
    pub direction: Option<Direction6>,
    /// How this run was surfaced.
    pub surface_mode: SurfaceMode,
    /// First index (into `indices`) of the run.
    pub start: u32,
    /// Number of indices in the run (a multiple of 3).
    pub count: u32,
}

/// Axis-aligned bounds of the mesh, in chunk-local space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeshBounds {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

/// Debug counters for the mesher.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MeshStats {
    pub surface_mode: SurfaceMode,
    pub vertices: u32,
    pub indices: u32,
    pub triangles: u32,
    /// Greedy output rectangles.
    pub quads: u32,
    /// Greedy output rectangles, retained as the historical emitted-face name.
    pub faces_emitted: u32,
    /// Visible unit faces before greedy merging.
    ///
    /// Runtime admission charges this value so compression never weakens the
    /// existing bounded-work contract.
    pub source_faces: u32,
    /// Faces culled because the neighbour was opaque (internal or resident border).
    pub faces_culled: u32,
    /// Scalar cells inspected by reconstructed modes. Greedy cubes report 0.
    pub sampled_cells: u64,
    /// Dual-contouring cells whose Hermite system did not have full rank.
    pub qef_rank_deficient: u32,
    /// Dual-contouring cells that used the deterministic mass-point fallback.
    pub qef_fallbacks: u32,
}

/// A renderable mesh for one chunk: separate `f32` attribute streams, a `u32`
/// index stream, material-slot groups, bounds, and stats (ADR 0007).
#[derive(Debug, Clone, PartialEq)]
pub struct MeshPayload {
    pub surface_mode: SurfaceMode,
    /// 3 `f32` per vertex (chunk-local).
    pub positions: Vec<f32>,
    /// 3 `f32` per vertex (outward face normal).
    pub normals: Vec<f32>,
    /// 2 signed `f32` cell-space coordinates per vertex. World chunks use
    /// absolute voxel coordinates; voxel objects use object-local coordinates.
    pub tile_coordinates: Vec<f32>,
    /// With [`SurfaceMeshOptions::terrain_layers`], 4 `f32` per vertex: each
    /// layer's weight, summing to 1. A reconstructed vertex blends the layers
    /// of the voxels around it; a cube face takes its own slot's layer.
    /// Empty otherwise.
    pub layer_weights: Vec<f32>,
    /// 3 `u32` per triangle.
    pub indices: Vec<u32>,
    /// Groups whose `count`s tile `indices`.
    pub groups: Vec<MeshGroup>,
    /// The voxel (or scalar sample) that owns each triangle, in absolute
    /// coordinates: the inside endpoint of a dual-contoured edge, the first
    /// inside corner of a marched cell, or the first cell of a cube face.
    /// Collision maps a reconstructed triangle to its voxel through it.
    pub triangle_owners: Vec<[i64; 3]>,
    pub bounds: MeshBounds,
    pub stats: MeshStats,
}

/// One local-space material cell accepted by the standalone object mesher.
///
/// The mesher deliberately owns no durable voxel-object schema. Asset admission
/// resolves that schema into this small service input before asking for a mesh.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeshVoxelCell {
    pub coordinate: [i64; 3],
    pub material_slot: u16,
}

/// A meshing failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeshError {
    StateRequiresGreedyCubes,
    InvalidSurfaceCharacter,
    /// A terrain layer set needs 1 to 16 distinct slots, each on a layer
    /// below 4, and a transition of 1 to 4 voxels.
    InvalidTerrainLayers,
    DuplicateMaterialSurface {
        slot: u16,
    },
    /// The chunk would emit more vertices than a `u32` index can address.
    TooManyVertices {
        vertices: u64,
    },
    TooManyFaces {
        faces: u64,
        limit: u64,
    },
    TooManySampledCells {
        cells: u64,
        limit: u64,
    },
    TooManyIndices {
        indices: u64,
        limit: u32,
    },
    TooManyMaterialPartitions {
        partitions: u64,
        limit: u32,
    },
    TemporaryFieldTooLarge {
        bytes: u64,
        limit: u64,
    },
    CoordinateRangeTooLarge,
    InvalidCellSize,
    InvalidPivot,
    InvalidSampleDimensions {
        dimensions: [usize; 3],
    },
    InvalidSampleCount {
        expected: usize,
        actual: usize,
    },
    InvalidScalarSample,
    InvalidScalarOrigin,
    InvalidIsovalue,
    DuplicateCell {
        coordinate: [i64; 3],
    },
    PositionOutOfRange,
    TextureMapping(VoxelTextureMappingError),
}

impl core::fmt::Display for MeshError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            MeshError::StateRequiresGreedyCubes => {
                write!(f, "cell orientation and variant require GreedyCubes")
            }
            MeshError::InvalidTerrainLayers => write!(
                f,
                "a terrain layer set needs 1 to {MAX_TERRAIN_LAYER_SLOTS} distinct material slots, each on a layer below {MAX_TERRAIN_LAYERS}, and a transition of 1 to {MAX_TERRAIN_TRANSITION_CELLS} voxels"
            ),
            MeshError::InvalidSurfaceCharacter => write!(
                f,
                "a surface character needs a crease angle of 0 to 180 degrees and a roughness of 0 to 0.5"
            ),
            MeshError::DuplicateMaterialSurface { slot } => {
                write!(f, "material slot {slot} has more than one surface")
            }
            MeshError::TooManyVertices { vertices } => {
                write!(
                    f,
                    "mesh would need {vertices} vertices, exceeding u32 index range"
                )
            }
            MeshError::TooManyFaces { faces, limit } => {
                write!(f, "mesh would emit {faces} faces; limit is {limit}")
            }
            MeshError::TooManySampledCells { cells, limit } => {
                write!(
                    f,
                    "surface extraction would sample {cells} cells; limit is {limit}"
                )
            }
            MeshError::TooManyIndices { indices, limit } => {
                write!(f, "mesh would need {indices} indices; limit is {limit}")
            }
            MeshError::TooManyMaterialPartitions { partitions, limit } => write!(
                f,
                "mesh would need {partitions} material partitions; limit is {limit}"
            ),
            MeshError::TemporaryFieldTooLarge { bytes, limit } => write!(
                f,
                "surface extraction would retain {bytes} temporary field bytes; limit is {limit}"
            ),
            MeshError::CoordinateRangeTooLarge => {
                write!(f, "surface extraction coordinate range is too large")
            }
            MeshError::InvalidCellSize => write!(f, "cell size must be finite and positive"),
            MeshError::InvalidPivot => write!(f, "pivot components must be finite"),
            MeshError::InvalidSampleDimensions { dimensions } => write!(
                f,
                "scalar sample dimensions must be at least 2 on every axis; got {dimensions:?}"
            ),
            MeshError::InvalidSampleCount { expected, actual } => write!(
                f,
                "scalar sample count is {actual}; dimensions require exactly {expected}"
            ),
            MeshError::InvalidScalarSample => write!(f, "scalar samples must be finite"),
            MeshError::InvalidScalarOrigin => write!(f, "scalar sample origin must be finite"),
            MeshError::InvalidIsovalue => write!(f, "scalar isovalue must be finite"),
            MeshError::DuplicateCell { coordinate } => {
                write!(f, "duplicate mesh cell at {coordinate:?}")
            }
            MeshError::PositionOutOfRange => {
                write!(f, "mesh position is outside the finite f32 render range")
            }
            MeshError::TextureMapping(source) => source.fmt(f),
        }
    }
}

impl std::error::Error for MeshError {}

// ── Face geometry ──────────────────────────────────────────────────────────────

fn in_plane_axes(dir: Direction6) -> (usize, usize) {
    let axis = dir.axis().index();
    // The two in-plane axes ordered so `u × v = +a` (right-handed), making the
    // CCW loop's normal point along `+a` for positive faces.
    match axis {
        0 => (1, 2), // X: Y,Z  (Y×Z = X)
        1 => (2, 0), // Y: Z,X  (Z×X = Y)
        _ => (0, 1), // Z: X,Y  (X×Y = Z)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Face {
    state: u16,
    slot: u16,
    coordinate: [i64; 3],
    dir: Direction6,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Quad {
    state: u16,
    slot: u16,
    coordinate: [i64; 3],
    dir: Direction6,
    u_length: u32,
    v_length: u32,
}

/// Merge each exact `(material, normal, plane)` lane independently.
///
/// The remaining cells in a lane are ordered `(v, u)`. The least cell starts a
/// rectangle, which grows along `u` and then across complete rows along `v`.
/// Removing each accepted rectangle makes disconnected regions and holes
/// deterministic without ever bridging absent cells.
fn greedy_merge_faces(faces: Vec<Face>) -> Result<Vec<Quad>, MeshError> {
    type FacePlane = (u16, u16, Direction6, i64);
    let mut planes: BTreeMap<FacePlane, BTreeSet<(i64, i64)>> = BTreeMap::new();
    for face in faces {
        let axis = face.dir.axis().index();
        let (u_axis, v_axis) = in_plane_axes(face.dir);
        planes
            .entry((face.slot, face.state, face.dir, face.coordinate[axis]))
            .or_default()
            .insert((face.coordinate[v_axis], face.coordinate[u_axis]));
    }

    let mut quads = Vec::new();
    for ((slot, state, dir, plane), mut cells) in planes {
        let axis = dir.axis().index();
        let (u_axis, v_axis) = in_plane_axes(dir);
        while let Some(&(v_start, u_start)) = cells.first() {
            let mut u_length = 1_u32;
            loop {
                let Some(next_u) = u_start.checked_add(i64::from(u_length)) else {
                    return Err(MeshError::PositionOutOfRange);
                };
                if cells.contains(&(v_start, next_u)) {
                    u_length = u_length.checked_add(1).ok_or(MeshError::TooManyVertices {
                        vertices: u64::from(u32::MAX) + 1,
                    })?;
                } else {
                    break;
                }
            }

            let mut v_length = 1_u32;
            'rows: loop {
                let Some(next_v) = v_start.checked_add(i64::from(v_length)) else {
                    return Err(MeshError::PositionOutOfRange);
                };
                for u_offset in 0..u_length {
                    let Some(u) = u_start.checked_add(i64::from(u_offset)) else {
                        return Err(MeshError::PositionOutOfRange);
                    };
                    if !cells.contains(&(next_v, u)) {
                        break 'rows;
                    }
                }
                v_length = v_length.checked_add(1).ok_or(MeshError::TooManyVertices {
                    vertices: u64::from(u32::MAX) + 1,
                })?;
            }

            for v_offset in 0..v_length {
                let v = v_start
                    .checked_add(i64::from(v_offset))
                    .ok_or(MeshError::PositionOutOfRange)?;
                for u_offset in 0..u_length {
                    let u = u_start
                        .checked_add(i64::from(u_offset))
                        .ok_or(MeshError::PositionOutOfRange)?;
                    cells.remove(&(v, u));
                }
            }

            let mut coordinate = [0_i64; 3];
            coordinate[axis] = plane;
            coordinate[u_axis] = u_start;
            coordinate[v_axis] = v_start;
            quads.push(Quad {
                state,
                slot,
                coordinate,
                dir,
                u_length,
                v_length,
            });
        }
    }
    Ok(quads)
}

/// The four absolute grid points of one greedy quad, wound CCW so the polygon
/// normal points outward along `quad.dir`.
fn quad_corners(quad: Quad) -> Result<[[i64; 3]; 4], MeshError> {
    let axis = quad.dir.axis().index();
    let (u_axis, v_axis) = in_plane_axes(quad.dir);
    let fixed = i64::from(quad.dir.is_positive());
    let loop_uv = [
        (0_i64, 0_i64),
        (i64::from(quad.u_length), 0),
        (i64::from(quad.u_length), i64::from(quad.v_length)),
        (0, i64::from(quad.v_length)),
    ];
    let mut out = [[0_i64; 3]; 4];
    for (index, (u_offset, v_offset)) in loop_uv.into_iter().enumerate() {
        let mut point = quad.coordinate;
        point[axis] = point[axis]
            .checked_add(fixed)
            .ok_or(MeshError::PositionOutOfRange)?;
        point[u_axis] = point[u_axis]
            .checked_add(u_offset)
            .ok_or(MeshError::PositionOutOfRange)?;
        point[v_axis] = point[v_axis]
            .checked_add(v_offset)
            .ok_or(MeshError::PositionOutOfRange)?;
        out[index] = point;
    }
    if !quad.dir.is_positive() {
        out.swap(1, 3);
    }
    Ok(out)
}

// ── Mesher ─────────────────────────────────────────────────────────────────────

/// Mesh a single chunk in isolation: out-of-chunk neighbours are treated as
/// **empty**, so all border faces are emitted. Good for standalone fixtures.
pub fn mesh_chunk_standalone(
    spec: &VoxelGridSpec,
    coord: ChunkCoord,
    chunk: &VoxelChunk,
) -> Result<MeshPayload, MeshError> {
    mesh_core(
        spec,
        coord,
        chunk,
        |_| true,
        |_, v| {
            let (c, l) = spec.voxel_to_chunk_local(v);
            c == coord && chunk.get(l).is_some_and(|x| x.is_opaque())
        },
    )
}

/// Mesh a complete local-space cell arrangement around an explicit pivot.
///
/// Cells are canonicalized by coordinate before face emission, making output
/// independent of caller iteration order. Omitted neighbours are empty. The
/// face limit bounds both work and the renderer payload allocation.
pub fn mesh_cells_standalone(
    cell_size: f64,
    pivot: [f64; 3],
    cells: &[MeshVoxelCell],
    max_faces: u32,
) -> Result<MeshPayload, MeshError> {
    mesh_cells_standalone_with_options(
        cell_size,
        pivot,
        cells,
        SurfaceMeshOptions {
            limits: SurfaceMeshLimits {
                max_source_faces: u64::from(max_faces),
                ..SurfaceMeshLimits::default()
            },
            ..SurfaceMeshOptions::default()
        },
    )
}

/// Extract a dual-contoured surface from an explicit, bounded scalar lattice.
///
/// Samples are lattice points in x-fastest order at
/// `origin + [x, y, z] * spacing`. Values below `isovalue` are inside; a value
/// equal to the isovalue is outside. The supplied point lattice is the entire
/// domain: crossings at its boundary remain open rather than receiving an
/// invented padding layer or cap. Sampled geometry uses material slot zero;
/// its product attributes are supplied by the owning implicit-surface service.
pub fn mesh_scalar_samples(
    origin: [f64; 3],
    spacing: f64,
    dimensions: [usize; 3],
    samples: &[f32],
    isovalue: f32,
    limits: SurfaceMeshLimits,
) -> Result<MeshPayload, MeshError> {
    let surface = mesh_scalar_surface(
        ScalarVolume {
            origin,
            spacing,
            dimensions,
            samples,
            materials: None,
            isovalue,
        },
        &SurfaceMaterials::default(),
        None,
        limits,
    )?;
    let mut lanes = BTreeMap::<u16, Vec<u32>>::new();
    for (triangle, slot) in surface.triangles.iter().zip(&surface.slots) {
        lanes.entry(*slot).or_default().extend_from_slice(triangle);
    }
    let mut indices = Vec::with_capacity(surface.triangles.len() * 3);
    let mut groups = Vec::with_capacity(lanes.len());
    for (slot, lane) in lanes {
        let start = indices.len() as u32;
        indices.extend(lane);
        groups.push(MeshGroup {
            state: 0,
            material_slot: slot,
            direction: None,
            surface_mode: SurfaceMode::DualContouring,
            start,
            count: indices.len() as u32 - start,
        });
    }
    let bounds = bounds_of(surface.positions.iter().copied());
    Ok(MeshPayload {
        surface_mode: SurfaceMode::DualContouring,
        positions: surface.positions.into_iter().flatten().collect(),
        normals: surface.normals.into_iter().flatten().collect(),
        tile_coordinates: Vec::new(),
        layer_weights: Vec::new(),
        indices,
        groups,
        triangle_owners: surface.owners,
        bounds,
        stats: surface.stats,
    })
}

/// A dense scalar lattice: `origin + [x, y, z] * spacing` in x-fastest order.
/// Values below `isovalue` are inside. Each sample may carry a material slot;
/// without them every sample is slot zero.
#[derive(Debug, Clone, Copy)]
pub struct ScalarVolume<'a> {
    pub origin: [f64; 3],
    pub spacing: f64,
    pub dimensions: [usize; 3],
    pub samples: &'a [f32],
    pub materials: Option<&'a [u16]>,
    pub isovalue: f32,
}

/// The samples `[min, max)` of a scalar volume whose surface one extraction
/// owns: a quad belongs to the region holding its edge's inside sample.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScalarRegion {
    pub min: [usize; 3],
    pub max: [usize; 3],
}

/// Dual-contoured scalar geometry with a material slot per triangle, before
/// render attributes. Positions are in the volume's world space.
#[derive(Debug, Clone, PartialEq)]
pub struct ScalarSurface {
    pub positions: Vec<[f32; 3]>,
    /// Per-vertex normals averaged from the cell's Hermite data.
    pub normals: Vec<[f32; 3]>,
    pub triangles: Vec<[u32; 3]>,
    /// The material slot of each triangle: its edge's inside sample.
    pub slots: Vec<u16>,
    /// The inside sample (lattice coordinate) of each triangle's edge.
    pub owners: Vec<[i64; 3]>,
    /// Triangles one sample beyond a requested region. They share the
    /// region's border vertices, so normals computed over every triangle
    /// agree with the neighbouring region; drop them before drawing.
    pub halo: Vec<bool>,
    pub stats: MeshStats,
}

/// Dual-contour a scalar volume, or one region of it with a one-sample halo
/// ring. A region's geometry is identical to the same triangles of the whole
/// volume, so regions meshed independently meet without seams. Each
/// material's [`SurfaceCharacter`] places the vertices of its cells.
pub fn mesh_scalar_surface(
    volume: ScalarVolume<'_>,
    materials: &SurfaceMaterials,
    region: Option<ScalarRegion>,
    limits: SurfaceMeshLimits,
) -> Result<ScalarSurface, MeshError> {
    if !volume.origin.iter().all(|value| value.is_finite()) {
        return Err(MeshError::InvalidScalarOrigin);
    }
    if !volume.spacing.is_finite() || volume.spacing <= 0.0 {
        return Err(MeshError::InvalidCellSize);
    }
    let pivot = volume.origin.map(|value| 0.5 - value / volume.spacing);
    if !pivot.iter().all(|value| value.is_finite()) {
        return Err(MeshError::InvalidPivot);
    }
    let dims = volume.dimensions;
    if dims.iter().any(|dimension| *dimension < 2) {
        return Err(MeshError::InvalidSampleDimensions { dimensions: dims });
    }
    let count = surface::checked_product(dims)?;
    if volume.samples.len() != count {
        return Err(MeshError::InvalidSampleCount {
            expected: count,
            actual: volume.samples.len(),
        });
    }
    if let Some(materials) = volume.materials {
        if materials.len() != count {
            return Err(MeshError::InvalidSampleCount {
                expected: count,
                actual: materials.len(),
            });
        }
    }
    let characters = surface::Characters {
        materials,
        default_mode: SurfaceMode::DualContouring,
    };
    let mut reconstruction = surface::Reconstruction::default();
    match region {
        None => {
            let lattice = surface::Lattice::scalar(
                [0; 3],
                dims,
                volume.samples.iter().copied(),
                volume.materials.map(<[u16]>::to_vec),
                volume.isovalue,
                limits,
            )?;
            let owner = surface::Owner {
                min: [0; 3],
                max: dims.map(|value| value as i64),
            };
            surface::dual_contour(
                &lattice,
                characters,
                owner,
                false,
                limits,
                &mut reconstruction,
            )?;
        }
        Some(region) => {
            if (0..3)
                .any(|axis| region.min[axis] >= region.max[axis] || region.max[axis] > dims[axis])
            {
                return Err(MeshError::CoordinateRangeTooLarge);
            }
            // The owned samples, their one-sample halo ring, and the samples
            // of every cell those quads use.
            let low: [usize; 3] = std::array::from_fn(|axis| region.min[axis].saturating_sub(2));
            let high: [usize; 3] =
                std::array::from_fn(|axis| (region.max[axis] + 2).min(dims[axis]));
            let sub_dims: [usize; 3] = std::array::from_fn(|axis| high[axis] - low[axis]);
            let index = |x: usize, y: usize, z: usize| (z * dims[1] + y) * dims[0] + x;
            let mut samples = Vec::with_capacity(surface::checked_product(sub_dims)?);
            let mut sub_materials = volume
                .materials
                .map(|_| Vec::with_capacity(samples.capacity()));
            for z in low[2]..high[2] {
                for y in low[1]..high[1] {
                    let row = index(low[0], y, z)..index(high[0], y, z);
                    samples.extend_from_slice(&volume.samples[row.clone()]);
                    if let (Some(target), Some(source)) = (&mut sub_materials, volume.materials) {
                        target.extend_from_slice(&source[row]);
                    }
                }
            }
            let lattice = surface::Lattice::scalar(
                low.map(|value| value as i64),
                sub_dims,
                samples.into_iter(),
                sub_materials,
                volume.isovalue,
                limits,
            )?;
            let owner = surface::Owner {
                min: region.min.map(|value| value as i64),
                max: region.max.map(|value| value as i64),
            };
            surface::dual_contour(
                &lattice,
                characters,
                owner,
                true,
                limits,
                &mut reconstruction,
            )?;
        }
    }
    let mut positions = Vec::with_capacity(reconstruction.positions.len());
    let mut normals = Vec::with_capacity(reconstruction.positions.len());
    for (point, normal) in reconstruction.positions.iter().zip(&reconstruction.normals) {
        let mut position = [0.0_f32; 3];
        let mut rendered_normal = [0.0_f32; 3];
        for axis in 0..3 {
            let value = (point[axis] - pivot[axis]) * volume.spacing;
            position[axis] = value as f32;
            rendered_normal[axis] = normal[axis] as f32;
            if !value.is_finite()
                || !position[axis].is_finite()
                || !rendered_normal[axis].is_finite()
            {
                return Err(MeshError::PositionOutOfRange);
            }
        }
        positions.push(position);
        normals.push(rendered_normal);
    }
    let triangles = reconstruction.triangles.len() as u32;
    let kept = reconstruction.halo.iter().filter(|halo| !**halo).count() as u32;
    Ok(ScalarSurface {
        stats: MeshStats {
            surface_mode: SurfaceMode::DualContouring,
            vertices: positions.len() as u32,
            indices: triangles * 3,
            triangles,
            quads: kept / 2,
            faces_emitted: kept,
            source_faces: kept / 2,
            faces_culled: 0,
            sampled_cells: reconstruction.sampled_cells,
            qef_rank_deficient: reconstruction.rank_deficient,
            qef_fallbacks: reconstruction.fallbacks,
        },
        positions,
        normals,
        triangles: reconstruction.triangles,
        slots: reconstruction.slots,
        owners: reconstruction.owners,
        halo: reconstruction.halo,
    })
}

/// Mesh a complete local-space cell arrangement with an explicit derived
/// presentation mode. The sampled field and all reconstructed geometry remain
/// disposable projections of `cells`.
pub fn mesh_cells_standalone_with_options(
    cell_size: f64,
    pivot: [f64; 3],
    cells: &[MeshVoxelCell],
    options: SurfaceMeshOptions,
) -> Result<MeshPayload, MeshError> {
    if !cell_size.is_finite() || cell_size <= 0.0 {
        return Err(MeshError::InvalidCellSize);
    }
    if !pivot.iter().all(|value| value.is_finite()) {
        return Err(MeshError::InvalidPivot);
    }

    let mut occupied = BTreeMap::new();
    for cell in cells {
        if occupied
            .insert(cell.coordinate, cell.material_slot)
            .is_some()
        {
            return Err(MeshError::DuplicateCell {
                coordinate: cell.coordinate,
            });
        }
    }

    let greedy_faces = |include: &dyn Fn(u16) -> bool,
                        hides: &dyn Fn(u16, u16) -> bool|
     -> Result<(Vec<Face>, u32), MeshError> {
        let mut faces = Vec::new();
        let mut faces_culled = 0_u32;
        for (&coordinate, &slot) in &occupied {
            if !include(slot) {
                continue;
            }
            for dir in Direction6::ALL {
                let normal = dir.normal();
                let neighbour = [
                    coordinate[0]
                        .checked_add(normal.x as i64)
                        .ok_or(MeshError::PositionOutOfRange)?,
                    coordinate[1]
                        .checked_add(normal.y as i64)
                        .ok_or(MeshError::PositionOutOfRange)?,
                    coordinate[2]
                        .checked_add(normal.z as i64)
                        .ok_or(MeshError::PositionOutOfRange)?,
                ];
                if occupied
                    .get(&neighbour)
                    .is_some_and(|neighbour| hides(slot, *neighbour))
                {
                    faces_culled = faces_culled.saturating_add(1);
                } else {
                    let face_count = faces.len() as u64 + 1;
                    if face_count > options.limits.max_source_faces {
                        return Err(MeshError::TooManyFaces {
                            faces: face_count,
                            limit: options.limits.max_source_faces,
                        });
                    }
                    faces.push(Face {
                        state: 0,
                        slot,
                        coordinate,
                        dir,
                    });
                }
            }
        }
        Ok((faces, faces_culled))
    };

    if options.all_greedy() {
        let (faces, faces_culled) =
            greedy_faces(&|_| true, &|slot, neighbour| options.hides(slot, neighbour))?;
        let source_faces = faces.len() as u32;
        let quads = greedy_merge_faces(faces)?;
        return emit_quads(
            &quads,
            cell_size,
            pivot,
            [0; 3],
            source_faces,
            faces_culled,
            options.limits,
        );
    }

    // Every face a solid voxel exposes to empty space: the work charged
    // to reconstructed admission, whichever mode draws it.
    let (exposed, _) = greedy_faces(&|_| true, &|_, _| true)?;
    let Some((&first, _)) = occupied.first_key_value() else {
        return Ok(empty_payload(options.mode));
    };
    let mut minimum = first;
    let mut maximum = first;
    for coordinate in occupied.keys() {
        for axis in 0..3 {
            minimum[axis] = minimum[axis].min(coordinate[axis]);
            maximum[axis] = maximum[axis].max(coordinate[axis]);
        }
    }
    let mut low = [0_i64; 3];
    let mut dims = [0_usize; 3];
    for axis in 0..3 {
        low[axis] = minimum[axis]
            .checked_sub(1)
            .ok_or(MeshError::CoordinateRangeTooLarge)?;
        let high = maximum[axis]
            .checked_add(2)
            .ok_or(MeshError::CoordinateRangeTooLarge)?;
        dims[axis] =
            usize::try_from(high - low[axis]).map_err(|_| MeshError::CoordinateRangeTooLarge)?;
    }
    let mut lattice = surface::Lattice::voxels(low, dims, options.limits)?;
    for (&coordinate, &slot) in &occupied {
        lattice.set_voxel(
            coordinate,
            Some(slot),
            -svc_volume::DEFAULT_DENSITY_MAGNITUDE,
        );
    }
    let owner = surface::Owner {
        min: minimum,
        max: maximum.map(|value| value + 1),
    };
    let smooth = reconstruct(&lattice, &options, owner, cell_size, pivot, None)?;
    let greedy = |slot: u16| options.surface(slot).mode == SurfaceMode::GreedyCubes;
    let (faces, faces_culled) = greedy_faces(&greedy, &|slot, neighbour| {
        greedy(neighbour) && options.hides(slot, neighbour)
    })?;
    let mut payload = if faces.is_empty() {
        smooth
    } else {
        let source_faces = faces.len() as u32;
        let quads = greedy_merge_faces(faces)?;
        let cubes = emit_quads(
            &quads,
            cell_size,
            pivot,
            [0; 3],
            source_faces,
            faces_culled,
            options.limits,
        )?;
        merge_payloads(cubes, smooth, options.mode, options.limits)?
    };
    payload.stats.source_faces = exposed.len() as u32;
    Ok(payload)
}

/// Reconstruct the owned surface of a voxel lattice in every reconstructed
/// mode its materials use.
fn reconstruct(
    lattice: &surface::Lattice,
    options: &SurfaceMeshOptions,
    owner: surface::Owner,
    cell_size: f64,
    pivot: [f64; 3],
    layers: Option<&terrain_layers::LayerField<'_>>,
) -> Result<MeshPayload, MeshError> {
    let characters = options.characters();
    let mut reconstruction = surface::Reconstruction::default();
    let present = lattice.materials_inside(&options.non_occluding);
    if present.is_empty() {
        extract(lattice, options, owner, &mut reconstruction)?;
    } else {
        // One layer of occluding materials, which meet water as they meet
        // air, and one per non-occluding material, which keeps only its own
        // surface: against air and other non-occluding materials.
        for kept in std::iter::once(None).chain(present.into_iter().map(Some)) {
            let layer = lattice.layer(&options.non_occluding, kept);
            let first = reconstruction.triangles.len();
            extract(&layer, options, owner, &mut reconstruction)?;
            if let Some(kept) = kept {
                for triangle in first..reconstruction.triangles.len() {
                    if reconstruction.slots[triangle] != kept {
                        reconstruction.halo[triangle] = true;
                    }
                }
            }
        }
    }
    surface::voxel_payload(
        reconstruction,
        characters,
        cell_size,
        pivot,
        options.limits,
        layers,
    )
}

/// Append the surfaces of every reconstructed mode in use.
fn extract(
    lattice: &surface::Lattice,
    options: &SurfaceMeshOptions,
    owner: surface::Owner,
    out: &mut surface::Reconstruction,
) -> Result<(), MeshError> {
    let characters = options.characters();
    if options.uses_mode(SurfaceMode::DualContouring) {
        surface::dual_contour(lattice, characters, owner, false, options.limits, out)?;
    }
    if options.uses_mode(SurfaceMode::MarchingCubes) {
        surface::march(lattice, characters, owner, options.limits, out)?;
    }
    Ok(())
}

/// Append `second` to `first`: offset indices, concatenate groups.
fn merge_payloads(
    mut first: MeshPayload,
    second: MeshPayload,
    mode: SurfaceMode,
    limits: SurfaceMeshLimits,
) -> Result<MeshPayload, MeshError> {
    if second.indices.is_empty() {
        first.surface_mode = mode;
        first.stats.surface_mode = mode;
        return Ok(first);
    }
    let vertex_base = (first.positions.len() / 3) as u32;
    let index_base = first.indices.len() as u32;
    surface::check_output_growth(
        u64::from(vertex_base),
        u64::from(index_base),
        (second.positions.len() / 3) as u64,
        second.indices.len() as u64,
        limits,
    )?;
    let had_first = !first.indices.is_empty();
    first.positions.extend(second.positions);
    first.normals.extend(second.normals);
    first.tile_coordinates.extend(second.tile_coordinates);
    first.layer_weights.extend(second.layer_weights);
    first
        .indices
        .extend(second.indices.into_iter().map(|index| index + vertex_base));
    first
        .groups
        .extend(second.groups.into_iter().map(|group| MeshGroup {
            start: group.start + index_base,
            ..group
        }));
    first.triangle_owners.extend(second.triangle_owners);
    first.bounds = if had_first {
        MeshBounds {
            min: std::array::from_fn(|axis| first.bounds.min[axis].min(second.bounds.min[axis])),
            max: std::array::from_fn(|axis| first.bounds.max[axis].max(second.bounds.max[axis])),
        }
    } else {
        second.bounds
    };
    let stats = &mut first.stats;
    stats.surface_mode = mode;
    stats.vertices += second.stats.vertices;
    stats.indices += second.stats.indices;
    stats.triangles += second.stats.triangles;
    stats.quads += second.stats.quads;
    stats.faces_emitted += second.stats.faces_emitted;
    stats.faces_culled += second.stats.faces_culled;
    stats.sampled_cells += second.stats.sampled_cells;
    stats.qef_rank_deficient += second.stats.qef_rank_deficient;
    stats.qef_fallbacks += second.stats.qef_fallbacks;
    first.surface_mode = mode;
    Ok(first)
}

fn bounds_of(points: impl Iterator<Item = [f32; 3]>) -> MeshBounds {
    let mut minimum = [f32::INFINITY; 3];
    let mut maximum = [f32::NEG_INFINITY; 3];
    let mut any = false;
    for point in points {
        any = true;
        for axis in 0..3 {
            minimum[axis] = minimum[axis].min(point[axis]);
            maximum[axis] = maximum[axis].max(point[axis]);
        }
    }
    if any {
        MeshBounds {
            min: minimum,
            max: maximum,
        }
    } else {
        MeshBounds {
            min: [0.0; 3],
            max: [0.0; 3],
        }
    }
}

fn empty_payload(mode: SurfaceMode) -> MeshPayload {
    MeshPayload {
        surface_mode: mode,
        positions: Vec::new(),
        normals: Vec::new(),
        tile_coordinates: Vec::new(),
        layer_weights: Vec::new(),
        indices: Vec::new(),
        groups: Vec::new(),
        triangle_owners: Vec::new(),
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

/// Mesh a resident chunk using its **resident neighbour chunks** for border
/// culling (faces against a non-resident/absent neighbour are emitted). Returns
/// `None` if `coord` is not resident in `world`.
pub fn mesh_chunk_in_world(
    world: &VoxelWorld,
    coord: ChunkCoord,
) -> Option<Result<MeshPayload, MeshError>> {
    mesh_chunk_in_world_with_options(world, coord, &SurfaceMeshOptions::default())
}

/// Mesh one resident chunk with its materials' surface modes.
///
/// Reconstructed materials sample the chunk and a one-voxel halo of its
/// resident neighbours (absent neighbours read as empty), including their
/// densities. A dual-contoured quad belongs to the chunk holding its edge's
/// solid endpoint and a marched polygon to the chunk holding its cell's first
/// solid corner, so adjacent chunk calls make identical decisions without
/// duplicating a primitive. Cube materials keep their greedy faces; a cube
/// face against a reconstructed material is kept, since that material's
/// surface may not cover it, and so is a face against a different
/// non-occluding material. Returned positions remain local to `coord`,
/// matching the existing chunk transform contract.
pub fn mesh_chunk_in_world_with_options(
    world: &VoxelWorld,
    coord: ChunkCoord,
    options: &SurfaceMeshOptions,
) -> Option<Result<MeshPayload, MeshError>> {
    let chunk = world.get(coord)?;
    if options.all_greedy() {
        let spec = world.grid();
        return Some(
            mesh_core(
                &spec,
                coord,
                chunk,
                |_| true,
                |slot, voxel| {
                    neighbour_slot(world, &spec, voxel).is_some_and(|n| options.hides(slot, n))
                },
            )
            .map(|mut cubes| {
                if let Some(layers) = &options.terrain_layers {
                    cube_layer_weights(&mut cubes, layers);
                }
                cubes
            }),
        );
    }
    Some(mesh_chunk_reconstructed(world, coord, chunk, options))
}

/// The material slot of a resident voxel, if solid.
fn neighbour_slot(world: &VoxelWorld, spec: &VoxelGridSpec, voxel: VoxelCoord) -> Option<u16> {
    let (chunk, local) = spec.voxel_to_chunk_local(voxel);
    world
        .get(chunk)
        .and_then(|chunk| chunk.get(local))
        .and_then(|value| value.material())
        .map(|material| material.raw())
}

fn mesh_chunk_reconstructed(
    world: &VoxelWorld,
    coord: ChunkCoord,
    chunk: &VoxelChunk,
    options: &SurfaceMeshOptions,
) -> Result<MeshPayload, MeshError> {
    let greedy = |slot: u16| options.surface(slot).mode == SurfaceMode::GreedyCubes;
    if chunk.iter().any(|(_, value)| {
        value.state().raw() != 0 && value.material().is_some_and(|m| !greedy(m.raw()))
    }) {
        return Err(MeshError::StateRequiresGreedyCubes);
    }
    let spec = world.grid();
    let origin = spec.chunk_origin_voxel(coord).to_array();
    let size = spec.chunk_dims().to_array().map(i64::from);
    let mut maximum = [0_i64; 3];
    let mut low = [0_i64; 3];
    for axis in 0..3 {
        maximum[axis] = origin[axis]
            .checked_add(size[axis])
            .ok_or(MeshError::CoordinateRangeTooLarge)?;
        low[axis] = origin[axis]
            .checked_sub(1)
            .ok_or(MeshError::CoordinateRangeTooLarge)?;
    }
    let dims = size.map(|value| value as usize + 2);
    let mut lattice = surface::Lattice::voxels(low, dims, options.limits)?;
    let high: [i64; 3] = std::array::from_fn(|axis| low[axis] + dims[axis] as i64);
    for dz in -1..=1_i64 {
        for dy in -1..=1_i64 {
            for dx in -1..=1_i64 {
                let neighbour = ChunkCoord::new(coord.x + dx, coord.y + dy, coord.z + dz);
                let Some(source) = world.get(neighbour) else {
                    continue;
                };
                let neighbour_origin = spec.chunk_origin_voxel(neighbour).to_array();
                let mut from = [0_u32; 3];
                let mut to = [0_u32; 3];
                for axis in 0..3 {
                    let start = low[axis].max(neighbour_origin[axis]);
                    let end = high[axis].min(neighbour_origin[axis] + size[axis]);
                    from[axis] = (start - neighbour_origin[axis]) as u32;
                    to[axis] =
                        (end - neighbour_origin[axis]).max(start - neighbour_origin[axis]) as u32;
                }
                for z in from[2]..to[2] {
                    for y in from[1]..to[1] {
                        for x in from[0]..to[0] {
                            let local = LocalVoxelCoord::new(x, y, z);
                            let value = source.get(local).expect("local within chunk");
                            let density = source.density(local).expect("local within chunk");
                            lattice.set_voxel(
                                [
                                    neighbour_origin[0] + i64::from(x),
                                    neighbour_origin[1] + i64::from(y),
                                    neighbour_origin[2] + i64::from(z),
                                ],
                                value.material().map(|material| material.raw()),
                                density,
                            );
                        }
                    }
                }
            }
        }
    }
    let owner = surface::Owner {
        min: origin,
        max: maximum,
    };
    let field = options
        .terrain_layers
        .as_ref()
        .map(|layers| terrain_layers::LayerField::around_chunk(world, &spec, layers, origin, size));
    let smooth = reconstruct(
        &lattice,
        options,
        owner,
        spec.voxel_size(),
        origin.map(|value| value as f64),
        field.as_ref(),
    )?;
    if !options.uses_mode(SurfaceMode::GreedyCubes) {
        return Ok(smooth);
    }
    let mut cubes = mesh_core(&spec, coord, chunk, greedy, |slot, voxel| {
        neighbour_slot(world, &spec, voxel).is_some_and(|n| greedy(n) && options.hides(slot, n))
    })?;
    if let Some(layers) = &options.terrain_layers {
        cube_layer_weights(&mut cubes, layers);
    }
    merge_payloads(cubes, smooth, options.mode, options.limits)
}

/// Gives each cube face vertex all of its slot's layer weight: cube faces
/// keep their block look.
fn cube_layer_weights(cubes: &mut MeshPayload, layers: &TerrainLayers) {
    let mut weights = vec![[1.0, 0.0, 0.0, 0.0]; cubes.positions.len() / 3];
    for group in &cubes.groups {
        let range = group.start as usize..(group.start + group.count) as usize;
        for &vertex in &cubes.indices[range] {
            weights[vertex as usize] = layers.one_hot(group.material_slot);
        }
    }
    cubes.layer_weights = weights.into_iter().flatten().collect();
}

/// Core mesher: `hides(slot, world_voxel)` answers whether a voxel hides the
/// face a `slot` voxel shows it. The current chunk's solid voxels of
/// `include`d materials drive emission.
fn mesh_core(
    spec: &VoxelGridSpec,
    coord: ChunkCoord,
    chunk: &VoxelChunk,
    include: impl Fn(u16) -> bool,
    hides: impl Fn(u16, VoxelCoord) -> bool,
) -> Result<MeshPayload, MeshError> {
    // Collect visible faces in deterministic order, with culling stats.
    let mut faces: Vec<Face> = Vec::new();
    let mut faces_culled = 0u32;
    for (local, value) in chunk.iter() {
        let Some(material) = value.material() else {
            continue;
        };
        if !include(material.raw()) {
            continue;
        }
        let world_voxel = spec.chunk_local_to_voxel(coord, local);
        for dir in Direction6::ALL {
            if hides(material.raw(), world_voxel.neighbor(dir)) {
                faces_culled += 1;
            } else {
                faces.push(Face {
                    state: value.state().raw(),
                    slot: material.raw(),
                    coordinate: [i64::from(local.x), i64::from(local.y), i64::from(local.z)],
                    dir,
                });
            }
        }
    }

    let source_faces = faces.len() as u32;
    let quads = greedy_merge_faces(faces)?;
    emit_quads(
        &quads,
        spec.voxel_size(),
        [0.0; 3],
        spec.chunk_origin_voxel(coord).to_array(),
        source_faces,
        faces_culled,
        SurfaceMeshLimits::default(),
    )
}

// Transform world faces and texture coordinates back into the authored local
// orientation. Cube occupancy and geometric normals remain unchanged.
fn state_direction(mut dir: Direction6, state: u16) -> Direction6 {
    for _ in 0..(state & 3) {
        dir = match dir {
            Direction6::PosX => Direction6::PosZ,
            Direction6::PosZ => Direction6::NegX,
            Direction6::NegX => Direction6::NegZ,
            Direction6::NegZ => Direction6::PosX,
            other => other,
        };
    }
    dir
}
fn state_tile_point(
    dir: Direction6,
    mut point: [i64; 3],
    mut origin: [i64; 3],
    state: u16,
) -> Result<[f32; 2], MeshError> {
    for _ in 0..(state & 3) {
        point = [
            point[2]
                .checked_neg()
                .ok_or(MeshError::PositionOutOfRange)?,
            point[1],
            point[0],
        ];
        origin = [
            origin[2]
                .checked_neg()
                .ok_or(MeshError::PositionOutOfRange)?,
            origin[1],
            origin[0],
        ];
    }
    project_voxel_surface_tile_point(state_direction(dir, state), point, origin)
        .map_err(MeshError::TextureMapping)
}

fn emit_quads(
    quads: &[Quad],
    cell_size: f64,
    pivot: [f64; 3],
    texture_coordinate_origin: [i64; 3],
    source_faces: u32,
    faces_culled: u32,
    limits: SurfaceMeshLimits,
) -> Result<MeshPayload, MeshError> {
    let vertex_count = quads.len() as u64 * 4;
    if vertex_count > u64::from(limits.max_vertices) {
        return Err(MeshError::TooManyVertices {
            vertices: vertex_count,
        });
    }
    let index_count = quads.len() as u64 * 6;
    if index_count > u64::from(limits.max_indices) {
        return Err(MeshError::TooManyIndices {
            indices: index_count,
            limit: limits.max_indices,
        });
    }
    let material_partitions = quads
        .iter()
        .map(|quad| quad.slot)
        .collect::<BTreeSet<_>>()
        .len() as u64;
    if material_partitions > u64::from(limits.max_material_partitions) {
        return Err(MeshError::TooManyMaterialPartitions {
            partitions: material_partitions,
            limit: limits.max_material_partitions,
        });
    }

    let mut positions: Vec<f32> = Vec::with_capacity(quads.len() * 12);
    let mut normals: Vec<f32> = Vec::with_capacity(quads.len() * 12);
    let mut tile_coordinates: Vec<f32> = Vec::with_capacity(quads.len() * 8);
    let mut indices: Vec<u32> = Vec::with_capacity(quads.len() * 6);
    let mut triangle_owners: Vec<[i64; 3]> = Vec::with_capacity(quads.len() * 2);
    let mut groups: Vec<MeshGroup> = Vec::new();
    let mut bmin = [f32::INFINITY; 3];
    let mut bmax = [f32::NEG_INFINITY; 3];

    let mut cur_group: Option<(u16, u16, Direction6)> = None;
    let mut group_start: u32 = 0;
    for quad in quads {
        let group = (quad.slot, quad.state, state_direction(quad.dir, quad.state));
        if cur_group != Some(group) {
            if let Some((slot, state, direction)) = cur_group {
                groups.push(MeshGroup {
                    state,
                    material_slot: slot,
                    direction: Some(direction),
                    surface_mode: SurfaceMode::GreedyCubes,
                    start: group_start,
                    count: indices.len() as u32 - group_start,
                });
            }
            cur_group = Some(group);
            group_start = indices.len() as u32;
        }

        let base = (positions.len() / 3) as u32;
        let normal = quad.dir.normal();
        let [nx, ny, nz] = [normal.x as f32, normal.y as f32, normal.z as f32];
        for point in quad_corners(*quad)? {
            let mut p = [0.0_f32; 3];
            for axis in 0..3 {
                let value = (point[axis] as f64 - pivot[axis]) * cell_size;
                let rendered = value as f32;
                if !value.is_finite() || !rendered.is_finite() {
                    return Err(MeshError::PositionOutOfRange);
                }
                p[axis] = rendered;
            }
            for axis in 0..3 {
                bmin[axis] = bmin[axis].min(p[axis]);
                bmax[axis] = bmax[axis].max(p[axis]);
            }
            positions.extend_from_slice(&p);
            normals.extend_from_slice(&[nx, ny, nz]);
            tile_coordinates.extend_from_slice(&state_tile_point(
                quad.dir,
                point,
                texture_coordinate_origin,
                quad.state,
            )?);
        }
        // Two CCW triangles of the quad: (0,1,2) (0,2,3).
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        let owner = std::array::from_fn(|axis| {
            quad.coordinate[axis].saturating_add(texture_coordinate_origin[axis])
        });
        triangle_owners.extend_from_slice(&[owner, owner]);
    }
    if let Some((slot, state, direction)) = cur_group {
        groups.push(MeshGroup {
            state,
            material_slot: slot,
            direction: Some(direction),
            surface_mode: SurfaceMode::GreedyCubes,
            start: group_start,
            count: indices.len() as u32 - group_start,
        });
    }

    let bounds = if quads.is_empty() {
        MeshBounds {
            min: [0.0; 3],
            max: [0.0; 3],
        }
    } else {
        MeshBounds {
            min: bmin,
            max: bmax,
        }
    };
    let stats = MeshStats {
        surface_mode: SurfaceMode::GreedyCubes,
        vertices: (positions.len() / 3) as u32,
        indices: indices.len() as u32,
        triangles: (indices.len() / 3) as u32,
        quads: quads.len() as u32,
        faces_emitted: quads.len() as u32,
        source_faces,
        faces_culled,
        sampled_cells: 0,
        qef_rank_deficient: 0,
        qef_fallbacks: 0,
    };
    Ok(MeshPayload {
        surface_mode: SurfaceMode::GreedyCubes,
        positions,
        normals,
        tile_coordinates,
        layer_weights: Vec::new(),
        indices,
        groups,
        triangle_owners,
        bounds,
        stats,
    })
}

impl MeshPayload {
    /// A deterministic, human-readable dump for golden fixtures.
    pub fn to_fixture_string(&self) -> String {
        use core::fmt::Write;
        let mut s = String::new();
        let st = self.stats;
        let _ = writeln!(
            s,
            "mesh v={} i={} quads={} emitted={} source={} culled={}",
            st.vertices, st.indices, st.quads, st.faces_emitted, st.source_faces, st.faces_culled
        );
        let _ = writeln!(
            s,
            "bounds min={:?} max={:?}",
            self.bounds.min, self.bounds.max
        );
        for g in &self.groups {
            if let Some(direction) = g.direction {
                let _ = writeln!(
                    s,
                    "group slot={} direction={direction:?} start={} count={}",
                    g.material_slot, g.start, g.count
                );
            } else {
                let _ = writeln!(
                    s,
                    "group slot={} start={} count={}",
                    g.material_slot, g.start, g.count
                );
            }
        }
        for (i, p) in self.positions.as_chunks::<3>().0.iter().enumerate() {
            let n = &self.normals[i * 3..i * 3 + 3];
            let _ = writeln!(s, "v{i} pos={:?} nrm={:?}", p, n);
        }
        for (t, tri) in self.indices.as_chunks::<3>().0.iter().enumerate() {
            let _ = writeln!(s, "t{t} {} {} {}", tri[0], tri[1], tri[2]);
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_space::{ChunkDims, GridId, LocalVoxelCoord};
    use core_voxel::VoxelValue;
    use texture_mapping::{
        project_voxel_surface_tile_corners, repeat_voxel_tile_coordinate, VoxelTextureMappingError,
    };

    fn spec() -> VoxelGridSpec {
        VoxelGridSpec::new(GridId::new(0), 1.0, ChunkDims::cubic(4).unwrap()).unwrap()
    }

    fn chunk_with(solids: &[(LocalVoxelCoord, u16)]) -> VoxelChunk {
        let mut c = VoxelChunk::from_spec(&spec());
        for &(loc, m) in solids {
            c.set(loc, VoxelValue::solid_raw(m)).unwrap();
        }
        c
    }

    fn l(x: u32, y: u32, z: u32) -> LocalVoxelCoord {
        LocalVoxelCoord::new(x, y, z)
    }

    #[test]
    fn single_voxel_emits_six_faces() {
        let c = chunk_with(&[(l(1, 1, 1), 1)]);
        let m = mesh_chunk_standalone(&spec(), ChunkCoord::ORIGIN, &c).unwrap();
        assert_eq!(m.stats.quads, 6);
        assert_eq!(m.stats.source_faces, 6);
        assert_eq!(m.stats.vertices, 24);
        assert_eq!(m.stats.indices, 36);
        assert_eq!(m.stats.faces_culled, 0);
        assert_eq!(m.groups.len(), 6);
        assert!(m
            .groups
            .iter()
            .all(|group| group.material_slot == 1 && group.count == 6));
        assert_eq!(
            m.groups
                .iter()
                .filter_map(|group| group.direction)
                .collect::<BTreeSet<_>>(),
            Direction6::ALL.into_iter().collect()
        );
    }

    #[test]
    fn emitted_winding_matches_emitted_normal() {
        let c = chunk_with(&[(l(1, 1, 1), 1), (l(2, 1, 1), 1)]);
        let m = mesh_chunk_standalone(&spec(), ChunkCoord::ORIGIN, &c).unwrap();
        for tri in m.indices.as_chunks::<3>().0.iter() {
            let p: Vec<[f32; 3]> = tri
                .iter()
                .map(|&i| {
                    let i = i as usize * 3;
                    [m.positions[i], m.positions[i + 1], m.positions[i + 2]]
                })
                .collect();
            let gn = cross(sub(p[1], p[0]), sub(p[2], p[0]));
            let i0 = tri[0] as usize * 3;
            let sn = [m.normals[i0], m.normals[i0 + 1], m.normals[i0 + 2]];
            assert!(
                dot(gn, sn) > 0.0,
                "winding/normal mismatch: gn={gn:?} sn={sn:?}"
            );
        }
    }

    fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
        [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
    }
    fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    }
    fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
        a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
    }

    #[test]
    fn two_adjacent_voxels_cull_the_shared_face() {
        let c = chunk_with(&[(l(1, 1, 1), 1), (l(2, 1, 1), 1)]);
        let m = mesh_chunk_standalone(&spec(), ChunkCoord::ORIGIN, &c).unwrap();
        // 12 potential faces, 2 shared faces culled, and the remaining ten
        // source faces merge into the six rectangles of one cuboid.
        assert_eq!(m.stats.quads, 6);
        assert_eq!(m.stats.source_faces, 10);
        assert_eq!(m.stats.faces_culled, 2);
    }

    #[test]
    fn full_solid_chunk_emits_only_the_exterior_shell() {
        let mut c = VoxelChunk::from_spec(&spec());
        c.fill_region(l(0, 0, 0), l(4, 4, 4), VoxelValue::solid_raw(1))
            .unwrap();
        let m = mesh_chunk_standalone(&spec(), ChunkCoord::ORIGIN, &c).unwrap();
        assert_eq!(m.stats.quads, 6);
        assert_eq!(m.stats.source_faces, 6 * 4 * 4);
        assert_eq!(m.bounds.min, [0.0; 3]);
        assert_eq!(m.bounds.max, [4.0; 3]);
    }

    #[test]
    fn faces_are_grouped_by_material_slot_and_direction() {
        let c = chunk_with(&[(l(0, 0, 0), 3), (l(2, 2, 2), 1)]);
        let m = mesh_chunk_standalone(&spec(), ChunkCoord::ORIGIN, &c).unwrap();
        assert_eq!(m.groups.len(), 12);
        assert_eq!(
            m.groups
                .iter()
                .map(|group| (group.material_slot, group.direction))
                .collect::<BTreeSet<_>>(),
            Direction6::ALL
                .into_iter()
                .flat_map(|direction| [(1, Some(direction)), (3, Some(direction))])
                .collect()
        );
        assert_eq!(
            m.groups.iter().map(|g| g.count).sum::<u32>(),
            m.stats.indices
        );
    }

    #[test]
    fn meshing_is_deterministic() {
        let c = chunk_with(&[(l(1, 1, 1), 1), (l(2, 1, 1), 2), (l(0, 3, 0), 1)]);
        let a = mesh_chunk_standalone(&spec(), ChunkCoord::ORIGIN, &c).unwrap();
        let b = mesh_chunk_standalone(&spec(), ChunkCoord::ORIGIN, &c).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn neighbor_chunk_culls_border_faces() {
        let mut world = VoxelWorld::new(spec());
        let mut c0 = VoxelChunk::from_spec(&spec());
        c0.set(l(3, 0, 0), VoxelValue::solid_raw(1)).unwrap(); // +X border of chunk 0
        let mut c1 = VoxelChunk::from_spec(&spec());
        c1.set(l(0, 0, 0), VoxelValue::solid_raw(1)).unwrap(); // -X border of chunk 1
        world.insert(ChunkCoord::new(0, 0, 0), c0);
        world.insert(ChunkCoord::new(1, 0, 0), c1);
        world.drain_dirty();

        let with_neighbor = mesh_chunk_in_world(&world, ChunkCoord::new(0, 0, 0))
            .unwrap()
            .unwrap();
        // The +X face is culled by the neighbour → 5 faces (vs 6 standalone).
        assert_eq!(with_neighbor.stats.quads, 5);
        assert_eq!(with_neighbor.stats.faces_culled, 1);
    }

    #[test]
    fn empty_chunk_meshes_to_nothing() {
        let c = VoxelChunk::from_spec(&spec());
        let m = mesh_chunk_standalone(&spec(), ChunkCoord::ORIGIN, &c).unwrap();
        assert_eq!(m.stats.quads, 0);
        assert!(m.positions.is_empty() && m.indices.is_empty() && m.groups.is_empty());
        assert_eq!(
            m.bounds,
            MeshBounds {
                min: [0.0; 3],
                max: [0.0; 3]
            }
        );
    }

    #[test]
    fn two_voxel_line_matches_committed_golden() {
        // The named golden fixture; regenerate intentionally if the mesher changes.
        let c = chunk_with(&[(l(0, 0, 0), 1), (l(1, 0, 0), 1)]);
        let m = mesh_chunk_standalone(&spec(), ChunkCoord::ORIGIN, &c).unwrap();
        let golden = include_str!("../../../../fixtures/voxel-mesh/two-voxel-line.mesh.txt");
        assert_eq!(m.to_fixture_string(), golden);
    }

    #[test]
    fn object_cells_are_order_independent_and_apply_fractional_pivot() {
        let cells = [
            MeshVoxelCell {
                coordinate: [1, 0, 0],
                material_slot: 2,
            },
            MeshVoxelCell {
                coordinate: [0, 0, 0],
                material_slot: 2,
            },
        ];
        let reversed = [cells[1], cells[0]];
        let a = mesh_cells_standalone(0.5, [0.5, 0.0, 0.0], &cells, 100).unwrap();
        let b = mesh_cells_standalone(0.5, [0.5, 0.0, 0.0], &reversed, 100).unwrap();

        assert_eq!(a, b);
        assert_eq!(a.stats.quads, 6);
        assert_eq!(a.stats.source_faces, 10);
        assert_eq!(a.bounds.min, [-0.25, 0.0, 0.0]);
        assert_eq!(a.bounds.max, [0.75, 0.5, 0.5]);
    }

    #[test]
    fn object_cell_face_budget_fails_before_payload_allocation() {
        let cells = [MeshVoxelCell {
            coordinate: [0, 0, 0],
            material_slot: 1,
        }];
        let exact = mesh_cells_standalone(1.0, [0.0; 3], &cells, 6).unwrap();
        assert_eq!(exact.stats.source_faces, 6);
        assert_eq!(
            mesh_cells_standalone(1.0, [0.0; 3], &cells, 5),
            Err(MeshError::TooManyFaces { faces: 6, limit: 5 })
        );
    }

    fn reconstructed(mode: SurfaceMode, cells: &[MeshVoxelCell]) -> MeshPayload {
        mesh_cells_standalone_with_options(
            1.0,
            [0.0; 3],
            cells,
            SurfaceMeshOptions {
                mode,
                ..SurfaceMeshOptions::default()
            },
        )
        .unwrap()
    }

    fn assert_valid_reconstructed(mesh: &MeshPayload, mode: SurfaceMode) {
        assert_eq!(mesh.surface_mode, mode);
        assert_eq!(mesh.stats.surface_mode, mode);
        assert_eq!(
            mesh.tile_coordinates.len(),
            mesh.stats.vertices as usize * 2
        );
        assert_eq!(mesh.triangle_owners.len(), mesh.indices.len() / 3);
        assert_eq!(mesh.positions.len(), mesh.stats.vertices as usize * 3);
        assert_eq!(mesh.normals.len(), mesh.stats.vertices as usize * 3);
        assert_eq!(mesh.indices.len(), mesh.stats.indices as usize);
        assert_eq!(mesh.indices.len() % 3, 0);
        assert!(mesh.positions.iter().all(|value| value.is_finite()));
        assert!(mesh.normals.iter().all(|value| value.is_finite()));
        assert!(mesh
            .indices
            .iter()
            .all(|index| *index < mesh.stats.vertices));
        assert_eq!(
            mesh.groups.iter().map(|group| group.count).sum::<u32>(),
            mesh.stats.indices
        );
        for normal in mesh.normals.as_chunks::<3>().0.iter() {
            let length =
                (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
            assert!((length - 1.0).abs() < 1.0e-4, "normal={normal:?}");
        }
    }

    fn scalar_samples(
        dimensions: [usize; 3],
        mut value: impl FnMut(usize, usize, usize) -> f32,
    ) -> Vec<f32> {
        let mut samples = Vec::with_capacity(dimensions.into_iter().product());
        for z in 0..dimensions[2] {
            for y in 0..dimensions[1] {
                for x in 0..dimensions[0] {
                    samples.push(value(x, y, z));
                }
            }
        }
        samples
    }

    #[test]
    fn scalar_samples_interpolate_nonbinary_sloped_planes_at_the_crossing() {
        let mesh = mesh_scalar_samples(
            [0.0; 3],
            1.0,
            [3, 3, 3],
            &scalar_samples([3, 3, 3], |x, _, _| x as f32 - 0.25),
            0.0,
            SurfaceMeshLimits::default(),
        )
        .unwrap();

        assert_eq!(mesh.surface_mode, SurfaceMode::DualContouring);
        assert_eq!(mesh.groups.len(), 1);
        assert_eq!(mesh.groups[0].material_slot, 0);
        assert_eq!(mesh.groups[0].direction, None);
        assert_eq!(mesh.indices.len(), 6);
        for point in mesh.positions.as_chunks::<3>().0 {
            assert!((point[0] - 0.25).abs() < 1.0e-5, "point={point:?}");
        }
    }

    #[test]
    fn scalar_sample_origin_and_spacing_are_applied_to_output_positions() {
        let mesh = mesh_scalar_samples(
            [10.0, -4.0, 2.0],
            2.0,
            [3, 3, 3],
            &scalar_samples([3, 3, 3], |x, _, _| x as f32 - 0.25),
            0.0,
            SurfaceMeshLimits::default(),
        )
        .unwrap();

        for point in mesh.positions.as_chunks::<3>().0 {
            assert!((point[0] - 10.5).abs() < 1.0e-5, "point={point:?}");
        }
    }

    #[test]
    fn scalar_sphere_is_closed_and_topologically_outward() {
        let dimensions = [5, 5, 5];
        let samples = scalar_samples(dimensions, |x, y, z| {
            let dx = x as f32 - 2.0;
            let dy = y as f32 - 2.0;
            let dz = z as f32 - 2.0;
            (dx * dx + dy * dy + dz * dz).sqrt() - 1.6
        });
        let mesh = mesh_scalar_samples(
            [0.0; 3],
            1.0,
            dimensions,
            &samples,
            0.0,
            SurfaceMeshLimits::default(),
        )
        .unwrap();

        let mut edge_uses = BTreeMap::<(u32, u32), u32>::new();
        for triangle in mesh.indices.as_chunks::<3>().0 {
            for (left, right) in [
                (triangle[0], triangle[1]),
                (triangle[1], triangle[2]),
                (triangle[2], triangle[0]),
            ] {
                *edge_uses
                    .entry((left.min(right), left.max(right)))
                    .or_default() += 1;
            }
            let point = |index: u32| -> [f32; 3] {
                let offset = index as usize * 3;
                [
                    mesh.positions[offset],
                    mesh.positions[offset + 1],
                    mesh.positions[offset + 2],
                ]
            };
            let a = point(triangle[0]);
            let b = point(triangle[1]);
            let c = point(triangle[2]);
            let centroid = [
                (a[0] + b[0] + c[0]) / 3.0 - 2.0,
                (a[1] + b[1] + c[1]) / 3.0 - 2.0,
                (a[2] + b[2] + c[2]) / 3.0 - 2.0,
            ];
            assert!(dot(cross(sub(b, a), sub(c, a)), centroid) > 0.0);
        }
        assert!(!edge_uses.is_empty());
        assert!(edge_uses.values().all(|uses| *uses == 2));
    }

    #[test]
    fn scalar_boundary_crossings_remain_open_without_a_cap() {
        let mesh = mesh_scalar_samples(
            [0.0; 3],
            1.0,
            [3, 3, 3],
            &scalar_samples([3, 3, 3], |x, _, _| x as f32 - 0.25),
            0.0,
            SurfaceMeshLimits::default(),
        )
        .unwrap();

        let mut edge_uses = BTreeMap::<(u32, u32), u32>::new();
        for triangle in mesh.indices.as_chunks::<3>().0 {
            for (left, right) in [
                (triangle[0], triangle[1]),
                (triangle[1], triangle[2]),
                (triangle[2], triangle[0]),
            ] {
                *edge_uses
                    .entry((left.min(right), left.max(right)))
                    .or_default() += 1;
            }
        }
        assert!(edge_uses.values().any(|uses| *uses == 1));
    }

    #[test]
    fn scalar_sample_input_rejects_invalid_shape_values_and_limits() {
        assert!(matches!(
            mesh_scalar_samples(
                [0.0; 3],
                1.0,
                [1, 2, 2],
                &[],
                0.0,
                SurfaceMeshLimits::default()
            ),
            Err(MeshError::InvalidSampleDimensions { .. })
        ));
        assert!(matches!(
            mesh_scalar_samples(
                [0.0; 3],
                1.0,
                [2, 2, 2],
                &[0.0; 7],
                0.0,
                SurfaceMeshLimits::default()
            ),
            Err(MeshError::InvalidSampleCount {
                expected: 8,
                actual: 7
            })
        ));
        assert!(matches!(
            mesh_scalar_samples(
                [f64::NAN, 0.0, 0.0],
                1.0,
                [2, 2, 2],
                &[0.0; 8],
                0.0,
                SurfaceMeshLimits::default()
            ),
            Err(MeshError::InvalidScalarOrigin)
        ));
        assert!(matches!(
            mesh_scalar_samples(
                [0.0; 3],
                f64::NAN,
                [2, 2, 2],
                &[0.0; 8],
                0.0,
                SurfaceMeshLimits::default()
            ),
            Err(MeshError::InvalidCellSize)
        ));
        assert!(matches!(
            mesh_scalar_samples(
                [0.0; 3],
                1.0,
                [2, 2, 2],
                &[f32::NAN; 8],
                0.0,
                SurfaceMeshLimits::default()
            ),
            Err(MeshError::InvalidScalarSample)
        ));
        assert!(matches!(
            mesh_scalar_samples(
                [0.0; 3],
                1.0,
                [2, 2, 2],
                &[0.0; 8],
                f32::INFINITY,
                SurfaceMeshLimits::default()
            ),
            Err(MeshError::InvalidIsovalue)
        ));
        assert!(matches!(
            mesh_scalar_samples(
                [0.0; 3],
                1.0,
                [2, 2, 2],
                &[0.0; 8],
                0.0,
                SurfaceMeshLimits {
                    max_sampled_cells: 0,
                    ..SurfaceMeshLimits::default()
                },
            ),
            Err(MeshError::TooManySampledCells { cells: 1, limit: 0 })
        ));
    }

    fn fixture_hash(mesh: &MeshPayload) -> String {
        // FNV-1a keeps this regression fingerprint dependency-free and makes
        // the exact checked bytes visible through `to_fixture_string`.
        let fixture = format!("mode={:?}\n{}", mesh.surface_mode, mesh.to_fixture_string());
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        for byte in fixture.bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        format!("{hash:016x}")
    }

    #[test]
    fn every_surface_mode_matches_its_committed_golden_hash() {
        let cells = [
            MeshVoxelCell {
                coordinate: [-1, 0, 0],
                material_slot: 2,
            },
            MeshVoxelCell {
                coordinate: [0, 0, 0],
                material_slot: 1,
            },
            MeshVoxelCell {
                coordinate: [0, 1, 0],
                material_slot: 1,
            },
            MeshVoxelCell {
                coordinate: [0, 1, 1],
                material_slot: 3,
            },
        ];
        let actual = [
            fixture_hash(&reconstructed(SurfaceMode::GreedyCubes, &cells)),
            fixture_hash(&reconstructed(SurfaceMode::MarchingCubes, &cells)),
            fixture_hash(&reconstructed(SurfaceMode::DualContouring, &cells)),
        ];
        assert_eq!(
            actual,
            ["4b47c7382d4377e3", "8ba5fdd72bee82c0", "443126b87c391206"]
        );
    }

    #[test]
    fn reconstructed_modes_are_deterministic_for_adversarial_voxel_topologies() {
        let fixtures = [
            vec![MeshVoxelCell {
                coordinate: [-2, 3, -1],
                material_slot: 1,
            }],
            vec![
                MeshVoxelCell {
                    coordinate: [0, 0, 0],
                    material_slot: 1,
                },
                MeshVoxelCell {
                    coordinate: [1, 1, 1],
                    material_slot: 2,
                },
            ],
            (0..4)
                .flat_map(|x| {
                    (0..4).filter_map(move |z| {
                        (x != 1 || z != 1).then_some(MeshVoxelCell {
                            coordinate: [x, 0, z],
                            material_slot: if x < 2 { 3 } else { 4 },
                        })
                    })
                })
                .collect(),
            (0..3)
                .flat_map(|x| {
                    (0..3).flat_map(move |y| {
                        (0..3).filter_map(move |z| {
                            (x == 0 || x == 2 || y == 0 || y == 2 || z == 0 || z == 2).then_some(
                                MeshVoxelCell {
                                    coordinate: [x, y, z],
                                    material_slot: 5,
                                },
                            )
                        })
                    })
                })
                .collect(),
            vec![
                MeshVoxelCell {
                    coordinate: [-3, 0, 0],
                    material_slot: 6,
                },
                MeshVoxelCell {
                    coordinate: [3, 0, 0],
                    material_slot: 7,
                },
            ],
        ];
        for mode in [SurfaceMode::MarchingCubes, SurfaceMode::DualContouring] {
            for cells in &fixtures {
                let first = reconstructed(mode, cells);
                let mut reversed = cells.clone();
                reversed.reverse();
                let second = reconstructed(mode, &reversed);
                assert_eq!(first, second, "mode={mode:?} cells={cells:?}");
                assert_valid_reconstructed(&first, mode);
            }
        }
    }

    #[test]
    fn marching_cubes_exercises_every_binary_cube_case_without_invalid_geometry() {
        for case in 0_u16..=255 {
            let cells = (0..8)
                .filter(|corner| case & (1 << corner) != 0)
                .map(|corner| MeshVoxelCell {
                    coordinate: [
                        i64::from((corner == 1 || corner == 2 || corner == 5 || corner == 6) as u8),
                        i64::from((corner == 2 || corner == 3 || corner == 6 || corner == 7) as u8),
                        i64::from((corner >= 4) as u8),
                    ],
                    material_slot: 1,
                })
                .collect::<Vec<_>>();
            let mesh = reconstructed(SurfaceMode::MarchingCubes, &cells);
            assert_valid_reconstructed(&mesh, SurfaceMode::MarchingCubes);
        }
    }

    #[test]
    fn reconstructed_quotas_reject_prospectively() {
        let cells = [
            MeshVoxelCell {
                coordinate: [0, 0, 0],
                material_slot: 1,
            },
            MeshVoxelCell {
                coordinate: [100, 100, 100],
                material_slot: 1,
            },
        ];
        let error = mesh_cells_standalone_with_options(
            1.0,
            [0.0; 3],
            &cells,
            SurfaceMeshOptions {
                mode: SurfaceMode::DualContouring,
                limits: SurfaceMeshLimits {
                    max_sampled_cells: 1_000,
                    ..SurfaceMeshLimits::default()
                },
                ..SurfaceMeshOptions::default()
            },
        )
        .unwrap_err();
        assert!(matches!(error, MeshError::TooManySampledCells { .. }));

        let error = mesh_cells_standalone_with_options(
            1.0,
            [0.0; 3],
            &cells[..1],
            SurfaceMeshOptions {
                mode: SurfaceMode::MarchingCubes,
                limits: SurfaceMeshLimits {
                    max_vertices: 1,
                    ..SurfaceMeshLimits::default()
                },
                ..SurfaceMeshOptions::default()
            },
        )
        .unwrap_err();
        assert!(matches!(error, MeshError::TooManyVertices { .. }));
    }

    fn triangle_positions(mesh: &MeshPayload, translation: [f32; 3]) -> Vec<Vec<[i32; 3]>> {
        let mut triangles = mesh
            .indices
            .as_chunks::<3>()
            .0
            .iter()
            .map(|triangle| {
                let mut points = triangle
                    .iter()
                    .map(|index| {
                        let offset = *index as usize * 3;
                        std::array::from_fn(|axis| {
                            ((mesh.positions[offset + axis] + translation[axis]) * 1_000_000.0)
                                .round() as i32
                        })
                    })
                    .collect::<Vec<_>>();
                points.sort_unstable();
                points
            })
            .collect::<Vec<_>>();
        triangles.sort_unstable();
        triangles
    }

    #[test]
    fn reconstructed_world_chunks_match_one_global_surface_without_seam_cracks() {
        let mut world = VoxelWorld::new(spec());
        let mut left = VoxelChunk::from_spec(&spec());
        left.set(l(3, 1, 1), VoxelValue::solid_raw(1)).unwrap();
        let mut right = VoxelChunk::from_spec(&spec());
        right.set(l(0, 1, 1), VoxelValue::solid_raw(1)).unwrap();
        world.insert(ChunkCoord::new(0, 0, 0), left);
        world.insert(ChunkCoord::new(1, 0, 0), right);
        world.drain_dirty();

        let global_cells = [
            MeshVoxelCell {
                coordinate: [3, 1, 1],
                material_slot: 1,
            },
            MeshVoxelCell {
                coordinate: [4, 1, 1],
                material_slot: 1,
            },
        ];
        for mode in [SurfaceMode::MarchingCubes, SurfaceMode::DualContouring] {
            let options = SurfaceMeshOptions {
                mode,
                ..SurfaceMeshOptions::default()
            };
            let global =
                mesh_cells_standalone_with_options(1.0, [0.0; 3], &global_cells, options.clone())
                    .unwrap();
            let left = mesh_chunk_in_world_with_options(&world, ChunkCoord::new(0, 0, 0), &options)
                .unwrap()
                .unwrap();
            let right =
                mesh_chunk_in_world_with_options(&world, ChunkCoord::new(1, 0, 0), &options)
                    .unwrap()
                    .unwrap();
            let mut partitioned = triangle_positions(&left, [0.0; 3]);
            partitioned.extend(triangle_positions(&right, [4.0, 0.0, 0.0]));
            partitioned.sort_unstable();
            assert_eq!(
                partitioned,
                triangle_positions(&global, [0.0; 3]),
                "mode={mode:?}"
            );
        }
    }

    #[test]
    fn greedy_rectangles_preserve_exact_material_surface_with_holes_and_relief() {
        let mut cells = Vec::new();
        for y in 0..3 {
            for x in 0..3 {
                if [x, y] != [1, 1] {
                    cells.push(MeshVoxelCell {
                        coordinate: [x, y, 0],
                        material_slot: 1,
                    });
                }
            }
        }
        cells.extend([
            MeshVoxelCell {
                coordinate: [0, 0, 1],
                material_slot: 1,
            },
            MeshVoxelCell {
                coordinate: [4, 0, 0],
                material_slot: 2,
            },
            MeshVoxelCell {
                coordinate: [4, 1, 0],
                material_slot: 2,
            },
        ]);

        let faces = visible_faces(&cells);
        let quads = greedy_merge_faces(faces.iter().copied().collect()).unwrap();
        assert_eq!(expand_quads(&quads), faces);
        assert!(
            quads.len() < faces.len(),
            "the fixture must exercise actual merging"
        );
        assert!(quads.iter().any(|quad| quad.slot == 1));
        assert!(quads.iter().any(|quad| quad.slot == 2));

        let mesh = mesh_cells_standalone(1.0, [0.0; 3], &cells, 1_000).unwrap();
        assert_eq!(mesh.stats.source_faces as usize, faces.len());
        assert_eq!(mesh.stats.quads as usize, quads.len());
        assert_eq!(mesh.bounds.min, [0.0, 0.0, 0.0]);
        assert_eq!(mesh.bounds.max, [5.0, 3.0, 2.0]);
    }

    #[test]
    fn broad_same_material_wall_collapses_without_weakening_source_face_accounting() {
        let cells = (0..32)
            .flat_map(|y| {
                (0..48).map(move |x| MeshVoxelCell {
                    coordinate: [x, y, 0],
                    material_slot: 7,
                })
            })
            .collect::<Vec<_>>();
        let mesh = mesh_cells_standalone(0.25, [0.0; 3], &cells, 4_000).unwrap();

        assert_eq!(mesh.stats.source_faces, 3_232);
        assert_eq!(mesh.stats.quads, 6);
        assert_eq!(mesh.stats.vertices, 24);
        assert_eq!(mesh.bounds.max, [12.0, 8.0, 0.25]);
    }

    #[test]
    fn texture_mapping_spike_preserves_greedy_geometry_for_rectangles_and_material_borders() {
        let cases = [(1_i64, 1_i64, 6_u32, 6_u32), (7, 1, 30, 6), (5, 3, 46, 6)];
        for (width, height, source_faces, quads) in cases {
            let cells = (0..height)
                .flat_map(|y| {
                    (0..width).map(move |x| MeshVoxelCell {
                        coordinate: [x, y, 0],
                        material_slot: 1,
                    })
                })
                .collect::<Vec<_>>();
            let mesh = mesh_cells_standalone(1.0, [0.0; 3], &cells, 100).unwrap();
            assert_eq!(mesh.stats.source_faces, source_faces);
            assert_eq!(mesh.stats.quads, quads);
            assert_eq!(mesh.stats.vertices, quads * 4);
            assert_eq!(mesh.stats.indices, quads * 6);
        }

        let mixed = [
            MeshVoxelCell {
                coordinate: [0, 0, 0],
                material_slot: 1,
            },
            MeshVoxelCell {
                coordinate: [1, 0, 0],
                material_slot: 2,
            },
        ];
        let mesh = mesh_cells_standalone(1.0, [0.0; 3], &mixed, 100).unwrap();
        assert_eq!(mesh.stats.source_faces, 10);
        assert_eq!(mesh.stats.quads, 10);
        assert_eq!(mesh.groups.len(), 10);
    }

    #[test]
    fn wound_greedy_corners_have_nonmirrored_tile_space_on_all_six_faces() {
        for dir in Direction6::ALL {
            let quad = Quad {
                state: 0,
                slot: 1,
                coordinate: [-7, -5, -3],
                dir,
                u_length: 5,
                v_length: 3,
            };
            let corners = quad_corners(quad).unwrap();
            let tiles = project_voxel_surface_tile_corners(dir, corners, [0, 0, 0]).unwrap();
            let signed_area = tiles
                .iter()
                .zip(tiles.iter().cycle().skip(1))
                .take(4)
                .map(|(left, right)| left[0] * right[1] - left[1] * right[0])
                .sum::<f32>()
                * 0.5;
            assert!(
                signed_area > 0.0,
                "tile winding mirrored for {dir:?}: {tiles:?}"
            );
            let u_min = tiles
                .iter()
                .map(|point| point[0])
                .fold(f32::INFINITY, f32::min);
            let u_max = tiles
                .iter()
                .map(|point| point[0])
                .fold(f32::NEG_INFINITY, f32::max);
            let v_min = tiles
                .iter()
                .map(|point| point[1])
                .fold(f32::INFINITY, f32::min);
            let v_max = tiles
                .iter()
                .map(|point| point[1])
                .fold(f32::NEG_INFINITY, f32::max);
            assert_eq!((u_max - u_min) * (v_max - v_min), 15.0);
        }
    }

    #[test]
    fn independently_meshed_chunk_origins_share_one_texture_phase() {
        let left = project_voxel_surface_tile_corners(
            Direction6::PosZ,
            [[15, 0, 1], [16, 0, 1], [16, 1, 1], [15, 1, 1]],
            [-16, -8, 0],
        )
        .unwrap();
        let right = project_voxel_surface_tile_corners(
            Direction6::PosZ,
            [[0, 0, 1], [1, 0, 1], [1, 1, 1], [0, 1, 1]],
            [0, -8, 0],
        )
        .unwrap();
        assert_eq!([left[1], left[2]], [right[0], right[3]]);
    }

    #[test]
    fn production_mesh_stream_uses_world_chunk_origins_and_object_local_coordinates() {
        let chunk = chunk_with(&[(l(0, 0, 0), 1)]);
        let world_mesh = mesh_chunk_standalone(&spec(), ChunkCoord::new(1, 0, 0), &chunk).unwrap();
        let object_mesh = mesh_cells_standalone(
            1.0,
            [0.0; 3],
            &[MeshVoxelCell {
                coordinate: [0, 0, 0],
                material_slot: 1,
            }],
            10,
        )
        .unwrap();
        assert!(world_mesh.tile_coordinates.contains(&4.0));
        assert!(!object_mesh.tile_coordinates.contains(&4.0));

        let pivoted = mesh_cells_standalone(
            1.0,
            [0.75, -0.5, 2.0],
            &[MeshVoxelCell {
                coordinate: [0, 0, 0],
                material_slot: 1,
            }],
            10,
        )
        .unwrap();
        assert_eq!(object_mesh.tile_coordinates, pivoted.tile_coordinates);
        assert_ne!(object_mesh.positions, pivoted.positions);
    }

    #[test]
    fn production_mesh_stream_rejects_the_first_unrepresentable_rectangle() {
        let last = MeshVoxelCell {
            coordinate: [texture_mapping::MAX_EXACT_TILE_COORDINATE - 1, 0, 0],
            material_slot: 1,
        };
        let accepted = mesh_cells_standalone(1.0, [0.0; 3], &[last], 10).unwrap();
        assert_eq!(
            accepted.tile_coordinates.len(),
            accepted.stats.vertices as usize * 2
        );

        let first_rejected = MeshVoxelCell {
            coordinate: [texture_mapping::MAX_EXACT_TILE_COORDINATE, 0, 0],
            material_slot: 1,
        };
        assert!(matches!(
            mesh_cells_standalone(1.0, [0.0; 3], &[first_rejected], 10),
            Err(MeshError::TextureMapping(
                VoxelTextureMappingError::CoordinateOutOfExactRange { .. }
            ))
        ));
    }

    #[test]
    fn representative_corpora_record_exact_tile_attribute_cost() {
        let sparse = mesh_cells_standalone(
            1.0,
            [0.0; 3],
            &[MeshVoxelCell {
                coordinate: [0, 0, 0],
                material_slot: 1,
            }],
            10,
        )
        .unwrap();
        let solid_cells = (0..4)
            .flat_map(|x| (0..4).flat_map(move |y| (0..4).map(move |z| [x, y, z])))
            .map(|coordinate| MeshVoxelCell {
                coordinate,
                material_slot: 1,
            })
            .collect::<Vec<_>>();
        let solid = mesh_cells_standalone(1.0, [0.0; 3], &solid_cells, 512).unwrap();
        let checker_cells = (0..4)
            .flat_map(|x| (0..4).map(move |y| [x, y, 0]))
            .map(|coordinate| MeshVoxelCell {
                coordinate,
                material_slot: if (coordinate[0] + coordinate[1]) % 2 == 0 {
                    1
                } else {
                    2
                },
            })
            .collect::<Vec<_>>();
        let checker = mesh_cells_standalone(1.0, [0.0; 3], &checker_cells, 256).unwrap();
        let strip_cells = (0..128)
            .map(|x| MeshVoxelCell {
                coordinate: [x, 0, 0],
                material_slot: 1,
            })
            .collect::<Vec<_>>();
        let strip = mesh_cells_standalone(1.0, [0.0; 3], &strip_cells, 1024).unwrap();

        let mut world = VoxelWorld::new(spec());
        for x in 0..2 {
            let mut chunk = VoxelChunk::from_spec(&spec());
            chunk
                .fill_region(l(0, 0, 0), l(4, 4, 4), VoxelValue::solid_raw(1))
                .unwrap();
            world.insert(ChunkCoord::new(x, 0, 0), chunk);
        }
        world.drain_dirty();
        let multi = (0..2)
            .map(|x| {
                mesh_chunk_in_world(&world, ChunkCoord::new(x, 0, 0))
                    .unwrap()
                    .unwrap()
            })
            .collect::<Vec<_>>();

        let measurements = [
            (
                "sparse",
                sparse.stats.quads,
                sparse.stats.vertices,
                sparse.stats.indices,
                sparse.tile_coordinates.len() * 4,
            ),
            (
                "solid",
                solid.stats.quads,
                solid.stats.vertices,
                solid.stats.indices,
                solid.tile_coordinates.len() * 4,
            ),
            (
                "checker",
                checker.stats.quads,
                checker.stats.vertices,
                checker.stats.indices,
                checker.tile_coordinates.len() * 4,
            ),
            (
                "strip",
                strip.stats.quads,
                strip.stats.vertices,
                strip.stats.indices,
                strip.tile_coordinates.len() * 4,
            ),
            (
                "multi",
                multi.iter().map(|mesh| mesh.stats.quads).sum(),
                multi.iter().map(|mesh| mesh.stats.vertices).sum(),
                multi.iter().map(|mesh| mesh.stats.indices).sum(),
                multi
                    .iter()
                    .map(|mesh| mesh.tile_coordinates.len() * 4)
                    .sum(),
            ),
        ];
        assert_eq!(
            measurements,
            [
                ("sparse", 6, 24, 36, 192),
                ("solid", 6, 24, 36, 192),
                ("checker", 48, 192, 288, 1536),
                ("strip", 6, 24, 36, 192),
                ("multi", 10, 40, 60, 320),
            ]
        );
        for (_, _, vertices, _, uv_bytes) in measurements {
            assert_eq!(uv_bytes, vertices as usize * 2 * 4);
        }
    }

    #[test]
    fn atlas_repeat_spike_keeps_each_region_isolated_over_an_n_by_m_quad() {
        let regions = [
            ([0.0_f64, 0.0_f64], [32.0_f64, 16.0_f64]),
            ([40.0_f64, 8.0_f64], [16.0_f64, 24.0_f64]),
        ];
        for (minimum, extent) in regions {
            for tile in [[-3.25, -1.5], [0.0, 0.0], [2.75, 1.25], [5.0, 3.0]] {
                let repeated = repeat_voxel_tile_coordinate(tile, [1.0, 1.0], [0.0, 0.0]).unwrap();
                let uv = [
                    (minimum[0] + 0.5 + repeated[0] * (extent[0] - 1.0)) / 64.0,
                    (minimum[1] + 0.5 + repeated[1] * (extent[1] - 1.0)) / 64.0,
                ];
                let safe_min = [(minimum[0] + 0.5) / 64.0, (minimum[1] + 0.5) / 64.0];
                let safe_max = [
                    (minimum[0] + extent[0] - 0.5) / 64.0,
                    (minimum[1] + extent[1] - 0.5) / 64.0,
                ];
                assert!((safe_min[0]..=safe_max[0]).contains(&uv[0]));
                assert!((safe_min[1]..=safe_max[1]).contains(&uv[1]));
            }
        }
    }

    fn visible_faces(cells: &[MeshVoxelCell]) -> BTreeSet<Face> {
        let occupied = cells
            .iter()
            .map(|cell| (cell.coordinate, cell.material_slot))
            .collect::<BTreeMap<_, _>>();
        let mut faces = BTreeSet::new();
        for (&coordinate, &slot) in &occupied {
            for dir in Direction6::ALL {
                let offset = dir.offset();
                let neighbour = [
                    coordinate[0] + i64::from(offset[0]),
                    coordinate[1] + i64::from(offset[1]),
                    coordinate[2] + i64::from(offset[2]),
                ];
                if !occupied.contains_key(&neighbour) {
                    faces.insert(Face {
                        state: 0,
                        slot,
                        coordinate,
                        dir,
                    });
                }
            }
        }
        faces
    }

    fn expand_quads(quads: &[Quad]) -> BTreeSet<Face> {
        let mut faces = BTreeSet::new();
        for quad in quads {
            let (u_axis, v_axis) = in_plane_axes(quad.dir);
            for v_offset in 0..quad.v_length {
                for u_offset in 0..quad.u_length {
                    let mut coordinate = quad.coordinate;
                    coordinate[u_axis] += i64::from(u_offset);
                    coordinate[v_axis] += i64::from(v_offset);
                    faces.insert(Face {
                        state: 0,
                        slot: quad.slot,
                        coordinate,
                        dir: quad.dir,
                    });
                }
            }
        }
        faces
    }
}
