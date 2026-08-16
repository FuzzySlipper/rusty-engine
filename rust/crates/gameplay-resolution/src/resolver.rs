use crate::{
    AttemptReceipt, AttemptStatus, CommitStatus, PolicyFailure, Program, ResolutionId,
    ResolutionIdentity, ResolutionLimitError, ResolutionLimits, ResolutionMode, ResolutionPhase,
    ResolutionPlan, ResolutionPolicy, ResolutionReceipt, ResolutionRequest, ResolutionTraceKind,
    ResolutionTraceRecord, ResolutionTraceSink, ResolutionTransaction,
};

type PlanOf<Policy> = ResolutionPlan<
    <Policy as ResolutionPolicy>::Effect,
    <Policy as ResolutionPolicy>::Event,
    <Policy as ResolutionPolicy>::RawIntent,
    <Policy as ResolutionPolicy>::Evidence,
>;

type AttemptOf<Policy> = AttemptReceipt<
    <Policy as ResolutionPolicy>::RawIntent,
    <Policy as ResolutionPolicy>::Intent,
    <Policy as ResolutionPolicy>::Facts,
    <Policy as ResolutionPolicy>::Evidence,
    <Policy as ResolutionPolicy>::Rejection,
    <Policy as ResolutionPolicy>::Fault,
    <Policy as ResolutionPolicy>::Suspension,
    <Policy as ResolutionPolicy>::TraceDetail,
>;

type ReceiptOf<Policy, Transaction> = ResolutionReceipt<
    <Policy as ResolutionPolicy>::RawIntent,
    <Policy as ResolutionPolicy>::Intent,
    <Policy as ResolutionPolicy>::Facts,
    <Policy as ResolutionPolicy>::Evidence,
    <Policy as ResolutionPolicy>::Effect,
    <Policy as ResolutionPolicy>::Event,
    <Policy as ResolutionPolicy>::Rejection,
    <Policy as ResolutionPolicy>::Fault,
    <Policy as ResolutionPolicy>::Suspension,
    <Policy as ResolutionPolicy>::TraceDetail,
    <Transaction as ResolutionTransaction>::Error,
>;

type TraversalResult<Policy> = Result<
    (),
    TraversalStop<
        <Policy as ResolutionPolicy>::Rejection,
        <Policy as ResolutionPolicy>::Fault,
        <Policy as ResolutionPolicy>::Suspension,
    >,
>;

struct AttemptBuild<Policy: ResolutionPolicy> {
    receipt: AttemptOf<Policy>,
    plan: PlanOf<Policy>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StandardResolver {
    limits: ResolutionLimits,
}

impl StandardResolver {
    pub fn new(limits: ResolutionLimits) -> Result<Self, ResolutionLimitError> {
        Ok(Self {
            limits: limits.validate()?,
        })
    }

    pub const fn limits(&self) -> ResolutionLimits {
        self.limits
    }

