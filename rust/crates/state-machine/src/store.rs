use std::collections::BTreeMap;

use core_ids::{EntityId, ModeId, ProcessId};
use entity_state::{EntityLifecycle, EntityState};

use crate::{
    apply_transition_to_instance, MachineInstance, StateMachineError, StateMachineFact,
    StateMachineSpec, TransitionApplied, TransitionRequest,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateMachineStore {
    machines: BTreeMap<ProcessId, StateMachineSpec>,
    instances: BTreeMap<(EntityId, ProcessId), MachineInstance>,
}

impl StateMachineStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn define_machine(&mut self, spec: StateMachineSpec) -> Result<(), StateMachineError> {
        spec.validate()?;
        if self.machines.contains_key(&spec.machine) {
            return Err(StateMachineError::MachineAlreadyDefined {
                machine: spec.machine,
            });
        }
        self.machines.insert(spec.machine, spec);
        Ok(())
    }

    pub fn attach(
        &mut self,
        entities: &EntityState,
        entity: EntityId,
        machine: ProcessId,
        initial: ModeId,
    ) -> Result<StateMachineFact, StateMachineError> {
        require_active_entity(entities, entity)?;
        let spec = self
            .machines
            .get(&machine)
            .ok_or(StateMachineError::MachineMissing { machine })?;
        if !spec.contains_state(initial) {
            return Err(StateMachineError::InvalidState {
                machine,
                state: initial,
            });
        }
        let key = (entity, machine);
        if self.instances.contains_key(&key) {
            return Err(StateMachineError::InstanceAlreadyAttached { entity, machine });
        }
        self.instances.insert(
            key,
            MachineInstance {
                entity,
                machine,
                current: initial,
                revision: 0,
            },
        );
        Ok(StateMachineFact::Attached {
            entity,
            machine,
            state: initial,
            revision: 0,
        })
    }

    pub fn instance(&self, entity: EntityId, machine: ProcessId) -> Option<MachineInstance> {
        self.instances.get(&(entity, machine)).copied()
    }

    pub fn apply_transition(
        &mut self,
        entities: &EntityState,
        request: TransitionRequest,
    ) -> Result<TransitionApplied, StateMachineError> {
        require_active_entity(entities, request.entity)?;
        let spec =
            self.machines
                .get(&request.machine)
                .ok_or(StateMachineError::MachineMissing {
                    machine: request.machine,
                })?;
        let key = (request.entity, request.machine);
        let instance = *self
            .instances
            .get(&key)
            .ok_or(StateMachineError::InstanceMissing {
                entity: request.entity,
                machine: request.machine,
            })?;
        let applied = apply_transition_to_instance(spec, instance, request)?;
        self.instances.insert(key, applied.instance);
        Ok(applied)
    }

    pub fn instances(&self) -> impl Iterator<Item = MachineInstance> + '_ {
        self.instances.values().copied()
    }
}

fn require_active_entity(
    entities: &EntityState,
    entity: EntityId,
) -> Result<(), StateMachineError> {
    match entities.lifecycle(entity) {
        Some(EntityLifecycle::Active) => Ok(()),
        Some(lifecycle) => Err(StateMachineError::EntityInactive { entity, lifecycle }),
        None => Err(StateMachineError::EntityMissing { entity }),
    }
}
