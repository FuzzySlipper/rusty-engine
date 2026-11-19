//! Durable opaque product-byte storage behind the generated NativeAOT table.
//!
//! Callers supply opaque bytes. This owner supplies relative-key admission,
//! revisions, atomic replacement, failure preservation, and retained blob lifetime.
//! The header magic identifies only the storage layout; payload formats belong
//! to their codecs, not this service.
//!
//! A deferred save is decided in the call and written on one
//! `rusty-persistence` thread, in the order saves were made. The thread
//! shares no lock with the caller: jobs go to it over a channel and come
//! back finished over another, taken in at the next persistence call, so the
//! caller frees each payload and never waits on the worker. Until a key's
//! write finishes its newest payload is held here, so `Load` and later
//! guards see it, and each key's last known revision is kept so a save of
//! it reads no file. Dropping the bridge writes every queued save first.

use std::{
    collections::{BTreeMap, HashMap},
    ffi::c_void,
    fs::{self, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc,
    },
    thread::JoinHandle,
};

use csharp_engine_abi::*;

use crate::{
    composition::{borrowed_utf8, ABI_OK},
    operation_diagnostics::{clear_receipt, OperationDiagnostics},
    CsharpEngineServicesError,
};

/// The durable layout: magic, revision, payload length, payload.
const HEADER_MAGIC: [u8; 4] = *b"RSP2";
const HEADER_LEN: usize = 4 + 8 + 8;
/// The cache layout: the durable header plus a CRC-32 of the payload. A cache
/// write is not synced, so after a crash a file may hold a full-length blob
/// whose tail never reached the disk; the CRC refuses it.
const CACHE_MAGIC: [u8; 4] = *b"RSC1";
const CACHE_HEADER_LEN: usize = HEADER_LEN + 4;

#[derive(Debug)]
struct DurableStore {
    root: PathBuf,
    cache: bool,
}

/// The worker thread and its two channels.
struct SaveWorker {
    jobs: Sender<SaveJob>,
    finished: Receiver<(SaveJob, std::io::Result<()>)>,
    thread: JoinHandle<()>,
}

struct PendingKey {
    revision: u64,
    payload: Arc<[u8]>,
    /// The queue position of the key's last queued write.
    sequence: u64,
}

#[derive(Default)]
struct StoreSaves {
    readout: NativePersistenceSaveReadout,
    failure: String,
}

struct SaveJob {
    store: u64,
    ticket: u64,
    sequence: u64,
    path: PathBuf,
    key: String,
    revision: u64,
    payload: Arc<[u8]>,
    cache: bool,
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
    /// Started by the first `open_store`.
    worker: Option<SaveWorker>,
    issued: u64,
    finished: u64,
    /// The newest queued payload of each key whose writes are not finished.
    pending: HashMap<PathBuf, PendingKey>,
    /// Per open store: its tickets and failures.
    saves: HashMap<u64, StoreSaves>,
    /// Held by a test to keep the worker from writing.
    #[cfg(test)]
    gate: Arc<std::sync::Mutex<()>>,
    /// The failure text a `read_save_failure` result borrows.
    failure_text: String,
    /// The revision this runtime last loaded, saved or deleted for each key,
    /// zero when absent. A save of a known key reads no file: a read in the
    /// call can wait behind the worker's writes in the same directory.
    known: HashMap<PathBuf, u64>,
}

