use std::collections::{BTreeMap, BTreeSet, VecDeque};

use core_ids::EntityId;
use core_math::Vec3;
use core_time::{Tick, TickDelta};
use engine_spatial::VoxelCollisionScene;
use serde::{Deserialize, Serialize};
use world_kernel::{WorldKernel, WorldSnapshot};

use crate::model::{
    DoorComponent, DoorConfig, DoorState, EncounterComponent, EncounterConfig, EncounterState,
    EnemyComponent, EnemyState, GameSession, SwitchComponent,
};
use crate::runtime::GameRuntime;
use crate::scheduler::{ScheduledIntent, ScheduledIntentKind, Scheduler};

pub const GAME_SNAPSHOT_SCHEMA_VERSION: u32 = 3;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GameSnapshot {
    pub schema_version: u32,
    pub tick: u64,
    pub world: WorldSnapshot,
    pub voxel_collision: Option<VoxelCollisionSnapshot>,
    pub doors: Vec<DoorSnapshot>,
    pub switches: Vec<SwitchSnapshot>,
    pub controls: Vec<ControlsSnapshot>,
    pub enemies: Vec<EnemySnapshot>,
    pub encounters: Vec<EncounterSnapshot>,
    pub scheduled: Vec<ScheduledSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelCollisionSnapshot {
    pub voxel_size: f64,
    pub chunk_size: u32,
    pub solid_voxels: Vec<[i64; 3]>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct DoorSnapshot {
    pub entity: u64,
    pub state: SnapshotDoorState,
    pub closed_translation: [f32; 3],
    pub open_translation: [f32; 3],
    pub auto_close_after_ticks: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SnapshotDoorState {
    Closed,
    Open,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SwitchSnapshot {
    pub entity: u64,
    pub activation_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ControlsSnapshot {
    pub switch: u64,
    pub targets: Vec<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct EnemySnapshot {
    pub entity: u64,
    pub state: SnapshotEnemyState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SnapshotEnemyState {
    Alive,
    Defeated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct EncounterSnapshot {
    pub entity: u64,
    pub state: SnapshotEncounterState,
    pub members: Vec<u64>,
    pub exit: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SnapshotEncounterState {
    Active,
    Cleared,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ScheduledSnapshot {
    pub due_tick: u64,
    pub kind: ScheduledSnapshotKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ScheduledSnapshotKind {
    CloseDoor { door: u64 },
}

#[derive(Debug)]
pub enum GameSnapshotError {
    Encode(serde_json::Error),
    Decode(serde_json::Error),
    UnsupportedSchema { actual: u32 },
    World(world_kernel::WorldSnapshotError),
    CollisionScene(engine_spatial::CollisionSceneError),
    DuplicateDoor { entity: u64 },
    DuplicateSwitch { entity: u64 },
    DuplicateEnemy { entity: u64 },
    DuplicateEncounter { entity: u64 },
    UnknownDoorEntity { entity: u64 },
    UnknownSwitchEntity { entity: u64 },
    UnknownEnemyEntity { entity: u64 },
    UnknownEncounterEntity { entity: u64 },
    UnknownControlTarget { switch: u64, target: u64 },
    UnknownEncounterMember { encounter: u64, member: u64 },
    UnknownEncounterExit { encounter: u64, exit: u64 },
    MissingDoorCapability { entity: u64 },
    MissingEnemyCapability { entity: u64 },
    DuplicateEncounterMember { encounter: u64, member: u64 },
    EnemyInMultipleEncounters { enemy: u64, first: u64, second: u64 },
    DuplicateSchedule { door: u64 },
}

impl std::fmt::Display for GameSnapshotError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for GameSnapshotError {}

impl GameRuntime {
    pub fn snapshot(&self) -> GameSnapshot {
        GameSnapshot {
            schema_version: GAME_SNAPSHOT_SCHEMA_VERSION,
            tick: self.tick.raw(),
            world: self.session.world.snapshot(),
            voxel_collision: self
                .collision_scene
                .as_ref()
                .map(|scene| VoxelCollisionSnapshot {
                    voxel_size: scene.voxel_size(),
                    chunk_size: scene.chunk_size(),
                    solid_voxels: scene.solid_voxels().to_vec(),
                }),
            doors: self
                .session
                .doors
                .iter()
                .map(|(entity, component)| DoorSnapshot {
                    entity: entity.raw(),
                    state: match component.state {
                        DoorState::Closed => SnapshotDoorState::Closed,
                        DoorState::Open => SnapshotDoorState::Open,
                    },
                    closed_translation: component.config.closed_translation.to_array(),
                    open_translation: component.config.open_translation.to_array(),
                    auto_close_after_ticks: component.config.auto_close_after.map(TickDelta::raw),
                })
                .collect(),
            switches: self
                .session
                .switches
                .iter()
                .map(|(entity, component)| SwitchSnapshot {
                    entity: entity.raw(),
                    activation_count: component.activation_count,
                })
                .collect(),
            controls: self
                .session
                .controls
                .iter()
                .map(|(switch, targets)| ControlsSnapshot {
                    switch: switch.raw(),
                    targets: targets.iter().map(|target| target.raw()).collect(),
                })
                .collect(),
            enemies: self
                .session
                .enemies
                .iter()
                .map(|(entity, component)| EnemySnapshot {
                    entity: entity.raw(),
                    state: match component.state {
                        EnemyState::Alive => SnapshotEnemyState::Alive,
                        EnemyState::Defeated => SnapshotEnemyState::Defeated,
                    },
                })
                .collect(),
            encounters: self
                .session
                .encounters
                .iter()
                .map(|(entity, component)| EncounterSnapshot {
                    entity: entity.raw(),
                    state: match component.state {
                        EncounterState::Active => SnapshotEncounterState::Active,
                        EncounterState::Cleared => SnapshotEncounterState::Cleared,
                    },
                    members: component
                        .config
                        .members
                        .iter()
                        .map(|member| member.raw())
                        .collect(),
                    exit: component.config.exit.raw(),
                })
                .collect(),
            scheduled: self
                .scheduler
                .entries()
                .map(|entry| ScheduledSnapshot {
                    due_tick: entry.due.raw(),
                    kind: match entry.kind {
                        ScheduledIntentKind::CloseDoor { door } => {
                            ScheduledSnapshotKind::CloseDoor { door: door.raw() }
                        }
                    },
                })
                .collect(),
        }
    }

    pub fn from_snapshot(snapshot: GameSnapshot) -> Result<Self, GameSnapshotError> {
        if snapshot.schema_version != GAME_SNAPSHOT_SCHEMA_VERSION {
            return Err(GameSnapshotError::UnsupportedSchema {
                actual: snapshot.schema_version,
            });
        }
        let collision_scene = snapshot
            .voxel_collision
            .map(|scene| {
                VoxelCollisionScene::from_solid_voxels(
                    scene.voxel_size,
                    scene.chunk_size,
                    scene.solid_voxels,
                )
                .map_err(GameSnapshotError::CollisionScene)
            })
            .transpose()?;
        let world = WorldKernel::from_snapshot(snapshot.world).map_err(GameSnapshotError::World)?;
        let mut doors = BTreeMap::new();
        let mut door_ids = BTreeSet::new();
        for door in snapshot.doors {
            if !door_ids.insert(door.entity) {
                return Err(GameSnapshotError::DuplicateDoor {
                    entity: door.entity,
                });
            }
            let entity = EntityId::new(door.entity);
            let view = world
                .view(entity)
                .map_err(|_| GameSnapshotError::UnknownDoorEntity {
                    entity: door.entity,
                })?;
            if view.transform.is_none() || view.collision.is_none() || view.renderable.is_none() {
                return Err(GameSnapshotError::MissingDoorCapability {
                    entity: door.entity,
                });
            }
            doors.insert(
                entity,
                DoorComponent {
                    config: DoorConfig {
                        closed_translation: array_vec3(door.closed_translation),
                        open_translation: array_vec3(door.open_translation),
                        auto_close_after: door.auto_close_after_ticks.map(TickDelta::new),
                    },
                    state: match door.state {
                        SnapshotDoorState::Closed => DoorState::Closed,
                        SnapshotDoorState::Open => DoorState::Open,
                    },
                },
            );
        }

        let mut switches = BTreeMap::new();
        let mut switch_ids = BTreeSet::new();
        for switch in snapshot.switches {
            if !switch_ids.insert(switch.entity) {
                return Err(GameSnapshotError::DuplicateSwitch {
                    entity: switch.entity,
                });
            }
            let entity = EntityId::new(switch.entity);
            if !world.contains(entity) {
                return Err(GameSnapshotError::UnknownSwitchEntity {
                    entity: switch.entity,
                });
            }
            switches.insert(
                entity,
                SwitchComponent {
                    activation_count: switch.activation_count,
                },
            );
        }

        let mut controls = BTreeMap::new();
        for control in snapshot.controls {
            let switch = EntityId::new(control.switch);
            if !switches.contains_key(&switch) {
                return Err(GameSnapshotError::UnknownSwitchEntity {
                    entity: control.switch,
                });
            }
            let targets: Vec<EntityId> = control.targets.into_iter().map(EntityId::new).collect();
            for target in &targets {
                if !doors.contains_key(target) {
                    return Err(GameSnapshotError::UnknownControlTarget {
                        switch: control.switch,
                        target: target.raw(),
                    });
                }
            }
            controls.insert(switch, targets);
        }

        let mut enemies = BTreeMap::new();
        let mut enemy_ids = BTreeSet::new();
        for enemy in snapshot.enemies {
            if !enemy_ids.insert(enemy.entity) {
                return Err(GameSnapshotError::DuplicateEnemy {
                    entity: enemy.entity,
                });
            }
            let entity = EntityId::new(enemy.entity);
            let view = world
                .view(entity)
                .map_err(|_| GameSnapshotError::UnknownEnemyEntity {
                    entity: enemy.entity,
                })?;
            if view.collision.is_none() || view.renderable.is_none() {
                return Err(GameSnapshotError::MissingEnemyCapability {
                    entity: enemy.entity,
                });
            }
            enemies.insert(
                entity,
                EnemyComponent {
                    state: match enemy.state {
                        SnapshotEnemyState::Alive => EnemyState::Alive,
                        SnapshotEnemyState::Defeated => EnemyState::Defeated,
                    },
                },
            );
        }

        let mut encounters = BTreeMap::new();
        let mut encounter_ids = BTreeSet::new();
        let mut encounter_by_enemy = BTreeMap::new();
        for encounter in snapshot.encounters {
            if !encounter_ids.insert(encounter.entity) {
                return Err(GameSnapshotError::DuplicateEncounter {
                    entity: encounter.entity,
                });
            }
            if !world.contains(EntityId::new(encounter.entity)) {
                return Err(GameSnapshotError::UnknownEncounterEntity {
                    entity: encounter.entity,
                });
            }
            if !doors.contains_key(&EntityId::new(encounter.exit)) {
                return Err(GameSnapshotError::UnknownEncounterExit {
                    encounter: encounter.entity,
                    exit: encounter.exit,
                });
            }
            let mut unique = BTreeSet::new();
            let mut members = Vec::with_capacity(encounter.members.len());
            for member in encounter.members {
                if !unique.insert(member) {
                    return Err(GameSnapshotError::DuplicateEncounterMember {
                        encounter: encounter.entity,
                        member,
                    });
                }
                if !enemies.contains_key(&EntityId::new(member)) {
                    return Err(GameSnapshotError::UnknownEncounterMember {
                        encounter: encounter.entity,
                        member,
                    });
                }
                if let Some(first) = encounter_by_enemy.insert(member, encounter.entity) {
                    return Err(GameSnapshotError::EnemyInMultipleEncounters {
                        enemy: member,
                        first,
                        second: encounter.entity,
                    });
                }
                members.push(EntityId::new(member));
            }
            encounters.insert(
                EntityId::new(encounter.entity),
                EncounterComponent {
                    config: EncounterConfig {
                        members,
                        exit: EntityId::new(encounter.exit),
                    },
                    state: match encounter.state {
                        SnapshotEncounterState::Active => EncounterState::Active,
                        SnapshotEncounterState::Cleared => EncounterState::Cleared,
                    },
                },
            );
        }

        let mut scheduler = Scheduler::default();
        let mut scheduled_doors = BTreeSet::new();
        for entry in snapshot.scheduled {
            let kind = match entry.kind {
                ScheduledSnapshotKind::CloseDoor { door } => {
                    if !doors.contains_key(&EntityId::new(door)) {
                        return Err(GameSnapshotError::UnknownDoorEntity { entity: door });
                    }
                    if !scheduled_doors.insert(door) {
                        return Err(GameSnapshotError::DuplicateSchedule { door });
                    }
                    ScheduledIntentKind::CloseDoor {
                        door: EntityId::new(door),
                    }
                }
            };
            scheduler.schedule(ScheduledIntent {
                due: Tick::new(entry.due_tick),
                kind,
            });
        }

        Ok(Self {
            session: GameSession {
                world,
                doors,
                switches,
                controls,
                enemies,
                encounters,
            },
            tick: Tick::new(snapshot.tick),
            scheduler,
            events: VecDeque::new(),
            journal: Vec::new(),
            collision_scene,
        })
    }
}

pub fn encode_game_snapshot(runtime: &GameRuntime) -> Result<String, GameSnapshotError> {
    serde_json::to_string_pretty(&runtime.snapshot()).map_err(GameSnapshotError::Encode)
}

pub fn decode_game_snapshot(input: &str) -> Result<GameRuntime, GameSnapshotError> {
    let snapshot: GameSnapshot = serde_json::from_str(input).map_err(GameSnapshotError::Decode)?;
    GameRuntime::from_snapshot(snapshot)
}

fn array_vec3(value: [f32; 3]) -> Vec3 {
    Vec3::new(value[0], value[1], value[2])
}
