//! Deterministic greedy visible-surface voxel mesher → [`MeshPayload`].
//!
//! # Lane
//!
//! `rust-service` — turns chunk voxel data (`svc-volume`) into a renderable
//! [`MeshPayload`] (voxel-capability-06). The payload layout is co-designed with
//! the render protocol (#2262) and the Three.js upload path (#2263); see
//! `docs/voxel-mesh-seam.md` / ADR 0007. It does not own the protocol contract or
//! touch the renderer.
//!
//! # This implementation
//!
//! Every solid voxel contributes the faces whose neighbour is non-opaque;
//! internal faces (and border faces against resident neighbour chunks) are
//! culled. Remaining coplanar faces with the same material and normal are
//! greedily merged into deterministic rectangles. Runtime texture coordinates
//! use the executable outward-facing basis in [`texture_mapping`]; emitting the
//! selected tile-space vertex stream remains the VTX2 follow-on.
//!
//! Output is **deterministic**: material slot, `Direction6`, plane, row, and
//! column order are all explicit. Rectangle growth prefers the positive
//! in-plane `u` axis before the positive `v` axis. Separate `f32`
//! position/normal streams + a `u32` index stream form a 1:1 `BufferGeometry`
//! match. Vertices are **chunk-local** (origin = chunk min corner); world
//! placement is the render node transform.

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};

use core_space::{ChunkCoord, Direction6, VoxelCoord, VoxelGridSpec};
use svc_spatial::VoxelWorld;
use svc_volume::VoxelChunk;
use texture_mapping::{project_voxel_surface_tile_point, VoxelTextureMappingError};

pub mod texture_mapping;

/// One contiguous run of indices sharing a material slot — maps 1:1 to a
/// `THREE.BufferGeometry` group (`addGroup(start, count, materialIndex)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeshGroup {
    pub material_slot: u16,
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
    pub vertices: u32,
    pub indices: u32,
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
}

/// A renderable mesh for one chunk: separate `f32` attribute streams, a `u32`
/// index stream, material-slot groups, bounds, and stats (ADR 0007).
#[derive(Debug, Clone, PartialEq)]
pub struct MeshPayload {
    /// 3 `f32` per vertex (chunk-local).
    pub positions: Vec<f32>,
    /// 3 `f32` per vertex (outward face normal).
    pub normals: Vec<f32>,
    /// 2 signed `f32` cell-space coordinates per vertex. World chunks use
    /// absolute voxel coordinates; voxel objects use object-local coordinates.
    pub tile_coordinates: Vec<f32>,
    /// 3 `u32` per triangle.
    pub indices: Vec<u32>,
    /// Groups in ascending `material_slot` order; their `count`s tile `indices`.
    pub groups: Vec<MeshGroup>,
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
    /// The chunk would emit more vertices than a `u32` index can address.
    TooManyVertices {
        vertices: u64,
    },
    TooManyFaces {
        faces: u64,
        limit: u32,
    },
    InvalidCellSize,
    InvalidPivot,
    DuplicateCell {
        coordinate: [i64; 3],
    },
    PositionOutOfRange,
    TextureMapping(VoxelTextureMappingError),
}

impl core::fmt::Display for MeshError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            MeshError::TooManyVertices { vertices } => {
                write!(
                    f,
                    "mesh would need {vertices} vertices, exceeding u32 index range"
                )
            }
            MeshError::TooManyFaces { faces, limit } => {
                write!(f, "mesh would emit {faces} faces; limit is {limit}")
            }
            MeshError::InvalidCellSize => write!(f, "cell size must be finite and positive"),
            MeshError::InvalidPivot => write!(f, "pivot components must be finite"),
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
    slot: u16,
    coordinate: [i64; 3],
    dir: Direction6,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Quad {
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
    let mut planes: BTreeMap<(u16, Direction6, i64), BTreeSet<(i64, i64)>> = BTreeMap::new();
    for face in faces {
        let axis = face.dir.axis().index();
        let (u_axis, v_axis) = in_plane_axes(face.dir);
        planes
            .entry((face.slot, face.dir, face.coordinate[axis]))
            .or_default()
            .insert((face.coordinate[v_axis], face.coordinate[u_axis]));
    }

    let mut quads = Vec::new();
    for ((slot, dir, plane), mut cells) in planes {
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
    mesh_core(spec, coord, chunk, |v| {
        let (c, l) = spec.voxel_to_chunk_local(v);
        c == coord && chunk.get(l).is_some_and(|x| x.is_opaque())
    })
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

    let mut faces = Vec::new();
    let mut faces_culled = 0_u32;
    for (&coordinate, &slot) in &occupied {
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
            if occupied.contains_key(&neighbour) {
                faces_culled = faces_culled.saturating_add(1);
            } else {
                let face_count = faces.len() as u64 + 1;
                if face_count > u64::from(max_faces) {
                    return Err(MeshError::TooManyFaces {
                        faces: face_count,
                        limit: max_faces,
                    });
                }
                faces.push(Face {
                    slot,
                    coordinate,
                    dir,
                });
            }
        }
    }

    let source_faces = faces.len() as u32;
    let quads = greedy_merge_faces(faces)?;
    emit_quads(&quads, cell_size, pivot, [0; 3], source_faces, faces_culled)
}

