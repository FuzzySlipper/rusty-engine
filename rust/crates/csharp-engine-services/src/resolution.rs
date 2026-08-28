//! NativeAOT bridge for the non-generic gameplay-resolution structural owner.

use std::{collections::BTreeMap, ffi::c_void};

use csharp_engine_abi::*;
use gameplay_resolution::{
    CorrelationId, ResolutionId, ResolutionIdentity, ResolutionLimits, ResolutionMode,
    ResolutionPhase, ResolutionTraceKind, StructuralAttemptStatus, StructuralBudget,
    StructuralCommitStatus, StructuralResolutionSession,
};

use crate::composition::ABI_OK;

pub(crate) struct RuntimeResolutionBridge {
    sessions: BTreeMap<u64, StructuralResolutionSession>,
    next_session: u64,
    readouts: BTreeMap<u64, ResolutionReadoutBacking>,
    next_readout: u64,
}

struct ResolutionReadoutBacking {
    attempts: Vec<NativeResolutionAttemptReadoutRow>,
    traces: Vec<NativeResolutionTraceReadoutRow>,
}

impl RuntimeResolutionBridge {
    pub(crate) fn new() -> Self {
        Self {
            sessions: BTreeMap::new(),
            next_session: 1,
            readouts: BTreeMap::new(),
            next_readout: 1,
        }
    }
    fn create(
        &mut self,
        request: NativeResolutionSessionCreateRequest,
    ) -> Option<NativeResolutionSessionHandle> {
        let root = ResolutionIdentity::root(
            ResolutionId::new(request.root_resolution).ok()?,
            CorrelationId::new(request.correlation).ok()?,
        );
        let session = StructuralResolutionSession::new(
            root,
            mode(request.mode),
            limits(request.limits),
            budget(request.root_budget),
            request.root_evidence as usize,
        )
        .ok()?;
        let value = take_next(&mut self.next_session)?;
        self.sessions.insert(value, session);
        Some(NativeResolutionSessionHandle { value })
    }
    fn session_mut(
        &mut self,
        handle: NativeResolutionSessionHandle,
    ) -> Option<&mut StructuralResolutionSession> {
        (handle.value != 0)
            .then(|| self.sessions.get_mut(&handle.value))
            .flatten()
    }
    fn destroy(&mut self, handle: NativeResolutionSessionHandle) -> bool {
        let Some(mut session) = self.sessions.remove(&handle.value) else {
            return false;
        };
        session.abandon();
        true
    }
    fn read(
        &mut self,
        handle: NativeResolutionSessionHandle,
    ) -> Option<NativeResolutionSessionReadoutLease> {
        let session = self.sessions.get(&handle.value)?;
        let value = take_next(&mut self.next_readout)?;
        let mode = native_mode(session.mode());
        let commit = native_commit(session.commit_status());
        let attempts = session
            .attempts()
            .map(|row| {
                Some(NativeResolutionAttemptReadoutRow {
                    identity: identity(row.identity),
                    mode,
                    is_root: row.is_root,
                    status: native_attempt(row.status),
                    commit,
                    evidence: narrow(row.counts.evidence)?,
                    program_nodes: narrow(row.counts.program_nodes)?,
                    program_depth: row.counts.program_depth,
                    interceptors: narrow(row.counts.interceptors)?,
                    effects: narrow(row.counts.effects)?,
                    events: narrow(row.counts.events)?,
                    children: narrow(row.counts.children)?,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        let traces = session
            .traces()
            .map(|row| {
                Some(NativeResolutionTraceReadoutRow {
                    identity: identity(row.identity),
                    phase: native_phase(row.phase),
                    kind: native_trace(row.kind),
                    scalar: narrow(row.scalar)?,
                    passed: row.passed,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        let backing = ResolutionReadoutBacking { attempts, traces };
        let lease = NativeResolutionSessionReadoutLease {
            handle: NativeResolutionReadoutLeaseHandle { value },
            attempts: backing.attempts.as_ptr(),
            attempts_len: backing.attempts.len(),
            traces: backing.traces.as_ptr(),
            traces_len: backing.traces.len(),
        };
        self.readouts.insert(value, backing);
        Some(lease)
    }
    fn destroy_readout(&mut self, handle: NativeResolutionReadoutLeaseHandle) -> bool {
        handle.value != 0 && self.readouts.remove(&handle.value).is_some()
    }
}

fn take_next(value: &mut u64) -> Option<u64> {
    let result = *value;
    *value = result.checked_add(1)?;
    Some(result)
}
fn narrow(value: usize) -> Option<u32> {
    u32::try_from(value).ok()
}
fn limits(value: NativeResolutionLimits) -> ResolutionLimits {
    ResolutionLimits {
        max_evidence: value.max_evidence as usize,
        max_program_nodes: value.max_program_nodes as usize,
        max_program_depth: value.max_program_depth,
        max_interceptors: value.max_interceptors as usize,
        max_effects: value.max_effects as usize,
        max_events: value.max_events as usize,
        max_trace_records: value.max_trace_records as usize,
        max_child_resolutions: value.max_child_resolutions as usize,
        max_child_depth: value.max_child_depth,
    }
}
fn budget(value: NativeResolutionStructuralBudget) -> StructuralBudget {
    StructuralBudget {
        max_evidence: value.max_evidence as usize,
        max_program_nodes: value.max_program_nodes as usize,
        max_program_depth: value.max_program_depth,
        max_interceptors: value.max_interceptors as usize,
        max_effects: value.max_effects as usize,
        max_events: value.max_events as usize,
        max_trace_records: value.max_trace_records as usize,
        max_children: value.max_children as usize,
    }
}
fn mode(value: NativeResolutionMode) -> ResolutionMode {
    match value {
        NativeResolutionMode::Preview => ResolutionMode::Preview,
        NativeResolutionMode::Apply => ResolutionMode::Apply,
    }
}
fn native_mode(value: ResolutionMode) -> NativeResolutionMode {
    match value {
        ResolutionMode::Preview => NativeResolutionMode::Preview,
        ResolutionMode::Apply => NativeResolutionMode::Apply,
    }
}
fn phase(value: NativeResolutionPhase) -> ResolutionPhase {
    match value {
        NativeResolutionPhase::Admit => ResolutionPhase::Admit,
        NativeResolutionPhase::Gather => ResolutionPhase::Gather,
        NativeResolutionPhase::Check => ResolutionPhase::Check,
        NativeResolutionPhase::Plan => ResolutionPhase::Plan,
        NativeResolutionPhase::BeforeCommit => ResolutionPhase::BeforeCommit,
        NativeResolutionPhase::Commit => ResolutionPhase::Commit,
        NativeResolutionPhase::Consequences => ResolutionPhase::Consequences,
    }
}
fn native_phase(value: ResolutionPhase) -> NativeResolutionPhase {
    match value {
        ResolutionPhase::Admit => NativeResolutionPhase::Admit,
        ResolutionPhase::Gather => NativeResolutionPhase::Gather,
        ResolutionPhase::Check => NativeResolutionPhase::Check,
        ResolutionPhase::Plan => NativeResolutionPhase::Plan,
        ResolutionPhase::BeforeCommit => NativeResolutionPhase::BeforeCommit,
        ResolutionPhase::Commit => NativeResolutionPhase::Commit,
        ResolutionPhase::Consequences => NativeResolutionPhase::Consequences,
    }
}
fn attempt(value: NativeResolutionAttemptStatus) -> StructuralAttemptStatus {
    match value {
        NativeResolutionAttemptStatus::Open => StructuralAttemptStatus::Open,
        NativeResolutionAttemptStatus::Planned => StructuralAttemptStatus::Planned,
        NativeResolutionAttemptStatus::Rejected => StructuralAttemptStatus::Rejected,
        NativeResolutionAttemptStatus::Suspended => StructuralAttemptStatus::Suspended,
        NativeResolutionAttemptStatus::Faulted => StructuralAttemptStatus::Faulted,
        NativeResolutionAttemptStatus::LimitExceeded => StructuralAttemptStatus::LimitExceeded,
        NativeResolutionAttemptStatus::ChildFailed => StructuralAttemptStatus::ChildFailed,
    }
}
fn native_attempt(value: StructuralAttemptStatus) -> NativeResolutionAttemptStatus {
    match value {
        StructuralAttemptStatus::Open => NativeResolutionAttemptStatus::Open,
        StructuralAttemptStatus::Planned => NativeResolutionAttemptStatus::Planned,
        StructuralAttemptStatus::Rejected => NativeResolutionAttemptStatus::Rejected,
        StructuralAttemptStatus::Suspended => NativeResolutionAttemptStatus::Suspended,
        StructuralAttemptStatus::Faulted => NativeResolutionAttemptStatus::Faulted,
        StructuralAttemptStatus::LimitExceeded => NativeResolutionAttemptStatus::LimitExceeded,
        StructuralAttemptStatus::ChildFailed => NativeResolutionAttemptStatus::ChildFailed,
    }
}
fn native_commit(value: StructuralCommitStatus) -> NativeResolutionCommitStatus {
    match value {
        StructuralCommitStatus::NotAttempted => NativeResolutionCommitStatus::NotAttempted,
        StructuralCommitStatus::Prepared => NativeResolutionCommitStatus::Prepared,
        StructuralCommitStatus::Previewed => NativeResolutionCommitStatus::Previewed,
        StructuralCommitStatus::Applied => NativeResolutionCommitStatus::Applied,
        StructuralCommitStatus::TransactionFailed => {
            NativeResolutionCommitStatus::TransactionFailed
        }
        StructuralCommitStatus::Abandoned => NativeResolutionCommitStatus::Abandoned,
    }
}
fn native_trace(value: ResolutionTraceKind) -> NativeResolutionTraceKind {
    match value {
        ResolutionTraceKind::PhaseStarted => NativeResolutionTraceKind::PhaseStarted,
        ResolutionTraceKind::PhaseCompleted => NativeResolutionTraceKind::PhaseCompleted,
        ResolutionTraceKind::PredicateEvaluated { .. } => {
            NativeResolutionTraceKind::PredicateEvaluated
        }
        ResolutionTraceKind::OperationPlanned => NativeResolutionTraceKind::OperationPlanned,
        ResolutionTraceKind::InterceptorApplied { .. } => {
            NativeResolutionTraceKind::InterceptorApplied
        }
        ResolutionTraceKind::ChildStarted { .. } => NativeResolutionTraceKind::ChildStarted,
        ResolutionTraceKind::ChildCompleted { .. } => NativeResolutionTraceKind::ChildCompleted,
        ResolutionTraceKind::CommitApplied => NativeResolutionTraceKind::CommitApplied,
        ResolutionTraceKind::PreviewAborted => NativeResolutionTraceKind::PreviewAborted,
        ResolutionTraceKind::EffectsStaged { .. } => NativeResolutionTraceKind::EffectsStaged,
        ResolutionTraceKind::Rejected => NativeResolutionTraceKind::Rejected,
        ResolutionTraceKind::Suspended => NativeResolutionTraceKind::Suspended,
        ResolutionTraceKind::Faulted => NativeResolutionTraceKind::Faulted,
        ResolutionTraceKind::LimitExceeded => NativeResolutionTraceKind::LimitExceeded,
        ResolutionTraceKind::ChildFailed => NativeResolutionTraceKind::ChildFailed,
        ResolutionTraceKind::TransactionFailed => NativeResolutionTraceKind::TransactionFailed,
        ResolutionTraceKind::PolicyDetail => NativeResolutionTraceKind::PhaseCompleted,
    }
}
fn identity(value: ResolutionIdentity) -> NativeResolutionIdentityRow {
    NativeResolutionIdentityRow {
        resolution: value.resolution().get(),
        correlation: value.correlation().get(),
        parent: value.parent().map_or(0, ResolutionId::get),
        has_parent: value.parent().is_some(),
        depth: value.depth(),
    }
}

pub(crate) fn api(bridge: &mut RuntimeResolutionBridge) -> NativeResolutionApi {
    NativeResolutionApi {
        context: (bridge as *mut RuntimeResolutionBridge).cast(),
        create_session,
        destroy_session,
        begin_phase,
        complete_phase,
        record_predicate,
        record_sequence,
        record_operation,
        record_interceptor,
        begin_child,
        complete_attempt,
        prepare_finalization,
        finalize_preview,
        finalize_applied,
        finalize_failed,
        read_session,
        destroy_readout_lease,
    }
}
unsafe extern "C" fn create_session(
    context: *mut c_void,
    request: *const NativeResolutionSessionCreateRequest,
    result: *mut NativeResolutionSessionHandle,
) -> i32 {
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    match unsafe { (&mut *context.cast::<RuntimeResolutionBridge>()).create(*request) } {
        Some(value) => {
            unsafe { *result = value };
            ABI_OK
        }
        None => 0,
    }
}
unsafe extern "C" fn destroy_session(
    context: *mut c_void,
    handle: NativeResolutionSessionHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    i32::from(unsafe { (&mut *context.cast::<RuntimeResolutionBridge>()).destroy(handle) })
}
unsafe extern "C" fn begin_phase(
    context: *mut c_void,
    request: *const NativeResolutionBeginPhaseRequest,
) -> i32 {
    with_phase(context, request, StructuralResolutionSession::begin_phase)
}
unsafe extern "C" fn complete_phase(
    context: *mut c_void,
    request: *const NativeResolutionBeginPhaseRequest,
) -> i32 {
    with_phase(
        context,
        request,
        StructuralResolutionSession::complete_phase,
    )
}
fn with_phase(
    context: *mut c_void,
    request: *const NativeResolutionBeginPhaseRequest,
    operation: fn(
        &mut StructuralResolutionSession,
        ResolutionPhase,
    ) -> Result<(), gameplay_resolution::StructuralResolutionError>,
) -> i32 {
    if context.is_null() || request.is_null() {
        return 0;
    }
    let request = unsafe { *request };
    let bridge = unsafe { &mut *context.cast::<RuntimeResolutionBridge>() };
    bridge
        .session_mut(request.session)
        .is_some_and(|session| operation(session, phase(request.phase)).is_ok())
        .then_some(ABI_OK)
        .unwrap_or(0)
}
unsafe extern "C" fn record_predicate(
    context: *mut c_void,
    request: *const NativeResolutionRecordPredicateRequest,
) -> i32 {
    if context.is_null() || request.is_null() {
        return 0;
    }
    let request = unsafe { *request };
    let bridge = unsafe { &mut *context.cast::<RuntimeResolutionBridge>() };
    i32::from(bridge.session_mut(request.session).is_some_and(|session| {
        session
            .record_predicate(request.program_depth, request.passed)
            .is_ok()
    }))
}
unsafe extern "C" fn record_sequence(
    context: *mut c_void,
    request: *const NativeResolutionRecordSequenceRequest,
) -> i32 {
    if context.is_null() || request.is_null() {
        return 0;
    }
    let request = unsafe { *request };
    let bridge = unsafe { &mut *context.cast::<RuntimeResolutionBridge>() };
    i32::from(
        bridge
            .session_mut(request.session)
            .is_some_and(|session| session.record_sequence(request.program_depth).is_ok()),
    )
}
unsafe extern "C" fn record_operation(
    context: *mut c_void,
    request: *const NativeResolutionRecordOperationRequest,
) -> i32 {
    if context.is_null() || request.is_null() {
        return 0;
    }
    let request = unsafe { *request };
    let bridge = unsafe { &mut *context.cast::<RuntimeResolutionBridge>() };
    i32::from(bridge.session_mut(request.session).is_some_and(|session| {
        session
            .record_operation(
                request.program_depth,
                request.effects as usize,
                request.events as usize,
            )
            .is_ok()
    }))
}
unsafe extern "C" fn record_interceptor(
    context: *mut c_void,
    request: *const NativeResolutionRecordInterceptorRequest,
) -> i32 {
    if context.is_null() || request.is_null() {
        return 0;
    }
    let request = unsafe { *request };
    let bridge = unsafe { &mut *context.cast::<RuntimeResolutionBridge>() };
    i32::from(bridge.session_mut(request.session).is_some_and(|session| {
        session
            .record_interceptor(request.effects as usize, request.events as usize)
            .is_ok()
    }))
}
unsafe extern "C" fn begin_child(
    context: *mut c_void,
    request: *const NativeResolutionBeginChildRequest,
    result: *mut NativeResolutionChildReceipt,
) -> i32 {
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let request = unsafe { *request };
    let bridge = unsafe { &mut *context.cast::<RuntimeResolutionBridge>() };
    match bridge.session_mut(request.session).and_then(|session| {
        session
            .begin_child(budget(request.budget), request.evidence as usize)
            .ok()
    }) {
        Some(value) => {
            unsafe {
                *result = NativeResolutionChildReceipt {
                    identity: identity(value),
                }
            };
            ABI_OK
        }
        None => 0,
    }
}
unsafe extern "C" fn complete_attempt(
    context: *mut c_void,
    request: *const NativeResolutionCompleteAttemptRequest,
) -> i32 {
    if context.is_null() || request.is_null() {
        return 0;
    }
    let request = unsafe { *request };
    let bridge = unsafe { &mut *context.cast::<RuntimeResolutionBridge>() };
    i32::from(
        bridge
            .session_mut(request.session)
            .is_some_and(|session| session.complete_attempt(attempt(request.status)).is_ok()),
    )
}
unsafe extern "C" fn prepare_finalization(
    context: *mut c_void,
    handle: NativeResolutionSessionHandle,
) -> i32 {
    terminal(
        context,
        handle,
        StructuralResolutionSession::prepare_finalization,
    )
}
unsafe extern "C" fn finalize_preview(
    context: *mut c_void,
    handle: NativeResolutionSessionHandle,
) -> i32 {
    terminal(
        context,
        handle,
        StructuralResolutionSession::finalize_preview,
    )
}
unsafe extern "C" fn finalize_applied(
    context: *mut c_void,
    handle: NativeResolutionSessionHandle,
) -> i32 {
    terminal(
        context,
        handle,
        StructuralResolutionSession::finalize_applied,
    )
}
unsafe extern "C" fn finalize_failed(
    context: *mut c_void,
    handle: NativeResolutionSessionHandle,
) -> i32 {
    terminal(
        context,
        handle,
        StructuralResolutionSession::finalize_failed,
    )
}
fn terminal(
    context: *mut c_void,
    handle: NativeResolutionSessionHandle,
    operation: fn(
        &mut StructuralResolutionSession,
    ) -> Result<(), gameplay_resolution::StructuralResolutionError>,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeResolutionBridge>() };
    i32::from(
        bridge
            .session_mut(handle)
            .is_some_and(|session| operation(session).is_ok()),
    )
}
unsafe extern "C" fn read_session(
    context: *mut c_void,
    request: *const NativeResolutionSessionReadRequest,
    result: *mut NativeResolutionSessionReadoutLease,
) -> i32 {
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let request = unsafe { *request };
    match unsafe { (&mut *context.cast::<RuntimeResolutionBridge>()).read(request.session) } {
        Some(value) => {
            unsafe { *result = value };
            ABI_OK
        }
        None => 0,
    }
}
unsafe extern "C" fn destroy_readout_lease(
    context: *mut c_void,
    handle: NativeResolutionReadoutLeaseHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    i32::from(unsafe { (&mut *context.cast::<RuntimeResolutionBridge>()).destroy_readout(handle) })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn limits() -> NativeResolutionLimits {
        NativeResolutionLimits {
            max_evidence: 4,
            max_program_nodes: 8,
            max_program_depth: 4,
            max_interceptors: 4,
            max_effects: 8,
            max_events: 8,
            max_trace_records: 64,
            max_child_resolutions: 4,
            max_child_depth: 4,
        }
    }
    fn budget() -> NativeResolutionStructuralBudget {
        NativeResolutionStructuralBudget {
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
    fn create(
        bridge: &mut RuntimeResolutionBridge,
        mode: NativeResolutionMode,
    ) -> NativeResolutionSessionHandle {
        bridge
            .create(NativeResolutionSessionCreateRequest {
                root_resolution: 7,
                correlation: 9,
                mode,
                limits: limits(),
                root_budget: budget(),
                root_evidence: 1,
            })
            .unwrap()
    }
    fn plan(session: &mut StructuralResolutionSession) {
        for phase in [
            ResolutionPhase::Admit,
            ResolutionPhase::Gather,
            ResolutionPhase::Check,
            ResolutionPhase::Plan,
            ResolutionPhase::BeforeCommit,
        ] {
            session.begin_phase(phase).unwrap();
            if phase == ResolutionPhase::Check {
                // Check has no structural program traversal.
            }
            if phase == ResolutionPhase::Plan {
                session.record_sequence(1).unwrap();
                session.record_predicate(2, true).unwrap();
                session.record_operation(1, 1, 1).unwrap();
            }
            if phase == ResolutionPhase::BeforeCommit {
                session.record_interceptor(1, 1).unwrap();
            }
            session.complete_phase(phase).unwrap();
        }
    }
    #[test]
    fn bridge_exercises_preview_apply_failure_child_limit_read_and_release() {
        let mut bridge = RuntimeResolutionBridge::new();
        let preview = create(&mut bridge, NativeResolutionMode::Preview);
        plan(bridge.session_mut(preview).unwrap());
        let session = bridge.session_mut(preview).unwrap();
        session
            .complete_attempt(StructuralAttemptStatus::Planned)
            .unwrap();
        session.prepare_finalization().unwrap();
        session.finalize_preview().unwrap();
        let preview_readout = bridge.read(preview).unwrap();
        assert_eq!(preview_readout.attempts_len, 1);
        assert!(bridge.destroy_readout(preview_readout.handle));

        let apply = create(&mut bridge, NativeResolutionMode::Apply);
        plan(bridge.session_mut(apply).unwrap());
        let session = bridge.session_mut(apply).unwrap();
        session
            .complete_attempt(StructuralAttemptStatus::Planned)
            .unwrap();
        session.prepare_finalization().unwrap();
        session.finalize_applied().unwrap();

        let failed = create(&mut bridge, NativeResolutionMode::Apply);
        plan(bridge.session_mut(failed).unwrap());
        let session = bridge.session_mut(failed).unwrap();
        session
            .complete_attempt(StructuralAttemptStatus::Planned)
            .unwrap();
        session.prepare_finalization().unwrap();
        session.finalize_failed().unwrap();

        let child = create(&mut bridge, NativeResolutionMode::Preview);
        let session = bridge.session_mut(child).unwrap();
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
        let identity = session.begin_child(super::budget(budget()), 0).unwrap();
        assert_eq!(identity.parent().unwrap().get(), 7);
        session.begin_phase(ResolutionPhase::Admit).unwrap();
        session
            .complete_attempt(StructuralAttemptStatus::Rejected)
            .unwrap();
        assert!(session.prepare_finalization().is_err());
        let child_readout = bridge.read(child).unwrap();
        assert_eq!(child_readout.attempts_len, 2);
        assert!(bridge.destroy_readout(child_readout.handle));

        let limited = create(&mut bridge, NativeResolutionMode::Apply);
        let session = bridge.session_mut(limited).unwrap();
        session.begin_phase(ResolutionPhase::Admit).unwrap();
        session.complete_phase(ResolutionPhase::Admit).unwrap();
        session.begin_phase(ResolutionPhase::Gather).unwrap();
        session.complete_phase(ResolutionPhase::Gather).unwrap();
        session.begin_phase(ResolutionPhase::Check).unwrap();
        session.complete_phase(ResolutionPhase::Check).unwrap();
        session.begin_phase(ResolutionPhase::Plan).unwrap();
        assert!(session.record_operation(5, 0, 0).is_err());
        assert!(
            bridge.destroy(preview)
                && bridge.destroy(apply)
                && bridge.destroy(failed)
                && bridge.destroy(child)
                && bridge.destroy(limited)
        );
    }
}
