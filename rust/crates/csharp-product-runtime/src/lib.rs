//! Deliberately permissive, experimental loader for one trusted NativeAOT C# product.
//!
//! This is a walking trial, not a product plugin framework or a compatibility
//! promise. The product is first-party trusted code. This adapter owns only the
//! fixed C ABI, copying borrowed/owned buffers, and deterministic library
//! lifetime; the C# product owns its gameplay state and orchestration.

mod native_api;

pub use native_api::*;

use std::{collections::BTreeMap, ffi::c_void, fs, path::Path, ptr};

use libloading::Library;
use product_dev_host::{
    CanonicalU64, ProductDevInputBatch, ProductDevInputResult, ProductDevLifecycleOperation,
    ProductDevOperationKind, ProductDevOperationResult, ProductDevRuntime,
    ProductDevRuntimeBinding, ProductDevRuntimeError, ProductDevRuntimeOutput,
    ProductDevRuntimeReadout, ProductDevRuntimeReceipt, ProductDevRuntimeState,
    ProductDevTimelineCompletion, ProductDevTimelineCompletionResult,
};
use core_ids::EntityId;
use core_math::{Vec2, Vec3};
use core_space::{ChunkDims, GridId, VoxelCoord, VoxelGridSpec};
use entity_state::{CharacterMotionComponent, CharacterStance, EntityAuthoringService, EntityDefinition, EntityState, EntityTransform, Quat};
use engine_spatial::{
    CharacterContactKind, CharacterControllerCommand, CharacterControllerConfig,
    CharacterControllerService, FirstPersonLookCommand, FirstPersonLookConfig,
    FirstPersonLookService, FirstPersonLookState, StaticMeshAssetId, StaticMeshColliderAsset,
    StaticMeshColliderInstance, StaticMeshInstanceId, StaticMeshTransform, VoxelCollisionScene,
};
use render_model::Transform;
use render_projection::{
    RuntimeAppearanceCatalog, RuntimeAppearanceFact, RuntimeAppearanceProjector,
};
use runtime_input::{RuntimeInputEvent, RuntimeIntentValue};
use runtime_lifecycle::{RuntimeControlRevision, RuntimeGeneration, RuntimeInstanceId};
use runtime_ui::{RuntimeUiProjectionEnvelope, RuntimeUiRuntimeBinding};
use serde_json::{Map, Number, Value};
use svc_pathfinding::{
    find_path_with_policy, propose_direct_nav_movement, DirectNavMovementRequest, NavPathOutcome,
    NavPathQuery, NavProjection, PlanarNavNeighborPolicy,
};

const ABI_OK: i32 = 1;
const INSTANCE_ID: u64 = 1;
const GENERATION: u64 = 1;
const CONTROL_REVISION: u64 = 1;

struct LoadedProductApi {
    // NativeAOT initializes process-wide managed runtime support. It does not
    // provide a safe shared-library unload contract, so a successfully created
    // product keeps its library mapped until process exit after destroy.
    library: Option<Library>,
    create: NativeProductCreate,
    start: NativeProductAction,
    turn: NativeProductTurn,
    pause: NativeProductAction,
    resume: NativeProductAction,
    shutdown: NativeProductAction,
    destroy: NativeProductDestroy,
}

impl LoadedProductApi {
    fn load(path: &Path) -> Result<Self, CsharpProductRuntimeError> {
        // SAFETY: Loading is the explicitly requested trusted-first-party
        // product boundary. `Library` remains owned by `Self` until after the
        // product instance has been destroyed in `CsharpProductRuntime::drop`.
        let library = unsafe { Library::new(path) }.map_err(|error| {
            CsharpProductRuntimeError::new(
                "CSHARP_LIBRARY_LOAD",
                format!("{}: {error}", path.display()),
            )
        })?;
        // SAFETY: every function pointer is copied from a required fixed symbol
        // while the owning `Library` is retained in this struct.
        unsafe fn symbol<T: Copy>(
            library: &Library,
            name: &[u8],
        ) -> Result<T, CsharpProductRuntimeError> {
            // SAFETY: the API deliberately fixes the expected C ABI signatures;
            // a mismatched trusted product is outside this experiment's safety
            // contract and is rejected when an expected symbol is absent.
            unsafe { library.get::<T>(name) }
                .map(|value| *value)
                .map_err(|error| {
                    CsharpProductRuntimeError::new(
                        "CSHARP_REQUIRED_EXPORT",
                        format!(
                            "required NativeAOT export `{}` is unavailable: {error}",
                            String::from_utf8_lossy(&name[..name.len() - 1])
                        ),
                    )
                })
        }
        let bind: NativeProductBind = unsafe { symbol(&library, b"rusty_product_bind\0") }?;
        let mut product = NativeProductApi::default();
        // SAFETY: `product` is a writable generated table with exact C layout.
        let status = unsafe { bind(&mut product) };
        checked_status(status, "bind")?;
        Ok(Self {
            create: required_function(product.create, "create")?,
            start: required_function(product.start, "start")?,
            turn: required_function(product.turn, "turn")?,
            pause: required_function(product.pause, "pause")?,
            resume: required_function(product.resume, "resume")?,
            shutdown: required_function(product.shutdown, "shutdown")?,
            destroy: required_function(product.destroy, "destroy")?,
            library: Some(library),
        })
    }
}

fn required_function<T>(function: Option<T>, name: &str) -> Result<T, CsharpProductRuntimeError> {
    function.ok_or_else(|| {
        CsharpProductRuntimeError::new(
            "CSHARP_REQUIRED_FUNCTION",
            format!("C# product did not bind required function `{name}`"),
        )
    })
}

/// A loaded trusted C# product adapted to the existing local browser host.
pub struct CsharpProductRuntime {
    api: LoadedProductApi,
    handle: *mut c_void,
    binding: ProductDevRuntimeBinding,
    state: ProductDevRuntimeState,
    turns: u64,
    pending_inputs: Vec<NativeInputOwned>,
    appearance_bridge: Box<RuntimeAppearanceBridge>,
    #[allow(dead_code)] // Retained to keep native EngineApi callback contexts valid.
    spatial_bridge: Box<RuntimeSpatialBridge>,
    ui_bridge: Box<RuntimeUiBridge>,
    shutdown_called: bool,
}

// The development host serializes every call with one mutex. The native handle
// has no ambient access from Rust and is destroyed before the process-lifetime
// NativeAOT library mapping is retained for process exit.
unsafe impl Send for CsharpProductRuntime {}

/// Callback state remains Engine-owned for the complete NativeAOT runtime lifetime.
/// A C# call only borrows its value arena; Rust copies it into envelopes and commits
/// the staged output after the matching product call has returned successfully.
struct RuntimeUiBridge {
    staged: Vec<RuntimeUiProjectionEnvelope>,
    streams: BTreeMap<u64, RuntimeUiStream>,
    staged_streams: Option<BTreeMap<u64, RuntimeUiStream>>,
    next_stream: u64,
    staged_next_stream: Option<u64>,
    callback_error: Option<CsharpProductRuntimeError>,
}

#[derive(Debug, Clone)]
struct RuntimeUiStream {
    stream: String,
    contract: String,
    last_sequence: Option<u64>,
}

/// The retained visual path from the original NativeAOT trial remains live
/// through phase A. It owns its own staged retained projector so a failing C#
/// call never advances the renderer-visible retained state.
struct RuntimeAppearanceBridge {
    projector: RuntimeAppearanceProjector,
    staged: Option<(RuntimeAppearanceProjector, render_model::RenderFrameDiff)>,
    callback_error: Option<CsharpProductRuntimeError>,
}

impl RuntimeAppearanceBridge {
    fn new(catalog: RuntimeAppearanceCatalog) -> Self {
        Self {
            projector: RuntimeAppearanceProjector::new(catalog),
            staged: None,
            callback_error: None,
        }
    }

    fn begin_call(&mut self) {
        self.staged = None;
        self.callback_error = None;
    }

    fn discard_call(&mut self) {
        self.staged = None;
        self.callback_error = None;
    }

    fn take_staged_call(
        &mut self,
    ) -> Result<Option<(RuntimeAppearanceProjector, render_model::RenderFrameDiff)>, CsharpProductRuntimeError> {
        if let Some(error) = self.callback_error.take() {
            self.staged = None;
            return Err(error);
        }
        Ok(self.staged.take())
    }

    fn commit(&mut self, staged: Option<(RuntimeAppearanceProjector, render_model::RenderFrameDiff)>) {
        if let Some((projector, _)) = staged {
            self.projector = projector;
        }
    }

