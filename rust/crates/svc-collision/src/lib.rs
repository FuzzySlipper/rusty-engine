//! `parry3d`-backed collision projection derived from voxel and static-mesh authority.
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
//!   detects drift so rebuilds stay coordinated with the chunk dirty queue. Immutable
//!   triangle assets and caller-owned instances enter through one exact-revision,
//!   fail-atomic replacement boundary.
//! - **Typed boundary.** ASHA coordinate types (`WorldPos`, `VoxelGridSpec`) cross
//!   the public API; `parry3d` `Pose`/`Vector`/`Compound` (glam-backed) stay
//!   internal so coordinate-space distinctions are not erased.
//! - **f64 throughout** (`parry3d-f64`) to match `core-space`'s `WorldScalar`.
//! - **No raw parry-world mutation is exposed.** Callers build/reconcile and query;
//!   they never poke the parry compound directly.
//!
//! Each solid voxel becomes a world-positioned cuboid in a per-chunk `Compound`.
//! External static assets use bounded Parry triangle meshes; callers retain asset,
//! entity, transform, storage, and lifecycle authority.
//!
//! Queries are the **one shared vocabulary** for picking, camera, and placement:
//! [`CollisionProjection::contains_point`] (occupancy), [`CollisionProjection::raycast`]
//! (nearest authoritative [`VoxelHit`] with face/distance), and
//! [`CollisionProjection::aabb_overlaps_solid`] (placement/camera shape test), and
//! [`CollisionProjection::axis_swept_aabb_overlaps_solid`] (continuous axis-aligned
//! camera movement). There is no separate renderer-owned authoritative raycast;
//! renderer picks are hints revalidated here (#2259).

#![forbid(unsafe_code)]

mod dynamics;
mod static_mesh;

pub use dynamics::{
    simulate_dynamics, DynamicsAction, DynamicsBodyId, DynamicsBodyInput, DynamicsBodyOutput,
    DynamicsContact, DynamicsError, DynamicsMassProperties, DynamicsShape, DynamicsStepInput,
    DynamicsStepOutput, MAX_CCD_TRANSLATION_PER_STEP, MAX_DISCRETE_TRANSLATION_PER_STEP,
    MAX_DYNAMICS_ACTIONS, MAX_DYNAMICS_BODIES, MAX_DYNAMICS_CONTACTS, MAX_DYNAMICS_STEPS,
    MAX_DYNAMICS_STEP_SECONDS, MIN_DYNAMICS_STEP_SECONDS,
};

pub use static_mesh::{
    StaticMeshAssetId, StaticMeshColliderAsset, StaticMeshColliderInstance,
    StaticMeshCollisionError, StaticMeshCollisionProjection, StaticMeshCollisionReceipt,
    StaticMeshHit, StaticMeshInstanceId, StaticMeshTransform, MAX_STATIC_MESH_ASSETS,
    MAX_STATIC_MESH_INSTANCES, MAX_STATIC_MESH_TRIANGLES, MAX_STATIC_MESH_TRIANGLES_PER_ASSET,
    MAX_STATIC_MESH_VERTICES, MAX_STATIC_MESH_VERTICES_PER_ASSET,
};

use std::collections::BTreeMap;

use core_space::{ChunkCoord, ChunkRegion, Face, VoxelCoord, VoxelGridSpec, WorldPos, WorldVec};
use core_voxel::VoxelValue;
use svc_spatial::VoxelWorld;
use svc_volume::VoxelChunk;

use parry3d_f64::math::{Pose, Real, Vector};
use parry3d_f64::query::{
    cast_shapes, contact, intersection_test, Contact, Ray as ParryRay, RayCast, ShapeCastHit,
    ShapeCastOptions, ShapeCastStatus,
};
use parry3d_f64::shape::{Capsule, Compound, Cuboid, SharedShape};

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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CollisionHit {
    Voxel(VoxelHit),
    StaticMesh(StaticMeshHit),
}

impl CollisionHit {
    pub fn distance(self) -> f64 {
        match self {
            Self::Voxel(hit) => hit.distance,
            Self::StaticMesh(hit) => hit.distance,
        }
    }
}

// ── Projection ─────────────────────────────────────────────────────────────────

/// The collision projection of a single resident chunk.
#[derive(Clone)]
struct ChunkCollider {
    /// `content_hash` of the `VoxelChunk` this was built from — the staleness key.
    source_hash: u64,
    /// World-positioned solid cuboids. A chunk with no solids has no collider entry.
    shape: Compound,
}

