//! `parry3d`-backed collision projection derived from voxel authority.
//!
//! # Lane
//!
//! `rust-service` — the **only** crate permitted the `parry3d-f64` dependency
//! (voxel-capability-11). It builds a collision world as a *derived projection*
//! from canonical voxel/chunk state (`svc-volume`/`svc-spatial`); it does **not**
//! own truth. It owns fast queries over projected truth and rebuilds when chunks
//! change.
//!
//! # Design soul
//!
//! - **Derived, not authoritative.** Each chunk collider records the
//!   `content_hash` of the chunk it was built from; [`CollisionProjection::is_chunk_stale`]
//!   detects drift so rebuilds stay coordinated with the chunk dirty queue.
//! - **Typed boundary.** ASHA coordinate types (`WorldPos`, `VoxelGridSpec`) cross
//!   the public API; `parry3d` `Pose`/`Vector`/`Compound` (glam-backed) stay
//!   internal so coordinate-space distinctions are not erased.
//! - **f64 throughout** (`parry3d-f64`) to match `core-space`'s `WorldScalar`.
//! - **No raw parry-world mutation is exposed.** Callers build/reconcile and query;
//!   they never poke the parry compound directly.
//!
//! Initial projection: each solid voxel becomes a world-positioned cuboid in a
//! per-chunk `Compound`. Greedy/heightfield/trimesh optimisation and per-material
//! collision classes are deferred (decisions 1/5).
//!
//! Queries are the **one shared vocabulary** for picking, camera, and placement:
//! [`CollisionProjection::contains_point`] (occupancy), [`CollisionProjection::raycast`]
//! (nearest authoritative [`VoxelHit`] with face/distance), and
//! [`CollisionProjection::aabb_overlaps_solid`] (placement/camera shape test), and
//! [`CollisionProjection::axis_swept_aabb_overlaps_solid`] (continuous axis-aligned
//! camera movement). There is no separate renderer-owned authoritative raycast;
//! renderer picks are hints revalidated here (#2259).

#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use core_space::{ChunkCoord, ChunkRegion, Face, VoxelCoord, VoxelGridSpec, WorldPos, WorldVec};
use core_voxel::VoxelValue;
use svc_spatial::VoxelWorld;
use svc_volume::VoxelChunk;

use parry3d_f64::math::{Pose, Real, Vector};
use parry3d_f64::query::{intersection_test, Ray as ParryRay, RayCast};
use parry3d_f64::shape::{Compound, Cuboid, SharedShape};

/// How a voxel value participates in collision. Derived from the value/material;
/// per-material collision kinds (decision 1) are deferred behind this enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollisionClass {
    /// Does not collide (empty space, and — once modelled — non-solid materials).
    None,
    /// A solid obstacle.
    Solid,
}

/// Map a voxel value to its collision class. Today: solids collide, empty does not
/// (mirrors `core_voxel::VoxelValue::is_collidable`); transparency/per-material
/// behaviour is deferred.
pub fn collision_class(value: VoxelValue) -> CollisionClass {
    if value.is_collidable() {
        CollisionClass::Solid
    } else {
        CollisionClass::None
    }
}

// ── Typed boundary (ASHA ↔ parry) ──────────────────────────────────────────────

#[inline]
fn world_to_point(p: WorldPos) -> Vector {
    Vector::new(p.x, p.y, p.z)
}

#[inline]
fn identity() -> Pose {
    Pose::from_translation(Vector::ZERO)
}

/// How a face is chosen when a ray strikes exactly on a shared **edge or corner**,
/// where the surface normal is ambiguous between two or three axes.
///
/// This is a *signposted* policy rather than an accident of float-comparison order:
/// an exact edge/corner hit must always name the same face so picking is
/// deterministic and reproducible across platforms. New policies (e.g. "prefer the
/// face most opposed to the ray direction") can be added as variants without
/// changing the raycast call sites.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FaceAmbiguityPolicy {
    /// Default: pick the axis with the largest `|component|`; break ties by the
    /// fixed axis priority **X > Y > Z**, then **positive over negative** within
    /// the winning axis. So a normal of `(1,1,0)` resolves to `+X`, `(0,1,1)` to
    /// `+Y`, `(1,1,1)` to `+X`, and `(-1,-1,0)` to `-X`.
    #[default]
    AxisPriorityXyzPositiveFirst,
}