    unsafe fn stage_snapshot(
        &mut self,
        facts: *const NativeVisualFact,
        fact_count: usize,
    ) -> Result<(), CsharpProductRuntimeError> {
        if fact_count > 0 && facts.is_null() {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_VISUAL_FACTS_POINTER",
                "C# visual snapshot had facts without a facts pointer",
            ));
        }
        // SAFETY: a non-empty snapshot was checked above and the callback is synchronous.
        let facts = if fact_count == 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(facts, fact_count) }
        };
        let mut owned = Vec::with_capacity(facts.len());
        for fact in facts {
            owned.push(RuntimeAppearanceFact {
                object_id: fact.object_id,
                appearance: borrowed_utf8(fact.appearance, fact.appearance_len, "appearance")?.to_owned(),
                transform: Transform {
                    translation: [
                        fact.transform.translation.x,
                        fact.transform.translation.y,
                        fact.transform.translation.z,
                    ],
                    rotation: [
                        fact.transform.rotation.x,
                        fact.transform.rotation.y,
                        fact.transform.rotation.z,
                        fact.transform.rotation.w,
                    ],
                    scale: [
                        fact.transform.scale.x,
                        fact.transform.scale.y,
                        fact.transform.scale.z,
                    ],
                },
                visible: fact.visible != 0,
            });
        }
        let mut projector = self.projector.clone();
        let frame = projector
            .project(&owned)
            .map_err(|error| CsharpProductRuntimeError::new("CSHARP_VISUAL_SNAPSHOT", format!("{error:?}")))?
            .frame;
        self.staged = Some((projector, frame));
        Ok(())
    }
}

unsafe extern "C" fn publish_visual_snapshot(
    context: *mut c_void,
    facts: *const NativeVisualFact,
    fact_count: usize,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    // SAFETY: context points at a box retained by `CsharpProductRuntime`.
    let bridge = unsafe { &mut *context.cast::<RuntimeAppearanceBridge>() };
    // SAFETY: callback inputs are copied/validated before this method returns.
    match unsafe { bridge.stage_snapshot(facts, fact_count) } {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.callback_error = Some(error);
            0
        }
    }
}

/// Engine-owned collision/navigation mechanisms. Player and game state never
/// live here: a character proposal builds its EntityState only for the call.
struct RuntimeSpatialBridge {
    sessions: BTreeMap<u64, SpatialSession>,
    next_session: u64,
}

struct SpatialSession {
    scene: VoxelCollisionScene,
    navigation: Option<(NavProjection, PlanarNavNeighborPolicy)>,
    controller: CharacterControllerService,
}

impl RuntimeSpatialBridge {
    fn new() -> Self {
        Self { sessions: BTreeMap::new(), next_session: 1 }
    }

    fn session_mut(&mut self, handle: NativeSpatialSessionHandle) -> Result<&mut SpatialSession, CsharpProductRuntimeError> {
        self.sessions.get_mut(&handle.value).ok_or_else(|| CsharpProductRuntimeError::new("CSHARP_SPATIAL_SESSION", "C# used an unknown spatial session"))
    }

    fn create(&mut self, config: NativeSpatialSessionConfig) -> Result<NativeSpatialSessionHandle, CsharpProductRuntimeError> {
        let scene = VoxelCollisionScene::from_solid_voxels(config.collision_voxel_size, config.collision_chunk_size, std::iter::empty())
            .map_err(|error| CsharpProductRuntimeError::new("CSHARP_SPATIAL_CREATE", error.to_string()))?;
        let value = self.next_session;
        self.next_session = self.next_session.checked_add(1).ok_or_else(|| CsharpProductRuntimeError::new("CSHARP_SPATIAL_SESSION", "spatial session handles exhausted"))?;
        self.sessions.insert(value, SpatialSession { scene, navigation: None, controller: CharacterControllerService::default() });
        Ok(NativeSpatialSessionHandle { value })
    }

    fn replace_collision(&mut self, request: &NativeCollisionReplaceRequest) -> Result<NativeCollisionReplaceReceipt, CsharpProductRuntimeError> {
        let assets = unsafe { borrowed_slice(request.assets, request.assets_len, "collision assets") }?;
        let vertices = unsafe { borrowed_slice(request.vertices, request.vertices_len, "collision vertices") }?;
        let triangles = unsafe { borrowed_slice(request.triangles, request.triangles_len, "collision triangles") }?;
        let instances = unsafe { borrowed_slice(request.instances, request.instances_len, "collision instances") }?;
        let mut admitted = Vec::with_capacity(assets.len());
        for asset in assets {
            let positions = range_slice(vertices, asset.first_vertex, asset.vertex_count, "asset vertices")?.iter()
                .map(|value| [f64::from(value.x), f64::from(value.y), f64::from(value.z)]).collect::<Vec<_>>();
            let triangles = range_slice(triangles, asset.first_triangle, asset.triangle_count, "asset triangles")?.iter()
                .map(|value| [value.a, value.b, value.c]).collect::<Vec<_>>();
            admitted.push(StaticMeshColliderAsset::new(StaticMeshAssetId(asset.id), positions, triangles)
                .map_err(|error| CsharpProductRuntimeError::new("CSHARP_COLLISION_ASSET", format!("{error:?}")))?);
        }
        let geometry = admitted.iter().map(|asset| (asset.id, asset.geometry_hash)).collect::<BTreeMap<_, _>>();
        let instances = instances.iter().map(|instance| {
            let asset = StaticMeshAssetId(instance.asset);
            let expected_geometry_hash = *geometry.get(&asset).ok_or_else(|| CsharpProductRuntimeError::new("CSHARP_COLLISION_INSTANCE", "instance referenced an unavailable asset"))?;
            Ok(StaticMeshColliderInstance { id: StaticMeshInstanceId(instance.id), asset, expected_geometry_hash, transform: static_mesh_transform(instance.transform) })
        }).collect::<Result<Vec<_>, CsharpProductRuntimeError>>()?;
        let session = self.session_mut(request.session)?;
        let receipt = session.scene.replace_static_mesh_colliders(session.scene.static_mesh_collision_revision(), admitted, instances)
            .map_err(|error| CsharpProductRuntimeError::new("CSHARP_COLLISION_REPLACE", format!("{error:?}")))?;
        Ok(NativeCollisionReplaceReceipt { revision_before: receipt.revision_before, revision_after: receipt.revision_after, asset_count: receipt.asset_count as u64, instance_count: receipt.instance_count as u64, projection_hash: receipt.projection_hash })
    }

    fn replace_navigation(&mut self, request: &NativeNavigationReplaceRequest) -> Result<NativeNavigationReplaceReceipt, CsharpProductRuntimeError> {
        let cells = unsafe { borrowed_slice(request.cells, request.cells_len, "navigation cells") }?;
        let dimensions = ChunkDims::cubic(request.config.chunk_size).ok_or_else(|| CsharpProductRuntimeError::new("CSHARP_NAVIGATION_CONFIG", "navigation chunk size was zero"))?;
        let grid_id = u32::try_from(request.config.grid_id).map_err(|_| CsharpProductRuntimeError::new("CSHARP_NAVIGATION_CONFIG", "navigation grid id exceeded u32"))?;
        let grid = VoxelGridSpec::new(GridId::new(grid_id), request.config.cell_size, dimensions)
            .ok_or_else(|| CsharpProductRuntimeError::new("CSHARP_NAVIGATION_CONFIG", "navigation cell size was invalid"))?;
        let projection = NavProjection::from_walkable_cells(grid, cells.iter().map(|cell| VoxelCoord::new(cell.x, cell.y, cell.z)));
        let receipt = NativeNavigationReplaceReceipt { walkable_cell_count: projection.walkable_len() as u64, projection_hash: projection.projection_hash() };
        self.session_mut(request.session)?.navigation = Some((projection, PlanarNavNeighborPolicy {
            max_step_cells: u8::try_from(request.config.max_step_cells).map_err(|_| CsharpProductRuntimeError::new("CSHARP_NAVIGATION_CONFIG", "maximum navigation step exceeded u8"))?,
        }));
        Ok(receipt)
    }