/// A `parry3d`-backed collision world derived from a [`VoxelWorld`].
#[derive(Clone)]
pub struct CollisionProjection {
    grid: VoxelGridSpec,
    /// Translation from canonical voxel coordinates into the runtime coordinate
    /// frame queried by cameras, combat, and picking.
    world_offset: WorldVec,
    /// Only chunks with at least one solid voxel appear here (deterministic order).
    chunks: BTreeMap<ChunkCoord, ChunkCollider>,
    static_meshes: StaticMeshCollisionProjection,
    /// Bumped on every (re)build so downstream can cheaply detect projection changes.
    version: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CharacterCapsule {
    pub center: WorldPos,
    /// Half the central line segment, excluding the spherical caps.
    pub half_height: f64,
    pub radius: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CharacterObstacle {
    pub id: u64,
    pub center: WorldPos,
    pub half_extents: WorldVec,
    pub linear_velocity: WorldVec,
    pub angular_velocity: WorldVec,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CharacterCollisionSource {
    VoxelChunk(ChunkCoord),
    StaticMesh {
        instance: StaticMeshInstanceId,
        asset: StaticMeshAssetId,
        geometry_hash: u64,
    },
    ActiveEntity(u64),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CharacterCapsuleCastHit {
    pub source: CharacterCollisionSource,
    /// Fraction in `[0, 1]` of the requested translation.
    pub time_of_impact: f64,
    pub point: WorldPos,
    /// World-space surface normal pointing away from the obstacle.
    pub normal: WorldVec,
    pub start_solid: bool,
    pub converged: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CharacterCapsuleOverlap {
    pub source: CharacterCollisionSource,
    pub point: WorldPos,
    /// World-space separation direction for the capsule.
    pub normal: WorldVec,
    pub penetration_depth: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharacterCollisionQueryError {
    InvalidCapsule,
    InvalidTranslation,
    InvalidContactSkin,
    UnsupportedBackendQuery,
}

impl std::fmt::Display for CharacterCollisionQueryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "character collision query rejected: {self:?}")
    }
}

impl std::error::Error for CharacterCollisionQueryError {}

pub fn cast_character_capsule_against_obstacles(
    capsule: CharacterCapsule,
    translation: WorldVec,
    contact_skin: f64,
    obstacles: &[CharacterObstacle],
) -> Result<Option<CharacterCapsuleCastHit>, CharacterCollisionQueryError> {
    validate_character_query(capsule, translation, contact_skin)?;
    let moving_pose = Pose::translation(capsule.center.x, capsule.center.y, capsule.center.z);
    let moving_shape = Capsule::new_y(capsule.half_height, capsule.radius);
    let velocity = Vector::new(translation.x, translation.y, translation.z);
    let options = ShapeCastOptions {
        max_time_of_impact: 1.0,
        target_distance: contact_skin,
        stop_at_penetration: true,
        compute_impact_geometry_on_penetration: true,
    };
    let mut best = None;
    for obstacle in obstacles {
        validate_obstacle(*obstacle)?;
        let obstacle_pose =
            Pose::translation(obstacle.center.x, obstacle.center.y, obstacle.center.z);
        let obstacle_shape = Cuboid::new(Vector::new(
            obstacle.half_extents.x,
            obstacle.half_extents.y,
            obstacle.half_extents.z,
        ));
        let hit = cast_shapes(
            &moving_pose,
            velocity,
            &moving_shape,
            &obstacle_pose,
            Vector::ZERO,
            &obstacle_shape,
            options,
        )
        .map_err(|_| CharacterCollisionQueryError::UnsupportedBackendQuery)?;
        let hit = hit.map(|mut hit| {
            hit.witness2 += Vector::new(obstacle.center.x, obstacle.center.y, obstacle.center.z);
            hit
        });
        keep_nearest_character_hit(
            &mut best,
            CharacterCollisionSource::ActiveEntity(obstacle.id),
            hit,
        );
    }
    Ok(best)
}

pub fn character_capsule_overlap_obstacles(
    capsule: CharacterCapsule,
    obstacles: &[CharacterObstacle],
) -> Result<Option<CharacterCapsuleOverlap>, CharacterCollisionQueryError> {
    validate_character_query(capsule, WorldVec::ZERO, 0.0)?;
    let capsule_pose = Pose::translation(capsule.center.x, capsule.center.y, capsule.center.z);
    let capsule_shape = Capsule::new_y(capsule.half_height, capsule.radius);
    let mut best = None;
    for obstacle in obstacles {
        validate_obstacle(*obstacle)?;
        let obstacle_pose =
            Pose::translation(obstacle.center.x, obstacle.center.y, obstacle.center.z);
        let obstacle_shape = Cuboid::new(Vector::new(
            obstacle.half_extents.x,
            obstacle.half_extents.y,
            obstacle.half_extents.z,
        ));
        let result = contact(
            &capsule_pose,
            &capsule_shape,
            &obstacle_pose,
            &obstacle_shape,
            0.0,
        )
        .map_err(|_| CharacterCollisionQueryError::UnsupportedBackendQuery)?;
        let result = result.map(|mut contact| {
            contact.point2 += Vector::new(obstacle.center.x, obstacle.center.y, obstacle.center.z);
            contact
        });
        keep_deepest_character_overlap(
            &mut best,
            CharacterCollisionSource::ActiveEntity(obstacle.id),
            result,
        );
    }
    Ok(best)
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
            static_meshes: StaticMeshCollisionProjection::default(),
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

    pub fn static_mesh_revision(&self) -> u64 {
        self.static_meshes.revision()
    }

    pub fn static_mesh_asset_count(&self) -> usize {
        self.static_meshes.asset_count()
    }

    pub fn static_mesh_instance_count(&self) -> usize {
        self.static_meshes.instance_count()
    }

    /// Cast a local +Y capsule through the immutable voxel/static-mesh snapshot.
    /// Exact-distance ties retain deterministic source order: voxel chunks first,
    /// then static-mesh instance identity.
    pub fn cast_character_capsule(
        &self,
        capsule: CharacterCapsule,
        translation: WorldVec,
        contact_skin: f64,
    ) -> Result<Option<CharacterCapsuleCastHit>, CharacterCollisionQueryError> {
        validate_character_query(capsule, translation, contact_skin)?;
        let moving_pose = Pose::translation(capsule.center.x, capsule.center.y, capsule.center.z);
        let moving_shape = Capsule::new_y(capsule.half_height, capsule.radius);
        let velocity = Vector::new(translation.x, translation.y, translation.z);
        let obstacle_pose = identity();
        let options = ShapeCastOptions {
            max_time_of_impact: 1.0,
            target_distance: contact_skin,
            stop_at_penetration: true,
            compute_impact_geometry_on_penetration: true,
        };
        let mut best = None;
        for (coord, collider) in &self.chunks {
            let hit = cast_shapes(
                &moving_pose,
                velocity,
                &moving_shape,
                &obstacle_pose,
                Vector::ZERO,
                &collider.shape,
                options,
            )
            .map_err(|_| CharacterCollisionQueryError::UnsupportedBackendQuery)?;
            keep_nearest_character_hit(
                &mut best,
                CharacterCollisionSource::VoxelChunk(*coord),
                hit,
            );
        }
        for (instance, asset, geometry_hash, shape) in self.static_meshes.character_shapes() {
            let hit = cast_shapes(
                &moving_pose,
                velocity,
                &moving_shape,
                &obstacle_pose,
                Vector::ZERO,
                shape.as_ref(),
                options,
            )
            .map_err(|_| CharacterCollisionQueryError::UnsupportedBackendQuery)?;
            keep_nearest_character_hit(
                &mut best,
                CharacterCollisionSource::StaticMesh {
                    instance,
                    asset,
                    geometry_hash,
                },
                hit,
            );
        }
        Ok(best)
    }

    /// Return the deepest current capsule overlap, with deterministic source
    /// ordering on equal penetration. Repeated calls after bounded correction
    /// let the character service recover multiple simultaneous overlaps.
    pub fn character_capsule_overlap(
        &self,
        capsule: CharacterCapsule,
    ) -> Result<Option<CharacterCapsuleOverlap>, CharacterCollisionQueryError> {
        validate_character_query(capsule, WorldVec::ZERO, 0.0)?;
        let capsule_pose = Pose::translation(capsule.center.x, capsule.center.y, capsule.center.z);
        let capsule_shape = Capsule::new_y(capsule.half_height, capsule.radius);
        let obstacle_pose = identity();
        let mut best = None;
        for (coord, collider) in &self.chunks {
            let result = contact(
                &capsule_pose,
                &capsule_shape,
                &obstacle_pose,
                &collider.shape,
                0.0,
            )
            .map_err(|_| CharacterCollisionQueryError::UnsupportedBackendQuery)?;
            keep_deepest_character_overlap(
                &mut best,
                CharacterCollisionSource::VoxelChunk(*coord),
                result,
            );
        }
        for (instance, asset, geometry_hash, shape) in self.static_meshes.character_shapes() {
            let result = contact(
                &capsule_pose,
                &capsule_shape,
                &obstacle_pose,
                shape.as_ref(),
                0.0,
            )
            .map_err(|_| CharacterCollisionQueryError::UnsupportedBackendQuery)?;
            keep_deepest_character_overlap(
                &mut best,
                CharacterCollisionSource::StaticMesh {
                    instance,
                    asset,
                    geometry_hash,
                },
                result,
            );
        }
        Ok(best)
    }

    pub fn replace_static_meshes(
        &mut self,
        expected_revision: u64,
        assets: impl IntoIterator<Item = StaticMeshColliderAsset>,
        instances: impl IntoIterator<Item = StaticMeshColliderInstance>,
    ) -> Result<StaticMeshCollisionReceipt, StaticMeshCollisionError> {
        self.static_meshes
            .replace_all(expected_revision, assets, instances)
    }

    /// Preserve the caller-owned derived static-mesh projection while voxel
    /// authority is rebuilt transactionally.
    pub fn copy_static_meshes_from(&mut self, source: &Self) {
        self.static_meshes = source.static_meshes.clone();
    }

    /// Copy the immutable static-mesh projection into another local coordinate
    /// frame while preserving its authored revision and stable identities.
    pub fn copy_translated_static_meshes_from(
        &mut self,
        source: &Self,
        delta: WorldVec,
    ) -> Result<(), StaticMeshCollisionError> {
        self.static_meshes = source.static_meshes.translated(delta)?;
        Ok(())
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
        let grid_origin = self.grid.origin_world();
        let mut projection_key = if self.world_offset == WorldVec::ZERO
            && grid_origin == WorldPos::ORIGIN
        {
            format!(
                "{source_hash:016x}|v{}|n{}|{chunks}",
                self.version(),
                self.collider_count()
            )
        } else {
            format!(
                "{source_hash:016x}|v{}|n{}|o{:016x},{:016x},{:016x}|g{:016x},{:016x},{:016x}|{chunks}",
                self.version(),
                self.collider_count(),
                self.world_offset.x.to_bits(),
                self.world_offset.y.to_bits(),
                self.world_offset.z.to_bits(),
                grid_origin.x.to_bits(),
                grid_origin.y.to_bits(),
                grid_origin.z.to_bits(),
            )
        };
        if self.static_meshes.revision() != 0 {
            projection_key.push_str(&format!(
                "|s{}:{:016x}",
                self.static_meshes.revision(),
                self.static_meshes.identity_hash()
            ));
        }
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

    /// Cast against voxel and external static-mesh colliders and return the
    /// nearest hit. Exact-distance ties prefer voxel authority so existing voxel
    /// edit anchors remain deterministic.
    pub fn raycast_world(&self, ray: Ray, max_distance: f64) -> Option<CollisionHit> {
        let voxel = self.raycast(ray, max_distance).map(CollisionHit::Voxel);
        let static_mesh = self
            .static_meshes
            .raycast(ray, max_distance)
            .map(CollisionHit::StaticMesh);
        match (voxel, static_mesh) {
            (Some(voxel), Some(static_mesh)) => {
                if static_mesh.distance() < voxel.distance() {
                    Some(static_mesh)
                } else {
                    Some(voxel)
                }
            }
            (Some(hit), None) | (None, Some(hit)) => Some(hit),
            (None, None) => None,
        }
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
        self.static_meshes.aabb_overlaps(lo, hi)
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
        let swept_min = WorldPos::new(
            min.x.min(destination_min.x),
            min.y.min(destination_min.y),
            min.z.min(destination_min.z),
        );
        let swept_max = WorldPos::new(
            max.x.max(destination_max.x),
            max.y.max(destination_max.y),
            max.z.max(destination_max.z),
        );
        let voxel_overlap = self.aabb_overlaps_voxels(swept_min, swept_max);
        voxel_overlap
            || self
                .static_meshes
                .swept_aabb_overlaps(min, max, translation)
    }

    fn aabb_overlaps_voxels(&self, min: WorldPos, max: WorldPos) -> bool {
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
        let vmin = self.grid.world_to_voxel(lo - self.world_offset);
        let vmax = self.grid.world_to_voxel(hi - self.world_offset);
        let span = ChunkRegion::new(self.grid.voxel_to_chunk(vmin), {
            let c = self.grid.voxel_to_chunk(vmax);
            ChunkCoord::new(c.x + 1, c.y + 1, c.z + 1)
        });
        span.iter().any(|chunk| {
            self.chunks.get(&chunk).is_some_and(|collider| {
                intersection_test(&pose, &cuboid, &id, &collider.shape) == Ok(true)
            })
        })
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

fn validate_character_query(
    capsule: CharacterCapsule,
    translation: WorldVec,
    contact_skin: f64,
) -> Result<(), CharacterCollisionQueryError> {
    if ![
        capsule.center.x,
        capsule.center.y,
        capsule.center.z,
        capsule.half_height,
        capsule.radius,
    ]
    .into_iter()
    .all(f64::is_finite)
        || capsule.half_height < 0.0
        || capsule.radius <= 0.0
    {
        return Err(CharacterCollisionQueryError::InvalidCapsule);
    }
    if ![translation.x, translation.y, translation.z]
        .into_iter()
        .all(f64::is_finite)
    {
        return Err(CharacterCollisionQueryError::InvalidTranslation);
    }
    if !contact_skin.is_finite() || contact_skin < 0.0 {
        return Err(CharacterCollisionQueryError::InvalidContactSkin);
    }
    Ok(())
}

fn validate_obstacle(obstacle: CharacterObstacle) -> Result<(), CharacterCollisionQueryError> {
    if ![
        obstacle.center.x,
        obstacle.center.y,
        obstacle.center.z,
        obstacle.half_extents.x,
        obstacle.half_extents.y,
        obstacle.half_extents.z,
        obstacle.linear_velocity.x,
        obstacle.linear_velocity.y,
        obstacle.linear_velocity.z,
        obstacle.angular_velocity.x,
        obstacle.angular_velocity.y,
        obstacle.angular_velocity.z,
    ]
    .into_iter()
    .all(f64::is_finite)
        || obstacle.half_extents.x <= 0.0
        || obstacle.half_extents.y <= 0.0
        || obstacle.half_extents.z <= 0.0
    {
        Err(CharacterCollisionQueryError::InvalidCapsule)
    } else {
        Ok(())
    }
}

fn keep_nearest_character_hit(
    best: &mut Option<CharacterCapsuleCastHit>,
    source: CharacterCollisionSource,
    hit: Option<ShapeCastHit>,
) {
    let Some(hit) = hit else {
        return;
    };
    if best.as_ref().is_some_and(|current| {
        current.time_of_impact < hit.time_of_impact
            || (current.time_of_impact == hit.time_of_impact && current.source <= source)
    }) {
        return;
    }
    *best = Some(CharacterCapsuleCastHit {
        source,
        time_of_impact: hit.time_of_impact.clamp(0.0, 1.0),
        point: WorldPos::new(hit.witness2.x, hit.witness2.y, hit.witness2.z),
        normal: WorldVec::new(hit.normal2.x, hit.normal2.y, hit.normal2.z),
        start_solid: hit.status == ShapeCastStatus::PenetratingOrWithinTargetDist,
        converged: hit.status == ShapeCastStatus::Converged,
    });
}

fn keep_deepest_character_overlap(
    best: &mut Option<CharacterCapsuleOverlap>,
    source: CharacterCollisionSource,
    contact: Option<Contact>,
) {
    let Some(contact) = contact.filter(|contact| contact.dist < 0.0) else {
        return;
    };
    let depth = -contact.dist;
    if best.as_ref().is_some_and(|current| {
        current.penetration_depth > depth
            || (current.penetration_depth == depth && current.source <= source)
    }) {
        return;
    }
    *best = Some(CharacterCapsuleOverlap {
        source,
        point: WorldPos::new(contact.point2.x, contact.point2.y, contact.point2.z),
        normal: WorldVec::new(contact.normal2.x, contact.normal2.y, contact.normal2.z),
        penetration_depth: depth,
    });
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
    fn character_capsule_cast_returns_toi_normal_and_stable_source() {
        let world = world_with(ChunkCoord::new(0, 0, 0), &[LocalVoxelCoord::new(2, 0, 2)]);
        let projection = CollisionProjection::build(&world);
        let hit = projection
            .cast_character_capsule(
                CharacterCapsule {
                    center: WorldPos::new(0.5, 1.0, 2.5),
                    half_height: 0.5,
                    radius: 0.4,
                },
                WorldVec::new(3.0, 0.0, 0.0),
                0.0,
            )
            .unwrap()
            .expect("wall hit");
        assert_eq!(
            hit.source,
            CharacterCollisionSource::VoxelChunk(ChunkCoord::new(0, 0, 0))
        );
        assert!((hit.time_of_impact - (1.1 / 3.0)).abs() < 1.0e-6);
        assert!(hit.normal.x < -0.99);
        assert!(!hit.start_solid);
    }

    #[test]
    fn character_capsule_overlap_reports_separation_depth() {
        let world = world_with(ChunkCoord::new(0, 0, 0), &[LocalVoxelCoord::new(2, 0, 2)]);
        let projection = CollisionProjection::build(&world);
        let overlap = projection
            .character_capsule_overlap(CharacterCapsule {
                center: WorldPos::new(1.8, 1.0, 2.5),
                half_height: 0.5,
                radius: 0.4,
            })
            .unwrap()
            .expect("overlap");
        assert!((overlap.penetration_depth - 0.2).abs() < 1.0e-6);
        assert!(overlap.normal.x < -0.99);
    }

    #[test]
    fn active_character_obstacle_ties_use_identity_and_report_world_points() {
        let capsule = CharacterCapsule {
            center: WorldPos::new(0.0, 1.0, 0.0),
            half_height: 0.5,
            radius: 0.4,
        };
        let obstacle = |id| CharacterObstacle {
            id,
            center: WorldPos::new(2.0, 1.0, 0.0),
            half_extents: WorldVec::new(0.5, 0.5, 0.5),
            linear_velocity: WorldVec::ZERO,
            angular_velocity: WorldVec::ZERO,
        };
        for obstacles in [[obstacle(9), obstacle(3)], [obstacle(3), obstacle(9)]] {
            let hit = cast_character_capsule_against_obstacles(
                capsule,
                WorldVec::new(3.0, 0.0, 0.0),
                0.0,
                &obstacles,
            )
            .unwrap()
            .unwrap();
            assert_eq!(hit.source, CharacterCollisionSource::ActiveEntity(3));
            assert!((hit.point.x - 1.5).abs() < 1.0e-6);
        }
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
    fn voxel_and_static_mesh_colliders_share_nearest_hit_and_sweep_queries() {
        let world = world_with(ChunkCoord::new(0, 0, 0), &[LocalVoxelCoord::new(0, 0, 4)]);
        let mut projection = CollisionProjection::build(&world);
        let asset = StaticMeshColliderAsset::new(
            StaticMeshAssetId(3),
            vec![[-1.0, -1.0, 2.0], [1.0, -1.0, 2.0], [0.0, 1.0, 2.0]],
            vec![[0, 1, 2]],
        )
        .unwrap();
        let hash = asset.geometry_hash;
        projection
            .replace_static_meshes(
                0,
                [asset],
                [StaticMeshColliderInstance {
                    id: StaticMeshInstanceId(9),
                    asset: StaticMeshAssetId(3),
                    expected_geometry_hash: hash,
                    transform: StaticMeshTransform::IDENTITY,
                }],
            )
            .unwrap();

        let ray = Ray::new(WorldPos::new(0.0, 0.0, 0.0), WorldVec::new(0.0, 0.0, 1.0));
        assert_eq!(projection.raycast(ray, 10.0).unwrap().voxel.z, 4);
        assert!(matches!(
            projection.raycast_world(ray, 10.0),
            Some(CollisionHit::StaticMesh(StaticMeshHit {
                instance: StaticMeshInstanceId(9),
                distance,
                ..
            })) if (distance - 2.0).abs() < 1.0e-9
        ));
        assert!(projection.axis_swept_aabb_overlaps_solid(
            WorldPos::new(-0.25, -0.25, 0.0),
            WorldPos::new(0.25, 0.25, 0.5),
            WorldVec::new(0.0, 0.0, 3.0),
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
