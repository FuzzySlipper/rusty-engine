//! NativeAOT bridge for the detached state-machine owner.
//!
//! Definitions are retained behind typed handles. Detached instances remain
//! caller-owned values; this bridge never mirrors EntityState or stores an
//! instance map.

use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::c_void,
};

use csharp_engine_abi::*;
use state_machine::{
    apply_detached_transition, DetachedMachineInstance, DetachedTransitionRequest,
    StateMachineError, StateMachineSpec, MAX_DETACHED_DEFINITION_STATES,
    MAX_DETACHED_DEFINITION_TRANSITIONS,
};

use crate::composition::{borrowed_slice, ABI_OK};

const SERVICE: &[u8] = b"StateMachine";
const ADMIT_OPERATION: &[u8] = b"AdmitDefinition";
const APPLY_OPERATION: &[u8] = b"ApplyTransition";
const MAX_DIAGNOSTIC_SOURCE_BYTES: usize = 512;

pub(crate) struct RuntimeStateMachineBridge {
    definitions: BTreeMap<u64, StateMachineSpec>,
    next_definition: u64,
    readout_leases: BTreeMap<u64, StateMachineDefinitionReadoutBacking>,
    next_readout_lease: u64,
    diagnostic_leases: BTreeMap<u64, StateMachineDiagnosticLease>,
    next_diagnostic_lease: u64,
}

struct StateMachineDefinitionReadoutBacking {
    definitions: Vec<NativeStateMachineDefinitionReadoutRow>,
    states: Vec<NativeStateMachineState>,
    transitions: Vec<NativeStateMachineTransition>,
}

struct StateMachineDiagnosticLease {
    _values: Vec<StateMachineDiagnosticValue>,
    readout: Vec<NativeEngineDiagnostic>,
}

struct StateMachineDiagnosticValue {
    code: String,
    message: String,
    source: String,
}

#[derive(Debug)]
enum StateMachineOperationError {
    Request {
        code: &'static str,
        source: &'static str,
    },
    Kernel(StateMachineError),
    UnknownDefinition {
        value: u64,
    },
    LeaseExhausted {
        field: &'static str,
    },
}

impl RuntimeStateMachineBridge {
    pub(crate) fn new() -> Self {
        Self {
            definitions: BTreeMap::new(),
            next_definition: 1,
            readout_leases: BTreeMap::new(),
            next_readout_lease: 1,
            diagnostic_leases: BTreeMap::new(),
            next_diagnostic_lease: 1,
        }
    }

    fn admit(
        &mut self,
        request: NativeStateMachineDefinitionRequest,
        states: &[NativeStateMachineState],
        transitions: &[NativeStateMachineTransition],
    ) -> Result<NativeStateMachineDefinitionHandle, StateMachineOperationError> {
        let machine = core_ids::ProcessId::new(request.machine);
        if states.len() > MAX_DETACHED_DEFINITION_STATES {
            return Err(StateMachineOperationError::Kernel(
                StateMachineError::DefinitionStateLimitExceeded {
                    machine,
                    maximum: MAX_DETACHED_DEFINITION_STATES,
                    actual: states.len(),
                },
            ));
        }
        if transitions.len() > MAX_DETACHED_DEFINITION_TRANSITIONS {
            return Err(StateMachineOperationError::Kernel(
                StateMachineError::DefinitionTransitionLimitExceeded {
                    machine,
                    maximum: MAX_DETACHED_DEFINITION_TRANSITIONS,
                    actual: transitions.len(),
                },
            ));
        }

        let mut state_ids = BTreeSet::new();
        for state in states {
            let value = core_ids::ModeId::new(state.value);
            if !state_ids.insert(value) {
                return Err(StateMachineOperationError::Kernel(
                    StateMachineError::DuplicateState {
                        machine,
                        state: value,
                    },
                ));
            }
        }

        if self
            .definitions
            .values()
            .any(|definition| definition.machine == machine)
        {
            return Err(StateMachineOperationError::Kernel(
                StateMachineError::MachineAlreadyDefined { machine },
            ));
        }

        let mut transition_ids = BTreeSet::new();
        for transition in transitions {
            let edge = (
                core_ids::ModeId::new(transition.from),
                core_ids::ModeId::new(transition.to),
            );
            if !transition_ids.insert(edge) {
                return Err(StateMachineOperationError::Kernel(
                    StateMachineError::DuplicateTransition {
                        machine,
                        from: edge.0,
                        to: edge.1,
                    },
                ));
            }
        }

        let mut spec = StateMachineSpec::new(machine, state_ids.iter().copied());
        for (from, to) in transition_ids {
            spec = spec.allow(from, to);
        }
        spec.validate_detached()
            .map_err(StateMachineOperationError::Kernel)?;

        let value = self.next_definition;
        self.next_definition =
            value
                .checked_add(1)
                .ok_or(StateMachineOperationError::LeaseExhausted {
                    field: "stateMachine.definitionHandle",
                })?;
        self.definitions.insert(value, spec);
        Ok(NativeStateMachineDefinitionHandle { value })
    }

