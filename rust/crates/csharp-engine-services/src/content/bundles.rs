//! Build content bundles, loose or packed. Discovery retains metadata only;
//! each open owns an immutable collection, and references own independent Arc
//! clones.
use super::*;
use crate::composition::CsharpEngineServicesError;
use product_container::{join, Container, Entry, ProductSource};
use serde::Deserialize;
use std::{path::Path, sync::Mutex};

pub const INDEX: &str = ".rusty-bundles.json";

#[derive(Debug, Default)]
pub struct ProductContentBundles {
    /// Absent only for content with no bundle inventory.
    source: Option<BundleSource>,
    /// The content root within `source`.
    content_root: String,
    bundles: Vec<BundleDefinition>,
}

/// Where bundle files are read from: a staged Product, or the content files a
/// test supplied (`EngineTestHost`), keyed by content-relative path.
#[derive(Debug)]
enum BundleSource {
    Product(ProductSource),
    Supplied(Arc<BTreeMap<String, Arc<[u8]>>>),
}

impl BundleSource {
    fn read(&self, path: &str) -> Option<Arc<[u8]>> {
        match self {
            Self::Product(source) => source
                .read(path)
                .ok()
                .map(|bytes| Arc::from(bytes.as_ref())),
            Self::Supplied(files) => files.get(path).cloned(),
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
        Self::checked(BundleSource::Supplied(Arc::new(files.clone())), "", bundles)
    }

    fn checked(
        source: BundleSource,
        content_root: &str,
        bundles: Vec<BundleDefinition>,
    ) -> Result<Self, String> {
        let mut source = Self {
            source: Some(source),
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

    fn load(&self, id: &str) -> Option<OpenBundle> {
        let bundle = self.bundles.iter().find(|b| b.id == id)?;
        let mut bodies = BTreeMap::new();
        let mut hashes = BTreeMap::new();
        let source = self.source.as_ref()?;
        for file in &bundle.files {
            let path = join(
                &self.content_root,
                &format!("{}/{}", bundle.root, file.path),
            );
            let bytes = source.read(&path)?;
            // Staging wrote the manifest's SHA-256 from these same bytes, so
            // its identity is trusted rather than recomputed on every open. A
            // body changed after staging gets a new identity on the next
            // restage. The length check costs nothing and catches a file cut
            // short by an interrupted restage.
            if bytes.len() as u64 != file.byte_length {
                return None;
            }
            hashes.insert(file.path.as_str(), sha256_words(&hex_digest(&file.sha256)?));
            bodies.insert(format!("{}/{}", bundle.root, file.path), bytes);
        }
        let identities = bundle
            .files
            .iter()
            .map(|file| Some((file.path.as_str(), hex_digest(&file.sha256)?)))
            .collect::<Option<Vec<_>>>()?;
        let snapshot = super::ContentFiles::snapshot(bodies.clone());
        let files = bundle
            .files
            .iter()
            .map(|file| {
                let path = format!("{}/{}", bundle.root, file.path);
                let bytes = Arc::clone(&bodies[&path]);
                (
                    file.path.clone(),
                    AdmittedContent {
                        path,
                        identity: super::ContentIdentity::known(hashes[file.path.as_str()]),
                        bytes,
                        transient: false,
                        files: snapshot.clone(),
                    },
                )
            })
            .collect();
        Some(OpenBundle {
            identity: collection_identity(identities),
            files: OpenFiles::Snapshot(files),
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

/// An open content container. Opening checked its header and inventory; each
/// file's bytes are read once, when first used, and shared from then on.
/// References keep it (and its read-only map) alive after its bundle closes.
pub(crate) struct ContainerFiles {
    container: Container,
    read: Mutex<BTreeMap<String, Arc<[u8]>>>,
}

impl ContainerFiles {
    /// The file's bytes, or none when the container has no such file. A file
    /// that cannot be read (a corrupt compressed entry) refuses with the
    /// container's code, naming the container and the entry.
    pub(crate) fn bytes(&self, path: &str) -> Result<Option<Arc<[u8]>>, CsharpEngineServicesError> {
        let mut read = self
            .read
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if let Some(bytes) = read.get(path) {
            return Ok(Some(Arc::clone(bytes)));
        }
        let bytes: Arc<[u8]> = match self.container.get(path) {
            Ok(bytes) => Arc::from(bytes.as_ref()),
            Err(product_container::Error::Missing(_)) => return Ok(None),
            Err(failure) => return Err(container_error(&failure)),
        };
        read.insert(path.to_owned(), Arc::clone(&bytes));
        Ok(Some(bytes))
    }

    fn content(
        self: &Arc<Self>,
        entry: &Entry,
    ) -> Result<Option<AdmittedContent>, CsharpEngineServicesError> {
        let (Some(digest), Some(bytes)) = (hex_digest(&entry.sha256), self.bytes(&entry.path)?)
        else {
            return Ok(None);
        };
        Ok(Some(AdmittedContent {
            path: entry.path.clone(),
            identity: super::ContentIdentity::known(sha256_words(&digest)),
            bytes,
            transient: false,
            files: super::ContentFiles::Container(Arc::clone(self)),
        }))
    }
}

/// One open bundle: a build bundle read whole, or a container read on use.
struct OpenBundle {
    identity: NativeContentSha256,
    files: OpenFiles,
}

enum OpenFiles {
    Snapshot(BTreeMap<String, AdmittedContent>),
    Container(Arc<ContainerFiles>),
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
            files: OpenFiles::Container(Arc::new(ContainerFiles {
                container,
                read: Mutex::default(),
            })),
        })
    }

    /// Path, SHA-256 and length of every file, in path order.
    fn entries(&self) -> Vec<(String, NativeContentSha256, u64)> {
        match &self.files {
            OpenFiles::Snapshot(files) => files
                .iter()
                .map(|(path, file)| (path.clone(), file.sha256(), file.bytes.len() as u64))
                .collect(),
            OpenFiles::Container(files) => files
                .container
                .entries()
                .iter()
                .filter_map(|entry| {
                    let sha256 = sha256_words(&hex_digest(&entry.sha256)?);
                    Some((entry.path.clone(), sha256, entry.byte_length))
                })
                .collect(),
        }
    }

    fn reference(&self, path: &str) -> Result<Option<AdmittedContent>, CsharpEngineServicesError> {
        match &self.files {
            OpenFiles::Snapshot(files) => Ok(files.get(path).cloned()),
            OpenFiles::Container(files) => match files.container.entry(path) {
                Some(entry) => files.content(entry),
                None => Ok(None),
            },
        }
    }

    fn resolve(
        &self,
        path: &str,
        hash: NativeContentSha256,
    ) -> Result<Option<AdmittedContent>, CsharpEngineServicesError> {
        match &self.files {
            OpenFiles::Snapshot(files) => Ok(files
                .values()
                .find(|file| file.path == path && file.sha256() == hash)
                .cloned()),
            OpenFiles::Container(files) => match files.container.entry(path) {
                Some(entry)
                    if hex_digest(&entry.sha256).map(|digest| sha256_words(&digest))
                        == Some(hash) =>
                {
                    files.content(entry)
                }
                _ => Ok(None),
            },
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
        // implicit catalog mount and no reads of the unused bundle.
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
        assert_eq!(
            unsafe {
                open_bundle(
                    context,
                    &NativeContentBundleOpenRequest { id: text("unused") },
                    &mut bundle,
                )
            },
            0
        );
        assert!(bridge.bundles.open.is_empty());
        // Opening trusts the staged manifest identity instead of re-hashing
        // every file: a same-length edit after staging opens under the old
        // identity until the next restage, while a file of the wrong length
        // (for example cut short) is refused.
        fs::write(directory.path().join("rules/a.json"), b"{\"id\":2}").unwrap();
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
        fs::write(directory.path().join("rules/a.json"), b"{\"id\":22}").unwrap();
        assert_eq!(
            unsafe {
                open_bundle(
                    context,
                    &NativeContentBundleOpenRequest { id: text("rules") },
                    &mut bundle,
                )
            },
            0
        );
        assert!(bridge.bundles.open.is_empty());
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
