//! Directory-backed build content. Discovery retains metadata only; each open
//! owns an immutable collection, and references own independent Arc clones.
use super::*;
use serde::Deserialize;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub const INDEX: &str = ".rusty-bundles.json";

#[derive(Debug, Default, Deserialize)]
pub struct ProductContentBundles {
    #[serde(skip)]
    content_root: PathBuf,
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

fn relative(path: &str) -> bool {
    !path.is_empty()
        && !path.contains(['\\', ':', '\0'])
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

impl ProductContentBundles {
    /// Read only the SDK-generated inventory; no bundle payload is read here.
    pub fn admit(root: &Path) -> Result<Self, String> {
        let index = root.join(INDEX);
        if !index.exists() {
            return Ok(Self::default());
        }
        let mut source: Self = serde_json::from_slice(&fs::read(index).map_err(|e| e.to_string())?)
            .map_err(|e| format!("invalid ProductContent bundle inventory: {e}"))?;
        source.content_root = root.to_path_buf();
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

    /// Reserved inventory and declared bundle roots are excluded from legacy
    /// eager ProductContent and browser initial-content publication.
    pub fn owns_path(&self, path: &str) -> bool {
        path == INDEX
            || self
                .bundles
                .iter()
                .any(|b| path.starts_with(&format!("{}/", b.root)))
    }

    fn load(&self, id: &str) -> Option<BTreeMap<String, AdmittedContent>> {
        let bundle = self.bundles.iter().find(|b| b.id == id)?;
        let mut bodies = BTreeMap::new();
        let mut hashes = BTreeMap::new();
        for file in &bundle.files {
            let path = self.content_root.join(&bundle.root).join(&file.path);
            let bytes = fs::read(path).ok()?;
            if bytes.len() as u64 != file.byte_length {
                return None;
            }
            let digest = Sha256::digest(&bytes);
            if format!("{digest:x}") != file.sha256.to_ascii_lowercase() {
                return None;
            }
            hashes.insert(file.path.as_str(), sha256_words(&digest));
            bodies.insert(
                format!("{}/{}", bundle.root, file.path),
                Arc::<[u8]>::from(bytes),
            );
        }
        let bodies = Arc::new(bodies);
        Some(
            bundle
                .files
                .iter()
                .map(|file| {
                    let path = format!("{}/{}", bundle.root, file.path);
                    let bytes = Arc::clone(&bodies[&path]);
                    (
                        file.path.clone(),
                        AdmittedContent {
                            path,
                            sha256: hashes[file.path.as_str()],
                            bytes,
                            files: Arc::clone(&bodies),
                        },
                    )
                })
                .collect(),
        )
    }
}

struct BundleInfoLease {
    _ids: Vec<String>,
    infos: Vec<NativeContentBundleInfo>,
}

#[derive(Default)]
pub(super) struct BundleState {
    pub(super) source: ProductContentBundles,
    open: BTreeMap<u64, BTreeMap<String, AdmittedContent>>,
    infos: BTreeMap<u64, BundleInfoLease>,
    next: u64,
}

impl BundleState {
    pub(super) fn resolve(&self, path: &str, hash: NativeContentSha256) -> Option<AdmittedContent> {
        self.open
            .values()
            .flat_map(|files| files.values())
            .find(|file| file.path == path && file.sha256 == hash)
            .cloned()
    }

    fn next(&mut self) -> Option<u64> {
        self.next = self.next.checked_add(1)?;
        Some(self.next)
    }
}

pub(super) unsafe extern "C" fn list_bundles(
    context: *mut c_void,
    result: *mut NativeContentBundleInfoLease,
) -> i32 {
    if context.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeContentBridge>() };
    let Some(value) = bridge.bundles.next() else {
        return 0;
    };
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
        .collect();
    bridge
        .bundles
        .infos
        .insert(value, BundleInfoLease { _ids: ids, infos });
    let lease = &bridge.bundles.infos[&value];
    unsafe {
        *result = NativeContentBundleInfoLease {
            handle: NativeContentBundleInfoLeaseHandle { value },
            bundles: lease.infos.as_ptr(),
            bundles_len: lease.infos.len(),
        };
    }
    ABI_OK
}

pub(super) unsafe extern "C" fn destroy_bundle_info_lease(
    context: *mut c_void,
    handle: NativeContentBundleInfoLeaseHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeContentBridge>() };
    if bridge.bundles.infos.remove(&handle.value).is_some() {
        ABI_OK
    } else {
        0
    }
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
    let Some(files) = bridge.bundles.source.load(id) else {
        return 0;
    };
    let Some(value) = bridge.bundles.next() else {
        return 0;
    };
    bridge.bundles.open.insert(value, files);
    unsafe {
        *result = NativeContentBundleHandle { value };
    }
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
    result: *mut NativeContentReferenceInfoLease,
) -> i32 {
    if context.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeContentBridge>() };
    let Some(files) = bridge.bundles.open.get(&bundle.value) else {
        return 0;
    };
    let entries = files
        .iter()
        .map(|(path, file)| (path.clone(), file.sha256, file.bytes.len() as u64))
        .collect();
    let Some(lease) = bridge.retain_info(entries) else {
        return 0;
    };
    unsafe {
        *result = lease;
    }
    ABI_OK
}

pub(super) unsafe extern "C" fn open_bundle_reference(
    context: *mut c_void,
    request: *const NativeContentBundleReferenceRequest,
    result: *mut NativeContentReferenceHandle,
) -> i32 {
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let request = unsafe { &*request };
    let Ok(path) = (unsafe { borrowed_utf8(request.path.bytes, request.path.len, "bundle file") })
    else {
        return 0;
    };
    let bridge = unsafe { &mut *context.cast::<RuntimeContentBridge>() };
    let Some(file) = bridge
        .bundles
        .open
        .get(&request.bundle.value)
        .and_then(|b| b.get(path))
        .cloned()
    else {
        return 0;
    };
    let Some(handle) = bridge.retain(file) else {
        return 0;
    };
    unsafe {
        *result = handle;
    }
    ABI_OK
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let source = ProductContentBundles::admit(directory.path()).unwrap();
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
        let weak = Arc::downgrade(&bridge.bundles.open[&bundle.value]["a.json"].bytes);
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
        fs::write(directory.path().join("rules/a.json"), b"{\"id\":2}").unwrap();
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
        let source = ProductContentBundles::admit(directory.path()).unwrap();
        let actors = source.load("actors").unwrap();
        let other = source.load("other").unwrap();
        let retained = actors["model.glb"].clone();
        let dependency = Arc::downgrade(&retained.files["actors/skin.png"]);
        drop(actors);
        drop(other);
        assert_eq!(retained.files["actors/skin.png"].as_ref(), b"actors");
        assert!(!retained.files.contains_key("other/skin.png"));
        assert!(dependency.upgrade().is_some());
        drop(retained);
        assert!(dependency.upgrade().is_none());
    }

    #[test]
    fn overlapping_roots_are_rejected_before_loading() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(
            directory.path().join(INDEX),
            br#"{"bundles":[{"id":"a","root":"a","files":[]},{"id":"b","root":"a/b","files":[]}]}"#,
        )
        .unwrap();
        assert!(ProductContentBundles::admit(directory.path())
            .unwrap_err()
            .contains("overlapping"));
    }
}
