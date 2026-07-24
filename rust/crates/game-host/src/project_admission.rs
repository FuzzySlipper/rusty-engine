//! One direct admission path from stored project data to concrete game state.

use std::collections::BTreeMap;

use core_assets::{AssetId, AssetKind};
use core_ids::EntityId;
use core_math::Vec3;
use core_time::TickDelta;
use engine_spatial::{GeneratedRoomConfig, VoxelCollisionScene};
use entity_state::{EntityDefinition, MAX_ABS_TRANSLATION};

use crate::combat::{HealthConfig, WeaponConfig};
use crate::content::AdmittedProject;
use crate::definition::{GameEntityDefinition, GameEntityDefinitionError};
use crate::door::DoorConfig;
use crate::navigation::NavigationConfig;
use crate::player::{PlayerControllerConfig, PlayerInputBindings};
use crate::session::GameSession;
use crate::stored_project::{
    decode_stored_project, diagnostic_code, validate_stored_project, StoredEntityDefinition,
    StoredProject, StoredProjectError, StoredScene, StoredVoxelEnvironment,
};

pub fn decode_and_admit_stored_project(input: &str) -> Result<AdmittedProject, StoredProjectError> {
    admit_stored_project(decode_stored_project(input)?)
}

pub fn admit_stored_project(
    document: StoredProject,
) -> Result<AdmittedProject, StoredProjectError> {
    validate_stored_project(&document)?;
    let scene_index = document
        .scenes
        .iter()
        .position(|scene| scene.id == document.entry_scene)
        .expect("validated entry scene");
    let scene = &document.scenes[scene_index];
    let catalog = ProjectAssetCatalog::new(&document);
    catalog.validate_scene(scene, scene_index)?;

    let entity_indexes = index_entities(scene, scene_index)?;
    require_spatial_source(scene, scene_index)?;
    let collision_scene = build_collision_scene(scene, scene_index)?;
    let definitions = scene
        .entities
        .iter()
        .enumerate()
        .map(|(entity_index, entity)| authored_definition(entity, scene_index, entity_index))
        .collect::<Result<Vec<_>, _>>()?;
    let session = GameSession::from_definitions(definitions)
        .map_err(|error| definition_error(error, scene_index, &entity_indexes))?;

    Ok(AdmittedProject {
        session,
        collision_scene,
    })
}

struct ProjectAssetCatalog {
    kinds: BTreeMap<String, AssetKind>,
}

impl ProjectAssetCatalog {
    fn new(document: &StoredProject) -> Self {
        let kinds = document
            .assets
            .iter()
            .map(|asset| {
                let id = AssetId::parse(&asset.id).expect("validated asset identity");
                (asset.id.clone(), id.kind())
            })
            .collect();
        Self { kinds }
    }

    fn validate_scene(
        &self,
        scene: &StoredScene,
        scene_index: usize,
    ) -> Result<(), StoredProjectError> {
        for (entity_index, entity) in scene.entities.iter().enumerate() {
            let Some(renderable) = &entity.renderable else {
                continue;
            };
            let path = format!("scenes[{scene_index}].entities[{entity_index}].renderable.asset");
            let id = AssetId::parse(&renderable.asset).map_err(|error| {
                StoredProjectError::new(diagnostic_code::INVALID_ASSET_ID, &path, error.to_string())
            })?;
            if id.kind() != AssetKind::StaticMesh {
                return Err(StoredProjectError::new(
                    diagnostic_code::WRONG_ASSET_KIND,
                    path,
                    format!("renderable requires `mesh` identity, found `{}`", id.kind()),
                ));
            }
            let Some(kind) = self.kinds.get(id.as_str()) else {
                return Err(StoredProjectError::new(
                    diagnostic_code::MISSING_ASSET,
                    path,
                    format!("asset `{id}` is not declared in `assets`"),
                ));
            };
            if *kind != AssetKind::StaticMesh {
                return Err(StoredProjectError::new(
                    diagnostic_code::WRONG_ASSET_KIND,
                    path,
                    format!("catalog entry `{id}` is `{kind}`, expected `mesh`"),
                ));
            }
        }
        Ok(())
    }
}

