//! World/chunk partitioning: the chunk map, lifecycle states, active regions,
//! and the dirty set.
//!
//! # Lane
//!
//! `rust-service` — the authoritative world-level partition over chunk-local
//! storage (`svc-volume`). It tracks *which chunks exist, which are resident, and
//! which need rework* (voxel-capability-04). It does **not** own storage layout,
//! meshing, collision, generation, or render state — it exposes deterministic
//! hooks those lanes drive.
//!
//! # Design soul
//!
//! - **Distinct presence states** (decision 1): `Absent` (never tracked),
//!   `Pending` (requested, not generated), `Resident` (data loaded), `Unloaded`
//!   (was resident, data evicted, slot remembered). These are *authoritative*;
//!   render-only states (meshed/rendered) live in the render lane (decision 2).
//! - **Deterministic everywhere** (replay-safe): the chunk map is a `BTreeMap`
//!   and the dirty set a `BTreeSet`, so iteration/drain order is coordinate-
//!   ascending regardless of insertion order — no hash-iteration nondeterminism.
//! - **Scheduling is not here** (cap 13): the partition exposes a dirty *set*;
//!   prioritisation/budgets belong to a scheduler.

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};

use core_space::{ChunkCoord, ChunkRegion, Direction6, VoxelGridSpec};
use svc_volume::VoxelChunk;

/// The authoritative lifecycle state of a chunk coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkState {
    /// Not tracked at all — never requested, generated, or loaded.
    Absent,
    /// Requested/queued for generation; no voxel data yet.
    Pending,
    /// Voxel data is loaded and authoritative.
    Resident,
    /// Was resident; data evicted but the slot is remembered (distinct from
    /// `Absent`, which was never tracked).
    Unloaded,
}

/// An invalid lifecycle transition was requested.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartitionError {
    /// `request` was called on a coordinate that already has a slot.
    AlreadyTracked {
        coord: ChunkCoord,
        state: ChunkState,
    },
    /// An operation needed a resident chunk but the coordinate was not resident.
    NotResident {
        coord: ChunkCoord,
        state: ChunkState,
    },
}

impl core::fmt::Display for PartitionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            PartitionError::AlreadyTracked { coord, state } => {
                write!(
                    f,
                    "chunk {:?} already tracked ({state:?})",
                    coord.to_array()
                )
            }
            PartitionError::NotResident { coord, state } => {
                write!(
                    f,
                    "chunk {:?} is not resident ({state:?})",
                    coord.to_array()
                )
            }
        }
    }
}

impl std::error::Error for PartitionError {}

#[derive(Debug, Clone)]
struct Slot {
    state: ChunkState,
    data: Option<VoxelChunk>,
}

/// The world-level voxel partition for a single grid.
#[derive(Debug, Clone)]
pub struct VoxelWorld {
    grid: VoxelGridSpec,
    chunks: BTreeMap<ChunkCoord, Slot>,
    dirty: BTreeSet<ChunkCoord>,
}

impl VoxelWorld {
    /// An empty partition for `grid`.
    pub fn new(grid: VoxelGridSpec) -> Self {
        Self {
            grid,
            chunks: BTreeMap::new(),
            dirty: BTreeSet::new(),
        }
    }

    /// The grid spec this partition is for.
    pub fn grid(&self) -> VoxelGridSpec {
        self.grid
    }

    /// Number of tracked chunks (any non-`Absent` state).
    pub fn tracked_len(&self) -> usize {
        self.chunks.len()
    }

    // ── lifecycle ───────────────────────────────────────────────────────────────

    /// The lifecycle state of a coordinate.
    pub fn state(&self, coord: ChunkCoord) -> ChunkState {
        self.chunks
            .get(&coord)
            .map_or(ChunkState::Absent, |s| s.state)
    }

    /// Reserve a slot for generation. Valid only from `Absent`.
    pub fn request(&mut self, coord: ChunkCoord) -> Result<(), PartitionError> {
        match self.state(coord) {
            ChunkState::Absent => {
                self.chunks.insert(
                    coord,
                    Slot {
                        state: ChunkState::Pending,
                        data: None,
                    },
                );
                Ok(())
            }
            state => Err(PartitionError::AlreadyTracked { coord, state }),
        }
    }

    /// Insert/replace resident voxel data (generation result or reload). Valid
    /// from any state; marks the chunk dirty (new data needs meshing/collision).
    /// Returns the previous data if it was resident.
    pub fn insert(&mut self, coord: ChunkCoord, chunk: VoxelChunk) -> Option<VoxelChunk> {
        let prev = self
            .chunks
            .insert(
                coord,
                Slot {
                    state: ChunkState::Resident,
                    data: Some(chunk),
                },
            )
            .and_then(|s| s.data);
        self.dirty.insert(coord);
        prev
    }

