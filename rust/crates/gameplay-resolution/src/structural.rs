//! Non-generic structural coordinator for trusted product resolution sessions.
//!
//! This owner deliberately records only lifecycle shape.  Product policy,
//! semantic values, effects, events, and transaction state never enter it.

use crate::{
    ResolutionId, ResolutionIdentity, ResolutionIdentityError, ResolutionLimits, ResolutionMode,
    ResolutionPhase, ResolutionTraceKind,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StructuralBudget {
    pub max_evidence: usize,
    pub max_program_nodes: usize,
    pub max_program_depth: u16,
    pub max_interceptors: usize,
    pub max_effects: usize,
    pub max_events: usize,
    pub max_trace_records: usize,
    pub max_children: usize,
}

impl StructuralBudget {
    pub fn validate(self, limits: ResolutionLimits) -> Result<Self, StructuralResolutionError> {
        let fields = [
            ("evidence", self.max_evidence, limits.max_evidence),
            (
                "program nodes",
                self.max_program_nodes,
                limits.max_program_nodes,
            ),
            (
                "program depth",
                usize::from(self.max_program_depth),
                usize::from(limits.max_program_depth),
            ),
            (
                "interceptors",
                self.max_interceptors,
                limits.max_interceptors,
            ),
            ("effects", self.max_effects, limits.max_effects),
            ("events", self.max_events, limits.max_events),
            (
                "trace records",
                self.max_trace_records,
                limits.max_trace_records,
            ),
            (
                "child resolutions",
                self.max_children,
                limits.max_child_resolutions,
            ),
        ];
        for (resource, value, ceiling) in fields {
            if value == 0 || value > ceiling {
                return Err(StructuralResolutionError::Budget {
                    resource,
                    value,
                    ceiling,
                });
            }
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructuralAttemptStatus {
    Open,
    Planned,
    Rejected,
    Suspended,
    Faulted,
    LimitExceeded,
    ChildFailed,
}

impl StructuralAttemptStatus {
    pub const fn terminal(self) -> bool {
        !matches!(self, Self::Open)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructuralCommitStatus {
    NotAttempted,
    Prepared,
    Previewed,
    Applied,
    TransactionFailed,
    Abandoned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructuralResolutionError {
    Limits,
    Identity(ResolutionIdentityError),
    Budget {
        resource: &'static str,
        value: usize,
        ceiling: usize,
    },
    InvalidTransition,
    LimitExceeded {
        resource: &'static str,
        actual: usize,
        maximum: usize,
    },
    ArithmeticOverflow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StructuralAttemptCounts {
    pub evidence: usize,
    pub program_nodes: usize,
    pub program_depth: u16,
    pub interceptors: usize,
    pub effects: usize,
    pub events: usize,
    pub children: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StructuralAttemptRow {
    pub identity: ResolutionIdentity,
    pub is_root: bool,
    pub status: StructuralAttemptStatus,
    pub counts: StructuralAttemptCounts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StructuralTraceRow {
    pub identity: ResolutionIdentity,
    pub phase: ResolutionPhase,
    pub kind: ResolutionTraceKind,
    /// Scalar payload for trace kinds with one count/index; otherwise zero.
    pub scalar: usize,
    /// Predicate outcome when `kind` is `PredicateEvaluated`.
    pub passed: bool,
}

#[derive(Debug, Clone)]
struct Attempt {
    identity: ResolutionIdentity,
    budget: StructuralBudget,
    status: StructuralAttemptStatus,
    next_phase: usize,
    active_phase: Option<ResolutionPhase>,
    counts: StructuralAttemptCounts,
    trace: Vec<StructuralTraceRow>,
}

impl Attempt {
    fn new(identity: ResolutionIdentity, budget: StructuralBudget, evidence: usize) -> Self {
        Self {
            identity,
            budget,
            status: StructuralAttemptStatus::Open,
            next_phase: 0,
            active_phase: None,
            counts: StructuralAttemptCounts {
                evidence,
                program_nodes: 0,
                program_depth: 0,
                interceptors: 0,
                effects: 0,
                events: 0,
                children: 0,
            },
            trace: Vec::new(),
        }
    }
}

const PLANNING_PHASES: [ResolutionPhase; 5] = [
    ResolutionPhase::Admit,
    ResolutionPhase::Gather,
    ResolutionPhase::Check,
    ResolutionPhase::Plan,
    ResolutionPhase::BeforeCommit,
];

fn failure_trace(status: StructuralAttemptStatus) -> ResolutionTraceKind {
    match status {
        StructuralAttemptStatus::Rejected => ResolutionTraceKind::Rejected,
        StructuralAttemptStatus::Suspended => ResolutionTraceKind::Suspended,
        StructuralAttemptStatus::Faulted => ResolutionTraceKind::Faulted,
        StructuralAttemptStatus::LimitExceeded => ResolutionTraceKind::LimitExceeded,
        StructuralAttemptStatus::ChildFailed => ResolutionTraceKind::ChildFailed,
        StructuralAttemptStatus::Open | StructuralAttemptStatus::Planned => {
            unreachable!("not failure")
        }
    }
}

/// A retained, non-generic structural session.  It is intentionally agnostic
/// about every product-defined value and only validates the lifecycle claims
/// published through its narrow methods.
#[derive(Debug)]
pub struct StructuralResolutionSession {
    limits: ResolutionLimits,
    mode: ResolutionMode,
    attempts: Vec<Attempt>,
    active: Vec<usize>,
    next_resolution: u64,
    commit: StructuralCommitStatus,
}

impl StructuralResolutionSession {
    pub fn new(
        root: ResolutionIdentity,
        mode: ResolutionMode,
        limits: ResolutionLimits,
        root_budget: StructuralBudget,
        root_evidence: usize,
    ) -> Result<Self, StructuralResolutionError> {
        let limits = limits
            .validate()
            .map_err(|_| StructuralResolutionError::Limits)?;
        let root_budget = root_budget.validate(limits)?;
        if root_evidence > root_budget.max_evidence {
            return Err(StructuralResolutionError::LimitExceeded {
                resource: "evidence",
                actual: root_evidence,
                maximum: root_budget.max_evidence,
            });
        }
        let next_resolution = root
            .resolution()
            .get()
            .checked_add(1)
            .ok_or(StructuralResolutionError::ArithmeticOverflow)?;
        Ok(Self {
            limits,
            mode,
            attempts: vec![Attempt::new(root, root_budget, root_evidence)],
            active: vec![0],
            next_resolution,
            commit: StructuralCommitStatus::NotAttempted,
        })
    }

    pub const fn mode(&self) -> ResolutionMode {
        self.mode
    }
    pub const fn commit_status(&self) -> StructuralCommitStatus {
        self.commit
    }
    pub const fn limits(&self) -> ResolutionLimits {
        self.limits
    }
    pub fn attempts(&self) -> impl Iterator<Item = StructuralAttemptRow> + '_ {
        self.attempts
            .iter()
            .enumerate()
            .map(|(index, attempt)| StructuralAttemptRow {
                identity: attempt.identity,
                is_root: index == 0,
                status: attempt.status,
                counts: attempt.counts,
            })
    }
    pub fn traces(&self) -> impl Iterator<Item = StructuralTraceRow> + '_ {
        self.attempts
            .iter()
            .flat_map(|attempt| attempt.trace.iter().copied())
    }

    pub fn begin_phase(&mut self, phase: ResolutionPhase) -> Result<(), StructuralResolutionError> {
        self.ensure_open()?;
        let index = self.current()?;
        let attempt = &mut self.attempts[index];
        if attempt.status != StructuralAttemptStatus::Open
            || attempt.active_phase.is_some()
            || PLANNING_PHASES.get(attempt.next_phase).copied() != Some(phase)
        {
            return Err(StructuralResolutionError::InvalidTransition);
        }
        Self::trace(attempt, phase, ResolutionTraceKind::PhaseStarted, 0, false)?;
        attempt.active_phase = Some(phase);
        Ok(())
    }

    pub fn complete_phase(
        &mut self,
        phase: ResolutionPhase,
    ) -> Result<(), StructuralResolutionError> {
        self.ensure_open()?;
        let index = self.current()?;
        let attempt = &mut self.attempts[index];
        if attempt.status != StructuralAttemptStatus::Open || attempt.active_phase != Some(phase) {
            return Err(StructuralResolutionError::InvalidTransition);
        }
        Self::trace(
            attempt,
            phase,
            ResolutionTraceKind::PhaseCompleted,
            0,
            false,
        )?;
        attempt.active_phase = None;
        attempt.next_phase = attempt
            .next_phase
            .checked_add(1)
            .ok_or(StructuralResolutionError::ArithmeticOverflow)?;
        Ok(())
    }

    pub fn record_sequence(&mut self, program_depth: u16) -> Result<(), StructuralResolutionError> {
        self.ensure_phase(ResolutionPhase::Plan)?;
        let index = self.current()?;
        Self::record_program_node(&mut self.attempts[index], program_depth)
    }

    pub fn record_predicate(
        &mut self,
        program_depth: u16,
        passed: bool,
    ) -> Result<(), StructuralResolutionError> {
        self.ensure_phase(ResolutionPhase::Plan)?;
        let index = self.current()?;
        let attempt = &mut self.attempts[index];
        Self::ensure_trace_room(attempt, 1)?;
        Self::record_program_node(attempt, program_depth)?;
        Self::trace(
            attempt,
            ResolutionPhase::Plan,
            ResolutionTraceKind::PredicateEvaluated { passed },
            0,
            passed,
        )
    }

    pub fn record_operation(
        &mut self,
        program_depth: u16,
        effects: usize,
        events: usize,
    ) -> Result<(), StructuralResolutionError> {
        self.ensure_phase(ResolutionPhase::Plan)?;
        let index = self.current()?;
        let attempt = &mut self.attempts[index];
        Self::ensure_trace_room(attempt, 1)?;
        let next_nodes = Self::next_program_nodes(attempt, program_depth)?;
        let next_effects = attempt
            .counts
            .effects
            .checked_add(effects)
            .ok_or(StructuralResolutionError::ArithmeticOverflow)?;
        let next_events = attempt
            .counts
            .events
            .checked_add(events)
            .ok_or(StructuralResolutionError::ArithmeticOverflow)?;
        if next_effects > attempt.budget.max_effects || next_events > attempt.budget.max_events {
            return Err(StructuralResolutionError::LimitExceeded {
                resource: if next_effects > attempt.budget.max_effects {
                    "effects"
                } else {
                    "events"
                },
                actual: if next_effects > attempt.budget.max_effects {
                    next_effects
                } else {
                    next_events
                },
                maximum: if next_effects > attempt.budget.max_effects {
                    attempt.budget.max_effects
                } else {
                    attempt.budget.max_events
                },
            });
        }
        attempt.counts.program_nodes = next_nodes;
        attempt.counts.program_depth = attempt.counts.program_depth.max(program_depth);
        attempt.counts.effects = next_effects;
        attempt.counts.events = next_events;
        Self::trace(
            attempt,
            ResolutionPhase::Plan,
            ResolutionTraceKind::OperationPlanned,
            effects,
            false,
        )
    }

    pub fn record_interceptor(
        &mut self,
        effects: usize,
        events: usize,
    ) -> Result<(), StructuralResolutionError> {
        self.ensure_phase(ResolutionPhase::BeforeCommit)?;
        let index = self.current()?;
        let attempt = &mut self.attempts[index];
        if attempt.counts.children != 0 {
            return Err(StructuralResolutionError::InvalidTransition);
        }
        if effects > attempt.budget.max_effects || events > attempt.budget.max_events {
            return Err(StructuralResolutionError::LimitExceeded {
                resource: if effects > attempt.budget.max_effects {
                    "effects"
                } else {
                    "events"
                },
                actual: if effects > attempt.budget.max_effects {
                    effects
                } else {
                    events
                },
                maximum: if effects > attempt.budget.max_effects {
                    attempt.budget.max_effects
                } else {
                    attempt.budget.max_events
                },
            });
        }
        let index = attempt.counts.interceptors;
        Self::increment(
            &mut attempt.counts.interceptors,
            1,
            attempt.budget.max_interceptors,
            "interceptors",
        )?;
        attempt.counts.effects = effects;
        attempt.counts.events = events;
        Self::trace(
            attempt,
            ResolutionPhase::BeforeCommit,
            ResolutionTraceKind::InterceptorApplied { index },
            index,
            false,
        )
    }

    pub fn begin_child(
        &mut self,
        budget: StructuralBudget,
        evidence: usize,
    ) -> Result<ResolutionIdentity, StructuralResolutionError> {
        self.ensure_phase(ResolutionPhase::BeforeCommit)?;
        let parent_index = self.current()?;
        let budget = budget.validate(self.limits)?;
        if evidence > budget.max_evidence {
            return Err(StructuralResolutionError::LimitExceeded {
                resource: "evidence",
                actual: evidence,
                maximum: budget.max_evidence,
            });
        }
        let global_children = self
            .attempts
            .len()
            .checked_sub(1)
            .and_then(|value| value.checked_add(1))
            .ok_or(StructuralResolutionError::ArithmeticOverflow)?;
        if global_children > self.limits.max_child_resolutions {
            return Err(StructuralResolutionError::LimitExceeded {
                resource: "child resolutions",
                actual: global_children,
                maximum: self.limits.max_child_resolutions,
            });
        }
        let parent = &mut self.attempts[parent_index];
        if usize::from(parent.identity.depth()) >= usize::from(self.limits.max_child_depth) {
            return Err(StructuralResolutionError::LimitExceeded {
                resource: "child depth",
                actual: usize::from(parent.identity.depth()) + 1,
                maximum: usize::from(self.limits.max_child_depth),
            });
        }
        let next_children = parent
            .counts
            .children
            .checked_add(1)
            .ok_or(StructuralResolutionError::ArithmeticOverflow)?;
        if next_children > parent.budget.max_children {
            return Err(StructuralResolutionError::LimitExceeded {
                resource: "child resolutions",
                actual: next_children,
                maximum: parent.budget.max_children,
            });
        }
        let resolution =
            ResolutionId::new(self.next_resolution).map_err(StructuralResolutionError::Identity)?;
        let identity = parent
            .identity
            .child(resolution)
            .map_err(StructuralResolutionError::Identity)?;
        let next_resolution = self
            .next_resolution
            .checked_add(1)
            .ok_or(StructuralResolutionError::ArithmeticOverflow)?;
        Self::trace(
            parent,
            ResolutionPhase::BeforeCommit,
            ResolutionTraceKind::ChildStarted { child: resolution },
            resolution.get() as usize,
            false,
        )?;
        parent.counts.children = next_children;
        self.next_resolution = next_resolution;
        self.attempts.push(Attempt::new(identity, budget, evidence));
        self.active.push(self.attempts.len() - 1);
        Ok(identity)
    }

    pub fn complete_attempt(
        &mut self,
        status: StructuralAttemptStatus,
    ) -> Result<(), StructuralResolutionError> {
        self.ensure_open()?;
        let index = self.current()?;
        let attempt = &self.attempts[index];
        if attempt.status != StructuralAttemptStatus::Open || !status.terminal() {
            return Err(StructuralResolutionError::InvalidTransition);
        }
        if status == StructuralAttemptStatus::Planned
            && (attempt.active_phase.is_some() || attempt.next_phase != PLANNING_PHASES.len())
        {
            return Err(StructuralResolutionError::InvalidTransition);
        }
        let failure_phase = if status != StructuralAttemptStatus::Planned {
            Some(
                attempt
                    .active_phase
                    .or_else(|| {
                        attempt
                            .next_phase
                            .checked_sub(1)
                            .and_then(|index| PLANNING_PHASES.get(index).copied())
                    })
                    .ok_or(StructuralResolutionError::InvalidTransition)?,
            )
        } else {
            None
        };
        let child_counts = attempt.counts;
        let identity = attempt.identity;
        let parent_index = self.active.iter().rev().nth(1).copied();
        if status == StructuralAttemptStatus::Planned {
            if let Some(parent_index) = parent_index {
                let parent = &self.attempts[parent_index];
                let effects = parent
                    .counts
                    .effects
                    .checked_add(child_counts.effects)
                    .ok_or(StructuralResolutionError::ArithmeticOverflow)?;
                let events = parent
                    .counts
                    .events
                    .checked_add(child_counts.events)
                    .ok_or(StructuralResolutionError::ArithmeticOverflow)?;
                if effects > parent.budget.max_effects || events > parent.budget.max_events {
                    return Err(StructuralResolutionError::LimitExceeded {
                        resource: if effects > parent.budget.max_effects {
                            "effects"
                        } else {
                            "events"
                        },
                        actual: if effects > parent.budget.max_effects {
                            effects
                        } else {
                            events
                        },
                        maximum: if effects > parent.budget.max_effects {
                            parent.budget.max_effects
                        } else {
                            parent.budget.max_events
                        },
                    });
                }
            }
        }
        if let Some(failure_phase) = failure_phase {
            Self::trace(
                &mut self.attempts[index],
                failure_phase,
                failure_trace(status),
                0,
                false,
            )?;
        }
        self.attempts[index].status = status;
        self.attempts[index].active_phase = None;
        self.active.pop();
        if let Some(parent_index) = self.active.last().copied() {
            if status == StructuralAttemptStatus::Planned {
                let parent = &mut self.attempts[parent_index];
                parent.counts.effects += child_counts.effects;
                parent.counts.events += child_counts.events;
                Self::trace(
                    parent,
                    ResolutionPhase::BeforeCommit,
                    ResolutionTraceKind::ChildCompleted {
                        child: identity.resolution(),
                    },
                    identity.resolution().get() as usize,
                    false,
                )?;
            } else {
                let parent = &mut self.attempts[parent_index];
                Self::trace(
                    parent,
                    ResolutionPhase::BeforeCommit,
                    ResolutionTraceKind::ChildCompleted {
                        child: identity.resolution(),
                    },
                    identity.resolution().get() as usize,
                    false,
                )?;
                for ancestor in self.active.iter().copied() {
                    let ancestor = &mut self.attempts[ancestor];
                    ancestor.status = StructuralAttemptStatus::ChildFailed;
                    ancestor.active_phase = None;
                }
                self.active.clear();
            }
        }
        Ok(())
    }

    pub fn prepare_finalization(&mut self) -> Result<(), StructuralResolutionError> {
        if self.commit != StructuralCommitStatus::NotAttempted
            || !self.active.is_empty()
            || self.attempts[0].status != StructuralAttemptStatus::Planned
        {
            return Err(StructuralResolutionError::InvalidTransition);
        }
        let root = &mut self.attempts[0];
        // Reserve the entire successful terminal suffix before any commit
        // discipline begins: Commit started/effects staged/final result/commit
        // completed/consequences started/consequences completed.
        Self::ensure_trace_room(root, 6)?;
        Self::trace(
            root,
            ResolutionPhase::Commit,
            ResolutionTraceKind::PhaseStarted,
            0,
            false,
        )?;
        Self::trace(
            root,
            ResolutionPhase::Commit,
            ResolutionTraceKind::EffectsStaged {
                count: root.counts.effects,
            },
            root.counts.effects,
            false,
        )?;
        self.commit = StructuralCommitStatus::Prepared;
        Ok(())
    }

    pub fn finalize_preview(&mut self) -> Result<(), StructuralResolutionError> {
        if self.commit != StructuralCommitStatus::Prepared || self.mode != ResolutionMode::Preview {
            return Err(StructuralResolutionError::InvalidTransition);
        }
        self.finish(
            StructuralCommitStatus::Previewed,
            ResolutionTraceKind::PreviewAborted,
        )
    }
    pub fn finalize_applied(&mut self) -> Result<(), StructuralResolutionError> {
        if self.commit != StructuralCommitStatus::Prepared || self.mode != ResolutionMode::Apply {
            return Err(StructuralResolutionError::InvalidTransition);
        }
        self.finish(
            StructuralCommitStatus::Applied,
            ResolutionTraceKind::CommitApplied,
        )
    }
    pub fn finalize_failed(&mut self) -> Result<(), StructuralResolutionError> {
        if self.commit != StructuralCommitStatus::Prepared {
            return Err(StructuralResolutionError::InvalidTransition);
        }
        self.finish(
            StructuralCommitStatus::TransactionFailed,
            ResolutionTraceKind::TransactionFailed,
        )
    }
    pub fn abandon(&mut self) {
        if matches!(
            self.commit,
            StructuralCommitStatus::NotAttempted | StructuralCommitStatus::Prepared
        ) {
            self.commit = StructuralCommitStatus::Abandoned;
        }
    }

    fn finish(
        &mut self,
        status: StructuralCommitStatus,
        kind: ResolutionTraceKind,
    ) -> Result<(), StructuralResolutionError> {
        let root = &mut self.attempts[0];
        Self::trace(root, ResolutionPhase::Commit, kind, 0, false)?;
        Self::trace(
            root,
            ResolutionPhase::Commit,
            ResolutionTraceKind::PhaseCompleted,
            0,
            false,
        )?;
        if matches!(
            status,
            StructuralCommitStatus::Previewed | StructuralCommitStatus::Applied
        ) {
            Self::trace(
                root,
                ResolutionPhase::Consequences,
                ResolutionTraceKind::PhaseStarted,
                0,
                false,
            )?;
            Self::trace(
                root,
                ResolutionPhase::Consequences,
                ResolutionTraceKind::PhaseCompleted,
                0,
                false,
            )?;
        }
        self.commit = status;
        Ok(())
    }
    fn ensure_open(&self) -> Result<(), StructuralResolutionError> {
        if self.commit == StructuralCommitStatus::NotAttempted {
            Ok(())
        } else {
            Err(StructuralResolutionError::InvalidTransition)
        }
    }
    fn current(&self) -> Result<usize, StructuralResolutionError> {
        self.active
            .last()
            .copied()
            .ok_or(StructuralResolutionError::InvalidTransition)
    }
    fn ensure_phase(&self, phase: ResolutionPhase) -> Result<(), StructuralResolutionError> {
        self.ensure_open()?;
        let index = self.current()?;
        if self.attempts[index].status == StructuralAttemptStatus::Open
            && self.attempts[index].active_phase == Some(phase)
        {
            Ok(())
        } else {
            Err(StructuralResolutionError::InvalidTransition)
        }
    }
    fn increment(
        value: &mut usize,
        by: usize,
        maximum: usize,
        resource: &'static str,
    ) -> Result<(), StructuralResolutionError> {
        let next = value
            .checked_add(by)
            .ok_or(StructuralResolutionError::ArithmeticOverflow)?;
        if next > maximum {
            return Err(StructuralResolutionError::LimitExceeded {
                resource,
                actual: next,
                maximum,
            });
        }
        *value = next;
        Ok(())
    }
    fn next_program_nodes(
        attempt: &Attempt,
        program_depth: u16,
    ) -> Result<usize, StructuralResolutionError> {
        if program_depth > attempt.budget.max_program_depth {
            return Err(StructuralResolutionError::LimitExceeded {
                resource: "program depth",
                actual: usize::from(program_depth),
                maximum: usize::from(attempt.budget.max_program_depth),
            });
        }
        let next = attempt
            .counts
            .program_nodes
            .checked_add(1)
            .ok_or(StructuralResolutionError::ArithmeticOverflow)?;
        if next > attempt.budget.max_program_nodes {
            return Err(StructuralResolutionError::LimitExceeded {
                resource: "program nodes",
                actual: next,
                maximum: attempt.budget.max_program_nodes,
            });
        }
        Ok(next)
    }
    fn record_program_node(
        attempt: &mut Attempt,
        program_depth: u16,
    ) -> Result<(), StructuralResolutionError> {
        let next = Self::next_program_nodes(attempt, program_depth)?;
        attempt.counts.program_nodes = next;
        attempt.counts.program_depth = attempt.counts.program_depth.max(program_depth);
        Ok(())
    }
    fn trace(
        attempt: &mut Attempt,
        phase: ResolutionPhase,
        kind: ResolutionTraceKind,
        scalar: usize,
        passed: bool,
    ) -> Result<(), StructuralResolutionError> {
        Self::ensure_trace_room(attempt, 1)?;
        attempt.trace.push(StructuralTraceRow {
            identity: attempt.identity,
            phase,
            kind,
            scalar,
            passed,
        });
        Ok(())
    }
    fn ensure_trace_room(
        attempt: &Attempt,
        additional: usize,
    ) -> Result<(), StructuralResolutionError> {
        let next = attempt
            .trace
            .len()
            .checked_add(additional)
            .ok_or(StructuralResolutionError::ArithmeticOverflow)?;
        if next > attempt.budget.max_trace_records {
            return Err(StructuralResolutionError::LimitExceeded {
                resource: "trace records",
                actual: next,
                maximum: attempt.budget.max_trace_records,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CorrelationId;

    fn limits() -> ResolutionLimits {
        ResolutionLimits::default()
    }
    fn budget() -> StructuralBudget {
        StructuralBudget {
            max_evidence: 4,
            max_program_nodes: 8,
            max_program_depth: 4,
            max_interceptors: 4,
            max_effects: 8,
            max_events: 8,
            max_trace_records: 64,
            max_children: 4,
        }
    }
    fn session(mode: ResolutionMode) -> StructuralResolutionSession {
        StructuralResolutionSession::new(
            ResolutionIdentity::root(
                ResolutionId::new(10).unwrap(),
                CorrelationId::new(90).unwrap(),
            ),
            mode,
            limits(),
            budget(),
            1,
        )
        .unwrap()
    }
    fn plan(session: &mut StructuralResolutionSession, effects: usize, events: usize) {
        session.begin_phase(ResolutionPhase::Admit).unwrap();
        session.complete_phase(ResolutionPhase::Admit).unwrap();
        session.begin_phase(ResolutionPhase::Gather).unwrap();
        session.complete_phase(ResolutionPhase::Gather).unwrap();
        session.begin_phase(ResolutionPhase::Check).unwrap();
        session.complete_phase(ResolutionPhase::Check).unwrap();
        session.begin_phase(ResolutionPhase::Plan).unwrap();
        session.record_sequence(1).unwrap();
        session.record_predicate(2, true).unwrap();
        session.record_operation(1, effects, events).unwrap();
        session.complete_phase(ResolutionPhase::Plan).unwrap();
        session.begin_phase(ResolutionPhase::BeforeCommit).unwrap();
        session.record_interceptor(effects, events).unwrap();
        session
            .complete_phase(ResolutionPhase::BeforeCommit)
            .unwrap();
    }

    #[test]
    fn preview_preserves_contract_phase_order_and_flat_trace() {
        let mut session = session(ResolutionMode::Preview);
        plan(&mut session, 2, 1);
        session
            .complete_attempt(StructuralAttemptStatus::Planned)
            .unwrap();
        session.prepare_finalization().unwrap();
        session.finalize_preview().unwrap();
        assert_eq!(session.commit_status(), StructuralCommitStatus::Previewed);
        let trace = session.traces().collect::<Vec<_>>();
        assert_eq!(trace.first().unwrap().phase, ResolutionPhase::Admit);
        assert!(trace.iter().any(|row| matches!(
            row.kind,
            ResolutionTraceKind::PredicateEvaluated { passed: true }
        )));
        assert!(matches!(
            trace[trace.len() - 2].phase,
            ResolutionPhase::Consequences
        ));
        assert_eq!(session.attempts().count(), 1);
    }

    #[test]
    fn child_lineage_is_sequential_and_child_failure_blocks_root_finalization() {
        let mut session = session(ResolutionMode::Apply);
        session.begin_phase(ResolutionPhase::Admit).unwrap();
        session.complete_phase(ResolutionPhase::Admit).unwrap();
        session.begin_phase(ResolutionPhase::Gather).unwrap();
        session.complete_phase(ResolutionPhase::Gather).unwrap();
        session.begin_phase(ResolutionPhase::Check).unwrap();
        session.complete_phase(ResolutionPhase::Check).unwrap();
        session.begin_phase(ResolutionPhase::Plan).unwrap();
        session.complete_phase(ResolutionPhase::Plan).unwrap();
        session.begin_phase(ResolutionPhase::BeforeCommit).unwrap();
        session.record_interceptor(0, 0).unwrap();
        let child = session.begin_child(budget(), 0).unwrap();
        assert_eq!(child.parent().unwrap().get(), 10);
        assert_eq!(child.correlation().get(), 90);
        session.begin_phase(ResolutionPhase::Admit).unwrap();
        session
            .complete_attempt(StructuralAttemptStatus::Rejected)
            .unwrap();
        assert!(matches!(
            session.prepare_finalization(),
            Err(StructuralResolutionError::InvalidTransition)
        ));
        let root = session.attempts().next().unwrap();
        assert_eq!(root.status, StructuralAttemptStatus::ChildFailed);
        assert_eq!(session.attempts().nth(1).unwrap().identity.depth(), 1);
        assert!(session
            .traces()
            .any(|row| row.kind == ResolutionTraceKind::Rejected));
    }

    #[test]
    fn limits_reject_before_finalization_without_mutating_counts() {
        let mut session = session(ResolutionMode::Apply);
        session.begin_phase(ResolutionPhase::Admit).unwrap();
        session.complete_phase(ResolutionPhase::Admit).unwrap();
        session.begin_phase(ResolutionPhase::Gather).unwrap();
        session.complete_phase(ResolutionPhase::Gather).unwrap();
        session.begin_phase(ResolutionPhase::Check).unwrap();
        session.complete_phase(ResolutionPhase::Check).unwrap();
        session.begin_phase(ResolutionPhase::Plan).unwrap();
        assert!(matches!(
            session.record_operation(5, 0, 0),
            Err(StructuralResolutionError::LimitExceeded {
                resource: "program depth",
                ..
            })
        ));
        assert_eq!(session.attempts().next().unwrap().counts.program_nodes, 0);
        assert_eq!(
            session.commit_status(),
            StructuralCommitStatus::NotAttempted
        );
    }

    #[test]
    fn structural_program_rows_count_sequence_predicate_and_operation_depth() {
        let mut constrained = budget();
        constrained.max_program_nodes = 3;
        let mut session = StructuralResolutionSession::new(
            ResolutionIdentity::root(
                ResolutionId::new(20).unwrap(),
                CorrelationId::new(21).unwrap(),
            ),
            ResolutionMode::Preview,
            limits(),
            constrained,
            0,
        )
        .unwrap();
        session.begin_phase(ResolutionPhase::Admit).unwrap();
        session.complete_phase(ResolutionPhase::Admit).unwrap();
        session.begin_phase(ResolutionPhase::Gather).unwrap();
        session.complete_phase(ResolutionPhase::Gather).unwrap();
        session.begin_phase(ResolutionPhase::Check).unwrap();
        session.complete_phase(ResolutionPhase::Check).unwrap();
        session.begin_phase(ResolutionPhase::Plan).unwrap();
        session.record_sequence(1).unwrap();
        session.record_predicate(2, true).unwrap();
        session.record_operation(3, 0, 0).unwrap();
        let root = session.attempts().next().unwrap();
        assert_eq!(
            (root.counts.program_nodes, root.counts.program_depth),
            (3, 3)
        );
        assert!(matches!(
            session.record_sequence(4),
            Err(StructuralResolutionError::LimitExceeded {
                resource: "program nodes",
                ..
            })
        ));
        assert!(matches!(
            session.record_operation(5, 0, 0),
            Err(StructuralResolutionError::LimitExceeded {
                resource: "program depth",
                ..
            })
        ));
        assert_eq!(session.attempts().next().unwrap().counts.program_nodes, 3);
    }

    #[test]
    fn successful_children_aggregate_after_before_commit_and_global_cap_holds() {
        let mut session = session(ResolutionMode::Apply);
        session.begin_phase(ResolutionPhase::Admit).unwrap();
        session.complete_phase(ResolutionPhase::Admit).unwrap();
        session.begin_phase(ResolutionPhase::Gather).unwrap();
        session.complete_phase(ResolutionPhase::Gather).unwrap();
        session.begin_phase(ResolutionPhase::Check).unwrap();
        session.complete_phase(ResolutionPhase::Check).unwrap();
        session.begin_phase(ResolutionPhase::Plan).unwrap();
        session.record_operation(1, 1, 1).unwrap();
        session.complete_phase(ResolutionPhase::Plan).unwrap();
        session.begin_phase(ResolutionPhase::BeforeCommit).unwrap();
        session.record_interceptor(1, 1).unwrap();
        session.begin_child(budget(), 2).unwrap();
        plan(&mut session, 2, 3);
        session
            .complete_attempt(StructuralAttemptStatus::Planned)
            .unwrap();
        let root = session.attempts().next().unwrap();
        assert_eq!(
            (
                root.counts.evidence,
                root.counts.effects,
                root.counts.events
            ),
            (1, 3, 4)
        );
        assert!(matches!(
            session.record_interceptor(3, 4),
            Err(StructuralResolutionError::InvalidTransition)
        ));
        assert_eq!(session.attempts().next().unwrap().counts.interceptors, 1);
        session
            .complete_phase(ResolutionPhase::BeforeCommit)
            .unwrap();
        session
            .complete_attempt(StructuralAttemptStatus::Planned)
            .unwrap();

        let mut limits = limits();
        limits.max_child_resolutions = 1;
        let mut capped_budget = budget();
        capped_budget.max_children = 1;
        let mut capped = StructuralResolutionSession::new(
            ResolutionIdentity::root(
                ResolutionId::new(50).unwrap(),
                CorrelationId::new(51).unwrap(),
            ),
            ResolutionMode::Preview,
            limits,
            capped_budget,
            0,
        )
        .unwrap();
        capped.begin_phase(ResolutionPhase::Admit).unwrap();
        capped.complete_phase(ResolutionPhase::Admit).unwrap();
        capped.begin_phase(ResolutionPhase::Gather).unwrap();
        capped.complete_phase(ResolutionPhase::Gather).unwrap();
        capped.begin_phase(ResolutionPhase::Check).unwrap();
        capped.complete_phase(ResolutionPhase::Check).unwrap();
        capped.begin_phase(ResolutionPhase::Plan).unwrap();
        capped.complete_phase(ResolutionPhase::Plan).unwrap();
        capped.begin_phase(ResolutionPhase::BeforeCommit).unwrap();
        capped.record_interceptor(0, 0).unwrap();
        capped.begin_child(capped_budget, 0).unwrap();
        plan(&mut capped, 0, 0);
        capped
            .complete_attempt(StructuralAttemptStatus::Planned)
            .unwrap();
        assert!(matches!(
            capped.begin_child(capped_budget, 0),
            Err(StructuralResolutionError::LimitExceeded {
                resource: "child resolutions",
                ..
            })
        ));
    }

    #[test]
    fn nested_child_failure_marks_all_active_ancestors_terminal() {
        let mut session = session(ResolutionMode::Preview);
        session.begin_phase(ResolutionPhase::Admit).unwrap();
        session.complete_phase(ResolutionPhase::Admit).unwrap();
        session.begin_phase(ResolutionPhase::Gather).unwrap();
        session.complete_phase(ResolutionPhase::Gather).unwrap();
        session.begin_phase(ResolutionPhase::Check).unwrap();
        session.complete_phase(ResolutionPhase::Check).unwrap();
        session.begin_phase(ResolutionPhase::Plan).unwrap();
        session.complete_phase(ResolutionPhase::Plan).unwrap();
        session.begin_phase(ResolutionPhase::BeforeCommit).unwrap();
        session.record_interceptor(0, 0).unwrap();
        session.begin_child(budget(), 0).unwrap();
        session.begin_phase(ResolutionPhase::Admit).unwrap();
        session.complete_phase(ResolutionPhase::Admit).unwrap();
        session.begin_phase(ResolutionPhase::Gather).unwrap();
        session.complete_phase(ResolutionPhase::Gather).unwrap();
        session.begin_phase(ResolutionPhase::Check).unwrap();
        session.complete_phase(ResolutionPhase::Check).unwrap();
        session.begin_phase(ResolutionPhase::Plan).unwrap();
        session.complete_phase(ResolutionPhase::Plan).unwrap();
        session.begin_phase(ResolutionPhase::BeforeCommit).unwrap();
        session.record_interceptor(0, 0).unwrap();
        session.begin_child(budget(), 0).unwrap();
        session.begin_phase(ResolutionPhase::Admit).unwrap();
        session
            .complete_attempt(StructuralAttemptStatus::Faulted)
            .unwrap();
        assert_eq!(session.attempts().count(), 3);
        assert!(session
            .attempts()
            .all(|row| row.status != StructuralAttemptStatus::Open));
    }
}