fn index_entities(
    scene: &StoredScene,
    scene_index: usize,
) -> Result<BTreeMap<EntityId, usize>, StoredProjectError> {
    let mut indexes = BTreeMap::new();
    for (entity_index, entity) in scene.entities.iter().enumerate() {
        let id = EntityId::new(entity.id);
        if let Some(first) = indexes.insert(id, entity_index) {
            return Err(StoredProjectError::new(
                diagnostic_code::DUPLICATE_ENTITY,
                format!("scenes[{scene_index}].entities[{entity_index}].id"),
                format!(
                    "entity {} was already declared at scenes[{scene_index}].entities[{first}].id",
                    entity.id
                ),
            ));
        }
    }
    Ok(indexes)
}

fn require_spatial_source(
    scene: &StoredScene,
    scene_index: usize,
) -> Result<(), StoredProjectError> {
    if scene.voxel_environment.is_some() {
        return Ok(());
    }
    if let Some((entity_index, _)) = scene
        .entities
        .iter()
        .enumerate()
        .find(|(_, entity)| entity.kinematic.is_some() || entity.navigation.is_some())
    {
        return Err(StoredProjectError::new(
            diagnostic_code::INVALID_SPATIAL,
            format!("scenes[{scene_index}].entities[{entity_index}].kinematic"),
            "kinematic/navigation components require a voxel environment",
        ));
    }
    Ok(())
}

fn build_collision_scene(
    scene: &StoredScene,
    scene_index: usize,
) -> Result<Option<VoxelCollisionScene>, StoredProjectError> {
    let Some(environment) = &scene.voxel_environment else {
        return Ok(None);
    };
    let result = match environment {
        StoredVoxelEnvironment::Solid(environment) => VoxelCollisionScene::from_solid_voxels(
            environment.voxel_size,
            environment.chunk_size,
            environment.solid_voxels.iter().copied(),
        ),
        StoredVoxelEnvironment::GeneratedRoom(environment) => {
            VoxelCollisionScene::from_generated_room(GeneratedRoomConfig {
                seed: environment.seed,
                voxel_size: environment.voxel_size,
                chunk_size: environment.chunk_size,
                width: environment.width,
                height: environment.height,
                length: environment.length,
            })
        }
    };
    result.map(Some).map_err(|error| {
        StoredProjectError::new(
            diagnostic_code::INVALID_SPATIAL,
            format!("scenes[{scene_index}].voxelEnvironment"),
            error.to_string(),
        )
    })
}

