use std::{cell::RefCell, collections::BTreeMap, ffi::c_void, rc::Rc, sync::Arc};

use core_ids::EntityId;
use core_math::{Vec2, Vec3};
use core_space::{ChunkDims, GridId, VoxelCoord, VoxelGridSpec};
use csharp_engine_abi::*;
use engine_spatial::{
    CharacterCapsule, CharacterCollisionSource, CharacterContactFact, CharacterContactKind,
    CharacterControllerCommand, CharacterControllerConfig, CharacterControllerReceipt,
    CharacterControllerService, CharacterGroundFact, CharacterObstacle,
    SpatialOcclusionHitboxOverride, SpatialOcclusionQuery, SpatialOcclusionService,
    StaticMeshAssetId, StaticMeshColliderAsset, StaticMeshColliderInstance, StaticMeshInstanceId,
    StaticMeshTransform, TriggerGeometrySource, TriggerOverlapFact, TriggerOverlapFactKind,
    TriggerReconcileCause, TriggerVolumeError, TriggerVolumeSystem, VoxelCollisionScene,
    VoxelPickHint, VoxelPickService,
};
use entity_state::{
    CharacterMotionComponent, CharacterStance, EntityAuthoringService, EntityDefinition,
    EntityState, EntityTransform, Quat,
};
use svc_pathfinding::{
    build_nav_projection, find_path_with_policy, find_volumetric_path, propose_direct_nav_movement,
    DirectNavMovementRequest, NavError, NavPathOutcome, NavPathQuery, NavProjection,
    NavProjectionConfig, PlanarNavNeighborPolicy, VolumetricAgentVolume, VolumetricNavConfig,
    VolumetricNavError, VolumetricNavOutcome, VolumetricNavQuery, VolumetricNeighborSet,
    VolumetricTraversalRule, VolumetricVerticalPolicy,
};

use crate::composition::{
    borrowed_slice, borrowed_utf8, native_quat, native_quat_value, native_vec3, native_vec3_value,
    CsharpEngineServicesError, ABI_OK,
};

const MAX_SPATIAL_QUERY_ENTITIES: usize = engine_spatial::MAX_OCCLUSION_QUERY_ENTITIES;
const MAX_SPATIAL_QUERY_IGNORED_ENTITIES: usize = engine_spatial::MAX_OCCLUSION_IGNORED_ENTITIES;
const SPATIAL_SERVICE: &[u8] = b"Spatial";
const REGISTER_TRIGGER_OPERATION: &[u8] = b"RegisterTrigger";
const RECONCILE_TRIGGERS_OPERATION: &[u8] = b"ReconcileTriggers";
const MAX_TRIGGER_OPERATION_DIAGNOSTICS: usize = 64;
const MAX_TRIGGER_DIAGNOSTIC_TEXT_BYTES: usize = 512;

/// Engine-owned collision/navigation mechanisms. Player and game state never
/// live here: a character proposal builds its EntityState only for the call.
pub(crate) struct RuntimeSpatialBridge {
    sessions: BTreeMap<u64, SpatialSession>,
    pub(crate) voxel_history_exports: BTreeMap<u64, Arc<[u8]>>,
    trigger_diagnostic_leases: BTreeMap<u64, SpatialTriggerDiagnosticLease>,
    collision_source: SpatialCollisionSource,
    next_session: u64,
    pub(crate) next_voxel_history_export: u64,
    next_trigger_diagnostic_lease: u64,
}

pub(crate) struct SpatialSession {
    pub(crate) scene: Arc<VoxelCollisionScene>,
    pub(crate) voxel_history: engine_spatial::VoxelEditHistory,
    pub(crate) voxel_leases: engine_spatial::VoxelChunkLeaseRegistry,
    pub(crate) last_voxel_dirty_chunks: Vec<[i64; 3]>,
    navigation: Option<NavigationState>,
    navigation_revision: u64,
    controller: CharacterControllerService,
    last_character_receipt: Option<CharacterControllerReceipt>,
    triggers: TriggerVolumeSystem,
    last_trigger_facts: Vec<TriggerOverlapFact>,
}

/// Trigger failures retain their original diagnostics until generated C# has
/// copied them and called the named Spatial release operation. This keeps the
/// stable code/message/entity correlation intact at the ABI boundary.
struct SpatialTriggerDiagnosticLease {
    _values: Vec<SpatialTriggerDiagnosticValue>,
    diagnostics: Box<[NativeEngineDiagnostic]>,
}

struct SpatialTriggerDiagnosticValue {
    code: String,
    message: String,
    source: String,
}

enum SpatialTriggerOperationError {
    Service(CsharpEngineServicesError),
    Trigger(TriggerVolumeError),
}

impl SpatialTriggerDiagnosticLease {
    fn new(values: Vec<SpatialTriggerDiagnosticValue>) -> Option<Self> {
        (!values.is_empty()).then(|| {
            let diagnostics = values
                .iter()
                .map(|value| NativeEngineDiagnostic {
                    code: native_utf8(value.code.as_bytes()),
                    message: native_utf8(value.message.as_bytes()),
                    source: native_utf8(value.source.as_bytes()),
                })
                .collect();
            Self {
                _values: values,
                diagnostics,
            }
        })
    }
}

/// A retained navigation source is Engine-owned state. C# only borrows typed
/// cells for replacement calls and observes bounded query facts; it never owns
/// a pathfinding graph or a voxel world.
enum NavigationSource {
    HostWalkableCells,
    VoxelDerived(VoxelCollisionScene),
}

struct NavigationState {
    source: NavigationSource,
    projection: NavProjection,
    policy: PlanarNavNeighborPolicy,
    agent_height_voxels: u32,
    require_solid_floor: bool,
    revision: u64,
    last_path: Vec<VoxelCoord>,
}

impl NavigationState {
    fn kind(&self) -> NativeNavigationProjectionKind {
        match &self.source {
            NavigationSource::HostWalkableCells => {
                NativeNavigationProjectionKind::HostWalkableCells
            }
            NavigationSource::VoxelDerived(_) => NativeNavigationProjectionKind::VoxelDerived,
        }
    }

    fn voxel_world(&self) -> Option<&svc_spatial::VoxelWorld> {
        match &self.source {
            NavigationSource::HostWalkableCells => None,
            NavigationSource::VoxelDerived(scene) => Some(scene.voxel_world()),
        }
    }
}

/// Private Engine wiring between the named Spatial and Dynamics families.
/// Product code can only pass the typed session owner through the generated
/// binding request; it never retains or observes this projection directly.
#[derive(Clone)]
pub(crate) struct SpatialCollisionSource {
    scenes: Rc<RefCell<BTreeMap<u64, Arc<VoxelCollisionScene>>>>,
}

impl SpatialCollisionSource {
    fn new() -> Self {
        Self {
            scenes: Rc::new(RefCell::new(BTreeMap::new())),
        }
    }

    pub(crate) fn scene(
        &self,
        handle: NativeSpatialSessionHandle,
    ) -> Result<Arc<VoxelCollisionScene>, CsharpEngineServicesError> {
        self.scenes
            .borrow()
            .get(&handle.value)
            .cloned()
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_SPATIAL_SESSION",
                    "C# used an unknown or disposed spatial session",
                )
            })
    }

    pub(crate) fn publish_scene(
        &self,
        handle: NativeSpatialSessionHandle,
        scene: Arc<VoxelCollisionScene>,
    ) {
        self.scenes.borrow_mut().insert(handle.value, scene);
    }
}

impl RuntimeSpatialBridge {
    pub(crate) fn new() -> Self {
        Self {
            sessions: BTreeMap::new(),
            voxel_history_exports: BTreeMap::new(),
            trigger_diagnostic_leases: BTreeMap::new(),
            collision_source: SpatialCollisionSource::new(),
            next_session: 1,
            next_voxel_history_export: 1,
            next_trigger_diagnostic_lease: 1,
        }
    }

    pub(crate) fn collision_source(&self) -> SpatialCollisionSource {
        self.collision_source.clone()
    }

    pub(crate) fn publish_scene(
        &self,
        handle: NativeSpatialSessionHandle,
        scene: Arc<VoxelCollisionScene>,
    ) {
        self.collision_source.publish_scene(handle, scene);
    }

