//! Chunk-local voxel storage and deterministic volume APIs.
//!
//! # Lane
//!
//! `rust-service` — authoritative dense voxel storage behind a representation-safe
//! boundary (voxel-capability-03). Builds on `core-space` (grid/coords) and
//! `core-voxel` (values). It does **not** know about world partitioning,
//! generation, meshing, or rendering — those are separate lanes/capabilities.
//!
//! # Design soul
//!
//! - **Representation is hidden.** The dense `Vec<VoxelValue>` backing is private;
//!   callers use `get`/`set`/`fill_region`/`iter`. A later palette/sparse/RLE
//!   representation can replace it without changing the API.
//! - **No single global chunk size.** A [`VoxelChunk`] carries its own
//!   [`ChunkDims`] and [`GridId`] (from a [`VoxelGridSpec`]); terrain, object, and
//!   local grids store at different shapes through the same API.
//! - **Deterministic.** Storage index order is **X-fastest, then Y, then Z**,
//!   matching `core-space` region iteration, so iteration, hashing, and fixtures
//!   are reproducible.
//! - Chunk-local storage is distinct from world-partition *missing/unloaded/
//!   generated* states — those live one layer up (voxel-capability-04).

#![forbid(unsafe_code)]

use core_space::{ChunkDims, Direction6, GridId, LocalVoxelCoord, VoxelGridSpec};
use core_voxel::VoxelValue;

/// Monotonic edit counter for a chunk. Bumped only on a *meaningful* change
/// (a `set`/`fill` that actually altered a cell).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct ChunkVersion(pub u64);

/// A deterministic content fingerprint of a chunk (FNV-1a over dims + values).
///
/// Covers chunk *shape and voxel values only* — not version, dirty state, or grid
/// id — so two independently-built chunks with the same shape and contents hash
/// equal (useful for dedup/snapshot), and a no-op edit leaves the hash unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChunkHash(pub u64);

/// A bounded access failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolumeError {
    /// A local coordinate (or region corner) was outside the chunk dimensions.
    OutOfBounds {
        local: LocalVoxelCoord,
        dims: ChunkDims,
    },
}

impl core::fmt::Display for VolumeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            VolumeError::OutOfBounds { local, dims } => write!(
                f,
                "local voxel {:?} out of chunk bounds {:?}",
                local.to_array(),
                dims.to_array()
            ),
        }
    }
}

impl std::error::Error for VolumeError {}

/// Dense, fixed-size, chunk-local voxel storage.
///
/// The backing array is private; its element type and layout are an
/// implementation detail behind the get/set/fill/iter API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoxelChunk {
    grid_id: GridId,
    dims: ChunkDims,
    cells: Vec<VoxelValue>,
    version: ChunkVersion,
    dirty: bool,
}

impl VoxelChunk {
    /// A new chunk of `dims`, all [`VoxelValue::Empty`], at version 0 and clean.
    pub fn new(grid_id: GridId, dims: ChunkDims) -> Self {
        Self::filled(grid_id, dims, VoxelValue::EMPTY)
    }

    /// A new chunk uniformly filled with `value`.
    pub fn filled(grid_id: GridId, dims: ChunkDims, value: VoxelValue) -> Self {
        Self {
            grid_id,
            dims,
            cells: vec![value; dims.volume() as usize],
            version: ChunkVersion(0),
            dirty: false,
        }
    }

    /// A new empty chunk whose dims/grid come from a [`VoxelGridSpec`].
    pub fn from_spec(spec: &VoxelGridSpec) -> Self {
        Self::new(spec.id(), spec.chunk_dims())
    }

    /// Reconstruct a chunk from its cell values in storage order (X-fastest), e.g.
    /// when decoding a snapshot. `values.len()` must equal `dims.volume()`. The
    /// result is version 0 and clean. A chunk rebuilt from another chunk's
    /// [`iter`](Self::iter) values has the same [`content_hash`](Self::content_hash).
    pub fn from_values(
        grid_id: GridId,
        dims: ChunkDims,
        values: &[VoxelValue],
    ) -> Result<Self, VolumeError> {
        if values.len() as u64 != dims.volume() {
            return Err(VolumeError::OutOfBounds {
                local: LocalVoxelCoord::new(values.len() as u32, 0, 0),
                dims,
            });
        }
        Ok(Self {
            grid_id,
            dims,
            cells: values.to_vec(),
            version: ChunkVersion(0),
            dirty: false,
        })
    }

    pub fn grid_id(&self) -> GridId {
        self.grid_id
    }

