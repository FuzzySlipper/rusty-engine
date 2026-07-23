use std::collections::{BTreeMap, BTreeSet, VecDeque};

use core_ids::EntityId;
use core_math::Vec3;
use core_time::{Tick, TickDelta};
use serde::{Deserialize, Serialize};
use world_kernel::{WorldKernel, WorldSnapshot};

use crate::model::{DoorComponent, DoorConfig, DoorState, GameSession, SwitchComponent};
use crate::runtime::GameRuntime;
use crate::scheduler::{ScheduledIntent, ScheduledIntentKind, Scheduler};

pub const GAME_SNAPSHOT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GameSnapshot {
    pub schema_version: u32,
    pub tick: u64,
    pub world: WorldSnapshot,
    pub doors: Vec<DoorSnapshot>,
    pub switches: Vec<SwitchSnapshot>,
    pub controls: Vec<ControlsSnapshot>,
    pub scheduled: Vec<ScheduledSnapshot>,
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
    DuplicateDoor { entity: u64 },
    DuplicateSwitch { entity: u64 },
    UnknownDoorEntity { entity: u64 },
    UnknownSwitchEntity { entity: u64 },
    UnknownControlTarget { switch: u64, target: u64 },
    MissingDoorCapability { entity: u64 },
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
            },
            tick: Tick::new(snapshot.tick),
            scheduler,
            events: VecDeque::new(),
            journal: Vec::new(),
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
