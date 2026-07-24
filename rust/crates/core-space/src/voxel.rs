//! Integer grid spaces: voxel cells, chunk coordinates, and chunk-local addresses.
//!
//! These are deliberately separate newtypes so a [`ChunkCoord`] can never be used
//! where a [`VoxelCoord`] is expected. They carry no grid scale themselves — all
//! world↔grid and voxel↔chunk conversion goes through [`crate::VoxelGridSpec`].

use crate::direction::Direction6;

/// An integer voxel cell coordinate within *some* voxel grid.
///
/// Which grid it belongs to is contextual (held by the caller / the owning
/// [`crate::VoxelGridSpec`]); this type does not bake in a voxel size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VoxelCoord {
    pub x: i64,
    pub y: i64,
    pub z: i64,
}

/// An integer chunk coordinate (which chunk, not which voxel).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChunkCoord {
    pub x: i64,
    pub y: i64,
    pub z: i64,
}

/// A voxel address *inside* a chunk, bounded by the grid's chunk dimensions.
///
/// Always non-negative and `< chunk_dims` on each axis (enforced by construction
/// through [`crate::VoxelGridSpec`]). `u32` is plenty for any practical chunk size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LocalVoxelCoord {
    pub x: u32,
    pub y: u32,
    pub z: u32,
}

impl VoxelCoord {
    pub const ORIGIN: VoxelCoord = VoxelCoord { x: 0, y: 0, z: 0 };

    pub const fn new(x: i64, y: i64, z: i64) -> Self {
        Self { x, y, z }
    }

    pub const fn to_array(self) -> [i64; 3] {
        [self.x, self.y, self.z]
    }

    /// The adjacent voxel one step along `dir` (no grid context needed: voxel
    /// neighbours are always ±1 on a single axis). Crossing a chunk boundary is
    /// resolved by re-deriving chunk/local through the grid spec.
    pub fn neighbor(self, dir: Direction6) -> VoxelCoord {
        let [dx, dy, dz] = dir.offset();
        VoxelCoord::new(self.x + dx as i64, self.y + dy as i64, self.z + dz as i64)
    }
}

impl ChunkCoord {
    pub const ORIGIN: ChunkCoord = ChunkCoord { x: 0, y: 0, z: 0 };

    pub const fn new(x: i64, y: i64, z: i64) -> Self {
        Self { x, y, z }
    }

    pub const fn to_array(self) -> [i64; 3] {
        [self.x, self.y, self.z]
    }

    /// The adjacent chunk one step along `dir`.
    pub fn neighbor(self, dir: Direction6) -> ChunkCoord {
        let [dx, dy, dz] = dir.offset();
        ChunkCoord::new(self.x + dx as i64, self.y + dy as i64, self.z + dz as i64)
    }
}

impl LocalVoxelCoord {
    pub const ORIGIN: LocalVoxelCoord = LocalVoxelCoord { x: 0, y: 0, z: 0 };

    pub const fn new(x: u32, y: u32, z: u32) -> Self {
        Self { x, y, z }
    }

    pub const fn to_array(self) -> [u32; 3] {
        [self.x, self.y, self.z]
    }
}
