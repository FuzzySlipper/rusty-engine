//! Conventional spatial services over object-centric entity state.
//!
//! The voxel authority and Parry collision projection expose a typed query
//! vocabulary to a small, centrally scheduled motion system. Gameplay objects
//! remain ordinary entity views with data components; collision internals
//! never become the runtime spine and components do not acquire scattered
//! update hooks.

#![forbid(unsafe_code)]

mod active_collision;
mod character_modes;
pub use character_modes::{CharacterMovementFact, CharacterMovementMode, CharacterMovementRequest};
mod character_controller;
mod character_tether;
pub use character_tether::{CharacterTetherFact, CharacterTetherRequest};
mod entity_motion;
mod occlusion;
mod perception;
mod physics;
mod rigid_body;
mod trigger;
mod trigger_codec;
mod trigger_geometry;
mod voxel_edit;
mod voxel_picking;
mod voxel_primitive;
mod voxel_residency;
mod voxel_template;
mod world_origin;

pub use character_controller::{
    character_edge_is_traversable, CharacterAirConfig, CharacterBlockKind, CharacterConfigError,
    CharacterContactFact, CharacterContactKind, CharacterControllerCommand,
    CharacterControllerConfig, CharacterControllerError, CharacterControllerReadout,
    CharacterControllerReceipt, CharacterControllerService, CharacterExternalMotionConfig,
    CharacterGroundConfig, CharacterGroundFact, CharacterJumpConfig, CharacterMeshInstance,
    CharacterPlatformConfig, CharacterPlatformFact, CharacterRecoveryConfig, CharacterShapeConfig,
    CharacterSolverConfig, CharacterStanceFact, CharacterStepColliders, CharacterStepFact,
    CharacterStepSubject, CharacterStepWorld, CharacterSurfaceConfig, CharacterVerticalConfig,
    DynamicImpulseProposal, FirstPersonLookCommand, FirstPersonLookConfig,
    FirstPersonLookDiagnostic, FirstPersonLookError, FirstPersonLookReceipt,
    FirstPersonLookService, FirstPersonLookState, PreparedCharacterControllerStep,
};
pub use core_space::{GlobalPosition, WorldOrigin};
pub use entity_motion::{
    EntityMotionCommand, EntityMotionError, EntityMotionOutcome, EntityMotionReceipt,
    EntityMotionResolution, EntityMotionService, FirstPersonBasis, FirstPersonMotionCommand,
    FirstPersonMotionError, FirstPersonMotionInput, FirstPersonMotionReadout,
    FirstPersonMotionReceipt, FirstPersonMotionService, FirstPersonPose, MotionSpatialEntity,
};
pub use occlusion::{
    SpatialOcclusionCollider, SpatialOcclusionError, SpatialOcclusionHit,
    SpatialOcclusionHitboxOverride, SpatialOcclusionQuery, SpatialOcclusionService,
};
pub use perception::{
    SpatialPerceptionAggregate, SpatialPerceptionError, SpatialPerceptionObserver,
    SpatialPerceptionPage, SpatialPerceptionPair, SpatialPerceptionPairKind,
    SpatialPerceptionQuery, SpatialPerceptionReadout, SpatialPerceptionService,
    SpatialPerceptionTarget,
};
pub use physics::{
    integrate_kinematic, integrate_kinematic_with_query, CollisionMode, CollisionResolution,
    IntegrationResult, KinematicBody, KinematicCollisionQuery, KinematicShape, PhysicsError,
    PhysicsStep, PhysicsWorld,
};
pub use rigid_body::{
    cuboid_mass_properties, rigid_body_component_mass_properties, rigid_body_mass_properties,
    RigidBodyMassProperties,
};
pub use svc_collision::{
    DynamicsAction, DynamicsAnchorObservation, DynamicsBodyId, DynamicsBodyInput,
    DynamicsBodyOutput, DynamicsContact, DynamicsEnvironmentReceipt, DynamicsError,
    DynamicsMassProperties, DynamicsRopeSolverConfig, DynamicsShape, DynamicsSolver,
    DynamicsStepReceipt, DynamicsTether, DynamicsTetherEndpoint, DynamicsTetherError,
    DynamicsTetherReadout,
};
pub use trigger::{
    KinematicTriggerDefinition, TriggerGeometrySource, TriggerLifecycleReceipt, TriggerOverlapFact,
    TriggerOverlapFactKind, TriggerOverlapPage, TriggerOverlapPair, TriggerOverlapReadout,
    TriggerReconcileCause, TriggerReconcileReceipt, TriggerRestoreReceipt, TriggerVolumeDiagnostic,
    TriggerVolumeDiagnosticCode, TriggerVolumeError, TriggerVolumeSystem,
    TRIGGER_VOLUME_SNAPSHOT_SCHEMA_VERSION,
};
pub use trigger_codec::{decode_trigger_snapshot, encode_trigger_snapshot, TriggerVolumeSnapshot};

pub use svc_collision::{
    cast_character_capsule_against_obstacles, character_capsule_overlap_obstacles,
    CharacterCapsule, CharacterCapsuleCastHit, CharacterCapsuleOverlap,
    CharacterCollisionQueryError, CharacterCollisionQueryStats, CharacterCollisionSource,
    CharacterObstacle, StaticMeshAssetId, StaticMeshColliderAsset, StaticMeshColliderInstance,
    StaticMeshCollisionError, StaticMeshCollisionReceipt, StaticMeshHit, StaticMeshInstanceId,
    StaticMeshTransform,
};
pub use svc_mesh::{SurfaceMeshLimits, SurfaceMeshOptions, SurfaceMode};
pub use voxel_edit::{
    validate_material_voxel, validate_voxel_address, validate_voxel_material_slot,
    VoxelAuthorityValidationError, VoxelEdit, VoxelEditApplyError, VoxelEditFact, VoxelEditReceipt,
    VoxelEditRejection, VoxelEditService, VoxelProjectionRevisions, VoxelSourceRevision,
    MAX_VOXEL_COORDINATE_ABS, MAX_VOXEL_MATERIAL_SLOT,
};
pub use voxel_picking::{
    InstanceVoxelPickAnchor, VoxelPickAnchor, VoxelPickError, VoxelPickHint, VoxelPickService,
};
pub use voxel_primitive::{
    VoxelBoxFill, VoxelPrimitive, VoxelPrimitiveEditService, VoxelPrimitiveError,
    VoxelPrimitiveMaterial, VoxelPrimitiveRequest,
};
pub use voxel_residency::{
    ResidentVoxelChunk, VoxelChunkContentHash, VoxelChunkIdentity, VoxelChunkPayload,
    VoxelChunkResidencyApplyError, VoxelChunkResidencyOperation, VoxelChunkResidencyReceipt,
    VoxelChunkResidencyRejection, VoxelChunkResidencyService,
};
pub use voxel_template::{
    VoxelTemplate, VoxelTemplateEditService, VoxelTemplateError, VoxelTemplateRequest,
    VOXEL_HOUSE_TEMPLATE_BOUNDS,
};
pub use world_origin::{
    decode_world_origin_state, encode_world_origin_state, PreparedWorldOriginRebase,
    PreparedWorldOriginSpatialRebase, WorldOriginAffectedTransform, WorldOriginEntity,
    WorldOriginReadout, WorldOriginRebaseError, WorldOriginRebaseReceipt, WorldOriginRebaseRequest,
    WorldOriginRebaseService, WorldOriginSpatialRebaseReceipt, WorldOriginState,
    DEFAULT_LOCAL_COORDINATE_ENVELOPE, WORLD_ORIGIN_SNAPSHOT_SCHEMA_VERSION,
};

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use core_ids::EntityId;
use core_math::Vec3;
use core_space::{
    ChunkCoord, ChunkDims, Face, GridId, LocalVoxelCoord, VoxelCoord, VoxelGridSpec, WorldPos,
    WorldVec,
};
use core_voxel::{VoxelMaterialId, VoxelValue};
use entity_state::{
    BatchRejection, EntityCommand, EntityCommandBatch, EntityFact, EntityState, KinematicBodyView,
};
use svc_collision::{CollisionHit, CollisionProjection, Ray};
use svc_mesh::{mesh_chunk_in_world_with_options, MeshError};
use svc_pathfinding::{
    propose_direct_nav_movement, propose_projected_direct_nav_movement, DirectNavMovementRequest,
    NavError, NavProjection, NavProjectionConfig, ProjectedDirectNavMovementError,
    ProjectedDirectNavMovementRequest,
};
use svc_spatial::VoxelWorld;
use svc_volume::{VolumeError, VoxelChunk};

