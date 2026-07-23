use std::collections::{BTreeMap, BTreeSet};

use core_ids::EntityId;
use core_time::Tick;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use world_kernel::{WorldKernel, WorldSnapshot};

use crate::runtime::{
    validate_payload_size, validate_stable_name, ProjectCodeRuntime, ProjectHostError,
    MAX_MESSAGE_PAYLOAD_BYTES, MAX_PROJECT_STATE_BYTES,
};
use crate::types::{BehaviorBinding, ProjectStateRecord, ScheduledProjectMessage};

pub const PROJECT_HOST_SNAPSHOT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ProjectHostSnapshot {
    pub schema_version: u32,
    pub tick: u64,
    pub world: WorldSnapshot,
    pub bindings: Vec<BindingSnapshot>,
    pub states: Vec<ProjectStateRecord>,
    pub scheduled: Vec<ScheduledMessageSnapshot>,
    pub next_invocation_id: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct BindingSnapshot {
    pub instance_id: String,
    pub behavior_type: String,
    pub version: u32,
    pub owner_entity: u64,
    pub related_entities: Vec<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ScheduledMessageSnapshot {
    pub instance_id: String,
    pub message_id: String,
    pub due_tick: u64,
    pub message_kind: String,
    pub payload: Value,
}

impl ProjectCodeRuntime {
    pub fn snapshot(&self) -> Result<ProjectHostSnapshot, ProjectHostError> {
        if self.pending.is_some() {
            return Err(ProjectHostError::SnapshotWhileInvocationPending);
        }
        Ok(ProjectHostSnapshot {
            schema_version: PROJECT_HOST_SNAPSHOT_SCHEMA_VERSION,
            tick: self.tick.raw(),
            world: self.world.snapshot(),
            bindings: self
                .bindings
                .values()
                .map(|binding| BindingSnapshot {
                    instance_id: binding.instance_id.clone(),
                    behavior_type: binding.behavior_type.clone(),
                    version: binding.version,
                    owner_entity: binding.owner_entity.raw(),
                    related_entities: binding
                        .related_entities
                        .iter()
                        .map(|entity| entity.raw())
                        .collect(),
                })
                .collect(),
            states: self.states.values().cloned().collect(),
            scheduled: self
                .scheduled
                .values()
                .map(|message| ScheduledMessageSnapshot {
                    instance_id: message.instance_id.clone(),
                    message_id: message.message_id.clone(),
                    due_tick: message.due.raw(),
                    message_kind: message.message_kind.clone(),
                    payload: message.payload.clone(),
                })
                .collect(),
            next_invocation_id: self.next_invocation_id,
        })
    }

    pub fn from_snapshot(snapshot: ProjectHostSnapshot) -> Result<Self, ProjectHostError> {
        if snapshot.schema_version != PROJECT_HOST_SNAPSHOT_SCHEMA_VERSION {
            return Err(ProjectHostError::InvalidSchema {
                expected: PROJECT_HOST_SNAPSHOT_SCHEMA_VERSION,
                actual: snapshot.schema_version,
            });
        }
        let world = WorldKernel::from_snapshot(snapshot.world).map_err(|error| {
            ProjectHostError::Serialization(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                error.to_string(),
            )))
        })?;

        let mut bindings = BTreeMap::new();
        let mut binding_by_entity = BTreeMap::new();
        for binding in snapshot.bindings {
            validate_stable_name("instanceId", &binding.instance_id)?;
            validate_stable_name("behaviorType", &binding.behavior_type)?;
            let owner_entity = EntityId::new(binding.owner_entity);
            if !world.contains(owner_entity) {
                return Err(ProjectHostError::UnknownEntity {
                    entity: owner_entity,
                });
            }
            if bindings.contains_key(&binding.instance_id)
                || binding_by_entity.contains_key(&owner_entity)
            {
                return Err(ProjectHostError::InvalidStableName {
                    field: "duplicateBinding",
                });
            }
            let related_entities: Vec<EntityId> = binding
                .related_entities
                .into_iter()
                .map(EntityId::new)
                .collect();
            for entity in &related_entities {
                if !world.contains(*entity) {
                    return Err(ProjectHostError::UnknownEntity { entity: *entity });
                }
            }
            let value = BehaviorBinding {
                instance_id: binding.instance_id.clone(),
                behavior_type: binding.behavior_type,
                version: binding.version,
                owner_entity,
                related_entities,
            };
            binding_by_entity.insert(owner_entity, binding.instance_id.clone());
            bindings.insert(binding.instance_id, value);
        }

        let mut states = BTreeMap::new();
        for state in snapshot.states {
            let binding = bindings.get(&state.instance_id).ok_or_else(|| {
                ProjectHostError::UnknownBehaviorInstance {
                    instance_id: state.instance_id.clone(),
                }
            })?;
            if state.behavior_type != binding.behavior_type || state.version != binding.version {
                return Err(ProjectHostError::StateVersionMismatch {
                    expected: binding.version,
                    actual: state.version,
                });
            }
            validate_payload_size("projectState", &state.payload, MAX_PROJECT_STATE_BYTES)
                .map_err(|(actual, limit)| ProjectHostError::StateTooLarge { actual, limit })?;
            if states.insert(state.instance_id.clone(), state).is_some() {
                return Err(ProjectHostError::InvalidStableName {
                    field: "duplicateState",
                });
            }
        }
        if states.len() != bindings.len() {
            return Err(ProjectHostError::DecisionSetMismatch);
        }

        let mut scheduled = BTreeMap::new();
        let mut schedule_keys = BTreeSet::new();
        for message in snapshot.scheduled {
            if !bindings.contains_key(&message.instance_id) {
                return Err(ProjectHostError::UnknownBehaviorInstance {
                    instance_id: message.instance_id,
                });
            }
            validate_stable_name("messageId", &message.message_id)?;
            validate_stable_name("messageKind", &message.message_kind)?;
            validate_payload_size(
                "messagePayload",
                &message.payload,
                MAX_MESSAGE_PAYLOAD_BYTES,
            )
            .map_err(|(actual, limit)| ProjectHostError::PayloadTooLarge {
                field: "messagePayload",
                actual,
                limit,
            })?;
            let key = (message.instance_id.clone(), message.message_id.clone());
            if !schedule_keys.insert(key.clone()) {
                return Err(ProjectHostError::InvalidStableName {
                    field: "duplicateSchedule",
                });
            }
            scheduled.insert(
                key,
                ScheduledProjectMessage {
                    instance_id: message.instance_id,
                    message_id: message.message_id,
                    due: Tick::new(message.due_tick),
                    message_kind: message.message_kind,
                    payload: message.payload,
                },
            );
        }

        Ok(Self {
            world,
            tick: Tick::new(snapshot.tick),
            bindings,
            binding_by_entity,
            states,
            scheduled,
            pending: None,
            facts: Vec::new(),
            next_invocation_id: snapshot.next_invocation_id.max(1),
        })
    }
}

pub fn encode_project_snapshot(runtime: &ProjectCodeRuntime) -> Result<String, ProjectHostError> {
    let snapshot = runtime.snapshot()?;
    serde_json::to_string_pretty(&snapshot).map_err(ProjectHostError::Serialization)
}

pub fn decode_project_snapshot(input: &str) -> Result<ProjectCodeRuntime, ProjectHostError> {
    let snapshot: ProjectHostSnapshot =
        serde_json::from_str(input).map_err(ProjectHostError::Serialization)?;
    ProjectCodeRuntime::from_snapshot(snapshot)
}
