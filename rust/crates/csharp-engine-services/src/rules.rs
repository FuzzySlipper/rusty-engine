use std::{collections::BTreeMap, ffi::c_void};

use csharp_engine_abi::*;
use gameplay_rules::{
    decode_rule_package, resolve_rule_packages, select_rule_payload_subtree, AdmittedRulePackage,
    RuleFingerprint, RulePackageError, RulePackageSetError, RulePayloadPath,
    RulePayloadPathSegment, RuleSubtreeSelectionError, MAX_DEPENDENCIES_PER_RULE_PACKAGE,
    MAX_ENCODED_RULE_PACKAGE_BYTES, MAX_JSON_NESTING_DEPTH, MAX_JSON_NODES_PER_RULE_PACKAGE,
    MAX_JSON_STRING_BYTES, MAX_PROVENANCE_PER_RULE_PACKAGE, MAX_SOURCES_PER_RULE_PACKAGE,
};

use crate::{
    composition::ABI_OK,
    composition::{borrowed_slice, borrowed_utf8},
};

const SERVICE: &[u8] = b"Rules";
const ADMIT_OPERATION: &[u8] = b"AdmitPackage";
const RESOLVE_OPERATION: &[u8] = b"ResolvePackages";
const SELECT_OPERATION: &[u8] = b"SelectPayload";
const MAX_DIAGNOSTIC_SOURCE_BYTES: usize = 512;

pub(crate) struct RuntimeRulesBridge {
    packages: BTreeMap<u64, AdmittedRulePackage>,
    next_package: u64,
    readout_leases: BTreeMap<u64, RulesPackageReadoutBacking>,
    next_readout_lease: u64,
    resolved_package_set_leases: BTreeMap<u64, RulesResolvedPackageSetBacking>,
    next_resolved_package_set_lease: u64,
    payload_selection_leases: BTreeMap<u64, RulesPayloadSelectionBacking>,
    next_payload_selection_lease: u64,
    diagnostic_leases: BTreeMap<u64, RulesDiagnosticLease>,
    next_diagnostic_lease: u64,
}

struct RulesPackageReadoutBacking {
    _text: Vec<String>,
    _canonical_bytes: Vec<u8>,
    packages: Vec<NativeRulesPackageReadoutRow>,
    dependencies: Vec<NativeRulesPackageDependencyRow>,
    sources: Vec<NativeRulesPackageSourceRow>,
    provenance: Vec<NativeRulesPackageProvenanceRow>,
}

struct RulesResolvedPackageSetBacking {
    _text: Vec<String>,
    packages: Vec<NativeRulesResolvedPackageRow>,
}

struct RulesPayloadSelectionBacking {
    _text: Vec<String>,
    _canonical_bytes: Vec<u8>,
    selections: Vec<NativeRulesPayloadSelectionRow>,
}

struct RulesDiagnosticLease {
    _values: Vec<RulesDiagnosticValue>,
    readout: Vec<NativeEngineDiagnostic>,
}

struct RulesDiagnosticValue {
    code: String,
    message: String,
    source: String,
}

#[derive(Debug)]
enum RulesOperationError {
    UnknownPackageHandle { value: u64 },
    ResolvedPackageSet(RulePackageSetError),
    PayloadSelection(RuleSubtreeSelectionError),
    LeaseExhausted { field: &'static str },
}

struct ReadoutText {
    values: Vec<String>,
}

impl ReadoutText {
    fn copy(&mut self, value: &str) -> NativeUtf8Slice {
        self.values.push(value.to_owned());
        let value = self.values.last().expect("pushed text");
        NativeUtf8Slice {
            bytes: value.as_ptr(),
            len: value.len(),
        }
    }
}

impl RuntimeRulesBridge {
    pub(crate) fn new() -> Self {
        Self {
            packages: BTreeMap::new(),
            next_package: 1,
            readout_leases: BTreeMap::new(),
            next_readout_lease: 1,
            resolved_package_set_leases: BTreeMap::new(),
            next_resolved_package_set_lease: 1,
            payload_selection_leases: BTreeMap::new(),
            next_payload_selection_lease: 1,
            diagnostic_leases: BTreeMap::new(),
            next_diagnostic_lease: 1,
        }
    }

    fn admit(&mut self, encoded: &[u8]) -> Result<NativeRulesPackageHandle, RulePackageError> {
        let package = decode_rule_package(encoded)?;
        let value = self.next_package;
        self.next_package = value
            .checked_add(1)
            .ok_or(RulePackageError::ArithmeticOverflow {
                path: "rules.packageHandle".to_owned(),
            })?;
        self.packages.insert(value, package);
        Ok(NativeRulesPackageHandle { value })
    }

    fn destroy(&mut self, handle: NativeRulesPackageHandle) -> bool {
        handle.value != 0 && self.packages.remove(&handle.value).is_some()
    }