    fn destroy_definition(&mut self, handle: NativeStateMachineDefinitionHandle) -> bool {
        handle.value != 0 && self.definitions.remove(&handle.value).is_some()
    }

    fn read_definition(
        &mut self,
        handle: NativeStateMachineDefinitionHandle,
    ) -> Option<NativeStateMachineDefinitionReadoutLease> {
        let spec = self.definitions.get(&handle.value)?;
        let lease_value = self.next_readout_lease;
        self.next_readout_lease = lease_value.checked_add(1)?;
        let states = spec
            .states()
            .map(|state| NativeStateMachineState { value: state.raw() })
            .collect::<Vec<_>>();
        let transitions = spec
            .transitions()
            .map(|(from, to)| NativeStateMachineTransition {
                from: from.raw(),
                to: to.raw(),
            })
            .collect::<Vec<_>>();
        let states_len = u32::try_from(states.len()).ok()?;
        let transitions_len = u32::try_from(transitions.len()).ok()?;
        let backing = StateMachineDefinitionReadoutBacking {
            definitions: vec![NativeStateMachineDefinitionReadoutRow {
                machine: spec.machine.raw(),
                states_start: 0,
                states_len,
                transitions_start: 0,
                transitions_len,
            }],
            states,
            transitions,
        };
        let lease = NativeStateMachineDefinitionReadoutLease {
            handle: NativeStateMachineDefinitionReadoutLeaseHandle { value: lease_value },
            definitions: backing.definitions.as_ptr(),
            definitions_len: backing.definitions.len(),
            states: backing.states.as_ptr(),
            states_len: backing.states.len(),
            transitions: backing.transitions.as_ptr(),
            transitions_len: backing.transitions.len(),
        };
        self.readout_leases.insert(lease_value, backing);
        Some(lease)
    }

    fn destroy_definition_readout_lease(
        &mut self,
        handle: NativeStateMachineDefinitionReadoutLeaseHandle,
    ) -> bool {
        handle.value != 0 && self.readout_leases.remove(&handle.value).is_some()
    }

    fn apply(
        &self,
        request: NativeStateMachineTransitionRequest,
    ) -> Result<NativeStateMachineTransitionReceipt, StateMachineOperationError> {
        let spec = self.definitions.get(&request.definition.value).ok_or(
            StateMachineOperationError::UnknownDefinition {
                value: request.definition.value,
            },
        )?;
        let instance = DetachedMachineInstance::new(
            core_ids::ProcessId::new(request.instance.machine),
            core_ids::ModeId::new(request.instance.current),
            request.instance.revision,
        );
        let mut transition = DetachedTransitionRequest::new(
            core_ids::ModeId::new(request.expected),
            core_ids::ModeId::new(request.next),
        );
        if request.has_expected_revision {
            transition = transition.expecting_revision(request.expected_revision);
        }
        let applied = apply_detached_transition(spec, instance, transition)
            .map_err(StateMachineOperationError::Kernel)?;
        Ok(NativeStateMachineTransitionReceipt {
            instance: NativeStateMachineInstance {
                machine: applied.instance.machine.raw(),
                current: applied.instance.current.raw(),
                revision: applied.instance.revision,
            },
            previous: applied.previous.raw(),
            revision: applied.revision,
        })
    }

