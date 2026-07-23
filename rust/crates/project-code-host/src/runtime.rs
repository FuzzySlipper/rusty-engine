use std::collections::{BTreeMap, BTreeSet};

use core_ids::EntityId;
use core_math::Vec3;
use core_time::{Tick, TickDelta};
use serde_json::Value;
use world_kernel::{EntityDefinition, WorldCommand, WorldCommandBatch, WorldKernel};

use crate::types::{
    BehaviorBinding, HostEngineFact, HostEntityView, HostInputEvent, HostProjectionNode,
    InvocationBehavior, ProjectApplyReceipt, ProjectDecision, ProjectDecisionBatch, ProjectDoorIds,
    ProjectFactJournalEntry, ProjectInvocation, ProjectInvocationWave, ProjectRuntimeReadout,
    ProjectScheduleRequest, ProjectStateRecord, ProjectWorldCommand, ScheduledProjectMessage,
    PROJECT_HOST_SCHEMA_VERSION,
};

pub const MAX_PROJECT_STATE_BYTES: usize = 16 * 1024;
pub const MAX_MESSAGE_PAYLOAD_BYTES: usize = 4 * 1024;
pub const MAX_FACT_PAYLOAD_BYTES: usize = 4 * 1024;
pub const MAX_STABLE_NAME_BYTES: usize = 128;
pub const MAX_SCHEDULE_DELAY_TICKS: u64 = 1_000_000;
pub const MAX_TICK_ADVANCE: u64 = 100_000;

#[derive(Debug)]
pub enum ProjectHostError {
    WorldDefinition(world_kernel::EntityDefinitionError),
    InvalidStableName {
        field: &'static str,
    },
    StateTooLarge {
        actual: usize,
        limit: usize,
    },
    PayloadTooLarge {
        field: &'static str,
        actual: usize,
        limit: usize,
    },
    UnknownEntity {
        entity: EntityId,
    },
    NoBehaviorBinding {
        entity: EntityId,
    },
    UnknownBehaviorInstance {
        instance_id: String,
    },
    InvocationAlreadyPending,
    NoInvocationPending,
    InvalidSchema {
        expected: u32,
        actual: u32,
    },
    StaleWorldRevision {
        expected: u64,
        actual: u64,
    },
    DecisionSetMismatch,
    DuplicateDecision {
        invocation_id: u64,
    },
    UnknownInvocation {
        invocation_id: u64,
    },
    StateRevisionMismatch {
        expected: u64,
        actual: u64,
    },
    StateVersionMismatch {
        expected: u32,
        actual: u32,
    },
    ScheduleDelayOutOfRange {
        requested: u64,
        max: u64,
    },
    TickAdvanceLimit {
        requested: u64,
        limit: u64,
    },
    WorldBatch(world_kernel::BatchRejection),
    SnapshotWhileInvocationPending,
    Serialization(serde_json::Error),
}

impl std::fmt::Display for ProjectHostError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ProjectHostError {}

#[derive(Debug, Clone)]
pub(crate) struct PendingInvocation {
    pub instance_id: String,
}

#[derive(Debug, Clone)]
pub(crate) struct PendingWave {
    pub expected_world_revision: u64,
    pub invocations: BTreeMap<u64, PendingInvocation>,
}

#[derive(Debug)]
pub struct ProjectCodeRuntime {
    pub(crate) world: WorldKernel,
    pub(crate) tick: Tick,
    pub(crate) bindings: BTreeMap<String, BehaviorBinding>,
    pub(crate) binding_by_entity: BTreeMap<EntityId, String>,
    pub(crate) states: BTreeMap<String, ProjectStateRecord>,
    pub(crate) scheduled: BTreeMap<(String, String), ScheduledProjectMessage>,
    pub(crate) pending: Option<PendingWave>,
    pub(crate) facts: Vec<ProjectFactJournalEntry>,
    pub(crate) next_invocation_id: u64,
}

