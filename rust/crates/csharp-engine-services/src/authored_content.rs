use std::{collections::BTreeMap, ffi::c_void};

use asset_catalog::{
    AdmittedAssetCatalog, AssetCatalog, CatalogAdmissionError, CatalogDiagnostic, CatalogEntry,
    CatalogResolveError,
};
use core_assets::{AssetHash, AssetId, AssetKind, AssetReference, AssetVersionReq};
use csharp_engine_abi::*;

use crate::{
    composition::{borrowed_slice, borrowed_utf8, ABI_OK},
    content::RuntimeContentBridge,
};

const SERVICE: &[u8] = b"AuthoredContent";
const MAX_ENTRIES: usize = 4096;
const MAX_DEPENDENCIES: usize = 16384;
const MAX_TEXT: usize = 4096;
const MAX_DIAGNOSTICS: usize = 128;

pub(crate) struct RuntimeAuthoredContentBridge {
    catalogs: BTreeMap<u64, AdmittedAssetCatalog>,
    next_catalog: u64,
    leases: BTreeMap<u64, CatalogLease>,
    next_lease: u64,
    resolved_leases: BTreeMap<u64, ResolvedLease>,
    next_resolved_lease: u64,
    diagnostics: BTreeMap<u64, DiagnosticLease>,
    next_diagnostic: u64,
    content: Option<*const RuntimeContentBridge>,
}
struct Text {
    values: Vec<String>,
}
impl Text {
    fn copy(&mut self, value: &str) -> NativeUtf8Slice {
        self.values.push(value.to_owned());
        let value = self.values.last().unwrap();
        NativeUtf8Slice {
            bytes: value.as_ptr(),
            len: value.len(),
        }
    }
}
struct CatalogLease {
    _text: Text,
    entries: Vec<NativeAuthoredCatalogEntryReadout>,
    dependencies: Vec<NativeAuthoredCatalogDependencyReadout>,
    hash: String,
}
struct ResolvedLease {
    _text: Text,
    entry: Vec<NativeAuthoredCatalogEntryReadout>,
    dependencies: Vec<NativeAuthoredCatalogDependencyReadout>,
}
struct DiagnosticLease {
    _text: Text,
    values: Vec<NativeEngineDiagnostic>,
}
#[derive(Debug)]
enum AuthoredError {
    Validation(Vec<CatalogDiagnostic>),
    Simple {
        code: &'static str,
        message: String,
        source: String,
    },
}
impl AuthoredError {
    fn simple(message: impl Into<String>) -> Self {
        Self::Simple {
            code: "AUTHORED_CONTENT_INPUT",
            message: message.into(),
            source: "catalog".into(),
        }
    }
}

