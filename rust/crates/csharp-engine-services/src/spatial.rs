use std::{collections::BTreeMap, ffi::c_void};

use core_ids::EntityId;
use core_math::{Vec2, Vec3};
use core_space::{ChunkDims, GridId, VoxelCoord, VoxelGridSpec};
use csharp_engine_abi::*;
use engine_spatial::{
    CharacterContactKind, CharacterControllerCommand, CharacterControllerConfig,
    CharacterControllerService, StaticMeshAssetId, StaticMeshColliderAsset,
    StaticMeshColliderInstance, StaticMeshInstanceId, StaticMeshTransform, VoxelCollisionScene,
};
use entity_state::{
    CharacterMotionComponent, CharacterStance, EntityAuthoringService, EntityDefinition,
    EntityState, EntityTransform, Quat,
};
use svc_pathfinding::{
    find_path_with_policy, propose_direct_nav_movement, DirectNavMovementRequest, NavPathOutcome,
    NavPathQuery, NavProjection, PlanarNavNeighborPolicy,
};

use crate::composition::{
    borrowed_slice, native_quat, native_quat_value, native_vec3, native_vec3_value,
    CsharpEngineServicesError, ABI_OK,
};

/// Engine-owned collision/navigation mechanisms. Player and game state never
/// live here: a character proposal builds its EntityState only for the call.
pub(crate) struct RuntimeSpatialBridge {
    sessions: BTreeMap<u64, SpatialSession>,
    next_session: u64,
}

struct SpatialSession {
    scene: VoxelCollisionScene,
    navigation: Option<(NavProjection, PlanarNavNeighborPolicy)>,
    controller: CharacterControllerService,
}

impl RuntimeSpatialBridge {
    pub(crate) fn new() -> Self {
        Self {
            sessions: BTreeMap::new(),
            next_session: 1,
        }
    }

    fn session_mut(
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
        self.sessions.insert(
            value,
            SpatialSession {
                scene,
                navigation: None,
                controller: CharacterControllerService::default(),
            },
        );
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
        let session = self.session_mut(request.session)?;
        let receipt = session
            .scene
            .replace_static_mesh_colliders(
                session.scene.static_mesh_collision_revision(),
                admitted,
                instances,
            )
            .map_err(|error| {
                CsharpEngineServicesError::new("CSHARP_COLLISION_REPLACE", format!("{error:?}"))
            })?;
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
        let receipt = NativeNavigationReplaceReceipt {
            walkable_cell_count: projection.walkable_len() as u64,
            projection_hash: projection.projection_hash(),
        };
        self.session_mut(request.session)?.navigation = Some((
            projection,
            PlanarNavNeighborPolicy {
                max_step_cells: u8::try_from(request.config.max_step_cells).map_err(|_| {
                    CsharpEngineServicesError::new(
                        "CSHARP_NAVIGATION_CONFIG",
                        "maximum navigation step exceeded u8",
                    )
                })?,
            },
        ));
        Ok(receipt)
    }

