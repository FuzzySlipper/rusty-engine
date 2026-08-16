use gameplay_resolution::{
    AttemptStatus, ChildResolution, CommitStatus, CorrelationId, PolicyFailure, PolicyResult,
    Program, ResolutionId, ResolutionIdentity, ResolutionLimits, ResolutionMode, ResolutionPlan,
    ResolutionPolicy, ResolutionRequest, ResolutionTraceKind, ResolutionTraceSink,
    ResolutionTransaction, StandardResolver,
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct RawIntent {
    amount: i64,
    reject: bool,
    suspend: bool,
    spawn_child: bool,
    reject_child: bool,
}

impl RawIntent {
    const fn ordinary(amount: i64) -> Self {
        Self {
            amount,
            reject: false,
            suspend: false,
            spawn_child: false,
            reject_child: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Intent {
    amount: i64,
    spawn_child: bool,
    reject_child: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Facts {
    multiplier: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Predicate {
    Positive,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Operation {
    Change(u16),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Effect {
    subject: u16,
    amount: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Event {
    Planned(u16),
    Intercepted(&'static str),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Evidence(i64);

#[derive(Debug, Clone, PartialEq, Eq)]
enum Interceptor {
    Cap(i64),
    Record(&'static str),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Rejection {
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Fault {
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Suspension(Vec<u8>);

#[derive(Debug, Default)]
struct FixturePolicy {
    subjects: Vec<u16>,
    hook_order: Vec<&'static str>,
}

impl FixturePolicy {
    fn with_subjects(subjects: &[u16]) -> Self {
        Self {
            subjects: subjects.to_vec(),
            hook_order: Vec::new(),
        }
    }
}

impl ResolutionPolicy for FixturePolicy {
    type RawIntent = RawIntent;
    type Intent = Intent;
    type Facts = Facts;
    type Predicate = Predicate;
    type Operation = Operation;
    type Effect = Effect;
    type Event = Event;
    type Evidence = Evidence;
    type Interceptor = Interceptor;
    type TraceDetail = &'static str;
    type Rejection = Rejection;
    type Fault = Fault;
    type Suspension = Suspension;

    fn admit(
        &mut self,
        intent: &RawIntent,
        _evidence: &[Evidence],
        trace: &mut dyn ResolutionTraceSink<&'static str>,
    ) -> PolicyResult<Intent, Rejection, Fault, Suspension> {
        trace.record("intent admitted");
        if intent.suspend {
            return Err(PolicyFailure::Suspended(Suspension(vec![4, 2])));
        }
        if intent.reject {
            return Err(PolicyFailure::Rejected(Rejection::Blocked));
        }
        if intent.amount < 0 {
            return Err(PolicyFailure::Fault(Fault::Invalid));
        }
        Ok(Intent {
            amount: intent.amount,
            spawn_child: intent.spawn_child,
            reject_child: intent.reject_child,
        })
    }

    fn gather(
        &mut self,
        _intent: &Intent,
        evidence: &[Evidence],
        trace: &mut dyn ResolutionTraceSink<&'static str>,
    ) -> PolicyResult<Facts, Rejection, Fault, Suspension> {
        trace.record("facts gathered");
        Ok(Facts {
            multiplier: evidence.first().map_or(1, |value| value.0),
        })
    }

    fn check(
        &mut self,
        _intent: &Intent,
        _facts: &Facts,
        _evidence: &[Evidence],
        trace: &mut dyn ResolutionTraceSink<&'static str>,
    ) -> PolicyResult<(), Rejection, Fault, Suspension> {
        trace.record("policy checked");
        Ok(())
    }

    fn plan(
        &mut self,
        _intent: &Intent,
        _facts: &Facts,
        _evidence: &[Evidence],
        trace: &mut dyn ResolutionTraceSink<&'static str>,
    ) -> PolicyResult<Program<Predicate, Operation>, Rejection, Fault, Suspension> {
        trace.record("program selected");
        Ok(Program::When {
            predicate: Predicate::Positive,
            then_program: Box::new(Program::Sequence {
                steps: self
                    .subjects
                    .iter()
                    .map(|subject| Program::Operation(Operation::Change(*subject)))
                    .collect(),
            }),
            otherwise_program: None,
        })
    }

    fn interceptors(
        &mut self,
        _intent: &Intent,
        _facts: &Facts,
        _evidence: &[Evidence],
        _trace: &mut dyn ResolutionTraceSink<&'static str>,
    ) -> PolicyResult<Vec<Interceptor>, Rejection, Fault, Suspension> {
        Ok(vec![Interceptor::Cap(10), Interceptor::Record("ward")])
    }

    fn evaluate_predicate(
        &mut self,
        _predicate: &Predicate,
        intent: &Intent,
        _facts: &Facts,
        _evidence: &[Evidence],
        _trace: &mut dyn ResolutionTraceSink<&'static str>,
    ) -> PolicyResult<bool, Rejection, Fault, Suspension> {
        Ok(intent.amount > 0)
    }

    fn plan_operation(
        &mut self,
        operation: &Operation,
        intent: &Intent,
        facts: &Facts,
        _evidence: &[Evidence],
        trace: &mut dyn ResolutionTraceSink<&'static str>,
    ) -> PolicyResult<
        ResolutionPlan<Effect, Event, RawIntent, Evidence>,
        Rejection,
        Fault,
        Suspension,
    > {
        trace.record("operation planned");
        let Operation::Change(subject) = operation;
        let mut plan = ResolutionPlan::new();
        plan.push_effect(Effect {
            subject: *subject,
            amount: intent.amount * facts.multiplier,
        });
        plan.push_event(Event::Planned(*subject));
        if intent.spawn_child && *subject == 1 {
            let mut child = RawIntent::ordinary(2);
            child.reject = intent.reject_child;
            plan.push_child(ChildResolution::new(child, vec![Evidence(1)]));
        }
        Ok(plan)
    }

    fn before_commit(
        &mut self,
        interceptor: &Interceptor,
        _intent: &Intent,
        _facts: &Facts,
        _evidence: &[Evidence],
        plan: &mut ResolutionPlan<Effect, Event, RawIntent, Evidence>,
        trace: &mut dyn ResolutionTraceSink<&'static str>,
    ) -> PolicyResult<(), Rejection, Fault, Suspension> {
        match interceptor {
            Interceptor::Cap(maximum) => {
                self.hook_order.push("cap");
                for effect in plan.effects_mut() {
                    effect.amount = effect.amount.min(*maximum);
                }
                trace.record("cap applied");
            }
            Interceptor::Record(label) => {
                self.hook_order.push("record");
                plan.push_event(Event::Intercepted(label));
                trace.record("event recorded");
            }
        }
        Ok(())
    }
}

#[derive(Debug, Default)]
struct FixtureTransaction {
    authority: Vec<Effect>,
    staged: Vec<Effect>,
    commits: usize,
    fail_stage: bool,
    fail_commit: bool,
}

impl ResolutionTransaction for FixtureTransaction {
    type Effect = Effect;
    type Error = &'static str;

    fn stage(&mut self, effect: &Effect) -> Result<(), Self::Error> {
        if self.fail_stage {
            return Err("stage failed");
        }
        self.staged.push(effect.clone());
        Ok(())
    }

    fn commit(&mut self) -> Result<(), Self::Error> {
        if self.fail_commit {
            return Err("commit failed");
        }
        self.authority.append(&mut self.staged);
        self.commits += 1;
        Ok(())
    }

    fn abort(&mut self) {
        self.staged.clear();
    }
}

fn request(mode: ResolutionMode, intent: RawIntent) -> ResolutionRequest<RawIntent, Evidence> {
    ResolutionRequest::new(
        ResolutionIdentity::root(
            ResolutionId::new(10).unwrap(),
            CorrelationId::new(90).unwrap(),
        ),
        mode,
        intent,
        vec![Evidence(3)],
    )
}

#[test]
fn preview_and_apply_share_the_plan_and_ordered_interceptors() {
    let resolver = StandardResolver::default();

    let mut preview_policy = FixturePolicy::with_subjects(&[1, 2]);
    let mut preview_transaction = FixtureTransaction::default();
    let preview = resolver.resolve(
        &mut preview_policy,
        &mut preview_transaction,
        request(ResolutionMode::Preview, RawIntent::ordinary(7)),
    );

    let mut apply_policy = FixturePolicy::with_subjects(&[1, 2]);
    let mut apply_transaction = FixtureTransaction::default();
    let applied = resolver.resolve(
        &mut apply_policy,
        &mut apply_transaction,
        request(ResolutionMode::Apply, RawIntent::ordinary(7)),
    );

    assert!(preview.succeeded());
    assert!(applied.succeeded());
    assert_eq!(preview.effects(), applied.effects());
    assert_eq!(preview.events(), applied.events());
    assert_eq!(preview_policy.hook_order, ["cap", "record"]);
    assert_eq!(apply_policy.hook_order, ["cap", "record"]);
    assert_eq!(preview.commit(), &CommitStatus::Previewed);
    assert!(preview_transaction.authority.is_empty());
    assert!(preview_transaction.staged.is_empty());
    assert_eq!(applied.commit(), &CommitStatus::Applied);
    assert_eq!(apply_transaction.authority, applied.effects());
    assert_eq!(apply_transaction.commits, 1);
    assert!(applied
        .attempt()
        .trace()
        .iter()
        .any(|record| matches!(record.kind(), ResolutionTraceKind::CommitApplied)));
}

#[test]
fn rejection_and_suspension_preserve_evidence_without_mutation() {
    let resolver = StandardResolver::default();
    let mut policy = FixturePolicy::with_subjects(&[1]);
    let mut transaction = FixtureTransaction::default();
    let mut rejected_intent = RawIntent::ordinary(3);
    rejected_intent.reject = true;
    let rejected = resolver.resolve(
        &mut policy,
        &mut transaction,
        request(ResolutionMode::Apply, rejected_intent),
    );
    assert_eq!(
        rejected.attempt().status(),
        &AttemptStatus::Rejected(Rejection::Blocked)
    );
    assert_eq!(rejected.commit(), &CommitStatus::NotAttempted);
    assert!(transaction.authority.is_empty());

    let mut suspended_intent = RawIntent::ordinary(3);
    suspended_intent.suspend = true;
    let suspended = resolver.resolve(
        &mut policy,
        &mut transaction,
        request(ResolutionMode::Apply, suspended_intent),
    );
    assert_eq!(
        suspended.attempt().status(),
        &AttemptStatus::Suspended(Suspension(vec![4, 2]))
    );
    assert_eq!(suspended.attempt().evidence(), &[Evidence(3)]);
    assert!(transaction.authority.is_empty());
}

#[test]
fn children_share_correlation_and_commit_with_the_root_once() {
    let resolver = StandardResolver::default();
    let mut policy = FixturePolicy::with_subjects(&[1, 2]);
    let mut transaction = FixtureTransaction::default();
    let mut intent = RawIntent::ordinary(2);
    intent.spawn_child = true;
    let receipt = resolver.resolve(
        &mut policy,
        &mut transaction,
        request(ResolutionMode::Apply, intent),
    );

    assert!(receipt.succeeded());
    assert_eq!(receipt.attempt().children().len(), 1);
    let child = &receipt.attempt().children()[0];
    assert_eq!(
        child.identity().correlation(),
        receipt.attempt().identity().correlation()
    );
    assert_eq!(
        child.identity().parent(),
        Some(receipt.attempt().identity().resolution())
    );
    assert_eq!(transaction.commits, 1);
    assert_eq!(transaction.authority.len(), 4);
}

#[test]
fn failed_child_and_transaction_failures_never_publish_authority() {
    let resolver = StandardResolver::default();
    let mut policy = FixturePolicy::with_subjects(&[1]);
    let mut transaction = FixtureTransaction::default();
    let mut intent = RawIntent::ordinary(2);
    intent.spawn_child = true;
    intent.reject_child = true;
    let failed_child = resolver.resolve(
        &mut policy,
        &mut transaction,
        request(ResolutionMode::Apply, intent),
    );
    assert_eq!(failed_child.attempt().status(), &AttemptStatus::ChildFailed);
    assert_eq!(failed_child.commit(), &CommitStatus::NotAttempted);
    assert!(transaction.authority.is_empty());

    let mut stage_failure = FixtureTransaction {
        fail_stage: true,
        ..FixtureTransaction::default()
    };
    let staged = resolver.resolve(
        &mut policy,
        &mut stage_failure,
        request(ResolutionMode::Apply, RawIntent::ordinary(2)),
    );
    assert_eq!(staged.commit(), &CommitStatus::Failed("stage failed"));
    assert!(stage_failure.authority.is_empty());
    assert!(stage_failure.staged.is_empty());

    let mut commit_failure = FixtureTransaction {
        fail_commit: true,
        ..FixtureTransaction::default()
    };
    let committed = resolver.resolve(
        &mut policy,
        &mut commit_failure,
        request(ResolutionMode::Apply, RawIntent::ordinary(2)),
    );
    assert_eq!(committed.commit(), &CommitStatus::Failed("commit failed"));
    assert_eq!(
        committed.into_commit(),
        CommitStatus::Failed("commit failed")
    );
    assert!(commit_failure.authority.is_empty());
    assert!(commit_failure.staged.is_empty());
}

#[test]
fn program_and_trace_limits_fail_before_staging() {
    let limits = ResolutionLimits {
        max_program_nodes: 2,
        ..ResolutionLimits::default()
    };
    let resolver = StandardResolver::new(limits).unwrap();
    let mut policy = FixturePolicy::with_subjects(&[1, 2]);
    let mut transaction = FixtureTransaction::default();
    let receipt = resolver.resolve(
        &mut policy,
        &mut transaction,
        request(ResolutionMode::Apply, RawIntent::ordinary(2)),
    );
    assert!(matches!(
        receipt.attempt().status(),
        AttemptStatus::LimitExceeded(_)
    ));
    assert_eq!(receipt.commit(), &CommitStatus::NotAttempted);
    assert!(transaction.authority.is_empty());

    let trace_limits = ResolutionLimits {
        max_trace_records: 2,
        ..ResolutionLimits::default()
    };
    let trace_resolver = StandardResolver::new(trace_limits).unwrap();
    let trace_receipt = trace_resolver.resolve(
        &mut policy,
        &mut transaction,
        request(ResolutionMode::Apply, RawIntent::ordinary(2)),
    );
    assert!(matches!(
        trace_receipt.attempt().status(),
        AttemptStatus::LimitExceeded(_)
    ));
    assert!(transaction.authority.is_empty());
}