    /// Evict a resident chunk's data, keeping the slot as `Unloaded`. The
    /// coordinate also leaves the dirty set. Returns the evicted data.
    pub fn unload(&mut self, coord: ChunkCoord) -> Result<VoxelChunk, PartitionError> {
        let state = self.state(coord);
        let slot = self
            .chunks
            .get_mut(&coord)
            .filter(|s| s.state == ChunkState::Resident);
        match slot {
            Some(slot) => {
                slot.state = ChunkState::Unloaded;
                self.dirty.remove(&coord);
                Ok(slot.data.take().expect("resident slot has data"))
            }
            None => Err(PartitionError::NotResident { coord, state }),
        }
    }

    /// Fully forget a coordinate (→ `Absent`). Returns its data if it was resident.
    pub fn remove(&mut self, coord: ChunkCoord) -> Option<VoxelChunk> {
        self.dirty.remove(&coord);
        self.chunks.remove(&coord).and_then(|s| s.data)
    }

    /// Borrow a resident chunk's data.
    pub fn get(&self, coord: ChunkCoord) -> Option<&VoxelChunk> {
        self.chunks.get(&coord).and_then(|s| s.data.as_ref())
    }

    /// Mutably borrow a resident chunk's data, marking it dirty (the caller is
    /// about to change it). Use [`get`](Self::get) for read-only access.
    pub fn get_mut(&mut self, coord: ChunkCoord) -> Option<&mut VoxelChunk> {
        let slot = self.chunks.get_mut(&coord)?;
        let data = slot.data.as_mut()?;
        self.dirty.insert(coord);
        Some(data)
    }

    /// Deterministic iterator over resident `(coord, &chunk)`, coordinate-ascending.
    pub fn resident_chunks(&self) -> impl Iterator<Item = (ChunkCoord, &VoxelChunk)> {
        self.chunks
            .iter()
            .filter_map(|(c, s)| s.data.as_ref().map(|d| (*c, d)))
    }

