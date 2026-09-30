//! Durable opaque product-byte storage behind the generated NativeAOT table.
//!
//! Callers supply opaque bytes. This owner supplies relative-key admission,
//! revisions, atomic replacement, failure preservation, and retained blob lifetime.
//! The header magic identifies only the storage layout; payload formats belong
//! to their codecs, not this service.

use std::{
    collections::BTreeMap,
    ffi::c_void,
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
};

use csharp_engine_abi::*;

use crate::{
    composition::{borrowed_utf8, ABI_OK},
    operation_diagnostics::{clear_receipt, OperationDiagnostics},
    CsharpEngineServicesError,
};

const HEADER_MAGIC: [u8; 4] = *b"RSP2";
const HEADER_LEN: usize = 4 + 8 + 8;

#[derive(Debug)]
struct DurableStore {
    root: PathBuf,
}

#[derive(Debug, Clone)]
struct PersistenceBlob {
    present: bool,
    revision: u64,
    payload: Vec<u8>,
}

pub(crate) struct RuntimePersistenceBridge {
    /// Host-selected application root. Product code supplies only a relative
    /// scope to `open_store`; this base is never exposed across the ABI.
    persistence_root: Option<PathBuf>,
    stores: BTreeMap<u64, DurableStore>,
    blobs: BTreeMap<u64, PersistenceBlob>,
    next_store: u64,
    next_blob: u64,
    diagnostics: OperationDiagnostics,
}

impl RuntimePersistenceBridge {
    pub(crate) fn new(persistence_root: Option<PathBuf>) -> Self {
        Self {
            persistence_root,
            stores: BTreeMap::new(),
            blobs: BTreeMap::new(),
            next_store: 1,
            next_blob: 1,
            diagnostics: OperationDiagnostics::default(),
        }
    }

    fn insert_store(&mut self, store: DurableStore) -> Option<NativePersistenceStoreHandle> {
        let value = self.next_store;
        self.next_store = value.checked_add(1)?;
        self.stores.insert(value, store);
        Some(NativePersistenceStoreHandle { value })
    }

    fn insert_blob(&mut self, blob: PersistenceBlob) -> Option<NativePersistenceBlobHandle> {
        let value = self.next_blob;
        self.next_blob = value.checked_add(1)?;
        self.blobs.insert(value, blob);
        Some(NativePersistenceBlobHandle { value })
    }