impl FaceAmbiguityPolicy {
    /// Resolve a (possibly ambiguous) outward normal to a single [`Face`] under
    /// this policy. Axis-aligned normals are unambiguous; the tie-break only bites
    /// on exact edge/corner hits where two or three components are equal.
    pub fn resolve(self, n: Vector) -> Face {
        match self {
            FaceAmbiguityPolicy::AxisPriorityXyzPositiveFirst => {
                let (ax, ay, az) = (n.x.abs(), n.y.abs(), n.z.abs());
                // `>=` encodes the X > Y > Z priority: on a tie the earlier axis wins.
                if ax >= ay && ax >= az {
                    if n.x >= 0.0 {
                        Face::PosX
                    } else {
                        Face::NegX
                    }
                } else if ay >= az {
                    if n.y >= 0.0 {
                        Face::PosY
                    } else {
                        Face::NegY
                    }
                } else if n.z >= 0.0 {
                    Face::PosZ
                } else {
                    Face::NegZ
                }
            }
        }
    }
}

/// Map an axis-aligned outward normal (from a cuboid hit) to a [`Face`] using the
/// default [`FaceAmbiguityPolicy`].
fn normal_to_face(n: Vector) -> Face {
    FaceAmbiguityPolicy::default().resolve(n)
}

// ── Query vocabulary ───────────────────────────────────────────────────────────

/// A world-space ray (typed; the renderer constructs it from screen coords).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ray {
    pub origin: WorldPos,
    pub dir: WorldVec,
}

impl Ray {
    pub fn new(origin: WorldPos, dir: WorldVec) -> Self {
        Self { origin, dir }
    }
}

/// An **authoritative** ray hit against the collision projection (derived from
/// authoritative voxel state). Renderer-side picks are only hints and must be
/// revalidated through this service before driving edits (see #2259).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VoxelHit {
    /// The solid voxel that was hit.
    pub voxel: VoxelCoord,
    /// The chunk containing [`voxel`](Self::voxel).
    pub chunk: ChunkCoord,
    /// The face of the voxel that was struck (outward normal direction) — the
    /// anchor a "place" edit builds against (`voxel.neighbor(face)`).
    pub face: Face,
    /// The world-space point of impact.
    pub point: WorldPos,
    /// Distance from the ray origin along the (unit-normalised) direction.
    pub distance: f64,
}

// ── Projection ─────────────────────────────────────────────────────────────────

/// The collision projection of a single resident chunk.
struct ChunkCollider {
    /// `content_hash` of the `VoxelChunk` this was built from — the staleness key.
    source_hash: u64,
    /// World-positioned solid cuboids. A chunk with no solids has no collider entry.
    shape: Compound,
}

/// A `parry3d`-backed collision world derived from a [`VoxelWorld`].
pub struct CollisionProjection {
    grid: VoxelGridSpec,
    /// Translation from canonical voxel coordinates into the runtime coordinate
    /// frame queried by cameras, combat, and picking.
    world_offset: WorldVec,
    /// Only chunks with at least one solid voxel appear here (deterministic order).
    chunks: BTreeMap<ChunkCoord, ChunkCollider>,
    /// Bumped on every (re)build so downstream can cheaply detect projection changes.
    version: u64,
}

/// Stable identity for a collision projection and the voxel authority it was
/// derived from. Receipts expose these values so separately invoked operations
/// can prove they queried the same projection substrate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollisionProjectionIdentity {
    pub source_hash: u64,
    pub projection_hash: u64,
}

impl CollisionProjectionIdentity {
    pub fn source_hash_hex(self) -> String {
        format!("{:016x}", self.source_hash)
    }

    pub fn projection_hash_label(self) -> String {
        format!("fnv1a64:{:016x}", self.projection_hash)
    }
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

impl CollisionProjection {
    /// Build a fresh projection over every resident chunk of `world`.
    pub fn build(world: &VoxelWorld) -> Self {
        Self::build_with_offset(world, WorldVec::ZERO)
    }

    /// Build a projection translated into a runtime coordinate frame.
    ///
    /// The canonical voxel world remains unchanged. This is used when a generated
    /// volume is authored in grid-local positive coordinates but its runtime room
    /// frame is centered around the origin.
    pub fn build_with_offset(world: &VoxelWorld, world_offset: WorldVec) -> Self {
        let mut proj = Self {
            grid: world.grid(),
            world_offset,
            chunks: BTreeMap::new(),
            version: 0,
        };
        for (coord, chunk) in world.resident_chunks() {
            proj.set_chunk(coord, chunk);
        }
        proj.version = 1;
        proj
    }

