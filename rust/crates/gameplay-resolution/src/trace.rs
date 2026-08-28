use crate::ResolutionIdentity;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionPhase {
    Admit,
    Gather,
    Check,
    Plan,
    BeforeCommit,
    Commit,
    Consequences,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionTraceKind {
    PhaseStarted,
    PhaseCompleted,
    PolicyDetail,
    PredicateEvaluated { passed: bool },
    OperationPlanned,
    InterceptorApplied { index: usize },
    ChildStarted { child: crate::ResolutionId },
    ChildCompleted { child: crate::ResolutionId },
    EffectsStaged { count: usize },
    CommitApplied,
    PreviewAborted,
    Rejected,
    Suspended,
    Faulted,
    LimitExceeded,
    ChildFailed,
    TransactionFailed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolutionTraceRecord<Detail> {
    identity: ResolutionIdentity,
    phase: ResolutionPhase,
    kind: ResolutionTraceKind,
    detail: Option<Detail>,
}

impl<Detail> ResolutionTraceRecord<Detail> {
    pub(crate) const fn structural(
        identity: ResolutionIdentity,
        phase: ResolutionPhase,
        kind: ResolutionTraceKind,
    ) -> Self {
        Self {
            identity,
            phase,
            kind,
            detail: None,
        }
    }

    pub(crate) const fn policy_detail(
        identity: ResolutionIdentity,
        phase: ResolutionPhase,
        detail: Detail,
    ) -> Self {
        Self {
            identity,
            phase,
            kind: ResolutionTraceKind::PolicyDetail,
            detail: Some(detail),
        }
    }

    pub const fn identity(&self) -> ResolutionIdentity {
        self.identity
    }

    pub const fn phase(&self) -> ResolutionPhase {
        self.phase
    }

    pub const fn kind(&self) -> &ResolutionTraceKind {
        &self.kind
    }

    pub const fn detail(&self) -> Option<&Detail> {
        self.detail.as_ref()
    }
}

pub trait ResolutionTraceSink<Detail> {
    fn record(&mut self, detail: Detail);
}