    pub(crate) fn session_mut(
        &mut self,
        handle: NativeSpatialSessionHandle,
    ) -> Result<&mut SpatialSession, CsharpEngineServicesError> {
        self.sessions.get_mut(&handle.value).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_SPATIAL_SESSION",
                "C# used an unknown spatial session",
            )
        })
    }

    fn create(
        &mut self,
        config: NativeSpatialSessionConfig,
    ) -> Result<NativeSpatialSessionHandle, CsharpEngineServicesError> {
        let scene = VoxelCollisionScene::from_solid_voxels(
            config.collision_voxel_size,
            config.collision_chunk_size,
            std::iter::empty(),
        )
        .map_err(|error| {
            CsharpEngineServicesError::new("CSHARP_SPATIAL_CREATE", error.to_string())
        })?;
        let value = self.next_session;
        self.next_session = self.next_session.checked_add(1).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_SPATIAL_SESSION",
                "spatial session handles exhausted",
            )
        })?;
        let scene = Arc::new(scene);
        self.sessions.insert(
            value,
            SpatialSession {
                voxel_history: engine_spatial::VoxelEditHistory::new(&scene),
                voxel_leases: engine_spatial::VoxelChunkLeaseRegistry::default(),
                last_voxel_dirty_chunks: Vec::new(),
                scene: Arc::clone(&scene),
                navigation: None,
                navigation_revision: 0,
                controller: CharacterControllerService::default(),
                last_character_receipt: None,
                triggers: TriggerVolumeSystem::default(),
                last_trigger_facts: Vec::new(),
            },
        );
        self.collision_source
            .scenes
            .borrow_mut()
            .insert(value, scene);
        Ok(NativeSpatialSessionHandle { value })
    }

    fn replace_collision(
        &mut self,
        request: &NativeCollisionReplaceRequest,
    ) -> Result<NativeCollisionReplaceReceipt, CsharpEngineServicesError> {
        let assets =
            unsafe { borrowed_slice(request.assets, request.assets_len, "collision assets") }?;
        let vertices = unsafe {
            borrowed_slice(request.vertices, request.vertices_len, "collision vertices")
        }?;
        let triangles = unsafe {
            borrowed_slice(
                request.triangles,
                request.triangles_len,
                "collision triangles",
            )
        }?;
        let instances = unsafe {
            borrowed_slice(
                request.instances,
                request.instances_len,
                "collision instances",
            )
        }?;
        let mut admitted = Vec::with_capacity(assets.len());
        for asset in assets {
            let positions = range_slice(
                vertices,
                asset.first_vertex,
                asset.vertex_count,
                "asset vertices",
            )?
            .iter()
            .map(|value| [f64::from(value.x), f64::from(value.y), f64::from(value.z)])
            .collect::<Vec<_>>();
            let triangles = range_slice(
                triangles,
                asset.first_triangle,
                asset.triangle_count,
                "asset triangles",
            )?
            .iter()
            .map(|value| [value.a, value.b, value.c])
            .collect::<Vec<_>>();
            admitted.push(
                StaticMeshColliderAsset::new(StaticMeshAssetId(asset.id), positions, triangles)
                    .map_err(|error| {
                        CsharpEngineServicesError::new(
                            "CSHARP_COLLISION_ASSET",
                            format!("{error:?}"),
                        )
                    })?,
            );
        }
        let geometry = admitted
            .iter()
            .map(|asset| (asset.id, asset.geometry_hash))
            .collect::<BTreeMap<_, _>>();
        let instances = instances
            .iter()
            .map(|instance| {
                let asset = StaticMeshAssetId(instance.asset);
                let expected_geometry_hash = *geometry.get(&asset).ok_or_else(|| {
                    CsharpEngineServicesError::new(
                        "CSHARP_COLLISION_INSTANCE",
                        "instance referenced an unavailable asset",
                    )
                })?;
                Ok(StaticMeshColliderInstance {
                    id: StaticMeshInstanceId(instance.id),
                    asset,
                    expected_geometry_hash,
                    transform: static_mesh_transform(instance.transform),
                })
            })
            .collect::<Result<Vec<_>, CsharpEngineServicesError>>()?;
        let (scene, receipt) = {
            let session = self.session_mut(request.session)?;
            let mut candidate = (*session.scene).clone();
            let receipt = candidate
                .replace_static_mesh_colliders(
                    candidate.static_mesh_collision_revision(),
                    admitted,
                    instances,
                )
                .map_err(|error| {
                    CsharpEngineServicesError::new("CSHARP_COLLISION_REPLACE", format!("{error:?}"))
                })?;
            let candidate = Arc::new(candidate);
            session.scene = Arc::clone(&candidate);
            (candidate, receipt)
        };
        self.collision_source
            .scenes
            .borrow_mut()
            .insert(request.session.value, scene);
        Ok(NativeCollisionReplaceReceipt {
            revision_before: receipt.revision_before,
            revision_after: receipt.revision_after,
            asset_count: receipt.asset_count as u64,
            instance_count: receipt.instance_count as u64,
            projection_hash: receipt.projection_hash,
        })
    }

    fn replace_navigation(
        &mut self,
        request: &NativeNavigationReplaceRequest,
    ) -> Result<NativeNavigationReplaceReceipt, CsharpEngineServicesError> {
        let cells =
            unsafe { borrowed_slice(request.cells, request.cells_len, "navigation cells") }?;
        let dimensions = ChunkDims::cubic(request.config.chunk_size).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_NAVIGATION_CONFIG",
                "navigation chunk size was zero",
            )
        })?;
        let grid_id = u32::try_from(request.config.grid_id).map_err(|_| {
            CsharpEngineServicesError::new(
                "CSHARP_NAVIGATION_CONFIG",
                "navigation grid id exceeded u32",
            )
        })?;
        let grid = VoxelGridSpec::new(GridId::new(grid_id), request.config.cell_size, dimensions)
            .ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_NAVIGATION_CONFIG",
                "navigation cell size was invalid",
            )
        })?;
        let projection = NavProjection::from_walkable_cells(
            grid,
            cells
                .iter()
                .map(|cell| VoxelCoord::new(cell.x, cell.y, cell.z)),
        );
        let policy = PlanarNavNeighborPolicy {
            max_step_cells: u8::try_from(request.config.max_step_cells).map_err(|_| {
                CsharpEngineServicesError::new(
                    "CSHARP_NAVIGATION_CONFIG",
                    "maximum navigation step exceeded u8",
                )
            })?,
        };
        let session = self.session_mut(request.session)?;
        session.navigation_revision = next_navigation_revision(session.navigation_revision)?;
        let navigation_revision = session.navigation_revision;
        let receipt = NativeNavigationReplaceReceipt {
            walkable_cell_count: projection.walkable_len() as u64,
            projection_hash: projection.projection_hash(),
            navigation_revision,
        };
        session.navigation = Some(NavigationState {
            source: NavigationSource::HostWalkableCells,
            projection,
            policy,
            agent_height_voxels: 0,
            require_solid_floor: false,
            revision: navigation_revision,
            last_path: Vec::new(),
        });
        Ok(receipt)
    }

    fn replace_voxel_navigation(
        &mut self,
        request: &NativeNavigationVoxelReplaceRequest,
    ) -> Result<NativeNavigationReplaceReceipt, CsharpEngineServicesError> {
        let solids = unsafe {
            borrowed_slice(
                request.solid_cells,
                request.solid_cells_len,
                "navigation solid cells",
            )
        }?;
        navigation_grid(request.config)?;
        let max_step_cells = u8::try_from(request.config.max_step_cells).map_err(|_| {
            CsharpEngineServicesError::new(
                "CSHARP_NAVIGATION_CONFIG",
                "maximum navigation step exceeded u8",
            )
        })?;
        if request.agent_height_voxels == 0 {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_NAVIGATION_CONFIG",
                "agent height must be non-zero",
            ));
        }
        let scene = VoxelCollisionScene::from_solid_voxels(
            request.config.cell_size,
            request.config.chunk_size,
            solids.iter().map(|cell| [cell.x, cell.y, cell.z]),
        )
        .map_err(|error| {
            CsharpEngineServicesError::new("CSHARP_NAVIGATION_VOXELS", error.to_string())
        })?;
        let projection = build_nav_projection(
            scene.voxel_world(),
            NavProjectionConfig {
                agent_height_voxels: request.agent_height_voxels,
                require_solid_floor: request.require_solid_floor,
            },
        )
        .map_err(|error| {
            CsharpEngineServicesError::new("CSHARP_NAVIGATION_CONFIG", error.label())
        })?;
        let session = self.session_mut(request.session)?;
        session.navigation_revision = next_navigation_revision(session.navigation_revision)?;
        let navigation_revision = session.navigation_revision;
        let receipt = NativeNavigationReplaceReceipt {
            walkable_cell_count: projection.walkable_len() as u64,
            projection_hash: projection.projection_hash(),
            navigation_revision,
        };
        session.navigation = Some(NavigationState {
            source: NavigationSource::VoxelDerived(scene),
            projection,
            policy: PlanarNavNeighborPolicy { max_step_cells },
            agent_height_voxels: request.agent_height_voxels,
            require_solid_floor: request.require_solid_floor,
            revision: navigation_revision,
            last_path: Vec::new(),
        });
        Ok(receipt)
    }

    fn read_navigation_projection(
        &mut self,
        request: NativeNavigationProjectionReadRequest,
    ) -> Result<NativeNavigationProjectionReadout, CsharpEngineServicesError> {
        let session = self.session_mut(request.session)?;
        let Some(navigation) = session.navigation.as_ref() else {
            return Ok(NativeNavigationProjectionReadout {
                navigation_revision: session.navigation_revision,
                ..Default::default()
            });
        };
        Ok(NativeNavigationProjectionReadout {
            present: true,
            kind: navigation.kind(),
            walkable_cell_count: navigation.projection.walkable_len() as u64,
            projection_hash: navigation.projection.projection_hash(),
            navigation_revision: navigation.revision,
            agent_height_voxels: navigation.agent_height_voxels,
            require_solid_floor: navigation.require_solid_floor,
            max_step_cells: u32::from(navigation.policy.max_step_cells),
        })
    }

    fn request_navigation_path(
        &mut self,
        request: NativeNavigationPathRequest,
    ) -> Result<NativeNavigationPathReadout, CsharpEngineServicesError> {
        let session = self.session_mut(request.session)?;
        let Some(navigation) = session.navigation.as_mut() else {
            return Ok(NativeNavigationPathReadout {
                outcome: NativeNavigationPathOutcome::ProjectionUnavailable,
                ..Default::default()
            });
        };
        let result = find_path_with_policy(
            &navigation.projection,
            NavPathQuery {
                start: nav_cell(request.start),
                goal: nav_cell(request.goal),
                max_visited: request.max_visited as usize,
            },
            navigation.policy,
        );
        let (outcome, visited, path, path_hash) = match result {
            Ok(result) => (
                match result.outcome {
                    NavPathOutcome::Reached => NativeNavigationPathOutcome::Reached,
                    NavPathOutcome::NoPath => NativeNavigationPathOutcome::NoPath,
                },
                result.visited,
                result.path,
                result.path_hash,
            ),
            Err(error) => (navigation_outcome(error), 0, Vec::new(), 0),
        };
        navigation.last_path = path;
        Ok(NativeNavigationPathReadout {
            outcome,
            kind: navigation.kind(),
            visited: u32::try_from(visited).unwrap_or(u32::MAX),
            path_len: u32::try_from(navigation.last_path.len()).unwrap_or(u32::MAX),
            navigation_revision: navigation.revision,
            projection_hash: navigation.projection.projection_hash(),
            path_hash,
        })
    }

    fn request_volumetric_navigation_path(
        &mut self,
        request: NativeNavigationVolumetricPathRequest,
    ) -> Result<NativeNavigationPathReadout, CsharpEngineServicesError> {
        let session = self.session_mut(request.session)?;
        let Some(navigation) = session.navigation.as_mut() else {
            return Ok(NativeNavigationPathReadout {
                outcome: NativeNavigationPathOutcome::ProjectionUnavailable,
                ..Default::default()
            });
        };
        let kind = navigation.kind();
        let revision = navigation.revision;
        let projection_hash = navigation.projection.projection_hash();
        let Some(world) = navigation.voxel_world() else {
            navigation.last_path.clear();
            return Ok(NativeNavigationPathReadout {
                outcome: NativeNavigationPathOutcome::ProjectionUnavailable,
                kind,
                navigation_revision: revision,
                projection_hash,
                ..Default::default()
            });
        };
        let result = find_volumetric_path(
            world,
            VolumetricNavQuery {
                start: nav_cell(request.start),
                goal: nav_cell(request.goal),
                max_visited: request.max_visited as usize,
                config: volumetric_config(request.config),
            },
        );
        let (outcome, visited, path, path_hash) = match result {
            Ok(result) => (
                match result.outcome {
                    VolumetricNavOutcome::Reached => NativeNavigationPathOutcome::Reached,
                    VolumetricNavOutcome::NoPath => NativeNavigationPathOutcome::NoPath,
                    VolumetricNavOutcome::BudgetExhausted => {
                        NativeNavigationPathOutcome::BudgetExhausted
                    }
                },
                result.visited,
                result.path,
                result.path_hash,
            ),
            Err(error) => (volumetric_navigation_outcome(error), 0, Vec::new(), 0),
        };
        navigation.last_path = path;
        Ok(NativeNavigationPathReadout {
            outcome,
            kind,
            visited: u32::try_from(visited).unwrap_or(u32::MAX),
            path_len: u32::try_from(navigation.last_path.len()).unwrap_or(u32::MAX),
            navigation_revision: revision,
            projection_hash,
            path_hash,
        })
    }

    fn read_navigation_path_cell_at(
        &mut self,
        request: NativeNavigationPathCellAtRequest,
    ) -> Result<NativeNavigationPathCellAtReceipt, CsharpEngineServicesError> {
        let session = self.session_mut(request.session)?;
        let Some(cell) = session
            .navigation
            .as_ref()
            .and_then(|navigation| navigation.last_path.get(request.index as usize))
            .copied()
        else {
            return Ok(NativeNavigationPathCellAtReceipt::default());
        };
        Ok(NativeNavigationPathCellAtReceipt {
            present: true,
            cell: native_nav_cell(cell),
        })
    }

    fn clear_navigation(
        &mut self,
        request: NativeNavigationClearRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let session = self.session_mut(request.session)?;
        session.navigation_revision = next_navigation_revision(session.navigation_revision)?;
        session.navigation = None;
        Ok(())
    }

    fn propose_navigation(
        &mut self,
        request: NativeNavigationStepRequest,
    ) -> Result<NativeNavigationStepReceipt, CsharpEngineServicesError> {
        let session = self.session_mut(request.session)?;
        let Some(navigation) = session.navigation.as_mut() else {
            return Ok(NativeNavigationStepReceipt {
                outcome: NativeNavigationPathOutcome::ProjectionUnavailable,
                ..Default::default()
            });
        };
        let from = native_vec3_value(request.from);
        let target = native_vec3_value(request.target);
        if !finite_vec3(from) || !finite_vec3(target) {
            return Ok(navigation_step_failure(
                navigation,
                NativeNavigationPathOutcome::NonFinitePosition,
            ));
        }
        let start = navigation
            .projection
            .grid()
            .world_to_voxel(core_space::WorldPos::new(
                f64::from(from.x),
                f64::from(from.y),
                f64::from(from.z),
            ));
        let goal = navigation
            .projection
            .grid()
            .world_to_voxel(core_space::WorldPos::new(
                f64::from(target.x),
                f64::from(target.y),
                f64::from(target.z),
            ));
        let path = find_path_with_policy(
            &navigation.projection,
            NavPathQuery {
                start,
                goal,
                max_visited: request.max_visited as usize,
            },
            navigation.policy,
        );
        let path = match path {
            Ok(path) => path,
            Err(error) => {
                return Ok(navigation_step_failure(
                    navigation,
                    navigation_outcome(error),
                ))
            }
        };
        if path.outcome == NavPathOutcome::NoPath {
            navigation.last_path.clear();
            return Ok(navigation_step_failure(
                navigation,
                NativeNavigationPathOutcome::NoPath,
            ));
        }
        let next_cell = path.path.get(1).copied().unwrap_or(start);
        let step_target = if next_cell == goal {
            target
        } else {
            let center = navigation.projection.grid().voxel_center_world(next_cell);
            Vec3::new(center.x as f32, center.y as f32, center.z as f32)
        };
        let movement = propose_direct_nav_movement(DirectNavMovementRequest {
            from,
            target: step_target,
            max_step_units: request.max_step_units,
        });
        let movement = match movement {
            Ok(movement) => movement,
            Err(error) => {
                let outcome = match error.label() {
                    "nonFinitePosition" => NativeNavigationPathOutcome::NonFinitePosition,
                    _ => NativeNavigationPathOutcome::InvalidStep,
                };
                return Ok(navigation_step_failure(navigation, outcome));
            }
        };
        navigation.last_path = path.path;
        Ok(NativeNavigationStepReceipt {
            outcome: NativeNavigationPathOutcome::Reached,
            next_waypoint: native_vec3(movement.next_waypoint),
            next_path_cell: native_nav_cell(next_cell),
            reached: u32::from(next_cell == goal && movement.reached),
            visited: path.visited as u32,
            path_len: navigation.last_path.len() as u32,
            navigation_revision: navigation.revision,
            projection_hash: navigation.projection.projection_hash(),
            path_hash: path.path_hash,
        })
    }

    fn propose_character(
        &mut self,
        request: NativeCharacterStepRequest,
    ) -> Result<NativeCharacterStepReceipt, CsharpEngineServicesError> {
        let session = self.session_mut(request.session)?;
        let position = native_vec3_value(request.position);
        let motion = character_motion(request.motion)?;
        let player = EntityDefinition::new(EntityId::new(1), "spatial-proposal")
            .with_full_transform(EntityTransform::at(position))
            .with_character_motion(motion);
        let support = character_support_definition(request.motion, request.support)?;
        let mut definitions = vec![player];
        if let Some(definition) = support.as_ref() {
            definitions.push(definition.clone());
        }
        let mut entities = EntityState::from_definitions(definitions).map_err(|error| {
            CsharpEngineServicesError::new("CSHARP_CHARACTER_STATE", error.to_string())
        })?;
        if let Some(support) = support {
            apply_support_lifecycle(&mut entities, support.id, request.support.lifecycle)?;
        }
        let receipt = session
            .controller
            .step(
                &mut entities,
                &session.scene,
                EntityId::new(1),
                &character_config(request.config)?,
                character_command(request.command),
            )
            .map_err(|error| {
                CsharpEngineServicesError::new("CSHARP_CHARACTER_STEP", error.code())
            })?;
        session.last_character_receipt = Some(receipt.clone());
        Ok(native_character_receipt(&receipt))
    }

    fn default_character_controller_config(&self) -> NativeCharacterControllerConfig {
        native_character_config(CharacterControllerConfig::responsive_fps())
    }

    fn read_character_controller(
        &mut self,
        request: NativeCharacterControllerReadRequest,
    ) -> Result<NativeCharacterControllerReadout, CsharpEngineServicesError> {
        let session = self.session_mut(request.session)?;
        let Some(readout) = session.controller.readout() else {
            return Ok(NativeCharacterControllerReadout::default());
        };
        let receipt = session.last_character_receipt.as_ref();
        Ok(NativeCharacterControllerReadout {
            present: true,
            generation: readout.generation,
            entity: readout.entity.raw(),
            command_sequence: readout.command_sequence,
            grounded: readout.grounded,
            contact_count: readout.contact_count as u32,
            block_count: receipt.map_or(0, |value| value.blocks.len() as u32),
            dynamic_impulse_count: receipt.map_or(0, |value| value.dynamic_impulses.len() as u32),
            collision_world_hash: readout.collision_world_hash,
            recovery_distance: receipt.map_or(0.0, |value| value.recovery_distance),
        })
    }

    fn read_character_contact_at(
        &mut self,
        request: NativeCharacterContactAtRequest,
    ) -> Result<NativeCharacterContactAtReceipt, CsharpEngineServicesError> {
        let session = self.session_mut(request.session)?;
        Ok(session
            .last_character_receipt
            .as_ref()
            .and_then(|receipt| receipt.contacts.get(request.index as usize))
            .map(|contact| NativeCharacterContactAtReceipt {
                present: true,
                contact: native_character_contact(*contact),
            })
            .unwrap_or_default())
    }

    fn read_character_dynamic_impulse_at(
        &mut self,
        request: NativeCharacterDynamicImpulseAtRequest,
    ) -> Result<NativeCharacterDynamicImpulseAtReceipt, CsharpEngineServicesError> {
        let session = self.session_mut(request.session)?;
        Ok(session
            .last_character_receipt
            .as_ref()
            .and_then(|receipt| receipt.dynamic_impulses.get(request.index as usize))
            .map(|proposal| NativeCharacterDynamicImpulseAtReceipt {
                present: true,
                proposal: NativeCharacterDynamicImpulse {
                    entity: proposal.entity.raw(),
                    point: native_vec3(proposal.point),
                    impulse: native_vec3(proposal.impulse),
                },
            })
            .unwrap_or_default())
    }

    fn read_projection(
        &mut self,
        request: NativeSpatialProjectionReadRequest,
    ) -> Result<NativeSpatialProjectionReadout, CsharpEngineServicesError> {
        let scene = self.session_mut(request.session)?.scene.as_ref();
        Ok(NativeSpatialProjectionReadout {
            source_revision: scene.source_revision().raw(),
            collision_revision: scene.static_mesh_collision_revision(),
            projection_version: scene.projection_version(),
            authority_hash: scene.authority_hash(),
            resident_chunk_count: scene.resident_chunk_count() as u64,
            collider_chunk_count: scene.collider_chunk_count() as u64,
            static_mesh_revision: scene.static_mesh_collision_revision(),
            static_mesh_asset_count: scene.projection_static_mesh_asset_count() as u64,
            static_mesh_instance_count: scene.projection_static_mesh_instance_count() as u64,
        })
    }

    fn contains_point(
        &mut self,
        request: NativeSpatialContainsPointRequest,
    ) -> Result<NativeSpatialQueryReceipt, CsharpEngineServicesError> {
        let point = native_vec3_value(request.point);
        if !finite_vec3(point) {
            return Err(spatial_error(
                "CSHARP_SPATIAL_POINT",
                "point was not finite",
            ));
        }
        let scene = self.session_mut(request.session)?.scene.as_ref();
        Ok(query_receipt(
            scene,
            scene.contains_point([f64::from(point.x), f64::from(point.y), f64::from(point.z)]),
            false,
            0,
        ))
    }

    fn cast_ray(
        &mut self,
        request: &NativeSpatialRaycastRequest,
    ) -> Result<NativeSpatialHit, CsharpEngineServicesError> {
        let entities = unsafe {
            borrowed_slice(
                request.entities,
                request.entities_len,
                "spatial query entities",
            )
        }?;
        let ignored = unsafe {
            borrowed_slice(
                request.ignored_entities,
                request.ignored_entities_len,
                "spatial ignored entities",
            )
        }?;
        let overrides = unsafe {
            borrowed_slice(
                request.hitbox_overrides,
                request.hitbox_overrides_len,
                "spatial hitbox overrides",
            )
        }?;
        let scene = self.session_mut(request.session)?.scene.clone();
        cast_ray_parts(
            scene.as_ref(),
            request.origin,
            request.direction,
            request.max_distance,
            request.filter,
            entities,
            ignored,
            overrides,
        )
    }

    fn cast_segment(
        &mut self,
        request: &NativeSpatialSegmentCastRequest,
    ) -> Result<NativeSpatialHit, CsharpEngineServicesError> {
        let start = native_vec3_value(request.start);
        let end = native_vec3_value(request.end);
        let delta = end - start;
        let max_distance = f64::from(delta.length());
        let entities = unsafe {
            borrowed_slice(
                request.entities,
                request.entities_len,
                "spatial query entities",
            )
        }?;
        let ignored = unsafe {
            borrowed_slice(
                request.ignored_entities,
                request.ignored_entities_len,
                "spatial ignored entities",
            )
        }?;
        let overrides = unsafe {
            borrowed_slice(
                request.hitbox_overrides,
                request.hitbox_overrides_len,
                "spatial hitbox overrides",
            )
        }?;
        let scene = self.session_mut(request.session)?.scene.clone();
        let direction = NativeVec3 {
            x: request.end.x - request.start.x,
            y: request.end.y - request.start.y,
            z: request.end.z - request.start.z,
        };
        cast_ray_parts(
            scene.as_ref(),
            request.start,
            direction,
            max_distance,
            request.filter,
            entities,
            ignored,
            overrides,
        )
    }

    fn overlap_aabb(
        &mut self,
        request: &NativeSpatialAabbQueryRequest,
    ) -> Result<NativeSpatialQueryReceipt, CsharpEngineServicesError> {
        self.aabb_query(request, false)
    }

    fn sweep_aabb(
        &mut self,
        request: &NativeSpatialAabbQueryRequest,
    ) -> Result<NativeSpatialQueryReceipt, CsharpEngineServicesError> {
        self.aabb_query(request, true)
    }

    fn aabb_query(
        &mut self,
        request: &NativeSpatialAabbQueryRequest,
        sweep: bool,
    ) -> Result<NativeSpatialQueryReceipt, CsharpEngineServicesError> {
        let min = native_vec3_value(request.min);
        let max = native_vec3_value(request.max);
        let translation = native_vec3_value(request.translation);
        validate_aabb(min, max)?;
        if !finite_vec3(translation) {
            return Err(spatial_error(
                "CSHARP_SPATIAL_SWEEP",
                "translation was not finite",
            ));
        }
        let entities = unsafe {
            borrowed_slice(
                request.entities,
                request.entities_len,
                "spatial query entities",
            )
        }?;
        let ignored = unsafe {
            borrowed_slice(
                request.ignored_entities,
                request.ignored_entities_len,
                "spatial ignored entities",
            )
        }?;
        let scene = self.session_mut(request.session)?.scene.clone();
        let ignored = ignored_set(ignored)?;
        let records = filtered_entities(entities, request.filter, &ignored)?;
        let (min, max) = (
            [f64::from(min.x), f64::from(min.y), f64::from(min.z)],
            [f64::from(max.x), f64::from(max.y), f64::from(max.z)],
        );
        let voxel_or_mesh = if sweep {
            scene.axis_sweep_overlaps(
                min,
                max,
                [
                    f64::from(translation.x),
                    f64::from(translation.y),
                    f64::from(translation.z),
                ],
            )
        } else {
            scene.aabb_overlaps_solid(min, max)
        };
        let entity_hit = records.iter().any(|record| {
            let (record_min, record_max) = collider_bounds(*record);
            if sweep {
                swept_aabb_overlaps(
                    min,
                    max,
                    [
                        f64::from(translation.x),
                        f64::from(translation.y),
                        f64::from(translation.z),
                    ],
                    record_min,
                    record_max,
                )
            } else {
                aabb_overlaps(min, max, record_min, record_max)
            }
        });
        let hit = voxel_or_mesh || entity_hit;
        Ok(query_receipt(
            scene.as_ref(),
            hit,
            sweep && hit,
            u32::from(hit),
        ))
    }

    fn cast_capsule(
        &mut self,
        request: &NativeSpatialCapsuleQueryRequest,
    ) -> Result<NativeSpatialHit, CsharpEngineServicesError> {
        self.capsule_query(request, false)
    }

    fn overlap_capsule(
        &mut self,
        request: &NativeSpatialCapsuleQueryRequest,
    ) -> Result<NativeSpatialHit, CsharpEngineServicesError> {
        self.capsule_query(request, true)
    }

    fn capsule_query(
        &mut self,
        request: &NativeSpatialCapsuleQueryRequest,
        overlap: bool,
    ) -> Result<NativeSpatialHit, CsharpEngineServicesError> {
        let center = native_vec3_value(request.center);
        let translation = native_vec3_value(request.translation);
        if !finite_vec3(center) || !finite_vec3(translation) {
            return Err(spatial_error(
                "CSHARP_SPATIAL_CAPSULE",
                "capsule position or translation was not finite",
            ));
        }
        let entities = unsafe {
            borrowed_slice(
                request.entities,
                request.entities_len,
                "spatial query entities",
            )
        }?;
        let ignored = unsafe {
            borrowed_slice(
                request.ignored_entities,
                request.ignored_entities_len,
                "spatial ignored entities",
            )
        }?;
        let scene = self.session_mut(request.session)?.scene.clone();
        let ignored = ignored_set(ignored)?;
        let records = filtered_entities(entities, request.filter, &ignored)?;
        let obstacles = records
            .iter()
            .copied()
            .map(character_obstacle)
            .collect::<Result<Vec<_>, _>>()?;
        let capsule = CharacterCapsule {
            center: core_space::WorldPos::new(
                f64::from(center.x),
                f64::from(center.y),
                f64::from(center.z),
            ),
            half_height: request.half_height,
            radius: request.radius,
        };
        if overlap {
            let scene_hit = scene
                .character_capsule_overlap(capsule)
                .map_err(|error| spatial_error("CSHARP_SPATIAL_CAPSULE", format!("{error:?}")))?;
            let entity_hit = engine_spatial::character_capsule_overlap_obstacles(
                capsule, &obstacles,
            )
            .map_err(|error| spatial_error("CSHARP_SPATIAL_CAPSULE", format!("{error:?}")))?;
            Ok(nearest_capsule_overlap(scene_hit, entity_hit))
        } else {
            let scene_hit = scene
                .cast_character_capsule(
                    capsule,
                    core_space::WorldVec::new(
                        f64::from(translation.x),
                        f64::from(translation.y),
                        f64::from(translation.z),
                    ),
                    request.contact_skin,
                )
                .map_err(|error| spatial_error("CSHARP_SPATIAL_CAPSULE", format!("{error:?}")))?;
            let entity_hit = engine_spatial::cast_character_capsule_against_obstacles(
                capsule,
                core_space::WorldVec::new(
                    f64::from(translation.x),
                    f64::from(translation.y),
                    f64::from(translation.z),
                ),
                request.contact_skin,
                &obstacles,
            )
            .map_err(|error| spatial_error("CSHARP_SPATIAL_CAPSULE", format!("{error:?}")))?;
            Ok(nearest_capsule_cast(scene_hit, entity_hit))
        }
    }

    fn pick_voxel(
        &mut self,
        request: NativeSpatialPickRequest,
    ) -> Result<NativeSpatialHit, CsharpEngineServicesError> {
        let scene = self.session_mut(request.session)?.scene.clone();
        let anchor = VoxelPickService::validate(
            scene.as_ref(),
            VoxelPickHint {
                origin: native_array(request.origin),
                direction: native_array(request.direction),
                max_distance: request.max_distance,
                claimed_voxel: [
                    request.claimed_voxel_x,
                    request.claimed_voxel_y,
                    request.claimed_voxel_z,
                ],
                claimed_face: native_face(request.claimed_face)?,
            },
        )
        .map_err(|error| spatial_error("CSHARP_SPATIAL_PICK", format!("{error:?}")))?;
        Ok(NativeSpatialHit {
            present: true,
            kind: NativeSpatialHitKind::Voxel,
            voxel_x: anchor.hit_voxel[0],
            voxel_y: anchor.hit_voxel[1],
            voxel_z: anchor.hit_voxel[2],
            face: native_face_value(anchor.hit_face),
            point: native_f64_vec3(anchor.point),
            distance: anchor.distance,
            ..Default::default()
        })
    }

    fn register_trigger(
        &mut self,
        request: &NativeSpatialTriggerRegisterRequest,
    ) -> Result<(), SpatialTriggerOperationError> {
        let scope =
            unsafe { borrowed_utf8(request.scope.bytes, request.scope.len, "trigger scope") }
                .map_err(SpatialTriggerOperationError::Service)?;
        let tag = unsafe { borrowed_utf8(request.tag.bytes, request.tag.len, "trigger tag") }
            .map_err(SpatialTriggerOperationError::Service)?;
        let geometry = match request.geometry {
            NativeSpatialTriggerGeometry::ActiveCollision => TriggerGeometrySource::ActiveCollision,
            NativeSpatialTriggerGeometry::EntityBounds => TriggerGeometrySource::EntityBounds,
        };
        let tags = (!tag.is_empty()).then_some(tag).into_iter();
        self.session_mut(request.session)
            .map_err(SpatialTriggerOperationError::Service)?
            .triggers
            .register(
                engine_spatial::KinematicTriggerDefinition::new(
                    EntityId::new(request.trigger),
                    scope,
                    tags,
                )
                .with_geometry_source(geometry),
            )
            .map_err(SpatialTriggerOperationError::Trigger)
    }

    fn reconcile_triggers(
        &mut self,
        request: &NativeSpatialTriggerReconcileRequest,
    ) -> Result<NativeSpatialTriggerReceipt, SpatialTriggerOperationError> {
        let entities =
            unsafe { borrowed_slice(request.entities, request.entities_len, "trigger entities") }
                .map_err(SpatialTriggerOperationError::Service)?;
        let state = entity_state(entities).map_err(SpatialTriggerOperationError::Service)?;
        let cause = native_trigger_cause(request.cause);
        let session = self
            .session_mut(request.session)
            .map_err(SpatialTriggerOperationError::Service)?;
        let receipt = session
            .triggers
            .reconcile(&state, request.tick, cause)
            .map_err(SpatialTriggerOperationError::Trigger)?;
        session.last_trigger_facts = receipt.facts.clone();
        Ok(NativeSpatialTriggerReceipt {
            tick: receipt.tick,
            cause: request.cause,
            revision: receipt.revision,
            fact_count: checked_u32(receipt.facts.len(), "trigger fact count")
                .map_err(SpatialTriggerOperationError::Service)?,
            continued_count: checked_u32(receipt.continued.len(), "trigger continued count")
                .map_err(SpatialTriggerOperationError::Service)?,
            active_overlap_count: checked_u32(
                receipt.active_overlaps.len(),
                "trigger active overlap count",
            )
            .map_err(SpatialTriggerOperationError::Service)?,
            diagnostic_count: checked_u32(receipt.diagnostics.len(), "trigger diagnostic count")
                .map_err(SpatialTriggerOperationError::Service)?,
        })
    }

    fn retain_trigger_operation_diagnostic(
        &mut self,
        error: &SpatialTriggerOperationError,
    ) -> Option<NativeEngineDiagnosticLease> {
        let values = match error {
            SpatialTriggerOperationError::Trigger(error) => error
                .diagnostics
                .iter()
                .take(MAX_TRIGGER_OPERATION_DIAGNOSTICS)
                .map(|diagnostic| SpatialTriggerDiagnosticValue {
                    code: diagnostic.code.code().to_owned(),
                    message: bounded_trigger_diagnostic_text(&diagnostic.message),
                    source: diagnostic
                        .entity
                        .map(|entity| format!("entity:{}", entity.raw()))
                        .unwrap_or_default(),
                })
                .collect(),
            SpatialTriggerOperationError::Service(error) => vec![SpatialTriggerDiagnosticValue {
                code: error.code().to_owned(),
                message: bounded_trigger_diagnostic_text(error.detail()),
                source: String::new(),
            }],
        };
        let lease = SpatialTriggerDiagnosticLease::new(values)?;
        let value = self.next_trigger_diagnostic_lease;
        self.next_trigger_diagnostic_lease = value.checked_add(1)?;
        let diagnostics = NativeEngineDiagnosticLease {
            handle: NativeEngineDiagnosticLeaseHandle { value },
            diagnostics: lease.diagnostics.as_ptr(),
            diagnostics_len: lease.diagnostics.len(),
        };
        self.trigger_diagnostic_leases.insert(value, lease);
        Some(diagnostics)
    }

    fn destroy_trigger_operation_diagnostic_lease(
        &mut self,
        handle: NativeEngineDiagnosticLeaseHandle,
    ) -> bool {
        handle.value != 0
            && self
                .trigger_diagnostic_leases
                .remove(&handle.value)
                .is_some()
    }

    fn read_trigger(
        &mut self,
        request: NativeSpatialTriggerReadRequest,
    ) -> Result<NativeSpatialTriggerReadReceipt, CsharpEngineServicesError> {
        let readout = self
            .session_mut(request.session)?
            .triggers
            .current_overlaps(EntityId::new(request.trigger), u32::MAX as usize)
            .map_err(|error| spatial_error("CSHARP_SPATIAL_TRIGGER", error.to_string()))?;
        Ok(NativeSpatialTriggerReadReceipt {
            trigger: readout.trigger.raw(),
            revision: readout.revision,
            overlap_count: checked_u32(readout.subjects.len(), "trigger overlap count")?,
        })
    }

    fn read_trigger_overlap_at(
        &mut self,
        request: NativeSpatialTriggerOverlapAtRequest,
    ) -> Result<NativeSpatialTriggerOverlapAtReceipt, CsharpEngineServicesError> {
        let readout = self
            .session_mut(request.session)?
            .triggers
            .current_overlaps(EntityId::new(request.trigger), u32::MAX as usize)
            .map_err(|error| spatial_error("CSHARP_SPATIAL_TRIGGER", error.to_string()))?;
        let Some(subject) = readout.subjects.get(request.index as usize) else {
            return Ok(NativeSpatialTriggerOverlapAtReceipt {
                trigger: request.trigger,
                revision: readout.revision,
                ..Default::default()
            });
        };
        Ok(NativeSpatialTriggerOverlapAtReceipt {
            present: true,
            trigger: request.trigger,
            subject: subject.raw(),
            revision: readout.revision,
        })
    }

    fn read_trigger_fact_at(
        &mut self,
        request: NativeSpatialTriggerFactAtRequest,
    ) -> Result<NativeSpatialTriggerFactAtReceipt, CsharpEngineServicesError> {
        let fact = self
            .session_mut(request.session)?
            .last_trigger_facts
            .get(request.index as usize)
            .cloned();
        let Some(fact) = fact else {
            return Ok(NativeSpatialTriggerFactAtReceipt::default());
        };
        Ok(NativeSpatialTriggerFactAtReceipt {
            present: true,
            enter: fact.kind == TriggerOverlapFactKind::Enter,
            trigger: fact.pair.trigger,
            subject: fact.pair.subject,
            tick: fact.tick,
            cause: native_trigger_cause_value(fact.cause),
        })
    }
}