    pub fn resolve<Policy, Transaction>(
        &self,
        policy: &mut Policy,
        transaction: &mut Transaction,
        request: ResolutionRequest<Policy::RawIntent, Policy::Evidence>,
    ) -> ReceiptOf<Policy, Transaction>
    where
        Policy: ResolutionPolicy,
        Transaction: ResolutionTransaction<Effect = Policy::Effect>,
    {
        let (identity, mode, intent, evidence) = request.into_parts();
        let mut next_resolution = identity.resolution().get().checked_add(1);
        let mut child_count = 0_usize;
        let mut build = self.plan_attempt(
            policy,
            identity,
            intent,
            evidence,
            &mut next_resolution,
            &mut child_count,
        );

        let (effects, events) = build.plan.into_parts();
        let mut commit = CommitStatus::NotAttempted;
        if build.receipt.is_planned() {
            let final_trace_count = build.receipt.trace.len().checked_add(6);
            let final_trace_error = match final_trace_count {
                Some(actual) => ResolutionLimits::enforce(
                    "trace records",
                    actual,
                    self.limits.max_trace_records,
                )
                .err(),
                None => Some(ResolutionLimitError::ArithmeticOverflow {
                    resource: "trace records",
                }),
            };
            if let Some(error) = final_trace_error {
                build.receipt.status = AttemptStatus::LimitExceeded(error);
                transaction.abort();
            } else {
                self.push_unchecked(
                    &mut build.receipt.trace,
                    identity,
                    ResolutionPhase::Commit,
                    ResolutionTraceKind::PhaseStarted,
                );
                let mut transaction_error = None;
                for effect in &effects {
                    if let Err(error) = transaction.stage(effect) {
                        transaction_error = Some(error);
                        break;
                    }
                }
                if let Some(error) = transaction_error {
                    transaction.abort();
                    self.push_unchecked(
                        &mut build.receipt.trace,
                        identity,
                        ResolutionPhase::Commit,
                        ResolutionTraceKind::TransactionFailed,
                    );
                    commit = CommitStatus::Failed(error);
                } else {
                    self.push_unchecked(
                        &mut build.receipt.trace,
                        identity,
                        ResolutionPhase::Commit,
                        ResolutionTraceKind::EffectsStaged {
                            count: effects.len(),
                        },
                    );
                    match mode {
                        ResolutionMode::Preview => {
                            transaction.abort();
                            self.push_unchecked(
                                &mut build.receipt.trace,
                                identity,
                                ResolutionPhase::Commit,
                                ResolutionTraceKind::PreviewAborted,
                            );
                            commit = CommitStatus::Previewed;
                        }
                        ResolutionMode::Apply => match transaction.commit() {
                            Ok(()) => {
                                self.push_unchecked(
                                    &mut build.receipt.trace,
                                    identity,
                                    ResolutionPhase::Commit,
                                    ResolutionTraceKind::CommitApplied,
                                );
                                commit = CommitStatus::Applied;
                            }
                            Err(error) => {
                                transaction.abort();
                                self.push_unchecked(
                                    &mut build.receipt.trace,
                                    identity,
                                    ResolutionPhase::Commit,
                                    ResolutionTraceKind::TransactionFailed,
                                );
                                commit = CommitStatus::Failed(error);
                            }
                        },
                    }
                }
                self.push_unchecked(
                    &mut build.receipt.trace,
                    identity,
                    ResolutionPhase::Commit,
                    ResolutionTraceKind::PhaseCompleted,
                );
                if matches!(commit, CommitStatus::Previewed | CommitStatus::Applied) {
                    self.push_unchecked(
                        &mut build.receipt.trace,
                        identity,
                        ResolutionPhase::Consequences,
                        ResolutionTraceKind::PhaseStarted,
                    );
                    self.push_unchecked(
                        &mut build.receipt.trace,
                        identity,
                        ResolutionPhase::Consequences,
                        ResolutionTraceKind::PhaseCompleted,
                    );
                }
            }
        } else {
            transaction.abort();
        }

        ResolutionReceipt::new(mode, build.receipt, effects, events, commit)
    }