fn authored_definition(
    authored: &StoredEntityDefinition,
    scene_index: usize,
    entity_index: usize,
) -> Result<GameEntityDefinition, StoredProjectError> {
    let entity = EntityId::new(authored.id);
    let path =
        |component: &str| format!("scenes[{scene_index}].entities[{entity_index}].{component}");
    let initial_translation = authored.translation.map(array_vec3);
    let mut entity_definition = EntityDefinition::new(entity, authored.name.clone());
    if let Some(translation) = initial_translation {
        entity_definition = entity_definition.with_transform(translation);
    }
    if let Some(collision) = authored.collision {
        entity_definition =
            entity_definition.with_collision(collision.enabled, collision.static_collider);
    }
    if let Some(renderable) = &authored.renderable {
        entity_definition =
            entity_definition.with_renderable(renderable.asset.clone(), renderable.visible);
    }
    if let Some(kinematic) = authored.kinematic {
        entity_definition = entity_definition.with_kinematic(
            array_vec3(kinematic.half_extents),
            array_vec3(kinematic.velocity),
        );
    }

    let mut definition = GameEntityDefinition::new(entity_definition);
    if let Some(door) = authored.door {
        let Some(closed_translation) = initial_translation else {
            return Err(StoredProjectError::new(
                diagnostic_code::INVALID_COMPONENT,
                path("door"),
                "door requires an initial translation",
            ));
        };
        let open_translation = array_vec3(door.open_translation);
        if !translation_is_valid(open_translation) {
            return Err(StoredProjectError::new(
                diagnostic_code::INVALID_COMPONENT,
                path("door.openTranslation"),
                "door open translation is invalid",
            ));
        }
        let auto_close_after = match door.auto_close_after_ticks {
            Some(0) => {
                return Err(StoredProjectError::new(
                    diagnostic_code::INVALID_COMPONENT,
                    path("door.autoCloseAfterTicks"),
                    "auto-close duration must be greater than zero",
                ));
            }
            Some(ticks) => Some(TickDelta::new(ticks)),
            None => None,
        };
        definition = definition.as_door(DoorConfig::new(
            closed_translation,
            open_translation,
            auto_close_after,
        ));
    }
    if let Some(switch) = &authored.switch {
        definition = definition
            .as_switch()
            .controls(switch.controls.iter().copied().map(EntityId::new));
    }
    if authored.enemy {
        definition = definition.as_enemy();
    }
    if let Some(health) = authored.health {
        definition = definition.with_health(HealthConfig {
            max: health.max,
            hitbox_half_extents: array_vec3(health.hitbox_half_extents),
        });
    }
    if let Some(encounter) = &authored.encounter {
        definition = definition.as_encounter(
            encounter.members.iter().copied().map(EntityId::new),
            EntityId::new(encounter.exit),
        );
    }
    if let Some(navigation) = authored.navigation {
        definition = definition.with_navigation(NavigationConfig {
            goal: array_vec3(navigation.goal),
            speed_units_per_second: navigation.speed_units_per_second,
            max_visited: navigation.max_visited,
        });
    }
    if let Some(controller) = &authored.player_controller {
        definition = definition.with_player_controller(PlayerControllerConfig {
            move_speed_units_per_second: controller.move_speed_units_per_second,
            move_step_seconds: controller.move_step_seconds,
            look_degrees_per_unit: controller.look_degrees_per_unit,
            initial_yaw_degrees: controller.initial_yaw_degrees,
            initial_pitch_degrees: controller.initial_pitch_degrees,
            bindings: PlayerInputBindings::new(
                controller.bindings.move_forward.clone(),
                controller.bindings.move_backward.clone(),
                controller.bindings.move_left.clone(),
                controller.bindings.move_right.clone(),
                controller.bindings.mouse_look.clone(),
                controller.bindings.primary_fire.clone(),
            ),
        });
    }
    if let Some(weapon) = authored.weapon {
        definition = definition.with_weapon(WeaponConfig {
            damage: weapon.damage,
            max_distance: weapon.max_distance,
            cooldown_ticks: weapon.cooldown_ticks,
            ammo_capacity: weapon.ammo_capacity,
            muzzle_offset: array_vec3(weapon.muzzle_offset),
        });
    }
    Ok(definition)
}