    pub fn grid(&self) -> VoxelGridSpec {
        self.grid
    }

    /// The projection version (incremented on each build/rebuild/reconcile change).
    pub fn version(&self) -> u64 {
        self.version
    }

    /// Number of chunks that currently have a collider (non-empty chunks).
    pub fn collider_count(&self) -> usize {
        self.chunks.len()
    }

    /// Whether `chunk` currently has a collider in the projection.
    pub fn has_collider(&self, chunk: ChunkCoord) -> bool {
        self.chunks.contains_key(&chunk)
    }

    /// Deterministic iterator over chunks that have colliders.
    pub fn collider_chunks(&self) -> impl Iterator<Item = ChunkCoord> + '_ {
        self.chunks.keys().copied()
    }

    /// Compute the versioned identity of this projection from canonical voxel
    /// chunk hashes and deterministic collider coordinates.
    pub fn identity(&self, world: &VoxelWorld) -> CollisionProjectionIdentity {
        let mut source_key = String::new();
        for (coord, chunk) in world.resident_chunks() {
            source_key.push_str(&format!(
                "{},{},{}={:016x};",
                coord.x,
                coord.y,
                coord.z,
                chunk.content_hash().0
            ));
        }
        let source_hash = fnv1a64(source_key.as_bytes());
        let chunks = self
            .collider_chunks()
            .map(|coord| format!("{},{},{}", coord.x, coord.y, coord.z))
            .collect::<Vec<_>>()
            .join(";");
        let projection_key = if self.world_offset == WorldVec::ZERO {
            format!(
                "{source_hash:016x}|v{}|n{}|{chunks}",
                self.version(),
                self.collider_count()
            )
        } else {
            format!(
                "{source_hash:016x}|v{}|n{}|o{:016x},{:016x},{:016x}|{chunks}",
                self.version(),
                self.collider_count(),
                self.world_offset.x.to_bits(),
                self.world_offset.y.to_bits(),
                self.world_offset.z.to_bits(),
            )
        };
        CollisionProjectionIdentity {
            source_hash,
            projection_hash: fnv1a64(projection_key.as_bytes()),
        }
    }

    /// Build/replace the collider for one chunk from its current voxels. Drops the
    /// entry if the chunk has become all-empty.
    fn set_chunk(&mut self, coord: ChunkCoord, chunk: &VoxelChunk) {
        match build_chunk_shape(&self.grid, self.world_offset, coord, chunk) {
            Some(shape) => {
                self.chunks.insert(
                    coord,
                    ChunkCollider {
                        source_hash: chunk.content_hash().0,
                        shape,
                    },
                );
            }
            None => {
                self.chunks.remove(&coord);
            }
        }
    }

    /// Rebuild one chunk's collider from `world`. If the chunk is not resident its
    /// collider is dropped. Bumps the version.
    pub fn rebuild_chunk(&mut self, world: &VoxelWorld, coord: ChunkCoord) {
        match world.get(coord) {
            Some(chunk) => self.set_chunk(coord, chunk),
            None => {
                self.chunks.remove(&coord);
            }
        }
        self.version += 1;
    }

    /// Reconcile a batch of changed chunks (e.g. the partition's drained dirty set)
    /// deterministically. One version bump for the whole batch.
    pub fn reconcile(&mut self, world: &VoxelWorld, changed: &[ChunkCoord]) {
        for &coord in changed {
            match world.get(coord) {
                Some(chunk) => self.set_chunk(coord, chunk),
                None => {
                    self.chunks.remove(&coord);
                }
            }
        }
        self.version += 1;
    }

    /// Whether the projection for `chunk` no longer matches `world`'s current data
    /// (content changed, a chunk gained its first solids, or a collider's chunk is
    /// gone). The basis for coordinated, version-checked rebuilds.
    pub fn is_chunk_stale(&self, world: &VoxelWorld, chunk: ChunkCoord) -> bool {
        match (self.chunks.get(&chunk), world.get(chunk)) {
            (Some(c), Some(data)) => c.source_hash != data.content_hash().0,
            // No collider but the chunk now has solids → stale (needs a build).
            (None, Some(data)) => {
                build_chunk_shape(&self.grid, self.world_offset, chunk, data).is_some()
            }
            // Have a collider but the chunk is gone/unloaded → stale (needs a drop).
            (Some(_), None) => true,
            (None, None) => false,
        }
    }