    fn store_path(
        &self,
        store: NativePersistenceStoreHandle,
        key: NativeUtf8Slice,
    ) -> Result<(PathBuf, String), CsharpEngineServicesError> {
        let key = unsafe { borrowed_utf8(key.bytes, key.len, "persistence key") }?;
        let store = self.stores.get(&store.value).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_PERSISTENCE_STORE",
                "the persistence store handle is not open",
            )
        })?;
        Ok((storage_path(&store.root, key, "key")?, key.to_owned()))
    }

    fn open_store(
        &mut self,
        request: &NativePersistenceOpenRequest,
    ) -> Result<NativePersistenceStoreHandle, CsharpEngineServicesError> {
        let scope =
            unsafe { borrowed_utf8(request.scope.bytes, request.scope.len, "persistence scope") }?;
        let base_root = self.persistence_root.as_ref().ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_PERSISTENCE_ROOT",
                "the host was started without a persistence root",
            )
        })?;
        let root = storage_path(base_root, scope, "scope")?;
        fs::create_dir_all(&root).map_err(|error| {
            io_error(format!(
                "creating persistence scope `{scope}` failed: {error}"
            ))
        })?;
        self.insert_store(DurableStore { root })
            .ok_or_else(handle_exhausted)
    }

    fn save(
        &mut self,
        request: &NativePersistenceSaveRequest,
    ) -> Result<NativePersistenceSaveReceipt, CsharpEngineServicesError> {
        let (path, key) = self.store_path(request.store, request.key)?;
        let payload = unsafe { borrowed_bytes(request.payload, "persistence payload") }?.to_vec();
        let current = read_blob(&path, &key)?;
        if !matches_guard(
            request.revision_guard,
            request.expected_revision,
            current.as_ref(),
        ) {
            return Ok(NativePersistenceSaveReceipt {
                outcome: NativePersistenceSaveOutcome::RevisionConflict,
                revision: current.as_ref().map_or(0, |blob| blob.revision),
            });
        }
        let revision = match current {
            Some(value) => value.revision.checked_add(1).ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_PERSISTENCE_REVISION",
                    format!("persistence key `{key}` has no revision after u64::MAX"),
                )
            })?,
            None => 1,
        };
        let next = PersistenceBlob {
            present: true,
            revision,
            payload,
        };
        write_atomically(&path, &next).map_err(|error| {
            io_error(format!("writing persistence key `{key}` failed: {error}"))
        })?;
        Ok(NativePersistenceSaveReceipt {
            outcome: NativePersistenceSaveOutcome::Saved,
            revision,
        })
    }

    fn delete(
        &mut self,
        request: &NativePersistenceDeleteRequest,
    ) -> Result<NativePersistenceDeleteReceipt, CsharpEngineServicesError> {
        let (path, key) = self.store_path(request.store, request.key)?;
        let current = read_blob(&path, &key)?;
        let revision = current.as_ref().map_or(0, |blob| blob.revision);
        let outcome = if !matches_guard(
            request.revision_guard,
            request.expected_revision,
            current.as_ref(),
        ) {
            NativePersistenceDeleteOutcome::RevisionConflict
        } else if current.is_none() {
            NativePersistenceDeleteOutcome::Missing
        } else {
            // Flush the directory entry removal before reporting durable success.
            // A failed flush is an operation failure, not a deletion receipt.
            let removed = path
                .parent()
                .ok_or_else(|| std::io::Error::other("the key has no parent directory"))
                .and_then(File::open)
                .and_then(|directory| {
                    fs::remove_file(&path)?;
                    directory.sync_all()
                });
            removed.map_err(|error| {
                io_error(format!("deleting persistence key `{key}` failed: {error}"))
            })?;
            NativePersistenceDeleteOutcome::Deleted
        };
        Ok(NativePersistenceDeleteReceipt { outcome, revision })
    }

    fn load(
        &mut self,
        request: &NativePersistenceLoadRequest,
    ) -> Result<NativePersistenceBlobHandle, CsharpEngineServicesError> {
        let (path, key) = self.store_path(request.store, request.key)?;
        let blob = read_blob(&path, &key)?.unwrap_or(PersistenceBlob {
            present: false,
            revision: 0,
            payload: Vec::new(),
        });
        self.insert_blob(blob).ok_or_else(handle_exhausted)
    }
}

pub(crate) fn api(bridge: &mut RuntimePersistenceBridge) -> NativePersistenceApi {
    NativePersistenceApi {
        context: (bridge as *mut RuntimePersistenceBridge).cast(),
        open_store,
        destroy_store,
        save,
        delete,
        load,
        destroy_blob,
        describe_blob,
        copy_blob,
        read_blob_bytes,
    }
}

/// Runs one refusable operation: writes its result or reports its refusal
/// through the receipt.
unsafe fn refusable<Request, Output>(
    context: *mut c_void,
    request: *const Request,
    result: *mut Output,
    receipt: *mut NativeOperationErrorReceipt,
    operation: impl FnOnce(
        &mut RuntimePersistenceBridge,
        &Request,
    ) -> Result<Output, CsharpEngineServicesError>,
) -> i32 {
    clear_receipt(receipt);
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimePersistenceBridge>() };
    match operation(bridge, unsafe { &*request }) {
        Ok(value) => {
            unsafe { *result = value };
            ABI_OK
        }
        Err(error) => {
            bridge.diagnostics.retain(&error, receipt);
            0
        }
    }
}

unsafe extern "C" fn open_store(
    context: *mut c_void,
    request: *const NativePersistenceOpenRequest,
    result: *mut NativePersistenceStoreHandle,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe {
        refusable(context, request, result, receipt, |bridge, request| {
            bridge.open_store(request)
        })
    }
}

unsafe extern "C" fn destroy_store(
    context: *mut c_void,
    store: NativePersistenceStoreHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimePersistenceBridge>() };
    if bridge.stores.remove(&store.value).is_some() {
        ABI_OK
    } else {
        0
    }
}

unsafe extern "C" fn save(
    context: *mut c_void,
    request: *const NativePersistenceSaveRequest,
    result: *mut NativePersistenceSaveReceipt,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe {
        refusable(context, request, result, receipt, |bridge, request| {
            bridge.save(request)
        })
    }
}

unsafe extern "C" fn delete(
    context: *mut c_void,
    request: *const NativePersistenceDeleteRequest,
    result: *mut NativePersistenceDeleteReceipt,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe {
        refusable(context, request, result, receipt, |bridge, request| {
            bridge.delete(request)
        })
    }
}