/// Mesh a resident chunk using its **resident neighbour chunks** for border
/// culling (faces against a non-resident/absent neighbour are emitted). Returns
/// `None` if `coord` is not resident in `world`.
pub fn mesh_chunk_in_world(
    world: &VoxelWorld,
    coord: ChunkCoord,
) -> Option<Result<MeshPayload, MeshError>> {
    let chunk = world.get(coord)?;
    let spec = world.grid();
    Some(mesh_core(&spec, coord, chunk, |v| {
        let (c, l) = spec.voxel_to_chunk_local(v);
        world
            .get(c)
            .and_then(|ch| ch.get(l))
            .is_some_and(|x| x.is_opaque())
    }))
}

/// Core mesher: `occupied(world_voxel)` answers whether a voxel is opaque (used
/// for face culling). The current chunk's solid voxels drive emission.
fn mesh_core(
    spec: &VoxelGridSpec,
    coord: ChunkCoord,
    chunk: &VoxelChunk,
    occupied: impl Fn(VoxelCoord) -> bool,
) -> Result<MeshPayload, MeshError> {
    // Collect visible faces in deterministic order, with culling stats.
    let mut faces: Vec<Face> = Vec::new();
    let mut faces_culled = 0u32;
    for (local, value) in chunk.iter() {
        let Some(material) = value.material() else {
            continue;
        };
        let world_voxel = spec.chunk_local_to_voxel(coord, local);
        for dir in Direction6::ALL {
            if occupied(world_voxel.neighbor(dir)) {
                faces_culled += 1;
            } else {
                faces.push(Face {
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
    )
}

fn emit_quads(
    quads: &[Quad],
    cell_size: f64,
    pivot: [f64; 3],
    texture_coordinate_origin: [i64; 3],
    source_faces: u32,
    faces_culled: u32,
) -> Result<MeshPayload, MeshError> {
    let vertex_count = quads.len() as u64 * 4;
    if vertex_count > u32::MAX as u64 {
        return Err(MeshError::TooManyVertices {
            vertices: vertex_count,
        });
    }

    let mut positions: Vec<f32> = Vec::with_capacity(quads.len() * 12);
    let mut normals: Vec<f32> = Vec::with_capacity(quads.len() * 12);
    let mut tile_coordinates: Vec<f32> = Vec::with_capacity(quads.len() * 8);
    let mut indices: Vec<u32> = Vec::with_capacity(quads.len() * 6);
    let mut groups: Vec<MeshGroup> = Vec::new();
    let mut bmin = [f32::INFINITY; 3];
    let mut bmax = [f32::NEG_INFINITY; 3];

    let mut cur_slot: Option<u16> = None;
    let mut group_start: u32 = 0;
    for quad in quads {
        if cur_slot != Some(quad.slot) {
            if let Some(slot) = cur_slot {
                groups.push(MeshGroup {
                    material_slot: slot,
                    start: group_start,
                    count: indices.len() as u32 - group_start,
                });
            }
            cur_slot = Some(quad.slot);
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
            tile_coordinates.extend_from_slice(
                &project_voxel_surface_tile_point(quad.dir, point, texture_coordinate_origin)
                    .map_err(MeshError::TextureMapping)?,
            );
        }
        // Two CCW triangles of the quad: (0,1,2) (0,2,3).
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    if let Some(slot) = cur_slot {
        groups.push(MeshGroup {
            material_slot: slot,
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
        vertices: (positions.len() / 3) as u32,
        indices: indices.len() as u32,
        quads: quads.len() as u32,
        faces_emitted: quads.len() as u32,
        source_faces,
        faces_culled,
    };
    Ok(MeshPayload {
        positions,
        normals,
        tile_coordinates,
        indices,
        groups,
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
            let _ = writeln!(
                s,
                "group slot={} start={} count={}",
                g.material_slot, g.start, g.count
            );
        }
        for (i, p) in self.positions.chunks_exact(3).enumerate() {
            let n = &self.normals[i * 3..i * 3 + 3];
            let _ = writeln!(s, "v{i} pos={:?} nrm={:?}", p, n);
        }
        for (t, tri) in self.indices.chunks_exact(3).enumerate() {
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
        assert_eq!(
            m.groups,
            vec![MeshGroup {
                material_slot: 1,
                start: 0,
                count: 36
            }]
        );
    }

    #[test]
    fn emitted_winding_matches_emitted_normal() {
        let c = chunk_with(&[(l(1, 1, 1), 1), (l(2, 1, 1), 1)]);
        let m = mesh_chunk_standalone(&spec(), ChunkCoord::ORIGIN, &c).unwrap();
        for tri in m.indices.chunks_exact(3) {
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
    fn faces_are_grouped_by_material_slot() {
        let c = chunk_with(&[(l(0, 0, 0), 3), (l(2, 2, 2), 1)]);
        let m = mesh_chunk_standalone(&spec(), ChunkCoord::ORIGIN, &c).unwrap();
        assert_eq!(m.groups.len(), 2);
        assert_eq!(m.groups[0].material_slot, 1); // ascending
        assert_eq!(m.groups[1].material_slot, 3);
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
        assert_eq!(mesh.groups.len(), 2);
    }

    #[test]
    fn wound_greedy_corners_have_nonmirrored_tile_space_on_all_six_faces() {
        for dir in Direction6::ALL {
            let quad = Quad {
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