    /// Occupancy query: is `p` inside a solid voxel's collider? The first query over
    /// the projection (ray/shape queries follow in #2258). Routes to the single
    /// chunk that can contain `p`, then tests the projected cuboids.
    pub fn contains_point(&self, p: WorldPos) -> bool {
        let voxel = self.grid.world_to_voxel(p - self.world_offset);
        let chunk = self.grid.voxel_to_chunk(voxel);
        let Some(collider) = self.chunks.get(&chunk) else {
            return false;
        };
        let point = world_to_point(p);
        // Each part is already world-positioned; test against the part transforms.
        collider
            .shape
            .shapes()
            .iter()
            .any(|(pose, shape)| shape.contains_point(pose, point))
    }

    /// Cast a ray against the projection and return the nearest authoritative hit
    /// within `max_distance`, or `None` on a miss. The shared picking/camera/
    /// placement query — there is no separate renderer-owned authoritative raycast.
    ///
    /// Note: scans all collider chunks and keeps the nearest; a chunk-walk
    /// acceleration is a deferred optimisation, not a separate query system.
    pub fn raycast(&self, ray: Ray, max_distance: f64) -> Option<VoxelHit> {
        let len = ray.dir.length();
        if !len.is_finite() || len <= 0.0 || !max_distance.is_finite() || max_distance <= 0.0 {
            return None;
        }
        let inv = 1.0 / len;
        let dir = WorldVec::new(ray.dir.x * inv, ray.dir.y * inv, ray.dir.z * inv);
        let parry_ray = ParryRay::new(world_to_point(ray.origin), Vector::new(dir.x, dir.y, dir.z));
        let id = identity();

        let mut best: Option<(Real, Vector)> = None;
        for collider in self.chunks.values() {
            if let Some(hit) =
                collider
                    .shape
                    .cast_ray_and_get_normal(&id, &parry_ray, max_distance, true)
            {
                if best.is_none_or(|(t, _)| hit.time_of_impact < t) {
                    best = Some((hit.time_of_impact, hit.normal));
                }
            }
        }

        let (toi, normal) = best?;
        // Impact point, then step a hair inside along the inward normal to name the
        // solid voxel that was hit (the surface point sits exactly on its boundary).
        let point = WorldPos::new(
            ray.origin.x + dir.x * toi,
            ray.origin.y + dir.y * toi,
            ray.origin.z + dir.z * toi,
        );
        let eps = self.grid.voxel_size() * 1e-4;
        let inside = WorldPos::new(
            point.x - normal.x * eps,
            point.y - normal.y * eps,
            point.z - normal.z * eps,
        );
        let voxel = self.grid.world_to_voxel(inside - self.world_offset);
        Some(VoxelHit {
            voxel,
            chunk: self.grid.voxel_to_chunk(voxel),
            face: normal_to_face(normal),
            point,
            distance: toi,
        })
    }

    /// Whether the world-space AABB `[min, max]` overlaps any solid voxel collider.
    /// The placement/camera-basics shape query. Only chunks the AABB spans are tested.
    pub fn aabb_overlaps_solid(&self, min: WorldPos, max: WorldPos) -> bool {
        let lo = WorldPos::new(min.x.min(max.x), min.y.min(max.y), min.z.min(max.z));
        let hi = WorldPos::new(min.x.max(max.x), min.y.max(max.y), min.z.max(max.z));
        let half = Vector::new(
            (hi.x - lo.x) * 0.5,
            (hi.y - lo.y) * 0.5,
            (hi.z - lo.z) * 0.5,
        );
        let cuboid = Cuboid::new(half);
        let pose = Pose::from_translation(Vector::new(
            (lo.x + hi.x) * 0.5,
            (lo.y + hi.y) * 0.5,
            (lo.z + hi.z) * 0.5,
        ));
        let id = identity();
        // Chunk span the AABB covers (inclusive); `hi` is on a boundary so step in.
        let vmin = self.grid.world_to_voxel(lo - self.world_offset);
        let vmax = self.grid.world_to_voxel(hi - self.world_offset);
        let span = ChunkRegion::new(self.grid.voxel_to_chunk(vmin), {
            let c = self.grid.voxel_to_chunk(vmax);
            ChunkCoord::new(c.x + 1, c.y + 1, c.z + 1)
        });
        for chunk in span.iter() {
            if let Some(collider) = self.chunks.get(&chunk) {
                if intersection_test(&pose, &cuboid, &id, &collider.shape) == Ok(true) {
                    return true;
                }
            }
        }
        false
    }