    fn retain_operation_diagnostic(
        &mut self,
        error: &StateMachineOperationError,
    ) -> Option<NativeEngineDiagnosticLease> {
        let handle = self.next_diagnostic_lease;
        self.next_diagnostic_lease = handle.checked_add(1)?;
        let value = StateMachineDiagnosticValue::from_operation_error(error);
        let lease = StateMachineDiagnosticLease::new(value);
        let native = NativeEngineDiagnosticLease {
            handle: NativeEngineDiagnosticLeaseHandle { value: handle },
            diagnostics: lease.readout.as_ptr(),
            diagnostics_len: lease.readout.len(),
        };
        self.diagnostic_leases.insert(handle, lease);
        Some(native)
    }

    fn destroy_diagnostic_lease(&mut self, handle: NativeEngineDiagnosticLeaseHandle) -> bool {
        handle.value != 0 && self.diagnostic_leases.remove(&handle.value).is_some()
    }
}

impl StateMachineDiagnosticLease {
    fn new(value: StateMachineDiagnosticValue) -> Self {
        let values = vec![value];
        let readout = values
            .iter()
            .map(|value| NativeEngineDiagnostic {
                code: native_utf8(value.code.as_bytes()),
                message: native_utf8(value.message.as_bytes()),
                source: native_utf8(value.source.as_bytes()),
            })
            .collect();
        Self {
            _values: values,
            readout,
        }
    }
}

impl StateMachineDiagnosticValue {
    fn fixed(code: &'static str, message: &'static str, source: impl Into<String>) -> Self {
        let source = source.into();
        Self {
            code: code.to_owned(),
            message: message.to_owned(),
            source: source.chars().take(MAX_DIAGNOSTIC_SOURCE_BYTES).collect(),
        }
    }

    fn from_operation_error(error: &StateMachineOperationError) -> Self {
        match error {
            StateMachineOperationError::Request { code, source } => {
                Self::fixed(code, "State-machine request was not well formed.", *source)
            }
            StateMachineOperationError::UnknownDefinition { value } => Self::fixed(
                "STATE_MACHINE_DEFINITION_HANDLE",
                "State-machine operation received an unknown retained definition handle.",
                value.to_string(),
            ),
            StateMachineOperationError::LeaseExhausted { field } => Self::fixed(
                "STATE_MACHINE_LEASE_ARITHMETIC",
                "State-machine handle allocation overflowed.",
                *field,
            ),
            StateMachineOperationError::Kernel(error) => Self::fixed(
                kernel_code(error),
                "State-machine operation was rejected.",
                format!("{error:?}"),
            ),
        }
    }
}

fn kernel_code(error: &StateMachineError) -> &'static str {
    match error {
        StateMachineError::EmptyMachine { .. } => "STATE_MACHINE_EMPTY_DEFINITION",
        StateMachineError::MachineAlreadyDefined { .. } => "STATE_MACHINE_MACHINE_ALREADY_DEFINED",
        StateMachineError::MachineMissing { .. } => "STATE_MACHINE_MACHINE_MISMATCH",
        StateMachineError::InvalidState { .. } => "STATE_MACHINE_INVALID_STATE",
        StateMachineError::InvalidTransition { .. } => "STATE_MACHINE_INVALID_TRANSITION",
        StateMachineError::DetachedStaleCurrentState { .. } => "STATE_MACHINE_STALE_STATE",
        StateMachineError::DetachedStaleRevision { .. } => "STATE_MACHINE_STALE_REVISION",
        StateMachineError::DetachedRevisionOverflow { .. } => "STATE_MACHINE_REVISION_OVERFLOW",
        StateMachineError::DuplicateState { .. } => "STATE_MACHINE_DUPLICATE_STATE",
        StateMachineError::DuplicateTransition { .. } => "STATE_MACHINE_DUPLICATE_TRANSITION",
        StateMachineError::DefinitionStateLimitExceeded { .. } => "STATE_MACHINE_STATE_QUOTA",
        StateMachineError::DefinitionTransitionLimitExceeded { .. } => {
            "STATE_MACHINE_TRANSITION_QUOTA"
        }
        StateMachineError::EntityMissing { .. }
        | StateMachineError::EntityInactive { .. }
        | StateMachineError::InstanceAlreadyAttached { .. }
        | StateMachineError::InstanceMissing { .. }
        | StateMachineError::StaleCurrentState { .. }
        | StateMachineError::StaleRevision { .. }
        | StateMachineError::RevisionOverflow { .. } => "STATE_MACHINE_ENTITY_PATH_ERROR",
    }
}

