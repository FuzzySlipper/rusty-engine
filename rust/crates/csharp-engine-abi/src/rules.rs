//! Immutable gameplay-rules package artifacts exposed to trusted NativeAOT code.
//!
//! This family admits the existing bounded package envelope and exposes its
//! owner-validated metadata. It deliberately does not expose payload meaning,
//! package resolution, or payload selection.

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

/// Borrowed encoded package bytes. Rust validates pointer/length coherence and
/// copies this immutable artifact before decoding or retaining it.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeRulesPackageAdmitRequest {
    pub encoded_package: NativeByteSlice,
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
