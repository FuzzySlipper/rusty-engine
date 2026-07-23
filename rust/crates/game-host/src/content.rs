use core_ids::EntityId;
use core_math::Vec3;
use core_time::TickDelta;
use engine_spatial::VoxelCollisionScene;
use serde::Deserialize;
use world_kernel::{EntityDefinition, MAX_ABS_TRANSLATION};

use crate::model::{DoorConfig, GameEntityDefinition, GameEntityDefinitionError, GameSession};

pub const PROJECT_CONTENT_SCHEMA_VERSION: u32 = 2;

#[derive(Debug)]
pub struct AdmittedProject {
    pub session: GameSession,
    pub collision_scene: Option<VoxelCollisionScene>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ProjectContent {
    schema_version: u32,
    entities: Vec<AuthoredEntityDefinition>,
    voxel_collision: Option<AuthoredVoxelCollision>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct AuthoredEntityDefinition {
    id: u64,
    name: String,
    translation: Option<[f32; 3]>,
    collision: Option<AuthoredCollision>,
    renderable: Option<AuthoredRenderable>,
    door: Option<AuthoredDoor>,
    switch: Option<AuthoredSwitch>,
    #[serde(default)]
    enemy: bool,
    encounter: Option<AuthoredEncounter>,
    kinematic: Option<AuthoredKinematic>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct AuthoredVoxelCollision {
    voxel_size: f64,
    chunk_size: u32,
    solid_voxels: Vec<[i64; 3]>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct AuthoredCollision {
    enabled: bool,
    static_collider: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct AuthoredRenderable {
    asset: String,
    visible: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct AuthoredDoor {
    open_translation: [f32; 3],
    auto_close_after_ticks: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct AuthoredSwitch {
    controls: Vec<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct AuthoredEncounter {
    members: Vec<u64>,
    exit: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct AuthoredKinematic {
    half_extents: [f32; 3],
    velocity: [f32; 3],
}

#[derive(Debug)]
pub enum ProjectContentError {
    Decode(serde_json::Error),
    UnsupportedSchema { actual: u32 },
    DoorMissingInitialTranslation { entity: EntityId },
    InvalidDoorOpenTranslation { entity: EntityId },
    InvalidAutoCloseTicks { entity: EntityId },
    KinematicMissingCollisionScene { entity: EntityId },
    CollisionScene(engine_spatial::CollisionSceneError),
    Definition(GameEntityDefinitionError),
}

impl std::fmt::Display for ProjectContentError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ProjectContentError {}

pub fn decode_project_content(input: &str) -> Result<AdmittedProject, ProjectContentError> {
    let content: ProjectContent =
        serde_json::from_str(input).map_err(ProjectContentError::Decode)?;
    if content.schema_version != PROJECT_CONTENT_SCHEMA_VERSION {
        return Err(ProjectContentError::UnsupportedSchema {
            actual: content.schema_version,
        });
    }

    let definitions = content
        .entities
        .into_iter()
        .map(authored_definition)
        .collect::<Result<Vec<_>, _>>()?;
    let session =
        GameSession::from_definitions(definitions).map_err(ProjectContentError::Definition)?;
    let collision_scene = content
        .voxel_collision
        .map(|authored| {
            VoxelCollisionScene::from_solid_voxels(
                authored.voxel_size,
                authored.chunk_size,
                authored.solid_voxels,
            )
            .map_err(ProjectContentError::CollisionScene)
        })
        .transpose()?;
    if let Some(entity) = session
        .world()
        .kinematic_bodies()
        .next()
        .map(|body| body.entity)
        .filter(|_| collision_scene.is_none())
    {
        return Err(ProjectContentError::KinematicMissingCollisionScene { entity });
    }
    Ok(AdmittedProject {
        session,
        collision_scene,
    })
}

fn authored_definition(
    authored: AuthoredEntityDefinition,
) -> Result<GameEntityDefinition, ProjectContentError> {
    let entity = EntityId::new(authored.id);
    let initial_translation = authored.translation.map(array_vec3);
    let mut world = EntityDefinition::new(entity, authored.name);
    if let Some(translation) = initial_translation {
        world = world.with_transform(translation);
    }
    if let Some(collision) = authored.collision {
        world = world.with_collision(collision.enabled, collision.static_collider);
    }
    if let Some(renderable) = authored.renderable {
        world = world.with_renderable(renderable.asset, renderable.visible);
    }
    if let Some(kinematic) = authored.kinematic {
        world = world.with_kinematic(
            array_vec3(kinematic.half_extents),
            array_vec3(kinematic.velocity),
        );
    }

    let mut definition = GameEntityDefinition::new(world);
    if let Some(door) = authored.door {
        let Some(closed_translation) = initial_translation else {
            return Err(ProjectContentError::DoorMissingInitialTranslation { entity });
        };
        let open_translation = array_vec3(door.open_translation);
        if !translation_is_valid(open_translation) {
            return Err(ProjectContentError::InvalidDoorOpenTranslation { entity });
        }
        let auto_close_after = match door.auto_close_after_ticks {
            Some(0) => return Err(ProjectContentError::InvalidAutoCloseTicks { entity }),
            Some(ticks) => Some(TickDelta::new(ticks)),
            None => None,
        };
        definition = definition.as_door(DoorConfig::new(
            closed_translation,
            open_translation,
            auto_close_after,
        ));
    }
    if let Some(switch) = authored.switch {
        definition = definition
            .as_switch()
            .controls(switch.controls.into_iter().map(EntityId::new));
    }
    if authored.enemy {
        definition = definition.as_enemy();
    }
    if let Some(encounter) = authored.encounter {
        definition = definition.as_encounter(
            encounter.members.into_iter().map(EntityId::new),
            EntityId::new(encounter.exit),
        );
    }
    Ok(definition)
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