/// Upper bound for one scheduled motion phase. The caller controls cadence, but
/// a single accidental multi-second step cannot become an unbounded entity-state edit.
pub const MAX_MOTION_DELTA_SECONDS: f32 = 1.0;
pub const MAX_CHUNK_SIZE: u32 = 64;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct MaterialVoxel {
    #[serde(default)]
    pub state: u16,
    pub address: [i64; 3],
    pub material_slot: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoxelMeshGroup {
    pub state: u16,
    pub material_slot: u16,
    pub direction: Option<core_space::Direction6>,
    pub start: u32,
    pub count: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VoxelMeshChunk {
    pub chunk: [i64; 3],
    /// Hash of the complete derived mesh payload, including the selected surface
    /// mode and neighbor-dependent seam geometry.
    pub content_hash: u64,
    /// Hash of the canonical resident chunk. This is diagnostic provenance, not
    /// the retained-render replacement key.
    pub source_chunk_hash: u64,
    pub surface_mode: SurfaceMode,
    pub translation: [f32; 3],
    pub positions: Vec<f32>,
    pub normals: Vec<f32>,
    pub tile_coordinates: Vec<f32>,
    pub indices: Vec<u32>,
    pub groups: Vec<VoxelMeshGroup>,
    pub bounds_min: [f32; 3],
    pub bounds_max: [f32; 3],
    pub vertices: u32,
    pub quads: u32,
    pub faces_culled: u32,
}

/// Exact chunk-mesh publication performed at one accepted voxel revision.
///
/// Dirty coordinates include removed chunks so retained projection can destroy
/// their stable handles. Rebuilt and reused counts describe the candidate that
/// was published; all coordinates are deterministic signed world chunk IDs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoxelChunkMeshUpdate {
    pub source_revision: VoxelSourceRevision,
    pub surface_mode: SurfaceMode,
    pub dirty_chunks: Vec<[i64; 3]>,
    pub rebuilt_chunks: usize,
    pub reused_chunks: usize,
    pub removed_chunks: usize,
}

/// Static collision authority plus its query-optimized derived projection.
///
/// Keeping both layers together preserves the important invariant that the
/// Parry representation accelerates queries but never becomes canonical state.
#[derive(Clone)]
pub struct VoxelCollisionScene {
    voxel_world: VoxelWorld,
    projection: CollisionProjection,
    navigation: NavProjection,
    voxel_size: f64,
    chunk_size: u32,
    solid_voxel_count: usize,
    noncollidable_materials: BTreeSet<u16>,
    /// Shared so an unchanged chunk's mesh is never copied.
    mesh_chunks: BTreeMap<ChunkCoord, Arc<VoxelMeshChunk>>,
    mesh_options: SurfaceMeshOptions,
    mesh_update: VoxelChunkMeshUpdate,
    /// Process-unique identity of this scene's chunk-mesh history. A build
    /// (including a world-origin rebase) starts a new lineage; local changes
    /// and clones continue it.
    mesh_lineage: u64,
    source_revision: VoxelSourceRevision,
    /// Order-independent sum of every solid voxel's hash, maintained by each
    /// local change.
    authority_hash: u64,
    world_origin: WorldOrigin,
    rebase_revision: u64,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SceneBuildRevision {
    pub source: VoxelSourceRevision,
    pub world_origin: WorldOrigin,
    pub rebase: u64,
}

/// Navigation walks any empty cell a one-voxel agent occupies; floors are
/// not required.
const SCENE_NAVIGATION: NavProjectionConfig = NavProjectionConfig {
    agent_height_voxels: 1,
    require_solid_floor: false,
};

impl SceneBuildRevision {
    const fn initial(source: VoxelSourceRevision) -> Self {
        Self {
            source,
            world_origin: WorldOrigin::ZERO,
            rebase: 0,
        }
    }
}

fn next_mesh_lineage() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

impl std::fmt::Debug for VoxelCollisionScene {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("VoxelCollisionScene")
            .field("voxel_size", &self.voxel_size)
            .field("chunk_size", &self.chunk_size)
            .field("solid_voxel_count", &self.solid_voxel_count)
            .field("mesh_chunk_count", &self.mesh_chunks.len())
            .field("source_revision", &self.source_revision)
            .field("authority_hash", &self.authority_hash)
            .field("world_origin", &self.world_origin)
            .field("rebase_revision", &self.rebase_revision)
            .field(
                "resident_chunk_count",
                &self.voxel_world.resident_chunks().count(),
            )
            .field("projection_version", &self.projection.version())
            .field("navigation_cell_count", &self.navigation.walkable_len())
            .field("navigation_hash", &self.navigation.projection_hash())
            .finish()
    }
}

#[derive(Debug)]
pub enum CollisionSceneError {
    InvalidVoxelSize,
    InvalidChunkSize,
    Volume {
        voxel: [i64; 3],
        source: VolumeError,
    },
    ConflictingVoxelState {
        voxel: [i64; 3],
        first: u16,
        second: u16,
    },
    ConflictingVoxelMaterial {
        voxel: [i64; 3],
        first: u16,
        second: u16,
    },
    InvalidMaterialVoxel(VoxelAuthorityValidationError),
    Mesh(MeshError),
    NavigationProjection(NavError),
    StaticMeshRebase(StaticMeshCollisionError),
}

impl std::fmt::Display for CollisionSceneError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for CollisionSceneError {}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CollisionRayHit {
    pub voxel: [i64; 3],
    pub face: Face,
    pub point: [f64; 3],
    pub distance: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpatialCollisionHit {
    Voxel(CollisionRayHit),
    StaticMesh(StaticMeshHit),
}

/// One bounded path-following proposal derived from the scene's canonical
/// voxel authority. Applying it remains the caller's responsibility.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NavigationStep {
    pub next_waypoint: Vec3,
    pub reached: bool,
    pub visited: usize,
    pub path_len: usize,
    pub projection_hash: u64,
    pub path_hash: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationStepError {
    InvalidRequest { reason: &'static str },
    StartNotWalkable { start: [i64; 3] },
    GoalNotWalkable { goal: [i64; 3] },
    NoPath { start: [i64; 3], goal: [i64; 3] },
}

impl VoxelCollisionScene {
    /// Build a scene from canonical integer voxel addresses. Input order and
    /// duplicate addresses do not affect the resulting projection.
    pub fn from_solid_voxels(
        voxel_size: f64,
        chunk_size: u32,
        solids: impl IntoIterator<Item = [i64; 3]>,
    ) -> Result<Self, CollisionSceneError> {
        Self::build(
            voxel_size,
            chunk_size,
            solids.into_iter().map(|address| MaterialVoxel {
                state: 0,
                address,
                material_slot: 1,
            }),
            SurfaceMeshOptions::default(),
        )
    }

    pub fn from_solid_voxels_with_mesh_options(
        voxel_size: f64,
        chunk_size: u32,
        solids: impl IntoIterator<Item = [i64; 3]>,
        mesh_options: SurfaceMeshOptions,
    ) -> Result<Self, CollisionSceneError> {
        Self::build(
            voxel_size,
            chunk_size,
            solids.into_iter().map(|address| MaterialVoxel {
                state: 0,
                address,
                material_slot: 1,
            }),
            mesh_options,
        )
    }

    pub fn from_material_voxels(
        voxel_size: f64,
        chunk_size: u32,
        voxels: impl IntoIterator<Item = MaterialVoxel>,
    ) -> Result<Self, CollisionSceneError> {
        Self::build(
            voxel_size,
            chunk_size,
            voxels,
            SurfaceMeshOptions::default(),
        )
    }

    pub fn from_material_voxels_with_mesh_options(
        voxel_size: f64,
        chunk_size: u32,
        voxels: impl IntoIterator<Item = MaterialVoxel>,
        mesh_options: SurfaceMeshOptions,
    ) -> Result<Self, CollisionSceneError> {
        Self::build(voxel_size, chunk_size, voxels, mesh_options)
    }

    /// Rebuild concrete persisted authority at its accepted live revision.
    /// Authored projects normally start at revision zero; runtime snapshots use
    /// this constructor to retain their optimistic-concurrency boundary.
    pub fn from_material_voxels_at_revision(
        voxel_size: f64,
        chunk_size: u32,
        voxels: impl IntoIterator<Item = MaterialVoxel>,
        source_revision: VoxelSourceRevision,
    ) -> Result<Self, CollisionSceneError> {
        Self::build_at_revision(
            voxel_size,
            chunk_size,
            voxels,
            source_revision,
            SurfaceMeshOptions::default(),
        )
    }

    fn build(
        voxel_size: f64,
        chunk_size: u32,
        voxels: impl IntoIterator<Item = MaterialVoxel>,
        mesh_options: SurfaceMeshOptions,
    ) -> Result<Self, CollisionSceneError> {
        Self::build_at_revision(
            voxel_size,
            chunk_size,
            voxels,
            VoxelSourceRevision::INITIAL,
            mesh_options,
        )
    }

    fn build_at_revision(
        voxel_size: f64,
        chunk_size: u32,
        voxels: impl IntoIterator<Item = MaterialVoxel>,
        source_revision: VoxelSourceRevision,
        mesh_options: SurfaceMeshOptions,
    ) -> Result<Self, CollisionSceneError> {
        let voxel_world = Self::build_voxel_world(voxel_size, chunk_size, voxels)?;
        Self::build_from_voxel_world_at_revision(
            voxel_size,
            chunk_size,
            voxel_world,
            SceneBuildRevision::initial(source_revision),
            mesh_options,
        )
    }

    fn build_voxel_world(
        voxel_size: f64,
        chunk_size: u32,
        voxels: impl IntoIterator<Item = MaterialVoxel>,
    ) -> Result<VoxelWorld, CollisionSceneError> {
        if !(1..=MAX_CHUNK_SIZE).contains(&chunk_size) {
            return Err(CollisionSceneError::InvalidChunkSize);
        }
        let dimensions = ChunkDims::cubic(chunk_size).expect("validated non-zero chunk size");
        let grid = VoxelGridSpec::new(GridId::new(0), voxel_size, dimensions)
            .ok_or(CollisionSceneError::InvalidVoxelSize)?;
        let mut unique_voxels = BTreeMap::new();
        for voxel in voxels {
            validate_material_voxel(voxel).map_err(CollisionSceneError::InvalidMaterialVoxel)?;
            if let Some(first) =
                unique_voxels.insert(voxel.address, (voxel.material_slot, voxel.state))
            {
                if first.0 != voxel.material_slot {
                    return Err(CollisionSceneError::ConflictingVoxelMaterial {
                        voxel: voxel.address,
                        first: first.0,
                        second: voxel.material_slot,
                    });
                }
                if first.1 != voxel.state {
                    return Err(CollisionSceneError::ConflictingVoxelState {
                        voxel: voxel.address,
                        first: first.1,
                        second: voxel.state,
                    });
                }
            }
        }
        let material_voxels: Vec<_> = unique_voxels
            .into_iter()
            .map(|(address, (material_slot, state))| MaterialVoxel {
                state,
                address,
                material_slot,
            })
            .collect();
        let mut chunks = BTreeMap::new();

        for material_voxel in &material_voxels {
            let address = material_voxel.address;
            let voxel = VoxelCoord::new(address[0], address[1], address[2]);
            let (chunk_coord, local) = grid.voxel_to_chunk_local(voxel);
            let chunk = chunks
                .entry(chunk_coord)
                .or_insert_with(|| VoxelChunk::from_spec(&grid));
            chunk
                .set(
                    local,
                    VoxelValue::solid(VoxelMaterialId::new(material_voxel.material_slot))
                        .with_state(
                            core_voxel::VoxelState::from_raw(material_voxel.state)
                                .expect("validated state"),
                        ),
                )
                .map_err(|source| CollisionSceneError::Volume {
                    voxel: address,
                    source,
                })?;
        }

        let mut voxel_world = VoxelWorld::new(grid);
        for (coord, chunk) in chunks {
            voxel_world.insert(coord, chunk);
        }
        Ok(voxel_world)
    }

    /// Build every derived projection from `voxel_world`. Used for fresh
    /// scenes and world-origin rebases; local edits and residency changes
    /// update only what they touch.
    pub(crate) fn build_from_voxel_world_at_revision(
        voxel_size: f64,
        chunk_size: u32,
        voxel_world: VoxelWorld,
        revisions: SceneBuildRevision,
        mesh_options: SurfaceMeshOptions,
    ) -> Result<Self, CollisionSceneError> {
        let grid = voxel_world.grid();
        debug_assert_eq!(grid.voxel_size(), voxel_size);
        debug_assert_eq!(grid.chunk_dims().to_array(), [chunk_size; 3]);
        let mut solid_voxel_count = 0usize;
        let mut authority_hash = 0u64;
        for (coordinate, chunk) in voxel_world.resident_chunks() {
            for (local, value) in chunk.iter() {
                let Some(voxel) = material_voxel_at(grid, coordinate, local, value) else {
                    continue;
                };
                validate_material_voxel(voxel)
                    .map_err(CollisionSceneError::InvalidMaterialVoxel)?;
                solid_voxel_count += 1;
                authority_hash = authority_hash.wrapping_add(voxel_hash(voxel));
            }
        }
        let mesh_chunks = build_mesh_chunks(&voxel_world, mesh_options)?;
        let dirty_chunks = voxel_world
            .resident_chunks()
            .map(|(coordinate, _)| coordinate.to_array())
            .collect();
        let rebuilt_chunks = mesh_chunks.len();
        let mut scene = Self {
            projection: CollisionProjection::build(&voxel_world),
            navigation: NavProjection::from_walkable_cells(grid, []),
            voxel_world,
            voxel_size,
            chunk_size,
            solid_voxel_count,
            noncollidable_materials: BTreeSet::new(),
            mesh_chunks,
            mesh_options,
            mesh_update: VoxelChunkMeshUpdate {
                source_revision: revisions.source,
                surface_mode: mesh_options.mode,
                dirty_chunks,
                rebuilt_chunks,
                reused_chunks: 0,
                removed_chunks: 0,
            },
            mesh_lineage: next_mesh_lineage(),
            source_revision: revisions.source,
            authority_hash,
            world_origin: revisions.world_origin,
            rebase_revision: revisions.rebase,
        };
        scene.rebuild_navigation();
        Ok(scene)
    }

    pub fn solid_voxel_count(&self) -> usize {
        self.solid_voxel_count
    }

    pub fn voxel_size(&self) -> f64 {
        self.voxel_size
    }

    pub fn chunk_size(&self) -> u32 {
        self.chunk_size
    }

    /// Every solid voxel address, sorted. Computed from the chunks on each call.
    pub fn solid_voxels(&self) -> Vec<[i64; 3]> {
        self.material_voxels()
            .into_iter()
            .map(|voxel| voxel.address)
            .collect()
    }

    /// Every solid voxel with its material and state, sorted by address.
    /// Computed from the chunks on each call.
    pub fn material_voxels(&self) -> Vec<MaterialVoxel> {
        let grid = self.voxel_world.grid();
        let mut voxels: Vec<_> = self
            .voxel_world
            .resident_chunks()
            .flat_map(|(coordinate, chunk)| {
                chunk.iter().filter_map(move |(local, value)| {
                    material_voxel_at(grid, coordinate, local, value)
                })
            })
            .collect();
        voxels.sort_unstable();
        voxels
    }

    /// The voxel at `address`, if solid.
    pub fn material_voxel(&self, address: [i64; 3]) -> Option<MaterialVoxel> {
        let grid = self.voxel_world.grid();
        let (coordinate, local) =
            grid.voxel_to_chunk_local(VoxelCoord::new(address[0], address[1], address[2]));
        let value = self.voxel_world.get(coordinate)?.get(local)?;
        material_voxel_at(grid, coordinate, local, value)
    }

    /// Chunk meshes in chunk order.
    pub fn mesh_chunks(&self) -> impl ExactSizeIterator<Item = &VoxelMeshChunk> + '_ {
        self.mesh_chunks.values().map(|chunk| chunk.as_ref())
    }

    pub const fn mesh_options(&self) -> SurfaceMeshOptions {
        self.mesh_options
    }

    /// The mesh of one chunk, if it has one.
    pub fn mesh_chunk(&self, chunk: [i64; 3]) -> Option<&VoxelMeshChunk> {
        self.mesh_chunks
            .get(&ChunkCoord::new(chunk[0], chunk[1], chunk[2]))
            .map(|chunk| chunk.as_ref())
    }

    pub fn mesh_update(&self) -> &VoxelChunkMeshUpdate {
        &self.mesh_update
    }

    /// Identity of the chunk-mesh history. While it is unchanged, each source
    /// revision's `mesh_update().dirty_chunks` names every chunk mesh that
    /// changed since the previous revision, so a consumer that saw revision
    /// `n` can catch up to `n + 1` from that list alone.
    pub const fn mesh_lineage(&self) -> u64 {
        self.mesh_lineage
    }

    pub const fn source_revision(&self) -> VoxelSourceRevision {
        self.source_revision
    }

    /// Every projection is updated with its voxel change, so all share the
    /// source revision.
    pub const fn projection_revisions(&self) -> VoxelProjectionRevisions {
        VoxelProjectionRevisions::coherent(self.source_revision)
    }

    pub const fn authority_hash(&self) -> u64 {
        self.authority_hash
    }

    pub const fn world_origin(&self) -> WorldOrigin {
        self.world_origin
    }

    pub const fn rebase_revision(&self) -> u64 {
        self.rebase_revision
    }

    fn rebased_candidate(
        &self,
        target: WorldOrigin,
        rebase_revision: u64,
    ) -> Result<Self, CollisionSceneError> {
        let target_cell = target.cell();
        let origin_world = WorldPos::new(
            -(target_cell[0] as f64),
            -(target_cell[1] as f64),
            -(target_cell[2] as f64),
        );
        let voxel_world = self.voxel_world.clone().with_world_origin(origin_world);
        let mut candidate = Self::build_from_voxel_world_at_revision(
            self.voxel_size,
            self.chunk_size,
            voxel_world,
            SceneBuildRevision {
                source: self.source_revision,
                world_origin: target,
                rebase: rebase_revision,
            },
            self.mesh_options,
        )?;
        if !self.noncollidable_materials.is_empty() {
            candidate.set_noncollidable_materials(self.noncollidable_materials.clone());
        }
        let previous = self.world_origin.cell();
        let delta = WorldVec::new(
            (i128::from(previous[0]) - i128::from(target_cell[0])) as f64,
            (i128::from(previous[1]) - i128::from(target_cell[1])) as f64,
            (i128::from(previous[2]) - i128::from(target_cell[2])) as f64,
        );
        candidate
            .projection
            .copy_translated_static_meshes_from(&self.projection, delta)
            .map_err(CollisionSceneError::StaticMeshRebase)?;
        candidate.world_origin = target;
        candidate.rebase_revision = rebase_revision;
        Ok(candidate)
    }

    pub fn resident_chunk_count(&self) -> usize {
        self.voxel_world.resident_chunks().count()
    }

    pub fn resident_chunk_coordinates(&self) -> Vec<[i64; 3]> {
        self.voxel_world
            .resident_chunks()
            .map(|(coordinate, _)| coordinate.to_array())
            .collect()
    }

    pub fn projection_version(&self) -> u64 {
        self.projection.version()
    }

    /// The collision shapes a Dynamics world binds as its static environment.
    pub fn bind_dynamics_environment(
        &self,
        solver: &mut DynamicsSolver,
    ) -> DynamicsEnvironmentReceipt {
        solver.bind_environment(&self.projection)
    }

    pub fn collider_chunk_count(&self) -> usize {
        self.projection.collider_count()
    }

    pub fn has_collider_chunk(&self, chunk: [i64; 3]) -> bool {
        self.projection
            .has_collider(ChunkCoord::new(chunk[0], chunk[1], chunk[2]))
    }

    pub fn navigation_cell_count(&self) -> usize {
        self.navigation.walkable_len()
    }

    pub fn navigation_hash(&self) -> u64 {
        self.navigation.projection_hash()
    }

    /// The retained voxel authority that owns this scene's collision and
    /// navigation derivations. Consumers may derive another named Engine
    /// projection from it, but do not receive mutable voxel storage through
    /// this read-only access.
    pub fn voxel_world(&self) -> &VoxelWorld {
        &self.voxel_world
    }

    pub fn navigation_step(
        &self,
        from: Vec3,
        target: Vec3,
        current_velocity: Vec3,
        max_step_units: f32,
        max_visited: usize,
    ) -> Result<NavigationStep, NavigationStepError> {
        let readout = propose_projected_direct_nav_movement(
            &self.navigation,
            ProjectedDirectNavMovementRequest {
                from,
                target,
                max_step_units,
                max_visited,
            },
        )
        .map_err(|error| match error {
            ProjectedDirectNavMovementError::NonFinitePosition => {
                NavigationStepError::InvalidRequest {
                    reason: "nonFinitePosition",
                }
            }
            ProjectedDirectNavMovementError::InvalidStep => NavigationStepError::InvalidRequest {
                reason: "invalidStep",
            },
            ProjectedDirectNavMovementError::InvalidQueryBudget => {
                NavigationStepError::InvalidRequest {
                    reason: "invalidQueryBudget",
                }
            }
            ProjectedDirectNavMovementError::StartNotWalkable { start } => {
                NavigationStepError::StartNotWalkable {
                    start: start.to_array(),
                }
            }
            ProjectedDirectNavMovementError::GoalNotWalkable { goal } => {
                NavigationStepError::GoalNotWalkable {
                    goal: goal.to_array(),
                }
            }
            ProjectedDirectNavMovementError::NoPath { start, goal } => {
                NavigationStepError::NoPath {
                    start: start.to_array(),
                    goal: goal.to_array(),
                }
            }
        })?;
        // The navigation query is deliberately stateless. Once an agent crosses a
        // voxel boundary it would otherwise immediately turn toward the next
        // cell and cut the corner of an adjacent solid. Finish centering in the
        // newly entered cell before advancing; collision remains the fail-closed
        // authority for the actual body volume.
        let start_center = self.navigation.grid().voxel_center_world(readout.start);
        let start_center = Vec3::new(
            start_center.x as f32,
            start_center.y as f32,
            start_center.z as f32,
        );
        let to_center = start_center - from;
        let centered = to_center.length() <= 0.001;
        let moving_toward_center = to_center.x * current_velocity.x
            + to_center.y * current_velocity.y
            + to_center.z * current_velocity.z
            > 0.0;
        let (next_waypoint, reached) = if readout.path_len > 1 && !centered && moving_toward_center
        {
            let centering = propose_direct_nav_movement(DirectNavMovementRequest {
                from,
                target: start_center,
                max_step_units,
            })
            .map_err(|error| NavigationStepError::InvalidRequest {
                reason: error.label(),
            })?;
            (centering.next_waypoint, false)
        } else {
            (readout.next_waypoint, readout.reached)
        };
        Ok(NavigationStep {
            next_waypoint,
            reached,
            visited: readout.visited,
            path_len: readout.path_len,
            projection_hash: readout.projection_hash,
            path_hash: readout.path_hash,
        })
    }

    pub fn contains_point(&self, point: [f64; 3]) -> bool {
        self.projection
            .contains_point(WorldPos::new(point[0], point[1], point[2]))
    }

    pub fn raycast(
        &self,
        origin: [f64; 3],
        direction: [f64; 3],
        max_distance: f64,
    ) -> Option<CollisionRayHit> {
        self.projection
            .raycast(
                Ray::new(
                    WorldPos::new(origin[0], origin[1], origin[2]),
                    WorldVec::new(direction[0], direction[1], direction[2]),
                ),
                max_distance,
            )
            .map(|hit| CollisionRayHit {
                voxel: hit.voxel.to_array(),
                face: hit.face,
                point: hit.point.to_array(),
                distance: hit.distance,
            })
    }

    /// Cast against the coherent voxel projection plus the caller-supplied
    /// static-mesh projection. Voxel-only editing continues to use [`Self::raycast`].
    pub fn raycast_world(
        &self,
        origin: [f64; 3],
        direction: [f64; 3],
        max_distance: f64,
    ) -> Option<SpatialCollisionHit> {
        self.projection
            .raycast_world(
                Ray::new(
                    WorldPos::new(origin[0], origin[1], origin[2]),
                    WorldVec::new(direction[0], direction[1], direction[2]),
                ),
                max_distance,
            )
            .map(|hit| match hit {
                CollisionHit::Voxel(hit) => SpatialCollisionHit::Voxel(CollisionRayHit {
                    voxel: hit.voxel.to_array(),
                    face: hit.face,
                    point: hit.point.to_array(),
                    distance: hit.distance,
                }),
                CollisionHit::StaticMesh(hit) => SpatialCollisionHit::StaticMesh(hit),
            })
    }

    pub fn static_mesh_collision_revision(&self) -> u64 {
        self.projection.static_mesh_revision()
    }

    /// Read one retained collision-resident mesh instance's authored identity
    /// and current pose for a bounded call-local Engine service. The returned
    /// value does not expose the retained shape or create a second authority.
    pub fn static_mesh_instance(
        &self,
        id: StaticMeshInstanceId,
    ) -> Option<(StaticMeshAssetId, u64, StaticMeshTransform)> {
        self.projection.static_mesh_instance(id)
    }

    /// Stable identity for the retained static-mesh collision topology. Pose
    /// is excluded only for explicitly admitted moving instances; all other
    /// retained instances keep their complete pose in the identity.
    pub fn static_mesh_collision_topology_hash_excluding_pose(
        &self,
        moving_instances: &std::collections::BTreeSet<StaticMeshInstanceId>,
    ) -> u64 {
        self.projection
            .static_mesh_collision_topology_hash_excluding_pose(moving_instances)
    }

    /// Number of caller-owned static collision assets currently admitted into
    /// the derived projection.
    pub fn projection_static_mesh_asset_count(&self) -> usize {
        self.projection.static_mesh_asset_count()
    }

    /// Number of caller-owned static collision instances currently admitted
    /// into the derived projection.
    pub fn projection_static_mesh_instance_count(&self) -> usize {
        self.projection.static_mesh_instance_count()
    }

    pub fn replace_static_mesh_colliders(
        &mut self,
        assets: impl IntoIterator<Item = StaticMeshColliderAsset>,
        instances: impl IntoIterator<Item = StaticMeshColliderInstance>,
    ) -> Result<StaticMeshCollisionReceipt, StaticMeshCollisionError> {
        self.projection.replace_static_meshes(assets, instances)
    }

    pub fn static_mesh_asset_geometry_hash(&self, id: StaticMeshAssetId) -> Option<u64> {
        self.projection.static_mesh_asset_geometry_hash(id)
    }

    /// Incremental authored collision residency in the current local origin frame.
    pub fn apply_static_mesh_residency(
        &mut self,
        assets: impl IntoIterator<Item = StaticMeshColliderAsset>,
        instances: impl IntoIterator<Item = StaticMeshColliderInstance>,
        removed_assets: impl IntoIterator<Item = StaticMeshAssetId>,
        removed_instances: impl IntoIterator<Item = StaticMeshInstanceId>,
    ) -> Result<StaticMeshCollisionReceipt, StaticMeshCollisionError> {
        self.projection.apply_static_mesh_residency(
            assets,
            instances,
            removed_assets,
            removed_instances,
        )
    }

    /// Select collision participation independently of retained visual voxels.
    /// Material slots omitted from this set retain ordinary solid collision.
    pub fn noncollidable_materials(&self) -> &BTreeSet<u16> {
        &self.noncollidable_materials
    }

    pub fn set_noncollidable_materials(&mut self, materials: BTreeSet<u16>) {
        self.noncollidable_materials = materials;
        let mut projection = CollisionProjection::build(&self.voxel_world);
        projection.copy_static_meshes_from(&self.projection);
        let filtered: Vec<_> = self
            .voxel_world
            .resident_chunks()
            .filter_map(|(coordinate, _)| {
                match collision_chunk(&self.voxel_world, &self.noncollidable_materials, coordinate)
                {
                    Some(Cow::Owned(chunk)) => Some((coordinate, chunk)),
                    _ => None,
                }
            })
            .collect();
        projection.reconcile_chunks(
            filtered
                .iter()
                .map(|(coordinate, chunk)| (*coordinate, Some(chunk))),
        );
        self.projection = projection;
        self.rebuild_navigation();
    }

    fn rebuild_navigation(&mut self) {
        let grid = self.voxel_world.grid();
        let cells: Vec<_> = self
            .voxel_world
            .resident_chunks()
            .flat_map(|(coordinate, chunk)| {
                chunk
                    .iter()
                    .map(move |(local, _)| grid.chunk_local_to_voxel(coordinate, local))
            })
            .collect();
        self.navigation = NavProjection::from_walkable_cells(grid, []);
        let (world, noncollidable) = (&self.voxel_world, &self.noncollidable_materials);
        self.navigation
            .refresh_cells(SCENE_NAVIGATION, cells, |cell| {
                collision_solid(world, noncollidable, cell)
            });
    }

    /// Meshes for `dirty` chunks from the current voxels; `None` removes one.
    pub(crate) fn build_meshes(
        &self,
        dirty: &BTreeSet<ChunkCoord>,
    ) -> Result<BTreeMap<ChunkCoord, Option<Arc<VoxelMeshChunk>>>, CollisionSceneError> {
        dirty
            .iter()
            .map(|coordinate| {
                let mesh = match self.voxel_world.get(*coordinate) {
                    Some(chunk) if !chunk.is_empty() => Some(Arc::new(build_mesh_chunk(
                        &self.voxel_world,
                        *coordinate,
                        self.mesh_options,
                    )?)),
                    _ => None,
                };
                Ok((*coordinate, mesh))
            })
            .collect()
    }

    /// Chunks whose mesh depends on voxels in `owner`: the owner and its
    /// resident face neighbours (all 26 neighbours for reconstructed
    /// surfaces, whose samples cross edges and corners).
    pub(crate) fn mesh_neighbourhood(&self, owner: ChunkCoord) -> Vec<ChunkCoord> {
        const FACES: [(i64, i64, i64); 6] = [
            (-1, 0, 0),
            (1, 0, 0),
            (0, -1, 0),
            (0, 1, 0),
            (0, 0, -1),
            (0, 0, 1),
        ];
        let mut chunks = vec![owner];
        let mut consider = |(x, y, z): (i64, i64, i64)| {
            let candidate = ChunkCoord::new(owner.x + x, owner.y + y, owner.z + z);
            if (x, y, z) != (0, 0, 0) && self.voxel_world.get(candidate).is_some() {
                chunks.push(candidate);
            }
        };
        if self.mesh_options.mode == SurfaceMode::GreedyCubes {
            FACES.into_iter().for_each(&mut consider);
        } else {
            for x in -1..=1 {
                for y in -1..=1 {
                    for z in -1..=1 {
                        consider((x, y, z));
                    }
                }
            }
        }
        chunks
    }

    /// Chunks whose mesh depends on the voxel at `voxel`: its own chunk, plus
    /// a resident neighbour only when the voxel lies on their shared boundary
    /// (edges and corners too for reconstructed surfaces).
    pub(crate) fn mesh_neighbourhood_of_voxel(&self, voxel: VoxelCoord) -> Vec<ChunkCoord> {
        let grid = self.voxel_world.grid();
        let [width, height, depth] = grid.chunk_dims().to_array();
        let (owner, local) = grid.voxel_to_chunk_local(voxel);
        let offsets = |at: u32, extent: u32| {
            let mut offsets = vec![0i64];
            if at == 0 {
                offsets.push(-1);
            }
            if at + 1 == extent {
                offsets.push(1);
            }
            offsets
        };
        let (xs, ys, zs) = (
            offsets(local.x, width),
            offsets(local.y, height),
            offsets(local.z, depth),
        );
        let mut chunks = vec![owner];
        let mut consider = |x: i64, y: i64, z: i64| {
            let candidate = ChunkCoord::new(owner.x + x, owner.y + y, owner.z + z);
            if (x, y, z) != (0, 0, 0) && self.voxel_world.get(candidate).is_some() {
                chunks.push(candidate);
            }
        };
        if self.mesh_options.mode == SurfaceMode::GreedyCubes {
            xs.iter().skip(1).for_each(|x| consider(*x, 0, 0));
            ys.iter().skip(1).for_each(|y| consider(0, *y, 0));
            zs.iter().skip(1).for_each(|z| consider(0, 0, *z));
        } else {
            for x in &xs {
                for y in &ys {
                    for z in &zs {
                        consider(*x, *y, *z);
                    }
                }
            }
        }
        chunks
    }

    /// Every cell of `coordinate`.
    pub(crate) fn chunk_cells(&self, coordinate: ChunkCoord) -> impl Iterator<Item = VoxelCoord> {
        let origin = self.voxel_world.grid().chunk_origin_voxel(coordinate);
        let extent = i64::from(self.chunk_size);
        (0..extent).flat_map(move |x| {
            (0..extent).flat_map(move |y| {
                (0..extent).map(move |z| VoxelCoord::new(origin.x + x, origin.y + y, origin.z + z))
            })
        })
    }

    /// Add (`present`) or remove one solid voxel's share of the count and hash.
    pub(crate) fn account_voxel(&mut self, voxel: MaterialVoxel, present: bool) {
        if present {
            self.solid_voxel_count += 1;
            self.authority_hash = self.authority_hash.wrapping_add(voxel_hash(voxel));
        } else {
            self.solid_voxel_count -= 1;
            self.authority_hash = self.authority_hash.wrapping_sub(voxel_hash(voxel));
        }
    }

    /// Account every solid voxel of a chunk.
    pub(crate) fn account_chunk(
        &mut self,
        coordinate: ChunkCoord,
        chunk: &VoxelChunk,
        present: bool,
    ) {
        let grid = self.voxel_world.grid();
        let voxels: Vec<_> = chunk
            .iter()
            .filter_map(|(local, value)| material_voxel_at(grid, coordinate, local, value))
            .collect();
        for voxel in voxels {
            self.account_voxel(voxel, present);
        }
    }

    /// Publish one local change whose voxels are already written: install the
    /// rebuilt meshes, replace the changed chunks' colliders, refresh the
    /// affected navigation cells, and advance the source revision.
    pub(crate) fn publish_local_change(
        &mut self,
        changed: &BTreeSet<ChunkCoord>,
        dirty: &BTreeSet<ChunkCoord>,
        meshes: BTreeMap<ChunkCoord, Option<Arc<VoxelMeshChunk>>>,
        navigation_cells: BTreeSet<VoxelCoord>,
    ) {
        let mut rebuilt_chunks = 0;
        let mut removed_chunks = 0;
        for (coordinate, mesh) in meshes {
            match mesh {
                Some(mesh) => {
                    self.mesh_chunks.insert(coordinate, mesh);
                    rebuilt_chunks += 1;
                }
                None => {
                    if self.mesh_chunks.remove(&coordinate).is_some() {
                        removed_chunks += 1;
                    }
                }
            }
        }
        let colliders: Vec<_> = changed
            .iter()
            .map(|coordinate| {
                (
                    *coordinate,
                    collision_chunk(
                        &self.voxel_world,
                        &self.noncollidable_materials,
                        *coordinate,
                    ),
                )
            })
            .collect();
        self.projection.reconcile_chunks(
            colliders
                .iter()
                .map(|(coordinate, chunk)| (*coordinate, chunk.as_deref())),
        );
        let (world, noncollidable) = (&self.voxel_world, &self.noncollidable_materials);
        self.navigation
            .refresh_cells(SCENE_NAVIGATION, navigation_cells, |cell| {
                collision_solid(world, noncollidable, cell)
            });
        self.source_revision = self.source_revision.next();
        self.mesh_update = VoxelChunkMeshUpdate {
            source_revision: self.source_revision,
            surface_mode: self.mesh_options.mode,
            dirty_chunks: dirty
                .iter()
                .map(|coordinate| coordinate.to_array())
                .collect(),
            rebuilt_chunks,
            reused_chunks: self.mesh_chunks.len() - rebuilt_chunks,
            removed_chunks,
        };
    }

    pub fn aabb_overlaps_solid(&self, min: [f64; 3], max: [f64; 3]) -> bool {
        self.projection.aabb_overlaps_solid(
            WorldPos::new(min[0], min[1], min[2]),
            WorldPos::new(max[0], max[1], max[2]),
        )
    }

    /// Query the coherent voxel/static-mesh projection for an axis-aligned
    /// swept box. Foreign callers cannot reconstruct collision geometry from
    /// source facts; they ask this Engine-owned projection instead.
    pub fn axis_sweep_overlaps(&self, min: [f64; 3], max: [f64; 3], translation: [f64; 3]) -> bool {
        self.projection.axis_swept_aabb_overlaps_solid(
            WorldPos::new(min[0], min[1], min[2]),
            WorldPos::new(max[0], max[1], max[2]),
            WorldVec::new(translation[0], translation[1], translation[2]),
        )
    }

    /// Cast the Engine-owned capsule representation through the current
    /// voxel/static-mesh projection.
    pub fn cast_character_capsule(
        &self,
        capsule: CharacterCapsule,
        translation: WorldVec,
        contact_skin: f64,
    ) -> Result<Option<CharacterCapsuleCastHit>, CharacterCollisionQueryError> {
        self.projection
            .cast_character_capsule(capsule, translation, contact_skin)
    }

    /// Read the deepest Engine-owned capsule overlap in the current
    /// voxel/static-mesh projection.
    pub fn character_capsule_overlap(
        &self,
        capsule: CharacterCapsule,
    ) -> Result<Option<CharacterCapsuleOverlap>, CharacterCollisionQueryError> {
        self.projection.character_capsule_overlap(capsule)
    }
}

fn build_mesh_chunks(
    world: &VoxelWorld,
    options: SurfaceMeshOptions,
) -> Result<BTreeMap<ChunkCoord, Arc<VoxelMeshChunk>>, CollisionSceneError> {
    world
        .resident_chunks()
        .filter(|(_, chunk)| !chunk.is_empty())
        .map(|(coordinate, _)| {
            Ok((
                coordinate,
                Arc::new(build_mesh_chunk(world, coordinate, options)?),
            ))
        })
        .collect()
}

fn build_mesh_chunk(
    world: &VoxelWorld,
    coordinate: ChunkCoord,
    options: SurfaceMeshOptions,
) -> Result<VoxelMeshChunk, CollisionSceneError> {
    let grid = world.grid();
    let chunk = world.get(coordinate).expect("resident coordinate");
    let mesh = mesh_chunk_in_world_with_options(world, coordinate, options)
        .expect("resident coordinate")
        .map_err(CollisionSceneError::Mesh)?;
    let origin = grid.voxel_min_world(grid.chunk_origin_voxel(coordinate));
    Ok(voxel_mesh_chunk(
        coordinate,
        origin,
        chunk.content_hash().0,
        mesh,
    ))
}

fn material_voxel_at(
    grid: VoxelGridSpec,
    coordinate: ChunkCoord,
    local: LocalVoxelCoord,
    value: VoxelValue,
) -> Option<MaterialVoxel> {
    value.material().map(|material| MaterialVoxel {
        state: value.state().raw(),
        address: grid.chunk_local_to_voxel(coordinate, local).to_array(),
        material_slot: material.raw(),
    })
}

/// A chunk as collision sees it: noncollidable materials cleared. Borrowed
/// when nothing is excluded.
fn collision_chunk<'a>(
    world: &'a VoxelWorld,
    noncollidable: &BTreeSet<u16>,
    coordinate: ChunkCoord,
) -> Option<Cow<'a, VoxelChunk>> {
    let chunk = world.get(coordinate)?;
    let excluded: Vec<_> = chunk
        .iter()
        .filter(|(_, value)| {
            value
                .material()
                .is_some_and(|material| noncollidable.contains(&material.raw()))
        })
        .map(|(local, _)| local)
        .collect();
    if excluded.is_empty() {
        return Some(Cow::Borrowed(chunk));
    }
    let mut filtered = chunk.clone();
    for local in excluded {
        filtered
            .set(local, VoxelValue::EMPTY)
            .expect("local coordinate came from chunk");
    }
    Some(Cow::Owned(filtered))
}

