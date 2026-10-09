//! Build content bundles, loose or packed, and content containers. Discovery
//! retains metadata only; an open holds its inventory and reads each file when
//! first used, and references own independent Arc clones.
use super::*;
use crate::composition::CsharpEngineServicesError;
use product_container::{join, Container, ProductSource};
use serde::Deserialize;
use std::{path::Path, sync::Mutex};

pub const INDEX: &str = ".rusty-bundles.json";

#[derive(Debug, Default)]
pub struct ProductContentBundles {
    /// Absent only for content with no bundle inventory.
    source: Option<Arc<BundleSource>>,
    /// The content root within `source`.
    content_root: String,
    bundles: Vec<BundleDefinition>,
}

/// Where bundle files are read from: a staged Product, or the content files a
/// test supplied (`EngineTestHost`), keyed by content-relative path.
#[derive(Debug)]
enum BundleSource {
    Product(ProductSource),
    Supplied(BTreeMap<String, Arc<[u8]>>),
}

impl BundleSource {
    fn read(&self, path: &str) -> Result<Arc<[u8]>, CsharpEngineServicesError> {
        match self {
            Self::Product(source) => source
                .read(path)
                .map(|bytes| Arc::from(bytes.as_ref()))
                .map_err(|failure| container_error(&failure)),
            Self::Supplied(files) => files.get(path).cloned().ok_or_else(|| {
                container_error(&product_container::Error::Missing(path.to_owned()))
            }),
        }
    }
}

#[derive(Debug, Deserialize)]
struct Index {
    bundles: Vec<BundleDefinition>,
}

#[derive(Debug, Deserialize)]
struct BundleDefinition {
    id: String,
    root: String,
    files: Vec<FileDefinition>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileDefinition {
    path: String,
    byte_length: u64,
    sha256: String,
}

pub(super) use product_container::is_relative_path as relative;

impl ProductContentBundles {
    /// Read only the bundle inventory; no bundle payload is read here. A loose
    /// Product carries the SDK-generated `.rusty-bundles.json` in its content
    /// root; a container carries the same facts in its own inventory.
    pub fn admit(source: &ProductSource, content_root: &str) -> Result<Self, String> {
        let bundles = match source.container() {
            Some(container) => packed_definitions(container, content_root)?,
            None => {
                let index = join(content_root, INDEX);
                if !source.is_file(&index) {
                    Vec::new()
                } else {
                    let bytes = source.read(&index).map_err(|e| e.to_string())?;
                    serde_json::from_slice::<Index>(&bytes)
                        .map_err(|e| format!("invalid ProductContent bundle inventory: {e}"))?
                        .bundles
                }
            }
        };
        Self::checked(BundleSource::Product(source.clone()), content_root, bundles)
    }

    /// The inventory among content files a test supplied, as a loose
    /// Product's content root carries it; none without one. Bundle files are
    /// read from the same files when a bundle opens.
    pub fn admit_supplied(files: &BTreeMap<String, Arc<[u8]>>) -> Result<Self, String> {
        let Some(index) = files.get(INDEX) else {
            return Ok(Self::default());
        };
        let bundles = serde_json::from_slice::<Index>(index)
            .map_err(|e| format!("invalid ProductContent bundle inventory: {e}"))?
            .bundles;
        Self::checked(BundleSource::Supplied(files.clone()), "", bundles)
    }

    fn checked(
        source: BundleSource,
        content_root: &str,
        bundles: Vec<BundleDefinition>,
    ) -> Result<Self, String> {
        let mut source = Self {
            source: Some(Arc::new(source)),
            content_root: content_root.to_owned(),
            bundles,
        };
        source.bundles.sort_by(|a, b| a.id.cmp(&b.id));
        for (i, bundle) in source.bundles.iter().enumerate() {
            if !relative(&bundle.id)
                || bundle.id.contains('/')
                || !relative(&bundle.root)
                || bundle.root == INDEX
            {
                return Err(format!("invalid bundle ID/root: {}", bundle.id));
            }
            for prior in &source.bundles[..i] {
                if bundle.id == prior.id
                    || bundle.root == prior.root
                    || bundle.root.starts_with(&format!("{}/", prior.root))
                    || prior.root.starts_with(&format!("{}/", bundle.root))
                {
                    return Err(format!(
                        "duplicate ID or overlapping bundle roots: {}",
                        bundle.id
                    ));
                }
            }
            let mut names = std::collections::BTreeSet::new();
            let mut total = 0u64;
            for file in &bundle.files {
                if !relative(&file.path)
                    || !names.insert(&file.path)
                    || file.sha256.len() != 64
                    || !file.sha256.bytes().all(|c| c.is_ascii_hexdigit())
                {
                    return Err(format!(
                        "invalid bundle file identity: {}/{}",
                        bundle.id, file.path
                    ));
                }
                total = total
                    .checked_add(file.byte_length)
                    .ok_or("bundle byte length overflow")?;
            }
        }
        Ok(source)
    }