fn definition_error(
    error: GameEntityDefinitionError,
    scene_index: usize,
    indexes: &BTreeMap<EntityId, usize>,
) -> StoredProjectError {
    use GameEntityDefinitionError as Error;

    let (code, path) = match &error {
        Error::EntityState(source) => match source {
            entity_state::EntityDefinitionError::DuplicateEntity { entity } => (
                diagnostic_code::DUPLICATE_ENTITY,
                entity_path(scene_index, indexes, *entity, "id"),
            ),
            entity_state::EntityDefinitionError::EmptyName { entity } => (
                diagnostic_code::INVALID_COMPONENT,
                entity_path(scene_index, indexes, *entity, "name"),
            ),
            entity_state::EntityDefinitionError::InvalidTranslation { entity } => (
                diagnostic_code::INVALID_COMPONENT,
                entity_path(scene_index, indexes, *entity, "translation"),
            ),
            entity_state::EntityDefinitionError::EmptyAsset { entity } => (
                diagnostic_code::INVALID_COMPONENT,
                entity_path(scene_index, indexes, *entity, "renderable.asset"),
            ),
            entity_state::EntityDefinitionError::KinematicMissingTransform { entity }
            | entity_state::EntityDefinitionError::InvalidKinematicHalfExtents { entity }
            | entity_state::EntityDefinitionError::InvalidKinematicVelocity { entity } => (
                diagnostic_code::INVALID_COMPONENT,
                entity_path(scene_index, indexes, *entity, "kinematic"),
            ),
        },
        Error::DuplicateControlTarget { switch, .. }
        | Error::UnknownControlTarget { switch, .. }
        | Error::ControlTargetIsNotDoor { switch, .. } => (
            diagnostic_code::INVALID_RELATIONSHIP,
            entity_path(scene_index, indexes, *switch, "switch.controls"),
        ),
        Error::ControlsWithoutSwitch { entity } => (
            diagnostic_code::INVALID_RELATIONSHIP,
            entity_path(scene_index, indexes, *entity, "switch"),
        ),
        Error::DoorMissingTransform { entity }
        | Error::DoorMissingCollision { entity }
        | Error::DoorMissingRenderable { entity } => (
            diagnostic_code::INVALID_COMPONENT,
            entity_path(scene_index, indexes, *entity, "door"),
        ),
        Error::EnemyMissingCollision { entity } | Error::EnemyMissingRenderable { entity } => (
            diagnostic_code::INVALID_COMPONENT,
            entity_path(scene_index, indexes, *entity, "enemy"),
        ),
        Error::HealthMissingTransform { entity }
        | Error::HealthMissingCollision { entity }
        | Error::InvalidHealthConfig { entity } => (
            diagnostic_code::INVALID_COMPONENT,
            entity_path(scene_index, indexes, *entity, "health"),
        ),
        Error::NavigationWithoutEnemy { entity }
        | Error::NavigationMissingTransform { entity }
        | Error::NavigationMissingCollision { entity }
        | Error::NavigationMissingKinematic { entity }
        | Error::InvalidNavigationGoal { entity }
        | Error::InvalidNavigationSpeed { entity }
        | Error::InvalidNavigationQueryBudget { entity } => (
            diagnostic_code::INVALID_COMPONENT,
            entity_path(scene_index, indexes, *entity, "navigation"),
        ),
        Error::PlayerControllerMissingTransform { entity }
        | Error::PlayerControllerMissingCollision { entity }
        | Error::PlayerControllerMissingKinematic { entity }
        | Error::PlayerControllerMissingRenderable { entity }
        | Error::InvalidPlayerControllerConfig { entity } => (
            diagnostic_code::INVALID_COMPONENT,
            entity_path(scene_index, indexes, *entity, "playerController"),
        ),
        Error::WeaponWithoutPlayerController { entity } | Error::InvalidWeaponConfig { entity } => {
            (
                diagnostic_code::INVALID_COMPONENT,
                entity_path(scene_index, indexes, *entity, "weapon"),
            )
        }
        Error::EmptyEncounter { encounter }
        | Error::DuplicateEncounterMember { encounter, .. }
        | Error::UnknownEncounterMember { encounter, .. }
        | Error::EncounterMemberIsNotEnemy { encounter, .. } => (
            diagnostic_code::INVALID_RELATIONSHIP,
            entity_path(scene_index, indexes, *encounter, "encounter.members"),
        ),
        Error::UnknownEncounterExit { encounter, .. }
        | Error::EncounterExitIsNotDoor { encounter, .. } => (
            diagnostic_code::INVALID_RELATIONSHIP,
            entity_path(scene_index, indexes, *encounter, "encounter.exit"),
        ),
        Error::EnemyInMultipleEncounters { second, .. } => (
            diagnostic_code::INVALID_RELATIONSHIP,
            entity_path(scene_index, indexes, *second, "encounter.members"),
        ),
    };
    StoredProjectError::new(code, path, error.to_string())
}

fn entity_path(
    scene_index: usize,
    indexes: &BTreeMap<EntityId, usize>,
    entity: EntityId,
    suffix: &str,
) -> String {
    indexes.get(&entity).map_or_else(
        || format!("scenes[{scene_index}].entities"),
        |index| format!("scenes[{scene_index}].entities[{index}].{suffix}"),
    )
}

fn array_vec3(value: [f32; 3]) -> Vec3 {
    Vec3::new(value[0], value[1], value[2])
}

fn translation_is_valid(value: Vec3) -> bool {
    value.x.is_finite()
        && value.y.is_finite()
        && value.z.is_finite()
        && value.x.abs() <= MAX_ABS_TRANSLATION
        && value.y.abs() <= MAX_ABS_TRANSLATION
        && value.z.abs() <= MAX_ABS_TRANSLATION
}