fn range_slice<'a, T>(
    values: &'a [T],
    first: u32,
    count: u32,
    field: &'static str,
) -> Result<&'a [T], CsharpEngineServicesError> {
    let end = (first as usize)
        .checked_add(count as usize)
        .ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_SPATIAL_RANGE",
                format!("C# {field} range overflowed"),
            )
        })?;
    values.get(first as usize..end).ok_or_else(|| {
        CsharpEngineServicesError::new(
            "CSHARP_SPATIAL_RANGE",
            format!("C# {field} exceeded its span"),
        )
    })
}

fn static_mesh_transform(value: NativeTransform) -> StaticMeshTransform {
    StaticMeshTransform {
        translation: [
            f64::from(value.translation.x),
            f64::from(value.translation.y),
            f64::from(value.translation.z),
        ],
        rotation: [
            f64::from(value.rotation.x),
            f64::from(value.rotation.y),
            f64::from(value.rotation.z),
            f64::from(value.rotation.w),
        ],
        scale: [
            f64::from(value.scale.x),
            f64::from(value.scale.y),
            f64::from(value.scale.z),
        ],
    }
}

fn character_motion(
    value: NativeCharacterMotion,
) -> Result<CharacterMotionComponent, CsharpEngineServicesError> {
    let stance = match value.stance {
        NativeCharacterStance::Standing => CharacterStance::Standing,
        NativeCharacterStance::Crouched => CharacterStance::Crouched,
    };
    Ok(CharacterMotionComponent {
        controlled_velocity: native_vec3_value(value.controlled_velocity),
        external_velocity: native_vec3_value(value.external_velocity),
        stance,
        grounded: value.grounded,
        jump_buffer_remaining: value.jump_buffer_remaining,
        coyote_remaining: value.coyote_remaining,
        landing_lockout_remaining: value.landing_lockout_remaining,
        support_entity: value
            .support_entity_present
            .then_some(EntityId::new(value.support_entity)),
        support_local_anchor: native_vec3_value(value.support_local_anchor),
        support_previous_translation: native_vec3_value(value.support_previous_translation),
        support_previous_rotation: if !value.support_entity_present {
            Quat::IDENTITY
        } else {
            native_quat_value(value.support_previous_rotation)
        },
        support_point_velocity: native_vec3_value(value.support_point_velocity),
        fall_origin_y: value.fall_origin_y,
        peak_y: value.peak_y,
        last_command_sequence: value.last_command_sequence,
        collision_world_hash: value.collision_world_hash,
    })
}