    fn propose_navigation(
        &mut self,
        request: NativeNavigationStepRequest,
    ) -> Result<NativeNavigationStepReceipt, CsharpEngineServicesError> {
        let (projection, policy) = self
            .session_mut(request.session)?
            .navigation
            .as_ref()
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_NAVIGATION",
                    "spatial session had no navigation projection",
                )
            })?;
        let from = native_vec3_value(request.from);
        let target = native_vec3_value(request.target);
        let start = projection.grid().world_to_voxel(core_space::WorldPos::new(
            f64::from(from.x),
            f64::from(from.y),
            f64::from(from.z),
        ));
        let goal = projection.grid().world_to_voxel(core_space::WorldPos::new(
            f64::from(target.x),
            f64::from(target.y),
            f64::from(target.z),
        ));
        let path = find_path_with_policy(
            projection,
            NavPathQuery {
                start,
                goal,
                max_visited: request.max_visited as usize,
            },
            *policy,
        )
        .map_err(|error| CsharpEngineServicesError::new("CSHARP_NAVIGATION", error.label()))?;
        if path.outcome == NavPathOutcome::NoPath {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_NAVIGATION",
                "noPath",
            ));
        }
        let next_cell = path.path.get(1).copied().unwrap_or(start);
        let step_target = if next_cell == goal {
            target
        } else {
            let center = projection.grid().voxel_center_world(next_cell);
            Vec3::new(center.x as f32, center.y as f32, center.z as f32)
        };
        let movement = propose_direct_nav_movement(DirectNavMovementRequest {
            from,
            target: step_target,
            max_step_units: request.max_step_units,
        })
        .map_err(|error| CsharpEngineServicesError::new("CSHARP_NAVIGATION", error.label()))?;
        Ok(NativeNavigationStepReceipt {
            next_waypoint: native_vec3(movement.next_waypoint),
            reached: u32::from(next_cell == goal && movement.reached),
            visited: path.visited as u32,
            path_len: path.path.len() as u32,
            reserved: 0,
            projection_hash: projection.projection_hash(),
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
                &character_config(request.config),
                character_command(request.command),
            )
            .map_err(|error| {
                CsharpEngineServicesError::new("CSHARP_CHARACTER_STEP", error.code())
            })?;
        Ok(native_character_receipt(&receipt))
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
        0 => CharacterStance::Standing,
        1 => CharacterStance::Crouched,
        _ => {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_CHARACTER_MOTION",
                "C# stance was unknown",
            ))
        }
    };
    Ok(CharacterMotionComponent {
        controlled_velocity: native_vec3_value(value.controlled_velocity),
        external_velocity: native_vec3_value(value.external_velocity),
        stance,
        grounded: value.grounded != 0,
        jump_buffer_remaining: value.jump_buffer_remaining,
        coyote_remaining: value.coyote_remaining,
        landing_lockout_remaining: value.landing_lockout_remaining,
        support_entity: (value.support_entity_present != 0)
            .then_some(EntityId::new(value.support_entity)),
        support_local_anchor: native_vec3_value(value.support_local_anchor),
        support_previous_translation: native_vec3_value(value.support_previous_translation),
        support_previous_rotation: if value.support_entity_present == 0 {
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
    if motion.support_entity_present == 0 {
        return Ok(None);
    }
    if support.present == 0 || support.entity != motion.support_entity {
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
        0..=2 => Ok(Some(
            EntityDefinition::new(EntityId::new(support.entity), "spatial-support")
                .with_full_transform(native_entity_transform(support.transform)),
        )),
        _ => Err(CsharpEngineServicesError::new(
            "CSHARP_CHARACTER_SUPPORT",
            "C# support lifecycle was unknown",
        )),
    }
}

fn apply_support_lifecycle(
    entities: &mut EntityState,
    entity: EntityId,
    lifecycle: u32,
) -> Result<(), CsharpEngineServicesError> {
    let authoring = EntityAuthoringService;
    let transition = match lifecycle {
        0 => return Ok(()),
        1 => authoring.disable(entities, entities.revision(), entity),
        2 => authoring.destroy(entities, entities.revision(), entity),
        _ => unreachable!("support lifecycle was checked before materialization"),
    };
    transition.map(|_| ()).map_err(|error| {
        CsharpEngineServicesError::new("CSHARP_CHARACTER_SUPPORT", error.to_string())
    })
}

fn character_config(value: NativeCharacterControllerConfig) -> CharacterControllerConfig {
    let mut config = CharacterControllerConfig::responsive_fps();
    config.shape.standing_height = value.standing_height;
    config.shape.crouched_height = value.crouched_height;
    config.shape.radius = value.radius;
    config.shape.contact_skin = value.contact_skin;
    config.ground.forward_speed = value.forward_speed;
    config.ground.backward_speed = value.backward_speed;
    config.ground.strafe_speed = value.strafe_speed;
    config.ground.acceleration = value.acceleration;
    config.ground.braking = value.braking;
    config.ground.friction = value.friction;
    config.vertical.gravity = value.gravity;
    config.vertical.jump_speed = value.jump_speed;
    config.surface.maximum_slope_radians = value.maximum_slope_radians;
    config.surface.maximum_step_height = value.maximum_step_height;
    config.surface.floor_snap_distance = value.floor_snap_distance;
    config.solver.maximum_displacement_per_step = value.maximum_displacement_per_step;
    config
}

fn character_command(value: NativeCharacterControllerCommand) -> CharacterControllerCommand {
    CharacterControllerCommand {
        planar_intent: Vec2::new(value.planar_intent.x, value.planar_intent.y),
        heading_yaw_radians: value.heading_yaw_radians,
        jump_pressed: value.jump_pressed != 0,
        jump_held: value.jump_held != 0,
        crouch_requested: value.crouch_requested != 0,
        external_velocity: Vec3::ZERO,
        external_impulse: Vec3::ZERO,
        step_seconds: value.step_seconds,
        sequence: value.sequence,
    }
}

fn native_character_motion(value: CharacterMotionComponent) -> NativeCharacterMotion {
    NativeCharacterMotion {
        controlled_velocity: native_vec3(value.controlled_velocity),
        external_velocity: native_vec3(value.external_velocity),
        grounded: u32::from(value.grounded),
        stance: match value.stance {
            CharacterStance::Standing => 0,
            CharacterStance::Crouched => 1,
        },
        jump_buffer_remaining: value.jump_buffer_remaining,
        coyote_remaining: value.coyote_remaining,
        landing_lockout_remaining: value.landing_lockout_remaining,
        support_entity_present: u32::from(value.support_entity.is_some()),
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

fn native_character_receipt(
    receipt: &engine_spatial::CharacterControllerReceipt,
) -> NativeCharacterStepReceipt {
    let contact = receipt
        .contacts
        .first()
        .map(|contact| NativeCharacterContact {
            present: 1,
            kind: match contact.kind {
                CharacterContactKind::Ground => 1,
                CharacterContactKind::SteepSlope => 2,
                CharacterContactKind::Wall => 3,
                CharacterContactKind::Ceiling => 4,
            },
            start_solid: u32::from(contact.start_solid),
            reserved: 0,
            point: native_vec3(contact.point),
            normal: native_vec3(contact.normal),
        })
        .unwrap_or_default();
    let ground = receipt
        .ground
        .map(|ground| NativeCharacterGround {
            present: 1,
            reserved: 0,
            point: native_vec3(ground.point),
            normal: native_vec3(ground.normal),
            snapped_distance: ground.snapped_distance,
        })
        .unwrap_or_default();
    NativeCharacterStepReceipt {
        transform: NativeTransform {
            translation: native_vec3(receipt.transform_after.translation),
            rotation: native_quat(receipt.transform_after.rotation),
            scale: native_vec3(receipt.transform_after.scale),
        },
        motion: native_character_motion(receipt.motion_after),
        displacement: native_vec3(receipt.displacement),
        contact,
        ground,
        stepped: u32::from(receipt.step.is_some()),
        step_accepted: u32::from(receipt.step.is_some_and(|step| step.accepted)),
        cast_count: receipt.cast_count as u32,
        recovery_passes: receipt.recovery_passes as u32,
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

pub(crate) fn api(bridge: &mut RuntimeSpatialBridge) -> NativeSpatialApi {
    NativeSpatialApi {
        context: (bridge as *mut RuntimeSpatialBridge).cast(),
        create_session: create_spatial_session,
        destroy_session: destroy_spatial_session,
        replace_collision: replace_spatial_collision,
        replace_navigation: replace_spatial_navigation,
        propose_character_step,
        propose_navigation_step,
    }
}