    fn propose_navigation(&mut self, request: NativeNavigationStepRequest) -> Result<NativeNavigationStepReceipt, CsharpProductRuntimeError> {
        let (projection, policy) = self.session_mut(request.session)?.navigation.as_ref().ok_or_else(|| CsharpProductRuntimeError::new("CSHARP_NAVIGATION", "spatial session had no navigation projection"))?;
        let from = native_vec3_value(request.from);
        let target = native_vec3_value(request.target);
        let start = projection.grid().world_to_voxel(core_space::WorldPos::new(f64::from(from.x), f64::from(from.y), f64::from(from.z)));
        let goal = projection.grid().world_to_voxel(core_space::WorldPos::new(f64::from(target.x), f64::from(target.y), f64::from(target.z)));
        let path = find_path_with_policy(projection, NavPathQuery { start, goal, max_visited: request.max_visited as usize }, *policy)
            .map_err(|error| CsharpProductRuntimeError::new("CSHARP_NAVIGATION", error.label()))?;
        if path.outcome == NavPathOutcome::NoPath {
            return Err(CsharpProductRuntimeError::new("CSHARP_NAVIGATION", "noPath"));
        }
        let next_cell = path.path.get(1).copied().unwrap_or(start);
        let step_target = if next_cell == goal { target } else {
            let center = projection.grid().voxel_center_world(next_cell);
            Vec3::new(center.x as f32, center.y as f32, center.z as f32)
        };
        let movement = propose_direct_nav_movement(DirectNavMovementRequest { from, target: step_target, max_step_units: request.max_step_units })
            .map_err(|error| CsharpProductRuntimeError::new("CSHARP_NAVIGATION", error.label()))?;
        Ok(NativeNavigationStepReceipt { next_waypoint: native_vec3(movement.next_waypoint), reached: u32::from(next_cell == goal && movement.reached), visited: path.visited as u32, path_len: path.path.len() as u32, reserved: 0, projection_hash: projection.projection_hash(), path_hash: path.path_hash })
    }

    fn propose_character(&mut self, request: NativeCharacterStepRequest) -> Result<NativeCharacterStepReceipt, CsharpProductRuntimeError> {
        let session = self.session_mut(request.session)?;
        let position = native_vec3_value(request.position);
        let motion = character_motion(request.motion)?;
        let player = EntityDefinition::new(EntityId::new(1), "spatial-proposal")
            .with_full_transform(EntityTransform::at(position)).with_character_motion(motion);
        let support = character_support_definition(request.motion, request.support)?;
        let mut definitions = vec![player];
        if let Some(definition) = support.as_ref() {
            definitions.push(definition.clone());
        }
        let mut entities = EntityState::from_definitions(definitions)
            .map_err(|error| CsharpProductRuntimeError::new("CSHARP_CHARACTER_STATE", error.to_string()))?;
        if let Some(support) = support {
            apply_support_lifecycle(&mut entities, support.id, request.support.lifecycle)?;
        }
        let receipt = session.controller.step(&mut entities, &session.scene, EntityId::new(1), &character_config(request.config), character_command(request.command))
            .map_err(|error| CsharpProductRuntimeError::new("CSHARP_CHARACTER_STEP", error.code()))?;
        Ok(native_character_receipt(&receipt))
    }
}

unsafe fn borrowed_slice<'a, T>(pointer: *const T, len: usize, field: &'static str) -> Result<&'a [T], CsharpProductRuntimeError> {
    if len > 0 && pointer.is_null() {
        return Err(CsharpProductRuntimeError::new("CSHARP_SPATIAL_POINTER", format!("C# {field} had length without a pointer")));
    }
    if len == 0 { Ok(&[]) } else {
        // SAFETY: direct-call borrowing retains this span until callback return.
        Ok(unsafe { std::slice::from_raw_parts(pointer, len) })
    }
}

fn range_slice<'a, T>(values: &'a [T], first: u32, count: u32, field: &'static str) -> Result<&'a [T], CsharpProductRuntimeError> {
    let end = (first as usize).checked_add(count as usize).ok_or_else(|| CsharpProductRuntimeError::new("CSHARP_SPATIAL_RANGE", format!("C# {field} range overflowed")))?;
    values.get(first as usize..end).ok_or_else(|| CsharpProductRuntimeError::new("CSHARP_SPATIAL_RANGE", format!("C# {field} exceeded its span")))
}

fn native_vec3_value(value: NativeVec3) -> Vec3 { Vec3::new(value.x, value.y, value.z) }

fn static_mesh_transform(value: NativeTransform) -> StaticMeshTransform {
    StaticMeshTransform {
        translation: [f64::from(value.translation.x), f64::from(value.translation.y), f64::from(value.translation.z)],
        rotation: [f64::from(value.rotation.x), f64::from(value.rotation.y), f64::from(value.rotation.z), f64::from(value.rotation.w)],
        scale: [f64::from(value.scale.x), f64::from(value.scale.y), f64::from(value.scale.z)],
    }
}

fn character_motion(value: NativeCharacterMotion) -> Result<CharacterMotionComponent, CsharpProductRuntimeError> {
    let stance = match value.stance {
        0 => CharacterStance::Standing,
        1 => CharacterStance::Crouched,
        _ => return Err(CsharpProductRuntimeError::new("CSHARP_CHARACTER_MOTION", "C# stance was unknown")),
    };
    Ok(CharacterMotionComponent {
        controlled_velocity: native_vec3_value(value.controlled_velocity), external_velocity: native_vec3_value(value.external_velocity), stance,
        grounded: value.grounded != 0, jump_buffer_remaining: value.jump_buffer_remaining, coyote_remaining: value.coyote_remaining,
        landing_lockout_remaining: value.landing_lockout_remaining,
        support_entity: (value.support_entity_present != 0).then_some(EntityId::new(value.support_entity)),
        support_local_anchor: native_vec3_value(value.support_local_anchor),
        support_previous_translation: native_vec3_value(value.support_previous_translation),
        support_previous_rotation: native_quat_value(value.support_previous_rotation),
        support_point_velocity: native_vec3_value(value.support_point_velocity),
        fall_origin_y: value.fall_origin_y, peak_y: value.peak_y, last_command_sequence: value.last_command_sequence,
        collision_world_hash: value.collision_world_hash,
    })
}

fn character_support_definition(
    motion: NativeCharacterMotion,
    support: NativeCharacterSupport,
) -> Result<Option<EntityDefinition>, CsharpProductRuntimeError> {
    if motion.support_entity_present == 0 {
        return Ok(None);
    }
    if support.present == 0 || support.entity != motion.support_entity {
        return Err(CsharpProductRuntimeError::new("CSHARP_CHARACTER_SUPPORT", "C# support context did not match character continuation"));
    }
    if support.entity == 1 {
        return Err(CsharpProductRuntimeError::new("CSHARP_CHARACTER_SUPPORT", "C# support entity conflicted with the call-local character"));
    }
    match support.lifecycle {
        0..=2 => Ok(Some(
            EntityDefinition::new(EntityId::new(support.entity), "spatial-support")
                .with_full_transform(native_entity_transform(support.transform)),
        )),
        _ => Err(CsharpProductRuntimeError::new("CSHARP_CHARACTER_SUPPORT", "C# support lifecycle was unknown")),
    }
}

fn apply_support_lifecycle(
    entities: &mut EntityState,
    entity: EntityId,
    lifecycle: u32,
) -> Result<(), CsharpProductRuntimeError> {
    let authoring = EntityAuthoringService;
    let transition = match lifecycle {
        0 => return Ok(()),
        1 => authoring.disable(entities, entities.revision(), entity),
        2 => authoring.destroy(entities, entities.revision(), entity),
        _ => unreachable!("support lifecycle was checked before materialization"),
    };
    transition.map(|_| ()).map_err(|error| CsharpProductRuntimeError::new("CSHARP_CHARACTER_SUPPORT", error.to_string()))
}

fn character_config(value: NativeCharacterControllerConfig) -> CharacterControllerConfig {
    let mut config = CharacterControllerConfig::responsive_fps();
    config.shape.standing_height = value.standing_height; config.shape.crouched_height = value.crouched_height;
    config.shape.radius = value.radius; config.shape.contact_skin = value.contact_skin;
    config.ground.forward_speed = value.forward_speed; config.ground.backward_speed = value.backward_speed;
    config.ground.strafe_speed = value.strafe_speed; config.ground.acceleration = value.acceleration;
    config.ground.braking = value.braking; config.ground.friction = value.friction;
    config.vertical.gravity = value.gravity; config.vertical.jump_speed = value.jump_speed;
    config.surface.maximum_slope_radians = value.maximum_slope_radians; config.surface.maximum_step_height = value.maximum_step_height;
    config.surface.floor_snap_distance = value.floor_snap_distance; config.solver.maximum_displacement_per_step = value.maximum_displacement_per_step;
    config
}

