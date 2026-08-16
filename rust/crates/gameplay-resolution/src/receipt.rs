use crate::{ResolutionIdentity, ResolutionLimitError, ResolutionTraceRecord};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionMode {
    Preview,
    Apply,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolutionRequest<RawIntent, Evidence> {
    identity: ResolutionIdentity,
    mode: ResolutionMode,
    intent: RawIntent,
    evidence: Vec<Evidence>,
}

impl<RawIntent, Evidence> ResolutionRequest<RawIntent, Evidence> {
    pub const fn new(
        identity: ResolutionIdentity,
        mode: ResolutionMode,
        intent: RawIntent,
        evidence: Vec<Evidence>,
    ) -> Self {
        Self {
            identity,
            mode,
            intent,
            evidence,
        }
    }

    pub const fn identity(&self) -> ResolutionIdentity {
        self.identity
    }

    pub const fn mode(&self) -> ResolutionMode {
        self.mode
    }

    pub fn intent(&self) -> &RawIntent {
        &self.intent
    }

    pub fn evidence(&self) -> &[Evidence] {
        &self.evidence
    }

    pub(crate) fn into_parts(
        self,
    ) -> (ResolutionIdentity, ResolutionMode, RawIntent, Vec<Evidence>) {
        (self.identity, self.mode, self.intent, self.evidence)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttemptStatus<Rejection, Fault, Suspension> {
    Planned,
    Rejected(Rejection),
    Suspended(Suspension),
    Faulted(Fault),
    LimitExceeded(ResolutionLimitError),
    ChildFailed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommitStatus<TransactionError> {
    NotAttempted,
    Previewed,
    Applied,
    Failed(TransactionError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttemptReceipt<
    RawIntent,
    Intent,
    Facts,
    Evidence,
    Rejection,
    Fault,
    Suspension,
    TraceDetail,
> {
    pub(crate) identity: ResolutionIdentity,
    pub(crate) raw_intent: RawIntent,
    pub(crate) evidence: Vec<Evidence>,
    pub(crate) intent: Option<Intent>,
    pub(crate) facts: Option<Facts>,
    pub(crate) status: AttemptStatus<Rejection, Fault, Suspension>,
    pub(crate) trace: Vec<ResolutionTraceRecord<TraceDetail>>,
    pub(crate) children: Vec<Self>,
}

impl<RawIntent, Intent, Facts, Evidence, Rejection, Fault, Suspension, TraceDetail>
    AttemptReceipt<RawIntent, Intent, Facts, Evidence, Rejection, Fault, Suspension, TraceDetail>
{
    pub const fn identity(&self) -> ResolutionIdentity {
        self.identity
    }

    pub fn raw_intent(&self) -> &RawIntent {
        &self.raw_intent
    }

    pub fn evidence(&self) -> &[Evidence] {
        &self.evidence
    }

    pub const fn intent(&self) -> Option<&Intent> {
        self.intent.as_ref()
    }

    pub const fn facts(&self) -> Option<&Facts> {
        self.facts.as_ref()
    }

    pub const fn status(&self) -> &AttemptStatus<Rejection, Fault, Suspension> {
        &self.status
    }

    pub fn trace(&self) -> &[ResolutionTraceRecord<TraceDetail>] {
        &self.trace
    }

    pub fn children(&self) -> &[Self] {
        &self.children
    }

    pub(crate) fn is_planned(&self) -> bool {
        matches!(self.status, AttemptStatus::Planned)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolutionReceipt<
    RawIntent,
    Intent,
    Facts,
    Evidence,
    Effect,
    Event,
    Rejection,
    Fault,
    Suspension,
    TraceDetail,
    TransactionError,
> {
    mode: ResolutionMode,
    attempt: AttemptReceipt<
        RawIntent,
        Intent,
        Facts,
        Evidence,
        Rejection,
        Fault,
        Suspension,
        TraceDetail,
    >,
    effects: Vec<Effect>,
    events: Vec<Event>,
    commit: CommitStatus<TransactionError>,
}

impl<
        RawIntent,
        Intent,
        Facts,
        Evidence,
        Effect,
        Event,
        Rejection,
        Fault,
        Suspension,
        TraceDetail,
        TransactionError,
    >
    ResolutionReceipt<
        RawIntent,
        Intent,
        Facts,
        Evidence,
        Effect,
        Event,
        Rejection,
        Fault,
        Suspension,
        TraceDetail,
        TransactionError,
    >
{
    pub(crate) const fn new(
        mode: ResolutionMode,
        attempt: AttemptReceipt<
            RawIntent,
            Intent,
            Facts,
            Evidence,
            Rejection,
            Fault,
            Suspension,
            TraceDetail,
        >,
        effects: Vec<Effect>,
        events: Vec<Event>,
        commit: CommitStatus<TransactionError>,
    ) -> Self {
        Self {
            mode,
            attempt,
            effects,
            events,
            commit,
        }
    }

    pub const fn mode(&self) -> ResolutionMode {
        self.mode
    }

    pub const fn attempt(
        &self,
    ) -> &AttemptReceipt<
        RawIntent,
        Intent,
        Facts,
        Evidence,
        Rejection,
        Fault,
        Suspension,
        TraceDetail,
    > {
        &self.attempt
    }

    pub fn effects(&self) -> &[Effect] {
        &self.effects
    }

    pub fn events(&self) -> &[Event] {
        &self.events
    }

    pub const fn commit(&self) -> &CommitStatus<TransactionError> {
        &self.commit
    }

    /// Consume the receipt and return its downstream transaction outcome.
    ///
    /// Runtime consumers use this after projecting any effects, events, and
    /// traces they need so an owned transaction error can re-enter the
    /// downstream game's existing error path without requiring `Clone`.
    pub fn into_commit(self) -> CommitStatus<TransactionError> {
        self.commit
    }

    pub fn succeeded(&self) -> bool {
        self.attempt.is_planned()
            && matches!(self.commit, CommitStatus::Previewed | CommitStatus::Applied)
    }
}