fn native_utf8(value: &[u8]) -> NativeUtf8Slice {
    NativeUtf8Slice {
        bytes: value.as_ptr(),
        len: value.len(),
    }
}

pub(crate) fn api(bridge: &mut RuntimeStateMachineBridge) -> NativeStateMachineApi {
    NativeStateMachineApi {
        context: (bridge as *mut RuntimeStateMachineBridge).cast(),
        admit_definition,
        destroy_definition,
        read_definition,
        destroy_definition_readout_lease,
        apply_transition,
        destroy_operation_diagnostic_lease,
    }
}

unsafe extern "C" fn admit_definition(
    context: *mut c_void,
    request: *const NativeStateMachineDefinitionRequest,
    result: *mut NativeStateMachineDefinitionHandle,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    if receipt.is_null() {
        return 0;
    }
    // SAFETY: receipt was checked and is reset before this direct operation.
    unsafe { *receipt = std::mem::zeroed() };
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    // SAFETY: request was checked and its borrowed spans are valid only for this call.
    let request = unsafe { *request };
    let states =
        match unsafe { borrowed_slice(request.states, request.states_len, "state-machine states") }
        {
            Ok(value) => value,
            Err(_) => {
                let bridge = unsafe { &mut *context.cast::<RuntimeStateMachineBridge>() };
                let error = StateMachineOperationError::Request {
                    code: "STATE_MACHINE_STATES_POINTER",
                    source: "states",
                };
                if let Some(diagnostics) = bridge.retain_operation_diagnostic(&error) {
                    unsafe {
                        *receipt = NativeOperationErrorReceipt {
                            service: native_utf8(SERVICE),
                            operation: native_utf8(ADMIT_OPERATION),
                            status: 0,
                            diagnostics,
                        };
                    }
                }
                return 0;
            }
        };
    let transitions = match unsafe {
        borrowed_slice(
            request.transitions,
            request.transitions_len,
            "state-machine transitions",
        )
    } {
        Ok(value) => value,
        Err(_) => {
            let bridge = unsafe { &mut *context.cast::<RuntimeStateMachineBridge>() };
            let error = StateMachineOperationError::Request {
                code: "STATE_MACHINE_TRANSITIONS_POINTER",
                source: "transitions",
            };
            if let Some(diagnostics) = bridge.retain_operation_diagnostic(&error) {
                unsafe {
                    *receipt = NativeOperationErrorReceipt {
                        service: native_utf8(SERVICE),
                        operation: native_utf8(ADMIT_OPERATION),
                        status: 0,
                        diagnostics,
                    };
                }
            }
            return 0;
        }
    };
    // SAFETY: context is the stable bridge retained for the product lifetime.
    let bridge = unsafe { &mut *context.cast::<RuntimeStateMachineBridge>() };
    match bridge.admit(request, states, transitions) {
        Ok(handle) => {
            // SAFETY: result was checked and belongs to this direct call.
            unsafe { *result = handle };
            ABI_OK
        }
        Err(error) => {
            if let Some(diagnostics) = bridge.retain_operation_diagnostic(&error) {
                // SAFETY: receipt was checked and names the retained diagnostic lease.
                unsafe {
                    *receipt = NativeOperationErrorReceipt {
                        service: native_utf8(SERVICE),
                        operation: native_utf8(ADMIT_OPERATION),
                        status: 0,
                        diagnostics,
                    };
                }
            }
            0
        }
    }
}

