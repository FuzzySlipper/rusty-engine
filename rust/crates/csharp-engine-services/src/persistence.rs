//! Durable opaque product-byte storage behind the generated NativeAOT table.
//!
//! Product code supplies state bytes, schema versions, and migration meaning.
//! This owner supplies relative-key admission, revisions, atomic replacement,
//! failure preservation, and explicit retained blob lifetime.

use std::{
    collections::BTreeMap,
    ffi::c_void,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

use csharp_engine_abi::*;

use crate::{composition::borrowed_utf8, composition::ABI_OK};

const HEADER_MAGIC: [u8; 4] = *b"RSP1";
const HEADER_LEN: usize = 4 + 4 + 8 + 8;
const MAX_PRODUCT_PAYLOAD_BYTES: usize = 256 * 1024 * 1024;

#[derive(Debug)]
struct DurableStore {
    root: PathBuf,
}

#[derive(Debug, Clone)]
struct PersistenceBlob {
    present: bool,
    schema_version: u32,
    revision: u64,
    payload: Vec<u8>,
}

pub(crate) struct RuntimePersistenceBridge {
    stores: BTreeMap<u64, DurableStore>,
    blobs: BTreeMap<u64, PersistenceBlob>,
    next_store: u64,
    next_blob: u64,
}

impl RuntimePersistenceBridge {
    pub(crate) fn new() -> Self {
        Self {
            stores: BTreeMap::new(),
            blobs: BTreeMap::new(),
            next_store: 1,
            next_blob: 1,
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
}

pub(crate) fn api(bridge: &mut RuntimePersistenceBridge) -> NativePersistenceApi {
    NativePersistenceApi {
        context: (bridge as *mut RuntimePersistenceBridge).cast(),
        open_store,
        destroy_store,
        save,
        load,
        destroy_blob,
        describe_blob,
        copy_blob,
    }
}

unsafe extern "C" fn open_store(
    context: *mut c_void,
    request: *const NativePersistenceOpenRequest,
    result: *mut NativePersistenceStoreHandle,
) -> i32 {
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let request = unsafe { &*request };
    let root =
        match unsafe { borrowed_utf8(request.root.bytes, request.root.len, "persistence root") } {
            Ok(value) if !value.is_empty() => PathBuf::from(value),
            _ => return 0,
        };
    if fs::create_dir_all(&root).is_err() || !root.is_dir() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimePersistenceBridge>() };
    match bridge.insert_store(DurableStore { root }) {
        Some(handle) => {
            unsafe { *result = handle };
            ABI_OK
        }
        None => 0,
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
    receipt: *mut NativePersistenceSaveReceipt,
) -> i32 {
    if context.is_null() || request.is_null() || receipt.is_null() {
        return 0;
    }
    let request = unsafe { &*request };
    let key = match unsafe { borrowed_utf8(request.key.bytes, request.key.len, "persistence key") }
    {
        Ok(value) => value,
        Err(_) => return 0,
    };
    let payload = match unsafe { borrowed_bytes(request.payload, "persistence payload") } {
        Ok(value) => value.to_vec(),
        Err(_) => return 0,
    };
    if payload.len() > MAX_PRODUCT_PAYLOAD_BYTES {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimePersistenceBridge>() };
    let Some(store) = bridge.stores.get(&request.store.value) else {
        return 0;
    };
    let Ok(path) = storage_path(&store.root, key) else {
        return 0;
    };
    let current = match read_blob(&path) {
        Ok(value) => value,
        Err(_) => return 0,
    };
    if !matches_guard(
        request.revision_guard,
        request.expected_revision,
        current.as_ref(),
    ) {
        return 0;
    }
    let revision = match current {
        Some(value) => match value.revision.checked_add(1) {
            Some(value) => value,
            None => return 0,
        },
        None => 1,
    };
    let next = PersistenceBlob {
        present: true,
        schema_version: request.schema_version,
        revision,
        payload,
    };
    if write_atomically(&path, &next).is_err() {
        return 0;
    }
    unsafe {
        *receipt = NativePersistenceSaveReceipt {
            revision,
            schema_version: request.schema_version,
        };
    }
    ABI_OK
}

unsafe extern "C" fn load(
    context: *mut c_void,
    request: *const NativePersistenceLoadRequest,
    result: *mut NativePersistenceBlobHandle,
) -> i32 {
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let request = unsafe { &*request };
    let key = match unsafe { borrowed_utf8(request.key.bytes, request.key.len, "persistence key") }
    {
        Ok(value) => value,
        Err(_) => return 0,
    };
    let bridge = unsafe { &mut *context.cast::<RuntimePersistenceBridge>() };
    let Some(store) = bridge.stores.get(&request.store.value) else {
        return 0;
    };
    let Ok(path) = storage_path(&store.root, key) else {
        return 0;
    };
    let blob = match read_blob(&path) {
        Ok(Some(blob)) => blob,
        Ok(None) => PersistenceBlob {
            present: false,
            schema_version: 0,
            revision: 0,
            payload: Vec::new(),
        },
        Err(_) => return 0,
    };
    match bridge.insert_blob(blob) {
        Some(handle) => {
            unsafe { *result = handle };
            ABI_OK
        }
        None => 0,
    }
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
            schema_version: blob.schema_version,
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
        // SAFETY: the C# facade pins exactly `destination.len` writable bytes
        // for this immediate call; this bridge retains neither pointer.
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

unsafe fn borrowed_bytes<'a>(value: NativeByteSlice, _field: &'static str) -> Result<&'a [u8], ()> {
    if value.len > 0 && value.bytes.is_null() {
        return Err(());
    }
    if value.len == 0 {
        Ok(&[])
    } else {
        // SAFETY: the C# facade pins this source span until the callback returns.
        Ok(unsafe { std::slice::from_raw_parts(value.bytes, value.len) })
    }
}

fn storage_path(root: &Path, key: &str) -> Result<PathBuf, ()> {
    let path = Path::new(key);
    if key.is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(());
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

fn read_blob(path: &Path) -> Result<Option<PersistenceBlob>, ()> {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(()),
    };
    let mut header = [0_u8; HEADER_LEN];
    file.read_exact(&mut header).map_err(|_| ())?;
    if header[..4] != HEADER_MAGIC {
        return Err(());
    }
    let schema_version = u32::from_le_bytes(header[4..8].try_into().map_err(|_| ())?);
    let revision = u64::from_le_bytes(header[8..16].try_into().map_err(|_| ())?);
    let payload_len = u64::from_le_bytes(header[16..24].try_into().map_err(|_| ())?);
    let payload_len: usize = payload_len.try_into().map_err(|_| ())?;
    if payload_len > MAX_PRODUCT_PAYLOAD_BYTES {
        return Err(());
    }
    let mut payload = vec![0; payload_len];
    file.read_exact(&mut payload).map_err(|_| ())?;
    if file.read(&mut [0_u8; 1]).map_err(|_| ())? != 0 {
        return Err(());
    }
    Ok(Some(PersistenceBlob {
        present: true,
        schema_version,
        revision,
        payload,
    }))
}

fn write_atomically(path: &Path, blob: &PersistenceBlob) -> Result<(), ()> {
    let parent = path.parent().ok_or(())?;
    fs::create_dir_all(parent).map_err(|_| ())?;
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or(())?;
    let temporary = parent.join(format!(".{name}.rusty-engine-pending"));
    // A prior interrupted commit can leave only this never-published sibling.
    // The target is untouched until the later same-directory rename succeeds.
    if temporary.exists() {
        fs::remove_file(&temporary).map_err(|_| ())?;
    }
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|_| ())?;
        file.write_all(&HEADER_MAGIC).map_err(|_| ())?;
        file.write_all(&blob.schema_version.to_le_bytes())
            .map_err(|_| ())?;
        file.write_all(&blob.revision.to_le_bytes())
            .map_err(|_| ())?;
        file.write_all(&(blob.payload.len() as u64).to_le_bytes())
            .map_err(|_| ())?;
        file.write_all(&blob.payload).map_err(|_| ())?;
        file.sync_all().map_err(|_| ())?;
        fs::rename(&temporary, path).map_err(|_| ())?;
        Ok(())
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
    fn direct_storage_round_trip_and_stale_save_preserves_committed_payload() {
        let root = tempfile::tempdir().unwrap();
        let root_text = root.path().to_str().unwrap().as_bytes();
        let mut bridge = RuntimePersistenceBridge::new();
        let context = (&mut bridge as *mut RuntimePersistenceBridge).cast();
        let open = NativePersistenceOpenRequest {
            root: NativeUtf8Slice {
                bytes: root_text.as_ptr(),
                len: root_text.len(),
            },
        };
        let mut store = NativePersistenceStoreHandle::default();
        assert_eq!(unsafe { open_store(context, &open, &mut store) }, ABI_OK);

        let key = b"campaign.state";
        let first_payload = b"first";
        let first = NativePersistenceSaveRequest {
            store,
            key: NativeUtf8Slice {
                bytes: key.as_ptr(),
                len: key.len(),
            },
            schema_version: 7,
            revision_guard: NativePersistenceRevisionGuard::Absent,
            expected_revision: 0,
            payload: NativeByteSlice {
                bytes: first_payload.as_ptr(),
                len: first_payload.len(),
            },
        };
        let mut saved = NativePersistenceSaveReceipt::default();
        assert_eq!(unsafe { save(context, &first, &mut saved) }, ABI_OK);
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
        assert_eq!(unsafe { save(context, &stale, &mut saved) }, 0);

        let load_request = NativePersistenceLoadRequest {
            store,
            key: NativeUtf8Slice {
                bytes: key.as_ptr(),
                len: key.len(),
            },
        };
        let mut blob = NativePersistenceBlobHandle::default();
        assert_eq!(unsafe { load(context, &load_request, &mut blob) }, ABI_OK);
        let mut info = NativePersistenceBlobInfo {
            present: false,
            schema_version: 0,
            revision: 0,
            payload_len: 0,
        };
        assert_eq!(unsafe { describe_blob(context, blob, &mut info) }, ABI_OK);
        assert!(info.present);
        assert_eq!(
            (info.schema_version, info.revision, info.payload_len),
            (7, 1, 5)
        );
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
        assert_eq!(unsafe { destroy_blob(context, blob) }, ABI_OK);
        assert_eq!(unsafe { destroy_store(context, store) }, ABI_OK);
    }
}