impl ProjectCodeRuntime {
    pub fn security_door(initial_state: Value) -> Result<(ProjectDoorIds, Self), ProjectHostError> {
        validate_payload_size("projectState", &initial_state, MAX_PROJECT_STATE_BYTES)
            .map_err(|(actual, limit)| ProjectHostError::StateTooLarge { actual, limit })?;
        let ids = ProjectDoorIds::standard();
        let world = WorldKernel::from_definitions([
            EntityDefinition::new(ids.actor, "player"),
            EntityDefinition::new(ids.switch, "security-switch"),
            EntityDefinition::new(ids.door, "security-door")
                .with_transform(Vec3::ZERO)
                .with_collision(true, true)
                .with_renderable("mesh/security-door", true),
        ])
        .map_err(ProjectHostError::WorldDefinition)?;
        let binding = BehaviorBinding {
            instance_id: "security-door-controller@2".to_owned(),
            behavior_type: "securityDoorController".to_owned(),
            version: 1,
            owner_entity: ids.switch,
            related_entities: vec![ids.door],
        };
        let state = ProjectStateRecord {
            instance_id: binding.instance_id.clone(),
            behavior_type: binding.behavior_type.clone(),
            version: binding.version,
            revision: 0,
            payload: initial_state,
        };
        let mut bindings = BTreeMap::new();
        bindings.insert(binding.instance_id.clone(), binding.clone());
        let mut binding_by_entity = BTreeMap::new();
        binding_by_entity.insert(binding.owner_entity, binding.instance_id.clone());
        let mut states = BTreeMap::new();
        states.insert(state.instance_id.clone(), state);
        Ok((
            ids,
            Self {
                world,
                tick: Tick::ZERO,
                bindings,
                binding_by_entity,
                states,
                scheduled: BTreeMap::new(),
                pending: None,
                facts: Vec::new(),
                next_invocation_id: 1,
            },
        ))
    }

    pub fn world(&self) -> &WorldKernel {
        &self.world
    }

    pub fn tick(&self) -> Tick {
        self.tick
    }

    pub fn begin_interaction(
        &mut self,
        actor: EntityId,
        target: EntityId,
    ) -> Result<ProjectInvocationWave, ProjectHostError> {
        self.ensure_no_pending()?;
        if !self.world.contains(actor) {
            return Err(ProjectHostError::UnknownEntity { entity: actor });
        }
        if !self.world.contains(target) {
            return Err(ProjectHostError::UnknownEntity { entity: target });
        }
        let instance_id = self
            .binding_by_entity
            .get(&target)
            .cloned()
            .ok_or(ProjectHostError::NoBehaviorBinding { entity: target })?;
        let mut events = BTreeMap::new();
        events.insert(
            instance_id,
            vec![HostInputEvent::Interaction {
                actor: actor.raw(),
                target: target.raw(),
            }],
        );
        self.create_wave(events)
    }

    pub fn advance_by(
        &mut self,
        ticks: u64,
    ) -> Result<Option<ProjectInvocationWave>, ProjectHostError> {
        self.ensure_no_pending()?;
        if ticks > MAX_TICK_ADVANCE {
            return Err(ProjectHostError::TickAdvanceLimit {
                requested: ticks,
                limit: MAX_TICK_ADVANCE,
            });
        }
        self.tick = self.tick.advance(TickDelta::new(ticks));
        let due_keys: Vec<(String, String)> = self
            .scheduled
            .iter()
            .filter(|(_, message)| message.due <= self.tick)
            .map(|(key, _)| key.clone())
            .collect();
        if due_keys.is_empty() {
            return Ok(None);
        }
        let mut events = BTreeMap::<String, Vec<HostInputEvent>>::new();
        for key in due_keys {
            let message = self
                .scheduled
                .remove(&key)
                .expect("due key came from scheduler");
            events
                .entry(message.instance_id)
                .or_default()
                .push(HostInputEvent::Message {
                    message_id: message.message_id,
                    message_kind: message.message_kind,
                    payload: message.payload,
                });
        }
        self.create_wave(events).map(Some)
    }

