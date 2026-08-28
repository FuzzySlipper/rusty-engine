//! Generated C ABI for the detached state-machine service.
//!
//! Definitions are retained by typed handles, while instances remain
//! caller-owned values. The service validates transitions and returns fixed
//! receipts; it does not retain an instance map or an entity mirror.

use crate::NativeOperationErrorReceipt;
use std::ffi::c_void;

/// One state identity in a borrowed definition or copied definition readout.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeStateMachineState {
    pub value: u64,
}

/// One directed edge in a flat state-machine definition.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeStateMachineTransition {
    pub from: u64,
    pub to: u64,
}

/// Typed owner for one validated retained definition.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeStateMachineDefinitionHandle {
    pub value: u64,
}

/// Typed owner for one copied definition readout lease.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeStateMachineDefinitionReadoutLeaseHandle {
    pub value: u64,
}

/// Borrowed flat definition rows. The service validates and copies these rows
/// before retaining the definition.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStateMachineDefinitionRequest {
    pub machine: u64,
    pub states: *const NativeStateMachineState,
    pub states_len: usize,
    pub transitions: *const NativeStateMachineTransition,
    pub transitions_len: usize,
}

/// One copied retained-definition row. The state and transition ranges are
/// offsets into the matching readout lease's flat arrays.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeStateMachineDefinitionReadoutRow {
    pub machine: u64,
    pub states_start: u32,
    pub states_len: u32,
    pub transitions_start: u32,
    pub transitions_len: u32,
}

/// One deterministic bounded readout of a retained definition.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStateMachineDefinitionReadoutLease {
    pub handle: NativeStateMachineDefinitionReadoutLeaseHandle,
    pub definitions: *const NativeStateMachineDefinitionReadoutRow,
    pub definitions_len: usize,
    pub states: *const NativeStateMachineState,
    pub states_len: usize,
    pub transitions: *const NativeStateMachineTransition,
    pub transitions_len: usize,
}

/// Caller-owned detached instance value. It carries machine identity only to
/// ensure the value is used with its matching retained definition; it is not
/// an entity identity and is never retained by the service.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeStateMachineInstance {
    pub machine: u64,
    pub current: u64,
    pub revision: u64,
}

/// Guarded transition over one caller-owned detached instance.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStateMachineTransitionRequest {
    pub definition: NativeStateMachineDefinitionHandle,
    pub instance: NativeStateMachineInstance,
    pub expected: u64,
    pub next: u64,
    pub has_expected_revision: bool,
    pub expected_revision: u64,
}

/// Fixed result for one successful detached transition.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeStateMachineTransitionReceipt {
    pub instance: NativeStateMachineInstance,
    pub previous: u64,
    pub revision: u64,
}

pub type NativeAdmitStateMachineDefinition = unsafe extern "C" fn(
    *mut c_void,
    *const NativeStateMachineDefinitionRequest,
    *mut NativeStateMachineDefinitionHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyStateMachineDefinition =
    unsafe extern "C" fn(*mut c_void, NativeStateMachineDefinitionHandle) -> i32;
pub type NativeReadStateMachineDefinition = unsafe extern "C" fn(
    *mut c_void,
    NativeStateMachineDefinitionHandle,
    *mut NativeStateMachineDefinitionReadoutLease,
) -> i32;
pub type NativeDestroyStateMachineDefinitionReadoutLease =
    unsafe extern "C" fn(*mut c_void, NativeStateMachineDefinitionReadoutLeaseHandle) -> i32;
pub type NativeApplyStateMachineTransition = unsafe extern "C" fn(
    *mut c_void,
    *const NativeStateMachineTransitionRequest,
    *mut NativeStateMachineTransitionReceipt,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyStateMachineOperationDiagnosticLease =
    unsafe extern "C" fn(*mut c_void, crate::NativeEngineDiagnosticLeaseHandle) -> i32;

/// Named detached state-machine Engine service.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStateMachineApi {
    pub context: *mut c_void,
    pub admit_definition: NativeAdmitStateMachineDefinition,
    pub destroy_definition: NativeDestroyStateMachineDefinition,
    pub read_definition: NativeReadStateMachineDefinition,
    pub destroy_definition_readout_lease: NativeDestroyStateMachineDefinitionReadoutLease,
    pub apply_transition: NativeApplyStateMachineTransition,
    pub destroy_operation_diagnostic_lease: NativeDestroyStateMachineOperationDiagnosticLease,
}