unsafe extern "C" fn load(
    context: *mut c_void,
    request: *const NativePersistenceLoadRequest,
    result: *mut NativePersistenceBlobHandle,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe {
        refusable(context, request, result, receipt, |bridge, request| {
            bridge.load(request)
        })
    }
}

unsafe extern "C" fn read_blob_bytes(
    context: *mut c_void,
    blob: NativePersistenceBlobHandle,
    result: *mut NativeByteResult,
) -> i32 {
    if context.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimePersistenceBridge>() };
    let Some(blob) = bridge.blobs.get(&blob.value) else {
        return 0;
    };
    // The blob stays retained until a later call destroys it, so the result
    // borrows its payload directly.
    unsafe {
        *result = NativeByteResult {
            bytes: blob.payload.as_ptr(),
            len: blob.payload.len(),
        }
    };
    ABI_OK
}

unsafe extern "C" fn destroy_blob(context: *mut c_void, blob: NativePersistenceBlobHandle) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimePersistenceBridge>() };
    if bridge.blobs.remove(&blob.value).is_some() {
        ABI_OK
    } else {
        0
    }
}

unsafe extern "C" fn describe_blob(
    context: *mut c_void,
    blob: NativePersistenceBlobHandle,
    receipt: *mut NativePersistenceBlobInfo,
) -> i32 {
    if context.is_null() || receipt.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimePersistenceBridge>() };
    let Some(blob) = bridge.blobs.get(&blob.value) else {
        return 0;
    };
    unsafe {
        *receipt = NativePersistenceBlobInfo {
            present: blob.present,
            revision: blob.revision,
            payload_len: blob.payload.len(),
        };
    }
    ABI_OK
}

unsafe extern "C" fn copy_blob(
    context: *mut c_void,
    request: *const NativePersistenceCopyBlobRequest,
) -> i32 {
    if context.is_null() || request.is_null() {
        return 0;
    }
    let request = unsafe { &*request };
    let bridge = unsafe { &mut *context.cast::<RuntimePersistenceBridge>() };
    let Some(blob) = bridge.blobs.get(&request.blob.value) else {
        return 0;
    };
    if request.destination.len != blob.payload.len()
        || (request.destination.len > 0 && request.destination.bytes.is_null())
    {
        return 0;
    }
    if !blob.payload.is_empty() {
        // SAFETY: this legacy direct copy retains neither caller pointer nor
        // storage. New generated output APIs return a borrowed byte result.
        unsafe {
            std::ptr::copy_nonoverlapping(
                blob.payload.as_ptr(),
                request.destination.bytes,
                blob.payload.len(),
            );
        }
    }
    ABI_OK
}

unsafe fn borrowed_bytes<'a>(
    value: NativeByteSlice,
    field: &'static str,
) -> Result<&'a [u8], CsharpEngineServicesError> {
    if value.len > 0 && value.bytes.is_null() {
        return Err(CsharpEngineServicesError::new(
            "CSHARP_BYTES_POINTER",
            format!("C# {field} had length without bytes"),
        ));
    }
    if value.len == 0 {
        Ok(&[])
    } else {
        // SAFETY: the C# facade pins this source span until the callback returns.
        Ok(unsafe { std::slice::from_raw_parts(value.bytes, value.len) })
    }
}

fn io_error(detail: String) -> CsharpEngineServicesError {
    CsharpEngineServicesError::new("CSHARP_PERSISTENCE_IO", detail)
}

fn handle_exhausted() -> CsharpEngineServicesError {
    CsharpEngineServicesError::new(
        "CSHARP_PERSISTENCE_HANDLES",
        "persistence handle identities are exhausted",
    )
}

fn storage_path(
    root: &Path,
    relative: &str,
    field: &str,
) -> Result<PathBuf, CsharpEngineServicesError> {
    let path = Path::new(relative);
    if relative.is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(CsharpEngineServicesError::new(
            "CSHARP_PERSISTENCE_PATH",
            format!("persistence {field} `{relative}` must be a nonempty relative path without `.` or `..`"),
        ));
    }
    Ok(root.join(path))
}

fn matches_guard(
    guard: NativePersistenceRevisionGuard,
    expected: u64,
    current: Option<&PersistenceBlob>,
) -> bool {
    match guard {
        NativePersistenceRevisionGuard::Any => true,
        NativePersistenceRevisionGuard::Exact => {
            current.is_some_and(|value| value.revision == expected)
        }
        NativePersistenceRevisionGuard::Absent => current.is_none(),
    }
}

