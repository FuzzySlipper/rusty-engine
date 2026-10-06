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
mod tether;

pub use tether::{
    DynamicsRopeSolverConfig, DynamicsTether, DynamicsTetherEndpoint, DynamicsTetherError,
    DynamicsTetherReadout, TETHER_SOLVER_ITERATIONS, TETHER_SUBSTEPS,
};

pub use dynamics::{
    DynamicsAction, DynamicsAnchorObservation, DynamicsBodyId, DynamicsBodyInput,
    DynamicsBodyOutput, DynamicsContact, DynamicsEnvironmentReceipt, DynamicsError,
    DynamicsMassProperties, DynamicsShape, DynamicsSolver, DynamicsStepReceipt,
};

pub use static_mesh::{
    StaticMeshAssetId, StaticMeshColliderAsset, StaticMeshColliderInstance,
    StaticMeshCollisionError, StaticMeshCollisionProjection, StaticMeshCollisionReceipt,
    StaticMeshHit, StaticMeshInstanceId, StaticMeshTransform,
};

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use core_space::{ChunkCoord, ChunkRegion, Face, VoxelCoord, VoxelGridSpec, WorldPos, WorldVec};
use core_voxel::VoxelValue;
use svc_spatial::VoxelWorld;
use svc_volume::VoxelChunk;

use parry3d_f64::math::{Pose, Real, Vector};
use parry3d_f64::query::{
    cast_shapes, contact, intersection_test, Contact, PointQueryWithLocation, Ray as ParryRay,
    ShapeCastHit, ShapeCastOptions, ShapeCastStatus,
};
use parry3d_f64::shape::{
    Capsule, CompositeShapeRef, Compound, Cuboid, Shape, SharedShape, TriMesh,
};

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
    /// The unit surface normal at the impact: a face axis on a cube, the
    /// triangle's normal on a reconstructed surface.
    pub normal: WorldVec,
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

/// The collision projection of a single resident chunk. A chunk with no
/// collidable part has no collider entry.
#[derive(Clone)]
struct ChunkCollider {
    /// `content_hash` of the `VoxelChunk` this was built from — the staleness key.
    source_hash: u64,
    /// World-positioned solid cuboids.
    cubes: Option<Arc<Compound>>,
    /// The canonical voxels of each Compound child, in the same order as
    /// `cubes.shapes()`. A cube voxel's own cuboid names its voxel exactly:
    /// a ray may strike a top face precisely at an adjacent sparse-cell
    /// boundary. A merged box names the voxel under the impact.
    boxes: Vec<VoxelBox>,
    /// The reconstructed surface drawn for this chunk, when its materials are
    /// not drawn as cubes.
    surface: Option<ChunkSurfacePart>,
    /// Conservatively cached world-space bounds for outer character-query
    /// pruning. `None` deliberately means "unknown": queries fail open to
    /// the established full scan instead of risking a false negative.
    bounds: Option<WorldAabb>,
}

/// An inclusive box of voxels: one collided cuboid, or the voxels owning a
/// merged surface face. A ray hit names the voxel just inside it at the
/// impact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VoxelBox {
    pub min: VoxelCoord,
    pub max: VoxelCoord,
}

impl VoxelBox {
    pub const fn single(voxel: VoxelCoord) -> Self {
        Self {
            min: voxel,
            max: voxel,
        }
    }
}

/// One chunk's collider parts, built by
/// [`CollisionProjection::prepare_chunk_parts`] or
/// [`CollisionProjection::prepare_chunk`].
pub struct PreparedChunkParts {
    boxes: Vec<VoxelBox>,
    cubes: Option<Arc<Compound>>,
    surface: Option<ChunkSurfacePart>,
}

#[derive(Clone)]
struct ChunkSurfacePart {
    /// The caller's identity of the triangles, so unchanged ones are reused.
    key: u64,
    /// Oriented, so a point near the surface can be classified inside or out.
    shape: Arc<TriMesh>,
    /// The voxels owning each triangle.
    owners: Vec<VoxelBox>,
}

impl ChunkCollider {
    fn shapes(&self) -> impl Iterator<Item = &dyn Shape> {
        self.cubes.iter().map(|cubes| &**cubes as &dyn Shape).chain(
            self.surface
                .iter()
                .map(|surface| &*surface.shape as &dyn Shape),
        )
    }
}

/// One chunk's reconstructed surface in collision space: world-positioned
/// triangles (vertices may repeat) and the voxels owning each triangle: one
/// voxel, or the box under a merged face.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ChunkSurfaceCollider {
    pub positions: Vec<[f64; 3]>,
    pub triangles: Vec<[u32; 3]>,
    pub owners: Vec<VoxelBox>,
}

/// How far from a reconstructed surface [`CollisionProjection::contains_point`]
/// looks for it, in voxels. Solid voxels deeper than this are covered by the
/// interior cuboids the owner supplies with the surface.
const SURFACE_CONTAINMENT_REACH_VOXELS: f64 = 2.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct WorldAabb {
    min: WorldPos,
    max: WorldPos,
}

impl WorldAabb {
    fn intersects(self, other: Self) -> bool {
        self.min.x <= other.max.x
            && self.max.x >= other.min.x
            && self.min.y <= other.max.y
            && self.max.y >= other.min.y
            && self.min.z <= other.max.z
            && self.max.z >= other.min.z
    }
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
    /// A reconstructed surface has been installed: its triangles may reach up
    /// to a cell past their chunk, so box queries look one cell further.
    has_surfaces: bool,
    /// Chunks whose collider bounds could not be computed. Character queries
    /// scan every chunk while any exist rather than risk missing one.
    unbounded: BTreeSet<ChunkCoord>,
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

/// Per logical capsule query fan-out. Candidate counts are projection entries
/// whose cached conservative bounds intersected the capsule query bounds;
/// narrow-phase counts are the Parry calls actually made. Active obstacles are
/// call-local and currently have no cached projection bounds, so each valid
/// obstacle is both a candidate and a narrow-phase call.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CharacterCollisionQueryStats {
    pub voxel_candidates: u64,
    pub voxel_narrow_phase_queries: u64,
    pub static_mesh_candidates: u64,
    pub static_mesh_narrow_phase_queries: u64,
    pub active_obstacle_candidates: u64,
    pub active_obstacle_narrow_phase_queries: u64,
}

impl CharacterCollisionQueryStats {
    pub fn candidate_count(self) -> u64 {
        self.voxel_candidates
            .saturating_add(self.static_mesh_candidates)
            .saturating_add(self.active_obstacle_candidates)
    }

    pub fn narrow_phase_count(self) -> u64 {
        self.voxel_narrow_phase_queries
            .saturating_add(self.static_mesh_narrow_phase_queries)
            .saturating_add(self.active_obstacle_narrow_phase_queries)
    }

