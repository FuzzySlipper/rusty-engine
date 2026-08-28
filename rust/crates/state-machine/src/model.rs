use std::collections::BTreeSet;

use core_ids::{EntityId, ModeId, ProcessId};
use entity_state::EntityLifecycle;

use crate::{MAX_DETACHED_DEFINITION_STATES, MAX_DETACHED_DEFINITION_TRANSITIONS};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateMachineSpec {
    pub machine: ProcessId,
    states: BTreeSet<ModeId>,
    transitions: BTreeSet<(ModeId, ModeId)>,
}

impl StateMachineSpec {
    pub fn new(machine: ProcessId, states: impl IntoIterator<Item = ModeId>) -> Self {
        Self {
            machine,
            states: states.into_iter().collect(),
            transitions: BTreeSet::new(),
        }
    }

    pub fn allow(mut self, from: ModeId, to: ModeId) -> Self {
        self.transitions.insert((from, to));
        self
    }

    pub fn contains_state(&self, state: ModeId) -> bool {
        self.states.contains(&state)
    }

    pub fn allows_transition(&self, from: ModeId, to: ModeId) -> bool {
        self.transitions.contains(&(from, to))
    }

    pub fn states(&self) -> impl Iterator<Item = ModeId> + '_ {
        self.states.iter().copied()
    }

    pub fn transitions(&self) -> impl Iterator<Item = (ModeId, ModeId)> + '_ {
        self.transitions.iter().copied()
    }

    /// Validate a definition before retaining or applying it.
    pub fn validate(&self) -> Result<(), StateMachineError> {
        if self.states.is_empty() {
            return Err(StateMachineError::EmptyMachine {
                machine: self.machine,
            });
        }
        for &(from, to) in &self.transitions {
            if !self.states.contains(&from) {
                return Err(StateMachineError::InvalidState {
                    machine: self.machine,
                    state: from,
                });
            }
            if !self.states.contains(&to) {
                return Err(StateMachineError::InvalidState {
                    machine: self.machine,
                    state: to,
                });
            }
        }
        Ok(())
    }

    /// Validate the bounded flat shape used by the detached generated API.
    pub fn validate_detached(&self) -> Result<(), StateMachineError> {
        self.validate()?;
        if self.states.len() > MAX_DETACHED_DEFINITION_STATES {
            return Err(StateMachineError::DefinitionStateLimitExceeded {
                machine: self.machine,
                maximum: MAX_DETACHED_DEFINITION_STATES,
                actual: self.states.len(),
            });
        }
        if self.transitions.len() > MAX_DETACHED_DEFINITION_TRANSITIONS {
            return Err(StateMachineError::DefinitionTransitionLimitExceeded {
                machine: self.machine,
                maximum: MAX_DETACHED_DEFINITION_TRANSITIONS,
                actual: self.transitions.len(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MachineInstance {
    pub entity: EntityId,
    pub machine: ProcessId,
    pub current: ModeId,
    pub revision: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransitionRequest {
    pub entity: EntityId,
    pub machine: ProcessId,
    pub expected: ModeId,
    pub next: ModeId,
    pub expected_revision: Option<u64>,
}

impl TransitionRequest {
    pub const fn new(entity: EntityId, machine: ProcessId, expected: ModeId, next: ModeId) -> Self {
        Self {
            entity,
            machine,
            expected,
            next,
            expected_revision: None,
        }
    }

    pub const fn expecting_revision(mut self, revision: u64) -> Self {
        self.expected_revision = Some(revision);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateMachineFact {
    Attached {
        entity: EntityId,
        machine: ProcessId,
        state: ModeId,
        revision: u64,
    },
    Transitioned {
        entity: EntityId,
        machine: ProcessId,
        from: ModeId,
        to: ModeId,
        revision: u64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransitionApplied {
    pub instance: MachineInstance,
    pub previous: ModeId,
    pub fact: StateMachineFact,
}

/// A caller-owned state-machine value with no entity or lifecycle identity.
///
/// Detached values are intentionally not retained by the Engine. A product
/// keeps this value, decides where it belongs, and supplies it again when it
/// asks the Engine to validate a transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DetachedMachineInstance {
    pub machine: ProcessId,
    pub current: ModeId,
    pub revision: u64,
}

impl DetachedMachineInstance {
    pub const fn new(machine: ProcessId, current: ModeId, revision: u64) -> Self {
        Self {
            machine,
            current,
            revision,
        }
    }
}

/// A guarded transition over a caller-owned detached instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DetachedTransitionRequest {
    pub expected: ModeId,
    pub next: ModeId,
    pub expected_revision: Option<u64>,
}

impl DetachedTransitionRequest {
    pub const fn new(expected: ModeId, next: ModeId) -> Self {
        Self {
            expected,
            next,
            expected_revision: None,
        }
    }

    pub const fn expecting_revision(mut self, revision: u64) -> Self {
        self.expected_revision = Some(revision);
        self
    }
}

/// Fixed receipt for one successful detached transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DetachedTransitionApplied {
    pub instance: DetachedMachineInstance,
    pub previous: ModeId,
    pub revision: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateMachineError {
    EmptyMachine {
        machine: ProcessId,
    },
    MachineAlreadyDefined {
        machine: ProcessId,
    },
    MachineMissing {
        machine: ProcessId,
    },
    EntityMissing {
        entity: EntityId,
    },
    EntityInactive {
        entity: EntityId,
        lifecycle: EntityLifecycle,
    },
    InstanceAlreadyAttached {
        entity: EntityId,
        machine: ProcessId,
    },
    InstanceMissing {
        entity: EntityId,
        machine: ProcessId,
    },
    InvalidState {
        machine: ProcessId,
        state: ModeId,
    },
    InvalidTransition {
        machine: ProcessId,
        from: ModeId,
        to: ModeId,
    },
    StaleCurrentState {
        entity: EntityId,
        machine: ProcessId,
        expected: ModeId,
        actual: ModeId,
    },
    StaleRevision {
        entity: EntityId,
        machine: ProcessId,
        expected: u64,
        actual: u64,
    },
    RevisionOverflow {
        entity: EntityId,
        machine: ProcessId,
    },
    DetachedStaleCurrentState {
        machine: ProcessId,
        expected: ModeId,
        actual: ModeId,
    },
    DetachedStaleRevision {
        machine: ProcessId,
        expected: u64,
        actual: u64,
    },
    DetachedRevisionOverflow {
        machine: ProcessId,
    },
    DuplicateState {
        machine: ProcessId,
        state: ModeId,
    },
    DuplicateTransition {
        machine: ProcessId,
        from: ModeId,
        to: ModeId,
    },
    DefinitionStateLimitExceeded {
        machine: ProcessId,
        maximum: usize,
        actual: usize,
    },
    DefinitionTransitionLimitExceeded {
        machine: ProcessId,
        maximum: usize,
        actual: usize,
    },
}

impl StateMachineError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::EmptyMachine { .. } => "empty-machine",
            Self::MachineAlreadyDefined { .. } => "machine-already-defined",
            Self::MachineMissing { .. } => "machine-missing",
            Self::EntityMissing { .. } => "entity-missing",
            Self::EntityInactive { .. } => "entity-inactive",
            Self::InstanceAlreadyAttached { .. } => "instance-already-attached",
            Self::InstanceMissing { .. } => "instance-missing",
            Self::InvalidState { .. } => "invalid-state",
            Self::InvalidTransition { .. } => "invalid-transition",
            Self::StaleCurrentState { .. } => "stale-current-state",
            Self::StaleRevision { .. } => "stale-revision",
            Self::RevisionOverflow { .. } => "state-machine-revision-overflow",
            Self::DetachedStaleCurrentState { .. } => "detached-stale-current-state",
            Self::DetachedStaleRevision { .. } => "detached-stale-revision",
            Self::DetachedRevisionOverflow { .. } => "detached-revision-overflow",
            Self::DuplicateState { .. } => "duplicate-state",
            Self::DuplicateTransition { .. } => "duplicate-transition",
            Self::DefinitionStateLimitExceeded { .. } => "definition-state-limit-exceeded",
            Self::DefinitionTransitionLimitExceeded { .. } => {
                "definition-transition-limit-exceeded"
            }
        }
    }
}

impl std::fmt::Display for StateMachineError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "state-machine operation rejected: {self:?}")
    }
}

impl std::error::Error for StateMachineError {}

pub fn apply_transition_to_instance(
    spec: &StateMachineSpec,
    instance: MachineInstance,
    request: TransitionRequest,
) -> Result<TransitionApplied, StateMachineError> {
    spec.validate()?;
    if spec.machine != request.machine || instance.machine != request.machine {
        return Err(StateMachineError::MachineMissing {
            machine: request.machine,
        });
    }
    if instance.entity != request.entity {
        return Err(StateMachineError::EntityMissing {
            entity: request.entity,
        });
    }
    if !spec.contains_state(request.next) {
        return Err(StateMachineError::InvalidState {
            machine: request.machine,
            state: request.next,
        });
    }
    if !spec.allows_transition(request.expected, request.next) {
        return Err(StateMachineError::InvalidTransition {
            machine: request.machine,
            from: request.expected,
            to: request.next,
        });
    }
    if instance.current != request.expected {
        return Err(StateMachineError::StaleCurrentState {
            entity: request.entity,
            machine: request.machine,
            expected: request.expected,
            actual: instance.current,
        });
    }
    if let Some(expected) = request.expected_revision {
        if instance.revision != expected {
            return Err(StateMachineError::StaleRevision {
                entity: request.entity,
                machine: request.machine,
                expected,
                actual: instance.revision,
            });
        }
    }
    let revision = instance
        .revision
        .checked_add(1)
        .ok_or(StateMachineError::RevisionOverflow {
            entity: request.entity,
            machine: request.machine,
        })?;
    let updated = MachineInstance {
        current: request.next,
        revision,
        ..instance
    };
    Ok(TransitionApplied {
        instance: updated,
        previous: instance.current,
        fact: StateMachineFact::Transitioned {
            entity: request.entity,
            machine: request.machine,
            from: instance.current,
            to: request.next,
            revision,
        },
    })
}

/// Apply one guarded transition to a caller-owned detached value.
///
/// The Engine validates the retained definition and returns a new value. It
/// never stores or mutates the supplied instance, so stale or invalid input
/// leaves product-owned state untouched.
pub fn apply_detached_transition(
    spec: &StateMachineSpec,
    instance: DetachedMachineInstance,
    request: DetachedTransitionRequest,
) -> Result<DetachedTransitionApplied, StateMachineError> {
    spec.validate_detached()?;
    if spec.machine != instance.machine {
        return Err(StateMachineError::MachineMissing {
            machine: instance.machine,
        });
    }
    if !spec.contains_state(request.next) {
        return Err(StateMachineError::InvalidState {
            machine: instance.machine,
            state: request.next,
        });
    }
    if !spec.allows_transition(request.expected, request.next) {
        return Err(StateMachineError::InvalidTransition {
            machine: instance.machine,
            from: request.expected,
            to: request.next,
        });
    }
    if instance.current != request.expected {
        return Err(StateMachineError::DetachedStaleCurrentState {
            machine: instance.machine,
            expected: request.expected,
            actual: instance.current,
        });
    }
    if let Some(expected) = request.expected_revision {
        if instance.revision != expected {
            return Err(StateMachineError::DetachedStaleRevision {
                machine: instance.machine,
                expected,
                actual: instance.revision,
            });
        }
    }
    let revision =
        instance
            .revision
            .checked_add(1)
            .ok_or(StateMachineError::DetachedRevisionOverflow {
                machine: instance.machine,
            })?;
    let updated = DetachedMachineInstance {
        current: request.next,
        revision,
        ..instance
    };
    Ok(DetachedTransitionApplied {
        instance: updated,
        previous: instance.current,
        revision,
    })
}