impl Drop for RuntimePersistenceBridge {
    /// Writes every queued save, then stops the worker.
    fn drop(&mut self) {
        if let Some(SaveWorker { jobs, thread, .. }) = self.worker.take() {
            // The worker writes what is queued, then sees the channel close.
            drop(jobs);
            let _ = thread.join();
        }
    }
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
            worker: None,
            issued: 0,
            finished: 0,
            pending: HashMap::new(),
            saves: HashMap::new(),
            #[cfg(test)]
            gate: Arc::default(),
            failure_text: String::new(),
            known: HashMap::new(),
        }
    }

    /// Takes in the writes the worker has finished; with `wait`, until every
    /// queued write has, so a synchronous save or delete lands after them.
    fn take_finished(&mut self, wait: bool) {
        let Some(worker) = &self.worker else {
            return;
        };
        while self.finished < self.issued {
            let received = if wait {
                worker.finished.recv().ok()
            } else {
                worker.finished.try_recv().ok()
            };
            let Some((job, result)) = received else {
                return;
            };
            self.finished += 1;
            if let Some(saves) = self.saves.get_mut(&job.store) {
                saves.readout.finished = job.ticket;
                if let Err(error) = result {
                    saves.readout.failed += 1;
                    saves.readout.last_failed = job.ticket;
                    saves.failure = format!(
                        "CSHARP_PERSISTENCE_IO: writing persistence key `{}` (ticket {}) failed: {error}",
                        job.key, job.ticket
                    );
                }
            }
            if self
                .pending
                .get(&job.path)
                .is_some_and(|pending| pending.sequence == job.sequence)
            {
                self.pending.remove(&job.path);
            }
        }
    }

    fn start_worker(&mut self) -> Result<(), CsharpEngineServicesError> {
        if self.worker.is_none() {
            let (jobs, queued) = mpsc::channel();
            let (written, finished) = mpsc::channel();
            #[cfg(test)]
            let gate = self.gate.clone();
            let thread = std::thread::Builder::new()
                .name("rusty-persistence".to_owned())
                .spawn(move || {
                    for job in queued {
                        let job: SaveJob = job;
                        #[cfg(test)]
                        let held = gate.lock();
                        let result =
                            write_atomically(&job.path, job.revision, &job.payload, job.cache);
                        #[cfg(test)]
                        drop(held);
                        // The job goes back to be freed by the caller's thread.
                        if written.send((job, result)).is_err() {
                            return;
                        }
                    }
                })
                .map_err(|error| {
                    io_error(format!("the persistence worker did not start: {error}"))
                })?;
            self.worker = Some(SaveWorker {
                jobs,
                finished,
                thread,
            });
        }
        Ok(())
    }

    fn read_saves(
        &mut self,
        store: NativePersistenceStoreHandle,
    ) -> Result<NativePersistenceSaveReadout, CsharpEngineServicesError> {
        if !self.stores.contains_key(&store.value) {
            return Err(store_not_open());
        }
        self.take_finished(false);
        Ok(self
            .saves
            .get(&store.value)
            .map(|saves| saves.readout)
            .unwrap_or_default())
    }

    fn read_save_failure(
        &mut self,
        store: NativePersistenceStoreHandle,
    ) -> Result<NativeByteResult, CsharpEngineServicesError> {
        if !self.stores.contains_key(&store.value) {
            return Err(store_not_open());
        }
        self.take_finished(false);
        self.failure_text = self
            .saves
            .get(&store.value)
            .map(|saves| saves.failure.clone())
            .unwrap_or_default();
        Ok(NativeByteResult {
            bytes: self.failure_text.as_ptr(),
            len: self.failure_text.len(),
        })
    }

    fn insert_store(&mut self, store: DurableStore) -> NativePersistenceStoreHandle {
        let value = self.next_store;
        self.next_store += 1;
        self.stores.insert(value, store);
        NativePersistenceStoreHandle { value }
    }

    fn insert_blob(&mut self, blob: PersistenceBlob) -> NativePersistenceBlobHandle {
        let value = self.next_blob;
        self.next_blob += 1;
        self.blobs.insert(value, blob);
        NativePersistenceBlobHandle { value }
    }

    /// The key's path, its text and whether its store is a cache.
    fn store_path(
        &self,
        store: NativePersistenceStoreHandle,
        key: NativeUtf8Slice,
    ) -> Result<(PathBuf, String, bool), CsharpEngineServicesError> {
        let key = unsafe { borrowed_utf8(key.bytes, key.len, "persistence key") }?;
        let store = self.stores.get(&store.value).ok_or_else(store_not_open)?;
        Ok((
            storage_path(&store.root, key, "key")?,
            key.to_owned(),
            store.cache,
        ))
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
        // Started here, so no save pays for it.
        self.start_worker()?;
        Ok(self.insert_store(DurableStore {
            root,
            cache: request.cache,
        }))
    }

    /// The key's newest state: its queued payload while a write is pending,
    /// else what is stored. In a cache store a damaged blob reads as absent:
    /// a crash may leave one, and the cache regenerates it.
    fn current(
        &mut self,
        path: &Path,
        key: &str,
        cache: bool,
    ) -> Result<Option<PersistenceBlob>, CsharpEngineServicesError> {
        self.take_finished(false);
        if let Some(pending) = self.pending.get(path) {
            return Ok(Some(PersistenceBlob {
                present: true,
                revision: pending.revision,
                payload: pending.payload.to_vec(),
            }));
        }
        let blob = match read_blob(path, key) {
            Err(error)
                if cache
                    && matches!(
                        error.code(),
                        "CSHARP_PERSISTENCE_CONTAINER_MALFORMED"
                            | "CSHARP_PERSISTENCE_CONTAINER_UNRECOGNIZED"
                    ) =>
            {
                None
            }
            read => read?,
        };
        self.known.insert(
            path.to_owned(),
            blob.as_ref().map_or(0, |blob| blob.revision),
        );
        Ok(blob)
    }

    /// The key's newest revision, zero when absent, read from the file only
    /// for a key this runtime has not loaded, saved or deleted.
    fn current_revision(
        &mut self,
        path: &Path,
        key: &str,
        cache: bool,
    ) -> Result<u64, CsharpEngineServicesError> {
        match self.known.get(path) {
            Some(revision) => Ok(*revision),
            None => Ok(self
                .current(path, key, cache)?
                .map_or(0, |blob| blob.revision)),
        }
    }

    fn save(
        &mut self,
        request: &NativePersistenceSaveRequest,
    ) -> Result<NativePersistenceSaveReceipt, CsharpEngineServicesError> {
        let (path, key, cache) = self.store_path(request.store, request.key)?;
        let payload = unsafe { borrowed_bytes(request.payload, "persistence payload") }?;
        self.take_finished(!request.deferred);
        let current = self.current_revision(&path, &key, cache)?;
        if !matches_guard(request.revision_guard, request.expected_revision, current) {
            return Ok(NativePersistenceSaveReceipt {
                outcome: NativePersistenceSaveOutcome::RevisionConflict,
                revision: current,
                ticket: 0,
            });
        }
        let revision = current.checked_add(1).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_PERSISTENCE_REVISION",
                format!("persistence key `{key}` has no revision after u64::MAX"),
            )
        })?;
        if !request.deferred {
            write_atomically(&path, revision, payload, cache).map_err(|error| {
                io_error(format!("writing persistence key `{key}` failed: {error}"))
            })?;
            self.known.insert(path, revision);
            return Ok(NativePersistenceSaveReceipt {
                outcome: NativePersistenceSaveOutcome::Saved,
                revision,
                ticket: 0,
            });
        }
        let payload: Arc<[u8]> = Arc::from(payload);
        let worker = self
            .worker
            .as_ref()
            .ok_or_else(|| io_error("the persistence worker is not running".to_owned()))?;
        let sequence = self.issued + 1;
        let ticket = self
            .saves
            .get(&request.store.value)
            .map_or(0, |saves| saves.readout.queued)
            + 1;
        worker
            .jobs
            .send(SaveJob {
                store: request.store.value,
                ticket,
                sequence,
                path: path.clone(),
                key,
                revision,
                payload: payload.clone(),
                cache,
            })
            .map_err(|_| io_error("the persistence worker stopped".to_owned()))?;
        self.issued = sequence;
        self.saves
            .entry(request.store.value)
            .or_default()
            .readout
            .queued = ticket;
        self.pending.insert(
            path.clone(),
            PendingKey {
                revision,
                payload,
                sequence,
            },
        );
        self.known.insert(path, revision);
        Ok(NativePersistenceSaveReceipt {
            outcome: NativePersistenceSaveOutcome::Queued,
            revision,
            ticket,
        })
    }

    fn delete(
        &mut self,
        request: &NativePersistenceDeleteRequest,
    ) -> Result<NativePersistenceDeleteReceipt, CsharpEngineServicesError> {
        let (path, key, cache) = self.store_path(request.store, request.key)?;
        self.take_finished(true);
        let revision = self
            .current(&path, &key, cache)?
            .map_or(0, |blob| blob.revision);
        let outcome = if !matches_guard(request.revision_guard, request.expected_revision, revision)
        {
            NativePersistenceDeleteOutcome::RevisionConflict
        } else if revision == 0 {
            // A cache store's damaged blob reads as absent; it goes too.
            if cache && path.exists() {
                remove_key(&path, cache).map_err(|error| {
                    io_error(format!("deleting persistence key `{key}` failed: {error}"))
                })?;
            }
            NativePersistenceDeleteOutcome::Missing
        } else {
            remove_key(&path, cache).map_err(|error| {
                io_error(format!("deleting persistence key `{key}` failed: {error}"))
            })?;
            self.known.insert(path, 0);
            NativePersistenceDeleteOutcome::Deleted
        };
        Ok(NativePersistenceDeleteReceipt { outcome, revision })
    }

    fn load(
        &mut self,
        request: &NativePersistenceLoadRequest,
    ) -> Result<NativePersistenceBlobHandle, CsharpEngineServicesError> {
        let (path, key, cache) = self.store_path(request.store, request.key)?;
        let blob = self
            .current(&path, &key, cache)?
            .unwrap_or(PersistenceBlob {
                present: false,
                revision: 0,
                payload: Vec::new(),
            });
        Ok(self.insert_blob(blob))
    }
}

