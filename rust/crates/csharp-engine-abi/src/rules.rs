//! Immutable gameplay-rules package artifacts exposed to trusted NativeAOT code.
//!
//! This family admits the existing bounded package envelope and exposes its
//! owner-validated metadata. Resolution and selection remain named bounded
//! operations over explicitly retained packages; neither exposes payload
//! meaning or a package lookup registry.

use crate::{NativeByteSlice, NativeUtf8Slice};

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeRulesPackageHandle {
    pub value: u64,
}

/// A typed owner for one copied package inspection. Every pointer in the
/// matching lease remains valid only until `destroy_package_readout_lease`.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeRulesPackageReadoutLeaseHandle {
    pub value: u64,
}

/// A typed owner for one copied resolved package-set inspection.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeRulesResolvedPackageSetLeaseHandle {
    pub value: u64,
}

/// A typed owner for one copied selected payload subtree.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeRulesPayloadSelectionLeaseHandle {
    pub value: u64,
}

/// Borrowed encoded package bytes. Rust validates pointer/length coherence and
/// copies this immutable artifact before decoding or retaining it.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeRulesPackageAdmitRequest {
    pub encoded_package: NativeByteSlice,
}

/// Bounded borrowed package handles. The service copies package facts into its
/// result lease, so callers retain and release these input handles normally.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeRulesResolvePackagesRequest {
    pub packages: *const NativeRulesPackageHandle,
    pub packages_len: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeRulesPayloadPathSegmentKind {
    Field = 1,
    Index = 2,
}

/// One explicit payload traversal segment. `field` is used only for `Field`;
/// `index` is used only for `Index`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeRulesPayloadPathSegment {
    pub kind: NativeRulesPayloadPathSegmentKind,
    pub field: NativeUtf8Slice,
    pub index: u64,
}

/// Select one subtree from precisely one retained package. The expected
/// fingerprint makes stale parent selection an explicit typed failure.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeRulesSelectPayloadRequest {
    pub package: NativeRulesPackageHandle,
    pub expected_parent_fingerprint: NativeUtf8Slice,
    pub path: *const NativeRulesPayloadPathSegment,
    pub path_len: usize,
}

/// One copied, owner-admitted package metadata record. `canonical_bytes` are
/// canonical artifact transport only; the generated managed facade copies them
/// and does not interpret their opaque payload.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeRulesPackageReadoutRow {
    pub schema_version: u64,
    pub domain: NativeUtf8Slice,
    pub package: NativeUtf8Slice,
    pub version: u64,
    pub fingerprint: NativeUtf8Slice,
    pub canonical_bytes: NativeByteSlice,
    pub dependency_count: u32,
    pub source_count: u32,
    pub provenance_count: u32,
    pub json_node_count: u32,
    pub max_encoded_bytes: u32,
    pub max_dependencies: u32,
    pub max_sources: u32,
    pub max_provenance: u32,
    pub max_json_depth: u32,
    pub max_json_nodes: u32,
    pub max_json_string_bytes: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeRulesPackageDependencyRow {
    pub domain: NativeUtf8Slice,
    pub package: NativeUtf8Slice,
    pub version: u64,
    pub has_fingerprint: bool,
    pub fingerprint: NativeUtf8Slice,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeRulesPackageSourceRow {
    pub id: NativeUtf8Slice,
    pub path: NativeUtf8Slice,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeRulesPackageProvenanceRow {
    pub subject: NativeUtf8Slice,
    pub source: NativeUtf8Slice,
    pub has_line: bool,
    pub line: u64,
    pub has_column: bool,
    pub column: u64,
}

/// Exact bounded inspection of one retained package. The one `packages` row,
/// dependency rows, source rows, provenance rows, and all UTF-8/byte slices are
/// copied by generated bindings before `destroy_package_readout_lease`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeRulesPackageReadoutLease {
    pub handle: NativeRulesPackageReadoutLeaseHandle,
    pub packages: *const NativeRulesPackageReadoutRow,
    pub packages_len: usize,
    pub dependencies: *const NativeRulesPackageDependencyRow,
    pub dependencies_len: usize,
    pub sources: *const NativeRulesPackageSourceRow,
    pub sources_len: usize,
    pub provenance: *const NativeRulesPackageProvenanceRow,
    pub provenance_len: usize,
}

/// One package in the owner's deterministic dependency-first resolution
/// order. All fields are copied before the matching lease is released.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeRulesResolvedPackageRow {
    pub schema_version: u64,
    pub domain: NativeUtf8Slice,
    pub package: NativeUtf8Slice,
    pub version: u64,
    pub fingerprint: NativeUtf8Slice,
}

/// Aggregate facts measured by the rules owner while resolving this exact set.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeRulesResolvedPackageSetAggregate {
    pub canonical_bytes: u64,
    pub dependency_count: u64,
    pub source_count: u64,
    pub provenance_count: u64,
    pub json_node_count: u64,
}

/// Copied deterministic resolved package order plus aggregate owner facts.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeRulesResolvedPackageSetLease {
    pub handle: NativeRulesResolvedPackageSetLeaseHandle,
    pub packages: *const NativeRulesResolvedPackageRow,
    pub packages_len: usize,
    pub aggregate: NativeRulesResolvedPackageSetAggregate,
}

/// Copied parent proof and canonical bytes for the explicitly selected
/// subtree. The field/index path itself remains request-only input.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeRulesPayloadSelectionRow {
    pub parent_schema_version: u64,
    pub parent_domain: NativeUtf8Slice,
    pub parent_package: NativeUtf8Slice,
    pub parent_version: u64,
    pub parent_fingerprint: NativeUtf8Slice,
    pub canonical_bytes: NativeByteSlice,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeRulesPayloadSelectionLease {
    pub handle: NativeRulesPayloadSelectionLeaseHandle,
    pub selections: *const NativeRulesPayloadSelectionRow,
    pub selections_len: usize,
}
