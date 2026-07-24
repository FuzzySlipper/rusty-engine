//! Half-open axis-aligned regions with deterministic iteration order.
//!
//! Iteration is **X-fastest, then Y, then Z** (`z` outermost). The order is fixed
//! so meshing, generation, hashing, and golden fixtures are reproducible.

use crate::voxel::{ChunkCoord, VoxelCoord};

/// A half-open box of voxel cells: `[min, max)` on each axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoxelRegion {
    pub min: VoxelCoord,
    pub max: VoxelCoord,
}

/// A half-open box of chunk coordinates: `[min, max)` on each axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkRegion {
    pub min: ChunkCoord,
    pub max: ChunkCoord,
}

/// Deterministic [min, max) iterator over integer triples (X-fastest).
#[derive(Debug, Clone)]
struct TripleIter {
    min: [i64; 3],
    max: [i64; 3],
    cur: [i64; 3],
    done: bool,
}

impl TripleIter {
    fn new(min: [i64; 3], max: [i64; 3]) -> Self {
        let empty = (0..3).any(|i| max[i] <= min[i]);
        TripleIter {
            min,
            max,
            cur: min,
            done: empty,
        }
    }

    fn count_remaining(&self) -> usize {
        if self.done {
            return 0;
        }
        // Total cells minus how far we have advanced. Computed directly from the
        // box volume for the size hint; exact because of the half-open invariant.
        let dims: [i64; 3] = [
            self.max[0] - self.min[0],
            self.max[1] - self.min[1],
            self.max[2] - self.min[2],
        ];
        let total = dims[0] as u128 * dims[1] as u128 * dims[2] as u128;
        let done = (self.cur[2] - self.min[2]) as u128 * dims[0] as u128 * dims[1] as u128
            + (self.cur[1] - self.min[1]) as u128 * dims[0] as u128
            + (self.cur[0] - self.min[0]) as u128;
        (total - done) as usize
    }
}

impl Iterator for TripleIter {
    type Item = [i64; 3];

    fn next(&mut self) -> Option<[i64; 3]> {
        if self.done {
            return None;
        }
        let out = self.cur;
        // Advance X, carry into Y, carry into Z.
        self.cur[0] += 1;
        if self.cur[0] >= self.max[0] {
            self.cur[0] = self.min[0];
            self.cur[1] += 1;
            if self.cur[1] >= self.max[1] {
                self.cur[1] = self.min[1];
                self.cur[2] += 1;
                if self.cur[2] >= self.max[2] {
                    self.done = true;
                }
            }
        }
        Some(out)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.count_remaining();
        (n, Some(n))
    }
}

impl VoxelRegion {
    /// A half-open region from `min` (inclusive) to `max` (exclusive). Empty if
    /// `max <= min` on any axis.
    pub const fn new(min: VoxelCoord, max: VoxelCoord) -> Self {
        Self { min, max }
    }

    /// A region spanning two arbitrary corners (sorted per axis), `max` exclusive
    /// of the larger corner + 1 so both corners are included.
    pub fn from_inclusive_corners(a: VoxelCoord, b: VoxelCoord) -> Self {
        let min = VoxelCoord::new(a.x.min(b.x), a.y.min(b.y), a.z.min(b.z));
        let max = VoxelCoord::new(a.x.max(b.x) + 1, a.y.max(b.y) + 1, a.z.max(b.z) + 1);
        Self { min, max }
    }

    /// Number of voxels in the region (0 if empty).
    pub fn len(&self) -> u64 {
        if self.is_empty() {
            return 0;
        }
        (self.max.x - self.min.x) as u64
            * (self.max.y - self.min.y) as u64
            * (self.max.z - self.min.z) as u64
    }

    pub fn is_empty(&self) -> bool {
        self.max.x <= self.min.x || self.max.y <= self.min.y || self.max.z <= self.min.z
    }

    pub fn contains(&self, v: VoxelCoord) -> bool {
        (self.min.x..self.max.x).contains(&v.x)
            && (self.min.y..self.max.y).contains(&v.y)
            && (self.min.z..self.max.z).contains(&v.z)
    }

    /// Deterministic iterator over the contained voxels (X-fastest, Z-outermost).
    pub fn iter(&self) -> impl Iterator<Item = VoxelCoord> {
        TripleIter::new(
            [self.min.x, self.min.y, self.min.z],
            [self.max.x, self.max.y, self.max.z],
        )
        .map(|[x, y, z]| VoxelCoord::new(x, y, z))
    }
}

impl ChunkRegion {
    pub const fn new(min: ChunkCoord, max: ChunkCoord) -> Self {
        Self { min, max }
    }

    pub fn len(&self) -> u64 {
        if self.is_empty() {
            return 0;
        }
        (self.max.x - self.min.x) as u64
            * (self.max.y - self.min.y) as u64
            * (self.max.z - self.min.z) as u64
    }

    pub fn is_empty(&self) -> bool {
        self.max.x <= self.min.x || self.max.y <= self.min.y || self.max.z <= self.min.z
    }

    pub fn contains(&self, c: ChunkCoord) -> bool {
        (self.min.x..self.max.x).contains(&c.x)
            && (self.min.y..self.max.y).contains(&c.y)
            && (self.min.z..self.max.z).contains(&c.z)
    }

    /// Deterministic iterator over the contained chunks (X-fastest, Z-outermost).
    pub fn iter(&self) -> impl Iterator<Item = ChunkCoord> {
        TripleIter::new(
            [self.min.x, self.min.y, self.min.z],
            [self.max.x, self.max.y, self.max.z],
        )
        .map(|[x, y, z]| ChunkCoord::new(x, y, z))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_region_yields_nothing() {
        let r = VoxelRegion::new(VoxelCoord::new(2, 2, 2), VoxelCoord::new(2, 5, 5));
        assert!(r.is_empty());
        assert_eq!(r.len(), 0);
        assert_eq!(r.iter().count(), 0);
    }

    #[test]
    fn iteration_is_x_fastest_then_y_then_z() {
        let r = VoxelRegion::new(VoxelCoord::ORIGIN, VoxelCoord::new(2, 2, 2));
        let got: Vec<_> = r.iter().map(|v| v.to_array()).collect();
        assert_eq!(
            got,
            vec![
                [0, 0, 0],
                [1, 0, 0],
                [0, 1, 0],
                [1, 1, 0],
                [0, 0, 1],
                [1, 0, 1],
                [0, 1, 1],
                [1, 1, 1],
            ]
        );
    }

    #[test]
    fn len_and_iter_count_agree_including_negatives() {
        let r = VoxelRegion::new(VoxelCoord::new(-3, -1, -2), VoxelCoord::new(1, 2, 0));
        assert_eq!(r.len() as usize, r.iter().count());
        assert_eq!(r.len(), 4 * 3 * 2);
        // size_hint is exact.
        assert_eq!(r.iter().size_hint(), (24, Some(24)));
    }

    #[test]
    fn from_inclusive_corners_includes_both() {
        let r =
            VoxelRegion::from_inclusive_corners(VoxelCoord::new(5, 0, 0), VoxelCoord::new(1, 0, 0));
        assert!(r.contains(VoxelCoord::new(1, 0, 0)));
        assert!(r.contains(VoxelCoord::new(5, 0, 0)));
        assert_eq!(r.len(), 5);
    }
}