fn character_support_definition(
    motion: NativeCharacterMotion,
    support: NativeCharacterSupport,
) -> Result<Option<EntityDefinition>, CsharpEngineServicesError> {
    if !motion.support_entity_present {
        return Ok(None);
    }
    if !support.present || support.entity != motion.support_entity {
        return Err(CsharpEngineServicesError::new(
            "CSHARP_CHARACTER_SUPPORT",
            "C# support context did not match character continuation",
        ));
    }
    if support.entity == 1 {
        return Err(CsharpEngineServicesError::new(
            "CSHARP_CHARACTER_SUPPORT",
            "C# support entity conflicted with the call-local character",
        ));
    }
    match support.lifecycle {
        NativeCharacterSupportLifecycle::Active
        | NativeCharacterSupportLifecycle::Disabled
        | NativeCharacterSupportLifecycle::Destroyed => Ok(Some(
            EntityDefinition::new(EntityId::new(support.entity), "spatial-support")
                .with_full_transform(native_entity_transform(support.transform)),
        )),
    }
}

fn apply_support_lifecycle(
    entities: &mut EntityState,
    entity: EntityId,
    lifecycle: NativeCharacterSupportLifecycle,
) -> Result<(), CsharpEngineServicesError> {
    let authoring = EntityAuthoringService;
    let transition = match lifecycle {
        NativeCharacterSupportLifecycle::Active => return Ok(()),
        NativeCharacterSupportLifecycle::Disabled => {
            authoring.disable(entities, entities.revision(), entity)
        }
        NativeCharacterSupportLifecycle::Destroyed => {
            authoring.destroy(entities, entities.revision(), entity)
        }
    };
    transition.map(|_| ()).map_err(|error| {
        CsharpEngineServicesError::new("CSHARP_CHARACTER_SUPPORT", error.to_string())
    })
}

fn character_config(
    value: NativeCharacterControllerConfig,
) -> Result<CharacterControllerConfig, CsharpEngineServicesError> {
    let u8_field = |value: u32, field| {
        u8::try_from(value).map_err(|_| {
            CsharpEngineServicesError::new(
                "CSHARP_CHARACTER_CONFIG",
                format!("{field} exceeded u8"),
            )
        })
    };
    let u16_field = |value: u32, field| {
        u16::try_from(value).map_err(|_| {
            CsharpEngineServicesError::new(
                "CSHARP_CHARACTER_CONFIG",
                format!("{field} exceeded u16"),
            )
        })
    };
    let mut config = CharacterControllerConfig::responsive_fps();
    config.shape.standing_height = value.shape.standing_height;
    config.shape.crouched_height = value.shape.crouched_height;
    config.shape.radius = value.shape.radius;
    config.shape.contact_skin = value.shape.contact_skin;
    config.shape.clearance_padding = value.shape.clearance_padding;
    config.ground.forward_speed = value.ground.forward_speed;
    config.ground.backward_speed = value.ground.backward_speed;
    config.ground.strafe_speed = value.ground.strafe_speed;
    config.ground.acceleration = value.ground.acceleration;
    config.ground.braking = value.ground.braking;
    config.ground.friction = value.ground.friction;
    config.ground.stop_speed = value.ground.stop_speed;
    config.ground.direction_change_multiplier = value.ground.direction_change_multiplier;
    config.air.maximum_speed = value.air.maximum_speed;
    config.air.acceleration = value.air.acceleration;
    config.air.braking = value.air.braking;
    config.air.wish_speed_cap = value.air.wish_speed_cap;
    config.air.lateral_control = value.air.lateral_control;
    config.air.drag = value.air.drag;
    config.vertical.gravity = value.vertical.gravity;
    config.vertical.terminal_rise_speed = value.vertical.terminal_rise_speed;
    config.vertical.terminal_fall_speed = value.vertical.terminal_fall_speed;
    config.vertical.jump_speed = value.vertical.jump_speed;
    config.vertical.grounded_downward_bias = value.vertical.grounded_downward_bias;
    config.jump.buffer_seconds = value.jump.buffer_seconds;
    config.jump.coyote_seconds = value.jump.coyote_seconds;
    config.jump.landing_lockout_seconds = value.jump.landing_lockout_seconds;
    config.jump.held_input_retriggers = value.jump.held_input_retriggers;
    config.surface.maximum_slope_radians = value.surface.maximum_slope_radians;
    config.surface.slope_hysteresis_radians = value.surface.slope_hysteresis_radians;
    config.surface.steep_slide_acceleration = value.surface.steep_slide_acceleration;
    config.surface.steep_slide_speed = value.surface.steep_slide_speed;
    config.surface.maximum_step_height = value.surface.maximum_step_height;
    config.surface.minimum_step_width = value.surface.minimum_step_width;
    config.surface.floor_snap_distance = value.surface.floor_snap_distance;
    config.surface.floor_snap_speed_limit = value.surface.floor_snap_speed_limit;
    config.surface.ledge_support_fraction = value.surface.ledge_support_fraction;
    config.recovery.maximum_distance = value.recovery.maximum_distance;
    config.recovery.maximum_speed = value.recovery.maximum_speed;
    config.recovery.normal_nudge = value.recovery.normal_nudge;
    config.recovery.unresolved_tolerance = value.recovery.unresolved_tolerance;
    config.platform.carry_translation = value.platform.carry_translation;
    config.platform.carry_rotation = value.platform.carry_rotation;
    config.platform.inherit_departure_velocity = value.platform.inherit_departure_velocity;
    config.platform.departure_velocity_factor = value.platform.departure_velocity_factor;
    config.platform.support_loss_grace_seconds = value.platform.support_loss_grace_seconds;
    config.platform.crush_tolerance = value.platform.crush_tolerance;
    config.external_motion.impulse_scale = value.external_motion.impulse_scale;
    config.external_motion.external_decay_per_second =
        value.external_motion.external_decay_per_second;
    config.external_motion.maximum_external_speed = value.external_motion.maximum_external_speed;
    config.external_motion.authored_mass = value.external_motion.authored_mass;
    config.external_motion.dynamic_impulse_factor = value.external_motion.dynamic_impulse_factor;
    config.external_motion.maximum_dynamic_impulse = value.external_motion.maximum_dynamic_impulse;
    config.solver.maximum_slide_planes =
        u8_field(value.solver.maximum_slide_planes, "maximumSlidePlanes")?;
    config.solver.maximum_cast_iterations = u8_field(
        value.solver.maximum_cast_iterations,
        "maximumCastIterations",
    )?;
    config.solver.maximum_recovery_passes = u8_field(
        value.solver.maximum_recovery_passes,
        "maximumRecoveryPasses",
    )?;
    config.solver.maximum_contacts = u16_field(value.solver.maximum_contacts, "maximumContacts")?;
    config.solver.maximum_step_attempts =
        u8_field(value.solver.maximum_step_attempts, "maximumStepAttempts")?;
    config.solver.maximum_displacement_per_step = value.solver.maximum_displacement_per_step;
    config.solver.maximum_queries_per_step = u16_field(
        value.solver.maximum_queries_per_step,
        "maximumQueriesPerStep",
    )?;
    Ok(config)
}