    #[allow(clippy::too_many_arguments)]
    fn plan_attempt<Policy: ResolutionPolicy>(
        &self,
        policy: &mut Policy,
        identity: ResolutionIdentity,
        raw_intent: Policy::RawIntent,
        evidence: Vec<Policy::Evidence>,
        next_resolution: &mut Option<u64>,
        child_count: &mut usize,
    ) -> AttemptBuild<Policy> {
        let mut trace = Vec::new();
        let mut children = Vec::new();
        let mut plan = ResolutionPlan::new();

        if let Err(error) =
            ResolutionLimits::enforce("evidence", evidence.len(), self.limits.max_evidence)
        {
            return self.finish_attempt(
                identity,
                raw_intent,
                evidence,
                None,
                None,
                AttemptStatus::LimitExceeded(error),
                trace,
                children,
                plan,
            );
        }
        if usize::from(identity.depth()) > usize::from(self.limits.max_child_depth) {
            return self.finish_attempt(
                identity,
                raw_intent,
                evidence,
                None,
                None,
                AttemptStatus::LimitExceeded(ResolutionLimitError::Exceeded {
                    resource: "child depth",
                    actual: usize::from(identity.depth()),
                    maximum: usize::from(self.limits.max_child_depth),
                }),
                trace,
                children,
                plan,
            );
        }

        if let Err(error) = self.push_trace(
            &mut trace,
            identity,
            ResolutionPhase::Admit,
            ResolutionTraceKind::PhaseStarted,
        ) {
            return self.finish_attempt(
                identity,
                raw_intent,
                evidence,
                None,
                None,
                AttemptStatus::LimitExceeded(error),
                trace,
                children,
                plan,
            );
        }
        let admitted =
            self.traced_policy_call(&mut trace, identity, ResolutionPhase::Admit, |sink| {
                policy.admit(&raw_intent, &evidence, sink)
            });
        let admitted = match admitted {
            Err(error) => {
                return self.finish_attempt(
                    identity,
                    raw_intent,
                    evidence,
                    None,
                    None,
                    AttemptStatus::LimitExceeded(error),
                    trace,
                    children,
                    plan,
                )
            }
            Ok(Err(failure)) => {
                let status = self.record_policy_failure(
                    &mut trace,
                    identity,
                    ResolutionPhase::Admit,
                    failure,
                );
                return self.finish_attempt(
                    identity, raw_intent, evidence, None, None, status, trace, children, plan,
                );
            }
            Ok(Ok(value)) => value,
        };
        if let Err(error) = self.complete_phase(&mut trace, identity, ResolutionPhase::Admit) {
            return self.finish_attempt(
                identity,
                raw_intent,
                evidence,
                Some(admitted),
                None,
                AttemptStatus::LimitExceeded(error),
                trace,
                children,
                plan,
            );
        }

        if let Err(error) = self.start_phase(&mut trace, identity, ResolutionPhase::Gather) {
            return self.finish_attempt(
                identity,
                raw_intent,
                evidence,
                Some(admitted),
                None,
                AttemptStatus::LimitExceeded(error),
                trace,
                children,
                plan,
            );
        }
        let facts =
            self.traced_policy_call(&mut trace, identity, ResolutionPhase::Gather, |sink| {
                policy.gather(&admitted, &evidence, sink)
            });
        let facts = match facts {
            Err(error) => {
                return self.finish_attempt(
                    identity,
                    raw_intent,
                    evidence,
                    Some(admitted),
                    None,
                    AttemptStatus::LimitExceeded(error),
                    trace,
                    children,
                    plan,
                )
            }
            Ok(Err(failure)) => {
                let status = self.record_policy_failure(
                    &mut trace,
                    identity,
                    ResolutionPhase::Gather,
                    failure,
                );
                return self.finish_attempt(
                    identity,
                    raw_intent,
                    evidence,
                    Some(admitted),
                    None,
                    status,
                    trace,
                    children,
                    plan,
                );
            }
            Ok(Ok(value)) => value,
        };

        let interceptors =
            self.traced_policy_call(&mut trace, identity, ResolutionPhase::Gather, |sink| {
                policy.interceptors(&admitted, &facts, &evidence, sink)
            });
        let interceptors = match interceptors {
            Err(error) => {
                return self.finish_attempt(
                    identity,
                    raw_intent,
                    evidence,
                    Some(admitted),
                    Some(facts),
                    AttemptStatus::LimitExceeded(error),
                    trace,
                    children,
                    plan,
                )
            }
            Ok(Err(failure)) => {
                let status = self.record_policy_failure(
                    &mut trace,
                    identity,
                    ResolutionPhase::Gather,
                    failure,
                );
                return self.finish_attempt(
                    identity,
                    raw_intent,
                    evidence,
                    Some(admitted),
                    Some(facts),
                    status,
                    trace,
                    children,
                    plan,
                );
            }
            Ok(Ok(value)) => value,
        };
        if let Err(error) = ResolutionLimits::enforce(
            "interceptors",
            interceptors.len(),
            self.limits.max_interceptors,
        ) {
            return self.finish_attempt(
                identity,
                raw_intent,
                evidence,
                Some(admitted),
                Some(facts),
                AttemptStatus::LimitExceeded(error),
                trace,
                children,
                plan,
            );
        }
        if let Err(error) = self.complete_phase(&mut trace, identity, ResolutionPhase::Gather) {
            return self.finish_attempt(
                identity,
                raw_intent,
                evidence,
                Some(admitted),
                Some(facts),
                AttemptStatus::LimitExceeded(error),
                trace,
                children,
                plan,
            );
        }

        if let Err(error) = self.start_phase(&mut trace, identity, ResolutionPhase::Check) {
            return self.finish_attempt(
                identity,
                raw_intent,
                evidence,
                Some(admitted),
                Some(facts),
                AttemptStatus::LimitExceeded(error),
                trace,
                children,
                plan,
            );
        }
        let checked =
            self.traced_policy_call(&mut trace, identity, ResolutionPhase::Check, |sink| {
                policy.check(&admitted, &facts, &evidence, sink)
            });
        if let Some(status) =
            self.policy_call_status(&mut trace, identity, ResolutionPhase::Check, checked)
        {
            return self.finish_attempt(
                identity,
                raw_intent,
                evidence,
                Some(admitted),
                Some(facts),
                status,
                trace,
                children,
                plan,
            );
        }
        if let Err(error) = self.complete_phase(&mut trace, identity, ResolutionPhase::Check) {
            return self.finish_attempt(
                identity,
                raw_intent,
                evidence,
                Some(admitted),
                Some(facts),
                AttemptStatus::LimitExceeded(error),
                trace,
                children,
                plan,
            );
        }

        if let Err(error) = self.start_phase(&mut trace, identity, ResolutionPhase::Plan) {
            return self.finish_attempt(
                identity,
                raw_intent,
                evidence,
                Some(admitted),
                Some(facts),
                AttemptStatus::LimitExceeded(error),
                trace,
                children,
                plan,
            );
        }
        let program =
            self.traced_policy_call(&mut trace, identity, ResolutionPhase::Plan, |sink| {
                policy.plan(&admitted, &facts, &evidence, sink)
            });
        let program = match program {
            Err(error) => {
                return self.finish_attempt(
                    identity,
                    raw_intent,
                    evidence,
                    Some(admitted),
                    Some(facts),
                    AttemptStatus::LimitExceeded(error),
                    trace,
                    children,
                    plan,
                )
            }
            Ok(Err(failure)) => {
                let status = self.record_policy_failure(
                    &mut trace,
                    identity,
                    ResolutionPhase::Plan,
                    failure,
                );
                return self.finish_attempt(
                    identity,
                    raw_intent,
                    evidence,
                    Some(admitted),
                    Some(facts),
                    status,
                    trace,
                    children,
                    plan,
                );
            }
            Ok(Ok(value)) => value,
        };
        let mut traversal = TraversalCounts::default();
        if let Err(stop) = self.traverse_program(
            policy,
            identity,
            &program,
            None,
            &admitted,
            &facts,
            &evidence,
            &mut trace,
            &mut plan,
            &mut traversal,
            0,
        ) {
            let status =
                self.record_traversal_stop(&mut trace, identity, ResolutionPhase::Plan, stop);
            return self.finish_attempt(
                identity,
                raw_intent,
                evidence,
                Some(admitted),
                Some(facts),
                status,
                trace,
                children,
                plan,
            );
        }
        if let Err(error) = self.complete_phase(&mut trace, identity, ResolutionPhase::Plan) {
            return self.finish_attempt(
                identity,
                raw_intent,
                evidence,
                Some(admitted),
                Some(facts),
                AttemptStatus::LimitExceeded(error),
                trace,
                children,
                plan,
            );
        }

        if let Err(error) = self.start_phase(&mut trace, identity, ResolutionPhase::BeforeCommit) {
            return self.finish_attempt(
                identity,
                raw_intent,
                evidence,
                Some(admitted),
                Some(facts),
                AttemptStatus::LimitExceeded(error),
                trace,
                children,
                plan,
            );
        }
        for (index, interceptor) in interceptors.iter().enumerate() {
            let applied = self.traced_policy_call(
                &mut trace,
                identity,
                ResolutionPhase::BeforeCommit,
                |sink| {
                    policy.before_commit(interceptor, &admitted, &facts, &evidence, &mut plan, sink)
                },
            );
            if let Some(status) = self.policy_call_status(
                &mut trace,
                identity,
                ResolutionPhase::BeforeCommit,
                applied,
            ) {
                return self.finish_attempt(
                    identity,
                    raw_intent,
                    evidence,
                    Some(admitted),
                    Some(facts),
                    status,
                    trace,
                    children,
                    plan,
                );
            }
            if let Err(error) = self.enforce_plan::<Policy>(&plan) {
                return self.finish_attempt(
                    identity,
                    raw_intent,
                    evidence,
                    Some(admitted),
                    Some(facts),
                    AttemptStatus::LimitExceeded(error),
                    trace,
                    children,
                    plan,
                );
            }
            if let Err(error) = self.push_trace(
                &mut trace,
                identity,
                ResolutionPhase::BeforeCommit,
                ResolutionTraceKind::InterceptorApplied { index },
            ) {
                return self.finish_attempt(
                    identity,
                    raw_intent,
                    evidence,
                    Some(admitted),
                    Some(facts),
                    AttemptStatus::LimitExceeded(error),
                    trace,
                    children,
                    plan,
                );
            }
        }

        for child in plan.take_children() {
            *child_count = match child_count.checked_add(1) {
                Some(value) => value,
                None => {
                    return self.finish_attempt(
                        identity,
                        raw_intent,
                        evidence,
                        Some(admitted),
                        Some(facts),
                        AttemptStatus::LimitExceeded(ResolutionLimitError::ArithmeticOverflow {
                            resource: "child resolutions",
                        }),
                        trace,
                        children,
                        plan,
                    )
                }
            };
            if let Err(error) = ResolutionLimits::enforce(
                "child resolutions",
                *child_count,
                self.limits.max_child_resolutions,
            ) {
                return self.finish_attempt(
                    identity,
                    raw_intent,
                    evidence,
                    Some(admitted),
                    Some(facts),
                    AttemptStatus::LimitExceeded(error),
                    trace,
                    children,
                    plan,
                );
            }
            let Some(child_id_value) = *next_resolution else {
                return self.finish_attempt(
                    identity,
                    raw_intent,
                    evidence,
                    Some(admitted),
                    Some(facts),
                    AttemptStatus::LimitExceeded(ResolutionLimitError::ArithmeticOverflow {
                        resource: "resolution identity",
                    }),
                    trace,
                    children,
                    plan,
                );
            };
            *next_resolution = child_id_value.checked_add(1);
            let child_id = ResolutionId::new(child_id_value)
                .expect("checked child resolution identity is nonzero");
            let child_identity = match identity.child(child_id) {
                Ok(value) => value,
                Err(_) => {
                    return self.finish_attempt(
                        identity,
                        raw_intent,
                        evidence,
                        Some(admitted),
                        Some(facts),
                        AttemptStatus::LimitExceeded(ResolutionLimitError::ArithmeticOverflow {
                            resource: "child depth",
                        }),
                        trace,
                        children,
                        plan,
                    )
                }
            };
            if let Err(error) = self.push_trace(
                &mut trace,
                identity,
                ResolutionPhase::BeforeCommit,
                ResolutionTraceKind::ChildStarted { child: child_id },
            ) {
                return self.finish_attempt(
                    identity,
                    raw_intent,
                    evidence,
                    Some(admitted),
                    Some(facts),
                    AttemptStatus::LimitExceeded(error),
                    trace,
                    children,
                    plan,
                );
            }
            let (child_intent, child_evidence) = child.into_parts();
            let child_build = self.plan_attempt(
                policy,
                child_identity,
                child_intent,
                child_evidence,
                next_resolution,
                child_count,
            );
            let child_planned = child_build.receipt.is_planned();
            plan.append(child_build.plan);
            children.push(child_build.receipt);
            if let Err(error) = self.enforce_plan::<Policy>(&plan) {
                return self.finish_attempt(
                    identity,
                    raw_intent,
                    evidence,
                    Some(admitted),
                    Some(facts),
                    AttemptStatus::LimitExceeded(error),
                    trace,
                    children,
                    plan,
                );
            }
            if let Err(error) = self.push_trace(
                &mut trace,
                identity,
                ResolutionPhase::BeforeCommit,
                ResolutionTraceKind::ChildCompleted { child: child_id },
            ) {
                return self.finish_attempt(
                    identity,
                    raw_intent,
                    evidence,
                    Some(admitted),
                    Some(facts),
                    AttemptStatus::LimitExceeded(error),
                    trace,
                    children,
                    plan,
                );
            }
            if !child_planned {
                return self.finish_attempt(
                    identity,
                    raw_intent,
                    evidence,
                    Some(admitted),
                    Some(facts),
                    AttemptStatus::ChildFailed,
                    trace,
                    children,
                    plan,
                );
            }
        }
        if let Err(error) = self.complete_phase(&mut trace, identity, ResolutionPhase::BeforeCommit)
        {
            return self.finish_attempt(
                identity,
                raw_intent,
                evidence,
                Some(admitted),
                Some(facts),
                AttemptStatus::LimitExceeded(error),
                trace,
                children,
                plan,
            );
        }

        self.finish_attempt(
            identity,
            raw_intent,
            evidence,
            Some(admitted),
            Some(facts),
            AttemptStatus::Planned,
            trace,
            children,
            plan,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn traverse_program<Policy: ResolutionPolicy>(
        &self,
        policy: &mut Policy,
        identity: ResolutionIdentity,
        program: &Program<Policy::Predicate, Policy::Selector, Policy::Operation>,
        subject: Option<&Policy::Subject>,
        intent: &Policy::Intent,
        facts: &Policy::Facts,
        evidence: &[Policy::Evidence],
        trace: &mut Vec<ResolutionTraceRecord<Policy::TraceDetail>>,
        plan: &mut PlanOf<Policy>,
        counts: &mut TraversalCounts,
        depth: u16,
    ) -> TraversalResult<Policy> {
        counts.nodes = counts.nodes.checked_add(1).ok_or_else(|| {
            TraversalStop::Limit(ResolutionLimitError::ArithmeticOverflow {
                resource: "program nodes",
            })
        })?;
        ResolutionLimits::enforce("program nodes", counts.nodes, self.limits.max_program_nodes)
            .map_err(TraversalStop::Limit)?;
        if depth > self.limits.max_program_depth {
            return Err(TraversalStop::Limit(ResolutionLimitError::Exceeded {
                resource: "program depth",
                actual: usize::from(depth),
                maximum: usize::from(self.limits.max_program_depth),
            }));
        }

        match program {
            Program::Sequence { steps } => {
                let next_depth = depth.checked_add(1).ok_or_else(|| {
                    TraversalStop::Limit(ResolutionLimitError::ArithmeticOverflow {
                        resource: "program depth",
                    })
                })?;
                for step in steps {
                    self.traverse_program(
                        policy, identity, step, subject, intent, facts, evidence, trace, plan,
                        counts, next_depth,
                    )?;
                }
            }
            Program::When {
                predicate,
                then_program,
                otherwise_program,
            } => {
                let result = self
                    .traced_policy_call(trace, identity, ResolutionPhase::Plan, |sink| {
                        policy.evaluate_predicate(predicate, subject, intent, facts, evidence, sink)
                    })
                    .map_err(TraversalStop::Limit)?
                    .map_err(TraversalStop::Policy)?;
                self.push_trace(
                    trace,
                    identity,
                    ResolutionPhase::Plan,
                    ResolutionTraceKind::PredicateEvaluated { passed: result },
                )
                .map_err(TraversalStop::Limit)?;
                let selected = if result {
                    Some(then_program.as_ref())
                } else {
                    otherwise_program.as_deref()
                };
                if let Some(selected) = selected {
                    self.traverse_program(
                        policy,
                        identity,
                        selected,
                        subject,
                        intent,
                        facts,
                        evidence,
                        trace,
                        plan,
                        counts,
                        depth.checked_add(1).ok_or_else(|| {
                            TraversalStop::Limit(ResolutionLimitError::ArithmeticOverflow {
                                resource: "program depth",
                            })
                        })?,
                    )?;
                }
            }
            Program::ForEach {
                selector,
                maximum,
                body,
            } => {
                let subjects = self
                    .traced_policy_call(trace, identity, ResolutionPhase::Plan, |sink| {
                        policy.select(selector, subject, intent, facts, evidence, sink)
                    })
                    .map_err(TraversalStop::Limit)?
                    .map_err(TraversalStop::Policy)?;
                ResolutionLimits::enforce(
                    "selector maximum",
                    subjects.len(),
                    usize::from(*maximum),
                )
                .map_err(TraversalStop::Limit)?;
                counts.selected_subjects = counts
                    .selected_subjects
                    .checked_add(subjects.len())
                    .ok_or_else(|| {
                        TraversalStop::Limit(ResolutionLimitError::ArithmeticOverflow {
                            resource: "selected subjects",
                        })
                    })?;
                ResolutionLimits::enforce(
                    "selected subjects",
                    counts.selected_subjects,
                    self.limits.max_selected_subjects,
                )
                .map_err(TraversalStop::Limit)?;
                self.push_trace(
                    trace,
                    identity,
                    ResolutionPhase::Plan,
                    ResolutionTraceKind::SubjectsSelected {
                        count: subjects.len(),
                    },
                )
                .map_err(TraversalStop::Limit)?;
                let next_depth = depth.checked_add(1).ok_or_else(|| {
                    TraversalStop::Limit(ResolutionLimitError::ArithmeticOverflow {
                        resource: "program depth",
                    })
                })?;
                for selected_subject in &subjects {
                    self.traverse_program(
                        policy,
                        identity,
                        body,
                        Some(selected_subject),
                        intent,
                        facts,
                        evidence,
                        trace,
                        plan,
                        counts,
                        next_depth,
                    )?;
                }
            }
            Program::Operation(operation) => {
                let operation_plan = self
                    .traced_policy_call(trace, identity, ResolutionPhase::Plan, |sink| {
                        policy.plan_operation(operation, subject, intent, facts, evidence, sink)
                    })
                    .map_err(TraversalStop::Limit)?
                    .map_err(TraversalStop::Policy)?;
                plan.append(operation_plan);
                self.enforce_plan::<Policy>(plan)
                    .map_err(TraversalStop::Limit)?;
                self.push_trace(
                    trace,
                    identity,
                    ResolutionPhase::Plan,
                    ResolutionTraceKind::OperationPlanned,
                )
                .map_err(TraversalStop::Limit)?;
            }
        }
        Ok(())
    }

    fn enforce_plan<Policy: ResolutionPolicy>(
        &self,
        plan: &PlanOf<Policy>,
    ) -> Result<(), ResolutionLimitError> {
        ResolutionLimits::enforce("effects", plan.effects().len(), self.limits.max_effects)?;
        ResolutionLimits::enforce("events", plan.events().len(), self.limits.max_events)?;
        let total = plan
            .effects()
            .len()
            .checked_add(plan.events().len())
            .and_then(|value| value.checked_add(plan.children().len()))
            .ok_or(ResolutionLimitError::ArithmeticOverflow {
                resource: "plan entries",
            })?;
        let maximum = self
            .limits
            .max_effects
            .checked_add(self.limits.max_events)
            .and_then(|value| value.checked_add(self.limits.max_child_resolutions))
            .ok_or(ResolutionLimitError::ArithmeticOverflow {
                resource: "plan entries",
            })?;
        ResolutionLimits::enforce("plan entries", total, maximum)
    }

    fn traced_policy_call<Detail, Value>(
        &self,
        trace: &mut Vec<ResolutionTraceRecord<Detail>>,
        identity: ResolutionIdentity,
        phase: ResolutionPhase,
        call: impl FnOnce(&mut dyn ResolutionTraceSink<Detail>) -> Value,
    ) -> Result<Value, ResolutionLimitError> {
        let mut sink = TraceCollector {
            trace,
            identity,
            phase,
            maximum: self.limits.max_trace_records,
            overflow: None,
        };
        let value = call(&mut sink);
        match sink.overflow {
            Some(error) => Err(error),
            None => Ok(value),
        }
    }

    fn policy_call_status<Rejection, Fault, Suspension>(
        &self,
        trace: &mut Vec<ResolutionTraceRecord<impl Sized>>,
        identity: ResolutionIdentity,
        phase: ResolutionPhase,
        result: Result<
            Result<(), PolicyFailure<Rejection, Fault, Suspension>>,
            ResolutionLimitError,
        >,
    ) -> Option<AttemptStatus<Rejection, Fault, Suspension>> {
        match result {
            Err(error) => Some(AttemptStatus::LimitExceeded(error)),
            Ok(Err(failure)) => Some(self.record_policy_failure(trace, identity, phase, failure)),
            Ok(Ok(())) => None,
        }
    }

    fn record_traversal_stop<Detail, Rejection, Fault, Suspension>(
        &self,
        trace: &mut Vec<ResolutionTraceRecord<Detail>>,
        identity: ResolutionIdentity,
        phase: ResolutionPhase,
        stop: TraversalStop<Rejection, Fault, Suspension>,
    ) -> AttemptStatus<Rejection, Fault, Suspension> {
        match stop {
            TraversalStop::Policy(failure) => {
                self.record_policy_failure(trace, identity, phase, failure)
            }
            TraversalStop::Limit(error) => {
                self.push_terminal(trace, identity, phase, ResolutionTraceKind::LimitExceeded);
                AttemptStatus::LimitExceeded(error)
            }
        }
    }

    fn record_policy_failure<Detail, Rejection, Fault, Suspension>(
        &self,
        trace: &mut Vec<ResolutionTraceRecord<Detail>>,
        identity: ResolutionIdentity,
        phase: ResolutionPhase,
        failure: PolicyFailure<Rejection, Fault, Suspension>,
    ) -> AttemptStatus<Rejection, Fault, Suspension> {
        match failure {
            PolicyFailure::Rejected(value) => {
                self.push_terminal(trace, identity, phase, ResolutionTraceKind::Rejected);
                AttemptStatus::Rejected(value)
            }
            PolicyFailure::Fault(value) => {
                self.push_terminal(trace, identity, phase, ResolutionTraceKind::Faulted);
                AttemptStatus::Faulted(value)
            }
            PolicyFailure::Suspended(value) => {
                self.push_terminal(trace, identity, phase, ResolutionTraceKind::Suspended);
                AttemptStatus::Suspended(value)
            }
        }
    }

    fn push_terminal<Detail>(
        &self,
        trace: &mut Vec<ResolutionTraceRecord<Detail>>,
        identity: ResolutionIdentity,
        phase: ResolutionPhase,
        kind: ResolutionTraceKind,
    ) {
        let _ = self.push_trace(trace, identity, phase, kind);
    }

    fn start_phase<Detail>(
        &self,
        trace: &mut Vec<ResolutionTraceRecord<Detail>>,
        identity: ResolutionIdentity,
        phase: ResolutionPhase,
    ) -> Result<(), ResolutionLimitError> {
        self.push_trace(trace, identity, phase, ResolutionTraceKind::PhaseStarted)
    }

    fn complete_phase<Detail>(
        &self,
        trace: &mut Vec<ResolutionTraceRecord<Detail>>,
        identity: ResolutionIdentity,
        phase: ResolutionPhase,
    ) -> Result<(), ResolutionLimitError> {
        self.push_trace(trace, identity, phase, ResolutionTraceKind::PhaseCompleted)
    }

    fn push_trace<Detail>(
        &self,
        trace: &mut Vec<ResolutionTraceRecord<Detail>>,
        identity: ResolutionIdentity,
        phase: ResolutionPhase,
        kind: ResolutionTraceKind,
    ) -> Result<(), ResolutionLimitError> {
        let actual =
            trace
                .len()
                .checked_add(1)
                .ok_or(ResolutionLimitError::ArithmeticOverflow {
                    resource: "trace records",
                })?;
        ResolutionLimits::enforce("trace records", actual, self.limits.max_trace_records)?;
        trace.push(ResolutionTraceRecord::structural(identity, phase, kind));
        Ok(())
    }

    fn push_unchecked<Detail>(
        &self,
        trace: &mut Vec<ResolutionTraceRecord<Detail>>,
        identity: ResolutionIdentity,
        phase: ResolutionPhase,
        kind: ResolutionTraceKind,
    ) {
        debug_assert!(trace.len() < self.limits.max_trace_records);
        trace.push(ResolutionTraceRecord::structural(identity, phase, kind));
    }

    #[allow(clippy::too_many_arguments)]
    fn finish_attempt<Policy: ResolutionPolicy>(
        &self,
        identity: ResolutionIdentity,
        raw_intent: Policy::RawIntent,
        evidence: Vec<Policy::Evidence>,
        intent: Option<Policy::Intent>,
        facts: Option<Policy::Facts>,
        status: AttemptStatus<Policy::Rejection, Policy::Fault, Policy::Suspension>,
        trace: Vec<ResolutionTraceRecord<Policy::TraceDetail>>,
        children: Vec<AttemptOf<Policy>>,
        plan: PlanOf<Policy>,
    ) -> AttemptBuild<Policy> {
        AttemptBuild {
            receipt: AttemptReceipt {
                identity,
                raw_intent,
                evidence,
                intent,
                facts,
                status,
                trace,
                children,
            },
            plan,
        }
    }
}

#[derive(Debug, Default)]
struct TraversalCounts {
    nodes: usize,
    selected_subjects: usize,
}

enum TraversalStop<Rejection, Fault, Suspension> {
    Policy(PolicyFailure<Rejection, Fault, Suspension>),
    Limit(ResolutionLimitError),
}

struct TraceCollector<'a, Detail> {
    trace: &'a mut Vec<ResolutionTraceRecord<Detail>>,
    identity: ResolutionIdentity,
    phase: ResolutionPhase,
    maximum: usize,
    overflow: Option<ResolutionLimitError>,
}

impl<Detail> ResolutionTraceSink<Detail> for TraceCollector<'_, Detail> {
    fn record(&mut self, detail: Detail) {
        if self.overflow.is_some() {
            return;
        }
        let Some(actual) = self.trace.len().checked_add(1) else {
            self.overflow = Some(ResolutionLimitError::ArithmeticOverflow {
                resource: "trace records",
            });
            return;
        };
        if actual > self.maximum {
            self.overflow = Some(ResolutionLimitError::Exceeded {
                resource: "trace records",
                actual,
                maximum: self.maximum,
            });
            return;
        }
        self.trace.push(ResolutionTraceRecord::policy_detail(
            self.identity,
            self.phase,
            detail,
        ));
    }
}