    pub fn apply_decisions(
        &mut self,
        batch: ProjectDecisionBatch,
    ) -> Result<ProjectApplyReceipt, ProjectHostError> {
        if batch.schema_version != PROJECT_HOST_SCHEMA_VERSION {
            return Err(ProjectHostError::InvalidSchema {
                expected: PROJECT_HOST_SCHEMA_VERSION,
                actual: batch.schema_version,
            });
        }
        let pending = self
            .pending
            .clone()
            .ok_or(ProjectHostError::NoInvocationPending)?;
        if batch.expected_world_revision != pending.expected_world_revision
            || self.world.revision() != pending.expected_world_revision
        {
            return Err(ProjectHostError::StaleWorldRevision {
                expected: pending.expected_world_revision,
                actual: self.world.revision(),
            });
        }

        let mut decision_ids = BTreeSet::new();
        for decision in &batch.decisions {
            if !decision_ids.insert(decision.invocation_id) {
                return Err(ProjectHostError::DuplicateDecision {
                    invocation_id: decision.invocation_id,
                });
            }
            if !pending.invocations.contains_key(&decision.invocation_id) {
                return Err(ProjectHostError::UnknownInvocation {
                    invocation_id: decision.invocation_id,
                });
            }
        }
        if decision_ids.len() != pending.invocations.len() {
            return Err(ProjectHostError::DecisionSetMismatch);
        }

        let mut world_commands = Vec::new();
        let mut validated = Vec::new();
        for decision in batch.decisions {
            let pending_invocation = pending
                .invocations
                .get(&decision.invocation_id)
                .expect("decision set validated");
            self.validate_decision(pending_invocation, &decision)?;
            world_commands.extend(decision.commands.iter().cloned().map(project_world_command));
            validated.push((pending_invocation.instance_id.clone(), decision));
        }

        let world_receipt = self
            .world
            .apply_batch(
                WorldCommandBatch::new(world_commands).expecting(pending.expected_world_revision),
            )
            .map_err(ProjectHostError::WorldBatch)?;

        let mut project_facts = Vec::new();
        for (instance_id, decision) in validated {
            if let Some(update) = decision.state_update {
                let state = self
                    .states
                    .get_mut(&instance_id)
                    .expect("decision state validated");
                state.payload = update.payload;
                state.revision = state.revision.saturating_add(1);
            }
            for request in decision.schedules {
                self.apply_schedule(&instance_id, request);
            }
            for fact in decision.facts {
                self.facts.push(ProjectFactJournalEntry {
                    tick: self.tick.raw(),
                    instance_id: instance_id.clone(),
                    fact: fact.clone(),
                });
                project_facts.push(fact);
            }
        }
        self.pending = None;

        Ok(ProjectApplyReceipt {
            tick: self.tick.raw(),
            revision_before: world_receipt.revision_before,
            revision_after: world_receipt.revision_after,
            engine_facts: world_receipt
                .facts
                .into_iter()
                .map(HostEngineFact::from)
                .collect(),
            project_facts,
            state_records: self.states.values().cloned().collect(),
            pending_message_count: self.scheduled.len(),
            projection: self
                .world
                .projection()
                .into_iter()
                .map(HostProjectionNode::from)
                .collect(),
        })
    }

    pub fn readout(&self) -> ProjectRuntimeReadout {
        ProjectRuntimeReadout {
            tick: self.tick.raw(),
            world_revision: self.world.revision(),
            state_records: self.states.values().cloned().collect(),
            pending_message_count: self.scheduled.len(),
            pending_invocation: self.pending.is_some(),
            project_facts: self.facts.clone(),
            projection: self
                .world
                .projection()
                .into_iter()
                .map(HostProjectionNode::from)
                .collect(),
        }
    }

    fn ensure_no_pending(&self) -> Result<(), ProjectHostError> {
        if self.pending.is_some() {
            Err(ProjectHostError::InvocationAlreadyPending)
        } else {
            Ok(())
        }
    }

    fn create_wave(
        &mut self,
        events: BTreeMap<String, Vec<HostInputEvent>>,
    ) -> Result<ProjectInvocationWave, ProjectHostError> {
        let expected_world_revision = self.world.revision();
        let mut invocations = Vec::with_capacity(events.len());
        let mut pending_invocations = BTreeMap::new();

        for (instance_id, events) in events {
            let binding = self.bindings.get(&instance_id).ok_or_else(|| {
                ProjectHostError::UnknownBehaviorInstance {
                    instance_id: instance_id.clone(),
                }
            })?;
            let state = self
                .states
                .get(&instance_id)
                .ok_or_else(|| ProjectHostError::UnknownBehaviorInstance {
                    instance_id: instance_id.clone(),
                })?
                .clone();
            let invocation_id = self.next_invocation_id;
            self.next_invocation_id = self.next_invocation_id.saturating_add(1);
            invocations.push(ProjectInvocation {
                invocation_id,
                behavior: InvocationBehavior {
                    instance_id: binding.instance_id.clone(),
                    behavior_type: binding.behavior_type.clone(),
                    version: binding.version,
                },
                events,
                owner: HostEntityView::from(
                    self.world
                        .view(binding.owner_entity)
                        .expect("binding owner validated at construction"),
                ),
                related: self
                    .world
                    .view_batch(binding.related_entities.iter().copied())
                    .expect("binding relations validated at construction")
                    .into_iter()
                    .map(HostEntityView::from)
                    .collect(),
                state,
            });
            pending_invocations.insert(
                invocation_id,
                PendingInvocation {
                    instance_id: binding.instance_id.clone(),
                },
            );
        }
        self.pending = Some(PendingWave {
            expected_world_revision,
            invocations: pending_invocations,
        });
        Ok(ProjectInvocationWave {
            schema_version: PROJECT_HOST_SCHEMA_VERSION,
            tick: self.tick.raw(),
            expected_world_revision,
            invocations,
        })
    }