    /// Whether `path` is the reserved bundle inventory or under a declared
    /// bundle root. The product runtime leaves such paths out of the eager
    /// content it reads at admission (the `Create` content and the renderer
    /// resources); bundles load them on demand.
    pub fn owns_path(&self, path: &str) -> bool {
        path == INDEX
            || self
                .bundles
                .iter()
                .any(|b| path.starts_with(&format!("{}/", b.root)))
    }

    /// Open a bundle from its inventory alone; no file body is read until used.
    fn load(&self, id: &str) -> Option<OpenBundle> {
        let bundle = self.bundles.iter().find(|b| b.id == id)?;
        let prefix = format!("{}/", bundle.root);
        let mut files = BTreeMap::new();
        for file in &bundle.files {
            files.insert(
                format!("{prefix}{}", file.path),
                Inventoried {
                    byte_length: file.byte_length,
                    digest: hex_digest(&file.sha256)?,
                },
            );
        }
        let identity = collection_identity(
            files
                .iter()
                .map(|(path, file)| (&path[prefix.len()..], file.digest))
                .collect(),
        );
        Some(OpenBundle {
            identity,
            files: Arc::new(BundleFiles::new(Store::Build {
                source: Arc::clone(self.source.as_ref()?),
                content_root: self.content_root.clone(),
                prefix,
                files,
            })),
        })
    }
}

/// A container's bundles under `content_root`, with content-relative roots and
/// bundle-relative file paths, as the loose inventory records them.
fn packed_definitions(
    container: &product_container::Container,
    content_root: &str,
) -> Result<Vec<BundleDefinition>, String> {
    container
        .bundles()
        .iter()
        .map(|bundle| {
            let root = match content_root {
                "" => Some(bundle.root.as_str()),
                prefix => bundle
                    .root
                    .strip_prefix(prefix)
                    .and_then(|root| root.strip_prefix('/')),
            }
            .ok_or_else(|| format!("bundle {} lies outside the content root", bundle.id))?;
            let under = format!("{}/", bundle.root);
            let files = container
                .entries()
                .iter()
                .filter(|entry| entry.bundle.as_deref() == Some(bundle.id.as_str()))
                .map(|entry| FileDefinition {
                    path: entry.path[under.len()..].to_owned(),
                    byte_length: entry.byte_length,
                    sha256: entry.sha256.clone(),
                })
                .collect();
            Ok(BundleDefinition {
                id: bundle.id.clone(),
                root: root.to_owned(),
                files,
            })
        })
        .collect()
}

/// One file's facts from a bundle inventory.
struct Inventoried {
    byte_length: u64,
    digest: [u8; 32],
}

/// Where an open bundle's files come from.
enum Store {
    /// A container: paths are container-relative. Opening checked its header
    /// and inventory.
    Container(Container),
    /// A build bundle: `files` by content-relative path (`prefix` is the
    /// bundle root and `/`), read from `source` under `content_root`.
    Build {
        source: Arc<BundleSource>,
        content_root: String,
        prefix: String,
        files: BTreeMap<String, Inventoried>,
    },
}

/// An open bundle's files: its inventory, with each file's bytes read once,
/// when first used, and shared from then on. References keep it (and the bytes
/// it read) alive after its bundle closes.
pub(crate) struct BundleFiles {
    store: Store,
    read: Mutex<BTreeMap<String, Arc<[u8]>>>,
}

impl BundleFiles {
    fn new(store: Store) -> Self {
        Self {
            store,
            read: Mutex::default(),
        }
    }

    /// The prefix that turns a bundle-relative path into a content path.
    fn prefix(&self) -> &str {
        match &self.store {
            Store::Container(_) => "",
            Store::Build { prefix, .. } => prefix,
        }
    }

    /// The inventory's SHA-256 for the file at `path`, if it lists one.
    fn digest(&self, path: &str) -> Option<[u8; 32]> {
        match &self.store {
            Store::Container(container) => hex_digest(&container.entry(path)?.sha256),
            Store::Build { files, .. } => Some(files.get(path)?.digest),
        }
    }

    /// The file's bytes, or none when the inventory has no such file. A listed
    /// file that cannot be read (missing, a corrupt compressed entry, or a
    /// length the inventory does not record) refuses, naming the file.
    pub(crate) fn bytes(&self, path: &str) -> Result<Option<Arc<[u8]>>, CsharpEngineServicesError> {
        let mut read = self
            .read
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if let Some(bytes) = read.get(path) {
            return Ok(Some(Arc::clone(bytes)));
        }
        let bytes: Arc<[u8]> = match &self.store {
            Store::Container(container) => match container.get(path) {
                Ok(bytes) => Arc::from(bytes.as_ref()),
                Err(product_container::Error::Missing(_)) => return Ok(None),
                Err(failure) => return Err(container_error(&failure)),
            },
            Store::Build {
                source,
                content_root,
                files,
                ..
            } => {
                let Some(file) = files.get(path) else {
                    return Ok(None);
                };
                let bytes = source.read(&join(content_root, path))?;
                // Staging wrote the inventory's SHA-256 from these same bytes,
                // so it is trusted rather than recomputed; a body changed after
                // staging gets a new identity on the next restage. The length
                // check costs nothing and catches a file cut short by an
                // interrupted restage.
                if bytes.len() as u64 != file.byte_length {
                    return Err(CsharpEngineServicesError::new(
                        "PRODUCT_BUNDLE_FILE_CHANGED",
                        format!(
                            "`{path}` is {} bytes but its bundle inventory records {}; \
                             reopen the bundle after restaging",
                            bytes.len(),
                            file.byte_length
                        ),
                    ));
                }
                bytes
            }
        };
        read.insert(path.to_owned(), Arc::clone(&bytes));
        Ok(Some(bytes))
    }