    pub fn saturating_add_assign(&mut self, other: Self) {
        self.voxel_candidates = self.voxel_candidates.saturating_add(other.voxel_candidates);
        self.voxel_narrow_phase_queries = self
            .voxel_narrow_phase_queries
            .saturating_add(other.voxel_narrow_phase_queries);
        self.static_mesh_candidates = self
            .static_mesh_candidates
            .saturating_add(other.static_mesh_candidates);
        self.static_mesh_narrow_phase_queries = self
            .static_mesh_narrow_phase_queries
            .saturating_add(other.static_mesh_narrow_phase_queries);
        self.active_obstacle_candidates = self
            .active_obstacle_candidates
            .saturating_add(other.active_obstacle_candidates);
        self.active_obstacle_narrow_phase_queries = self
            .active_obstacle_narrow_phase_queries
            .saturating_add(other.active_obstacle_narrow_phase_queries);
    }
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
    cast_character_capsule_against_obstacles_with_stats(
        capsule,
        translation,
        contact_skin,
        obstacles,
    )
    .map(|(hit, _)| hit)
}

pub fn cast_character_capsule_against_obstacles_with_stats(
    capsule: CharacterCapsule,
    translation: WorldVec,
    contact_skin: f64,
    obstacles: &[CharacterObstacle],
) -> Result<
    (
        Option<CharacterCapsuleCastHit>,
        CharacterCollisionQueryStats,
    ),
    CharacterCollisionQueryError,