    fn validate_decision(
        &self,
        pending: &PendingInvocation,
        decision: &ProjectDecision,
    ) -> Result<(), ProjectHostError> {
        if let Some(update) = &decision.state_update {
            let state = self.states.get(&pending.instance_id).ok_or_else(|| {
                ProjectHostError::UnknownBehaviorInstance {
                    instance_id: pending.instance_id.clone(),
                }
            })?;
            if update.expected_revision != state.revision {
                return Err(ProjectHostError::StateRevisionMismatch {
                    expected: state.revision,
                    actual: update.expected_revision,
                });
            }
            if update.version != state.version {
                return Err(ProjectHostError::StateVersionMismatch {
                    expected: state.version,
                    actual: update.version,
                });
            }
            validate_payload_size("projectState", &update.payload, MAX_PROJECT_STATE_BYTES)
                .map_err(|(actual, limit)| ProjectHostError::StateTooLarge { actual, limit })?;
        }
        for request in &decision.schedules {
            match request {
                ProjectScheduleRequest::Upsert {
                    message_id,
                    due_after_ticks,
                    message_kind,
                    payload,
                } => {
                    validate_stable_name("messageId", message_id)?;
                    validate_stable_name("messageKind", message_kind)?;
                    if *due_after_ticks == 0 || *due_after_ticks > MAX_SCHEDULE_DELAY_TICKS {
                        return Err(ProjectHostError::ScheduleDelayOutOfRange {
                            requested: *due_after_ticks,
                            max: MAX_SCHEDULE_DELAY_TICKS,
                        });
                    }
                    validate_payload_size("messagePayload", payload, MAX_MESSAGE_PAYLOAD_BYTES)
                        .map_err(|(actual, limit)| ProjectHostError::PayloadTooLarge {
                            field: "messagePayload",
                            actual,
                            limit,
                        })?;
                }
                ProjectScheduleRequest::Cancel { message_id } => {
                    validate_stable_name("messageId", message_id)?;
                }
            }
        }
        for fact in &decision.facts {
            validate_stable_name("factKind", &fact.kind)?;
            validate_payload_size("factPayload", &fact.payload, MAX_FACT_PAYLOAD_BYTES).map_err(
                |(actual, limit)| ProjectHostError::PayloadTooLarge {
                    field: "factPayload",
                    actual,
                    limit,
                },
            )?;
        }
        Ok(())
    }

    fn apply_schedule(&mut self, instance_id: &str, request: ProjectScheduleRequest) {
        match request {
            ProjectScheduleRequest::Upsert {
                message_id,
                due_after_ticks,
                message_kind,
                payload,
            } => {
                let key = (instance_id.to_owned(), message_id.clone());
                self.scheduled.insert(
                    key,
                    ScheduledProjectMessage {
                        instance_id: instance_id.to_owned(),
                        message_id,
                        due: self.tick.advance(TickDelta::new(due_after_ticks)),
                        message_kind,
                        payload,
                    },
                );
            }
            ProjectScheduleRequest::Cancel { message_id } => {
                self.scheduled.remove(&(instance_id.to_owned(), message_id));
            }
        }
    }
}

fn project_world_command(command: ProjectWorldCommand) -> WorldCommand {
    match command {
        ProjectWorldCommand::SetTranslation {
            entity,
            translation,
        } => WorldCommand::SetTranslation {
            entity: EntityId::new(entity),
            translation: Vec3::new(translation[0], translation[1], translation[2]),
        },
        ProjectWorldCommand::SetCollisionEnabled { entity, enabled } => {
            WorldCommand::SetCollisionEnabled {
                entity: EntityId::new(entity),
                enabled,
            }
        }
        ProjectWorldCommand::SetVisible { entity, visible } => WorldCommand::SetVisible {
            entity: EntityId::new(entity),
            visible,
        },
    }
}

pub(crate) fn validate_stable_name(
    field: &'static str,
    value: &str,
) -> Result<(), ProjectHostError> {
    if value.is_empty() || value.len() > MAX_STABLE_NAME_BYTES {
        return Err(ProjectHostError::InvalidStableName { field });
    }
    Ok(())
}

pub(crate) fn validate_payload_size(
    _field: &'static str,
    value: &Value,
    limit: usize,
) -> Result<(), (usize, usize)> {
    let actual = serde_json::to_vec(value)
        .map_err(|_| (usize::MAX, limit))?
        .len();
    if actual > limit {
        Err((actual, limit))
    } else {
        Ok(())
    }
}
