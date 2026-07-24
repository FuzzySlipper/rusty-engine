//! Grid specs and the explicit world↔voxel↔chunk/local conversions.
//!
//! All conversion between continuous world space and integer grid space goes
//! through a [`VoxelGridSpec`]. There is intentionally no free function that
//! converts a [`WorldPos`] to a [`VoxelCoord`] without a spec — the grid scale is
//! never implicit, so terrain/object/local grids of different resolutions coexist.

use crate::voxel::{ChunkCoord, LocalVoxelCoord, VoxelCoord};
use crate::world::{WorldPos, WorldScalar};
use crate::{floor_div, rem_euclid};

/// Integer address of one cell in a generic spatial grid.
///
/// This is deliberately distinct from [`VoxelCoord`]. Editor guides, tiles,
/// navigation overlays, and voxel storage may share conversion semantics
/// without pretending that every grid cell is a stored voxel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SpatialCellCoord {
    pub x: i64,
    pub y: i64,
    pub z: i64,
}

impl SpatialCellCoord {
    pub const ORIGIN: Self = Self { x: 0, y: 0, z: 0 };

    pub const fn new(x: i64, y: i64, z: i64) -> Self {
        Self { x, y, z }
    }

    pub const fn to_array(self) -> [i64; 3] {
        [self.x, self.y, self.z]
    }
}

/// Shared axis-aligned world↔cell conversion semantics.
///
/// ASHA world space is right-handed and Y-up. `origin_world` is the minimum
/// corner of cell `(0,0,0)`, and `cell_size` is explicit per axis so no caller
/// can accidentally inherit a global grid scale. Grid lines therefore lie on
/// `origin + n * cell_size`; cell centers add exactly half a cell.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpatialGridSpec {
    origin_world: WorldPos,
    cell_size: [WorldScalar; 3],
}

impl SpatialGridSpec {
    /// Construct a grid with an explicit origin and positive finite cell size.
    pub fn new(origin_world: WorldPos, cell_size: [WorldScalar; 3]) -> Option<Self> {
        if !origin_world
            .to_array()
            .iter()
            .all(|value| value.is_finite())
            || !cell_size
                .iter()
                .all(|value| value.is_finite() && *value > 0.0)
        {
            return None;
        }
        Some(Self {
            origin_world,
            cell_size,
        })
    }

    /// Construct a uniform grid at the world origin.
    pub fn uniform(cell_size: WorldScalar) -> Option<Self> {
        Self::new(WorldPos::ORIGIN, [cell_size; 3])
    }

    pub const fn origin_world(self) -> WorldPos {
        self.origin_world
    }

    pub const fn cell_size(self) -> [WorldScalar; 3] {
        self.cell_size
    }

    /// Return a copy rebased to another world-space minimum corner.
    pub fn with_origin(self, origin_world: WorldPos) -> Option<Self> {
        Self::new(origin_world, self.cell_size)
    }

    /// World position → containing cell using origin-relative floor semantics.
    pub fn world_to_cell(self, pos: WorldPos) -> SpatialCellCoord {
        SpatialCellCoord::new(
            floor_world_axis(pos.x - self.origin_world.x, self.cell_size[0]),
            floor_world_axis(pos.y - self.origin_world.y, self.cell_size[1]),
            floor_world_axis(pos.z - self.origin_world.z, self.cell_size[2]),
        )
    }

    /// World-space minimum corner of a cell.
    pub fn cell_min_world(self, cell: SpatialCellCoord) -> WorldPos {
        WorldPos::new(
            self.origin_world.x + cell.x as WorldScalar * self.cell_size[0],
            self.origin_world.y + cell.y as WorldScalar * self.cell_size[1],
            self.origin_world.z + cell.z as WorldScalar * self.cell_size[2],
        )
    }

    /// World-space center of a cell.
    pub fn cell_center_world(self, cell: SpatialCellCoord) -> WorldPos {
        let min = self.cell_min_world(cell);
        WorldPos::new(
            min.x + self.cell_size[0] * 0.5,
            min.y + self.cell_size[1] * 0.5,
            min.z + self.cell_size[2] * 0.5,
        )
    }