    /// Whether an AABB translated along one axis intersects any solid collider
    /// anywhere on its path. The swept volume of an axis-aligned box moving on a
    /// single axis is itself an AABB, so this continuous query cannot tunnel past
    /// an intervening voxel the way an endpoint-only overlap test can.
    ///
    /// Callers must pass a translation with at most one non-zero component. The
    /// query is intentionally conservative and returns `true` for invalid vectors;
    /// authority callers validate and bound movement before reaching this service.
    pub fn axis_swept_aabb_overlaps_solid(
        &self,
        min: WorldPos,
        max: WorldPos,
        translation: WorldVec,
    ) -> bool {
        let components = [translation.x, translation.y, translation.z];
        if !components.iter().all(|component| component.is_finite())
            || components
                .iter()
                .filter(|component| **component != 0.0)
                .count()
                > 1
        {
            return true;
        }
        let destination_min = WorldPos::new(
            min.x + translation.x,
            min.y + translation.y,
            min.z + translation.z,
        );
        let destination_max = WorldPos::new(
            max.x + translation.x,
            max.y + translation.y,
            max.z + translation.z,
        );
        self.aabb_overlaps_solid(
            WorldPos::new(
                min.x.min(destination_min.x),
                min.y.min(destination_min.y),
                min.z.min(destination_min.z),
            ),
            WorldPos::new(
                max.x.max(destination_max.x),
                max.y.max(destination_max.y),
                max.z.max(destination_max.z),
            ),
        )
    }
}