/// Reads the stored container for `key`. A file that is not an RSP2
/// container, including a retired layout, is refused, never migrated; a
/// truncated or overlong RSP2 container is malformed. Neither is changed.
fn read_blob(path: &Path, key: &str) -> Result<Option<PersistenceBlob>, CsharpEngineServicesError> {
    let io = |error: std::io::Error| {
        io_error(format!("reading persistence key `{key}` failed: {error}"))
    };
    let malformed = |detail: &str| {
        CsharpEngineServicesError::new(
            "CSHARP_PERSISTENCE_CONTAINER_MALFORMED",
            format!("persistence key `{key}` is a damaged RSP2 container: {detail}"),
        )
    };
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(io(error)),
    };
    if bytes.len() < HEADER_MAGIC.len() || bytes[..HEADER_MAGIC.len()] != HEADER_MAGIC {
        let header = String::from_utf8_lossy(&bytes[..bytes.len().min(HEADER_MAGIC.len())])
            .escape_debug()
            .to_string();
        return Err(CsharpEngineServicesError::new(
            "CSHARP_PERSISTENCE_CONTAINER_UNRECOGNIZED",
            format!(
                "persistence key `{key}` is not an RSP2 container (it starts with \"{header}\"); the Engine does not migrate other layouts, so discard or convert the file"
            ),
        ));
    }
    if bytes.len() < HEADER_LEN {
        return Err(malformed("the header is truncated"));
    }
    let revision = u64::from_le_bytes(bytes[4..12].try_into().expect("eight header bytes"));
    let payload_len = u64::from_le_bytes(bytes[12..20].try_into().expect("eight header bytes"));
    let stored = (bytes.len() - HEADER_LEN) as u64;
    if payload_len != stored {
        return Err(malformed(&format!(
            "the header declares {payload_len} payload bytes but {stored} follow it"
        )));
    }
    let mut payload = bytes;
    payload.drain(..HEADER_LEN);
    Ok(Some(PersistenceBlob {
        present: true,
        revision,
        payload,
    }))
}