    /// World-space `(min, max)` cell corners; max is the exclusive extent.
    pub fn cell_bounds_world(self, cell: SpatialCellCoord) -> (WorldPos, WorldPos) {
        let min = self.cell_min_world(cell);
        (
            min,
            WorldPos::new(
                min.x + self.cell_size[0],
                min.y + self.cell_size[1],
                min.z + self.cell_size[2],
            ),
        )
    }

    /// Snap to the nearest grid boundary/intersection, relative to the origin.
    /// Exact half-cell ties choose the boundary in the positive-axis direction.
    pub fn snap_to_boundary(self, pos: WorldPos) -> WorldPos {
        WorldPos::new(
            snap_boundary_axis(pos.x, self.origin_world.x, self.cell_size[0]),
            snap_boundary_axis(pos.y, self.origin_world.y, self.cell_size[1]),
            snap_boundary_axis(pos.z, self.origin_world.z, self.cell_size[2]),
        )
    }

    /// Snap to the center of the cell containing the position.
    pub fn snap_to_cell_center(self, pos: WorldPos) -> WorldPos {
        self.cell_center_world(self.world_to_cell(pos))
    }
}

#[inline]
fn floor_world_axis(world_delta: WorldScalar, cell_size: WorldScalar) -> i64 {
    (world_delta / cell_size).floor() as i64
}

#[inline]
fn snap_boundary_axis(
    value: WorldScalar,
    origin: WorldScalar,
    cell_size: WorldScalar,
) -> WorldScalar {
    let boundary = ((value - origin) / cell_size + 0.5).floor();
    origin + boundary * cell_size
}

/// Identifies which voxel grid a spec describes, so multiple grids (terrain,
/// object, local) can be distinguished at call sites and in storage keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GridId(pub u32);

impl GridId {
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u32 {
        self.0
    }
}

/// The voxel dimensions of a chunk, per axis. Each axis is `>= 1`; chunks may be
/// non-cubic (e.g. tall terrain columns) — no global cubic-chunk assumption.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkDims {
    x: u32,
    y: u32,
    z: u32,
}

impl ChunkDims {
    /// Construct chunk dimensions. Returns `None` if any axis is zero.
    pub const fn new(x: u32, y: u32, z: u32) -> Option<Self> {
        if x == 0 || y == 0 || z == 0 {
            None
        } else {
            Some(Self { x, y, z })
        }
    }

    /// A cubic chunk `n × n × n`. Returns `None` if `n == 0`.
    pub const fn cubic(n: u32) -> Option<Self> {
        Self::new(n, n, n)
    }

    pub const fn x(self) -> u32 {
        self.x
    }
    pub const fn y(self) -> u32 {
        self.y
    }
    pub const fn z(self) -> u32 {
        self.z
    }

    pub const fn to_array(self) -> [u32; 3] {
        [self.x, self.y, self.z]
    }

    /// Total voxels in one chunk.
    pub const fn volume(self) -> u64 {
        self.x as u64 * self.y as u64 * self.z as u64
    }

    const fn axis(self, index: usize) -> u32 {
        match index {
            0 => self.x,
            1 => self.y,
            _ => self.z,
        }
    }
}

/// Describes a voxel grid: its voxel size, chunk shape, identity, and world
/// origin. The single context object for every world↔grid conversion.
///
/// `origin_world` is the rebasing hook: it is the world position of voxel
/// `(0,0,0)`'s minimum corner. Today it defaults to the world origin; a future
/// rebasing pass can shift it without changing any conversion call site.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VoxelGridSpec {
    id: GridId,
    chunk_dims: ChunkDims,
    spatial: SpatialGridSpec,
}

impl VoxelGridSpec {
    /// Construct a grid spec. Returns `None` if `voxel_size` is not a positive,
    /// finite number.
    pub fn new(id: GridId, voxel_size: WorldScalar, chunk_dims: ChunkDims) -> Option<Self> {
        let spatial = SpatialGridSpec::uniform(voxel_size)?;
        Some(Self {
            id,
            chunk_dims,
            spatial,
        })
    }