    /// The file at content path `path` as admitted content, if listed.
    fn content(
        self: &Arc<Self>,
        path: &str,
    ) -> Result<Option<AdmittedContent>, CsharpEngineServicesError> {
        let (Some(digest), Some(bytes)) = (self.digest(path), self.bytes(path)?) else {
            return Ok(None);
        };
        Ok(Some(AdmittedContent {
            path: path.to_owned(),
            identity: super::ContentIdentity::known(sha256_words(&digest)),
            bytes,
            transient: false,
            files: super::ContentFiles::Bundle(Arc::clone(self)),
        }))
    }
}

/// One open bundle: a build bundle or a container, read on use.
struct OpenBundle {
    identity: NativeContentSha256,
    files: Arc<BundleFiles>,
}

impl OpenBundle {
    fn container(container: Container) -> Option<Self> {
        let identities = container
            .entries()
            .iter()
            .map(|entry| Some((entry.path.as_str(), hex_digest(&entry.sha256)?)))
            .collect::<Option<Vec<_>>>()?;
        Some(Self {
            identity: collection_identity(identities),
            files: Arc::new(BundleFiles::new(Store::Container(container))),
        })
    }

    /// Bundle-relative path, SHA-256 and length of every file, in path order.
    fn entries(&self) -> Vec<(String, NativeContentSha256, u64)> {
        match &self.files.store {
            Store::Container(container) => container
                .entries()
                .iter()
                .filter_map(|entry| {
                    let sha256 = sha256_words(&hex_digest(&entry.sha256)?);
                    Some((entry.path.clone(), sha256, entry.byte_length))
                })
                .collect(),
            Store::Build { prefix, files, .. } => files
                .iter()
                .map(|(path, file)| {
                    (
                        path[prefix.len()..].to_owned(),
                        sha256_words(&file.digest),
                        file.byte_length,
                    )
                })
                .collect(),
        }
    }

    /// The file at bundle-relative `path`.
    fn reference(&self, path: &str) -> Result<Option<AdmittedContent>, CsharpEngineServicesError> {
        self.files
            .content(&format!("{}{path}", self.files.prefix()))
    }

    /// The file at content path `path`, when the inventory lists it with `hash`.
    fn resolve(
        &self,
        path: &str,
        hash: NativeContentSha256,
    ) -> Result<Option<AdmittedContent>, CsharpEngineServicesError> {
        match self.files.digest(path) {
            Some(digest) if sha256_words(&digest) == hash => self.files.content(path),
            _ => Ok(None),
        }
    }
}

/// A collection's identity: SHA-256 over each file's path and SHA-256 digest
/// in path order, so it follows the files and not how they are stored.
fn collection_identity(mut files: Vec<(&str, [u8; 32])>) -> NativeContentSha256 {
    files.sort_unstable();
    let mut hasher = Sha256::new();
    for (path, digest) in files {
        hasher.update(path.as_bytes());
        hasher.update([0]);
        hasher.update(digest);
    }
    sha256_words(&hasher.finalize())
}

#[derive(Default)]
pub(super) struct BundleState {
    pub(super) source: ProductContentBundles,
    open: BTreeMap<u64, OpenBundle>,
    next: u64,
}

impl BundleState {
    pub(super) fn resolve(
        &self,
        path: &str,
        hash: NativeContentSha256,
    ) -> Result<Option<AdmittedContent>, CsharpEngineServicesError> {
        for bundle in self.open.values() {
            if let Some(content) = bundle.resolve(path, hash)? {
                return Ok(Some(content));
            }
        }
        Ok(None)
    }

    fn next(&mut self) -> Option<u64> {
        self.next = self.next.checked_add(1)?;
        Some(self.next)
    }
}

pub(super) unsafe extern "C" fn list_bundles(
    context: *mut c_void,
    result: *mut NativeContentBundleInfoResult,
) -> i32 {
    if context.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeContentBridge>() };
    let ids: Vec<String> = bridge
        .bundles
        .source
        .bundles
        .iter()
        .map(|b| b.id.clone())
        .collect();
    let infos = ids
        .iter()
        .zip(&bridge.bundles.source.bundles)
        .map(|(id, b)| NativeContentBundleInfo {
            id: NativeUtf8Slice {
                bytes: id.as_ptr(),
                len: id.len(),
            },
            file_count: b.files.len() as u64,
            byte_length: b.files.iter().map(|f| f.byte_length).sum(),
        })
        .collect::<Vec<_>>();
    unsafe {
        *result = NativeContentBundleInfoResult {
            bundles: infos.as_ptr(),
            bundles_len: infos.len(),
        };
    }
    bridge.borrowed.hold((ids, infos));
    ABI_OK
}

