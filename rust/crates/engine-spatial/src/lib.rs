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
mod surface_collision;
mod trigger;
mod voxel_density;
mod voxel_edit;
mod voxel_picking;
mod voxel_primitive;
mod voxel_residency;
mod voxel_template;
mod world_origin;

pub use character_controller::{
    character_edge_is_traversable, character_edge_outcome,
    character_edge_outcome_between_clear_supports, character_jump_outcome, character_jump_plan,
    CharacterAirConfig, CharacterBlockKind, CharacterConfigError, CharacterContactFact,
    CharacterContactKind, CharacterControllerCommand, CharacterControllerConfig,
    CharacterControllerError, CharacterControllerReadout, CharacterControllerReceipt,
    CharacterControllerService, CharacterEdgeOutcome, CharacterExternalMotionConfig,
    CharacterGroundConfig, CharacterGroundFact, CharacterJumpConfig, CharacterJumpOutcome,
    CharacterJumpPlan, CharacterMeshInstance, CharacterPlatformConfig, CharacterPlatformFact,
    CharacterRecoveryConfig, CharacterShapeConfig, CharacterSolverConfig, CharacterStanceFact,
    CharacterStepColliders, CharacterStepFact, CharacterStepSubject, CharacterStepWorld,
    CharacterSurfaceConfig, CharacterVerticalConfig, DynamicImpulseProposal,
    FirstPersonLookCommand, FirstPersonLookConfig, FirstPersonLookDiagnostic, FirstPersonLookError,
    FirstPersonLookReceipt, FirstPersonLookService, FirstPersonLookState,
    PreparedCharacterControllerStep,
};
pub use core_space::{GlobalPosition, WorldOrigin};
pub use entity_motion::{
    EntityMotionCommand, EntityMotionError, EntityMotionOutcome, EntityMotionReceipt,
    EntityMotionResolution, EntityMotionService, FirstPersonBasis, FirstPersonMotionCommand,
    FirstPersonMotionError, FirstPersonMotionInput, FirstPersonMotionReadout,
    FirstPersonMotionReceipt, FirstPersonMotionService, FirstPersonPose, MotionSpatialEntity,
};
pub use occlusion::{
    SpatialOcclusionCollider, SpatialOcclusionError, SpatialOcclusionHit, SpatialOcclusionQuery,
    SpatialOcclusionService,
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
    KinematicTriggerDefinition, TriggerCollider, TriggerGeometrySource, TriggerLifecycleReceipt,
    TriggerOverlapFact, TriggerOverlapFactKind, TriggerOverlapPair, TriggerOverlapReadout,
    TriggerReconcileCause, TriggerReconcileReceipt, TriggerRestoreReceipt, TriggerVolumeDiagnostic,
    TriggerVolumeDiagnosticCode, TriggerVolumeError, TriggerVolumeSystem,
};