> {
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
    let mut stats = CharacterCollisionQueryStats::default();
    for obstacle in obstacles {
        validate_obstacle(*obstacle)?;
        stats.active_obstacle_candidates = stats.active_obstacle_candidates.saturating_add(1);
        stats.active_obstacle_narrow_phase_queries =
            stats.active_obstacle_narrow_phase_queries.saturating_add(1);
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
    Ok((best, stats))
}

pub fn character_capsule_overlap_obstacles(
    capsule: CharacterCapsule,
    obstacles: &[CharacterObstacle],
) -> Result<Option<CharacterCapsuleOverlap>, CharacterCollisionQueryError> {
    character_capsule_overlap_obstacles_with_stats(capsule, obstacles).map(|(overlap, _)| overlap)
}

pub fn character_capsule_overlap_obstacles_with_stats(
    capsule: CharacterCapsule,
    obstacles: &[CharacterObstacle],
) -> Result<
    (
        Option<CharacterCapsuleOverlap>,
        CharacterCollisionQueryStats,
    ),
    CharacterCollisionQueryError,
> {
    validate_character_query(capsule, WorldVec::ZERO, 0.0)?;
    let capsule_pose = Pose::translation(capsule.center.x, capsule.center.y, capsule.center.z);
    let capsule_shape = Capsule::new_y(capsule.half_height, capsule.radius);
    let mut best = None;
    let mut stats = CharacterCollisionQueryStats::default();
    for obstacle in obstacles {
        validate_obstacle(*obstacle)?;
        stats.active_obstacle_candidates = stats.active_obstacle_candidates.saturating_add(1);
        stats.active_obstacle_narrow_phase_queries =
            stats.active_obstacle_narrow_phase_queries.saturating_add(1);
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
    Ok((best, stats))
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
            has_surfaces: false,
            unbounded: BTreeSet::new(),
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

    /// Return one retained static-mesh instance's authored identity and current
    /// pose. The returned transform is a value snapshot for a call-local
    /// consumer; collision queries continue using the retained shape.
    pub fn static_mesh_instance(
        &self,
        id: StaticMeshInstanceId,
    ) -> Option<(StaticMeshAssetId, u64, StaticMeshTransform)> {
        self.static_meshes.instance_descriptor(id)
    }

    /// Stable identity for retained static-mesh collision topology. Pose is
    /// excluded only for explicitly admitted moving instances; all other
    /// retained instances keep their complete pose in the identity.
    pub fn static_mesh_collision_topology_hash_excluding_pose(
        &self,
        moving_instances: &std::collections::BTreeSet<StaticMeshInstanceId>,
    ) -> u64 {
        self.static_meshes
            .topology_hash_excluding_pose(moving_instances)
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
        self.cast_character_capsule_with_stats(capsule, translation, contact_skin)
            .map(|(hit, _)| hit)
    }

    /// The existing cast result together with internal fan-out facts. This
    /// preserves the source-compatible query while diagnostics can distinguish
    /// logical casts from actual Parry work.
    pub fn cast_character_capsule_with_stats(
        &self,
        capsule: CharacterCapsule,
        translation: WorldVec,
        contact_skin: f64,
    ) -> Result<
        (
            Option<CharacterCapsuleCastHit>,
            CharacterCollisionQueryStats,
        ),
        CharacterCollisionQueryError,
    > {
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
        let query_bounds = swept_capsule_bounds(capsule, translation, contact_skin);
        let mut stats = CharacterCollisionQueryStats::default();
        for (coord, collider) in self.character_candidates(query_bounds) {
            if !character_query_may_intersect(query_bounds, collider.bounds) {
                continue;
            }
            stats.voxel_candidates = stats.voxel_candidates.saturating_add(1);
            for shape in collider.shapes() {
                stats.voxel_narrow_phase_queries =
                    stats.voxel_narrow_phase_queries.saturating_add(1);
                let hit = cast_shapes(
                    &moving_pose,
                    velocity,
                    &moving_shape,
                    &obstacle_pose,
                    Vector::ZERO,
                    shape,
                    options,
                )
                .map_err(|_| CharacterCollisionQueryError::UnsupportedBackendQuery)?;
                keep_nearest_character_hit(
                    &mut best,
                    CharacterCollisionSource::VoxelChunk(*coord),
                    hit,
                );
            }
        }
        for (instance, asset, geometry_hash, shape, bounds) in self.static_meshes.character_shapes()
        {
            if !character_query_may_intersect(query_bounds, bounds) {
                continue;
            }
            stats.static_mesh_candidates = stats.static_mesh_candidates.saturating_add(1);
            stats.static_mesh_narrow_phase_queries =
                stats.static_mesh_narrow_phase_queries.saturating_add(1);
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
        Ok((best, stats))
    }

    /// Return the deepest current capsule overlap, with deterministic source
    /// ordering on equal penetration. Repeated calls after bounded correction
    /// let the character service recover multiple simultaneous overlaps.
    pub fn character_capsule_overlap(
        &self,
        capsule: CharacterCapsule,
    ) -> Result<Option<CharacterCapsuleOverlap>, CharacterCollisionQueryError> {
        self.character_capsule_overlap_with_stats(capsule)
            .map(|(overlap, _)| overlap)
    }

    /// Whether the capsule overlaps any voxel or static-mesh collision, the
    /// same overlap [`Self::character_capsule_overlap`] finds, stopping at the
    /// first rather than measuring the deepest.
    pub fn character_capsule_intersects(
        &self,
        capsule: CharacterCapsule,
    ) -> Result<bool, CharacterCollisionQueryError> {
        validate_character_query(capsule, WorldVec::ZERO, 0.0)?;
        let capsule_pose = Pose::translation(capsule.center.x, capsule.center.y, capsule.center.z);
        let capsule_shape = Capsule::new_y(capsule.half_height, capsule.radius);
        let obstacle_pose = identity();
        let query_bounds = swept_capsule_bounds(capsule, WorldVec::ZERO, 0.0);
        let intersects = |shape: &dyn Shape| {
            intersection_test(&capsule_pose, &capsule_shape, &obstacle_pose, shape)
                .map_err(|_| CharacterCollisionQueryError::UnsupportedBackendQuery)
        };
        for (_, collider) in self.character_candidates(query_bounds) {
            if !character_query_may_intersect(query_bounds, collider.bounds) {
                continue;
            }
            for shape in collider.shapes() {
                if intersects(shape)? {
                    return Ok(true);
                }
            }
        }
        for (_, _, _, shape, bounds) in self.static_meshes.character_shapes() {
            if character_query_may_intersect(query_bounds, bounds) && intersects(shape.as_ref())? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn character_capsule_overlap_with_stats(
        &self,
        capsule: CharacterCapsule,
    ) -> Result<
        (
            Option<CharacterCapsuleOverlap>,
            CharacterCollisionQueryStats,
        ),
        CharacterCollisionQueryError,
    > {
        validate_character_query(capsule, WorldVec::ZERO, 0.0)?;
        let capsule_pose = Pose::translation(capsule.center.x, capsule.center.y, capsule.center.z);
        let capsule_shape = Capsule::new_y(capsule.half_height, capsule.radius);
        let obstacle_pose = identity();
        let mut best = None;
        let query_bounds = swept_capsule_bounds(capsule, WorldVec::ZERO, 0.0);
        let mut stats = CharacterCollisionQueryStats::default();
        for (coord, collider) in self.character_candidates(query_bounds) {
            if !character_query_may_intersect(query_bounds, collider.bounds) {
                continue;
            }
            stats.voxel_candidates = stats.voxel_candidates.saturating_add(1);
            for shape in collider.shapes() {
                stats.voxel_narrow_phase_queries =
                    stats.voxel_narrow_phase_queries.saturating_add(1);
                let result = contact(&capsule_pose, &capsule_shape, &obstacle_pose, shape, 0.0)
                    .map_err(|_| CharacterCollisionQueryError::UnsupportedBackendQuery)?;
                keep_deepest_character_overlap(
                    &mut best,
                    CharacterCollisionSource::VoxelChunk(*coord),
                    result,
                );
            }
        }
        for (instance, asset, geometry_hash, shape, bounds) in self.static_meshes.character_shapes()
        {
            if !character_query_may_intersect(query_bounds, bounds) {
                continue;
            }
            stats.static_mesh_candidates = stats.static_mesh_candidates.saturating_add(1);
            stats.static_mesh_narrow_phase_queries =
                stats.static_mesh_narrow_phase_queries.saturating_add(1);
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
        Ok((best, stats))
    }

    #[cfg(test)]
    fn cast_character_capsule_full_scan_for_test(
        &self,
        capsule: CharacterCapsule,
        translation: WorldVec,
        contact_skin: f64,
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
        let obstacle_pose = identity();
        let mut best = None;
        for (coord, collider) in &self.chunks {
            for shape in collider.shapes() {
                let hit = cast_shapes(
                    &moving_pose,
                    velocity,
                    &moving_shape,
                    &obstacle_pose,
                    Vector::ZERO,
                    shape,
                    options,
                )
                .map_err(|_| CharacterCollisionQueryError::UnsupportedBackendQuery)?;
                keep_nearest_character_hit(
                    &mut best,
                    CharacterCollisionSource::VoxelChunk(*coord),
                    hit,
                );
            }
        }
        for (instance, asset, geometry_hash, shape, _) in self.static_meshes.character_shapes() {
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

    pub fn replace_static_meshes(
        &mut self,
        assets: impl IntoIterator<Item = StaticMeshColliderAsset>,
        instances: impl IntoIterator<Item = StaticMeshColliderInstance>,
    ) -> Result<StaticMeshCollisionReceipt, StaticMeshCollisionError> {
        self.static_meshes.replace_all(assets, instances)
    }

    pub fn apply_static_mesh_residency(
        &mut self,
        assets: impl IntoIterator<Item = StaticMeshColliderAsset>,
        instances: impl IntoIterator<Item = StaticMeshColliderInstance>,
        removed_assets: impl IntoIterator<Item = StaticMeshAssetId>,
        removed_instances: impl IntoIterator<Item = StaticMeshInstanceId>,
    ) -> Result<StaticMeshCollisionReceipt, StaticMeshCollisionError> {
        self.static_meshes
            .apply_residency(assets, instances, removed_assets, removed_instances)
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

    /// Build/replace the collider for one chunk from its current voxels, as
    /// merged boxes. Drops the entry if the chunk has become all-empty.
    fn set_chunk(&mut self, coord: ChunkCoord, chunk: &VoxelChunk) {
        let prepared = self.prepare_chunk(coord, chunk);
        self.install_chunk_parts(coord, chunk.content_hash().0, prepared);
    }

    /// The parts [`Self::set_chunk`] would install for a chunk of cube
    /// voxels, built without changing the projection.
    pub fn prepare_chunk(&self, coord: ChunkCoord, chunk: &VoxelChunk) -> PreparedChunkParts {
        let voxels = solid_voxels(&self.grid, coord, chunk);
        self.prepare_chunk_parts(coord, &voxels, 0, || None)
    }

    /// Rebuild one chunk's collider from `world`. If the chunk is not resident its
    /// collider is dropped. Bumps the version.
    pub fn rebuild_chunk(&mut self, world: &VoxelWorld, coord: ChunkCoord) {
        match world.get(coord) {
            Some(chunk) => self.set_chunk(coord, chunk),
            None => {
                self.drop_collider(coord);
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
                    self.drop_collider(coord);
                }
            }
        }
        self.version += 1;
    }

    /// Replace the colliders of the given chunks with colliders built from the
    /// supplied contents (`None` drops the chunk). Callers pass contents that
    /// differ from resident voxels, such as chunks with noncollidable materials
    /// cleared. Unlisted chunks keep their shapes. One version bump.
    pub fn reconcile_chunks<'a>(
        &mut self,
        chunks: impl IntoIterator<Item = (ChunkCoord, Option<&'a VoxelChunk>)>,
    ) {
        for (coord, chunk) in chunks {
            match chunk {
                Some(chunk) => self.set_chunk(coord, chunk),
                None => {
                    self.drop_collider(coord);
                }
            }
        }
        self.version += 1;
    }

    /// Replace one chunk's collider with explicit parts: boxes merged over
    /// the `cubes` voxels and the reconstructed `surface` drawn for the chunk.
    /// For chunks whose materials are drawn as reconstructed surfaces, so
    /// collision follows what is drawn, the owner supplies cubes only for
    /// cube materials and for solid voxels deep enough that no surface passes
    /// through them. A neighbour's arrival rebuilds a chunk's mesh without
    /// always changing it, so parts equal to the current ones are kept: the
    /// cuboids when the voxel list is unchanged, and the surface when
    /// `surface_key` (the triangles' identity, such as the mesh's content
    /// hash) is unchanged; `surface` is only called otherwise. Does not bump
    /// the version; call [`Self::touch`] after a batch.
    pub fn set_chunk_parts(
        &mut self,
        coord: ChunkCoord,
        source_hash: u64,
        cube_voxels: &[VoxelCoord],
        surface_key: u64,
        surface: impl FnOnce() -> Option<ChunkSurfaceCollider>,
    ) {
        let prepared = self.prepare_chunk_parts(coord, cube_voxels, surface_key, surface);
        self.install_chunk_parts(coord, source_hash, prepared);
    }

    /// The parts [`Self::set_chunk_parts`] would install, built without
    /// changing the projection, so several chunks' can be built at once.
    pub fn prepare_chunk_parts(
        &self,
        coord: ChunkCoord,
        cube_voxels: &[VoxelCoord],
        surface_key: u64,
        surface: impl FnOnce() -> Option<ChunkSurfaceCollider>,
    ) -> PreparedChunkParts {
        let boxes = merge_boxes(&self.grid, coord, cube_voxels);
        let previous = self.chunks.get(&coord);
        let kept_cubes = previous
            .filter(|previous| previous.boxes == boxes)
            .map(|previous| previous.cubes.clone());
        let kept_surface = previous
            .and_then(|previous| previous.surface.as_ref())
            .filter(|previous| previous.key == surface_key)
            .cloned();
        let cubes = kept_cubes.unwrap_or_else(|| {
            (!boxes.is_empty()).then(|| {
                let parts = boxes
                    .iter()
                    .map(|cells| {
                        let (low, _) = self.grid.voxel_bounds_world(cells.min);
                        let (_, high) = self.grid.voxel_bounds_world(cells.max);
                        let center = WorldPos::new(
                            (low.x + high.x) * 0.5,
                            (low.y + high.y) * 0.5,
                            (low.z + high.z) * 0.5,
                        ) + self.world_offset;
                        (
                            Pose::translation(center.x, center.y, center.z),
                            SharedShape::cuboid(
                                (high.x - low.x) * 0.5,
                                (high.y - low.y) * 0.5,
                                (high.z - low.z) * 0.5,
                            ),
                        )
                    })
                    .collect();
                Arc::new(Compound::new(parts))
            })
        });
        let surface = kept_surface.or_else(|| {
            surface()
                .filter(|surface| !surface.triangles.is_empty())
                .and_then(|surface| {
                    let offset = self.world_offset;
                    let vertices = surface
                        .positions
                        .iter()
                        .map(|p| Vector::new(p[0] + offset.x, p[1] + offset.y, p[2] + offset.z))
                        .collect();
                    // Containment reads the nearest triangle's own normal, so the
                    // mesh needs no merged topology or pseudo-normals.
                    let shape = TriMesh::new(vertices, surface.triangles).ok()?;
                    debug_assert_eq!(shape.indices().len(), surface.owners.len());
                    Some(ChunkSurfacePart {
                        key: surface_key,
                        shape: Arc::new(shape),
                        owners: surface.owners,
                    })
                })
        });
        PreparedChunkParts {
            boxes,
            cubes,
            surface,
        }
    }

    /// Install one chunk's parts from [`Self::prepare_chunk_parts`] or
    /// [`Self::prepare_chunk`]. Does not bump the version.
    pub fn install_chunk_parts(
        &mut self,
        coord: ChunkCoord,
        source_hash: u64,
        prepared: PreparedChunkParts,
    ) {
        let PreparedChunkParts {
            boxes,
            cubes,
            surface,
        } = prepared;
        self.has_surfaces |= surface.is_some();
        if cubes.is_none() && surface.is_none() {
            self.drop_collider(coord);
            return;
        }
        let bounds = match (&cubes, &surface) {
            (Some(cubes), Some(surface)) => {
                match (
                    shape_world_aabb(&**cubes),
                    shape_world_aabb(&*surface.shape),
                ) {
                    (Some(left), Some(right)) => Some(WorldAabb {
                        min: WorldPos::new(
                            left.min.x.min(right.min.x),
                            left.min.y.min(right.min.y),
                            left.min.z.min(right.min.z),
                        ),
                        max: WorldPos::new(
                            left.max.x.max(right.max.x),
                            left.max.y.max(right.max.y),
                            left.max.z.max(right.max.z),
                        ),
                    }),
                    _ => None,
                }
            }
            (Some(cubes), None) => shape_world_aabb(&**cubes),
            (None, Some(surface)) => shape_world_aabb(&*surface.shape),
            (None, None) => unreachable!("an empty collider was removed"),
        };
        self.note_bounds(coord, bounds);
        self.chunks.insert(
            coord,
            ChunkCollider {
                source_hash,
                cubes,
                boxes,
                surface,
                bounds,
            },
        );
    }

    /// Drop one chunk's collider. Does not bump the version.
    pub fn remove_chunk(&mut self, coord: ChunkCoord) {
        self.drop_collider(coord);
    }

    fn drop_collider(&mut self, coord: ChunkCoord) {
        self.chunks.remove(&coord);
        self.unbounded.remove(&coord);
    }

    fn note_bounds(&mut self, coord: ChunkCoord, bounds: Option<WorldAabb>) {
        if bounds.is_some() {
            self.unbounded.remove(&coord);
        } else {
            self.unbounded.insert(coord);
        }
    }

    /// The chunk colliders a character query sweeping through `bounds` can
    /// touch, in chunk order: those in the chunks the bounds cover (one cell
    /// wider with reconstructed surfaces, as for box queries), or every chunk
    /// when the bounds or a collider's bounds are unknown.
    fn character_candidates(
        &self,
        bounds: Option<WorldAabb>,
    ) -> Vec<(&ChunkCoord, &ChunkCollider)> {
        // A box spanning more chunk cells than there are colliders is
        // cheaper to answer by scanning them; this also keeps far or unbounded
        // queries out of the chunk arithmetic.
        let chunk_extent = |low: f64, high: f64, cells: u32| {
            (high - low) / (self.grid.voxel_size() * f64::from(cells)) + 3.0
        };
        let dims = self.grid.chunk_dims();
        let compact = |bounds: &WorldAabb| {
            let cells = chunk_extent(bounds.min.x, bounds.max.x, dims.x())
                * chunk_extent(bounds.min.y, bounds.max.y, dims.y())
                * chunk_extent(bounds.min.z, bounds.max.z, dims.z());
            cells.is_finite() && cells < self.chunks.len() as f64
        };
        match bounds {
            Some(bounds) if self.unbounded.is_empty() && compact(&bounds) => {
                // A query touching a chunk boundary meets the voxel faces on
                // both sides of it, so the span reaches one voxel further.
                let voxel = self.grid.voxel_size();
                let margin = WorldVec::new(voxel, voxel, voxel);
                let span = self.chunk_span(bounds.min - margin, bounds.max + margin);
                let mut candidates: Vec<_> = span
                    .iter()
                    .filter_map(|chunk| self.chunks.get_key_value(&chunk))
                    .collect();
                candidates.sort_unstable_by_key(|(chunk, _)| **chunk);
                candidates
            }
            _ => self.chunks.iter().collect(),
        }
    }

    /// Mark a batch of [`Self::set_chunk_parts`] changes with one version bump.
    pub fn touch(&mut self) {
        self.version += 1;
    }

    /// Whether the projection for `chunk` no longer matches `world`'s current data
    /// (content changed, a chunk gained its first solids, or a collider's chunk is
    /// gone). The basis for coordinated, version-checked rebuilds.
    pub fn is_chunk_stale(&self, world: &VoxelWorld, chunk: ChunkCoord) -> bool {
        match (self.chunks.get(&chunk), world.get(chunk)) {
            (Some(c), Some(data)) => c.source_hash != data.content_hash().0,
            // No collider but the chunk now has solids → stale (needs a build).
            (None, Some(data)) => data
                .iter()
                .any(|(_, value)| collision_class(value) == CollisionClass::Solid),
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
        let point = world_to_point(p);
        if let Some(cubes) = self.chunks.get(&chunk).and_then(|c| c.cubes.as_ref()) {
            // Each part is already world-positioned; test against the part transforms.
            if cubes
                .shapes()
                .iter()
                .any(|(pose, shape)| shape.contains_point(pose, point))
            {
                return true;
            }
        }
        self.surface_contains(chunk, point)
    }

    /// Whether `point` lies behind the nearest reconstructed surface
    /// triangle of its chunk and the chunks around it, within reach.
    fn surface_contains(&self, chunk: ChunkCoord, point: Vector) -> bool {
        let reach = self.grid.voxel_size() * SURFACE_CONTAINMENT_REACH_VOXELS;
        let mut nearest: Option<(Real, bool)> = None;
        for dz in -1..=1 {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let candidate = ChunkCoord::new(chunk.x + dx, chunk.y + dy, chunk.z + dz);
                    let Some(surface) =
                        self.chunks.get(&candidate).and_then(|c| c.surface.as_ref())
                    else {
                        continue;
                    };
                    let Some((projection, (triangle, _))) = surface
                        .shape
                        .project_local_point_and_get_location_with_max_dist(point, false, reach)
                    else {
                        continue;
                    };
                    let distance = (projection.point - point).length();
                    if nearest.is_none_or(|(best, _)| distance < best) {
                        let normal = surface.shape.triangle(triangle).scaled_normal();
                        nearest = Some((distance, (point - projection.point).dot(normal) <= 0.0));
                    }
                }
            }
        }
        nearest.is_some_and(|(_, inside)| inside)
    }

    /// Cast a ray against the projection and return the nearest authoritative hit
    /// within `max_distance`, or `None` on a miss. The shared picking/camera/
    /// placement query — there is no separate renderer-owned authoritative raycast.
    ///
    /// Tests the chunks whose colliders meet the ray's segment and keeps the
    /// nearest hit.
    /// The voxel of `cells` just inside what the ray hit: the only one, or
    /// the one under the impact for a box of several.
    fn voxel_at_impact(
        &self,
        cells: VoxelBox,
        ray: Ray,
        dir: WorldVec,
        hit: &parry3d_f64::query::RayIntersection,
    ) -> VoxelCoord {
        if cells.min == cells.max {
            return cells.min;
        }
        let toi = hit.time_of_impact;
        let inward = self.grid.voxel_size() * 1.0e-3;
        let point = WorldPos::new(
            ray.origin.x + dir.x * toi - hit.normal.x * inward,
            ray.origin.y + dir.y * toi - hit.normal.y * inward,
            ray.origin.z + dir.z * toi - hit.normal.z * inward,
        );
        let at = self.grid.world_to_voxel(point - self.world_offset);
        VoxelCoord::new(
            at.x.clamp(cells.min.x, cells.max.x),
            at.y.clamp(cells.min.y, cells.max.y),
            at.z.clamp(cells.min.z, cells.max.z),
        )
    }

    pub fn raycast(&self, ray: Ray, max_distance: f64) -> Option<VoxelHit> {
        let len = ray.dir.length();
        if !len.is_finite() || len <= 0.0 || !max_distance.is_finite() || max_distance <= 0.0 {
            return None;
        }
        let inv = 1.0 / len;
        let dir = WorldVec::new(ray.dir.x * inv, ray.dir.y * inv, ray.dir.z * inv);
        let parry_ray = ParryRay::new(world_to_point(ray.origin), Vector::new(dir.x, dir.y, dir.z));
        let end = WorldPos::new(
            ray.origin.x + dir.x * max_distance,
            ray.origin.y + dir.y * max_distance,
            ray.origin.z + dir.z * max_distance,
        );
        let segment = Some(WorldAabb {
            min: WorldPos::new(
                ray.origin.x.min(end.x),
                ray.origin.y.min(end.y),
                ray.origin.z.min(end.z),
            ),
            max: WorldPos::new(
                ray.origin.x.max(end.x),
                ray.origin.y.max(end.y),
                ray.origin.z.max(end.z),
            ),
        });
        let mut best: Option<(Real, Vector, VoxelCoord)> = None;
        for (_, collider) in self.character_candidates(segment) {
            if !character_query_may_intersect(segment, collider.bounds) {
                continue;
            }
            if let Some((primitive, hit)) = collider.cubes.as_ref().and_then(|cubes| {
                CompositeShapeRef(&**cubes).cast_local_ray_and_get_normal(
                    &parry_ray,
                    max_distance,
                    true,
                )
            }) {
                let Some(&cells) = collider.boxes.get(primitive as usize) else {
                    debug_assert!(false, "compound ray primitive must retain a voxel owner");
                    continue;
                };
                if best.is_none_or(|(t, _, _)| hit.time_of_impact < t) {
                    let voxel = self.voxel_at_impact(cells, ray, dir, &hit);
                    best = Some((hit.time_of_impact, hit.normal, voxel));
                }
            }
            if let Some(surface) = &collider.surface {
                if let Some((triangle, hit)) = CompositeShapeRef(&*surface.shape)
                    .cast_local_ray_and_get_normal(&parry_ray, max_distance, true)
                {
                    let Some(&cells) = surface.owners.get(triangle as usize) else {
                        debug_assert!(false, "surface triangle must retain a voxel owner");
                        continue;
                    };
                    if best.is_none_or(|(t, _, _)| hit.time_of_impact < t) {
                        let voxel = self.voxel_at_impact(cells, ray, dir, &hit);
                        best = Some((hit.time_of_impact, hit.normal, voxel));
                    }
                }
            }
        }

        let (toi, normal, voxel) = best?;
        let point = WorldPos::new(
            ray.origin.x + dir.x * toi,
            ray.origin.y + dir.y * toi,
            ray.origin.z + dir.z * toi,
        );
        Some(VoxelHit {
            voxel,
            chunk: self.grid.voxel_to_chunk(voxel),
            face: normal_to_face(normal),
            point,
            normal: WorldVec::new(normal.x, normal.y, normal.z),
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
        if self.aabb_overlaps_chunks(self.chunk_span(lo, hi), &pose, &cuboid) {
            return true;
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
        self.aabb_overlaps_chunks(self.chunk_span(lo, hi), &pose, &cuboid)
    }

    /// The chunks a box query over `[lo, hi]` tests: those the box covers,
    /// widened by one voxel when reconstructed surfaces are installed, since
    /// a surface's triangles reach up to a cell past their owning chunk.
    fn chunk_span(&self, lo: WorldPos, hi: WorldPos) -> ChunkRegion {
        let mut vmin = self.grid.world_to_voxel(lo - self.world_offset);
        let mut vmax = self.grid.world_to_voxel(hi - self.world_offset);
        if self.has_surfaces {
            vmin = VoxelCoord::new(vmin.x - 1, vmin.y - 1, vmin.z - 1);
            vmax = VoxelCoord::new(vmax.x + 1, vmax.y + 1, vmax.z + 1);
        }
        let last = self.grid.voxel_to_chunk(vmax);
        ChunkRegion::new(
            self.grid.voxel_to_chunk(vmin),
            ChunkCoord::new(last.x + 1, last.y + 1, last.z + 1),
        )
    }

    /// A box overlaps a chunk's solid when it meets a cuboid or surface
    /// triangle, or lies wholly behind a reconstructed surface.
    fn aabb_overlaps_chunks(&self, span: ChunkRegion, pose: &Pose, cuboid: &Cuboid) -> bool {
        let id = identity();
        let mut surfaces = false;
        for chunk in span.iter() {
            if let Some(collider) = self.chunks.get(&chunk) {
                surfaces |= collider.surface.is_some();
                if collider
                    .shapes()
                    .any(|shape| intersection_test(pose, cuboid, &id, shape) == Ok(true))
                {
                    return true;
                }
            }
        }
        surfaces
            && self.contains_point(WorldPos::new(
                pose.translation.x,
                pose.translation.y,
                pose.translation.z,
            ))
    }
}

/// Merge one chunk's voxels into boxes: each grows along X, then whole rows
/// along Y, then whole slabs along Z. Deterministic for a given voxel set.
fn merge_boxes(grid: &VoxelGridSpec, chunk: ChunkCoord, voxels: &[VoxelCoord]) -> Vec<VoxelBox> {
    if voxels.is_empty() {
        return Vec::new();
    }
    let size = grid.chunk_dims().to_array().map(|value| value as usize);
    let origin = grid.chunk_origin_voxel(chunk);
    let index = |x: usize, y: usize, z: usize| (z * size[1] + y) * size[0] + x;
    let mut open = vec![false; size[0] * size[1] * size[2]];
    for voxel in voxels {
        let local = [voxel.x - origin.x, voxel.y - origin.y, voxel.z - origin.z];
        debug_assert!((0..3).all(|axis| (0..size[axis] as i64).contains(&local[axis])));
        open[index(local[0] as usize, local[1] as usize, local[2] as usize)] = true;
    }
    let mut boxes = Vec::new();
    for z in 0..size[2] {
        for y in 0..size[1] {
            for x in 0..size[0] {
                if !open[index(x, y, z)] {
                    continue;
                }
                let mut end_x = x + 1;
                while end_x < size[0] && open[index(end_x, y, z)] {
                    end_x += 1;
                }
                let row =
                    |y: usize, z: usize, open: &[bool]| (x..end_x).all(|x| open[index(x, y, z)]);
                let mut end_y = y + 1;
                while end_y < size[1] && row(end_y, z, &open) {
                    end_y += 1;
                }
                let mut end_z = z + 1;
                while end_z < size[2] && (y..end_y).all(|y| row(y, end_z, &open)) {
                    end_z += 1;
                }
                for zz in z..end_z {
                    for yy in y..end_y {
                        for xx in x..end_x {
                            open[index(xx, yy, zz)] = false;
                        }
                    }
                }
                let at = |x: usize, y: usize, z: usize| {
                    VoxelCoord::new(
                        origin.x + x as i64,
                        origin.y + y as i64,
                        origin.z + z as i64,
                    )
                };
                boxes.push(VoxelBox {
                    min: at(x, y, z),
                    max: at(end_x - 1, end_y - 1, end_z - 1),
                });
            }
        }
    }
    boxes
}

/// Build the parry `Compound` of world-positioned cuboids for one chunk's solid
/// voxels, or `None` if the chunk has no solids.
/// The collidable voxels of one chunk, in storage order.
fn solid_voxels(spec: &VoxelGridSpec, coord: ChunkCoord, chunk: &VoxelChunk) -> Vec<VoxelCoord> {
    chunk
        .iter()
        .filter(|(_, value)| collision_class(*value) == CollisionClass::Solid)
        .map(|(local, _)| spec.chunk_local_to_voxel(coord, local))
        .collect()
}

fn shape_world_aabb(shape: &dyn Shape) -> Option<WorldAabb> {
    let bounds = shape.compute_aabb(&identity());
    let min = WorldPos::new(bounds.mins.x, bounds.mins.y, bounds.mins.z);
    let max = WorldPos::new(bounds.maxs.x, bounds.maxs.y, bounds.maxs.z);
    if [min.x, min.y, min.z, max.x, max.y, max.z]
        .into_iter()
        .all(f64::is_finite)
        && min.x <= max.x
        && min.y <= max.y
        && min.z <= max.z
    {
        Some(WorldAabb { min, max })
    } else {
        None
    }
}

/// A conservative swept bound for the local +Y capsule. Every arithmetic
/// result is checked: a nonfinite/overflowed result intentionally disables
/// pruning for that query rather than excluding a potential collider.
fn swept_capsule_bounds(
    capsule: CharacterCapsule,
    translation: WorldVec,
    contact_skin: f64,
) -> Option<WorldAabb> {
    let end = WorldPos::new(
        capsule.center.x + translation.x,
        capsule.center.y + translation.y,
        capsule.center.z + translation.z,
    );
    let horizontal = capsule.radius + contact_skin;
    let vertical = capsule.half_height + capsule.radius + contact_skin;
    let min = WorldPos::new(
        capsule.center.x.min(end.x) - horizontal,
        capsule.center.y.min(end.y) - vertical,
        capsule.center.z.min(end.z) - horizontal,
    );
    let max = WorldPos::new(
        capsule.center.x.max(end.x) + horizontal,
        capsule.center.y.max(end.y) + vertical,
        capsule.center.z.max(end.z) + horizontal,
    );
    if [
        end.x, end.y, end.z, horizontal, vertical, min.x, min.y, min.z, max.x, max.y, max.z,
    ]
    .into_iter()
    .all(f64::is_finite)
    {
        Some(WorldAabb { min, max })
    } else {
        None
    }
}

fn character_query_may_intersect(query: Option<WorldAabb>, collider: Option<WorldAabb>) -> bool {
    match (query, collider) {
        (Some(query), Some(collider)) => query.intersects(collider),
        // Unknown derived bounds are never allowed to turn into a prune.
        _ => true,
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

    fn world_with_chunks(chunks: &[(ChunkCoord, &[LocalVoxelCoord])]) -> VoxelWorld {
        let mut world = VoxelWorld::new(spec());
        for (coord, solids) in chunks {
            let mut chunk = VoxelChunk::from_spec(&spec());
            for local in *solids {
                chunk.set(*local, VoxelValue::solid_raw(1)).unwrap();
            }
            world.insert(*coord, chunk);
        }
        world.drain_dirty();
        world
    }

    #[test]
    fn dynamics_reuses_immutable_chunk_shapes_across_snapshots() {
        let coord = ChunkCoord::new(0, 0, 0);
        let mut world = world_with(coord, &[LocalVoxelCoord::new(0, 0, 0)]);
        let mut projection = CollisionProjection::build(&world);
        let snapshot = projection.clone();
        assert!(Arc::ptr_eq(
            projection.chunks[&coord].cubes.as_ref().unwrap(),
            snapshot.chunks[&coord].cubes.as_ref().unwrap()
        ));
        // Replacing a chunk must leave the prior snapshot intact.
        world
            .get_mut(coord)
            .unwrap()
            .set(LocalVoxelCoord::new(1, 0, 0), VoxelValue::solid_raw(1))
            .unwrap();
        projection.rebuild_chunk(&world, coord);
        assert!(!Arc::ptr_eq(
            projection.chunks[&coord].cubes.as_ref().unwrap(),
            snapshot.chunks[&coord].cubes.as_ref().unwrap()
        ));
        assert_eq!(
            snapshot.chunks[&coord]
                .cubes
                .as_ref()
                .unwrap()
                .shapes()
                .len(),
            1
        );
        // The two neighbouring voxels merge into one box.
        assert_eq!(
            projection.chunks[&coord].boxes,
            [VoxelBox {
                min: VoxelCoord::new(0, 0, 0),
                max: VoxelCoord::new(1, 0, 0),
            }]
        );
    }

    #[test]
    fn merged_part_boxes_name_the_voxel_under_a_ray_and_surfaces_classify_points() {
        let coord = ChunkCoord::new(0, 0, 0);
        let mut projection = CollisionProjection::build(&VoxelWorld::new(spec()));
        // A 4 x 2 x 3 slab of cuboid voxels, and one surface triangle above it.
        let cubes: Vec<_> = (0..4)
            .flat_map(|x| (0..2).flat_map(move |y| (0..3).map(move |z| VoxelCoord::new(x, y, z))))
            .collect();
        let surface = ChunkSurfaceCollider {
            positions: vec![[0.0, 3.0, 0.0], [0.0, 3.0, 8.0], [8.0, 3.0, 0.0]],
            triangles: vec![[0, 1, 2]],
            owners: vec![VoxelBox::single(VoxelCoord::new(1, 2, 1))],
        };
        projection.set_chunk_parts(coord, 1, &cubes, 7, || Some(surface.clone()));
        projection.touch();
        assert_eq!(projection.chunks[&coord].boxes.len(), 1);
        let side = projection
            .raycast(
                Ray::new(WorldPos::new(-2.0, 1.5, 2.5), WorldVec::new(1.0, 0.0, 0.0)),
                10.0,
            )
            .unwrap();
        assert_eq!(side.voxel, VoxelCoord::new(0, 1, 2));
        assert_eq!(side.face, Face::NegX);
        let top = projection
            .raycast(
                Ray::new(WorldPos::new(1.5, 2.5, 1.5), WorldVec::new(0.0, -1.0, 0.0)),
                10.0,
            )
            .unwrap();
        assert_eq!(top.voxel, VoxelCoord::new(1, 1, 1));
        let surface_hit = projection
            .raycast(
                Ray::new(WorldPos::new(1.0, 6.0, 1.0), WorldVec::new(0.0, -1.0, 0.0)),
                10.0,
            )
            .unwrap();
        assert_eq!(surface_hit.voxel, VoxelCoord::new(1, 2, 1));
        assert!((surface_hit.normal.y - 1.0).abs() < 1.0e-9);
        // Below the triangle reads inside, above it outside.
        assert!(projection.contains_point(WorldPos::new(1.0, 2.6, 1.0)));
        assert!(!projection.contains_point(WorldPos::new(1.0, 3.4, 1.0)));
        // Unchanged parts are kept rather than rebuilt.
        let kept = projection.chunks[&coord]
            .surface
            .as_ref()
            .unwrap()
            .shape
            .clone();
        projection.set_chunk_parts(coord, 2, &cubes, 7, || panic!("the surface is unchanged"));
        assert!(Arc::ptr_eq(
            &kept,
            &projection.chunks[&coord].surface.as_ref().unwrap().shape
        ));
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
    fn character_capsule_pruning_matches_full_scan_for_ground_wall_no_hit_and_zero_motion() {
        let world = world_with_chunks(&[
            (ChunkCoord::new(0, 0, 0), &[LocalVoxelCoord::new(2, 0, 2)]),
            (ChunkCoord::new(5, 0, 0), &[LocalVoxelCoord::new(0, 0, 0)]),
            (ChunkCoord::new(-5, 0, 0), &[LocalVoxelCoord::new(0, 0, 0)]),
        ]);
        let projection = CollisionProjection::build(&world);
        let cases = [
            // Ground probe.
            (
                CharacterCapsule {
                    center: WorldPos::new(2.5, 3.0, 2.5),
                    half_height: 0.5,
                    radius: 0.4,
                },
                WorldVec::new(0.0, -3.0, 0.0),
                0.02,
            ),
            // Wall sweep.
            (
                CharacterCapsule {
                    center: WorldPos::new(0.5, 1.0, 2.5),
                    half_height: 0.5,
                    radius: 0.4,
                },
                WorldVec::new(3.0, 0.0, 0.0),
                0.02,
            ),
            // No hit through the gap between resident chunk bounds.
            (
                CharacterCapsule {
                    center: WorldPos::new(20.5, 1.0, 20.5),
                    half_height: 0.5,
                    radius: 0.4,
                },
                WorldVec::new(1.0, 0.0, 0.0),
                0.02,
            ),
            // Zero motion retains inclusive current-capsule candidates.
            (
                CharacterCapsule {
                    center: WorldPos::new(2.5, 1.0, 2.5),
                    half_height: 0.5,
                    radius: 0.4,
                },
                WorldVec::ZERO,
                0.0,
            ),
        ];
        for (capsule, translation, skin) in cases {
            let (pruned, stats) = projection
                .cast_character_capsule_with_stats(capsule, translation, skin)
                .unwrap();
            let full = projection
                .cast_character_capsule_full_scan_for_test(capsule, translation, skin)
                .unwrap();
            assert_eq!(pruned, full);
            assert!(stats.voxel_candidates <= 3);
            assert_eq!(stats.voxel_candidates, stats.voxel_narrow_phase_queries);
        }
    }

    #[test]
    fn capsule_pruning_keeps_coincident_voxel_static_order_and_inclusive_touching_candidates() {
        let world = world_with_chunks(&[
            (ChunkCoord::new(0, 0, 0), &[LocalVoxelCoord::new(0, 0, 2)]),
            (ChunkCoord::new(6, 0, 0), &[LocalVoxelCoord::new(0, 0, 0)]),
        ]);
        let mut projection = CollisionProjection::build(&world);
        let asset = StaticMeshColliderAsset::new(
            StaticMeshAssetId(41),
            vec![[-1.0, -1.0, 2.0], [2.0, -1.0, 2.0], [0.5, 2.0, 2.0]],
            vec![[0, 1, 2]],
        )
        .unwrap();
        projection
            .replace_static_meshes(
                [asset],
                [StaticMeshColliderInstance {
                    id: StaticMeshInstanceId(41),
                    asset: StaticMeshAssetId(41),
                    transform: StaticMeshTransform::IDENTITY,
                }],
            )
            .unwrap();
        let capsule = CharacterCapsule {
            center: WorldPos::new(0.5, 0.5, 0.0),
            half_height: 0.0,
            radius: 0.25,
        };
        let (pruned, stats) = projection
            .cast_character_capsule_with_stats(capsule, WorldVec::new(0.0, 0.0, 3.0), 0.0)
            .unwrap();
        let full = projection
            .cast_character_capsule_full_scan_for_test(capsule, WorldVec::new(0.0, 0.0, 3.0), 0.0)
            .unwrap();
        assert_eq!(pruned, full);
        assert!(pruned.is_some());
        assert_eq!(stats.voxel_candidates, 1);
        assert_eq!(stats.static_mesh_candidates, 1);
        assert_eq!(stats.narrow_phase_count(), 2);
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
    fn character_query_stats_saturate_without_wrapping() {
        let mut stats = CharacterCollisionQueryStats {
            voxel_candidates: u64::MAX,
            voxel_narrow_phase_queries: u64::MAX,
            static_mesh_candidates: u64::MAX,
            static_mesh_narrow_phase_queries: u64::MAX,
            active_obstacle_candidates: u64::MAX,
            active_obstacle_narrow_phase_queries: u64::MAX,
        };
        stats.saturating_add_assign(CharacterCollisionQueryStats {
            voxel_candidates: 1,
            voxel_narrow_phase_queries: 1,
            static_mesh_candidates: 1,
            static_mesh_narrow_phase_queries: 1,
            active_obstacle_candidates: 1,
            active_obstacle_narrow_phase_queries: 1,
        });
        assert_eq!(stats.candidate_count(), u64::MAX);
        assert_eq!(stats.narrow_phase_count(), u64::MAX);
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
    fn a_boundary_hit_survives_distant_residency() {
        // Enough distant chunks switch the ray to its compact chunk lookup,
        // which must still include the chunk whose face the ray touches.
        let mut world = world_with(ChunkCoord::new(0, 0, 0), &[LocalVoxelCoord::new(7, 3, 6)]);
        let ray = Ray::new(WorldPos::new(8.0, 5.0, 7.0), WorldVec::new(0.0, -1.0, 0.0));
        let before = CollisionProjection::build(&world).raycast(ray, 10.0);
        assert!(before.is_some());
        for i in 10..110 {
            let mut chunk = VoxelChunk::from_spec(&spec());
            chunk
                .set(LocalVoxelCoord::new(0, 0, 0), VoxelValue::solid_raw(1))
                .unwrap();
            world.insert(ChunkCoord::new(i, 0, 0), chunk);
        }
        let after = CollisionProjection::build(&world).raycast(ray, 10.0);
        assert_eq!(after, before);
    }

    #[test]
    fn raycast_keeps_the_owning_voxel_at_a_sparse_top_face_boundary() {
        // This is the streamed-residency failure shape: the ray enters the top
        // face of one sparse voxel exactly where two empty neighbouring cells
        // meet it. Deriving a coordinate from that boundary point would name
        // (8,3,7), which is not solid; the hit must retain its Compound child.
        let world = world_with(ChunkCoord::new(0, 0, 0), &[LocalVoxelCoord::new(7, 3, 6)]);
        let projection = CollisionProjection::build(&world);
        let hit = projection
            .raycast(
                Ray::new(WorldPos::new(8.0, 5.0, 7.0), WorldVec::new(0.0, -1.0, 0.0)),
                10.0,
            )
            .expect("ray should retain the sparse voxel hit");

        assert_eq!(hit.voxel, VoxelCoord::new(7, 3, 6));
        assert_eq!(hit.face, Face::PosY);
        assert!(projection.contains_point(spec().voxel_center_world(hit.voxel)));
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
        projection
            .replace_static_meshes(
                [asset],
                [StaticMeshColliderInstance {
                    id: StaticMeshInstanceId(9),
                    asset: StaticMeshAssetId(3),
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