fn character_command(value: NativeCharacterControllerCommand) -> CharacterControllerCommand {
    CharacterControllerCommand {
        planar_intent: Vec2::new(value.planar_intent.x, value.planar_intent.y), heading_yaw_radians: value.heading_yaw_radians,
        jump_pressed: value.jump_pressed != 0, jump_held: value.jump_held != 0, crouch_requested: value.crouch_requested != 0,
        external_velocity: Vec3::ZERO, external_impulse: Vec3::ZERO, step_seconds: value.step_seconds, sequence: value.sequence,
    }
}

fn native_character_motion(value: CharacterMotionComponent) -> NativeCharacterMotion {
    NativeCharacterMotion {
        controlled_velocity: native_vec3(value.controlled_velocity), external_velocity: native_vec3(value.external_velocity), grounded: u32::from(value.grounded),
        stance: match value.stance { CharacterStance::Standing => 0, CharacterStance::Crouched => 1 }, jump_buffer_remaining: value.jump_buffer_remaining,
        coyote_remaining: value.coyote_remaining, landing_lockout_remaining: value.landing_lockout_remaining,
        support_entity_present: u32::from(value.support_entity.is_some()), support_entity: value.support_entity.map_or(0, EntityId::raw),
        support_local_anchor: native_vec3(value.support_local_anchor), support_previous_translation: native_vec3(value.support_previous_translation),
        support_previous_rotation: native_quat(value.support_previous_rotation), support_point_velocity: native_vec3(value.support_point_velocity),
        fall_origin_y: value.fall_origin_y, peak_y: value.peak_y, last_command_sequence: value.last_command_sequence,
        collision_world_hash: value.collision_world_hash,
    }
}

fn native_character_receipt(receipt: &engine_spatial::CharacterControllerReceipt) -> NativeCharacterStepReceipt {
    let contact = receipt.contacts.first().map(|contact| NativeCharacterContact {
        present: 1, kind: match contact.kind { CharacterContactKind::Ground => 1, CharacterContactKind::SteepSlope => 2, CharacterContactKind::Wall => 3, CharacterContactKind::Ceiling => 4 },
        start_solid: u32::from(contact.start_solid), reserved: 0, point: native_vec3(contact.point), normal: native_vec3(contact.normal),
    }).unwrap_or_default();
    let ground = receipt.ground.map(|ground| NativeCharacterGround { present: 1, reserved: 0, point: native_vec3(ground.point), normal: native_vec3(ground.normal), snapped_distance: ground.snapped_distance }).unwrap_or_default();
    NativeCharacterStepReceipt {
        transform: NativeTransform { translation: native_vec3(receipt.transform_after.translation), rotation: native_quat(receipt.transform_after.rotation), scale: native_vec3(receipt.transform_after.scale) },
        motion: native_character_motion(receipt.motion_after), displacement: native_vec3(receipt.displacement), contact, ground,
        stepped: u32::from(receipt.step.is_some()), step_accepted: u32::from(receipt.step.is_some_and(|step| step.accepted)),
        cast_count: receipt.cast_count as u32, recovery_passes: receipt.recovery_passes as u32,
    }
}

impl RuntimeUiBridge {
    fn new() -> Self {
        Self {
            staged: Vec::new(),
            streams: BTreeMap::new(),
            staged_streams: None,
            next_stream: 1,
            staged_next_stream: None,
            callback_error: None,
        }
    }

    fn begin_call(&mut self) {
        self.staged.clear();
        self.staged_streams = Some(self.streams.clone());
        self.staged_next_stream = Some(self.next_stream);
        self.callback_error = None;
    }

    fn discard_call(&mut self) {
        self.staged.clear();
        self.staged_streams = None;
        self.staged_next_stream = None;
        self.callback_error = None;
    }

    fn take_staged_call(&mut self) -> Result<RuntimeUiCall, CsharpProductRuntimeError> {
        if let Some(error) = self.callback_error.take() {
            self.discard_call();
            return Err(error);
        }
        Ok(RuntimeUiCall {
            projections: std::mem::take(&mut self.staged),
            streams: self.staged_streams.take().expect("every native call starts a UI stage"),
            next_stream: self.staged_next_stream.take().expect("every native call starts a UI stage"),
        })
    }

    fn commit(&mut self, staged: RuntimeUiCall) {
        self.streams = staged.streams;
        self.next_stream = staged.next_stream;
    }

    fn stage_open_stream(
        &mut self,
        request: *const NativeUiStreamRequest,
        handle: *mut NativeUiStreamHandle,
    ) -> Result<(), CsharpProductRuntimeError> {
        if request.is_null() || handle.is_null() {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_UI_STREAM_POINTER",
                "C# UI stream open had a null request or result pointer",
            ));
        }
        // SAFETY: pointers are valid for this synchronous callback and each UTF-8 slice is copied.
        let request = unsafe { *request };
        let stream = unsafe { borrowed_utf8(request.stream.bytes, request.stream.len, "stream") }?.to_owned();
        let contract = unsafe { borrowed_utf8(request.contract.bytes, request.contract.len, "contract") }?.to_owned();
        let streams = self.staged_streams.as_mut().expect("open stream only during a native call");
        let next_stream = self.staged_next_stream.as_mut().expect("open stream only during a native call");
        let value = *next_stream;
        *next_stream = next_stream.checked_add(1).ok_or_else(|| CsharpProductRuntimeError::new("CSHARP_UI_STREAM_HANDLE", "C# UI stream handles exhausted"))?;
        streams.insert(value, RuntimeUiStream { stream, contract, last_sequence: None });
        // SAFETY: result pointer was checked above and belongs to the immediate direct call.
        unsafe { *handle = NativeUiStreamHandle { value } };
        Ok(())
    }

    unsafe fn stage_projection(
        &mut self,
        projection: *const NativeUiProjection,
    ) -> Result<(), CsharpProductRuntimeError> {
        if projection.is_null() {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_UI_PROJECTION_POINTER",
                "C# UI publication had a null projection pointer",
            ));
        }
        // SAFETY: the callback is synchronous and its projection points to product memory
        // retained for the direct call. `decode_structured_value` copies it before return.
        let projection = unsafe { *projection };
        let stream = self
            .staged_streams
            .as_mut()
            .expect("publish only during a native call")
            .get_mut(&projection.stream.value)
            .ok_or_else(|| CsharpProductRuntimeError::new("CSHARP_UI_STREAM", "C# UI projection used an unopened stream handle"))?;
        if stream.last_sequence.is_some_and(|sequence| projection.sequence <= sequence) {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_UI_SEQUENCE",
                "C# UI sequence did not advance",
            ));
        }
        // SAFETY: pointer/null and range checks occur in the decoder before every slice.
        let value = unsafe { decode_structured_value(projection.value) }?;
        let envelope = RuntimeUiProjectionEnvelope::new(
            RuntimeUiRuntimeBinding::new(
                RuntimeInstanceId::new(INSTANCE_ID),
                RuntimeGeneration::new(GENERATION),
                RuntimeControlRevision::new(CONTROL_REVISION),
            ),
            projection.sequence,
            &stream.stream,
            &stream.contract,
            value,
        )
        .map_err(|error| CsharpProductRuntimeError::new("CSHARP_UI_PROJECTION", error.to_string()))?;
        stream.last_sequence = Some(projection.sequence);
        self.staged.push(envelope);
        Ok(())
    }
}

struct RuntimeUiCall {
    projections: Vec<RuntimeUiProjectionEnvelope>,
    streams: BTreeMap<u64, RuntimeUiStream>,
    next_stream: u64,
}

unsafe extern "C" fn publish_ui_projection(
    context: *mut c_void,
    projection: *const NativeUiProjection,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    // SAFETY: `context` is a stable pointer to the Box retained by
    // `CsharpProductRuntime`, and calls are serialized by the development host.
    let bridge = unsafe { &mut *context.cast::<RuntimeUiBridge>() };
    // SAFETY: all raw callback pointers are validated and copied by this helper.
    match unsafe { bridge.stage_projection(projection) } {
        Ok(()) => 1,
        Err(error) => {
            bridge.callback_error = Some(error);
            0
        }
    }
}

unsafe extern "C" fn open_ui_stream(
    context: *mut c_void,
    request: *const NativeUiStreamRequest,
    handle: *mut NativeUiStreamHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    // SAFETY: `context` is stable for the complete product lifetime.
    let bridge = unsafe { &mut *context.cast::<RuntimeUiBridge>() };
    match bridge.stage_open_stream(request, handle) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.callback_error = Some(error);
            0
        }
    }
}