impl RuntimeAuthoredContentBridge {
    pub(crate) fn new() -> Self {
        Self {
            catalogs: BTreeMap::new(),
            next_catalog: 1,
            leases: BTreeMap::new(),
            next_lease: 1,
            resolved_leases: BTreeMap::new(),
            next_resolved_lease: 1,
            diagnostics: BTreeMap::new(),
            next_diagnostic: 1,
            content: None,
        }
    }
    pub(crate) fn bind_content(&mut self, content: &RuntimeContentBridge) {
        self.content = Some(content);
    }
    fn retain(
        &mut self,
        catalog: AdmittedAssetCatalog,
    ) -> Result<NativeAuthoredCatalogHandle, AuthoredError> {
        let value = self.next_catalog;
        self.next_catalog = value
            .checked_add(1)
            .ok_or_else(|| AuthoredError::simple("catalog handle exhausted"))?;
        self.catalogs.insert(value, catalog);
        Ok(NativeAuthoredCatalogHandle { value })
    }
    fn admit_rows(
        &mut self,
        entries: &[NativeAuthoredCatalogEntryInput],
        dependencies: &[NativeAuthoredCatalogDependencyInput],
    ) -> Result<NativeAuthoredCatalogHandle, AuthoredError> {
        if entries.len() > MAX_ENTRIES || dependencies.len() > MAX_DEPENDENCIES {
            return Err(AuthoredError::simple("catalog input exceeds engine bounds"));
        }
        let mut values = Vec::with_capacity(entries.len());
        for row in entries {
            let id = parse_id(row.id, "entry id").map_err(AuthoredError::simple)?;
            if id.kind() == AssetKind::Material {
                return Err(AuthoredError::simple(
                    "material requires its payload and is not admitted by AuthoredContent base",
                ));
            }
            let mut entry = CatalogEntry::new(id, row.version);
            if row.has_hash {
                entry.hash =
                    Some(parse_hash(row.hash, "entry hash").map_err(AuthoredError::simple)?);
            }
            if row.has_source_path {
                entry.source_path = Some(
                    parse_text(row.source_path, "source path").map_err(AuthoredError::simple)?,
                );
            }
            if row.has_label {
                entry.label = Some(parse_text(row.label, "label").map_err(AuthoredError::simple)?);
            }
            values.push(entry);
        }
        for row in dependencies {
            let entry_id =
                parse_id(row.entry_id, "dependency entry id").map_err(AuthoredError::simple)?;
            let dependency = parse_reference_parts(
                row.reference_id,
                row.reference_version_kind,
                row.reference_version,
                row.reference_has_hash,
                row.reference_hash,
            )
            .map_err(AuthoredError::simple)?;
            let Some(entry) = values.iter_mut().find(|entry| entry.id == entry_id) else {
                return Err(AuthoredError::simple(
                    "dependency entry is absent from request",
                ));
            };
            entry.dependencies.push(dependency);
        }
        self.retain(
            AdmittedAssetCatalog::admit(AssetCatalog::from_entries(values))
                .map_err(admission_error)?,
        )
    }
    fn admit_content(
        &mut self,
        reference: NativeContentReferenceHandle,
    ) -> Result<NativeAuthoredCatalogHandle, AuthoredError> {
        let content = unsafe {
            self.content
                .ok_or_else(|| AuthoredError::simple("authored content is not composed"))?
                .as_ref()
        }
        .ok_or_else(|| AuthoredError::simple("authored content is not composed"))?;
        let bytes = content
            .retained_bytes(reference)
            .ok_or_else(|| AuthoredError::simple("unknown content reference"))?;
        let text = std::str::from_utf8(&bytes)
            .map_err(|_| AuthoredError::simple("catalog content was not UTF-8"))?;
        self.retain(AdmittedAssetCatalog::reopen(text).map_err(admission_error)?)
    }
    fn read_catalog(
        &mut self,
        handle: NativeAuthoredCatalogHandle,
    ) -> Option<NativeAuthoredCatalogReadoutLease> {
        let catalog = self.catalogs.get(&handle.value)?.clone();
        self.lease_for(catalog.catalog(), catalog.canonical_hash())
    }
    fn resolve(
        &mut self,
        request: NativeAuthoredCatalogResolveRequest,
    ) -> Result<NativeAuthoredResolvedEntryLease, AuthoredError> {
        let catalog = self
            .catalogs
            .get(&request.catalog.value)
            .ok_or_else(|| AuthoredError::simple("unknown catalog handle"))?;
        let reference = parse_reference_parts(
            request.reference_id,
            request.reference_version_kind,
            request.reference_version,
            request.reference_has_hash,
            request.reference_hash,
        )
        .map_err(AuthoredError::simple)?;
        let entry = catalog
            .catalog()
            .resolve_reference(&reference)
            .map_err(|error| match error {
                CatalogResolveError::Missing { .. } => AuthoredError::Simple {
                    code: "AUTHORED_CONTENT_REFERENCE_MISSING",
                    message: "catalog reference is missing".into(),
                    source: reference.id().as_str().into(),
                },
                CatalogResolveError::Stale { .. } => AuthoredError::Simple {
                    code: "AUTHORED_CONTENT_REFERENCE_STALE",
                    message: "catalog reference is stale".into(),
                    source: reference.id().as_str().into(),
                },
            })?
            .clone();
        let value = self.next_resolved_lease;
        self.next_resolved_lease = value
            .checked_add(1)
            .ok_or_else(|| AuthoredError::simple("resolved entry lease exhausted"))?;
        let mut text = Text { values: vec![] };
        let dependencies = entry
            .dependencies
            .iter()
            .map(|dependency| dependency_row(&mut text, &entry, dependency))
            .collect();
        let rows = vec![entry_row(&mut text, &entry)];
        let lease = ResolvedLease {
            _text: text,
            entry: rows,
            dependencies,
        };
        let out = NativeAuthoredResolvedEntryLease {
            handle: NativeAuthoredResolvedEntryLeaseHandle { value },
            entry: lease.entry.as_ptr(),
            entry_len: lease.entry.len(),
            dependencies: lease.dependencies.as_ptr(),
            dependencies_len: lease.dependencies.len(),
        };
        self.resolved_leases.insert(value, lease);
        Ok(out)
    }
    fn lease_for(
        &mut self,
        catalog: &AssetCatalog,
        canonical_hash: &str,
    ) -> Option<NativeAuthoredCatalogReadoutLease> {
        let value = self.next_lease;
        self.next_lease = value.checked_add(1)?;
        let mut text = Text { values: vec![] };
        let mut dependencies = vec![];
        let entries = catalog
            .iter()
            .map(|entry| {
                for dependency in &entry.dependencies {
                    dependencies.push(dependency_row(&mut text, entry, dependency));
                }
                entry_row(&mut text, entry)
            })
            .collect::<Vec<_>>();
        let hash = canonical_hash.to_owned();
        let lease = CatalogLease {
            _text: text,
            entries,
            dependencies,
            hash,
        };
        let out = NativeAuthoredCatalogReadoutLease {
            handle: NativeAuthoredCatalogReadoutLeaseHandle { value },
            canonical_hash: NativeUtf8Slice {
                bytes: lease.hash.as_ptr(),
                len: lease.hash.len(),
            },
            entry_count: u32::try_from(lease.entries.len()).ok()?,
            entries: lease.entries.as_ptr(),
            entries_len: lease.entries.len(),
            dependencies: lease.dependencies.as_ptr(),
            dependencies_len: lease.dependencies.len(),
        };
        self.leases.insert(value, lease);
        Some(out)
    }
    fn diagnostic(&mut self, error: AuthoredError) -> Option<NativeEngineDiagnosticLease> {
        let value = self.next_diagnostic;
        self.next_diagnostic = value.checked_add(1)?;
        let mut text = Text { values: vec![] };
        let facts = match error {
            AuthoredError::Validation(values) => values
                .into_iter()
                .take(MAX_DIAGNOSTICS)
                .map(|value| (value.code, value.message, value.path))
                .collect(),
            AuthoredError::Simple {
                code,
                message,
                source,
            } => vec![(code.to_owned(), message, source)],
        };
        let values = facts
            .into_iter()
            .map(|(code, message, source)| NativeEngineDiagnostic {
                code: text.copy(&code),
                message: text.copy(&message),
                source: text.copy(&source),
            })
            .collect();
        let lease = DiagnosticLease {
            _text: text,
            values,
        };
        let out = NativeEngineDiagnosticLease {
            handle: NativeEngineDiagnosticLeaseHandle { value },
            diagnostics: lease.values.as_ptr(),
            diagnostics_len: lease.values.len(),
        };
        self.diagnostics.insert(value, lease);
        Some(out)
    }
}
fn entry_row(text: &mut Text, entry: &CatalogEntry) -> NativeAuthoredCatalogEntryReadout {
    NativeAuthoredCatalogEntryReadout {
        id: text.copy(entry.id.as_str()),
        kind: native_kind(entry.kind()),
        version: entry.version,
        has_hash: entry.hash.is_some(),
        hash: text.copy(entry.hash.as_ref().map_or("", AssetHash::as_str)),
        has_source_path: entry.source_path.is_some(),
        source_path: text.copy(entry.source_path.as_deref().unwrap_or("")),
        has_label: entry.label.is_some(),
        label: text.copy(entry.label.as_deref().unwrap_or("")),
        dependency_count: u32::try_from(entry.dependencies.len()).unwrap_or(u32::MAX),
    }
}
fn dependency_row(
    text: &mut Text,
    entry: &CatalogEntry,
    reference: &AssetReference,
) -> NativeAuthoredCatalogDependencyReadout {
    NativeAuthoredCatalogDependencyReadout {
        entry_id: text.copy(entry.id.as_str()),
        reference: native_reference(text, reference),
    }
}
fn native_reference(text: &mut Text, value: &AssetReference) -> NativeAuthoredAssetReference {
    NativeAuthoredAssetReference {
        id: text.copy(value.id().as_str()),
        version_kind: match value.version() {
            AssetVersionReq::Any => NativeAssetVersionRequirementKind::Any,
            AssetVersionReq::Exact(_) => NativeAssetVersionRequirementKind::Exact,
            AssetVersionReq::AtLeast(_) => NativeAssetVersionRequirementKind::AtLeast,
        },
        version: match value.version() {
            AssetVersionReq::Any => 0,
            AssetVersionReq::Exact(v) | AssetVersionReq::AtLeast(v) => v,
        },
        has_hash: value.hash().is_some(),
        hash: text.copy(value.hash().map_or("", AssetHash::as_str)),
    }
}
fn native_kind(kind: AssetKind) -> NativeAssetKind {
    match kind {
        AssetKind::Material => NativeAssetKind::Material,
        AssetKind::StaticMesh => NativeAssetKind::StaticMesh,
        AssetKind::AnimatedMesh => NativeAssetKind::AnimatedMesh,
        AssetKind::Sprite => NativeAssetKind::Sprite,
        AssetKind::SpriteSheet => NativeAssetKind::SpriteSheet,
        AssetKind::Texture => NativeAssetKind::Texture,
        AssetKind::AudioClip => NativeAssetKind::AudioClip,
        AssetKind::Font => NativeAssetKind::Font,
        AssetKind::VoxelVolume => NativeAssetKind::VoxelVolume,
        AssetKind::VoxelObject => NativeAssetKind::VoxelObject,
        AssetKind::Script => NativeAssetKind::Script,
        AssetKind::Scene => NativeAssetKind::Scene,
    }
}
fn admission_error(error: CatalogAdmissionError) -> AuthoredError {
    match error {
        CatalogAdmissionError::Validation(report) => {
            AuthoredError::Validation(report.diagnostics())
        }
        CatalogAdmissionError::Codec(error) => AuthoredError::Simple {
            code: "AUTHORED_CONTENT_CODEC",
            message: error.message,
            source: error.path,
        },
        CatalogAdmissionError::RevisionExhausted => {
            AuthoredError::simple("catalog revision exhausted")
        }
    }
}
fn parse_text(value: NativeUtf8Slice, field: &'static str) -> Result<String, String> {
    let value = unsafe { borrowed_utf8(value.bytes, value.len, field) }
        .map_err(|error| error.to_string())?;
    if value.len() > MAX_TEXT {
        return Err(format!("{field} exceeds engine bound"));
    }
    Ok(value.to_owned())
}
fn parse_id(value: NativeUtf8Slice, field: &'static str) -> Result<AssetId, String> {
    AssetId::parse(&parse_text(value, field)?).map_err(|error| error.to_string())
}
fn parse_hash(value: NativeUtf8Slice, field: &'static str) -> Result<AssetHash, String> {
    AssetHash::parse(&parse_text(value, field)?).map_err(|error| error.to_string())
}
fn parse_reference_parts(
    id: NativeUtf8Slice,
    version_kind: NativeAssetVersionRequirementKind,
    version: u32,
    has_hash: bool,
    hash: NativeUtf8Slice,
) -> Result<AssetReference, String> {
    let version = match version_kind {
        NativeAssetVersionRequirementKind::Any => AssetVersionReq::Any,
        NativeAssetVersionRequirementKind::Exact => AssetVersionReq::Exact(version),
        NativeAssetVersionRequirementKind::AtLeast => AssetVersionReq::AtLeast(version),
    };
    Ok(AssetReference::new(
        parse_id(id, "asset reference id")?,
        version,
        if has_hash {
            Some(parse_hash(hash, "asset reference hash")?)
        } else {
            None
        },
    ))
}
pub(crate) fn api(bridge: &mut RuntimeAuthoredContentBridge) -> NativeAuthoredContentApi {
    NativeAuthoredContentApi {
        context: (bridge as *mut RuntimeAuthoredContentBridge).cast(),
        admit_catalog,
        admit_catalog_from_content,
        destroy_catalog,
        read_catalog,
        destroy_catalog_readout_lease,
        resolve_reference,
        destroy_resolved_entry_lease,
        destroy_operation_diagnostic_lease,
    }
}
fn receipt(
    bridge: &mut RuntimeAuthoredContentBridge,
    operation: &[u8],
    error: AuthoredError,
) -> NativeOperationErrorReceipt {
    NativeOperationErrorReceipt {
        service: NativeUtf8Slice {
            bytes: SERVICE.as_ptr(),
            len: SERVICE.len(),
        },
        operation: NativeUtf8Slice {
            bytes: operation.as_ptr(),
            len: operation.len(),
        },
        status: 0,
        diagnostics: bridge
            .diagnostic(error)
            .unwrap_or(NativeEngineDiagnosticLease {
                handle: NativeEngineDiagnosticLeaseHandle::default(),
                diagnostics: std::ptr::null(),
                diagnostics_len: 0,
            }),
    }
}
unsafe extern "C" fn admit_catalog(
    context: *mut c_void,
    request: *const NativeAuthoredCatalogAdmitRequest,
    result: *mut NativeAuthoredCatalogHandle,
    receipt_out: *mut NativeOperationErrorReceipt,
) -> i32 {
    if receipt_out.is_null() {
        return 0;
    }
    unsafe { *receipt_out = std::mem::zeroed() };
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let request = unsafe { *request };
    let entries = match unsafe {
        borrowed_slice(
            request.entries,
            request.entries_len,
            "authored catalog entries",
        )
    } {
        Ok(v) => v,
        Err(_) => return 0,
    };
    let dependencies = match unsafe {
        borrowed_slice(
            request.dependencies,
            request.dependencies_len,
            "authored catalog dependencies",
        )
    } {
        Ok(v) => v,
        Err(_) => return 0,
    };
    let bridge = unsafe { &mut *context.cast::<RuntimeAuthoredContentBridge>() };
    match bridge.admit_rows(entries, dependencies) {
        Ok(handle) => {
            unsafe { *result = handle };
            ABI_OK
        }
        Err(error) => {
            unsafe { *receipt_out = receipt(bridge, b"AdmitCatalog", error) };
            0
        }
    }
}
unsafe extern "C" fn admit_catalog_from_content(
    context: *mut c_void,
    request: *const NativeAuthoredCatalogFromContentRequest,
    result: *mut NativeAuthoredCatalogHandle,
    receipt_out: *mut NativeOperationErrorReceipt,
) -> i32 {
    if receipt_out.is_null() {
        return 0;
    }
    unsafe { *receipt_out = std::mem::zeroed() };
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAuthoredContentBridge>() };
    let request = unsafe { *request };
    match bridge.admit_content(request.content) {
        Ok(handle) => {
            unsafe { *result = handle };
            ABI_OK
        }
        Err(error) => {
            unsafe { *receipt_out = receipt(bridge, b"AdmitCatalogFromContent", error) };
            0
        }
    }
}
unsafe extern "C" fn destroy_catalog(
    context: *mut c_void,
    handle: NativeAuthoredCatalogHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAuthoredContentBridge>() };
    i32::from(handle.value != 0 && bridge.catalogs.remove(&handle.value).is_some())
}
unsafe extern "C" fn read_catalog(
    context: *mut c_void,
    handle: NativeAuthoredCatalogHandle,
    result: *mut NativeAuthoredCatalogReadoutLease,
) -> i32 {
    if context.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAuthoredContentBridge>() };
    match bridge.read_catalog(handle) {
        Some(value) => {
            unsafe { *result = value };
            ABI_OK
        }
        None => 0,
    }
}
unsafe extern "C" fn destroy_catalog_readout_lease(
    context: *mut c_void,
    handle: NativeAuthoredCatalogReadoutLeaseHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAuthoredContentBridge>() };
    i32::from(handle.value != 0 && bridge.leases.remove(&handle.value).is_some())
}
unsafe extern "C" fn resolve_reference(
    context: *mut c_void,
    request: *const NativeAuthoredCatalogResolveRequest,
    result: *mut NativeAuthoredResolvedEntryLease,
    receipt_out: *mut NativeOperationErrorReceipt,
) -> i32 {
    if receipt_out.is_null() {
        return 0;
    }
    unsafe { *receipt_out = std::mem::zeroed() };
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAuthoredContentBridge>() };
    match bridge.resolve(unsafe { *request }) {
        Ok(value) => {
            unsafe { *result = value };
            ABI_OK
        }
        Err(error) => {
            unsafe { *receipt_out = receipt(bridge, b"ResolveReference", error) };
            0
        }
    }
}
unsafe extern "C" fn destroy_resolved_entry_lease(
    context: *mut c_void,
    handle: NativeAuthoredResolvedEntryLeaseHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAuthoredContentBridge>() };
    i32::from(handle.value != 0 && bridge.resolved_leases.remove(&handle.value).is_some())
}
unsafe extern "C" fn destroy_operation_diagnostic_lease(
    context: *mut c_void,
    handle: NativeEngineDiagnosticLeaseHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAuthoredContentBridge>() };
    i32::from(handle.value != 0 && bridge.diagnostics.remove(&handle.value).is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slice(value: &'static [u8]) -> NativeUtf8Slice {
        NativeUtf8Slice {
            bytes: value.as_ptr(),
            len: value.len(),
        }
    }

    #[test]
    fn admits_typed_and_retained_content_catalogs_through_named_callbacks() {
        use std::{collections::BTreeMap, sync::Arc};

        let source = AssetCatalog::from_entries(vec![CatalogEntry::new(
            AssetId::parse("scene/test").unwrap(),
            2,
        )
        .with_hash(AssetHash::parse("aabb").unwrap())
        .with_label("Test")]);
        let canonical = AdmittedAssetCatalog::admit(source)
            .unwrap()
            .canonical_json()
            .as_bytes()
            .to_vec();
        let mut resources = BTreeMap::new();
        resources.insert("catalog.json".to_owned(), Arc::from(canonical));
        let mut content = RuntimeContentBridge::new(resources);
        let content_api = crate::content::api(&mut content);
        let mut reference = NativeContentReferenceHandle::default();
        assert_eq!(
            unsafe {
                (content_api.open_reference)(
                    content_api.context,
                    &NativeContentOpenRequest {
                        path: slice(b"catalog.json"),
                    },
                    &mut reference,
                )
            },
            ABI_OK
        );

        let mut bridge = RuntimeAuthoredContentBridge::new();
        bridge.bind_content(&content);
        let api = super::api(&mut bridge);

        let mut from_content = NativeAuthoredCatalogHandle::default();
        let mut receipt = NativeOperationErrorReceipt {
            service: slice(b""),
            operation: slice(b""),
            status: 0,
            diagnostics: NativeEngineDiagnosticLease {
                handle: NativeEngineDiagnosticLeaseHandle::default(),
                diagnostics: std::ptr::null(),
                diagnostics_len: 0,
            },
        };
        assert_eq!(
            unsafe {
                (api.admit_catalog_from_content)(
                    api.context,
                    &NativeAuthoredCatalogFromContentRequest { content: reference },
                    &mut from_content,
                    &mut receipt,
                )
            },
            ABI_OK
        );
        assert_eq!(receipt.diagnostics.handle.value, 0);
        let mut readout = NativeAuthoredCatalogReadoutLease {
            handle: NativeAuthoredCatalogReadoutLeaseHandle::default(),
            canonical_hash: slice(b""),
            entry_count: 0,
            entries: std::ptr::null(),
            entries_len: 0,
            dependencies: std::ptr::null(),
            dependencies_len: 0,
        };
        assert_eq!(
            unsafe { (api.read_catalog)(api.context, from_content, &mut readout) },
            ABI_OK
        );
        assert_eq!(readout.entry_count, 1);
        assert_eq!(
            unsafe {
                std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                    (*readout.entries).id.bytes,
                    (*readout.entries).id.len,
                ))
            },
            "scene/test"
        );
        assert_eq!(
            unsafe { (api.destroy_catalog_readout_lease)(api.context, readout.handle) },
            ABI_OK
        );
        let mut resolved = NativeAuthoredResolvedEntryLease {
            handle: NativeAuthoredResolvedEntryLeaseHandle::default(),
            entry: std::ptr::null(),
            entry_len: 0,
            dependencies: std::ptr::null(),
            dependencies_len: 0,
        };
        assert_eq!(
            unsafe {
                (api.resolve_reference)(
                    api.context,
                    &NativeAuthoredCatalogResolveRequest {
                        catalog: from_content,
                        reference_id: slice(b"scene/test"),
                        reference_version_kind: NativeAssetVersionRequirementKind::Exact,
                        reference_version: 2,
                        reference_has_hash: true,
                        reference_hash: slice(b"aabb"),
                    },
                    &mut resolved,
                    &mut receipt,
                )
            },
            ABI_OK
        );
        assert_eq!(resolved.entry_len, 1);
        assert_eq!(
            unsafe { (api.destroy_resolved_entry_lease)(api.context, resolved.handle) },
            ABI_OK
        );
        assert_eq!(
            unsafe { (api.destroy_catalog)(api.context, from_content) },
            ABI_OK
        );
        assert_eq!(
            unsafe { (content_api.destroy_reference)(content_api.context, reference) },
            ABI_OK
        );

        let entries = [NativeAuthoredCatalogEntryInput {
            id: slice(b"scene/test"),
            version: 2,
            has_hash: true,
            hash: slice(b"aabb"),
            has_source_path: false,
            source_path: slice(b""),
            has_label: true,
            label: slice(b"Test"),
        }];
        let mut typed = NativeAuthoredCatalogHandle::default();
        assert_eq!(
            unsafe {
                (api.admit_catalog)(
                    api.context,
                    &NativeAuthoredCatalogAdmitRequest {
                        entries: entries.as_ptr(),
                        entries_len: entries.len(),
                        dependencies: std::ptr::null(),
                        dependencies_len: 0,
                    },
                    &mut typed,
                    &mut receipt,
                )
            },
            ABI_OK
        );
        assert_eq!(unsafe { (api.destroy_catalog)(api.context, typed) }, ABI_OK);

        let material = [NativeAuthoredCatalogEntryInput {
            id: slice(b"material/test"),
            ..entries[0]
        }];
        assert_eq!(
            unsafe {
                (api.admit_catalog)(
                    api.context,
                    &NativeAuthoredCatalogAdmitRequest {
                        entries: material.as_ptr(),
                        entries_len: material.len(),
                        dependencies: std::ptr::null(),
                        dependencies_len: 0,
                    },
                    &mut typed,
                    &mut receipt,
                )
            },
            0
        );
        assert_ne!(receipt.diagnostics.handle.value, 0);
        assert_eq!(
            unsafe {
                (api.destroy_operation_diagnostic_lease)(api.context, receipt.diagnostics.handle)
            },
            ABI_OK
        );
    }
}