    /// Return a copy with an explicit world origin (rebasing hook).
    pub fn with_origin(mut self, origin_world: WorldPos) -> Self {
        self.spatial = self
            .spatial
            .with_origin(origin_world)
            .expect("existing finite cell size plus finite origin must remain a valid grid");
        self
    }

    pub const fn id(self) -> GridId {
        self.id
    }
    pub const fn voxel_size(self) -> WorldScalar {
        self.spatial.cell_size()[0]
    }
    pub const fn chunk_dims(self) -> ChunkDims {
        self.chunk_dims
    }
    pub const fn origin_world(self) -> WorldPos {
        self.spatial.origin_world()
    }

    // ── world ↔ voxel ─────────────────────────────────────────────────────────

    /// World position → the voxel cell that contains it (floor, origin-relative).
    pub fn world_to_voxel(self, pos: WorldPos) -> VoxelCoord {
        let cell = self.spatial.world_to_cell(pos);
        VoxelCoord::new(cell.x, cell.y, cell.z)
    }

    /// World position of a voxel's minimum corner.
    pub fn voxel_min_world(self, v: VoxelCoord) -> WorldPos {
        self.spatial
            .cell_min_world(SpatialCellCoord::new(v.x, v.y, v.z))
    }

    /// World position of a voxel's center.
    pub fn voxel_center_world(self, v: VoxelCoord) -> WorldPos {
        self.spatial
            .cell_center_world(SpatialCellCoord::new(v.x, v.y, v.z))
    }

    /// World-space `(min, max)` corners of a voxel cell (`max` exclusive extent).
    pub fn voxel_bounds_world(self, v: VoxelCoord) -> (WorldPos, WorldPos) {
        self.spatial
            .cell_bounds_world(SpatialCellCoord::new(v.x, v.y, v.z))
    }

    // ── voxel ↔ chunk / local ──────────────────────────────────────────────────

    /// Which chunk a voxel belongs to (floor division by chunk dims, per axis).
    pub fn voxel_to_chunk(self, v: VoxelCoord) -> ChunkCoord {
        let d = self.chunk_dims;
        ChunkCoord::new(
            floor_div(v.x, d.x() as i64),
            floor_div(v.y, d.y() as i64),
            floor_div(v.z, d.z() as i64),
        )
    }

    /// The voxel's address within its chunk (always in `0..chunk_dims`).
    pub fn voxel_to_local(self, v: VoxelCoord) -> LocalVoxelCoord {
        let d = self.chunk_dims;
        LocalVoxelCoord::new(
            rem_euclid(v.x, d.x() as i64) as u32,
            rem_euclid(v.y, d.y() as i64) as u32,
            rem_euclid(v.z, d.z() as i64) as u32,
        )
    }

    /// Both halves of the split at once.
    pub fn voxel_to_chunk_local(self, v: VoxelCoord) -> (ChunkCoord, LocalVoxelCoord) {
        (self.voxel_to_chunk(v), self.voxel_to_local(v))
    }

    /// Reassemble a voxel coordinate from its chunk + local parts.
    ///
    /// `local` is assumed to be within `chunk_dims`; out-of-range locals simply
    /// address a voxel in an adjacent chunk (the arithmetic stays consistent).
    pub fn chunk_local_to_voxel(self, c: ChunkCoord, local: LocalVoxelCoord) -> VoxelCoord {
        let d = self.chunk_dims;
        VoxelCoord::new(
            c.x * d.x() as i64 + local.x as i64,
            c.y * d.y() as i64 + local.y as i64,
            c.z * d.z() as i64 + local.z as i64,
        )
    }

    /// The minimum (origin) voxel of a chunk.
    pub fn chunk_origin_voxel(self, c: ChunkCoord) -> VoxelCoord {
        self.chunk_local_to_voxel(c, LocalVoxelCoord::ORIGIN)
    }