/// Collision solidity of one cell; `None` outside every resident chunk.
fn collision_solid(
    world: &VoxelWorld,
    noncollidable: &BTreeSet<u16>,
    cell: VoxelCoord,
) -> Option<bool> {
    let (coordinate, local) = world.grid().voxel_to_chunk_local(cell);
    let value = world.get(coordinate)?.get(local)?;
    Some(
        value
            .material()
            .is_some_and(|material| !noncollidable.contains(&material.raw())),
    )
}

fn voxel_mesh_chunk(
    coordinate: ChunkCoord,
    origin: WorldPos,
    source_chunk_hash: u64,
    mesh: svc_mesh::MeshPayload,
) -> VoxelMeshChunk {
    let content_hash = mesh_payload_hash(&mesh);
    VoxelMeshChunk {
        chunk: coordinate.to_array(),
        content_hash,
        source_chunk_hash,
        surface_mode: mesh.surface_mode,
        translation: [origin.x as f32, origin.y as f32, origin.z as f32],
        positions: mesh.positions,
        normals: mesh.normals,
        tile_coordinates: mesh.tile_coordinates,
        indices: mesh.indices,
        groups: mesh
            .groups
            .into_iter()
            .map(|group| VoxelMeshGroup {
                state: group.state,
                material_slot: group.material_slot,
                direction: group.direction,
                start: group.start,
                count: group.count,
            })
            .collect(),
        bounds_min: mesh.bounds.min,
        bounds_max: mesh.bounds.max,
        vertices: mesh.stats.vertices,
        quads: mesh.stats.quads,
        faces_culled: mesh.stats.faces_culled,
    }
}