fn native_character_config(value: CharacterControllerConfig) -> NativeCharacterControllerConfig {
    NativeCharacterControllerConfig {
        shape: NativeCharacterShapeConfig {
            standing_height: value.shape.standing_height,
            crouched_height: value.shape.crouched_height,
            radius: value.shape.radius,
            contact_skin: value.shape.contact_skin,
            clearance_padding: value.shape.clearance_padding,
        },
        ground: NativeCharacterGroundConfig {
            forward_speed: value.ground.forward_speed,
            backward_speed: value.ground.backward_speed,
            strafe_speed: value.ground.strafe_speed,
            acceleration: value.ground.acceleration,
            braking: value.ground.braking,
            friction: value.ground.friction,
            stop_speed: value.ground.stop_speed,
            direction_change_multiplier: value.ground.direction_change_multiplier,
        },
        air: NativeCharacterAirConfig {
            maximum_speed: value.air.maximum_speed,
            acceleration: value.air.acceleration,
            braking: value.air.braking,
            wish_speed_cap: value.air.wish_speed_cap,
            lateral_control: value.air.lateral_control,
            drag: value.air.drag,
        },
        vertical: NativeCharacterVerticalConfig {
            gravity: value.vertical.gravity,
            terminal_rise_speed: value.vertical.terminal_rise_speed,
            terminal_fall_speed: value.vertical.terminal_fall_speed,
            jump_speed: value.vertical.jump_speed,
            grounded_downward_bias: value.vertical.grounded_downward_bias,
        },
        jump: NativeCharacterJumpConfig {
            buffer_seconds: value.jump.buffer_seconds,
            coyote_seconds: value.jump.coyote_seconds,
            landing_lockout_seconds: value.jump.landing_lockout_seconds,
            held_input_retriggers: value.jump.held_input_retriggers,
        },
        surface: NativeCharacterSurfaceConfig {
            maximum_slope_radians: value.surface.maximum_slope_radians,
            slope_hysteresis_radians: value.surface.slope_hysteresis_radians,
            steep_slide_acceleration: value.surface.steep_slide_acceleration,
            steep_slide_speed: value.surface.steep_slide_speed,
            maximum_step_height: value.surface.maximum_step_height,
            minimum_step_width: value.surface.minimum_step_width,
            floor_snap_distance: value.surface.floor_snap_distance,
            floor_snap_speed_limit: value.surface.floor_snap_speed_limit,
            ledge_support_fraction: value.surface.ledge_support_fraction,
        },
        recovery: NativeCharacterRecoveryConfig {
            maximum_distance: value.recovery.maximum_distance,
            maximum_speed: value.recovery.maximum_speed,
            normal_nudge: value.recovery.normal_nudge,
            unresolved_tolerance: value.recovery.unresolved_tolerance,
        },
        platform: NativeCharacterPlatformConfig {
            carry_translation: value.platform.carry_translation,
            carry_rotation: value.platform.carry_rotation,
            inherit_departure_velocity: value.platform.inherit_departure_velocity,
            departure_velocity_factor: value.platform.departure_velocity_factor,
            support_loss_grace_seconds: value.platform.support_loss_grace_seconds,
            crush_tolerance: value.platform.crush_tolerance,
        },
        external_motion: NativeCharacterExternalMotionConfig {
            impulse_scale: value.external_motion.impulse_scale,
            external_decay_per_second: value.external_motion.external_decay_per_second,
            maximum_external_speed: value.external_motion.maximum_external_speed,
            authored_mass: value.external_motion.authored_mass,
            dynamic_impulse_factor: value.external_motion.dynamic_impulse_factor,
            maximum_dynamic_impulse: value.external_motion.maximum_dynamic_impulse,
        },
        solver: NativeCharacterSolverConfig {
            maximum_slide_planes: u32::from(value.solver.maximum_slide_planes),
            maximum_cast_iterations: u32::from(value.solver.maximum_cast_iterations),
            maximum_recovery_passes: u32::from(value.solver.maximum_recovery_passes),
            maximum_contacts: u32::from(value.solver.maximum_contacts),
            maximum_step_attempts: u32::from(value.solver.maximum_step_attempts),
            maximum_displacement_per_step: value.solver.maximum_displacement_per_step,
            maximum_queries_per_step: u32::from(value.solver.maximum_queries_per_step),
        },
    }
}

fn character_command(value: NativeCharacterControllerCommand) -> CharacterControllerCommand {
    CharacterControllerCommand {
        planar_intent: Vec2::new(value.planar_intent.x, value.planar_intent.y),
        heading_yaw_radians: value.heading_yaw_radians,
        jump_pressed: value.jump_pressed,
        jump_held: value.jump_held,
        crouch_requested: value.crouch_requested,
        external_velocity: native_vec3_value(value.external_velocity),
        external_impulse: native_vec3_value(value.external_impulse),
        step_seconds: value.step_seconds,
        sequence: value.sequence,
    }
}

fn native_character_motion(value: CharacterMotionComponent) -> NativeCharacterMotion {
    NativeCharacterMotion {
        controlled_velocity: native_vec3(value.controlled_velocity),
        external_velocity: native_vec3(value.external_velocity),
        grounded: value.grounded,
        stance: match value.stance {
            CharacterStance::Standing => NativeCharacterStance::Standing,
            CharacterStance::Crouched => NativeCharacterStance::Crouched,
        },
        jump_buffer_remaining: value.jump_buffer_remaining,
        coyote_remaining: value.coyote_remaining,
        landing_lockout_remaining: value.landing_lockout_remaining,
        support_entity_present: value.support_entity.is_some(),
        support_entity: value.support_entity.map_or(0, EntityId::raw),
        support_local_anchor: native_vec3(value.support_local_anchor),
        support_previous_translation: native_vec3(value.support_previous_translation),
        support_previous_rotation: native_quat(value.support_previous_rotation),
        support_point_velocity: native_vec3(value.support_point_velocity),
        fall_origin_y: value.fall_origin_y,
        peak_y: value.peak_y,
        last_command_sequence: value.last_command_sequence,
        collision_world_hash: value.collision_world_hash,
    }
}

fn native_character_source(
    source: CharacterCollisionSource,
) -> (
    NativeCharacterCollisionSourceKind,
    u64,
    u64,
    u64,
    u64,
    i64,
    i64,
    i64,
) {
    match source {
        CharacterCollisionSource::VoxelChunk(chunk) => (
            NativeCharacterCollisionSourceKind::VoxelChunk,
            0,
            0,
            0,
            0,
            chunk.x,
            chunk.y,
            chunk.z,
        ),
        CharacterCollisionSource::StaticMesh {
            instance,
            asset,
            geometry_hash,
        } => (
            NativeCharacterCollisionSourceKind::StaticMesh,
            0,
            instance.0,
            asset.0,
            geometry_hash,
            0,
            0,
            0,
        ),
        CharacterCollisionSource::ActiveEntity(entity) => (
            NativeCharacterCollisionSourceKind::ActiveEntity,
            entity,
            0,
            0,
            0,
            0,
            0,
            0,
        ),
    }
}

fn native_character_block_flags(mask: u32) -> NativeCharacterBlockFlags {
    match mask {
        0 => NativeCharacterBlockFlags::None,
        1 => NativeCharacterBlockFlags::Wall,
        2 => NativeCharacterBlockFlags::Ceiling,
        3 => NativeCharacterBlockFlags::WallCeiling,
        4 => NativeCharacterBlockFlags::SteepSlope,
        5 => NativeCharacterBlockFlags::WallSteepSlope,
        6 => NativeCharacterBlockFlags::CeilingSteepSlope,
        7 => NativeCharacterBlockFlags::WallCeilingSteepSlope,
        8 => NativeCharacterBlockFlags::StartSolid,
        9 => NativeCharacterBlockFlags::WallStartSolid,
        10 => NativeCharacterBlockFlags::CeilingStartSolid,
        11 => NativeCharacterBlockFlags::WallCeilingStartSolid,
        12 => NativeCharacterBlockFlags::SteepSlopeStartSolid,
        13 => NativeCharacterBlockFlags::WallSteepSlopeStartSolid,
        14 => NativeCharacterBlockFlags::CeilingSteepSlopeStartSolid,
        15 => NativeCharacterBlockFlags::WallCeilingSteepSlopeStartSolid,
        16 => NativeCharacterBlockFlags::SolverBudget,
        17 => NativeCharacterBlockFlags::WallSolverBudget,
        18 => NativeCharacterBlockFlags::CeilingSolverBudget,
        19 => NativeCharacterBlockFlags::WallCeilingSolverBudget,
        20 => NativeCharacterBlockFlags::SteepSlopeSolverBudget,
        21 => NativeCharacterBlockFlags::WallSteepSlopeSolverBudget,
        22 => NativeCharacterBlockFlags::CeilingSteepSlopeSolverBudget,
        23 => NativeCharacterBlockFlags::WallCeilingSteepSlopeSolverBudget,
        24 => NativeCharacterBlockFlags::StartSolidSolverBudget,
        25 => NativeCharacterBlockFlags::WallStartSolidSolverBudget,
        26 => NativeCharacterBlockFlags::CeilingStartSolidSolverBudget,
        27 => NativeCharacterBlockFlags::WallCeilingStartSolidSolverBudget,
        28 => NativeCharacterBlockFlags::SteepSlopeStartSolidSolverBudget,
        29 => NativeCharacterBlockFlags::WallSteepSlopeStartSolidSolverBudget,
        30 => NativeCharacterBlockFlags::CeilingSteepSlopeStartSolidSolverBudget,
        31 => NativeCharacterBlockFlags::WallCeilingSteepSlopeStartSolidSolverBudget,
        _ => unreachable!("character block mask only contains five owner bits"),
    }
}

fn native_character_contact(value: CharacterContactFact) -> NativeCharacterContact {
    let (
        source_kind,
        source_entity,
        source_instance,
        source_asset,
        source_geometry_hash,
        source_voxel_x,
        source_voxel_y,
        source_voxel_z,
    ) = native_character_source(value.source);
    NativeCharacterContact {
        present: true,
        kind: match value.kind {
            CharacterContactKind::Ground => NativeCharacterContactKind::Ground,
            CharacterContactKind::SteepSlope => NativeCharacterContactKind::SteepSlope,
            CharacterContactKind::Wall => NativeCharacterContactKind::Wall,
            CharacterContactKind::Ceiling => NativeCharacterContactKind::Ceiling,
        },
        start_solid: value.start_solid,
        point: native_vec3(value.point),
        normal: native_vec3(value.normal),
        time_of_impact: value.time_of_impact,
        source_kind,
        source_entity,
        source_instance,
        source_asset,
        source_geometry_hash,
        source_voxel_x,
        source_voxel_y,
        source_voxel_z,
    }
}

fn native_character_ground(value: CharacterGroundFact) -> NativeCharacterGround {
    let (
        source_kind,
        source_entity,
        source_instance,
        source_asset,
        source_geometry_hash,
        source_voxel_x,
        source_voxel_y,
        source_voxel_z,
    ) = native_character_source(value.source);
    NativeCharacterGround {
        present: true,
        point: native_vec3(value.point),
        normal: native_vec3(value.normal),
        snapped_distance: value.snapped_distance,
        source_kind,
        source_entity,
        source_instance,
        source_asset,
        source_geometry_hash,
        source_voxel_x,
        source_voxel_y,
        source_voxel_z,
    }
}

fn native_character_receipt(
    receipt: &engine_spatial::CharacterControllerReceipt,
) -> NativeCharacterStepReceipt {
    let contact = receipt
        .contacts
        .first()
        .copied()
        .map(native_character_contact)
        .unwrap_or_default();
    let ground = receipt
        .ground
        .map(native_character_ground)
        .unwrap_or_default();
    let floor_probe = receipt
        .floor_probe
        .map(|probe| NativeCharacterFloorProbe {
            present: true,
            rejected_hit: probe
                .rejected_hit
                .map(native_character_contact)
                .unwrap_or_default(),
            accepted_support: probe
                .accepted_support
                .map(native_character_ground)
                .unwrap_or_default(),
        })
        .unwrap_or_default();
    let stance = NativeCharacterStanceFact {
        requested: match receipt.stance.requested {
            CharacterStance::Standing => NativeCharacterStance::Standing,
            CharacterStance::Crouched => NativeCharacterStance::Crouched,
        },
        accepted: match receipt.stance.accepted {
            CharacterStance::Standing => NativeCharacterStance::Standing,
            CharacterStance::Crouched => NativeCharacterStance::Crouched,
        },
        blocked: receipt.stance.blocked,
    };
    let step = receipt
        .step
        .map(|value| NativeCharacterStep {
            present: true,
            attempted: value.attempted,
            accepted: value.accepted,
            rise: value.rise,
        })
        .unwrap_or_default();
    let platform = receipt
        .platform
        .map(|value| NativeCharacterPlatform {
            present: true,
            entity: value.entity.raw(),
            carried_displacement: native_vec3(value.carried_displacement),
            point_velocity: native_vec3(value.point_velocity),
            departed: value.departed,
        })
        .unwrap_or_default();
    let block_mask = receipt.blocks.iter().fold(0u32, |mask, block| {
        mask | match block {
            engine_spatial::CharacterBlockKind::Wall => 1,
            engine_spatial::CharacterBlockKind::Ceiling => 2,
            engine_spatial::CharacterBlockKind::SteepSlope => 4,
            engine_spatial::CharacterBlockKind::StartSolid => 8,
            engine_spatial::CharacterBlockKind::SolverBudget => 16,
        }
    });
    NativeCharacterStepReceipt {
        generation: receipt.generation,
        revision_before: receipt.revision_before,
        revision_after: receipt.revision_after,
        entity: receipt.entity.raw(),
        command_sequence: receipt.command_sequence,
        transform_before: NativeTransform {
            translation: native_vec3(receipt.transform_before.translation),
            rotation: native_quat(receipt.transform_before.rotation),
            scale: native_vec3(receipt.transform_before.scale),
        },
        transform: NativeTransform {
            translation: native_vec3(receipt.transform_after.translation),
            rotation: native_quat(receipt.transform_after.rotation),
            scale: native_vec3(receipt.transform_after.scale),
        },
        motion: native_character_motion(receipt.motion_after),
        wish_velocity: native_vec3(receipt.wish_velocity),
        displacement: native_vec3(receipt.displacement),
        contact,
        ground,
        floor_probe,
        stance,
        step,
        platform,
        block_flags: native_character_block_flags(block_mask),
        contact_count: receipt.contacts.len() as u32,
        dynamic_impulse_count: receipt.dynamic_impulses.len() as u32,
        cast_count: receipt.cast_count as u32,
        recovery_passes: receipt.recovery_passes as u32,
        recovery_distance: receipt.recovery_distance,
    }
}