    /// `true` if `local` is within this grid's chunk dimensions.
    pub fn local_in_bounds(self, local: LocalVoxelCoord) -> bool {
        let [lx, ly, lz] = local.to_array();
        lx < self.chunk_dims.axis(0) && ly < self.chunk_dims.axis(1) && lz < self.chunk_dims.axis(2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::region::VoxelRegion;
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct ConformanceFixture {
        coordinate_system: String,
        cases: Vec<ConformanceCase>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct ConformanceCase {
        name: String,
        spec: ConformanceSpec,
        world: [f64; 3],
        cell: [i64; 3],
        cell_min: [f64; 3],
        cell_center: [f64; 3],
        cell_max: [f64; 3],
        boundary_snap: [f64; 3],
        center_snap: [f64; 3],
    }

    #[derive(Debug, Deserialize)]
    struct ConformanceSpec {
        origin: [f64; 3],
        spacing: [f64; 3],
    }

    fn assert_world_pos_eq(actual: WorldPos, expected: WorldPos) {
        let actual = actual.to_array();
        let expected = expected.to_array();
        for axis in 0..3 {
            assert!(
                (actual[axis] - expected[axis]).abs() < 1e-9,
                "axis {axis}: expected {}, got {}",
                expected[axis],
                actual[axis]
            );
        }
    }

    fn world_pos(values: [f64; 3]) -> WorldPos {
        WorldPos::new(values[0], values[1], values[2])
    }

    fn terrain() -> VoxelGridSpec {
        // 1 world-unit voxels, 16×16×16 chunks.
        VoxelGridSpec::new(GridId::new(0), 1.0, ChunkDims::cubic(16).unwrap()).unwrap()
    }

    #[test]
    fn committed_public_fixture_matches_authority_grid_math() {
        let fixture: ConformanceFixture = serde_json::from_str(include_str!(
            "../../../fixtures/spatial-grid/conformance.json"
        ))
        .unwrap();
        assert_eq!(fixture.coordinate_system, "rightHandedYUp");
        for case in fixture.cases {
            let grid = SpatialGridSpec::new(
                WorldPos::new(
                    case.spec.origin[0],
                    case.spec.origin[1],
                    case.spec.origin[2],
                ),
                case.spec.spacing,
            )
            .unwrap();
            let world = WorldPos::new(case.world[0], case.world[1], case.world[2]);
            let cell = SpatialCellCoord::new(case.cell[0], case.cell[1], case.cell[2]);
            assert_eq!(grid.world_to_cell(world), cell, "{}", case.name);
            assert_world_pos_eq(grid.cell_min_world(cell), world_pos(case.cell_min));
            assert_world_pos_eq(grid.cell_center_world(cell), world_pos(case.cell_center));
            let (min, max) = grid.cell_bounds_world(cell);
            assert_world_pos_eq(min, world_pos(case.cell_min));
            assert_world_pos_eq(max, world_pos(case.cell_max));
            assert_world_pos_eq(grid.snap_to_boundary(world), world_pos(case.boundary_snap));
            assert_world_pos_eq(grid.snap_to_cell_center(world), world_pos(case.center_snap));
        }
    }

    #[test]
    fn rejects_degenerate_specs() {
        assert!(VoxelGridSpec::new(GridId::new(0), 0.0, ChunkDims::cubic(8).unwrap()).is_none());
        assert!(VoxelGridSpec::new(GridId::new(0), -1.0, ChunkDims::cubic(8).unwrap()).is_none());
        assert!(
            VoxelGridSpec::new(GridId::new(0), f64::NAN, ChunkDims::cubic(8).unwrap()).is_none()
        );
        assert!(ChunkDims::new(0, 4, 4).is_none());
        assert!(SpatialGridSpec::new(WorldPos::ORIGIN, [1.0, 0.0, 1.0]).is_none());
        assert!(SpatialGridSpec::new(WorldPos::ORIGIN, [1.0, f64::NAN, 1.0]).is_none());
        assert!(SpatialGridSpec::new(WorldPos::new(f64::INFINITY, 0.0, 0.0), [1.0; 3]).is_none());
    }

    #[test]
    fn generic_grid_uses_min_corner_center_and_bounds_convention() {
        let grid = SpatialGridSpec::new(WorldPos::new(10.0, -2.0, 3.0), [2.0, 0.5, 4.0]).unwrap();
        let cell = SpatialCellCoord::new(-2, 3, 1);
        assert_world_pos_eq(grid.cell_min_world(cell), WorldPos::new(6.0, -0.5, 7.0));
        assert_world_pos_eq(grid.cell_center_world(cell), WorldPos::new(7.0, -0.25, 9.0));
        let (min, max) = grid.cell_bounds_world(cell);
        assert_world_pos_eq(min, WorldPos::new(6.0, -0.5, 7.0));
        assert_world_pos_eq(max, WorldPos::new(8.0, 0.0, 11.0));
        assert_eq!(
            grid.world_to_cell(WorldPos::new(7.999, -0.001, 10.999)),
            cell
        );
    }

    #[test]
    fn generic_grid_floor_and_snaps_are_origin_relative_across_zero() {
        let grid = SpatialGridSpec::new(WorldPos::new(0.25, 1.0, -0.5), [0.5, 2.0, 0.25]).unwrap();
        let input = WorldPos::new(-0.01, -0.01, -0.64);
        assert_eq!(grid.world_to_cell(input), SpatialCellCoord::new(-1, -1, -1));
        assert_world_pos_eq(
            grid.snap_to_boundary(input),
            WorldPos::new(-0.25, -1.0, -0.75),
        );
        assert_world_pos_eq(
            grid.snap_to_cell_center(input),
            WorldPos::new(0.0, 0.0, -0.625),
        );
    }

    #[test]
    fn boundary_snap_half_cell_ties_choose_positive_direction() {
        let grid = SpatialGridSpec::uniform(1.0).unwrap();
        assert_world_pos_eq(
            grid.snap_to_boundary(WorldPos::new(-0.5, 0.5, 1.5)),
            WorldPos::new(0.0, 1.0, 2.0),
        );
    }

    #[test]
    fn world_to_voxel_floors_including_near_boundaries_and_negatives() {
        let g = terrain();
        assert_eq!(
            g.world_to_voxel(WorldPos::new(0.0, 0.0, 0.0)),
            VoxelCoord::new(0, 0, 0)
        );
        assert_eq!(
            g.world_to_voxel(WorldPos::new(0.999, 0.5, 0.0)),
            VoxelCoord::new(0, 0, 0)
        );
        assert_eq!(
            g.world_to_voxel(WorldPos::new(1.0, 2.5, 0.0)),
            VoxelCoord::new(1, 2, 0)
        );
        // Negative: -0.001 is in voxel -1, not 0 (floor, not truncate).
        assert_eq!(
            g.world_to_voxel(WorldPos::new(-0.001, -1.0, -16.0)),
            VoxelCoord::new(-1, -1, -16)
        );
        assert_eq!(
            g.world_to_voxel(WorldPos::new(-0.001, -0.999, -0.5)),
            VoxelCoord::new(-1, -1, -1)
        );
    }

    #[test]
    fn voxel_center_and_bounds_use_the_unit_occupancy_convention() {
        let g = terrain();
        let v = VoxelCoord::new(2, 0, -1);
        assert_eq!(g.voxel_min_world(v), WorldPos::new(2.0, 0.0, -1.0));
        assert_eq!(g.voxel_center_world(v), WorldPos::new(2.5, 0.5, -0.5));
        let (min, max) = g.voxel_bounds_world(v);
        assert_eq!(min, WorldPos::new(2.0, 0.0, -1.0));
        assert_eq!(max, WorldPos::new(3.0, 1.0, 0.0));
        // Center of a cell maps back to that cell.
        assert_eq!(g.world_to_voxel(g.voxel_center_world(v)), v);
    }

    #[test]
    fn voxel_chunk_local_roundtrips_including_negatives() {
        let g = terrain();
        for v in [
            VoxelCoord::new(0, 0, 0),
            VoxelCoord::new(15, 15, 15),
            VoxelCoord::new(16, 0, 0),
            VoxelCoord::new(-1, -1, -1),
            VoxelCoord::new(-16, -17, 33),
        ] {
            let (c, l) = g.voxel_to_chunk_local(v);
            assert!(
                g.local_in_bounds(l),
                "local {l:?} must be within chunk for {v:?}"
            );
            assert_eq!(
                g.chunk_local_to_voxel(c, l),
                v,
                "roundtrip failed for {v:?}"
            );
        }
    }

    #[test]
    fn negative_voxel_maps_to_expected_chunk_and_local() {
        let g = terrain();
        // voxel -1 is in chunk -1, local 15 (floor div / euclid rem).
        let (c, l) = g.voxel_to_chunk_local(VoxelCoord::new(-1, -16, -17));
        assert_eq!(c, ChunkCoord::new(-1, -1, -2));
        assert_eq!(l, LocalVoxelCoord::new(15, 0, 15));
    }

    #[test]
    fn chunk_origin_is_the_minimum_voxel_of_the_chunk() {
        let g = terrain();
        assert_eq!(
            g.chunk_origin_voxel(ChunkCoord::new(-1, 0, 2)),
            VoxelCoord::new(-16, 0, 32)
        );
        // Every voxel in a chunk reports that chunk.
        let c = ChunkCoord::new(-1, 0, 2);
        let origin = g.chunk_origin_voxel(c);
        let region = VoxelRegion::new(
            origin,
            VoxelCoord::new(origin.x + 16, origin.y + 16, origin.z + 16),
        );
        assert!(region.iter().all(|v| g.voxel_to_chunk(v) == c));
    }

    #[test]
    fn same_world_position_resolves_differently_under_two_grid_specs() {
        // No single universal voxel size: a coarse 4-unit grid and a fine
        // 0.25-unit grid disagree about which cell a world point falls in.
        let coarse = VoxelGridSpec::new(GridId::new(1), 4.0, ChunkDims::cubic(8).unwrap()).unwrap();
        let fine = VoxelGridSpec::new(GridId::new(2), 0.25, ChunkDims::cubic(8).unwrap()).unwrap();
        let p = WorldPos::new(10.0, 10.0, 10.0);
        assert_eq!(coarse.world_to_voxel(p), VoxelCoord::new(2, 2, 2));
        assert_eq!(fine.world_to_voxel(p), VoxelCoord::new(40, 40, 40));
        assert_ne!(coarse.world_to_voxel(p), fine.world_to_voxel(p));
    }

    #[test]
    fn non_cubic_chunks_split_per_axis() {
        // Tall terrain columns: 16 × 256 × 16.
        let g =
            VoxelGridSpec::new(GridId::new(3), 1.0, ChunkDims::new(16, 256, 16).unwrap()).unwrap();
        let (c, l) = g.voxel_to_chunk_local(VoxelCoord::new(20, 300, -1));
        assert_eq!(c, ChunkCoord::new(1, 1, -1));
        assert_eq!(l, LocalVoxelCoord::new(4, 44, 15));
        assert_eq!(g.chunk_local_to_voxel(c, l), VoxelCoord::new(20, 300, -1));
    }

    #[test]
    fn origin_rebasing_shifts_world_mapping_without_changing_grid_logic() {
        let base = terrain();
        let shifted = terrain().with_origin(WorldPos::new(100.0, 0.0, 0.0));
        // The same world point is a different voxel under a shifted origin...
        let p = WorldPos::new(100.0, 0.0, 0.0);
        assert_eq!(base.world_to_voxel(p), VoxelCoord::new(100, 0, 0));
        assert_eq!(shifted.world_to_voxel(p), VoxelCoord::new(0, 0, 0));
        // ...but voxel→chunk/local math is origin-independent.
        let v = VoxelCoord::new(-5, 0, 33);
        assert_eq!(
            base.voxel_to_chunk_local(v),
            shifted.voxel_to_chunk_local(v)
        );
    }
}