pub(super) unsafe extern "C" fn open_bundle(
    context: *mut c_void,
    request: *const NativeContentBundleOpenRequest,
    result: *mut NativeContentBundleHandle,
) -> i32 {
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let request = unsafe { &*request };
    let Ok(id) = (unsafe { borrowed_utf8(request.id.bytes, request.id.len, "bundle ID") }) else {
        return 0;
    };
    let bridge = unsafe { &mut *context.cast::<RuntimeContentBridge>() };
    let Some(bundle) = bridge.bundles.source.load(id) else {
        return 0;
    };
    let Some(value) = bridge.bundles.next() else {
        return 0;
    };
    bridge.bundles.open.insert(value, bundle);
    unsafe {
        *result = NativeContentBundleHandle { value };
    }
    ABI_OK
}

pub(super) unsafe extern "C" fn open_container(
    context: *mut c_void,
    request: *const NativeContentContainerOpenRequest,
    result: *mut NativeContentBundleHandle,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(error);
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let request = unsafe { &*request };
    let path =
        match unsafe { borrowed_utf8(request.path.bytes, request.path.len, "container path") } {
            Ok(path) => path,
            Err(refusal) => return refuse(&refusal, error),
        };
    let bundle = Container::open(Path::new(path))
        .map_err(|failure| container_error(&failure))
        .and_then(|container| {
            OpenBundle::container(container).ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "PRODUCT_CONTAINER_CORRUPT",
                    format!("`{path}`: an entry has an invalid SHA-256"),
                )
            })
        });
    let bundle = match bundle {
        Ok(bundle) => bundle,
        Err(refusal) => return refuse(&refusal, error),
    };
    let bridge = unsafe { &mut *context.cast::<RuntimeContentBridge>() };
    let Some(value) = bridge.bundles.next() else {
        return 0;
    };
    bridge.bundles.open.insert(value, bundle);
    unsafe {
        *result = NativeContentBundleHandle { value };
    }
    ABI_OK
}

pub(super) unsafe extern "C" fn pack_container(
    context: *mut c_void,
    request: *const NativeContentContainerPackRequest,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(error);
    if context.is_null() || request.is_null() {
        return 0;
    }
    let request = unsafe { &*request };
    let paths = unsafe {
        borrowed_utf8(request.directory.bytes, request.directory.len, "directory").and_then(
            |directory| {
                borrowed_utf8(request.output.bytes, request.output.len, "output")
                    .map(|output| (directory, output))
            },
        )
    };
    let (directory, output) = match paths {
        Ok(paths) => paths,
        Err(refusal) => return refuse(&refusal, error),
    };
    match product_container::pack_content(Path::new(directory), Path::new(output), request.compress)
    {
        Ok(_) => ABI_OK,
        Err(failure) => refuse(&container_error(&failure), error),
    }
}

pub(super) unsafe extern "C" fn read_bundle_identity(
    context: *mut c_void,
    bundle: NativeContentBundleHandle,
    result: *mut NativeContentSha256,
) -> i32 {
    if context.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeContentBridge>() };
    let Some(bundle) = bridge.bundles.open.get(&bundle.value) else {
        return 0;
    };
    unsafe { *result = bundle.identity };
    ABI_OK
}

pub(super) unsafe extern "C" fn destroy_bundle(
    context: *mut c_void,
    handle: NativeContentBundleHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeContentBridge>() };
    if bridge.bundles.open.remove(&handle.value).is_some() {
        ABI_OK
    } else {
        0
    }
}

pub(super) unsafe extern "C" fn read_bundle_files(
    context: *mut c_void,
    bundle: NativeContentBundleHandle,
    result: *mut NativeContentReferenceInfoResult,
) -> i32 {
    if context.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeContentBridge>() };
    let Some(bundle) = bridge.bundles.open.get(&bundle.value) else {
        return 0;
    };
    let entries = bundle.entries();
    let Some(value) = bridge.retain_info(entries) else {
        return 0;
    };
    unsafe {
        *result = value;
    }
    ABI_OK
}

pub(super) unsafe extern "C" fn open_bundle_reference(
    context: *mut c_void,
    request: *const NativeContentBundleReferenceRequest,
    result: *mut NativeContentReferenceHandle,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(error);
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let request = unsafe { &*request };
    let Ok(path) = (unsafe { borrowed_utf8(request.path.bytes, request.path.len, "bundle file") })
    else {
        return 0;
    };
    let bridge = unsafe { &mut *context.cast::<RuntimeContentBridge>() };
    let Some(bundle) = bridge.bundles.open.get(&request.bundle.value) else {
        return 0;
    };
    let file = match bundle.reference(path) {
        Ok(Some(file)) => file,
        Ok(None) => return 0,
        Err(refusal) => return refuse(&refusal, error),
    };
    let Some(handle) = bridge.retain(file) else {
        return 0;
    };
    unsafe {
        *result = handle;
    }
    ABI_OK
}

/// A container failure as a Content refusal: its `PRODUCT_*` code and a
/// message naming the file.
fn container_error(failure: &product_container::Error) -> CsharpEngineServicesError {
    CsharpEngineServicesError::new(failure.code(), failure.to_string())
}