    fn read(&mut self, handle: NativeRulesPackageHandle) -> Option<NativeRulesPackageReadoutLease> {
        let package = self.packages.get(&handle.value)?;
        let lease_value = self.next_readout_lease;
        self.next_readout_lease = lease_value.checked_add(1)?;
        let mut text = ReadoutText { values: Vec::new() };
        let canonical_bytes = package.canonical_bytes().to_vec();
        let dependencies = package
            .dependencies()
            .iter()
            .map(|dependency| NativeRulesPackageDependencyRow {
                domain: text.copy(dependency.domain().as_str()),
                package: text.copy(dependency.package().as_str()),
                version: dependency.version().get(),
                has_fingerprint: dependency.fingerprint().is_some(),
                fingerprint: text.copy(
                    dependency
                        .fingerprint()
                        .map_or("", |fingerprint| fingerprint.as_str()),
                ),
            })
            .collect();
        let sources = package
            .sources()
            .iter()
            .map(|source| NativeRulesPackageSourceRow {
                id: text.copy(source.id().as_str()),
                path: text.copy(source.path()),
            })
            .collect();
        let provenance = package
            .provenance()
            .iter()
            .map(|provenance| NativeRulesPackageProvenanceRow {
                subject: text.copy(provenance.subject().as_str()),
                source: text.copy(provenance.source().as_str()),
                has_line: provenance.line().is_some(),
                line: provenance.line().unwrap_or_default(),
                has_column: provenance.column().is_some(),
                column: provenance.column().unwrap_or_default(),
            })
            .collect();
        let package_row = NativeRulesPackageReadoutRow {
            schema_version: package.schema_version().get(),
            domain: text.copy(package.identity().domain().as_str()),
            package: text.copy(package.identity().package().as_str()),
            version: package.identity().version().get(),
            fingerprint: text.copy(package.fingerprint().as_str()),
            canonical_bytes: NativeByteSlice {
                bytes: canonical_bytes.as_ptr(),
                len: canonical_bytes.len(),
            },
            dependency_count: narrow(package.dependencies().len())?,
            source_count: narrow(package.sources().len())?,
            provenance_count: narrow(package.provenance().len())?,
            json_node_count: narrow(package.json_nodes())?,
            max_encoded_bytes: narrow(MAX_ENCODED_RULE_PACKAGE_BYTES)?,
            max_dependencies: narrow(MAX_DEPENDENCIES_PER_RULE_PACKAGE)?,
            max_sources: narrow(MAX_SOURCES_PER_RULE_PACKAGE)?,
            max_provenance: narrow(MAX_PROVENANCE_PER_RULE_PACKAGE)?,
            max_json_depth: narrow(MAX_JSON_NESTING_DEPTH)?,
            max_json_nodes: narrow(MAX_JSON_NODES_PER_RULE_PACKAGE)?,
            max_json_string_bytes: narrow(MAX_JSON_STRING_BYTES)?,
        };
        let backing = RulesPackageReadoutBacking {
            _text: text.values,
            _canonical_bytes: canonical_bytes,
            packages: vec![package_row],
            dependencies,
            sources,
            provenance,
        };
        let lease = NativeRulesPackageReadoutLease {
            handle: NativeRulesPackageReadoutLeaseHandle { value: lease_value },
            packages: backing.packages.as_ptr(),
            packages_len: backing.packages.len(),
            dependencies: backing.dependencies.as_ptr(),
            dependencies_len: backing.dependencies.len(),
            sources: backing.sources.as_ptr(),
            sources_len: backing.sources.len(),
            provenance: backing.provenance.as_ptr(),
            provenance_len: backing.provenance.len(),
        };
        self.readout_leases.insert(lease_value, backing);
        Some(lease)
    }

    fn destroy_readout_lease(&mut self, handle: NativeRulesPackageReadoutLeaseHandle) -> bool {
        handle.value != 0 && self.readout_leases.remove(&handle.value).is_some()
    }

