//! Deterministic visible-face voxel mesher → [`MeshPayload`].
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
//! **Naive visible-face** meshing: every solid voxel emits the faces whose
//! neighbour is non-opaque; internal faces (and border faces against resident
//! neighbour chunks) are culled. Greedy/face merging, UV/atlas, and interleaved
//! buffers are deferred (ADR 0007 non-goals).
//!
//! Output is **deterministic**: voxels in `core-space` X-fastest order, faces in
//! `Direction6::ALL` order, faces grouped by ascending `material_slot`. Separate
//! `f32` position/normal streams + a `u32` index stream — a 1:1 `BufferGeometry`
//! match. Vertices are **chunk-local** (origin = chunk min corner); world
//! placement is the render node transform.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use core_space::{ChunkCoord, Direction6, VoxelCoord, VoxelGridSpec};
use svc_spatial::VoxelWorld;
use svc_volume::VoxelChunk;

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
    /// Emitted faces (quads).
    pub quads: u32,
    pub faces_emitted: u32,
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
        }
    }
}

impl std::error::Error for MeshError {}

// ── Face geometry ──────────────────────────────────────────────────────────────

/// The four corner offsets (in `{0,1}` voxel units) of the face on `dir`, wound
/// CCW so the polygon normal points outward along `dir`.
fn face_corners(dir: Direction6) -> [[u32; 3]; 4] {
    let a = dir.axis().index();
    // The two in-plane axes ordered so `u × v = +a` (right-handed), making the
    // CCW loop's normal point along `+a` for positive faces.
    let (u, v) = match a {
        0 => (1, 2), // X: Y,Z  (Y×Z = X)
        1 => (2, 0), // Y: Z,X  (Z×X = Y)
        _ => (0, 1), // Z: X,Y  (X×Y = Z)
    };
    let fixed = if dir.is_positive() { 1 } else { 0 };
    // CCW loop in the (u,v) plane.
    let loop_uv = [(0u32, 0u32), (1, 0), (1, 1), (0, 1)];
    let mut out = [[0u32; 3]; 4];
    for (i, (uu, vv)) in loop_uv.iter().enumerate() {
        out[i][a] = fixed;
        out[i][u] = *uu;
        out[i][v] = *vv;
    }
    // Flip winding for negative faces so the normal still points outward.
    if !dir.is_positive() {
        out.swap(1, 3);
    }
    out
}

#[derive(Debug, Clone, Copy)]
struct Face {
    slot: u16,
    coordinate: [i64; 3],
    dir: Direction6,
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

    emit_faces(&mut faces, cell_size, pivot, faces_culled)
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

    emit_faces(&mut faces, spec.voxel_size(), [0.0; 3], faces_culled)
}

fn emit_faces(
    faces: &mut [Face],
    cell_size: f64,
    pivot: [f64; 3],
    faces_culled: u32,
) -> Result<MeshPayload, MeshError> {
    // Group by material slot (stable sort preserves voxel/face order within a slot).
    // Stable sorting retains each input lane's canonical cell/direction order.
    faces.sort_by_key(|face| face.slot);

    let vertex_count = faces.len() as u64 * 4;
    if vertex_count > u32::MAX as u64 {
        return Err(MeshError::TooManyVertices {
            vertices: vertex_count,
        });
    }

    let mut positions: Vec<f32> = Vec::with_capacity(faces.len() * 12);
    let mut normals: Vec<f32> = Vec::with_capacity(faces.len() * 12);
    let mut indices: Vec<u32> = Vec::with_capacity(faces.len() * 6);
    let mut groups: Vec<MeshGroup> = Vec::new();
    let mut bmin = [f32::INFINITY; 3];
    let mut bmax = [f32::NEG_INFINITY; 3];

    let mut cur_slot: Option<u16> = None;
    let mut group_start: u32 = 0;
    for face in faces.iter() {
        if cur_slot != Some(face.slot) {
            if let Some(slot) = cur_slot {
                groups.push(MeshGroup {
                    material_slot: slot,
                    start: group_start,
                    count: indices.len() as u32 - group_start,
                });
            }
            cur_slot = Some(face.slot);
            group_start = indices.len() as u32;
        }

        let base = (positions.len() / 3) as u32;
        let normal = face.dir.normal();
        let [nx, ny, nz] = [normal.x as f32, normal.y as f32, normal.z as f32];
        for corner in face_corners(face.dir) {
            let mut p = [0.0_f32; 3];
            for axis in 0..3 {
                let value = ((face.coordinate[axis] as f64 + f64::from(corner[axis]))
                    - pivot[axis])
                    * cell_size;
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

    let bounds = if faces.is_empty() {
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
        quads: faces.len() as u32,
        faces_emitted: faces.len() as u32,
        faces_culled,
    };
    Ok(MeshPayload {
        positions,
        normals,
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
            "mesh v={} i={} quads={} emitted={} culled={}",
            st.vertices, st.indices, st.quads, st.faces_emitted, st.faces_culled
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
        let c = chunk_with(&[(l(1, 1, 1), 1)]);
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
        // 12 potential faces, 2 shared (one each side) culled → 10 emitted.
        assert_eq!(m.stats.quads, 10);
        assert_eq!(m.stats.faces_culled, 2);
    }

    #[test]
    fn full_solid_chunk_emits_only_the_exterior_shell() {
        let mut c = VoxelChunk::from_spec(&spec());
        c.fill_region(l(0, 0, 0), l(4, 4, 4), VoxelValue::solid_raw(1))
            .unwrap();
        let m = mesh_chunk_standalone(&spec(), ChunkCoord::ORIGIN, &c).unwrap();
        assert_eq!(m.stats.quads, 6 * 4 * 4); // exterior shell only
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
        assert_eq!(a.stats.quads, 10);
        assert_eq!(a.bounds.min, [-0.25, 0.0, 0.0]);
        assert_eq!(a.bounds.max, [0.75, 0.5, 0.5]);
    }

    #[test]
    fn object_cell_face_budget_fails_before_payload_allocation() {
        let cells = [MeshVoxelCell {
            coordinate: [0, 0, 0],
            material_slot: 1,
        }];
        assert_eq!(
            mesh_cells_standalone(1.0, [0.0; 3], &cells, 5),
            Err(MeshError::TooManyFaces { faces: 6, limit: 5 })
        );
    }
}
