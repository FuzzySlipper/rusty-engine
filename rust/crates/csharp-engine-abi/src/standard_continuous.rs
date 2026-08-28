//! Typed Standard Continuous definition and evaluation ABI.
//!
//! Values cross the ABI as their admitted finite IEEE-754 binary64 bits.

use crate::NativeUtf8Slice;

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeStandardContinuousNodeKind {
    #[default]
    Literal = 0,
    Input = 1,
    Add = 2,
    Subtract = 3,
    Multiply = 4,
    Divide = 5,
    Min = 6,
    Max = 7,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeStandardContinuousInputKind {
    #[default]
    Parameter = 0,
    Fact = 1,
    Roll = 2,
    Choice = 3,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeStandardContinuousComparisonKind {
    #[default]
    Equal = 0,
    LessThan = 1,
    LessOrEqual = 2,
    GreaterThan = 3,
    GreaterOrEqual = 4,
}

/// One flat post-order continuous expression node. Min/max child span offsets
/// refer to the request's explicit child-index table; child node indices must
/// precede their parent. Literal bits are admitted only through
/// `ContinuousValue::from_bits`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStandardContinuousNode {
    pub kind: NativeStandardContinuousNodeKind,
    pub literal_bits: u64,
    pub input_kind: NativeStandardContinuousInputKind,
    pub role: NativeUtf8Slice,
    pub input_id: NativeUtf8Slice,
    pub left: u32,
    pub right: u32,
    pub children_start: u32,
    pub children_len: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStandardContinuousRole {
    pub role: NativeUtf8Slice,
    pub capabilities_start: u32,
    pub capabilities_len: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStandardContinuousCapability {
    pub capability: NativeUtf8Slice,
}

/// One exact raw-bits evidence value; nonfinite bits are rejected by the
/// continuous owner. Duplicate input rows are never collapsed by this bridge.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStandardContinuousEvidence {
    pub kind: NativeStandardContinuousInputKind,
    pub role: NativeUtf8Slice,
    pub input_id: NativeUtf8Slice,
    pub value_bits: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStandardContinuousAdmitRequest {
    pub domain: NativeUtf8Slice,
    pub package: NativeUtf8Slice,
    pub package_version: u64,
    pub subject: NativeUtf8Slice,
    pub source: NativeUtf8Slice,
    pub source_path: NativeUtf8Slice,
    pub has_provenance_line: bool,
    pub provenance_line: u64,
    pub has_provenance_column: bool,
    pub provenance_column: u64,
    pub roles: *const NativeStandardContinuousRole,
    pub roles_len: usize,
    pub capabilities: *const NativeStandardContinuousCapability,
    pub capabilities_len: usize,
    pub nodes: *const NativeStandardContinuousNode,
    pub nodes_len: usize,
    pub child_indices: *const u32,
    pub child_indices_len: usize,
    pub root_node_index: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeStandardContinuousDefinitionHandle {
    pub value: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeStandardContinuousPredicateHandle {
    pub value: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeStandardContinuousReadoutLeaseHandle {
    pub value: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeStandardContinuousEvaluationLeaseHandle {
    pub value: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeStandardContinuousPredicateReadoutLeaseHandle {
    pub value: u64,
}
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeStandardContinuousPredicateEvaluationLeaseHandle {
    pub value: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStandardContinuousDefinitionReadoutRow {
    pub domain: NativeUtf8Slice,
    pub package: NativeUtf8Slice,
    pub package_version: u64,
    pub fingerprint: NativeUtf8Slice,
    pub subject: NativeUtf8Slice,
    pub source: NativeUtf8Slice,
    pub family: NativeUtf8Slice,
    pub semantics_version: u32,
    pub maximum_depth: u32,
    pub maximum_nodes: u32,
    pub maximum_inputs: u32,
    pub maximum_arity: u32,
    pub maximum_work: u32,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStandardContinuousRoleRequirementRow {
    pub role: NativeUtf8Slice,
    pub capabilities_start: u32,
    pub capabilities_len: u32,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStandardContinuousCapabilityRequirementRow {
    pub capability: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStandardContinuousInputRequirementRow {
    pub kind: NativeStandardContinuousInputKind,
    pub role: NativeUtf8Slice,
    pub input_id: NativeUtf8Slice,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStandardContinuousReadoutLease {
    pub handle: NativeStandardContinuousReadoutLeaseHandle,
    pub definitions: *const NativeStandardContinuousDefinitionReadoutRow,
    pub definitions_len: usize,
    pub roles: *const NativeStandardContinuousRoleRequirementRow,
    pub roles_len: usize,
    pub capabilities: *const NativeStandardContinuousCapabilityRequirementRow,
    pub capabilities_len: usize,
    pub inputs: *const NativeStandardContinuousInputRequirementRow,
    pub inputs_len: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStandardContinuousPredicateAdmitRequest {
    pub comparison: NativeStandardContinuousComparisonKind,
    pub nodes: *const NativeStandardContinuousNode,
    pub nodes_len: usize,
    pub child_indices: *const u32,
    pub child_indices_len: usize,
    pub left_node_index: u32,
    pub right_node_index: u32,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStandardContinuousPredicateReadoutRow {
    pub comparison: NativeStandardContinuousComparisonKind,
    pub maximum_depth: u32,
    pub maximum_nodes: u32,
    pub maximum_inputs: u32,
    pub maximum_arity: u32,
    pub maximum_work: u32,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStandardContinuousPredicateReadoutLease {
    pub handle: NativeStandardContinuousPredicateReadoutLeaseHandle,
    pub predicates: *const NativeStandardContinuousPredicateReadoutRow,
    pub predicates_len: usize,
    pub inputs: *const NativeStandardContinuousInputRequirementRow,
    pub inputs_len: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStandardContinuousEvaluateRequest {
    pub definition: NativeStandardContinuousDefinitionHandle,
    pub evidence: *const NativeStandardContinuousEvidence,
    pub evidence_len: usize,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStandardContinuousEvaluatePredicateRequest {
    pub predicate: NativeStandardContinuousPredicateHandle,
    pub evidence: *const NativeStandardContinuousEvidence,
    pub evidence_len: usize,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStandardContinuousEvaluationRow {
    pub value_bits: u64,
    pub work_used: u32,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStandardContinuousPredicateEvaluationRow {
    pub value: bool,
    pub work_used: u32,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStandardContinuousEvaluationLease {
    pub handle: NativeStandardContinuousEvaluationLeaseHandle,
    pub results: *const NativeStandardContinuousEvaluationRow,
    pub results_len: usize,
}
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStandardContinuousPredicateEvaluationLease {
    pub handle: NativeStandardContinuousPredicateEvaluationLeaseHandle,
    pub results: *const NativeStandardContinuousPredicateEvaluationRow,
    pub results_len: usize,
}