unsafe extern "C" fn create_spatial_session(context: *mut c_void, config: NativeSpatialSessionConfig, handle: *mut NativeSpatialSessionHandle) -> i32 {
    if context.is_null() || handle.is_null() { return 0; }
    let bridge = unsafe { &mut *context.cast::<RuntimeSpatialBridge>() };
    match bridge.create(config) {
        Ok(value) => { unsafe { *handle = value }; ABI_OK }
        Err(_) => 0,
    }
}

unsafe extern "C" fn destroy_spatial_session(context: *mut c_void, handle: NativeSpatialSessionHandle) -> i32 {
    if context.is_null() { return 0; }
    let bridge = unsafe { &mut *context.cast::<RuntimeSpatialBridge>() };
    if bridge.sessions.remove(&handle.value).is_some() { ABI_OK } else { 0 }
}

unsafe extern "C" fn replace_spatial_collision(context: *mut c_void, request: *const NativeCollisionReplaceRequest, receipt: *mut NativeCollisionReplaceReceipt) -> i32 {
    if context.is_null() || request.is_null() || receipt.is_null() { return 0; }
    let bridge = unsafe { &mut *context.cast::<RuntimeSpatialBridge>() };
    match bridge.replace_collision(unsafe { &*request }) {
        Ok(value) => { unsafe { *receipt = value }; ABI_OK }
        Err(_) => 0,
    }
}

unsafe extern "C" fn replace_spatial_navigation(context: *mut c_void, request: *const NativeNavigationReplaceRequest, receipt: *mut NativeNavigationReplaceReceipt) -> i32 {
    if context.is_null() || request.is_null() || receipt.is_null() { return 0; }
    let bridge = unsafe { &mut *context.cast::<RuntimeSpatialBridge>() };
    match bridge.replace_navigation(unsafe { &*request }) {
        Ok(value) => { unsafe { *receipt = value }; ABI_OK }
        Err(_) => 0,
    }
}

unsafe extern "C" fn propose_character_step(context: *mut c_void, request: NativeCharacterStepRequest, receipt: *mut NativeCharacterStepReceipt) -> i32 {
    if context.is_null() || receipt.is_null() { return 0; }
    let bridge = unsafe { &mut *context.cast::<RuntimeSpatialBridge>() };
    match bridge.propose_character(request) {
        Ok(value) => { unsafe { *receipt = value }; ABI_OK }
        Err(_) => 0,
    }
}

unsafe extern "C" fn propose_navigation_step(context: *mut c_void, request: NativeNavigationStepRequest, receipt: *mut NativeNavigationStepReceipt) -> i32 {
    if context.is_null() || receipt.is_null() { return 0; }
    let bridge = unsafe { &mut *context.cast::<RuntimeSpatialBridge>() };
    match bridge.propose_navigation(request) {
        Ok(value) => { unsafe { *receipt = value }; ABI_OK }
        Err(_) => 0,
    }
}

fn engine_api(
    appearance_bridge: &mut RuntimeAppearanceBridge,
    spatial_bridge: &mut RuntimeSpatialBridge,
    ui_bridge: &mut RuntimeUiBridge,
) -> NativeEngineApi {
    NativeEngineApi {
        context: (appearance_bridge as *mut RuntimeAppearanceBridge).cast(),
        publish_visual_snapshot,
        look: NativeLookApi {
            context: ptr::null_mut(),
            integrate: integrate_look,
        },
        spatial: NativeSpatialApi {
            context: (spatial_bridge as *mut RuntimeSpatialBridge).cast(),
            create_session: create_spatial_session,
            destroy_session: destroy_spatial_session,
            replace_collision: replace_spatial_collision,
            replace_navigation: replace_spatial_navigation,
            propose_character_step: propose_character_step,
            propose_navigation_step: propose_navigation_step,
        },
        ui: NativeUiApi {
            context: (ui_bridge as *mut RuntimeUiBridge).cast(),
            open_stream: open_ui_stream,
            publish_projection: publish_ui_projection,
        },
    }
}

unsafe extern "C" fn integrate_look(
    _context: *mut c_void,
    request: NativeLookRequest,
    receipt: *mut NativeLookReceipt,
) -> i32 {
    if receipt.is_null() {
        return 0;
    }
    let mut config = FirstPersonLookConfig::default();
    config.horizontal_radians_per_unit = request.config.horizontal_radians_per_unit;
    config.vertical_radians_per_unit = request.config.vertical_radians_per_unit;
    config.minimum_pitch_radians = request.config.minimum_pitch_radians;
    config.maximum_pitch_radians = request.config.maximum_pitch_radians;
    config.invert_horizontal = request.config.invert_horizontal != 0;
    config.invert_vertical = request.config.invert_vertical != 0;
    config.wrap_yaw = request.config.wrap_yaw != 0;
    config.maximum_delta_radians = request.config.maximum_delta_radians;
    let result = FirstPersonLookService.integrate(
        &config,
        FirstPersonLookState {
            yaw_radians: request.state.yaw_radians,
            pitch_radians: request.state.pitch_radians,
        },
        FirstPersonLookCommand {
            delta: Vec2::new(request.delta.x, request.delta.y),
        },
    );
    match result {
        Ok(result) => {
            // SAFETY: null was rejected above; the receipt is borrowed for this call only.
            unsafe {
                *receipt = NativeLookReceipt {
                    state: NativeLookState {
                        yaw_radians: result.after.yaw_radians,
                        pitch_radians: result.after.pitch_radians,
                    },
                    orientation: native_quat(result.orientation),
                    forward: native_vec3(result.forward),
                    right: native_vec3(result.right),
                    up: native_vec3(result.up),
                };
            }
            ABI_OK
        }
        Err(_) => 0,
    }
}

fn native_vec3(value: Vec3) -> NativeVec3 {
    NativeVec3 {
        x: value.x,
        y: value.y,
        z: value.z,
    }
}

fn native_quat(value: Quat) -> NativeQuat {
    NativeQuat {
        x: value.x,
        y: value.y,
        z: value.z,
        w: value.w,
    }
}

fn native_quat_value(value: NativeQuat) -> Quat {
    Quat::new(value.x, value.y, value.z, value.w)
}

fn native_entity_transform(value: NativeTransform) -> EntityTransform {
    EntityTransform {
        translation: native_vec3_value(value.translation),
        rotation: native_quat_value(value.rotation),
        scale: native_vec3_value(value.scale),
    }
}

unsafe fn decode_structured_value(
    arena: NativeStructuredValue,
) -> Result<Value, CsharpProductRuntimeError> {
    if arena.node_count == 0 || arena.nodes.is_null() {
        return Err(CsharpProductRuntimeError::new(
            "CSHARP_UI_NODES",
            "C# UI arena had no root node",
        ));
    }
    if arena.utf8_len > 0 && arena.utf8.is_null() {
        return Err(CsharpProductRuntimeError::new(
            "CSHARP_UI_UTF8_POINTER",
            "C# UI arena had UTF-8 length without bytes",
        ));
    }
    if arena.edge_count > 0 && arena.edges.is_null() {
        return Err(CsharpProductRuntimeError::new(
            "CSHARP_UI_EDGES_POINTER",
            "C# UI arena had edge length without edges",
        ));
    }
    if usize::try_from(arena.root).map_err(|_| CsharpProductRuntimeError::new("CSHARP_UI_ROOT", "C# UI root overflowed"))? >= arena.node_count {
        return Err(CsharpProductRuntimeError::new(
            "CSHARP_UI_ROOT",
            "C# UI root was outside its node arena",
        ));
    }
    // SAFETY: pointers were checked and the fixed callback contract keeps source storage alive.
    let nodes = unsafe { std::slice::from_raw_parts(arena.nodes, arena.node_count) };
    let bytes = if arena.utf8_len == 0 {
        &[]
    } else {
        // SAFETY: non-empty byte ranges were checked for a non-null pointer above.
        unsafe { std::slice::from_raw_parts(arena.utf8, arena.utf8_len) }
    };
    let edges = if arena.edge_count == 0 {
        &[]
    } else {
        // SAFETY: non-empty edge ranges were checked for a non-null pointer above.
        unsafe { std::slice::from_raw_parts(arena.edges, arena.edge_count) }
    };
    let mut visiting = vec![false; nodes.len()];
    decode_structured_node(arena.root as usize, nodes, edges, bytes, &mut visiting)
}