fn mesh_payload_hash(mesh: &svc_mesh::MeshPayload) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET;
    let mut feed = |bytes: &[u8]| {
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(PRIME);
        }
    };
    feed(mesh.surface_mode.as_str().as_bytes());
    for value in &mesh.positions {
        feed(&value.to_bits().to_le_bytes());
    }
    for value in &mesh.normals {
        feed(&value.to_bits().to_le_bytes());
    }
    for value in &mesh.tile_coordinates {
        feed(&value.to_bits().to_le_bytes());
    }
    for value in &mesh.indices {
        feed(&value.to_le_bytes());
    }
    for group in &mesh.groups {
        feed(&group.material_slot.to_le_bytes());
        feed(&group.state.to_le_bytes());
        feed(&group.start.to_le_bytes());
        feed(&group.count.to_le_bytes());
    }
    for value in mesh.bounds.min.into_iter().chain(mesh.bounds.max) {
        feed(&value.to_bits().to_le_bytes());
    }
    hash
}

fn voxel_hash(voxel: MaterialVoxel) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for coordinate in voxel.address {
        feed_hash(&mut hash, &coordinate.to_le_bytes());
    }
    feed_hash(&mut hash, &voxel.material_slot.to_le_bytes());
    feed_hash(&mut hash, &voxel.state.to_le_bytes());
    hash
}