unsafe extern "C" fn destroy_definition(
    context: *mut c_void,
    handle: NativeStateMachineDefinitionHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    // SAFETY: context is the stable bridge retained for the product lifetime.
    let bridge = unsafe { &mut *context.cast::<RuntimeStateMachineBridge>() };
    i32::from(bridge.destroy_definition(handle))
}

unsafe extern "C" fn read_definition(
    context: *mut c_void,
    handle: NativeStateMachineDefinitionHandle,
    result: *mut NativeStateMachineDefinitionReadoutLease,
) -> i32 {
    if context.is_null() || result.is_null() {
        return 0;
    }
    // SAFETY: context is the stable bridge and result belongs to this direct call.
    let bridge = unsafe { &mut *context.cast::<RuntimeStateMachineBridge>() };
    match bridge.read_definition(handle) {
        Some(lease) => {
            unsafe { *result = lease };
            ABI_OK
        }
        None => 0,
    }
}

unsafe extern "C" fn destroy_definition_readout_lease(
    context: *mut c_void,
    handle: NativeStateMachineDefinitionReadoutLeaseHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    // SAFETY: context is the stable bridge retained for the product lifetime.
    let bridge = unsafe { &mut *context.cast::<RuntimeStateMachineBridge>() };
    i32::from(bridge.destroy_definition_readout_lease(handle))
}

unsafe extern "C" fn apply_transition(
    context: *mut c_void,
    request: *const NativeStateMachineTransitionRequest,
    result: *mut NativeStateMachineTransitionReceipt,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    if receipt.is_null() {
        return 0;
    }
    // SAFETY: receipt was checked and is reset before this direct operation.
    unsafe { *receipt = std::mem::zeroed() };
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    // SAFETY: request was checked and borrowed only for this direct call.
    let request = unsafe { *request };
    // SAFETY: context is the stable bridge retained for the product lifetime.
    let bridge = unsafe { &mut *context.cast::<RuntimeStateMachineBridge>() };
    match bridge.apply(request) {
        Ok(value) => {
            // SAFETY: result was checked and belongs to this direct call.
            unsafe { *result = value };
            ABI_OK
        }
        Err(error) => {
            if let Some(diagnostics) = bridge.retain_operation_diagnostic(&error) {
                // SAFETY: receipt was checked and names the retained diagnostic lease.
                unsafe {
                    *receipt = NativeOperationErrorReceipt {
                        service: native_utf8(SERVICE),
                        operation: native_utf8(APPLY_OPERATION),
                        status: 0,
                        diagnostics,
                    };
                }
            }
            0
        }
    }
}