pub use svc_collision::{
    cast_character_capsule_against_obstacles, character_capsule_overlap_obstacles,
    CharacterCapsule, CharacterCapsuleCastHit, CharacterCapsuleOverlap,
    CharacterCollisionQueryError, CharacterCollisionQueryStats, CharacterCollisionSource,
    CharacterObstacle, StaticMeshAssetId, StaticMeshColliderAsset, StaticMeshColliderInstance,
    StaticMeshCollisionError, StaticMeshCollisionReceipt, StaticMeshHit, StaticMeshInstanceId,
    StaticMeshTransform,
};
pub use svc_mesh::distance_field;
pub use svc_mesh::{
    MaterialSurface, MeshError as SurfaceMeshError, SurfaceCharacter, SurfaceMaterials,
    SurfaceMeshLimits, SurfaceMeshOptions, SurfaceMode, TerrainLayers, VertexPlacement,
    MAX_TERRAIN_LAYERS, MAX_TERRAIN_LAYER_SLOTS, MAX_TERRAIN_TRANSITION_CELLS,
};
pub use svc_volume::DEFAULT_DENSITY_MAGNITUDE;
pub use voxel_density::{
    VoxelDensityApplyError, VoxelDensityEdit, VoxelDensityEditService, VoxelDensityOperation,
    VoxelDensityReceipt, VoxelDensityRejection, VoxelDensityShape, MAX_DENSITY_EDIT_VOXELS,
};
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
    PreparedWorldOriginRebase, WorldOriginAffectedTransform, WorldOriginEntity, WorldOriginReadout,
    WorldOriginRebaseError, WorldOriginRebaseReceipt, WorldOriginRebaseRequest,
    WorldOriginRebaseService, WorldOriginState, DEFAULT_LOCAL_COORDINATE_ENVELOPE,
};

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Instant;

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
use svc_collision::{CollisionHit, CollisionProjection, PreparedChunkParts, Ray};
use svc_mesh::{mesh_chunk_in_world_with_options, MeshError};
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
    /// The cube face of a greedy run, or the box-projection face of a
    /// reconstructed run.
    pub direction: Option<core_space::Direction6>,
    pub surface_mode: SurfaceMode,
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
    /// The absolute voxel at the chunk's local origin, and the voxel size:
    /// `origin_voxel + position / voxel_size` is a vertex's absolute cell
    /// position, stable across world-origin rebases.
    pub origin_voxel: [i64; 3],
    pub voxel_size: f32,
    /// Voxels per axis: the chunk's box is `size * voxel_size` from its
    /// origin.
    pub size: [u32; 3],
    pub positions: Vec<f32>,
    pub normals: Vec<f32>,
    pub tile_coordinates: Vec<f32>,
    /// Four terrain layer weights per vertex when the session has terrain
    /// layers (`svc_mesh::MeshPayload::layer_weights`); empty otherwise.
    pub layer_weights: Vec<f32>,
    /// The chunk's coarse signed distance field over its box
    /// (`svc_mesh::distance_field`, `FIELD_CELLS`³ bytes, x fastest); empty
    /// when the mesher made none.
    pub distance_field: Vec<u8>,
    /// Hash of `distance_field` alone: a neighbour's edit within the field's
    /// reach changes it without `content_hash`, and republishes the field
    /// alone.
    pub field_hash: u64,
    pub indices: Vec<u32>,
    pub groups: Vec<VoxelMeshGroup>,
    /// The chunk-local storage index (x-fastest) of the voxel owning each
    /// triangle. Collision maps a reconstructed triangle to its voxel.
    pub triangle_owners: Vec<u32>,
    /// For each triangle, how many voxels along x, y and z its owners cover
    /// from its owner: a merged block face is owned by the voxels under it.
    /// Empty when every triangle has one owner.
    pub triangle_owner_spans: Vec<[u32; 3]>,
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
    /// The `mesh_state` this update was applied to, or `None` for a build.
    pub previous_mesh_state: Option<u64>,
    pub surface_mode: SurfaceMode,
    pub dirty_chunks: Vec<[i64; 3]>,
    pub rebuilt_chunks: usize,
    pub reused_chunks: usize,
    pub removed_chunks: usize,
    /// Time spent meshing the rebuilt chunks, summed over chunks (chunks may
    /// mesh in parallel, so wall time can be shorter).
    pub mesh_microseconds: u64,
}