/// The 32 bytes of a validated 64-digit hex SHA-256.
fn hex_digest(hex: &str) -> Option<[u8; 32]> {
    let mut digest = [0_u8; 32];
    for (index, byte) in digest.iter_mut().enumerate() {
        *byte = u8::from_str_radix(hex.get(index * 2..index * 2 + 2)?, 16).ok()?;
    }
    Some(digest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::Path};

    fn loose(root: &Path) -> ProductSource {
        ProductSource::open(root).unwrap()
    }

    fn text(value: &str) -> NativeUtf8Slice {
        NativeUtf8Slice {
            bytes: value.as_ptr(),
            len: value.len(),
        }
    }

    fn fixture(root: &Path) {
        fs::create_dir(root.join("rules")).unwrap();
        fs::create_dir(root.join("unused")).unwrap();
        fs::write(root.join("rules/a.json"), b"{\"id\":1}").unwrap();
        let file = |path: &str| serde_json::json!({"path":path,"byteLength":8,"sha256":format!("{:x}", Sha256::digest(b"{\"id\":1}"))});
        fs::write(
            root.join(INDEX),
            serde_json::to_vec(&serde_json::json!({"bundles":[
                {"id":"rules","root":"rules","files":[file("a.json")]},
                {"id":"unused","root":"unused","files":[file("missing.json")]}
            ]}))
            .unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn discovery_is_metadata_only_and_reference_ownership_survives_bundle_close() {
        let directory = tempfile::tempdir().unwrap();
        fixture(directory.path());
        let source = ProductContentBundles::admit(&loose(directory.path()), "").unwrap();
        assert!(source.owns_path("rules/a.json"));
        assert!(!source.owns_path("rules-other/a.json"));
        let mut bridge = RuntimeContentBridge::new(BTreeMap::new());
        bridge.bind_bundles(source);
        let context = (&mut bridge as *mut RuntimeContentBridge).cast();
        let mut bundle = NativeContentBundleHandle::default();
        assert_eq!(
            unsafe {
                open_bundle(
                    context,
                    &NativeContentBundleOpenRequest { id: text("rules") },
                    &mut bundle,
                )
            },
            ABI_OK
        );
        assert_eq!(bridge.bundles.open.len(), 1);
        let weak = Arc::downgrade(
            &bridge.bundles.open[&bundle.value]
                .reference("a.json")
                .unwrap()
                .unwrap()
                .bytes,
        );
        let mut reference = NativeContentReferenceHandle::default();
        assert_eq!(
            unsafe {
                open_bundle_reference(
                    context,
                    &NativeContentBundleReferenceRequest {
                        bundle,
                        path: text("a.json"),
                    },
                    &mut reference,
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        assert_eq!(unsafe { destroy_bundle(context, bundle) }, ABI_OK);
        assert!(bridge.bundles.open.is_empty());
        assert_eq!(
            bridge.retained_bytes(reference).unwrap().as_ref(),
            b"{\"id\":1}"
        );
        assert!(weak.upgrade().is_some());
        assert_eq!(
            unsafe { super::super::destroy_reference(context, reference) },
            ABI_OK
        );
        assert!(weak.upgrade().is_none());
        // Closing and reopening uses the same build identity, with no leaked
        // implicit catalog mount.
        assert_eq!(
            unsafe {
                open_bundle(
                    context,
                    &NativeContentBundleOpenRequest { id: text("rules") },
                    &mut bundle,
                )
            },
            ABI_OK
        );
        assert_eq!(unsafe { destroy_bundle(context, bundle) }, ABI_OK);
        assert!(bridge.bundles.open.is_empty());
    }

    fn bundle_file(path: &str, bytes: &[u8]) -> serde_json::Value {
        serde_json::json!({"path": path, "byteLength": bytes.len(), "sha256": format!("{:x}", Sha256::digest(bytes))})
    }

    /// Opening costs the inventory: a bundle whose files are missing or
    /// changed still opens with its inventory's entries and identity, and
    /// each read reads only its own file, refusing one that is gone or whose
    /// length the inventory does not record. The inventory's SHA-256 is
    /// trusted, so a same-length edit reads under the staged identity.
    #[test]
    fn opening_reads_no_bodies_and_each_read_reads_only_its_file() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("world");
        fs::create_dir(&root).unwrap();
        let bodies: [(&str, &[u8]); 4] = [
            ("a.json", b"{\"a\":1}"),
            ("gone.bin", b"gone"),
            ("short.bin", b"long enough"),
            ("same.txt", b"before"),
        ];
        for (path, bytes) in bodies {
            fs::write(root.join(path), bytes).unwrap();
        }
        fs::write(
            directory.path().join(INDEX),
            serde_json::to_vec(
                &serde_json::json!({"bundles": [{"id": "world", "root": "world",
                "files": bodies.map(|(path, bytes)| bundle_file(path, bytes))}]}),
            )
            .unwrap(),
        )
        .unwrap();
        let source = ProductContentBundles::admit(&loose(directory.path()), "").unwrap();
        let intact = source.load("world").unwrap().identity;

        fs::remove_file(root.join("gone.bin")).unwrap();
        fs::write(root.join("short.bin"), b"cut").unwrap();
        fs::write(root.join("same.txt"), b"after!").unwrap();
        let mut bridge = RuntimeContentBridge::new(BTreeMap::new());
        bridge.bind_bundles(source);
        let context = (&mut bridge as *mut RuntimeContentBridge).cast();
        let mut bundle = NativeContentBundleHandle::default();
        assert_eq!(
            unsafe {
                open_bundle(
                    context,
                    &NativeContentBundleOpenRequest { id: text("world") },
                    &mut bundle,
                )
            },
            ABI_OK
        );
        let open = &bridge.bundles.open[&bundle.value];
        assert_eq!(open.identity, intact);
        assert_eq!(
            open.entries()
                .into_iter()
                .map(|(path, _, length)| (path, length))
                .collect::<Vec<_>>(),
            [
                ("a.json", 7),
                ("gone.bin", 4),
                ("same.txt", 6),
                ("short.bin", 11)
            ]
            .map(|(path, length)| (path.to_owned(), length))
        );
        assert!(
            open.files.read.lock().unwrap().is_empty(),
            "opening read a body"
        );

        let a = open.reference("a.json").unwrap().unwrap();
        assert_eq!(a.path, "world/a.json");
        assert_eq!(a.bytes.as_ref(), b"{\"a\":1}");
        assert_eq!(
            open.files.read.lock().unwrap().keys().collect::<Vec<_>>(),
            ["world/a.json"]
        );
        let same = open.reference("same.txt").unwrap().unwrap();
        assert_eq!(same.bytes.as_ref(), b"after!");
        assert_eq!(same.sha256(), sha256_words(&Sha256::digest(b"before")));
        for (path, code) in [
            ("gone.bin", "PRODUCT_SOURCE_MISSING"),
            ("short.bin", "PRODUCT_BUNDLE_FILE_CHANGED"),
        ] {
            let refusal = open.reference(path).err().unwrap();
            assert_eq!(refusal.code(), code);
            assert!(refusal.detail().contains(path), "{}", refusal.detail());
        }
        assert!(open.reference("absent.json").unwrap().is_none());
    }

    #[test]
    fn retained_source_keeps_only_its_own_dependency_collection() {
        let directory = tempfile::tempdir().unwrap();
        let mut definitions = Vec::new();
        for root in ["actors", "other"] {
            fs::create_dir(directory.path().join(root)).unwrap();
            let mut files = Vec::new();
            for (path, bytes) in [
                ("model.glb", b"model".as_slice()),
                ("skin.png", root.as_bytes()),
            ] {
                fs::write(directory.path().join(root).join(path), bytes).unwrap();
                files.push(serde_json::json!({"path": path, "byteLength": bytes.len(), "sha256": format!("{:x}", Sha256::digest(bytes))}));
            }
            definitions.push(serde_json::json!({"id": root, "root": root, "files": files}));
        }
        fs::write(
            directory.path().join(INDEX),
            serde_json::to_vec(&serde_json::json!({"bundles": definitions})).unwrap(),
        )
        .unwrap();
        let source = ProductContentBundles::admit(&loose(directory.path()), "").unwrap();
        let actors = source.load("actors").unwrap();
        let other = source.load("other").unwrap();
        let retained = actors.reference("model.glb").unwrap().unwrap();
        let dependency = Arc::downgrade(&retained.files.get("actors/skin.png").unwrap().unwrap());
        drop(actors);
        drop(other);
        assert_eq!(
            retained
                .files
                .get("actors/skin.png")
                .unwrap()
                .unwrap()
                .as_ref(),
            b"actors"
        );
        assert!(retained.files.get("other/skin.png").unwrap().is_none());
        assert!(dependency.upgrade().is_some());
        drop(retained);
        assert!(dependency.upgrade().is_none());
    }

    #[test]
    fn packed_bundles_open_with_the_loose_identity() {
        for compress in [false, true] {
            packed_bundle_parity(compress);
        }
    }

    fn packed_bundle_parity(compress: bool) {
        use product_container::{write, Body, Bundle, NewEntry};
        let directory = tempfile::tempdir().unwrap();
        fixture(directory.path());
        let out = directory.path().join("product.rpak");
        let entries = vec![
            NewEntry {
                path: "content/rules/a.json".into(),
                bundle: Some("rules".into()),
                body: Body::Bytes(b"{\"id\":1}"),
            },
            NewEntry {
                path: "content/loose.txt".into(),
                bundle: None,
                body: Body::Bytes(b"loose"),
            },
        ];
        let bundles = vec![Bundle {
            id: "rules".into(),
            root: "content/rules".into(),
        }];
        write(&out, entries, bundles, compress).unwrap();
        let packed =
            ProductContentBundles::admit(&ProductSource::open(&out).unwrap(), "content").unwrap();
        let loose = ProductContentBundles::admit(&loose(directory.path()), "").unwrap();
        assert!(packed.owns_path("rules/a.json") && !packed.owns_path("loose.txt"));
        let (packed, loose) = (packed.load("rules").unwrap(), loose.load("rules").unwrap());
        assert_eq!(
            packed.reference("a.json").unwrap().unwrap().path,
            "rules/a.json"
        );
        assert_eq!(
            packed.reference("a.json").unwrap().unwrap().bytes.as_ref(),
            loose.reference("a.json").unwrap().unwrap().bytes.as_ref()
        );
        assert_eq!(
            packed.reference("a.json").unwrap().unwrap().sha256(),
            loose.reference("a.json").unwrap().unwrap().sha256()
        );
    }

    fn diagnostic_code(receipt: NativeOperationErrorReceipt) -> (String, String) {
        assert_eq!(receipt.diagnostics_len, 1);
        let diagnostic = unsafe { *receipt.diagnostics };
        let text = |value: NativeUtf8Slice| unsafe {
            String::from_utf8(std::slice::from_raw_parts(value.bytes, value.len).to_vec()).unwrap()
        };
        (text(diagnostic.code), text(diagnostic.message))
    }

    /// A content directory packed on its own opens at run time as a bundle:
    /// text, bytes and a GLB whose relative image resolves in the same
    /// container, with the same identity raw or compressed; references outlive
    /// the bundle; unreadable containers refuse with codes naming the file.
    #[test]
    fn a_packed_content_container_opens_as_a_bundle_and_bad_containers_refuse() {
        use crate::appearance::tests::RGBA_PNG;
        use crate::render_resources::{tests::external_image_glb, RenderResourceImports};
        let directory = tempfile::tempdir().unwrap();
        let module = directory.path().join("module");
        fs::create_dir_all(module.join("models")).unwrap();
        fs::write(module.join("module.json"), b"{\"id\":\"walls\"}").unwrap();
        fs::write(module.join("general.bin"), [0_u8, 1, 255]).unwrap();
        let character = include_bytes!(
            "../../../../../fixtures/render/assets/kenney-retro-character/character-medium.glb"
        );
        fs::write(
            module.join("models/character.glb"),
            external_image_glb(character, "skin.png"),
        )
        .unwrap();
        fs::write(module.join("models/skin.png"), RGBA_PNG).unwrap();

        let mut bridge = RuntimeContentBridge::new(BTreeMap::new());
        let context = (&mut bridge as *mut RuntimeContentBridge).cast();
        let open = |path: &Path| {
            let path = path.to_str().unwrap();
            let mut bundle = NativeContentBundleHandle::default();
            let mut receipt: NativeOperationErrorReceipt = unsafe { std::mem::zeroed() };
            let status = unsafe {
                open_container(
                    context,
                    &NativeContentContainerOpenRequest { path: text(path) },
                    &mut bundle,
                    &mut receipt,
                )
            };
            (status == ABI_OK)
                .then_some(bundle)
                .ok_or_else(|| diagnostic_code(receipt))
        };
        let pack = |out: &Path, compress: bool| {
            let mut receipt: NativeOperationErrorReceipt = unsafe { std::mem::zeroed() };
            let request = NativeContentContainerPackRequest {
                directory: text(module.to_str().unwrap()),
                output: text(out.to_str().unwrap()),
                compress,
            };
            let status = unsafe { pack_container(context, &request, &mut receipt) };
            (status == ABI_OK)
                .then_some(())
                .ok_or_else(|| diagnostic_code(receipt))
        };
        assert_eq!(
            pack(&module.join("inside.rpak"), false).unwrap_err().0,
            "PRODUCT_PACK_OVERLAP"
        );
        let mut identities = Vec::new();
        for compress in [false, true] {
            let out = directory.path().join(format!("walls-{compress}.rpak"));
            pack(&out, compress).unwrap();
            let bundle = open(&out).unwrap();
            let mut identity = NativeContentSha256::default();
            assert_eq!(
                unsafe { read_bundle_identity(context, bundle, &mut identity) },
                ABI_OK
            );
            identities.push(identity);
            let mut files = NativeContentReferenceInfoResult {
                references: std::ptr::null(),
                references_len: 0,
            };
            assert_eq!(
                unsafe { read_bundle_files(context, bundle, &mut files) },
                ABI_OK
            );
            assert_eq!(files.references_len, 4);
            let reference = |path: &str| {
                let mut reference = NativeContentReferenceHandle::default();
                assert_eq!(
                    unsafe {
                        open_bundle_reference(
                            context,
                            &NativeContentBundleReferenceRequest {
                                bundle,
                                path: text(path),
                            },
                            &mut reference,
                            std::ptr::null_mut(),
                        )
                    },
                    ABI_OK,
                    "{path}"
                );
                reference
            };
            let manifest = reference("module.json");
            let general = reference("general.bin");
            let model = reference("models/character.glb");
            assert_eq!(unsafe { destroy_bundle(context, bundle) }, ABI_OK);
            assert_eq!(
                bridge.retained_bytes(manifest).unwrap().as_ref(),
                b"{\"id\":\"walls\"}"
            );
            assert_eq!(
                bridge.retained_bytes(general).unwrap().as_ref(),
                [0, 1, 255]
            );
            let model = bridge.retained_content(model).unwrap();
            assert_eq!(model.path, "models/character.glb");
            RenderResourceImports::default()
                .animated(model, false)
                .expect("the GLB's relative image resolves in its container");
        }
        assert_eq!(
            identities[0], identities[1],
            "compression changed the identity"
        );

        let packed = fs::read(directory.path().join("walls-false.rpak")).unwrap();
        let truncated = directory.path().join("truncated.rpak");
        fs::write(&truncated, &packed[..packed.len() - 10]).unwrap();
        let corrupt = directory.path().join("corrupt.rpak");
        fs::write(&corrupt, [packed.as_slice(), b"x"].concat()).unwrap();
        let text_file = directory.path().join("notes.txt");
        fs::write(&text_file, b"not a container").unwrap();
        for (path, code) in [
            (&truncated, "PRODUCT_CONTAINER_TRUNCATED"),
            (&corrupt, "PRODUCT_CONTAINER_CORRUPT"),
            (&text_file, "PRODUCT_CONTAINER_NOT_A_CONTAINER"),
            (&directory.path().join("absent.rpak"), "PRODUCT_SOURCE_IO"),
        ] {
            let (actual, message) = open(path).unwrap_err();
            assert_eq!(actual, code);
            assert!(message.contains(path.to_str().unwrap()), "{message}");
        }
        assert!(bridge.bundles.open.is_empty());
    }

    /// A compressed entry that fails to decompress refuses each lazy read
    /// (a reference, a resolve, a dependency lookup) with the container's code
    /// and a message naming the container and the entry.
    #[test]
    fn an_unreadable_container_entry_refuses_with_the_container_code() {
        let directory = tempfile::tempdir().unwrap();
        let module = directory.path().join("module");
        fs::create_dir_all(module.join("data")).unwrap();
        fs::write(module.join("module.json"), b"{\"id\":\"tables\"}").unwrap();
        fs::write(module.join("data/table.json"), "[1,2,3],".repeat(4096)).unwrap();
        let out = directory.path().join("tables.rpak");
        product_container::pack_content(&module, &out, true).unwrap();
        let entry = Container::open(&out)
            .unwrap()
            .entry("data/table.json")
            .unwrap()
            .clone();
        assert!(
            entry.zstd_length.is_some(),
            "the table is stored compressed"
        );
        // Break the zstd frame magic: the header and inventory stay valid.
        let mut bytes = fs::read(&out).unwrap();
        bytes[entry.offset as usize..entry.offset as usize + 4].copy_from_slice(b"XXXX");
        fs::write(&out, bytes).unwrap();

        let mut bridge = RuntimeContentBridge::new(BTreeMap::new());
        let context = (&mut bridge as *mut RuntimeContentBridge).cast();
        let path = out.to_str().unwrap();
        let mut bundle = NativeContentBundleHandle::default();
        let mut receipt: NativeOperationErrorReceipt = unsafe { std::mem::zeroed() };
        assert_eq!(
            unsafe {
                open_container(
                    context,
                    &NativeContentContainerOpenRequest { path: text(path) },
                    &mut bundle,
                    &mut receipt,
                )
            },
            ABI_OK
        );
        let corrupt = |(code, message): (String, String)| {
            assert_eq!(code, "PRODUCT_CONTAINER_CORRUPT");
            assert!(
                message.contains(path) && message.contains("data/table.json"),
                "{message}"
            );
        };
        let open_reference = |file: &str| {
            let mut reference = NativeContentReferenceHandle::default();
            let mut receipt: NativeOperationErrorReceipt = unsafe { std::mem::zeroed() };
            let status = unsafe {
                open_bundle_reference(
                    context,
                    &NativeContentBundleReferenceRequest {
                        bundle,
                        path: text(file),
                    },
                    &mut reference,
                    &mut receipt,
                )
            };
            (status == ABI_OK)
                .then_some(reference)
                .ok_or_else(|| diagnostic_code(receipt))
        };
        corrupt(open_reference("data/table.json").unwrap_err());

        let mut resolved = NativeContentReferenceHandle::default();
        let mut receipt: NativeOperationErrorReceipt = unsafe { std::mem::zeroed() };
        assert_eq!(
            unsafe {
                super::super::resolve_reference(
                    context,
                    &NativeContentResolveRequest {
                        path: text("data/table.json"),
                        sha256: sha256_words(&hex_digest(&entry.sha256).unwrap()),
                    },
                    &mut resolved,
                    &mut receipt,
                )
            },
            0
        );
        corrupt(diagnostic_code(receipt));

        // A readable file's dependency context reports the same refusal.
        let manifest = open_reference("module.json").unwrap();
        let refusal = bridge
            .retained_content(manifest)
            .unwrap()
            .files
            .get("data/table.json")
            .unwrap_err();
        corrupt((refusal.code().to_owned(), refusal.detail().to_owned()));
    }

    #[test]
    fn overlapping_roots_are_rejected_before_loading() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(
            directory.path().join(INDEX),
            br#"{"bundles":[{"id":"a","root":"a","files":[]},{"id":"b","root":"a/b","files":[]}]}"#,
        )
        .unwrap();
        assert!(ProductContentBundles::admit(&loose(directory.path()), "")
            .unwrap_err()
            .contains("overlapping"));
    }
}