unsafe extern "C" fn destroy_operation_diagnostic_lease(
    context: *mut c_void,
    handle: NativeEngineDiagnosticLeaseHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    // SAFETY: context is the stable bridge retained for the product lifetime.
    let bridge = unsafe { &mut *context.cast::<RuntimeStateMachineBridge>() };
    i32::from(bridge.destroy_diagnostic_lease(handle))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn definition_request<'a>(
        states: &'a [NativeStateMachineState],
        transitions: &'a [NativeStateMachineTransition],
    ) -> NativeStateMachineDefinitionRequest {
        NativeStateMachineDefinitionRequest {
            machine: 10,
            states: states.as_ptr(),
            states_len: states.len(),
            transitions: transitions.as_ptr(),
            transitions_len: transitions.len(),
        }
    }

    fn definition(bridge: &mut RuntimeStateMachineBridge) -> NativeStateMachineDefinitionHandle {
        let states = [
            NativeStateMachineState { value: 1 },
            NativeStateMachineState { value: 2 },
            NativeStateMachineState { value: 3 },
        ];
        let transitions = [
            NativeStateMachineTransition { from: 1, to: 2 },
            NativeStateMachineTransition { from: 2, to: 3 },
        ];
        bridge
            .admit(
                definition_request(&states, &transitions),
                &states,
                &transitions,
            )
            .unwrap()
    }

    fn transition_request(
        definition: NativeStateMachineDefinitionHandle,
        current: u64,
        revision: u64,
    ) -> NativeStateMachineTransitionRequest {
        NativeStateMachineTransitionRequest {
            definition,
            instance: NativeStateMachineInstance {
                machine: 10,
                current,
                revision,
            },
            expected: current,
            next: current + 1,
            has_expected_revision: true,
            expected_revision: revision,
        }
    }

    #[test]
    fn definition_readout_is_sorted_and_exactly_releasable() {
        let mut bridge = RuntimeStateMachineBridge::new();
        let handle = definition(&mut bridge);
        let lease = bridge.read_definition(handle).unwrap();
        assert_eq!(lease.definitions_len, 1);
        assert_eq!(lease.states_len, 3);
        assert_eq!(lease.transitions_len, 2);
        let states = unsafe { std::slice::from_raw_parts(lease.states, lease.states_len) };
        assert_eq!(
            states.iter().map(|state| state.value).collect::<Vec<_>>(),
            [1, 2, 3]
        );
        assert!(bridge.destroy_definition_readout_lease(lease.handle));
        assert!(!bridge.destroy_definition_readout_lease(lease.handle));
        assert!(bridge.destroy_definition(handle));
        assert!(!bridge.destroy_definition(handle));
    }

    #[test]
    fn transition_is_caller_owned_and_stale_failures_do_not_mutate() {
        let mut bridge = RuntimeStateMachineBridge::new();
        let handle = definition(&mut bridge);
        let request = transition_request(handle, 1, 0);
        let applied = bridge.apply(request).unwrap();
        assert_eq!(applied.instance.current, 2);
        assert_eq!(applied.instance.revision, 1);

        let stale = NativeStateMachineTransitionRequest {
            instance: NativeStateMachineInstance {
                machine: 10,
                current: 2,
                revision: 1,
            },
            expected: 2,
            next: 3,
            has_expected_revision: true,
            expected_revision: 0,
            ..transition_request(handle, 1, 0)
        };
        let error = bridge.apply(stale).unwrap_err();
        assert!(matches!(
            error,
            StateMachineOperationError::Kernel(StateMachineError::DetachedStaleRevision { .. })
        ));
        assert_eq!(stale.instance.current, 2);
        assert_eq!(stale.instance.revision, 1);
    }

    #[test]
    fn abi_reports_stale_transition_and_releases_diagnostic() {
        let mut bridge = RuntimeStateMachineBridge::new();
        let api = api(&mut bridge);
        let states = [
            NativeStateMachineState { value: 1 },
            NativeStateMachineState { value: 2 },
        ];
        let transitions = [NativeStateMachineTransition { from: 1, to: 2 }];
        let request = definition_request(&states, &transitions);
        let mut definition = NativeStateMachineDefinitionHandle::default();
        let mut admit_receipt: NativeOperationErrorReceipt = unsafe { std::mem::zeroed() };
        let admitted = unsafe {
            (api.admit_definition)(api.context, &request, &mut definition, &mut admit_receipt)
        };
        assert_eq!(admitted, ABI_OK);

        let transition = NativeStateMachineTransitionRequest {
            definition,
            instance: NativeStateMachineInstance {
                machine: 10,
                current: 1,
                revision: 3,
            },
            expected: 1,
            next: 2,
            has_expected_revision: true,
            expected_revision: 4,
        };
        let mut result = NativeStateMachineTransitionReceipt::default();
        let mut receipt: NativeOperationErrorReceipt = unsafe { std::mem::zeroed() };
        let status =
            unsafe { (api.apply_transition)(api.context, &transition, &mut result, &mut receipt) };
        assert_eq!(status, 0);
        assert_eq!(receipt.service.len, SERVICE.len());
        assert_eq!(receipt.operation.len, APPLY_OPERATION.len());
        assert_ne!(receipt.diagnostics.handle.value, 0);
        assert_eq!(
            unsafe { std::slice::from_raw_parts(receipt.diagnostics.diagnostics, 1) }[0]
                .code
                .len,
            "STATE_MACHINE_STALE_REVISION".len()
        );
        assert_eq!(
            unsafe {
                (api.destroy_operation_diagnostic_lease)(api.context, receipt.diagnostics.handle)
            },
            ABI_OK
        );
        assert_eq!(
            unsafe { (api.destroy_definition)(api.context, definition) },
            ABI_OK
        );
    }
}