/// Build the parry `Compound` of world-positioned cuboids for one chunk's solid
/// voxels, or `None` if the chunk has no solids.
fn build_chunk_shape(
    spec: &VoxelGridSpec,
    world_offset: WorldVec,
    coord: ChunkCoord,
    chunk: &VoxelChunk,
) -> Option<Compound> {
    let half: Real = spec.voxel_size() * 0.5;
    let mut parts: Vec<(Pose, SharedShape)> = Vec::new();
    for (local, value) in chunk.iter() {
        if collision_class(value) != CollisionClass::Solid {
            continue;
        }
        let voxel = spec.chunk_local_to_voxel(coord, local);
        let center = spec.voxel_center_world(voxel) + world_offset;
        let pose = Pose::translation(center.x, center.y, center.z);
        parts.push((pose, SharedShape::cuboid(half, half, half)));
    }
    if parts.is_empty() {
        None
    } else {
        Some(Compound::new(parts))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_space::{ChunkDims, GridId, LocalVoxelCoord, VoxelCoord};

    fn spec() -> VoxelGridSpec {
        VoxelGridSpec::new(GridId::new(0), 1.0, ChunkDims::cubic(8).unwrap()).unwrap()
    }

    fn world_with(coord: ChunkCoord, solids: &[LocalVoxelCoord]) -> VoxelWorld {
        let mut w = VoxelWorld::new(spec());
        let mut chunk = VoxelChunk::from_spec(&spec());
        for &l in solids {
            chunk.set(l, VoxelValue::solid_raw(1)).unwrap();
        }
        w.insert(coord, chunk);
        w.drain_dirty();
        w
    }

    #[test]
    fn collision_class_maps_solid_and_empty() {
        assert_eq!(collision_class(VoxelValue::EMPTY), CollisionClass::None);
        assert_eq!(
            collision_class(VoxelValue::solid_raw(3)),
            CollisionClass::Solid
        );
    }

    #[test]
    fn build_skips_empty_chunks_and_keeps_solid_ones() {
        let world = world_with(ChunkCoord::new(0, 0, 0), &[LocalVoxelCoord::new(2, 2, 2)]);
        let proj = CollisionProjection::build(&world);
        assert_eq!(proj.collider_count(), 1);
        assert!(proj.has_collider(ChunkCoord::new(0, 0, 0)));

        // An all-empty resident chunk produces no collider.
        let empty = {
            let mut w = VoxelWorld::new(spec());
            w.insert(ChunkCoord::new(1, 0, 0), VoxelChunk::from_spec(&spec()));
            w
        };
        assert_eq!(CollisionProjection::build(&empty).collider_count(), 0);
    }

    #[test]
    fn contains_point_hits_solid_and_misses_empty_and_negatives() {
        // Solid voxel at local (2,2,2) of chunk 0 → world voxel (2,2,2), cube [2,3)³.
        let world = world_with(ChunkCoord::new(0, 0, 0), &[LocalVoxelCoord::new(2, 2, 2)]);
        let proj = CollisionProjection::build(&world);
        assert!(proj.contains_point(WorldPos::new(2.5, 2.5, 2.5))); // center
        assert!(!proj.contains_point(WorldPos::new(3.5, 2.5, 2.5))); // neighbouring empty cell
        assert!(!proj.contains_point(WorldPos::new(-1.0, -1.0, -1.0))); // outside any chunk

        // A solid in a negative chunk, at a chunk-boundary voxel.
        let neg = world_with(ChunkCoord::new(-1, 0, 0), &[LocalVoxelCoord::new(7, 0, 0)]);
        let negp = CollisionProjection::build(&neg);
        // Chunk -1 local (7,0,0) → world voxel (-1,0,0), cube [-1,0)×[0,1)².
        assert_eq!(
            spec().chunk_local_to_voxel(ChunkCoord::new(-1, 0, 0), LocalVoxelCoord::new(7, 0, 0)),
            VoxelCoord::new(-1, 0, 0)
        );
        assert!(negp.contains_point(WorldPos::new(-0.5, 0.5, 0.5)));
        assert!(!negp.contains_point(WorldPos::new(0.5, 0.5, 0.5)));
    }

    #[test]
    fn projection_detects_staleness_and_rebuilds() {
        let mut world = world_with(ChunkCoord::new(0, 0, 0), &[LocalVoxelCoord::new(0, 0, 0)]);
        let mut proj = CollisionProjection::build(&world);
        let chunk = ChunkCoord::new(0, 0, 0);
        assert!(!proj.is_chunk_stale(&world, chunk));

        // Edit the chunk → projection is now stale until rebuilt.
        world
            .get_mut(chunk)
            .unwrap()
            .set(LocalVoxelCoord::new(1, 1, 1), VoxelValue::solid_raw(1))
            .unwrap();
        assert!(proj.is_chunk_stale(&world, chunk));
        let before = proj.version();
        proj.reconcile(&world, &[chunk]);
        assert!(!proj.is_chunk_stale(&world, chunk));
        assert!(proj.version() > before);
        assert!(proj.contains_point(WorldPos::new(1.5, 1.5, 1.5)));
    }

    #[test]
    fn first_solid_in_untracked_chunk_reads_as_stale() {
        // Chunk resident but all-empty → no collider; after gaining a solid it is stale.
        let mut world = VoxelWorld::new(spec());
        let chunk = ChunkCoord::new(2, 0, 0);
        world.insert(chunk, VoxelChunk::from_spec(&spec()));
        world.drain_dirty();
        let mut proj = CollisionProjection::build(&world);
        assert!(!proj.has_collider(chunk));
        assert!(!proj.is_chunk_stale(&world, chunk));
        world
            .get_mut(chunk)
            .unwrap()
            .set(LocalVoxelCoord::new(0, 0, 0), VoxelValue::solid_raw(1))
            .unwrap();
        assert!(proj.is_chunk_stale(&world, chunk));
        proj.rebuild_chunk(&world, chunk);
        assert!(proj.has_collider(chunk));
    }

    #[test]
    fn unloading_a_chunk_makes_its_collider_stale_then_dropped() {
        let mut world = world_with(ChunkCoord::new(0, 0, 0), &[LocalVoxelCoord::new(0, 0, 0)]);
        let mut proj = CollisionProjection::build(&world);
        let chunk = ChunkCoord::new(0, 0, 0);
        world.unload(chunk).unwrap();
        assert!(proj.is_chunk_stale(&world, chunk));
        proj.rebuild_chunk(&world, chunk);
        assert!(!proj.has_collider(chunk));
    }

    // ── ray / shape queries (#2258) ────────────────────────────────────────────

    #[test]
    fn raycast_hits_nearest_solid_with_correct_face_and_distance() {
        // Solid at world voxel (5,0,0) → cube x in [5,6). Ray from x=0 toward +X
        // along y=z=0.5 strikes the -X face at x=5.
        let world = world_with(ChunkCoord::new(0, 0, 0), &[LocalVoxelCoord::new(5, 0, 0)]);
        let proj = CollisionProjection::build(&world);
        let hit = proj
            .raycast(
                Ray::new(WorldPos::new(0.0, 0.5, 0.5), WorldVec::new(1.0, 0.0, 0.0)),
                100.0,
            )
            .expect("ray should hit");
        assert_eq!(hit.voxel, VoxelCoord::new(5, 0, 0));
        assert_eq!(hit.chunk, ChunkCoord::new(0, 0, 0));
        assert_eq!(hit.face, Face::NegX);
        assert!((hit.distance - 5.0).abs() < 1e-9);
        assert!((hit.point.x - 5.0).abs() < 1e-9);
        // The "place" anchor is the empty neighbour across the struck face.
        assert_eq!(hit.voxel.neighbor(hit.face), VoxelCoord::new(4, 0, 0));
    }

    #[test]
    fn raycast_picks_the_nearest_of_several() {
        let world = world_with(
            ChunkCoord::new(0, 0, 0),
            &[LocalVoxelCoord::new(2, 0, 0), LocalVoxelCoord::new(5, 0, 0)],
        );
        let proj = CollisionProjection::build(&world);
        let hit = proj
            .raycast(
                Ray::new(WorldPos::new(0.0, 0.5, 0.5), WorldVec::new(1.0, 0.0, 0.0)),
                100.0,
            )
            .unwrap();
        assert_eq!(hit.voxel, VoxelCoord::new(2, 0, 0)); // nearer one
    }

    #[test]
    fn raycast_misses_empty_space_and_respects_max_distance() {
        let world = world_with(ChunkCoord::new(0, 0, 0), &[LocalVoxelCoord::new(5, 0, 0)]);
        let proj = CollisionProjection::build(&world);
        // Parallel ray that never enters the solid cell.
        assert!(proj
            .raycast(
                Ray::new(WorldPos::new(0.0, 2.5, 0.5), WorldVec::new(1.0, 0.0, 0.0)),
                100.0
            )
            .is_none());
        // Hits exist but are beyond max_distance.
        assert!(proj
            .raycast(
                Ray::new(WorldPos::new(0.0, 0.5, 0.5), WorldVec::new(1.0, 0.0, 0.0)),
                3.0
            )
            .is_none());
        // Degenerate ray.
        assert!(proj
            .raycast(
                Ray::new(WorldPos::new(0.0, 0.5, 0.5), WorldVec::ZERO),
                100.0
            )
            .is_none());
    }

    #[test]
    fn raycast_traverses_chunk_boundary_and_negatives() {
        // Solid in a negative chunk; ray travels in -X from positive space.
        let world = world_with(ChunkCoord::new(-1, 0, 0), &[LocalVoxelCoord::new(7, 0, 0)]);
        let proj = CollisionProjection::build(&world);
        // World voxel (-1,0,0), cube x in [-1,0). Ray from x=5 toward -X strikes +X face at x=0.
        let hit = proj
            .raycast(
                Ray::new(WorldPos::new(5.0, 0.5, 0.5), WorldVec::new(-1.0, 0.0, 0.0)),
                100.0,
            )
            .unwrap();
        assert_eq!(hit.voxel, VoxelCoord::new(-1, 0, 0));
        assert_eq!(hit.chunk, ChunkCoord::new(-1, 0, 0));
        assert_eq!(hit.face, Face::PosX);
        assert!((hit.distance - 5.0).abs() < 1e-9);
    }

    #[test]
    fn aabb_overlap_detects_solid_and_clears_empty() {
        let world = world_with(ChunkCoord::new(0, 0, 0), &[LocalVoxelCoord::new(2, 2, 2)]);
        let proj = CollisionProjection::build(&world);
        // Box around the solid cube [2,3)³ overlaps.
        assert!(
            proj.aabb_overlaps_solid(WorldPos::new(2.2, 2.2, 2.2), WorldPos::new(2.8, 2.8, 2.8))
        );
        // Box in empty space does not.
        assert!(
            !proj.aabb_overlaps_solid(WorldPos::new(5.0, 5.0, 5.0), WorldPos::new(5.5, 5.5, 5.5))
        );
    }

    #[test]
    fn aabb_overlap_spans_chunks() {
        let mut world = VoxelWorld::new(spec());
        let mut c1 = VoxelChunk::from_spec(&spec());
        c1.set(LocalVoxelCoord::new(7, 0, 0), VoxelValue::solid_raw(1))
            .unwrap(); // world (7,0,0)
        world.insert(ChunkCoord::new(0, 0, 0), c1);
        world.insert(ChunkCoord::new(1, 0, 0), VoxelChunk::from_spec(&spec()));
        world.drain_dirty();
        let proj = CollisionProjection::build(&world);
        // AABB straddling the chunk-0/chunk-1 boundary still finds the solid in chunk 0.
        assert!(
            proj.aabb_overlaps_solid(WorldPos::new(7.5, 0.5, 0.5), WorldPos::new(8.5, 0.5, 0.5))
        );
    }

    #[test]
    fn axis_swept_aabb_detects_intervening_solid_without_endpoint_overlap() {
        let world = world_with(ChunkCoord::new(0, 0, 0), &[LocalVoxelCoord::new(2, 0, 0)]);
        let proj = CollisionProjection::build(&world);
        let min = WorldPos::new(0.1, 0.1, 0.1);
        let max = WorldPos::new(0.9, 0.9, 0.9);

        assert!(
            !proj.aabb_overlaps_solid(WorldPos::new(4.1, 0.1, 0.1), WorldPos::new(4.9, 0.9, 0.9))
        );
        assert!(proj.axis_swept_aabb_overlaps_solid(min, max, WorldVec::new(4.0, 0.0, 0.0)));
        assert!(!proj.axis_swept_aabb_overlaps_solid(
            WorldPos::new(0.1, 2.1, 0.1),
            WorldPos::new(0.9, 2.9, 0.9),
            WorldVec::new(4.0, 0.0, 0.0)
        ));
    }

    #[test]
    fn axis_swept_aabb_fails_closed_for_non_axis_translation() {
        let projection = CollisionProjection::build(&VoxelWorld::new(spec()));
        let min = WorldPos::new(0.0, 0.0, 0.0);
        let max = WorldPos::new(1.0, 1.0, 1.0);

        assert!(projection.axis_swept_aabb_overlaps_solid(min, max, WorldVec::new(1.0, 1.0, 0.0)));
        assert!(projection.axis_swept_aabb_overlaps_solid(
            min,
            max,
            WorldVec::new(f64::INFINITY, 0.0, 0.0)
        ));
    }

    #[test]
    fn translated_projection_queries_runtime_frame_and_reports_canonical_voxel() {
        let world = world_with(ChunkCoord::ORIGIN, &[LocalVoxelCoord::new(2, 1, 4)]);
        let offset = WorldVec::new(-2.5, -1.0, -4.5);
        let projection = CollisionProjection::build_with_offset(&world, offset);
        let runtime_center = WorldPos::new(0.0, 0.5, 0.0);

        assert!(projection.contains_point(runtime_center));
        assert!(!projection.contains_point(WorldPos::new(2.5, 1.5, 4.5)));

        let hit = projection
            .raycast(
                Ray::new(WorldPos::new(0.0, 0.5, -2.0), WorldVec::new(0.0, 0.0, 1.0)),
                4.0,
            )
            .expect("translated collider is hit in the runtime frame");
        assert_eq!(hit.voxel, VoxelCoord::new(2, 1, 4));
        assert_eq!(hit.point, WorldPos::new(0.0, 0.5, -0.5));
    }

    #[test]
    fn face_ambiguity_policy_resolves_edge_and_corner_ties_deterministically() {
        use parry3d_f64::math::Vector;
        let p = FaceAmbiguityPolicy::default();
        // Axis-aligned normals are unambiguous.
        assert_eq!(p.resolve(Vector::new(1.0, 0.0, 0.0)), Face::PosX);
        assert_eq!(p.resolve(Vector::new(0.0, -1.0, 0.0)), Face::NegY);
        assert_eq!(p.resolve(Vector::new(0.0, 0.0, 1.0)), Face::PosZ);
        // Exact EDGE hits (two equal components) → fixed axis priority X > Y > Z.
        assert_eq!(p.resolve(Vector::new(1.0, 1.0, 0.0)), Face::PosX);
        assert_eq!(p.resolve(Vector::new(0.0, 1.0, 1.0)), Face::PosY);
        assert_eq!(p.resolve(Vector::new(1.0, 0.0, 1.0)), Face::PosX);
        // Exact CORNER hit (three equal components) → X wins.
        assert_eq!(p.resolve(Vector::new(1.0, 1.0, 1.0)), Face::PosX);
        // Sign tie-break keeps the winning axis's own sign.
        assert_eq!(p.resolve(Vector::new(-1.0, -1.0, 0.0)), Face::NegX);
        assert_eq!(p.resolve(Vector::new(0.0, -1.0, -1.0)), Face::NegY);
    }
}
