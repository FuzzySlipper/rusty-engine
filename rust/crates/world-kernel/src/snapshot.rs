use std::collections::BTreeSet;

use core_ids::EntityId;
use core_math::Vec3;
use serde::{Deserialize, Serialize};

use crate::model::{
    CollisionCapability, EntityDefinition, EntityLifecycle, KinematicCapability,
    RenderableCapability, TransformCapability, WorldKernel,
};

pub const WORLD_SNAPSHOT_SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct WorldSnapshot {
    pub schema_version: u32,
    pub revision: u64,
    pub entities: Vec<EntitySnapshot>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct EntitySnapshot {
    pub id: u64,
    pub name: String,
    pub lifecycle: SnapshotLifecycle,
    pub translation: Option<[f32; 3]>,
    pub collision: Option<CollisionSnapshot>,
    pub renderable: Option<RenderableSnapshot>,
    pub kinematic: Option<KinematicSnapshot>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SnapshotLifecycle {
    Active,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CollisionSnapshot {
    pub enabled: bool,
    pub static_collider: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RenderableSnapshot {
    pub visible: bool,
    pub asset: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct KinematicSnapshot {
    pub half_extents: [f32; 3],
    pub velocity: [f32; 3],
}

#[derive(Debug)]
pub enum WorldSnapshotError {
    Encode(serde_json::Error),
    Decode(serde_json::Error),
    UnsupportedSchema { actual: u32 },
    DuplicateEntity { entity: u64 },
    InvalidDefinition(crate::model::EntityDefinitionError),
}

impl std::fmt::Display for WorldSnapshotError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for WorldSnapshotError {}

impl WorldKernel {
    pub fn snapshot(&self) -> WorldSnapshot {
        WorldSnapshot {
            schema_version: WORLD_SNAPSHOT_SCHEMA_VERSION,
            revision: self.revision,
            entities: self
                .entities
                .iter()
                .map(|(entity, core)| EntitySnapshot {
                    id: entity.raw(),
                    name: core.name.clone(),
                    lifecycle: match core.lifecycle {
                        EntityLifecycle::Active => SnapshotLifecycle::Active,
                        EntityLifecycle::Disabled => SnapshotLifecycle::Disabled,
                    },
                    translation: self
                        .transforms
                        .get(entity)
                        .map(|value| value.translation.to_array()),
                    collision: self.collisions.get(entity).map(|value| CollisionSnapshot {
                        enabled: value.enabled,
                        static_collider: value.static_collider,
                    }),
                    renderable: self
                        .renderables
                        .get(entity)
                        .map(|value| RenderableSnapshot {
                            visible: value.visible,
                            asset: value.asset.clone(),
                        }),
                    kinematic: self.kinematics.get(entity).map(|value| KinematicSnapshot {
                        half_extents: value.half_extents.to_array(),
                        velocity: value.velocity.to_array(),
                    }),
                })
                .collect(),
        }
    }

    pub fn from_snapshot(snapshot: WorldSnapshot) -> Result<Self, WorldSnapshotError> {
        if snapshot.schema_version != WORLD_SNAPSHOT_SCHEMA_VERSION {
            return Err(WorldSnapshotError::UnsupportedSchema {
                actual: snapshot.schema_version,
            });
        }
        let mut ids = BTreeSet::new();
        let mut lifecycles = Vec::with_capacity(snapshot.entities.len());
        let mut definitions = Vec::with_capacity(snapshot.entities.len());

        for entity in snapshot.entities {
            if !ids.insert(entity.id) {
                return Err(WorldSnapshotError::DuplicateEntity { entity: entity.id });
            }
            let id = EntityId::new(entity.id);
            let mut definition = EntityDefinition::new(id, entity.name);
            definition.transform = entity.translation.map(|value| TransformCapability {
                translation: Vec3::new(value[0], value[1], value[2]),
            });
            definition.collision = entity.collision.map(|value| CollisionCapability {
                enabled: value.enabled,
                static_collider: value.static_collider,
            });
            definition.renderable = entity.renderable.map(|value| RenderableCapability {
                visible: value.visible,
                asset: value.asset,
            });
            definition.kinematic = entity.kinematic.map(|value| KinematicCapability {
                half_extents: Vec3::new(
                    value.half_extents[0],
                    value.half_extents[1],
                    value.half_extents[2],
                ),
                velocity: Vec3::new(value.velocity[0], value.velocity[1], value.velocity[2]),
            });
            lifecycles.push((id, entity.lifecycle));
            definitions.push(definition);
        }

        let mut world = WorldKernel::from_definitions(definitions)
            .map_err(WorldSnapshotError::InvalidDefinition)?;
        world.revision = snapshot.revision;
        for (entity, lifecycle) in lifecycles {
            world
                .entities
                .get_mut(&entity)
                .expect("snapshot definition created entity")
                .lifecycle = match lifecycle {
                SnapshotLifecycle::Active => EntityLifecycle::Active,
                SnapshotLifecycle::Disabled => EntityLifecycle::Disabled,
            };
        }
        Ok(world)
    }
}

pub fn encode_snapshot(world: &WorldKernel) -> Result<String, WorldSnapshotError> {
    serde_json::to_string_pretty(&world.snapshot()).map_err(WorldSnapshotError::Encode)
}

pub fn decode_snapshot(input: &str) -> Result<WorldKernel, WorldSnapshotError> {
    let snapshot: WorldSnapshot =
        serde_json::from_str(input).map_err(WorldSnapshotError::Decode)?;
    WorldKernel::from_snapshot(snapshot)
}