fn decode_structured_node(
    index: usize,
    nodes: &[NativeStructuredValueNode],
    edges: &[u32],
    bytes: &[u8],
    visiting: &mut [bool],
) -> Result<Value, CsharpProductRuntimeError> {
    let node = nodes.get(index).ok_or_else(|| CsharpProductRuntimeError::new("CSHARP_UI_NODE", "C# UI child was outside its node arena"))?;
    if visiting[index] {
        return Err(CsharpProductRuntimeError::new("CSHARP_UI_CYCLE", "C# UI arena contained a child cycle"));
    }
    visiting[index] = true;
    let value = match node.kind {
        0 => Value::Null,
        1 => Value::Bool(node.bool_value != 0),
        2 => Value::Number(Number::from_f64(node.number_value).ok_or_else(|| CsharpProductRuntimeError::new("CSHARP_UI_NUMBER", "C# UI number was not finite"))?),
        3 => Value::String(arena_text(bytes, node.text_offset, node.text_len, "text")?.to_owned()),
        4 => {
            let children = arena_children(node, edges)?;
            let mut values = Vec::with_capacity(children.len());
            for child in children {
                values.push(decode_structured_node(child, nodes, edges, bytes, visiting)?);
            }
            Value::Array(values)
        }
        5 => {
            let children = arena_children(node, edges)?;
            let mut values = Map::new();
            for child in children {
                let child_node = nodes.get(child).ok_or_else(|| {
                    CsharpProductRuntimeError::new(
                        "CSHARP_UI_CHILDREN",
                        "C# UI child index exceeded nodes",
                    )
                })?;
                let key = arena_text(bytes, child_node.key_offset, child_node.key_len, "key")?.to_owned();
                values.insert(key, decode_structured_node(child, nodes, edges, bytes, visiting)?);
            }
            Value::Object(values)
        }
        _ => return Err(CsharpProductRuntimeError::new("CSHARP_UI_KIND", "C# UI node had an unknown kind")),
    };
    visiting[index] = false;
    Ok(value)
}

fn arena_children(
    node: &NativeStructuredValueNode,
    edges: &[u32],
) -> Result<Vec<usize>, CsharpProductRuntimeError> {
    let first = node.first_edge as usize;
    let end = first.checked_add(node.child_count as usize).ok_or_else(|| CsharpProductRuntimeError::new("CSHARP_UI_CHILDREN", "C# UI child range overflowed"))?;
    let child_edges = edges.get(first..end).ok_or_else(|| {
        CsharpProductRuntimeError::new("CSHARP_UI_CHILDREN", "C# UI child range exceeded edges")
    })?;
    child_edges
        .iter()
        .map(|child| {
            usize::try_from(*child).map_err(|_| {
                CsharpProductRuntimeError::new("CSHARP_UI_CHILDREN", "C# UI child index overflowed")
            })
        })
        .collect()
}

unsafe fn borrowed_utf8<'a>(
    pointer: *const u8,
    len: usize,
    field: &'static str,
) -> Result<&'a str, CsharpProductRuntimeError> {
    if len > 0 && pointer.is_null() {
        return Err(CsharpProductRuntimeError::new(
            "CSHARP_UTF8_POINTER",
            format!("C# {field} had length without bytes"),
        ));
    }
    let bytes = if len == 0 {
        &[]
    } else {
        // SAFETY: a non-empty borrowed range was checked above and is only used during this callback.
        unsafe { std::slice::from_raw_parts(pointer, len) }
    };
    std::str::from_utf8(bytes).map_err(|_| {
        CsharpProductRuntimeError::new("CSHARP_UTF8", format!("C# {field} was not UTF-8"))
    })
}

fn arena_text<'a>(
    bytes: &'a [u8],
    offset: u32,
    len: u32,
    field: &'static str,
) -> Result<&'a str, CsharpProductRuntimeError> {
    let start = offset as usize;
    let end = start.checked_add(len as usize).ok_or_else(|| CsharpProductRuntimeError::new("CSHARP_UI_UTF8_RANGE", format!("C# UI {field} range overflowed")))?;
    let slice = bytes.get(start..end).ok_or_else(|| CsharpProductRuntimeError::new("CSHARP_UI_UTF8_RANGE", format!("C# UI {field} range exceeded bytes")))?;
    std::str::from_utf8(slice).map_err(|_| CsharpProductRuntimeError::new("CSHARP_UI_UTF8", format!("C# UI {field} was not UTF-8")))
}

impl CsharpProductRuntime {
    /// Loads one C# library and creates its authoritative product state.
    pub fn load(
        library_path: impl AsRef<Path>,
        content_root: impl AsRef<Path>,
    ) -> Result<Self, CsharpProductRuntimeError> {
        let api = LoadedProductApi::load(library_path.as_ref())?;
        let content = collect_content(content_root.as_ref())?;
        let appearance_catalog = load_runtime_appearance_catalog(content_root.as_ref())?;
        let mut appearance_bridge = Box::new(RuntimeAppearanceBridge::new(appearance_catalog));
        let mut spatial_bridge = Box::new(RuntimeSpatialBridge::new());
        let mut ui_bridge = Box::new(RuntimeUiBridge::new());
        let native_content: Vec<NativeContentFile> = content
            .iter()
            .map(|file| NativeContentFile {
                path: file.path.as_ptr(),
                path_len: file.path.len(),
                bytes: file.bytes.as_ptr(),
                bytes_len: file.bytes.len(),
            })
            .collect();
        let args = NativeProductCreateArgs {
            content: native_content.as_ptr(),
            content_len: native_content.len(),
            engine: engine_api(&mut appearance_bridge, &mut spatial_bridge, &mut ui_bridge),
        };
        let mut handle = ptr::null_mut();
        appearance_bridge.begin_call();
        ui_bridge.begin_call();
        match call_create(&api, &args, &mut handle) {
            Ok(()) => {}
            Err(error) => {
                appearance_bridge.discard_call();
                ui_bridge.discard_call();
                if !handle.is_null() {
                    // SAFETY: a failing create may still have returned an owned
                    // handle; releasing it is part of the fixed ownership ABI.
                    unsafe { (api.destroy)(handle) };
                }
                return Err(error);
            }
        }
        let staged_appearance = match appearance_bridge.take_staged_call() {
            Ok(staged) => staged,
            Err(error) => {
                ui_bridge.discard_call();
                if !handle.is_null() {
                    // SAFETY: successful create produced this owned product handle,
                    // but its staged callback output was not accepted by Rust.
                    unsafe { (api.destroy)(handle) };
                }
                return Err(error);
            }
        };
        let staged_ui = match ui_bridge.take_staged_call() {
            Ok(staged) => staged,
            Err(error) => {
                if !handle.is_null() {
                    // SAFETY: successful create produced this owned product handle.
                    unsafe { (api.destroy)(handle) };
                }
                return Err(error);
            }
        };
        if handle.is_null() {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_CREATE_HANDLE",
                "rusty_product_create succeeded but returned a null product handle",
            ));
        }
        appearance_bridge.commit(staged_appearance);
        ui_bridge.commit(staged_ui);
        Ok(Self {
            api,
            handle,
            binding: binding(),
            state: ProductDevRuntimeState::Created,
            turns: 0,
            pending_inputs: Vec::new(),
            appearance_bridge,
            spatial_bridge,
            ui_bridge,
            shutdown_called: false,
        })
    }

    /// Calls the fixed lifecycle and two direct stateful turns for the small
    /// NativeAOT fixture. Service call success proves the generated facade can
    /// borrow structured UI publication and the product can retain state.
    pub fn exercise_turns(&mut self) -> Result<(), CsharpProductRuntimeError> {
        self.action(
            self.api.start,
            ProductDevOperationKind::Start,
            ProductDevRuntimeState::Running,
        )?;
        self.pending_inputs
            .push(input_owned(1, 1, 1, 0.0, 0.0, "KeyW".to_owned()));
        self.turn(2, 1)?;
        self.turn(2, 2)?;
        self.action(
            self.api.pause,
            ProductDevOperationKind::Pause,
            ProductDevRuntimeState::Paused,
        )?;
        self.action(
            self.api.resume,
            ProductDevOperationKind::Resume,
            ProductDevRuntimeState::Running,
        )?;
        Ok(())
    }

    fn turn(
        &mut self,
        kind: u32,
        observed_time_or_step: u64,
    ) -> Result<Vec<ProductDevRuntimeOutput>, CsharpProductRuntimeError> {
        let events: Vec<NativeInputEvent> = self
            .pending_inputs
            .iter()
            .map(NativeInputOwned::as_native)
            .collect();
        self.appearance_bridge.begin_call();
        self.ui_bridge.begin_call();
        match call_turn(
            &self.api,
            self.handle,
            NativeTurnArgs {
                kind,
                reserved: 0,
                observed_time_or_step,
                events: events.as_ptr(),
                event_count: events.len(),
            },
        ) {
            Ok(()) => {}
            Err(error) => {
                self.appearance_bridge.discard_call();
                self.ui_bridge.discard_call();
                return Err(error);
            }
        }
        let staged_appearance = self.appearance_bridge.take_staged_call()?;
        let staged_ui = self.ui_bridge.take_staged_call()?;
        // The C# call has accepted the batch. Do not replay already-applied
        // product input on a later timing turn.
        self.pending_inputs.clear();
        let mut outputs = Vec::new();
        append_frame(&mut outputs, staged_appearance.as_ref().map(|(_, frame)| frame))?;
        append_ui(&mut outputs, &staged_ui.projections)?;
        let turns = self.turns.checked_add(1).ok_or_else(|| {
            CsharpProductRuntimeError::new("CSHARP_TURN_COUNTER", "turn counter overflowed")
        })?;
        self.appearance_bridge.commit(staged_appearance);
        self.ui_bridge.commit(staged_ui);
        self.turns = turns;
        Ok(outputs)
    }

    fn action(
        &mut self,
        action: NativeProductAction,
        operation: ProductDevOperationKind,
        state: ProductDevRuntimeState,
    ) -> Result<Vec<ProductDevRuntimeOutput>, CsharpProductRuntimeError> {
        self.appearance_bridge.begin_call();
        self.ui_bridge.begin_call();
        match call_action(action, self.handle, operation) {
            Ok(()) => {}
            Err(error) => {
                self.appearance_bridge.discard_call();
                self.ui_bridge.discard_call();
                return Err(error);
            }
        }
        let staged_appearance = self.appearance_bridge.take_staged_call()?;
        let staged_ui = self.ui_bridge.take_staged_call()?;
        let mut outputs = Vec::new();
        append_frame(&mut outputs, staged_appearance.as_ref().map(|(_, frame)| frame))?;
        append_ui(&mut outputs, &staged_ui.projections)?;
        self.appearance_bridge.commit(staged_appearance);
        self.ui_bridge.commit(staged_ui);
        self.state = state;
        Ok(outputs)
    }

    fn receipt(
        &self,
        operation: ProductDevOperationKind,
        outputs: Vec<ProductDevRuntimeOutput>,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevOperationResult>, ProductDevRuntimeError> {
        let readout = self.readout();
        let result = ProductDevOperationResult::accepted(operation, self.binding, readout)
            .map_err(host_error)?;
        ProductDevRuntimeReceipt::new(result, outputs).map_err(host_runtime_error)
    }

    fn readout(&self) -> ProductDevRuntimeReadout {
        ProductDevRuntimeReadout::new(
            self.binding,
            product_dev_host::ProductDevRuntimeMode::Realtime,
            self.state,
        )
        .with_counters(self.turns, self.turns, 0, 0)
    }

    fn runtime_error(&self, error: CsharpProductRuntimeError) -> ProductDevRuntimeError {
        ProductDevRuntimeError::new(error.code, error.detail)
            .expect("fixed bounded NativeAOT error")
    }
}