fn feed_hash(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionAxis {
    X,
    Y,
    Z,
}

impl MotionAxis {
    const ALL: [Self; 3] = [Self::X, Self::Y, Self::Z];

    const fn index(self) -> usize {
        match self {
            Self::X => 0,
            Self::Y => 1,
            Self::Z => 2,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum MotionFact {
    Moved {
        entity: EntityId,
        before: Vec3,
        after: Vec3,
    },
    Blocked {
        entity: EntityId,
        axis: MotionAxis,
        attempted_delta: f32,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct MotionPhaseReceipt {
    pub bodies_considered: usize,
    pub moved_bodies: usize,
    pub blocked_axes: usize,
    pub revision_before: u64,
    pub revision_after: u64,
    pub facts: Vec<MotionFact>,
    pub entity_facts: Vec<EntityFact>,
}

/// One kinematic body whose translation or velocity changed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedKinematicBody {
    pub entity: EntityId,
    pub translation_before: Vec3,
    pub translation_after: Vec3,
    pub velocity_before: Vec3,
    pub velocity_after: Vec3,
}

/// Motion resolved for a body set without publishing it anywhere.
#[derive(Debug, Clone, PartialEq)]
pub struct KinematicMotionResolution {
    pub bodies_considered: usize,
    pub moved_bodies: usize,
    pub blocked_axes: usize,
    /// Changed bodies in input order.
    pub changed: Vec<ResolvedKinematicBody>,
    pub facts: Vec<MotionFact>,
}

#[derive(Debug)]
pub enum MotionPhaseError {
    InvalidDeltaSeconds { actual: f32 },
    EntityBatch(BatchRejection),
}

impl std::fmt::Display for MotionPhaseError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for MotionPhaseError {}

/// A centrally scheduled service that resolves every kinematic body once.
///
/// Static voxel collision is checked independently on X, Y, then Z. A blocked
/// axis stops and zeroes that velocity component while other axes can still
/// move. All resulting object changes commit as one atomic entity batch.
pub struct KinematicMotionSystem;

impl KinematicMotionSystem {
    pub fn run(
        entities: &mut EntityState,
        scene: &VoxelCollisionScene,
        delta_seconds: f32,
    ) -> Result<MotionPhaseReceipt, MotionPhaseError> {
        Self::run_matching(entities, scene, delta_seconds, |_| true, &[])
    }

    /// Resolve only a named set of bodies. This lets a responsible gameplay
    /// system use the same collision invariant without accidentally advancing
    /// unrelated kinematic objects in the phase.
    pub fn run_selected(
        entities: &mut EntityState,
        scene: &VoxelCollisionScene,
        delta_seconds: f32,
        selected: &BTreeSet<EntityId>,
    ) -> Result<MotionPhaseReceipt, MotionPhaseError> {
        let dynamic_blockers: Vec<_> = entities
            .kinematic_bodies()
            .filter(|body| !selected.contains(&body.entity))
            .filter(|body| {
                entities
                    .view(body.entity)
                    .ok()
                    .and_then(|view| view.collision)
                    .is_some_and(|collision| collision.enabled)
            })
            .collect();
        Self::run_matching(
            entities,
            scene,
            delta_seconds,
            |entity| selected.contains(&entity),
            &dynamic_blockers,
        )
    }

    fn run_matching(
        entities: &mut EntityState,
        scene: &VoxelCollisionScene,
        delta_seconds: f32,
        mut include: impl FnMut(EntityId) -> bool,
        dynamic_blockers: &[KinematicBodyView],
    ) -> Result<MotionPhaseReceipt, MotionPhaseError> {
        let bodies: Vec<_> = entities
            .kinematic_bodies()
            .filter(|body| include(body.entity))
            .collect();
        let resolution = Self::resolve(scene, delta_seconds, &bodies, dynamic_blockers)?;
        let revision_before = entities.revision();
        let mut commands = Vec::new();
        for body in &resolution.changed {
            if body.translation_after != body.translation_before {
                commands.push(EntityCommand::SetTranslation {
                    entity: body.entity,
                    translation: body.translation_after,
                });
            }
            if body.velocity_after != body.velocity_before {
                commands.push(EntityCommand::SetKinematicVelocity {
                    entity: body.entity,
                    velocity: body.velocity_after,
                });
            }
        }
        let (revision_after, entity_facts) = if commands.is_empty() {
            (revision_before, Vec::new())
        } else {
            let receipt = entities
                .apply_batch(EntityCommandBatch::new(commands))
                .map_err(MotionPhaseError::EntityBatch)?;
            (receipt.revision_after, receipt.facts)
        };

        Ok(MotionPhaseReceipt {
            bodies_considered: resolution.bodies_considered,
            moved_bodies: resolution.moved_bodies,
            blocked_axes: resolution.blocked_axes,
            revision_before,
            revision_after,
            facts: resolution.facts,
            entity_facts,
        })
    }

    /// Resolve `bodies` against static collision and `dynamic_blockers`
    /// without an entity store. Hosts that own body state apply the changed
    /// translations and velocities themselves.
    pub fn resolve(
        scene: &VoxelCollisionScene,
        delta_seconds: f32,
        bodies: &[KinematicBodyView],
        dynamic_blockers: &[KinematicBodyView],
    ) -> Result<KinematicMotionResolution, MotionPhaseError> {
        if !delta_seconds.is_finite() || !(0.0..=MAX_MOTION_DELTA_SECONDS).contains(&delta_seconds)
        {
            return Err(MotionPhaseError::InvalidDeltaSeconds {
                actual: delta_seconds,
            });
        }

        let mut changed = Vec::new();
        let mut facts = Vec::new();
        let mut moved_bodies = 0usize;
        let mut blocked_axes = 0usize;

        for body in bodies {
            let before = body.translation;
            let mut position = body.translation.to_array();
            let before_velocity = body.velocity;
            let mut velocity = body.velocity.to_array();
            let half_extents = body.half_extents.to_array();

            for axis in MotionAxis::ALL {
                let index = axis.index();
                let delta = velocity[index] * delta_seconds;
                if delta == 0.0 {
                    continue;
                }
                let min = [
                    f64::from(position[0] - half_extents[0]),
                    f64::from(position[1] - half_extents[1]),
                    f64::from(position[2] - half_extents[2]),
                ];
                let max = [
                    f64::from(position[0] + half_extents[0]),
                    f64::from(position[1] + half_extents[1]),
                    f64::from(position[2] + half_extents[2]),
                ];
                let mut translation = [0.0; 3];
                translation[index] = f64::from(delta);

                if scene.axis_sweep_overlaps(min, max, translation)
                    || dynamic_axis_sweep_overlaps(
                        body.entity,
                        min,
                        max,
                        translation,
                        dynamic_blockers,
                    )
                {
                    velocity[index] = 0.0;
                    blocked_axes += 1;
                    facts.push(MotionFact::Blocked {
                        entity: body.entity,
                        axis,
                        attempted_delta: delta,
                    });
                } else {
                    position[index] += delta;
                }
            }

            let after = Vec3::new(position[0], position[1], position[2]);
            let after_velocity = Vec3::new(velocity[0], velocity[1], velocity[2]);
            if after != before {
                moved_bodies += 1;
                facts.push(MotionFact::Moved {
                    entity: body.entity,
                    before,
                    after,
                });
            }
            if after != before || after_velocity != before_velocity {
                changed.push(ResolvedKinematicBody {
                    entity: body.entity,
                    translation_before: before,
                    translation_after: after,
                    velocity_before: before_velocity,
                    velocity_after: after_velocity,
                });
            }
        }

        Ok(KinematicMotionResolution {
            bodies_considered: bodies.len(),
            moved_bodies,
            blocked_axes,
            changed,
            facts,
        })
    }
}

fn dynamic_axis_sweep_overlaps(
    moving: EntityId,
    min: [f64; 3],
    max: [f64; 3],
    translation: [f64; 3],
    blockers: &[KinematicBodyView],
) -> bool {
    let swept_min = [
        min[0].min(min[0] + translation[0]),
        min[1].min(min[1] + translation[1]),
        min[2].min(min[2] + translation[2]),
    ];
    let swept_max = [
        max[0].max(max[0] + translation[0]),
        max[1].max(max[1] + translation[1]),
        max[2].max(max[2] + translation[2]),
    ];
    blockers.iter().any(|blocker| {
        if blocker.entity == moving {
            return false;
        }
        let center = blocker.translation.to_array();
        let half = blocker.half_extents.to_array();
        let blocker_min = [
            f64::from(center[0] - half[0]),
            f64::from(center[1] - half[1]),
            f64::from(center[2] - half[2]),
        ];
        let blocker_max = [
            f64::from(center[0] + half[0]),
            f64::from(center[1] + half[1]),
            f64::from(center[2] + half[2]),
        ];
        (0..3)
            .all(|axis| swept_min[axis] < blocker_max[axis] && swept_max[axis] > blocker_min[axis])
    })
}