fn store_not_open() -> CsharpEngineServicesError {
    CsharpEngineServicesError::new(
        "CSHARP_PERSISTENCE_STORE",
        "the persistence store handle is not open",
    )
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
        read_saves,
        read_save_failure,
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
        // Its queued saves are still written; only their readout goes.
        bridge.saves.remove(&store.value);
        ABI_OK
    } else {
        0
    }
}

unsafe extern "C" fn read_saves(
    context: *mut c_void,
    store: NativePersistenceStoreHandle,
    result: *mut NativePersistenceSaveReadout,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe {
        refusable(context, &store, result, receipt, |bridge, store| {
            bridge.read_saves(*store)
        })
    }
}

unsafe extern "C" fn read_save_failure(
    context: *mut c_void,
    store: NativePersistenceStoreHandle,
    result: *mut NativeByteResult,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe {
        refusable(context, &store, result, receipt, |bridge, store| {
            bridge.read_save_failure(*store)
        })
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

/// Removes a key's file. For a durable store on Unix, it then flushes the
/// directory entry's removal before success is reported; a failed flush is an
/// operation failure, not a deletion receipt. A cache store skips the flush.
/// Windows opens no directory as a file (std's `File::open` on one is
/// access-denied), and NTFS journals the entry itself, so there the removal
/// is the whole operation.
fn remove_key(path: &Path, cache: bool) -> std::io::Result<()> {
    fs::remove_file(path)?;
    if cache {
        return Ok(());
    }
    sync_parent(path)
}

/// Flushes the directory that holds `path`, so a rename or removal in it
/// survives a crash. Unix only; see `remove_key`.
fn sync_parent(path: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        path.parent()
            .ok_or_else(|| std::io::Error::other("the key has no parent directory"))
            .and_then(fs::File::open)?
            .sync_all()
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(())
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

/// Whether a guard admits the key's current revision, zero when absent.
fn matches_guard(guard: NativePersistenceRevisionGuard, expected: u64, current: u64) -> bool {
    match guard {
        NativePersistenceRevisionGuard::Any => true,
        NativePersistenceRevisionGuard::Exact => current != 0 && current == expected,
        NativePersistenceRevisionGuard::Absent => current == 0,
    }
}

/// Reads the stored container for `key`, in either layout. A file that is
/// neither, including a retired layout, is refused, never migrated; a
/// truncated or overlong container, or a cache blob whose CRC disagrees, is
/// malformed. Neither is changed.
fn read_blob(path: &Path, key: &str) -> Result<Option<PersistenceBlob>, CsharpEngineServicesError> {
    let io = |error: std::io::Error| {
        io_error(format!("reading persistence key `{key}` failed: {error}"))
    };
    let malformed = |detail: &str| {
        CsharpEngineServicesError::new(
            "CSHARP_PERSISTENCE_CONTAINER_MALFORMED",
            format!("persistence key `{key}` is a damaged container: {detail}"),
        )
    };
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(io(error)),
    };
    let magic = &bytes[..bytes.len().min(HEADER_MAGIC.len())];
    let header_len = if magic == HEADER_MAGIC {
        HEADER_LEN
    } else if magic == CACHE_MAGIC {
        CACHE_HEADER_LEN
    } else {
        let header = String::from_utf8_lossy(magic).escape_debug().to_string();
        return Err(CsharpEngineServicesError::new(
            "CSHARP_PERSISTENCE_CONTAINER_UNRECOGNIZED",
            format!(
                "persistence key `{key}` is not an RSP2 or RSC1 container (it starts with \"{header}\"); the Engine does not migrate other layouts, so discard or convert the file"
            ),
        ));
    };
    if bytes.len() < header_len {
        return Err(malformed("the header is truncated"));
    }
    let revision = u64::from_le_bytes(bytes[4..12].try_into().expect("eight header bytes"));
    let payload_len = u64::from_le_bytes(bytes[12..20].try_into().expect("eight header bytes"));
    let stored = (bytes.len() - header_len) as u64;
    if payload_len != stored {
        return Err(malformed(&format!(
            "the header declares {payload_len} payload bytes but {stored} follow it"
        )));
    }
    if header_len == CACHE_HEADER_LEN {
        let crc = u32::from_le_bytes(bytes[20..24].try_into().expect("four header bytes"));
        if crc32fast::hash(&bytes[header_len..]) != crc {
            return Err(malformed("the payload does not match its CRC"));
        }
    }
    let mut payload = bytes;
    payload.drain(..header_len);
    Ok(Some(PersistenceBlob {
        present: true,
        revision,
        payload,
    }))
}

/// Replaces the key's file through a synced temporary file and a rename,
/// then syncs the directory, so a finished write survives a crash. A cache
/// write skips both syncs and adds the payload's CRC to its header.
fn write_atomically(
    path: &Path,
    revision: u64,
    payload: &[u8],
    cache: bool,
) -> std::io::Result<()> {
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
        let mut header = Vec::with_capacity(CACHE_HEADER_LEN);
        header.extend_from_slice(if cache { &CACHE_MAGIC } else { &HEADER_MAGIC });
        header.extend_from_slice(&revision.to_le_bytes());
        header.extend_from_slice(&(payload.len() as u64).to_le_bytes());
        if cache {
            header.extend_from_slice(&crc32fast::hash(payload).to_le_bytes());
        }
        file.write_all(&header)?;
        file.write_all(payload)?;
        if !cache {
            file.sync_all()?;
        }
        fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
        return result;
    }
    if cache {
        Ok(())
    } else {
        sync_parent(path)
    }
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
            cache: false,
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
            deferred: false,
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
            cache: false,
            scope: utf8("campaign"),
        };
        let mut store = NativePersistenceStoreHandle::default();
        assert_eq!(
            unsafe { open_store(context, &open, &mut store, std::ptr::null_mut()) },
            ABI_OK
        );
        let first = NativePersistenceSaveRequest {
            deferred: false,
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
                        cache: false,
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
            cache: false,
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
                deferred: false,
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
            deferred: false,
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
        // An unwritable scope (Unix permissions) refuses the save.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
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
        }
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
            cache: false,
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
            cache: false,
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

    mod queued {
        use super::*;
        use crate::operation_diagnostics::{empty_receipt, receipt_codes};

        fn utf8(value: &str) -> NativeUtf8Slice {
            NativeUtf8Slice {
                bytes: value.as_ptr(),
                len: value.len(),
            }
        }

        fn context(bridge: &mut RuntimePersistenceBridge) -> *mut c_void {
            (bridge as *mut RuntimePersistenceBridge).cast()
        }

        fn open(
            bridge: &mut RuntimePersistenceBridge,
            scope: &str,
            cache: bool,
        ) -> NativePersistenceStoreHandle {
            let mut store = NativePersistenceStoreHandle::default();
            let request = NativePersistenceOpenRequest {
                scope: utf8(scope),
                cache,
            };
            assert_eq!(
                unsafe { open_store(context(bridge), &request, &mut store, std::ptr::null_mut()) },
                ABI_OK
            );
            store
        }

        fn save_key(
            bridge: &mut RuntimePersistenceBridge,
            store: NativePersistenceStoreHandle,
            key: &str,
            guard: (NativePersistenceRevisionGuard, u64),
            payload: &[u8],
            deferred: bool,
        ) -> NativePersistenceSaveReceipt {
            let request = NativePersistenceSaveRequest {
                store,
                key: utf8(key),
                revision_guard: guard.0,
                expected_revision: guard.1,
                payload: NativeByteSlice {
                    bytes: payload.as_ptr(),
                    len: payload.len(),
                },
                deferred,
            };
            let mut receipt = NativePersistenceSaveReceipt::default();
            let mut refusal = empty_receipt();
            assert_eq!(
                unsafe { save(context(bridge), &request, &mut receipt, &mut refusal) },
                ABI_OK,
                "{:?}",
                receipt_codes(&refusal)
            );
            receipt
        }

        /// The key's present revision and payload, or the refusal's code.
        fn load_key(
            bridge: &mut RuntimePersistenceBridge,
            store: NativePersistenceStoreHandle,
            key: &str,
        ) -> Result<(u64, Vec<u8>), String> {
            let request = NativePersistenceLoadRequest {
                store,
                key: utf8(key),
            };
            let mut blob = NativePersistenceBlobHandle::default();
            let mut refusal = empty_receipt();
            if unsafe { load(context(bridge), &request, &mut blob, &mut refusal) } != ABI_OK {
                return Err(receipt_codes(&refusal)[0].to_owned());
            }
            let blob = bridge.blobs.remove(&blob.value).unwrap();
            Ok((blob.revision, blob.payload))
        }

        fn readout(
            bridge: &mut RuntimePersistenceBridge,
            store: NativePersistenceStoreHandle,
        ) -> NativePersistenceSaveReadout {
            let mut value = NativePersistenceSaveReadout::default();
            assert_eq!(
                unsafe { read_saves(context(bridge), store, &mut value, std::ptr::null_mut()) },
                ABI_OK
            );
            value
        }

        const ANY: (NativePersistenceRevisionGuard, u64) = (NativePersistenceRevisionGuard::Any, 0);

        #[test]
        fn queued_saves_decide_guards_now_load_the_newest_and_land_in_order() {
            let root = tempfile::tempdir().unwrap();
            let mut bridge = RuntimePersistenceBridge::new(Some(root.path().to_path_buf()));
            let store = open(&mut bridge, "chunks", false);
            let gate = bridge.gate.clone();
            let held = gate.lock().unwrap();
            for (index, payload) in [b"one", b"two", b"six"].iter().enumerate() {
                let receipt = save_key(&mut bridge, store, "a", ANY, *payload, true);
                assert_eq!(receipt.outcome, NativePersistenceSaveOutcome::Queued);
                assert_eq!(
                    (receipt.revision, receipt.ticket),
                    (index as u64 + 1, index as u64 + 1)
                );
            }
            // Load and the guards see the queued state; nothing is written yet.
            assert_eq!(
                load_key(&mut bridge, store, "a").unwrap(),
                (3, b"six".to_vec())
            );
            assert!(!root.path().join("chunks/a").exists());
            let exact = (NativePersistenceRevisionGuard::Exact, 3);
            let receipt = save_key(&mut bridge, store, "a", exact, b"ten", true);
            assert_eq!(
                (receipt.outcome, receipt.revision, receipt.ticket),
                (NativePersistenceSaveOutcome::Queued, 4, 4)
            );
            let receipt = save_key(&mut bridge, store, "a", exact, b"old", true);
            assert_eq!(
                (receipt.outcome, receipt.revision, receipt.ticket),
                (NativePersistenceSaveOutcome::RevisionConflict, 4, 0)
            );
            let absent = (NativePersistenceRevisionGuard::Absent, 0);
            let receipt = save_key(&mut bridge, store, "a", absent, b"new", true);
            assert_eq!(
                receipt.outcome,
                NativePersistenceSaveOutcome::RevisionConflict
            );
            let pending = readout(&mut bridge, store);
            assert_eq!(
                (pending.queued, pending.finished, pending.failed),
                (4, 0, 0)
            );
            drop(held);

            // A save that is not deferred lands after the queued ones.
            let receipt = save_key(
                &mut bridge,
                store,
                "a",
                (NativePersistenceRevisionGuard::Exact, 4),
                b"last",
                false,
            );
            assert_eq!(
                (receipt.outcome, receipt.revision, receipt.ticket),
                (NativePersistenceSaveOutcome::Saved, 5, 0)
            );
            let done = readout(&mut bridge, store);
            assert_eq!(
                (done.queued, done.finished, done.failed, done.last_failed),
                (4, 4, 0, 0)
            );
            assert!(bridge.pending.is_empty());
            let stored = read_blob(&root.path().join("chunks/a"), "a")
                .unwrap()
                .unwrap();
            assert_eq!((stored.revision, stored.payload), (5, b"last".to_vec()));
            assert_eq!(
                &fs::read(root.path().join("chunks/a")).unwrap()[..4],
                b"RSP2"
            );
        }

        #[test]
        fn a_save_of_a_key_this_runtime_knows_reads_no_file() {
            let root = tempfile::tempdir().unwrap();
            let mut bridge = RuntimePersistenceBridge::new(Some(root.path().to_path_buf()));
            let store = open(&mut bridge, "slots", false);
            assert_eq!(
                load_key(&mut bridge, store, "fresh").unwrap(),
                (0, Vec::new())
            );
            assert_eq!(
                save_key(&mut bridge, store, "slot", ANY, b"one", false).revision,
                1
            );
            // Junk in the files would refuse any read of them.
            fs::write(root.path().join("slots/slot"), b"junk").unwrap();
            fs::write(root.path().join("slots/fresh"), b"junk").unwrap();
            let exact = (NativePersistenceRevisionGuard::Exact, 1);
            assert_eq!(
                save_key(&mut bridge, store, "slot", exact, b"two", true).revision,
                2
            );
            let absent = (NativePersistenceRevisionGuard::Absent, 0);
            assert_eq!(
                save_key(&mut bridge, store, "fresh", absent, b"new", true).revision,
                1
            );
            bridge.take_finished(true);
            assert_eq!(
                load_key(&mut bridge, store, "slot").unwrap(),
                (2, b"two".to_vec())
            );
            assert_eq!(
                load_key(&mut bridge, store, "fresh").unwrap(),
                (1, b"new".to_vec())
            );
        }

        #[test]
        fn a_failed_queued_save_is_reported_and_later_saves_still_land() {
            let root = tempfile::tempdir().unwrap();
            let mut bridge = RuntimePersistenceBridge::new(Some(root.path().to_path_buf()));
            let store = open(&mut bridge, "caves", false);
            let gate = bridge.gate.clone();
            let held = gate.lock().unwrap();
            assert_eq!(
                save_key(&mut bridge, store, "blocked/cave", ANY, b"lost", true).ticket,
                1
            );
            assert_eq!(
                save_key(&mut bridge, store, "cave", ANY, b"kept", true).ticket,
                2
            );
            // A file where the key's directory should be fails the write.
            fs::write(root.path().join("caves/blocked"), b"").unwrap();
            drop(held);
            bridge.take_finished(true);
            let done = readout(&mut bridge, store);
            assert_eq!(
                (done.queued, done.finished, done.failed, done.last_failed),
                (2, 2, 1, 1)
            );
            let mut text = NativeByteResult {
                bytes: std::ptr::null(),
                len: 0,
            };
            assert_eq!(
                unsafe {
                    read_save_failure(context(&mut bridge), store, &mut text, std::ptr::null_mut())
                },
                ABI_OK
            );
            let text = String::from_utf8(
                unsafe { std::slice::from_raw_parts(text.bytes, text.len) }.to_vec(),
            )
            .unwrap();
            assert!(
                text.starts_with("CSHARP_PERSISTENCE_IO")
                    && text.contains("`blocked/cave` (ticket 1)"),
                "{text}"
            );
            assert_eq!(
                load_key(&mut bridge, store, "cave").unwrap(),
                (1, b"kept".to_vec())
            );
            assert!(load_key(&mut bridge, store, "blocked/cave").is_err());

            // A destroyed store's readout is refused; another store has its own tickets.
            let other = open(&mut bridge, "other", false);
            assert_eq!(readout(&mut bridge, other).queued, 0);
            assert_eq!(
                unsafe { destroy_store(context(&mut bridge), store) },
                ABI_OK
            );
            let mut value = NativePersistenceSaveReadout::default();
            let mut refusal = empty_receipt();
            assert_eq!(
                unsafe { read_saves(context(&mut bridge), store, &mut value, &mut refusal) },
                0
            );
            assert_eq!(receipt_codes(&refusal), ["CSHARP_PERSISTENCE_STORE"]);
        }

        #[test]
        fn dropping_the_bridge_writes_every_queued_save_and_delete_waits_for_them() {
            let root = tempfile::tempdir().unwrap();
            let payload = |index: usize| vec![index as u8; 8 * 1024];
            {
                let mut bridge = RuntimePersistenceBridge::new(Some(root.path().to_path_buf()));
                let store = open(&mut bridge, "world", false);
                for index in 0..64 {
                    save_key(
                        &mut bridge,
                        store,
                        &format!("chunk-{index}"),
                        ANY,
                        &payload(index),
                        true,
                    );
                }
                save_key(&mut bridge, store, "doomed", ANY, b"gone", true);
                let mut deleted = NativePersistenceDeleteReceipt::default();
                let request = NativePersistenceDeleteRequest {
                    store,
                    key: utf8("doomed"),
                    revision_guard: NativePersistenceRevisionGuard::Exact,
                    expected_revision: 1,
                };
                assert_eq!(
                    unsafe {
                        delete(
                            context(&mut bridge),
                            &request,
                            &mut deleted,
                            std::ptr::null_mut(),
                        )
                    },
                    ABI_OK
                );
                assert_eq!(
                    (deleted.outcome, deleted.revision),
                    (NativePersistenceDeleteOutcome::Deleted, 1)
                );
                for index in 0..64 {
                    save_key(
                        &mut bridge,
                        store,
                        &format!("chunk-{index}"),
                        ANY,
                        &payload(index + 1),
                        true,
                    );
                }
            }
            let mut bridge = RuntimePersistenceBridge::new(Some(root.path().to_path_buf()));
            let store = open(&mut bridge, "world", false);
            for index in 0..64 {
                assert_eq!(
                    load_key(&mut bridge, store, &format!("chunk-{index}")).unwrap(),
                    (2, payload(index + 1))
                );
            }
            assert!(!root.path().join("world/doomed").exists());
        }

        #[test]
        fn a_cache_store_reads_a_torn_blob_as_absent_and_regenerates_it_after_a_restart() {
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("cache/chunk");
            let payload = vec![7_u8; 8 * 1024];
            let bytes = {
                let mut bridge = RuntimePersistenceBridge::new(Some(root.path().to_path_buf()));
                let store = open(&mut bridge, "cache", true);
                assert_eq!(
                    save_key(&mut bridge, store, "chunk", ANY, &payload, false).revision,
                    1
                );
                assert_eq!(
                    save_key(&mut bridge, store, "chunk", ANY, &payload, true).revision,
                    2
                );
                bridge.take_finished(true);
                let bytes = fs::read(&path).unwrap();
                assert_eq!(
                    (&bytes[..4], bytes.len()),
                    (&b"RSC1"[..], CACHE_HEADER_LEN + payload.len())
                );
                assert_eq!(
                    load_key(&mut bridge, store, "chunk").unwrap(),
                    (2, payload.clone())
                );
                bytes
            };

            // After a crash: a full-length blob whose tail never reached the
            // disk, a truncated one, and one that is not a container at all.
            let mut torn = bytes.clone();
            torn[CACHE_HEADER_LEN + 4096..].fill(0);
            let damaged = [torn, bytes[..CACHE_HEADER_LEN + 100].to_vec(), vec![0; 64]];
            for (index, damage) in damaged.iter().enumerate() {
                fs::write(&path, damage).unwrap();
                let mut bridge = RuntimePersistenceBridge::new(Some(root.path().to_path_buf()));
                // A durable store refuses the damaged blob and leaves it.
                let durable = open(&mut bridge, "cache", false);
                assert!(load_key(&mut bridge, durable, "chunk")
                    .unwrap_err()
                    .starts_with("CSHARP_PERSISTENCE_CONTAINER_"));
                let cache = open(&mut bridge, "cache", true);
                assert_eq!(
                    load_key(&mut bridge, cache, "chunk").unwrap(),
                    (0, Vec::new()),
                    "damage {index}"
                );
                if index == 2 {
                    // A delete discards it.
                    let mut deleted = NativePersistenceDeleteReceipt::default();
                    let request = NativePersistenceDeleteRequest {
                        store: cache,
                        key: utf8("chunk"),
                        revision_guard: NativePersistenceRevisionGuard::Any,
                        expected_revision: 0,
                    };
                    assert_eq!(
                        unsafe {
                            delete(
                                context(&mut bridge),
                                &request,
                                &mut deleted,
                                std::ptr::null_mut(),
                            )
                        },
                        ABI_OK
                    );
                    assert_eq!(deleted.outcome, NativePersistenceDeleteOutcome::Missing);
                    assert!(!path.exists());
                    continue;
                }
                // The cache regenerates the key over it.
                let absent = (NativePersistenceRevisionGuard::Absent, 0);
                let regenerated = vec![index as u8 + 1; 8 * 1024];
                assert_eq!(
                    save_key(
                        &mut bridge,
                        cache,
                        "chunk",
                        absent,
                        &regenerated,
                        index == 0
                    )
                    .revision,
                    1
                );
                bridge.take_finished(true);
                drop(bridge);
                let mut bridge = RuntimePersistenceBridge::new(Some(root.path().to_path_buf()));
                let cache = open(&mut bridge, "cache", true);
                assert_eq!(
                    load_key(&mut bridge, cache, "chunk").unwrap(),
                    (1, regenerated)
                );
            }
        }
    }
}