fn write_atomically(path: &Path, blob: &PersistenceBlob) -> std::io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::other("the key has no parent directory"))?;
    fs::create_dir_all(parent)?;
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| std::io::Error::other("the key has no UTF-8 file name"))?;
    let temporary = parent.join(format!(".{name}.rusty-engine-pending"));
    // A prior interrupted commit can leave only this never-published sibling.
    // The target is untouched until the later same-directory rename succeeds.
    if temporary.exists() {
        fs::remove_file(&temporary)?;
    }
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(&HEADER_MAGIC)?;
        file.write_all(&blob.revision.to_le_bytes())?;
        file.write_all(&(blob.payload.len() as u64).to_le_bytes())?;
        file.write_all(&blob.payload)?;
        file.sync_all()?;
        fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_corrupt_length_is_refused_without_sizing_an_allocation_from_it() {
        // The payload cap is gone; the declared length must fit the file.
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("corrupt");
        let mut bytes = HEADER_MAGIC.to_vec();
        bytes.extend_from_slice(&1_u64.to_le_bytes());
        bytes.extend_from_slice(&(u64::MAX / 2).to_le_bytes());
        bytes.extend_from_slice(b"short");
        std::fs::write(&path, bytes).unwrap();
        assert_eq!(
            read_blob(&path, "corrupt").unwrap_err().code(),
            "CSHARP_PERSISTENCE_CONTAINER_MALFORMED"
        );
    }

    #[test]
    fn direct_storage_round_trip_and_stale_save_preserves_committed_payload() {
        let root = tempfile::tempdir().unwrap();
        let mut bridge = RuntimePersistenceBridge::new(Some(root.path().to_path_buf()));
        let context = (&mut bridge as *mut RuntimePersistenceBridge).cast();
        let scope = b"campaign";
        let open = NativePersistenceOpenRequest {
            scope: NativeUtf8Slice {
                bytes: scope.as_ptr(),
                len: scope.len(),
            },
        };
        let mut store = NativePersistenceStoreHandle::default();
        assert_eq!(
            unsafe { open_store(context, &open, &mut store, std::ptr::null_mut()) },
            ABI_OK
        );

        let key = b"campaign.state";
        let first_payload = b"first";
        let first = NativePersistenceSaveRequest {
            store,
            key: NativeUtf8Slice {
                bytes: key.as_ptr(),
                len: key.len(),
            },
            revision_guard: NativePersistenceRevisionGuard::Absent,
            expected_revision: 0,
            payload: NativeByteSlice {
                bytes: first_payload.as_ptr(),
                len: first_payload.len(),
            },
        };
        let mut saved = NativePersistenceSaveReceipt::default();
        assert_eq!(
            unsafe { save(context, &first, &mut saved, std::ptr::null_mut()) },
            ABI_OK
        );
        assert_eq!(saved.revision, 1);

        let stale_payload = b"stale";
        let stale = NativePersistenceSaveRequest {
            payload: NativeByteSlice {
                bytes: stale_payload.as_ptr(),
                len: stale_payload.len(),
            },
            revision_guard: NativePersistenceRevisionGuard::Exact,
            expected_revision: 0,
            ..first
        };
        assert_eq!(
            unsafe { save(context, &stale, &mut saved, std::ptr::null_mut()) },
            ABI_OK
        );
        assert_eq!(
            saved.outcome,
            NativePersistenceSaveOutcome::RevisionConflict
        );
        assert_eq!(saved.revision, 1);
        assert_eq!(
            unsafe { save(context, &first, &mut saved, std::ptr::null_mut()) },
            ABI_OK
        );
        assert_eq!(
            saved.outcome,
            NativePersistenceSaveOutcome::RevisionConflict
        );

        let load_request = NativePersistenceLoadRequest {
            store,
            key: NativeUtf8Slice {
                bytes: key.as_ptr(),
                len: key.len(),
            },
        };
        let mut blob = NativePersistenceBlobHandle::default();
        assert_eq!(
            unsafe { load(context, &load_request, &mut blob, std::ptr::null_mut()) },
            ABI_OK
        );
        let mut info = NativePersistenceBlobInfo {
            present: false,
            revision: 0,
            payload_len: 0,
        };
        assert_eq!(unsafe { describe_blob(context, blob, &mut info) }, ABI_OK);
        assert!(info.present);
        assert_eq!((info.revision, info.payload_len), (1, 5));
        let mut copied = vec![0; info.payload_len];
        let copy = NativePersistenceCopyBlobRequest {
            blob,
            destination: NativeWritableByteSlice {
                bytes: copied.as_mut_ptr(),
                len: copied.len(),
            },
        };
        assert_eq!(unsafe { copy_blob(context, &copy) }, ABI_OK);
        assert_eq!(copied, first_payload);
        let mut bytes_result = NativeByteResult {
            bytes: std::ptr::null(),
            len: 0,
        };
        assert_eq!(
            unsafe { read_blob_bytes(context, blob, &mut bytes_result) },
            ABI_OK
        );
        let borrowed_bytes =
            unsafe { std::slice::from_raw_parts(bytes_result.bytes, bytes_result.len) };
        assert_eq!(borrowed_bytes, first_payload);

        assert_eq!(unsafe { destroy_blob(context, blob) }, ABI_OK);
        assert_eq!(unsafe { destroy_store(context, store) }, ABI_OK);
        let path = root.path().join("campaign/campaign.state");
        assert!(path.is_file());
        let bytes = fs::read(&path).unwrap();
        assert_eq!(bytes.len(), HEADER_LEN + first_payload.len());
        assert_eq!(&bytes[..4], b"RSP2");
        // Reopen through a fresh bridge: the roundtrip must use disk, not live handles.
        drop(bridge);
        let mut reopened = RuntimePersistenceBridge::new(Some(root.path().to_path_buf()));
        let context = (&mut reopened as *mut RuntimePersistenceBridge).cast();
        assert_eq!(
            unsafe { open_store(context, &open, &mut store, std::ptr::null_mut()) },
            ABI_OK
        );
        let request = NativePersistenceLoadRequest {
            store,
            ..load_request
        };
        assert_eq!(
            unsafe { load(context, &request, &mut blob, std::ptr::null_mut()) },
            ABI_OK
        );
        assert_eq!(unsafe { describe_blob(context, blob, &mut info) }, ABI_OK);
        assert_eq!((info.revision, info.payload_len), (1, first_payload.len()));
        let copy = NativePersistenceCopyBlobRequest { blob, ..copy };
        copied.fill(0);
        assert_eq!(unsafe { copy_blob(context, &copy) }, ABI_OK);
        assert_eq!(copied, first_payload);
        assert_eq!(unsafe { destroy_blob(context, blob) }, ABI_OK);
        assert_eq!(unsafe { destroy_store(context, store) }, ABI_OK);
    }

    #[test]
    fn deletion_is_guarded_durable_scoped_and_preserves_retained_blobs() {
        let root = tempfile::tempdir().unwrap();
        let mut bridge = RuntimePersistenceBridge::new(Some(root.path().to_path_buf()));
        let context = (&mut bridge as *mut RuntimePersistenceBridge).cast();
        fn utf8(value: &str) -> NativeUtf8Slice {
            NativeUtf8Slice {
                bytes: value.as_ptr(),
                len: value.len(),
            }
        }
        let open = NativePersistenceOpenRequest {
            scope: utf8("campaign"),
        };
        let mut store = NativePersistenceStoreHandle::default();
        assert_eq!(
            unsafe { open_store(context, &open, &mut store, std::ptr::null_mut()) },
            ABI_OK
        );
        let first = NativePersistenceSaveRequest {
            store,
            key: utf8("nested/slot"),
            revision_guard: NativePersistenceRevisionGuard::Any,
            expected_revision: 0,
            payload: NativeByteSlice {
                bytes: b"saved".as_ptr(),
                len: 5,
            },
        };
        let mut saved = NativePersistenceSaveReceipt::default();
        assert_eq!(
            unsafe { save(context, &first, &mut saved, std::ptr::null_mut()) },
            ABI_OK
        );
        assert_eq!(
            unsafe {
                save(
                    context,
                    &NativePersistenceSaveRequest {
                        key: utf8("other"),
                        ..first
                    },
                    &mut saved,
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        let mut other_store = NativePersistenceStoreHandle::default();
        assert_eq!(
            unsafe {
                open_store(
                    context,
                    &NativePersistenceOpenRequest {
                        scope: utf8("elsewhere"),
                    },
                    &mut other_store,
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        assert_eq!(
            unsafe {
                save(
                    context,
                    &NativePersistenceSaveRequest {
                        store: other_store,
                        ..first
                    },
                    &mut saved,
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        let load_request = NativePersistenceLoadRequest {
            store,
            key: first.key,
        };
        let mut retained = NativePersistenceBlobHandle::default();
        assert_eq!(
            unsafe { load(context, &load_request, &mut retained, std::ptr::null_mut()) },
            ABI_OK
        );
        let request = NativePersistenceDeleteRequest {
            store,
            key: first.key,
            revision_guard: NativePersistenceRevisionGuard::Exact,
            expected_revision: 2,
        };
        let mut receipt = NativePersistenceDeleteReceipt::default();
        assert_eq!(
            unsafe { delete(context, &request, &mut receipt, std::ptr::null_mut()) },
            ABI_OK
        );
        assert_eq!(
            receipt.outcome,
            NativePersistenceDeleteOutcome::RevisionConflict
        );
        assert_eq!(receipt.revision, 1);
        assert!(root.path().join("campaign/nested/slot").exists());
        assert_eq!(
            unsafe {
                delete(
                    context,
                    &NativePersistenceDeleteRequest {
                        revision_guard: NativePersistenceRevisionGuard::Absent,
                        ..request
                    },
                    &mut receipt,
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        assert_eq!(
            receipt.outcome,
            NativePersistenceDeleteOutcome::RevisionConflict
        );
        let request = NativePersistenceDeleteRequest {
            expected_revision: 1,
            ..request
        };
        assert_eq!(
            unsafe { delete(context, &request, &mut receipt, std::ptr::null_mut()) },
            ABI_OK
        );
        assert_eq!(receipt.outcome, NativePersistenceDeleteOutcome::Deleted);
        assert_eq!(receipt.revision, 1);
        assert!(!root.path().join("campaign/nested/slot").exists());
        assert_eq!(bridge.blobs[&retained.value].payload, b"saved");
        assert_eq!(
            unsafe { delete(context, &request, &mut receipt, std::ptr::null_mut()) },
            ABI_OK
        );
        assert_eq!(
            receipt.outcome,
            NativePersistenceDeleteOutcome::RevisionConflict
        );
        assert_eq!(receipt.revision, 0);
        for guard in [
            NativePersistenceRevisionGuard::Any,
            NativePersistenceRevisionGuard::Absent,
        ] {
            assert_eq!(
                unsafe {
                    delete(
                        context,
                        &NativePersistenceDeleteRequest {
                            revision_guard: guard,
                            ..request
                        },
                        &mut receipt,
                        std::ptr::null_mut(),
                    )
                },
                ABI_OK
            );
            assert_eq!(receipt.outcome, NativePersistenceDeleteOutcome::Missing);
        }
        assert_eq!(unsafe { destroy_store(context, store) }, ABI_OK);
        receipt = NativePersistenceDeleteReceipt::default();
        assert_eq!(
            unsafe { delete(context, &request, &mut receipt, std::ptr::null_mut()) },
            0
        );
        assert_ne!(receipt.outcome, NativePersistenceDeleteOutcome::Deleted);
        drop(bridge);

        let mut reopened = RuntimePersistenceBridge::new(Some(root.path().to_path_buf()));
        let context = (&mut reopened as *mut RuntimePersistenceBridge).cast();
        assert_eq!(
            unsafe { open_store(context, &open, &mut store, std::ptr::null_mut()) },
            ABI_OK
        );
        let mut blob = NativePersistenceBlobHandle::default();
        assert_eq!(
            unsafe {
                load(
                    context,
                    &NativePersistenceLoadRequest {
                        store,
                        ..load_request
                    },
                    &mut blob,
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        assert!(!reopened.blobs[&blob.value].present);
        assert!(read_blob(&root.path().join("campaign/other"), "other")
            .unwrap()
            .is_some());
        assert!(
            read_blob(&root.path().join("elsewhere/nested/slot"), "nested/slot")
                .unwrap()
                .is_some()
        );
        // Invalid paths and unreadable/corrupt storage cannot produce a success receipt.
        fs::write(root.path().join("campaign/broken"), b"invalid header").unwrap();
        for key in ["../escape", "broken", "nested"] {
            receipt = NativePersistenceDeleteReceipt::default();
            assert_eq!(
                unsafe {
                    delete(
                        context,
                        &NativePersistenceDeleteRequest {
                            store,
                            key: utf8(key),
                            revision_guard: NativePersistenceRevisionGuard::Any,
                            ..request
                        },
                        &mut receipt,
                        std::ptr::null_mut(),
                    )
                },
                0
            );
            assert_ne!(receipt.outcome, NativePersistenceDeleteOutcome::Deleted);
        }
        assert_eq!(
            fs::read(root.path().join("campaign/broken")).unwrap(),
            b"invalid header"
        );
    }

    #[test]
    fn retired_malformed_and_unreadable_storage_are_refused_with_codes_and_left_untouched() {
        use crate::operation_diagnostics::{empty_receipt, receipt_codes};
        use std::os::unix::fs::PermissionsExt;
        fn utf8(value: &str) -> NativeUtf8Slice {
            NativeUtf8Slice {
                bytes: value.as_ptr(),
                len: value.len(),
            }
        }
        let root = tempfile::tempdir().unwrap();
        let mut bridge = RuntimePersistenceBridge::new(Some(root.path().to_path_buf()));
        let context = (&mut bridge as *mut RuntimePersistenceBridge).cast();
        let mut store = NativePersistenceStoreHandle::default();
        let mut refusal = empty_receipt();
        let open = NativePersistenceOpenRequest {
            scope: utf8("slots"),
        };
        assert_eq!(
            unsafe { open_store(context, &open, &mut store, &mut refusal) },
            ABI_OK
        );
        let scope = root.path().join("slots");
        let mut rsp2 = b"RSP2".to_vec();
        rsp2.extend_from_slice(&7_u64.to_le_bytes());
        rsp2.extend_from_slice(&3_u64.to_le_bytes());
        let files: [(&str, Vec<u8>); 5] = [
            ("retired", b"RSP1\x01\x00\x00\x00old save".to_vec()),
            ("tiny", b"RS".to_vec()),
            ("truncated-header", b"RSP2\x01".to_vec()),
            ("short-payload", [rsp2.as_slice(), b"ab"].concat()),
            ("trailing-payload", [rsp2.as_slice(), b"abcd"].concat()),
        ];
        for (key, bytes) in &files {
            fs::write(scope.join(key), bytes).unwrap();
        }
        fs::create_dir(scope.join("directory")).unwrap();
        let expected = [
            ("retired", "CSHARP_PERSISTENCE_CONTAINER_UNRECOGNIZED"),
            ("tiny", "CSHARP_PERSISTENCE_CONTAINER_UNRECOGNIZED"),
            ("truncated-header", "CSHARP_PERSISTENCE_CONTAINER_MALFORMED"),
            ("short-payload", "CSHARP_PERSISTENCE_CONTAINER_MALFORMED"),
            ("trailing-payload", "CSHARP_PERSISTENCE_CONTAINER_MALFORMED"),
            ("directory", "CSHARP_PERSISTENCE_IO"),
        ];
        for (key, code) in expected {
            let save_request = NativePersistenceSaveRequest {
                store,
                key: utf8(key),
                revision_guard: NativePersistenceRevisionGuard::Any,
                expected_revision: 0,
                payload: NativeByteSlice {
                    bytes: b"new".as_ptr(),
                    len: 3,
                },
            };
            let mut saved = NativePersistenceSaveReceipt::default();
            refusal = empty_receipt();
            assert_eq!(
                unsafe { save(context, &save_request, &mut saved, &mut refusal) },
                0
            );
            assert_eq!(receipt_codes(&refusal), [code], "save {key}");
            let mut blob = NativePersistenceBlobHandle::default();
            let load_request = NativePersistenceLoadRequest {
                store,
                key: utf8(key),
            };
            assert_eq!(
                unsafe { load(context, &load_request, &mut blob, &mut refusal) },
                0
            );
            assert_eq!(receipt_codes(&refusal), [code], "load {key}");
            let delete_request = NativePersistenceDeleteRequest {
                store,
                key: utf8(key),
                revision_guard: NativePersistenceRevisionGuard::Any,
                expected_revision: 0,
            };
            let mut deleted = NativePersistenceDeleteReceipt::default();
            assert_eq!(
                unsafe { delete(context, &delete_request, &mut deleted, &mut refusal) },
                0
            );
            assert_eq!(receipt_codes(&refusal), [code], "delete {key}");
        }
        for (key, bytes) in &files {
            assert_eq!(
                &fs::read(scope.join(key)).unwrap(),
                bytes,
                "{key} was changed"
            );
        }
        assert!(scope.join("directory").is_dir());

        // A write failure keeps the committed revision and payload.
        let committed = NativePersistenceSaveRequest {
            store,
            key: utf8("committed"),
            revision_guard: NativePersistenceRevisionGuard::Any,
            expected_revision: 0,
            payload: NativeByteSlice {
                bytes: b"kept".as_ptr(),
                len: 4,
            },
        };
        let mut saved = NativePersistenceSaveReceipt::default();
        assert_eq!(
            unsafe { save(context, &committed, &mut saved, &mut refusal) },
            ABI_OK
        );
        let before = fs::read(scope.join("committed")).unwrap();
        fs::set_permissions(&scope, fs::Permissions::from_mode(0o500)).unwrap();
        // A privileged user writes through permissions; there is no failure to observe.
        let writable = fs::write(scope.join("probe"), b"").is_ok();
        if !writable {
            assert_eq!(
                unsafe { save(context, &committed, &mut saved, &mut refusal) },
                0
            );
            assert_eq!(receipt_codes(&refusal), ["CSHARP_PERSISTENCE_IO"]);
        }
        fs::set_permissions(&scope, fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(fs::read(scope.join("committed")).unwrap(), before);
        let mut blob = NativePersistenceBlobHandle::default();
        let load_request = NativePersistenceLoadRequest {
            store,
            key: utf8("committed"),
        };
        assert_eq!(
            unsafe { load(context, &load_request, &mut blob, &mut refusal) },
            ABI_OK
        );
        assert_eq!(bridge.blobs[&blob.value].revision, 1);
        assert_eq!(bridge.blobs[&blob.value].payload, b"kept");
    }

    #[test]
    fn open_store_requires_host_root_and_rejects_escape_scopes() {
        let scope = b"campaign";
        let request = NativePersistenceOpenRequest {
            scope: NativeUtf8Slice {
                bytes: scope.as_ptr(),
                len: scope.len(),
            },
        };
        let mut store = NativePersistenceStoreHandle::default();
        let mut unconfigured = RuntimePersistenceBridge::new(None);
        let unconfigured_context = (&mut unconfigured as *mut RuntimePersistenceBridge).cast();
        assert_eq!(
            unsafe {
                open_store(
                    unconfigured_context,
                    &request,
                    &mut store,
                    std::ptr::null_mut(),
                )
            },
            0
        );

        let root = tempfile::tempdir().unwrap();
        let mut bridge = RuntimePersistenceBridge::new(Some(root.path().to_path_buf()));
        let context = (&mut bridge as *mut RuntimePersistenceBridge).cast();
        let escape = b"../outside";
        let request = NativePersistenceOpenRequest {
            scope: NativeUtf8Slice {
                bytes: escape.as_ptr(),
                len: escape.len(),
            },
        };
        assert_eq!(
            unsafe { open_store(context, &request, &mut store, std::ptr::null_mut()) },
            0
        );
        assert!(!root.path().join("outside").exists());
    }
}