/// Static collision authority plus its query-optimized derived projection.
///
/// Keeping both layers together preserves the important invariant that the
/// Parry representation accelerates queries but never becomes canonical state.
#[derive(Clone)]
pub struct VoxelCollisionScene {
    voxel_world: VoxelWorld,
    projection: CollisionProjection,
    voxel_size: f64,
    chunk_size: u32,
    solid_voxel_count: usize,
    noncollidable_materials: BTreeSet<u16>,
    /// Shared so an unchanged chunk's mesh is never copied.
    mesh_chunks: BTreeMap<ChunkCoord, Arc<VoxelMeshChunk>>,
    mesh_options: SurfaceMeshOptions,
    mesh_update: VoxelChunkMeshUpdate,
    /// Process-unique identity of this scene's chunk meshes. Every build
    /// (including a world-origin rebase) and every local change takes a new
    /// one; a clone shares it until either copy changes.
    mesh_state: u64,
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

impl SceneBuildRevision {
    const fn initial(source: VoxelSourceRevision) -> Self {
        Self {
            source,
            world_origin: WorldOrigin::ZERO,
            rebase: 0,
        }
    }
}

fn next_mesh_state() -> u64 {
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
    /// The unit surface normal at the impact.
    pub normal: [f64; 3],
    pub distance: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpatialCollisionHit {
    Voxel(CollisionRayHit),
    StaticMesh(StaticMeshHit),
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
        // An edit remeshes only the chunks next to its own, so a transition
        // must stay within one chunk.
        if mesh_options
            .terrain_layers
            .as_ref()
            .is_some_and(|layers| u32::from(layers.transition_cells()) + 1 >= chunk_size)
        {
            return Err(CollisionSceneError::Mesh(MeshError::InvalidTerrainLayers));
        }
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
        let (mesh_chunks, mesh_microseconds) = build_mesh_chunks(&voxel_world, &mesh_options)?;
        let dirty_chunks = voxel_world
            .resident_chunks()
            .map(|(coordinate, _)| coordinate.to_array())
            .collect();
        let rebuilt_chunks = mesh_chunks.len();
        let projection = CollisionProjection::build(&VoxelWorld::new(grid));
        let mut scene = Self {
            projection,
            voxel_world,
            voxel_size,
            chunk_size,
            solid_voxel_count,
            noncollidable_materials: BTreeSet::new(),
            mesh_chunks,
            mesh_update: VoxelChunkMeshUpdate {
                source_revision: revisions.source,
                previous_mesh_state: None,
                surface_mode: mesh_options.mode,
                dirty_chunks,
                rebuilt_chunks,
                reused_chunks: 0,
                removed_chunks: 0,
                mesh_microseconds,
            },
            mesh_options,
            mesh_state: next_mesh_state(),
            source_revision: revisions.source,
            authority_hash,
            world_origin: revisions.world_origin,
            rebase_revision: revisions.rebase,
        };
        let coordinates: Vec<_> = scene
            .voxel_world
            .resident_chunks()
            .map(|(coordinate, _)| coordinate)
            .collect();
        // Part of building the projection: no version bump.
        let colliders = scene.prepare_colliders(&coordinates);
        scene.install_prepared(coordinates.into_iter().zip(colliders));
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

    /// The signed density at `address` (negative inside, in voxel units),
    /// or `None` outside the resident chunks. A voxel without an explicit
    /// density reads -0.5 when solid and 0.5 when empty.
    pub fn density(&self, address: [i64; 3]) -> Option<f32> {
        let grid = self.voxel_world.grid();
        let (coordinate, local) =
            grid.voxel_to_chunk_local(VoxelCoord::new(address[0], address[1], address[2]));
        self.voxel_world.get(coordinate)?.density(local)
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

    pub const fn mesh_options(&self) -> &SurfaceMeshOptions {
        &self.mesh_options
    }

    /// Whether chunks build their distance fields
    /// (`SurfaceMeshOptions::distance_fields`).
    pub fn distance_fields(&self) -> bool {
        self.mesh_options.distance_fields
    }

    /// Build every resident chunk's distance field, or drop them all: the
    /// renderer's distance-field occlusion was selected or deselected. The
    /// meshes, colliders, voxels and source revision are unchanged; the
    /// refreshed chunks are the next mesh update's dirty chunks, so a
    /// projection republishes their fields alone. Nothing happens when the
    /// setting is already so.
    pub fn set_distance_fields(&mut self, enabled: bool) {
        if self.mesh_options.distance_fields == enabled {
            return;
        }
        self.mesh_options.distance_fields = enabled;
        let coordinates: Vec<ChunkCoord> = self.mesh_chunks.keys().copied().collect();
        let refreshed: Vec<(ChunkCoord, VoxelMeshChunk)> =
            surface_collision::in_parallel(&coordinates, |coordinate| {
                self.mesh_chunks.get(&coordinate).and_then(|retained| {
                    refresh_distance_field(&self.voxel_world, coordinate, retained, enabled)
                        .map(|chunk| (coordinate, chunk))
                })
            })
            .into_iter()
            .flatten()
            .collect();
        if refreshed.is_empty() {
            return;
        }
        let rebuilt_chunks = refreshed.len();
        let mut dirty_chunks = Vec::with_capacity(rebuilt_chunks);
        for (coordinate, chunk) in refreshed {
            self.mesh_chunks.insert(coordinate, Arc::new(chunk));
            dirty_chunks.push(coordinate.to_array());
        }
        let previous_mesh_state = std::mem::replace(&mut self.mesh_state, next_mesh_state());
        self.mesh_update = VoxelChunkMeshUpdate {
            source_revision: self.source_revision,
            previous_mesh_state: Some(previous_mesh_state),
            surface_mode: self.mesh_options.mode,
            dirty_chunks,
            rebuilt_chunks,
            reused_chunks: self.mesh_chunks.len() - rebuilt_chunks,
            removed_chunks: 0,
            mesh_microseconds: 0,
        };
    }

    /// Replace the surface modes and characters. Every chunk is remeshed and
    /// its collision rebuilt from what is drawn; the voxels, static meshes,
    /// collision materials, source revision and world origin are kept. A
    /// failed rebuild leaves the scene unchanged.
    pub fn set_mesh_options(
        &mut self,
        options: SurfaceMeshOptions,
    ) -> Result<(), CollisionSceneError> {
        let mut candidate = Self::build_from_voxel_world_at_revision(
            self.voxel_size,
            self.chunk_size,
            self.voxel_world.clone(),
            SceneBuildRevision {
                source: self.source_revision,
                world_origin: self.world_origin,
                rebase: self.rebase_revision,
            },
            options,
        )?;
        if !self.noncollidable_materials.is_empty() {
            candidate.set_noncollidable_materials(self.noncollidable_materials.clone());
        }
        candidate
            .projection
            .copy_static_meshes_from(&self.projection);
        *self = candidate;
        Ok(())
    }

    /// Whether a distant chunk can be drawn coarse: some material is
    /// reconstructed and the chunk edge is even.
    pub fn has_coarse_meshes(&self) -> bool {
        !self.mesh_options.all_greedy() && self.chunk_size.is_multiple_of(2)
    }

    /// The meshes distant chunks are drawn with, in input order: their
    /// reconstructed materials from a lattice twice as coarse, with skirts
    /// (`svc_mesh::mesh_chunk_coarse_in_world`), built in parallel. Drawing
    /// only: collision and picking keep each chunk's own mesh. `None` for a
    /// chunk without a mesh, or for every chunk when the scene has no coarse
    /// meshes.
    pub fn coarse_mesh_chunks(
        &self,
        chunks: &[[i64; 3]],
    ) -> Vec<Option<Result<VoxelMeshChunk, CollisionSceneError>>> {
        let coordinates: Vec<_> = chunks
            .iter()
            .map(|chunk| ChunkCoord::new(chunk[0], chunk[1], chunk[2]))
            .collect();
        surface_collision::in_parallel(&coordinates, |coordinate| {
            if !self.has_coarse_meshes() || !self.mesh_chunks.contains_key(&coordinate) {
                return None;
            }
            let grid = self.voxel_world.grid();
            let source = self.voxel_world.get(coordinate)?;
            // EXPLORE #9513: per-chunk CPU time.
            let started = std::time::Instant::now();
            let mesh = svc_mesh::mesh_chunk_coarse_in_world(
                &self.voxel_world,
                coordinate,
                &self.mesh_options,
            );
            COARSE_MESH_MICROS.fetch_add(
                started.elapsed().as_micros() as u64,
                std::sync::atomic::Ordering::Relaxed,
            );
            let mesh = mesh?;
            Some(mesh.map_err(CollisionSceneError::Mesh).map(|mesh| {
                voxel_mesh_chunk(
                    coordinate,
                    grid.voxel_min_world(grid.chunk_origin_voxel(coordinate)),
                    grid.chunk_origin_voxel(coordinate).to_array(),
                    grid.chunk_dims().to_array(),
                    grid.voxel_size() as f32,
                    source.content_hash().0,
                    mesh,
                )
            }))
        })
    }

    /// The coarse mesh of one chunk; see [`Self::coarse_mesh_chunks`].
    pub fn coarse_mesh_chunk(
        &self,
        chunk: [i64; 3],
    ) -> Option<Result<VoxelMeshChunk, CollisionSceneError>> {
        self.coarse_mesh_chunks(&[chunk]).pop().flatten()
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

    /// Identity of the current chunk meshes. Two scenes with the same state
    /// have the same meshes. When `mesh_update().previous_mesh_state` is the
    /// state a consumer last saw, `mesh_update().dirty_chunks` names every
    /// chunk mesh that changed since then.
    pub const fn mesh_state(&self) -> u64 {
        self.mesh_state
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
            self.mesh_options.clone(),
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

    /// The retained voxel authority that owns this scene's collision and
    /// mesh derivations. Consumers may derive another named Engine
    /// projection from it, but do not receive mutable voxel storage through
    /// this read-only access.
    pub fn voxel_world(&self) -> &VoxelWorld {
        &self.voxel_world
    }

    /// The height where the run of collidable voxels containing `point` ends
    /// below it, or `None` when `point` is not in a collidable voxel. The
    /// walk down stops at `floor`.
    pub fn collidable_voxel_run_bottom(&self, point: [f64; 3], floor: f64) -> Option<f64> {
        let grid = self.voxel_world.grid();
        let solid = |chunk: &VoxelChunk, local| {
            chunk.get(local).is_some_and(|value| {
                value
                    .material()
                    .is_some_and(|material| !self.noncollidable_materials.contains(&material.raw()))
            })
        };
        let mut cell = grid.world_to_voxel(WorldPos::new(point[0], point[1], point[2]));
        let (mut coordinate, mut local) = grid.voxel_to_chunk_local(cell);
        let mut chunk = self.voxel_world.get(coordinate)?;
        if !solid(chunk, local) {
            return None;
        }
        // The walk reads its chunk straight until it leaves it below.
        loop {
            let bottom = grid.voxel_min_world(cell).y;
            if bottom <= floor {
                return Some(bottom);
            }
            let below = if local.y > 0 {
                (chunk, LocalVoxelCoord::new(local.x, local.y - 1, local.z))
            } else {
                coordinate = ChunkCoord::new(coordinate.x, coordinate.y - 1, coordinate.z);
                let Some(chunk) = self.voxel_world.get(coordinate) else {
                    return Some(bottom);
                };
                (
                    chunk,
                    LocalVoxelCoord::new(local.x, grid.chunk_dims().y() - 1, local.z),
                )
            };
            if !solid(below.0, below.1) {
                return Some(bottom);
            }
            (chunk, local) = below;
            cell.y -= 1;
        }
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
                normal: [hit.normal.x, hit.normal.y, hit.normal.z],
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
                    normal: [hit.normal.x, hit.normal.y, hit.normal.z],
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
        let coordinates: Vec<_> = self
            .voxel_world
            .resident_chunks()
            .map(|(coordinate, _)| coordinate)
            .collect();
        let colliders = self.prepare_colliders(&coordinates);
        self.install_prepared(coordinates.into_iter().zip(colliders));
        self.projection.touch();
    }

    /// Rebuild `dirty` chunks from the current voxels: each one's mesh
    /// (`None` removes one) and the collider of each one whose collider
    /// changes, built together on the meshing threads. A cube chunk's
    /// collider follows its own voxels, so only `changed` chunks rebuild it;
    /// a reconstructed chunk's collider is its drawn mesh, which its
    /// neighbours shape.
    /// `field_dirty` chunks outside `dirty` keep their meshes and rebuild
    /// only their distance fields.
    pub(crate) fn rebuild_chunks(
        &self,
        changed: &BTreeSet<ChunkCoord>,
        dirty: &BTreeSet<ChunkCoord>,
        field_dirty: &BTreeSet<ChunkCoord>,
    ) -> Result<ChunkRebuild, CollisionSceneError> {
        let greedy = self.mesh_options.all_greedy();
        let coordinates: Vec<_> = dirty.iter().copied().collect();
        let built = surface_collision::in_parallel(&coordinates, |coordinate| {
            let mesh = match self.voxel_world.get(coordinate) {
                Some(chunk) if !chunk.is_empty() => {
                    let started = Instant::now();
                    let mesh = build_mesh_chunk(&self.voxel_world, coordinate, &self.mesh_options)?;
                    Some((Arc::new(mesh), started.elapsed().as_micros() as u64))
                }
                _ => None,
            };
            let collider = (!greedy || changed.contains(&coordinate)).then(|| {
                self.prepare_collider(coordinate, mesh.as_ref().map(|(mesh, _)| mesh.as_ref()))
            });
            Ok::<_, CollisionSceneError>((mesh, collider))
        });
        let mut rebuild = ChunkRebuild {
            meshes: BTreeMap::new(),
            colliders: Vec::new(),
            microseconds: 0,
        };
        for (coordinate, built) in coordinates.into_iter().zip(built) {
            let (mesh, collider) = built?;
            if let Some((_, microseconds)) = &mesh {
                rebuild.microseconds = rebuild.microseconds.saturating_add(*microseconds);
            }
            rebuild
                .meshes
                .insert(coordinate, mesh.map(|(mesh, _)| mesh));
            if let Some(collider) = collider {
                rebuild.colliders.push((coordinate, collider));
            }
        }
        // A neighbour's edit reached these chunks' fields without touching
        // their meshes: refresh the field alone.
        for coordinate in field_dirty.difference(dirty) {
            if let Some(refreshed) = self.mesh_chunks.get(coordinate).and_then(|retained| {
                refresh_distance_field(
                    &self.voxel_world,
                    *coordinate,
                    retained,
                    self.mesh_options.distance_fields,
                )
            }) {
                rebuild
                    .meshes
                    .insert(*coordinate, Some(Arc::new(refreshed)));
            }
        }
        Ok(rebuild)
    }

    /// The collider of `coordinate` from its current voxels and `mesh`, its
    /// drawn mesh; `None` when the chunk is not resident. A cube session
    /// collides with every collidable voxel; a reconstructed one with its
    /// drawn surfaces and the voxels no surface passes through.
    fn prepare_collider(
        &self,
        coordinate: ChunkCoord,
        mesh: Option<&VoxelMeshChunk>,
    ) -> Option<(u64, PreparedChunkParts)> {
        let chunk = self.voxel_world.get(coordinate)?;
        if self.mesh_options.all_greedy() {
            let collided =
                collision_chunk(&self.voxel_world, &self.noncollidable_materials, coordinate)?;
            return Some((
                collided.content_hash().0,
                self.projection.prepare_chunk(coordinate, &collided),
            ));
        }
        let participation = surface_collision::noncollidable_key(&self.noncollidable_materials);
        let cubes = surface_collision::collider_cubes(
            &self.voxel_world,
            &self.noncollidable_materials,
            &self.mesh_options,
            coordinate,
        );
        let parts = self.projection.prepare_chunk_parts(
            coordinate,
            &cubes,
            // The triangles depend on which materials collide as well as on
            // the drawn mesh.
            mesh.map_or(0, |mesh| mesh.content_hash) ^ participation,
            || {
                mesh.and_then(|mesh| {
                    surface_collision::collider_surface(
                        &self.voxel_world,
                        &self.noncollidable_materials,
                        coordinate,
                        mesh,
                    )
                })
            },
        );
        Some((chunk.content_hash().0, parts))
    }

    /// The colliders of `coordinates`, built in parallel from their current
    /// voxels and meshes.
    fn prepare_colliders(
        &self,
        coordinates: &[ChunkCoord],
    ) -> Vec<Option<(u64, PreparedChunkParts)>> {
        surface_collision::in_parallel(coordinates, |coordinate| {
            self.prepare_collider(
                coordinate,
                self.mesh_chunks.get(&coordinate).map(|mesh| mesh.as_ref()),
            )
        })
    }

    /// Install prepared colliders in order; `None` removes a chunk's. Does
    /// not bump the projection version.
    fn install_prepared(
        &mut self,
        colliders: impl IntoIterator<Item = (ChunkCoord, Option<(u64, PreparedChunkParts)>)>,
    ) {
        for (coordinate, prepared) in colliders {
            match prepared {
                Some((source_hash, parts)) => {
                    self.projection
                        .install_chunk_parts(coordinate, source_hash, parts)
                }
                None => self.projection.remove_chunk(coordinate),
            }
        }
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
        if self.mesh_options.all_greedy() {
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
    /// (edges and corners too for reconstructed surfaces), or within terrain
    /// layers' reach of it.
    pub(crate) fn mesh_neighbourhood_of_voxel(&self, voxel: VoxelCoord) -> Vec<ChunkCoord> {
        let grid = self.voxel_world.grid();
        let [width, height, depth] = grid.chunk_dims().to_array();
        let (owner, local) = grid.voxel_to_chunk_local(voxel);
        // A neighbour's vertices lie within a voxel of this chunk and weigh
        // the voxels within a transition of them (one more for margin).
        let reach = self
            .mesh_options
            .terrain_layers
            .as_ref()
            .map_or(1, |layers| 1 + u32::from(layers.transition_cells()));
        let offsets = |at: u32, extent: u32| {
            let mut offsets = vec![0i64];
            if at < reach {
                offsets.push(-1);
            }
            if at + reach >= extent {
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
        if self.mesh_options.all_greedy() {
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

    /// Chunks whose distance field depends on the voxel at `voxel`: its own
    /// chunk and the resident neighbours whose field reaches it
    /// (`svc_mesh::distance_field_reach_voxels`), edge and corner
    /// neighbours included.
    pub(crate) fn field_neighbourhood_of_voxel(&self, voxel: VoxelCoord) -> Vec<ChunkCoord> {
        if !self.mesh_options.distance_fields {
            return Vec::new();
        }
        let grid = self.voxel_world.grid();
        let [width, height, depth] = grid.chunk_dims().to_array();
        let (owner, local) = grid.voxel_to_chunk_local(voxel);
        let reach = svc_mesh::distance_field_reach_voxels(self.chunk_size);
        let offsets = |at: u32, extent: u32| {
            let mut offsets = vec![0i64];
            if at < reach {
                offsets.push(-1);
            }
            if at + reach >= extent {
                offsets.push(1);
            }
            offsets
        };
        let mut chunks = vec![owner];
        for x in offsets(local.x, width) {
            for y in offsets(local.y, height) {
                for z in &offsets(local.z, depth) {
                    let candidate = ChunkCoord::new(owner.x + x, owner.y + y, owner.z + z);
                    if (x, y, *z) != (0, 0, 0) && self.voxel_world.get(candidate).is_some() {
                        chunks.push(candidate);
                    }
                }
            }
        }
        chunks
    }

    /// Chunks whose distance field depends on `owner` being resident: it and
    /// all its resident neighbours.
    pub(crate) fn field_neighbourhood(&self, owner: ChunkCoord) -> Vec<ChunkCoord> {
        if !self.mesh_options.distance_fields {
            return Vec::new();
        }
        let mut chunks = vec![owner];
        for x in -1..=1 {
            for y in -1..=1 {
                for z in -1..=1 {
                    let candidate = ChunkCoord::new(owner.x + x, owner.y + y, owner.z + z);
                    if (x, y, z) != (0, 0, 0) && self.voxel_world.get(candidate).is_some() {
                        chunks.push(candidate);
                    }
                }
            }
        }
        chunks
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

    /// Account every solid voxel of a chunk: `chunk`, or the resident one.
    pub(crate) fn account_chunk(
        &mut self,
        coordinate: ChunkCoord,
        chunk: Option<&VoxelChunk>,
        present: bool,
    ) {
        let grid = self.voxel_world.grid();
        let Some(chunk) = chunk.or_else(|| self.voxel_world.get(coordinate)) else {
            return;
        };
        let (count, hash) = chunk
            .iter()
            .filter_map(|(local, value)| material_voxel_at(grid, coordinate, local, value))
            .fold((0, 0_u64), |(count, hash), voxel| {
                (count + 1, hash.wrapping_add(voxel_hash(voxel)))
            });
        if present {
            self.solid_voxel_count += count;
            self.authority_hash = self.authority_hash.wrapping_add(hash);
        } else {
            self.solid_voxel_count -= count;
            self.authority_hash = self.authority_hash.wrapping_sub(hash);
        }
    }

    /// Publish one local change whose voxels are already written: install the
    /// rebuilt meshes, replace the changed chunks' colliders, and advance the
    /// source revision.
    pub(crate) fn publish_local_change(
        &mut self,
        dirty: &BTreeSet<ChunkCoord>,
        rebuild: ChunkRebuild,
    ) {
        let mut rebuilt_chunks = 0;
        let mut removed_chunks = 0;
        let mut rebuilt = Vec::new();
        for (coordinate, mesh) in rebuild.meshes {
            match mesh {
                Some(mesh) => {
                    self.mesh_chunks.insert(coordinate, mesh);
                    rebuilt_chunks += 1;
                    rebuilt.push(coordinate);
                }
                None => {
                    if self.mesh_chunks.remove(&coordinate).is_some() {
                        removed_chunks += 1;
                    }
                }
            }
        }
        self.install_prepared(rebuild.colliders);
        self.projection.touch();
        self.source_revision = self.source_revision.next();
        let previous_mesh_state = std::mem::replace(&mut self.mesh_state, next_mesh_state());
        self.mesh_update = VoxelChunkMeshUpdate {
            source_revision: self.source_revision,
            previous_mesh_state: Some(previous_mesh_state),
            surface_mode: self.mesh_options.mode,
            // The rebuilt meshes, which may include field-only refreshes
            // past `dirty`.
            dirty_chunks: dirty
                .iter()
                .chain(rebuilt.iter())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .map(|coordinate| coordinate.to_array())
                .collect(),
            rebuilt_chunks,
            reused_chunks: self.mesh_chunks.len() - rebuilt_chunks,
            removed_chunks,
            mesh_microseconds: rebuild.microseconds,
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

    /// Whether the capsule overlaps anything, without finding the deepest
    /// contact: cheaper where only the answer matters.
    pub fn character_capsule_intersects(
        &self,
        capsule: CharacterCapsule,
    ) -> Result<bool, CharacterCollisionQueryError> {
        self.projection.character_capsule_intersects(capsule)
    }
}

/// Rebuilt `dirty` chunks: their meshes (`None` removes one), the colliders
/// that change (`None` removes one), and the summed meshing time.
pub(crate) struct ChunkRebuild {
    meshes: BTreeMap<ChunkCoord, Option<Arc<VoxelMeshChunk>>>,
    colliders: Vec<(ChunkCoord, Option<(u64, PreparedChunkParts)>)>,
    microseconds: u64,
}

fn build_mesh_chunks(
    world: &VoxelWorld,
    options: &SurfaceMeshOptions,
) -> Result<(BTreeMap<ChunkCoord, Arc<VoxelMeshChunk>>, u64), CollisionSceneError> {
    let coordinates: Vec<_> = world
        .resident_chunks()
        .filter(|(_, chunk)| !chunk.is_empty())
        .map(|(coordinate, _)| coordinate)
        .collect();
    let (meshes, microseconds) = surface_collision::mesh_chunks(world, &coordinates, options)?;
    Ok((coordinates.into_iter().zip(meshes).collect(), microseconds))
}

pub(crate) fn build_mesh_chunk(
    world: &VoxelWorld,
    coordinate: ChunkCoord,
    options: &SurfaceMeshOptions,
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
        grid.chunk_origin_voxel(coordinate).to_array(),
        grid.chunk_dims().to_array(),
        grid.voxel_size() as f32,
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

fn voxel_mesh_chunk(
    coordinate: ChunkCoord,
    origin: WorldPos,
    origin_voxel: [i64; 3],
    size: [u32; 3],
    voxel_size: f32,
    source_chunk_hash: u64,
    mesh: svc_mesh::MeshPayload,
) -> VoxelMeshChunk {
    let voxels = size;
    let size = size.map(i64::from);
    let content_hash = mesh_payload_hash(&mesh);
    let triangle_owners = mesh
        .triangle_owners
        .iter()
        .map(|owner| {
            let local: [i64; 3] = std::array::from_fn(|axis| owner[axis] - origin_voxel[axis]);
            debug_assert!((0..3).all(|axis| (0..size[axis]).contains(&local[axis])));
            ((local[2] * size[1] + local[1]) * size[0] + local[0]) as u32
        })
        .collect();
    let distance_field = mesh
        .distance_field
        .map_or_else(Vec::new, |field| field.data);
    let field_hash = field_hash(&distance_field);
    VoxelMeshChunk {
        chunk: coordinate.to_array(),
        content_hash,
        field_hash,
        source_chunk_hash,
        surface_mode: mesh.surface_mode,
        translation: [origin.x as f32, origin.y as f32, origin.z as f32],
        origin_voxel,
        voxel_size,
        size: voxels,
        positions: mesh.positions,
        normals: mesh.normals,
        tile_coordinates: mesh.tile_coordinates,
        layer_weights: mesh.layer_weights,
        distance_field,
        indices: mesh.indices,
        groups: mesh
            .groups
            .into_iter()
            .map(|group| VoxelMeshGroup {
                state: group.state,
                material_slot: group.material_slot,
                direction: group.direction,
                surface_mode: group.surface_mode,
                start: group.start,
                count: group.count,
            })
            .collect(),
        triangle_owners,
        triangle_owner_spans: mesh.triangle_owner_spans,
        bounds_min: mesh.bounds.min,
        bounds_max: mesh.bounds.max,
        vertices: mesh.stats.vertices,
        quads: mesh.stats.quads,
        faces_culled: mesh.stats.faces_culled,
    }
}

/// A retained chunk with its distance field rebuilt from the world as it is
/// now (for a chunk a neighbour's edit reached without touching its mesh),
/// or dropped when the scene no longer builds fields. `None` when nothing
/// changes.
pub(crate) fn refresh_distance_field(
    world: &VoxelWorld,
    coordinate: ChunkCoord,
    retained: &VoxelMeshChunk,
    enabled: bool,
) -> Option<VoxelMeshChunk> {
    let data = if enabled {
        svc_mesh::chunk_distance_field(world, coordinate)?.data
    } else {
        Vec::new()
    };
    if data == retained.distance_field {
        return None;
    }
    let mut chunk = retained.clone();
    chunk.distance_field = data;
    chunk.field_hash = field_hash(&chunk.distance_field);
    Some(chunk)
}

/// The replacement key of a chunk's distance field alone.
fn field_hash(field: &[u8]) -> u64 {
    let mut hash = WordHash::default();
    hash.words([field.len() as u32]);
    hash.words(field.chunks(4).map(|bytes| {
        let mut word = [0_u8; 4];
        word[..bytes.len()].copy_from_slice(bytes);
        u32::from_le_bytes(word)
    }));
    hash.finish()
}

fn mesh_payload_hash(mesh: &svc_mesh::MeshPayload) -> u64 {
    let mut hash = WordHash::default();
    hash.words(mesh.surface_mode.as_str().bytes().map(u32::from));
    for stream in [
        &mesh.positions,
        &mesh.normals,
        &mesh.tile_coordinates,
        &mesh.layer_weights,
    ] {
        hash.words([stream.len() as u32]);
        hash.words(stream.iter().map(|value| value.to_bits()));
    }
    hash.words([mesh.indices.len() as u32]);
    hash.words(mesh.indices.iter().copied());
    for group in &mesh.groups {
        hash.words([
            u32::from(group.material_slot),
            u32::from(group.state),
            group
                .direction
                .map_or(u32::MAX, |direction| direction as u32),
            group.start,
            group.count,
        ]);
        hash.words(group.surface_mode.as_str().bytes().map(u32::from));
    }
    hash.words(
        mesh.bounds
            .min
            .into_iter()
            .chain(mesh.bounds.max)
            .map(f32::to_bits),
    );
    hash.finish()
}

/// A content identity over 32-bit words: each word is mixed into one of four
/// independent lanes in turn, and the lanes are folded at the end. Words and
/// lanes rather than FNV over bytes keep a large chunk's mesh hash a small
/// share of meshing it.
struct WordHash {
    lanes: [u64; 4],
    words: u64,
}

impl Default for WordHash {
    fn default() -> Self {
        Self {
            lanes: [
                0x243f_6a88_85a3_08d3,
                0x1319_8a2e_0370_7344,
                0xa409_3822_299f_31d0,
                0x082e_fa98_ec4e_6c89,
            ],
            words: 0,
        }
    }
}

impl WordHash {
    fn words(&mut self, words: impl IntoIterator<Item = u32>) {
        for word in words {
            let lane = &mut self.lanes[(self.words % 4) as usize];
            // One multiply per word; the rotation brings the product's high
            // bits down for the next word's multiply.
            *lane = (*lane ^ u64::from(word))
                .wrapping_mul(0x9e37_79b9_7f4a_7c15)
                .rotate_left(31);
            self.words += 1;
        }
    }

    fn finish(self) -> u64 {
        self.lanes
            .into_iter()
            .fold(finalize(self.words), |hash, lane| finalize(hash ^ lane))
    }
}

/// splitmix64's finalizer: a bijection that spreads every input bit over the
/// result.
fn finalize(value: u64) -> u64 {
    let value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    let value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
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

// EXPLORE #9513: coarse meshing CPU microseconds since the last take.
static COARSE_MESH_MICROS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub fn take_coarse_mesh_micros() -> u64 {
    COARSE_MESH_MICROS.swap(0, std::sync::atomic::Ordering::Relaxed)
}
