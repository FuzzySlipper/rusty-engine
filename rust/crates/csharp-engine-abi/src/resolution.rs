//! Typed, purpose-neutral gameplay-resolution session ABI.
//!
//! The retained owner validates only structural lifecycle and quotas. Product
//! policy, semantic payloads, state, effects, and transactions stay in C#.

use std::ffi::c_void;

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeResolutionMode {
    #[default]
    Preview = 0,
    Apply = 1,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeResolutionPhase {
    #[default]
    Admit = 0,
    Gather = 1,
    Check = 2,
    Plan = 3,
    BeforeCommit = 4,
    Commit = 5,
    Consequences = 6,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeResolutionAttemptStatus {
    #[default]
    Open = 0,
    Planned = 1,
    Rejected = 2,
    Suspended = 3,
    Faulted = 4,
    LimitExceeded = 5,
    ChildFailed = 6,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeResolutionCommitStatus {
    #[default]
    NotAttempted = 0,
    Prepared = 1,
    Previewed = 2,
    Applied = 3,
    TransactionFailed = 4,
    Abandoned = 5,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeResolutionTraceKind {
    #[default]
    PhaseStarted = 0,
    PhaseCompleted = 1,
    PredicateEvaluated = 2,
    OperationPlanned = 3,
    InterceptorApplied = 4,
    ChildStarted = 5,
    ChildCompleted = 6,
    CommitApplied = 7,
    PreviewAborted = 8,
    EffectsStaged = 9,
    Rejected = 10,
    Suspended = 11,
    Faulted = 12,
    LimitExceeded = 13,
    ChildFailed = 14,
    TransactionFailed = 15,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeResolutionLimits {
    pub max_evidence: u32,
    pub max_program_nodes: u32,
    pub max_program_depth: u16,
    pub max_interceptors: u32,
    pub max_effects: u32,
    pub max_events: u32,
    pub max_trace_records: u32,
    pub max_child_resolutions: u32,
    pub max_child_depth: u16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeResolutionStructuralBudget {
    pub max_evidence: u32,
    pub max_program_nodes: u32,
    pub max_program_depth: u16,
    pub max_interceptors: u32,
    pub max_effects: u32,
    pub max_events: u32,
    pub max_trace_records: u32,
    pub max_children: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeResolutionSessionHandle {
    pub value: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeResolutionReadoutLeaseHandle {
    pub value: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeResolutionSessionCreateRequest {
    pub root_resolution: u64,
    pub correlation: u64,
    pub mode: NativeResolutionMode,
    pub limits: NativeResolutionLimits,
    pub root_budget: NativeResolutionStructuralBudget,
    pub root_evidence: u32,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeResolutionBeginPhaseRequest {
    pub session: NativeResolutionSessionHandle,
    pub phase: NativeResolutionPhase,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeResolutionRecordPredicateRequest {
    pub session: NativeResolutionSessionHandle,
    pub program_depth: u16,
    pub passed: bool,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeResolutionRecordSequenceRequest {
    pub session: NativeResolutionSessionHandle,
    pub program_depth: u16,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeResolutionRecordOperationRequest {
    pub session: NativeResolutionSessionHandle,
    pub program_depth: u16,
    pub effects: u32,
    pub events: u32,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeResolutionRecordInterceptorRequest {
    pub session: NativeResolutionSessionHandle,
    pub effects: u32,
    pub events: u32,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeResolutionBeginChildRequest {
    pub session: NativeResolutionSessionHandle,
    pub budget: NativeResolutionStructuralBudget,
    pub evidence: u32,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeResolutionIdentityRow {
    pub resolution: u64,
    pub correlation: u64,
    pub parent: u64,
    pub has_parent: bool,
    pub depth: u16,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeResolutionChildReceipt {
    pub identity: NativeResolutionIdentityRow,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeResolutionCompleteAttemptRequest {
    pub session: NativeResolutionSessionHandle,
    pub status: NativeResolutionAttemptStatus,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeResolutionSessionReadRequest {
    pub session: NativeResolutionSessionHandle,
}

/// One copied, nonrecursive attempt result.  `parent` is ignored unless
/// `has_parent` is true. `mode` and `commit` make a readout self-describing.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeResolutionAttemptReadoutRow {
    pub identity: NativeResolutionIdentityRow,
    pub mode: NativeResolutionMode,
    pub is_root: bool,
    pub status: NativeResolutionAttemptStatus,
    pub commit: NativeResolutionCommitStatus,
    pub evidence: u32,
    pub program_nodes: u32,
    pub program_depth: u16,
    pub interceptors: u32,
    pub effects: u32,
    pub events: u32,
    pub children: u32,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeResolutionTraceReadoutRow {
    pub identity: NativeResolutionIdentityRow,
    pub phase: NativeResolutionPhase,
    pub kind: NativeResolutionTraceKind,
    pub scalar: u32,
    pub passed: bool,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeResolutionSessionReadoutLease {
    pub handle: NativeResolutionReadoutLeaseHandle,
    pub attempts: *const NativeResolutionAttemptReadoutRow,
    pub attempts_len: usize,
    pub traces: *const NativeResolutionTraceReadoutRow,
    pub traces_len: usize,
}

pub type NativeCreateResolutionSession = unsafe extern "C" fn(
    *mut c_void,
    *const NativeResolutionSessionCreateRequest,
    *mut NativeResolutionSessionHandle,
) -> i32;
pub type NativeDestroyResolutionSession =
    unsafe extern "C" fn(*mut c_void, NativeResolutionSessionHandle) -> i32;
pub type NativeBeginResolutionPhase =
    unsafe extern "C" fn(*mut c_void, *const NativeResolutionBeginPhaseRequest) -> i32;
pub type NativeCompleteResolutionPhase =
    unsafe extern "C" fn(*mut c_void, *const NativeResolutionBeginPhaseRequest) -> i32;
pub type NativeRecordResolutionPredicate =
    unsafe extern "C" fn(*mut c_void, *const NativeResolutionRecordPredicateRequest) -> i32;
pub type NativeRecordResolutionSequence =
    unsafe extern "C" fn(*mut c_void, *const NativeResolutionRecordSequenceRequest) -> i32;
pub type NativeRecordResolutionOperation =
    unsafe extern "C" fn(*mut c_void, *const NativeResolutionRecordOperationRequest) -> i32;
pub type NativeRecordResolutionInterceptor =
    unsafe extern "C" fn(*mut c_void, *const NativeResolutionRecordInterceptorRequest) -> i32;
pub type NativeBeginResolutionChild = unsafe extern "C" fn(
    *mut c_void,
    *const NativeResolutionBeginChildRequest,
    *mut NativeResolutionChildReceipt,
) -> i32;
pub type NativeCompleteResolutionAttempt =
    unsafe extern "C" fn(*mut c_void, *const NativeResolutionCompleteAttemptRequest) -> i32;
pub type NativePrepareResolutionFinalization =
    unsafe extern "C" fn(*mut c_void, NativeResolutionSessionHandle) -> i32;
pub type NativeFinalizeResolutionPreview =
    unsafe extern "C" fn(*mut c_void, NativeResolutionSessionHandle) -> i32;
pub type NativeFinalizeResolutionApplied =
    unsafe extern "C" fn(*mut c_void, NativeResolutionSessionHandle) -> i32;
pub type NativeFinalizeResolutionFailed =
    unsafe extern "C" fn(*mut c_void, NativeResolutionSessionHandle) -> i32;
pub type NativeReadResolutionSession = unsafe extern "C" fn(
    *mut c_void,
    *const NativeResolutionSessionReadRequest,
    *mut NativeResolutionSessionReadoutLease,
) -> i32;
pub type NativeDestroyResolutionReadoutLease =
    unsafe extern "C" fn(*mut c_void, NativeResolutionReadoutLeaseHandle) -> i32;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeResolutionApi {
    pub context: *mut c_void,
    pub create_session: NativeCreateResolutionSession,
    pub destroy_session: NativeDestroyResolutionSession,
    pub begin_phase: NativeBeginResolutionPhase,
    pub complete_phase: NativeCompleteResolutionPhase,
    pub record_predicate: NativeRecordResolutionPredicate,
    pub record_sequence: NativeRecordResolutionSequence,
    pub record_operation: NativeRecordResolutionOperation,
    pub record_interceptor: NativeRecordResolutionInterceptor,
    pub begin_child: NativeBeginResolutionChild,
    pub complete_attempt: NativeCompleteResolutionAttempt,
    pub prepare_finalization: NativePrepareResolutionFinalization,
    pub finalize_preview: NativeFinalizeResolutionPreview,
    pub finalize_applied: NativeFinalizeResolutionApplied,
    pub finalize_failed: NativeFinalizeResolutionFailed,
    pub read_session: NativeReadResolutionSession,
    pub destroy_readout_lease: NativeDestroyResolutionReadoutLease,
}