    fn resolve(
        &mut self,
        handles: &[NativeRulesPackageHandle],
    ) -> Result<NativeRulesResolvedPackageSetLease, RulesOperationError> {
        let packages = handles
            .iter()
            .map(|handle| {
                self.packages.get(&handle.value).cloned().ok_or(
                    RulesOperationError::UnknownPackageHandle {
                        value: handle.value,
                    },
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let resolved =
            resolve_rule_packages(packages).map_err(RulesOperationError::ResolvedPackageSet)?;
        let lease_value = self.next_resolved_package_set_lease;
        self.next_resolved_package_set_lease =
            lease_value
                .checked_add(1)
                .ok_or(RulesOperationError::LeaseExhausted {
                    field: "rules.resolvedPackageSetLease",
                })?;
        let mut text = ReadoutText { values: Vec::new() };
        let packages = resolved
            .packages()
            .iter()
            .map(|package| NativeRulesResolvedPackageRow {
                schema_version: package.schema_version().get(),
                domain: text.copy(package.identity().domain().as_str()),
                package: text.copy(package.identity().package().as_str()),
                version: package.identity().version().get(),
                fingerprint: text.copy(package.fingerprint().as_str()),
            })
            .collect::<Vec<_>>();
        let aggregate = NativeRulesResolvedPackageSetAggregate {
            canonical_bytes: u64::try_from(resolved.canonical_bytes()).expect("usize fits u64"),
            dependency_count: u64::try_from(resolved.dependency_count()).expect("usize fits u64"),
            source_count: u64::try_from(resolved.source_count()).expect("usize fits u64"),
            provenance_count: u64::try_from(resolved.provenance_count()).expect("usize fits u64"),
            json_node_count: u64::try_from(resolved.json_nodes()).expect("usize fits u64"),
        };
        let backing = RulesResolvedPackageSetBacking {
            _text: text.values,
            packages,
        };
        let lease = NativeRulesResolvedPackageSetLease {
            handle: NativeRulesResolvedPackageSetLeaseHandle { value: lease_value },
            packages: backing.packages.as_ptr(),
            packages_len: backing.packages.len(),
            aggregate,
        };
        self.resolved_package_set_leases
            .insert(lease_value, backing);
        Ok(lease)
    }

    fn destroy_resolved_package_set_lease(
        &mut self,
        handle: NativeRulesResolvedPackageSetLeaseHandle,
    ) -> bool {
        handle.value != 0
            && self
                .resolved_package_set_leases
                .remove(&handle.value)
                .is_some()
    }

    fn select_payload(
        &mut self,
        package_handle: NativeRulesPackageHandle,
        expected_parent_fingerprint: RuleFingerprint,
        path: RulePayloadPath,
    ) -> Result<NativeRulesPayloadSelectionLease, RulesOperationError> {
        let package = self.packages.get(&package_handle.value).ok_or(
            RulesOperationError::UnknownPackageHandle {
                value: package_handle.value,
            },
        )?;
        let selected = select_rule_payload_subtree(package, &expected_parent_fingerprint, path)
            .map_err(RulesOperationError::PayloadSelection)?;
        let lease_value = self.next_payload_selection_lease;
        self.next_payload_selection_lease =
            lease_value
                .checked_add(1)
                .ok_or(RulesOperationError::LeaseExhausted {
                    field: "rules.payloadSelectionLease",
                })?;
        let mut text = ReadoutText { values: Vec::new() };
        let canonical_bytes = selected.canonical_bytes().to_vec();
        let selection = NativeRulesPayloadSelectionRow {
            parent_schema_version: selected.parent_schema_version().get(),
            parent_domain: text.copy(selected.parent_identity().domain().as_str()),
            parent_package: text.copy(selected.parent_identity().package().as_str()),
            parent_version: selected.parent_identity().version().get(),
            parent_fingerprint: text.copy(selected.parent_fingerprint().as_str()),
            canonical_bytes: NativeByteSlice {
                bytes: canonical_bytes.as_ptr(),
                len: canonical_bytes.len(),
            },
        };
        let backing = RulesPayloadSelectionBacking {
            _text: text.values,
            _canonical_bytes: canonical_bytes,
            selections: vec![selection],
        };
        let lease = NativeRulesPayloadSelectionLease {
            handle: NativeRulesPayloadSelectionLeaseHandle { value: lease_value },
            selections: backing.selections.as_ptr(),
            selections_len: backing.selections.len(),
        };
        self.payload_selection_leases.insert(lease_value, backing);
        Ok(lease)
    }

    fn destroy_payload_selection_lease(
        &mut self,
        handle: NativeRulesPayloadSelectionLeaseHandle,
    ) -> bool {
        handle.value != 0
            && self
                .payload_selection_leases
                .remove(&handle.value)
                .is_some()
    }

    fn retain_diagnostic(
        &mut self,
        error: &RulePackageError,
    ) -> Option<NativeEngineDiagnosticLease> {
        let handle = self.next_diagnostic_lease;
        self.next_diagnostic_lease = handle.checked_add(1)?;
        let value = RulesDiagnosticValue::from_package_error(error);
        let lease = RulesDiagnosticLease::new(value);
        let native = NativeEngineDiagnosticLease {
            handle: NativeEngineDiagnosticLeaseHandle { value: handle },
            diagnostics: lease.readout.as_ptr(),
            diagnostics_len: lease.readout.len(),
        };
        self.diagnostic_leases.insert(handle, lease);
        Some(native)
    }

    fn retain_operation_diagnostic(
        &mut self,
        error: &RulesOperationError,
    ) -> Option<NativeEngineDiagnosticLease> {
        let handle = self.next_diagnostic_lease;
        self.next_diagnostic_lease = handle.checked_add(1)?;
        let value = RulesDiagnosticValue::from_operation_error(error);
        let lease = RulesDiagnosticLease::new(value);
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

impl RulesDiagnosticLease {
    fn new(value: RulesDiagnosticValue) -> Self {
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

impl RulesDiagnosticValue {
    fn fixed(code: &'static str, message: &'static str, source: &str) -> Self {
        Self {
            code: code.to_owned(),
            message: message.to_owned(),
            source: bounded_source(source),
        }
    }

    fn from_operation_error(error: &RulesOperationError) -> Self {
        match error {
            RulesOperationError::UnknownPackageHandle { value } => Self::fixed(
                "RULE_PACKAGE_HANDLE",
                "Rules operation received an unknown retained package handle.",
                &value.to_string(),
            ),
            RulesOperationError::ResolvedPackageSet(error) => Self::from_package_set_error(error),
            RulesOperationError::PayloadSelection(error) => {
                Self::from_payload_selection_error(error)
            }
            RulesOperationError::LeaseExhausted { field } => Self::fixed(
                "RULE_PACKAGE_LEASE_ARITHMETIC",
                "Rules result lease allocation overflowed.",
                field,
            ),
        }
    }

    fn from_package_set_error(error: &RulePackageSetError) -> Self {
        match error {
            RulePackageSetError::AggregateQuotaExceeded { field, .. } => Self::fixed(
                "RULE_PACKAGE_SET_QUOTA",
                "Resolved package set exceeded an aggregate owner quota.",
                field,
            ),
            RulePackageSetError::ArithmeticOverflow { field } => Self::fixed(
                "RULE_PACKAGE_SET_ARITHMETIC",
                "Resolved package-set arithmetic overflowed.",
                field,
            ),
            RulePackageSetError::DuplicatePackage { package } => Self::fixed(
                "RULE_PACKAGE_SET_DUPLICATE",
                "Resolved package set repeated an exact package identity.",
                &package.to_string(),
            ),
            RulePackageSetError::ConflictingVersions {
                domain, package, ..
            } => Self::fixed(
                "RULE_PACKAGE_SET_CONFLICTING_VERSION",
                "Resolved package set provided conflicting versions for one logical package.",
                &format!("{domain}/{package}"),
            ),
            RulePackageSetError::MissingDependency { package, .. } => Self::fixed(
                "RULE_PACKAGE_SET_MISSING_DEPENDENCY",
                "Resolved package set omitted a declared dependency.",
                &package.to_string(),
            ),
            RulePackageSetError::DependencyVersionMismatch { package, .. } => Self::fixed(
                "RULE_PACKAGE_SET_DEPENDENCY_VERSION",
                "Resolved package set supplied a dependency at the wrong version.",
                &package.to_string(),
            ),
            RulePackageSetError::DependencyFingerprintMismatch { package, .. } => Self::fixed(
                "RULE_PACKAGE_SET_DEPENDENCY_FINGERPRINT",
                "Resolved package set supplied a dependency with the wrong fingerprint.",
                &package.to_string(),
            ),
            RulePackageSetError::DependencyCycle { packages } => Self::fixed(
                "RULE_PACKAGE_SET_DEPENDENCY_CYCLE",
                "Resolved package set contains a dependency cycle.",
                &packages
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(" -> "),
            ),
        }
    }

    fn from_payload_selection_error(error: &RuleSubtreeSelectionError) -> Self {
        match error {
            RuleSubtreeSelectionError::Path(error) => match error {
                gameplay_rules::RulePayloadPathError::Empty => Self::fixed(
                    "RULE_PAYLOAD_PATH_EMPTY",
                    "Payload selection requires at least one path segment.",
                    "path",
                ),
                gameplay_rules::RulePayloadPathError::TooManySegments { .. } => Self::fixed(
                    "RULE_PAYLOAD_PATH_SEGMENT_QUOTA",
                    "Payload selection exceeded its path segment quota.",
                    "path",
                ),
                gameplay_rules::RulePayloadPathError::InvalidField { field } => Self::fixed(
                    "RULE_PAYLOAD_PATH_FIELD",
                    "Payload selection field segment was invalid.",
                    field,
                ),
                gameplay_rules::RulePayloadPathError::IndexTooLarge { .. } => Self::fixed(
                    "RULE_PAYLOAD_PATH_INDEX",
                    "Payload selection index exceeded its owner bound.",
                    "path",
                ),
                gameplay_rules::RulePayloadPathError::DisplayTooLong { .. } => Self::fixed(
                    "RULE_PAYLOAD_PATH_DISPLAY_QUOTA",
                    "Payload selection path display exceeded its owner bound.",
                    "path",
                ),
            },
            RuleSubtreeSelectionError::ParentFingerprintMismatch { actual, .. } => Self::fixed(
                "RULE_PAYLOAD_PARENT_FINGERPRINT",
                "Payload selection expected a different parent fingerprint.",
                actual.as_str(),
            ),
            RuleSubtreeSelectionError::MissingField { path } => Self::fixed(
                "RULE_PAYLOAD_MISSING_FIELD",
                "Payload selection field was absent from the parent package.",
                path,
            ),
            RuleSubtreeSelectionError::IndexOutOfBounds { path, .. } => Self::fixed(
                "RULE_PAYLOAD_INDEX_OUT_OF_BOUNDS",
                "Payload selection index was outside the selected array.",
                path,
            ),
            RuleSubtreeSelectionError::ExpectedObject { path } => Self::fixed(
                "RULE_PAYLOAD_EXPECTED_OBJECT",
                "Payload selection field segment required an object.",
                path,
            ),
            RuleSubtreeSelectionError::ExpectedArray { path } => Self::fixed(
                "RULE_PAYLOAD_EXPECTED_ARRAY",
                "Payload selection index segment required an array.",
                path,
            ),
            RuleSubtreeSelectionError::Canonical(error) => Self::from_package_error(error),
        }
    }

    fn from_package_error(error: &RulePackageError) -> Self {
        let (code, message, source) = match error {
            RulePackageError::ArtifactQuotaExceeded { .. } => (
                "RULE_PACKAGE_ARTIFACT_QUOTA",
                "Encoded package exceeded its byte quota.",
                "",
            ),
            RulePackageError::MalformedUtf8 { .. } => (
                "RULE_PACKAGE_MALFORMED_UTF8",
                "Encoded package was not valid UTF-8.",
                "",
            ),
            RulePackageError::MalformedJson { path, .. } => (
                "RULE_PACKAGE_MALFORMED_JSON",
                "Encoded package contained malformed JSON.",
                path.as_str(),
            ),
            RulePackageError::DuplicateJsonKey { path, .. } => (
                "RULE_PACKAGE_DUPLICATE_JSON_KEY",
                "Encoded package contained a duplicate JSON key.",
                path.as_str(),
            ),
            RulePackageError::WrongArtifactKind { .. } => (
                "RULE_PACKAGE_KIND",
                "Encoded artifact was not a gameplay-rules package.",
                "",
            ),
            RulePackageError::UnsupportedSchemaVersion { .. } => (
                "RULE_PACKAGE_SCHEMA",
                "Encoded package used an unsupported schema version.",
                "",
            ),
            RulePackageError::MissingField { path } => (
                "RULE_PACKAGE_MISSING_FIELD",
                "Encoded package omitted a required field.",
                path.as_str(),
            ),
            RulePackageError::UnknownField { path } => (
                "RULE_PACKAGE_UNKNOWN_FIELD",
                "Encoded package contained an unknown field.",
                path.as_str(),
            ),
            RulePackageError::InvalidFieldType { path, .. } => (
                "RULE_PACKAGE_FIELD_TYPE",
                "Encoded package field had an invalid type.",
                path.as_str(),
            ),
            RulePackageError::InvalidIdentity { path, .. } => (
                "RULE_PACKAGE_IDENTITY",
                "Encoded package identity was invalid.",
                path.as_str(),
            ),
            RulePackageError::InvalidVersion { path, .. } => (
                "RULE_PACKAGE_VERSION",
                "Encoded package version was invalid.",
                path.as_str(),
            ),
            RulePackageError::InvalidSourcePath { path, .. } => (
                "RULE_PACKAGE_SOURCE_PATH",
                "Encoded package source path was invalid.",
                path.as_str(),
            ),
            RulePackageError::InvalidSourceLocation { path, .. } => (
                "RULE_PACKAGE_SOURCE_LOCATION",
                "Encoded package source location was invalid.",
                path.as_str(),
            ),
            RulePackageError::InvalidFingerprint { path, .. } => (
                "RULE_PACKAGE_FINGERPRINT",
                "Encoded package fingerprint was invalid.",
                path.as_str(),
            ),
            RulePackageError::JsonIntegerOutOfRange { path, .. } => (
                "RULE_PACKAGE_INTEGER_RANGE",
                "Encoded package integer exceeded the supported range.",
                path.as_str(),
            ),
            RulePackageError::JsonNumberOutOfRange { path, .. } => (
                "RULE_PACKAGE_NUMBER_RANGE",
                "Encoded package number exceeded the supported range.",
                path.as_str(),
            ),
            RulePackageError::QuotaExceeded { path, .. } => (
                "RULE_PACKAGE_QUOTA",
                "Encoded package exceeded a bounded collection quota.",
                path.as_str(),
            ),
            RulePackageError::JsonDepthExceeded { path, .. } => (
                "RULE_PACKAGE_JSON_DEPTH",
                "Encoded package exceeded JSON depth.",
                path.as_str(),
            ),
            RulePackageError::JsonNodeQuotaExceeded { path, .. } => (
                "RULE_PACKAGE_JSON_NODES",
                "Encoded package exceeded JSON node quota.",
                path.as_str(),
            ),
            RulePackageError::DuplicateDependency { .. } => (
                "RULE_PACKAGE_DUPLICATE_DEPENDENCY",
                "Encoded package repeated a dependency.",
                "dependencies",
            ),
            RulePackageError::DuplicateSource { .. } => (
                "RULE_PACKAGE_DUPLICATE_SOURCE",
                "Encoded package repeated a source.",
                "sources",
            ),
            RulePackageError::DuplicateProvenance { .. } => (
                "RULE_PACKAGE_DUPLICATE_PROVENANCE",
                "Encoded package repeated provenance.",
                "provenance",
            ),
            RulePackageError::UnknownProvenanceSource { .. } => (
                "RULE_PACKAGE_PROVENANCE_SOURCE",
                "Encoded package provenance referenced an unknown source.",
                "provenance",
            ),
            RulePackageError::SelfDependency { .. } => (
                "RULE_PACKAGE_SELF_DEPENDENCY",
                "Encoded package depended on itself.",
                "dependencies",
            ),
            RulePackageError::NonCanonicalArtifact { .. } => (
                "RULE_PACKAGE_NON_CANONICAL",
                "Encoded package was not canonical.",
                "",
            ),
            RulePackageError::ArithmeticOverflow { path } => (
                "RULE_PACKAGE_ARITHMETIC",
                "Package admission arithmetic overflowed.",
                path.as_str(),
            ),
        };
        Self {
            code: code.to_owned(),
            message: message.to_owned(),
            source: bounded_source(source),
        }
    }
}

pub(crate) fn api(bridge: &mut RuntimeRulesBridge) -> NativeRulesApi {
    NativeRulesApi {
        context: (bridge as *mut RuntimeRulesBridge).cast(),
        admit_package,
        destroy_package,
        read_package,
        destroy_package_readout_lease,
        resolve_packages,
        destroy_resolved_package_set_lease,
        select_payload,
        destroy_payload_selection_lease,
        destroy_operation_diagnostic_lease,
    }
}

unsafe extern "C" fn admit_package(
    context: *mut c_void,
    request: *const NativeRulesPackageAdmitRequest,
    result: *mut NativeRulesPackageHandle,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    if receipt.is_null() {
        return 0;
    }
    // SAFETY: receipt is valid for this direct call and is initialized on all
    // observable paths before a retained diagnostic can be returned.
    unsafe { *receipt = std::mem::zeroed() };
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    // SAFETY: product borrowing ends at callback return; admission copies bytes.
    let request = unsafe { *request };
    let encoded = match unsafe {
        borrowed_slice(
            request.encoded_package.bytes,
            request.encoded_package.len,
            "rules encoded package",
        )
    } {
        Ok(value) => value,
        Err(_) => return 0,
    };
    // SAFETY: context is the stable Rules bridge retained for the product lifetime.
    let bridge = unsafe { &mut *context.cast::<RuntimeRulesBridge>() };
    match bridge.admit(encoded) {
        Ok(handle) => {
            // SAFETY: result was checked and belongs to this direct call.
            unsafe { *result = handle };
            ABI_OK
        }
        Err(error) => {
            if let Some(diagnostics) = bridge.retain_diagnostic(&error) {
                // SAFETY: receipt was checked and names the retained lease exactly.
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

unsafe extern "C" fn destroy_package(
    context: *mut c_void,
    handle: NativeRulesPackageHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    // SAFETY: context remains valid for the product lifetime.
    let bridge = unsafe { &mut *context.cast::<RuntimeRulesBridge>() };
    i32::from(bridge.destroy(handle))
}

unsafe extern "C" fn read_package(
    context: *mut c_void,
    handle: NativeRulesPackageHandle,
    result: *mut NativeRulesPackageReadoutLease,
) -> i32 {
    if context.is_null() || result.is_null() {
        return 0;
    }
    // SAFETY: context and result are valid for this direct call.
    let bridge = unsafe { &mut *context.cast::<RuntimeRulesBridge>() };
    match bridge.read(handle) {
        Some(lease) => {
            // SAFETY: result was checked above.
            unsafe { *result = lease };
            ABI_OK
        }
        None => 0,
    }
}

unsafe extern "C" fn destroy_package_readout_lease(
    context: *mut c_void,
    handle: NativeRulesPackageReadoutLeaseHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    // SAFETY: context remains valid for the product lifetime.
    let bridge = unsafe { &mut *context.cast::<RuntimeRulesBridge>() };
    i32::from(bridge.destroy_readout_lease(handle))
}

unsafe extern "C" fn resolve_packages(
    context: *mut c_void,
    request: *const NativeRulesResolvePackagesRequest,
    result: *mut NativeRulesResolvedPackageSetLease,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    if receipt.is_null() {
        return 0;
    }
    // SAFETY: receipt is valid for this direct call and starts without a lease.
    unsafe { *receipt = std::mem::zeroed() };
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    // SAFETY: request and its handle span are borrowed for this direct call.
    let request = unsafe { *request };
    let handles = match unsafe {
        borrowed_slice(
            request.packages,
            request.packages_len,
            "rules package handles",
        )
    } {
        Ok(value) => value,
        Err(_) => return 0,
    };
    // SAFETY: context is the stable Rules bridge retained for the product lifetime.
    let bridge = unsafe { &mut *context.cast::<RuntimeRulesBridge>() };
    match bridge.resolve(handles) {
        Ok(lease) => {
            // SAFETY: result was checked and belongs to this direct call.
            unsafe { *result = lease };
            ABI_OK
        }
        Err(error) => {
            if let Some(diagnostics) = bridge.retain_operation_diagnostic(&error) {
                // SAFETY: receipt was checked and names the retained lease exactly.
                unsafe {
                    *receipt = NativeOperationErrorReceipt {
                        service: native_utf8(SERVICE),
                        operation: native_utf8(RESOLVE_OPERATION),
                        status: 0,
                        diagnostics,
                    };
                }
            }
            0
        }
    }
}

unsafe extern "C" fn destroy_resolved_package_set_lease(
    context: *mut c_void,
    handle: NativeRulesResolvedPackageSetLeaseHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    // SAFETY: context remains valid for the product lifetime.
    let bridge = unsafe { &mut *context.cast::<RuntimeRulesBridge>() };
    i32::from(bridge.destroy_resolved_package_set_lease(handle))
}

unsafe extern "C" fn select_payload(
    context: *mut c_void,
    request: *const NativeRulesSelectPayloadRequest,
    result: *mut NativeRulesPayloadSelectionLease,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    if receipt.is_null() {
        return 0;
    }
    // SAFETY: receipt is valid for this direct call and starts without a lease.
    unsafe { *receipt = std::mem::zeroed() };
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    // SAFETY: request and its path span are borrowed for this direct call.
    let request = unsafe { *request };
    let segments =
        match unsafe { borrowed_slice(request.path, request.path_len, "rules payload path") } {
            Ok(value) => value,
            Err(_) => return 0,
        };
    let path = match native_payload_path(segments) {
        Ok(value) => value,
        Err(error) => {
            // SAFETY: context is the stable Rules bridge retained for the product lifetime.
            let bridge = unsafe { &mut *context.cast::<RuntimeRulesBridge>() };
            if let Some(diagnostics) = bridge.retain_operation_diagnostic(&error) {
                // SAFETY: receipt was checked and names the retained lease exactly.
                unsafe {
                    *receipt = NativeOperationErrorReceipt {
                        service: native_utf8(SERVICE),
                        operation: native_utf8(SELECT_OPERATION),
                        status: 0,
                        diagnostics,
                    };
                }
            }
            return 0;
        }
    };
    let expected_parent_fingerprint = match unsafe {
        borrowed_utf8(
            request.expected_parent_fingerprint.bytes,
            request.expected_parent_fingerprint.len,
            "rules expected parent fingerprint",
        )
    } {
        Ok(value) => match RuleFingerprint::parse(value) {
            Ok(value) => value,
            Err(error) => {
                // SAFETY: context is the stable Rules bridge retained for the product lifetime.
                let bridge = unsafe { &mut *context.cast::<RuntimeRulesBridge>() };
                if let Some(diagnostics) = bridge.retain_diagnostic(&error) {
                    // SAFETY: receipt was checked and names the retained lease exactly.
                    unsafe {
                        *receipt = NativeOperationErrorReceipt {
                            service: native_utf8(SERVICE),
                            operation: native_utf8(SELECT_OPERATION),
                            status: 0,
                            diagnostics,
                        };
                    }
                }
                return 0;
            }
        },
        Err(_) => return 0,
    };
    // SAFETY: context is the stable Rules bridge retained for the product lifetime.
    let bridge = unsafe { &mut *context.cast::<RuntimeRulesBridge>() };
    match bridge.select_payload(request.package, expected_parent_fingerprint, path) {
        Ok(lease) => {
            // SAFETY: result was checked and belongs to this direct call.
            unsafe { *result = lease };
            ABI_OK
        }
        Err(error) => {
            if let Some(diagnostics) = bridge.retain_operation_diagnostic(&error) {
                // SAFETY: receipt was checked and names the retained lease exactly.
                unsafe {
                    *receipt = NativeOperationErrorReceipt {
                        service: native_utf8(SERVICE),
                        operation: native_utf8(SELECT_OPERATION),
                        status: 0,
                        diagnostics,
                    };
                }
            }
            0
        }
    }
}

unsafe extern "C" fn destroy_payload_selection_lease(
    context: *mut c_void,
    handle: NativeRulesPayloadSelectionLeaseHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    // SAFETY: context remains valid for the product lifetime.
    let bridge = unsafe { &mut *context.cast::<RuntimeRulesBridge>() };
    i32::from(bridge.destroy_payload_selection_lease(handle))
}

fn native_payload_path(
    segments: &[NativeRulesPayloadPathSegment],
) -> Result<RulePayloadPath, RulesOperationError> {
    let values = segments
        .iter()
        .map(|segment| match segment.kind {
            NativeRulesPayloadPathSegmentKind::Field => {
                let field = unsafe {
                    borrowed_utf8(
                        segment.field.bytes,
                        segment.field.len,
                        "rules payload field segment",
                    )
                }
                .map_err(|_| {
                    RulesOperationError::PayloadSelection(RuleSubtreeSelectionError::Path(
                        gameplay_rules::RulePayloadPathError::InvalidField {
                            field: "<invalid UTF-8>".to_owned(),
                        },
                    ))
                })?;
                RulePayloadPathSegment::field(field.to_owned()).map_err(|error| {
                    RulesOperationError::PayloadSelection(RuleSubtreeSelectionError::Path(error))
                })
            }
            NativeRulesPayloadPathSegmentKind::Index => {
                let index = usize::try_from(segment.index).map_err(|_| {
                    RulesOperationError::PayloadSelection(RuleSubtreeSelectionError::Path(
                        gameplay_rules::RulePayloadPathError::IndexTooLarge {
                            actual: usize::MAX,
                            maximum: gameplay_rules::MAX_RULE_PAYLOAD_PATH_INDEX,
                        },
                    ))
                })?;
                RulePayloadPathSegment::index(index).map_err(|error| {
                    RulesOperationError::PayloadSelection(RuleSubtreeSelectionError::Path(error))
                })
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    RulePayloadPath::new(values).map_err(|error| {
        RulesOperationError::PayloadSelection(RuleSubtreeSelectionError::Path(error))
    })
}

unsafe extern "C" fn destroy_operation_diagnostic_lease(
    context: *mut c_void,
    handle: NativeEngineDiagnosticLeaseHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    // SAFETY: context remains valid for the product lifetime.
    let bridge = unsafe { &mut *context.cast::<RuntimeRulesBridge>() };
    i32::from(bridge.destroy_diagnostic_lease(handle))
}

fn native_utf8(bytes: &[u8]) -> NativeUtf8Slice {
    NativeUtf8Slice {
        bytes: bytes.as_ptr(),
        len: bytes.len(),
    }
}

fn narrow(value: usize) -> Option<u32> {
    u32::try_from(value).ok()
}

fn bounded_source(value: &str) -> String {
    if value.len() <= MAX_DIAGNOSTIC_SOURCE_BYTES {
        return value.to_owned();
    }
    let mut end = MAX_DIAGNOSTIC_SOURCE_BYTES;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use gameplay_rules::{
        admit_rule_package, encode_rule_package, RuleDomainId, RulePackageCandidate,
        RulePackageDependency, RulePackageId, RuleVersion,
    };
    use serde_json::json;

    fn copied_bytes(value: NativeByteSlice) -> Vec<u8> {
        // SAFETY: this helper is used only while its owning readout lease is live.
        unsafe { std::slice::from_raw_parts(value.bytes, value.len) }.to_vec()
    }

    fn copied_utf8(value: NativeUtf8Slice) -> String {
        // SAFETY: this helper is used only while its owning readout lease is live.
        let bytes = unsafe { std::slice::from_raw_parts(value.bytes, value.len) };
        std::str::from_utf8(bytes).unwrap().to_owned()
    }

    fn admitted(
        package: &str,
        version: u64,
        dependencies: Vec<RulePackageDependency>,
    ) -> AdmittedRulePackage {
        admit_rule_package(RulePackageCandidate::new(
            RuleDomainId::parse("fixture").unwrap(),
            RulePackageId::parse(package).unwrap(),
            RuleVersion::new(version).unwrap(),
            dependencies,
            vec![],
            vec![],
            json!({"items": [{"name": package, "value": version}]}),
        ))
        .unwrap()
    }

    fn admit_bridge_package(
        bridge: &mut RuntimeRulesBridge,
        package: AdmittedRulePackage,
    ) -> NativeRulesPackageHandle {
        bridge.admit(&encode_rule_package(&package)).unwrap()
    }

    #[test]
    fn reads_integer_and_binary64_packages_after_parent_release() {
        let mut bridge = RuntimeRulesBridge::new();
        for bytes in [
            include_bytes!("../../../../fixtures/gameplay-rules/package-v1.canonical.json")
                .as_slice(),
            include_bytes!(
                "../../../../fixtures/gameplay-rules/package-v2-binary64.canonical.json"
            )
            .as_slice(),
        ] {
            let handle = bridge.admit(bytes).expect("admit canonical package");
            let readout = bridge.read(handle).expect("read admitted package");
            assert!(bridge.destroy(handle));
            let package = unsafe { &*readout.packages };
            assert_eq!(copied_bytes(package.canonical_bytes), bytes);
            assert_eq!(package.dependency_count as usize, readout.dependencies_len);
            assert_eq!(package.source_count as usize, readout.sources_len);
            assert_eq!(package.provenance_count as usize, readout.provenance_len);
            assert_eq!(copied_utf8(package.fingerprint).len(), 64);
            assert!(bridge.destroy_readout_lease(readout.handle));
            assert!(!bridge.destroy_readout_lease(readout.handle));
        }
    }

    #[test]
    fn maps_owner_decode_failure_to_fixed_typed_diagnostic() {
        let mut bridge = RuntimeRulesBridge::new();
        let error = bridge.admit(b"{").expect_err("invalid package must fail");
        let lease = bridge
            .retain_diagnostic(&error)
            .expect("diagnostic lease handle");
        assert_eq!(lease.diagnostics_len, 1);
        let diagnostic = unsafe { &*lease.diagnostics };
        assert_eq!(copied_utf8(diagnostic.code), "RULE_PACKAGE_MALFORMED_JSON");
        assert!(bridge.destroy_diagnostic_lease(lease.handle));
        assert!(!bridge.destroy_diagnostic_lease(lease.handle));
    }

    #[test]
    fn resolves_copied_package_facts_in_deterministic_order_after_input_release() {
        let mut bridge = RuntimeRulesBridge::new();
        let core = bridge
            .admit(include_bytes!(
                "../../../../fixtures/gameplay-rules/package-v1.canonical.json"
            ))
            .unwrap();
        let binary64 = bridge
            .admit(include_bytes!(
                "../../../../fixtures/gameplay-rules/package-v2-binary64.canonical.json"
            ))
            .unwrap();

        let resolved = bridge.resolve(&[core, binary64]).unwrap();
        assert!(bridge.destroy(core));
        assert!(bridge.destroy(binary64));
        assert_eq!(resolved.packages_len, 2);
        let packages =
            unsafe { std::slice::from_raw_parts(resolved.packages, resolved.packages_len) };
        assert_eq!(copied_utf8(packages[0].package), "binary64");
        assert_eq!(copied_utf8(packages[1].package), "core");
        assert_eq!(resolved.aggregate.dependency_count, 0);
        assert_eq!(resolved.aggregate.source_count, 1);
        assert!(resolved.aggregate.canonical_bytes > 0);
        assert!(bridge.destroy_resolved_package_set_lease(resolved.handle));
        assert!(!bridge.destroy_resolved_package_set_lease(resolved.handle));
    }

    #[test]
    fn retains_typed_resolution_failures_without_display_parsing() {
        let mut bridge = RuntimeRulesBridge::new();
        let available = admit_bridge_package(&mut bridge, admitted("available", 1, vec![]));
        let dependent = admit_bridge_package(
            &mut bridge,
            admitted(
                "dependent",
                1,
                vec![RulePackageDependency::new(
                    RuleDomainId::parse("fixture").unwrap(),
                    RulePackageId::parse("available").unwrap(),
                    RuleVersion::new(2).unwrap(),
                    None,
                )],
            ),
        );
        let error = bridge.resolve(&[dependent, available]).unwrap_err();
        let diagnostic = RulesDiagnosticValue::from_operation_error(&error);
        assert_eq!(diagnostic.code, "RULE_PACKAGE_SET_DEPENDENCY_VERSION");

        let fingerprint_mismatch = admit_bridge_package(
            &mut bridge,
            admitted(
                "fingerprint-mismatch",
                1,
                vec![RulePackageDependency::new(
                    RuleDomainId::parse("fixture").unwrap(),
                    RulePackageId::parse("available").unwrap(),
                    RuleVersion::new(1).unwrap(),
                    Some(RuleFingerprint::parse("0".repeat(64)).unwrap()),
                )],
            ),
        );
        let error = bridge
            .resolve(&[fingerprint_mismatch, available])
            .unwrap_err();
        let diagnostic_lease = bridge.retain_operation_diagnostic(&error).unwrap();
        let diagnostic = unsafe { &*diagnostic_lease.diagnostics };
        assert_eq!(
            copied_utf8(diagnostic.code),
            "RULE_PACKAGE_SET_DEPENDENCY_FINGERPRINT"
        );
        assert!(bridge.destroy_diagnostic_lease(diagnostic_lease.handle));
        assert!(!bridge.destroy_diagnostic_lease(diagnostic_lease.handle));

        let cycle_a = admit_bridge_package(
            &mut bridge,
            admitted(
                "cycle-a",
                1,
                vec![RulePackageDependency::new(
                    RuleDomainId::parse("fixture").unwrap(),
                    RulePackageId::parse("cycle-b").unwrap(),
                    RuleVersion::new(1).unwrap(),
                    None,
                )],
            ),
        );
        let cycle_b = admit_bridge_package(
            &mut bridge,
            admitted(
                "cycle-b",
                1,
                vec![RulePackageDependency::new(
                    RuleDomainId::parse("fixture").unwrap(),
                    RulePackageId::parse("cycle-a").unwrap(),
                    RuleVersion::new(1).unwrap(),
                    None,
                )],
            ),
        );
        let error = bridge.resolve(&[cycle_b, cycle_a]).unwrap_err();
        let diagnostic = RulesDiagnosticValue::from_operation_error(&error);
        assert_eq!(diagnostic.code, "RULE_PACKAGE_SET_DEPENDENCY_CYCLE");
    }

    #[test]
    fn selects_field_and_index_with_stale_fingerprint_diagnostics_and_independent_release() {
        let mut bridge = RuntimeRulesBridge::new();
        let handle = bridge
            .admit(include_bytes!(
                "../../../../fixtures/gameplay-rules/package-v1.canonical.json"
            ))
            .unwrap();
        let readout = bridge.read(handle).unwrap();
        let fingerprint = copied_utf8(unsafe { &*readout.packages }.fingerprint);
        assert!(bridge.destroy_readout_lease(readout.handle));
        let machines = b"machines";
        let output = b"output";
        let path = native_payload_path(&[
            NativeRulesPayloadPathSegment {
                kind: NativeRulesPayloadPathSegmentKind::Field,
                field: NativeUtf8Slice {
                    bytes: machines.as_ptr(),
                    len: machines.len(),
                },
                index: 0,
            },
            NativeRulesPayloadPathSegment {
                kind: NativeRulesPayloadPathSegmentKind::Index,
                field: NativeUtf8Slice::default(),
                index: 0,
            },
            NativeRulesPayloadPathSegment {
                kind: NativeRulesPayloadPathSegmentKind::Field,
                field: NativeUtf8Slice {
                    bytes: output.as_ptr(),
                    len: output.len(),
                },
                index: 0,
            },
        ])
        .unwrap();
        let selection = bridge
            .select_payload(handle, RuleFingerprint::parse(fingerprint).unwrap(), path)
            .unwrap();
        let stale = RuleFingerprint::parse("0".repeat(64)).unwrap();
        let error = bridge
            .select_payload(
                handle,
                stale,
                RulePayloadPath::new(vec![RulePayloadPathSegment::field("machines").unwrap()])
                    .unwrap(),
            )
            .unwrap_err();
        let diagnostic = RulesDiagnosticValue::from_operation_error(&error);
        assert_eq!(diagnostic.code, "RULE_PAYLOAD_PARENT_FINGERPRINT");
        assert_eq!(diagnostic.source.len(), 64);
        assert!(bridge.destroy(handle));
        let selected = unsafe { &*selection.selections };
        assert_eq!(copied_utf8(selected.parent_package), "core");
        assert_eq!(copied_bytes(selected.canonical_bytes), b"10");
        assert!(bridge.destroy_payload_selection_lease(selection.handle));
        assert!(!bridge.destroy_payload_selection_lease(selection.handle));
    }
}