    /// Deterministic iterator over every tracked coordinate + state.
    pub fn tracked(&self) -> impl Iterator<Item = (ChunkCoord, ChunkState)> + '_ {
        self.chunks.iter().map(|(c, s)| (*c, s.state))
    }

    // ── active region ────────────────────────────────────────────────────────────

    /// The cube of chunk coordinates within Chebyshev `radius` of `focus`
    /// (a `(2r+1)³` [`ChunkRegion`]). The activation source (camera/focus/editor)
    /// is the caller's concern — this is pure geometry.
    pub fn active_region(focus: ChunkCoord, radius: u32) -> ChunkRegion {
        let r = radius as i64;
        ChunkRegion::new(
            ChunkCoord::new(focus.x - r, focus.y - r, focus.z - r),
            ChunkCoord::new(focus.x + r + 1, focus.y + r + 1, focus.z + r + 1),
        )
    }

    // ── dirty set ────────────────────────────────────────────────────────────────

    /// Mark a coordinate dirty (needs remesh/collision rebuild/persist).
    pub fn mark_dirty(&mut self, coord: ChunkCoord) {
        self.dirty.insert(coord);
    }

    /// Mark `coord` dirty plus every **resident** face neighbour — a border edit in
    /// `coord` changes the visible faces of its neighbours too (decision 6).
    pub fn mark_dirty_with_neighbors(&mut self, coord: ChunkCoord) {
        self.dirty.insert(coord);
        for dir in Direction6::ALL {
            let n = coord.neighbor(dir);
            if self.state(n) == ChunkState::Resident {
                self.dirty.insert(n);
            }
        }
    }

    pub fn is_dirty(&self, coord: ChunkCoord) -> bool {
        self.dirty.contains(&coord)
    }

    pub fn dirty_count(&self) -> usize {
        self.dirty.len()
    }

    /// The dirty coordinates in deterministic (coordinate-ascending) order.
    pub fn dirty_chunks(&self) -> impl Iterator<Item = ChunkCoord> + '_ {
        self.dirty.iter().copied()
    }

    /// Take and clear the dirty set, returned in deterministic order. The
    /// scheduler decides what to actually do with them.
    pub fn drain_dirty(&mut self) -> Vec<ChunkCoord> {
        let out: Vec<_> = self.dirty.iter().copied().collect();
        self.dirty.clear();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_space::{ChunkDims, GridId};
    use core_voxel::VoxelValue;

    fn grid() -> VoxelGridSpec {
        VoxelGridSpec::new(GridId::new(0), 1.0, ChunkDims::cubic(16).unwrap()).unwrap()
    }

    fn cc(x: i64, y: i64, z: i64) -> ChunkCoord {
        ChunkCoord::new(x, y, z)
    }

    fn chunk(world: &VoxelWorld) -> VoxelChunk {
        VoxelChunk::from_spec(&world.grid())
    }

    #[test]
    fn unknown_coordinate_is_absent_and_get_is_none() {
        let w = VoxelWorld::new(grid());
        assert_eq!(w.state(cc(0, 0, 0)), ChunkState::Absent);
        assert!(w.get(cc(0, 0, 0)).is_none());
        assert_eq!(w.tracked_len(), 0);
    }

    #[test]
    fn lifecycle_absent_pending_resident_unloaded_reload() {
        let mut w = VoxelWorld::new(grid());
        let c = cc(1, 0, -2);
        assert_eq!(w.request(c), Ok(()));
        assert_eq!(w.state(c), ChunkState::Pending);
        // Requesting again is an invalid transition.
        assert_eq!(
            w.request(c),
            Err(PartitionError::AlreadyTracked {
                coord: c,
                state: ChunkState::Pending
            }),
        );
        // Generation result.
        assert!(w.insert(c, chunk(&w)).is_none());
        assert_eq!(w.state(c), ChunkState::Resident);
        assert!(w.get(c).is_some());
        // Unload keeps the slot, distinct from Absent.
        let evicted = w.unload(c).unwrap();
        assert_eq!(evicted.grid_id(), GridId::new(0));
        assert_eq!(w.state(c), ChunkState::Unloaded);
        assert!(w.get(c).is_none());
        assert_ne!(w.state(c), ChunkState::Absent);
        // Reload.
        w.insert(c, chunk(&w));
        assert_eq!(w.state(c), ChunkState::Resident);
        // Remove → Absent.
        assert!(w.remove(c).is_some());
        assert_eq!(w.state(c), ChunkState::Absent);
    }

    #[test]
    fn unload_requires_resident() {
        let mut w = VoxelWorld::new(grid());
        let c = cc(0, 0, 0);
        assert_eq!(
            w.unload(c),
            Err(PartitionError::NotResident {
                coord: c,
                state: ChunkState::Absent
            }),
        );
        w.request(c).unwrap();
        assert!(matches!(
            w.unload(c),
            Err(PartitionError::NotResident { .. })
        ));
    }

    #[test]
    fn active_region_is_a_cube_around_focus_including_negatives() {
        let region = VoxelWorld::active_region(cc(0, 0, 0), 1);
        assert_eq!(region.len(), 27);
        assert!(region.contains(cc(-1, -1, -1)));
        assert!(region.contains(cc(1, 1, 1)));
        assert!(!region.contains(cc(2, 0, 0)));
        // radius 0 is just the focus.
        assert_eq!(VoxelWorld::active_region(cc(5, -3, 2), 0).len(), 1);
    }

    #[test]
    fn resident_iteration_is_coordinate_ascending_regardless_of_insert_order() {
        let mut w = VoxelWorld::new(grid());
        for c in [cc(2, 0, 0), cc(-1, 0, 0), cc(0, 0, 0), cc(-1, 0, -1)] {
            w.insert(c, chunk(&w));
        }
        let order: Vec<_> = w.resident_chunks().map(|(c, _)| c.to_array()).collect();
        assert_eq!(order, vec![[-1, 0, -1], [-1, 0, 0], [0, 0, 0], [2, 0, 0]]);
    }

    #[test]
    fn dirty_set_dedups_and_drains_in_deterministic_order() {
        let mut w = VoxelWorld::new(grid());
        // insert marks dirty; mark again is idempotent.
        w.insert(cc(3, 0, 0), chunk(&w));
        w.insert(cc(-2, 0, 0), chunk(&w));
        w.mark_dirty(cc(3, 0, 0));
        assert_eq!(w.dirty_count(), 2);
        let drained = w.drain_dirty();
        assert_eq!(drained, vec![cc(-2, 0, 0), cc(3, 0, 0)]);
        assert_eq!(w.dirty_count(), 0);
    }

    #[test]
    fn border_edit_invalidates_all_six_resident_neighbors() {
        let mut w = VoxelWorld::new(grid());
        let center = cc(0, 0, 0);
        w.insert(center, chunk(&w));
        // Five resident neighbours; leave +X absent to prove only resident ones mark.
        for dir in Direction6::ALL {
            if dir != Direction6::PosX {
                w.insert(center.neighbor(dir), chunk(&w));
            }
        }
        w.drain_dirty(); // clear the insert-dirties
        w.mark_dirty_with_neighbors(center);
        let dirty: Vec<_> = w.dirty_chunks().collect();
        assert!(dirty.contains(&center));
        assert!(dirty.contains(&center.neighbor(Direction6::NegX)));
        // +X is absent, so it is not marked.
        assert!(!dirty.contains(&center.neighbor(Direction6::PosX)));
        assert_eq!(dirty.len(), 6); // center + 5 resident neighbours
    }

    #[test]
    fn get_mut_marks_dirty_and_supports_editing() {
        let mut w = VoxelWorld::new(grid());
        let c = cc(0, 0, 0);
        w.insert(c, chunk(&w));
        w.drain_dirty();
        assert!(!w.is_dirty(c));
        let chunk = w.get_mut(c).unwrap();
        chunk
            .set(
                core_space::LocalVoxelCoord::new(0, 0, 0),
                VoxelValue::solid_raw(1),
            )
            .unwrap();
        assert!(w.is_dirty(c));
    }
}