fn native_entity_transform(value: NativeTransform) -> EntityTransform {
    EntityTransform {
        translation: native_vec3_value(value.translation),
        rotation: native_quat_value(value.rotation),
        scale: native_vec3_value(value.scale),
    }
}

unsafe extern "C" fn create_spatial_session(
    context: *mut c_void,
    config: NativeSpatialSessionConfig,
    handle: *mut NativeSpatialSessionHandle,
) -> i32 {
    if context.is_null() || handle.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeSpatialBridge>() };
    match bridge.create(config) {
        Ok(value) => {
            unsafe { *handle = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn destroy_spatial_session(
    context: *mut c_void,
    handle: NativeSpatialSessionHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeSpatialBridge>() };
    if bridge.sessions.remove(&handle.value).is_some() {
        bridge
            .collision_source
            .scenes
            .borrow_mut()
            .remove(&handle.value);
        ABI_OK
    } else {
        0
    }
}

unsafe extern "C" fn replace_spatial_collision(
    context: *mut c_void,
    request: *const NativeCollisionReplaceRequest,
    receipt: *mut NativeCollisionReplaceReceipt,
) -> i32 {
    if context.is_null() || request.is_null() || receipt.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeSpatialBridge>() };
    match bridge.replace_collision(unsafe { &*request }) {
        Ok(value) => {
            unsafe { *receipt = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn replace_spatial_navigation(
    context: *mut c_void,
    request: *const NativeNavigationReplaceRequest,
    receipt: *mut NativeNavigationReplaceReceipt,
) -> i32 {
    if context.is_null() || request.is_null() || receipt.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeSpatialBridge>() };
    match bridge.replace_navigation(unsafe { &*request }) {
        Ok(value) => {
            unsafe { *receipt = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn replace_spatial_voxel_navigation(
    context: *mut c_void,
    request: *const NativeNavigationVoxelReplaceRequest,
    receipt: *mut NativeNavigationReplaceReceipt,
) -> i32 {
    if context.is_null() || request.is_null() || receipt.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }
        .replace_voxel_navigation(unsafe { &*request })
    {
        Ok(value) => {
            unsafe { *receipt = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn read_navigation_projection(
    context: *mut c_void,
    request: NativeNavigationProjectionReadRequest,
    readout: *mut NativeNavigationProjectionReadout,
) -> i32 {
    if context.is_null() || readout.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }
        .read_navigation_projection(request)
    {
        Ok(value) => {
            unsafe { *readout = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn request_navigation_path(
    context: *mut c_void,
    request: NativeNavigationPathRequest,
    readout: *mut NativeNavigationPathReadout,
) -> i32 {
    if context.is_null() || readout.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }.request_navigation_path(request) {
        Ok(value) => {
            unsafe { *readout = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn read_navigation_path_cell_at(
    context: *mut c_void,
    request: NativeNavigationPathCellAtRequest,
    receipt: *mut NativeNavigationPathCellAtReceipt,
) -> i32 {
    if context.is_null() || receipt.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }
        .read_navigation_path_cell_at(request)
    {
        Ok(value) => {
            unsafe { *receipt = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn request_volumetric_navigation_path(
    context: *mut c_void,
    request: NativeNavigationVolumetricPathRequest,
    readout: *mut NativeNavigationPathReadout,
) -> i32 {
    if context.is_null() || readout.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }
        .request_volumetric_navigation_path(request)
    {
        Ok(value) => {
            unsafe { *readout = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn clear_navigation(
    context: *mut c_void,
    request: NativeNavigationClearRequest,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }.clear_navigation(request) {
        Ok(()) => ABI_OK,
        Err(_) => 0,
    }
}

unsafe extern "C" fn propose_character_step(
    context: *mut c_void,
    request: NativeCharacterStepRequest,
    receipt: *mut NativeCharacterStepReceipt,
) -> i32 {
    if context.is_null() || receipt.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeSpatialBridge>() };
    match bridge.propose_character(request) {
        Ok(value) => {
            unsafe { *receipt = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn default_character_controller_config(
    context: *mut c_void,
    config: *mut NativeCharacterControllerConfig,
) -> i32 {
    if context.is_null() || config.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeSpatialBridge>() };
    unsafe { *config = bridge.default_character_controller_config() };
    ABI_OK
}

unsafe extern "C" fn read_character_controller(
    context: *mut c_void,
    request: NativeCharacterControllerReadRequest,
    readout: *mut NativeCharacterControllerReadout,
) -> i32 {
    if context.is_null() || readout.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }.read_character_controller(request)
    {
        Ok(value) => {
            unsafe { *readout = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn read_character_contact_at(
    context: *mut c_void,
    request: NativeCharacterContactAtRequest,
    receipt: *mut NativeCharacterContactAtReceipt,
) -> i32 {
    if context.is_null() || receipt.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }.read_character_contact_at(request)
    {
        Ok(value) => {
            unsafe { *receipt = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn read_character_dynamic_impulse_at(
    context: *mut c_void,
    request: NativeCharacterDynamicImpulseAtRequest,
    receipt: *mut NativeCharacterDynamicImpulseAtReceipt,
) -> i32 {
    if context.is_null() || receipt.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }
        .read_character_dynamic_impulse_at(request)
    {
        Ok(value) => {
            unsafe { *receipt = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn propose_navigation_step(
    context: *mut c_void,
    request: NativeNavigationStepRequest,
    receipt: *mut NativeNavigationStepReceipt,
) -> i32 {
    if context.is_null() || receipt.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeSpatialBridge>() };
    match bridge.propose_navigation(request) {
        Ok(value) => {
            unsafe { *receipt = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn read_projection(
    context: *mut c_void,
    request: NativeSpatialProjectionReadRequest,
    readout: *mut NativeSpatialProjectionReadout,
) -> i32 {
    if context.is_null() || readout.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }.read_projection(request) {
        Ok(value) => {
            unsafe { *readout = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn contains_point(
    context: *mut c_void,
    request: NativeSpatialContainsPointRequest,
    receipt: *mut NativeSpatialQueryReceipt,
) -> i32 {
    if context.is_null() || receipt.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }.contains_point(request) {
        Ok(value) => {
            unsafe { *receipt = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

macro_rules! spatial_request_with_output {
    ($name:ident, $method:ident, $request:ty, $output:ty) => {
        unsafe extern "C" fn $name(
            context: *mut c_void,
            request: *const $request,
            output: *mut $output,
        ) -> i32 {
            if context.is_null() || request.is_null() || output.is_null() {
                return 0;
            }
            match unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }
                .$method(unsafe { &*request })
            {
                Ok(value) => {
                    unsafe { *output = value };
                    ABI_OK
                }
                Err(_) => 0,
            }
        }
    };
}

spatial_request_with_output!(
    cast_ray,
    cast_ray,
    NativeSpatialRaycastRequest,
    NativeSpatialHit
);
spatial_request_with_output!(
    cast_segment,
    cast_segment,
    NativeSpatialSegmentCastRequest,
    NativeSpatialHit
);
spatial_request_with_output!(
    overlap_aabb,
    overlap_aabb,
    NativeSpatialAabbQueryRequest,
    NativeSpatialQueryReceipt
);
spatial_request_with_output!(
    sweep_aabb,
    sweep_aabb,
    NativeSpatialAabbQueryRequest,
    NativeSpatialQueryReceipt
);
spatial_request_with_output!(
    cast_capsule,
    cast_capsule,
    NativeSpatialCapsuleQueryRequest,
    NativeSpatialHit
);
spatial_request_with_output!(
    overlap_capsule,
    overlap_capsule,
    NativeSpatialCapsuleQueryRequest,
    NativeSpatialHit
);
unsafe extern "C" fn pick_voxel(
    context: *mut c_void,
    request: NativeSpatialPickRequest,
    hit: *mut NativeSpatialHit,
) -> i32 {
    if context.is_null() || hit.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }.pick_voxel(request) {
        Ok(value) => {
            unsafe { *hit = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn register_trigger(
    context: *mut c_void,
    request: *const NativeSpatialTriggerRegisterRequest,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    if receipt.is_null() {
        return 0;
    }
    // SAFETY: the receipt is borrowed for this direct call and starts without
    // a retained diagnostic on every observable path.
    unsafe { *receipt = std::mem::zeroed() };
    if context.is_null() || request.is_null() {
        return 0;
    }
    // SAFETY: bridge context and request are retained for this callback only.
    let bridge = unsafe { &mut *context.cast::<RuntimeSpatialBridge>() };
    match bridge.register_trigger(unsafe { &*request }) {
        Ok(()) => ABI_OK,
        Err(error) => {
            retain_trigger_operation_error(bridge, &error, receipt, REGISTER_TRIGGER_OPERATION);
            0
        }
    }
}

unsafe extern "C" fn reconcile_triggers(
    context: *mut c_void,
    request: *const NativeSpatialTriggerReconcileRequest,
    output: *mut NativeSpatialTriggerReceipt,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    if receipt.is_null() {
        return 0;
    }
    // SAFETY: the receipt is borrowed for this direct call and starts without
    // a retained diagnostic on every observable path.
    unsafe { *receipt = std::mem::zeroed() };
    if context.is_null() || request.is_null() || output.is_null() {
        return 0;
    }
    // SAFETY: bridge context/request/output are retained for this callback only.
    let bridge = unsafe { &mut *context.cast::<RuntimeSpatialBridge>() };
    match bridge.reconcile_triggers(unsafe { &*request }) {
        Ok(value) => {
            unsafe { *output = value };
            ABI_OK
        }
        Err(error) => {
            retain_trigger_operation_error(bridge, &error, receipt, RECONCILE_TRIGGERS_OPERATION);
            0
        }
    }
}

fn retain_trigger_operation_error(
    bridge: &mut RuntimeSpatialBridge,
    error: &SpatialTriggerOperationError,
    receipt: *mut NativeOperationErrorReceipt,
    operation: &'static [u8],
) {
    if let Some(diagnostics) = bridge.retain_trigger_operation_diagnostic(error) {
        // SAFETY: receipt was checked by the direct callback and names only
        // this independently retained Spatial diagnostic lease.
        unsafe {
            *receipt = NativeOperationErrorReceipt {
                service: native_utf8(SPATIAL_SERVICE),
                operation: native_utf8(operation),
                status: 0,
                diagnostics,
            };
        }
    }
}

unsafe extern "C" fn destroy_trigger_operation_diagnostic_lease(
    context: *mut c_void,
    handle: NativeEngineDiagnosticLeaseHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    // SAFETY: context remains valid for the product lifetime.
    let bridge = unsafe { &mut *context.cast::<RuntimeSpatialBridge>() };
    i32::from(bridge.destroy_trigger_operation_diagnostic_lease(handle))
}

unsafe extern "C" fn read_trigger(
    context: *mut c_void,
    request: NativeSpatialTriggerReadRequest,
    receipt: *mut NativeSpatialTriggerReadReceipt,
) -> i32 {
    if context.is_null() || receipt.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }.read_trigger(request) {
        Ok(value) => {
            unsafe { *receipt = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn read_trigger_overlap_at(
    context: *mut c_void,
    request: NativeSpatialTriggerOverlapAtRequest,
    receipt: *mut NativeSpatialTriggerOverlapAtReceipt,
) -> i32 {
    if context.is_null() || receipt.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }.read_trigger_overlap_at(request) {
        Ok(value) => {
            unsafe { *receipt = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn read_trigger_fact_at(
    context: *mut c_void,
    request: NativeSpatialTriggerFactAtRequest,
    receipt: *mut NativeSpatialTriggerFactAtReceipt,
) -> i32 {
    if context.is_null() || receipt.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }.read_trigger_fact_at(request) {
        Ok(value) => {
            unsafe { *receipt = value };
            ABI_OK
        }
        Err(_) => 0,
    }
}

pub(crate) fn api(bridge: &mut RuntimeSpatialBridge) -> NativeSpatialApi {
    NativeSpatialApi {
        context: (bridge as *mut RuntimeSpatialBridge).cast(),
        create_session: create_spatial_session,
        destroy_session: destroy_spatial_session,
        replace_collision: replace_spatial_collision,
        replace_navigation: replace_spatial_navigation,
        replace_voxel_navigation: replace_spatial_voxel_navigation,
        read_navigation_projection,
        request_navigation_path,
        read_navigation_path_cell_at,
        request_volumetric_navigation_path,
        clear_navigation,
        default_character_controller_config,
        propose_character_step,
        read_character_controller,
        read_character_contact_at,
        read_character_dynamic_impulse_at,
        propose_navigation_step,
        read_projection,
        contains_point,
        cast_ray,
        cast_segment,
        overlap_aabb,
        sweep_aabb,
        cast_capsule,
        overlap_capsule,
        pick_voxel,
        register_trigger,
        reconcile_triggers,
        destroy_operation_diagnostic_lease: destroy_trigger_operation_diagnostic_lease,
        read_trigger,
        read_trigger_overlap_at,
        read_trigger_fact_at,
    }
}

fn spatial_error(code: &'static str, detail: impl Into<String>) -> CsharpEngineServicesError {
    CsharpEngineServicesError::new(code, detail)
}

fn native_utf8(value: &[u8]) -> NativeUtf8Slice {
    NativeUtf8Slice {
        bytes: if value.is_empty() {
            std::ptr::null()
        } else {
            value.as_ptr()
        },
        len: value.len(),
    }
}

fn bounded_trigger_diagnostic_text(value: &str) -> String {
    let mut end = value.len().min(MAX_TRIGGER_DIAGNOSTIC_TEXT_BYTES);
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_owned()
}

fn finite_vec3(value: Vec3) -> bool {
    [value.x, value.y, value.z].into_iter().all(f32::is_finite)
}

fn navigation_grid(
    config: NativePlanarNavConfig,
) -> Result<VoxelGridSpec, CsharpEngineServicesError> {
    let dimensions = ChunkDims::cubic(config.chunk_size).ok_or_else(|| {
        CsharpEngineServicesError::new("CSHARP_NAVIGATION_CONFIG", "navigation chunk size was zero")
    })?;
    let grid_id = u32::try_from(config.grid_id).map_err(|_| {
        CsharpEngineServicesError::new(
            "CSHARP_NAVIGATION_CONFIG",
            "navigation grid id exceeded u32",
        )
    })?;
    VoxelGridSpec::new(GridId::new(grid_id), config.cell_size, dimensions).ok_or_else(|| {
        CsharpEngineServicesError::new(
            "CSHARP_NAVIGATION_CONFIG",
            "navigation cell size was invalid",
        )
    })
}

fn next_navigation_revision(current: u64) -> Result<u64, CsharpEngineServicesError> {
    current.checked_add(1).ok_or_else(|| {
        CsharpEngineServicesError::new("CSHARP_NAVIGATION", "navigation revision exhausted")
    })
}

fn nav_cell(value: NativePlanarNavCell) -> VoxelCoord {
    VoxelCoord::new(value.x, value.y, value.z)
}

fn native_nav_cell(value: VoxelCoord) -> NativePlanarNavCell {
    let [x, y, z] = value.to_array();
    NativePlanarNavCell { x, y, z }
}

fn navigation_outcome(error: NavError) -> NativeNavigationPathOutcome {
    match error {
        NavError::InvalidAgentHeight => NativeNavigationPathOutcome::InvalidAgentHeight,
        NavError::InvalidQueryBudget => NativeNavigationPathOutcome::InvalidQueryBudget,
        NavError::StartNotWalkable { .. } => NativeNavigationPathOutcome::StartNotWalkable,
        NavError::GoalNotWalkable { .. } => NativeNavigationPathOutcome::GoalNotWalkable,
    }
}

fn volumetric_navigation_outcome(error: VolumetricNavError) -> NativeNavigationPathOutcome {
    match error {
        VolumetricNavError::InvalidAgentVolume => NativeNavigationPathOutcome::InvalidAgentVolume,
        VolumetricNavError::InvalidQueryBudget => NativeNavigationPathOutcome::InvalidQueryBudget,
        VolumetricNavError::StartNotTraversable { .. } => {
            NativeNavigationPathOutcome::StartNotTraversable
        }
        VolumetricNavError::GoalNotTraversable { .. } => {
            NativeNavigationPathOutcome::GoalNotTraversable
        }
    }
}

fn volumetric_config(value: NativeNavigationVolumetricConfig) -> VolumetricNavConfig {
    VolumetricNavConfig {
        agent_volume: VolumetricAgentVolume {
            size_x: value.size_x,
            size_y: value.size_y,
            size_z: value.size_z,
        },
        neighbor_set: match value.neighbor_set {
            NativeNavigationVolumetricNeighborSet::Planar4 => VolumetricNeighborSet::Planar4,
            NativeNavigationVolumetricNeighborSet::Faces6 => VolumetricNeighborSet::Faces6,
        },
        vertical_policy: match value.vertical_policy {
            NativeNavigationVolumetricVerticalPolicy::DisallowVertical => {
                VolumetricVerticalPolicy::DisallowVertical
            }
            NativeNavigationVolumetricVerticalPolicy::AllowVertical => {
                VolumetricVerticalPolicy::AllowVertical
            }
        },
        traversal_rule: match value.traversal_rule {
            NativeNavigationVolumetricTraversalRule::EmptyCells => {
                VolumetricTraversalRule::EmptyCells
            }
            NativeNavigationVolumetricTraversalRule::SolidCells => {
                VolumetricTraversalRule::SolidCells
            }
        },
    }
}

fn navigation_step_failure(
    navigation: &mut NavigationState,
    outcome: NativeNavigationPathOutcome,
) -> NativeNavigationStepReceipt {
    navigation.last_path.clear();
    NativeNavigationStepReceipt {
        outcome,
        navigation_revision: navigation.revision,
        projection_hash: navigation.projection.projection_hash(),
        ..Default::default()
    }
}

fn native_array(value: NativeVec3) -> [f64; 3] {
    [f64::from(value.x), f64::from(value.y), f64::from(value.z)]
}

fn native_f64_vec3(value: [f64; 3]) -> NativeVec3 {
    NativeVec3 {
        x: value[0] as f32,
        y: value[1] as f32,
        z: value[2] as f32,
    }
}

fn query_receipt(
    scene: &VoxelCollisionScene,
    present: bool,
    blocked: bool,
    overlaps: u32,
) -> NativeSpatialQueryReceipt {
    NativeSpatialQueryReceipt {
        present,
        blocked,
        overlaps,
        projection_version: scene.projection_version(),
        source_revision: scene.source_revision().raw(),
    }
}

fn checked_u32(value: usize, field: &'static str) -> Result<u32, CsharpEngineServicesError> {
    u32::try_from(value)
        .map_err(|_| spatial_error("CSHARP_SPATIAL_RESULT", format!("{field} exceeded u32")))
}

fn validate_aabb(min: Vec3, max: Vec3) -> Result<(), CsharpEngineServicesError> {
    if !finite_vec3(min) || !finite_vec3(max) || min.x > max.x || min.y > max.y || min.z > max.z {
        return Err(spatial_error(
            "CSHARP_SPATIAL_AABB",
            "AABB endpoints were invalid",
        ));
    }
    Ok(())
}

fn ignored_set(values: &[u64]) -> Result<Vec<EntityId>, CsharpEngineServicesError> {
    if values.len() > MAX_SPATIAL_QUERY_IGNORED_ENTITIES {
        return Err(spatial_error(
            "CSHARP_SPATIAL_FILTER",
            format!(
                "ignored entity count {} exceeded {}",
                values.len(),
                MAX_SPATIAL_QUERY_IGNORED_ENTITIES
            ),
        ));
    }
    Ok(values.iter().copied().map(EntityId::new).collect())
}

fn filter_matches(value: NativeSpatialEntityCollider, filter: NativeSpatialQueryFilter) -> bool {
    if filter.collision_group == 0 && filter.collision_mask == 0 {
        return true;
    }
    let groups_match = filter.collision_mask == 0
        || value.collision_group == 0
        || value.collision_group & filter.collision_mask != 0;
    let masks_match = filter.collision_group == 0
        || value.collision_mask == 0
        || filter.collision_group & value.collision_mask != 0;
    groups_match && masks_match
}

fn filtered_entities<'a>(
    values: &'a [NativeSpatialEntityCollider],
    filter: NativeSpatialQueryFilter,
    ignored: &[EntityId],
) -> Result<Vec<NativeSpatialEntityCollider>, CsharpEngineServicesError> {
    if values.len() > MAX_SPATIAL_QUERY_ENTITIES {
        return Err(spatial_error(
            "CSHARP_SPATIAL_FILTER",
            format!(
                "entity count {} exceeded {}",
                values.len(),
                MAX_SPATIAL_QUERY_ENTITIES
            ),
        ));
    }
    for value in values {
        validate_entity_collider(*value)?;
    }
    Ok(values
        .iter()
        .copied()
        .filter(|value| {
            filter_matches(*value, filter) && !ignored.contains(&EntityId::new(value.entity))
        })
        .collect())
}

fn validate_entity_collider(
    value: NativeSpatialEntityCollider,
) -> Result<(), CsharpEngineServicesError> {
    validate_aabb(native_vec3_value(value.min), native_vec3_value(value.max))?;
    Ok(())
}

fn entity_state(
    values: &[NativeSpatialEntityCollider],
) -> Result<EntityState, CsharpEngineServicesError> {
    let definitions = values.iter().copied().map(|value| {
        EntityDefinition::new(
            EntityId::new(value.entity),
            format!("spatial-entity-{}", value.entity),
        )
        .with_transform(Vec3::ZERO)
        .with_bounds(native_vec3_value(value.min), native_vec3_value(value.max))
        .with_collision(value.enabled, value.static_collider)
    });
    EntityState::from_definitions(definitions)
        .map_err(|error| spatial_error("CSHARP_SPATIAL_ENTITY", error.to_string()))
}

fn collider_bounds(value: NativeSpatialEntityCollider) -> ([f64; 3], [f64; 3]) {
    (native_array(value.min), native_array(value.max))
}

fn aabb_overlaps(
    first_min: [f64; 3],
    first_max: [f64; 3],
    second_min: [f64; 3],
    second_max: [f64; 3],
) -> bool {
    first_min[0] <= second_max[0]
        && first_max[0] >= second_min[0]
        && first_min[1] <= second_max[1]
        && first_max[1] >= second_min[1]
        && first_min[2] <= second_max[2]
        && first_max[2] >= second_min[2]
}

fn swept_aabb_overlaps(
    min: [f64; 3],
    max: [f64; 3],
    translation: [f64; 3],
    obstacle_min: [f64; 3],
    obstacle_max: [f64; 3],
) -> bool {
    let destination_min = [
        min[0] + translation[0],
        min[1] + translation[1],
        min[2] + translation[2],
    ];
    let destination_max = [
        max[0] + translation[0],
        max[1] + translation[1],
        max[2] + translation[2],
    ];
    let swept_min = [
        min[0].min(destination_min[0]),
        min[1].min(destination_min[1]),
        min[2].min(destination_min[2]),
    ];
    let swept_max = [
        max[0].max(destination_max[0]),
        max[1].max(destination_max[1]),
        max[2].max(destination_max[2]),
    ];
    aabb_overlaps(swept_min, swept_max, obstacle_min, obstacle_max)
}

fn cast_ray_parts(
    scene: &VoxelCollisionScene,
    origin: NativeVec3,
    direction: NativeVec3,
    max_distance: f64,
    filter: NativeSpatialQueryFilter,
    entity_values: &[NativeSpatialEntityCollider],
    ignored_values: &[u64],
    override_values: &[NativeSpatialEntityCollider],
) -> Result<NativeSpatialHit, CsharpEngineServicesError> {
    let origin = native_array(origin);
    let direction = native_array(direction);
    if !origin.into_iter().chain(direction).all(f64::is_finite)
        || max_distance <= 0.0
        || !max_distance.is_finite()
    {
        return Err(spatial_error("CSHARP_SPATIAL_RAY", "ray was invalid"));
    }
    let ignored = ignored_set(ignored_values)?;
    let entities = filtered_entities(entity_values, filter, &ignored)?;
    let state = entity_state(&entities)?;
    let overrides = override_values
        .iter()
        .copied()
        .map(|value| {
            validate_entity_collider(value)?;
            Ok(SpatialOcclusionHitboxOverride {
                entity: EntityId::new(value.entity),
                min: native_array(value.min),
                max: native_array(value.max),
            })
        })
        .collect::<Result<Vec<_>, CsharpEngineServicesError>>()?;
    let entity_hit = SpatialOcclusionService::cast_ray_with_overrides(
        scene,
        &state,
        SpatialOcclusionQuery {
            origin,
            direction,
            max_distance,
            ignored_entities: &ignored,
        },
        &overrides,
    )
    .map_err(|error| spatial_error("CSHARP_SPATIAL_RAY", error.to_string()))?;
    let mut best = entity_hit.map(native_occlusion_hit);
    if let Some(world_hit) = scene.raycast_world(origin, direction, max_distance) {
        let candidate = native_world_hit(world_hit);
        if best.is_none_or(|current| spatial_hit_precedes(candidate, current)) {
            best = Some(candidate);
        }
    }
    Ok(best.unwrap_or_default())
}

fn native_occlusion_hit(value: engine_spatial::SpatialOcclusionHit) -> NativeSpatialHit {
    match value {
        engine_spatial::SpatialOcclusionHit::Entity {
            entity,
            point,
            distance,
        } => NativeSpatialHit {
            present: true,
            kind: NativeSpatialHitKind::Entity,
            entity: entity.raw(),
            point: native_f64_vec3(point),
            distance,
            ..Default::default()
        },
        engine_spatial::SpatialOcclusionHit::Voxel(hit) => NativeSpatialHit {
            present: true,
            kind: NativeSpatialHitKind::Voxel,
            voxel_x: hit.voxel[0],
            voxel_y: hit.voxel[1],
            voxel_z: hit.voxel[2],
            face: native_face_value(hit.face),
            point: native_f64_vec3(hit.point),
            distance: hit.distance,
            ..Default::default()
        },
    }
}

fn native_world_hit(value: engine_spatial::SpatialCollisionHit) -> NativeSpatialHit {
    match value {
        engine_spatial::SpatialCollisionHit::Voxel(hit) => NativeSpatialHit {
            present: true,
            kind: NativeSpatialHitKind::Voxel,
            voxel_x: hit.voxel[0],
            voxel_y: hit.voxel[1],
            voxel_z: hit.voxel[2],
            face: native_face_value(hit.face),
            point: native_f64_vec3(hit.point),
            distance: hit.distance,
            ..Default::default()
        },
        engine_spatial::SpatialCollisionHit::StaticMesh(hit) => NativeSpatialHit {
            present: true,
            kind: NativeSpatialHitKind::StaticMesh,
            instance: hit.instance.0,
            asset: hit.asset.0,
            geometry_hash: hit.geometry_hash,
            point: native_f64_vec3([hit.point.x, hit.point.y, hit.point.z]),
            normal: native_f64_vec3([hit.normal.x, hit.normal.y, hit.normal.z]),
            distance: hit.distance,
            ..Default::default()
        },
    }
}

fn spatial_hit_precedes(candidate: NativeSpatialHit, current: NativeSpatialHit) -> bool {
    use std::cmp::Ordering;
    match candidate.distance.total_cmp(&current.distance) {
        Ordering::Less => true,
        Ordering::Greater => false,
        Ordering::Equal => spatial_hit_tie_key(candidate) < spatial_hit_tie_key(current),
    }
}

fn spatial_hit_tie_key(value: NativeSpatialHit) -> (u8, u64, u64) {
    match value.kind {
        NativeSpatialHitKind::Entity => (0, value.entity, 0),
        NativeSpatialHitKind::Voxel => (1, 0, 0),
        NativeSpatialHitKind::StaticMesh => (2, value.instance, value.asset),
        NativeSpatialHitKind::None => (3, 0, 0),
    }
}

fn character_obstacle(
    value: NativeSpatialEntityCollider,
) -> Result<CharacterObstacle, CsharpEngineServicesError> {
    let (min, max) = collider_bounds(value);
    Ok(CharacterObstacle {
        id: value.entity,
        center: core_space::WorldPos::new(
            (min[0] + max[0]) * 0.5,
            (min[1] + max[1]) * 0.5,
            (min[2] + max[2]) * 0.5,
        ),
        half_extents: core_space::WorldVec::new(
            (max[0] - min[0]) * 0.5,
            (max[1] - min[1]) * 0.5,
            (max[2] - min[2]) * 0.5,
        ),
        linear_velocity: core_space::WorldVec::ZERO,
        angular_velocity: core_space::WorldVec::ZERO,
    })
}

fn nearest_capsule_cast(
    scene: Option<engine_spatial::CharacterCapsuleCastHit>,
    entity: Option<engine_spatial::CharacterCapsuleCastHit>,
) -> NativeSpatialHit {
    let winner = match (scene, entity) {
        (Some(scene), Some(entity)) => {
            if entity.time_of_impact < scene.time_of_impact
                || (entity.time_of_impact == scene.time_of_impact && entity.source < scene.source)
            {
                Some(entity)
            } else {
                Some(scene)
            }
        }
        (Some(scene), None) => Some(scene),
        (None, Some(entity)) => Some(entity),
        (None, None) => None,
    };
    winner.map(native_capsule_cast_hit).unwrap_or_default()
}

fn native_capsule_cast_hit(value: engine_spatial::CharacterCapsuleCastHit) -> NativeSpatialHit {
    let mut result = native_character_collision_source(value.source);
    result.present = true;
    result.point = native_f64_vec3([value.point.x, value.point.y, value.point.z]);
    result.normal = native_f64_vec3([value.normal.x, value.normal.y, value.normal.z]);
    result.time_of_impact = value.time_of_impact;
    result.distance = value.time_of_impact;
    result.start_solid = value.start_solid;
    result.converged = value.converged;
    result
}

fn nearest_capsule_overlap(
    scene: Option<engine_spatial::CharacterCapsuleOverlap>,
    entity: Option<engine_spatial::CharacterCapsuleOverlap>,
) -> NativeSpatialHit {
    let winner = match (scene, entity) {
        (Some(scene), Some(entity)) => {
            if entity.penetration_depth > scene.penetration_depth
                || (entity.penetration_depth == scene.penetration_depth
                    && entity.source < scene.source)
            {
                Some(entity)
            } else {
                Some(scene)
            }
        }
        (Some(scene), None) => Some(scene),
        (None, Some(entity)) => Some(entity),
        (None, None) => None,
    };
    winner.map(native_capsule_overlap).unwrap_or_default()
}

fn native_capsule_overlap(value: engine_spatial::CharacterCapsuleOverlap) -> NativeSpatialHit {
    let mut result = native_character_collision_source(value.source);
    result.present = true;
    result.point = native_f64_vec3([value.point.x, value.point.y, value.point.z]);
    result.normal = native_f64_vec3([value.normal.x, value.normal.y, value.normal.z]);
    result.penetration_depth = value.penetration_depth;
    result
}

fn native_character_collision_source(
    value: engine_spatial::CharacterCollisionSource,
) -> NativeSpatialHit {
    match value {
        engine_spatial::CharacterCollisionSource::VoxelChunk(_) => NativeSpatialHit {
            kind: NativeSpatialHitKind::Voxel,
            ..Default::default()
        },
        engine_spatial::CharacterCollisionSource::StaticMesh {
            instance,
            asset,
            geometry_hash,
        } => NativeSpatialHit {
            kind: NativeSpatialHitKind::StaticMesh,
            instance: instance.0,
            asset: asset.0,
            geometry_hash,
            ..Default::default()
        },
        engine_spatial::CharacterCollisionSource::ActiveEntity(entity) => NativeSpatialHit {
            kind: NativeSpatialHitKind::Entity,
            entity,
            ..Default::default()
        },
    }
}

fn native_face(value: NativeSpatialFace) -> Result<core_space::Face, CsharpEngineServicesError> {
    match value {
        NativeSpatialFace::PosX => Ok(core_space::Face::PosX),
        NativeSpatialFace::NegX => Ok(core_space::Face::NegX),
        NativeSpatialFace::PosY => Ok(core_space::Face::PosY),
        NativeSpatialFace::NegY => Ok(core_space::Face::NegY),
        NativeSpatialFace::PosZ => Ok(core_space::Face::PosZ),
        NativeSpatialFace::NegZ => Ok(core_space::Face::NegZ),
        NativeSpatialFace::None => Err(spatial_error("CSHARP_SPATIAL_PICK", "face was missing")),
    }
}

fn native_face_value(value: core_space::Face) -> NativeSpatialFace {
    match value {
        core_space::Face::PosX => NativeSpatialFace::PosX,
        core_space::Face::NegX => NativeSpatialFace::NegX,
        core_space::Face::PosY => NativeSpatialFace::PosY,
        core_space::Face::NegY => NativeSpatialFace::NegY,
        core_space::Face::PosZ => NativeSpatialFace::PosZ,
        core_space::Face::NegZ => NativeSpatialFace::NegZ,
    }
}

fn native_trigger_cause(value: NativeSpatialTriggerCause) -> TriggerReconcileCause {
    match value {
        NativeSpatialTriggerCause::Scheduled => TriggerReconcileCause::Scheduled,
        NativeSpatialTriggerCause::Spawn => TriggerReconcileCause::Spawn,
        NativeSpatialTriggerCause::Movement => TriggerReconcileCause::Movement,
        NativeSpatialTriggerCause::Teleport => TriggerReconcileCause::Teleport,
        NativeSpatialTriggerCause::ActivationChanged => TriggerReconcileCause::ActivationChanged,
        NativeSpatialTriggerCause::LifecycleChanged => TriggerReconcileCause::LifecycleChanged,
        NativeSpatialTriggerCause::Restore => TriggerReconcileCause::Restore,
    }
}

fn native_trigger_cause_value(value: TriggerReconcileCause) -> NativeSpatialTriggerCause {
    match value {
        TriggerReconcileCause::Scheduled => NativeSpatialTriggerCause::Scheduled,
        TriggerReconcileCause::Spawn => NativeSpatialTriggerCause::Spawn,
        TriggerReconcileCause::Movement => NativeSpatialTriggerCause::Movement,
        TriggerReconcileCause::Teleport => NativeSpatialTriggerCause::Teleport,
        TriggerReconcileCause::ActivationChanged => NativeSpatialTriggerCause::ActivationChanged,
        TriggerReconcileCause::LifecycleChanged => NativeSpatialTriggerCause::LifecycleChanged,
        TriggerReconcileCause::Restore => NativeSpatialTriggerCause::Restore,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utf8(value: &str) -> NativeUtf8Slice {
        NativeUtf8Slice {
            bytes: value.as_ptr(),
            len: value.len(),
        }
    }

    fn copied_utf8(value: NativeUtf8Slice) -> String {
        // SAFETY: this test copies the receipt before its exact Spatial lease
        // release, which is the generated binding's required lifetime rule.
        unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(value.bytes, value.len)) }
            .to_owned()
    }

    fn create_session(api: &NativeSpatialApi) -> NativeSpatialSessionHandle {
        let mut session = NativeSpatialSessionHandle::default();
        assert_eq!(
            unsafe {
                (api.create_session)(
                    api.context,
                    NativeSpatialSessionConfig {
                        collision_voxel_size: 1.0,
                        collision_chunk_size: 16,
                        reserved: 0,
                    },
                    &mut session,
                )
            },
            ABI_OK
        );
        session
    }

    #[test]
    fn trigger_callbacks_retain_copyable_diagnostics_and_release_spatial_leases() {
        let mut bridge = RuntimeSpatialBridge::new();
        let api = api(&mut bridge);
        let session = create_session(&api);
        let scope = "fixture.trigger";
        let tag = "fixture";
        let request = NativeSpatialTriggerRegisterRequest {
            session,
            trigger: 41,
            scope: utf8(scope),
            tag: utf8(tag),
            geometry: NativeSpatialTriggerGeometry::EntityBounds,
        };
        let mut receipt: NativeOperationErrorReceipt = unsafe { std::mem::zeroed() };

        assert_eq!(
            unsafe { (api.register_trigger)(api.context, &request, &mut receipt) },
            ABI_OK
        );
        assert_eq!(receipt.diagnostics.handle.value, 0);

        receipt = unsafe { std::mem::zeroed() };
        assert_eq!(
            unsafe { (api.register_trigger)(api.context, &request, &mut receipt) },
            0
        );
        assert_eq!(copied_utf8(receipt.service), "Spatial");
        assert_eq!(copied_utf8(receipt.operation), "RegisterTrigger");
        assert_eq!(receipt.status, 0);
        assert_eq!(receipt.diagnostics.diagnostics_len, 1);
        let diagnostic = unsafe { *receipt.diagnostics.diagnostics };
        assert_eq!(copied_utf8(diagnostic.code), "duplicate-trigger-definition");
        assert_eq!(
            copied_utf8(diagnostic.message),
            "trigger entity already has a registered definition"
        );
        assert_eq!(copied_utf8(diagnostic.source), "entity:41");
        assert_eq!(
            unsafe {
                (api.destroy_operation_diagnostic_lease)(api.context, receipt.diagnostics.handle)
            },
            ABI_OK
        );
        assert_eq!(
            unsafe {
                (api.destroy_operation_diagnostic_lease)(api.context, receipt.diagnostics.handle)
            },
            0
        );

        let mut read = NativeSpatialTriggerReadReceipt::default();
        assert_eq!(
            unsafe {
                (api.read_trigger)(
                    api.context,
                    NativeSpatialTriggerReadRequest {
                        session,
                        trigger: 41,
                    },
                    &mut read,
                )
            },
            ABI_OK
        );
        assert_eq!(read.trigger, 41);

        let duplicate_entities = [
            NativeSpatialEntityCollider {
                entity: 41,
                min: NativeVec3::default(),
                max: NativeVec3 {
                    x: 1.0,
                    y: 1.0,
                    z: 1.0,
                },
                enabled: true,
                ..Default::default()
            },
            NativeSpatialEntityCollider {
                entity: 41,
                min: NativeVec3::default(),
                max: NativeVec3 {
                    x: 1.0,
                    y: 1.0,
                    z: 1.0,
                },
                enabled: true,
                ..Default::default()
            },
        ];
        let mut reconcile = NativeSpatialTriggerReceipt::default();
        receipt = unsafe { std::mem::zeroed() };
        assert_eq!(
            unsafe {
                (api.reconcile_triggers)(
                    api.context,
                    &NativeSpatialTriggerReconcileRequest {
                        session,
                        tick: 7,
                        cause: NativeSpatialTriggerCause::Scheduled,
                        entities: duplicate_entities.as_ptr(),
                        entities_len: duplicate_entities.len(),
                    },
                    &mut reconcile,
                    &mut receipt,
                )
            },
            0
        );
        assert_eq!(copied_utf8(receipt.service), "Spatial");
        assert_eq!(copied_utf8(receipt.operation), "ReconcileTriggers");
        assert_eq!(receipt.diagnostics.diagnostics_len, 1);
        let diagnostic = unsafe { *receipt.diagnostics.diagnostics };
        assert_eq!(copied_utf8(diagnostic.code), "CSHARP_SPATIAL_ENTITY");
        assert!(!copied_utf8(diagnostic.message).is_empty());
        assert_eq!(
            unsafe {
                (api.destroy_operation_diagnostic_lease)(api.context, receipt.diagnostics.handle)
            },
            ABI_OK
        );

        let mut unchanged = NativeSpatialTriggerReadReceipt::default();
        assert_eq!(
            unsafe {
                (api.read_trigger)(
                    api.context,
                    NativeSpatialTriggerReadRequest {
                        session,
                        trigger: 41,
                    },
                    &mut unchanged,
                )
            },
            ABI_OK
        );
        assert_eq!(unchanged.trigger, read.trigger);
        assert_eq!(unchanged.revision, read.revision);
        assert_eq!(unchanged.overlap_count, read.overlap_count);
    }
}
