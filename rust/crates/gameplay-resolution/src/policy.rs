use crate::{Program, ResolutionTraceSink};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyFailure<Rejection, Fault, Suspension> {
    Rejected(Rejection),
    Fault(Fault),
    Suspended(Suspension),
}

pub type PolicyResult<Value, Rejection, Fault, Suspension> =
    Result<Value, PolicyFailure<Rejection, Fault, Suspension>>;

pub type PolicyOutcome<Policy, Value> = PolicyResult<
    Value,
    <Policy as ResolutionPolicy>::Rejection,
    <Policy as ResolutionPolicy>::Fault,
    <Policy as ResolutionPolicy>::Suspension,
>;

pub type PolicyProgram<Policy> = Program<
    <Policy as ResolutionPolicy>::Predicate,
    <Policy as ResolutionPolicy>::Selector,
    <Policy as ResolutionPolicy>::Operation,
>;

pub type PolicyPlan<Policy> = ResolutionPlan<
    <Policy as ResolutionPolicy>::Effect,
    <Policy as ResolutionPolicy>::Event,
    <Policy as ResolutionPolicy>::RawIntent,
    <Policy as ResolutionPolicy>::Evidence,
>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildResolution<RawIntent, Evidence> {
    intent: RawIntent,
    evidence: Vec<Evidence>,
}

impl<RawIntent, Evidence> ChildResolution<RawIntent, Evidence> {
    pub const fn new(intent: RawIntent, evidence: Vec<Evidence>) -> Self {
        Self { intent, evidence }
    }

    pub fn intent(&self) -> &RawIntent {
        &self.intent
    }

    pub fn evidence(&self) -> &[Evidence] {
        &self.evidence
    }

    pub(crate) fn into_parts(self) -> (RawIntent, Vec<Evidence>) {
        (self.intent, self.evidence)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolutionPlan<Effect, Event, RawIntent, Evidence> {
    effects: Vec<Effect>,
    events: Vec<Event>,
    children: Vec<ChildResolution<RawIntent, Evidence>>,
}

impl<Effect, Event, RawIntent, Evidence> Default
    for ResolutionPlan<Effect, Event, RawIntent, Evidence>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<Effect, Event, RawIntent, Evidence> ResolutionPlan<Effect, Event, RawIntent, Evidence> {
    pub const fn new() -> Self {
        Self {
            effects: Vec::new(),
            events: Vec::new(),
            children: Vec::new(),
        }
    }

    pub fn push_effect(&mut self, effect: Effect) {
        self.effects.push(effect);
    }

    pub fn push_event(&mut self, event: Event) {
        self.events.push(event);
    }

    pub fn push_child(&mut self, child: ChildResolution<RawIntent, Evidence>) {
        self.children.push(child);
    }

    pub fn effects(&self) -> &[Effect] {
        &self.effects
    }

    pub fn effects_mut(&mut self) -> &mut Vec<Effect> {
        &mut self.effects
    }

    pub fn events(&self) -> &[Event] {
        &self.events
    }

    pub fn events_mut(&mut self) -> &mut Vec<Event> {
        &mut self.events
    }

    pub fn children(&self) -> &[ChildResolution<RawIntent, Evidence>] {
        &self.children
    }

    pub fn children_mut(&mut self) -> &mut Vec<ChildResolution<RawIntent, Evidence>> {
        &mut self.children
    }

    pub(crate) fn append(&mut self, mut other: Self) {
        self.effects.append(&mut other.effects);
        self.events.append(&mut other.events);
        self.children.append(&mut other.children);
    }

    pub(crate) fn take_children(&mut self) -> Vec<ChildResolution<RawIntent, Evidence>> {
        std::mem::take(&mut self.children)
    }

    pub(crate) fn into_parts(self) -> (Vec<Effect>, Vec<Event>) {
        debug_assert!(self.children.is_empty());
        (self.effects, self.events)
    }
}

pub trait ResolutionPolicy {
    type RawIntent;
    type Intent;
    type Facts;
    type Predicate;
    type Selector;
    type Subject;
    type Operation;
    type Effect;
    type Event;
    type Evidence;
    type Interceptor;
    type TraceDetail;
    type Rejection;
    type Fault;
    type Suspension;

    fn admit(
        &mut self,
        intent: &Self::RawIntent,
        evidence: &[Self::Evidence],
        trace: &mut dyn ResolutionTraceSink<Self::TraceDetail>,
    ) -> PolicyResult<Self::Intent, Self::Rejection, Self::Fault, Self::Suspension>;

    fn gather(
        &mut self,
        intent: &Self::Intent,
        evidence: &[Self::Evidence],
        trace: &mut dyn ResolutionTraceSink<Self::TraceDetail>,
    ) -> PolicyResult<Self::Facts, Self::Rejection, Self::Fault, Self::Suspension>;

    fn check(
        &mut self,
        intent: &Self::Intent,
        facts: &Self::Facts,
        evidence: &[Self::Evidence],
        trace: &mut dyn ResolutionTraceSink<Self::TraceDetail>,
    ) -> PolicyResult<(), Self::Rejection, Self::Fault, Self::Suspension>;

    fn plan(
        &mut self,
        intent: &Self::Intent,
        facts: &Self::Facts,
        evidence: &[Self::Evidence],
        trace: &mut dyn ResolutionTraceSink<Self::TraceDetail>,
    ) -> PolicyOutcome<Self, PolicyProgram<Self>>;

    fn interceptors(
        &mut self,
        _intent: &Self::Intent,
        _facts: &Self::Facts,
        _evidence: &[Self::Evidence],
        _trace: &mut dyn ResolutionTraceSink<Self::TraceDetail>,
    ) -> PolicyResult<Vec<Self::Interceptor>, Self::Rejection, Self::Fault, Self::Suspension> {
        Ok(Vec::new())
    }

    fn evaluate_predicate(
        &mut self,
        predicate: &Self::Predicate,
        subject: Option<&Self::Subject>,
        intent: &Self::Intent,
        facts: &Self::Facts,
        evidence: &[Self::Evidence],
        trace: &mut dyn ResolutionTraceSink<Self::TraceDetail>,
    ) -> PolicyResult<bool, Self::Rejection, Self::Fault, Self::Suspension>;

    fn select(
        &mut self,
        selector: &Self::Selector,
        subject: Option<&Self::Subject>,
        intent: &Self::Intent,
        facts: &Self::Facts,
        evidence: &[Self::Evidence],
        trace: &mut dyn ResolutionTraceSink<Self::TraceDetail>,
    ) -> PolicyResult<Vec<Self::Subject>, Self::Rejection, Self::Fault, Self::Suspension>;

    fn plan_operation(
        &mut self,
        operation: &Self::Operation,
        subject: Option<&Self::Subject>,
        intent: &Self::Intent,
        facts: &Self::Facts,
        evidence: &[Self::Evidence],
        trace: &mut dyn ResolutionTraceSink<Self::TraceDetail>,
    ) -> PolicyOutcome<Self, PolicyPlan<Self>>;

    fn before_commit(
        &mut self,
        _interceptor: &Self::Interceptor,
        _intent: &Self::Intent,
        _facts: &Self::Facts,
        _evidence: &[Self::Evidence],
        _plan: &mut ResolutionPlan<Self::Effect, Self::Event, Self::RawIntent, Self::Evidence>,
        _trace: &mut dyn ResolutionTraceSink<Self::TraceDetail>,
    ) -> PolicyResult<(), Self::Rejection, Self::Fault, Self::Suspension> {
        Ok(())
    }
}
