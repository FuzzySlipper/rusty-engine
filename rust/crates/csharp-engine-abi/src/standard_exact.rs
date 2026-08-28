//! Typed Standard Exact definition and evaluation ABI.
//!
//! The input tree is deliberately a flat, post-order table.  C# supplies
//! authored facts, while `gameplay-standard` remains the only evaluator and
//! canonical package/admission owner.

use crate::NativeUtf8Slice;

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeStandardExactNodeKind {
    #[default]
    Literal = 0,
    Input = 1,
    Add = 2,
    Subtract = 3,
    Multiply = 4,
    FloorDivide = 5,
    TruncatingDivide = 6,
    FixedPower = 7,
    Min = 8,
    Max = 9,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeStandardExactInputKind {
    #[default]
    Parameter = 0,
    Fact = 1,
    Roll = 2,
    BoundedRoll = 3,
    Choice = 4,
    StandardStat = 5,
    StandardTrackCurrent = 6,
    StandardTrackMaximum = 7,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NativeStandardExactComparisonKind {
    #[default]
    Equal = 0,
    LessThan = 1,
    LessOrEqual = 2,
    GreaterThan = 3,
    GreaterOrEqual = 4,
}

/// One flat, post-order authored expression node. For binary and fixed-power
/// nodes `left` and `right` must identify earlier nodes. For min/max,
/// `children_start..children_start + children_len` identifies entries in the
/// request's explicit `child_indices` span, and every selected node must be
/// earlier than this aggregate node.
/// Input nodes use `input_kind`, `role`, and `input_id`; a bounded roll also
/// uses its inclusive `minimum` and `maximum`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStandardExactNode {
    pub kind: NativeStandardExactNodeKind,
    pub literal: i64,
    pub input_kind: NativeStandardExactInputKind,
    pub role: NativeUtf8Slice,
    pub input_id: NativeUtf8Slice,
    pub minimum: i64,
    pub maximum: i64,
    pub left: u32,
    pub right: u32,
    pub children_start: u32,
    pub children_len: u32,
    pub fixed_power_scale: i64,
}

/// One declared role and its explicit range in the request capability span.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStandardExactRole {
    pub role: NativeUtf8Slice,
    pub capabilities_start: u32,
    pub capabilities_len: u32,
}

/// One capability identity in a role's explicit capability range.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStandardExactCapability {
    pub capability: NativeUtf8Slice,
}

/// A scalar observation for one exact input. The descriptor fields must match
/// the admitted exact input requirement; bounded-roll values are additionally
/// checked by the canonical evaluator.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStandardExactEvidence {
    pub kind: NativeStandardExactInputKind,
    pub role: NativeUtf8Slice,
    pub input_id: NativeUtf8Slice,
    pub minimum: i64,
    pub maximum: i64,
    pub value: i64,
}

/// Authored exact definition and its one-source IntegerOnlyV1 package context.
/// All strings and rows are borrowed only for this direct call; the Engine
/// parses and copies them before retaining the admitted definition.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStandardExactAdmitRequest {
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
    pub roles: *const NativeStandardExactRole,
    pub roles_len: usize,
    pub capabilities: *const NativeStandardExactCapability,
    pub capabilities_len: usize,
    pub nodes: *const NativeStandardExactNode,
    pub nodes_len: usize,
    pub child_indices: *const u32,
    pub child_indices_len: usize,
    pub root_node_index: u32,
}

/// Typed retained canonical exact definition owner.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeStandardExactDefinitionHandle {
    pub value: u64,
}

/// Typed retained exact comparison/predicate owner. Unlike an admitted exact
/// definition this is a direct canonical expression comparison, so it retains
/// no synthetic package artifact.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeStandardExactPredicateHandle {
    pub value: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeStandardExactReadoutLeaseHandle {
    pub value: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeStandardExactEvaluationLeaseHandle {
    pub value: u64,
}

/// One copied immutable definition identity and public owner quota policy.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStandardExactDefinitionReadoutRow {
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
pub struct NativeStandardExactRoleRequirementRow {
    pub role: NativeUtf8Slice,
    pub capabilities_start: u32,
    pub capabilities_len: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStandardExactCapabilityRequirementRow {
    pub capability: NativeUtf8Slice,
}

/// Canonical input requirement copied from the admitted exact definition.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStandardExactInputRequirementRow {
    pub kind: NativeStandardExactInputKind,
    pub role: NativeUtf8Slice,
    pub input_id: NativeUtf8Slice,
    pub minimum: i64,
    pub maximum: i64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStandardExactReadoutLease {
    pub handle: NativeStandardExactReadoutLeaseHandle,
    pub definitions: *const NativeStandardExactDefinitionReadoutRow,
    pub definitions_len: usize,
    pub roles: *const NativeStandardExactRoleRequirementRow,
    pub roles_len: usize,
    pub capabilities: *const NativeStandardExactCapabilityRequirementRow,
    pub capabilities_len: usize,
    pub inputs: *const NativeStandardExactInputRequirementRow,
    pub inputs_len: usize,
}

/// A second flat post-order exact tree for one predicate. The two root indices
/// identify the left/right operands and share one explicit child-index span.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStandardExactPredicateAdmitRequest {
    pub comparison: NativeStandardExactComparisonKind,
    pub nodes: *const NativeStandardExactNode,
    pub nodes_len: usize,
    pub child_indices: *const u32,
    pub child_indices_len: usize,
    pub left_node_index: u32,
    pub right_node_index: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStandardExactPredicateReadoutRow {
    pub comparison: NativeStandardExactComparisonKind,
    pub maximum_depth: u32,
    pub maximum_nodes: u32,
    pub maximum_inputs: u32,
    pub maximum_arity: u32,
    pub maximum_work: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeStandardExactPredicateReadoutLeaseHandle {
    pub value: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStandardExactPredicateReadoutLease {
    pub handle: NativeStandardExactPredicateReadoutLeaseHandle,
    pub predicates: *const NativeStandardExactPredicateReadoutRow,
    pub predicates_len: usize,
    pub inputs: *const NativeStandardExactInputRequirementRow,
    pub inputs_len: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStandardExactEvaluateRequest {
    pub definition: NativeStandardExactDefinitionHandle,
    pub evidence: *const NativeStandardExactEvidence,
    pub evidence_len: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStandardExactEvaluatePredicateRequest {
    pub predicate: NativeStandardExactPredicateHandle,
    pub evidence: *const NativeStandardExactEvidence,
    pub evidence_len: usize,
}

/// One exact scalar result and the deterministic work the canonical
/// gameplay-standard evaluator actually consumed.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStandardExactEvaluationRow {
    pub value: i64,
    pub work_used: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStandardExactEvaluationLease {
    pub handle: NativeStandardExactEvaluationLeaseHandle,
    pub results: *const NativeStandardExactEvaluationRow,
    pub results_len: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStandardExactPredicateEvaluationRow {
    pub value: bool,
    pub work_used: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeStandardExactPredicateEvaluationLeaseHandle {
    pub value: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStandardExactPredicateEvaluationLease {
    pub handle: NativeStandardExactPredicateEvaluationLeaseHandle,
    pub results: *const NativeStandardExactPredicateEvaluationRow,
    pub results_len: usize,
}