    pub fn dims(&self) -> ChunkDims {
        self.dims
    }

    pub fn version(&self) -> ChunkVersion {
        self.version
    }

    /// Whether the chunk has unflushed changes since the last [`mark_clean`](Self::mark_clean).
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Clear the dirty flag (e.g. after meshing/persisting). Leaves the version.
    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }

    /// Whether `local` is within the chunk dimensions.
    pub fn in_bounds(&self, local: LocalVoxelCoord) -> bool {
        local.x < self.dims.x() && local.y < self.dims.y() && local.z < self.dims.z()
    }

    /// Flat storage index for a local coordinate (X-fastest), if in bounds.
    fn index(&self, local: LocalVoxelCoord) -> Option<usize> {
        if !self.in_bounds(local) {
            return None;
        }
        let (dx, dy) = (self.dims.x() as usize, self.dims.y() as usize);
        Some(local.x as usize + dx * (local.y as usize + dy * local.z as usize))
    }

    /// Decode a flat index back to a local coordinate (inverse of [`index`]).
    fn delinearize(&self, idx: usize) -> LocalVoxelCoord {
        let dx = self.dims.x() as usize;
        let dy = self.dims.y() as usize;
        let x = idx % dx;
        let rem = idx / dx;
        let y = rem % dy;
        let z = rem / dy;
        LocalVoxelCoord::new(x as u32, y as u32, z as u32)
    }

    /// The voxel at `local`, or `None` if out of bounds.
    pub fn get(&self, local: LocalVoxelCoord) -> Option<VoxelValue> {
        self.index(local).map(|i| self.cells[i])
    }

    /// Set the voxel at `local`. Returns whether the value actually changed;
    /// version/dirty are bumped only on a real change (a same-value set is a no-op).
    pub fn set(&mut self, local: LocalVoxelCoord, value: VoxelValue) -> Result<bool, VolumeError> {
        let idx = self.index(local).ok_or(VolumeError::OutOfBounds {
            local,
            dims: self.dims,
        })?;
        let changed = self.write_cell(idx, value);
        if changed {
            self.bump();
        }
        Ok(changed)
    }

    /// Fill the half-open local region `[min, max)` with `value`. Returns the
    /// number of cells that actually changed; version/dirty bump once if any did.
    /// Both corners must be in bounds (`max` is the exclusive extent, so
    /// `max == dims` is allowed).
    pub fn fill_region(
        &mut self,
        min: LocalVoxelCoord,
        max: LocalVoxelCoord,
        value: VoxelValue,
    ) -> Result<u64, VolumeError> {
        self.check_corner(min)?;
        // `max` is exclusive; it may equal dims but not exceed it.
        if max.x > self.dims.x() || max.y > self.dims.y() || max.z > self.dims.z() {
            return Err(VolumeError::OutOfBounds {
                local: max,
                dims: self.dims,
            });
        }
        let mut changed = 0u64;
        for z in min.z..max.z {
            for y in min.y..max.y {
                for x in min.x..max.x {
                    let idx = self
                        .index(LocalVoxelCoord::new(x, y, z))
                        .expect("region within bounds");
                    if self.write_cell(idx, value) {
                        changed += 1;
                    }
                }
            }
        }
        if changed > 0 {
            self.bump();
        }
        Ok(changed)
    }

    fn check_corner(&self, local: LocalVoxelCoord) -> Result<(), VolumeError> {
        if self.in_bounds(local) {
            Ok(())
        } else {
            Err(VolumeError::OutOfBounds {
                local,
                dims: self.dims,
            })
        }
    }

    fn write_cell(&mut self, idx: usize, value: VoxelValue) -> bool {
        if self.cells[idx] == value {
            false
        } else {
            self.cells[idx] = value;
            true
        }
    }

    fn bump(&mut self) {
        self.version.0 += 1;
        self.dirty = true;
    }

    /// Deterministic iterator over `(local, value)` in storage order (X-fastest).
    pub fn iter(&self) -> impl Iterator<Item = (LocalVoxelCoord, VoxelValue)> + '_ {
        self.cells
            .iter()
            .enumerate()
            .map(|(i, &v)| (self.delinearize(i), v))
    }

    /// `true` if every cell is [`VoxelValue::Empty`].
    pub fn is_empty(&self) -> bool {
        self.cells.iter().all(|v| v.is_empty())
    }

    /// The boundary voxel layer on face `dir` — the cells whose face is exposed at
    /// the chunk edge. Used by meshing to sample across chunk borders. Order is
    /// deterministic (lower in-plane axis fastest).
    pub fn border_layer(&self, dir: Direction6) -> Vec<(LocalVoxelCoord, VoxelValue)> {
        let [dx, dy, dz] = self.dims.to_array();
        let mut out = Vec::new();
        let mut push = |this: &Self, x: u32, y: u32, z: u32| {
            let l = LocalVoxelCoord::new(x, y, z);
            out.push((l, this.get(l).expect("border in bounds")));
        };
        match dir {
            Direction6::NegX | Direction6::PosX => {
                let x = if dir == Direction6::PosX { dx - 1 } else { 0 };
                for z in 0..dz {
                    for y in 0..dy {
                        push(self, x, y, z);
                    }
                }
            }
            Direction6::NegY | Direction6::PosY => {
                let y = if dir == Direction6::PosY { dy - 1 } else { 0 };
                for z in 0..dz {
                    for x in 0..dx {
                        push(self, x, y, z);
                    }
                }
            }
            Direction6::NegZ | Direction6::PosZ => {
                let z = if dir == Direction6::PosZ { dz - 1 } else { 0 };
                for y in 0..dy {
                    for x in 0..dx {
                        push(self, x, y, z);
                    }
                }
            }
        }
        out
    }

    /// Deterministic FNV-1a content hash over chunk dims + voxel values.
    pub fn content_hash(&self) -> ChunkHash {
        const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
        const PRIME: u64 = 0x0000_0100_0000_01b3;
        let mut h = OFFSET;
        let mut feed = |word: u32| {
            for b in word.to_le_bytes() {
                h ^= b as u64;
                h = h.wrapping_mul(PRIME);
            }
        };
        // Shape first, so different-shaped chunks with the same prefix differ.
        for d in self.dims.to_array() {
            feed(d);
        }
        for cell in &self.cells {
            feed(cell.to_encoded());
        }
        ChunkHash(h)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_space::ChunkDims;

    fn dims(n: u32) -> ChunkDims {
        ChunkDims::cubic(n).unwrap()
    }

    fn chunk(n: u32) -> VoxelChunk {
        VoxelChunk::new(GridId::new(0), dims(n))
    }

    fn l(x: u32, y: u32, z: u32) -> LocalVoxelCoord {
        LocalVoxelCoord::new(x, y, z)
    }

    #[test]
    fn new_chunk_is_empty_clean_v0() {
        let c = chunk(8);
        assert!(c.is_empty());
        assert_eq!(c.version(), ChunkVersion(0));
        assert!(!c.is_dirty());
        assert_eq!(c.iter().count(), 8 * 8 * 8);
    }

    #[test]
    fn get_set_roundtrip_and_bounds() {
        let mut c = chunk(4);
        assert_eq!(c.get(l(1, 2, 3)), Some(VoxelValue::EMPTY));
        assert_eq!(c.set(l(1, 2, 3), VoxelValue::solid_raw(5)), Ok(true));
        assert_eq!(c.get(l(1, 2, 3)), Some(VoxelValue::solid_raw(5)));
        // Out of bounds:
        assert_eq!(c.get(l(4, 0, 0)), None);
        assert_eq!(
            c.set(l(0, 9, 0), VoxelValue::solid_raw(1)),
            Err(VolumeError::OutOfBounds {
                local: l(0, 9, 0),
                dims: dims(4)
            }),
        );
    }

    #[test]
    fn setting_same_value_does_not_bump_version_or_dirty() {
        let mut c = chunk(4);
        assert_eq!(c.set(l(0, 0, 0), VoxelValue::solid_raw(2)), Ok(true));
        assert_eq!(c.version(), ChunkVersion(1));
        c.mark_clean();
        // Same value again → no-op.
        assert_eq!(c.set(l(0, 0, 0), VoxelValue::solid_raw(2)), Ok(false));
        assert_eq!(c.version(), ChunkVersion(1));
        assert!(!c.is_dirty());
        // A different value bumps.
        assert_eq!(c.set(l(0, 0, 0), VoxelValue::EMPTY), Ok(true));
        assert_eq!(c.version(), ChunkVersion(2));
        assert!(c.is_dirty());
    }

    #[test]
    fn fill_region_counts_changes_and_respects_exclusive_max() {
        let mut c = chunk(4);
        // Fill the whole 4³ chunk.
        assert_eq!(
            c.fill_region(l(0, 0, 0), l(4, 4, 4), VoxelValue::solid_raw(1)),
            Ok(64)
        );
        assert_eq!(c.version(), ChunkVersion(1)); // one bump for the whole fill
                                                  // Re-fill a sub-box with the same value → 0 changes, no bump.
        c.mark_clean();
        assert_eq!(
            c.fill_region(l(1, 1, 1), l(3, 3, 3), VoxelValue::solid_raw(1)),
            Ok(0)
        );
        assert!(!c.is_dirty());
        // max beyond dims is rejected.
        assert!(c
            .fill_region(l(0, 0, 0), l(5, 4, 4), VoxelValue::EMPTY)
            .is_err());
    }

    #[test]
    fn iteration_is_x_fastest_and_matches_get() {
        let mut c = VoxelChunk::new(GridId::new(0), ChunkDims::new(2, 2, 1).unwrap());
        c.set(l(1, 0, 0), VoxelValue::solid_raw(9)).unwrap();
        let coords: Vec<_> = c.iter().map(|(loc, _)| loc.to_array()).collect();
        assert_eq!(coords, vec![[0, 0, 0], [1, 0, 0], [0, 1, 0], [1, 1, 0]]);
        for (loc, v) in c.iter() {
            assert_eq!(c.get(loc), Some(v));
        }
    }

    #[test]
    fn hash_is_stable_across_construction_and_changes_on_meaningful_edit() {
        let a = {
            let mut c = chunk(4);
            c.set(l(2, 2, 2), VoxelValue::solid_raw(3)).unwrap();
            c
        };
        let b = {
            let mut c = chunk(4);
            c.set(l(2, 2, 2), VoxelValue::solid_raw(3)).unwrap();
            c
        };
        // Same shape + contents → same hash, regardless of version/dirty.
        assert_eq!(a.content_hash(), b.content_hash());
        // A meaningful edit changes the hash.
        let mut c = a.clone();
        c.set(l(0, 0, 0), VoxelValue::solid_raw(1)).unwrap();
        assert_ne!(a.content_hash(), c.content_hash());
        // A no-op edit does not.
        let mut d = a.clone();
        d.set(l(2, 2, 2), VoxelValue::solid_raw(3)).unwrap();
        assert_eq!(a.content_hash(), d.content_hash());
    }

    #[test]
    fn different_shapes_with_same_values_hash_differently() {
        let flat = VoxelChunk::new(GridId::new(0), ChunkDims::new(4, 1, 1).unwrap());
        let tall = VoxelChunk::new(GridId::new(0), ChunkDims::new(1, 4, 1).unwrap());
        // Both all-empty, same cell count, but different shape.
        assert_ne!(flat.content_hash(), tall.content_hash());
    }

    #[test]
    fn border_layer_samples_the_expected_face() {
        let mut c = chunk(3);
        // Mark the +X face.
        for z in 0..3 {
            for y in 0..3 {
                c.set(l(2, y, z), VoxelValue::solid_raw(7)).unwrap();
            }
        }
        let layer = c.border_layer(Direction6::PosX);
        assert_eq!(layer.len(), 9);
        assert!(layer
            .iter()
            .all(|(loc, v)| loc.x == 2 && *v == VoxelValue::solid_raw(7)));
        // The -X face is still empty.
        assert!(c
            .border_layer(Direction6::NegX)
            .iter()
            .all(|(_, v)| v.is_empty()));
    }

    #[test]
    fn the_same_api_serves_two_different_grid_specs() {
        // A coarse terrain grid and a fine object grid use the same storage API at
        // different shapes — no terrain-only assumption is baked in.
        let terrain =
            VoxelGridSpec::new(GridId::new(1), 2.0, ChunkDims::cubic(32).unwrap()).unwrap();
        let object =
            VoxelGridSpec::new(GridId::new(2), 0.1, ChunkDims::new(8, 8, 16).unwrap()).unwrap();
        let mut tc = VoxelChunk::from_spec(&terrain);
        let mut oc = VoxelChunk::from_spec(&object);
        assert_eq!(tc.grid_id(), GridId::new(1));
        assert_eq!(oc.grid_id(), GridId::new(2));
        assert_eq!(tc.set(l(31, 31, 31), VoxelValue::solid_raw(1)), Ok(true));
        assert_eq!(oc.set(l(7, 7, 15), VoxelValue::solid_raw(1)), Ok(true));
        // The object grid's tall axis is addressable where a cube of 8 would not be.
        assert_eq!(oc.get(l(0, 0, 15)), Some(VoxelValue::EMPTY));
        assert_eq!(tc.get(l(0, 0, 15)), Some(VoxelValue::EMPTY));
    }
}