impl ProductDevRuntime for CsharpProductRuntime {
    fn lifecycle(
        &mut self,
        operation: ProductDevLifecycleOperation,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevOperationResult>, ProductDevRuntimeError> {
        match operation {
            ProductDevLifecycleOperation::Start => {
                let outputs = self
                    .action(
                        self.api.start,
                        ProductDevOperationKind::Start,
                        ProductDevRuntimeState::Running,
                    )
                    .map_err(|error| self.runtime_error(error))?;
                self.receipt(ProductDevOperationKind::Start, outputs)
            }
            ProductDevLifecycleOperation::Pause => {
                let outputs = self
                    .action(
                        self.api.pause,
                        ProductDevOperationKind::Pause,
                        ProductDevRuntimeState::Paused,
                    )
                    .map_err(|error| self.runtime_error(error))?;
                self.receipt(ProductDevOperationKind::Pause, outputs)
            }
            ProductDevLifecycleOperation::Resume => {
                let outputs = self
                    .action(
                        self.api.resume,
                        ProductDevOperationKind::Resume,
                        ProductDevRuntimeState::Running,
                    )
                    .map_err(|error| self.runtime_error(error))?;
                self.receipt(ProductDevOperationKind::Resume, outputs)
            }
            ProductDevLifecycleOperation::Shutdown => {
                let outputs = self
                    .action(
                        self.api.shutdown,
                        ProductDevOperationKind::Shutdown,
                        ProductDevRuntimeState::Shutdown,
                    )
                    .map_err(|error| self.runtime_error(error))?;
                let receipt = self.receipt(ProductDevOperationKind::Shutdown, outputs);
                self.shutdown_called = receipt.is_ok();
                receipt
            }
            ProductDevLifecycleOperation::Restart | ProductDevLifecycleOperation::ReportFault => {
                Err(ProductDevRuntimeError::new(
                    "CSHARP_UNSUPPORTED_LIFECYCLE",
                    "this trusted NativeAOT trial exposes only start, pause, resume, and shutdown",
                )
                .expect("fixed error"))
            }
        }
    }

    fn input(
        &mut self,
        batch: ProductDevInputBatch,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevInputResult>, ProductDevRuntimeError> {
        self.pending_inputs.extend(native_events(batch.events()));
        let result =
            ProductDevInputResult::accepted(batch.events().len(), self.binding, self.readout())
                .map_err(host_error)?;
        ProductDevRuntimeReceipt::new(result, Vec::new()).map_err(host_runtime_error)
    }

    fn advance_realtime(
        &mut self,
        observed_time_ns: CanonicalU64,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevOperationResult>, ProductDevRuntimeError> {
        let outputs = self
            .turn(1, observed_time_ns.get())
            .map_err(|error| self.runtime_error(error))?;
        self.receipt(ProductDevOperationKind::AdvanceRealtime, outputs)
    }

    fn admit_demand_step(
        &mut self,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevOperationResult>, ProductDevRuntimeError> {
        let outputs = self
            .turn(2, self.turns.saturating_add(1))
            .map_err(|error| self.runtime_error(error))?;
        self.receipt(ProductDevOperationKind::AdmitDemandStep, outputs)
    }

    fn admit_external_step(
        &mut self,
        step: CanonicalU64,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevOperationResult>, ProductDevRuntimeError> {
        let outputs = self
            .turn(3, step.get())
            .map_err(|error| self.runtime_error(error))?;
        self.receipt(ProductDevOperationKind::AdmitExternalStep, outputs)
    }

    fn complete_timeline(
        &mut self,
        _completion: ProductDevTimelineCompletion,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevTimelineCompletionResult>, ProductDevRuntimeError>
    {
        Err(ProductDevRuntimeError::new(
            "CSHARP_TIMELINE_UNSUPPORTED",
            "the NativeAOT walking trial has no timeline bridge",
        )
        .expect("fixed error"))
    }
}

impl Drop for CsharpProductRuntime {
    fn drop(&mut self) {
        if self.handle.is_null() {
            return;
        }
        if !self.shutdown_called {
            // SAFETY: `handle` was produced by this retained library, and no
            // other Rust path destroys it. Native exceptions must not cross ABI.
            let _ = unsafe { (self.api.shutdown)(self.handle) };
        }
        // SAFETY: destroy runs exactly once before the `Library` field drops.
        unsafe { (self.api.destroy)(self.handle) };
        self.handle = ptr::null_mut();
        // A NativeAOT shared library may retain runtime worker infrastructure
        // beyond its exported destroy function. Process-lifetime mapping keeps
        // Drop safe while preserving the required product destroy ordering.
        if let Some(library) = self.api.library.take() {
            std::mem::forget(library);
        }
    }
}

fn binding() -> ProductDevRuntimeBinding {
    ProductDevRuntimeBinding {
        instance_id: CanonicalU64::new(INSTANCE_ID),
        generation: CanonicalU64::new(GENERATION),
        control_revision: CanonicalU64::new(CONTROL_REVISION),
    }
}

fn call_create(
    api: &LoadedProductApi,
    args: &NativeProductCreateArgs,
    handle: &mut *mut c_void,
) -> Result<(), CsharpProductRuntimeError> {
    // SAFETY: fixed ABI pointers are valid for the duration of this call.
    let status = unsafe { (api.create)(args, handle) };
    checked_status(status, "create")
}

fn call_action(
    action: NativeProductAction,
    handle: *mut c_void,
    operation: ProductDevOperationKind,
) -> Result<(), CsharpProductRuntimeError> {
    // SAFETY: `handle` is retained by the runtime.
    let status = unsafe { action(handle) };
    checked_status(status, operation_name(operation))
}

fn call_turn(
    api: &LoadedProductApi,
    handle: *mut c_void,
    args: NativeTurnArgs,
) -> Result<(), CsharpProductRuntimeError> {
    // SAFETY: event label pointers borrow local strings that remain alive for
    // the call; the C# product is required to copy anything it retains.
    let status = unsafe { (api.turn)(handle, &args) };
    checked_status(status, "turn")
}

fn checked_status(status: i32, operation: &str) -> Result<(), CsharpProductRuntimeError> {
    if status != ABI_OK {
        return Err(CsharpProductRuntimeError::new(
            "CSHARP_PRODUCT_CALL",
            format!("C# product {operation} returned status {status}"),
        ));
    }
    Ok(())
}

fn operation_name(operation: ProductDevOperationKind) -> &'static str {
    match operation {
        ProductDevOperationKind::Start => "start",
        ProductDevOperationKind::Pause => "pause",
        ProductDevOperationKind::Resume => "resume",
        ProductDevOperationKind::Shutdown => "shutdown",
        _ => "operation",
    }
}

#[derive(Debug)]
struct ContentFile {
    path: Vec<u8>,
    bytes: Vec<u8>,
}

fn collect_content(root: &Path) -> Result<Vec<ContentFile>, CsharpProductRuntimeError> {
    if !root.is_dir() {
        return Err(CsharpProductRuntimeError::new(
            "CSHARP_CONTENT_ROOT",
            format!("content directory does not exist: {}", root.display()),
        ));
    }
    let mut files = Vec::new();
    collect_content_inner(root, root, &mut files)?;
    Ok(files)
}

fn load_runtime_appearance_catalog(
    content_root: &Path,
) -> Result<RuntimeAppearanceCatalog, CsharpProductRuntimeError> {
    let path = content_root.join("runtime-appearances.json");
    let bytes = fs::read(&path).map_err(|error| {
        CsharpProductRuntimeError::new(
            "CSHARP_RUNTIME_APPEARANCES",
            format!("{}: {error}", path.display()),
        )
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        CsharpProductRuntimeError::new(
            "CSHARP_RUNTIME_APPEARANCES",
            format!("{}: {error}", path.display()),
        )
    })
}

fn collect_content_inner(
    root: &Path,
    directory: &Path,
    files: &mut Vec<ContentFile>,
) -> Result<(), CsharpProductRuntimeError> {
    for entry in fs::read_dir(directory)
        .map_err(|error| CsharpProductRuntimeError::new("CSHARP_CONTENT_READ", error.to_string()))?
    {
        let entry = entry.map_err(|error| {
            CsharpProductRuntimeError::new("CSHARP_CONTENT_READ", error.to_string())
        })?;
        let path = entry.path();
        if path.is_dir() {
            collect_content_inner(root, &path, files)?;
        } else if path.is_file() {
            let relative = path.strip_prefix(root).expect("walked below content root");
            let path = relative.to_string_lossy().replace('\\', "/").into_bytes();
            files.push(ContentFile {
                path,
                bytes: fs::read(entry.path()).map_err(|error| {
                    CsharpProductRuntimeError::new("CSHARP_CONTENT_READ", error.to_string())
                })?,
            });
        }
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(())
}

struct NativeInputOwned {
    kind: u32,
    edge: u32,
    sequence: u64,
    x: f32,
    y: f32,
    label: Vec<u8>,
}

impl NativeInputOwned {
    fn as_native(&self) -> NativeInputEvent {
        NativeInputEvent {
            kind: self.kind,
            edge: self.edge,
            sequence: self.sequence,
            x: self.x,
            y: self.y,
            label: self.label.as_ptr(),
            label_len: self.label.len(),
        }
    }
}

fn native_events(events: &[RuntimeInputEvent]) -> Vec<NativeInputOwned> {
    events.iter().map(native_event).collect()
}

fn native_event(event: &RuntimeInputEvent) -> NativeInputOwned {
    match event {
        RuntimeInputEvent::Physical(physical) => {
            let (kind, edge, x, y, label) = match physical.fact() {
                runtime_input::RuntimeInputFact::Key { code, edge } => {
                    (1, edge_value(*edge), 0.0, 0.0, format!("{code:?}"))
                }
                runtime_input::RuntimeInputFact::PointerButton { button, edge } => {
                    (2, edge_value(*edge), 0.0, 0.0, format!("{button:?}"))
                }
                runtime_input::RuntimeInputFact::PointerDelta { x, y } => {
                    (3, 0, x.value(), y.value(), String::new())
                }
                runtime_input::RuntimeInputFact::Wheel { x, y } => {
                    (4, 0, x.value(), y.value(), String::new())
                }
                runtime_input::RuntimeInputFact::ControllerButton { button, edge } => {
                    (5, edge_value(*edge), 0.0, 0.0, format!("{button:?}"))
                }
                runtime_input::RuntimeInputFact::ControllerAxis { axis, value } => {
                    (6, 0, value.value(), 0.0, format!("{axis:?}"))
                }
                runtime_input::RuntimeInputFact::Clear { reason } => {
                    (7, 0, 0.0, 0.0, format!("{reason:?}"))
                }
            };
            input_owned(kind, edge, physical.sequence(), x, y, label)
        }
        RuntimeInputEvent::DirectIntent(intent) => {
            let (kind, x, y) = match intent.value() {
                RuntimeIntentValue::Digital { active } => (8, if active { 1.0 } else { 0.0 }, 0.0),
                RuntimeIntentValue::Axis { value } => (9, value.value(), 0.0),
                RuntimeIntentValue::ProductPayload { .. } => (10, 0.0, 0.0),
            };
            input_owned(kind, 0, intent.sequence(), x, y, intent.intent().to_owned())
        }
    }
}

fn input_owned(
    kind: u32,
    edge: u32,
    sequence: u64,
    x: f32,
    y: f32,
    label: String,
) -> NativeInputOwned {
    let label = label.into_bytes();
    NativeInputOwned {
        kind,
        edge,
        sequence,
        x,
        y,
        label,
    }
}

fn edge_value(edge: runtime_input::PhysicalEdge) -> u32 {
    match edge {
        runtime_input::PhysicalEdge::Pressed => 1,
        runtime_input::PhysicalEdge::Released => 2,
    }
}

fn append_ui(
    outputs: &mut Vec<ProductDevRuntimeOutput>,
    projections: &[RuntimeUiProjectionEnvelope],
) -> Result<(), CsharpProductRuntimeError> {
    for projection in projections {
        outputs.push(ProductDevRuntimeOutput::ui_projection(projection).map_err(host_error)?);
    }
    Ok(())
}

fn append_frame(
    outputs: &mut Vec<ProductDevRuntimeOutput>,
    frame: Option<&render_model::RenderFrameDiff>,
) -> Result<(), CsharpProductRuntimeError> {
    if let Some(frame) = frame {
        outputs.push(ProductDevRuntimeOutput::frame(frame).map_err(host_error)?);
    }
    Ok(())
}

fn host_error(error: product_dev_host::ProductDevHostError) -> CsharpProductRuntimeError {
    CsharpProductRuntimeError::new(error.code(), error.detail().to_owned())
}
fn host_runtime_error(error: product_dev_host::ProductDevHostError) -> ProductDevRuntimeError {
    ProductDevRuntimeError::new(error.code(), error.detail().to_owned())
        .expect("bounded host error")
}

#[derive(Debug)]
pub struct CsharpProductRuntimeError {
    code: &'static str,
    detail: String,
}

impl CsharpProductRuntimeError {
    fn new(code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }
    pub const fn code(&self) -> &'static str {
        self.code
    }
    pub fn detail(&self) -> &str {
        &self.detail
    }
}

impl std::fmt::Display for CsharpProductRuntimeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.detail)
    }
}
impl std::error::Error for CsharpProductRuntimeError {}
impl From<CsharpProductRuntimeError> for ProductDevRuntimeError {
    fn from(error: CsharpProductRuntimeError) -> Self {
        ProductDevRuntimeError::new(error.code, error.detail)
            .expect("fixed bounded NativeAOT error")
    }
}
